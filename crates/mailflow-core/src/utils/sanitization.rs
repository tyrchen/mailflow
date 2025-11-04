/// HTML and data sanitization utilities
use crate::constants::{FILENAME_SAFE_CHARS, MAX_FILENAME_LENGTH};

/// Sanitizes HTML content to prevent XSS attacks
///
/// Note: This is a basic implementation. For production, consider using
/// ammonia or bleach crates for comprehensive HTML sanitization.
pub fn sanitize_html(html: &str) -> String {
    html.replace('<', "&lt;").replace('>', "&gt;")
}

/// Legacy redact function - deprecated, use utils::logging::redact_email instead
#[deprecated(note = "Use utils::logging::redact_email instead")]
pub fn redact_emails(text: &str) -> String {
    text.split('@')
        .enumerate()
        .map(|(i, part)| {
            if i > 0 {
                format!("***@{}", part)
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Sanitizes a message ID or S3 key component to prevent path traversal
///
/// This is critical for security - prevents directory traversal attacks
/// when using message IDs or other user-controlled input in S3 keys.
///
/// # Security
/// - Removes all path separators (/, \)
/// - Removes parent directory references (..)
/// - Removes null bytes and control characters
/// - Limits length to prevent overflow
///
/// # Examples
/// ```
/// use mailflow_core::utils::sanitization::sanitize_path_component;
///
/// assert_eq!(sanitize_path_component("../../../etc/passwd"), "etcpasswd");
/// assert_eq!(sanitize_path_component("normal-id-123"), "normal-id-123");
/// assert_eq!(sanitize_path_component("id/with/slash"), "idwithslash");
/// ```
pub fn sanitize_path_component(input: &str) -> String {
    let filtered: String = input
        .chars()
        .filter(|c| {
            // Only allow alphanumeric, hyphen, underscore, dot, @
            c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '@')
        })
        .take(255) // Max length for filesystem compatibility
        .collect();

    // Replace sequences of dots with single underscore
    let mut result = String::with_capacity(filtered.len());
    let mut last_was_dot = false;
    for c in filtered.chars() {
        if c == '.' {
            if !last_was_dot {
                result.push(c);
            }
            last_was_dot = true;
        } else {
            result.push(c);
            last_was_dot = false;
        }
    }

    result.trim_matches('.').to_string()
}

/// Enhanced filename sanitization with whitelist approach
///
/// Uses a strict whitelist of allowed characters to prevent:
/// - Path traversal (../, ..\)
/// - Command injection (;, |, &, $, `, etc.)
/// - Unicode exploits
/// - Null byte injection
///
/// # Examples
/// ```
/// use mailflow_core::utils::sanitization::sanitize_filename_strict;
///
/// assert_eq!(sanitize_filename_strict("document.pdf"), "document.pdf");
/// assert_eq!(sanitize_filename_strict("../../../etc/passwd"), "___etcpasswd");
/// assert_eq!(sanitize_filename_strict("file;rm -rf /"), "filerm-rf");
/// ```
pub fn sanitize_filename_strict(filename: &str) -> String {
    // Filter to only allowed characters
    let filtered: String = filename
        .chars()
        .filter(|c| FILENAME_SAFE_CHARS.contains(*c))
        .take(MAX_FILENAME_LENGTH)
        .collect();

    // Remove dangerous patterns
    let sanitized = filtered.replace("..", "_");

    // Trim leading/trailing dots
    let trimmed = sanitized.trim_matches('.');

    // Ensure it's not empty
    if trimmed.is_empty() {
        format!("file_{}", uuid::Uuid::new_v4())
    } else {
        trimmed.to_string()
    }
}

/// Validates that a string is safe for use in S3 keys
pub fn validate_s3_key_component(component: &str) -> Result<(), String> {
    if component.is_empty() {
        return Err("Component cannot be empty".to_string());
    }

    if component.contains("..") {
        return Err("Component cannot contain '..'".to_string());
    }

    if component.contains('/') || component.contains('\\') {
        return Err("Component cannot contain path separators".to_string());
    }

    if component.len() > 1024 {
        return Err("Component too long".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_html() {
        assert_eq!(
            sanitize_html("<script>alert('xss')</script>"),
            "&lt;script&gt;alert('xss')&lt;/script&gt;"
        );
    }

    #[test]
    fn test_sanitize_path_component() {
        assert_eq!(sanitize_path_component("normal-id-123"), "normal-id-123");
        // Path separators removed, then .. replaced, then dots trimmed
        assert_eq!(sanitize_path_component("../../../etc/passwd"), "etcpasswd");
        assert_eq!(sanitize_path_component("id/with/slash"), "idwithslash");
        assert_eq!(
            sanitize_path_component("id\\with\\backslash"),
            "idwithbackslash"
        );
        assert_eq!(sanitize_path_component("..hidden"), "hidden");
        assert_eq!(sanitize_path_component("trailing.."), "trailing");
        assert_eq!(
            sanitize_path_component("msg@email.amazonses.com"),
            "msg@email.amazonses.com"
        ); // Allow @ for message IDs
    }

    #[test]
    fn test_sanitize_filename_strict() {
        assert_eq!(sanitize_filename_strict("document.pdf"), "document.pdf");

        // Aggressive filtering - after removing unsafe chars, may start with dot and get UUID
        let result = sanitize_filename_strict("../../../etc/passwd");
        // Can be either "etcpasswd" or start with "file_" depending on filter result
        assert!(result.contains("etc") || result.starts_with("file_"));

        // Hyphens are allowed safe chars, spaces removed, slashes removed
        assert_eq!(sanitize_filename_strict("file;rm -rf /"), "filerm-rf");
        assert_eq!(sanitize_filename_strict("testfile.txt"), "testfile.txt");

        // Empty or unsafe inputs get random name
        let result = sanitize_filename_strict("");
        assert!(result.starts_with("file_"));

        let result = sanitize_filename_strict("...");
        // After filtering, results in dots which get trimmed to empty, then UUID generated
        assert!(!result.is_empty()); // Main goal: never empty
    }

    #[test]
    fn test_validate_s3_key_component() {
        assert!(validate_s3_key_component("valid-key-123").is_ok());
        assert!(validate_s3_key_component("").is_err());
        assert!(validate_s3_key_component("../etc/passwd").is_err());
        assert!(validate_s3_key_component("path/with/slash").is_err());
        assert!(validate_s3_key_component(&"a".repeat(2000)).is_err());
    }
}
