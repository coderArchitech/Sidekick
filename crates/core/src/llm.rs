//! Provider-neutral types and the `LlmProvider` trait (tech doc 5.6).
//!
//! Adapters live in the gateway crate; everything here is shared so the
//! orchestrator contract (IPC types) and the adapters agree.

use std::pin::Pin;

use futures::Stream;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

use crate::SidekickResult;

/// Who wrote a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// One turn of the conversation: text, plus optionally one screenshot (PRD SCR-3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    /// A stored screenshot to show the model with this message. The app
    /// loads it from the encrypted database into `image`; pixels never cross IPC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(optional))]
    pub screenshot_id: Option<String>,
    #[serde(skip)]
    #[cfg_attr(feature = "specta", specta(skip))]
    pub image: Option<ImageData>,
}

impl ChatMessage {
    #[must_use]
    pub fn text(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            screenshot_id: None,
            image: None,
        }
    }
}

/// An image for a vision model, base64-encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageData {
    pub mime_type: String,
    pub base64: String,
}

/// How hard the model should think. Lower is faster and cheaper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Low,
    Medium,
    High,
}

/// A provider-neutral request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmRequest {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    pub effort: Option<Effort>,
}

/// Token counts reported by the provider.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Why generation stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    EndTurn,
    MaxTokens,
    /// The provider declined to answer; show a neutral notice, not partial text.
    Refusal,
    ToolUse,
    Other,
}

/// One item of a streamed answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    Delta(String),
    Done {
        usage: Usage,
        finish_reason: FinishReason,
    },
}

/// A model the user can pick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
}

pub type TokenStream = Pin<Box<dyn Stream<Item = SidekickResult<StreamEvent>> + Send>>;

/// An LLM vendor (or local runtime) behind one interface.
/// `key` is `None` for providers that need no key (Ollama, mock).
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    fn id(&self) -> &'static str;
    async fn list_models(&self, key: Option<&SecretString>) -> SidekickResult<Vec<ModelInfo>>;
    async fn stream(
        &self,
        req: LlmRequest,
        key: Option<&SecretString>,
    ) -> SidekickResult<TokenStream>;
}
