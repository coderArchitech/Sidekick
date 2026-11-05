//! Shared app state, built once at startup and handed to commands.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use sidekick_core::{SidekickError, SidekickResult};
use sidekick_secrets::{KeyStore, KeychainStore, MemoryStore};
use sidekick_store::{DB_FILE_NAME, Database, database_key};

/// Everything commands need. Crates receive only the parts they use.
pub struct AppState {
    db: Mutex<Database>,
    #[allow(dead_code)] // Read by the key commands (Task T-007).
    pub keys: Arc<dyn KeyStore>,
}

impl AppState {
    /// Opens the key store and database for this run.
    ///
    /// `SIDEKICK_MOCK_PROVIDERS=1` uses an in-memory key store so tests and demos
    /// never touch the real keychain (tech doc 6.4).
    pub fn init(data_dir: PathBuf) -> SidekickResult<Self> {
        let keys: Arc<dyn KeyStore> = if mock_providers() {
            Arc::new(MemoryStore::default())
        } else {
            Arc::new(KeychainStore::default())
        };
        Self::init_with(keys, data_dir)
    }

    /// Opens the database in `data_dir` using the given key store.
    pub fn init_with(keys: Arc<dyn KeyStore>, data_dir: PathBuf) -> SidekickResult<Self> {
        std::fs::create_dir_all(&data_dir)
            .map_err(|_| SidekickError::internal("Couldn't create the app data folder."))?;
        let key = database_key(keys.as_ref())?;
        let db = Database::open(&data_dir.join(DB_FILE_NAME), &key)?;
        Ok(Self {
            db: Mutex::new(db),
            keys,
        })
    }

    /// Locks the database. A panic in another command doesn't make it unusable.
    pub fn db(&self) -> MutexGuard<'_, Database> {
        self.db.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// `SIDEKICK_DATA_DIR` if set, otherwise the OS app data folder (tech doc 3.7).
pub fn data_dir(default: PathBuf) -> PathBuf {
    std::env::var_os("SIDEKICK_DATA_DIR").map_or(default, PathBuf::from)
}

fn mock_providers() -> bool {
    std::env::var("SIDEKICK_MOCK_PROVIDERS").as_deref() == Ok("1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidekick_secrets::DB_KEY_ACCOUNT;

    #[test]
    fn first_run_creates_key_and_database_then_reopens() {
        let dir = tempfile::tempdir().unwrap();
        let keys: Arc<dyn KeyStore> = Arc::new(MemoryStore::default());
        drop(AppState::init_with(keys.clone(), dir.path().join("data")).unwrap());
        assert!(dir.path().join("data").join(DB_FILE_NAME).exists());
        assert!(keys.status(DB_KEY_ACCOUNT).unwrap().saved);
        // Second start reuses the same key.
        let state = AppState::init_with(keys, dir.path().join("data")).unwrap();
        assert!(sidekick_settings::load(&state.db()).unwrap().auto_answer);
    }

    #[test]
    fn data_dir_defaults_to_os_folder() {
        if std::env::var_os("SIDEKICK_DATA_DIR").is_none() {
            assert_eq!(
                data_dir(PathBuf::from("/os/default")),
                PathBuf::from("/os/default")
            );
        }
    }
}
