//! Streaming answers (tech doc 5.1 `llm_stream`, `cancel_stream`). Overlay window.
//! Tokens arrive as `answer-token` events; the stream ends with exactly one
//! `answer-done` or `answer-error`.

use std::sync::Arc;

use sidekick_core::SidekickResult;
use sidekick_core::llm::LlmRequest;
use tauri::{AppHandle, State};

use crate::events::TauriAnswerSink;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub fn llm_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    request_id: String,
    provider: String,
    request: LlmRequest,
) -> SidekickResult<()> {
    state.gateway.start_stream(
        request_id,
        &provider,
        request,
        Arc::new(TauriAnswerSink(app)),
    )
}

/// Returns false if the stream had already finished.
#[tauri::command]
#[specta::specta]
pub fn cancel_stream(state: State<'_, AppState>, request_id: String) -> bool {
    state.gateway.cancel_stream(&request_id)
}
