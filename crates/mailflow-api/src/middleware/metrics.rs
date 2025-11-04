/// CloudWatch metrics middleware
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use std::time::Instant;
use tracing::error;

use crate::context::ApiContext;

/// Metrics middleware
///
/// Emits CloudWatch metrics for:
/// - Request count (by endpoint and status)
/// - Response time (by endpoint)
/// - Error count (by endpoint)
pub async fn metrics_middleware(
    State(ctx): State<Arc<ApiContext>>,
    request: Request,
    next: Next,
) -> Response {
    let start = Instant::now();

    // Extract path for metrics dimension
    let path = request.uri().path().to_string();
    let method = request.method().to_string();

    // Process request
    let response = next.run(request).await;

    // Calculate duration
    let duration = start.elapsed();
    let status = response.status();

    // Emit metrics to CloudWatch
    let endpoint = format!("{} {}", method, path);

    // Emit request count metric
    if let Err(e) = emit_metric(
        &ctx,
        "RequestCount",
        1.0,
        &endpoint,
        status.as_u16().to_string(),
    )
    .await
    {
        error!("Failed to emit RequestCount metric: {}", e);
    }

    // Emit response time metric
    if let Err(e) = emit_metric(
        &ctx,
        "ResponseTime",
        duration.as_millis() as f64,
        &endpoint,
        status.as_u16().to_string(),
    )
    .await
    {
        error!("Failed to emit ResponseTime metric: {}", e);
    }

    // Emit error count metric if request failed
    if (status.is_client_error() || status.is_server_error())
        && let Err(e) = emit_metric(
            &ctx,
            "ErrorCount",
            1.0,
            &endpoint,
            status.as_u16().to_string(),
        )
        .await
    {
        error!("Failed to emit ErrorCount metric: {}", e);
    }

    response
}

/// Emit a CloudWatch metric
async fn emit_metric(
    ctx: &ApiContext,
    metric_name: &str,
    value: f64,
    endpoint: &str,
    status: String,
) -> Result<(), String> {
    use aws_sdk_cloudwatch::types::{Dimension, MetricDatum, StandardUnit};

    let namespace =
        std::env::var("CLOUDWATCH_NAMESPACE").unwrap_or_else(|_| "Mailflow/API".to_string());

    // Build dimensions
    let endpoint_dimension = Dimension::builder()
        .name("Endpoint")
        .value(endpoint)
        .build();

    let status_dimension = Dimension::builder()
        .name("StatusCode")
        .value(status)
        .build();

    // Build metric datum
    let metric = MetricDatum::builder()
        .metric_name(metric_name)
        .value(value)
        .unit(if metric_name == "ResponseTime" {
            StandardUnit::Milliseconds
        } else {
            StandardUnit::Count
        })
        .dimensions(endpoint_dimension)
        .dimensions(status_dimension)
        .build();

    // Emit metric to CloudWatch
    ctx.cloudwatch_client
        .put_metric_data()
        .namespace(namespace)
        .metric_data(metric)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_metric_namespace() {
        unsafe {
            std::env::remove_var("CLOUDWATCH_NAMESPACE");
        }
        let namespace =
            std::env::var("CLOUDWATCH_NAMESPACE").unwrap_or_else(|_| "Mailflow/API".to_string());
        assert_eq!(namespace, "Mailflow/API");
    }

    #[test]
    fn test_metric_namespace_override() {
        unsafe {
            std::env::set_var("CLOUDWATCH_NAMESPACE", "Custom/Namespace");
        }
        let namespace =
            std::env::var("CLOUDWATCH_NAMESPACE").unwrap_or_else(|_| "Mailflow/API".to_string());
        assert_eq!(namespace, "Custom/Namespace");
        unsafe {
            std::env::remove_var("CLOUDWATCH_NAMESPACE");
        }
    }
}
