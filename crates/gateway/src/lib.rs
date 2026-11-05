//! Sidekick's gateway to external providers (tech doc 3.2 `gateway`).
//!
//! The only crate that attaches API keys to requests. Keys are read from the
//! [`KeyStore`] for each call and never returned to callers (NFR-7).

pub mod catalog;
pub mod providers;
pub mod retry;
mod sse;
pub mod streams;

use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;
use sidekick_core::keys::KeyVerifier;
use sidekick_core::llm::{LlmProvider, LlmRequest, ModelInfo};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_secrets::{KeyStatus, KeyStore, SecretString};

pub use catalog::{ProviderInfo, ProviderKind};
pub use providers::anthropic::{DEFAULT_ANSWER_MODEL, DEFAULT_CUE_MODEL};
use retry::{RetryPolicy, with_retries};
use streams::{AnswerSink, StreamManager};

/// A catalog entry with the state of its saved key.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ProviderEntry {
    #[serde(flatten)]
    pub info: ProviderInfo,
    pub key: KeyStatus,
}

/// Result of saving or testing a key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct KeyCheck {
    pub key: KeyStatus,
    /// True if the provider accepted the key. False means it was saved without
    /// a live check because this provider's adapter isn't built yet.
    pub verified: bool,
}

pub struct Gateway {
    llms: HashMap<&'static str, Arc<dyn LlmProvider>>,
    /// Key checks for non-LLM providers (STT, embeddings, search).
    verifiers: HashMap<&'static str, Arc<dyn KeyVerifier>>,
    keys: Arc<dyn KeyStore>,
    policy: RetryPolicy,
    streams: StreamManager,
    mock: bool,
}

impl Gateway {
    /// Real providers. With `mock`, every LLM id is served by the deterministic
    /// mock and any non-empty key is accepted (tech doc 6.4).
    pub fn new(keys: Arc<dyn KeyStore>, mock: bool) -> Self {
        let mut llms: HashMap<&'static str, Arc<dyn LlmProvider>> = HashMap::new();
        if mock {
            for p in catalog::of_kind(ProviderKind::Llm) {
                llms.insert(p.id, Arc::new(providers::mock::MockProvider));
            }
        } else {
            llms.insert(
                "anthropic",
                Arc::new(providers::anthropic::AnthropicProvider::default()),
            );
        }
        Self {
            llms,
            verifiers: HashMap::new(),
            keys,
            policy: RetryPolicy::default(),
            streams: StreamManager::default(),
            mock,
        }
    }

    /// Replaces an adapter (tests point Anthropic at a mock server).
    #[must_use]
    pub fn with_llm(mut self, id: &'static str, provider: Arc<dyn LlmProvider>) -> Self {
        self.llms.insert(id, provider);
        self
    }

    /// Registers a key check for a non-LLM provider (e.g. Deepgram from the stt crate).
    #[must_use]
    pub fn with_verifier(mut self, id: &'static str, verifier: Arc<dyn KeyVerifier>) -> Self {
        self.verifiers.insert(id, verifier);
        self
    }

    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Catalog entries of one kind with their key status (`list_providers`).
    pub fn providers(&self, kind: ProviderKind) -> SidekickResult<Vec<ProviderEntry>> {
        catalog::of_kind(kind)
            .into_iter()
            .map(|mut info| {
                if self.mock && info.kind == ProviderKind::Llm {
                    info.implemented = true;
                }
                if info.kind != ProviderKind::Llm && self.verifiers.contains_key(info.id) {
                    info.implemented = true;
                }
                Ok(ProviderEntry {
                    key: self.keys.status(info.id)?,
                    info,
                })
            })
            .collect()
    }

    /// Tests a key and saves it only if the provider accepts it (`set_api_key`).
    pub async fn set_key(&self, provider: &str, key: SecretString) -> SidekickResult<KeyCheck> {
        use sidekick_secrets::ExposeSecret;
        known(provider)?;
        if key.expose_secret().trim().is_empty() {
            return Err(SidekickError::new(
                ErrorCode::KeyMissing,
                "Paste a key first.",
            ));
        }
        let verified = self.verify(provider, &key).await?;
        self.keys.set(provider, &key)?;
        Ok(KeyCheck {
            key: self.keys.status(provider)?,
            verified,
        })
    }

    /// Re-tests the saved key (`test_api_key`).
    pub async fn test_key(&self, provider: &str) -> SidekickResult<KeyCheck> {
        known(provider)?;
        let key = self
            .keys
            .get(provider)?
            .ok_or_else(|| key_missing(provider))?;
        let verified = self.verify(provider, &key).await?;
        Ok(KeyCheck {
            key: self.keys.status(provider)?,
            verified,
        })
    }

    /// Removes a saved key (`delete_api_key`).
    pub fn delete_key(&self, provider: &str) -> SidekickResult<()> {
        known(provider)?;
        self.keys.delete(provider)
    }

    /// Models for the model dropdown (`list_models`).
    pub async fn list_models(&self, provider: &str) -> SidekickResult<Vec<ModelInfo>> {
        let llm = self.llm(provider)?;
        let key = self.key_for(provider)?;
        with_retries(&self.policy, || llm.list_models(key.as_ref())).await
    }

    /// Starts a streaming answer (`llm_stream`). Errors that are known up front
    /// (unknown provider, missing key) are returned here; later ones go to the sink.
    pub fn start_stream(
        &self,
        request_id: String,
        provider: &str,
        req: LlmRequest,
        sink: Arc<dyn AnswerSink>,
    ) -> SidekickResult<()> {
        let llm = self.llm(provider)?;
        let key = self.key_for(provider)?;
        self.streams
            .start(request_id, llm, req, key, self.policy.clone(), sink);
        Ok(())
    }

    /// Cancels a stream (`cancel_stream`). Returns false if it had already finished.
    pub fn cancel_stream(&self, request_id: &str) -> bool {
        self.streams.cancel(request_id)
    }

    pub fn active_streams(&self) -> usize {
        self.streams.active_count()
    }

    fn llm(&self, provider: &str) -> SidekickResult<Arc<dyn LlmProvider>> {
        self.llms.get(provider).cloned().ok_or_else(|| {
            SidekickError::new(
                ErrorCode::Internal,
                format!("{provider} isn't available yet in this version."),
            )
        })
    }

    /// The saved key, or `None` for providers that need none.
    fn key_for(&self, provider: &str) -> SidekickResult<Option<SecretString>> {
        if self.mock || catalog::key_required(provider) == Some(false) {
            return Ok(None);
        }
        self.keys
            .get(provider)?
            .map(Some)
            .ok_or_else(|| key_missing(provider))
    }

    async fn verify(&self, provider: &str, key: &SecretString) -> SidekickResult<bool> {
        if self.mock {
            return Ok(true);
        }
        if let Some(llm) = self.llms.get(provider) {
            return with_retries(&self.policy, || llm.list_models(Some(key)))
                .await
                .map(|_| true);
        }
        match self.verifiers.get(provider) {
            Some(v) => with_retries(&self.policy, || v.verify(key))
                .await
                .map(|()| true),
            None => Ok(false), // No adapter yet: save unverified.
        }
    }
}

fn known(provider: &str) -> SidekickResult<()> {
    catalog::key_required(provider)
        .map(|_| ())
        .ok_or_else(|| SidekickError::new(ErrorCode::Internal, "Unknown provider."))
}

fn key_missing(provider: &str) -> SidekickError {
    SidekickError::new(
        ErrorCode::KeyMissing,
        format!("Add your {provider} API key in Settings."),
    )
}
