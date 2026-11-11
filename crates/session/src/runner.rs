//! A live session: for each allowed audio stream, one capture feeding one STT run.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sidekick_audio::{
    CaptureHandle, DeviceSource, FixtureSource, FrameSender, Pacing, StreamKind, frame_channel,
};
use sidekick_core::{SidekickError, SidekickResult};
use sidekick_secrets::SecretString;
use sidekick_stt::{SttConfig, SttProvider, SttSink};

use crate::sinks::{EchoFilter, RoomSink};
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
    /// The whole session is paused (LIS-6).
    paused: AtomicBool,
    /// The user turned their own transcription off (the microphone stream).
    mic_off: AtomicBool,
    /// Non-fatal problems to show the user (e.g. system audio unavailable).
    pub warnings: Vec<String>,
}

impl LiveSession {
    /// Starts every stream in `kinds`. If system audio can't start but the
    /// microphone can, the session continues with the mic and records a warning
    /// (ADR-014: e.g. macOS before 14.6). Any other failure stops everything.
    ///
    /// System audio starts first, so the microphone knows whether it is the
    /// only way to hear the other person: then its lines count as the room
    /// ([`RoomSink`]), so their questions are answered. With both streams, mic
    /// lines that only echo the other person (speakers, no headphones) are
    /// dropped ([`EchoFilter`]).
    pub fn start(
        kinds: &[StreamKind],
        audio: &dyn AudioFactory,
        stt: &SttPlan,
        sink: Arc<dyn SttSink>,
    ) -> SidekickResult<Self> {
        let cancel = CancellationToken::new();
        let mut streams: Vec<Stream> = Vec::new();
        let mut warnings = Vec::new();
        let echo: Arc<dyn SttSink> = Arc::new(EchoFilter::new(Arc::clone(&sink)));
        let mut order = kinds.to_vec();
        order.sort_by_key(|k| u8::from(*k != StreamKind::System));
        for kind in order {
            let system_running = streams.iter().any(|s| s.kind == StreamKind::System);
            let stream_sink: Arc<dyn SttSink> = match kind {
                StreamKind::System => Arc::clone(&echo),
                StreamKind::Mic if system_running => Arc::clone(&echo),
                // Consent covers others, but system audio failed: hear the room.
                StreamKind::Mic if kinds.contains(&StreamKind::System) => {
                    Arc::new(RoomSink::new(Arc::clone(&sink)))
                }
                StreamKind::Mic => Arc::clone(&sink),
            };
            let (tx, rx) = frame_channel();
            match audio.start(kind, tx) {
                Ok(capture) => {
                    // System audio is often quiet for good reason (nobody else
                    // talking); a microphone of pure zeros means no permission.
                    let rx = if kind == StreamKind::Mic {
                        crate::silence::watch(kind, rx, Arc::clone(&sink))
                    } else {
                        rx
                    };
                    let mut cfg = SttConfig::new(kind);
                    cfg.language.clone_from(&stt.language);
                    cfg.keyterms.clone_from(&stt.keyterms);
                    let provider = Arc::clone(&stt.provider);
                    let key = stt.key.clone();
                    let sink = stream_sink;
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
                        "System audio isn't available, so Sidekick listens through your microphone: it hears the room and answers questions from anyone, including you. Headphones won't work this way. {}",
                        e.message
                    ));
                }
                Err(mut e) => {
                    if kind == StreamKind::System {
                        // "The other people only": there is no other way to hear them.
                        e.message = format!(
                            "{} To listen through your microphone instead, choose \"The other people and me\".",
                            e.message
                        );
                    }
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
            paused: AtomicBool::new(false),
            mic_off: AtomicBool::new(false),
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
        self.paused.store(true, Ordering::SeqCst);
        for s in &self.streams {
            s.capture.pause();
        }
    }

    /// Listens again; the microphone stays off if the user turned it off.
    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
        let mic_off = self.mic_off.load(Ordering::SeqCst);
        for s in &self.streams {
            if !(mic_off && s.kind == StreamKind::Mic) {
                s.capture.resume();
            }
        }
    }

    /// Turns transcription of the user's own voice (the microphone stream) on
    /// or off; the other people keep being transcribed. Audio while it is off
    /// is discarded, never sent. Returns whether the session has a microphone
    /// stream to switch.
    pub fn set_mic(&self, on: bool) -> bool {
        self.mic_off.store(!on, Ordering::SeqCst);
        let Some(mic) = self.streams.iter().find(|s| s.kind == StreamKind::Mic) else {
            return false;
        };
        if !on {
            mic.capture.pause();
        } else if !self.paused.load(Ordering::SeqCst) {
            mic.capture.resume();
        }
        true
    }

    /// Whether the user's own voice is being transcribed.
    #[must_use]
    pub fn mic_on(&self) -> bool {
        self.streams.iter().any(|s| s.kind == StreamKind::Mic)
            && !self.mic_off.load(Ordering::SeqCst)
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
