//! Shared building blocks for every Sidekick crate.
//!
//! Rule (tech doc 7.8): this crate depends on no other Sidekick crate and no
//! Tauri code, so every domain crate can depend on it without cycles.

pub mod embed;
pub mod error;
pub mod keys;
pub mod llm;
pub mod retention;

pub use error::{ErrorCode, SidekickError, SidekickResult};
