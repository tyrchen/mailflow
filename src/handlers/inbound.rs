/// Inbound email handler - processes S3 events from SES
use crate::constants::{MAX_EMAIL_SIZE_BYTES, MESSAGE_ID_PREFIX, MESSAGE_VERSION, SOURCE_NAME};
use crate::email::parser::{EmailParser, MailParserEmailParser};
use crate::error::MailflowError;
use crate::handlers::common::send_error_to_dlq;
use crate::models::{InboundEmail, InboundMessage, MessageMetadata, S3Event};
use crate::routing::engine::{MailflowRouter, Router};
use crate::services::config::{ConfigProvider, EnvConfigProvider};
use crate::services::metrics::{CloudWatchMetricsService, MetricsService};
use crate::services::rate_limiter::{MockRateLimiter, RateLimiter};
use crate::services::s3::{S3StorageService, StorageService};
use crate::services::sqs::{QueueService, SqsQueueService};
use crate::utils::logging::{redact_email, redact_subject};
use chrono::Utc;
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info};

/// Inbound handler context
pub struct InboundContext {
    pub storage: Arc<dyn StorageService>,
    pub queue: Arc<dyn QueueService>,
    pub parser: Arc<dyn EmailParser>,
    pub router: Arc<dyn Router>,
    pub config: Arc<dyn ConfigProvider>,
    pub rate_limiter: Arc<dyn RateLimiter>,
    pub metrics: Arc<dyn MetricsService>,
}

impl InboundContext {
    pub async fn new() -> Result<Self, MailflowError> {
        let aws_config = aws_config::load_from_env().await;

        let s3_client = aws_sdk_s3::Client::new(&aws_config);
        let sqs_client = aws_sdk_sqs::Client::new(&aws_config);
        let cloudwatch_client = aws_sdk_cloudwatch::Client::new(&aws_config);

        let env_config = EnvConfigProvider::new()?;
        let config = env_config.get_config().await?;

        // Initialize rate limiter (use mock for now until DynamoDB table created)
        // TODO: Replace with DynamoDbRateLimiter when RATE_LIMITER_TABLE env var available
        let rate_limiter: Arc<dyn RateLimiter> = Arc::new(MockRateLimiter::allow_all());

        Ok(Self {
            storage: Arc::new(S3StorageService::new(s3_client)),
            queue: Arc::new(SqsQueueService::new(sqs_client)),
            parser: Arc::new(MailParserEmailParser::new()),
            router: Arc::new(MailflowRouter::new(config.clone())),
            config: Arc::new(env_config),
            rate_limiter,
            metrics: Arc::new(CloudWatchMetricsService::new(cloudwatch_client)),
        })
    }
}

pub async fn handle(event: S3Event) -> Result<(), MailflowError> {
    info!("Processing {} S3 record(s)", event.records.len());

    let ctx = InboundContext::new().await?;
    let dlq_url = std::env::var("DLQ_URL").ok();

    for record in event.records {
        if let Err(e) = process_record(&ctx, record.clone()).await {
            error!("Failed to process record: {}", e);

            // Send error to DLQ using common handler
            send_error_to_dlq(
                ctx.queue.as_ref(),
                Some(ctx.metrics.as_ref()),
                dlq_url.as_deref(),
                &e,
                "inbound",
                serde_json::json!({
                    "bucket": record.s3.bucket.name,
                    "key": record.s3.object.key,
                }),
            )
            .await;
        }
    }

    Ok(())
}

#[tracing::instrument(
    name = "inbound.process_record",
    skip(ctx, record),
    fields(
        bucket = %record.s3.bucket.name,
        key = %record.s3.object.key,
        size = record.s3.object.size.unwrap_or(0)
    )
)]
async fn process_record(
    ctx: &InboundContext,
    record: crate::models::S3EventRecord,
) -> Result<(), MailflowError> {
    let start_time = Instant::now();
    let bucket = &record.s3.bucket.name;
    let key = &record.s3.object.key;

    info!("Processing email from s3://{}/{}", bucket, key);

    // Emit metric for email received
    ctx.metrics
        .record_counter("InboundEmailsReceived", 1.0, &[])
        .await;

    // 1. Validate email size
    let size = record.s3.object.size.unwrap_or(0);
    if size > MAX_EMAIL_SIZE_BYTES as i64 {
        return Err(MailflowError::Validation(format!(
            "Email size {} bytes exceeds maximum {} bytes (40 MB)",
            size, MAX_EMAIL_SIZE_BYTES
        )));
    }

    // 2. Download raw email from S3
    let raw_email = ctx.storage.download(bucket, key).await?;

    // 3. Parse email
    let email = ctx.parser.parse(&raw_email).await?;
    info!(
        "Parsed email - from: {}, subject: {}, size: {} bytes",
        redact_email(&email.from.address),
        redact_subject(&email.subject),
        raw_email.len()
    );

    // 4. Check rate limit
    let config = ctx.config.get_config().await?;
    ctx.rate_limiter
        .check_rate_limit(
            &email.from.address,
            config.security.max_emails_per_sender_per_hour,
            3600, // 1 hour window
        )
        .await?;

    // 5. Determine routing
    let routes = ctx.router.route(&email).await?;
    info!("Determined {} route(s)", routes.len());

    // 6. For each route, validate queue exists and send message
    for route in routes {
        // Validate queue exists before sending
        if !ctx.queue.queue_exists(&route.queue_url).await? {
            return Err(MailflowError::Routing(format!(
                "Target queue for app '{}' does not exist: {}",
                route.app_name, route.queue_url
            )));
        }

        let inbound_message = build_inbound_message(&email, &route.app_name)?;
        let message_json = serde_json::to_string(&inbound_message)
            .map_err(|e| MailflowError::Queue(format!("Failed to serialize message: {}", e)))?;

        // Send to target queue
        let message_id = ctx
            .queue
            .send_message(&route.queue_url, &message_json)
            .await?;
        info!(
            "Sent message to queue {} (app: {}, message_id: {})",
            route.queue_url, route.app_name, message_id
        );

        // Emit routing metric
        ctx.metrics
            .record_counter("RoutingDecisions", 1.0, &[("app", route.app_name.as_str())])
            .await;
    }

    // 7. Emit processing metrics
    let duration_ms = start_time.elapsed().as_millis() as f64;
    ctx.metrics
        .record_counter("InboundEmailsProcessed", 1.0, &[])
        .await;
    ctx.metrics
        .record_gauge(
            "InboundProcessingTime",
            duration_ms,
            crate::services::metrics::MetricUnit::Milliseconds,
            &[],
        )
        .await;

    if !email.attachments.is_empty() {
        ctx.metrics
            .record_counter("AttachmentsProcessed", email.attachments.len() as f64, &[])
            .await;
    }

    // 8. Optional: Delete raw email from S3 (based on retention policy)
    // For now, rely on lifecycle policies

    Ok(())
}

pub fn build_inbound_message(
    email: &crate::models::Email,
    routing_key: &str,
) -> Result<InboundMessage, MailflowError> {
    let domain = email
        .to
        .first()
        .and_then(|addr| addr.address.split('@').nth(1))
        .unwrap_or("unknown")
        .to_string();

    Ok(InboundMessage {
        version: MESSAGE_VERSION.to_string(),
        message_id: format!("{}-{}", MESSAGE_ID_PREFIX, uuid::Uuid::new_v4()),
        timestamp: Utc::now(),
        source: SOURCE_NAME.to_string(),
        email: InboundEmail {
            message_id: email.message_id.clone(),
            from: email.from.clone(),
            to: email.to.clone(),
            cc: email.cc.clone(),
            reply_to: email.reply_to.clone(),
            subject: email.subject.clone(),
            body: email.body.clone(),
            attachments: email.attachments.clone(),
            headers: email.headers.clone(),
            received_at: email.received_at,
        },
        metadata: MessageMetadata {
            routing_key: routing_key.to_string(),
            domain,
            spam_score: 0.0,
            dkim_verified: false,
            spf_verified: false,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_inbound_message() {
        use crate::models::{EmailAddress, EmailBody, EmailHeaders};

        let email = crate::models::Email {
            message_id: "test-123".to_string(),
            from: EmailAddress {
                address: "sender@example.com".to_string(),
                name: Some("Sender".to_string()),
            },
            to: vec![EmailAddress {
                address: "_app1@acme.com".to_string(),
                name: None,
            }],
            cc: vec![],
            bcc: vec![],
            reply_to: None,
            subject: "Test Subject".to_string(),
            body: EmailBody {
                text: Some("Body".to_string()),
                html: None,
            },
            attachments: vec![],
            attachments_data: vec![],
            headers: EmailHeaders::default(),
            received_at: Utc::now(),
        };

        let result = build_inbound_message(&email, "app1");
        assert!(result.is_ok());

        let message = result.unwrap();
        assert_eq!(message.version, "1.0");
        assert_eq!(message.metadata.routing_key, "app1");
        assert_eq!(message.metadata.domain, "acme.com");
    }
}
