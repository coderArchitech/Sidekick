//! OpenAI embeddings (`POST /v1/embeddings`) and key check (`GET /v1/models`).
//! Chat completions arrive with T-026.

use serde_json::{Value, json};
use sidekick_core::embed::EmbeddingProvider;
use sidekick_core::keys::KeyVerifier;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_secrets::{ExposeSecret, SecretString};

use super::http::{status_error, transport_error};

pub const DEFAULT_BASE_URL: &str = "https://api.openai.com";
pub const DEFAULT_EMBED_MODEL: &str = "text-embedding-3-small";
const NAME: &str = "OpenAI";

/// Known embedding sizes. Unknown models are rejected rather than guessed.
#[must_use]
pub fn dims_for(model: &str) -> Option<usize> {
    match model {
        "text-embedding-3-small" | "text-embedding-ada-002" => Some(1536),
        "text-embedding-3-large" => Some(3072),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiEmbedder {
    base_url: String,
    model: String,
    dims: usize,
    http: reqwest::Client,
}

impl OpenAiEmbedder {
    pub fn new(base_url: impl Into<String>, model: &str) -> SidekickResult<Self> {
        let dims = dims_for(model).ok_or_else(|| {
            SidekickError::new(
                ErrorCode::Internal,
                format!("Sidekick doesn't know the OpenAI embedding model {model}."),
            )
        })?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            model: model.to_owned(),
            dims,
            http: reqwest::Client::new(),
        })
    }
}

async fn check(resp: reqwest::Response) -> SidekickResult<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let retry = resp
        .headers()
        .get("retry-after")
        .and_then(|h| h.to_str().ok())
        .map(str::to_owned);
    Err(status_error(NAME, resp.status().as_u16(), retry.as_deref()))
}

#[async_trait::async_trait]
impl EmbeddingProvider for OpenAiEmbedder {
    fn id(&self) -> &'static str {
        "openai"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn dims(&self) -> usize {
        self.dims
    }

    async fn embed(
        &self,
        texts: &[String],
        key: Option<&SecretString>,
    ) -> SidekickResult<Vec<Vec<f32>>> {
        let key = key.ok_or_else(|| {
            SidekickError::new(
                ErrorCode::KeyMissing,
                "Add your OpenAI API key in Settings.",
            )
        })?;
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let resp = self
            .http
            .post(format!("{}/v1/embeddings", self.base_url))
            .bearer_auth(key.expose_secret())
            .json(&json!({ "model": self.model, "input": texts }))
            .send()
            .await
            .map_err(|e| transport_error(NAME, &e))?;
        let body: Value = check(resp).await?.json().await.map_err(|_| {
            SidekickError::new(
                ErrorCode::ProviderUnavailable,
                "Unexpected reply from OpenAI.",
            )
        })?;
        let mut rows: Vec<(usize, Vec<f32>)> = body["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|d| {
                let index = usize::try_from(d["index"].as_u64()?).ok()?;
                #[allow(clippy::cast_possible_truncation)]
                let v = d["embedding"]
                    .as_array()?
                    .iter()
                    .filter_map(|x| x.as_f64().map(|f| f as f32))
                    .collect();
                Some((index, v))
            })
            .collect();
        rows.sort_by_key(|(i, _)| *i);
        if rows.len() != texts.len() || rows.iter().any(|(_, v)| v.len() != self.dims) {
            return Err(SidekickError::new(
                ErrorCode::ProviderUnavailable,
                "OpenAI returned an unexpected number of embeddings.",
            ));
        }
        Ok(rows.into_iter().map(|(_, v)| v).collect())
    }
}

/// Checks an OpenAI key with `GET /v1/models`.
#[derive(Debug, Clone)]
pub struct OpenAiVerifier {
    base_url: String,
    http: reqwest::Client,
}

impl OpenAiVerifier {
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            http: reqwest::Client::new(),
        }
    }
}

impl Default for OpenAiVerifier {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

#[async_trait::async_trait]
impl KeyVerifier for OpenAiVerifier {
    async fn verify(&self, key: &SecretString) -> SidekickResult<()> {
        let resp = self
            .http
            .get(format!("{}/v1/models", self.base_url))
            .bearer_auth(key.expose_secret())
            .send()
            .await
            .map_err(|e| transport_error(NAME, &e))?;
        check(resp).await.map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_model_sizes() {
        assert_eq!(dims_for("text-embedding-3-small"), Some(1536));
        assert_eq!(dims_for("text-embedding-3-large"), Some(3072));
        assert_eq!(dims_for("made-up"), None);
        assert!(OpenAiEmbedder::new(DEFAULT_BASE_URL, "made-up").is_err());
    }
}
