//! Local Whisper (whisper.cpp via `whisper-rs`), behind the `local-whisper`
//! feature. Audio never leaves the computer.
//!
//! Whisper isn't streaming: speech frames (per the audio VAD) are collected
//! into an utterance that ends after 600 ms of silence or at 15 s, and each
//! utterance is transcribed on a blocking thread while audio keeps arriving.
//! No partials and no diarization: system audio is `speaker_1`.

use std::path::PathBuf;
use std::sync::Arc;

use sidekick_audio::{AudioFrame, FRAME_MS};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_secrets::SecretString;
use tokio::sync::mpsc::Receiver;
use tokio_util::sync::CancellationToken;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::speakers::{self, Word};
use crate::{SttConfig, SttProvider, SttSink, SttStatus};

/// Silence that ends an utterance.
const END_SILENCE_FRAMES: u32 = 600 / FRAME_MS;
/// Longest utterance sent at once.
const MAX_UTTERANCE_FRAMES: usize = 15_000 / FRAME_MS as usize;

#[derive(Debug, Clone)]
pub struct WhisperProvider {
    model_path: PathBuf,
}

impl WhisperProvider {
    #[must_use]
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
        }
    }
}

/// One utterance: samples and when it started in meeting time.
struct Utterance {
    start_ms: u64,
    samples: Vec<i16>,
}

fn load(path: &PathBuf) -> SidekickResult<WhisperContext> {
    if !path.exists() {
        return Err(SidekickError::new(
            ErrorCode::KeyMissing,
            "Download a Whisper model in Settings → Speech-to-text first.",
        ));
    }
    WhisperContext::new_with_params(path, WhisperContextParameters::default()).map_err(|e| {
        tracing::error!(target: "stt", error = %e, "whisper model load failed");
        SidekickError::internal("Couldn't load the Whisper model. Download it again in Settings.")
    })
}

/// Transcribes one utterance into words with meeting-time stamps.
fn transcribe(ctx: &WhisperContext, cfg: &SttConfig, u: &Utterance) -> SidekickResult<Vec<Word>> {
    let mut state = ctx
        .create_state()
        .map_err(|_| SidekickError::internal("Whisper couldn't start."))?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    let language = if cfg.language == "multi" {
        "auto"
    } else {
        cfg.language.split('-').next().unwrap_or("en")
    };
    params.set_language(Some(language));
    params.set_no_context(true);
    params.set_suppress_blank(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    let threads = std::thread::available_parallelism().map_or(2, |n| n.get().min(4));
    params.set_n_threads(i32::try_from(threads).unwrap_or(2));
    // Whisper's prompt window is about 224 tokens; keep well inside it.
    let prompt = crate::keyterm_prompt(cfg, 600);
    if !prompt.is_empty() {
        // Names in the prompt make Whisper spell them right (PRD LIS-5a).
        params.set_initial_prompt(&prompt);
    }
    let audio: Vec<f32> = u.samples.iter().map(|s| f32::from(*s) / 32_768.0).collect();
    state
        .full(params, &audio)
        .map_err(|_| SidekickError::internal("Whisper couldn't transcribe that audio."))?;
    Ok(state
        .as_iter()
        .filter(|seg| seg.no_speech_probability() < 0.8)
        .filter_map(|seg| {
            let text = seg.to_str_lossy().ok()?.trim().to_owned();
            // Centiseconds from the utterance start.
            let at = |cs: i64| u.start_ms + u64::try_from(cs.max(0)).unwrap_or(0) * 10;
            (!text.is_empty() && !is_noise(&text)).then(|| Word {
                text,
                start_ms: at(seg.start_timestamp()),
                end_ms: at(seg.end_timestamp()),
                speaker: None,
            })
        })
        .collect())
}

/// Whisper's stock hallucinations on near-silence.
fn is_noise(text: &str) -> bool {
    let t = text
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase();
    t.is_empty()
        || matches!(
            t.as_str(),
            "blank_audio" | "music" | "silence" | "inaudible"
        )
}

#[async_trait::async_trait]
impl SttProvider for WhisperProvider {
    fn id(&self) -> &'static str {
        "whisper-local"
    }

    async fn run(
        &self,
        config: SttConfig,
        _key: Option<SecretString>,
        mut frames: Receiver<AudioFrame>,
        sink: Arc<dyn SttSink>,
        cancel: CancellationToken,
    ) -> SidekickResult<()> {
        let kind = config.kind;
        sink.status(kind, SttStatus::Connecting);
        let path = self.model_path.clone();
        let ctx = match tokio::task::spawn_blocking(move || load(&path)).await {
            Ok(Ok(ctx)) => Arc::new(ctx),
            Ok(Err(e)) => {
                sink.status(kind, SttStatus::Failed);
                return Err(e);
            }
            Err(_) => return Err(SidekickError::internal("Whisper stopped unexpectedly.")),
        };
        sink.status(kind, SttStatus::Live);

        // Transcribe on one worker so audio keeps flowing during inference.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Utterance>();
        let worker = {
            let sink = Arc::clone(&sink);
            let cfg = config.clone();
            tokio::spawn(async move {
                while let Some(u) = rx.recv().await {
                    let ctx = Arc::clone(&ctx);
                    let cfg2 = cfg.clone();
                    let words =
                        tokio::task::spawn_blocking(move || transcribe(&ctx, &cfg2, &u)).await;
                    match words {
                        Ok(Ok(words)) => {
                            for seg in speakers::split(cfg.kind, &words, None) {
                                sink.segment(seg);
                            }
                        }
                        Ok(Err(e)) => {
                            tracing::warn!(target: "stt", code = ?e.code, "whisper utterance failed")
                        }
                        Err(_) => tracing::warn!(target: "stt", "whisper worker panicked"),
                    }
                }
            })
        };

        let mut current: Option<Utterance> = None;
        let mut silence = 0u32;
        let mut frames_in = 0usize;
        loop {
            let frame = tokio::select! {
                () = cancel.cancelled() => None,
                f = frames.recv() => f,
            };
            let Some(f) = frame else { break };
            if f.speech {
                silence = 0;
                let u = current.get_or_insert_with(|| {
                    frames_in = 0;
                    Utterance {
                        start_ms: f.start_ms,
                        samples: Vec::new(),
                    }
                });
                u.samples.extend_from_slice(&f.samples);
                frames_in += 1;
            } else if let Some(u) = current.as_mut() {
                u.samples.extend_from_slice(&f.samples);
                frames_in += 1;
                silence += 1;
            }
            if current.is_some()
                && (silence >= END_SILENCE_FRAMES || frames_in >= MAX_UTTERANCE_FRAMES)
            {
                if let Some(u) = current.take() {
                    let _ = tx.send(u);
                }
                silence = 0;
            }
        }
        if !cancel.is_cancelled()
            && let Some(u) = current.take()
        {
            let _ = tx.send(u);
        }
        drop(tx);
        if cancel.is_cancelled() {
            worker.abort();
        } else {
            let _ = worker.await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidekick_audio::StreamKind;

    #[test]
    fn filters_whisper_noise_markers() {
        assert!(is_noise("[BLANK_AUDIO]"));
        assert!(is_noise("(music)"));
        assert!(is_noise(" ... "));
        assert!(!is_noise("Hello there."));
    }

    #[tokio::test]
    async fn missing_model_fails_with_a_clear_message() {
        struct Nop;
        impl SttSink for Nop {
            fn partial(&self, _: crate::Partial) {}
            fn segment(&self, _: crate::Segment) {}
            fn status(&self, _: StreamKind, _: SttStatus) {}
        }
        let (_tx, rx) = tokio::sync::mpsc::channel(1);
        let err = WhisperProvider::new("/nonexistent/ggml-tiny.en.bin")
            .run(
                SttConfig::new(StreamKind::Mic),
                None,
                rx,
                Arc::new(Nop),
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(err.message.contains("Download a Whisper model"));
    }
}
