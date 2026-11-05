//! Typed events pushed to the frontend (tech doc 5.2). Tauri event names can't
//! contain dots, so `answer.token` is emitted as `answer-token`, and so on.

use serde::Serialize;
use sidekick_core::SidekickError;
use sidekick_core::llm::{FinishReason, Usage};
use sidekick_gateway::streams::AnswerSink;
use tauri::AppHandle;
use tauri_specta::Event;

#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct AnswerToken {
    pub request_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct AnswerDone {
    pub request_id: String,
    pub usage: Usage,
    pub finish_reason: FinishReason,
}

#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct AnswerError {
    pub request_id: String,
    pub error: SidekickError,
}

/// Sends gateway stream output to the frontend as events.
pub struct TauriAnswerSink(pub AppHandle);

impl AnswerSink for TauriAnswerSink {
    fn token(&self, request_id: &str, delta: &str) {
        let _ = AnswerToken {
            request_id: request_id.into(),
            delta: delta.into(),
        }
        .emit(&self.0);
    }
    fn done(&self, request_id: &str, usage: Usage, finish_reason: FinishReason) {
        let _ = AnswerDone {
            request_id: request_id.into(),
            usage,
            finish_reason,
        }
        .emit(&self.0);
    }
    fn error(&self, request_id: &str, error: &SidekickError) {
        let _ = AnswerError {
            request_id: request_id.into(),
            error: error.clone(),
        }
        .emit(&self.0);
    }
}
