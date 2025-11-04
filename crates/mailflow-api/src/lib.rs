/// Mailflow API - Dashboard API Lambda
///
/// This module contains the REST API handlers for the Mailflow dashboard.
pub mod api;
pub mod auth;
pub mod context;
pub mod error;
pub mod middleware;

pub use context::ApiContext;
pub use error::ApiError;

use axum::{
    Router,
    body::Body as AxumBody,
    http::{HeaderValue, Method, header},
    middleware as axum_middleware,
    routing::{get, post},
};
use lambda_http::{Body, Error as LambdaError, Request, Response};
use std::sync::Arc;
use tower::ServiceExt;
use tower_http::cors::CorsLayer;
use tracing::{error, info};

/// Main API handler - converts Lambda HTTP request to Axum router
pub async fn handler(ctx: Arc<ApiContext>, event: Request) -> Result<Response<Body>, LambdaError> {
    info!("Processing API request: {} {}", event.method(), event.uri());

    // Get allowed origin from environment or use default
    let allowed_origin = std::env::var("ALLOWED_ORIGIN")
        .unwrap_or_else(|_| "https://dashboard.example.com".to_string());

    let origin = allowed_origin
        .parse::<HeaderValue>()
        .unwrap_or_else(|_| HeaderValue::from_static("https://dashboard.example.com"));

    // Build protected routes that require JWT authentication
    let protected = Router::new()
        // Metrics endpoints
        .route("/api/metrics/summary", get(api::metrics::summary))
        .route("/api/metrics/timeseries", get(api::metrics::timeseries))
        // Queue endpoints
        .route("/api/queues", get(api::queues::list))
        .route("/api/queues/:name/messages", get(api::queues::messages))
        // Logs endpoint
        .route("/api/logs/query", post(api::logs::query))
        // Storage endpoints
        .route("/api/storage/stats", get(api::storage::stats))
        .route("/api/storage/:bucket/objects", get(api::storage::objects))
        // Test email endpoints
        .route("/api/test/inbound", post(api::test::inbound))
        .route("/api/test/outbound", post(api::test::outbound))
        .route("/api/test/history", get(api::test::history))
        // Config endpoint
        .route("/api/config", get(api::config::get_config))
        // Apply JWT authentication middleware to all protected routes
        .route_layer(axum_middleware::from_fn_with_state(
            Arc::clone(&ctx),
            auth::auth_middleware,
        ));

    // Build the main router with public and protected routes
    let app = Router::new()
        // Health endpoint (no auth required)
        .route("/api/health", get(api::health::handler))
        // Merge protected routes
        .merge(protected)
        // Add observability middleware (logging + metrics)
        .route_layer(axum_middleware::from_fn_with_state(
            Arc::clone(&ctx),
            middleware::logging_middleware,
        ))
        .route_layer(axum_middleware::from_fn_with_state(
            Arc::clone(&ctx),
            middleware::metrics_middleware,
        ))
        // Add CORS middleware with restricted origin
        .layer(
            CorsLayer::new()
                .allow_origin(origin)
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
                .allow_credentials(true),
        )
        // Add API context
        .with_state(ctx);

    // Convert Lambda HTTP request to Axum request
    let (parts, body) = event.into_parts();
    let body_bytes = body.to_vec();

    let axum_request = http::Request::from_parts(parts, AxumBody::from(body_bytes));

    // Process request with Axum
    match app.oneshot(axum_request).await {
        Ok(response) => {
            let (parts, body) = response.into_parts();

            // Convert Axum response body to Lambda response body
            let body_bytes = axum::body::to_bytes(body, usize::MAX)
                .await
                .unwrap_or_default();

            let lambda_response = Response::from_parts(parts, Body::from(body_bytes.to_vec()));
            Ok(lambda_response)
        }
        Err(err) => {
            error!("Axum router error: {}", err);
            let response = Response::builder()
                .status(500)
                .body(Body::from(
                    serde_json::json!({
                        "error": "Internal server error"
                    })
                    .to_string(),
                ))
                .unwrap();
            Ok(response)
        }
    }
}
