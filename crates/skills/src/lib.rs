//! Skills (tech doc 3.2 `skills`, 5.7; PRD SKL-1 to SKL-3; ADR-009).
//!
//! A Skill is a Markdown file: YAML front matter (validated against
//! `schemas/skill.schema.json`) and a body with the instructions.
//!
//! - [`format`]: parse, validate and write Skill files, with field-level errors.
//! - [`repo`]: the Skills folder (the files are the source of truth) and the
//!   `skills` index table; built-in templates seeded on first run.

pub mod format;
pub mod repo;

pub use format::{AnswerFormat, FieldError, Skill, Tool, Trigger, parse, to_markdown, validate};
pub use repo::{ImportPreview, SkillEntry, TEMPLATES};
