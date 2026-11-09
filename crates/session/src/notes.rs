//! SQL for `notes`, `action_items` and `agenda_items` (tech doc 4.2; PRD
//! MEM-1 to MEM-4): rolling notes with an optimistic version check, the
//! end-of-session summary with its action items, and the session's agenda.

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_store::Database;

/// A session's notes. `version` is 0 before the first save.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Notes {
    pub content_md: String,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub version: i64,
    /// Epoch ms; 0 before the first save.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub updated_at: i64,
    /// Summary and decisions, written when the session ends.
    pub summary_md: Option<String>,
    pub action_items: Vec<ActionItem>,
    /// The agenda, in order (PRD MEM-2).
    pub agenda: Vec<AgendaItem>,
}

/// Where the meeting is with an agenda item (PRD MEM-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum AgendaStatus {
    NotStarted,
    InProgress,
    Covered,
}

impl AgendaStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::NotStarted => "not_started",
            Self::InProgress => "in_progress",
            Self::Covered => "covered",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "in_progress" => Self::InProgress,
            "covered" => Self::Covered,
            _ => Self::NotStarted,
        }
    }
}

/// One agenda item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AgendaItem {
    pub id: String,
    pub text: String,
    pub status: AgendaStatus,
}

/// A status change for one agenda item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AgendaUpdate {
    pub id: String,
    pub status: AgendaStatus,
}

/// Most agenda items a session can have, and the longest item.
pub const MAX_AGENDA_ITEMS: usize = 30;
pub const MAX_AGENDA_TEXT: usize = 200;

/// One action item from the summary (PRD MEM-3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ActionItem {
    pub id: String,
    pub text: String,
    pub owner: Option<String>,
    pub due: Option<String>,
    pub done: bool,
}

/// An action item to save with a summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NewActionItem {
    pub text: String,
    pub owner: Option<String>,
    pub due: Option<String>,
}

fn db_err(context: &str, e: &rusqlite::Error) -> SidekickError {
    tracing::error!(target: "session", context, error = %e, "database error");
    SidekickError::new(
        ErrorCode::DbError,
        format!("Database error while {context}."),
    )
}

fn session_exists(db: &Database, session_id: &str) -> SidekickResult<()> {
    let found: Option<i64> = db
        .conn()
        .query_row("SELECT 1 FROM sessions WHERE id = ?1", [session_id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|e| db_err("reading notes", &e))?;
    found
        .map(|_| ())
        .ok_or_else(|| SidekickError::internal("That session no longer exists."))
}

/// The session's notes, summary and action items (empty notes at version 0).
pub fn get(db: &Database, session_id: &str) -> SidekickResult<Notes> {
    session_exists(db, session_id)?;
    let row: Option<(String, i64, i64, Option<String>)> = db
        .conn()
        .query_row(
            "SELECT content_md, version, updated_at, summary_md FROM notes WHERE session_id = ?1",
            [session_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(|e| db_err("reading notes", &e))?;
    let (content_md, version, updated_at, summary_md) = row.unwrap_or_default();
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT id, text, owner, due, done FROM action_items WHERE session_id = ?1 ORDER BY rowid",
        )
        .map_err(|e| db_err("reading action items", &e))?;
    let action_items = stmt
        .query_map([session_id], |r| {
            Ok(ActionItem {
                id: r.get(0)?,
                text: r.get(1)?,
                owner: r.get(2)?,
                due: r.get(3)?,
                done: r.get(4)?,
            })
        })
        .map_err(|e| db_err("reading action items", &e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_err("reading action items", &e))?;
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT id, text, status FROM agenda_items WHERE session_id = ?1 ORDER BY position",
        )
        .map_err(|e| db_err("reading the agenda", &e))?;
    let agenda = stmt
        .query_map([session_id], |r| {
            Ok(AgendaItem {
                id: r.get(0)?,
                text: r.get(1)?,
                status: AgendaStatus::parse(&r.get::<_, String>(2)?),
            })
        })
        .map_err(|e| db_err("reading the agenda", &e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_err("reading the agenda", &e))?;
    Ok(Notes {
        content_md,
        version,
        updated_at,
        summary_md,
        action_items,
        agenda,
    })
}

/// Replaces the agenda with `items` (blank lines dropped). An item whose text
/// was already on the agenda keeps its id and status.
pub fn set_agenda(db: &mut Database, session_id: &str, items: &[String]) -> SidekickResult<Notes> {
    session_exists(db, session_id)?;
    let items: Vec<&str> = items
        .iter()
        .map(|i| i.trim())
        .filter(|i| !i.is_empty())
        .collect();
    if items.len() > MAX_AGENDA_ITEMS || items.iter().any(|i| i.chars().count() > MAX_AGENDA_TEXT) {
        return Err(SidekickError::internal(format!(
            "An agenda can have up to {MAX_AGENDA_ITEMS} items of up to {MAX_AGENDA_TEXT} characters."
        )));
    }
    let existing = get(db, session_id)?.agenda;
    let tx = db
        .conn_mut()
        .transaction()
        .map_err(|e| db_err("saving the agenda", &e))?;
    tx.execute(
        "DELETE FROM agenda_items WHERE session_id = ?1",
        [session_id],
    )
    .map_err(|e| db_err("saving the agenda", &e))?;
    for (position, text) in items.iter().enumerate() {
        let kept = existing.iter().find(|a| a.text == *text);
        let id = kept.map_or_else(|| uuid::Uuid::now_v7().to_string(), |a| a.id.clone());
        let status = kept.map_or(AgendaStatus::NotStarted, |a| a.status);
        tx.execute(
            "INSERT INTO agenda_items (id, session_id, position, text, status) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, session_id, i64::try_from(position).unwrap_or(i64::MAX), text, status.as_str()],
        )
        .map_err(|e| db_err("saving the agenda", &e))?;
    }
    tx.commit().map_err(|e| db_err("saving the agenda", &e))?;
    get(db, session_id)
}

/// Sets the status of agenda items; ids not on this session's agenda are ignored.
pub fn update_agenda(
    db: &mut Database,
    session_id: &str,
    updates: &[AgendaUpdate],
) -> SidekickResult<Notes> {
    session_exists(db, session_id)?;
    let tx = db
        .conn_mut()
        .transaction()
        .map_err(|e| db_err("updating the agenda", &e))?;
    for u in updates {
        tx.execute(
            "UPDATE agenda_items SET status = ?3 WHERE session_id = ?1 AND id = ?2",
            params![session_id, u.id, u.status.as_str()],
        )
        .map_err(|e| db_err("updating the agenda", &e))?;
    }
    tx.commit().map_err(|e| db_err("updating the agenda", &e))?;
    get(db, session_id)
}

/// Ticks an action item off (or back on).
pub fn set_action_item_done(
    db: &Database,
    session_id: &str,
    item_id: &str,
    done: bool,
) -> SidekickResult<Notes> {
    session_exists(db, session_id)?;
    let changed = db
        .conn()
        .execute(
            "UPDATE action_items SET done = ?3 WHERE session_id = ?1 AND id = ?2",
            params![session_id, item_id, done],
        )
        .map_err(|e| db_err("updating the action item", &e))?;
    if changed == 0 {
        return Err(SidekickError::internal(
            "That action item no longer exists.",
        ));
    }
    get(db, session_id)
}

/// Saves rolling notes if `version` is still the current one (0 for the
/// first save); otherwise fails with `VERSION_CONFLICT` so the caller can
/// merge into the latest notes and try again.
pub fn save(
    db: &Database,
    session_id: &str,
    content_md: &str,
    version: i64,
) -> SidekickResult<Notes> {
    session_exists(db, session_id)?;
    let now = crate::repo::now_ms();
    let changed = if version == 0 {
        db.conn()
            .execute(
                "INSERT INTO notes (session_id, content_md, version, updated_at) VALUES (?1, ?2, 1, ?3)
                 ON CONFLICT(session_id) DO UPDATE SET content_md = excluded.content_md,
                   version = 1, updated_at = excluded.updated_at
                 WHERE notes.version = 0",
                params![session_id, content_md, now],
            )
            .map_err(|e| db_err("saving notes", &e))?
    } else {
        db.conn()
            .execute(
                "UPDATE notes SET content_md = ?2, version = version + 1, updated_at = ?3
                 WHERE session_id = ?1 AND version = ?4",
                params![session_id, content_md, now, version],
            )
            .map_err(|e| db_err("saving notes", &e))?
    };
    if changed == 0 {
        return Err(SidekickError::new(
            ErrorCode::VersionConflict,
            "The notes changed while they were being updated.",
        ));
    }
    get(db, session_id)
}

/// Saves the end-of-session summary and replaces the action items.
pub fn save_summary(
    db: &mut Database,
    session_id: &str,
    summary_md: &str,
    items: &[NewActionItem],
) -> SidekickResult<Notes> {
    session_exists(db, session_id)?;
    let now = crate::repo::now_ms();
    let tx = db
        .conn_mut()
        .transaction()
        .map_err(|e| db_err("saving the summary", &e))?;
    tx.execute(
        "INSERT INTO notes (session_id, content_md, version, updated_at, summary_md) VALUES (?1, '', 0, ?2, ?3)
         ON CONFLICT(session_id) DO UPDATE SET summary_md = excluded.summary_md",
        params![session_id, now, summary_md],
    )
    .map_err(|e| db_err("saving the summary", &e))?;
    tx.execute(
        "DELETE FROM action_items WHERE session_id = ?1",
        [session_id],
    )
    .map_err(|e| db_err("saving action items", &e))?;
    for item in items.iter().filter(|i| !i.text.trim().is_empty()) {
        tx.execute(
            "INSERT INTO action_items (id, session_id, text, owner, due) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                uuid::Uuid::now_v7().to_string(),
                session_id,
                item.text.trim(),
                item.owner,
                item.due
            ],
        )
        .map_err(|e| db_err("saving action items", &e))?;
    }
    tx.commit().map_err(|e| db_err("saving the summary", &e))?;
    get(db, session_id)
}
