//! Plays a WAV file as if it were a live device (tests, demos,
//! `SIDEKICK_AUDIO_FIXTURE`, tech doc 6.4).

use std::path::PathBuf;
use std::time::Duration;

use sidekick_core::{ErrorCode, SidekickError, SidekickResult};

use crate::capture::{CaptureHandle, Flags};
use crate::{FRAME_MS, FrameSender, Pipeline, StreamKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pacing {
    /// One frame per 50 ms, like a real device.
    RealTime,
    /// As fast as the consumer accepts frames (tests).
    Fast,
}

#[derive(Debug, Clone)]
pub struct FixtureSource {
    pub path: PathBuf,
    pub kind: StreamKind,
    pub pacing: Pacing,
}

impl FixtureSource {
    /// Starts streaming the file on a background thread.
    pub fn start(&self, tx: FrameSender) -> SidekickResult<CaptureHandle> {
        let mut reader = hound::WavReader::open(&self.path).map_err(|_| {
            SidekickError::new(ErrorCode::Internal, "Couldn't read the audio fixture.")
        })?;
        let spec = reader.spec();
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Int => {
                let scale = f32::from(1u16 << 15).max(1.0)
                    * if spec.bits_per_sample > 16 {
                        65_536.0
                    } else {
                        1.0
                    };
                reader
                    .samples::<i32>()
                    .map(|s| {
                        s.map(|v| {
                            #[allow(clippy::cast_precision_loss)]
                            let f = v as f32 / scale;
                            f
                        })
                    })
                    .collect::<Result<_, _>>()
            }
            hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>(),
        }
        .map_err(|_| SidekickError::new(ErrorCode::Internal, "The audio fixture is damaged."))?;

        let flags = Flags::default();
        let thread_flags = flags.clone();
        let (kind, pacing) = (self.kind, self.pacing);
        let thread = std::thread::Builder::new()
            .name("sidekick-audio-fixture".into())
            .spawn(move || {
                let mut pipeline = Pipeline::new(kind, spec.sample_rate, spec.channels);
                // Feed 10 ms at a time, like a device callback.
                let chunk = (spec.sample_rate as usize / 100) * usize::from(spec.channels);
                for block in samples.chunks(chunk.max(1)) {
                    if thread_flags.stopped() {
                        return;
                    }
                    let mut frames = Vec::new();
                    pipeline.push(block, |f| frames.push(f));
                    for f in frames {
                        // Time passes while paused too: pace first, then drop if paused.
                        if pacing == Pacing::RealTime {
                            std::thread::sleep(Duration::from_millis(u64::from(FRAME_MS)));
                        }
                        if thread_flags.paused() {
                            continue;
                        }
                        if tx.blocking_send(f).is_err() {
                            return;
                        }
                    }
                }
            })
            .map_err(|_| SidekickError::internal("Couldn't start audio playback."))?;
        Ok(CaptureHandle::new(flags.paused, flags.stopped, thread))
    }
}
