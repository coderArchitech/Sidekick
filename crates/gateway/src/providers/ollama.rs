//! Ollama adapter for local models (`POST /api/chat`, newline-delimited JSON).
//!
//! - No key. The server is `OLLAMA_HOST` if set, else `http://localhost:11434`.
//! - Images go in the message's `images` array (base64), for vision models.
//! - `"default"` picks the first installed model.

use serde_json::{Value, json};
use sidekick_core::llm::{
    FinishReason, LlmProvider, LlmRequest, ModelInfo, Role, StreamEvent, TokenStream, Usage,
};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_secrets::SecretString;

use super::http::status_error;
use super::openai_chat::u32_at;
use super::pump::{Framing, Handler, pump};

pub const DEFAULT_BASE_URL: &str = "http://localhost:11434";
const NAME: &str = "Ollama";

#[derive(Debug, Clone)]
pub struct OllamaProvider {
    base_url: String,
    http: reqwest::Client,
}

impl Default for OllamaProvider {
    fn default() -> Self {
        let host = std::env::var("OLLAMA_HOST")
            .ok()
            .filter(|h| !h.trim().is_empty())
            .map(|h| {
                if h.starts_with("http://") || h.starts_with("https://") {
                    h
                } else {
                    format!("http://{h}")
                }
            });
        Self::new(host.as_deref().unwrap_or(DEFAULT_BASE_URL))
    }
}

impl OllamaProvider {
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            http: reqwest::Client::new(),
        }
    }

    /// Network failure: almost always "Ollama isn't running".
    fn unreachable(&self, err: &reqwest::Error) -> SidekickError {
        tracing::warn!(target: "gateway", provider = NAME, connect = err.is_connect(), "transport error");
        SidekickError::new(
            ErrorCode::ProviderUnavailable,
            format!(
                "Couldn't reach Ollama at {}. Start Ollama (or check OLLAMA_HOST) and try again.",
                self.base_url
            ),
        )
    }

    async fn tags(&self) -> SidekickResult<Vec<String>> {
        let resp = self
            .http
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .map_err(|e| self.unreachable(&e))?;
        let body: Value = check_status(resp, None).await?.json().await.map_err(|_| {
            SidekickError::new(
                ErrorCode::ProviderUnavailable,
                "Unexpected reply from Ollama.",
            )
        })?;
        Ok(body["models"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| m["name"].as_str().map(str::to_owned))
            .collect())
    }
}

/// Builds the request body. Pure, so it can be snapshot-tested.
#[must_use]
pub fn build_body(req: &LlmRequest, model: &str) -> Value {
    let mut messages = Vec::with_capacity(req.messages.len() + 1);
    if let Some(system) = &req.system {
        messages.push(json!({ "role": "system", "content": system }));
    }
    for m in &req.messages {
        let role = match m.role {
            Role::User => "user",
            Role::Assistant => "assistant",
        };
        let mut msg = json!({ "role": role, "content": m.content });
        if let Some(img) = &m.image {
            msg["images"] = json!([img.base64]);
        }
        messages.push(msg);
    }
    json!({
        "model": model,
        "messages": messages,
        "stream": true,
        "options": { "num_predict": req.max_tokens },
    })
}

#[derive(Debug, Default)]
pub struct LineState;

impl Handler for LineState {
    fn on_message(
        &mut self,
        _event: Option<&str>,
        data: &str,
    ) -> SidekickResult<Option<StreamEvent>> {
        let Ok(v) = serde_json::from_str::<Value>(data) else {
            return Ok(None);
        };
        if let Some(err) = v["error"].as_str() {
            tracing::warn!(target: "gateway", provider = NAME, len = err.len(), "stream error");
            return Err(SidekickError::new(
                ErrorCode::ProviderUnavailable,
                "Ollama stopped the answer with an error.",
            ));
        }
        if v["done"] == json!(true) {
            let finish_reason = match v["done_reason"].as_str() {
                Some("stop") | None => FinishReason::EndTurn,
                Some("length") => FinishReason::MaxTokens,
                Some(_) => FinishReason::Other,
            };
            return Ok(Some(StreamEvent::Done {
                usage: Usage {
                    input_tokens: u32_at(&v["prompt_eval_count"]),
                    output_tokens: u32_at(&v["eval_count"]),
                },
                finish_reason,
            }));
        }
        Ok(v["message"]["content"]
            .as_str()
            .filter(|t| !t.is_empty())
            .map(|t| StreamEvent::Delta(t.to_owned())))
    }
}

async fn check_status(
    resp: reqwest::Response,
    model: Option<&str>,
) -> SidekickResult<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    if resp.status().as_u16() == 404
        && let Some(model) = model
    {
        return Err(SidekickError::new(
            ErrorCode::Internal,
            format!(
                "Ollama doesn't have the model {model}. Run `ollama pull {model}` or pick another model in Settings."
            ),
        ));
    }
    Err(status_error(NAME, resp.status().as_u16(), None))
}

#[async_trait::async_trait]
impl LlmProvider for OllamaProvider {
    fn id(&self) -> &'static str {
        "ollama"
    }

    async fn list_models(&self, _key: Option<&SecretString>) -> SidekickResult<Vec<ModelInfo>> {
        Ok(self
            .tags()
            .await?
            .into_iter()
            .map(|id| ModelInfo {
                display_name: id.clone(),
                id,
            })
            .collect())
    }

    async fn stream(
        &self,
        req: LlmRequest,
        _key: Option<&SecretString>,
    ) -> SidekickResult<TokenStream> {
        let model = if req.model == "default" {
            self.tags().await?.into_iter().next().ok_or_else(|| {
                SidekickError::new(
                    ErrorCode::Internal,
                    "Ollama has no models yet. Run `ollama pull llama3.2` (or a vision model like `llava` for screenshots).",
                )
            })?
        } else {
            req.model.clone()
        };
        let resp = self
            .http
            .post(format!("{}/api/chat", self.base_url))
            .json(&build_body(&req, &model))
            .send()
            .await
            .map_err(|e| self.unreachable(&e))?;
        let resp = check_status(resp, Some(&model)).await?;
        Ok(pump(resp, NAME, Framing::Lines, LineState))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use sidekick_core::llm::{ChatMessage, ImageData};

    #[test]
    fn builds_the_chat_body_with_images() {
        let req = LlmRequest {
            model: "llava".into(),
            system: Some("Be brief.".into()),
            messages: vec![ChatMessage {
                image: Some(ImageData {
                    mime_type: "image/jpeg".into(),
                    base64: "AAAA".into(),
                }),
                ..ChatMessage::text(Role::User, "What's this?")
            }],
            max_tokens: 80,
            effort: None,
        };
        assert_eq!(
            build_body(&req, "llava"),
            json!({
                "model": "llava",
                "messages": [
                    { "role": "system", "content": "Be brief." },
                    { "role": "user", "content": "What's this?", "images": ["AAAA"] }
                ],
                "stream": true,
                "options": { "num_predict": 80 }
            })
        );
    }

    #[test]
    fn parses_a_recorded_stream() {
        let fixture = include_str!("../../tests/fixtures/ollama/chat.ndjson");
        let mut s = LineState;
        let mut text = String::new();
        let mut done = None;
        for line in fixture.lines().filter(|l| !l.is_empty()) {
            match s.on_message(None, line).unwrap() {
                Some(StreamEvent::Delta(t)) => text.push_str(&t),
                Some(d) => done = Some(d),
                None => {}
            }
        }
        assert_eq!(text, "The Pro tier allows 600 requests per minute [1].");
        assert_eq!(
            done,
            Some(StreamEvent::Done {
                usage: Usage {
                    input_tokens: 640,
                    output_tokens: 13
                },
                finish_reason: FinishReason::EndTurn
            })
        );
        let err = s
            .on_message(None, r#"{"error":"model runner crashed"}"#)
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::ProviderUnavailable);
    }

    #[tokio::test]
    async fn unreachable_ollama_has_a_helpful_message() {
        // Port 9 (discard) on localhost: nothing listens there in CI.
        let p = OllamaProvider::new("http://127.0.0.1:9");
        let err = p.list_models(None).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::ProviderUnavailable);
        assert!(err.message.contains("Start Ollama"), "{}", err.message);
        assert!(err.retryable);
    }
}
