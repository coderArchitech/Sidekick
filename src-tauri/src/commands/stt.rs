//! Local Whisper models (PRD KEY-2, tech doc 3.6). Settings window: list the
//! models and download one, verified against its published checksum.

use std::path::PathBuf;

use serde::Serialize;
use sidekick_core::{SidekickError, SidekickResult};
use sidekick_stt::models::{self, WhisperModel};
use tauri::{AppHandle, State};
use tauri_specta::Event;

use crate::events::ModelDownloadProgress;
use crate::state::AppState;

/// Used when Settings has no Whisper model chosen: multilingual, so Hinglish works.
pub const DEFAULT_WHISPER_MODEL: &str = "base";

/// A model and whether it's on this computer.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WhisperModelEntry {
    #[serde(flatten)]
    pub model: WhisperModel,
    pub downloaded: bool,
}

pub fn models_dir(state: &AppState) -> PathBuf {
    state.data_dir.join("models")
}

pub fn whisper_model(name: Option<&str>) -> SidekickResult<&'static WhisperModel> {
    let name = name.unwrap_or(DEFAULT_WHISPER_MODEL);
    models::find(name)
        .ok_or_else(|| SidekickError::internal(format!("Unknown Whisper model {name}.")))
}

/// Whisper models and which are downloaded.
#[tauri::command]
#[specta::specta]
pub fn whisper_models(state: State<'_, AppState>) -> Vec<WhisperModelEntry> {
    let dir = models_dir(&state);
    models::MODELS
        .iter()
        .map(|m| WhisperModelEntry {
            model: *m,
            downloaded: models::path(&dir, m).exists(),
        })
        .collect()
}

/// Downloads a model, emitting `model-download-progress`; the file is kept only
/// if its checksum matches.
#[tauri::command]
#[specta::specta]
pub async fn download_whisper_model(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> SidekickResult<()> {
    let model = whisper_model(Some(&name))?;
    let dest = models::path(&models_dir(&state), model);
    let mut last_percent = None;
    let result = models::download(
        &reqwest::Client::new(),
        &models::url(model),
        &dest,
        model.digest,
        |done, total| {
            // At most one event per percent.
            let percent = total.map(|t| done * 100 / t.max(1));
            if percent != last_percent {
                last_percent = percent;
                let _ = ModelDownloadProgress {
                    name: model.name.to_owned(),
                    done_bytes: done as f64,
                    total_bytes: total.map(|t| t as f64),
                }
                .emit(&app);
            }
        },
    )
    .await;
    tracing::info!(target: "stt", model = model.name, ok = result.is_ok(), "whisper model download");
    result
}
