//! Overlay placement (PRD UI-2): restore the remembered position for a
//! connected monitor at startup; after the user moves or resizes the overlay
//! and it settles, snap it to nearby edges and remember it for that monitor.
//! The rules are in `sidekick_window::placement`; this applies them.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sidekick_window::placement::{self, Rect, SNAP_PX};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

use crate::state::AppState;

/// How long the window must be still before snapping and saving.
const SETTLE: Duration = Duration::from_millis(500);

/// Bumped on every move or resize; a pending save only runs if it's still the latest.
#[derive(Default)]
pub struct Generation(Arc<AtomicU64>);

fn monitors<R: Runtime>(w: &WebviewWindow<R>) -> Vec<(String, Rect)> {
    w.available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let area = m.work_area();
            (
                placement::monitor_key(
                    m.name().map(String::as_str),
                    m.size().width,
                    m.size().height,
                ),
                Rect {
                    x: area.position.x,
                    y: area.position.y,
                    width: area.size.width,
                    height: area.size.height,
                },
            )
        })
        .collect()
}

fn current<R: Runtime>(w: &WebviewWindow<R>) -> Option<Rect> {
    let pos = w.outer_position().ok()?;
    let size = w.outer_size().ok()?;
    Some(Rect {
        x: pos.x,
        y: pos.y,
        width: size.width,
        height: size.height,
    })
}

fn apply<R: Runtime>(w: &WebviewWindow<R>, r: Rect) {
    let _ = w.set_size(PhysicalSize::new(r.width, r.height));
    let _ = w.set_position(PhysicalPosition::new(r.x, r.y));
}

/// Startup: move the overlay to its remembered spot, if its monitor is connected.
pub fn restore<R: Runtime>(app: &AppHandle<R>) {
    let Some(w) = app.get_webview_window("overlay") else {
        return;
    };
    let state = app.state::<AppState>();
    let saved = match sidekick_settings::load(&state.db()) {
        Ok(s) => s.overlay.positions,
        Err(_) => return,
    };
    if let Some(r) = placement::restore(&saved, &monitors(&w)) {
        apply(&w, r);
    }
}

/// Called on every move or resize of the overlay.
pub fn on_changed<R: Runtime>(app: &AppHandle<R>) {
    let generation = Arc::clone(&app.state::<Generation>().0);
    let mine = generation.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SETTLE).await;
        if generation.load(Ordering::SeqCst) != mine {
            return; // Still moving.
        }
        settle(&app);
    });
}

fn settle<R: Runtime>(app: &AppHandle<R>) {
    let Some(w) = app.get_webview_window("overlay") else {
        return;
    };
    if !w.is_visible().unwrap_or(false) || w.is_minimized().unwrap_or(false) {
        return;
    }
    let Some(now) = current(&w) else { return };
    let mons = monitors(&w);
    let Some((key, area)) = placement::monitor_for(now, &mons).cloned() else {
        return;
    };
    let snapped = placement::snap(now, area, SNAP_PX);
    if snapped != now {
        // Moving again triggers one more settle, which then saves.
        apply(&w, snapped);
        return;
    }
    let state = app.state::<AppState>();
    let db = state.db();
    let Ok(settings) = sidekick_settings::load(&db) else {
        return;
    };
    let mut overlay = settings.overlay;
    let positions = placement::remember(&overlay.positions, &key, now);
    if positions == overlay.positions {
        return;
    }
    overlay.positions = positions;
    let patch = sidekick_settings::SettingsPatch {
        overlay: Some(overlay),
        ..Default::default()
    };
    if let Err(e) = sidekick_settings::update(&db, patch) {
        tracing::warn!(target: "window", code = ?e.code, "couldn't save the overlay position");
    }
}
