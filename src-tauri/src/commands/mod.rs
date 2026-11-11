//! IPC commands, one file per command group (tech doc 7.1, 7.5).
//! Commands stay thin: validate, call a crate, return its result.

pub mod app;
pub mod capture;
pub mod data;
pub mod diagnostics;
pub mod documents;
pub mod keys;
pub mod llm;
pub mod notes;
pub mod overlay;
pub mod packs;
pub mod recovery;
pub mod retrieval;
pub mod search;
pub mod session;
pub mod settings;
pub mod skills;
pub mod stt;
