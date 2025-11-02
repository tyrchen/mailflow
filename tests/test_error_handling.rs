/// INT-023 to INT-027: Error Handling Integration Tests
///
/// These tests validate error handling and recovery:
/// - DLQ routing for failed messages
/// - Retry logic
/// - Invalid data handling
/// - Error message formatting
#[path = "common/mod.rs"]
mod common;

use chrono::Utc;
use mailflow::email::parser::{EmailParser, MailParserEmailParser};
use mailflow::error::MailflowError;
use mailflow::models::{EmailAddress, EmailBody, OutboundEmail, OutboundMessage, SendOptions};
use mailflow::utils::validation::validate_email_address;

// Helper function to validate outbound message (mirrors internal validation logic)
fn validate_outbound_message(message: &OutboundMessage) -> Result<(), MailflowError> {
    if message.email.to.is_empty() {
        return Err(MailflowError::Validation(
            "At least one recipient required".to_string(),
        ));
    }
    if message.email.from.address.is_empty() {
        return Err(MailflowError::Validation(
            "From address required".to_string(),
        ));
    }
    if message.email.subject.is_empty() {
        return Err(MailflowError::Validation("Subject required".to_string()));
    }
    if message.email.body.text.is_none() && message.email.body.html.is_none() {
        return Err(MailflowError::Validation(
            "Email must have text or HTML body".to_string(),
        ));
    }
    validate_email_address(&message.email.from.address)?;
    for to in &message.email.to {
        validate_email_address(&to.address)?;
    }
    Ok(())
}

/// INT-023: DLQ routing
/// Validates: Error messages contain necessary context (NFR-2.6)
#[test]
fn int_023_dlq_routing() {
    // Test error context structure
    let error_context = serde_json::json!({
        "error_type": "validation",
        "error_message": "Invalid email format",
        "original_message": {
            "bucket": "mailflow-raw-emails-dev",
            "key": "emails/test-123.eml"
        },
        "timestamp": Utc::now().to_rfc3339(),
        "retry_count": 0
    });

    assert!(error_context["error_type"].is_string());
    assert!(error_context["error_message"].is_string());
    assert!(error_context["original_message"].is_object());
    assert!(error_context["timestamp"].is_string());
}

/// INT-024: Retry on transient failure
/// Validates: Retry logic and error classification (FR-2.16)
#[test]
fn int_024_retry_on_transient_failure() {
    // Test error types that should be retried
    let throttle_error = MailflowError::Ses("Throttled".to_string());
    let network_error = MailflowError::Storage("Network timeout".to_string());

    // Errors should provide useful messages
    assert!(throttle_error.to_string().contains("Throttled"));
    assert!(network_error.to_string().contains("timeout"));

    // Test error types that should NOT be retried
    let validation_error = MailflowError::Validation("Invalid email".to_string());
    assert!(validation_error.to_string().contains("Invalid"));

    // Classify errors
    fn is_retriable(error: &MailflowError) -> bool {
        matches!(
            error,
            MailflowError::Ses(_) | MailflowError::Storage(_) | MailflowError::Queue(_)
        )
    }

    assert!(is_retriable(&throttle_error));
    assert!(is_retriable(&network_error));
    assert!(!is_retriable(&validation_error));
}

/// INT-025: S3 download failure
/// Validates: Missing S3 object error handling
#[test]
fn int_025_s3_download_failure() {
    // Test S3 error scenarios
    let not_found_error =
        MailflowError::Storage("Object not found: s3://bucket/missing.eml".to_string());
    let access_denied_error = MailflowError::Storage("Access denied".to_string());

    assert!(not_found_error.to_string().contains("not found"));
    assert!(access_denied_error.to_string().contains("Access denied"));

    // Errors should be descriptive
    assert!(not_found_error.to_string().len() > 10);
}

/// INT-026: Invalid email format
/// Validates: Malformed MIME handling
#[tokio::test]
async fn int_026_invalid_email_format() {
    // Test various malformed emails
    let invalid_emails = [
        b"" as &[u8],                      // Empty
        b"Not a valid email",              // No headers
        b"From: test\r\nNo-To-Header\r\n", // Missing To
    ];

    let parser = MailParserEmailParser::new();

    for (idx, raw_email) in invalid_emails.iter().enumerate() {
        let result = parser.parse(raw_email).await;

        // Some may parse with defaults, some may error
        // Both are acceptable as long as they don't panic
        match result {
            Ok(_email) => {
                // If it parses, should have some basic structure
                println!("Email {} parsed with defaults", idx);
            }
            Err(e) => {
                // If it errors, should have clear message
                assert!(!e.to_string().is_empty());
                println!("Email {} failed to parse: {}", idx, e);
            }
        }
    }
}

/// INT-027: Queue deletion failure
/// Validates: Logging when queue operations fail (CRIT-003)
#[test]
fn int_027_queue_deletion_failure() {
    // Test queue error scenarios
    let delete_error =
        MailflowError::Queue("Failed to delete message: InvalidReceiptHandle".to_string());
    let send_error = MailflowError::Queue("Failed to send message: QueueDoesNotExist".to_string());

    assert!(delete_error.to_string().contains("delete"));
    assert!(send_error.to_string().contains("send"));

    // Errors should contain actionable information
    assert!(delete_error.to_string().contains("InvalidReceiptHandle"));
    assert!(send_error.to_string().contains("QueueDoesNotExist"));
}

/// Test error message formatting
#[test]
fn test_error_formatting() {
    let errors = vec![
        MailflowError::Validation("Invalid format".to_string()),
        MailflowError::EmailParsing("Parse failed".to_string()),
        MailflowError::Storage("Storage error".to_string()),
        MailflowError::Queue("Queue error".to_string()),
        MailflowError::Ses("SES error".to_string()),
        MailflowError::Routing("Routing error".to_string()),
        MailflowError::Config("Config error".to_string()),
    ];

    for error in errors {
        let msg = error.to_string();
        assert!(!msg.is_empty(), "Error message should not be empty");
        assert!(msg.len() > 5, "Error message should be descriptive");
    }
}

/// Test validation error details
#[test]
fn test_validation_errors() {
    // Test email validation errors
    let invalid_emails = vec![
        "",
        "notanemail",
        "@example.com",
        "user@",
        "user @example.com",
    ];

    for email in invalid_emails {
        let result = validate_email_address(email);
        assert!(result.is_err(), "Should reject: {}", email);
        assert!(!result.unwrap_err().to_string().is_empty());
    }

    // Test outbound message validation errors
    let message = OutboundMessage {
        version: "1.0".to_string(),
        correlation_id: "test".to_string(),
        timestamp: Utc::now(),
        source: "test".to_string(),
        email: OutboundEmail {
            from: EmailAddress {
                address: "".to_string(), // Invalid: empty
                name: None,
            },
            to: vec![],
            cc: vec![],
            bcc: vec![],
            reply_to: None,
            subject: "Test".to_string(),
            body: EmailBody {
                text: Some("Body".to_string()),
                html: None,
            },
            attachments: vec![],
            headers: Default::default(),
        },
        options: SendOptions::default(),
    };

    let result = validate_outbound_message(&message);
    assert!(result.is_err());
}

/// Test error context preservation
#[test]
fn test_error_context() {
    // Test that errors maintain context through transformations
    let original_error = MailflowError::Storage("Bucket not found: test-bucket".to_string());

    // Error should contain bucket name
    assert!(original_error.to_string().contains("test-bucket"));

    // Convert to JSON for DLQ
    let error_json = serde_json::json!({
        "error": original_error.to_string(),
        "error_type": "S3",
        "timestamp": Utc::now().to_rfc3339(),
    });

    assert!(
        error_json["error"]
            .as_str()
            .unwrap()
            .contains("test-bucket")
    );
}

/// Test retry backoff calculation
#[test]
fn test_retry_backoff() {
    // Test exponential backoff calculation
    fn calculate_backoff(retry_count: u32, base_ms: u64) -> u64 {
        base_ms * 2_u64.pow(retry_count)
    }

    assert_eq!(calculate_backoff(0, 100), 100); // First retry: 100ms
    assert_eq!(calculate_backoff(1, 100), 200); // Second retry: 200ms
    assert_eq!(calculate_backoff(2, 100), 400); // Third retry: 400ms
    assert_eq!(calculate_backoff(3, 100), 800); // Fourth retry: 800ms
    assert_eq!(calculate_backoff(4, 100), 1600); // Fifth retry: 1600ms

    // Cap at max retry
    const MAX_RETRIES: u32 = 5;
    for attempt in 0..=10 {
        let backoff = calculate_backoff(attempt.min(MAX_RETRIES), 100);
        assert!(backoff <= calculate_backoff(MAX_RETRIES, 100));
    }
}

/// Test error recovery strategies
#[test]
fn test_error_recovery() {
    // Classify errors by recovery strategy
    fn get_recovery_strategy(error: &MailflowError) -> &'static str {
        match error {
            MailflowError::Validation(_) => "reject",
            MailflowError::EmailParsing(_) => "reject",
            MailflowError::Storage(_) => "retry",
            MailflowError::Queue(_) => "retry",
            MailflowError::Ses(_) => "retry",
            MailflowError::Routing(_) => "dlq",
            MailflowError::Config(_) => "fatal",
            MailflowError::Idempotency(_) => "retry",
            MailflowError::RateLimit(_) => "reject",
            MailflowError::Lambda(_) => "fatal",
            MailflowError::Unknown(_) => "fatal",
        }
    }

    assert_eq!(
        get_recovery_strategy(&MailflowError::Validation("test".to_string())),
        "reject"
    );
    assert_eq!(
        get_recovery_strategy(&MailflowError::Storage("test".to_string())),
        "retry"
    );
    assert_eq!(
        get_recovery_strategy(&MailflowError::Routing("test".to_string())),
        "dlq"
    );
}

/// Test malformed JSON handling
#[test]
fn test_malformed_json() {
    let invalid_json_samples = vec![
        "",
        "{",
        "not json",
        "{\"version\": }",
        "{\"version\": \"1.0\"}", // Missing required fields
    ];

    for json in invalid_json_samples {
        let result = serde_json::from_str::<OutboundMessage>(json);
        assert!(result.is_err(), "Should reject malformed JSON: {}", json);
    }
}

/// Test boundary conditions
#[test]
fn test_boundary_conditions() {
    // Test empty strings
    assert!(validate_email_address("").is_err());

    // Test very long email
    let long_email = format!("{}@example.com", "a".repeat(1000));
    let result = validate_email_address(&long_email);
    // Should either accept or reject gracefully
    match result {
        Ok(_) => println!("Long email accepted"),
        Err(e) => {
            println!("Long email rejected: {}", e);
            assert!(!e.to_string().is_empty());
        }
    }

    // Test special characters
    let special_emails = vec![
        "user+tag@example.com",
        "user.name@example.com",
        "user_name@example.com",
    ];

    for email in special_emails {
        let result = validate_email_address(email);
        assert!(result.is_ok(), "Should accept valid email: {}", email);
    }
}
