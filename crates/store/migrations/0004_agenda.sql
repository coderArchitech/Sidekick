-- Migration 0004: agenda items per session (PRD MEM-2).
CREATE TABLE agenda_items (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  text TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'not_started'
    CHECK (status IN ('not_started', 'in_progress', 'covered'))
);
CREATE INDEX agenda_items_session ON agenda_items(session_id, position);
