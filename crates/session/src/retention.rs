//! Retention cleanup and "delete everything" (tech doc 4.4, 10.7; PRD 6).

use rusqlite::params;
use serde::Serialize;
use sidekick_core::retention::Retention;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_store::Database;

use crate::repo;

/// The consent log outlives session data so the user can show consent later.
pub const CONSENT_KEEP_MS: i64 = 365 * 24 * 60 * 60 * 1000;

/// What a cleanup removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CleanupReport {
    pub sessions_deleted: u32,
    pub consent_records_deleted: u32,
}

fn db_err(context: &str, e: &rusqlite::Error) -> SidekickError {
    tracing::error!(target: "session", context, error = %e, "database error");
    SidekickError::new(
        ErrorCode::DbError,
        format!("Database error while {context}."),
    )
}

/// Ended sessions whose retention has run out at `now` (epoch ms).
pub fn expired(db: &Database, now: i64) -> SidekickResult<Vec<String>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT id, retention, ended_at FROM sessions WHERE status = 'ended' AND ended_at IS NOT NULL")
        .map_err(|e| db_err("checking retention", &e))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| db_err("checking retention", &e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_err("checking retention", &e))?;
    Ok(rows
        .into_iter()
        .filter(|(_, retention, ended_at)| {
            Retention::parse(retention)
                .expires_at(*ended_at)
                .is_some_and(|t| t <= now)
        })
        .map(|(id, _, _)| id)
        .collect())
}

/// Deletes expired sessions and consent records older than a year.
pub fn run(db: &Database, now: i64) -> SidekickResult<CleanupReport> {
    let ids = expired(db, now)?;
    for id in &ids {
        repo::delete(db, id)?;
    }
    let consent = db
        .conn()
        .execute(
            "DELETE FROM consent_log WHERE confirmed_at < ?1",
            params![now.saturating_sub(CONSENT_KEEP_MS)],
        )
        .map_err(|e| db_err("cleaning the consent log", &e))?;
    let report = CleanupReport {
        sessions_deleted: u32::try_from(ids.len()).unwrap_or(u32::MAX),
        consent_records_deleted: u32::try_from(consent).unwrap_or(u32::MAX),
    };
    tracing::info!(target: "session", sessions = report.sessions_deleted, consent = report.consent_records_deleted, "retention cleanup");
    Ok(report)
}

/// "Delete everything": every session (with its documents, transcript,
/// answers, notes and screenshots) and every knowledge pack. Settings, keys,
/// Skills and the consent log stay. Returns how many sessions were deleted.
pub fn delete_all(db: &Database) -> SidekickResult<u32> {
    let ids: Vec<String> = {
        let mut stmt = db
            .conn()
            .prepare("SELECT id FROM sessions")
            .map_err(|e| db_err("deleting data", &e))?;
        stmt.query_map([], |r| r.get(0))
            .map_err(|e| db_err("deleting data", &e))?
            .collect::<Result<_, _>>()
            .map_err(|e| db_err("deleting data", &e))?
    };
    for id in &ids {
        repo::delete(db, id)?;
    }
    let packs: Vec<String> = {
        let mut stmt = db
            .conn()
            .prepare("SELECT id FROM knowledge_packs")
            .map_err(|e| db_err("deleting data", &e))?;
        stmt.query_map([], |r| r.get(0))
            .map_err(|e| db_err("deleting data", &e))?
            .collect::<Result<_, _>>()
            .map_err(|e| db_err("deleting data", &e))?
    };
    for p in &packs {
        sidekick_store::vectors::delete_owner(db, &sidekick_store::vectors::pack_owner(p))?;
    }
    db.conn()
        .execute("DELETE FROM knowledge_packs", [])
        .map_err(|e| db_err("deleting data", &e))?;
    Ok(u32::try_from(ids.len()).unwrap_or(u32::MAX))
}
