use sidekick_core::{ErrorCode, SidekickError};

/// Maps a SQLite error to `DB_ERROR`. Messages carry no row content.
pub(crate) fn db_error(context: &str, err: &rusqlite::Error) -> SidekickError {
    tracing::error!(target: "store", context, error = %err, "database error");
    SidekickError::new(
        ErrorCode::DbError,
        format!("Database error while {context}."),
    )
}
