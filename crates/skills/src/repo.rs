//! The Skills folder and the `skills` index table (tech doc 4.2, 4.5).
//! Files are the source of truth; the table lists them for quick access.

use std::path::{Path, PathBuf};

use rusqlite::params;
use serde::Serialize;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_store::Database;

use crate::format::{self, SCHEMA_VERSION, Skill, Tool};

/// Built-in templates (PRD SKL-3). Each is copied into the Skills folder once
/// per install: on first run, or on the first start after an update adds it.
pub const TEMPLATES: &[(&str, &str)] = &[
    (
        "interview-coach",
        include_str!("../../../skills/templates/interview-coach.md"),
    ),
    (
        "sales-discovery",
        include_str!("../../../skills/templates/sales-discovery.md"),
    ),
    (
        "technical-architect",
        include_str!("../../../skills/templates/technical-architect.md"),
    ),
    (
        "teacher",
        include_str!("../../../skills/templates/teacher.md"),
    ),
    (
        "negotiation-coach",
        include_str!("../../../skills/templates/negotiation-coach.md"),
    ),
    (
        "staff-engineer-interview",
        include_str!("../../../skills/templates/staff-engineer-interview.md"),
    ),
];

/// The templates of the first release, seeded before `SEEDED_KEY` existed.
const FIRST_TEMPLATES: [&str; 5] = [
    "interview-coach",
    "sales-discovery",
    "technical-architect",
    "teacher",
    "negotiation-coach",
];

/// Settings key listing the templates already offered on this install.
const SEEDED_KEY: &str = "skills.seededTemplates";

/// A saved Skill and whether it's enabled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SkillEntry {
    #[serde(flatten)]
    pub skill: Skill,
    pub enabled: bool,
    pub builtin: bool,
}

/// What an import would add, shown for approval before saving (tech doc 11.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub skill: Skill,
    /// Tools the Skill asks for; the user approves them by importing.
    pub requested_tools: Vec<Tool>,
    /// A Skill with this id already exists and would be replaced.
    pub replaces_existing: bool,
}

/// Imported files larger than this are refused before parsing.
const MAX_FILE_BYTES: u64 = 64 * 1024;

fn db_err(context: &str, e: &rusqlite::Error) -> SidekickError {
    tracing::error!(target: "skills", context, error = %e, "database error");
    SidekickError::new(
        ErrorCode::DbError,
        format!("Database error while {context}."),
    )
}

fn io_err(context: &str, e: &std::io::Error) -> SidekickError {
    tracing::error!(target: "skills", context, error = %e, "file error");
    SidekickError::internal(format!("Couldn't {context}."))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

fn file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.md"))
}

fn is_builtin(id: &str) -> bool {
    TEMPLATES.iter().any(|(t, _)| *t == id)
}

/// Copies the templates into `dir` when the index is empty (first run).
pub fn seed(db: &Database, dir: &Path) -> SidekickResult<usize> {
    let mut seeded: Vec<String> = match sidekick_store::settings::get(db, SEEDED_KEY)? {
        Some(v) => serde_json::from_value(v).unwrap_or_default(),
        None => {
            let count: i64 = db
                .conn()
                .query_row("SELECT count(*) FROM skills", [], |r| r.get(0))
                .map_err(|e| db_err("reading Skills", &e))?;
            // An install from before this list existed was seeded with the
            // first templates; the user may have deleted some since.
            if count > 0 {
                FIRST_TEMPLATES.iter().map(|s| (*s).to_owned()).collect()
            } else {
                Vec::new()
            }
        }
    };
    let mut added = 0;
    for (id, text) in TEMPLATES {
        if seeded.iter().any(|s| s == id) {
            continue;
        }
        // Each built-in is offered once: one the user deleted stays deleted.
        if get(db, id).is_err() {
            save(db, dir, &format::parse(text)?)?;
            added += 1;
        }
        seeded.push((*id).to_owned());
    }
    sidekick_store::settings::set(
        db,
        SEEDED_KEY,
        &serde_json::to_value(&seeded).unwrap_or_default(),
    )?;
    Ok(added)
}

/// Writes the file and updates the index. The id must be new or the same Skill.
pub fn save(db: &Database, dir: &Path, skill: &Skill) -> SidekickResult<SkillEntry> {
    // Validate exactly what will be written.
    let text = format::to_markdown(skill);
    let skill = format::parse(&text)?;
    std::fs::create_dir_all(dir).map_err(|e| io_err("create the Skills folder", &e))?;
    let path = file(dir, &skill.id);
    let tmp = path.with_extension("md.tmp");
    std::fs::write(&tmp, &text).map_err(|e| io_err("save the Skill", &e))?;
    std::fs::rename(&tmp, &path).map_err(|e| io_err("save the Skill", &e))?;
    db.conn()
        .execute(
            "INSERT INTO skills (id, name, file_path, schema_version, enabled, updated_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?5)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, file_path = excluded.file_path,
               schema_version = excluded.schema_version, updated_at = excluded.updated_at",
            params![
                skill.id,
                skill.name,
                path.to_string_lossy(),
                SCHEMA_VERSION,
                now_ms()
            ],
        )
        .map_err(|e| db_err("saving a Skill", &e))?;
    get(db, &skill.id)
}

/// One Skill, read from its file.
pub fn get(db: &Database, id: &str) -> SidekickResult<SkillEntry> {
    let (path, enabled): (String, bool) = db
        .conn()
        .query_row(
            "SELECT file_path, enabled FROM skills WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                SidekickError::internal("That Skill no longer exists.")
            }
            e => db_err("reading a Skill", &e),
        })?;
    let text = std::fs::read_to_string(&path).map_err(|e| io_err("read the Skill file", &e))?;
    Ok(SkillEntry {
        skill: format::parse(&text)?,
        enabled,
        builtin: is_builtin(id),
    })
}

/// Every Skill, by name. Files that went missing or broke are skipped and logged.
pub fn list(db: &Database) -> SidekickResult<Vec<SkillEntry>> {
    let ids: Vec<String> = {
        let mut stmt = db
            .conn()
            .prepare("SELECT id FROM skills ORDER BY name COLLATE NOCASE, id")
            .map_err(|e| db_err("listing Skills", &e))?;
        stmt.query_map([], |r| r.get(0))
            .map_err(|e| db_err("listing Skills", &e))?
            .collect::<Result<_, _>>()
            .map_err(|e| db_err("listing Skills", &e))?
    };
    Ok(ids
        .iter()
        .filter_map(|id| match get(db, id) {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::warn!(target: "skills", id, code = ?e.code, "skipping unreadable Skill");
                None
            }
        })
        .collect())
}

pub fn set_enabled(db: &Database, id: &str, enabled: bool) -> SidekickResult<SkillEntry> {
    db.conn()
        .execute(
            "UPDATE skills SET enabled = ?2 WHERE id = ?1",
            params![id, enabled],
        )
        .map_err(|e| db_err("updating a Skill", &e))?;
    get(db, id)
}

/// Deletes the file and the index row. Sessions using it keep running without it.
pub fn delete(db: &Database, dir: &Path, id: &str) -> SidekickResult<()> {
    db.conn()
        .execute(
            "UPDATE sessions SET skill_id = NULL WHERE skill_id = ?1",
            [id],
        )
        .map_err(|e| db_err("deleting a Skill", &e))?;
    db.conn()
        .execute("DELETE FROM skills WHERE id = ?1", [id])
        .map_err(|e| db_err("deleting a Skill", &e))?;
    match std::fs::remove_file(file(dir, id)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_err("delete the Skill file", &e)),
    }
}

/// Reads a file to import. Nothing is saved: the UI shows the requested
/// tools and saves only if the user approves.
pub fn preview_import(db: &Database, path: &Path) -> SidekickResult<ImportPreview> {
    let size = std::fs::metadata(path)
        .map_err(|e| io_err("open that file", &e))?
        .len();
    if size > MAX_FILE_BYTES {
        return Err(SidekickError::new(
            ErrorCode::UnsupportedFile,
            "That file is too large to be a Skill (64 KB at most).",
        ));
    }
    let text = std::fs::read_to_string(path).map_err(|e| io_err("read that file", &e))?;
    let skill = format::parse(&text)?;
    let replaces_existing = db
        .conn()
        .query_row(
            "SELECT 1 FROM skills WHERE id = ?1",
            [&skill.id],
            |_| Ok(()),
        )
        .is_ok();
    Ok(ImportPreview {
        requested_tools: skill.tools.clone(),
        skill,
        replaces_existing,
    })
}

/// Writes a Skill to a file the user chose.
pub fn export(db: &Database, id: &str, path: &Path) -> SidekickResult<()> {
    let entry = get(db, id)?;
    std::fs::write(path, format::to_markdown(&entry.skill))
        .map_err(|e| io_err("write that file", &e))
}
