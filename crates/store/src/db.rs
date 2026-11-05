use std::path::{Path, PathBuf};
use std::sync::Once;

use rusqlite::Connection;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_secrets::{ExposeSecret, SecretString};

use crate::{backup, error::db_error, migrations};

/// File name of the database inside the app data folder (tech doc 3.7).
pub const DB_FILE_NAME: &str = "sidekick.db";

/// An open, migrated, encrypted database connection.
pub struct Database {
    conn: Connection,
    path: Option<PathBuf>,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl Database {
    /// Opens (or creates) the database at `path`, unlocks it with `key`, and
    /// runs pending migrations after backing up an existing file.
    pub fn open(path: &Path, key: &SecretString) -> SidekickResult<Self> {
        Self::open_with(path, key, migrations::all())
    }

    /// An encrypted in-memory database, for tests.
    pub fn open_in_memory(key: &SecretString) -> SidekickResult<Self> {
        register_extensions();
        let conn = Connection::open_in_memory().map_err(|e| db_error("opening", &e))?;
        let mut db = Self { conn, path: None };
        db.unlock(key)?;
        db.configure()?;
        migrations::run(&mut db.conn, migrations::all())?;
        Ok(db)
    }

    pub(crate) fn open_with(
        path: &Path,
        key: &SecretString,
        sql: &[&'static str],
    ) -> SidekickResult<Self> {
        register_extensions();
        let existed = path.exists();
        let conn = Connection::open(path).map_err(|e| db_error("opening", &e))?;
        let mut db = Self {
            conn,
            path: Some(path.to_path_buf()),
        };
        db.unlock(key)?;
        db.configure()?;

        let version = migrations::current_version(&db.conn)?;
        #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
        let target = sql.len() as i64;
        if existed && version > 0 && version < target {
            // Flush the WAL into the main file so the copy is complete.
            db.conn
                .pragma_update(None, "wal_checkpoint", "TRUNCATE")
                .map_err(|e| db_error("preparing a backup", &e))?;
            let backup = backup::backup_before_migration(path, version)?;
            tracing::info!(target: "store", from = version, to = target, backup = %backup.display(), "migrating");
        }
        migrations::run(&mut db.conn, sql)?;
        Ok(db)
    }

    fn unlock(&self, key: &SecretString) -> SidekickResult<()> {
        // Raw 256-bit key as hex: skips SQLCipher's passphrase derivation.
        let pragma = format!("x'{}'", key.expose_secret());
        self.conn
            .pragma_update(None, "key", &pragma)
            .map_err(|e| db_error("unlocking", &e))?;
        // The key is only checked on first read; a wrong key fails here.
        self.conn
            .query_row("SELECT count(*) FROM sqlite_master", [], |r| {
                r.get::<_, i64>(0)
            })
            .map_err(|_| {
                SidekickError::new(
                    ErrorCode::DbError,
                    "Couldn't open the database: wrong key or damaged file.",
                )
            })?;
        Ok(())
    }

    fn configure(&self) -> SidekickResult<()> {
        self.conn
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|e| db_error("enabling foreign keys", &e))?;
        if self.path.is_some() {
            self.conn
                .pragma_update(None, "journal_mode", "WAL")
                .map_err(|e| db_error("enabling WAL", &e))?;
        }
        Ok(())
    }

    /// Schema version (`PRAGMA user_version`).
    pub fn schema_version(&self) -> SidekickResult<i64> {
        migrations::current_version(&self.conn)
    }

    /// Pre-migration backups next to the database file, oldest first.
    #[must_use]
    pub fn backups(&self) -> Vec<PathBuf> {
        self.path.as_deref().map(backup::list).unwrap_or_default()
    }

    /// Raw connection for repository functions inside this crate and for tests.
    /// Other crates use the typed repository modules instead.
    #[doc(hidden)]
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub(crate) fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}

/// Registers sqlite-vec for every connection opened in this process.
fn register_extensions() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        #[allow(unsafe_code)]
        // SAFETY: `sqlite3_vec_init` is the extension's documented entry point with
        // the signature SQLite expects; registering it as an auto-extension is the
        // usage shown in the sqlite-vec Rust docs. Called once per process.
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<
                *const (),
                unsafe extern "C" fn(
                    *mut rusqlite::ffi::sqlite3,
                    *mut *mut std::os::raw::c_char,
                    *const rusqlite::ffi::sqlite3_api_routines,
                ) -> i32,
            >(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const V1: &str = "CREATE TABLE a (x INTEGER);";
    const V2: &str = "CREATE TABLE b (y INTEGER);";
    const BROKEN: &str = "CREATE TABLE c (z INTEGER); THIS IS NOT SQL;";

    fn key() -> SecretString {
        SecretString::from("c".repeat(64))
    }

    fn table_exists(db: &Database, name: &str) -> bool {
        db.conn()
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [name],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
            == 1
    }

    #[test]
    fn upgrade_backs_up_first() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        drop(Database::open_with(&path, &key(), &[V1]).unwrap());
        let db = Database::open_with(&path, &key(), &[V1, V2]).unwrap();
        assert_eq!(db.schema_version().unwrap(), 2);
        assert_eq!(db.backups().len(), 1);
    }

    #[test]
    fn failed_migration_rolls_back_and_keeps_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        drop(Database::open_with(&path, &key(), &[V1]).unwrap());
        let err = Database::open_with(&path, &key(), &[V1, BROKEN]).unwrap_err();
        assert_eq!(err.code, ErrorCode::DbError);

        let db = Database::open_with(&path, &key(), &[V1]).unwrap();
        assert_eq!(db.schema_version().unwrap(), 1, "version unchanged");
        assert!(!table_exists(&db, "c"), "partial migration rolled back");
        assert_eq!(db.backups().len(), 1, "backup kept");
    }

    #[test]
    fn keeps_only_the_last_three_backups() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        let mut sql: Vec<&'static str> = vec![V1];
        drop(Database::open_with(&path, &key(), &sql).unwrap());
        let extra: [&'static str; 5] = [
            "CREATE TABLE t1 (a);",
            "CREATE TABLE t2 (a);",
            "CREATE TABLE t3 (a);",
            "CREATE TABLE t4 (a);",
            "CREATE TABLE t5 (a);",
        ];
        for m in extra {
            sql.push(m);
            std::thread::sleep(std::time::Duration::from_millis(2));
            drop(Database::open_with(&path, &key(), &sql).unwrap());
        }
        let db = Database::open_with(&path, &key(), &sql).unwrap();
        let backups = db.backups();
        assert_eq!(backups.len(), crate::MAX_BACKUPS);
        let newest = backups
            .last()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert!(newest.contains("bak-v5-"), "newest backup kept: {newest}");
    }
}
