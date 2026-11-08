//! Rolling notes and the end-of-session summary (tech doc 5.1 `save_notes`;
//! PRD MEM-1 to MEM-4), the agenda and action items. Overlay window. The
//! overlay's notes job writes them.

use sidekick_core::SidekickResult;
use sidekick_session::notes::{self, AgendaUpdate, NewActionItem, Notes};
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

/// Replaces the session's agenda; items already on it keep their status (PRD MEM-2).
#[tauri::command]
#[specta::specta]
pub fn set_agenda(
    state: State<'_, AppState>,
    session_id: String,
    items: Vec<String>,
) -> SidekickResult<Notes> {
    notes::set_agenda(&mut state.db(), &session_id, &items)
}

/// Sets agenda item statuses (from the notes job or the user).
#[tauri::command]
#[specta::specta]
pub fn update_agenda(
    state: State<'_, AppState>,
    session_id: String,
    updates: Vec<AgendaUpdate>,
) -> SidekickResult<Notes> {
    notes::update_agenda(&mut state.db(), &session_id, &updates)
}

/// Ticks an action item off or back on (PRD MEM-4).
#[tauri::command]
#[specta::specta]
pub fn set_action_item_done(
    state: State<'_, AppState>,
    session_id: String,
    item_id: String,
    done: bool,
) -> SidekickResult<Notes> {
    notes::set_action_item_done(&state.db(), &session_id, &item_id, done)
}
