//! SQL for `documents` and `chunks` (tech doc 4.2). Vectors go through
//! `sidekick_store::vectors`.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_store::Database;

use crate::chunk::Chunk;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum DocStatus {
    Queued,
    Parsing,
    Embedding,
    Ready,
    Failed,
}

impl DocStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Parsing => "parsing",
            Self::Embedding => "embedding",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "queued" => Self::Queued,
            "parsing" => Self::Parsing,
            "embedding" => Self::Embedding,
            "ready" => Self::Ready,
            _ => Self::Failed,
        }
    }
}

/// A document as the Library shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DocumentInfo {
    pub id: String,
    pub name: String,
    pub status: DocStatus,
    pub page_count: Option<u32>,
    pub chunk_count: u32,
    pub size_bytes: u32,
    pub error: Option<String>,
    pub created_at: i64,
}

fn db_err(context: &str, e: &rusqlite::Error) -> SidekickError {
    tracing::error!(target: "ingest", context, error = %e, "database error");
    SidekickError::new(
        ErrorCode::DbError,
        format!("Database error while {context}."),
    )
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

pub struct NewDocument<'a> {
    pub session_id: Option<&'a str>,
    pub pack_id: Option<&'a str>,
    pub name: &'a str,
    pub mime_type: &'a str,
    pub size_bytes: usize,
    pub sha256: &'a str,
    pub embedding_model: &'a str,
}

pub fn insert(db: &Database, d: &NewDocument<'_>) -> SidekickResult<String> {
    let id = uuid::Uuid::now_v7().to_string();
    db.conn()
        .execute(
            "INSERT INTO documents (id, session_id, pack_id, name, mime_type, size_bytes, sha256, embedding_model, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'queued', ?9)",
            params![
                id,
                d.session_id,
                d.pack_id,
                d.name,
                d.mime_type,
                i64::try_from(d.size_bytes).unwrap_or(i64::MAX),
                d.sha256,
                d.embedding_model,
                now_ms()
            ],
        )
        .map_err(|e| db_err("adding a document", &e))?;
    Ok(id)
}

/// A ready document with the same content in the same session or pack.
pub fn find_duplicate(
    db: &Database,
    session_id: Option<&str>,
    pack_id: Option<&str>,
    sha256: &str,
) -> SidekickResult<Option<String>> {
    db.conn()
        .query_row(
            "SELECT id FROM documents WHERE sha256 = ?1 AND status = 'ready'
               AND session_id IS ?2 AND pack_id IS ?3 LIMIT 1",
            params![sha256, session_id, pack_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| db_err("checking for duplicates", &e))
}

pub fn set_status(
    db: &Database,
    id: &str,
    status: DocStatus,
    error: Option<&str>,
) -> SidekickResult<()> {
    db.conn()
        .execute(
            "UPDATE documents SET status = ?2, error = ?3 WHERE id = ?1",
            params![id, status.as_str(), error],
        )
        .map_err(|e| db_err("updating a document", &e))?;
    Ok(())
}

pub fn set_page_count(db: &Database, id: &str, pages: usize) -> SidekickResult<()> {
    db.conn()
        .execute(
            "UPDATE documents SET page_count = ?2 WHERE id = ?1",
            params![id, i64::try_from(pages).unwrap_or(i64::MAX)],
        )
        .map_err(|e| db_err("updating a document", &e))?;
    Ok(())
}

/// Inserts all chunks in one transaction; returns their row ids in order.
pub fn insert_chunks(
    db: &mut Database,
    document_id: &str,
    chunks: &[Chunk],
) -> SidekickResult<Vec<i64>> {
    let tx = db
        .conn_mut()
        .transaction()
        .map_err(|e| db_err("saving chunks", &e))?;
    let mut ids = Vec::with_capacity(chunks.len());
    {
        let mut stmt = tx
            .prepare("INSERT INTO chunks (document_id, ord, page, text, token_count) VALUES (?1, ?2, ?3, ?4, ?5)")
            .map_err(|e| db_err("saving chunks", &e))?;
        for c in chunks {
            stmt.execute(params![
                document_id,
                i64::try_from(c.ord).unwrap_or(i64::MAX),
                c.page,
                c.text,
                i64::try_from(c.token_count).unwrap_or(i64::MAX)
            ])
            .map_err(|e| db_err("saving chunks", &e))?;
            ids.push(tx.last_insert_rowid());
        }
    }
    tx.commit().map_err(|e| db_err("saving chunks", &e))?;
    Ok(ids)
}

pub fn get(db: &Database, id: &str) -> SidekickResult<DocumentInfo> {
    list_where(db, "d.id = ?1", id)?
        .pop()
        .ok_or_else(|| SidekickError::new(ErrorCode::Internal, "That document no longer exists."))
}

/// Documents of a session, oldest first.
pub fn list_for_session(db: &Database, session_id: &str) -> SidekickResult<Vec<DocumentInfo>> {
    list_where(db, "d.session_id = ?1", session_id)
}

fn list_where(db: &Database, condition: &str, arg: &str) -> SidekickResult<Vec<DocumentInfo>> {
    let mut stmt = db
        .conn()
        .prepare(&format!(
            "SELECT d.id, d.name, d.status, d.page_count, d.size_bytes, d.error, d.created_at,
                    (SELECT count(*) FROM chunks c WHERE c.document_id = d.id)
             FROM documents d WHERE {condition} ORDER BY d.created_at, d.id"
        ))
        .map_err(|e| db_err("listing documents", &e))?;
    let rows = stmt
        .query_map([arg], |r| {
            Ok(DocumentInfo {
                id: r.get(0)?,
                name: r.get(1)?,
                status: DocStatus::parse(&r.get::<_, String>(2)?),
                page_count: r
                    .get::<_, Option<i64>>(3)?
                    .and_then(|p| u32::try_from(p).ok()),
                size_bytes: u32::try_from(r.get::<_, i64>(4)?).unwrap_or(u32::MAX),
                error: r.get(5)?,
                created_at: r.get(6)?,
                chunk_count: u32::try_from(r.get::<_, i64>(7)?).unwrap_or(u32::MAX),
            })
        })
        .map_err(|e| db_err("listing documents", &e))?;
    rows.collect::<Result<_, _>>()
        .map_err(|e| db_err("listing documents", &e))
}

/// Deletes a document, its chunks (cascade, FTS triggers) and its vectors.
pub fn delete(db: &Database, id: &str) -> SidekickResult<()> {
    let conn = db.conn();
    let has_vectors: bool = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'chunk_vectors'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map_err(|e| db_err("deleting a document", &e))?
        > 0;
    if has_vectors {
        conn.execute("DELETE FROM chunk_vectors WHERE chunk_id IN (SELECT id FROM chunks WHERE document_id = ?1)", [id])
            .map_err(|e| db_err("deleting a document", &e))?;
    }
    conn.execute("DELETE FROM documents WHERE id = ?1", [id])
        .map_err(|e| db_err("deleting a document", &e))?;
    Ok(())
}
