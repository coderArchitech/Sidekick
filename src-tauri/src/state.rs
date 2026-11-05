//! Shared app state, built once at startup and handed to commands.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use sidekick_core::{SidekickError, SidekickResult};
use sidekick_diagnostics::Recorder;
use sidekick_gateway::Gateway;
use sidekick_secrets::{KeyStore, KeychainStore, MemoryStore};
use sidekick_store::{DB_FILE_NAME, Database, database_key};

/// Everything commands need. Crates receive only the parts they use.
pub struct AppState {
    db: Arc<Mutex<Database>>,
    /// At most 3 documents are processed at once (tech doc 13.5).
    pub ingest_slots: Arc<tokio::sync::Semaphore>,
    pub gateway: Gateway,
    pub diagnostics: Arc<Recorder>,
    pub keys: Arc<dyn KeyStore>,
    /// The one live (or paused) session, if any: its id and runner.
    pub live: tokio::sync::Mutex<Option<(String, sidekick_session::LiveSession)>>,
    /// Screenshots: the OS, or a synthetic screen in mock mode.
    pub capturer: Arc<dyn sidekick_capture::Capturer>,
    /// The app data folder (database, Whisper models).
    pub data_dir: PathBuf,
    pub mock: bool,
}

/// Mock mode only (see `AppState::init`).
const MOCK_DB_KEY: &str = "6d6f636b2d6d6f64652d6f6e6c792d6e6f742d612d7265616c2d6b6579212121";

/// The in-memory key store for mock mode, with the fixed database key.
fn mock_keys() -> SidekickResult<Arc<dyn KeyStore>> {
    let keys = MemoryStore::default();
    keys.set(
        sidekick_secrets::DB_KEY_ACCOUNT,
        &sidekick_secrets::SecretString::from(MOCK_DB_KEY.to_owned()),
    )?;
    Ok(Arc::new(keys))
}

impl AppState {
    /// Opens the key store and database for this run.
    ///
    /// `SIDEKICK_MOCK_PROVIDERS=1` uses an in-memory key store so tests and demos
    /// never touch the real keychain (tech doc 6.4). Its database key is a
    /// fixed test value, so a mock data folder can be reopened after a restart
    /// (restart and E2E tests); mock data is never real meeting data.
    pub fn init(data_dir: PathBuf) -> SidekickResult<Self> {
        let keys: Arc<dyn KeyStore> = if mock_providers() {
            mock_keys()?
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
        // First run: copy the built-in Skill templates into the Skills folder (tech doc 4.5).
        if let Err(e) = sidekick_skills::repo::seed(&db, &data_dir.join("skills")) {
            tracing::warn!(target: "skills", code = ?e.code, "couldn't seed Skill templates");
        }
        let diagnostics = Arc::new(Recorder::default());
        let mut gateway =
            Gateway::new(Arc::clone(&keys), mock).with_recorder(Arc::clone(&diagnostics));
        if !mock {
            gateway = gateway
                .with_verifier(
                    "deepgram",
                    Arc::new(sidekick_stt::DeepgramVerifier::default()),
                )
                .with_verifier(
                    "assemblyai",
                    Arc::new(sidekick_stt::providers::assemblyai::AssemblyAiVerifier::default()),
                )
                .with_verifier(
                    "soniox",
                    Arc::new(sidekick_stt::providers::soniox::SonioxVerifier::default()),
                );
        }
        if cfg!(feature = "local-whisper") {
            gateway = gateway.with_local("whisper-local");
        }
        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            ingest_slots: Arc::new(tokio::sync::Semaphore::new(3)),
            gateway,
            diagnostics,
            keys,
            live: tokio::sync::Mutex::new(None),
            capturer: if mock {
                Arc::new(sidekick_capture::MockCapturer)
            } else {
                Arc::new(sidekick_capture::OsCapturer)
            },
            data_dir,
            mock,
        })
    }

    /// The shared database handle, for background work like document ingest.
    pub fn db_handle(&self) -> Arc<Mutex<Database>> {
        Arc::clone(&self.db)
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
    fn mock_data_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let first =
            AppState::init_with(mock_keys().unwrap(), dir.path().to_path_buf(), true).unwrap();
        first
            .db()
            .conn()
            .execute(
                "INSERT INTO sessions (id, title, status, stt_provider, llm_provider, llm_model, created_at) VALUES ('s', 't', 'draft', 'mock', 'mock', 'm', 0)",
                [],
            )
            .unwrap();
        drop(first);
        let second =
            AppState::init_with(mock_keys().unwrap(), dir.path().to_path_buf(), true).unwrap();
        let n: i64 = second
            .db()
            .conn()
            .query_row("SELECT count(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
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
