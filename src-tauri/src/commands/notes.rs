//! Rolling notes and the end-of-session summary (tech doc 5.1 `save_notes`;
//! PRD MEM-1, MEM-3). Overlay window. The overlay's notes job writes them.

use sidekick_core::SidekickResult;
use sidekick_session::notes::{self, NewActionItem, Notes};
use tauri::State;

use crate::state::AppState;

/// Notes, summary and action items for a session (version 0 before the first save).
#[tauri::command]
#[specta::specta]
pub fn get_notes(state: State<'_, AppState>, session_id: String) -> SidekickResult<Notes> {
    notes::get(&state.db(), &session_id)
}

/// Saves rolling notes if `version` is current; `VERSION_CONFLICT` otherwise.
#[tauri::command]
#[specta::specta]
pub fn save_notes(
    state: State<'_, AppState>,
    session_id: String,
    content_md: String,
    version: u32,
) -> SidekickResult<Notes> {
    notes::save(&state.db(), &session_id, &content_md, i64::from(version))
}

/// Saves the summary written when the session ends and replaces its action items.
#[tauri::command]
#[specta::specta]
pub fn save_summary(
    state: State<'_, AppState>,
    session_id: String,
    summary_md: String,
    action_items: Vec<NewActionItem>,
) -> SidekickResult<Notes> {
    notes::save_summary(&mut state.db(), &session_id, &summary_md, &action_items)
}
