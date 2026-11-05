//! Runs answer streams in the background and lets them be cancelled (tech doc 5.1
//! `llm_stream` / `cancel_stream`, 5.2 `answer.*` events).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use futures::StreamExt;
use sidekick_core::llm::{FinishReason, LlmProvider, LlmRequest, StreamEvent, Usage};
use sidekick_core::{ErrorCode, SidekickError};
use sidekick_secrets::SecretString;
use tokio_util::sync::CancellationToken;

use sidekick_diagnostics::{Recorder, Stage};

use crate::retry::{RetryPolicy, with_retries};

/// Where stream output goes. The app shell implements this with Tauri events,
/// so this crate never depends on Tauri (tech doc 7.8).
pub trait AnswerSink: Send + Sync {
    fn token(&self, request_id: &str, delta: &str);
    fn done(&self, request_id: &str, usage: Usage, finish_reason: FinishReason);
    fn error(&self, request_id: &str, error: &SidekickError);
}

#[derive(Default)]
pub struct StreamManager {
    active: Arc<Mutex<HashMap<String, CancellationToken>>>,
    recorder: Option<Arc<Recorder>>,
}

impl StreamManager {
    /// Records time-to-first-token for each stream (tech doc 10.1).
    #[must_use]
    pub fn with_recorder(recorder: Arc<Recorder>) -> Self {
        Self {
            active: Arc::default(),
            recorder: Some(recorder),
        }
    }

    /// Starts streaming in the background. Exactly one terminal call
    /// (`done` or `error`) is made on the sink per request.
    pub fn start(
        &self,
        request_id: String,
        provider: Arc<dyn LlmProvider>,
        req: LlmRequest,
        key: Option<SecretString>,
        policy: RetryPolicy,
        sink: Arc<dyn AnswerSink>,
    ) {
        let token = CancellationToken::new();
        let previous = self
            .active
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(request_id.clone(), token.clone());
        if let Some(old) = previous {
            old.cancel(); // Reusing an id replaces the old stream.
        }
        let active = Arc::clone(&self.active);
        let recorder = self.recorder.clone();
        tokio::spawn(async move {
            let started = std::time::Instant::now();
            let run = async {
                // Retries cover opening the stream only; once tokens flow, a
                // failure is reported rather than silently restarting the answer.
                let mut stream =
                    with_retries(&policy, || provider.stream(req.clone(), key.as_ref())).await?;
                let mut first_token = true;
                while let Some(item) = stream.next().await {
                    match item? {
                        StreamEvent::Delta(delta) => {
                            if first_token {
                                first_token = false;
                                if let Some(r) = &recorder {
                                    r.record(Stage::LlmFirstToken, started.elapsed());
                                }
                                tracing::info!(target: "gateway", request_id, first_token_ms = started.elapsed().as_millis() as u64, "first token");
                            }
                            sink.token(&request_id, &delta);
                        }
                        StreamEvent::Done {
                            usage,
                            finish_reason,
                        } => return Ok((usage, finish_reason)),
                    }
                }
                Err(SidekickError::new(
                    ErrorCode::ProviderUnavailable,
                    "The answer stream ended early.",
                ))
            };
            let outcome = tokio::select! {
                () = token.cancelled() => Err(SidekickError::new(ErrorCode::Cancelled, "Cancelled.")),
                r = run => r,
            };
            match outcome {
                Ok((usage, finish)) => sink.done(&request_id, usage, finish),
                Err(e) => sink.error(&request_id, &e),
            }
            let mut map = active.lock().unwrap_or_else(PoisonError::into_inner);
            // Only remove our own token; a newer stream may have reused the id.
            if map
                .get(&request_id)
                .is_some_and(|t| t.is_cancelled() || !token.is_cancelled())
            {
                map.remove(&request_id);
            }
        });
    }

    /// Cancels a stream. Returns false if no such stream is running.
    pub fn cancel(&self, request_id: &str) -> bool {
        match self
            .active
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(request_id)
        {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    /// Number of streams in flight (diagnostics and tests).
    pub fn active_count(&self) -> usize {
        self.active
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}
