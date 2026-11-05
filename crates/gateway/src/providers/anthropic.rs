//! Anthropic Messages API adapter (raw HTTPS + SSE; there is no official Rust SDK).
//!
//! - API version `2026-10-21`: no sampling parameters are sent.
//! - Effort goes in `output_config.effort`; thinking stays on its default, and
//!   thinking blocks are ignored when streaming (only text reaches the UI).
//! - Server-side refusal fallback (`fallbacks: "default"`) is enabled for models
//!   that support it, so a declined request is retried on another model in the
//!   same call. A final `refusal` stop reason is surfaced as `FinishReason::Refusal`.

use futures::StreamExt;
use serde_json::{Value, json};
use sidekick_core::llm::{
    Effort, FinishReason, LlmProvider, LlmRequest, ModelInfo, Role, StreamEvent, TokenStream, Usage,
};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_secrets::{ExposeSecret, SecretString};

use super::http::{status_error, transport_error};
use crate::sse::SseParser;

pub const API_VERSION: &str = "2026-10-21";
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
/// Main answers (tech doc 3.3).
pub const DEFAULT_ANSWER_MODEL: &str = "claude-opus-5-5";
/// One-line cues: small and fast (PRD MOD-3).
pub const DEFAULT_CUE_MODEL: &str = "claude-haiku-5-5";
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
const NAME: &str = "Anthropic";

#[derive(Debug, Clone)]
pub struct AnthropicProvider {
    base_url: String,
    http: reqwest::Client,
}

impl Default for AnthropicProvider {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

impl AnthropicProvider {
    /// `base_url` is overridable so tests can point at a mock server.
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            http: reqwest::Client::new(),
        }
    }

    fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        key: &SecretString,
    ) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}{path}", self.base_url))
            .header("x-api-key", key.expose_secret())
            .header("anthropic-version", API_VERSION)
    }
}

/// Models that accept `fallbacks: "default"` on the Claude API.
fn supports_server_fallback(model: &str) -> bool {
    matches!(
        model,
        "claude-opus-5-5" | "claude-opus-5" | "claude-fable-5-1" | "claude-sonnet-5-5"
    )
}

/// Builds the Messages API body. Pure, so it can be snapshot-tested.
#[must_use]
pub fn build_body(req: &LlmRequest) -> Value {
    let messages: Vec<Value> = req
        .messages
        .iter()
        .map(|m| {
            let role = match m.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            };
            match &m.image {
                // Image first, then the question about it (vision best practice).
                Some(img) => json!({ "role": role, "content": [
                    { "type": "image", "source": {
                        "type": "base64", "media_type": img.mime_type, "data": img.base64 } },
                    { "type": "text", "text": m.content }
                ] }),
                None => json!({ "role": role, "content": m.content }),
            }
        })
        .collect();
    let mut body = json!({
        "model": req.model,
        "max_tokens": req.max_tokens,
        "stream": true,
        "messages": messages,
    });
    if let Some(system) = &req.system {
        body["system"] = json!(system);
    }
    if let Some(effort) = req.effort {
        let level = match effort {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
        };
        body["output_config"] = json!({ "effort": level });
    }
    if supports_server_fallback(&req.model) {
        body["fallbacks"] = json!("default");
    }
    body
}

/// Folds one SSE event into the stream state; returns an event to emit, if any.
#[derive(Debug, Default)]
pub struct StreamState {
    usage: Usage,
    finish: Option<FinishReason>,
    /// Indexes of content blocks that are text (others, like thinking, are skipped).
    text_blocks: Vec<u64>,
}

impl StreamState {
    pub fn on_event(
        &mut self,
        event: Option<&str>,
        data: &str,
    ) -> SidekickResult<Option<StreamEvent>> {
        let v: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => return Ok(None), // Unknown or partial payloads are ignored, not fatal.
        };
        let kind = v["type"].as_str().or(event).unwrap_or_default();
        match kind {
            "message_start" => {
                self.usage.input_tokens = u32_at(&v["message"]["usage"]["input_tokens"]);
            }
            "content_block_start" => {
                if v["content_block"]["type"] == "text"
                    && let Some(i) = v["index"].as_u64()
                {
                    self.text_blocks.push(i);
                }
            }
            "content_block_delta" => {
                let is_text_block = v["index"]
                    .as_u64()
                    .is_some_and(|i| self.text_blocks.contains(&i));
                if is_text_block
                    && v["delta"]["type"] == "text_delta"
                    && let Some(text) = v["delta"]["text"].as_str()
                {
                    return Ok(Some(StreamEvent::Delta(text.to_owned())));
                }
            }
            "message_delta" => {
                self.usage.output_tokens = u32_at(&v["usage"]["output_tokens"]);
                if let Some(reason) = v["delta"]["stop_reason"].as_str() {
                    self.finish = Some(match reason {
                        "end_turn" | "stop_sequence" => FinishReason::EndTurn,
                        "max_tokens" => FinishReason::MaxTokens,
                        "refusal" => FinishReason::Refusal,
                        "tool_use" => FinishReason::ToolUse,
                        _ => FinishReason::Other,
                    });
                }
            }
            "message_stop" => {
                return Ok(Some(StreamEvent::Done {
                    usage: self.usage,
                    finish_reason: self.finish.unwrap_or(FinishReason::EndTurn),
                }));
            }
            "error" => {
                let code = match v["error"]["type"].as_str() {
                    Some("rate_limit_error") => ErrorCode::RateLimited,
                    Some("authentication_error" | "permission_error") => ErrorCode::KeyInvalid,
                    Some("request_too_large") => ErrorCode::ContextTooLarge,
                    _ => ErrorCode::ProviderUnavailable,
                };
                return Err(SidekickError::new(
                    code,
                    "Anthropic stopped the answer with an error.",
                ));
            }
            _ => {} // ping, content_block_stop, thinking deltas, fallback blocks...
        }
        Ok(None)
    }
}

fn u32_at(v: &Value) -> u32 {
    v.as_u64().and_then(|n| u32::try_from(n).ok()).unwrap_or(0)
}

fn require_key(key: Option<&SecretString>) -> SidekickResult<&SecretString> {
    key.ok_or_else(|| {
        SidekickError::new(
            ErrorCode::KeyMissing,
            "Add your Anthropic API key in Settings.",
        )
    })
}

async fn check_status(resp: reqwest::Response) -> SidekickResult<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let retry_after = resp
        .headers()
        .get("retry-after")
        .and_then(|h| h.to_str().ok())
        .map(str::to_owned);
    Err(status_error(
        NAME,
        resp.status().as_u16(),
        retry_after.as_deref(),
    ))
}

#[async_trait::async_trait]
impl LlmProvider for AnthropicProvider {
    fn id(&self) -> &'static str {
        "anthropic"
    }

    async fn list_models(&self, key: Option<&SecretString>) -> SidekickResult<Vec<ModelInfo>> {
        let key = require_key(key)?;
        let resp = self
            .request(reqwest::Method::GET, "/v1/models?limit=100", key)
            .send()
            .await
            .map_err(|e| transport_error(NAME, &e))?;
        let body: Value = check_status(resp).await?.json().await.map_err(|_| {
            SidekickError::new(
                ErrorCode::ProviderUnavailable,
                "Unexpected reply from Anthropic.",
            )
        })?;
        Ok(body["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| {
                Some(ModelInfo {
                    id: m["id"].as_str()?.to_owned(),
                    display_name: m["display_name"]
                        .as_str()
                        .unwrap_or_else(|| m["id"].as_str().unwrap_or(""))
                        .to_owned(),
                })
            })
            .collect())
    }

    async fn stream(
        &self,
        req: LlmRequest,
        key: Option<&SecretString>,
    ) -> SidekickResult<TokenStream> {
        let key = require_key(key)?;
        let mut builder = self
            .request(reqwest::Method::POST, "/v1/messages", key)
            .json(&build_body(&req));
        if supports_server_fallback(&req.model) {
            builder = builder.header("anthropic-beta", FALLBACK_BETA);
        }
        let resp = builder
            .send()
            .await
            .map_err(|e| transport_error(NAME, &e))?;
        let resp = check_status(resp).await?;

        let mut bytes = resp.bytes_stream();
        let stream = async_stream(move |tx| async move {
            let mut parser = SseParser::default();
            let mut state = StreamState::default();
            while let Some(chunk) = bytes.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(Err(transport_error(NAME, &e))).await;
                        return;
                    }
                };
                for ev in parser.push(&String::from_utf8_lossy(&chunk)) {
                    match state.on_event(ev.event.as_deref(), &ev.data) {
                        Ok(Some(out)) => {
                            let done = matches!(out, StreamEvent::Done { .. });
                            if tx.send(Ok(out)).await.is_err() || done {
                                return;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            let _ = tx.send(Err(e)).await;
                            return;
                        }
                    }
                }
            }
            let _ = tx
                .send(Err(SidekickError::new(
                    ErrorCode::ProviderUnavailable,
                    "The answer stream ended early.",
                )))
                .await;
        });
        Ok(stream)
    }
}

/// Turns a producer task into a `TokenStream` via a small channel.
pub(crate) fn async_stream<F, Fut>(producer: F) -> TokenStream
where
    F: FnOnce(tokio::sync::mpsc::Sender<SidekickResult<StreamEvent>>) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let (tx, rx) = tokio::sync::mpsc::channel(64);
    tokio::spawn(producer(tx));
    Box::pin(futures::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidekick_core::llm::ChatMessage;

    fn request(model: &str) -> LlmRequest {
        LlmRequest {
            model: model.into(),
            system: Some("Be brief.".into()),
            messages: vec![
                ChatMessage::text(Role::User, "Rate limit?"),
                ChatMessage::text(Role::Assistant, "Which tier?"),
                ChatMessage::text(Role::User, "Pro"),
            ],
            max_tokens: 1024,
            effort: Some(Effort::Low),
        }
    }

    #[test]
    fn builds_the_messages_body() {
        let body = build_body(&request(DEFAULT_CUE_MODEL));
        assert_eq!(
            body,
            json!({
                "model": "claude-haiku-5-5",
                "max_tokens": 1024,
                "stream": true,
                "system": "Be brief.",
                "output_config": { "effort": "low" },
                "messages": [
                    { "role": "user", "content": "Rate limit?" },
                    { "role": "assistant", "content": "Which tier?" },
                    { "role": "user", "content": "Pro" }
                ]
            })
        );
    }

    #[test]
    fn sends_a_screenshot_as_an_image_block_before_the_text() {
        let mut req = request(DEFAULT_ANSWER_MODEL);
        req.messages = vec![ChatMessage {
            image: Some(sidekick_core::llm::ImageData {
                mime_type: "image/jpeg".into(),
                base64: "AAAA".into(),
            }),
            ..ChatMessage::text(Role::User, "What does this error mean?")
        }];
        let body = build_body(&req);
        assert_eq!(
            body["messages"][0]["content"],
            json!([
                { "type": "image", "source": { "type": "base64", "media_type": "image/jpeg", "data": "AAAA" } },
                { "type": "text", "text": "What does this error mean?" }
            ])
        );
    }

    #[test]
    fn never_sends_sampling_parameters() {
        let body = build_body(&request(DEFAULT_ANSWER_MODEL));
        for field in ["temperature", "top_p", "top_k"] {
            assert!(
                body.get(field).is_none(),
                "{field} must not be sent on API 2026-10-21"
            );
        }
    }

    #[test]
    fn enables_refusal_fallback_only_where_supported() {
        assert_eq!(
            build_body(&request("claude-opus-5-5"))["fallbacks"],
            "default"
        );
        assert!(
            build_body(&request("claude-haiku-5-5"))
                .get("fallbacks")
                .is_none()
        );
    }

    fn run(events: &[(&str, &str)]) -> Vec<SidekickResult<Option<StreamEvent>>> {
        let mut state = StreamState::default();
        events
            .iter()
            .map(|(e, d)| state.on_event(Some(e), d))
            .collect()
    }

    #[test]
    fn parses_a_text_stream_and_skips_thinking() {
        let out = run(&[
            (
                "message_start",
                r#"{"type":"message_start","message":{"usage":{"input_tokens":3120}}}"#,
            ),
            (
                "content_block_start",
                r#"{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}"#,
            ),
            (
                "content_block_delta",
                r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hidden"}}"#,
            ),
            (
                "content_block_start",
                r#"{"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}"#,
            ),
            (
                "content_block_delta",
                r#"{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Pro tier: "}}"#,
            ),
            ("ping", r#"{"type":"ping"}"#),
            (
                "content_block_delta",
                r#"{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"600 requests/min"}}"#,
            ),
            (
                "message_delta",
                r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":142}}"#,
            ),
            ("message_stop", r#"{"type":"message_stop"}"#),
        ]);
        let emitted: Vec<StreamEvent> = out.into_iter().filter_map(|r| r.unwrap()).collect();
        assert_eq!(
            emitted,
            vec![
                StreamEvent::Delta("Pro tier: ".into()),
                StreamEvent::Delta("600 requests/min".into()),
                StreamEvent::Done {
                    usage: Usage {
                        input_tokens: 3120,
                        output_tokens: 142
                    },
                    finish_reason: FinishReason::EndTurn
                },
            ]
        );
    }

    #[test]
    fn reports_refusal() {
        let out = run(&[
            (
                "message_delta",
                r#"{"type":"message_delta","delta":{"stop_reason":"refusal"},"usage":{"output_tokens":0}}"#,
            ),
            ("message_stop", r#"{"type":"message_stop"}"#),
        ]);
        assert!(matches!(
            out.last().unwrap().as_ref().unwrap(),
            Some(StreamEvent::Done {
                finish_reason: FinishReason::Refusal,
                ..
            })
        ));
    }

    #[test]
    fn maps_in_stream_errors() {
        let out = run(&[(
            "error",
            r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#,
        )]);
        let err = out[0].as_ref().unwrap_err();
        assert_eq!(err.code, ErrorCode::ProviderUnavailable);
        assert!(
            !err.message.contains("Overloaded"),
            "provider text is not echoed"
        );
    }
}
