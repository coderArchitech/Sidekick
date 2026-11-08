-- Migration 0006: more than one Skill per session (PRD SKL-5). `sessions.skill_id`
-- stays the main Skill (switched by the dropdown and hotkey); these are the
-- Skills also in use, in order.
CREATE TABLE session_skills (
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  skill_id   TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
  position   INTEGER NOT NULL,
  PRIMARY KEY (session_id, skill_id)
);
