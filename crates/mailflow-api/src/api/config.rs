/// Config endpoints
use axum::{Json, extract::State};
use serde::Serialize;
use std::sync::Arc;

use crate::{context::ApiContext, error::ApiError};

#[derive(Debug, Serialize)]
pub struct ConfigResponse {
    pub version: String,
    pub source: String,
    pub routing: serde_json::Value,
    pub security: SecurityConfig,
    pub attachments: AttachmentsConfig,
}

#[derive(Debug, Serialize)]
pub struct SecurityConfig {
    #[serde(rename = "requireSpf")]
    pub require_spf: bool,
    #[serde(rename = "requireDkim")]
    pub require_dkim: bool,
    #[serde(rename = "requireDmarc")]
    pub require_dmarc: bool,
}

#[derive(Debug, Serialize)]
pub struct AttachmentsConfig {
    pub bucket: String,
    #[serde(rename = "presignedUrlExpiration")]
    pub presigned_url_expiration: i64,
    #[serde(rename = "maxSize")]
    pub max_size: i64,
}

pub async fn get_config(
    State(_ctx): State<Arc<ApiContext>>,
) -> Result<Json<ConfigResponse>, ApiError> {
    // Read config from environment variables
    let config = ConfigResponse {
        version: "1.0".to_string(),
        source: "environment".to_string(),
        routing: serde_json::json!({}), // Would read ROUTING_MAP env var
        security: SecurityConfig {
            require_spf: false,
            require_dkim: false,
            require_dmarc: false,
        },
        attachments: AttachmentsConfig {
            bucket: std::env::var("ATTACHMENTS_BUCKET").unwrap_or_default(),
            presigned_url_expiration: 604800,
            max_size: 36700160,
        },
    };

    Ok(Json(config))
}
