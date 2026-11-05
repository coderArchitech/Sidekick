//! STT adapters (tech doc 7.5). WebSocket providers share the engine in [`ws`].

pub mod assemblyai;
pub mod deepgram;
pub mod mock;
pub mod soniox;
#[cfg(feature = "local-whisper")]
pub mod whisper;
pub mod ws;

use sidekick_core::{ErrorCode, SidekickError, SidekickResult};

/// Maps a key-check response status (REST) to a result.
pub(crate) fn rest_status(provider: &str, status: u16) -> SidekickResult<()> {
    match status {
        200..=299 => Ok(()),
        401 | 403 => Err(SidekickError::new(
            ErrorCode::KeyInvalid,
            format!("{provider} rejected the API key."),
        )),
        429 => Err(SidekickError::new(
            ErrorCode::RateLimited,
            format!("{provider} is rate limiting requests."),
        )),
        s => Err(SidekickError::new(
            ErrorCode::ProviderUnavailable,
            format!("{provider} is unavailable (HTTP {s})."),
        )),
    }
}
