//! Raw key-value settings (`settings` table). The typed settings model lives
//! with its feature; this module only stores JSON values. Never store keys here.

use rusqlite::OptionalExtension;
use sidekick_core::SidekickResult;

use crate::{Database, error::db_error};

/// Returns the JSON value for `key`, if set.
pub fn get(db: &Database, key: &str) -> SidekickResult<Option<serde_json::Value>> {
    let raw: Option<String> = db
        .conn()
        .query_row(
            "SELECT value_json FROM settings WHERE key = ?1",
            [key],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| db_error("reading a setting", &e))?;
    raw.map(|s| {
        serde_json::from_str(&s).map_err(|_| {
            sidekick_core::SidekickError::internal("A stored setting is not valid JSON.")
        })
    })
    .transpose()
}

/// Inserts or replaces the JSON value for `key`.
pub fn set(db: &Database, key: &str, value: &serde_json::Value) -> SidekickResult<()> {
    db.conn()
        .execute(
            "INSERT INTO settings (key, value_json) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
            (key, value.to_string()),
        )
        .map_err(|e| db_error("saving a setting", &e))?;
    Ok(())
}

/// All settings as (key, value) pairs, ordered by key.
pub fn all(db: &Database) -> SidekickResult<Vec<(String, serde_json::Value)>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT key, value_json FROM settings ORDER BY key")
        .map_err(|e| db_error("reading settings", &e))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| db_error("reading settings", &e))?;
    let mut out = Vec::new();
    for row in rows {
        let (k, v) = row.map_err(|e| db_error("reading settings", &e))?;
        if let Ok(value) = serde_json::from_str(&v) {
            out.push((k, value));
        }
    }
    Ok(out)
}
