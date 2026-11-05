use std::path::{Path, PathBuf};

use sidekick_core::{SidekickError, SidekickResult};

/// How many pre-migration backups to keep (tech doc 4.6).
pub const MAX_BACKUPS: usize = 3;

/// Copies the (already encrypted) database file before a migration and prunes
/// old backups. Backups are named `<db>.bak-v<version>-<unix_ms>`.
pub(crate) fn backup_before_migration(
    db_path: &Path,
    from_version: i64,
) -> SidekickResult<PathBuf> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let file_name = db_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| SidekickError::internal("Invalid database path."))?;
    let backup = db_path.with_file_name(format!("{file_name}.bak-v{from_version}-{now_ms}"));
    std::fs::copy(db_path, &backup)
        .map_err(|_| SidekickError::internal("Couldn't back up the database before upgrading."))?;
    prune(db_path, file_name);
    Ok(backup)
}

pub(crate) fn list(db_path: &Path) -> Vec<PathBuf> {
    let Some(file_name) = db_path.file_name().and_then(|n| n.to_str()) else {
        return Vec::new();
    };
    let prefix = format!("{file_name}.bak-");
    let dir = db_path.parent().unwrap_or_else(|| Path::new("."));
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&prefix))
        })
        .collect();
    // Names end in a timestamp, so sorting by the suffix orders oldest first.
    found.sort_by_key(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.rsplit('-').next())
            .and_then(|t| t.parse::<u128>().ok())
            .unwrap_or(0)
    });
    found
}

fn prune(db_path: &Path, _file_name: &str) {
    let backups = list(db_path);
    let excess = backups.len().saturating_sub(MAX_BACKUPS);
    for old in backups.into_iter().take(excess) {
        let _ = std::fs::remove_file(old);
    }
}
