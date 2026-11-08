//! Knowledge Packs (PRD CTX-5) and the pre-meeting brief (CTX-4).
//! Packs are managed in the Settings window and attached to a session from
//! the overlay; pack documents ingest like session documents, with progress
//! as `ingest-progress` events.

use sidekick_core::SidekickResult;
use sidekick_ingest::{DocumentInfo, Owner, PackInfo, Source, packs};
use sidekick_retrieval::{BRIEF_CHARS, Brief, BriefSource, brief};
use tauri::{AppHandle, State};

use crate::commands::documents::start;
use crate::state::AppState;

/// Creates an empty pack.
#[tauri::command]
#[specta::specta]
pub fn create_pack(state: State<'_, AppState>, name: String) -> SidekickResult<PackInfo> {
    packs::create(&state.db(), &name)
}

/// Renames a pack.
#[tauri::command]
#[specta::specta]
pub fn rename_pack(
    state: State<'_, AppState>,
    pack_id: String,
    name: String,
) -> SidekickResult<PackInfo> {
    packs::rename(&state.db(), &pack_id, &name)
}

/// Deletes a pack with its documents; sessions that used it no longer do.
#[tauri::command]
#[specta::specta]
pub fn delete_pack(state: State<'_, AppState>, pack_id: String) -> SidekickResult<()> {
    packs::delete(&state.db(), &pack_id)
}

/// All packs, by name, with document counts.
#[tauri::command]
#[specta::specta]
pub fn list_packs(state: State<'_, AppState>) -> SidekickResult<Vec<PackInfo>> {
    packs::list(&state.db())
}

/// A pack's documents.
#[tauri::command]
#[specta::specta]
pub fn list_pack_documents(
    state: State<'_, AppState>,
    pack_id: String,
) -> SidekickResult<Vec<DocumentInfo>> {
    packs::documents(&state.db(), &pack_id)
}

/// Queues a file for a pack.
#[tauri::command]
#[specta::specta]
pub fn ingest_pack_document(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: String,
    path: String,
) -> SidekickResult<()> {
    packs::get(&state.db(), &pack_id)?;
    start(app, &state, Owner::Pack(pack_id), Source::File(path.into()))
}

/// Queues a web page for a pack.
#[tauri::command]
#[specta::specta]
pub fn ingest_pack_url(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: String,
    url: String,
) -> SidekickResult<()> {
    packs::get(&state.db(), &pack_id)?;
    start(app, &state, Owner::Pack(pack_id), Source::Url(url))
}

/// The packs attached to a session.
#[tauri::command]
#[specta::specta]
pub fn list_session_packs(
    state: State<'_, AppState>,
    session_id: String,
) -> SidekickResult<Vec<String>> {
    packs::for_session(&state.db(), &session_id)
}

/// Replaces the packs attached to a session; answers then search them too.
#[tauri::command]
#[specta::specta]
pub fn set_session_packs(
    state: State<'_, AppState>,
    session_id: String,
    pack_ids: Vec<String>,
) -> SidekickResult<Vec<String>> {
    packs::set_for_session(&mut state.db(), &session_id, &pack_ids)
}

/// The start of every ready document of the session and its packs, for the brief.
#[tauri::command]
#[specta::specta]
pub fn brief_material(
    state: State<'_, AppState>,
    session_id: String,
) -> SidekickResult<Vec<BriefSource>> {
    brief::material(&state.db(), &session_id, BRIEF_CHARS)
}

/// The session's saved brief, if any.
#[tauri::command]
#[specta::specta]
pub fn get_brief(state: State<'_, AppState>, session_id: String) -> SidekickResult<Option<Brief>> {
    brief::get(&state.db(), &session_id)
}

/// Saves the session's brief (written by the overlay).
#[tauri::command]
#[specta::specta]
pub fn save_brief(
    state: State<'_, AppState>,
    session_id: String,
    content_md: String,
) -> SidekickResult<Brief> {
    brief::save(&state.db(), &session_id, &content_md)
}
