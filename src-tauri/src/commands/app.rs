//! App-level commands that don't belong to a domain crate.

use serde::Serialize;
use sidekick_core::{SidekickError, SidekickResult};
use sidekick_window::ProtectionStatus;
use tauri::{AppHandle, Manager, State};

use crate::protection::Protection;

/// Basic facts about the running app, shown in the overlay and diagnostics.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub os: String,
}

#[tauri::command]
#[specta::specta]
pub fn app_info() -> SidekickResult<AppInfo> {
    Ok(AppInfo {
        name: "Sidekick".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        os: std::env::consts::OS.into(),
    })
}

/// Whether the overlay is hidden from screen sharing (PRD UI-4a).
#[tauri::command]
#[specta::specta]
pub fn get_capture_protection(protection: State<'_, Protection>) -> ProtectionStatus {
    protection.0.clone()
}

/// Shows and focuses the Settings window (created hidden at startup).
#[tauri::command]
#[specta::specta]
pub fn open_settings(app: AppHandle) -> SidekickResult<()> {
    let window = app
        .get_webview_window("settings")
        .ok_or_else(|| SidekickError::internal("The Settings window is missing."))?;
    window
        .show()
        .and_then(|()| window.unminimize())
        .and_then(|()| window.set_focus())
        .map_err(|_| SidekickError::internal("Couldn't open Settings."))
}

/// Quits Sidekick (the red close button in the overlay header). The overlay
/// ends a live session first, so its notes and summary are saved.
#[tauri::command]
#[specta::specta]
pub fn quit_app(app: AppHandle) {
    tracing::info!(target: "app", "quit from the overlay");
    app.exit(0);
}

#[cfg(test)]
mod tests {
    #[test]
    fn reports_package_version() {
        let info = super::app_info().unwrap();
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }
}
