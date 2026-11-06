//! Diagnostics (tech doc 10.1): local health and latency, never sent anywhere.

use serde::Serialize;
use sidekick_diagnostics::{Stage, StageStats};
use sidekick_window::ProtectionStatus;
use tauri::State;

use crate::protection::Protection;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub app_version: String,
    pub os: String,
    pub protection: ProtectionStatus,
    pub stages: Vec<StageStats>,
}

/// Settings window: the Diagnostics panel.
#[tauri::command]
#[specta::specta]
pub fn get_diagnostics(
    state: State<'_, AppState>,
    protection: State<'_, Protection>,
) -> Diagnostics {
    Diagnostics {
        app_version: env!("CARGO_PKG_VERSION").into(),
        os: std::env::consts::OS.into(),
        protection: protection.0.clone(),
        stages: state.diagnostics.snapshot(),
    }
}

/// Overlay: the orchestrator reports stages it measures (question → cue/answer).
#[tauri::command]
#[specta::specta]
pub fn record_latency(state: State<'_, AppState>, stage: Stage, ms: u32) {
    state
        .diagnostics
        .record(stage, std::time::Duration::from_millis(u64::from(ms)));
}

/// Frontend log levels (`log` command).
#[derive(Debug, Clone, Copy, serde::Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

/// The frontend's logs go to the same file as Rust's (tech doc 7.7). The
/// message is a short code-like description; never content. Truncated to keep
/// lines small; `request_id` / `session_id` correlate with Rust's lines.
#[tauri::command]
#[specta::specta]
pub fn log(
    level: LogLevel,
    message: String,
    request_id: Option<String>,
    session_id: Option<String>,
) {
    let message: String = message.chars().take(500).collect();
    let request_id = request_id.unwrap_or_default();
    let session_id = session_id.unwrap_or_default();
    match level {
        LogLevel::Error => {
            tracing::error!(target: "frontend", %request_id, %session_id, "{message}")
        }
        LogLevel::Warn => tracing::warn!(target: "frontend", %request_id, %session_id, "{message}"),
        LogLevel::Info => tracing::info!(target: "frontend", %request_id, %session_id, "{message}"),
        LogLevel::Debug => {
            tracing::debug!(target: "frontend", %request_id, %session_id, "{message}")
        }
    }
}

/// Settings → Diagnostics → Export logs: a zip with the log files and a
/// diagnostics snapshot, for bug reports. No database, settings or keys.
#[tauri::command]
#[specta::specta]
pub fn export_logs(
    state: State<'_, AppState>,
    protection: State<'_, Protection>,
    file_path: String,
) -> sidekick_core::SidekickResult<u32> {
    let files = crate::logging::files()
        .ok_or_else(|| sidekick_core::SidekickError::internal("Logging isn't running."))?;
    files.flush();
    let snapshot = serde_json::to_string_pretty(&get_diagnostics(state, protection))
        .map_err(|_| sidekick_core::SidekickError::internal("Couldn't write diagnostics."))?;
    let n = sidekick_diagnostics::logfile::export_zip(
        files.dir(),
        &snapshot,
        std::path::Path::new(&file_path),
    )
    .map_err(|e| {
        tracing::warn!(target: "diagnostics", error = %e, "log export failed");
        sidekick_core::SidekickError::internal("Couldn't write the log bundle there.")
    })?;
    tracing::info!(target: "diagnostics", files = n, "logs exported");
    Ok(u32::try_from(n).unwrap_or(u32::MAX))
}
