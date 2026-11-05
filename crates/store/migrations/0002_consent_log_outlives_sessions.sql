-- Migration 0002: the consent log must outlive the session (tech doc 4.4: kept
-- 1 year so the user can show consent later). 0001 deleted it with the session.
-- Drop the cascading foreign key and keep a copy of the session title.
CREATE TABLE consent_log_new (
  id            TEXT PRIMARY KEY,
  session_id    TEXT NOT NULL,                 -- no FK: the session may be deleted
  session_title TEXT NOT NULL DEFAULT '',
  option        TEXT NOT NULL CHECK (option IN ('participants_agreed','me_only')),
  confirmed_at  INTEGER NOT NULL
);
INSERT INTO consent_log_new (id, session_id, session_title, option, confirmed_at)
  SELECT c.id, c.session_id, COALESCE(s.title, ''), c.option, c.confirmed_at
  FROM consent_log c LEFT JOIN sessions s ON s.id = c.session_id;
DROP TABLE consent_log;
ALTER TABLE consent_log_new RENAME TO consent_log;
CREATE INDEX idx_consent_session ON consent_log(session_id);
