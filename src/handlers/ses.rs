use crate::error::MailflowError;
use crate::handlers::common::send_error_to_dlq;
use crate::handlers::inbound::InboundContext;
use crate::models::{SesEvent, SesEventRecord};
use crate::services::attachments::{AttachmentConfig, AttachmentProcessor, S3AttachmentProcessor};
use crate::services::metrics::{CloudWatchMetricsService, Metrics};
use crate::services::security::SecurityValidator;
use crate::utils::logging::{redact_email, redact_subject};
use crate::utils::retry::retry_default;
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info};

pub async fn handle(event: SesEvent) -> Result<(), MailflowError> {
    info!("Processing {} SES record(s)", event.records.len());

    let ctx = InboundContext::new().await?;
    let dlq_url = std::env::var("DLQ_URL").ok();

    // Initialize metrics service
    let aws_config = aws_config::load_from_env().await;
    let cw_client = aws_sdk_cloudwatch::Client::new(&aws_config);
    let metrics = CloudWatchMetricsService::new(cw_client);

    // Initialize security validator
    let config = ctx.config.get_config().await?;
    let security_validator = SecurityValidator::new(config.security.clone());

    for record in event.records {
        let start = Instant::now();

        if let Err(e) = process_ses_record(&ctx, &security_validator, &record).await {
            error!("Failed to process SES record: {}", e);

            // Send to DLQ with metrics
            send_error_to_dlq(
                &*ctx.queue,
                Some(&metrics),
                dlq_url.as_deref(),
                &e,
                "ses",
                serde_json::json!({
                    "message_id": record.ses.mail.message_id,
                    "recipients": record.ses.receipt.recipients,
                }),
            )
            .await;
        } else {
            // Record success metrics
            let duration_ms = start.elapsed().as_millis() as f64;
            for recipient in &record.ses.receipt.recipients {
                if let Some(app_name) = extract_app_name(recipient) {
                    Metrics::inbound_email_processed(&metrics, &app_name, duration_ms).await;
                }
            }
        }
    }

    Ok(())
}

fn extract_app_name(email: &str) -> Option<String> {
    email
        .split('@')
        .next()
        .and_then(|local| local.strip_prefix('_'))
        .map(|s| s.to_string())
}

async fn process_ses_record(
    ctx: &InboundContext,
    security_validator: &SecurityValidator,
    record: &SesEventRecord,
) -> Result<(), MailflowError> {
    // Validate security verdicts
    security_validator.validate_ses_verdicts(record)?;
    // Extract S3 location - either from action or use default bucket + message ID
    let bucket = if let Some(ref bucket_name) = record.ses.receipt.action.bucket_name {
        bucket_name.clone()
    } else {
        // Fall back to environment variable if not in action
        std::env::var("RAW_EMAILS_BUCKET").map_err(|_| {
            MailflowError::Lambda(
                "RAW_EMAILS_BUCKET not set and S3 bucket not found in SES action".to_string(),
            )
        })?
    };

    let key = if let Some(ref object_key) = record.ses.receipt.action.object_key {
        object_key.clone()
    } else {
        // SES stores the email with the message ID as the key
        record.ses.mail.message_id.clone()
    };

    info!(
        "Processing SES email - message_id: {}, s3://{}/{}",
        record.ses.mail.message_id, bucket, key
    );

    // Download raw email from S3 with retry
    let bucket_clone = bucket.clone();
    let key_clone = key.clone();
    let storage_clone = Arc::clone(&ctx.storage);
    let raw_email = retry_default(
        || {
            let b = bucket_clone.clone();
            let k = key_clone.clone();
            let s = Arc::clone(&storage_clone);
            async move { s.download(&b, &k).await }
        },
        "s3_download",
    )
    .await?;

    // Validate email size
    security_validator.validate_email_size(raw_email.len())?;

    // Parse email
    let mut email = ctx.parser.parse(&raw_email).await?;
    info!(
        "Parsed email - from: {}, subject: {}, attachments: {}, size: {} bytes",
        redact_email(&email.from.address),
        redact_subject(&email.subject),
        email.attachments_data.len(),
        raw_email.len()
    );

    // Validate sender domain
    security_validator.validate_sender_domain(&email.from.address)?;
    info!(
        "Sender domain validated for: {}",
        redact_email(&email.from.address)
    );

    // Process attachments if any
    if !email.attachments_data.is_empty() {
        let attachment_config = AttachmentConfig::from_env()?;
        let processor = S3AttachmentProcessor::new(Arc::clone(&ctx.storage), attachment_config);

        email.attachments = processor
            .process_attachments(&email.message_id, email.attachments_data.clone())
            .await?;

        info!(
            "Processed {} attachment(s) for message {}",
            email.attachments.len(),
            email.message_id
        );
    }

    // Determine routing
    let routes = ctx.router.route(&email).await?;
    info!("Determined {} route(s)", routes.len());

    // Extract security metadata from SES
    let spf_verified = record
        .ses
        .receipt
        .spf_verdict
        .as_ref()
        .map(|v| v.status == "PASS")
        .unwrap_or(false);

    let dkim_verified = record
        .ses
        .receipt
        .dkim_verdict
        .as_ref()
        .map(|v| v.status == "PASS")
        .unwrap_or(false);

    // For each route, construct and send message
    for route in routes {
        let mut inbound_message =
            crate::handlers::inbound::build_inbound_message(&email, &route.app_name)?;

        // Update metadata with SES security info
        inbound_message.metadata.spf_verified = spf_verified;
        inbound_message.metadata.dkim_verified = dkim_verified;

        let message_json = serde_json::to_string(&inbound_message)
            .map_err(|e| MailflowError::Queue(format!("Failed to serialize message: {}", e)))?;

        let message_id = ctx
            .queue
            .send_message(&route.queue_url, &message_json)
            .await?;

        info!(
            "Sent message to queue {} (app: {}, message_id: {})",
            route.queue_url, route.app_name, message_id
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ses_event_parsing() {
        let json = r#"{
            "Records": [{
                "eventSource": "aws:ses",
                "eventVersion": "1.0",
                "ses": {
                    "mail": {
                        "messageId": "test-123",
                        "timestamp": "2025-11-01T12:00:00.000Z",
                        "source": "sender@example.com",
                        "destination": ["_app1@acme.com"]
                    },
                    "receipt": {
                        "timestamp": "2025-11-01T12:00:00.000Z",
                        "recipients": ["_app1@acme.com"],
                        "spfVerdict": {"status": "PASS"},
                        "dkimVerdict": {"status": "PASS"},
                        "spamVerdict": {"status": "PASS"},
                        "virusVerdict": {"status": "PASS"},
                        "action": {
                            "type": "Lambda",
                            "bucketName": "test-bucket",
                            "objectKey": "test-key"
                        }
                    }
                }
            }]
        }"#;

        let event: SesEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.records.len(), 1);
        assert_eq!(event.records[0].ses.mail.message_id, "test-123");
    }
}
