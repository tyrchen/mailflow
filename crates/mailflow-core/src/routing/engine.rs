/// Routing engine
use crate::error::MailflowError;
use crate::models::{Email, MailflowConfig};
use crate::routing::{RouteDestination, extract_app_name, resolver::QueueResolver};
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;

#[async_trait]
pub trait Router: Send + Sync {
    async fn route(&self, email: &Email) -> Result<Vec<RouteDestination>, MailflowError>;
}

pub struct MailflowRouter {
    resolver: Arc<QueueResolver>,
}

impl MailflowRouter {
    pub fn new(config: MailflowConfig) -> Self {
        Self {
            resolver: Arc::new(QueueResolver::new(config)),
        }
    }

    /// Extract all app names from recipient addresses
    fn extract_app_names(email: &Email) -> HashSet<String> {
        email
            .to
            .iter()
            .chain(email.cc.iter())
            .chain(email.bcc.iter())
            .filter_map(|addr| extract_app_name(&addr.address))
            .collect()
    }
}

#[async_trait]
impl Router for MailflowRouter {
    async fn route(&self, email: &Email) -> Result<Vec<RouteDestination>, MailflowError> {
        let app_names = Self::extract_app_names(email);

        if app_names.is_empty() {
            // No app addresses found, route to default queue
            tracing::info!("No app addresses found, routing to default queue");
            return Ok(vec![RouteDestination {
                app_name: "default".to_string(),
                queue_url: self.resolver.default_queue().to_string(),
            }]);
        }

        let mut destinations = Vec::new();

        for app_name in app_names {
            match self.resolver.resolve(&app_name) {
                Ok(queue_url) => {
                    tracing::info!("Routing to app '{}': {}", app_name, queue_url);
                    destinations.push(RouteDestination {
                        app_name: app_name.clone(),
                        queue_url,
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to resolve queue for app '{}': {}", app_name, e);
                    // Route to default queue as fallback
                    destinations.push(RouteDestination {
                        app_name: "default".to_string(),
                        queue_url: self.resolver.default_queue().to_string(),
                    });
                }
            }
        }

        Ok(destinations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        AppRouting, AttachmentConfig, EmailAddress, RetentionConfig, SecurityConfig,
    };
    use chrono::Utc;
    use std::collections::HashMap;

    fn create_test_config() -> MailflowConfig {
        let mut routing = HashMap::new();
        routing.insert(
            "app1".to_string(),
            AppRouting {
                queue_url: "https://sqs.example.com/app1".to_string(),
                enabled: true,
                aliases: vec![],
            },
        );

        MailflowConfig {
            version: "1.0".to_string(),
            domains: vec!["acme.com".to_string()],
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
                allowed_sender_domains: vec![],
            },
            retention: RetentionConfig {
                raw_emails: 7,
                attachments: 30,
                logs: 30,
            },
        }
    }

    #[tokio::test]
    async fn test_route_to_app() {
        let config = create_test_config();
        let router = MailflowRouter::new(config);

        let email = Email {
            message_id: "test".to_string(),
            from: EmailAddress {
                address: "sender@example.com".to_string(),
                name: None,
            },
            to: vec![EmailAddress {
                address: "_app1@acme.com".to_string(),
                name: None,
            }],
            cc: vec![],
            bcc: vec![],
            reply_to: None,
            subject: "Test".to_string(),
            body: Default::default(),
            attachments: vec![],
            attachments_data: vec![],
            headers: Default::default(),
            received_at: Utc::now(),
        };

        let routes = router.route(&email).await.unwrap();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].app_name, "app1");
    }

    #[tokio::test]
    async fn test_route_no_app_addresses() {
        let config = create_test_config();
        let router = MailflowRouter::new(config);

        let email = Email {
            message_id: "test".to_string(),
            from: EmailAddress {
                address: "sender@example.com".to_string(),
                name: None,
            },
            to: vec![EmailAddress {
                address: "user@acme.com".to_string(),
                name: None,
            }],
            cc: vec![],
            bcc: vec![],
            reply_to: None,
            subject: "Test".to_string(),
            body: Default::default(),
            attachments: vec![],
            attachments_data: vec![],
            headers: Default::default(),
            received_at: Utc::now(),
        };

        let routes = router.route(&email).await.unwrap();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].app_name, "default");
    }
}
