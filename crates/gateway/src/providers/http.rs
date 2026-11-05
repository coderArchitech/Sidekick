//! Shared HTTP error mapping for provider adapters (tech doc 5.4, 7.5).

use std::time::Duration;

use sidekick_core::{ErrorCode, SidekickError};

/// Maps an HTTP failure to a stable code. `provider` is a display name; the
/// message never includes the response body (it may echo request content).
pub fn status_error(provider: &str, status: u16, retry_after: Option<&str>) -> SidekickError {
    let (code, message) = match status {
        401 | 403 => (
            ErrorCode::KeyInvalid,
            format!("{provider} rejected the API key."),
        ),
        429 => (
            ErrorCode::RateLimited,
            format!("{provider} is rate limiting requests. Retrying shortly."),
        ),
        413 => (
            ErrorCode::ContextTooLarge,
            format!("The request was too large for {provider}."),
        ),
        400 | 404 | 422 => (
            ErrorCode::Internal,
            format!("{provider} rejected the request (HTTP {status})."),
        ),
        // 5xx and Anthropic's 529 "overloaded".
        _ => (
            ErrorCode::ProviderUnavailable,
            format!("{provider} is unavailable (HTTP {status})."),
        ),
    };
    SidekickError::new(code, message).with_retry_after(retry_after.and_then(parse_retry_after))
}

/// Network failure before any response.
pub fn transport_error(provider: &str, err: &reqwest::Error) -> SidekickError {
    tracing::warn!(target: "gateway", provider, timeout = err.is_timeout(), connect = err.is_connect(), "transport error");
    SidekickError::new(
        ErrorCode::ProviderUnavailable,
        format!("Couldn't reach {provider}. Check your internet connection."),
    )
}

fn parse_retry_after(value: &str) -> Option<Duration> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|s| *s >= 0.0)
        .map(Duration::from_secs_f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_documented_status_codes() {
        assert_eq!(status_error("X", 401, None).code, ErrorCode::KeyInvalid);
        assert_eq!(status_error("X", 403, None).code, ErrorCode::KeyInvalid);
        assert_eq!(status_error("X", 429, None).code, ErrorCode::RateLimited);
        assert_eq!(
            status_error("X", 500, None).code,
            ErrorCode::ProviderUnavailable
        );
        assert_eq!(
            status_error("X", 529, None).code,
            ErrorCode::ProviderUnavailable
        );
        assert!(status_error("X", 503, None).retryable);
        assert!(!status_error("X", 401, None).retryable);
    }

    #[test]
    fn parses_retry_after_seconds() {
        let e = status_error("X", 429, Some("3"));
        assert_eq!(e.retry_after, Some(Duration::from_secs(3)));
        assert_eq!(status_error("X", 429, Some("soon")).retry_after, None);
    }
}
