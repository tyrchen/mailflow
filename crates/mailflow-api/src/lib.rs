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
    extract::DefaultBodyLimit,
    http::{Method, header},
    middleware as axum_middleware,
    routing::{get, post},
};
use lambda_http::{Body, Error as LambdaError, Request, Response};
use std::sync::Arc;
use tower::ServiceExt;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};

/// Main API handler - converts Lambda HTTP request to Axum router
pub async fn handler(ctx: Arc<ApiContext>, event: Request) -> Result<Response<Body>, LambdaError> {
    info!("Processing API request: {} {}", event.method(), event.uri());

    // Build protected routes that require JWT authentication
    let protected = Router::new()
        // Metrics endpoints
        .route("/metrics/summary", get(api::metrics::summary))
        .route("/metrics/timeseries", get(api::metrics::timeseries))
        // Queue endpoints
        .route("/queues", get(api::queues::list))
        .route("/queues/{name}/messages", get(api::queues::messages))
        // Logs endpoint
        .route("/logs/query", post(api::logs::query))
        // Storage endpoints
        .route("/storage/stats", get(api::storage::stats))
        .route("/storage/{bucket}/objects", get(api::storage::objects))
        // Test email endpoints
        .route("/test/inbound", post(api::test::inbound))
        .route("/test/outbound", post(api::test::outbound))
        .route("/test/history", get(api::test::history))
        // Config endpoint
        .route("/config", get(api::config::get_config))
        // Apply JWT authentication middleware to all protected routes
        .route_layer(axum_middleware::from_fn_with_state(
            Arc::clone(&ctx),
            auth::auth_middleware,
        ));

    // Build API v1 router with public and protected routes
    let v1_router = Router::new()
        // Health endpoint (no auth required)
        .route("/health", get(api::health::handler))
        // Merge protected routes
        .merge(protected);

    // Build the main router with v1 nested
    let app = Router::new()
        .nest("/v1", v1_router)
        // Add observability middleware (logging + metrics)
        .route_layer(axum_middleware::from_fn_with_state(
            Arc::clone(&ctx),
            middleware::logging_middleware,
        ))
        .route_layer(axum_middleware::from_fn_with_state(
            Arc::clone(&ctx),
            middleware::metrics_middleware,
        ))
        // Add CORS middleware allowing all origins
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]),
        )
        // Increase body size limit to 10MB (API Gateway max)
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
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
