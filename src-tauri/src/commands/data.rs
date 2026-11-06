//! Data control (tech doc 4.4; PRD 6): per-session retention and "delete everything".

use std::sync::atomic::Ordering;

use sidekick_core::SidekickResult;
use sidekick_core::retention::Retention;
use sidekick_session::Session;
use tauri::{AppHandle, State};

use crate::state::AppState;

/// How long one session's data is kept after it ends (overlay).
#[tauri::command]
#[specta::specta]
pub fn set_session_retention(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    retention: Retention,
) -> SidekickResult<Session> {
    let session = sidekick_session::repo::set_retention(&state.db(), &session_id, retention)?;
    crate::commands::session::changed(&app, &session);
    // "Until the session ends" applies at once to a session that has ended.
    if session.status == sidekick_session::SessionStatus::Ended
        && retention == Retention::SessionOnly
    {
        crate::housekeeping::cleanup(&state);
    }
    Ok(session)
}

/// Deletes every session and knowledge pack (Settings window). Settings,
/// keys, Skills and the consent log stay. Returns the number of sessions.
#[tauri::command]
#[specta::specta]
pub async fn delete_all_data(state: State<'_, AppState>) -> SidekickResult<u32> {
    // Stop listening first: the live session is deleted too.
    if let Some((_, runner)) = state.live.lock().await.take() {
        runner.stop().await;
    }
    let n = sidekick_session::retention::delete_all(&state.db())?;
    state.vacuum_pending.store(true, Ordering::SeqCst);
    crate::housekeeping::vacuum_if_idle(&state).await;
    tracing::info!(target: "session", sessions = n, "deleted all meeting data");
    Ok(n)
}
