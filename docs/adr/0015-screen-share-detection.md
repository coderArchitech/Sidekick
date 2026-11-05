# ADR-015: Detect screen sharing from meeting apps' own share windows

- **Status:** Proposed (needs validation on Windows and macOS, Task T-011)
- **Date:** 2026-11-05
- **Deciders:** @nitesh

## Context

The auto-hide fallback (PRD UI-4b) hides the overlay while the screen is shared and capture exclusion isn't guaranteed (macOS 15+, older Windows). That needs to know _when_ sharing starts and stops. Neither Windows nor macOS offers a public API that tells an app "another app is capturing the screen":

- **Windows:** Windows Graphics Capture draws a yellow border but exposes no system-wide "capture active" query. Desktop Duplication has none either.
- **macOS:** ScreenCaptureKit shows a menu bar indicator, but there is no public API to read it.

Every major meeting app, however, shows its own small control window while sharing (a "You are sharing" toolbar or bar with a Stop button).

## Decision (proposed)

Detect sharing by looking for those control windows:

1. Every 2 seconds while a session is live, list top-level windows (Windows: `EnumWindows` with title and class; macOS: `CGWindowListCopyWindowInfo` with owner name and title, which needs the Screen Recording permission that screenshots already require).
2. Match against a small, versioned table of signatures per app (owner process plus title or class pattern) for Teams, Zoom, Meet in Chrome/Edge/Safari, Webex and Slack. The table lives in `crates/window` as data, so updating it doesn't touch logic.
3. Treat sharing as started on the first match and stopped after two consecutive misses (avoids flicker), then emit `screen_share.detected`.
4. Keep the manual show/hide hotkey (T-015) as the always-available fallback, and say in Settings that detection covers the listed apps only.

## Alternatives considered

- **No detection, hotkey only.** Simplest and reliable, but users forget, and a missed hide leaks private notes.
- **Watch for capture APIs being used (Windows ETW, macOS private APIs).** Possible but fragile, needs elevated rights or private frameworks, and can break with OS updates.
- **Hide whenever a meeting app is running.** Too aggressive; the overlay would be hidden during every call.

## Consequences

- Works for the apps most users use, without special permissions on Windows.
- Signatures will drift as meeting apps update; the table needs a test per app (recorded window lists as fixtures) and a review each release.
- Browser-based sharing (Meet) is matched by the browser's own "is sharing your screen" bar, whose text is localised; the table needs entries per supported language.
- To validate before accepting: record the window lists while sharing in each app on both OSes (part of the T-011 manual check).
