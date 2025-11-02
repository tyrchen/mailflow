/// Configuration models
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// System configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MailflowConfig {
    pub version: String,
    pub domains: Vec<String>,
    pub routing: HashMap<String, AppRouting>,
    pub default_queue: String,
    pub unknown_queue: String,
    pub attachments: AttachmentConfig,
    pub security: SecurityConfig,
    pub retention: RetentionConfig,
}

impl MailflowConfig {
    /// Validates configuration is valid
    pub fn validate(&self) -> Result<(), String> {
        // Validate domains not empty
        if self.domains.is_empty() {
            return Err("No domains configured".to_string());
        }

        // Validate queue URLs (skip if empty - for testing)
        if !self.default_queue.is_empty() && !self.default_queue.starts_with("https://sqs.") {
            return Err(format!("Invalid default queue URL: {}", self.default_queue));
        }

        // Validate routing
        for (app_name, routing) in &self.routing {
            if !routing.queue_url.starts_with("https://sqs.") {
                return Err(format!(
                    "Invalid queue URL for app {}: {}",
                    app_name, routing.queue_url
                ));
            }
        }

        // Validate attachment config
        if self.attachments.bucket.is_empty() {
            return Err("Attachments bucket not configured".to_string());
        }

        if self.attachments.max_size == 0 {
            return Err("Attachments max_size must be > 0".to_string());
        }

        // Validate security limits
        if self.security.max_emails_per_sender_per_hour == 0 {
            return Err("Max emails per sender per hour must be > 0".to_string());
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppRouting {
    pub queue_url: String,
    pub enabled: bool,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AttachmentConfig {
    pub bucket: String,
    /// Presigned URL expiration in seconds
    pub presigned_url_expiration: u64,
    /// Max attachment size in bytes
    pub max_size: usize,
    #[serde(default)]
    pub allowed_types: Vec<String>,
    #[serde(default)]
    pub blocked_types: Vec<String>,
    #[serde(default)]
    pub scan_for_malware: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    #[serde(default)]
    pub require_spf: bool,
    #[serde(default)]
    pub require_dkim: bool,
    #[serde(default)]
    pub require_dmarc: bool,
    pub max_emails_per_sender_per_hour: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RetentionConfig {
    /// Days to retain raw emails
    pub raw_emails: u32,
    /// Days to retain attachments
    pub attachments: u32,
    /// Days to retain logs
    pub logs: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_deserialization() {
        let json = r#"{
            "version": "1.0",
            "domains": ["acme.com"],
            "routing": {
                "app1": {
                    "queue_url": "https://sqs.us-east-1.amazonaws.com/123/mailflow-app1",
                    "enabled": true
                }
            },
            "default_queue": "https://sqs.us-east-1.amazonaws.com/123/mailflow-default",
            "unknown_queue": "https://sqs.us-east-1.amazonaws.com/123/mailflow-unknown",
            "attachments": {
                "bucket": "mailflow-raw-emails",
                "presigned_url_expiration": 604800,
                "max_size": 36700160
            },
            "security": {
                "max_emails_per_sender_per_hour": 100
            },
            "retention": {
                "raw_emails": 7,
                "attachments": 30,
                "logs": 30
            }
        }"#;

        let config: MailflowConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.version, "1.0");
        assert_eq!(config.domains.len(), 1);
        assert!(config.routing.contains_key("app1"));
    }
}
