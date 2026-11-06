//! Vector storage with sqlite-vec (ADR-004, ADR-013).
//!
//! `chunk_vectors` is a `vec0` table partitioned by owner (`s:<session id>` or
//! `p:<pack id>`), so nearest-neighbour search only scans one owner's vectors,
//! and uses cosine distance. It's sized for the active embedding model, so it is
//! created here rather than in a migration. A change of size (or of this
//! layout) drops and recreates it; the re-embedding job (tech doc 4.5) refills it.

use sidekick_core::{ErrorCode, SidekickError, SidekickResult};

use crate::{Database, error::db_error, settings};

const LAYOUT_SETTING: &str = "embedding.index";
/// Bump when the table definition changes.
const LAYOUT: u64 = 2;

/// Partition key for a session's vectors.
#[must_use]
pub fn session_owner(session_id: &str) -> String {
    format!("s:{session_id}")
}

/// Partition key for a knowledge pack's vectors.
#[must_use]
pub fn pack_owner(pack_id: &str) -> String {
    format!("p:{pack_id}")
}

/// Ensures `chunk_vectors` exists with `dims` dimensions. Returns `true` if the
/// table was (re)created, meaning existing documents must be re-embedded.
pub fn ensure_table(db: &mut Database, dims: usize) -> SidekickResult<bool> {
    if dims == 0 || dims > 8192 {
        return Err(SidekickError::new(
            ErrorCode::Internal,
            "Unsupported embedding size.",
        ));
    }
    let wanted = serde_json::json!({ "dims": dims, "layout": LAYOUT });
    if settings::get(db, LAYOUT_SETTING)?.as_ref() == Some(&wanted) {
        return Ok(false);
    }
    let tx = db
        .conn_mut()
        .transaction()
        .map_err(|e| db_error("resizing the vector index", &e))?;
    tx.execute_batch(&format!(
        "DROP TABLE IF EXISTS chunk_vectors;
         CREATE VIRTUAL TABLE chunk_vectors USING vec0(
           chunk_id INTEGER PRIMARY KEY,
           owner TEXT PARTITION KEY,
           embedding float[{dims}] distance_metric=cosine
         );"
    ))
    .map_err(|e| db_error("resizing the vector index", &e))?;
    tx.execute(
        "INSERT INTO settings (key, value_json) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
        (LAYOUT_SETTING, wanted.to_string()),
    )
    .map_err(|e| db_error("resizing the vector index", &e))?;
    tx.commit()
        .map_err(|e| db_error("resizing the vector index", &e))?;
    Ok(true)
}

/// Stores or replaces the vector for a chunk.
pub fn upsert(db: &Database, chunk_id: i64, owner: &str, embedding: &[f32]) -> SidekickResult<()> {
    // A missing partition key makes sqlite-vec misbehave badly; never allow it.
    if owner.is_empty() {
        return Err(SidekickError::internal("A vector needs an owner."));
    }
    let conn = db.conn();
    conn.execute("DELETE FROM chunk_vectors WHERE chunk_id = ?1", [chunk_id])
        .map_err(|e| db_error("saving a vector", &e))?;
    conn.execute(
        "INSERT INTO chunk_vectors (chunk_id, owner, embedding) VALUES (?1, ?2, ?3)",
        (chunk_id, owner, to_blob(embedding)),
    )
    .map_err(|e| db_error("saving a vector", &e))?;
    Ok(())
}

/// The `k` nearest chunks of one owner, closest first, as (chunk_id, cosine distance).
pub fn nearest(
    db: &Database,
    owner: &str,
    query: &[f32],
    k: usize,
) -> SidekickResult<Vec<(i64, f64)>> {
    let mut stmt = db
        .conn()
        .prepare_cached(
            "SELECT chunk_id, distance FROM chunk_vectors
             WHERE embedding MATCH ?1 AND k = ?2 AND owner = ?3 ORDER BY distance",
        )
        .map_err(|e| db_error("searching vectors", &e))?;
    #[allow(clippy::cast_possible_wrap)]
    let rows = stmt
        .query_map((to_blob(query), k as i64, owner), |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(|e| db_error("searching vectors", &e))?;
    rows.collect::<Result<_, _>>()
        .map_err(|e| db_error("searching vectors", &e))
}

/// Whether the vector table exists yet (it's created with the first document).
pub fn exists(db: &Database) -> SidekickResult<bool> {
    db.conn()
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'chunk_vectors'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .map_err(|e| db_error("checking the vector index", &e))
}

fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Deletes every vector of one owner (a deleted session or pack). The vector
/// table has no foreign keys, so callers must do this explicitly.
pub fn delete_owner(db: &Database, owner: &str) -> SidekickResult<usize> {
    if !exists(db)? {
        return Ok(0);
    }
    db.conn()
        .execute("DELETE FROM chunk_vectors WHERE owner = ?1", [owner])
        .map_err(|e| db_error("deleting vectors", &e))
}
