//! Checking an API key against its provider, for providers that aren't LLMs
//! (STT, embeddings, search). The gateway calls these before saving a key.

use secrecy::SecretString;

use crate::SidekickResult;

#[async_trait::async_trait]
pub trait KeyVerifier: Send + Sync {
    /// Ok if the provider accepts the key; `KEY_INVALID` if it rejects it.
    async fn verify(&self, key: &SecretString) -> SidekickResult<()>;
}
