/// Logs endpoints
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{context::ApiContext, error::ApiError};

#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    #[serde(rename = "logGroup")]
    pub log_group: String,
    #[serde(rename = "startTime")]
    pub start_time: String,
    #[serde(rename = "endTime")]
    pub end_time: String,
    #[serde(rename = "filterPattern")]
    pub filter_pattern: Option<String>,
    pub limit: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct LogsResponse {
    pub logs: Vec<LogEntry>,
    #[serde(rename = "nextToken")]
    pub next_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub message: String,
    pub level: String,
    pub context: serde_json::Value,
}

pub async fn query(
    State(ctx): State<Arc<ApiContext>>,
    Json(query_req): Json<LogsQuery>,
) -> Result<Json<LogsResponse>, ApiError> {
    let start_time = chrono::DateTime::parse_from_rfc3339(&query_req.start_time)
        .map_err(|e| ApiError::BadRequest(format!("Invalid start_time: {}", e)))?
        .timestamp_millis();

    let end_time = chrono::DateTime::parse_from_rfc3339(&query_req.end_time)
        .map_err(|e| ApiError::BadRequest(format!("Invalid end_time: {}", e)))?
        .timestamp_millis();

    let limit = query_req.limit.unwrap_or(100).min(10000);

    let mut request = ctx
        .logs_client
        .filter_log_events()
        .log_group_name(&query_req.log_group)
        .start_time(start_time)
        .end_time(end_time)
        .limit(limit);

    if let Some(pattern) = &query_req.filter_pattern {
        request = request.filter_pattern(pattern);
    }

    let result = request
        .send()
        .await
        .map_err(|e| ApiError::Aws(e.to_string()))?;

    let logs: Vec<LogEntry> = result
        .events()
        .iter()
        .map(|event| {
            let timestamp = event
                .timestamp()
                .and_then(chrono::DateTime::from_timestamp_millis)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default();

            let message = event.message().unwrap_or("").to_string();

            // Try to parse JSON log for structured info
            let (level, context) =
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&message) {
                    let level = json
                        .get("level")
                        .and_then(|l| l.as_str())
                        .unwrap_or("INFO")
                        .to_string();
                    let context = json.clone();
                    (level, context)
                } else {
                    ("INFO".to_string(), serde_json::json!({}))
                };

            LogEntry {
                timestamp,
                message,
                level,
                context,
            }
        })
        .collect();

    Ok(Json(LogsResponse {
        logs,
        next_token: result.next_token().map(|s| s.to_string()),
    }))
}
