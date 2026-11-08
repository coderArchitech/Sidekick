-- Migration 0005: Knowledge Packs attached to sessions (PRD CTX-5) and the
-- pre-meeting brief (CTX-4).
CREATE TABLE session_packs (
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  pack_id    TEXT NOT NULL REFERENCES knowledge_packs(id) ON DELETE CASCADE,
  PRIMARY KEY (session_id, pack_id)
);
CREATE INDEX session_packs_pack ON session_packs(pack_id);

CREATE TABLE briefs (
  session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
  content_md TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
