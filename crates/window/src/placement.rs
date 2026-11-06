//! Where the overlay sits (PRD UI-2): snap to screen edges, stay on screen,
//! and remember a position per monitor. Pure logic in physical pixels; the
//! app shell reads monitors and moves the window.

use serde::{Deserialize, Serialize};

/// Distance from an edge that snaps to it.
pub const SNAP_PX: u32 = 24;
/// Positions remembered (one per monitor, most recent first).
pub const MAX_SAVED: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    fn right(self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }
    fn bottom(self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }
    fn center(self) -> (i64, i64) {
        (
            i64::from(self.x) + i64::from(self.width) / 2,
            i64::from(self.y) + i64::from(self.height) / 2,
        )
    }
    #[must_use]
    pub fn contains_point(self, (px, py): (i64, i64)) -> bool {
        px >= i64::from(self.x)
            && px < self.right()
            && py >= i64::from(self.y)
            && py < self.bottom()
    }
}

/// A remembered overlay position on one monitor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SavedPosition {
    /// [`monitor_key`] of the monitor it was on.
    pub monitor: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl SavedPosition {
    #[must_use]
    pub fn rect(&self) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
}

/// Identifies a monitor across restarts: its name and resolution (the same
/// display at another resolution gets its own position).
#[must_use]
pub fn monitor_key(name: Option<&str>, width: u32, height: u32) -> String {
    format!("{}@{width}x{height}", name.unwrap_or("display"))
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "values are clamped into i32 range first"
)]
fn to_i32(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// Moves the window flush with any work-area edge it's within `threshold` of.
#[must_use]
pub fn snap(win: Rect, area: Rect, threshold: u32) -> Rect {
    let t = i64::from(threshold);
    let mut out = win;
    if (i64::from(win.x) - i64::from(area.x)).abs() <= t {
        out.x = area.x;
    } else if (area.right() - win.right()).abs() <= t {
        out.x = to_i32(area.right() - i64::from(win.width));
    }
    if (i64::from(win.y) - i64::from(area.y)).abs() <= t {
        out.y = area.y;
    } else if (area.bottom() - win.bottom()).abs() <= t {
        out.y = to_i32(area.bottom() - i64::from(win.height));
    }
    out
}

/// Keeps the window fully inside the work area, shrinking it if it's bigger.
#[must_use]
pub fn clamp(win: Rect, area: Rect) -> Rect {
    let width = win.width.min(area.width);
    let height = win.height.min(area.height);
    let max_x = area.right() - i64::from(width);
    let max_y = area.bottom() - i64::from(height);
    Rect {
        x: to_i32(i64::from(win.x).clamp(i64::from(area.x), max_x)),
        y: to_i32(i64::from(win.y).clamp(i64::from(area.y), max_y)),
        width,
        height,
    }
}

/// The monitor (key and work area) holding the window's centre, or the first.
#[must_use]
pub fn monitor_for(win: Rect, monitors: &[(String, Rect)]) -> Option<&(String, Rect)> {
    monitors
        .iter()
        .find(|(_, area)| area.contains_point(win.center()))
        .or_else(|| monitors.first())
}

/// Where to put the overlay at startup: the most recent saved position whose
/// monitor is connected, kept on screen. `None` keeps the default placement.
#[must_use]
pub fn restore(saved: &[SavedPosition], monitors: &[(String, Rect)]) -> Option<Rect> {
    saved.iter().find_map(|s| {
        monitors
            .iter()
            .find(|(key, _)| *key == s.monitor)
            .map(|(_, area)| clamp(s.rect(), *area))
    })
}

/// Records a position for `monitor`, most recent first, one per monitor.
#[must_use]
pub fn remember(saved: &[SavedPosition], monitor: &str, rect: Rect) -> Vec<SavedPosition> {
    let mut out = vec![SavedPosition {
        monitor: monitor.to_owned(),
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }];
    out.extend(saved.iter().filter(|s| s.monitor != monitor).cloned());
    out.truncate(MAX_SAVED);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1040,
    };

    fn r(x: i32, y: i32) -> Rect {
        Rect {
            x,
            y,
            width: 420,
            height: 640,
        }
    }

    #[test]
    fn snaps_to_nearby_edges_only() {
        assert_eq!(snap(r(10, 20), AREA, SNAP_PX), r(0, 0), "top-left");
        assert_eq!(
            snap(r(1490, 390), AREA, SNAP_PX),
            r(1500, 400),
            "bottom-right"
        );
        assert_eq!(
            snap(r(300, 200), AREA, SNAP_PX),
            r(300, 200),
            "middle stays"
        );
        assert_eq!(
            snap(r(-15, 500), AREA, SNAP_PX),
            r(0, 500),
            "slightly off screen"
        );
        let second = Rect {
            x: 1920,
            y: 0,
            width: 2560,
            height: 1400,
        };
        assert_eq!(
            snap(r(1930, 700), second, SNAP_PX),
            r(1920, 700),
            "second monitor's left edge"
        );
    }

    #[test]
    fn clamps_onto_the_screen_and_shrinks_if_needed() {
        assert_eq!(clamp(r(1800, 900), AREA), r(1500, 400));
        assert_eq!(clamp(r(-500, -50), AREA), r(0, 0));
        let huge = Rect {
            x: 0,
            y: 0,
            width: 3000,
            height: 2000,
        };
        assert_eq!(clamp(huge, AREA), AREA);
    }

    #[test]
    fn remembers_one_position_per_monitor_most_recent_first() {
        let a = "Built-in@2880x1800";
        let b = "DELL U2720Q@3840x2160";
        let saved = remember(&[], a, r(10, 10));
        let saved = remember(&saved, b, r(20, 20));
        let saved = remember(&saved, a, r(30, 30));
        assert_eq!(saved.len(), 2);
        assert_eq!((saved[0].monitor.as_str(), saved[0].x), (a, 30));
        assert_eq!((saved[1].monitor.as_str(), saved[1].x), (b, 20));
        let many = (0..20).fold(Vec::new(), |s, i| remember(&s, &format!("m{i}"), r(i, i)));
        assert_eq!(many.len(), MAX_SAVED);
        assert_eq!(many[0].monitor, "m19");
    }

    #[test]
    fn restores_on_a_connected_monitor_only() {
        let laptop = (
            "Built-in@2880x1800".to_owned(),
            Rect {
                x: 0,
                y: 0,
                width: 1440,
                height: 875,
            },
        );
        let external = (
            "DELL@3840x2160".to_owned(),
            Rect {
                x: 1440,
                y: 0,
                width: 1920,
                height: 1055,
            },
        );
        let saved = remember(
            &remember(&[], &laptop.0, r(100, 100)),
            &external.0,
            r(2000, 300),
        );
        // Both connected: the most recent (external) wins.
        assert_eq!(
            restore(&saved, &[laptop.clone(), external.clone()]),
            Some(r(2000, 300))
        );
        // External unplugged: fall back to the laptop's saved spot.
        assert_eq!(
            restore(&saved, std::slice::from_ref(&laptop)),
            Some(r(100, 100))
        );
        // Nothing saved for this monitor: default placement.
        assert_eq!(restore(&saved, &[("Other@1x1".into(), AREA)]), None);
        // A saved spot that no longer fits is pulled back on screen.
        let off = remember(&[], &laptop.0, r(1300, 800));
        assert_eq!(
            restore(&off, std::slice::from_ref(&laptop)),
            Some(Rect {
                x: 1020,
                y: 235,
                width: 420,
                height: 640
            })
        );
    }

    #[test]
    fn finds_the_monitor_under_the_window_centre() {
        let mons = vec![
            (
                "a".to_owned(),
                Rect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
            ),
            (
                "b".to_owned(),
                Rect {
                    x: 1920,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
            ),
        ];
        assert_eq!(
            monitor_for(r(1800, 100), &mons).map(|m| m.0.as_str()),
            Some("b"),
            "centre is on b"
        );
        assert_eq!(
            monitor_for(r(-5000, 0), &mons).map(|m| m.0.as_str()),
            Some("a"),
            "fallback"
        );
        assert_eq!(monitor_key(Some("DELL"), 3840, 2160), "DELL@3840x2160");
        assert_eq!(monitor_key(None, 1, 2), "display@1x2");
    }
}
