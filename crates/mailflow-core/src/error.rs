/// Error types for Mailflow system
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MailflowError {
    #[error("Email parsing error: {0}")]
    EmailParsing(String),

    #[error("Routing error: {0}")]
    Routing(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Queue error: {0}")]
    Queue(String),

    #[error("SES error: {0}")]
    Ses(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Idempotency error: {0}")]
    Idempotency(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    #[error("Lambda runtime error: {0}")]
    Lambda(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl MailflowError {
    /// Determines if an error is retriable
    pub fn is_retriable(&self) -> bool {
        match self {
            Self::Storage(_) => true,
            Self::Queue(_) => true,
            Self::Ses(_) => true, // Some SES errors are retriable
            Self::Config(_) => false,
            Self::Validation(_) => false,
            Self::EmailParsing(_) => false,
            Self::Routing(_) => false,
            Self::Idempotency(_) => true,
            Self::RateLimit(_) => false, // Rate limits are permanent for this request
            Self::Lambda(_) => false,
            Self::Unknown(_) => false,
        }
    }
}

// Implement conversions for common error types
impl From<serde_json::Error> for MailflowError {
    fn from(err: serde_json::Error) -> Self {
        Self::Config(err.to_string())
    }
}

impl From<std::env::VarError> for MailflowError {
    fn from(err: std::env::VarError) -> Self {
        Self::Config(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retriable_errors() {
        assert!(MailflowError::Storage("test".to_string()).is_retriable());
        assert!(MailflowError::Queue("test".to_string()).is_retriable());
        assert!(!MailflowError::Validation("test".to_string()).is_retriable());
    }

    #[test]
    fn test_error_display() {
        let err = MailflowError::EmailParsing("invalid MIME".to_string());
        assert_eq!(err.to_string(), "Email parsing error: invalid MIME");
    }
}
