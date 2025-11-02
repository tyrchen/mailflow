/// Input validation utilities
use crate::error::MailflowError;
use regex::Regex;

lazy_static::lazy_static! {
    static ref EMAIL_REGEX: Regex = Regex::new(
        r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"
    ).unwrap();
}

pub fn validate_email_address(email: &str) -> Result<(), MailflowError> {
    if EMAIL_REGEX.is_match(email) {
        Ok(())
    } else {
        Err(MailflowError::Validation(format!(
            "Invalid email address: {}",
            email
        )))
    }
}

pub fn validate_attachment_size(size: usize, max_size: usize) -> Result<(), MailflowError> {
    if size <= max_size {
        Ok(())
    } else {
        Err(MailflowError::Validation(format!(
            "Attachment size {} exceeds maximum {}",
            size, max_size
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_email() {
        assert!(validate_email_address("test@example.com").is_ok());
        assert!(validate_email_address("user+tag@example.co.uk").is_ok());
        assert!(validate_email_address("invalid").is_err());
        assert!(validate_email_address("@example.com").is_err());
    }

    #[test]
    fn test_validate_attachment_size() {
        assert!(validate_attachment_size(1000, 2000).is_ok());
        assert!(validate_attachment_size(3000, 2000).is_err());
    }
}
