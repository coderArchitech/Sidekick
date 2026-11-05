//! Taking the picture. Capture is silent: the OS APIs used here don't notify
//! or interrupt the meeting app (PRD SCR-1).

use image::RgbaImage;
use serde::{Deserialize, Serialize};
use sidekick_core::{SidekickError, SidekickResult};

/// Part of the screen, as fractions (0 to 1) of the main display, so the UI
/// can select on a small preview without knowing the display's pixel size.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Region {
    /// Pixel rectangle inside a `w` x `h` image, or `None` if it's empty.
    #[must_use]
    pub fn to_pixels(self, w: u32, h: u32) -> Option<(u32, u32, u32, u32)> {
        let clamp = |v: f64| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        let (x0, y0) = (clamp(self.x), clamp(self.y));
        let (x1, y1) = (clamp(self.x + self.width), clamp(self.y + self.height));
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "fractions of a u32 size"
        )]
        let px = |f: f64, size: u32| (f * f64::from(size)).round() as u32;
        let (left, top) = (px(x0, w), px(y0, h));
        let (right, bottom) = (px(x1, w), px(y1, h));
        (right > left && bottom > top).then(|| (left, top, right - left, bottom - top))
    }
}

/// What to capture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CaptureTarget {
    /// The main display.
    Screen,
    /// One window, by the id from [`Capturer::windows`].
    Window(u32),
    /// Part of the main display.
    Region(Region),
}

/// A window the user can pick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub id: u32,
    pub app_name: String,
    pub title: String,
}

pub trait Capturer: Send + Sync {
    /// Captures `target` at full resolution.
    fn capture(&self, target: CaptureTarget) -> SidekickResult<RgbaImage>;
    /// Visible windows of other apps, front to back.
    fn windows(&self) -> SidekickResult<Vec<WindowInfo>>;
}

pub(crate) fn crop(img: &RgbaImage, region: Region) -> SidekickResult<RgbaImage> {
    let (x, y, w, h) = region
        .to_pixels(img.width(), img.height())
        .ok_or_else(|| SidekickError::internal("Select an area to capture."))?;
    Ok(image::imageops::crop_imm(img, x, y, w, h).to_image())
}

/// A synthetic 2560 x 1440 screen with two windows, for tests and mock mode.
#[derive(Debug, Default, Clone, Copy)]
pub struct MockCapturer;

impl Capturer for MockCapturer {
    fn capture(&self, target: CaptureTarget) -> SidekickResult<RgbaImage> {
        let screen = RgbaImage::from_fn(2560, 1440, |x, y| {
            #[allow(clippy::cast_possible_truncation, reason = "modulo 256")]
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
        });
        match target {
            CaptureTarget::Screen => Ok(screen),
            CaptureTarget::Region(r) => crop(&screen, r),
            CaptureTarget::Window(1) => Ok(RgbaImage::from_pixel(1200, 800, image::Rgba([240; 4]))),
            CaptureTarget::Window(2) => Ok(RgbaImage::from_pixel(900, 1600, image::Rgba([20; 4]))),
            CaptureTarget::Window(_) => Err(window_gone()),
        }
    }

    fn windows(&self) -> SidekickResult<Vec<WindowInfo>> {
        Ok(vec![
            WindowInfo {
                id: 1,
                app_name: "Code".into(),
                title: "main.rs".into(),
            },
            WindowInfo {
                id: 2,
                app_name: "Browser".into(),
                title: "Dashboard".into(),
            },
        ])
    }
}

fn window_gone() -> SidekickError {
    SidekickError::internal("That window is no longer open.")
}

/// Real capture with the OS APIs (Windows Graphics Capture / GDI, macOS
/// `ScreenCaptureKit` / Core Graphics via `xcap`).
#[derive(Debug, Default, Clone, Copy)]
pub struct OsCapturer;

#[cfg(any(windows, target_os = "macos"))]
mod os {
    use super::{CaptureTarget, Capturer, OsCapturer, WindowInfo, crop, window_gone};
    use image::RgbaImage;
    use sidekick_core::{ErrorCode, SidekickError, SidekickResult};

    fn failed(e: &xcap::XCapError) -> SidekickError {
        tracing::warn!(target: "capture", error = %e, "capture failed");
        // Most failures are a missing Screen Recording permission (macOS).
        SidekickError::new(
            ErrorCode::PermissionDenied,
            "Couldn't capture the screen. On macOS, allow Sidekick under System Settings → Privacy & Security → Screen Recording.",
        )
    }

    fn main_monitor() -> SidekickResult<xcap::Monitor> {
        let monitors = xcap::Monitor::all().map_err(|e| failed(&e))?;
        let primary = monitors
            .iter()
            .position(|m| m.is_primary().unwrap_or(false))
            .unwrap_or(0);
        monitors
            .into_iter()
            .nth(primary)
            .ok_or_else(|| SidekickError::internal("No display found."))
    }

    impl Capturer for OsCapturer {
        fn capture(&self, target: CaptureTarget) -> SidekickResult<RgbaImage> {
            match target {
                CaptureTarget::Screen => main_monitor()?.capture_image().map_err(|e| failed(&e)),
                CaptureTarget::Region(r) => {
                    let img = main_monitor()?.capture_image().map_err(|e| failed(&e))?;
                    crop(&img, r)
                }
                CaptureTarget::Window(id) => {
                    let windows = xcap::Window::all().map_err(|e| failed(&e))?;
                    let w = windows
                        .into_iter()
                        .find(|w| w.id().ok() == Some(id))
                        .ok_or_else(window_gone)?;
                    w.capture_image().map_err(|e| failed(&e))
                }
            }
        }

        fn windows(&self) -> SidekickResult<Vec<WindowInfo>> {
            let own = std::process::id();
            let windows = xcap::Window::all().map_err(|e| failed(&e))?;
            Ok(windows
                .into_iter()
                .filter(|w| w.pid().ok() != Some(own) && !w.is_minimized().unwrap_or(true))
                .filter_map(|w| {
                    let title = w.title().unwrap_or_default();
                    let app_name = w.app_name().unwrap_or_default();
                    let big = w.width().unwrap_or(0) >= 50 && w.height().unwrap_or(0) >= 50;
                    let id = w.id().ok()?;
                    (big && !(title.is_empty() && app_name.is_empty())).then_some(WindowInfo {
                        id,
                        app_name,
                        title,
                    })
                })
                .collect())
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
impl Capturer for OsCapturer {
    fn capture(&self, _target: CaptureTarget) -> SidekickResult<RgbaImage> {
        Err(unsupported())
    }

    fn windows(&self) -> SidekickResult<Vec<WindowInfo>> {
        Err(unsupported())
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn unsupported() -> SidekickError {
    SidekickError::new(
        sidekick_core::ErrorCode::PermissionDenied,
        "Screenshots work on Windows and macOS. Use SIDEKICK_MOCK_PROVIDERS=1 to try the flow here.",
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn region_maps_fractions_to_pixels_and_clamps() {
        let r = Region {
            x: 0.25,
            y: 0.5,
            width: 0.5,
            height: 0.25,
        };
        assert_eq!(r.to_pixels(2000, 1000), Some((500, 500, 1000, 250)));
        let outside = Region {
            x: 0.9,
            y: -0.2,
            width: 0.5,
            height: 0.5,
        };
        assert_eq!(outside.to_pixels(1000, 1000), Some((900, 0, 100, 300)));
        let empty = Region {
            x: 0.5,
            y: 0.5,
            width: 0.0,
            height: 0.2,
        };
        assert_eq!(empty.to_pixels(1000, 1000), None);
        let nan = Region {
            x: f64::NAN,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        };
        assert_eq!(nan.to_pixels(10, 10), None, "invalid input selects nothing");
    }

    #[test]
    fn mock_captures_screen_region_and_windows() {
        let m = MockCapturer;
        assert_eq!(
            m.capture(CaptureTarget::Screen).unwrap().dimensions(),
            (2560, 1440)
        );
        let r = m
            .capture(CaptureTarget::Region(Region {
                x: 0.0,
                y: 0.0,
                width: 0.5,
                height: 0.5,
            }))
            .unwrap();
        assert_eq!(r.dimensions(), (1280, 720));
        let ids: Vec<_> = m.windows().unwrap().iter().map(|w| w.id).collect();
        assert_eq!(ids, [1, 2]);
        assert_eq!(
            m.capture(CaptureTarget::Window(2)).unwrap().dimensions(),
            (900, 1600)
        );
        assert!(m.capture(CaptureTarget::Window(9)).is_err());
    }
}
