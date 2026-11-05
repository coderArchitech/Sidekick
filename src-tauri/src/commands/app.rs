//! App-level commands that don't belong to a domain crate.

use serde::Serialize;
use sidekick_core::{SidekickError, SidekickResult};
use tauri::{AppHandle, Manager};

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

#[cfg(test)]
mod tests {
    #[test]
    fn reports_package_version() {
        let info = super::app_info().unwrap();
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }
}
