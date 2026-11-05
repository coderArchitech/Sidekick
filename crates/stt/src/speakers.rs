//! Speaker labels (PRD LIS-3): the mic stream is always "me"; system audio is
//! split into one segment per run of words from the same diarized speaker.

use sidekick_audio::StreamKind;

use crate::Segment;

/// A recognised word with provider timestamps (ms) and optional speaker index.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: Option<u32>,
}

#[must_use]
pub fn label(kind: StreamKind, speaker: Option<u32>) -> String {
    match kind {
        StreamKind::Mic => "me".into(),
        StreamKind::System => format!("speaker_{}", speaker.unwrap_or(0) + 1),
    }
}

/// Splits a final result into segments, one per speaker run. Times are still
/// provider times; the caller maps them to meeting time.
#[must_use]
pub fn split(kind: StreamKind, words: &[Word], confidence: Option<f32>) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    for w in words {
        let speaker = if kind == StreamKind::Mic {
            None
        } else {
            w.speaker
        };
        let speaker_label = label(kind, speaker);
        match out.last_mut() {
            Some(seg) if seg.speaker_label == speaker_label => {
                seg.text.push(' ');
                seg.text.push_str(&w.text);
                seg.end_ms = w.end_ms;
            }
            _ => out.push(Segment {
                speaker_label,
                is_user: kind == StreamKind::Mic,
                text: w.text.clone(),
                start_ms: w.start_ms,
                end_ms: w.end_ms,
                confidence,
            }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str, start: u64, speaker: Option<u32>) -> Word {
        Word {
            text: text.into(),
            start_ms: start,
            end_ms: start + 200,
            speaker,
        }
    }

    #[test]
    fn mic_is_always_me() {
        let segs = split(
            StreamKind::Mic,
            &[w("Hello", 0, Some(1)), w("there.", 300, Some(2))],
            Some(0.9),
        );
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].speaker_label, "me");
        assert!(segs[0].is_user);
        assert_eq!(segs[0].text, "Hello there.");
    }

    #[test]
    fn system_audio_splits_by_speaker() {
        let segs = split(
            StreamKind::System,
            &[
                w("Priya,", 0, Some(0)),
                w("rate", 300, Some(0)),
                w("limit?", 600, Some(0)),
                w("Yes.", 1_200, Some(1)),
            ],
            None,
        );
        let summary: Vec<(&str, &str, u64, u64)> = segs
            .iter()
            .map(|s| {
                (
                    s.speaker_label.as_str(),
                    s.text.as_str(),
                    s.start_ms,
                    s.end_ms,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("speaker_1", "Priya, rate limit?", 0, 800),
                ("speaker_2", "Yes.", 1_200, 1_400)
            ]
        );
        assert!(segs.iter().all(|s| !s.is_user));
    }

    #[test]
    fn missing_diarization_uses_speaker_1() {
        assert_eq!(label(StreamKind::System, None), "speaker_1");
    }
}
