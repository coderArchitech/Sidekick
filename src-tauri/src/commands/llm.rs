//! Streaming answers (tech doc 5.1 `llm_stream`, `cancel_stream`). Overlay window.
//! Tokens arrive as `answer-token` events; the stream ends with exactly one
//! `answer-done` or `answer-error`.

use std::sync::Arc;

use sidekick_core::SidekickResult;
use sidekick_core::llm::{ImageData, LlmRequest};
use tauri::{AppHandle, State};

use crate::events::TauriAnswerSink;
use crate::state::AppState;

// Async so it runs on Tauri's Tokio runtime: the gateway spawns the stream as a
// Tokio task, which panics on the main thread where sync commands run.
#[tauri::command]
#[specta::specta]
pub async fn llm_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    request_id: String,
    provider: String,
    mut request: LlmRequest,
) -> SidekickResult<()> {
    attach_screenshots(&state, &mut request)?;
    state.gateway.start_stream(
        request_id,
        &provider,
        request,
        Arc::new(TauriAnswerSink(app)),
    )
}

/// Loads each referenced screenshot from the encrypted database into the
/// message (PRD SCR-3). The frontend only ever sends the id.
fn attach_screenshots(state: &AppState, request: &mut LlmRequest) -> SidekickResult<()> {
    for m in &mut request.messages {
        if let Some(id) = &m.screenshot_id {
            let img = sidekick_capture::repo::image(&state.db(), id)?;
            m.image = Some(ImageData {
                base64: img.base64(),
                mime_type: img.mime_type,
            });
        }
    }
    Ok(())
}

/// Returns false if the stream had already finished.
#[tauri::command]
#[specta::specta]
pub fn cancel_stream(state: State<'_, AppState>, request_id: String) -> bool {
    state.gateway.cancel_stream(&request_id)
}
