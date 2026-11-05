//! Local Whisper on a real model (Task T-027). Opt-in: needs the
//! `local-whisper` feature, a model and a speech recording, so it's ignored by
//! default. See docs/phase-1-manual-checks.md (T-027).
//!
//! ```text
//! SIDEKICK_WHISPER_MODEL=~/models/ggml-tiny.en.bin \
//! SIDEKICK_WHISPER_WAV=short-english.wav \
//! SIDEKICK_WHISPER_EXPECT="what is the rate limit for the pro tier" \
//! cargo test -p sidekick-stt --features local-whisper --test whisper -- --ignored
//! ```

#![cfg(feature = "local-whisper")]
#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Mutex};

use sidekick_audio::{FixtureSource, Pacing, StreamKind, frame_channel};
use sidekick_stt::providers::whisper::WhisperProvider;
use sidekick_stt::{Partial, Segment, SttConfig, SttProvider, SttSink, SttStatus};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct Sink(Mutex<Vec<Segment>>);

impl SttSink for Sink {
    fn partial(&self, _: Partial) {}
    fn segment(&self, s: Segment) {
        self.0.lock().unwrap().push(s);
    }
    fn status(&self, _: StreamKind, _: SttStatus) {}
}

fn words(s: &str) -> Vec<String> {
    s.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

#[tokio::test]
#[ignore = "needs SIDEKICK_WHISPER_MODEL, SIDEKICK_WHISPER_WAV and SIDEKICK_WHISPER_EXPECT"]
async fn whisper_transcribes_the_short_english_fixture() {
    let model = std::env::var("SIDEKICK_WHISPER_MODEL").unwrap();
    let wav = std::env::var("SIDEKICK_WHISPER_WAV").unwrap();
    let expect = words(&std::env::var("SIDEKICK_WHISPER_EXPECT").unwrap());

    let (tx, rx) = frame_channel();
    let _capture = FixtureSource {
        path: wav.into(),
        kind: StreamKind::Mic,
        pacing: Pacing::Fast,
    }
    .start(tx)
    .unwrap();
    let sink = Arc::new(Sink::default());
    WhisperProvider::new(model)
        .run(
            SttConfig::new(StreamKind::Mic),
            None,
            rx,
            sink.clone(),
            CancellationToken::new(),
        )
        .await
        .unwrap();

    let got = words(
        &sink
            .0
            .lock()
            .unwrap()
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" "),
    );
    let found = expect.iter().filter(|w| got.contains(w)).count();
    #[allow(clippy::cast_precision_loss)]
    let recall = found as f64 / expect.len() as f64;
    println!("whisper heard: {got:?} (recall {recall:.2})");
    assert!(
        recall >= 0.8,
        "expected at least 80% of the words, got {recall:.2}: {got:?}"
    );
}
