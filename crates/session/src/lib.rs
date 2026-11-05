//! Sessions and consent (tech doc 3.2 `session`, `consent`; PRD CTX-1, section 8).
//!
//! - [`model`]: the session lifecycle and consent rules, as pure functions.
//! - [`repo`]: this crate's tables (`sessions`, `consent_log`, `transcript_segments`).
//!   The store crate owns the schema; each domain crate owns the SQL for its tables.
//! - [`runner`]: a live session: one audio capture plus one STT run per stream.

pub mod answers;
pub mod model;
pub mod notes;
pub mod repo;
pub mod runner;

pub use model::{ConsentOption, Session, SessionStatus, streams_for};
pub use runner::{AudioFactory, DeviceAudio, FixtureAudio, LiveSession, SttPlan};
