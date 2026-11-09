//! Overlay appearance (tech doc 5.1 `set_overlay`; PRD UI-1, UI-3). Overlay
//! window: it may change only these, not other settings.

use serde::Deserialize;
use sidekick_core::SidekickResult;
use sidekick_settings::{OverlayPrefs, SettingsPatch};
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

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

/// Click-through (PRD UI-5): the overlay stays visible but mouse clicks go to
/// the app behind it. Not saved: the overlay always starts clickable. The
/// Click-through shortcut turns it off again.
#[tauri::command]
#[specta::specta]
pub fn set_click_through(
    app: AppHandle,
    state: State<'_, AppState>,
    on: bool,
) -> SidekickResult<bool> {
    let window = app
        .get_webview_window("overlay")
        .ok_or_else(|| sidekick_core::SidekickError::internal("The overlay window isn't open."))?;
    window.set_ignore_cursor_events(on).map_err(|e| {
        tracing::warn!(target: "window", error = %e, "click-through failed");
        sidekick_core::SidekickError::internal("Couldn't change click-through on this system.")
    })?;
    state
        .click_through
        .store(on, std::sync::atomic::Ordering::Relaxed);
    tracing::info!(target: "window", on, "click-through");
    let _ = crate::events::ClickThroughChanged { on }.emit(&app);
    Ok(on)
}

/// Whether click-through is on.
#[tauri::command]
#[specta::specta]
pub fn get_click_through(state: State<'_, AppState>) -> bool {
    state
        .click_through
        .load(std::sync::atomic::Ordering::Relaxed)
}
