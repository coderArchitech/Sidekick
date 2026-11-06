//! Retention and deletion (Task T-030; tech doc 4.4).

#![allow(clippy::unwrap_used)]

use rusqlite::params;
use sidekick_core::retention::Retention;
use sidekick_secrets::SecretString;
use sidekick_session::repo::{self, NewSession};
use sidekick_session::retention::{self, CONSENT_KEEP_MS};
use sidekick_session::{ConsentOption, SessionStatus};
use sidekick_store::{Database, vectors};
use sidekick_stt::Segment;

const DAY: i64 = 24 * 60 * 60 * 1000;
const SECRET_PHRASE: &str = "zebra-quartz-falcon confidential roadmap";

fn new(title: &str, retention: Retention) -> NewSession<'_> {
    NewSession {
        title,
        stt_provider: "deepgram",
        llm_provider: "anthropic",
        llm_model: "m",
        auto_answer: true,
        retention,
    }
}

/// A session with something in every table that belongs to it.
fn full_session(db: &mut Database, title: &str, retention: Retention, ended_at: i64) -> String {
    let s = repo::create(db, &new(title, retention)).unwrap();
    repo::record_consent(db, &s.id, ConsentOption::ParticipantsAgreed).unwrap();
    let seg = Segment {
        speaker_label: "speaker_1".into(),
        is_user: false,
        text: SECRET_PHRASE.into(),
        start_ms: 0,
        end_ms: 1000,
        confidence: None,
    };
    repo::insert_segment(db, &s.id, &seg).unwrap();
    repo::add_bookmark(db, &s.id, 500, Some("mark")).unwrap();
    let c = db.conn();
    c.execute(
        "INSERT INTO documents (id, session_id, name, mime_type, size_bytes, sha256, embedding_model, status, created_at)
         VALUES (?1, ?2, 'spec.md', 'text/markdown', 10, ?1, 'mock', 'ready', 0)",
        params![format!("doc-{title}"), s.id],
    )
    .unwrap();
    c.execute(
        "INSERT INTO chunks (document_id, ord, text, token_count) VALUES (?1, 0, ?2, 5)",
        params![format!("doc-{title}"), SECRET_PHRASE],
    )
    .unwrap();
    let chunk_id = c.last_insert_rowid();
    c.execute(
        "INSERT INTO screenshots (id, session_id, image, mime_type, width, height, created_at) VALUES (?1, ?2, x'00', 'image/jpeg', 1, 1, 0)",
        params![format!("shot-{title}"), s.id],
    )
    .unwrap();
    c.execute(
        "INSERT INTO notes (session_id, content_md, version, updated_at) VALUES (?1, ?2, 1, 0)",
        params![s.id, SECRET_PHRASE],
    )
    .unwrap();
    vectors::ensure_table(db, 4).unwrap();
    vectors::upsert(
        db,
        chunk_id,
        &vectors::session_owner(&s.id),
        &[0.1, 0.2, 0.3, 0.4],
    )
    .unwrap();
    repo::set_status(db, &s.id, SessionStatus::Live).unwrap();
    repo::set_status(db, &s.id, SessionStatus::Ended).unwrap();
    db.conn()
        .execute(
            "UPDATE sessions SET ended_at = ?2 WHERE id = ?1",
            params![s.id, ended_at],
        )
        .unwrap();
    s.id
}

fn count(db: &Database, sql: &str) -> i64 {
    db.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

fn mem() -> Database {
    Database::open_in_memory(&SecretString::from("r".repeat(64))).unwrap()
}

#[test]
fn expiry_follows_each_sessions_retention() {
    let mut db = mem();
    let now = 100 * DAY;
    let ids = [
        full_session(&mut db, "ephemeral", Retention::SessionOnly, now - 1),
        full_session(&mut db, "week-old", Retention::SevenDays, now - 8 * DAY),
        full_session(&mut db, "recent", Retention::SevenDays, now - 6 * DAY),
        full_session(&mut db, "month", Retention::ThirtyDays, now - 29 * DAY),
        full_session(&mut db, "forever", Retention::Forever, 0),
    ];
    let expired = retention::expired(&db, now).unwrap();
    assert_eq!(expired, vec![ids[0].clone(), ids[1].clone()]);
    // Live and draft sessions never expire, whatever their retention.
    let draft = repo::create(&db, &new("draft", Retention::SessionOnly)).unwrap();
    assert!(
        !retention::expired(&db, now + 1000 * DAY)
            .unwrap()
            .contains(&draft.id)
    );
}

#[test]
fn expired_sessions_go_with_every_child_row_and_consent_is_kept() {
    let mut db = mem();
    let now = 100 * DAY;
    let old = full_session(&mut db, "old", Retention::SevenDays, now - 10 * DAY);
    let kept = full_session(&mut db, "kept", Retention::Forever, now - 10 * DAY);
    let report = retention::run(&db, now).unwrap();
    assert_eq!(report.sessions_deleted, 1);

    assert!(repo::get(&db, &old).is_err());
    for (table, expected) in [
        ("transcript_segments", 1),
        ("bookmarks", 1),
        ("documents", 1),
        ("chunks", 1),
        ("screenshots", 1),
        ("notes", 1),
        ("chunk_vectors", 1),
    ] {
        assert_eq!(
            count(&db, &format!("SELECT count(*) FROM {table}")),
            expected,
            "{table}"
        );
    }
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM chunks_fts WHERE chunks_fts MATCH '\"zebra\"'"
        ),
        1,
        "search index keeps only the remaining session's chunk"
    );
    assert_eq!(
        count(
            &db,
            &format!(
                "SELECT count(*) FROM chunk_vectors WHERE owner = '{}'",
                vectors::session_owner(&old)
            )
        ),
        0,
        "vectors have no foreign key and must be deleted explicitly"
    );
    assert_eq!(
        count(
            &db,
            &format!("SELECT count(*) FROM consent_log WHERE session_id = '{old}'")
        ),
        1,
        "consent kept"
    );
    assert!(repo::get(&db, &kept).is_ok());
}

#[test]
fn consent_records_are_kept_for_a_year() {
    let mut db = mem();
    let now = 800 * DAY;
    let s = full_session(&mut db, "x", Retention::Forever, now);
    db.conn()
        .execute(
            "UPDATE consent_log SET confirmed_at = ?1",
            [now - CONSENT_KEEP_MS + DAY],
        )
        .unwrap();
    assert_eq!(retention::run(&db, now).unwrap().consent_records_deleted, 0);
    db.conn()
        .execute(
            "UPDATE consent_log SET confirmed_at = ?1",
            [now - CONSENT_KEEP_MS - DAY],
        )
        .unwrap();
    assert_eq!(retention::run(&db, now).unwrap().consent_records_deleted, 1);
    assert!(
        repo::get(&db, &s).is_ok(),
        "the session itself is unaffected"
    );
}

#[test]
fn delete_everything_keeps_settings_skills_and_consent() {
    let mut db = mem();
    full_session(&mut db, "a", Retention::Forever, 0);
    full_session(&mut db, "b", Retention::Forever, 0);
    db.conn()
        .execute_batch(
            "INSERT INTO settings (key, value_json) VALUES ('autoAnswer', 'false');
             INSERT INTO knowledge_packs (id, name, created_at) VALUES ('p1', 'Pack', 0);",
        )
        .unwrap();
    let settings_before = count(&db, "SELECT count(*) FROM settings");
    assert_eq!(retention::delete_all(&db).unwrap(), 2);
    for table in [
        "sessions",
        "documents",
        "chunks",
        "chunk_vectors",
        "knowledge_packs",
        "notes",
        "screenshots",
    ] {
        assert_eq!(
            count(&db, &format!("SELECT count(*) FROM {table}")),
            0,
            "{table}"
        );
    }
    assert_eq!(count(&db, "SELECT count(*) FROM settings"), settings_before);
    assert_eq!(count(&db, "SELECT count(*) FROM consent_log"), 2);
}

#[test]
fn after_delete_and_vacuum_the_text_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sidekick.db");
    let key = SecretString::from("v".repeat(64));
    let mut db = Database::open(&path, &key).unwrap();
    let id = full_session(&mut db, "secret", Retention::SessionOnly, 0);
    assert_eq!(
        count(&db, "PRAGMA secure_delete"),
        1,
        "deleted rows are overwritten"
    );
    repo::delete(&db, &id).unwrap();
    db.vacuum().unwrap();
    assert_eq!(
        count(&db, "PRAGMA freelist_count"),
        0,
        "no free pages left after VACUUM"
    );
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM chunks_fts WHERE chunks_fts MATCH '\"zebra\"'"
        ),
        0
    );
    drop(db);
    // The file is encrypted, and after VACUUM no page holds the text in any form.
    for f in std::fs::read_dir(dir.path()).unwrap() {
        let bytes = std::fs::read(f.unwrap().path()).unwrap();
        assert!(
            !bytes
                .windows(SECRET_PHRASE.len())
                .any(|w| w == SECRET_PHRASE.as_bytes())
        );
    }
    let db = Database::open(&path, &key).unwrap();
    assert_eq!(count(&db, "SELECT count(*) FROM sessions"), 0);
}

#[test]
fn retention_can_change_per_session() {
    let db = mem();
    let s = repo::create(&db, &new("t", Retention::ThirtyDays)).unwrap();
    assert_eq!(s.retention, Retention::ThirtyDays);
    assert_eq!(
        repo::set_retention(&db, &s.id, Retention::Forever)
            .unwrap()
            .retention,
        Retention::Forever
    );
    assert!(repo::set_retention(&db, "missing", Retention::Forever).is_err());
}
