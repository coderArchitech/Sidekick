//! Integration: capture → downscale → store encrypted with the session.

#![allow(clippy::unwrap_used)]

use secrecy::SecretString;
use sidekick_capture::{CaptureTarget, Capturer, MockCapturer, prepare, repo};
use sidekick_store::Database;

fn session(db: &Database, id: &str) {
    db.conn()
        .execute(
            "INSERT INTO sessions (id, title, status, stt_provider, llm_provider, llm_model, created_at)
             VALUES (?1, 't', 'live', 'mock', 'mock', 'mock', 0)",
            [id],
        )
        .unwrap();
}

#[test]
fn screenshot_is_stored_encrypted_and_linked_to_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sidekick.db");
    let key = SecretString::from("c".repeat(64));
    let db = Database::open(&path, &key).unwrap();
    session(&db, "s1");
    session(&db, "s2");

    let encoded = prepare(MockCapturer.capture(CaptureTarget::Screen).unwrap()).unwrap();
    let shot = repo::save(&db, "s1", &encoded).unwrap();
    assert_eq!((shot.width, shot.height), (1568, 882));
    assert_eq!(repo::list(&db, "s1").unwrap(), vec![shot.clone()]);
    assert!(repo::list(&db, "s2").unwrap().is_empty());
    assert_eq!(repo::latest(&db, "s1", 0).unwrap(), Some(shot.clone()));
    assert_eq!(repo::latest(&db, "s1", shot.created_at + 1).unwrap(), None);
    assert_eq!(repo::image(&db, &shot.id).unwrap().bytes, encoded.bytes);
    assert!(
        repo::image(&db, &shot.id)
            .unwrap()
            .data_url()
            .starts_with("data:image/jpeg;base64,/9j/")
    );
    drop(db);

    // The JPEG never appears in the file on disk.
    let raw = std::fs::read(&path).unwrap();
    let marker = &encoded.bytes[..64];
    assert!(
        !raw.windows(marker.len()).any(|w| w == marker),
        "image bytes must be encrypted at rest"
    );

    // Deleting the session deletes its screenshots.
    let db = Database::open(&path, &key).unwrap();
    db.conn()
        .execute("DELETE FROM sessions WHERE id = 's1'", [])
        .unwrap();
    assert!(repo::image(&db, &shot.id).is_err());
    assert!(repo::save(&db, "missing", &encoded).is_err());
}
