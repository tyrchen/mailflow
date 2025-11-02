/// Queue resolver
use crate::error::MailflowError;
use crate::models::MailflowConfig;

pub struct QueueResolver {
    config: MailflowConfig,
}

impl QueueResolver {
    pub fn new(config: MailflowConfig) -> Self {
        Self { config }
    }

    pub fn resolve(&self, app_name: &str) -> Result<String, MailflowError> {
        // Check direct match first
        if let Some(route) = self.config.routing.get(app_name).filter(|r| r.enabled) {
            return Ok(route.queue_url.clone());
        }

        // Check aliases
        for (canonical_app, route) in &self.config.routing {
            if route.enabled && route.aliases.contains(&app_name.to_string()) {
                tracing::debug!(
                    alias = %app_name,
                    canonical = %canonical_app,
                    "Resolved routing alias"
                );
                return Ok(route.queue_url.clone());
            }
        }

        Err(MailflowError::Routing(format!(
            "No queue configured for app: {}",
            app_name
        )))
    }

    pub fn default_queue(&self) -> &str {
        &self.config.default_queue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppRouting, AttachmentConfig, RetentionConfig, SecurityConfig};
    use std::collections::HashMap;

    #[test]
    fn test_queue_resolver() {
        let mut routing = HashMap::new();
        routing.insert(
            "app1".to_string(),
            AppRouting {
                queue_url: "https://sqs.example.com/app1".to_string(),
                enabled: true,
                aliases: vec![],
            },
        );

        let config = MailflowConfig {
            version: "1.0".to_string(),
            domains: vec![],
            routing,
            default_queue: "https://sqs.example.com/default".to_string(),
            unknown_queue: "https://sqs.example.com/unknown".to_string(),
            attachments: AttachmentConfig {
                bucket: "test".to_string(),
                presigned_url_expiration: 3600,
                max_size: 1024,
                allowed_types: vec![],
                blocked_types: vec![],
                scan_for_malware: false,
            },
            security: SecurityConfig {
                require_spf: false,
                require_dkim: false,
                require_dmarc: false,
                max_emails_per_sender_per_hour: 100,
            },
            retention: RetentionConfig {
                raw_emails: 7,
                attachments: 30,
                logs: 30,
            },
        };

        let resolver = QueueResolver::new(config);

        assert!(resolver.resolve("app1").is_ok());
        assert!(resolver.resolve("unknown").is_err());
        assert_eq!(resolver.default_queue(), "https://sqs.example.com/default");
    }
}
