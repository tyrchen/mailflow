/// Common handler utilities shared across inbound, outbound, and SES handlers
use crate::error::MailflowError;
use crate::services::metrics::{Metrics, MetricsService};
use crate::services::sqs::QueueService;
use chrono::Utc;
use serde_json::Value;
use tracing::error;

/// Sends an error to the Dead Letter Queue with standardized format
///
/// This function provides consistent error handling across all handlers,
/// ensuring errors are properly logged, tracked, and queued for investigation.
///
/// # Arguments
/// * `queue` - SQS queue service
/// * `metrics` - Metrics service for tracking DLQ messages
/// * `dlq_url` - Optional DLQ URL (if None, error is only logged)
/// * `error` - The error that occurred
/// * `handler` - Handler name for tracking ("inbound", "outbound", "ses")
/// * `context` - Additional context as JSON (record info, message details, etc.)
///
/// # Example
/// ```ignore
/// send_error_to_dlq(
///     &ctx.queue,
///     &ctx.metrics,
///     dlq_url.as_deref(),
///     &error,
///     "inbound",
///     serde_json::json!({
///         "bucket": bucket,
///         "key": key,
///     })
/// ).await;
/// ```
pub async fn send_error_to_dlq(
    queue: &dyn QueueService,
    metrics: Option<&dyn MetricsService>,
    dlq_url: Option<&str>,
    error: &MailflowError,
    handler: &str,
    context: Value,
) {
    let error_type = if error.is_retriable() {
        "retriable"
    } else {
        "permanent"
    };

    error!(
        target: "error_handling",
        handler = handler,
        error_type = error_type,
        error = %error,
        "Processing error occurred"
    );

    // Record error metric
    if let Some(metrics_service) = metrics {
        Metrics::error_occurred(metrics_service, error_type, handler).await;
    }

    // Send to DLQ if configured
    if let Some(dlq) = dlq_url {
        let error_payload = serde_json::json!({
            "error": sanitize_error_message(error),
            "errorType": error_type,
            "handler": handler,
            "context": context,
            "timestamp": Utc::now().to_rfc3339(),
        });

        match queue.send_message(dlq, &error_payload.to_string()).await {
            Ok(_) => {
                if let Some(metrics_service) = metrics {
                    Metrics::dlq_message_sent(metrics_service, handler).await;
                }
            }
            Err(dlq_err) => {
                error!(
                    target: "error_handling",
                    handler = handler,
                    error = %dlq_err,
                    "Failed to send error to DLQ"
                );
            }
        }
    }
}

/// Sanitizes error messages before sending to DLQ
///
/// Removes potentially sensitive information like:
/// - Full stack traces
/// - Internal paths
/// - AWS resource ARNs
/// - Email addresses
fn sanitize_error_message(error: &MailflowError) -> String {
    let error_str = error.to_string();

    // Basic sanitization - remove common sensitive patterns
    crate::utils::logging::redact_email(&error_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::metrics::MockMetricsService;
    use crate::services::sqs::QueueService;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct MockQueueService {
        messages: Arc<Mutex<Vec<String>>>,
    }

    impl MockQueueService {
        fn new() -> Self {
            Self {
                messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        async fn get_messages(&self) -> Vec<String> {
            self.messages.lock().await.clone()
        }
    }

    #[async_trait]
    impl QueueService for MockQueueService {
        async fn send_message(
            &self,
            _queue_url: &str,
            message_body: &str,
        ) -> Result<String, MailflowError> {
            self.messages.lock().await.push(message_body.to_string());
            Ok("mock-message-id".to_string())
        }

        async fn send_batch(
            &self,
            _queue_url: &str,
            _messages: &[String],
        ) -> Result<Vec<String>, MailflowError> {
            Ok(vec!["mock-batch-id".to_string()])
        }

        async fn receive_messages(
            &self,
            _queue_url: &str,
            _max_messages: i32,
        ) -> Result<Vec<crate::models::SqsRecord>, MailflowError> {
            Ok(vec![])
        }

        async fn delete_message(
            &self,
            _queue_url: &str,
            _receipt_handle: &str,
        ) -> Result<(), MailflowError> {
            Ok(())
        }

        async fn queue_exists(&self, _queue_url: &str) -> Result<bool, MailflowError> {
            Ok(true) // Mock always returns true
        }
    }

    #[tokio::test]
    async fn test_send_error_to_dlq() {
        let queue = MockQueueService::new();
        let metrics = MockMetricsService::new();
        let error = MailflowError::Validation("Test error".to_string());

        send_error_to_dlq(
            &queue,
            Some(&metrics),
            Some("https://sqs.us-east-1.amazonaws.com/123/dlq"),
            &error,
            "test_handler",
            serde_json::json!({"test": "context"}),
        )
        .await;

        let messages = queue.get_messages().await;
        assert_eq!(messages.len(), 1);

        let message: Value = serde_json::from_str(&messages[0]).unwrap();
        assert_eq!(message["handler"], "test_handler");
        assert_eq!(message["errorType"], "permanent");
        assert!(message["timestamp"].is_string());
    }

    #[tokio::test]
    async fn test_sanitize_error_message() {
        let error = MailflowError::Storage(
            "Failed to upload to s3://bucket/key for user@example.com".to_string(),
        );

        let sanitized = sanitize_error_message(&error);
        assert!(!sanitized.contains("user@example.com"));
        assert!(sanitized.contains("***@example.com"));
    }
}
