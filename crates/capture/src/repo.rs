//! SQL for `screenshots` (tech doc 4.2). The image lives in the encrypted
//! database as a BLOB and goes with the session when it's deleted.

use base64::Engine;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_store::Database;

use crate::image_ops::Encoded;

/// A stored screenshot, without its pixels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub id: String,
    pub session_id: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    /// Epoch ms.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub created_at: i64,
}

/// The pixels, for a model request or a thumbnail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenshotImage {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

impl ScreenshotImage {
    #[must_use]
    pub fn base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(&self.bytes)
    }

    /// `data:` URL for showing in the overlay.
    #[must_use]
    pub fn data_url(&self) -> String {
        format!("data:{};base64,{}", self.mime_type, self.base64())
    }
}

fn db_err(context: &str, e: &rusqlite::Error) -> SidekickError {
    tracing::error!(target: "capture", context, error = %e, "database error");
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

const COLUMNS: &str = "id, session_id, mime_type, width, height, created_at";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Screenshot> {
    Ok(Screenshot {
        id: r.get(0)?,
        session_id: r.get(1)?,
        mime_type: r.get(2)?,
        width: r.get(3)?,
        height: r.get(4)?,
        created_at: r.get(5)?,
    })
}

/// Stores an encoded screenshot with the session.
pub fn save(db: &Database, session_id: &str, img: &Encoded) -> SidekickResult<Screenshot> {
    let shot = Screenshot {
        id: uuid::Uuid::now_v7().to_string(),
        session_id: session_id.to_owned(),
        mime_type: img.mime_type.to_owned(),
        width: img.width,
        height: img.height,
        created_at: now_ms(),
    };
    db.conn()
        .execute(
            "INSERT INTO screenshots (id, session_id, image, mime_type, width, height, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                shot.id,
                shot.session_id,
                img.bytes,
                shot.mime_type,
                shot.width,
                shot.height,
                shot.created_at
            ],
        )
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(f, _)
                if f.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                SidekickError::internal("That session no longer exists.")
            }
            e => db_err("saving a screenshot", &e),
        })?;
    Ok(shot)
}

/// The session's screenshots, oldest first.
pub fn list(db: &Database, session_id: &str) -> SidekickResult<Vec<Screenshot>> {
    let mut stmt = db
        .conn()
        .prepare(&format!(
            "SELECT {COLUMNS} FROM screenshots WHERE session_id = ?1 ORDER BY created_at, id"
        ))
        .map_err(|e| db_err("listing screenshots", &e))?;
    stmt.query_map([session_id], row)
        .map_err(|e| db_err("listing screenshots", &e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_err("listing screenshots", &e))
}

/// The newest screenshot taken at or after `since_ms`, if any.
pub fn latest(
    db: &Database,
    session_id: &str,
    since_ms: i64,
) -> SidekickResult<Option<Screenshot>> {
    db.conn()
        .query_row(
            &format!(
                "SELECT {COLUMNS} FROM screenshots WHERE session_id = ?1 AND created_at >= ?2
                 ORDER BY created_at DESC, id DESC LIMIT 1"
            ),
            params![session_id, since_ms],
            row,
        )
        .optional()
        .map_err(|e| db_err("reading screenshots", &e))
}

/// The image of one screenshot.
pub fn image(db: &Database, id: &str) -> SidekickResult<ScreenshotImage> {
    db.conn()
        .query_row(
            "SELECT mime_type, image FROM screenshots WHERE id = ?1",
            [id],
            |r| {
                Ok(ScreenshotImage {
                    mime_type: r.get(0)?,
                    bytes: r.get(1)?,
                })
            },
        )
        .optional()
        .map_err(|e| db_err("reading a screenshot", &e))?
        .ok_or_else(|| SidekickError::internal("That screenshot no longer exists."))
}

/// Deletes one screenshot.
pub fn delete(db: &Database, id: &str) -> SidekickResult<()> {
    db.conn()
        .execute("DELETE FROM screenshots WHERE id = ?1", [id])
        .map_err(|e| db_err("deleting a screenshot", &e))?;
    Ok(())
}
