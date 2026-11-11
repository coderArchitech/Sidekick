//! Sidekick's encrypted local database (tech doc 4, ADR-004).
//!
//! One SQLCipher file, `sidekick.db`, in the app data folder. The 256-bit key
//! lives in the OS keychain (`sidekick/db-key`). sqlite-vec provides vector
//! search; FTS5 provides keyword search.
//!
//! Public API: [`Database`], [`database_key`], [`settings`], [`vectors`].

mod backup;
mod db;
mod error;
mod key;
mod migrations;
pub mod settings;
pub mod vectors;

pub use backup::{MAX_BACKUPS, list as list_backups};
pub use db::{DB_FILE_NAME, Database};
pub use key::database_key;
pub use migrations::LATEST_VERSION;
