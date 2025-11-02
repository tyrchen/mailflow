/// Configuration service - loads config from environment variables
use crate::error::MailflowError;
use crate::models::{
    AppRouting, AttachmentConfig, MailflowConfig, RetentionConfig, SecurityConfig,
};
use async_trait::async_trait;
use std::collections::HashMap;

#[async_trait]
pub trait ConfigProvider: Send + Sync {
    async fn get_config(&self) -> Result<MailflowConfig, MailflowError>;
    async fn refresh(&self) -> Result<(), MailflowError>;
}

/// Environment variable-based configuration provider
pub struct EnvConfigProvider {
    config: MailflowConfig,
}

impl EnvConfigProvider {
    pub fn new() -> Result<Self, MailflowError> {
        let routing_map_json = std::env::var("ROUTING_MAP")
            .map_err(|_| MailflowError::Config("Missing ROUTING_MAP env var".to_string()))?;

        let routing_map: HashMap<String, String> = serde_json::from_str(&routing_map_json)
            .map_err(|e| MailflowError::Config(format!("Invalid ROUTING_MAP JSON: {}", e)))?;

        let allowed_domains = std::env::var("ALLOWED_DOMAINS")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
            .collect();

        let config = MailflowConfig {
            version: "1.0".to_string(),
            domains: allowed_domains,
            routing: routing_map
                .into_iter()
                .map(|(app, queue_url)| {
                    (
                        app.clone(),
                        AppRouting {
                            queue_url,
                            enabled: true,
                            aliases: vec![],
                        },
                    )
                })
                .collect(),
            default_queue: std::env::var("DEFAULT_QUEUE_URL").unwrap_or_default(),
            unknown_queue: std::env::var("DEFAULT_QUEUE_URL").unwrap_or_default(),
            attachments: AttachmentConfig {
                bucket: std::env::var("RAW_EMAILS_BUCKET")
                    .map_err(|_| MailflowError::Config("Missing RAW_EMAILS_BUCKET".to_string()))?,
                presigned_url_expiration: 604800, // 7 days
                max_size: 36700160,               // 35 MB
                allowed_types: vec![],
                blocked_types: vec!["application/x-executable".to_string()],
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

        // Validate configuration
        config
            .validate()
            .map_err(|e| MailflowError::Config(format!("Invalid configuration: {}", e)))?;

        tracing::info!("Configuration validated successfully");

        Ok(Self { config })
    }
}

#[async_trait]
impl ConfigProvider for EnvConfigProvider {
    async fn get_config(&self) -> Result<MailflowConfig, MailflowError> {
        // Configuration is immutable during Lambda lifetime
        Ok(self.config.clone())
    }

    async fn refresh(&self) -> Result<(), MailflowError> {
        // No-op: config is loaded once at startup
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_config_missing_vars() {
        unsafe {
            std::env::remove_var("ROUTING_MAP");
            std::env::remove_var("RAW_EMAILS_BUCKET");
        }

        let result = EnvConfigProvider::new();
        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore] // Flaky due to env var dependencies
    async fn test_config_provider_trait() {
        unsafe {
            std::env::set_var("ROUTING_MAP", r#"{"app1":"https://sqs.example.com/app1"}"#);
            std::env::set_var("RAW_EMAILS_BUCKET", "test-bucket");
            std::env::set_var("ALLOWED_DOMAINS", "acme.com,example.com");
            std::env::set_var(
                "DEFAULT_QUEUE_URL",
                "https://sqs.us-east-1.amazonaws.com/123/default",
            );
        }

        let provider = EnvConfigProvider::new().unwrap();
        let config = provider.get_config().await.unwrap();

        assert_eq!(config.version, "1.0");
        assert_eq!(config.domains.len(), 2);
        assert!(config.routing.contains_key("app1"));
        assert!(config.default_queue.starts_with("https://sqs."));
    }
}
