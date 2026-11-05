//! The ingest pipeline: read → parse → chunk → embed → store, with progress.

use std::sync::{Arc, Mutex, PoisonError};

use serde::Serialize;
use sha2::{Digest, Sha256};
use sidekick_core::embed::EmbeddingProvider;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_gateway::retry::{RetryPolicy, with_retries};
use sidekick_secrets::SecretString;
use sidekick_store::{Database, vectors};

use crate::chunk::chunk_pages;
use crate::parse::{Kind, Page, parse};
use crate::repo::{self, DocStatus, NewDocument};

/// Embedding requests carry at most this many chunks (tech doc 13.5).
pub const BATCH: usize = 100;
/// Largest file accepted (keeps memory and API cost bounded).
pub const MAX_BYTES: usize = 50 * 1024 * 1024;

/// Who a document belongs to.
#[derive(Debug, Clone)]
pub enum Owner {
    Session(String),
    Pack(String),
}

/// Where the content comes from.
#[derive(Debug, Clone)]
pub enum Source {
    File(std::path::PathBuf),
    Url(String),
}

/// Sent as the document moves through the pipeline (`ingest.progress`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub document_id: String,
    pub status: DocStatus,
    /// 0–100.
    pub percent: u8,
    pub error: Option<String>,
}

type Db = Arc<Mutex<Database>>;

fn lock(db: &Db) -> std::sync::MutexGuard<'_, Database> {
    db.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Reads the source into a name and bytes.
async fn read(
    source: &Source,
    http: &reqwest::Client,
) -> SidekickResult<(String, Vec<u8>, Option<Vec<Page>>)> {
    match source {
        Source::File(path) => {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("document")
                .to_owned();
            let meta = tokio::fs::metadata(path).await.map_err(|_| {
                SidekickError::new(ErrorCode::ParseFailed, format!("Couldn't open {name}."))
            })?;
            if usize::try_from(meta.len()).unwrap_or(usize::MAX) > MAX_BYTES {
                return Err(SidekickError::new(
                    ErrorCode::UnsupportedFile,
                    format!("{name} is larger than 50 MB."),
                ));
            }
            let bytes = tokio::fs::read(path).await.map_err(|_| {
                SidekickError::new(ErrorCode::ParseFailed, format!("Couldn't open {name}."))
            })?;
            Ok((name, bytes, None))
        }
        Source::Url(url) => {
            if !(url.starts_with("https://") || url.starts_with("http://")) {
                return Err(SidekickError::new(
                    ErrorCode::UnsupportedFile,
                    "Only http and https links can be added.",
                ));
            }
            let resp = http.get(url).send().await.map_err(|_| {
                SidekickError::new(ErrorCode::ProviderUnavailable, "Couldn't load that link.")
            })?;
            if !resp.status().is_success() {
                return Err(SidekickError::new(
                    ErrorCode::ParseFailed,
                    format!("The link returned HTTP {}.", resp.status().as_u16()),
                ));
            }
            let body = resp.text().await.map_err(|_| {
                SidekickError::new(ErrorCode::ParseFailed, "Couldn't read that page.")
            })?;
            let (title, text) = crate::html::to_text(&body);
            let name = title.unwrap_or_else(|| url.clone());
            Ok((
                name,
                body.into_bytes(),
                Some(vec![Page { number: None, text }]),
            ))
        }
    }
}

/// Ingests one document end to end. Returns the document id (an existing one if
/// the same content is already ready in this session or pack). Progress is
/// reported through `progress`; failures are also stored on the document.
pub async fn ingest(
    db: Db,
    owner: Owner,
    source: Source,
    embedder: Arc<dyn EmbeddingProvider>,
    key: Option<SecretString>,
    progress: Arc<dyn Fn(Progress) + Send + Sync>,
) -> SidekickResult<String> {
    let http = reqwest::Client::new();
    let (name, bytes, pre_parsed) = read(&source, &http).await?;
    let sha = hex::encode(Sha256::digest(&bytes));
    let (session_id, pack_id) = match &owner {
        Owner::Session(s) => (Some(s.as_str()), None),
        Owner::Pack(p) => (None, Some(p.as_str())),
    };
    if let Some(existing) = repo::find_duplicate(&lock(&db), session_id, pack_id, &sha)? {
        tracing::info!(target: "ingest", document_id = %existing, "duplicate skipped");
        progress(Progress {
            document_id: existing.clone(),
            status: DocStatus::Ready,
            percent: 100,
            error: None,
        });
        return Ok(existing);
    }
    let mime = if pre_parsed.is_some() {
        "text/html"
    } else {
        Kind::from_name(&name).map_or("application/octet-stream", Kind::mime)
    };
    let id = repo::insert(
        &lock(&db),
        &NewDocument {
            session_id,
            pack_id,
            name: &name,
            mime_type: mime,
            size_bytes: bytes.len(),
            sha256: &sha,
            embedding_model: embedder.model(),
        },
    )?;
    let report = |status, percent, error: Option<String>| {
        progress(Progress {
            document_id: id.clone(),
            status,
            percent,
            error,
        })
    };
    report(DocStatus::Queued, 0, None);

    match run(
        &db,
        &id,
        &name,
        bytes,
        pre_parsed,
        embedder.as_ref(),
        key.as_ref(),
        &report,
    )
    .await
    {
        Ok(()) => {
            repo::set_status(&lock(&db), &id, DocStatus::Ready, None)?;
            report(DocStatus::Ready, 100, None);
            Ok(id)
        }
        Err(e) => {
            let _ = repo::set_status(&lock(&db), &id, DocStatus::Failed, Some(&e.message));
            report(DocStatus::Failed, 100, Some(e.message.clone()));
            Err(e)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run(
    db: &Db,
    id: &str,
    name: &str,
    bytes: Vec<u8>,
    pre_parsed: Option<Vec<Page>>,
    embedder: &dyn EmbeddingProvider,
    key: Option<&SecretString>,
    report: &(dyn Fn(DocStatus, u8, Option<String>) + Sync),
) -> SidekickResult<()> {
    repo::set_status(&lock(db), id, DocStatus::Parsing, None)?;
    report(DocStatus::Parsing, 5, None);
    let pages = match pre_parsed {
        Some(p) => p,
        None => {
            let name = name.to_owned();
            tokio::task::spawn_blocking(move || parse(&name, &bytes).map(|(_, pages)| pages))
                .await
                .map_err(|_| SidekickError::internal("Parsing stopped unexpectedly."))??
        }
    };
    let chunks = chunk_pages(&pages);
    if chunks.is_empty() {
        return Err(SidekickError::new(
            ErrorCode::ParseFailed,
            format!("{name} has no text."),
        ));
    }
    let chunk_ids = {
        let mut guard = lock(db);
        repo::set_page_count(&guard, id, pages.len())?;
        let ids = repo::insert_chunks(&mut guard, id, &chunks)?;
        // The vector index is sized for the active model (ADR-013).
        vectors::ensure_table(&mut guard, embedder.dims())?;
        repo::set_status(&guard, id, DocStatus::Embedding, None)?;
        ids
    };
    report(DocStatus::Embedding, 10, None);

    let policy = RetryPolicy::default();
    let total = chunks.len();
    for (start, batch) in chunks
        .chunks(BATCH)
        .enumerate()
        .map(|(i, b)| (i * BATCH, b))
    {
        let texts: Vec<String> = batch.iter().map(|c| c.text.clone()).collect();
        let vectors_out = with_retries(&policy, || embedder.embed(&texts, key)).await?;
        if vectors_out.len() != texts.len() {
            return Err(SidekickError::new(
                ErrorCode::ProviderUnavailable,
                "The embeddings service returned the wrong number of vectors.",
            ));
        }
        {
            let guard = lock(db);
            for (offset, v) in vectors_out.iter().enumerate() {
                vectors::upsert(&guard, chunk_ids[start + offset], v)?;
            }
        }
        let done = start + batch.len();
        #[allow(clippy::cast_possible_truncation)]
        let percent = (10 + done * 89 / total) as u8;
        report(DocStatus::Embedding, percent, None);
    }
    Ok(())
}
