//! `get_settings` / `update_settings` (tech doc 5.1). Settings window only (5.8).

use sidekick_core::SidekickResult;
use sidekick_settings::{Settings, SettingsPatch};
use tauri::State;

use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub fn get_settings(state: State<'_, AppState>) -> SidekickResult<Settings> {
    sidekick_settings::load(&state.db())
}

#[tauri::command]
#[specta::specta]
pub fn update_settings(
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> SidekickResult<Settings> {
    sidekick_settings::update(&state.db(), patch)
}
