//! SQL for `questions` and `answers` (tech doc 4.2): what was asked, how it was
//! answered, with which sources, model, latency and tokens.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_store::Database;

/// Where a question came from (`questions.source`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum QuestionSource {
    Detected,
    Chat,
    Screenshot,
}

impl QuestionSource {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Detected => "detected",
            Self::Chat => "chat",
            Self::Screenshot => "screenshot",
        }
    }
}

/// One citation (`answers.sources_json`, tech doc 4.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Source {
    /// `doc`, `transcript`, `url` or `screenshot`.
    #[serde(rename = "type")]
    pub kind: String,
    /// e.g. `<document id>#p14`, `t:754000`, a URL.
    #[serde(rename = "ref")]
    pub reference: String,
    pub label: String,
}

/// A finished answer to save.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NewAnswer {
    pub session_id: String,
    pub question: String,
    pub source: QuestionSource,
    pub directed: bool,
    /// The transcript segment the question came from, if detected.
    pub segment_id: Option<i64>,
    pub cue: Option<String>,
    pub body_md: String,
    pub sources: Vec<Source>,
    pub provider: String,
    pub model: String,
    pub latency_ms: u32,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

fn db_err(context: &str, e: &rusqlite::Error) -> SidekickError {
    tracing::error!(target: "session", context, error = %e, "database error");
    SidekickError::new(
        ErrorCode::DbError,
        format!("Database error while {context}."),
    )
}

/// Saves the question and its answer in one transaction; returns the answer id.
pub fn save(db: &mut Database, a: &NewAnswer) -> SidekickResult<String> {
    let now = crate::repo::now_ms();
    let question_id = uuid::Uuid::now_v7().to_string();
    let answer_id = uuid::Uuid::now_v7().to_string();
    let sources = serde_json::to_string(&a.sources)
        .map_err(|_| SidekickError::internal("Couldn't save sources."))?;
    let tx = db
        .conn_mut()
        .transaction()
        .map_err(|e| db_err("saving an answer", &e))?;
    tx.execute(
        "INSERT INTO questions (id, session_id, segment_id, source, directed, text, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![question_id, a.session_id, a.segment_id, a.source.as_str(), a.directed, a.question, now],
    )
    .map_err(|e| db_err("saving a question", &e))?;
    tx.execute(
        "INSERT INTO answers (id, question_id, cue, body_md, sources_json, provider, model, latency_ms, input_tokens, output_tokens, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            answer_id,
            question_id,
            a.cue,
            a.body_md,
            sources,
            a.provider,
            a.model,
            a.latency_ms,
            a.input_tokens,
            a.output_tokens,
            now
        ],
    )
    .map_err(|e| db_err("saving an answer", &e))?;
    tx.commit().map_err(|e| db_err("saving an answer", &e))?;
    Ok(answer_id)
}

/// Thumbs up (1), down (-1) or cleared (0) (PRD success metric "rated helpful").
pub fn rate(db: &Database, answer_id: &str, rating: i8) -> SidekickResult<()> {
    if !(-1..=1).contains(&rating) {
        return Err(SidekickError::internal("Rating must be -1, 0 or 1."));
    }
    let value: Option<i8> = if rating == 0 { None } else { Some(rating) };
    let n = db
        .conn()
        .execute(
            "UPDATE answers SET rating = ?2 WHERE id = ?1",
            params![answer_id, value],
        )
        .map_err(|e| db_err("rating an answer", &e))?;
    if n == 0 {
        return Err(SidekickError::internal("That answer no longer exists."));
    }
    Ok(())
}

/// A saved question with its answer, for history (Chat tab, PRD QA-8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SavedAnswer {
    pub answer_id: String,
    pub question: String,
    pub source: QuestionSource,
    pub directed: bool,
    pub cue: Option<String>,
    pub body_md: String,
    pub sources: Vec<Source>,
    pub rating: Option<i8>,
    /// Epoch ms.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub created_at: i64,
}

fn parse_source(s: &str) -> QuestionSource {
    match s {
        "chat" => QuestionSource::Chat,
        "screenshot" => QuestionSource::Screenshot,
        _ => QuestionSource::Detected,
    }
}

/// A session's answers, oldest first, optionally only from one source.
pub fn list(
    db: &Database,
    session_id: &str,
    source: Option<QuestionSource>,
) -> SidekickResult<Vec<SavedAnswer>> {
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT a.id, q.text, q.source, q.directed, a.cue, a.body_md, a.sources_json, a.rating, a.created_at
             FROM answers a JOIN questions q ON q.id = a.question_id
             WHERE q.session_id = ?1 AND (?2 IS NULL OR q.source = ?2)
             ORDER BY a.created_at, a.id",
        )
        .map_err(|e| db_err("loading answers", &e))?;
    let rows = stmt
        .query_map(
            params![session_id, source.map(QuestionSource::as_str)],
            |r| {
                let sources: String = r.get(6)?;
                let kind: String = r.get(2)?;
                Ok(SavedAnswer {
                    answer_id: r.get(0)?,
                    question: r.get(1)?,
                    source: parse_source(&kind),
                    directed: r.get(3)?,
                    cue: r.get(4)?,
                    body_md: r.get(5)?,
                    sources: serde_json::from_str(&sources).unwrap_or_default(),
                    rating: r.get(7)?,
                    created_at: r.get(8)?,
                })
            },
        )
        .map_err(|e| db_err("loading answers", &e))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_err("loading answers", &e))
}
