//! Deterministic STT for tests and `SIDEKICK_MOCK_PROVIDERS=1`: each burst of
//! speech frames becomes one segment: line N of the script if there is one
//! (`SIDEKICK_MOCK_TRANSCRIPT`, E2E tests), otherwise "Mock speech N".

use std::sync::Arc;

use sidekick_audio::{AudioFrame, FRAME_MS};
use sidekick_core::SidekickResult;
use sidekick_secrets::SecretString;
use tokio::sync::mpsc::Receiver;
use tokio_util::sync::CancellationToken;

use crate::{Partial, Segment, SttConfig, SttProvider, SttSink, SttStatus, speakers};

#[derive(Debug, Clone, Default)]
pub struct MockSttProvider {
    /// What each utterance says, in order.
    script: Vec<String>,
}

impl MockSttProvider {
    #[must_use]
    pub fn with_script(script: Vec<String>) -> Self {
        Self { script }
    }

    /// Reads one utterance per non-empty line from `SIDEKICK_MOCK_TRANSCRIPT`, if set.
    #[must_use]
    pub fn from_env() -> Self {
        let script = std::env::var_os("SIDEKICK_MOCK_TRANSCRIPT")
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|t| {
                t.lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        Self { script }
    }
}

#[async_trait::async_trait]
impl SttProvider for MockSttProvider {
    fn id(&self) -> &'static str {
        "mock"
    }

    async fn run(
        &self,
        config: SttConfig,
        _key: Option<SecretString>,
        mut frames: Receiver<AudioFrame>,
        sink: Arc<dyn SttSink>,
        cancel: CancellationToken,
    ) -> SidekickResult<()> {
        sink.status(config.kind, SttStatus::Live);
        let mut burst: Option<(u64, u64)> = None;
        let mut count = 0;
        let script = &self.script;
        let flush = |burst: &mut Option<(u64, u64)>, count: &mut u32| {
            if let Some((start_ms, end_ms)) = burst.take() {
                *count += 1;
                sink.segment(Segment {
                    speaker_label: speakers::label(config.kind, None),
                    is_user: config.kind == sidekick_audio::StreamKind::Mic,
                    text: script
                        .get(*count as usize - 1)
                        .cloned()
                        .unwrap_or_else(|| format!("Mock speech {count}")),
                    start_ms,
                    end_ms,
                    confidence: Some(1.0),
                });
            }
        };
        loop {
            tokio::select! {
                () = cancel.cancelled() => break,
                f = frames.recv() => match f {
                    Some(f) if f.speech => {
                        // A jump in time (e.g. a pause) ends the utterance, like silence does.
                        if burst.is_some_and(|(_, end)| f.start_ms > end) {
                            flush(&mut burst, &mut count);
                        }
                        let end = f.start_ms + u64::from(FRAME_MS);
                        if burst.is_none() {
                            sink.partial(Partial { kind: config.kind, text: "Mock".into() });
                        }
                        burst = Some(burst.map_or((f.start_ms, end), |(s, _)| (s, end)));
                    }
                    Some(_) => flush(&mut burst, &mut count),
                    None => break,
                },
            }
        }
        flush(&mut burst, &mut count);
        Ok(())
    }
}
