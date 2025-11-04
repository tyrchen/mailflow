use aws_sdk_cloudwatch::types::Statistic;
/// Metrics endpoints
use axum::{
    Json,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::error;

use crate::{context::ApiContext, error::ApiError};

#[derive(Debug, Serialize, Deserialize)]
pub struct MetricsSummaryResponse {
    pub period: String,
    pub inbound: InboundMetrics,
    pub outbound: OutboundMetrics,
    pub processing: ProcessingMetrics,
    pub queues: QueueMetrics,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InboundMetrics {
    pub total: f64,
    pub rate: f64,
    #[serde(rename = "errorRate")]
    pub error_rate: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OutboundMetrics {
    pub total: f64,
    pub rate: f64,
    #[serde(rename = "errorRate")]
    pub error_rate: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessingMetrics {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueMetrics {
    pub active: usize,
    #[serde(rename = "dlqMessages")]
    pub dlq_messages: i32,
}

#[derive(Debug, Deserialize)]
pub struct TimeseriesQuery {
    pub metric: String,
    pub period: Option<String>,
    pub interval: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TimeseriesResponse {
    pub metric: String,
    pub unit: String,
    pub datapoints: Vec<Datapoint>,
}

#[derive(Debug, Serialize)]
pub struct Datapoint {
    pub timestamp: String,
    pub value: f64,
}

pub async fn summary(
    State(ctx): State<Arc<ApiContext>>,
) -> Result<Json<MetricsSummaryResponse>, ApiError> {
    let end_time = chrono::Utc::now();
    let start_time = end_time - chrono::Duration::hours(24);

    // Get inbound email count
    let inbound_total = get_metric_sum(&ctx, "InboundEmailsReceived", &start_time, &end_time)
        .await
        .unwrap_or(0.0);

    // Get outbound email count
    let outbound_total = get_metric_sum(&ctx, "OutboundEmailsSent", &start_time, &end_time)
        .await
        .unwrap_or(0.0);

    // Calculate rates (per minute)
    let hours_in_period = 24.0;
    let minutes_in_period = hours_in_period * 60.0;
    let inbound_rate = inbound_total / minutes_in_period;
    let outbound_rate = outbound_total / minutes_in_period;

    // Get error counts
    let inbound_errors = get_metric_sum(&ctx, "InboundErrors", &start_time, &end_time)
        .await
        .unwrap_or(0.0);
    let outbound_errors = get_metric_sum(&ctx, "OutboundErrors", &start_time, &end_time)
        .await
        .unwrap_or(0.0);

    let inbound_error_rate = if inbound_total > 0.0 {
        inbound_errors / inbound_total
    } else {
        0.0
    };
    let outbound_error_rate = if outbound_total > 0.0 {
        outbound_errors / outbound_total
    } else {
        0.0
    };

    // Get processing time percentiles
    let p50 = get_metric_percentile(&ctx, "ProcessingTime", 50.0, &start_time, &end_time)
        .await
        .unwrap_or(0.0);
    let p95 = get_metric_percentile(&ctx, "ProcessingTime", 95.0, &start_time, &end_time)
        .await
        .unwrap_or(0.0);
    let p99 = get_metric_percentile(&ctx, "ProcessingTime", 99.0, &start_time, &end_time)
        .await
        .unwrap_or(0.0);

    // Get all queues
    let queues = ctx
        .sqs_client
        .list_queues()
        .send()
        .await
        .map_err(|e| ApiError::Aws(e.to_string()))?;

    let queue_urls = queues.queue_urls();

    // Count active queues (non-DLQ queues with messages)
    let mut active_count = 0;
    let mut dlq_message_count = 0;

    for queue_url in queue_urls {
        // Check if this is a DLQ (contains "dlq" in URL, case-insensitive)
        let is_dlq = queue_url.to_lowercase().contains("dlq");

        // Get queue attributes
        let attrs = ctx
            .sqs_client
            .get_queue_attributes()
            .queue_url(queue_url)
            .attribute_names(aws_sdk_sqs::types::QueueAttributeName::ApproximateNumberOfMessages)
            .send()
            .await;

        if let Ok(response) = attrs
            && let Some(attributes) = response.attributes()
            && let Some(count_str) =
                attributes.get(&aws_sdk_sqs::types::QueueAttributeName::ApproximateNumberOfMessages)
            && let Ok(count) = count_str.parse::<i32>()
        {
            if is_dlq {
                dlq_message_count += count;
            } else if count > 0 {
                active_count += 1;
            }
        }
    }

    let active_queues = active_count;
    let dlq_messages = dlq_message_count;

    Ok(Json(MetricsSummaryResponse {
        period: "24h".to_string(),
        inbound: InboundMetrics {
            total: inbound_total,
            rate: inbound_rate,
            error_rate: inbound_error_rate,
        },
        outbound: OutboundMetrics {
            total: outbound_total,
            rate: outbound_rate,
            error_rate: outbound_error_rate,
        },
        processing: ProcessingMetrics { p50, p95, p99 },
        queues: QueueMetrics {
            active: active_queues,
            dlq_messages,
        },
    }))
}

pub async fn timeseries(
    State(ctx): State<Arc<ApiContext>>,
    Query(query): Query<TimeseriesQuery>,
) -> Result<Json<TimeseriesResponse>, ApiError> {
    let period_hours = match query.period.as_deref() {
        Some("1h") => 1,
        Some("6h") => 6,
        Some("7d") => 168,
        Some("30d") => 720,
        _ => 24, // default 24h
    };

    let interval_minutes = match query.interval.as_deref() {
        Some("1m") => 1,
        Some("5m") => 5,
        Some("1h") => 60,
        Some("1d") => 1440,
        _ => 60, // default 1h
    };

    let end_time = chrono::Utc::now();
    let start_time = end_time - chrono::Duration::hours(period_hours);

    // Map metric name
    let metric_name = match query.metric.as_str() {
        "inbound_received" => "InboundEmailsReceived",
        "outbound_sent" => "OutboundEmailsSent",
        "error_rate" => "ErrorRate",
        "processing_time" => "ProcessingTime",
        _ => {
            return Err(ApiError::BadRequest(format!(
                "Unknown metric: {}",
                query.metric
            )));
        }
    };

    let datapoints =
        get_metric_timeseries(&ctx, metric_name, &start_time, &end_time, interval_minutes).await?;

    let unit = match query.metric.as_str() {
        "processing_time" => "milliseconds",
        "error_rate" => "percent",
        _ => "count",
    };

    Ok(Json(TimeseriesResponse {
        metric: query.metric,
        unit: unit.to_string(),
        datapoints,
    }))
}

/// Helper: Get sum of a metric over a time period
async fn get_metric_sum(
    ctx: &ApiContext,
    metric_name: &str,
    start_time: &chrono::DateTime<chrono::Utc>,
    end_time: &chrono::DateTime<chrono::Utc>,
) -> Result<f64, ApiError> {
    let result = ctx
        .cloudwatch_client
        .get_metric_statistics()
        .namespace("Mailflow")
        .metric_name(metric_name)
        .start_time(aws_smithy_types::DateTime::from_secs(
            start_time.timestamp(),
        ))
        .end_time(aws_smithy_types::DateTime::from_secs(end_time.timestamp()))
        .period(3600) // 1 hour
        .statistics(Statistic::Sum)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to get metric {}: {}", metric_name, e);
            ApiError::Aws(e.to_string())
        })?;

    let sum: f64 = result.datapoints().iter().filter_map(|dp| dp.sum()).sum();

    Ok(sum)
}

/// Helper: Get percentile of a metric
async fn get_metric_percentile(
    ctx: &ApiContext,
    metric_name: &str,
    percentile: f64,
    start_time: &chrono::DateTime<chrono::Utc>,
    end_time: &chrono::DateTime<chrono::Utc>,
) -> Result<f64, ApiError> {
    let percentile_str = format!("p{}", percentile as i32);

    let result = ctx
        .cloudwatch_client
        .get_metric_statistics()
        .namespace("Mailflow")
        .metric_name(metric_name)
        .start_time(aws_smithy_types::DateTime::from_secs(
            start_time.timestamp(),
        ))
        .end_time(aws_smithy_types::DateTime::from_secs(end_time.timestamp()))
        .period(3600)
        .extended_statistics(percentile_str)
        .send()
        .await
        .map_err(|e| ApiError::Aws(e.to_string()))?;

    // Get the average of extended statistics
    let avg = result
        .datapoints()
        .iter()
        .filter_map(|dp| {
            dp.extended_statistics()
                .and_then(|stats| stats.values().next().copied())
        })
        .sum::<f64>()
        / result.datapoints().len().max(1) as f64;

    Ok(avg)
}

/// Helper: Get timeseries data for a metric
async fn get_metric_timeseries(
    ctx: &ApiContext,
    metric_name: &str,
    start_time: &chrono::DateTime<chrono::Utc>,
    end_time: &chrono::DateTime<chrono::Utc>,
    interval_minutes: i64,
) -> Result<Vec<Datapoint>, ApiError> {
    let period_seconds = (interval_minutes * 60) as i32;

    let result = ctx
        .cloudwatch_client
        .get_metric_statistics()
        .namespace("Mailflow")
        .metric_name(metric_name)
        .start_time(aws_smithy_types::DateTime::from_secs(
            start_time.timestamp(),
        ))
        .end_time(aws_smithy_types::DateTime::from_secs(end_time.timestamp()))
        .period(period_seconds)
        .statistics(Statistic::Sum)
        .send()
        .await
        .map_err(|e| ApiError::Aws(e.to_string()))?;

    let mut datapoints: Vec<Datapoint> = result
        .datapoints()
        .iter()
        .filter_map(|dp| {
            let timestamp = dp.timestamp()?;
            let value = dp.sum()?;
            Some(Datapoint {
                timestamp: chrono::DateTime::from_timestamp(timestamp.secs(), 0)?.to_rfc3339(),
                value,
            })
        })
        .collect();

    // Sort by timestamp
    datapoints.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    Ok(datapoints)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_summary_response_structure() {
        let response = MetricsSummaryResponse {
            period: "24h".to_string(),
            inbound: InboundMetrics {
                total: 1000.0,
                rate: 0.69,
                error_rate: 0.01,
            },
            outbound: OutboundMetrics {
                total: 950.0,
                rate: 0.66,
                error_rate: 0.005,
            },
            processing: ProcessingMetrics {
                p50: 100.0,
                p95: 250.0,
                p99: 500.0,
            },
            queues: QueueMetrics {
                active: 3,
                dlq_messages: 0,
            },
        };

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["period"], "24h");
        assert_eq!(json["inbound"]["total"], 1000.0);
        assert_eq!(json["outbound"]["errorRate"], 0.005);
    }

    #[test]
    fn test_error_rate_calculation() {
        let total = 1000.0;
        let errors = 50.0;
        let error_rate = errors / total;
        assert_eq!(error_rate, 0.05);

        let total_zero = 0.0;
        let error_rate_zero = if total_zero > 0.0 {
            errors / total_zero
        } else {
            0.0
        };
        assert_eq!(error_rate_zero, 0.0);
    }

    #[test]
    fn test_period_interval_parsing() {
        let periods = vec![("1h", 1), ("6h", 6), ("7d", 168), ("30d", 720)];
        for (input, expected) in periods {
            let hours = match input {
                "1h" => 1,
                "6h" => 6,
                "7d" => 168,
                "30d" => 720,
                _ => 24,
            };
            assert_eq!(hours, expected);
        }
    }
}
