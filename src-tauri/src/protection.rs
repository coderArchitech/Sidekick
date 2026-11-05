//! Turns on capture exclusion for Sidekick's windows at startup (PRD UI-4) and
//! records the resulting status for the shield indicator (UI-4a).

use sidekick_window::{ProtectionStatus, current_os, protection_status};
use tauri::{AppHandle, Manager};

/// The status computed at startup, shared with commands.
pub struct Protection(pub ProtectionStatus);

/// Applies exclusion to every window (the Settings window shows key endings,
/// so it's protected too). `SIDEKICK_DISABLE_EXCLUSION=1` skips it (tech doc 6.4).
pub fn apply(app: &AppHandle) -> Protection {
    let disabled = std::env::var("SIDEKICK_DISABLE_EXCLUSION").as_deref() == Ok("1");
    let mut applied = true;
    if !disabled {
        for (label, window) in app.webview_windows() {
            if let Err(e) = window.set_content_protected(true) {
                tracing::error!(window = %label, error = %e, "capture exclusion failed");
                applied = false;
            }
        }
    }
    let (os, version) = current_os();
    let status = protection_status(os, version, disabled, applied);
    tracing::info!(state = ?status.state, ?os, ?version, "capture protection");
    Protection(status)
}
