//! `get_settings` / `update_settings` (tech doc 5.1). Settings window only (5.8).

use sidekick_core::SidekickResult;
use sidekick_settings::{Settings, SettingsPatch};
use tauri::{AppHandle, State};

use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub fn get_settings(state: State<'_, AppState>) -> SidekickResult<Settings> {
    sidekick_settings::load(&state.db())
}

#[tauri::command]
#[specta::specta]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> SidekickResult<Settings> {
    // Register new shortcuts with the OS first; only save them if that works.
    if let Some(new) = &patch.shortcuts {
        let previous = sidekick_settings::load(&state.db())?.shortcuts;
        let validated = sidekick_hotkeys::validate(new)?;
        crate::shortcuts::apply(&app, &validated, &previous)?;
    }
    sidekick_settings::update(&state.db(), patch)
}
