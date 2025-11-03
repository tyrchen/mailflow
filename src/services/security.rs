/// Security validation service for email verification and enforcement
use crate::error::MailflowError;
use crate::models::{SecurityConfig, SesEventRecord};
use tracing::{info, warn};

/// Security validation service
pub struct SecurityValidator {
    security_config: SecurityConfig,
}

impl SecurityValidator {
    pub fn new(security_config: SecurityConfig) -> Self {
        Self { security_config }
    }

    /// Validates SES security verdicts against configured requirements
    ///
    /// Returns Ok(()) if email passes all required checks, Err otherwise
    pub fn validate_ses_verdicts(&self, record: &SesEventRecord) -> Result<(), MailflowError> {
        let security_config = &self.security_config;

        // Check SPF if required
        if security_config.require_spf {
            let spf_pass = record
                .ses
                .receipt
                .spf_verdict
                .as_ref()
                .map(|v| v.status == "PASS")
                .unwrap_or(false);

            if !spf_pass {
                warn!(
                    message_id = %record.ses.mail.message_id,
                    "Email failed SPF check"
                );
                return Err(MailflowError::Validation(
                    "Email failed SPF verification".to_string(),
                ));
            }
        }

        // Check DKIM if required
        if security_config.require_dkim {
            let dkim_pass = record
                .ses
                .receipt
                .dkim_verdict
                .as_ref()
                .map(|v| v.status == "PASS")
                .unwrap_or(false);

            if !dkim_pass {
                warn!(
                    message_id = %record.ses.mail.message_id,
                    "Email failed DKIM check"
                );
                return Err(MailflowError::Validation(
                    "Email failed DKIM verification".to_string(),
                ));
            }
        }

        // Check virus/spam verdicts
        if let Some(virus_verdict) = record
            .ses
            .receipt
            .virus_verdict
            .as_ref()
            .filter(|v| v.status != "PASS")
        {
            warn!(
                message_id = %record.ses.mail.message_id,
                verdict = %virus_verdict.status,
                "Email failed virus scan"
            );
            return Err(MailflowError::Validation(
                "Email failed virus scan".to_string(),
            ));
        }

        if let Some(_spam_verdict) = record
            .ses
            .receipt
            .spam_verdict
            .as_ref()
            .filter(|v| v.status == "FAIL")
        {
            warn!(
                message_id = %record.ses.mail.message_id,
                "Email marked as spam"
            );
            // Don't reject spam, but log it
            // Apps can decide what to do based on metadata
        }

        info!(
            message_id = %record.ses.mail.message_id,
            "Email passed security validation"
        );

        Ok(())
    }

    /// Validates email doesn't exceed size limits
    pub fn validate_email_size(&self, size_bytes: usize) -> Result<(), MailflowError> {
        use crate::constants::MAX_EMAIL_SIZE_BYTES;

        if size_bytes > MAX_EMAIL_SIZE_BYTES {
            return Err(MailflowError::Validation(format!(
                "Email size {} exceeds maximum {}",
                size_bytes, MAX_EMAIL_SIZE_BYTES
            )));
        }

        Ok(())
    }

    /// Validates sender email domain against allowlist
    ///
    /// Returns Ok(()) if domain is allowed or allowlist is empty, Err otherwise
    pub fn validate_sender_domain(&self, sender_email: &str) -> Result<(), MailflowError> {
        use crate::utils::logging::redact_email;

        // If allowlist is empty, allow all domains (backward compatible)
        if self.security_config.allowed_sender_domains.is_empty() {
            return Ok(());
        }

        // Extract domain from email address
        let domain = sender_email
            .split('@')
            .nth(1)
            .ok_or_else(|| {
                MailflowError::Validation(format!(
                    "Invalid email address format: {}",
                    redact_email(sender_email)
                ))
            })?
            .to_lowercase();

        // Check if domain is in allowlist (case-insensitive)
        let allowed = self
            .security_config
            .allowed_sender_domains
            .iter()
            .any(|allowed_domain| allowed_domain.to_lowercase() == domain);

        if allowed {
            tracing::debug!(domain = %domain, "Sender domain allowed");
            Ok(())
        } else {
            warn!(
                domain = %domain,
                sender = %redact_email(sender_email),
                "Sender domain not in allowlist"
            );
            Err(MailflowError::Validation(format!(
                "Sender domain '{}' is not in the allowlist",
                domain
            )))
        }
    }

    /// Checks if sender is trusted (future: implement blacklist/whitelist)
    pub fn is_trusted_sender(&self, _sender: &str) -> bool {
        // TODO: Implement sender reputation checking
        // - Check against blacklist in DynamoDB
        // - Check sender domain reputation
        // - Check previous spam scores
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{SesAction, SesMail, SesPayload, SesReceiptFull, Verdict};

    fn create_test_config(require_spf: bool, require_dkim: bool) -> SecurityConfig {
        SecurityConfig {
            require_spf,
            require_dkim,
            require_dmarc: false,
            max_emails_per_sender_per_hour: 100,
            allowed_sender_domains: vec![],
        }
    }

    fn create_test_record(spf_pass: bool, dkim_pass: bool, virus_pass: bool) -> SesEventRecord {
        SesEventRecord {
            event_source: "aws:ses".to_string(),
            event_version: "1.0".to_string(),
            ses: SesPayload {
                mail: SesMail {
                    message_id: "test-123".to_string(),
                    timestamp: "2025-11-01T12:00:00Z".to_string(),
                    source: "sender@example.com".to_string(),
                    destination: vec!["_app1@test.com".to_string()],
                },
                receipt: SesReceiptFull {
                    timestamp: "2025-11-01T12:00:00Z".to_string(),
                    recipients: vec!["_app1@test.com".to_string()],
                    spf_verdict: Some(Verdict {
                        status: if spf_pass {
                            "PASS".to_string()
                        } else {
                            "FAIL".to_string()
                        },
                    }),
                    dkim_verdict: Some(Verdict {
                        status: if dkim_pass {
                            "PASS".to_string()
                        } else {
                            "FAIL".to_string()
                        },
                    }),
                    spam_verdict: Some(Verdict {
                        status: "PASS".to_string(),
                    }),
                    virus_verdict: Some(Verdict {
                        status: if virus_pass {
                            "PASS".to_string()
                        } else {
                            "FAIL".to_string()
                        },
                    }),
                    action: SesAction {
                        action_type: "Lambda".to_string(),
                        bucket_name: Some("test-bucket".to_string()),
                        object_key: Some("test-key".to_string()),
                    },
                },
            },
        }
    }

    #[test]
    fn test_spf_validation_pass() {
        let config = create_test_config(true, false);
        let validator = SecurityValidator::new(config);
        let record = create_test_record(true, false, true);

        assert!(validator.validate_ses_verdicts(&record).is_ok());
    }

    #[test]
    fn test_spf_validation_fail() {
        let config = create_test_config(true, false);
        let validator = SecurityValidator::new(config);
        let record = create_test_record(false, false, true);

        assert!(validator.validate_ses_verdicts(&record).is_err());
    }

    #[test]
    fn test_dkim_validation_pass() {
        let config = create_test_config(false, true);
        let validator = SecurityValidator::new(config);
        let record = create_test_record(false, true, true);

        assert!(validator.validate_ses_verdicts(&record).is_ok());
    }

    #[test]
    fn test_virus_scan_fail() {
        let config = create_test_config(false, false);
        let validator = SecurityValidator::new(config);
        let record = create_test_record(true, true, false);

        assert!(validator.validate_ses_verdicts(&record).is_err());
    }

    #[test]
    fn test_email_size_validation() {
        let config = create_test_config(false, false);
        let validator = SecurityValidator::new(config);

        assert!(validator.validate_email_size(1024).is_ok());
        assert!(validator.validate_email_size(40 * 1024 * 1024).is_ok());
        assert!(validator.validate_email_size(50 * 1024 * 1024).is_err());
    }

    #[test]
    fn test_validate_sender_domain_allowed() {
        let mut config = create_test_config(false, false);
        config.allowed_sender_domains = vec!["abc.com".to_string()];
        let validator = SecurityValidator::new(config);

        assert!(validator.validate_sender_domain("user@abc.com").is_ok());
    }

    #[test]
    fn test_validate_sender_domain_blocked() {
        let mut config = create_test_config(false, false);
        config.allowed_sender_domains = vec!["abc.com".to_string()];
        let validator = SecurityValidator::new(config);

        let result = validator.validate_sender_domain("user@blocked.com");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not in the allowlist"));
    }

    #[test]
    fn test_validate_sender_domain_case_insensitive() {
        let mut config = create_test_config(false, false);
        config.allowed_sender_domains = vec!["ABC.COM".to_string()];
        let validator = SecurityValidator::new(config);

        assert!(validator.validate_sender_domain("user@abc.com").is_ok());
        assert!(validator.validate_sender_domain("user@ABC.COM").is_ok());
        assert!(validator.validate_sender_domain("user@AbC.cOm").is_ok());
    }

    #[test]
    fn test_validate_sender_domain_empty_allowlist() {
        let config = create_test_config(false, false);
        let validator = SecurityValidator::new(config);

        // Empty allowlist should allow all domains
        assert!(
            validator
                .validate_sender_domain("user@any-domain.com")
                .is_ok()
        );
    }

    #[test]
    fn test_validate_sender_domain_invalid_email() {
        let mut config = create_test_config(false, false);
        config.allowed_sender_domains = vec!["abc.com".to_string()];
        let validator = SecurityValidator::new(config);

        let result = validator.validate_sender_domain("invalid-email");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Invalid email address format"));
    }

    #[test]
    fn test_validate_sender_domain_multiple_allowed() {
        let mut config = create_test_config(false, false);
        config.allowed_sender_domains = vec![
            "abc.com".to_string(),
            "example.org".to_string(),
            "test.net".to_string(),
        ];
        let validator = SecurityValidator::new(config);

        assert!(validator.validate_sender_domain("user@abc.com").is_ok());
        assert!(validator.validate_sender_domain("user@example.org").is_ok());
        assert!(validator.validate_sender_domain("user@test.net").is_ok());
        assert!(
            validator
                .validate_sender_domain("user@blocked.com")
                .is_err()
        );
    }
}
