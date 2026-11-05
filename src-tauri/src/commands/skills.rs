//! Skills (tech doc 5.1, 5.7; PRD SKL-1 to SKL-3). Managing Skills is a
//! Settings-window job; the overlay can only list them and pick the active one.

use std::path::{Path, PathBuf};

use sidekick_core::SidekickResult;
use sidekick_session::Session;
use sidekick_skills::format::{self, FieldError, Skill};
use sidekick_skills::repo::{self, ImportPreview, SkillEntry, TEMPLATES};
use tauri::{AppHandle, State};

use crate::state::AppState;

pub fn skills_dir(state: &AppState) -> PathBuf {
    state.data_dir.join("skills")
}

/// All Skills, by name.
#[tauri::command]
#[specta::specta]
pub fn list_skills(state: State<'_, AppState>) -> SidekickResult<Vec<SkillEntry>> {
    repo::list(&state.db())
}

/// The built-in templates, for "New Skill from template".
#[tauri::command]
#[specta::specta]
pub fn skill_templates() -> SidekickResult<Vec<Skill>> {
    TEMPLATES.iter().map(|(_, t)| format::parse(t)).collect()
}

/// Checks a Skill without saving it: field-level problems for the builder.
#[tauri::command]
#[specta::specta]
pub fn check_skill(skill: Skill) -> Vec<FieldError> {
    format::validate(&format::to_markdown(&skill))
        .err()
        .unwrap_or_default()
}

/// Validates and saves a Skill (new or edited).
#[tauri::command]
#[specta::specta]
pub fn save_skill(state: State<'_, AppState>, skill: Skill) -> SidekickResult<SkillEntry> {
    repo::save(&state.db(), &skills_dir(&state), &skill)
}

#[tauri::command]
#[specta::specta]
pub fn set_skill_enabled(
    state: State<'_, AppState>,
    skill_id: String,
    enabled: bool,
) -> SidekickResult<SkillEntry> {
    repo::set_enabled(&state.db(), &skill_id, enabled)
}

#[tauri::command]
#[specta::specta]
pub fn delete_skill(state: State<'_, AppState>, skill_id: String) -> SidekickResult<()> {
    repo::delete(&state.db(), &skills_dir(&state), &skill_id)
}

/// Reads a Skill file for import and returns it with the tools it asks for.
/// Nothing is saved until the user approves (then `save_skill`).
#[tauri::command]
#[specta::specta]
pub fn import_skill(
    state: State<'_, AppState>,
    file_path: String,
) -> SidekickResult<ImportPreview> {
    repo::preview_import(&state.db(), Path::new(&file_path))
}

/// Writes a Skill to a file the user picked in the save dialog.
#[tauri::command]
#[specta::specta]
pub fn export_skill(
    state: State<'_, AppState>,
    skill_id: String,
    file_path: String,
) -> SidekickResult<()> {
    repo::export(&state.db(), &skill_id, Path::new(&file_path))
}

/// Sets the session's active Skill (overlay; the Switch Skill shortcut).
#[tauri::command]
#[specta::specta]
pub fn set_session_skill(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    skill_id: Option<String>,
) -> SidekickResult<Session> {
    let session = sidekick_session::repo::set_skill(&state.db(), &session_id, skill_id.as_deref())?;
    crate::commands::session::changed(&app, &session);
    Ok(session)
}
