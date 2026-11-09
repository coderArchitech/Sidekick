//! Sink wrappers that make the other person's speech reliable (PRD LIS-2,
//! QA-1): they sit between the speech-to-text streams and the app's sink.
//!
//! - [`EchoFilter`]: on speakers (no headphones) the microphone also hears the
//!   other person, so their words came back a second time as "Me". Those mic
//!   lines are dropped when they repeat what the system stream just heard.
//! - [`RoomSink`]: when system audio can't start, the microphone is the only
//!   way to hear the other person. Its lines are then marked as the room
//!   (not the user), so questions in them are detected and answered.

use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use sidekick_audio::StreamKind;
use sidekick_stt::{Partial, Segment, SttSink, SttStatus};

/// How far apart in time a mic line and a system line may start to be the same speech.
const ECHO_WINDOW_MS: u64 = 4_000;
/// Share of the mic line's words that must appear in the system line.
const ECHO_OVERLAP: f32 = 0.6;
/// Shorter mic lines are kept: "yes" or "okay" can't be told apart from an echo.
const ECHO_MIN_WORDS: usize = 3;
/// How long other speakers' lines are remembered.
const MEMORY_MS: u64 = 30_000;

/// The speaker label for microphone lines in room mode.
pub const ROOM_LABEL: &str = "room";

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

struct Heard {
    words: HashSet<String>,
    start_ms: u64,
    end_ms: u64,
}

/// Drops microphone lines that echo what another speaker just said.
pub struct EchoFilter {
    inner: Arc<dyn SttSink>,
    recent: Mutex<VecDeque<Heard>>,
}

impl EchoFilter {
    pub fn new(inner: Arc<dyn SttSink>) -> Self {
        Self {
            inner,
            recent: Mutex::new(VecDeque::new()),
        }
    }

    /// Whether `seg` (a user line) repeats a recent line from another speaker.
    #[must_use]
    pub fn is_echo(&self, seg: &Segment) -> bool {
        let mine = words(&seg.text);
        if mine.len() < ECHO_MIN_WORDS {
            return false;
        }
        let Ok(recent) = self.recent.lock() else {
            return false;
        };
        recent.iter().any(|h| {
            let near = seg.start_ms.abs_diff(h.start_ms) <= ECHO_WINDOW_MS
                || (seg.start_ms <= h.end_ms && h.start_ms <= seg.end_ms);
            if !near {
                return false;
            }
            let shared = mine.iter().filter(|w| h.words.contains(*w)).count();
            #[allow(clippy::cast_precision_loss)]
            let share = shared as f32 / mine.len() as f32;
            share >= ECHO_OVERLAP
        })
    }

    fn remember(&self, seg: &Segment) {
        if let Ok(mut recent) = self.recent.lock() {
            recent.push_back(Heard {
                words: words(&seg.text).into_iter().collect(),
                start_ms: seg.start_ms,
                end_ms: seg.end_ms,
            });
            while recent
                .front()
                .is_some_and(|h| h.end_ms + MEMORY_MS < seg.end_ms)
            {
                recent.pop_front();
            }
        }
    }
}

impl SttSink for EchoFilter {
    fn partial(&self, p: Partial) {
        self.inner.partial(p);
    }

    fn segment(&self, s: Segment) {
        if s.is_user {
            if self.is_echo(&s) {
                tracing::debug!(target: "session", "dropped a microphone echo of the other speaker");
                return;
            }
        } else {
            self.remember(&s);
        }
        self.inner.segment(s);
    }

    fn status(&self, kind: StreamKind, status: SttStatus) {
        self.inner.status(kind, status);
    }
}

/// Marks microphone lines as the room, for sessions without system audio.
pub struct RoomSink {
    inner: Arc<dyn SttSink>,
}

impl RoomSink {
    pub fn new(inner: Arc<dyn SttSink>) -> Self {
        Self { inner }
    }
}

impl SttSink for RoomSink {
    fn partial(&self, p: Partial) {
        self.inner.partial(p);
    }

    fn segment(&self, mut s: Segment) {
        if s.is_user {
            s.is_user = false;
            ROOM_LABEL.clone_into(&mut s.speaker_label);
        }
        self.inner.segment(s);
    }

    fn status(&self, kind: StreamKind, status: SttStatus) {
        self.inner.status(kind, status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Collect(Mutex<Vec<Segment>>);

    impl SttSink for Collect {
        fn partial(&self, _: Partial) {}
        fn segment(&self, s: Segment) {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(s);
        }
        fn status(&self, _: StreamKind, _: SttStatus) {}
    }

    fn seg(user: bool, text: &str, start_ms: u64) -> Segment {
        Segment {
            speaker_label: if user {
                "me".into()
            } else {
                "speaker_1".into()
            },
            is_user: user,
            text: text.into(),
            start_ms,
            end_ms: start_ms + 2_000,
            confidence: None,
        }
    }

    fn texts(c: &Collect) -> Vec<(bool, String)> {
        c.0.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|s| (s.is_user, s.text.clone()))
            .collect()
    }

    #[test]
    fn drops_mic_echoes_of_the_other_speaker_but_keeps_the_user() {
        let out = Arc::new(Collect::default());
        let f = EchoFilter::new(out.clone());
        f.segment(seg(false, "Design a URL shortener like bitly.", 10_000));
        // The mic heard the speakers: a slightly different transcript of the same words.
        f.segment(seg(true, "design a URL shortener like Bitly", 10_400));
        // The user's own answer.
        f.segment(seg(
            true,
            "Sure, I would start with the requirements.",
            13_000,
        ));
        // Short replies are never treated as echoes.
        f.segment(seg(true, "Okay", 10_500));
        // The same words much later are the user's (e.g. repeating the question).
        f.segment(seg(true, "Design a URL shortener like bitly", 40_000));
        assert_eq!(
            texts(&out),
            [
                (false, "Design a URL shortener like bitly.".to_owned()),
                (
                    true,
                    "Sure, I would start with the requirements.".to_owned()
                ),
                (true, "Okay".to_owned()),
                (true, "Design a URL shortener like bitly".to_owned()),
            ]
        );
    }

    #[test]
    fn forgets_old_lines() {
        let f = EchoFilter::new(Arc::new(Collect::default()));
        f.segment(seg(false, "What is the time complexity here?", 0));
        f.segment(seg(false, "Another line much later on", 100_000));
        assert_eq!(f.recent.lock().map(|r| r.len()).unwrap_or(0), 1);
    }

    #[test]
    fn room_mode_marks_mic_lines_as_the_room() {
        let out = Arc::new(Collect::default());
        let room = RoomSink::new(out.clone());
        room.segment(seg(true, "Can you explain the event loop?", 0));
        let got = out
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert!(!got[0].is_user);
        assert_eq!(got[0].speaker_label, ROOM_LABEL);
    }
}
