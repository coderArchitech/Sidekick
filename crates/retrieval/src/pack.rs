//! Context packs: what one answer gets to read (tech doc 5.1 `retrieve_context`).

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_session::repo::{TranscriptLine, list_segments};
use sidekick_store::Database;

use crate::fusion::fuse;
use crate::search::{keyword, vector};

/// How much context to gather (tech doc 13.5: last 3 minutes, top 5 chunks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ContextOptions {
    pub window_ms: u32,
    pub top_k: u32,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            window_ms: 180_000,
            top_k: 5,
        }
    }
}

/// A document passage with what a citation needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ContextChunk {
    pub chunk_id: i64,
    pub document_id: String,
    pub document_name: String,
    pub page: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ContextPack {
    /// The most recent `window_ms` of transcript, in time order.
    pub transcript: Vec<TranscriptLine>,
    /// Rolling notes (filled by T-024).
    pub notes: Option<String>,
    /// Best document passages, best first.
    pub chunks: Vec<ContextChunk>,
}

/// How many candidates each search contributes before fusion.
const CANDIDATES: usize = 20;

fn db_err(context: &str, e: &rusqlite::Error) -> SidekickError {
    tracing::error!(target: "retrieval", context, error = %e, "database error");
    SidekickError::new(
        ErrorCode::DbError,
        format!("Database error while {context}."),
    )
}

/// The last `window_ms` of transcript, ending at the latest line.
#[must_use]
pub fn recent(lines: Vec<TranscriptLine>, window_ms: u32) -> Vec<TranscriptLine> {
    let Some(end) = lines.iter().map(|l| l.end_ms).max() else {
        return lines;
    };
    let from = end.saturating_sub(window_ms);
    lines.into_iter().filter(|l| l.end_ms >= from).collect()
}

/// Builds the pack. `query_vector` is the embedded question, or `None` to use
/// keyword search only (e.g. no embeddings key yet).
pub fn build_pack(
    db: &Database,
    session_id: &str,
    query: &str,
    query_vector: Option<&[f32]>,
    opts: ContextOptions,
) -> SidekickResult<ContextPack> {
    let transcript = recent(list_segments(db, session_id)?, opts.window_ms);
    let notes: Option<String> = db
        .conn()
        .query_row(
            "SELECT content_md FROM notes WHERE session_id = ?1",
            [session_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| db_err("reading notes", &e))?;

    let by_vector: Vec<i64> = match query_vector {
        Some(v) => vector(db, session_id, v, CANDIDATES)?
            .into_iter()
            .map(|h| h.chunk_id)
            .collect(),
        None => Vec::new(),
    };
    let by_keyword: Vec<i64> = keyword(db, session_id, query, CANDIDATES)?
        .into_iter()
        .map(|h| h.chunk_id)
        .collect();
    let top: Vec<i64> = fuse(&[by_vector, by_keyword])
        .into_iter()
        .take(opts.top_k as usize)
        .map(|(id, _)| id)
        .collect();

    let mut stmt = db
        .conn()
        .prepare_cached(
            "SELECT c.id, d.id, d.name, c.page, c.text FROM chunks c JOIN documents d ON d.id = c.document_id WHERE c.id = ?1",
        )
        .map_err(|e| db_err("reading passages", &e))?;
    let mut chunks = Vec::with_capacity(top.len());
    for id in top {
        let chunk = stmt
            .query_row([id], |r| {
                Ok(ContextChunk {
                    chunk_id: r.get(0)?,
                    document_id: r.get(1)?,
                    document_name: r.get(2)?,
                    page: r
                        .get::<_, Option<i64>>(3)?
                        .and_then(|p| u32::try_from(p).ok()),
                    text: r.get(4)?,
                })
            })
            .map_err(|e| db_err("reading passages", &e))?;
        chunks.push(chunk);
    }
    Ok(ContextPack {
        transcript,
        notes: notes.filter(|n| !n.trim().is_empty()),
        chunks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: i64, start: u32, end: u32) -> TranscriptLine {
        TranscriptLine {
            id,
            speaker_label: "me".into(),
            is_user: true,
            text: format!("l{id}"),
            start_ms: start,
            end_ms: end,
        }
    }

    #[test]
    fn keeps_the_last_three_minutes() {
        let lines = vec![
            line(1, 0, 1_000),
            line(2, 100_000, 101_000),
            line(3, 250_000, 260_000),
        ];
        let kept: Vec<i64> = recent(lines, 180_000).into_iter().map(|l| l.id).collect();
        assert_eq!(
            kept,
            [2, 3],
            "line 1 ended more than 3 minutes before the latest"
        );
        assert!(recent(vec![], 180_000).is_empty());
    }
}
