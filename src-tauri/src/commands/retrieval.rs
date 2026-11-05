//! `retrieve_context` (tech doc 5.1): the transcript window, notes and best
//! document passages for a question. Used by the orchestrator (T-021).

use std::time::Instant;

use sidekick_core::SidekickResult;
use sidekick_diagnostics::Stage;
use sidekick_retrieval::{ContextOptions, ContextPack, build_pack};
use tauri::State;

use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub async fn retrieve_context(
    state: State<'_, AppState>,
    session_id: String,
    query: String,
    options: Option<ContextOptions>,
) -> SidekickResult<ContextPack> {
    let started = Instant::now();
    let settings = sidekick_settings::load(&state.db())?;
    // Embed the question outside the database lock. Without an embeddings
    // provider (e.g. no key yet), fall back to keyword search.
    let query_vector = match state.gateway.embedder(
        &settings.embeddings.provider,
        settings.embeddings.model.as_deref(),
    ) {
        Ok((embedder, key)) => match embedder
            .embed(std::slice::from_ref(&query), key.as_ref())
            .await
        {
            Ok(mut v) => v.pop(),
            Err(e) => {
                tracing::warn!(code = ?e.code, "query embedding failed; keyword search only");
                None
            }
        },
        Err(_) => None,
    };
    let pack = build_pack(
        &state.db(),
        &session_id,
        &query,
        query_vector.as_deref(),
        options.unwrap_or_default(),
    )?;
    state
        .diagnostics
        .record(Stage::Retrieval, started.elapsed());
    Ok(pack)
}
