/// Outbound email handler - processes SQS events
use crate::email::composer::{EmailComposer, LettreEmailComposer};
use crate::error::MailflowError;
use crate::handlers::common::send_error_to_dlq;
use crate::models::{OutboundMessage, SqsEvent};
use crate::services::idempotency::{DynamoDbIdempotencyService, IdempotencyService};
use crate::services::metrics::{CloudWatchMetricsService, MetricsService};
use crate::services::ses::{EmailSender, SesEmailSender};
use crate::services::sqs::{QueueService, SqsQueueService};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

/// Outbound handler context
pub struct OutboundContext {
    queue: Arc<dyn QueueService>,
    ses: Arc<dyn EmailSender>,
    composer: Arc<dyn EmailComposer>,
    idempotency: Arc<dyn IdempotencyService>,
    metrics: Arc<dyn MetricsService>,
    outbound_queue_url: String,
}

impl OutboundContext {
    pub async fn new() -> Result<Self, MailflowError> {
        let aws_config = aws_config::load_from_env().await;

        let sqs_client = aws_sdk_sqs::Client::new(&aws_config);
        let ses_client = aws_sdk_ses::Client::new(&aws_config);
        let s3_client = aws_sdk_s3::Client::new(&aws_config);
        let dynamodb_client = aws_sdk_dynamodb::Client::new(&aws_config);
        let cloudwatch_client = aws_sdk_cloudwatch::Client::new(&aws_config);

        let outbound_queue_url = std::env::var("OUTBOUND_QUEUE_URL")
            .map_err(|_| MailflowError::Config("Missing OUTBOUND_QUEUE_URL".to_string()))?;

        let idempotency_table = std::env::var("IDEMPOTENCY_TABLE")
            .map_err(|_| MailflowError::Config("Missing IDEMPOTENCY_TABLE".to_string()))?;

        Ok(Self {
            queue: Arc::new(SqsQueueService::new(sqs_client)),
            ses: Arc::new(SesEmailSender::new(ses_client)),
            composer: Arc::new(LettreEmailComposer::new(s3_client)),
            idempotency: Arc::new(DynamoDbIdempotencyService::new(
                dynamodb_client,
                idempotency_table,
            )),
            metrics: Arc::new(CloudWatchMetricsService::new(cloudwatch_client)),
            outbound_queue_url,
        })
    }
}

pub async fn handle(event: SqsEvent) -> Result<(), MailflowError> {
    info!("Processing {} SQS record(s)", event.records.len());

    let ctx = OutboundContext::new().await?;
    let dlq_url = std::env::var("DLQ_URL").ok();

    for record in event.records {
        if let Err(e) = process_record(&ctx, record.clone()).await {
            error!("Failed to process outbound record: {}", e);

            // Send error to DLQ using common handler
            send_error_to_dlq(
                ctx.queue.as_ref(),
                Some(ctx.metrics.as_ref()),
                dlq_url.as_deref(),
                &e,
                "outbound",
                serde_json::json!({
                    "message_id": record.message_id,
                    "original_message": record.body,
                }),
            )
            .await;

            // Delete from queue even if failed (already in DLQ or will retry via SQS)
            if let Err(delete_err) = ctx
                .queue
                .delete_message(&ctx.outbound_queue_url, &record.receipt_handle)
                .await
            {
                error!(
                    receipt_handle = %record.receipt_handle,
                    error = %delete_err,
                    "Failed to delete message from outbound queue after processing error. Message may be reprocessed."
                );
                // Note: This is logged but not fatal. Idempotency should prevent duplicate sends if reprocessed.
            }
        }
    }

    Ok(())
}

#[tracing::instrument(
    name = "outbound.process_record",
    skip(ctx, record),
    fields(
        message_id = %record.message_id
    )
)]
async fn process_record(
    ctx: &OutboundContext,
    record: crate::models::SqsRecord,
) -> Result<(), MailflowError> {
    let start_time = Instant::now();
    let message_id = &record.message_id;
    info!("Processing outbound message: {}", message_id);

    // 1. Parse and validate message schema
    let outbound_message: OutboundMessage = serde_json::from_str(&record.body)
        .map_err(|e| MailflowError::Validation(format!("Invalid outbound message JSON: {}", e)))?;

    validate_outbound_message(&outbound_message)?;

    // 2. Check idempotency
    if ctx
        .idempotency
        .is_duplicate(&outbound_message.correlation_id)
        .await?
    {
        info!(
            "Message already processed (correlation_id: {}), skipping",
            outbound_message.correlation_id
        );
        // Delete from queue and skip
        ctx.queue
            .delete_message(&ctx.outbound_queue_url, &record.receipt_handle)
            .await?;
        return Ok(());
    }

    // 3. Verify sender identity
    if !ctx
        .ses
        .verify_sender_identity(&outbound_message.email.from.address)
        .await?
    {
        return Err(MailflowError::Validation(format!(
            "Sender address '{}' is not verified in SES. Please verify the email address or domain before sending.",
            outbound_message.email.from.address
        )));
    }

    // 4. Check SES quota
    let quota = ctx.ses.get_send_quota().await?;
    if quota.sent_last_24_hours >= quota.max_24_hour_send {
        warn!("SES daily quota exceeded, returning message to queue");
        return Err(MailflowError::Ses(
            "Daily sending quota exceeded".to_string(),
        ));
    }

    // 5. Compose email
    let raw_email = ctx.composer.compose(&outbound_message.email).await?;

    // 6. Send via SES
    let recipients: Vec<String> = outbound_message
        .email
        .to
        .iter()
        .chain(outbound_message.email.cc.iter())
        .chain(outbound_message.email.bcc.iter())
        .map(|addr| addr.address.clone())
        .collect();

    let ses_message_id = ctx
        .ses
        .send_raw_email(
            &raw_email,
            &outbound_message.email.from.address,
            &recipients,
        )
        .await?;

    info!(
        "Sent email via SES: {} (correlation_id: {})",
        ses_message_id, outbound_message.correlation_id
    );

    // 7. Emit metrics
    let duration_ms = start_time.elapsed().as_millis() as f64;
    ctx.metrics
        .record_counter("OutboundEmailsSent", 1.0, &[])
        .await;
    ctx.metrics
        .record_gauge(
            "OutboundProcessingTime",
            duration_ms,
            crate::services::metrics::MetricUnit::Milliseconds,
            &[],
        )
        .await;

    // Note: Attachment size metrics would require tracking during S3 fetch in composer

    // 8. Record idempotency
    ctx.idempotency
        .record(&outbound_message.correlation_id, Duration::from_secs(86400))
        .await?;

    // 9. Delete from queue
    ctx.queue
        .delete_message(&ctx.outbound_queue_url, &record.receipt_handle)
        .await?;

    Ok(())
}

fn validate_outbound_message(message: &OutboundMessage) -> Result<(), MailflowError> {
    // Validate required fields
    if message.email.to.is_empty() {
        return Err(MailflowError::Validation(
            "At least one recipient required".to_string(),
        ));
    }

    if message.email.from.address.is_empty() {
        return Err(MailflowError::Validation(
            "From address required".to_string(),
        ));
    }

    if message.email.subject.is_empty() {
        return Err(MailflowError::Validation("Subject required".to_string()));
    }

    if message.email.body.text.is_none() && message.email.body.html.is_none() {
        return Err(MailflowError::Validation(
            "Email must have text or HTML body".to_string(),
        ));
    }

    // Validate email addresses
    crate::utils::validate_email_address(&message.email.from.address)?;

    for to in &message.email.to {
        crate::utils::validate_email_address(&to.address)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{EmailAddress, EmailBody, EmailHeaders, OutboundEmail, SendOptions};
    use chrono::Utc;

    #[test]
    fn test_validate_outbound_message() {
        let valid_message = OutboundMessage {
            version: "1.0".to_string(),
            correlation_id: "test-123".to_string(),
            timestamp: Utc::now(),
            source: "app1".to_string(),
            email: OutboundEmail {
                from: EmailAddress {
                    address: "sender@example.com".to_string(),
                    name: None,
                },
                to: vec![EmailAddress {
                    address: "recipient@example.com".to_string(),
                    name: None,
                }],
                cc: vec![],
                bcc: vec![],
                reply_to: None,
                subject: "Test".to_string(),
                body: EmailBody {
                    text: Some("Body".to_string()),
                    html: None,
                },
                attachments: vec![],
                headers: EmailHeaders::default(),
            },
            options: SendOptions::default(),
        };

        assert!(validate_outbound_message(&valid_message).is_ok());

        // Test missing recipient
        let mut invalid = valid_message.clone();
        invalid.email.to = vec![];
        assert!(validate_outbound_message(&invalid).is_err());

        // Test missing subject
        let mut invalid = valid_message.clone();
        invalid.email.subject = String::new();
        assert!(validate_outbound_message(&invalid).is_err());

        // Test missing body
        let mut invalid = valid_message;
        invalid.email.body = EmailBody {
            text: None,
            html: None,
        };
        assert!(validate_outbound_message(&invalid).is_err());
    }
}
