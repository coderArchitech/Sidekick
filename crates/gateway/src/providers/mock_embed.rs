//! Deterministic local embeddings for tests and `SIDEKICK_MOCK_PROVIDERS=1`.
//!
//! A hashed bag of words: texts sharing words get similar vectors, so retrieval
//! tests behave sensibly without a network or model.

use sidekick_core::SidekickResult;
use sidekick_core::embed::EmbeddingProvider;
use sidekick_secrets::SecretString;

pub const MOCK_DIMS: usize = 256;

#[derive(Debug, Clone, Default)]
pub struct MockEmbedder;

/// Lowercase alphanumeric words, as used by the mock (and handy for tests).
pub fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1)
        .map(str::to_lowercase)
}

fn fnv(word: &str) -> u64 {
    word.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[must_use]
pub fn embed_one(text: &str) -> Vec<f32> {
    let mut v = vec![0.0_f32; MOCK_DIMS];
    for w in words(text) {
        #[allow(clippy::cast_possible_truncation)]
        let i = (fnv(&w) % MOCK_DIMS as u64) as usize;
        v[i] += 1.0;
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        v.iter_mut().for_each(|x| *x /= norm);
    }
    v
}

#[async_trait::async_trait]
impl EmbeddingProvider for MockEmbedder {
    fn id(&self) -> &'static str {
        "mock"
    }

    fn model(&self) -> &str {
        "mock-bow-256"
    }

    fn dims(&self) -> usize {
        MOCK_DIMS
    }

    async fn embed(
        &self,
        texts: &[String],
        _key: Option<&SecretString>,
    ) -> SidekickResult<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|t| embed_one(t)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dot(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b).map(|(x, y)| x * y).sum()
    }

    #[test]
    fn similar_texts_are_closer() {
        let q = embed_one("Pro tier rate limit");
        let near = embed_one("The Pro tier rate limit is 600 requests per minute");
        let far = embed_one("Lunch is served at noon in the cafeteria");
        assert!(dot(&q, &near) > dot(&q, &far));
        assert!((dot(&near, &near) - 1.0).abs() < 1e-5, "unit length");
        assert_eq!(embed_one("same text"), embed_one("same text"));
    }
}
