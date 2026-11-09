//! Registers global shortcuts with the OS (PRD UI-6) and turns presses into
//! `hotkey-pressed` events. Show/hide is handled here directly, so it works even
//! when the overlay's web view is hidden.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Mutex, PoisonError};

use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_hotkeys::{Action, Binding, label};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};
use tauri_specta::Event;

use crate::events::HotkeyPressed;

/// Which registered shortcut id means which action.
#[derive(Default)]
pub struct Registered(Mutex<HashMap<u32, Action>>);

pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event: ShortcutEvent| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            let action = app.try_state::<Registered>().and_then(|r| {
                r.0.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .get(&shortcut.id())
                    .copied()
            });
            if let Some(action) = action {
                on_action(app, action);
            }
        })
        .build()
}

fn on_action<R: Runtime>(app: &AppHandle<R>, action: Action) {
    if action == Action::ShowHide
        && let Some(w) = app.get_webview_window("overlay")
    {
        let visible = w.is_visible().unwrap_or(true);
        let _ = if visible {
            w.hide()
        } else {
            w.show().and_then(|()| {
                crate::topmost::pin(&w);
                w.set_focus()
            })
        };
    }
    // Ask types into Chat, so the overlay has to be visible and focused.
    if action == Action::Ask
        && let Some(w) = app.get_webview_window("overlay")
    {
        let _ = w.show().and_then(|()| {
            crate::topmost::pin(&w);
            w.set_focus()
        });
    }
    let _ = HotkeyPressed { action }.emit(app);
}

/// Replaces all registered shortcuts with `bindings`. If the OS refuses one
/// (another app owns it), the previous set is restored and the error names it.
pub fn apply(app: &AppHandle, bindings: &[Binding], previous: &[Binding]) -> SidekickResult<()> {
    match register_all(app, bindings) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = register_all(app, previous);
            Err(e)
        }
    }
}

fn register_all(app: &AppHandle, bindings: &[Binding]) -> SidekickResult<()> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut map = HashMap::new();
    for b in bindings {
        let shortcut = Shortcut::from_str(&b.keys).map_err(|_| {
            SidekickError::new(
                ErrorCode::Internal,
                format!("\"{}\" isn't a valid shortcut.", b.keys),
            )
        })?;
        gs.register(shortcut).map_err(|e| {
            tracing::warn!(keys = %b.keys, error = %e, "shortcut registration failed");
            SidekickError::new(
                ErrorCode::Internal,
                format!(
                    "{} is already used by another app. Pick a different shortcut for {}.",
                    b.keys,
                    label(b.action)
                ),
            )
        })?;
        map.insert(shortcut.id(), b.action);
    }
    if let Some(r) = app.try_state::<Registered>() {
        *r.0.lock().unwrap_or_else(PoisonError::into_inner) = map;
    }
    Ok(())
}
