//! OpenAI Chat Completions adapter (`POST /v1/chat/completions`, SSE).
//!
//! - Images go as `image_url` parts with a `data:` URL.
//! - Effort maps to `reasoning_effort` for reasoning models only (GPT-5 and
//!   o-series); other models reject it.
//! - `stream_options.include_usage` adds token counts in the last chunk.

use serde_json::{Value, json};
use sidekick_core::llm::{
    Effort, FinishReason, LlmProvider, LlmRequest, ModelInfo, Role, StreamEvent, TokenStream, Usage,
};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_secrets::{ExposeSecret, SecretString};

use super::http::{status_error, transport_error};
use super::pump::{Framing, Handler, pump};

pub use super::openai::DEFAULT_BASE_URL;
/// Used when the user hasn't picked a model (`"default"`).
pub const DEFAULT_MODEL: &str = "gpt-5";
const NAME: &str = "OpenAI";

#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    base_url: String,
    http: reqwest::Client,
}

impl Default for OpenAiProvider {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

impl OpenAiProvider {
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            http: reqwest::Client::new(),
        }
    }
}

fn is_reasoning_model(model: &str) -> bool {
    model.starts_with("gpt-5")
        || ["o1", "o3", "o4"]
            .iter()
            .any(|p| model == *p || model.starts_with(&format!("{p}-")))
}

/// Chat models only: the model list also has embeddings, audio, images...
fn is_chat_model(id: &str) -> bool {
    let chat_family =
        id.starts_with("gpt-") || id.starts_with("chatgpt-") || is_reasoning_model(id);
    let excluded = [
        "audio",
        "realtime",
        "tts",
        "transcribe",
        "image",
        "embedding",
        "search",
        "instruct",
    ];
    chat_family && !excluded.iter().any(|x| id.contains(x))
}

/// Builds the request body. Pure, so it can be snapshot-tested.
#[must_use]
pub fn build_body(req: &LlmRequest) -> Value {
    let model = if req.model == "default" {
        DEFAULT_MODEL
    } else {
        req.model.as_str()
    };
    let mut messages = Vec::with_capacity(req.messages.len() + 1);
    if let Some(system) = &req.system {
        messages.push(json!({ "role": "system", "content": system }));
    }
    for m in &req.messages {
        let role = match m.role {
            Role::User => "user",
            Role::Assistant => "assistant",
        };
        messages.push(match &m.image {
            Some(img) => json!({ "role": role, "content": [
                { "type": "image_url", "image_url": {
                    "url": format!("data:{};base64,{}", img.mime_type, img.base64) } },
                { "type": "text", "text": m.content }
            ] }),
            None => json!({ "role": role, "content": m.content }),
        });
    }
    let mut body = json!({
        "model": model,
        "messages": messages,
        "max_completion_tokens": req.max_tokens,
        "stream": true,
        "stream_options": { "include_usage": true },
    });
    if let Some(effort) = req.effort
        && is_reasoning_model(model)
    {
        body["reasoning_effort"] = json!(match effort {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
        });
    }
    body
}

/// Parses chat completion chunks.
#[derive(Debug, Default)]
pub struct ChunkState {
    usage: Usage,
    finish: Option<FinishReason>,
}

impl Handler for ChunkState {
    fn on_message(
        &mut self,
        _event: Option<&str>,
        data: &str,
    ) -> SidekickResult<Option<StreamEvent>> {
        if data.trim() == "[DONE]" {
            return Ok(Some(StreamEvent::Done {
                usage: self.usage,
                finish_reason: self.finish.unwrap_or(FinishReason::EndTurn),
            }));
        }
        let Ok(v) = serde_json::from_str::<Value>(data) else {
            return Ok(None);
        };
        if v.get("error").is_some_and(|e| !e.is_null()) {
            let code = match v["error"]["code"].as_str().or(v["error"]["type"].as_str()) {
                Some("rate_limit_exceeded" | "insufficient_quota") => ErrorCode::RateLimited,
                Some("invalid_api_key") => ErrorCode::KeyInvalid,
                Some("context_length_exceeded") => ErrorCode::ContextTooLarge,
                _ => ErrorCode::ProviderUnavailable,
            };
            return Err(SidekickError::new(
                code,
                "OpenAI stopped the answer with an error.",
            ));
        }
        if let Some(u) = v.get("usage").filter(|u| !u.is_null()) {
            self.usage = Usage {
                input_tokens: u32_at(&u["prompt_tokens"]),
                output_tokens: u32_at(&u["completion_tokens"]),
            };
        }
        let choice = &v["choices"][0];
        // A refusal stays a refusal even when the stream then reports "stop".
        if let Some(reason) = choice["finish_reason"].as_str()
            && self.finish != Some(FinishReason::Refusal)
        {
            self.finish = Some(match reason {
                "stop" => FinishReason::EndTurn,
                "length" => FinishReason::MaxTokens,
                "content_filter" => FinishReason::Refusal,
                "tool_calls" | "function_call" => FinishReason::ToolUse,
                _ => FinishReason::Other,
            });
        }
        if choice["delta"]["refusal"]
            .as_str()
            .is_some_and(|r| !r.is_empty())
        {
            self.finish = Some(FinishReason::Refusal);
        }
        Ok(choice["delta"]["content"]
            .as_str()
            .filter(|t| !t.is_empty())
            .map(|t| StreamEvent::Delta(t.to_owned())))
    }
}

pub(crate) fn u32_at(v: &Value) -> u32 {
    v.as_u64().and_then(|n| u32::try_from(n).ok()).unwrap_or(0)
}

fn require_key(key: Option<&SecretString>) -> SidekickResult<&SecretString> {
    key.ok_or_else(|| {
        SidekickError::new(
            ErrorCode::KeyMissing,
            "Add your OpenAI API key in Settings.",
        )
    })
}

pub(crate) async fn check_status(
    provider: &str,
    resp: reqwest::Response,
) -> SidekickResult<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let retry_after = resp
        .headers()
        .get("retry-after")
        .and_then(|h| h.to_str().ok())
        .map(str::to_owned);
    Err(status_error(
        provider,
        resp.status().as_u16(),
        retry_after.as_deref(),
    ))
}

fn unexpected() -> SidekickError {
    SidekickError::new(
        ErrorCode::ProviderUnavailable,
        "Unexpected reply from OpenAI.",
    )
}

#[async_trait::async_trait]
impl LlmProvider for OpenAiProvider {
    fn id(&self) -> &'static str {
        "openai"
    }

    async fn list_models(&self, key: Option<&SecretString>) -> SidekickResult<Vec<ModelInfo>> {
        let key = require_key(key)?;
        let resp = self
            .http
            .get(format!("{}/v1/models", self.base_url))
            .bearer_auth(key.expose_secret())
            .send()
            .await
            .map_err(|e| transport_error(NAME, &e))?;
        let body: Value = check_status(NAME, resp)
            .await?
            .json()
            .await
            .map_err(|_| unexpected())?;
        let mut models: Vec<ModelInfo> = body["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| m["id"].as_str())
            .filter(|id| is_chat_model(id))
            .map(|id| ModelInfo {
                id: id.to_owned(),
                display_name: id.to_owned(),
            })
            .collect();
        models.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(models)
    }

    async fn stream(
        &self,
        req: LlmRequest,
        key: Option<&SecretString>,
    ) -> SidekickResult<TokenStream> {
        let key = require_key(key)?;
        let resp = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(key.expose_secret())
            .json(&build_body(&req))
            .send()
            .await
            .map_err(|e| transport_error(NAME, &e))?;
        let resp = check_status(NAME, resp).await?;
        Ok(pump(resp, NAME, Framing::Sse, ChunkState::default()))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use sidekick_core::llm::{ChatMessage, ImageData};

    fn request(model: &str) -> LlmRequest {
        LlmRequest {
            model: model.into(),
            system: Some("Be brief.".into()),
            messages: vec![
                ChatMessage::text(Role::User, "Rate limit?"),
                ChatMessage::text(Role::Assistant, "Which tier?"),
                ChatMessage {
                    image: Some(ImageData {
                        mime_type: "image/jpeg".into(),
                        base64: "AAAA".into(),
                    }),
                    ..ChatMessage::text(Role::User, "Pro")
                },
            ],
            max_tokens: 500,
            effort: Some(Effort::Low),
        }
    }

    #[test]
    fn builds_the_chat_body_with_system_image_and_effort() {
        assert_eq!(
            build_body(&request("gpt-5-mini")),
            json!({
                "model": "gpt-5-mini",
                "messages": [
                    { "role": "system", "content": "Be brief." },
                    { "role": "user", "content": "Rate limit?" },
                    { "role": "assistant", "content": "Which tier?" },
                    { "role": "user", "content": [
                        { "type": "image_url", "image_url": { "url": "data:image/jpeg;base64,AAAA" } },
                        { "type": "text", "text": "Pro" }
                    ] }
                ],
                "max_completion_tokens": 500,
                "stream": true,
                "stream_options": { "include_usage": true },
                "reasoning_effort": "low"
            })
        );
    }

    #[test]
    fn effort_only_for_reasoning_models_and_default_model() {
        assert!(
            build_body(&request("gpt-4o"))
                .get("reasoning_effort")
                .is_none()
        );
        assert!(build_body(&request("o3")).get("reasoning_effort").is_some());
        assert!(
            build_body(&request("o3-mini"))
                .get("reasoning_effort")
                .is_some()
        );
        assert!(
            build_body(&request("omni-x"))
                .get("reasoning_effort")
                .is_none()
        );
        assert_eq!(build_body(&request("default"))["model"], DEFAULT_MODEL);
    }

    #[test]
    fn filters_the_model_list_to_chat_models() {
        for id in ["gpt-5", "gpt-4o", "o3", "o4-mini", "chatgpt-4o-latest"] {
            assert!(is_chat_model(id), "{id}");
        }
        for id in [
            "text-embedding-3-small",
            "gpt-4o-audio-preview",
            "gpt-image-1",
            "tts-1",
            "whisper-1",
            "gpt-4o-realtime-preview",
            "dall-e-3",
        ] {
            assert!(!is_chat_model(id), "{id}");
        }
    }

    fn run(chunks: &[&str]) -> Vec<SidekickResult<Option<StreamEvent>>> {
        let mut s = ChunkState::default();
        chunks.iter().map(|c| s.on_message(None, c)).collect()
    }

    #[test]
    fn parses_a_recorded_stream() {
        let fixture = include_str!("../../tests/fixtures/openai/stream.sse");
        let mut parser = crate::sse::SseParser::default();
        let mut state = ChunkState::default();
        let mut text = String::new();
        let mut done = None;
        for ev in parser.push(fixture) {
            match state.on_message(ev.event.as_deref(), &ev.data).unwrap() {
                Some(StreamEvent::Delta(t)) => text.push_str(&t),
                Some(d @ StreamEvent::Done { .. }) => done = Some(d),
                None => {}
            }
        }
        assert_eq!(text, "The Pro tier allows 600 requests per minute [1].");
        assert_eq!(
            done,
            Some(StreamEvent::Done {
                usage: Usage {
                    input_tokens: 812,
                    output_tokens: 14
                },
                finish_reason: FinishReason::EndTurn
            })
        );
    }

    #[test]
    fn maps_finish_reasons_and_errors() {
        let out = run(&[
            r#"{"choices":[{"delta":{"refusal":"I can't help with that."},"finish_reason":null}]}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
            "[DONE]",
        ]);
        assert!(matches!(
            out[2],
            Ok(Some(StreamEvent::Done {
                finish_reason: FinishReason::Refusal,
                ..
            }))
        ));
        let length = run(&[
            r#"{"choices":[{"delta":{},"finish_reason":"length"}]}"#,
            "[DONE]",
        ]);
        assert!(matches!(
            length[1],
            Ok(Some(StreamEvent::Done {
                finish_reason: FinishReason::MaxTokens,
                ..
            }))
        ));
        let err = run(&[
            r#"{"error":{"message":"slow down","type":"requests","code":"rate_limit_exceeded"}}"#,
        ]);
        assert_eq!(err[0].as_ref().unwrap_err().code, ErrorCode::RateLimited);
        assert!(run(&["not json"])[0].as_ref().unwrap().is_none());
    }
}
