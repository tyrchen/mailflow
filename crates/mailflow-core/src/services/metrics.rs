/// CloudWatch metrics service for monitoring and observability
use crate::constants::METRICS_NAMESPACE;
use async_trait::async_trait;
use aws_sdk_cloudwatch::types::{Dimension, MetricDatum, StandardUnit};
use std::collections::HashMap;
use tracing::{debug, error};

#[async_trait]
pub trait MetricsService: Send + Sync {
    /// Record a counter metric (count of events)
    async fn record_counter(&self, name: &str, value: f64, dimensions: &[(&str, &str)]);

    /// Record a histogram metric (distribution of values)
    async fn record_histogram(
        &self,
        name: &str,
        value: f64,
        unit: MetricUnit,
        dimensions: &[(&str, &str)],
    );

    /// Record a gauge metric (current value)
    async fn record_gauge(
        &self,
        name: &str,
        value: f64,
        unit: MetricUnit,
        dimensions: &[(&str, &str)],
    );
}

#[derive(Debug, Clone, Copy)]
pub enum MetricUnit {
    Count,
    Milliseconds,
    Seconds,
    Bytes,
    None,
}

impl From<MetricUnit> for StandardUnit {
    fn from(unit: MetricUnit) -> Self {
        match unit {
            MetricUnit::Count => StandardUnit::Count,
            MetricUnit::Milliseconds => StandardUnit::Milliseconds,
            MetricUnit::Seconds => StandardUnit::Seconds,
            MetricUnit::Bytes => StandardUnit::Bytes,
            MetricUnit::None => StandardUnit::None,
        }
    }
}

/// CloudWatch metrics service implementation
pub struct CloudWatchMetricsService {
    client: aws_sdk_cloudwatch::Client,
    namespace: String,
    /// Buffer for batching metrics
    buffer: tokio::sync::Mutex<Vec<MetricDatum>>,
}

impl CloudWatchMetricsService {
    pub fn new(client: aws_sdk_cloudwatch::Client) -> Self {
        Self {
            client,
            namespace: METRICS_NAMESPACE.to_string(),
            buffer: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn with_namespace(client: aws_sdk_cloudwatch::Client, namespace: String) -> Self {
        Self {
            client,
            namespace,
            buffer: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    async fn emit_metric(
        &self,
        name: &str,
        value: f64,
        unit: MetricUnit,
        dimensions: &[(&str, &str)],
    ) {
        let dims: Vec<Dimension> = dimensions
            .iter()
            .map(|(k, v)| {
                Dimension::builder()
                    .name(k.to_string())
                    .value(v.to_string())
                    .build()
            })
            .collect();

        let datum = MetricDatum::builder()
            .metric_name(name)
            .value(value)
            .unit(unit.into())
            .timestamp(aws_smithy_types::DateTime::from(
                std::time::SystemTime::now(),
            ))
            .set_dimensions(if dims.is_empty() { None } else { Some(dims) })
            .build();

        // For now, send immediately (could batch for better performance)
        match self
            .client
            .put_metric_data()
            .namespace(&self.namespace)
            .metric_data(datum)
            .send()
            .await
        {
            Ok(_) => {
                debug!(
                    target: "metrics",
                    metric = name,
                    value = value,
                    "Emitted metric to CloudWatch"
                );
            }
            Err(e) => {
                error!(
                    target: "metrics",
                    metric = name,
                    error = %e,
                    "Failed to emit metric to CloudWatch"
                );
            }
        }
    }

    /// Flush buffered metrics (for future batching implementation)
    #[allow(dead_code)]
    async fn flush(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut buffer = self.buffer.lock().await;
        if buffer.is_empty() {
            return Ok(());
        }

        let data: Vec<MetricDatum> = buffer.drain(..).collect();

        self.client
            .put_metric_data()
            .namespace(&self.namespace)
            .set_metric_data(Some(data))
            .send()
            .await?;

        Ok(())
    }
}

#[async_trait]
impl MetricsService for CloudWatchMetricsService {
    async fn record_counter(&self, name: &str, value: f64, dimensions: &[(&str, &str)]) {
        self.emit_metric(name, value, MetricUnit::Count, dimensions)
            .await;
    }

    async fn record_histogram(
        &self,
        name: &str,
        value: f64,
        unit: MetricUnit,
        dimensions: &[(&str, &str)],
    ) {
        self.emit_metric(name, value, unit, dimensions).await;
    }

    async fn record_gauge(
        &self,
        name: &str,
        value: f64,
        unit: MetricUnit,
        dimensions: &[(&str, &str)],
    ) {
        self.emit_metric(name, value, unit, dimensions).await;
    }
}

/// Helper functions for commonly used metrics
pub struct Metrics;

impl Metrics {
    /// Record inbound email received
    pub async fn inbound_email_received(service: &dyn MetricsService, app_name: &str) {
        service
            .record_counter("InboundEmailsReceived", 1.0, &[("App", app_name)])
            .await;
    }

    /// Record inbound email processed successfully
    pub async fn inbound_email_processed(
        service: &dyn MetricsService,
        app_name: &str,
        duration_ms: f64,
    ) {
        service
            .record_counter("InboundEmailsProcessed", 1.0, &[("App", app_name)])
            .await;
        service
            .record_histogram(
                "InboundProcessingTime",
                duration_ms,
                MetricUnit::Milliseconds,
                &[("App", app_name)],
            )
            .await;
    }

    /// Record attachment processed
    pub async fn attachment_processed(
        service: &dyn MetricsService,
        size_bytes: usize,
        content_type: &str,
    ) {
        service
            .record_counter(
                "AttachmentsProcessed",
                1.0,
                &[("ContentType", content_type)],
            )
            .await;
        service
            .record_histogram(
                "AttachmentSize",
                size_bytes as f64,
                MetricUnit::Bytes,
                &[("ContentType", content_type)],
            )
            .await;
    }

    /// Record outbound email sent
    pub async fn outbound_email_sent(service: &dyn MetricsService, duration_ms: f64) {
        service.record_counter("OutboundEmailsSent", 1.0, &[]).await;
        service
            .record_histogram(
                "OutboundProcessingTime",
                duration_ms,
                MetricUnit::Milliseconds,
                &[],
            )
            .await;
    }

    /// Record error
    pub async fn error_occurred(service: &dyn MetricsService, error_type: &str, handler: &str) {
        service
            .record_counter(
                "Errors",
                1.0,
                &[("ErrorType", error_type), ("Handler", handler)],
            )
            .await;
    }

    /// Record DLQ message
    pub async fn dlq_message_sent(service: &dyn MetricsService, handler: &str) {
        service
            .record_counter("DLQMessages", 1.0, &[("Handler", handler)])
            .await;
    }

    /// Record routing decision
    pub async fn routing_decision(service: &dyn MetricsService, app_name: &str) {
        service
            .record_counter("RoutingDecisions", 1.0, &[("App", app_name)])
            .await;
    }
}

/// Mock metrics service for testing
pub struct MockMetricsService {
    metrics: tokio::sync::Mutex<HashMap<String, Vec<f64>>>,
}

impl MockMetricsService {
    pub fn new() -> Self {
        Self {
            metrics: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    #[allow(dead_code)]
    pub async fn get_metric_values(&self, name: &str) -> Vec<f64> {
        self.metrics
            .lock()
            .await
            .get(name)
            .cloned()
            .unwrap_or_default()
    }
}

impl Default for MockMetricsService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MetricsService for MockMetricsService {
    async fn record_counter(&self, name: &str, value: f64, _dimensions: &[(&str, &str)]) {
        let mut metrics = self.metrics.lock().await;
        metrics.entry(name.to_string()).or_default().push(value);
    }

    async fn record_histogram(
        &self,
        name: &str,
        value: f64,
        _unit: MetricUnit,
        _dimensions: &[(&str, &str)],
    ) {
        let mut metrics = self.metrics.lock().await;
        metrics.entry(name.to_string()).or_default().push(value);
    }

    async fn record_gauge(
        &self,
        name: &str,
        value: f64,
        _unit: MetricUnit,
        _dimensions: &[(&str, &str)],
    ) {
        let mut metrics = self.metrics.lock().await;
        metrics.entry(name.to_string()).or_default().push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_metrics() {
        let service = MockMetricsService::new();

        service.record_counter("TestMetric", 1.0, &[]).await;
        service.record_counter("TestMetric", 2.0, &[]).await;

        let values = service.get_metric_values("TestMetric").await;
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], 1.0);
        assert_eq!(values[1], 2.0);
    }

    #[tokio::test]
    async fn test_metrics_helpers() {
        let service = MockMetricsService::new();

        Metrics::inbound_email_received(&service, "app1").await;
        Metrics::inbound_email_processed(&service, "app1", 100.0).await;
        Metrics::attachment_processed(&service, 1024, "application/pdf").await;

        let received = service.get_metric_values("InboundEmailsReceived").await;
        assert_eq!(received.len(), 1);
        assert_eq!(received[0], 1.0);
    }
}
