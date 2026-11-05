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

/// One turn of the conversation. Text only for now; images arrive with screenshots (T-025).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
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
