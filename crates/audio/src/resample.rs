//! Streaming downmix and resample to 16 kHz mono.
//!
//! A windowed-sinc low-pass (cutoff 7.2 kHz) removes everything the 16 kHz
//! output can't represent, then linear interpolation picks the output samples.
//! At 48 kHz this costs about 1.5 M multiply-adds per second.

use crate::SAMPLE_RATE;

const TAPS: usize = 63;
const CUTOFF_HZ: f64 = 7_200.0;

#[derive(Debug, Clone)]
pub struct Resampler {
    channels: usize,
    step: f64,
    /// Fractional read position into `pending`.
    pos: f64,
    /// FIR coefficients (empty when no filtering is needed).
    coeffs: Vec<f32>,
    /// Ring buffer of recent mono input for the FIR.
    history: Vec<f32>,
    history_idx: usize,
    /// Filtered mono samples not yet consumed.
    pending: Vec<f32>,
}

impl Resampler {
    /// `input_rate` in Hz; `channels` interleaved.
    #[must_use]
    pub fn new(input_rate: u32, channels: u16) -> Self {
        let ratio = f64::from(input_rate) / f64::from(SAMPLE_RATE);
        let coeffs = if input_rate > SAMPLE_RATE {
            lowpass(f64::from(input_rate))
        } else {
            Vec::new()
        };
        Self {
            channels: usize::from(channels.max(1)),
            step: ratio,
            pos: 0.0,
            history: vec![0.0; coeffs.len().max(1)],
            coeffs,
            history_idx: 0,
            pending: Vec::new(),
        }
    }

    /// Converts interleaved `f32` samples in [-1, 1] to 16 kHz mono `i16`.
    pub fn process(&mut self, interleaved: &[f32], out: &mut Vec<i16>) {
        #[allow(clippy::cast_precision_loss)]
        let channels = self.channels as f32;
        for frame in interleaved.chunks_exact(self.channels) {
            let mono = frame.iter().sum::<f32>() / channels;
            let filtered = if self.coeffs.is_empty() {
                mono
            } else {
                self.filter(mono)
            };
            self.pending.push(filtered);
        }
        // Emit while we have two neighbours to interpolate between.
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        while (self.pos as usize) + 1 < self.pending.len() {
            let i = self.pos as usize;
            let frac = (self.pos - i as f64) as f32;
            let v = self.pending[i] * (1.0 - frac) + self.pending[i + 1] * frac;
            out.push(to_i16(v));
            self.pos += self.step;
        }
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        {
            let consumed = (self.pos as usize).min(self.pending.len().saturating_sub(1));
            self.pending.drain(..consumed);
            self.pos -= consumed as f64;
        }
    }

    fn filter(&mut self, sample: f32) -> f32 {
        let n = self.history.len();
        self.history[self.history_idx] = sample;
        self.history_idx = (self.history_idx + 1) % n;
        // history_idx now points at the oldest sample.
        let mut acc = 0.0;
        for (k, c) in self.coeffs.iter().enumerate() {
            acc += c * self.history[(self.history_idx + k) % n];
        }
        acc
    }
}

/// Hamming-windowed sinc low-pass at `CUTOFF_HZ`, normalised to unity gain.
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
fn lowpass(input_rate: f64) -> Vec<f32> {
    let fc = CUTOFF_HZ / input_rate;
    let mid = (TAPS - 1) as f64 / 2.0;
    let raw: Vec<f64> = (0..TAPS)
        .map(|i| {
            let x = i as f64 - mid;
            let sinc = if x == 0.0 {
                2.0 * fc
            } else {
                (2.0 * std::f64::consts::PI * fc * x).sin() / (std::f64::consts::PI * x)
            };
            let window =
                0.54 - 0.46 * (2.0 * std::f64::consts::PI * i as f64 / (TAPS - 1) as f64).cos();
            sinc * window
        })
        .collect();
    let sum: f64 = raw.iter().sum();
    raw.iter().map(|c| (c / sum) as f32).collect()
}

#[allow(clippy::cast_possible_truncation)]
fn to_i16(v: f32) -> i16 {
    (v.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::cast_precision_loss)]
    fn sine(rate: u32, channels: u16, hz: f32, secs: f32, amp: f32) -> Vec<f32> {
        let n = (rate as f32 * secs) as usize;
        (0..n)
            .flat_map(|i| {
                let v = amp * (2.0 * std::f32::consts::PI * hz * i as f32 / rate as f32).sin();
                std::iter::repeat_n(v, usize::from(channels))
            })
            .collect()
    }

    fn zero_crossings(s: &[i16]) -> usize {
        s.windows(2).filter(|w| (w[0] < 0) != (w[1] < 0)).count()
    }

    #[test]
    fn converts_48k_stereo_to_16k_mono_preserving_pitch_and_level() {
        let mut r = Resampler::new(48_000, 2);
        let mut out = Vec::new();
        // Feed in uneven chunks, like a real device callback.
        let input = sine(48_000, 2, 440.0, 1.0, 0.5);
        for chunk in input.chunks(1234 * 2) {
            r.process(chunk, &mut out);
        }
        assert!((15_990..=16_000).contains(&out.len()), "len {}", out.len());
        // 440 Hz → ~880 zero crossings per second.
        let zc = zero_crossings(&out);
        assert!((870..=890).contains(&zc), "zero crossings {zc}");
        let peak = out.iter().map(|s| s.unsigned_abs()).max().unwrap();
        assert!((14_000..=16_500).contains(&peak), "peak {peak}");
    }

    #[test]
    fn handles_44_1k_and_passthrough_16k() {
        for (rate, ch) in [(44_100, 1), (16_000, 1), (32_000, 2)] {
            let mut r = Resampler::new(rate, ch);
            let mut out = Vec::new();
            r.process(&sine(rate, ch, 300.0, 0.5, 0.3), &mut out);
            assert!(
                (7_980..=8_000).contains(&out.len()),
                "{rate}: {}",
                out.len()
            );
        }
    }

    #[test]
    fn removes_content_above_the_new_nyquist() {
        // A 12 kHz tone can't exist at 16 kHz; it must be strongly attenuated.
        let mut r = Resampler::new(48_000, 1);
        let mut out = Vec::new();
        r.process(&sine(48_000, 1, 12_000.0, 0.5, 0.5), &mut out);
        // Skip the filter's start-up transient (the tone starts abruptly).
        let peak = out[64..].iter().map(|s| s.unsigned_abs()).max().unwrap();
        assert!(peak < 200, "peak {peak}"); // below -38 dB
    }
}
