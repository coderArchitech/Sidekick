//! Streaming speech-to-text (tech doc 3.2 `stt`, PRD LIS-2, LIS-3, ADR-005).
//!
//! A provider consumes [`AudioFrame`]s from one audio stream and reports
//! partial text, finished [`Segment`]s and connection status to an [`SttSink`].
//! One provider run per stream: the mic run labels everything "me"; the
//! system-audio run labels speakers `speaker_1`, `speaker_2`, ...
//!
//! Cost control: silent frames (per the audio VAD) are not sent, apart from a
//! short tail after speech; [`timeline::Timeline`] maps provider timestamps back
//! to meeting time, and keep-alives hold the connection open in silence.

pub mod models;
pub mod providers;
pub mod speakers;
pub mod timeline;

use std::time::Duration;

use sidekick_audio::{AudioFrame, StreamKind};
use sidekick_core::SidekickResult;
use sidekick_secrets::SecretString;
use tokio::sync::mpsc::Receiver;
use tokio_util::sync::CancellationToken;

pub use providers::deepgram::{DeepgramProvider, DeepgramVerifier};
pub use providers::mock::MockSttProvider;

/// One finalised utterance from one speaker (tech doc 4.1 "Segment").
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// `me`, or `speaker_1`, `speaker_2`, ...
    pub speaker_label: String,
    pub is_user: bool,
    pub text: String,
    /// Milliseconds from session start.
    pub start_ms: u64,
    pub end_ms: u64,
    pub confidence: Option<f32>,
}

/// Interim text, replaced by the next partial or a final segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partial {
    pub kind: StreamKind,
    pub text: String,
}

/// Connection state, shown in the UI and diagnostics (`stt.status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SttStatus {
    Connecting,
    Live,
    Reconnecting,
    Failed,
}

/// Where transcription output goes. The app shell turns these into events.
pub trait SttSink: Send + Sync {
    fn partial(&self, partial: Partial);
    fn segment(&self, segment: Segment);
    fn status(&self, kind: StreamKind, status: SttStatus);
}

/// Most keyterms sent to a provider (Deepgram and AssemblyAI accept 100).
pub const MAX_KEYTERMS: usize = 100;

/// The keyterms a provider gets: the first `MAX_KEYTERMS` (names come first).
#[must_use]
pub fn keyterms_for(cfg: &SttConfig) -> &[String] {
    &cfg.keyterms[..cfg.keyterms.len().min(MAX_KEYTERMS)]
}

/// Whisper's prompt: keyterms joined, cut to stay inside its prompt window.
#[must_use]
pub fn keyterm_prompt(cfg: &SttConfig, max_chars: usize) -> String {
    let mut prompt = String::new();
    for t in keyterms_for(cfg) {
        let extra = if prompt.is_empty() {
            t.len()
        } else {
            t.len() + 2
        };
        if prompt.len() + extra > max_chars {
            break;
        }
        if !prompt.is_empty() {
            prompt.push_str(", ");
        }
        prompt.push_str(t);
    }
    prompt
}

/// Settings for one provider run.
#[derive(Debug, Clone)]
pub struct SttConfig {
    pub kind: StreamKind,
    /// BCP-47 code, or `multi` for code-switching (e.g. Hinglish).
    pub language: String,
    /// Words to boost: the user's name and nicknames (PRD LIS-5a), then the
    /// custom vocabulary (LIS-5). Each provider sends at most `MAX_KEYTERMS`.
    pub keyterms: Vec<String>,
    /// Silence longer than this sends a keep-alive.
    pub keepalive: Duration,
    /// Give up reconnecting after this long (tech doc 10.2: notice after 10 s).
    pub reconnect_deadline: Duration,
    /// Frames still sent after speech ends, so the provider can finish words.
    pub tail_frames: u32,
}

impl SttConfig {
    #[must_use]
    pub fn new(kind: StreamKind) -> Self {
        Self {
            kind,
            language: "en".into(),
            keyterms: Vec::new(),
            keepalive: Duration::from_secs(5),
            reconnect_deadline: Duration::from_secs(10),
            tail_frames: 10, // 500 ms
        }
    }
}

/// A streaming speech-to-text service.
#[async_trait::async_trait]
pub trait SttProvider: Send + Sync {
    fn id(&self) -> &'static str;

    /// Transcribes `frames` until the channel closes or `cancel` fires.
    /// Reconnects on its own; returns an error only when it gives up.
    async fn run(
        &self,
        config: SttConfig,
        key: Option<SecretString>,
        frames: Receiver<AudioFrame>,
        sink: std::sync::Arc<dyn SttSink>,
        cancel: CancellationToken,
    ) -> SidekickResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whisper_prompt_stays_inside_its_window() {
        let mut cfg = SttConfig::new(sidekick_audio::StreamKind::Mic);
        cfg.keyterms = (0..150).map(|i| format!("Vocabulary{i}")).collect();
        let p = keyterm_prompt(&cfg, 600);
        assert!(p.len() <= 600, "{}", p.len());
        assert!(p.starts_with("Vocabulary0, Vocabulary1, "));
        assert_eq!(
            keyterm_prompt(&SttConfig::new(sidekick_audio::StreamKind::Mic), 600),
            ""
        );
    }
}
