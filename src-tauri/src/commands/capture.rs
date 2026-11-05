//! Screenshots (tech doc 5.1 `capture_screenshot`; PRD SCR-1 to SCR-4).
//! Overlay window. Images go straight into the encrypted database: never to
//! disk, the clipboard or shared folders.

use serde::Deserialize;
use sidekick_capture::{CaptureTarget, Region, Screenshot, WindowInfo, prepare, repo};
use sidekick_core::{SidekickError, SidekickResult};
use sidekick_session::SessionStatus;
use tauri::{AppHandle, State};
use tauri_specta::Event;

use crate::events::ScreenshotCaptured;
use crate::state::AppState;

/// What to capture.
#[derive(Debug, Clone, Copy, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "mode")]
pub enum CaptureRequest {
    /// The main display.
    Screen,
    /// One window from `list_windows`.
    Window {
        #[serde(rename = "windowId")]
        window_id: u32,
    },
    /// Part of the main display, as fractions of its size.
    Region { region: Region },
}

impl From<CaptureRequest> for CaptureTarget {
    fn from(r: CaptureRequest) -> Self {
        match r {
            CaptureRequest::Screen => Self::Screen,
            CaptureRequest::Window { window_id } => Self::Window(window_id),
            CaptureRequest::Region { region } => Self::Region(region),
        }
    }
}

/// Size of the screen preview used to select a region.
const PREVIEW_LONG_SIDE: u32 = 800;

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> SidekickResult<T> + Send + 'static,
) -> SidekickResult<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| SidekickError::internal("Screen capture stopped unexpectedly."))?
}

/// Captures, downscales to 1568 px and stores the screenshot with the session.
#[tauri::command]
#[specta::specta]
pub async fn capture_screenshot(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    request: CaptureRequest,
) -> SidekickResult<Screenshot> {
    let session = sidekick_session::repo::get(&state.db(), &session_id)?;
    if session.status == SessionStatus::Ended {
        return Err(SidekickError::internal(
            "That session has ended. Start a new one to take screenshots.",
        ));
    }
    let capturer = std::sync::Arc::clone(&state.capturer);
    let encoded = blocking(move || prepare(capturer.capture(request.into())?)).await?;
    let shot = repo::save(&state.db(), &session_id, &encoded)?;
    tracing::info!(target: "capture", width = shot.width, height = shot.height, bytes = encoded.bytes.len(), "screenshot stored");
    let _ = ScreenshotCaptured {
        screenshot: shot.clone(),
    }
    .emit(&app);
    Ok(shot)
}

/// Windows the user can capture (other apps only, not minimised).
#[tauri::command]
#[specta::specta]
pub async fn list_windows(state: State<'_, AppState>) -> SidekickResult<Vec<WindowInfo>> {
    let capturer = std::sync::Arc::clone(&state.capturer);
    blocking(move || capturer.windows()).await
}

/// A small, unsaved picture of the main display for selecting a region.
#[tauri::command]
#[specta::specta]
pub async fn preview_screen(state: State<'_, AppState>) -> SidekickResult<String> {
    let capturer = std::sync::Arc::clone(&state.capturer);
    blocking(move || {
        let img = sidekick_capture::downscale(
            capturer.capture(CaptureTarget::Screen)?,
            PREVIEW_LONG_SIDE,
        );
        let e = sidekick_capture::encode(&img)?;
        Ok(repo::ScreenshotImage {
            mime_type: e.mime_type.into(),
            bytes: e.bytes,
        }
        .data_url())
    })
    .await
}

/// The session's screenshots, oldest first.
#[tauri::command]
#[specta::specta]
pub fn list_screenshots(
    state: State<'_, AppState>,
    session_id: String,
) -> SidekickResult<Vec<Screenshot>> {
    repo::list(&state.db(), &session_id)
}

/// A screenshot as a `data:` URL, for thumbnails in the overlay.
#[tauri::command]
#[specta::specta]
pub fn screenshot_image(
    state: State<'_, AppState>,
    screenshot_id: String,
) -> SidekickResult<String> {
    Ok(repo::image(&state.db(), &screenshot_id)?.data_url())
}
