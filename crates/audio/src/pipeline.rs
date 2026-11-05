//! Raw device samples → 16 kHz mono frames with a VAD flag.

use crate::{AudioFrame, EnergyVad, FRAME_MS, FRAME_SAMPLES, Resampler, StreamKind};

#[derive(Debug, Clone)]
pub struct Pipeline {
    kind: StreamKind,
    resampler: Resampler,
    vad: EnergyVad,
    buffer: Vec<i16>,
    frames_out: u64,
}

impl Pipeline {
    #[must_use]
    pub fn new(kind: StreamKind, input_rate: u32, channels: u16) -> Self {
        Self {
            kind,
            resampler: Resampler::new(input_rate, channels),
            vad: EnergyVad::default(),
            buffer: Vec::with_capacity(FRAME_SAMPLES * 2),
            frames_out: 0,
        }
    }

    /// Feeds interleaved samples in [-1, 1]; calls `emit` for each complete frame.
    pub fn push(&mut self, interleaved: &[f32], mut emit: impl FnMut(AudioFrame)) {
        self.resampler.process(interleaved, &mut self.buffer);
        while self.buffer.len() >= FRAME_SAMPLES {
            let samples: Vec<i16> = self.buffer.drain(..FRAME_SAMPLES).collect();
            let speech = self.vad.is_speech(&samples);
            emit(AudioFrame {
                kind: self.kind,
                start_ms: self.frames_out * u64::from(FRAME_MS),
                samples,
                speech,
            });
            self.frames_out += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_fixed_size_frames_with_increasing_timestamps() {
        let mut p = Pipeline::new(StreamKind::Mic, 48_000, 1);
        let mut frames = Vec::new();
        // 1 second at 48 kHz in 10 ms device callbacks.
        for _ in 0..100 {
            p.push(&[0.0; 480], |f| frames.push(f));
        }
        assert!((19..=20).contains(&frames.len()), "{}", frames.len());
        assert!(
            frames
                .iter()
                .all(|f| f.samples.len() == FRAME_SAMPLES && f.kind == StreamKind::Mic)
        );
        let starts: Vec<u64> = frames.iter().map(|f| f.start_ms).take(3).collect();
        assert_eq!(starts, [0, 50, 100]);
        assert!(frames.iter().all(|f| !f.speech));
    }
}
