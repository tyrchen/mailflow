/// Rate limiting service using DynamoDB for distributed rate limiting
use crate::error::MailflowError;
use async_trait::async_trait;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use aws_sdk_dynamodb::types::AttributeValue;
use std::time::{SystemTime, UNIX_EPOCH};

#[async_trait]
pub trait RateLimiter: Send + Sync {
    /// Check if sender is within rate limit
    /// Returns Ok(()) if allowed, Err if rate limit exceeded
    async fn check_rate_limit(
        &self,
        sender: &str,
        limit: u32,
        window_seconds: u64,
    ) -> Result<(), MailflowError>;
}

pub struct DynamoDbRateLimiter {
    client: DynamoDbClient,
    table_name: String,
}

impl DynamoDbRateLimiter {
    pub fn new(client: DynamoDbClient, table_name: String) -> Self {
        Self { client, table_name }
    }

    async fn increment_counter(
        &self,
        key: &str,
        window_start: u64,
        ttl: u64,
    ) -> Result<u32, MailflowError> {
        // Use DynamoDB atomic counter
        let response = self
            .client
            .update_item()
            .table_name(&self.table_name)
            .key("sender", AttributeValue::S(key.to_string()))
            .key("window", AttributeValue::N(window_start.to_string()))
            .update_expression("ADD email_count :inc SET ttl = :ttl")
            .expression_attribute_values(":inc", AttributeValue::N("1".to_string()))
            .expression_attribute_values(":ttl", AttributeValue::N(ttl.to_string()))
            .return_values(aws_sdk_dynamodb::types::ReturnValue::AllNew)
            .send()
            .await
            .map_err(|e| MailflowError::RateLimit(format!("DynamoDB update_item failed: {}", e)))?;

        let count = response
            .attributes()
            .and_then(|attrs| attrs.get("email_count"))
            .and_then(|v| v.as_n().ok())
            .and_then(|n| n.parse::<u32>().ok())
            .unwrap_or(1);

        Ok(count)
    }
}

#[async_trait]
impl RateLimiter for DynamoDbRateLimiter {
    async fn check_rate_limit(
        &self,
        sender: &str,
        limit: u32,
        window_seconds: u64,
    ) -> Result<(), MailflowError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Calculate window start (sliding window)
        let window_start = (now / window_seconds) * window_seconds;
        let ttl = window_start + window_seconds + 3600; // TTL = window end + 1 hour buffer

        let count = self.increment_counter(sender, window_start, ttl).await?;

        if count > limit {
            tracing::warn!(
                sender = %sender,
                count = count,
                limit = limit,
                window_seconds = window_seconds,
                "Rate limit exceeded"
            );

            return Err(MailflowError::RateLimit(format!(
                "Sender {} exceeded rate limit: {} emails in {} seconds (limit: {})",
                sender, count, window_seconds, limit
            )));
        }

        tracing::debug!(
            sender = %sender,
            count = count,
            limit = limit,
            "Rate limit check passed"
        );

        Ok(())
    }
}

// Mock for testing
pub struct MockRateLimiter {
    allow: bool,
}

impl MockRateLimiter {
    pub fn new(allow: bool) -> Self {
        Self { allow }
    }

    pub fn allow_all() -> Self {
        Self { allow: true }
    }

    pub fn block_all() -> Self {
        Self { allow: false }
    }
}

#[async_trait]
impl RateLimiter for MockRateLimiter {
    async fn check_rate_limit(
        &self,
        _sender: &str,
        _limit: u32,
        _window_seconds: u64,
    ) -> Result<(), MailflowError> {
        if self.allow {
            Ok(())
        } else {
            Err(MailflowError::RateLimit(
                "Mock rate limit exceeded".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_rate_limiter_allows() {
        let limiter = MockRateLimiter::allow_all();
        assert!(
            limiter
                .check_rate_limit("test@example.com", 100, 3600)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn test_mock_rate_limiter_blocks() {
        let limiter = MockRateLimiter::block_all();
        assert!(
            limiter
                .check_rate_limit("test@example.com", 100, 3600)
                .await
                .is_err()
        );
    }
}
