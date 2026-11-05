//! The single error type that crosses module boundaries and IPC (tech doc 5.4).

use serde::Serialize;

/// Stable error codes. The UI decides what to show from the code, never the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    ConsentRequired,
    PermissionDenied,
    KeyMissing,
    KeyInvalid,
    RateLimited,
    ProviderUnavailable,
    SttDisconnected,
    UnsupportedFile,
    ParseFailed,
    ContextTooLarge,
    CaptureExclusionUnavailable,
    DbError,
    Cancelled,
    Internal,
}

impl ErrorCode {
    /// Whether the caller may retry the same request (tech doc 5.4).
    /// `DbError` is "maybe" in the doc; we treat it as retryable once.
    #[must_use]
    pub const fn retryable(self) -> bool {
        matches!(
            self,
            Self::RateLimited | Self::ProviderUnavailable | Self::SttDisconnected | Self::DbError
        )
    }
}

/// Error returned by every fallible Sidekick function and IPC command.
///
/// `message` is user-facing and must never contain API keys or meeting content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
#[error("{code:?}: {message}")]
pub struct SidekickError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
    /// How long the provider asked us to wait (`Retry-After`). Internal only.
    #[serde(skip)]
    #[cfg_attr(feature = "specta", specta(skip))]
    pub retry_after: Option<std::time::Duration>,
}

impl SidekickError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: code.retryable(),
            retry_after: None,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    /// Records the provider's requested wait before a retry.
    #[must_use]
    pub fn with_retry_after(mut self, wait: Option<std::time::Duration>) -> Self {
        self.retry_after = wait;
        self
    }
}

pub type SidekickResult<T> = Result<T, SidekickError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryable_follows_the_code() {
        assert!(SidekickError::new(ErrorCode::RateLimited, "busy").retryable);
        assert!(!SidekickError::new(ErrorCode::KeyInvalid, "rejected").retryable);
    }

    #[test]
    fn serialises_with_stable_wire_format() {
        let err = SidekickError::new(ErrorCode::ConsentRequired, "Confirm consent first");
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "code": "CONSENT_REQUIRED",
                "message": "Confirm consent first",
                "retryable": false
            })
        );
    }
}
