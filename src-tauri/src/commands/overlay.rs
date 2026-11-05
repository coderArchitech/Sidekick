//! Overlay appearance (tech doc 5.1 `set_overlay`; PRD UI-1, UI-3). Overlay
//! window: it may change only these, not other settings.

use serde::Deserialize;
use sidekick_core::SidekickResult;
use sidekick_settings::{OverlayPrefs, SettingsPatch};
use tauri::State;

use crate::state::AppState;

/// Fields to change; omitted ones stay as they are.
#[derive(Debug, Clone, Default, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OverlayPatch {
    #[serde(default)]
    #[specta(optional)]
    pub opacity: Option<u8>,
    #[serde(default)]
    #[specta(optional)]
    pub font_size: Option<u8>,
    #[serde(default)]
    #[specta(optional)]
    pub compact: Option<bool>,
}

#[tauri::command]
#[specta::specta]
pub fn set_overlay(
    state: State<'_, AppState>,
    patch: OverlayPatch,
) -> SidekickResult<OverlayPrefs> {
    let db = state.db();
    let mut overlay = sidekick_settings::load(&db)?.overlay;
    if let Some(v) = patch.opacity {
        overlay.opacity = v;
    }
    if let Some(v) = patch.font_size {
        overlay.font_size = v;
    }
    if let Some(v) = patch.compact {
        overlay.compact = v;
    }
    Ok(sidekick_settings::update(
        &db,
        SettingsPatch {
            overlay: Some(overlay),
            ..SettingsPatch::default()
        },
    )?
    .overlay)
}
