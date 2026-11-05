//! Vector storage with sqlite-vec (ADR-004, ADR-013).
//!
//! `chunk_vectors` is sized for the active embedding model, so it is created
//! here rather than in a migration. Changing dimension drops and recreates it;
//! the re-embedding job (tech doc 4.5) then refills it.

use sidekick_core::{ErrorCode, SidekickError, SidekickResult};

use crate::{Database, error::db_error, settings};

const DIMS_SETTING: &str = "embedding.dims";

/// Ensures `chunk_vectors` exists with `dims` dimensions. Returns `true` if the
/// table was (re)created, meaning existing documents must be re-embedded.
pub fn ensure_table(db: &mut Database, dims: usize) -> SidekickResult<bool> {
    if dims == 0 || dims > 8192 {
        return Err(SidekickError::new(
            ErrorCode::Internal,
            "Unsupported embedding size.",
        ));
    }
    let current = settings::get(db, DIMS_SETTING)?.and_then(|v| v.as_u64());
    if current == Some(dims as u64) {
        return Ok(false);
    }
    let tx = db
        .conn_mut()
        .transaction()
        .map_err(|e| db_error("resizing the vector index", &e))?;
    tx.execute_batch(&format!(
        "DROP TABLE IF EXISTS chunk_vectors;
         CREATE VIRTUAL TABLE chunk_vectors USING vec0(chunk_id INTEGER PRIMARY KEY, embedding float[{dims}]);"
    ))
    .map_err(|e| db_error("resizing the vector index", &e))?;
    tx.execute(
        "INSERT INTO settings (key, value_json) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
        (DIMS_SETTING, dims.to_string()),
    )
    .map_err(|e| db_error("resizing the vector index", &e))?;
    tx.commit()
        .map_err(|e| db_error("resizing the vector index", &e))?;
    Ok(true)
}

/// Stores or replaces the vector for a chunk.
pub fn upsert(db: &Database, chunk_id: i64, embedding: &[f32]) -> SidekickResult<()> {
    let conn = db.conn();
    conn.execute("DELETE FROM chunk_vectors WHERE chunk_id = ?1", [chunk_id])
        .map_err(|e| db_error("saving a vector", &e))?;
    conn.execute(
        "INSERT INTO chunk_vectors (chunk_id, embedding) VALUES (?1, ?2)",
        (chunk_id, to_blob(embedding)),
    )
    .map_err(|e| db_error("saving a vector", &e))?;
    Ok(())
}

/// The `k` nearest chunks to `query`, closest first, as (chunk_id, distance).
pub fn nearest(db: &Database, query: &[f32], k: usize) -> SidekickResult<Vec<(i64, f64)>> {
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT chunk_id, distance FROM chunk_vectors
             WHERE embedding MATCH ?1 AND k = ?2 ORDER BY distance",
        )
        .map_err(|e| db_error("searching vectors", &e))?;
    #[allow(clippy::cast_possible_wrap)]
    let rows = stmt
        .query_map((to_blob(query), k as i64), |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| db_error("searching vectors", &e))?;
    rows.collect::<Result<_, _>>()
        .map_err(|e| db_error("searching vectors", &e))
}

fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}
