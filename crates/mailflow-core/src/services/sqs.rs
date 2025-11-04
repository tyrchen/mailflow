/// SQS queue service
use crate::error::MailflowError;
use crate::models::SqsRecord;
use crate::utils::retry::{RetryConfig, retry_with_backoff};
use async_trait::async_trait;

#[async_trait]
pub trait QueueService: Send + Sync {
    async fn send_message(&self, queue_url: &str, message: &str) -> Result<String, MailflowError>;
    async fn send_batch(
        &self,
        queue_url: &str,
        messages: &[String],
    ) -> Result<Vec<String>, MailflowError>;
    async fn receive_messages(
        &self,
        queue_url: &str,
        max_messages: i32,
    ) -> Result<Vec<SqsRecord>, MailflowError>;
    async fn delete_message(
        &self,
        queue_url: &str,
        receipt_handle: &str,
    ) -> Result<(), MailflowError>;
    async fn queue_exists(&self, queue_url: &str) -> Result<bool, MailflowError>;
}

pub struct SqsQueueService {
    client: aws_sdk_sqs::Client,
}

impl SqsQueueService {
    pub fn new(client: aws_sdk_sqs::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl QueueService for SqsQueueService {
    async fn send_message(&self, queue_url: &str, message: &str) -> Result<String, MailflowError> {
        let queue_url_owned = queue_url.to_string();
        let message_owned = message.to_string();

        let response = retry_with_backoff(
            || {
                let client = self.client.clone();
                let queue = queue_url_owned.clone();
                let msg = message_owned.clone();

                async move {
                    client
                        .send_message()
                        .queue_url(queue)
                        .message_body(msg)
                        .send()
                        .await
                        .map_err(|e| {
                            MailflowError::Queue(format!("SQS send_message failed: {}", e))
                        })
                }
            },
            RetryConfig::default(),
            "sqs_send_message",
        )
        .await?;

        let message_id = response
            .message_id()
            .ok_or_else(|| MailflowError::Queue("No message ID returned".to_string()))?
            .to_string();

        tracing::info!("Sent message to queue: {} (id: {})", queue_url, message_id);
        Ok(message_id)
    }

    async fn send_batch(
        &self,
        queue_url: &str,
        messages: &[String],
    ) -> Result<Vec<String>, MailflowError> {
        use aws_sdk_sqs::types::SendMessageBatchRequestEntry;

        let entries: Vec<SendMessageBatchRequestEntry> = messages
            .iter()
            .enumerate()
            .map(|(i, msg)| {
                SendMessageBatchRequestEntry::builder()
                    .id(format!("msg-{}", i))
                    .message_body(msg)
                    .build()
                    .expect("Failed to build batch entry")
            })
            .collect();

        let response = self
            .client
            .send_message_batch()
            .queue_url(queue_url)
            .set_entries(Some(entries))
            .send()
            .await
            .map_err(|e| MailflowError::Queue(format!("SQS send_batch failed: {}", e)))?;

        let message_ids: Vec<String> = response
            .successful()
            .iter()
            .map(|r| r.message_id.clone())
            .collect();

        tracing::info!(
            "Sent {} messages to queue: {} ({} successful)",
            messages.len(),
            queue_url,
            message_ids.len()
        );

        Ok(message_ids)
    }

    async fn receive_messages(
        &self,
        queue_url: &str,
        max_messages: i32,
    ) -> Result<Vec<SqsRecord>, MailflowError> {
        let response = self
            .client
            .receive_message()
            .queue_url(queue_url)
            .max_number_of_messages(max_messages)
            .wait_time_seconds(20) // Long polling
            .send()
            .await
            .map_err(|e| MailflowError::Queue(format!("SQS receive_messages failed: {}", e)))?;

        let records: Vec<SqsRecord> = response
            .messages()
            .iter()
            .map(|msg| SqsRecord {
                message_id: msg.message_id().unwrap_or("unknown").to_string(),
                receipt_handle: msg.receipt_handle().unwrap_or("").to_string(),
                body: msg.body().unwrap_or("").to_string(),
                attributes: msg
                    .attributes()
                    .iter()
                    .flat_map(|m| m.iter().map(|(k, v)| (k.as_str().to_string(), v.clone())))
                    .collect(),
                message_attributes: Default::default(),
            })
            .collect();

        tracing::info!(
            "Received {} messages from queue: {}",
            records.len(),
            queue_url
        );
        Ok(records)
    }

    async fn delete_message(
        &self,
        queue_url: &str,
        receipt_handle: &str,
    ) -> Result<(), MailflowError> {
        let queue_url_owned = queue_url.to_string();
        let receipt_handle_owned = receipt_handle.to_string();

        retry_with_backoff(
            || {
                let client = self.client.clone();
                let queue = queue_url_owned.clone();
                let handle = receipt_handle_owned.clone();

                async move {
                    client
                        .delete_message()
                        .queue_url(queue)
                        .receipt_handle(handle)
                        .send()
                        .await
                        .map_err(|e| {
                            MailflowError::Queue(format!("SQS delete_message failed: {}", e))
                        })
                }
            },
            RetryConfig::default(),
            "sqs_delete_message",
        )
        .await?;

        tracing::debug!("Deleted message from queue: {}", queue_url);
        Ok(())
    }

    async fn queue_exists(&self, queue_url: &str) -> Result<bool, MailflowError> {
        let queue_url_owned = queue_url.to_string();

        let result = retry_with_backoff(
            || {
                let client = self.client.clone();
                let queue = queue_url_owned.clone();

                async move {
                    client
                        .get_queue_attributes()
                        .queue_url(queue)
                        .send()
                        .await
                        .map_err(|e| {
                            let error_str = e.to_string();
                            if error_str.contains("NonExistentQueue")
                                || error_str.contains("AWS.SimpleQueueService.NonExistentQueue")
                            {
                                // Queue doesn't exist - return Ok with false indicator
                                MailflowError::Queue("NonExistentQueue".to_string())
                            } else {
                                MailflowError::Queue(format!(
                                    "SQS get_queue_attributes failed: {}",
                                    e
                                ))
                            }
                        })
                }
            },
            RetryConfig::default(),
            "sqs_queue_exists",
        )
        .await;

        match result {
            Ok(_) => {
                tracing::debug!("Queue exists: {}", queue_url);
                Ok(true)
            }
            Err(e) if e.to_string().contains("NonExistentQueue") => {
                tracing::warn!("Queue does not exist: {}", queue_url);
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }
}
