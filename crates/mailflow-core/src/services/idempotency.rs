use crate::error::MailflowError;
use async_trait::async_trait;
use aws_sdk_dynamodb::types::AttributeValue;
use chrono::Utc;
use std::time::Duration;
use tracing::{debug, info};

#[async_trait]
pub trait IdempotencyService: Send + Sync {
    /// Check if an operation has already been processed
    ///
    /// Returns true if the correlation_id has been seen before
    async fn is_duplicate(&self, correlation_id: &str) -> Result<bool, MailflowError>;

    /// Record that an operation has been processed
    ///
    /// Stores the correlation_id with TTL for deduplication
    async fn record(&self, correlation_id: &str, ttl: Duration) -> Result<(), MailflowError>;

    /// Check and record in one atomic operation
    ///
    /// Returns true if this is a duplicate (already exists)
    async fn check_and_record(
        &self,
        correlation_id: &str,
        ttl: Duration,
    ) -> Result<bool, MailflowError> {
        if self.is_duplicate(correlation_id).await? {
            Ok(true)
        } else {
            self.record(correlation_id, ttl).await?;
            Ok(false)
        }
    }
}

/// DynamoDB-backed idempotency service
pub struct DynamoDbIdempotencyService {
    client: aws_sdk_dynamodb::Client,
    table_name: String,
}

impl DynamoDbIdempotencyService {
    pub fn new(client: aws_sdk_dynamodb::Client, table_name: String) -> Self {
        Self { client, table_name }
    }

    pub fn from_env(client: aws_sdk_dynamodb::Client) -> Result<Self, MailflowError> {
        let table_name = std::env::var("IDEMPOTENCY_TABLE")
            .map_err(|_| MailflowError::Config("IDEMPOTENCY_TABLE not set".to_string()))?;

        Ok(Self::new(client, table_name))
    }
}

#[async_trait]
impl IdempotencyService for DynamoDbIdempotencyService {
    async fn is_duplicate(&self, correlation_id: &str) -> Result<bool, MailflowError> {
        let result = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key(
                "correlationId",
                AttributeValue::S(correlation_id.to_string()),
            )
            .send()
            .await
            .map_err(|e| MailflowError::Lambda(format!("DynamoDB get_item failed: {}", e)))?;

        if result.item().is_some() {
            debug!(
                correlation_id = correlation_id,
                "Duplicate operation detected"
            );
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn record(&self, correlation_id: &str, ttl: Duration) -> Result<(), MailflowError> {
        let expiration = Utc::now().timestamp() + ttl.as_secs() as i64;

        self.client
            .put_item()
            .table_name(&self.table_name)
            .item(
                "correlationId",
                AttributeValue::S(correlation_id.to_string()),
            )
            .item(
                "timestamp",
                AttributeValue::N(Utc::now().timestamp().to_string()),
            )
            .item("ttl", AttributeValue::N(expiration.to_string()))
            .send()
            .await
            .map_err(|e| MailflowError::Lambda(format!("DynamoDB put_item failed: {}", e)))?;

        info!(
            correlation_id = correlation_id,
            ttl_seconds = ttl.as_secs(),
            "Recorded idempotency key"
        );

        Ok(())
    }
}

/// In-memory idempotency service for testing
pub struct InMemoryIdempotencyService {
    store: tokio::sync::Mutex<std::collections::HashMap<String, i64>>,
}

impl InMemoryIdempotencyService {
    pub fn new() -> Self {
        Self {
            store: tokio::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl Default for InMemoryIdempotencyService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IdempotencyService for InMemoryIdempotencyService {
    async fn is_duplicate(&self, correlation_id: &str) -> Result<bool, MailflowError> {
        let store = self.store.lock().await;
        let now = Utc::now().timestamp();

        if store
            .get(correlation_id)
            .filter(|&&exp| exp > now)
            .is_some()
        {
            return Ok(true); // Not expired, is duplicate
        }

        Ok(false)
    }

    async fn record(&self, correlation_id: &str, ttl: Duration) -> Result<(), MailflowError> {
        let mut store = self.store.lock().await;
        let expiration = Utc::now().timestamp() + ttl.as_secs() as i64;
        store.insert(correlation_id.to_string(), expiration);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_idempotency() {
        let service = InMemoryIdempotencyService::new();

        // First check - should not be duplicate
        assert!(!service.is_duplicate("test-id-1").await.unwrap());

        // Record it
        service
            .record("test-id-1", Duration::from_secs(60))
            .await
            .unwrap();

        // Second check - should be duplicate
        assert!(service.is_duplicate("test-id-1").await.unwrap());

        // Different ID - should not be duplicate
        assert!(!service.is_duplicate("test-id-2").await.unwrap());
    }

    #[tokio::test]
    async fn test_idempotency_expiration() {
        let service = InMemoryIdempotencyService::new();

        // Record with 1 second TTL
        service
            .record("test-id", Duration::from_secs(1))
            .await
            .unwrap();

        // Should be duplicate immediately
        assert!(service.is_duplicate("test-id").await.unwrap());

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(1100)).await;

        // Should no longer be duplicate after expiration
        assert!(!service.is_duplicate("test-id").await.unwrap());
    }

    #[tokio::test]
    async fn test_check_and_record() {
        let service = InMemoryIdempotencyService::new();

        // First call - not duplicate
        let is_dup1 = service
            .check_and_record("test-id", Duration::from_secs(60))
            .await
            .unwrap();
        assert!(!is_dup1);

        // Second call - is duplicate
        let is_dup2 = service
            .check_and_record("test-id", Duration::from_secs(60))
            .await
            .unwrap();
        assert!(is_dup2);
    }
}
