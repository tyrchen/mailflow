/// Health check endpoint
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::error;

use crate::context::ApiContext;

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub timestamp: String,
    pub checks: HealthChecks,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthChecks {
    pub sqs: String,
    pub s3: String,
    pub dynamodb: String,
    pub cloudwatch: String,
}

/// Health check handler
/// This endpoint does not require authentication
pub async fn handler(
    State(ctx): State<Arc<ApiContext>>,
) -> Result<Json<HealthResponse>, (StatusCode, Json<HealthResponse>)> {
    let mut all_healthy = true;

    // Check SQS connectivity
    let sqs_status = match ctx.sqs_client.list_queues().send().await {
        Ok(_) => "ok".to_string(),
        Err(e) => {
            error!("SQS health check failed: {}", e);
            all_healthy = false;
            "error".to_string()
        }
    };

    // Check S3 connectivity
    let s3_status = match ctx.s3_client.list_buckets().send().await {
        Ok(_) => "ok".to_string(),
        Err(e) => {
            error!("S3 health check failed: {}", e);
            all_healthy = false;
            "error".to_string()
        }
    };

    // Check DynamoDB connectivity
    let dynamodb_status = match ctx.dynamodb_client.list_tables().send().await {
        Ok(_) => "ok".to_string(),
        Err(e) => {
            error!("DynamoDB health check failed: {}", e);
            all_healthy = false;
            "error".to_string()
        }
    };

    // Check CloudWatch connectivity
    let cloudwatch_status = match ctx.cloudwatch_client.list_metrics().send().await {
        Ok(_) => "ok".to_string(),
        Err(e) => {
            error!("CloudWatch health check failed: {}", e);
            all_healthy = false;
            "error".to_string()
        }
    };

    let response = HealthResponse {
        status: if all_healthy {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        checks: HealthChecks {
            sqs: sqs_status,
            s3: s3_status,
            dynamodb: dynamodb_status,
            cloudwatch: cloudwatch_status,
        },
    };

    if all_healthy {
        Ok(Json(response))
    } else {
        Err((StatusCode::SERVICE_UNAVAILABLE, Json(response)))
    }
}
