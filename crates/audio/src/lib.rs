//! Audio capture for Sidekick (tech doc 3.2 `audio`, PRD LIS-1, LIS-6, ADR-006).
//!
//! Two independent streams: the microphone (the user, "Me") and system audio
//! (everyone else). Each is converted to 16 kHz mono 16-bit frames of
//! [`FRAME_MS`] with a speech/silence flag, and sent over a channel.
//!
//! Sources: [`DeviceSource`] (live devices via cpal; system audio uses WASAPI
//! loopback on Windows and a Core Audio tap on macOS 14.6+) and
//! [`FixtureSource`] (a WAV file, for tests and `SIDEKICK_AUDIO_FIXTURE`).
//!
//! Privacy (NFR-6): this crate never writes audio anywhere.

mod capture;
mod device;
mod fixture;
mod pipeline;
mod resample;
mod vad;

pub use capture::{CaptureHandle, FrameSender, frame_channel};
pub use device::DeviceSource;
pub use fixture::{FixtureSource, Pacing};
pub use pipeline::Pipeline;
pub use resample::Resampler;
pub use vad::EnergyVad;

/// Sample rate of every frame Sidekick produces.
pub const SAMPLE_RATE: u32 = 16_000;
/// Length of one frame.
pub const FRAME_MS: u32 = 50;
/// Samples per frame (800).
pub const FRAME_SAMPLES: usize = (SAMPLE_RATE * FRAME_MS / 1000) as usize;

/// Which stream a frame belongs to (PRD LIS-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamKind {
    /// The user's microphone, labelled "Me".
    Mic,
    /// Everything the computer plays: other participants.
    System,
}

/// 50 ms of 16 kHz mono audio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFrame {
    pub kind: StreamKind,
    /// Milliseconds since capture started.
    pub start_ms: u64,
    pub samples: Vec<i16>,
    /// True if the VAD heard speech in or just before this frame.
    pub speech: bool,
}
