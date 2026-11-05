-- Migration 0003: the end-of-session summary (PRD MEM-3) lives with the notes.
ALTER TABLE notes ADD COLUMN summary_md TEXT;
