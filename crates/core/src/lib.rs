//! Shared building blocks for every Sidekick crate.
//!
//! Rule (tech doc 7.8): this crate depends on no other Sidekick crate and no
//! Tauri code, so every domain crate can depend on it without cycles.

pub mod error;

pub use error::{ErrorCode, SidekickError, SidekickResult};
