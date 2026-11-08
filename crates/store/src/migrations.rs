use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};

/// Ordered, forward-only migrations (tech doc 4.5). Append; never edit a released one.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_consent_log_outlives_sessions.sql"),
    include_str!("../migrations/0003_notes_summary.sql"),
    include_str!("../migrations/0004_agenda.sql"),
    include_str!("../migrations/0005_packs_and_brief.sql"),
    include_str!("../migrations/0006_session_skills.sql"),
];

/// Schema version after all migrations (`PRAGMA user_version`).
#[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
pub const LATEST_VERSION: i64 = MIGRATIONS.len() as i64;

pub(crate) fn current_version(conn: &Connection) -> SidekickResult<i64> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| crate::error::db_error("reading the schema version", &e))
}

/// Runs pending migrations inside one transaction; on failure nothing changes.
pub(crate) fn run(conn: &mut Connection, sql: &[&'static str]) -> SidekickResult<()> {
    let migrations = Migrations::new(sql.iter().map(|s| M::up(s)).collect());
    migrations.to_latest(conn).map_err(|e| {
        tracing::error!(target: "store", error = %e, "migration failed");
        SidekickError::new(
            ErrorCode::DbError,
            "Database upgrade failed. Your data is unchanged and a backup was kept.",
        )
    })
}

pub(crate) fn all() -> &'static [&'static str] {
    MIGRATIONS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_valid() {
        let migrations = Migrations::new(MIGRATIONS.iter().map(|s| M::up(s)).collect());
        // Needs sqlite-vec / fts5 available, which the bundled build provides.
        assert!(migrations.validate().is_ok());
    }
}
