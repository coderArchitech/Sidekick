//! Deterministic STT for tests and `SIDEKICK_MOCK_PROVIDERS=1`: each burst of
//! speech frames becomes one segment, "Mock speech N".

use std::sync::Arc;

use sidekick_audio::{AudioFrame, FRAME_MS};
use sidekick_core::SidekickResult;
use sidekick_secrets::SecretString;
use tokio::sync::mpsc::Receiver;
use tokio_util::sync::CancellationToken;

use crate::{Partial, Segment, SttConfig, SttProvider, SttSink, SttStatus, speakers};

#[derive(Debug, Clone, Default)]
pub struct MockSttProvider;

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
        let flush = |burst: &mut Option<(u64, u64)>, count: &mut u32| {
            if let Some((start_ms, end_ms)) = burst.take() {
                *count += 1;
                sink.segment(Segment {
                    speaker_label: speakers::label(config.kind, None),
                    is_user: config.kind == sidekick_audio::StreamKind::Mic,
                    text: format!("Mock speech {count}"),
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
