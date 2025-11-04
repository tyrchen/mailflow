/// Request logging middleware
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn};
use uuid::Uuid;

use crate::auth::UserClaims;
use crate::context::ApiContext;

/// Request logging middleware
///
/// Logs all incoming requests with:
/// - Request ID (generated)
/// - HTTP method and path
/// - User identity (from JWT claims if available)
/// - Response status code
/// - Request duration
pub async fn logging_middleware(
    State(_ctx): State<Arc<ApiContext>>,
    request: Request,
    next: Next,
) -> Response {
    let start = Instant::now();

    // Generate request ID
    let request_id = Uuid::new_v4().to_string();

    // Extract request details
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path().to_string();

    // Extract user identity from JWT claims if present
    let user_identity = request
        .extensions()
        .get::<UserClaims>()
        .map(|claims| format!("{} ({})", claims.0.email, claims.0.sub))
        .unwrap_or_else(|| "anonymous".to_string());

    // Log incoming request
    info!(
        request_id = %request_id,
        method = %method,
        path = %path,
        user = %user_identity,
        "Incoming request"
    );

    // Process request
    let response = next.run(request).await;

    // Calculate duration
    let duration = start.elapsed();
    let status = response.status();

    // Log response
    if status.is_success() {
        info!(
            request_id = %request_id,
            method = %method,
            path = %path,
            user = %user_identity,
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            "Request completed"
        );
    } else if status.is_client_error() || status.is_server_error() {
        warn!(
            request_id = %request_id,
            method = %method,
            path = %path,
            user = %user_identity,
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            "Request failed"
        );
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_id_generation() {
        let id1 = Uuid::new_v4().to_string();
        let id2 = Uuid::new_v4().to_string();
        assert_ne!(id1, id2, "Request IDs should be unique");
    }
}
