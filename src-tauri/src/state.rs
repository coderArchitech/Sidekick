//! Shared app state, built once at startup and handed to commands.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use sidekick_core::{SidekickError, SidekickResult};
use sidekick_gateway::Gateway;
use sidekick_secrets::{KeyStore, KeychainStore, MemoryStore};
use sidekick_store::{DB_FILE_NAME, Database, database_key};

/// Everything commands need. Crates receive only the parts they use.
pub struct AppState {
    db: Mutex<Database>,
    pub gateway: Gateway,
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
        Self::init_with(keys, data_dir, mock_providers())
    }

    /// Opens the database in `data_dir` using the given key store.
    pub fn init_with(
        keys: Arc<dyn KeyStore>,
        data_dir: PathBuf,
        mock: bool,
    ) -> SidekickResult<Self> {
        std::fs::create_dir_all(&data_dir)
            .map_err(|_| SidekickError::internal("Couldn't create the app data folder."))?;
        let key = database_key(keys.as_ref())?;
        let db = Database::open(&data_dir.join(DB_FILE_NAME), &key)?;
        let mut gateway = Gateway::new(Arc::clone(&keys), mock);
        if !mock {
            gateway = gateway.with_verifier(
                "deepgram",
                Arc::new(sidekick_stt::DeepgramVerifier::default()),
            );
        }
        Ok(Self {
            db: Mutex::new(db),
            gateway,
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
        drop(AppState::init_with(keys.clone(), dir.path().join("data"), true).unwrap());
        assert!(dir.path().join("data").join(DB_FILE_NAME).exists());
        assert!(keys.status(DB_KEY_ACCOUNT).unwrap().saved);
        // Second start reuses the same key.
        let state = AppState::init_with(keys, dir.path().join("data"), true).unwrap();
        assert!(sidekick_settings::load(&state.db()).unwrap().auto_answer);
    }

    #[tokio::test]
    async fn api_keys_never_reach_the_database() {
        let dir = tempfile::tempdir().unwrap();
        let keys: Arc<dyn KeyStore> = Arc::new(MemoryStore::default());
        let state = AppState::init_with(keys, dir.path().to_path_buf(), true).unwrap();
        let secret = "sk-ant-NEVER-IN-DB-424242";
        state
            .gateway
            .set_key(
                "anthropic",
                sidekick_secrets::SecretString::from(secret.to_owned()),
            )
            .await
            .unwrap();
        sidekick_settings::update(&state.db(), sidekick_settings::SettingsPatch::default())
            .unwrap();

        // Read every row of every table as text and search for the key.
        let db = state.db();
        let conn = db.conn();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE '%fts%' AND name NOT LIKE 'chunk_vectors%'")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert!(tables.len() > 10, "{tables:?}");
        for table in tables {
            let mut stmt = conn.prepare(&format!("SELECT * FROM {table}")).unwrap();
            let cols = stmt.column_count();
            let mut rows = stmt.query([]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                for i in 0..cols {
                    let v: rusqlite::types::Value = row.get(i).unwrap();
                    assert!(
                        !format!("{v:?}").contains("NEVER-IN-DB"),
                        "key found in {table}"
                    );
                }
            }
        }
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
