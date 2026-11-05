//! Local Whisper models: the catalog and a verified download (tech doc 3.6).
//!
//! Files stream to `<name>.part` while being hashed, and are renamed into
//! place only if the digest matches, so a truncated or tampered download is
//! never used. whisper.cpp publishes SHA-1 checksums for its models; the
//! catalog uses those. SHA-256 is supported for models that publish it.

use std::path::{Path, PathBuf};

use futures::StreamExt;
use serde::Serialize;
use sha1::Sha1;
use sha2::{Digest as _, Sha256};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use tokio::io::AsyncWriteExt;

/// An expected file digest, lowercase hex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Digest {
    Sha1(&'static str),
    Sha256(&'static str),
}

/// A downloadable model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct WhisperModel {
    /// `tiny.en`, `base`, ...
    pub name: &'static str,
    pub size_mb: u32,
    /// English only, or multilingual (Hinglish needs multilingual).
    pub english_only: bool,
    #[serde(skip)]
    pub digest: Digest,
}

const BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// From whisper.cpp `models/README.md`. Small models only: larger ones are too
/// slow for live use on the reference laptops.
pub const MODELS: &[WhisperModel] = &[
    WhisperModel {
        name: "tiny.en",
        size_mb: 75,
        english_only: true,
        digest: Digest::Sha1("c78c86eb1a8faa21b369bcd33207cc90d64ae9df"),
    },
    WhisperModel {
        name: "base.en",
        size_mb: 142,
        english_only: true,
        digest: Digest::Sha1("137c40403d78fd54d454da0f9bd998f78703390c"),
    },
    WhisperModel {
        name: "base",
        size_mb: 142,
        english_only: false,
        digest: Digest::Sha1("465707469ff3a37a2b9b8d8f89f2f99de7299dac"),
    },
    WhisperModel {
        name: "small",
        size_mb: 466,
        english_only: false,
        digest: Digest::Sha1("55356645c2b361a969dfd0ef2c5a50d530afd8d5"),
    },
];

#[must_use]
pub fn find(name: &str) -> Option<&'static WhisperModel> {
    MODELS.iter().find(|m| m.name == name)
}

#[must_use]
pub fn file_name(model: &WhisperModel) -> String {
    format!("ggml-{}.bin", model.name)
}

#[must_use]
pub fn url(model: &WhisperModel) -> String {
    format!("{BASE_URL}/{}", file_name(model))
}

/// Where a model lives inside `models_dir`.
#[must_use]
pub fn path(models_dir: &Path, model: &WhisperModel) -> PathBuf {
    models_dir.join(file_name(model))
}

enum Hasher {
    Sha1(Sha1),
    Sha256(Sha256),
}

impl Hasher {
    fn new(d: Digest) -> Self {
        match d {
            Digest::Sha1(_) => Self::Sha1(Sha1::new()),
            Digest::Sha256(_) => Self::Sha256(Sha256::new()),
        }
    }
    fn update(&mut self, b: &[u8]) {
        match self {
            Self::Sha1(h) => h.update(b),
            Self::Sha256(h) => h.update(b),
        }
    }
    fn hex(self) -> String {
        match self {
            Self::Sha1(h) => hex::encode(h.finalize()),
            Self::Sha256(h) => hex::encode(h.finalize()),
        }
    }
}

fn io_err(context: &str, e: &std::io::Error) -> SidekickError {
    tracing::error!(target: "stt", context, error = %e, "model file error");
    SidekickError::internal(format!("Couldn't save the model ({context})."))
}

/// Downloads `url` to `dest`, checking `digest`. `progress(done, total)` is
/// called as bytes arrive. On a mismatch nothing is left at `dest`.
pub async fn download(
    http: &reqwest::Client,
    url: &str,
    dest: &Path,
    digest: Digest,
    mut progress: impl FnMut(u64, Option<u64>) + Send,
) -> SidekickResult<()> {
    let resp = http.get(url).send().await.map_err(|_| {
        SidekickError::new(
            ErrorCode::ProviderUnavailable,
            "Couldn't download the model. Check your internet connection.",
        )
    })?;
    if !resp.status().is_success() {
        return Err(SidekickError::new(
            ErrorCode::ProviderUnavailable,
            format!(
                "The model download failed (HTTP {}).",
                resp.status().as_u16()
            ),
        ));
    }
    let total = resp.content_length();
    if let Some(dir) = dest.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| io_err("folder", &e))?;
    }
    let part = dest.with_extension("part");
    let mut file = tokio::fs::File::create(&part)
        .await
        .map_err(|e| io_err("create", &e))?;
    let mut hasher = Hasher::new(digest);
    let mut done = 0u64;
    let mut body = resp.bytes_stream();
    while let Some(chunk) = body.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(_) => {
                drop(file);
                let _ = tokio::fs::remove_file(&part).await;
                return Err(SidekickError::new(
                    ErrorCode::ProviderUnavailable,
                    "The model download was interrupted. Try again.",
                ));
            }
        };
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|e| io_err("write", &e))?;
        done += chunk.len() as u64;
        progress(done, total);
    }
    file.flush().await.map_err(|e| io_err("write", &e))?;
    drop(file);
    let expected = match digest {
        Digest::Sha1(h) | Digest::Sha256(h) => h,
    };
    if hasher.hex() != expected.to_ascii_lowercase() {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(SidekickError::new(
            ErrorCode::ParseFailed,
            "The downloaded model didn't match its checksum, so it was deleted. Try again.",
        ));
    }
    tokio::fs::rename(&part, dest)
        .await
        .map_err(|e| io_err("rename", &e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_consistent() {
        assert_eq!(find("tiny.en").map(|m| m.size_mb), Some(75));
        assert!(find("large-v3").is_none());
        for m in MODELS {
            let Digest::Sha1(h) = m.digest else { panic!() };
            assert_eq!(h.len(), 40, "{}", m.name);
            assert!(url(m).ends_with(&format!("ggml-{}.bin", m.name)));
        }
    }
}
