//! Keeps the overlay above every other app (PRD UI-1).
//!
//! Tauri's `alwaysOnTop` uses the lowest "floating" level on macOS and isn't
//! re-applied after the window is hidden and shown, so the overlay could drop
//! behind another app, or behind a full-screen meeting on its own Space. Here:
//! - the overlay joins every Space, including full-screen ones;
//! - on macOS it sits at the status-window level (above floating windows and
//!   full-screen apps) and doesn't hide when Sidekick isn't the active app;
//! - "always on top" is applied again whenever the overlay loses focus or is
//!   shown, so other topmost windows (meeting toolbars) don't keep it under.

use tauri::{Runtime, WebviewWindow};

/// Pins `window` above other apps. Errors are logged, never fatal.
pub fn pin<R: Runtime>(window: &WebviewWindow<R>) {
    if let Err(e) = window.set_always_on_top(true) {
        tracing::warn!(target: "window", error = %e, "always on top failed");
    }
    if let Err(e) = window.set_visible_on_all_workspaces(true) {
        tracing::warn!(target: "window", error = %e, "visible on all workspaces failed");
    }
    #[cfg(target_os = "macos")]
    macos::float_above_apps(window);
}

/// Re-applies the pin: on Windows a topmost window only rises to the top of
/// the topmost band when the flag is set again.
pub fn refresh<R: Runtime>(window: &WebviewWindow<R>) {
    if !window.is_visible().unwrap_or(false) {
        return;
    }
    #[cfg(target_os = "windows")]
    let _ = window.set_always_on_top(false);
    pin(window);
}

/// What the overlay's window events need: re-pin when it loses focus.
pub fn on_event<R: Runtime>(window: &WebviewWindow<R>, event: &tauri::WindowEvent) {
    if matches!(event, tauri::WindowEvent::Focused(false)) {
        refresh(window);
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2_app_kit::{NSStatusWindowLevel, NSWindow, NSWindowCollectionBehavior};
    use tauri::{Runtime, WebviewWindow};

    /// Status-window level, on every Space and over full-screen apps, and not
    /// hidden when Sidekick is deactivated. AppKit calls run on the main thread.
    pub fn float_above_apps<R: Runtime>(window: &WebviewWindow<R>) {
        let w = window.clone();
        let _ = window.run_on_main_thread(move || {
            let Ok(ptr) = w.ns_window() else { return };
            if ptr.is_null() {
                return;
            }
            // SAFETY: `ns_window` returns this window's live `NSWindow`, and we
            // are on the main thread, where AppKit objects may be used.
            #[allow(unsafe_code)]
            let ns: &NSWindow = unsafe { &*ptr.cast::<NSWindow>() };
            ns.setLevel(NSStatusWindowLevel);
            ns.setCollectionBehavior(
                ns.collectionBehavior()
                    | NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::FullScreenAuxiliary
                    | NSWindowCollectionBehavior::Stationary,
            );
            ns.setHidesOnDeactivate(false);
        });
    }
}
