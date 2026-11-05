-- Migration 0001: initial schema (tech doc 4.2). Never edit after release; add 0002_... instead.
CREATE TABLE sessions (
  id              TEXT PRIMARY KEY,            -- UUID v7
  title           TEXT NOT NULL,
  status          TEXT NOT NULL CHECK (status IN ('draft','live','paused','ended')),
  stt_provider    TEXT NOT NULL,
  llm_provider    TEXT NOT NULL,
  llm_model       TEXT NOT NULL,
  skill_id        TEXT REFERENCES skills(id),
  auto_answer     INTEGER NOT NULL DEFAULT 1,
  retention       TEXT NOT NULL DEFAULT '30d',  -- 'session','7d','30d','forever'
  created_at      INTEGER NOT NULL,             -- Unix ms
  started_at      INTEGER,
  ended_at        INTEGER
);

CREATE TABLE knowledge_packs (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at INTEGER NOT NULL
);

CREATE TABLE documents (
  id              TEXT PRIMARY KEY,
  session_id      TEXT REFERENCES sessions(id) ON DELETE CASCADE,
  pack_id         TEXT REFERENCES knowledge_packs(id) ON DELETE CASCADE,
  name            TEXT NOT NULL,
  mime_type       TEXT NOT NULL,
  size_bytes      INTEGER NOT NULL,
  sha256          TEXT NOT NULL,
  page_count      INTEGER,
  embedding_model TEXT NOT NULL,
  status          TEXT NOT NULL CHECK (status IN ('queued','parsing','embedding','ready','failed')),
  error           TEXT,
  created_at      INTEGER NOT NULL,
  CHECK (session_id IS NOT NULL OR pack_id IS NOT NULL)
);

CREATE TABLE chunks (
  id          INTEGER PRIMARY KEY,
  document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  ord         INTEGER NOT NULL,
  page        INTEGER,
  text        TEXT NOT NULL,
  token_count INTEGER NOT NULL
);
CREATE INDEX idx_chunks_document ON chunks(document_id, ord);

-- chunk_vectors is NOT created by this migration: its size depends on the active
-- embedding model (ADR-013). The store creates it on first use:
--   CREATE VIRTUAL TABLE chunk_vectors USING vec0(chunk_id INTEGER PRIMARY KEY, embedding float[<dims>]);
-- and records the dimension in settings key 'embedding.dims'. Changing model drops
-- and recreates it, then the re-embedding job refills it (section 4.5).

CREATE VIRTUAL TABLE chunks_fts USING fts5(text, content='chunks', content_rowid='id');

-- Keep the external-content FTS table in sync with chunks.
CREATE TRIGGER chunks_ai AFTER INSERT ON chunks BEGIN
  INSERT INTO chunks_fts(rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER chunks_ad AFTER DELETE ON chunks BEGIN
  INSERT INTO chunks_fts(chunks_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
CREATE TRIGGER chunks_au AFTER UPDATE ON chunks BEGIN
  INSERT INTO chunks_fts(chunks_fts, rowid, text) VALUES ('delete', old.id, old.text);
  INSERT INTO chunks_fts(rowid, text) VALUES (new.id, new.text);
END;

CREATE TABLE transcript_segments (
  id            INTEGER PRIMARY KEY,
  session_id    TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  speaker_label TEXT NOT NULL,                  -- 'me', 'speaker_1', or a renamed label
  is_user       INTEGER NOT NULL,
  text          TEXT NOT NULL,
  start_ms      INTEGER NOT NULL,               -- offset from session start
  end_ms        INTEGER NOT NULL,
  confidence    REAL
);
CREATE INDEX idx_segments_session ON transcript_segments(session_id, start_ms);

CREATE TABLE screenshots (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  image BLOB NOT NULL, mime_type TEXT NOT NULL,
  width INTEGER NOT NULL, height INTEGER NOT NULL,
  description TEXT, created_at INTEGER NOT NULL
);

CREATE TABLE notes (
  session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
  content_md TEXT NOT NULL, version INTEGER NOT NULL, updated_at INTEGER NOT NULL
);

CREATE TABLE action_items (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  text TEXT NOT NULL, owner TEXT, due TEXT, done INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE bookmarks (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  at_ms INTEGER NOT NULL, label TEXT
);

CREATE TABLE questions (
  id         TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  segment_id INTEGER REFERENCES transcript_segments(id),
  source     TEXT NOT NULL CHECK (source IN ('detected','chat','screenshot')),
  directed   INTEGER NOT NULL DEFAULT 0,
  text       TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE answers (
  id            TEXT PRIMARY KEY,
  question_id   TEXT NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
  cue           TEXT,
  body_md       TEXT NOT NULL,
  sources_json  TEXT NOT NULL,                  -- [{type, ref, label}]
  provider      TEXT NOT NULL,
  model         TEXT NOT NULL,
  latency_ms    INTEGER NOT NULL,
  input_tokens  INTEGER, output_tokens INTEGER,
  rating        INTEGER,                        -- 1 up, -1 down, NULL unrated
  created_at    INTEGER NOT NULL
);

CREATE TABLE skills (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, file_path TEXT NOT NULL,
  schema_version INTEGER NOT NULL, enabled INTEGER NOT NULL DEFAULT 1,
  updated_at INTEGER NOT NULL
);

CREATE TABLE consent_log (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  option TEXT NOT NULL CHECK (option IN ('participants_agreed','me_only')),
  confirmed_at INTEGER NOT NULL
);

CREATE TABLE usage (
  id INTEGER PRIMARY KEY,
  session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
  provider TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('stt','llm','embed','search')),
  units REAL NOT NULL, unit_type TEXT NOT NULL,   -- 'minutes','input_tokens','output_tokens','requests'
  cost_estimate_usd REAL, created_at INTEGER NOT NULL
);

CREATE TABLE settings (key TEXT PRIMARY KEY, value_json TEXT NOT NULL);
