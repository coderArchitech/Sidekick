//! Embeddings, separate from LLMs because not every LLM vendor offers them (ADR-013).

use secrecy::SecretString;

use crate::SidekickResult;

#[async_trait::async_trait]
pub trait EmbeddingProvider: Send + Sync {
    fn id(&self) -> &'static str;
    /// Model id, stored with each document so a model change triggers re-embedding.
    fn model(&self) -> &str;
    /// Vector size; the store sizes `chunk_vectors` to match.
    fn dims(&self) -> usize;
    /// One vector per input text, in order. `key` is `None` for local providers.
    async fn embed(
        &self,
        texts: &[String],
        key: Option<&SecretString>,
    ) -> SidekickResult<Vec<Vec<f32>>>;
}
