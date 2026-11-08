//! Knowledge Packs (PRD CTX-5): named, reusable sets of documents that can be
//! attached to any session. Pack documents are ingested with `Owner::Pack`, so
//! their vectors live in the pack's own partition.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_store::Database;

use crate::repo::{self, DocumentInfo};

/// A pack with its document counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PackInfo {
    pub id: String,
    pub name: String,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub created_at: i64,
    pub documents: u32,
    pub ready: u32,
}

/// Longest pack name.
pub const MAX_NAME: usize = 100;

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

fn clean_name(name: &str) -> SidekickResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(SidekickError::internal(format!(
            "Give the pack a name (up to {MAX_NAME} characters)."
        )));
    }
    Ok(name.to_owned())
}

/// Creates an empty pack.
pub fn create(db: &Database, name: &str) -> SidekickResult<PackInfo> {
    let name = clean_name(name)?;
    let id = uuid::Uuid::now_v7().to_string();
    db.conn()
        .execute(
            "INSERT INTO knowledge_packs (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![id, name, now_ms()],
        )
        .map_err(|e| db_err("creating a pack", &e))?;
    get(db, &id)
}

/// Renames a pack.
pub fn rename(db: &Database, id: &str, name: &str) -> SidekickResult<PackInfo> {
    let name = clean_name(name)?;
    db.conn()
        .execute(
            "UPDATE knowledge_packs SET name = ?2 WHERE id = ?1",
            params![id, name],
        )
        .map_err(|e| db_err("renaming a pack", &e))?;
    get(db, id)
}

const SELECT: &str = "SELECT p.id, p.name, p.created_at,
        (SELECT count(*) FROM documents d WHERE d.pack_id = p.id),
        (SELECT count(*) FROM documents d WHERE d.pack_id = p.id AND d.status = 'ready')
     FROM knowledge_packs p";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<PackInfo> {
    Ok(PackInfo {
        id: r.get(0)?,
        name: r.get(1)?,
        created_at: r.get(2)?,
        documents: u32::try_from(r.get::<_, i64>(3)?).unwrap_or(u32::MAX),
        ready: u32::try_from(r.get::<_, i64>(4)?).unwrap_or(u32::MAX),
    })
}

/// One pack.
pub fn get(db: &Database, id: &str) -> SidekickResult<PackInfo> {
    db.conn()
        .query_row(&format!("{SELECT} WHERE p.id = ?1"), [id], row)
        .optional()
        .map_err(|e| db_err("reading a pack", &e))?
        .ok_or_else(|| SidekickError::internal("That Knowledge Pack no longer exists."))
}

/// All packs, by name.
pub fn list(db: &Database) -> SidekickResult<Vec<PackInfo>> {
    let mut stmt = db
        .conn()
        .prepare(&format!("{SELECT} ORDER BY p.name COLLATE NOCASE, p.id"))
        .map_err(|e| db_err("listing packs", &e))?;
    stmt.query_map([], row)
        .map_err(|e| db_err("listing packs", &e))?
        .collect::<Result<_, _>>()
        .map_err(|e| db_err("listing packs", &e))
}

/// A pack's documents, oldest first.
pub fn documents(db: &Database, pack_id: &str) -> SidekickResult<Vec<DocumentInfo>> {
    get(db, pack_id)?;
    repo::list_for_pack(db, pack_id)
}

/// Deletes a pack, its documents (with their chunks and vectors) and its
/// attachments to sessions.
pub fn delete(db: &Database, id: &str) -> SidekickResult<()> {
    for d in repo::list_for_pack(db, id)? {
        repo::delete(db, &d.id)?;
    }
    db.conn()
        .execute("DELETE FROM knowledge_packs WHERE id = ?1", [id])
        .map_err(|e| db_err("deleting a pack", &e))?;
    Ok(())
}

/// The packs attached to a session.
pub fn for_session(db: &Database, session_id: &str) -> SidekickResult<Vec<String>> {
    let mut stmt = db
        .conn()
        .prepare_cached(
            "SELECT sp.pack_id FROM session_packs sp JOIN knowledge_packs p ON p.id = sp.pack_id
             WHERE sp.session_id = ?1 ORDER BY p.name COLLATE NOCASE, p.id",
        )
        .map_err(|e| db_err("reading session packs", &e))?;
    stmt.query_map([session_id], |r| r.get(0))
        .map_err(|e| db_err("reading session packs", &e))?
        .collect::<Result<_, _>>()
        .map_err(|e| db_err("reading session packs", &e))
}

/// Replaces the packs attached to a session; unknown pack ids are an error.
pub fn set_for_session(
    db: &mut Database,
    session_id: &str,
    pack_ids: &[String],
) -> SidekickResult<Vec<String>> {
    for id in pack_ids {
        get(db, id)?;
    }
    let tx = db
        .conn_mut()
        .transaction()
        .map_err(|e| db_err("attaching packs", &e))?;
    tx.execute(
        "DELETE FROM session_packs WHERE session_id = ?1",
        [session_id],
    )
    .map_err(|e| db_err("attaching packs", &e))?;
    for id in pack_ids {
        tx.execute(
            "INSERT OR IGNORE INTO session_packs (session_id, pack_id) VALUES (?1, ?2)",
            params![session_id, id],
        )
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(f, _)
                if f.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                SidekickError::internal("That session no longer exists.")
            }
            e => db_err("attaching packs", &e),
        })?;
    }
    tx.commit().map_err(|e| db_err("attaching packs", &e))?;
    for_session(db, session_id)
}
