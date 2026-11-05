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
