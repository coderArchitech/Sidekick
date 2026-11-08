//! SQL for `sessions`, `consent_log` and `transcript_segments` (tech doc 4.2).

use rusqlite::{OptionalExtension, Row, params};
use serde::Serialize;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_store::Database;
use sidekick_stt::Segment;

use crate::model::{ConsentOption, Session, SessionStatus};

const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 200;

fn db_err(context: &str, e: &rusqlite::Error) -> SidekickError {
    tracing::error!(target: "session", context, error = %e, "database error");
    SidekickError::new(
        ErrorCode::DbError,
        format!("Database error while {context}."),
    )
}

#[must_use]
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Providers recorded with the session, so history shows what produced it.
#[derive(Debug, Clone)]
pub struct NewSession<'a> {
    pub title: &'a str,
    pub stt_provider: &'a str,
    pub llm_provider: &'a str,
    pub llm_model: &'a str,
    pub auto_answer: bool,
    /// From Settings; changeable per session.
    pub retention: sidekick_core::retention::Retention,
}

pub fn create(db: &Database, new: &NewSession<'_>) -> SidekickResult<Session> {
    let title = new.title.trim();
    if title.is_empty() || title.chars().count() > 200 {
        return Err(SidekickError::new(
            ErrorCode::Internal,
            "Give the session a name (up to 200 characters).",
        ));
    }
    let id = uuid::Uuid::now_v7().to_string();
    db.conn()
        .execute(
            "INSERT INTO sessions (id, title, status, stt_provider, llm_provider, llm_model, auto_answer, retention, created_at)
             VALUES (?1, ?2, 'draft', ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, title, new.stt_provider, new.llm_provider, new.llm_model, new.auto_answer, new.retention.as_str(), now_ms()],
        )
        .map_err(|e| db_err("creating a session", &e))?;
    get(db, &id)
}

const SELECT: &str = "SELECT s.id, s.title, s.status, s.auto_answer, s.created_at, s.started_at, s.ended_at,
    (SELECT c.option FROM consent_log c WHERE c.session_id = s.id ORDER BY c.confirmed_at DESC, c.rowid DESC LIMIT 1),
    s.skill_id, s.retention,
    (SELECT group_concat(skill_id, char(31)) FROM
       (SELECT ss.skill_id FROM session_skills ss WHERE ss.session_id = s.id ORDER BY ss.position)),
    s.stt_provider, s.llm_provider
    FROM sessions s";

fn from_row(r: &Row<'_>) -> rusqlite::Result<(Session, String, Option<String>)> {
    Ok((
        Session {
            id: r.get(0)?,
            title: r.get(1)?,
            status: SessionStatus::Draft, // replaced below
            auto_answer: r.get(3)?,
            created_at: r.get(4)?,
            started_at: r.get(5)?,
            ended_at: r.get(6)?,
            consent: None,
            skill_id: r.get(8)?,
            retention: sidekick_core::retention::Retention::parse(&r.get::<_, String>(9)?),
            extra_skill_ids: r
                .get::<_, Option<String>>(10)?
                .map(|s| s.split('\u{1f}').map(str::to_owned).collect())
                .unwrap_or_default(),
            stt_provider: r.get(11)?,
            llm_provider: r.get(12)?,
        },
        r.get(2)?,
        r.get(7)?,
    ))
}

fn finish((mut s, status, consent): (Session, String, Option<String>)) -> SidekickResult<Session> {
    s.status = SessionStatus::parse(&status)?;
    s.consent = consent.as_deref().map(ConsentOption::parse).transpose()?;
    Ok(s)
}

pub fn get(db: &Database, id: &str) -> SidekickResult<Session> {
    let row = db
        .conn()
        .query_row(&format!("{SELECT} WHERE s.id = ?1"), [id], from_row)
        .optional()
        .map_err(|e| db_err("reading a session", &e))?
        .ok_or_else(|| SidekickError::new(ErrorCode::Internal, "That session no longer exists."))?;
    finish(row)
}

/// One page of sessions, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SessionPage {
    pub items: Vec<Session>,
    /// Pass back to get the next page; `None` on the last page.
    pub next_cursor: Option<String>,
}

/// Lists sessions newest first. `cursor` is opaque; `limit` defaults to 50, max 200 (tech doc 5.5).
pub fn list(
    db: &Database,
    cursor: Option<&str>,
    limit: Option<u32>,
) -> SidekickResult<SessionPage> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let (after_ts, after_id) = match cursor {
        Some(c) => {
            let (ts, id) = c
                .split_once('|')
                .ok_or_else(|| SidekickError::internal("Invalid page cursor."))?;
            (
                ts.parse::<i64>()
                    .map_err(|_| SidekickError::internal("Invalid page cursor."))?,
                id.to_owned(),
            )
        }
        None => (i64::MAX, String::new()),
    };
    let mut stmt = db
        .conn()
        .prepare(&format!(
            "{SELECT} WHERE (s.created_at < ?1 OR (s.created_at = ?1 AND s.id < ?2) OR ?2 = '')
             ORDER BY s.created_at DESC, s.id DESC LIMIT ?3"
        ))
        .map_err(|e| db_err("listing sessions", &e))?;
    // With no cursor, ?2 = '' disables the id comparison and ?1 = MAX includes everything.
    let rows = stmt
        .query_map(params![after_ts, after_id, i64::from(limit) + 1], from_row)
        .map_err(|e| db_err("listing sessions", &e))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(finish(row.map_err(|e| db_err("listing sessions", &e))?)?);
    }
    let next_cursor = if items.len() > limit as usize {
        items.truncate(limit as usize);
        items.last().map(|s| format!("{}|{}", s.created_at, s.id))
    } else {
        None
    };
    Ok(SessionPage { items, next_cursor })
}

/// Saves a status change with its timestamps (first start, end).
pub fn set_status(db: &Database, id: &str, status: SessionStatus) -> SidekickResult<Session> {
    let now = now_ms();
    db.conn()
        .execute(
            "UPDATE sessions SET status = ?2,
                 started_at = CASE WHEN ?2 = 'live' AND started_at IS NULL THEN ?3 ELSE started_at END,
                 ended_at   = CASE WHEN ?2 = 'ended' THEN ?3 ELSE ended_at END
             WHERE id = ?1",
            params![id, status.as_str(), now],
        )
        .map_err(|e| db_err("updating a session", &e))?;
    get(db, id)
}

/// Records the consent choice (PRD 8, "Local consent log").
pub fn record_consent(
    db: &Database,
    session_id: &str,
    option: ConsentOption,
) -> SidekickResult<Session> {
    let session = get(db, session_id)?;
    if session.status != SessionStatus::Draft {
        return Err(SidekickError::internal(
            "Consent is confirmed before the session starts.",
        ));
    }
    db.conn()
        .execute(
            "INSERT INTO consent_log (id, session_id, session_title, option, confirmed_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![uuid::Uuid::now_v7().to_string(), session_id, session.title, option.as_str(), now_ms()],
        )
        .map_err(|e| db_err("recording consent", &e))?;
    get(db, session_id)
}

/// Deletes a session and everything in it. The consent log is kept (tech doc 4.4).
/// Deletes the session and everything in it: rows cascade (transcript,
/// documents, chunks and their search index, answers, notes, screenshots),
/// and its vectors, which have no foreign key, are deleted explicitly. The
/// consent record is kept (tech doc 4.4).
pub fn delete(db: &Database, id: &str) -> SidekickResult<()> {
    sidekick_store::vectors::delete_owner(db, &sidekick_store::vectors::session_owner(id))?;
    db.conn()
        .execute("DELETE FROM sessions WHERE id = ?1", [id])
        .map_err(|e| db_err("deleting a session", &e))?;
    Ok(())
}

/// Changes how long this session's data is kept.
pub fn set_retention(
    db: &Database,
    session_id: &str,
    retention: sidekick_core::retention::Retention,
) -> SidekickResult<Session> {
    let n = db
        .conn()
        .execute(
            "UPDATE sessions SET retention = ?2 WHERE id = ?1",
            params![session_id, retention.as_str()],
        )
        .map_err(|e| db_err("updating a session", &e))?;
    if n == 0 {
        return Err(SidekickError::internal("That session no longer exists."));
    }
    get(db, session_id)
}

/// Stores a finalised transcript segment and returns its row id.
pub fn insert_segment(db: &Database, session_id: &str, seg: &Segment) -> SidekickResult<i64> {
    db.conn()
        .execute(
            "INSERT INTO transcript_segments (session_id, speaker_label, is_user, text, start_ms, end_ms, confidence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                session_id,
                seg.speaker_label,
                seg.is_user,
                seg.text,
                i64::try_from(seg.start_ms).unwrap_or(i64::MAX),
                i64::try_from(seg.end_ms).unwrap_or(i64::MAX),
                seg.confidence.map(f64::from)
            ],
        )
        .map_err(|e| db_err("saving transcript", &e))?;
    Ok(db.conn().last_insert_rowid())
}

/// All consent records for a session (they survive deletion).
pub fn consent_log(db: &Database, session_id: &str) -> SidekickResult<Vec<(ConsentOption, i64)>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT option, confirmed_at FROM consent_log WHERE session_id = ?1 ORDER BY confirmed_at")
        .map_err(|e| db_err("reading consent", &e))?;
    let rows = stmt
        .query_map([session_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })
        .map_err(|e| db_err("reading consent", &e))?;
    let mut out = Vec::new();
    for row in rows {
        let (opt, at) = row.map_err(|e| db_err("reading consent", &e))?;
        out.push((ConsentOption::parse(&opt)?, at));
    }
    Ok(out)
}

/// A saved transcript line, as the UI shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TranscriptLine {
    pub id: i64,
    pub speaker_label: String,
    pub is_user: bool,
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
}

/// A moment the user marked (PRD 5 "Bookmarks").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub id: String,
    pub at_ms: u32,
    pub label: Option<String>,
}

fn ms_u32(v: i64) -> u32 {
    u32::try_from(v.max(0)).unwrap_or(u32::MAX)
}

/// All segments of a session in time order.
pub fn list_segments(db: &Database, session_id: &str) -> SidekickResult<Vec<TranscriptLine>> {
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT id, speaker_label, is_user, text, start_ms, end_ms FROM transcript_segments
             WHERE session_id = ?1 ORDER BY start_ms, id",
        )
        .map_err(|e| db_err("reading the transcript", &e))?;
    let rows = stmt
        .query_map([session_id], |r| {
            Ok(TranscriptLine {
                id: r.get(0)?,
                speaker_label: r.get(1)?,
                is_user: r.get(2)?,
                text: r.get(3)?,
                start_ms: ms_u32(r.get(4)?),
                end_ms: ms_u32(r.get(5)?),
            })
        })
        .map_err(|e| db_err("reading the transcript", &e))?;
    rows.collect::<Result<_, _>>()
        .map_err(|e| db_err("reading the transcript", &e))
}

/// Renames a speaker in every segment of the session (PRD LIS-3). Renaming to a
/// label another speaker already has merges the two. Returns rows changed.
pub fn rename_speaker(
    db: &Database,
    session_id: &str,
    from: &str,
    to: &str,
) -> SidekickResult<usize> {
    let to = to.trim();
    if to.is_empty() || to.chars().count() > 60 {
        return Err(SidekickError::new(
            ErrorCode::Internal,
            "Speaker names need 1 to 60 characters.",
        ));
    }
    db.conn()
        .execute(
            "UPDATE transcript_segments SET speaker_label = ?3 WHERE session_id = ?1 AND speaker_label = ?2",
            params![session_id, from, to],
        )
        .map_err(|e| db_err("renaming a speaker", &e))
}

pub fn add_bookmark(
    db: &Database,
    session_id: &str,
    at_ms: u32,
    label: Option<&str>,
) -> SidekickResult<Bookmark> {
    let label = label
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned);
    let id = uuid::Uuid::now_v7().to_string();
    db.conn()
        .execute(
            "INSERT INTO bookmarks (id, session_id, at_ms, label) VALUES (?1, ?2, ?3, ?4)",
            params![id, session_id, at_ms, label],
        )
        .map_err(|e| db_err("adding a bookmark", &e))?;
    Ok(Bookmark { id, at_ms, label })
}

pub fn list_bookmarks(db: &Database, session_id: &str) -> SidekickResult<Vec<Bookmark>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT id, at_ms, label FROM bookmarks WHERE session_id = ?1 ORDER BY at_ms")
        .map_err(|e| db_err("reading bookmarks", &e))?;
    let rows = stmt
        .query_map([session_id], |r| {
            Ok(Bookmark {
                id: r.get(0)?,
                at_ms: ms_u32(r.get(1)?),
                label: r.get(2)?,
            })
        })
        .map_err(|e| db_err("reading bookmarks", &e))?;
    rows.collect::<Result<_, _>>()
        .map_err(|e| db_err("reading bookmarks", &e))
}

/// Most Skills also in use alongside the main one (PRD SKL-5).
pub const MAX_EXTRA_SKILLS: usize = 2;

/// Replaces the Skills also in use (PRD SKL-5). Each must exist; the main
/// Skill and repeats are left out.
pub fn set_extra_skills(
    db: &mut Database,
    session_id: &str,
    skill_ids: &[String],
) -> SidekickResult<Session> {
    let main = get(db, session_id)?.skill_id;
    let mut ids: Vec<&str> = Vec::new();
    for id in skill_ids {
        if Some(id.as_str()) != main.as_deref() && !ids.contains(&id.as_str()) {
            ids.push(id);
        }
    }
    if ids.len() > MAX_EXTRA_SKILLS {
        return Err(SidekickError::internal(format!(
            "Use at most {} Skills at once.",
            MAX_EXTRA_SKILLS + 1
        )));
    }
    let tx = db
        .conn_mut()
        .transaction()
        .map_err(|e| db_err("updating a session", &e))?;
    tx.execute(
        "DELETE FROM session_skills WHERE session_id = ?1",
        [session_id],
    )
    .map_err(|e| db_err("updating a session", &e))?;
    for (position, id) in ids.iter().enumerate() {
        tx.execute(
            "INSERT INTO session_skills (session_id, skill_id, position) VALUES (?1, ?2, ?3)",
            params![session_id, id, i64::try_from(position).unwrap_or(i64::MAX)],
        )
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(f, _)
                if f.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                SidekickError::internal("That Skill no longer exists.")
            }
            e => db_err("updating a session", &e),
        })?;
    }
    tx.commit().map_err(|e| db_err("updating a session", &e))?;
    get(db, session_id)
}

/// Sets or clears the session's active Skill (PRD SKL-1). The Skill must exist.
pub fn set_skill(
    db: &Database,
    session_id: &str,
    skill_id: Option<&str>,
) -> SidekickResult<Session> {
    let n = db
        .conn()
        .execute(
            "UPDATE sessions SET skill_id = ?2 WHERE id = ?1",
            params![session_id, skill_id],
        )
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(f, _)
                if f.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                SidekickError::internal("That Skill no longer exists.")
            }
            e => db_err("updating a session", &e),
        })?;
    if n == 0 {
        return Err(SidekickError::internal("That session no longer exists."));
    }
    // The main Skill is never also an extra one.
    if let Some(id) = skill_id {
        db.conn()
            .execute(
                "DELETE FROM session_skills WHERE session_id = ?1 AND skill_id = ?2",
                params![session_id, id],
            )
            .map_err(|e| db_err("updating a session", &e))?;
    }
    get(db, session_id)
}

/// Turns Auto-answer on or off for one session (PRD QA-3).
pub fn set_auto_answer(db: &Database, session_id: &str, on: bool) -> SidekickResult<Session> {
    db.conn()
        .execute(
            "UPDATE sessions SET auto_answer = ?2 WHERE id = ?1",
            params![session_id, on],
        )
        .map_err(|e| db_err("updating a session", &e))?;
    get(db, session_id)
}
