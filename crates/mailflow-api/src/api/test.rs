/// Test email endpoints
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

use crate::{context::ApiContext, error::ApiError};

#[derive(Debug, Deserialize)]
pub struct TestInboundRequest {
    pub app: String,
    pub from: String,
    pub subject: String,
    pub body: EmailBody,
    #[serde(default)]
    pub attachments: Vec<AttachmentData>,
}

#[derive(Debug, Deserialize)]
pub struct TestOutboundRequest {
    pub from: String,
    pub to: String,
    pub subject: String,
    pub body: EmailBody,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EmailBody {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AttachmentData {
    pub filename: String,
    #[serde(rename = "contentType")]
    pub content_type: String,
    pub data: String, // base64 encoded
}

#[derive(Debug, Serialize)]
pub struct TestEmailResponse {
    pub success: bool,
    #[serde(rename = "messageId")]
    pub message_id: String,
    #[serde(rename = "sesMessageId", skip_serializing_if = "Option::is_none")]
    pub ses_message_id: Option<String>,
    #[serde(rename = "queueUrl", skip_serializing_if = "Option::is_none")]
    pub queue_url: Option<String>,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TestHistoryResponse {
    pub tests: Vec<TestHistoryEntry>,
}

#[derive(Debug, Serialize)]
pub struct TestHistoryEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub test_type: String,
    pub timestamp: String,
    pub recipient: String,
    pub status: String,
    #[serde(rename = "messageId")]
    pub message_id: String,
}

/// Send test inbound email
pub async fn inbound(
    State(ctx): State<Arc<ApiContext>>,
    Json(req): Json<TestInboundRequest>,
) -> Result<Json<TestEmailResponse>, ApiError> {
    info!("Sending test inbound email to app: {}", req.app);

    // Validate inputs
    if req.from.is_empty() || req.subject.is_empty() || req.body.text.is_empty() {
        return Err(ApiError::BadRequest(
            "from, subject, and body.text are required".to_string(),
        ));
    }

    // Get domain from environment
    let allowed_domains =
        std::env::var("ALLOWED_DOMAINS").unwrap_or_else(|_| "example.com".to_string());
    let domain = allowed_domains.split(',').next().unwrap_or("example.com");

    // Construct recipient address: _<app>@<domain>
    let to_address = format!("_{}@{}", req.app, domain);

    // Generate message ID
    let message_id = format!("test-{}", Uuid::new_v4());

    // Build email using lettre
    let email_builder = lettre::Message::builder()
        .from(
            req.from
                .parse()
                .map_err(|e| ApiError::BadRequest(format!("Invalid from address: {}", e)))?,
        )
        .to(to_address
            .parse()
            .map_err(|e| ApiError::BadRequest(format!("Invalid to address: {}", e)))?)
        .subject(&req.subject)
        .message_id(Some(message_id.clone()));

    // Build multipart email
    let email = if let Some(html) = &req.body.html {
        email_builder
            .multipart(
                lettre::message::MultiPart::alternative()
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(lettre::message::header::ContentType::TEXT_PLAIN)
                            .body(req.body.text.clone()),
                    )
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(lettre::message::header::ContentType::TEXT_HTML)
                            .body(html.clone()),
                    ),
            )
            .map_err(|e| ApiError::Internal(format!("Failed to build email: {}", e)))?
    } else {
        email_builder
            .body(req.body.text.clone())
            .map_err(|e| ApiError::Internal(format!("Failed to build email: {}", e)))?
    };

    // Convert to raw email bytes
    let email_bytes = email.formatted();

    // Send via SES
    let ses_result = ctx
        .ses_client
        .send_raw_email()
        .raw_message(
            aws_sdk_ses::types::RawMessage::builder()
                .data(aws_smithy_types::Blob::new(email_bytes))
                .build()
                .map_err(|e| ApiError::Internal(format!("Failed to build raw message: {}", e)))?,
        )
        .send()
        .await
        .map_err(|e| {
            error!("Failed to send test email via SES: {:?}", e);
            error!("From: {}, To: {}", req.from, to_address);
            ApiError::Aws(format!("SES send failed: {:?}. Check if email addresses are verified in SES (SES might be in sandbox mode)", e))
        })?;

    let ses_message_id = Some(ses_result.message_id().to_string());

    // Get queue URL for the app
    let queue_name = format!("mailflow-{}", req.app);
    let queue_url = get_queue_url_safe(&ctx, &queue_name).await;

    // Store test history in DynamoDB
    let _ = store_test_history(&ctx, "inbound", &to_address, &message_id, "success").await;

    info!(
        "Test inbound email sent successfully: {} to {}",
        message_id, to_address
    );

    Ok(Json(TestEmailResponse {
        success: true,
        message_id,
        ses_message_id,
        queue_url,
        timestamp: chrono::Utc::now().to_rfc3339(),
        error: None,
    }))
}

/// Send test outbound email
pub async fn outbound(
    State(ctx): State<Arc<ApiContext>>,
    Json(req): Json<TestOutboundRequest>,
) -> Result<Json<TestEmailResponse>, ApiError> {
    info!("Sending test outbound email from app: {}", req.from);

    // Validate inputs
    if req.from.is_empty()
        || req.to.is_empty()
        || req.subject.is_empty()
        || req.body.text.is_empty()
    {
        return Err(ApiError::BadRequest(
            "from, to, subject, and body.text are required".to_string(),
        ));
    }

    // Get outbound queue URL
    let outbound_queue_url = std::env::var("OUTBOUND_QUEUE_URL")
        .map_err(|_| ApiError::Internal("OUTBOUND_QUEUE_URL not configured".to_string()))?;

    // Get domain from environment
    let allowed_domains =
        std::env::var("ALLOWED_DOMAINS").unwrap_or_else(|_| "example.com".to_string());
    let domain = allowed_domains.split(',').next().unwrap_or("example.com");

    // Construct from address: _<app>@<domain>
    let from_address = format!("_{}@{}", req.from, domain);

    // Generate message ID and correlation ID
    let message_id = format!("test-{}", Uuid::new_v4());
    let correlation_id = Uuid::new_v4().to_string();

    // Build outbound message matching the spec format
    let outbound_message = serde_json::json!({
        "version": "1.0",
        "correlationId": correlation_id,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "source": format!("test-{}", req.from),
        "email": {
            "from": {
                "address": from_address,
                "name": format!("Test {}", req.from)
            },
            "to": [{
                "address": req.to,
                "name": ""
            }],
            "cc": [],
            "bcc": [],
            "replyTo": {
                "address": from_address,
                "name": ""
            },
            "subject": req.subject,
            "body": {
                "text": req.body.text,
                "html": req.body.html
            },
            "attachments": [],
            "headers": {}
        },
        "options": {
            "priority": "normal",
            "scheduledSendTime": null,
            "trackOpens": false,
            "trackClicks": false
        }
    });

    // Send message to outbound queue
    let send_result = ctx
        .sqs_client
        .send_message()
        .queue_url(&outbound_queue_url)
        .message_body(outbound_message.to_string())
        .message_group_id("test") // For FIFO queues
        .send()
        .await
        .map_err(|e| {
            error!("Failed to send message to outbound queue: {}", e);
            ApiError::Aws(format!("SQS send failed: {}", e))
        })?;

    let sqs_message_id = send_result.message_id().map(|s| s.to_string());
    let final_message_id = sqs_message_id.clone().unwrap_or(message_id);

    // Store test history in DynamoDB
    let _ = store_test_history(&ctx, "outbound", &req.to, &final_message_id, "success").await;

    info!(
        "Test outbound email queued successfully: {} to {}",
        final_message_id, req.to
    );

    Ok(Json(TestEmailResponse {
        success: true,
        message_id: final_message_id,
        ses_message_id: None,
        queue_url: Some(outbound_queue_url),
        timestamp: chrono::Utc::now().to_rfc3339(),
        error: None,
    }))
}

/// Get test email history
pub async fn history(
    State(ctx): State<Arc<ApiContext>>,
) -> Result<Json<TestHistoryResponse>, ApiError> {
    // Query DynamoDB for test history
    // Table: mailflow-test-history (would need to be created)
    let table_name = std::env::var("TEST_HISTORY_TABLE").unwrap_or_else(|_| {
        format!(
            "mailflow-test-history-{}",
            std::env::var("ENVIRONMENT").unwrap_or_else(|_| "dev".to_string())
        )
    });

    // Try to query the table
    let result = ctx
        .dynamodb_client
        .scan()
        .table_name(&table_name)
        .limit(20)
        .send()
        .await;

    let tests = match result {
        Ok(output) => output
            .items()
            .iter()
            .filter_map(|item| {
                let id = item.get("id")?.as_s().ok()?.to_string();
                let test_type = item.get("type")?.as_s().ok()?.to_string();
                let timestamp = item.get("timestamp")?.as_s().ok()?.to_string();
                let recipient = item.get("recipient")?.as_s().ok()?.to_string();
                let status = item.get("status")?.as_s().ok()?.to_string();
                let message_id = item.get("messageId")?.as_s().ok()?.to_string();

                Some(TestHistoryEntry {
                    id,
                    test_type,
                    timestamp,
                    recipient,
                    status,
                    message_id,
                })
            })
            .collect(),
        Err(e) => {
            // If table doesn't exist, return empty array
            info!("Test history table not found or error: {}", e);
            vec![]
        }
    };

    Ok(Json(TestHistoryResponse { tests }))
}

/// Helper: Get queue URL safely (returns None if not found)
async fn get_queue_url_safe(ctx: &ApiContext, queue_name: &str) -> Option<String> {
    ctx.sqs_client
        .get_queue_url()
        .queue_name(queue_name)
        .send()
        .await
        .ok()
        .and_then(|result| result.queue_url().map(|s| s.to_string()))
}

/// Helper: Store test email in history
async fn store_test_history(
    ctx: &ApiContext,
    test_type: &str,
    recipient: &str,
    message_id: &str,
    status: &str,
) -> Result<(), ApiError> {
    let table_name = std::env::var("TEST_HISTORY_TABLE").unwrap_or_else(|_| {
        format!(
            "mailflow-test-history-{}",
            std::env::var("ENVIRONMENT").unwrap_or_else(|_| "dev".to_string())
        )
    });

    let item = serde_json::json!({
        "id": Uuid::new_v4().to_string(),
        "type": test_type,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "recipient": recipient,
        "status": status,
        "messageId": message_id,
    });

    // Convert JSON to DynamoDB attribute map
    let mut attributes = std::collections::HashMap::new();
    if let serde_json::Value::Object(obj) = item {
        for (key, value) in obj {
            if let Some(s) = value.as_str() {
                attributes.insert(
                    key,
                    aws_sdk_dynamodb::types::AttributeValue::S(s.to_string()),
                );
            }
        }
    }

    // Try to put item (ignore errors if table doesn't exist)
    let _ = ctx
        .dynamodb_client
        .put_item()
        .table_name(table_name)
        .set_item(Some(attributes))
        .send()
        .await;

    Ok(())
}
