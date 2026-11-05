//! Energy-based voice activity detection with an adaptive noise floor and a
//! hangover, so word gaps don't cut speech. A perfectly steady sound lasting
//! more than ~4 s is treated as background noise; real speech has gaps that
//! keep the noise floor down. Silero VAD (tech doc 3.4) can
//! replace this behind the same interface if accuracy needs it.

const HANGOVER_FRAMES: u32 = 6; // 300 ms at 50 ms frames
const MIN_SPEECH_RMS: f32 = 300.0; // absolute floor, ~-40 dBFS
const SNR: f32 = 3.0; // speech must be this much louder than the noise floor
const FLOOR_RISE: f32 = 0.005;
const FLOOR_FALL: f32 = 0.2;

#[derive(Debug, Clone)]
pub struct EnergyVad {
    noise_floor: f32,
    hangover: u32,
}

impl Default for EnergyVad {
    fn default() -> Self {
        Self {
            noise_floor: MIN_SPEECH_RMS / SNR,
            hangover: 0,
        }
    }
}

impl EnergyVad {
    /// Classifies one frame. Returns true for speech (or the hangover after it).
    pub fn is_speech(&mut self, frame: &[i16]) -> bool {
        let rms = rms(frame);
        let loud = rms > MIN_SPEECH_RMS && rms > self.noise_floor * SNR;
        // The floor falls quickly to quieter levels and rises slowly (time
        // constant ~10 s), so constant hum is absorbed but speech isn't.
        let rate = if rms < self.noise_floor {
            FLOOR_FALL
        } else {
            FLOOR_RISE
        };
        self.noise_floor += rate * (rms.max(1.0) - self.noise_floor);
        if loud {
            self.hangover = HANGOVER_FRAMES;
            true
        } else {
            if self.hangover > 0 {
                self.hangover -= 1;
                true
            } else {
                false
            }
        }
    }
}

#[allow(clippy::cast_precision_loss)]
fn rms(frame: &[i16]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let sum: f64 = frame.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
    #[allow(clippy::cast_possible_truncation)]
    let v = (sum / frame.len() as f64).sqrt() as f32;
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    fn tone(amp: f32) -> Vec<i16> {
        (0..800)
            .map(|i| (amp * (i as f32 * 0.2).sin()) as i16)
            .collect()
    }

    #[test]
    fn ten_seconds_of_continuous_speech_stays_speech() {
        let mut v = EnergyVad::default();
        // Words with short gaps between them, as in real speech.
        let flagged = (0..200)
            .filter(|i| {
                v.is_speech(&tone(match i % 5 {
                    4 => 150.0,
                    0 => 3000.0,
                    _ => 7000.0,
                }))
            })
            .count();
        assert!(flagged > 190, "flagged {flagged} of 200");
    }

    #[test]
    fn silence_is_not_speech() {
        let mut v = EnergyVad::default();
        assert!((0..20).all(|_| !v.is_speech(&[0; 800])));
    }

    #[test]
    fn speech_is_detected_and_held_through_short_gaps() {
        let mut v = EnergyVad::default();
        assert!(v.is_speech(&tone(8000.0)));
        // 300 ms hangover, then silence.
        let after: Vec<bool> = (0..8).map(|_| v.is_speech(&[0; 800])).collect();
        assert_eq!(after, [true, true, true, true, true, true, false, false]);
    }

    #[test]
    fn steady_background_noise_is_ignored_after_adapting() {
        let mut v = EnergyVad::default();
        let hum = tone(900.0);
        let flagged = (0..200).filter(|_| v.is_speech(&hum)).count();
        assert!(
            flagged < 80,
            "flagged {flagged} of 200 hum frames (adapts within ~4 s)"
        );
        assert!(!v.is_speech(&hum));
        assert!(v.is_speech(&tone(9000.0)), "speech over hum still detected");
    }
}
