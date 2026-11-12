-- Migration 0007: a third consent choice, "Transcribe the other people only"
-- (system audio, never the microphone). SQLite can't alter a CHECK, so the
-- consent log is rebuilt with the new option allowed; every row is kept.
CREATE TABLE consent_log_new (
  id            TEXT PRIMARY KEY,
  session_id    TEXT NOT NULL,                 -- no FK: the session may be deleted
  session_title TEXT NOT NULL DEFAULT '',
  option        TEXT NOT NULL CHECK (option IN ('participants_agreed','me_only','others_only')),
  confirmed_at  INTEGER NOT NULL
);
INSERT INTO consent_log_new (id, session_id, session_title, option, confirmed_at)
  SELECT id, session_id, session_title, option, confirmed_at FROM consent_log;
DROP TABLE consent_log;
ALTER TABLE consent_log_new RENAME TO consent_log;
CREATE INDEX idx_consent_session ON consent_log(session_id);
