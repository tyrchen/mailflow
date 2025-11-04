/// Queue endpoints
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, warn};

use crate::{context::ApiContext, error::ApiError};

#[derive(Debug, Serialize)]
pub struct QueuesResponse {
    pub queues: Vec<QueueInfo>,
}

#[derive(Debug, Serialize)]
pub struct QueueInfo {
    pub name: String,
    pub url: String,
    #[serde(rename = "type")]
    pub queue_type: String,
    #[serde(rename = "messageCount")]
    pub message_count: i32,
    #[serde(rename = "messagesInFlight")]
    pub messages_in_flight: i32,
    #[serde(rename = "oldestMessageAge")]
    pub oldest_message_age: Option<i32>,
    #[serde(rename = "lastActivity")]
    pub last_activity: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessagesQuery {
    pub limit: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct MessagesResponse {
    #[serde(rename = "queueName")]
    pub queue_name: String,
    pub messages: Vec<MessageInfo>,
    #[serde(rename = "totalCount")]
    pub total_count: i32,
}

#[derive(Debug, Serialize)]
pub struct MessageInfo {
    #[serde(rename = "messageId")]
    pub message_id: String,
    #[serde(rename = "receiptHandle")]
    pub receipt_handle: String,
    pub body: String,
    pub attributes: serde_json::Value,
    pub preview: String,
}

pub async fn list(State(ctx): State<Arc<ApiContext>>) -> Result<Json<QueuesResponse>, ApiError> {
    // List all queues
    let list_result = ctx
        .sqs_client
        .list_queues()
        .send()
        .await
        .map_err(|e| ApiError::Aws(e.to_string()))?;

    let mut queues = Vec::new();

    for queue_url in list_result.queue_urls() {
        // Get queue attributes
        let attrs_result = ctx
            .sqs_client
            .get_queue_attributes()
            .queue_url(queue_url)
            .attribute_names(aws_sdk_sqs::types::QueueAttributeName::All)
            .send()
            .await;

        let queue_name = queue_url
            .split('/')
            .next_back()
            .unwrap_or("unknown")
            .to_string();

        let (message_count, messages_in_flight, oldest_message_age) = match attrs_result {
            Ok(attrs) => {
                let attributes = attrs.attributes();
                let msg_count = attributes
                    .and_then(|m| {
                        m.get(&aws_sdk_sqs::types::QueueAttributeName::ApproximateNumberOfMessages)
                    })
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                let in_flight = attributes
                    .and_then(|m| {
                        m.get(&aws_sdk_sqs::types::QueueAttributeName::ApproximateNumberOfMessagesNotVisible)
                    })
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                // Note: ApproximateAgeOfOldestMessage not available in this SDK version
                let oldest_age: Option<i32> = None;

                (msg_count, in_flight, oldest_age)
            }
            Err(e) => {
                warn!("Failed to get attributes for queue {}: {}", queue_name, e);
                (0, 0, None)
            }
        };

        // Determine queue type from name
        let queue_type = if queue_name.contains("-dlq") || queue_name.ends_with("-DLQ") {
            "dlq"
        } else if queue_name.contains("outbound") {
            "outbound"
        } else {
            "inbound"
        };

        queues.push(QueueInfo {
            name: queue_name,
            url: queue_url.to_string(),
            queue_type: queue_type.to_string(),
            message_count,
            messages_in_flight,
            oldest_message_age,
            last_activity: None, // Would need CloudWatch metrics for this
        });
    }

    Ok(Json(QueuesResponse { queues }))
}

pub async fn messages(
    State(ctx): State<Arc<ApiContext>>,
    Path(queue_name): Path<String>,
    Query(query): Query<MessagesQuery>,
) -> Result<Json<MessagesResponse>, ApiError> {
    let limit = query.limit.unwrap_or(10).min(10); // Max 10 messages

    // Get queue URL from name
    let queue_url = get_queue_url(&ctx, &queue_name).await?;

    // Get queue attributes to get total message count
    let attrs_result = ctx
        .sqs_client
        .get_queue_attributes()
        .queue_url(&queue_url)
        .attribute_names(aws_sdk_sqs::types::QueueAttributeName::ApproximateNumberOfMessages)
        .send()
        .await
        .map_err(|e| ApiError::Aws(e.to_string()))?;

    let total_count = attrs_result
        .attributes()
        .and_then(|m| m.get(&aws_sdk_sqs::types::QueueAttributeName::ApproximateNumberOfMessages))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    // Receive messages (peek without deleting)
    let receive_result = ctx
        .sqs_client
        .receive_message()
        .queue_url(&queue_url)
        .max_number_of_messages(limit)
        .visibility_timeout(0) // Don't hide messages from others
        .message_system_attribute_names(aws_sdk_sqs::types::MessageSystemAttributeName::All)
        .send()
        .await
        .map_err(|e| ApiError::Aws(e.to_string()))?;

    let messages: Vec<MessageInfo> = receive_result
        .messages()
        .iter()
        .map(|msg| {
            let message_id = msg.message_id().unwrap_or("").to_string();
            let receipt_handle = msg.receipt_handle().unwrap_or("").to_string();
            let body = msg.body().unwrap_or("").to_string();

            // Create preview (first 200 chars)
            let preview = create_message_preview(&body);

            // Convert attributes to JSON
            let attributes = msg
                .attributes()
                .map(|attrs| {
                    serde_json::json!(
                        attrs
                            .iter()
                            .map(|(k, v)| (format!("{:?}", k), v.clone()))
                            .collect::<std::collections::HashMap<_, _>>()
                    )
                })
                .unwrap_or_else(|| serde_json::json!({}));

            MessageInfo {
                message_id,
                receipt_handle,
                body,
                attributes,
                preview,
            }
        })
        .collect();

    Ok(Json(MessagesResponse {
        queue_name,
        messages,
        total_count,
    }))
}

/// Helper: Get queue URL from queue name
async fn get_queue_url(ctx: &ApiContext, queue_name: &str) -> Result<String, ApiError> {
    let result = ctx
        .sqs_client
        .get_queue_url()
        .queue_name(queue_name)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to get queue URL for {}: {}", queue_name, e);
            ApiError::NotFound(format!("Queue not found: {}", queue_name))
        })?;

    result
        .queue_url()
        .map(|u| u.to_string())
        .ok_or_else(|| ApiError::NotFound(format!("Queue URL not found for {}", queue_name)))
}

/// Helper: Create a preview of a message body
fn create_message_preview(body: &str) -> String {
    // Try to parse as JSON and extract email info
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body)
        && let Some(email) = json.get("email")
    {
        let from = email
            .get("from")
            .and_then(|f| f.get("address"))
            .and_then(|a| a.as_str())
            .unwrap_or("unknown");
        let subject = email
            .get("subject")
            .and_then(|s| s.as_str())
            .unwrap_or("(no subject)");
        return format!("Email from: {}, Subject: {}", from, subject);
    }

    // Fallback: just truncate
    if body.len() > 200 {
        format!("{}...", &body[..200])
    } else {
        body.to_string()
    }
}
