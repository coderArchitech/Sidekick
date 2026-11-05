//! Retries for provider calls (tech doc 5.5): up to 3 retries on retryable
//! errors, backoff 0.5 s, 1 s, 2 s with ±20% jitter, honouring `Retry-After`.

use std::future::Future;
use std::time::Duration;

use sidekick_core::SidekickResult;

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub delays: Vec<Duration>,
    /// Longest `Retry-After` we'll honour; beyond this we give up.
    pub max_retry_after: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            delays: vec![
                Duration::from_millis(500),
                Duration::from_secs(1),
                Duration::from_secs(2),
            ],
            max_retry_after: Duration::from_secs(10),
        }
    }
}

impl RetryPolicy {
    /// No waiting, same number of attempts. For tests.
    #[must_use]
    pub fn immediate() -> Self {
        Self {
            delays: vec![Duration::ZERO; 3],
            max_retry_after: Duration::from_secs(10),
        }
    }
}

/// Runs `op` until it succeeds, fails with a non-retryable error, or retries run out.
pub async fn with_retries<T, F, Fut>(policy: &RetryPolicy, mut op: F) -> SidekickResult<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = SidekickResult<T>>,
{
    let mut attempt = 0;
    loop {
        match op().await {
            Ok(v) => return Ok(v),
            Err(err) if err.retryable && attempt < policy.delays.len() => {
                let wait = match err.retry_after {
                    Some(after) if after > policy.max_retry_after => return Err(err),
                    Some(after) => after,
                    None => jitter(policy.delays[attempt]),
                };
                tracing::warn!(target: "gateway", code = ?err.code, attempt, wait_ms = wait.as_millis() as u64, "retrying");
                tokio::time::sleep(wait).await;
                attempt += 1;
            }
            Err(err) => return Err(err),
        }
    }
}

fn jitter(base: Duration) -> Duration {
    base.mul_f64(0.8 + fastrand::f64() * 0.4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidekick_core::{ErrorCode, SidekickError};
    use std::sync::atomic::{AtomicU32, Ordering};

    fn err(code: ErrorCode) -> SidekickError {
        SidekickError::new(code, "x")
    }

    #[tokio::test]
    async fn retries_retryable_errors_three_times_then_fails() {
        let calls = AtomicU32::new(0);
        let result: SidekickResult<()> = with_retries(&RetryPolicy::immediate(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            async { Err(err(ErrorCode::RateLimited)) }
        })
        .await;
        assert_eq!(result.unwrap_err().code, ErrorCode::RateLimited);
        assert_eq!(calls.load(Ordering::SeqCst), 4, "1 attempt + 3 retries");
    }

    #[tokio::test]
    async fn does_not_retry_non_retryable_errors() {
        let calls = AtomicU32::new(0);
        let _ = with_retries(&RetryPolicy::immediate(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            async { Err::<(), _>(err(ErrorCode::KeyInvalid)) }
        })
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn succeeds_after_a_transient_failure() {
        let calls = AtomicU32::new(0);
        let result = with_retries(&RetryPolicy::immediate(), || {
            let n = calls.fetch_add(1, Ordering::SeqCst);
            async move {
                if n == 0 {
                    Err(err(ErrorCode::ProviderUnavailable))
                } else {
                    Ok(42)
                }
            }
        })
        .await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test(start_paused = true)]
    async fn honours_retry_after() {
        let start = tokio::time::Instant::now();
        let calls = AtomicU32::new(0);
        let _ = with_retries(&RetryPolicy::default(), || {
            let n = calls.fetch_add(1, Ordering::SeqCst);
            async move {
                if n == 0 {
                    Err(err(ErrorCode::RateLimited).with_retry_after(Some(Duration::from_secs(7))))
                } else {
                    Ok(())
                }
            }
        })
        .await;
        assert!(start.elapsed() >= Duration::from_secs(7));
    }

    #[tokio::test]
    async fn gives_up_when_retry_after_is_too_long() {
        let calls = AtomicU32::new(0);
        let _ = with_retries(&RetryPolicy::immediate(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            async {
                Err::<(), _>(
                    err(ErrorCode::RateLimited).with_retry_after(Some(Duration::from_secs(60))),
                )
            }
        })
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn jitter_stays_within_20_percent() {
        for _ in 0..100 {
            let j = jitter(Duration::from_millis(1000)).as_millis();
            assert!((800..=1200).contains(&j), "{j}");
        }
    }
}
