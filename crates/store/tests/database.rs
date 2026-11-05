//! Integration tests for the encrypted database (Task T-005).

#![allow(clippy::unwrap_used)] // Test helpers outside #[test] functions.

use sidekick_secrets::SecretString;
use sidekick_store::{DB_FILE_NAME, Database, LATEST_VERSION, settings, vectors};

fn key(byte: char) -> SecretString {
    SecretString::from(byte.to_string().repeat(64))
}

fn count(db: &Database, table: &str) -> i64 {
    db.conn()
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

/// Inserts a session with one row in every child table.
fn seed_session(db: &Database, id: &str) {
    db.conn()
        .execute_batch(&format!(
            "INSERT INTO sessions (id, title, status, stt_provider, llm_provider, llm_model, created_at)
               VALUES ('{id}', 'Acme QBR', 'draft', 'deepgram', 'anthropic', 'model', 1);
             INSERT INTO documents (id, session_id, name, mime_type, size_bytes, sha256, embedding_model, status, created_at)
               VALUES ('{id}-d', '{id}', 'spec.pdf', 'application/pdf', 10, 'abc', 'm', 'ready', 1);
             INSERT INTO chunks (document_id, ord, page, text, token_count)
               VALUES ('{id}-d', 0, 1, 'The Pro tier allows 600 requests per minute', 9);
             INSERT INTO transcript_segments (session_id, speaker_label, is_user, text, start_ms, end_ms)
               VALUES ('{id}', 'me', 1, 'hello', 0, 500);
             INSERT INTO screenshots (id, session_id, image, mime_type, width, height, created_at)
               VALUES ('{id}-s', '{id}', x'00', 'image/png', 1, 1, 1);
             INSERT INTO notes (session_id, content_md, version, updated_at) VALUES ('{id}', '# Notes', 1, 1);
             INSERT INTO action_items (id, session_id, text) VALUES ('{id}-a', '{id}', 'Send deck');
             INSERT INTO bookmarks (id, session_id, at_ms) VALUES ('{id}-b', '{id}', 100);
             INSERT INTO questions (id, session_id, source, text, created_at) VALUES ('{id}-q', '{id}', 'chat', 'Rate limit?', 1);
             INSERT INTO answers (id, question_id, body_md, sources_json, provider, model, latency_ms, created_at)
               VALUES ('{id}-ans', '{id}-q', '600/min', '[]', 'mock', 'm', 10, 1);
             INSERT INTO consent_log (id, session_id, option, confirmed_at) VALUES ('{id}-c', '{id}', 'me_only', 1);"
        ))
        .unwrap();
}

#[test]
fn fresh_database_migrates_to_latest() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(&dir.path().join(DB_FILE_NAME), &key('a')).unwrap();
    assert_eq!(db.schema_version().unwrap(), LATEST_VERSION);
    assert!(
        db.backups().is_empty(),
        "no backup for a brand-new database"
    );
}

#[test]
fn reopening_with_the_same_key_keeps_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    {
        let db = Database::open(&path, &key('a')).unwrap();
        settings::set(&db, "llm.active_provider", &serde_json::json!("anthropic")).unwrap();
    }
    let db = Database::open(&path, &key('a')).unwrap();
    assert_eq!(
        settings::get(&db, "llm.active_provider").unwrap(),
        Some(serde_json::json!("anthropic"))
    );
}

#[test]
fn wrong_key_cannot_open_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    drop(Database::open(&path, &key('a')).unwrap());
    let err = Database::open(&path, &key('b')).unwrap_err();
    assert_eq!(err.code, sidekick_core::ErrorCode::DbError);
}

#[test]
fn file_on_disk_is_encrypted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    {
        let db = Database::open(&path, &key('a')).unwrap();
        seed_session(&db, "s1");
        db.conn()
            .pragma_update(None, "wal_checkpoint", "TRUNCATE")
            .unwrap();
    }
    let bytes = std::fs::read(&path).unwrap();
    assert!(
        !bytes.starts_with(b"SQLite format 3"),
        "plain SQLite header found"
    );
    let needle = b"600 requests per minute";
    assert!(
        !bytes.windows(needle.len()).any(|w| w == needle),
        "plaintext found in file"
    );
    // A plain SQLite connection (no key) can't read it.
    let plain = rusqlite::Connection::open(&path).unwrap();
    assert!(
        plain
            .query_row("SELECT count(*) FROM sqlite_master", [], |r| r
                .get::<_, i64>(0))
            .is_err()
    );
}

#[test]
fn deleting_a_session_cascades_to_every_child_table() {
    let db = Database::open_in_memory(&key('a')).unwrap();
    seed_session(&db, "s1");
    seed_session(&db, "s2");
    db.conn()
        .execute("DELETE FROM sessions WHERE id = 's1'", [])
        .unwrap();
    for table in [
        "documents",
        "chunks",
        "transcript_segments",
        "screenshots",
        "notes",
        "action_items",
        "bookmarks",
        "questions",
        "answers",
        "consent_log",
    ] {
        assert_eq!(count(&db, table), 1, "{table} should only hold s2's row");
    }
}

#[test]
fn foreign_keys_are_enforced() {
    let db = Database::open_in_memory(&key('a')).unwrap();
    let err = db.conn().execute(
        "INSERT INTO notes (session_id, content_md, version, updated_at) VALUES ('missing', '', 1, 1)",
        [],
    );
    assert!(err.is_err());
}

#[test]
fn full_text_index_follows_chunk_changes() {
    let db = Database::open_in_memory(&key('a')).unwrap();
    seed_session(&db, "s1");
    let hits = |q: &str| -> i64 {
        db.conn()
            .query_row(
                "SELECT count(*) FROM chunks_fts WHERE chunks_fts MATCH ?1",
                [q],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(hits("requests"), 1, "insert trigger");
    db.conn()
        .execute("UPDATE chunks SET text = 'Enterprise has no limit'", [])
        .unwrap();
    assert_eq!(hits("requests"), 0, "update trigger removes old text");
    assert_eq!(hits("enterprise"), 1, "update trigger adds new text");
    db.conn().execute("DELETE FROM chunks", []).unwrap();
    assert_eq!(hits("enterprise"), 0, "delete trigger");
}

#[test]
fn vector_search_returns_nearest_chunks() {
    let mut db = Database::open_in_memory(&key('a')).unwrap();
    assert!(
        vectors::ensure_table(&mut db, 3).unwrap(),
        "created on first use"
    );
    assert!(
        !vectors::ensure_table(&mut db, 3).unwrap(),
        "unchanged when dims match"
    );
    vectors::upsert(&db, 1, &[1.0, 0.0, 0.0]).unwrap();
    vectors::upsert(&db, 2, &[0.0, 1.0, 0.0]).unwrap();
    vectors::upsert(&db, 3, &[0.9, 0.1, 0.0]).unwrap();
    let hits = vectors::nearest(&db, &[1.0, 0.0, 0.0], 2).unwrap();
    let ids: Vec<i64> = hits.iter().map(|h| h.0).collect();
    assert_eq!(ids, vec![1, 3]);
    // Upsert replaces.
    vectors::upsert(&db, 2, &[1.0, 0.0, 0.0]).unwrap();
    assert_eq!(vectors::nearest(&db, &[1.0, 0.0, 0.0], 3).unwrap().len(), 3);
}

#[test]
fn changing_embedding_size_recreates_the_vector_table() {
    let mut db = Database::open_in_memory(&key('a')).unwrap();
    vectors::ensure_table(&mut db, 3).unwrap();
    vectors::upsert(&db, 1, &[1.0, 0.0, 0.0]).unwrap();
    assert!(vectors::ensure_table(&mut db, 4).unwrap());
    assert_eq!(
        count(&db, "chunk_vectors"),
        0,
        "old vectors dropped for re-embedding"
    );
    vectors::upsert(&db, 1, &[1.0, 0.0, 0.0, 0.0]).unwrap();
    assert!(vectors::ensure_table(&mut db, 0).is_err());
}

#[test]
fn settings_round_trip() {
    let db = Database::open_in_memory(&key('a')).unwrap();
    settings::set(&db, "a", &serde_json::json!({"x": 1})).unwrap();
    settings::set(&db, "a", &serde_json::json!({"x": 2})).unwrap();
    settings::set(&db, "b", &serde_json::json!(true)).unwrap();
    assert_eq!(
        settings::get(&db, "a").unwrap(),
        Some(serde_json::json!({"x": 2}))
    );
    assert_eq!(settings::get(&db, "missing").unwrap(), None);
    assert_eq!(settings::all(&db).unwrap().len(), 2);
}
