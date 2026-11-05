//! Logging setup (tech doc 7.7). Level comes from `SIDEKICK_LOG` (default `info`).
//! Never log keys, prompts, transcript text, document content or answers.

use tracing_subscriber::EnvFilter;

pub fn init() {
    let filter = EnvFilter::try_from_env("SIDEKICK_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    // File output with rotation (tech doc 10.3) arrives with the diagnostics crate.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .try_init();
}
