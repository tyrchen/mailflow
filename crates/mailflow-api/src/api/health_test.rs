#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_response_structure() {
        // Test that HealthResponse serializes correctly
        let response = HealthResponse {
            status: "healthy".to_string(),
            version: "0.2.2".to_string(),
            timestamp: "2025-11-03T10:00:00Z".to_string(),
            checks: HealthChecks {
                sqs: "ok".to_string(),
                s3: "ok".to_string(),
                dynamodb: "ok".to_string(),
                cloudwatch: "ok".to_string(),
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("healthy"));
        assert!(json.contains("0.2.2"));
    }

    #[test]
    fn test_health_checks_structure() {
        let checks = HealthChecks {
            sqs: "ok".to_string(),
            s3: "error".to_string(),
            dynamodb: "ok".to_string(),
            cloudwatch: "ok".to_string(),
        };

        assert_eq!(checks.sqs, "ok");
        assert_eq!(checks.s3, "error");
    }
}
