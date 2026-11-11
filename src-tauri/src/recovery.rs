//! When Sidekick can't start (tech doc 4.5): instead of quitting, the overlay
//! shows a recovery screen with the reason and the ways out. The file actions
//! here never delete data: an unreadable database is moved aside, not removed.

use std::path::{Path, PathBuf};

use serde::Serialize;
use sidekick_core::SidekickError;
use sidekick_store::{DB_FILE_NAME, list_backups};

/// Which startup step failed. Each kind gets its own advice and actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ProblemKind {
    /// The database exists but its key is gone from the keychain.
    KeyMissing,
    /// The keychain couldn't be read or written (locked, or access denied).
    KeychainUnavailable,
    /// The key was read but the database didn't open: wrong key, a damaged
    /// file, or a failed upgrade.
    DatabaseUnreadable,
    Other,
}

/// A startup failure and the step it happened in.
#[derive(Debug)]
pub struct StartupFailure {
    pub kind: ProblemKind,
    pub error: SidekickError,
}

impl StartupFailure {
    pub fn new(kind: ProblemKind, error: SidekickError) -> Self {
        Self { kind, error }
    }
}

/// What the recovery screen shows. No keys or meeting content.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StartupProblem {
    pub kind: ProblemKind,
    /// The underlying error, for the details line and bug reports.
    pub message: String,
    /// Where the database lives, so people can find the set-aside file.
    pub data_dir: String,
    /// A pre-migration backup exists that "Restore backup" can put back.
    pub has_backup: bool,
    /// What this OS calls its keychain, for the advice text.
    pub keychain_name: String,
}

/// Managed state: whether startup succeeded, and the data folder either way.
pub struct Startup {
    pub problem: Option<StartupProblem>,
    pub data_dir: PathBuf,
}

impl Startup {
    pub fn ok(data_dir: PathBuf) -> Self {
        Self {
            problem: None,
            data_dir,
        }
    }

    pub fn failed(data_dir: PathBuf, failure: &StartupFailure) -> Self {
        let has_backup = !list_backups(&data_dir.join(DB_FILE_NAME)).is_empty();
        Self {
            problem: Some(StartupProblem {
                kind: failure.kind,
                message: failure.error.message.clone(),
                data_dir: data_dir.display().to_string(),
                // A backup is only useful with the key that made it.
                has_backup: has_backup && failure.kind == ProblemKind::DatabaseUnreadable,
                keychain_name: keychain_name(std::env::consts::OS).to_owned(),
            }),
            data_dir,
        }
    }
}

/// The user-facing name of the OS keychain.
pub fn keychain_name(os: &str) -> &'static str {
    match os {
        "macos" => "Keychain",
        "windows" => "Credential Manager",
        _ => "the system keyring",
    }
}

/// Moves the database and its WAL and shared-memory files aside as
/// `sidekick.db.unreadable-<unix_ms>` (and `-wal`, `-shm` next to it, so the
/// set-aside file still opens with the right key). Returns the new path.
pub fn set_aside(data_dir: &Path, now_ms: u128) -> std::io::Result<PathBuf> {
    let db = data_dir.join(DB_FILE_NAME);
    let aside = data_dir.join(format!("{DB_FILE_NAME}.unreadable-{now_ms}"));
    for suffix in ["-wal", "-shm"] {
        let from = data_dir.join(format!("{DB_FILE_NAME}{suffix}"));
        if from.exists() {
            std::fs::rename(&from, data_dir.join(format!("{}{suffix}", aside.display())))?;
        }
    }
    if db.exists() {
        std::fs::rename(&db, &aside)?;
    }
    Ok(aside)
}

/// Sets the current database aside and copies the newest pre-migration
/// backup in its place. Returns the backup that was restored.
pub fn restore_latest_backup(data_dir: &Path, now_ms: u128) -> std::io::Result<PathBuf> {
    let db = data_dir.join(DB_FILE_NAME);
    let newest = list_backups(&db)
        .pop()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no backup to restore"))?;
    set_aside(data_dir, now_ms)?;
    std::fs::copy(&newest, &db)?;
    Ok(newest)
}

pub fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidekick_core::ErrorCode;

    fn write(path: &Path, text: &str) {
        std::fs::write(path, text).unwrap();
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn set_aside_moves_the_database_and_its_journal_files_together() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("sidekick.db"), "db");
        write(&dir.path().join("sidekick.db-wal"), "wal");
        write(&dir.path().join("sidekick.db-shm"), "shm");

        let aside = set_aside(dir.path(), 42).unwrap();

        assert_eq!(aside, dir.path().join("sidekick.db.unreadable-42"));
        assert_eq!(read(&aside), "db");
        assert_eq!(
            read(&dir.path().join("sidekick.db.unreadable-42-wal")),
            "wal"
        );
        assert_eq!(
            read(&dir.path().join("sidekick.db.unreadable-42-shm")),
            "shm"
        );
        assert!(!dir.path().join("sidekick.db").exists());
        assert!(!dir.path().join("sidekick.db-wal").exists());
    }

    #[test]
    fn set_aside_without_a_database_does_nothing() {
        let dir = tempfile::tempdir().unwrap();
        set_aside(dir.path(), 1).unwrap();
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn restore_puts_back_the_newest_backup_and_keeps_the_broken_file() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("sidekick.db"), "broken");
        write(&dir.path().join("sidekick.db.bak-v4-100"), "older");
        write(&dir.path().join("sidekick.db.bak-v5-200"), "newest");

        let restored = restore_latest_backup(dir.path(), 300).unwrap();

        assert!(restored.ends_with("sidekick.db.bak-v5-200"));
        assert_eq!(read(&dir.path().join("sidekick.db")), "newest");
        assert_eq!(
            read(&dir.path().join("sidekick.db.unreadable-300")),
            "broken"
        );
        // The backup itself stays, in case this one fails too.
        assert!(dir.path().join("sidekick.db.bak-v5-200").exists());
    }

    #[test]
    fn restore_without_a_backup_fails_and_touches_nothing() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("sidekick.db"), "broken");
        assert!(restore_latest_backup(dir.path(), 1).is_err());
        assert_eq!(read(&dir.path().join("sidekick.db")), "broken");
    }

    #[test]
    fn backups_are_offered_only_when_the_key_still_works() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("sidekick.db"), "x");
        write(&dir.path().join("sidekick.db.bak-v5-200"), "y");
        let err = || SidekickError::new(ErrorCode::DbError, "Couldn't open the database.");
        let unreadable = Startup::failed(
            dir.path().to_path_buf(),
            &StartupFailure::new(ProblemKind::DatabaseUnreadable, err()),
        );
        assert!(unreadable.problem.unwrap().has_backup);
        let key_missing = Startup::failed(
            dir.path().to_path_buf(),
            &StartupFailure::new(ProblemKind::KeyMissing, err()),
        );
        assert!(!key_missing.problem.unwrap().has_backup);
    }

    #[test]
    fn names_the_keychain_per_os() {
        assert_eq!(keychain_name("macos"), "Keychain");
        assert_eq!(keychain_name("windows"), "Credential Manager");
        assert_eq!(keychain_name("linux"), "the system keyring");
    }
}
