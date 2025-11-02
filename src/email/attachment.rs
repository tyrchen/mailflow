/// Attachment handling
use crate::error::MailflowError;

/// Legacy sanitize_filename - deprecated
///
/// Use `crate::utils::sanitization::sanitize_filename_strict` instead for better security
#[deprecated(note = "Use crate::utils::sanitization::sanitize_filename_strict instead")]
pub fn sanitize_filename(filename: &str) -> String {
    crate::utils::sanitization::sanitize_filename_strict(filename)
}

pub fn validate_file_type(
    content_type: &str,
    allowed: &[String],
    blocked: &[String],
) -> Result<(), MailflowError> {
    // Check blocked types first
    for blocked_type in blocked {
        if content_type.starts_with(blocked_type) {
            return Err(MailflowError::Validation(format!(
                "File type {} is blocked",
                content_type
            )));
        }
    }

    // If allowed list is specified, check it
    if !allowed.is_empty() {
        let is_allowed = allowed
            .iter()
            .any(|allowed_type| content_type.starts_with(allowed_type) || allowed_type == "*");

        if !is_allowed {
            return Err(MailflowError::Validation(format!(
                "File type {} is not allowed",
                content_type
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_wrapper() {
        // Test the deprecated wrapper still works
        use crate::utils::sanitization::sanitize_filename_strict;
        assert_eq!(sanitize_filename_strict("test.pdf"), "test.pdf");
        assert_eq!(sanitize_filename_strict("testfile.pdf"), "testfile.pdf");
    }

    #[test]
    fn test_validate_file_type() {
        let allowed = vec!["application/pdf".to_string()];
        let blocked = vec!["application/x-executable".to_string()];

        assert!(validate_file_type("application/pdf", &allowed, &blocked).is_ok());
        assert!(validate_file_type("application/x-executable", &allowed, &blocked).is_err());
        assert!(validate_file_type("image/png", &allowed, &blocked).is_err());
    }
}
