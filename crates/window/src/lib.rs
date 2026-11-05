//! Capture exclusion status (PRD UI-4, UI-4a, UI-4b; ADR-007; NFR-9).
//!
//! The shell turns exclusion on with Tauri's `set_content_protected(true)`:
//! - Windows: `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`, which needs
//!   Windows 10 2004 (build 19041) or later; older builds silently ignore it.
//! - macOS: `NSWindow.sharingType = .none`. Legacy capture APIs honour it, but
//!   apps that capture with ScreenCaptureKit on macOS 15 and later can still see
//!   the window, so it isn't guaranteed there.
//! - Linux: not available.
//!
//! These rules come from OS documentation and reports; the manual checklist
//! (Task T-011) confirms them per OS and meeting app and may tighten them.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    Linux,
    Other,
}

/// Major, minor, patch (or build for Windows: 10.0.19045 → (10, 0, 19045)).
pub type Version = (u32, u32, u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ProtectionState {
    /// The OS hides the overlay from screen sharing and recordings (green shield).
    Protected,
    /// Exclusion is on, but some capture methods may still see the overlay (amber).
    NotGuaranteed,
    /// This OS can't hide the overlay (red).
    Unavailable,
    /// Turned off with `SIDEKICK_DISABLE_EXCLUSION=1` (grey).
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ProtectionStatus {
    pub state: ProtectionState,
    /// One sentence for the shield tooltip and Diagnostics.
    pub reason: String,
}

const WINDOWS_MIN_BUILD: u32 = 19_041;
const MACOS_SCREENCAPTUREKIT_BYPASS: u32 = 15;

/// Status for a given OS, version, and whether exclusion was turned off or
/// failed to apply.
#[must_use]
pub fn protection_status(
    os: Os,
    version: Version,
    disabled: bool,
    applied: bool,
) -> ProtectionStatus {
    use ProtectionState::{Disabled, NotGuaranteed, Protected, Unavailable};
    let (state, reason) = if disabled {
        (
            Disabled,
            "Screen-share protection is turned off for this run.".to_owned(),
        )
    } else if !applied {
        (
            Unavailable,
            "Sidekick couldn't turn on screen-share protection.".to_owned(),
        )
    } else {
        match os {
            Os::Windows if version.2 >= WINDOWS_MIN_BUILD => {
                (Protected, "Windows hides this panel from screen sharing and recordings.".to_owned())
            }
            Os::Windows => (
                Unavailable,
                "This Windows version can't hide windows from screen sharing. Update to Windows 10 2004 or later.".to_owned(),
            ),
            Os::MacOs if version.0 >= MACOS_SCREENCAPTUREKIT_BYPASS => (
                NotGuaranteed,
                "On this macOS version some meeting apps can still capture this panel. Use auto-hide when sharing.".to_owned(),
            ),
            Os::MacOs => (Protected, "macOS hides this panel from screen sharing and recordings.".to_owned()),
            Os::Linux | Os::Other => (Unavailable, "This OS can't hide windows from screen sharing.".to_owned()),
        }
    };
    ProtectionStatus { state, reason }
}

/// The running OS and version.
#[must_use]
pub fn current_os() -> (Os, Version) {
    let os = if cfg!(target_os = "windows") {
        Os::Windows
    } else if cfg!(target_os = "macos") {
        Os::MacOs
    } else if cfg!(target_os = "linux") {
        Os::Linux
    } else {
        Os::Other
    };
    (os, parse_version(&os_info::get().version().to_string()))
}

/// Parses "10.0.19045", "15.1.1", "14.6" (missing parts are 0).
#[must_use]
pub fn parse_version(text: &str) -> Version {
    let mut parts = text
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .map(|p| p.parse::<u32>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

/// The fallback (UI-4b): hide the overlay while the screen is shared unless
/// protection is guaranteed. `auto_hide` is the user's setting.
#[must_use]
pub fn should_auto_hide(status: ProtectionState, sharing: bool, auto_hide: bool) -> bool {
    sharing && auto_hide && status != ProtectionState::Protected
}

#[cfg(test)]
mod tests {
    use super::*;
    use ProtectionState::*;

    #[test]
    fn windows_needs_build_19041() {
        assert_eq!(
            protection_status(Os::Windows, (10, 0, 22_631), false, true).state,
            Protected
        );
        assert_eq!(
            protection_status(Os::Windows, (10, 0, 19_045), false, true).state,
            Protected
        );
        assert_eq!(
            protection_status(Os::Windows, (10, 0, 18_363), false, true).state,
            Unavailable
        );
    }

    #[test]
    fn macos_15_and_later_is_not_guaranteed() {
        assert_eq!(
            protection_status(Os::MacOs, (14, 6, 1), false, true).state,
            Protected
        );
        assert_eq!(
            protection_status(Os::MacOs, (15, 0, 0), false, true).state,
            NotGuaranteed
        );
        assert_eq!(
            protection_status(Os::MacOs, (26, 1, 0), false, true).state,
            NotGuaranteed
        );
    }

    #[test]
    fn linux_is_unavailable_and_flags_win() {
        assert_eq!(
            protection_status(Os::Linux, (6, 8, 0), false, true).state,
            Unavailable
        );
        assert_eq!(
            protection_status(Os::Windows, (10, 0, 22_631), true, true).state,
            Disabled
        );
        assert_eq!(
            protection_status(Os::Windows, (10, 0, 22_631), false, false).state,
            Unavailable
        );
    }

    #[test]
    fn every_status_has_a_reason() {
        for os in [Os::Windows, Os::MacOs, Os::Linux, Os::Other] {
            for disabled in [false, true] {
                assert!(
                    !protection_status(os, (15, 0, 20_000), disabled, true)
                        .reason
                        .is_empty()
                );
            }
        }
    }

    #[test]
    fn parses_versions() {
        assert_eq!(parse_version("10.0.19045"), (10, 0, 19_045));
        assert_eq!(parse_version("14.6"), (14, 6, 0));
        assert_eq!(parse_version("Unknown"), (0, 0, 0));
    }

    #[test]
    fn auto_hide_only_when_sharing_and_not_guaranteed() {
        assert!(should_auto_hide(NotGuaranteed, true, true));
        assert!(should_auto_hide(Unavailable, true, true));
        assert!(!should_auto_hide(Protected, true, true));
        assert!(!should_auto_hide(NotGuaranteed, false, true));
        assert!(!should_auto_hide(NotGuaranteed, true, false));
    }

    #[test]
    fn serialises_for_the_ui() {
        let json =
            serde_json::to_value(protection_status(Os::MacOs, (15, 0, 0), false, true)).unwrap();
        assert_eq!(json["state"], "not_guaranteed");
    }

    #[test]
    fn detects_the_current_os() {
        let (os, _) = current_os();
        if cfg!(target_os = "linux") {
            assert_eq!(os, Os::Linux);
        }
    }
}
