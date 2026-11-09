//! Vector and keyword search over one session's chunks and the Knowledge
//! Packs attached to it (PRD CTX-5).

use rusqlite::params;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_store::{Database, vectors};

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub chunk_id: i64,
    /// Lower is better for vectors (cosine distance); higher is better for keywords.
    pub score: f64,
}

fn db_err(context: &str, e: &rusqlite::Error) -> SidekickError {
    tracing::error!(target: "retrieval", context, error = %e, "database error");
    SidekickError::new(
        ErrorCode::DbError,
        format!("Database error while {context}."),
    )
}

/// The vector partitions a session may read: its own and its attached packs'.
fn owners(db: &Database, session_id: &str) -> SidekickResult<Vec<String>> {
    let mut stmt = db
        .conn()
        .prepare_cached("SELECT pack_id FROM session_packs WHERE session_id = ?1")
        .map_err(|e| db_err("reading session packs", &e))?;
    let packs: Vec<String> = stmt
        .query_map([session_id], |r| r.get(0))
        .map_err(|e| db_err("reading session packs", &e))?
        .collect::<Result<_, _>>()
        .map_err(|e| db_err("reading session packs", &e))?;
    Ok(std::iter::once(vectors::session_owner(session_id))
        .chain(packs.iter().map(|p| vectors::pack_owner(p)))
        .collect())
}

/// Closest chunks by cosine distance among the session's ready documents and
/// those of its attached packs. The vector index is partitioned by owner, so
/// only those partitions are scanned and other sessions can never leak in.
pub fn vector(
    db: &Database,
    session_id: &str,
    query: &[f32],
    k: usize,
) -> SidekickResult<Vec<Hit>> {
    if !vectors::exists(db)? {
        return Ok(Vec::new());
    }
    let mut hits = Vec::new();
    for owner in owners(db, session_id)? {
        hits.extend(vectors::nearest(db, &owner, query, k)?);
    }
    hits.sort_by(|a, b| a.1.total_cmp(&b.1));
    hits.truncate(k);
    // Drop chunks of documents that are still embedding or failed.
    let mut ready = db
        .conn()
        .prepare_cached(
            "SELECT 1 FROM chunks c JOIN documents d ON d.id = c.document_id WHERE c.id = ?1 AND d.status = 'ready'",
        )
        .map_err(|e| db_err("searching documents", &e))?;
    let mut out = Vec::with_capacity(hits.len());
    for (chunk_id, distance) in hits {
        if ready
            .exists([chunk_id])
            .map_err(|e| db_err("searching documents", &e))?
        {
            out.push(Hit {
                chunk_id,
                score: distance,
            });
        }
    }
    Ok(out)
}

/// FTS5 query from free text: each word quoted (so punctuation can't break the
/// syntax) and OR-ed, with very common words dropped.
#[must_use]
pub fn fts_query(text: &str) -> Option<String> {
    const STOP: [&str; 24] = [
        "the", "a", "an", "is", "are", "was", "what", "whats", "how", "do", "does", "of", "to",
        "for", "in", "on", "and", "or", "it", "we", "you", "our", "can", "with",
    ];
    let words: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|w| w.chars().count() > 1 && !STOP.contains(&w.as_str()))
        .map(|w| format!("\"{w}\""))
        .collect();
    if words.is_empty() {
        None
    } else {
        Some(words.join(" OR "))
    }
}

/// Best keyword matches (BM25) among ready documents of the session and its
/// attached packs.
pub fn keyword(db: &Database, session_id: &str, text: &str, k: usize) -> SidekickResult<Vec<Hit>> {
    let Some(q) = fts_query(text) else {
        return Ok(Vec::new());
    };
    let mut stmt = db
        .conn()
        .prepare_cached(
            "SELECT c.id, -bm25(chunks_fts) AS s
             FROM chunks_fts
             JOIN chunks c ON c.id = chunks_fts.rowid
             JOIN documents doc ON doc.id = c.document_id
             WHERE chunks_fts MATCH ?1 AND doc.status = 'ready'
               AND (doc.session_id = ?2
                    OR doc.pack_id IN (SELECT pack_id FROM session_packs WHERE session_id = ?2))
             ORDER BY s DESC LIMIT ?3",
        )
        .map_err(|e| db_err("searching documents", &e))?;
    let rows = stmt
        .query_map(
            params![q, session_id, i64::try_from(k).unwrap_or(i64::MAX)],
            |r| {
                Ok(Hit {
                    chunk_id: r.get(0)?,
                    score: r.get(1)?,
                })
            },
        )
        .map_err(|e| db_err("searching documents", &e))?;
    rows.collect::<Result<_, _>>()
        .map_err(|e| db_err("searching documents", &e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_safe_fts_queries() {
        assert_eq!(
            fts_query("What's the Pro-tier rate limit?").as_deref(),
            Some("\"pro\" OR \"tier\" OR \"rate\" OR \"limit\"")
        );
        assert_eq!(fts_query("the a of"), None);
        assert_eq!(
            fts_query("\"quotes\" AND NEAR(x2)").as_deref(),
            Some("\"quotes\" OR \"near\" OR \"x2\""),
            "operators can't leak in"
        );
    }
}
