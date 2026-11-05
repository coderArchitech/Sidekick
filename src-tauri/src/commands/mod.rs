//! IPC commands, one file per command group (tech doc 7.1, 7.5).
//! Commands stay thin: validate, call a crate, return its result.

pub mod app;
pub mod keys;
pub mod llm;
pub mod settings;
