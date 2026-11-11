//! Recovery screen commands (tech doc 4.5). They work only after a failed
//! startup, so a normally running app can never set its data aside by mistake.

use serde::Serialize;
use sidekick_core::{SidekickError, SidekickResult};
use tauri::{AppHandle, State};

use crate::recovery::{self, Startup, StartupProblem};

/// Why startup failed, or `null` when Sidekick started normally.
#[tauri::command]
#[specta::specta]
pub fn startup_problem(startup: State<'_, Startup>) -> Option<StartupProblem> {
    startup.problem.clone()
}

fn require_problem(startup: &Startup) -> SidekickResult<&StartupProblem> {
    startup.problem.as_ref().ok_or_else(|| {
        SidekickError::internal("Sidekick started normally, so there's nothing to recover.")
    })
}

/// Moves the unreadable database aside (it isn't deleted) and restarts with
/// empty data and, if the key was missing, a new key.
#[tauri::command]
#[specta::specta]
pub fn recover_start_fresh(app: AppHandle, startup: State<'_, Startup>) -> SidekickResult<()> {
    require_problem(&startup)?;
    let aside = recovery::set_aside(&startup.data_dir, recovery::now_ms()).map_err(|e| {
        tracing::error!(target: "recovery", error = %e, "couldn't set the database aside");
        SidekickError::internal("Couldn't move the old data file aside.")
    })?;
    tracing::warn!(target: "recovery", aside = %aside.display(), "database set aside; starting fresh");
    app.restart()
}

/// Puts back the newest pre-migration backup, keeping the current file aside.
#[tauri::command]
#[specta::specta]
pub fn recover_restore_backup(app: AppHandle, startup: State<'_, Startup>) -> SidekickResult<()> {
    require_problem(&startup)?;
    let restored =
        recovery::restore_latest_backup(&startup.data_dir, recovery::now_ms()).map_err(|e| {
            tracing::error!(target: "recovery", error = %e, "couldn't restore the backup");
            SidekickError::internal("Couldn't restore the backup.")
        })?;
    tracing::warn!(target: "recovery", backup = %restored.display(), "backup restored");
    app.restart()
}

/// Starts Sidekick again, for example after unlocking the keychain.
#[tauri::command]
#[specta::specta]
pub fn recover_retry(app: AppHandle, startup: State<'_, Startup>) -> SidekickResult<()> {
    require_problem(&startup)?;
    tracing::info!(target: "recovery", "retrying startup");
    app.restart()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot<'a> {
    app_version: &'a str,
    os: &'a str,
    startup_problem: Option<&'a StartupProblem>,
}

/// Saves the logs plus the startup problem as a zip, for a bug report.
/// Returns how many log files were included.
#[tauri::command]
#[specta::specta]
pub fn export_startup_logs(startup: State<'_, Startup>, file_path: String) -> SidekickResult<u32> {
    let files =
        crate::logging::files().ok_or_else(|| SidekickError::internal("Logging isn't running."))?;
    files.flush();
    let snapshot = serde_json::to_string_pretty(&Snapshot {
        app_version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        startup_problem: startup.problem.as_ref(),
    })
    .map_err(|_| SidekickError::internal("Couldn't write diagnostics."))?;
    let n = sidekick_diagnostics::logfile::export_zip(
        files.dir(),
        &snapshot,
        std::path::Path::new(&file_path),
    )
    .map_err(|e| {
        tracing::warn!(target: "recovery", error = %e, "log export failed");
        SidekickError::internal("Couldn't write the log bundle there.")
    })?;
    Ok(u32::try_from(n).unwrap_or(u32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_actions_refuse_after_a_normal_start() {
        let ok = Startup::ok(std::path::PathBuf::from("/tmp/x"));
        assert!(require_problem(&ok).is_err());
    }
}
