//! App-level commands that don't belong to a domain crate.

use serde::Serialize;
use sidekick_core::SidekickResult;

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

#[cfg(test)]
mod tests {
    #[test]
    fn reports_package_version() {
        let info = super::app_info().unwrap();
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }
}
