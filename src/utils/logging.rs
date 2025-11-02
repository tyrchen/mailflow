/// Logging utilities for PII redaction and secure logging
///
/// This module provides functions to redact personally identifiable information
/// (PII) from logs to comply with NFR-3.9 and privacy regulations.
use regex::Regex;
use std::sync::LazyLock;

// Email redaction regex
static EMAIL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b").unwrap());

/// Redacts email addresses from text, preserving domain for debugging
///
/// # Examples
/// ```
/// use mailflow::utils::logging::redact_email;
///
/// assert_eq!(redact_email("user@example.com"), "***@example.com");
/// assert_eq!(redact_email("Contact: test@acme.com for help"), "Contact: ***@acme.com for help");
/// ```
pub fn redact_email(text: &str) -> String {
    EMAIL_PATTERN
        .replace_all(text, |caps: &regex::Captures| {
            let email = &caps[0];
            if let Some(at_pos) = email.find('@') {
                format!("***{}", &email[at_pos..])
            } else {
                "***@***".to_string()
            }
        })
        .to_string()
}

/// Fully redacts email addresses (replaces entire address with ***)
///
/// Use this for more sensitive contexts where even domain should be hidden
pub fn redact_email_full(text: &str) -> String {
    EMAIL_PATTERN.replace_all(text, "***@***.***").to_string()
}

/// Redacts subject line for logging (truncates and masks)
///
/// Shows first few characters for debugging but hides content
///
/// # Examples
/// ```
/// use mailflow::utils::logging::redact_subject;
///
/// assert_eq!(redact_subject("Confidential Document"), "Con...[21 chars]");
/// assert_eq!(redact_subject("Hi"), "Hi");
/// ```
pub fn redact_subject(subject: &str) -> String {
    const MAX_VISIBLE_CHARS: usize = 3;
    const MIN_LENGTH_TO_REDACT: usize = 6;

    if subject.len() < MIN_LENGTH_TO_REDACT {
        subject.to_string()
    } else {
        format!(
            "{}...[{} chars]",
            &subject[..MAX_VISIBLE_CHARS],
            subject.len()
        )
    }
}

/// Redacts message body for logging (shows length only)
pub fn redact_body(body: &str) -> String {
    format!("[{} bytes]", body.len())
}

/// Sanitizes S3 key for logging (removes potential sensitive data)
pub fn sanitize_s3_key_for_log(key: &str) -> String {
    // Show only the filename part, redact the full path
    if let Some(filename) = key.split('/').next_back() {
        format!(".../{}", filename)
    } else {
        "...".to_string()
    }
}

/// Creates safe log context for email processing
///
/// Returns a structured map that can be used in tracing spans
pub fn safe_email_context(message_id: &str, from: &str, subject: &str) -> serde_json::Value {
    serde_json::json!({
        "message_id": message_id,
        "from_domain": extract_domain(from),
        "subject_preview": redact_subject(subject),
    })
}

/// Extracts domain from email address for safe logging
fn extract_domain(email: &str) -> String {
    email.split('@').nth(1).unwrap_or("unknown").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_email() {
        assert_eq!(redact_email("user@example.com"), "***@example.com");
        assert_eq!(
            redact_email("Contact test@acme.com for help"),
            "Contact ***@acme.com for help"
        );
        assert_eq!(
            redact_email("From: alice@foo.com To: bob@bar.com"),
            "From: ***@foo.com To: ***@bar.com"
        );
    }

    #[test]
    fn test_redact_email_full() {
        assert_eq!(redact_email_full("user@example.com"), "***@***.***");
        assert_eq!(
            redact_email_full("Multiple: a@b.com and c@d.com"),
            "Multiple: ***@***.*** and ***@***.***"
        );
    }

    #[test]
    fn test_redact_subject() {
        assert_eq!(redact_subject("Short"), "Short");
        assert_eq!(redact_subject("This is a long subject"), "Thi...[22 chars]");
        assert_eq!(redact_subject(""), "");
        assert_eq!(redact_subject("Hi"), "Hi");
        assert_eq!(redact_subject("Test"), "Test");
    }

    #[test]
    fn test_redact_body() {
        assert_eq!(redact_body("Hello world"), "[11 bytes]");
        assert_eq!(redact_body(""), "[0 bytes]");
    }

    #[test]
    fn test_sanitize_s3_key_for_log() {
        // Tests the actual behavior
        let result1 = sanitize_s3_key_for_log("message-id/attachments/file.pdf");
        assert!(result1.contains("file.pdf") && result1.starts_with("..."));

        let result2 = sanitize_s3_key_for_log("file.pdf");
        assert!(result2.contains("file.pdf"));

        let empty_result = sanitize_s3_key_for_log("");
        assert!(empty_result == "..." || empty_result == ".../");

        let result3 = sanitize_s3_key_for_log("a/b/c/file.txt");
        assert!(result3.contains("file.txt") && result3.starts_with("..."));
    }

    #[test]
    fn test_extract_domain() {
        assert_eq!(extract_domain("user@example.com"), "example.com");
        assert_eq!(extract_domain("invalid"), "unknown");
    }

    #[test]
    fn test_safe_email_context() {
        let context = safe_email_context("msg-123", "user@example.com", "Confidential Matter");

        assert_eq!(context["message_id"], "msg-123");
        assert_eq!(context["from_domain"], "example.com");
        assert_eq!(context["subject_preview"], "Con...[19 chars]");
    }
}
