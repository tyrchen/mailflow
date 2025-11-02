/// Application constants
///
/// This module contains all hardcoded values used throughout the application.
/// Constants are organized by category for easy maintenance.
// ============================================================================
// Message Format Constants
// ============================================================================
/// Message protocol version for inbound and outbound messages
pub const MESSAGE_VERSION: &str = "1.0";

/// Source identifier for all messages originating from this system
pub const SOURCE_NAME: &str = "mailflow";

/// Prefix for generated message IDs
pub const MESSAGE_ID_PREFIX: &str = "mailflow";

// ============================================================================
// Timing Constants
// ============================================================================

/// Idempotency TTL in seconds (24 hours)
pub const IDEMPOTENCY_TTL_SECONDS: u64 = 86400;

/// SQS long polling wait time in seconds
pub const LONG_POLL_WAIT_SECONDS: i32 = 20;

/// Default presigned URL expiration in seconds (7 days)
pub const DEFAULT_PRESIGNED_URL_EXPIRATION_SECONDS: u64 = 7 * 24 * 60 * 60;

/// Maximum attachment lifetime in seconds (30 days)
pub const MAX_ATTACHMENT_LIFETIME_SECONDS: u64 = 30 * 24 * 60 * 60;

// ============================================================================
// Size Limits
// ============================================================================

/// Maximum email size supported by SES (40 MB)
pub const MAX_EMAIL_SIZE_BYTES: usize = 40 * 1024 * 1024;

/// Maximum size per attachment for inbound (35 MB, leaving room for headers)
pub const MAX_ATTACHMENT_SIZE_BYTES: usize = 35 * 1024 * 1024;

/// Maximum total attachment size for outbound via SES (10 MB)
pub const SES_MAX_ATTACHMENT_SIZE_BYTES: usize = 10 * 1024 * 1024;

/// Maximum number of attachments per email
pub const MAX_ATTACHMENTS_PER_EMAIL: usize = 50;

/// Maximum filename length
pub const MAX_FILENAME_LENGTH: usize = 255;

/// Maximum SQS message size (256 KB)
pub const SQS_MAX_MESSAGE_SIZE_BYTES: usize = 256 * 1024;

/// Maximum email address length (RFC 5321)
pub const MAX_EMAIL_ADDRESS_LENGTH: usize = 320;

/// Maximum subject line length
pub const MAX_SUBJECT_LENGTH: usize = 998;

// ============================================================================
// Retry Configuration
// ============================================================================

/// Maximum number of retries for transient failures
pub const MAX_RETRIES: u32 = 5;

/// Base delay for exponential backoff in milliseconds
pub const RETRY_BASE_DELAY_MS: u64 = 1000;

/// Maximum delay for exponential backoff in milliseconds (5 minutes)
pub const RETRY_MAX_DELAY_MS: u64 = 5 * 60 * 1000;

/// Jitter factor for retry delays (0.0 to 1.0)
pub const RETRY_JITTER_FACTOR: f64 = 0.1;

// ============================================================================
// SES Limits
// ============================================================================

/// Default SES sending rate (emails per second)
/// Note: Actual limit depends on account, check with GetSendQuota
pub const SES_DEFAULT_SEND_RATE: u32 = 14;

/// SES maximum recipients per email
pub const SES_MAX_RECIPIENTS: usize = 50;

// ============================================================================
// Security Constants
// ============================================================================

/// Blocked file extensions for security
pub const BLOCKED_FILE_EXTENSIONS: &[&str] = &[
    "exe", "bat", "cmd", "com", "pif", "scr", "vbs", "js", "jar", "msi", "app", "deb", "rpm",
];

/// Blocked content types for security
pub const BLOCKED_CONTENT_TYPES: &[&str] = &[
    "application/x-executable",
    "application/x-msdownload",
    "application/x-msdos-program",
    "application/x-sh",
    "application/x-shellscript",
];

/// Maximum rate: emails per sender per hour (default)
pub const DEFAULT_MAX_EMAILS_PER_SENDER_PER_HOUR: u32 = 100;

/// Maximum rate: emails per recipient per hour (default)
pub const DEFAULT_MAX_EMAILS_PER_RECIPIENT_PER_HOUR: u32 = 50;

// ============================================================================
// Validation Constants
// ============================================================================

/// Allowed characters in sanitized filenames
pub const FILENAME_SAFE_CHARS: &str =
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-";

/// Email validation regex (RFC 5322 simplified)
pub const EMAIL_REGEX_PATTERN: &str = r"^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$";

// ============================================================================
// Logging & Monitoring
// ============================================================================

/// Metric namespace for CloudWatch
pub const METRICS_NAMESPACE: &str = "Mailflow";

/// Log target for metrics
pub const LOG_TARGET_METRICS: &str = "metrics";

/// Log target for security events
pub const LOG_TARGET_SECURITY: &str = "security";

/// Log target for audit events
pub const LOG_TARGET_AUDIT: &str = "audit";

// ============================================================================
// Cache & Performance
// ============================================================================

/// Queue validation cache TTL in seconds
pub const QUEUE_VALIDATION_CACHE_TTL_SECONDS: u64 = 300;

/// Config refresh interval in seconds
pub const CONFIG_REFRESH_INTERVAL_SECONDS: u64 = 60;

/// Maximum parallel attachment processing workers
pub const MAX_PARALLEL_ATTACHMENT_WORKERS: usize = 4;

// ============================================================================
// Testing Constants
// ============================================================================

#[cfg(test)]
pub mod test_constants {
    /// Test bucket name
    pub const TEST_BUCKET: &str = "test-bucket";

    /// Test queue URL
    pub const TEST_QUEUE_URL: &str = "https://sqs.us-east-1.amazonaws.com/123456789/test-queue";

    /// Test email address
    pub const TEST_EMAIL: &str = "test@example.com";
}
