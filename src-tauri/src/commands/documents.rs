//! Session documents (tech doc 5.1 `ingest_document`, `delete_document`; PRD CTX-1, CTX-2).
//! Ingest runs in the background; progress arrives as `ingest-progress` events.

use std::sync::Arc;

use sidekick_core::SidekickResult;
use sidekick_ingest::{DocumentInfo, Owner, Progress, Source, repo};
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

use crate::events::IngestProgress;
use crate::state::AppState;

pub(crate) fn start(
    app: AppHandle,
    state: &AppState,
    owner: Owner,
    source: Source,
) -> SidekickResult<()> {
    // Resolve the embedder now so a missing key is reported immediately.
    let settings = sidekick_settings::load(&state.db())?;
    let (embedder, key) = state.gateway.embedder(
        &settings.embeddings.provider,
        settings.embeddings.model.as_deref(),
    )?;
    let db = state.db_handle();
    let slots = Arc::clone(&state.ingest_slots);
    let progress_app = app.clone();
    let progress: Arc<dyn Fn(Progress) + Send + Sync> = Arc::new(move |p: Progress| {
        let _ = IngestProgress { progress: p }.emit(&progress_app);
    });
    tauri::async_runtime::spawn(async move {
        let _permit = slots.acquire_owned().await;
        if let Err(e) = sidekick_ingest::ingest(db, owner, source, embedder, key, progress).await {
            tracing::warn!(code = ?e.code, "document ingest failed");
        }
        drop(app);
    });
    Ok(())
}

/// Queues a file for parsing and embedding.
#[tauri::command]
#[specta::specta]
pub fn ingest_document(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    path: String,
) -> SidekickResult<()> {
    start(
        app,
        &state,
        Owner::Session(session_id),
        Source::File(path.into()),
    )
}

/// Queues a web page (http/https) for ingest.
#[tauri::command]
#[specta::specta]
pub fn ingest_url(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    url: String,
) -> SidekickResult<()> {
    start(app, &state, Owner::Session(session_id), Source::Url(url))
}

#[tauri::command]
#[specta::specta]
pub fn list_documents(
    state: State<'_, AppState>,
    session_id: String,
) -> SidekickResult<Vec<DocumentInfo>> {
    repo::list_for_session(&state.db(), &session_id)
}

#[tauri::command]
#[specta::specta]
pub fn delete_document(app: AppHandle, document_id: String) -> SidekickResult<()> {
    let state = app.state::<AppState>();
    repo::delete(&state.db(), &document_id)
}
