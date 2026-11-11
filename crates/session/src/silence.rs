//! Notices a microphone that delivers only digital silence. macOS and Windows
//! give an app without microphone permission frames full of zeros, so capture
//! "works" and speech-to-text stays connected, yet nothing is ever transcribed.
//! Real microphones always pick up some noise, so all-zero audio is a clear sign.

use std::sync::Arc;

use sidekick_audio::{AudioFrame, FRAME_MS, StreamKind, frame_channel};
use sidekick_stt::{SttSink, SttStatus};
use tokio::sync::mpsc::Receiver;

/// How long the stream must be silent before the user is told.
pub const SILENT_AFTER_MS: u32 = 3_000;
/// Loudest sample still counted as digital silence (rounding after resampling).
const MAX_SILENT_SAMPLE: u16 = 2;

/// Counts consecutive silent frames and says when the state changes.
#[derive(Debug, Default)]
pub struct SilenceWatch {
    silent_frames: u32,
    reported: bool,
}

impl SilenceWatch {
    /// Looks at one frame. Returns `Silent` once the stream has been silent
    /// long enough, and `Live` when sound comes back after that.
    pub fn observe(&mut self, samples: &[i16]) -> Option<SttStatus> {
        let silent = samples
            .iter()
            .all(|s| s.unsigned_abs() <= MAX_SILENT_SAMPLE);
        if silent {
            self.silent_frames = self.silent_frames.saturating_add(1);
            if !self.reported && self.silent_frames * FRAME_MS >= SILENT_AFTER_MS {
                self.reported = true;
                return Some(SttStatus::Silent);
            }
            return None;
        }
        self.silent_frames = 0;
        if self.reported {
            self.reported = false;
            return Some(SttStatus::Live);
        }
        None
    }
}

/// Passes every frame through unchanged, reporting silence on `sink` as it goes.
pub fn watch(
    kind: StreamKind,
    mut frames: Receiver<AudioFrame>,
    sink: Arc<dyn SttSink>,
) -> Receiver<AudioFrame> {
    let (tx, rx) = frame_channel();
    tokio::spawn(async move {
        let mut watch = SilenceWatch::default();
        while let Some(frame) = frames.recv().await {
            if let Some(status) = watch.observe(&frame.samples) {
                tracing::warn!(target: "session", ?kind, ?status, "microphone level changed");
                sink.status(kind, status);
            }
            if tx.send(frame).await.is_err() {
                break;
            }
        }
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidekick_audio::FRAME_SAMPLES;

    fn frames_for(ms: u32) -> u32 {
        ms / FRAME_MS
    }

    #[test]
    fn three_seconds_of_zeros_is_reported_once() {
        let mut w = SilenceWatch::default();
        let zeros = vec![0i16; FRAME_SAMPLES];
        let reports: Vec<SttStatus> = (0..frames_for(10_000))
            .filter_map(|_| w.observe(&zeros))
            .collect();
        assert_eq!(reports, vec![SttStatus::Silent]);
    }

    #[test]
    fn not_reported_before_three_seconds() {
        let mut w = SilenceWatch::default();
        let zeros = vec![0i16; FRAME_SAMPLES];
        assert!((0..frames_for(2_900)).all(|_| w.observe(&zeros).is_none()));
    }

    #[test]
    fn a_quiet_room_is_not_silence() {
        // Real microphones always have some noise, even in a quiet room.
        let mut w = SilenceWatch::default();
        let hiss: Vec<i16> = (0..FRAME_SAMPLES).map(|i| [3, -4, 5, -3][i % 4]).collect();
        assert!((0..frames_for(10_000)).all(|_| w.observe(&hiss).is_none()));
    }

    #[test]
    fn sound_after_silence_reports_live_again() {
        let mut w = SilenceWatch::default();
        let zeros = vec![0i16; FRAME_SAMPLES];
        let voice = vec![900i16; FRAME_SAMPLES];
        for _ in 0..frames_for(3_000) {
            w.observe(&zeros);
        }
        assert_eq!(w.observe(&voice), Some(SttStatus::Live));
        assert_eq!(w.observe(&voice), None);
    }

    #[tokio::test]
    async fn watch_forwards_every_frame_and_reports_to_the_sink() {
        use std::sync::Mutex;
        #[derive(Default)]
        struct Statuses(Mutex<Vec<SttStatus>>);
        impl SttSink for Statuses {
            fn partial(&self, _: sidekick_stt::Partial) {}
            fn segment(&self, _: sidekick_stt::Segment) {}
            fn status(&self, _: StreamKind, s: SttStatus) {
                self.0.lock().unwrap().push(s);
            }
        }
        let sink = Arc::new(Statuses::default());
        let (tx, rx) = frame_channel();
        let mut out = watch(StreamKind::Mic, rx, sink.clone());
        let n = frames_for(3_500);
        for i in 0..n {
            tx.send(AudioFrame {
                kind: StreamKind::Mic,
                start_ms: u64::from(i * FRAME_MS),
                samples: vec![0; FRAME_SAMPLES],
                speech: false,
            })
            .await
            .unwrap();
            assert!(out.recv().await.is_some());
        }
        assert_eq!(*sink.0.lock().unwrap(), vec![SttStatus::Silent]);
    }
}
