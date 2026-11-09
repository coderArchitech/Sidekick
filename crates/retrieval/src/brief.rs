//! Material for the pre-meeting brief (PRD CTX-4): the start of every ready
//! document of the session and of its attached Knowledge Packs, within a
//! character budget shared fairly between documents.

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_store::Database;

/// One document's excerpt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BriefSource {
    pub document_name: String,
    /// The Knowledge Pack it comes from, if any.
    pub pack_name: Option<String>,
    pub text: String,
}

/// A saved brief.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Brief {
    pub content_md: String,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub created_at: i64,
}

/// Characters of document text sent for a brief, in total.
pub const BRIEF_CHARS: usize = 40_000;

fn db_err(context: &str, e: &rusqlite::Error) -> SidekickError {
    tracing::error!(target: "retrieval", context, error = %e, "database error");
    SidekickError::new(
        ErrorCode::DbError,
        format!("Database error while {context}."),
    )
}

/// The start of each ready document (session first, then packs), sharing
/// `max_chars` evenly; a short document leaves its unused share to the rest.
pub fn material(
    db: &Database,
    session_id: &str,
    max_chars: usize,
) -> SidekickResult<Vec<BriefSource>> {
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT d.id, d.name, p.name FROM documents d
             LEFT JOIN knowledge_packs p ON p.id = d.pack_id
             WHERE d.status = 'ready'
               AND (d.session_id = ?1
                    OR d.pack_id IN (SELECT pack_id FROM session_packs WHERE session_id = ?1))
             ORDER BY d.pack_id IS NOT NULL, p.name, d.created_at, d.id",
        )
        .map_err(|e| db_err("reading documents", &e))?;
    let docs: Vec<(String, String, Option<String>)> = stmt
        .query_map([session_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|e| db_err("reading documents", &e))?
        .collect::<Result<_, _>>()
        .map_err(|e| db_err("reading documents", &e))?;

    let mut chunks = db
        .conn()
        .prepare_cached("SELECT text FROM chunks WHERE document_id = ?1 ORDER BY ord")
        .map_err(|e| db_err("reading documents", &e))?;
    let mut left = max_chars;
    let mut out = Vec::with_capacity(docs.len());
    for (i, (id, name, pack)) in docs.iter().enumerate() {
        let share = left / (docs.len() - i);
        let mut text = String::new();
        let rows = chunks
            .query_map([id], |r| r.get::<_, String>(0))
            .map_err(|e| db_err("reading documents", &e))?;
        for chunk in rows {
            let chunk = chunk.map_err(|e| db_err("reading documents", &e))?;
            let room = share.saturating_sub(text.chars().count());
            if room == 0 {
                break;
            }
            if !text.is_empty() {
                text.push('\n');
            }
            text.extend(chunk.chars().take(room));
        }
        left = left.saturating_sub(text.chars().count());
        if !text.trim().is_empty() {
            out.push(BriefSource {
                document_name: name.clone(),
                pack_name: pack.clone(),
                text,
            });
        }
    }
    Ok(out)
}

/// The session's saved brief, if any.
pub fn get(db: &Database, session_id: &str) -> SidekickResult<Option<Brief>> {
    db.conn()
        .query_row(
            "SELECT content_md, created_at FROM briefs WHERE session_id = ?1",
            [session_id],
            |r| {
                Ok(Brief {
                    content_md: r.get(0)?,
                    created_at: r.get(1)?,
                })
            },
        )
        .optional()
        .map_err(|e| db_err("reading the brief", &e))
}

/// Saves (replaces) the session's brief.
pub fn save(db: &Database, session_id: &str, content_md: &str) -> SidekickResult<Brief> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
    db.conn()
        .execute(
            "INSERT INTO briefs (session_id, content_md, created_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(session_id) DO UPDATE SET content_md = excluded.content_md,
               created_at = excluded.created_at",
            rusqlite::params![session_id, content_md, now],
        )
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(f, _)
                if f.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                SidekickError::internal("That session no longer exists.")
            }
            e => db_err("saving the brief", &e),
        })?;
    Ok(Brief {
        content_md: content_md.to_owned(),
        created_at: now,
    })
}
