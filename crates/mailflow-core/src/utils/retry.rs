/// Exponential backoff retry utility for resilient operations
use crate::constants::{MAX_RETRIES, RETRY_BASE_DELAY_MS, RETRY_JITTER_FACTOR, RETRY_MAX_DELAY_MS};
use crate::error::MailflowError;
use std::time::Duration;
use tracing::{debug, warn};

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: MAX_RETRIES,
            base_delay_ms: RETRY_BASE_DELAY_MS,
            max_delay_ms: RETRY_MAX_DELAY_MS,
            jitter_factor: RETRY_JITTER_FACTOR,
        }
    }
}

impl RetryConfig {
    /// Creates a new retry config with custom values
    pub fn new(max_retries: u32, base_delay_ms: u64, max_delay_ms: u64) -> Self {
        Self {
            max_retries,
            base_delay_ms,
            max_delay_ms,
            jitter_factor: RETRY_JITTER_FACTOR,
        }
    }

    /// Calculates delay for a given attempt with exponential backoff and jitter
    ///
    /// Formula: min(base_delay * 2^attempt, max_delay) * (1 ± jitter)
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        // Exponential backoff: base_delay * 2^attempt
        let exponential_ms = self
            .base_delay_ms
            .saturating_mul(2u64.saturating_pow(attempt));

        // Cap at max delay
        let capped_ms = exponential_ms.min(self.max_delay_ms);

        // Add jitter: ±10% randomness
        let jitter = (rand::random::<f64>() - 0.5) * 2.0 * self.jitter_factor;
        let jittered_ms = (capped_ms as f64 * (1.0 + jitter)).max(0.0) as u64;

        Duration::from_millis(jittered_ms)
    }
}

/// Retries an async operation with exponential backoff
///
/// # Arguments
/// * `operation` - The async operation to retry
/// * `config` - Retry configuration
/// * `operation_name` - Name for logging
///
/// # Returns
/// * `Ok(T)` - If operation succeeds
/// * `Err(MailflowError)` - If all retries exhausted or permanent error
///
/// # Example
/// ```ignore
/// let result = retry_with_backoff(
///     || async { s3_client.get_object().send().await },
///     RetryConfig::default(),
///     "s3_download"
/// ).await?;
/// ```
pub async fn retry_with_backoff<F, Fut, T>(
    mut operation: F,
    config: RetryConfig,
    operation_name: &str,
) -> Result<T, MailflowError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, MailflowError>>,
{
    let mut attempt = 0;

    loop {
        match operation().await {
            Ok(result) => {
                if attempt > 0 {
                    debug!(
                        operation = operation_name,
                        attempt = attempt,
                        "Operation succeeded after retry"
                    );
                }
                return Ok(result);
            }
            Err(e) => {
                // Check if error is retriable
                if !e.is_retriable() {
                    warn!(
                        operation = operation_name,
                        error = %e,
                        "Permanent error, not retrying"
                    );
                    return Err(e);
                }

                // Check if we've exhausted retries
                if attempt >= config.max_retries {
                    warn!(
                        operation = operation_name,
                        attempt = attempt,
                        max_retries = config.max_retries,
                        error = %e,
                        "Max retries exhausted"
                    );
                    return Err(MailflowError::Lambda(format!(
                        "Operation '{}' failed after {} retries: {}",
                        operation_name, attempt, e
                    )));
                }

                // Calculate delay and sleep
                let delay = config.calculate_delay(attempt);
                warn!(
                    operation = operation_name,
                    attempt = attempt,
                    delay_ms = delay.as_millis(),
                    error = %e,
                    "Retriable error, will retry after delay"
                );

                tokio::time::sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

/// Retry with default configuration
pub async fn retry_default<F, Fut, T>(
    operation: F,
    operation_name: &str,
) -> Result<T, MailflowError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, MailflowError>>,
{
    retry_with_backoff(operation, RetryConfig::default(), operation_name).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_retry_config_delay_calculation() {
        let config = RetryConfig::new(5, 1000, 60000);

        // First retry (attempt 0): 1000ms * 2^0 = 1000ms
        let delay0 = config.calculate_delay(0);
        assert!(delay0.as_millis() >= 900 && delay0.as_millis() <= 1100); // Within jitter range

        // Second retry (attempt 1): 1000ms * 2^1 = 2000ms
        let delay1 = config.calculate_delay(1);
        assert!(delay1.as_millis() >= 1800 && delay1.as_millis() <= 2200);

        // High attempt should be capped at max_delay
        let delay_high = config.calculate_delay(10);
        assert!(delay_high.as_millis() <= 66000); // max_delay + jitter
    }

    #[tokio::test]
    async fn test_retry_success_first_attempt() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_with_backoff(
            || {
                let c = counter_clone.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok::<i32, MailflowError>(42)
                }
            },
            RetryConfig::new(3, 10, 1000),
            "test_op",
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 1); // Only called once
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_with_backoff(
            || {
                let c = counter_clone.clone();
                async move {
                    let count = c.fetch_add(1, Ordering::SeqCst);
                    if count < 2 {
                        // Fail first 2 attempts
                        Err(MailflowError::Storage("Retriable error".to_string()))
                    } else {
                        Ok::<i32, MailflowError>(42)
                    }
                }
            },
            RetryConfig::new(5, 10, 1000),
            "test_op",
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 3); // Called 3 times
    }

    #[tokio::test]
    async fn test_retry_permanent_error_no_retry() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_with_backoff(
            || {
                let c = counter_clone.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err::<i32, MailflowError>(MailflowError::Validation(
                        "Permanent error".to_string(),
                    ))
                }
            },
            RetryConfig::new(5, 10, 1000),
            "test_op",
        )
        .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 1); // Only called once, no retries
    }

    #[tokio::test]
    async fn test_retry_max_retries_exhausted() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_with_backoff(
            || {
                let c = counter_clone.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err::<i32, MailflowError>(MailflowError::Storage("Retriable".to_string()))
                }
            },
            RetryConfig::new(3, 10, 1000),
            "test_op",
        )
        .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 4); // Initial + 3 retries
    }
}
