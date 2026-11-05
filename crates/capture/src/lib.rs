//! Screenshots (tech doc 3.2 `capture`; PRD SCR-1 to SCR-4).
//!
//! - [`image_ops`]: downscale to the vision models' sweet spot and encode.
//! - [`capturer`]: the [`Capturer`] trait, the OS capturer (Windows, macOS)
//!   and a mock for tests and `SIDEKICK_MOCK_PROVIDERS`.
//! - [`repo`]: the `screenshots` table; images are stored in the encrypted
//!   database, never written to disk, the clipboard or shared folders.

pub mod capturer;
pub mod image_ops;
pub mod repo;

pub use capturer::{CaptureTarget, Capturer, MockCapturer, OsCapturer, Region, WindowInfo};
pub use image_ops::{Encoded, MAX_LONG_SIDE, downscale, encode, prepare};
pub use repo::{Screenshot, ScreenshotImage};
