//! Google Gemini adapter (`models/{model}:streamGenerateContent?alt=sse`).
//!
//! - The key goes in the `x-goog-api-key` header, never the URL (URLs end up in logs).
//! - Images go as `inline_data` parts before the text.
//! - Effort maps to a thinking budget; `maxOutputTokens` includes thinking on
//!   Gemini 2.5, so the budget is added on top of the answer's token limit.
//! - Thought parts are skipped; the stream simply ends after the last chunk.

use serde_json::{Value, json};
use sidekick_core::llm::{
    Effort, FinishReason, LlmProvider, LlmRequest, ModelInfo, Role, StreamEvent, TokenStream, Usage,
};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_secrets::{ExposeSecret, SecretString};

use super::http::{status_error, transport_error};
use super::openai_chat::u32_at;
use super::pump::{Framing, Handler, pump};

pub const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com";
/// Used when the user hasn't picked a model (`"default"`).
pub const DEFAULT_MODEL: &str = "gemini-2.5-pro";
const NAME: &str = "Google Gemini";

#[derive(Debug, Clone)]
pub struct GeminiProvider {
    base_url: String,
    http: reqwest::Client,
}

impl Default for GeminiProvider {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

impl GeminiProvider {
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            http: reqwest::Client::new(),
        }
    }
}

fn model_name(req: &LlmRequest) -> &str {
    if req.model == "default" {
        DEFAULT_MODEL
    } else {
        req.model.trim_start_matches("models/")
    }
}

/// Thinking tokens for an effort level. Flash models can switch thinking off.
fn thinking_budget(model: &str, effort: Effort) -> u32 {
    match effort {
        Effort::Low if model.contains("flash") => 0,
        Effort::Low => 128,
        Effort::Medium => 1024,
        Effort::High => 4096,
    }
}

/// Builds the request body. Pure, so it can be snapshot-tested.
#[must_use]
pub fn build_body(req: &LlmRequest) -> Value {
    let model = model_name(req);
    let contents: Vec<Value> = req
        .messages
        .iter()
        .map(|m| {
            let role = match m.role {
                Role::User => "user",
                Role::Assistant => "model",
            };
            let mut parts = Vec::new();
            if let Some(img) = &m.image {
                parts.push(
                    json!({ "inline_data": { "mime_type": img.mime_type, "data": img.base64 } }),
                );
            }
            parts.push(json!({ "text": m.content }));
            json!({ "role": role, "parts": parts })
        })
        .collect();
    let mut generation = json!({ "maxOutputTokens": req.max_tokens });
    if let Some(effort) = req.effort {
        let budget = thinking_budget(model, effort);
        generation["thinkingConfig"] = json!({ "thinkingBudget": budget });
        generation["maxOutputTokens"] = json!(req.max_tokens.saturating_add(budget));
    }
    let mut body = json!({ "contents": contents, "generationConfig": generation });
    if let Some(system) = &req.system {
        body["systemInstruction"] = json!({ "parts": [{ "text": system }] });
    }
    body
}

#[derive(Debug, Default)]
pub struct ChunkState {
    usage: Usage,
    finish: Option<FinishReason>,
    any_text: bool,
}

fn finish_reason(r: &str) -> FinishReason {
    match r {
        "STOP" => FinishReason::EndTurn,
        "MAX_TOKENS" => FinishReason::MaxTokens,
        "SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII" | "IMAGE_SAFETY" => {
            FinishReason::Refusal
        }
        _ => FinishReason::Other,
    }
}

impl Handler for ChunkState {
    fn on_message(
        &mut self,
        _event: Option<&str>,
        data: &str,
    ) -> SidekickResult<Option<StreamEvent>> {
        let Ok(v) = serde_json::from_str::<Value>(data) else {
            return Ok(None);
        };
        if let Some(err) = v.get("error") {
            let code = match err["status"].as_str() {
                Some("RESOURCE_EXHAUSTED") => ErrorCode::RateLimited,
                Some("UNAUTHENTICATED" | "PERMISSION_DENIED") => ErrorCode::KeyInvalid,
                _ => ErrorCode::ProviderUnavailable,
            };
            return Err(SidekickError::new(
                code,
                "Gemini stopped the answer with an error.",
            ));
        }
        if v["promptFeedback"]["blockReason"].is_string() {
            self.finish = Some(FinishReason::Refusal);
        }
        if let Some(u) = v.get("usageMetadata") {
            self.usage = Usage {
                input_tokens: u32_at(&u["promptTokenCount"]),
                output_tokens: u32_at(&u["candidatesTokenCount"]),
            };
        }
        let candidate = &v["candidates"][0];
        if let Some(r) = candidate["finishReason"].as_str() {
            self.finish = Some(finish_reason(r));
        }
        let text: String = candidate["content"]["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| p["thought"] != json!(true))
            .filter_map(|p| p["text"].as_str())
            .collect();
        if text.is_empty() {
            return Ok(None);
        }
        self.any_text = true;
        Ok(Some(StreamEvent::Delta(text)))
    }

    fn on_end(&mut self) -> SidekickResult<StreamEvent> {
        if self.finish.is_none() && !self.any_text {
            return Err(SidekickError::new(
                ErrorCode::ProviderUnavailable,
                "The answer stream ended early.",
            ));
        }
        Ok(StreamEvent::Done {
            usage: self.usage,
            finish_reason: self.finish.unwrap_or(FinishReason::EndTurn),
        })
    }
}

/// Gemini answers a bad key with HTTP 400 `API_KEY_INVALID`, not 401.
async fn check_status(resp: reqwest::Response) -> SidekickResult<reqwest::Response> {
    let status = resp.status().as_u16();
    if resp.status().is_success() {
        return Ok(resp);
    }
    let retry_after = resp
        .headers()
        .get("retry-after")
        .and_then(|h| h.to_str().ok())
        .map(str::to_owned);
    if status == 400 {
        let body = resp.text().await.unwrap_or_default();
        if body.contains("API_KEY_INVALID") || body.contains("API key not valid") {
            return Err(status_error(NAME, 401, None));
        }
    }
    Err(status_error(NAME, status, retry_after.as_deref()))
}

fn require_key(key: Option<&SecretString>) -> SidekickResult<&SecretString> {
    key.ok_or_else(|| {
        SidekickError::new(
            ErrorCode::KeyMissing,
            "Add your Google Gemini API key in Settings.",
        )
    })
}

#[async_trait::async_trait]
impl LlmProvider for GeminiProvider {
    fn id(&self) -> &'static str {
        "gemini"
    }

    async fn list_models(&self, key: Option<&SecretString>) -> SidekickResult<Vec<ModelInfo>> {
        let key = require_key(key)?;
        let resp = self
            .http
            .get(format!("{}/v1beta/models?pageSize=200", self.base_url))
            .header("x-goog-api-key", key.expose_secret())
            .send()
            .await
            .map_err(|e| transport_error(NAME, &e))?;
        let body: Value = check_status(resp).await?.json().await.map_err(|_| {
            SidekickError::new(
                ErrorCode::ProviderUnavailable,
                "Unexpected reply from Gemini.",
            )
        })?;
        Ok(body["models"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|m| {
                m["supportedGenerationMethods"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|x| x == "generateContent"))
            })
            .filter_map(|m| {
                let id = m["name"].as_str()?.trim_start_matches("models/");
                id.starts_with("gemini").then(|| ModelInfo {
                    id: id.to_owned(),
                    display_name: m["displayName"].as_str().unwrap_or(id).to_owned(),
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
        let url = format!(
            "{}/v1beta/models/{}:streamGenerateContent?alt=sse",
            self.base_url,
            model_name(&req)
        );
        let resp = self
            .http
            .post(url)
            .header("x-goog-api-key", key.expose_secret())
            .json(&build_body(&req))
            .send()
            .await
            .map_err(|e| transport_error(NAME, &e))?;
        let resp = check_status(resp).await?;
        Ok(pump(resp, NAME, Framing::Sse, ChunkState::default()))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use sidekick_core::llm::{ChatMessage, ImageData};

    fn request(model: &str, effort: Option<Effort>) -> LlmRequest {
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
            max_tokens: 80,
            effort,
        }
    }

    #[test]
    fn builds_the_body_with_roles_image_system_and_thinking() {
        assert_eq!(
            build_body(&request("gemini-2.5-flash", Some(Effort::Low))),
            json!({
                "contents": [
                    { "role": "user", "parts": [{ "text": "Rate limit?" }] },
                    { "role": "model", "parts": [{ "text": "Which tier?" }] },
                    { "role": "user", "parts": [
                        { "inline_data": { "mime_type": "image/jpeg", "data": "AAAA" } },
                        { "text": "Pro" }
                    ] }
                ],
                "generationConfig": { "maxOutputTokens": 80, "thinkingConfig": { "thinkingBudget": 0 } },
                "systemInstruction": { "parts": [{ "text": "Be brief." }] }
            })
        );
    }

    #[test]
    fn thinking_budget_adds_to_the_token_limit() {
        let b = build_body(&request("gemini-2.5-pro", Some(Effort::Medium)));
        assert_eq!(b["generationConfig"]["maxOutputTokens"], 80 + 1024);
        let low_pro = build_body(&request("gemini-2.5-pro", Some(Effort::Low)));
        assert_eq!(
            low_pro["generationConfig"]["thinkingConfig"]["thinkingBudget"], 128,
            "Pro can't turn thinking off"
        );
        assert!(
            build_body(&request("gemini-2.5-pro", None))["generationConfig"]
                .get("thinkingConfig")
                .is_none()
        );
    }

    #[test]
    fn parses_a_recorded_stream_and_skips_thoughts() {
        let fixture = include_str!("../../tests/fixtures/gemini/stream.sse");
        let mut parser = crate::sse::SseParser::default();
        let mut state = ChunkState::default();
        let mut text = String::new();
        for ev in parser.push(fixture) {
            if let Some(StreamEvent::Delta(t)) = state.on_message(None, &ev.data).unwrap() {
                text.push_str(&t);
            }
        }
        assert_eq!(text, "The Pro tier allows 600 requests per minute [1].");
        assert_eq!(
            state.on_end().unwrap(),
            StreamEvent::Done {
                usage: Usage {
                    input_tokens: 790,
                    output_tokens: 15
                },
                finish_reason: FinishReason::EndTurn
            }
        );
    }

    #[test]
    fn maps_safety_blocks_errors_and_early_end() {
        let mut s = ChunkState::default();
        s.on_message(None, r#"{"promptFeedback":{"blockReason":"SAFETY"}}"#)
            .unwrap();
        assert!(matches!(
            s.on_end(),
            Ok(StreamEvent::Done {
                finish_reason: FinishReason::Refusal,
                ..
            })
        ));
        let mut s = ChunkState::default();
        let err = s
            .on_message(
                None,
                r#"{"error":{"code":429,"status":"RESOURCE_EXHAUSTED"}}"#,
            )
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::RateLimited);
        assert_eq!(
            ChunkState::default().on_end().unwrap_err().code,
            ErrorCode::ProviderUnavailable
        );
    }
}
