//! A live session: for each allowed audio stream, one capture feeding one STT run.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use sidekick_audio::{
    CaptureHandle, DeviceSource, FixtureSource, FrameSender, Pacing, StreamKind, frame_channel,
};
use sidekick_core::{SidekickError, SidekickResult};
use sidekick_secrets::SecretString;
use sidekick_stt::{SttConfig, SttProvider, SttSink};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Starts audio capture for a stream. Lets tests and demos swap devices for files.
pub trait AudioFactory: Send + Sync {
    fn start(&self, kind: StreamKind, tx: FrameSender) -> SidekickResult<CaptureHandle>;
}

/// Real microphone and system audio.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeviceAudio;

impl AudioFactory for DeviceAudio {
    fn start(&self, kind: StreamKind, tx: FrameSender) -> SidekickResult<CaptureHandle> {
        DeviceSource { kind }.start(tx)
    }
}

/// Plays a WAV file in real time as the given stream (`SIDEKICK_AUDIO_FIXTURE`).
#[derive(Debug, Clone)]
pub struct FixtureAudio {
    pub path: PathBuf,
    pub pacing: Pacing,
}

impl AudioFactory for FixtureAudio {
    fn start(&self, kind: StreamKind, tx: FrameSender) -> SidekickResult<CaptureHandle> {
        FixtureSource {
            path: self.path.clone(),
            kind,
            pacing: self.pacing,
        }
        .start(tx)
    }
}

/// Which STT provider to use, with its key and settings.
#[derive(Clone)]
pub struct SttPlan {
    pub provider: Arc<dyn SttProvider>,
    pub key: Option<SecretString>,
    pub language: String,
    pub keyterms: Vec<String>,
}

struct Stream {
    kind: StreamKind,
    capture: CaptureHandle,
    task: JoinHandle<SidekickResult<()>>,
}

/// A running session. Must be created inside a Tokio runtime.
pub struct LiveSession {
    streams: Vec<Stream>,
    cancel: CancellationToken,
    /// Non-fatal problems to show the user (e.g. system audio unavailable).
    pub warnings: Vec<String>,
}

impl LiveSession {
    /// Starts every stream in `kinds`. If system audio can't start but the
    /// microphone can, the session continues with the mic and records a warning
    /// (ADR-014: e.g. macOS before 14.6). Any other failure stops everything.
    pub fn start(
        kinds: &[StreamKind],
        audio: &dyn AudioFactory,
        stt: &SttPlan,
        sink: Arc<dyn SttSink>,
    ) -> SidekickResult<Self> {
        let cancel = CancellationToken::new();
        let mut streams = Vec::new();
        let mut warnings = Vec::new();
        for &kind in kinds {
            let (tx, rx) = frame_channel();
            match audio.start(kind, tx) {
                Ok(capture) => {
                    let mut cfg = SttConfig::new(kind);
                    cfg.language.clone_from(&stt.language);
                    cfg.keyterms.clone_from(&stt.keyterms);
                    let provider = Arc::clone(&stt.provider);
                    let key = stt.key.clone();
                    let sink = Arc::clone(&sink);
                    let cancel = cancel.clone();
                    let task =
                        tokio::spawn(async move { provider.run(cfg, key, rx, sink, cancel).await });
                    streams.push(Stream {
                        kind,
                        capture,
                        task,
                    });
                }
                Err(e) if kind == StreamKind::System && kinds.contains(&StreamKind::Mic) => {
                    tracing::warn!(target: "session", code = ?e.code, "system audio unavailable; continuing with the microphone");
                    warnings.push(format!(
                        "Only your microphone is being transcribed. {}",
                        e.message
                    ));
                }
                Err(e) => {
                    cancel.cancel();
                    for s in streams {
                        s.capture.stop();
                        s.task.abort();
                    }
                    return Err(e);
                }
            }
        }
        if streams.is_empty() {
            return Err(SidekickError::internal("No audio stream could be started."));
        }
        Ok(Self {
            streams,
            cancel,
            warnings,
        })
    }

    /// Streams that are running.
    #[must_use]
    pub fn kinds(&self) -> Vec<StreamKind> {
        self.streams.iter().map(|s| s.kind).collect()
    }

    /// Stops listening; audio during the pause is discarded, never sent (LIS-6).
    pub fn pause(&self) {
        for s in &self.streams {
            s.capture.pause();
        }
    }

    pub fn resume(&self) {
        for s in &self.streams {
            s.capture.resume();
        }
    }

    /// Stops capture, lets STT flush its last results (up to 5 s), then ends.
    pub async fn stop(self) {
        let Self {
            streams, cancel, ..
        } = self;
        let mut tasks = Vec::new();
        for s in streams {
            // Stopping the capture closes the frame channel, so the STT run finishes cleanly.
            s.capture.stop();
            tasks.push(s.task);
        }
        for task in tasks {
            let abort = task.abort_handle();
            match tokio::time::timeout(Duration::from_secs(5), task).await {
                Ok(Ok(Err(e))) => {
                    tracing::warn!(target: "session", code = ?e.code, "stt ended with an error")
                }
                Err(_) => abort.abort(),
                _ => {}
            }
        }
        cancel.cancel();
    }
}
