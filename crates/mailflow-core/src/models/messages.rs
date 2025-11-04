/// Message schemas for inbound and outbound email processing
use super::email::{Attachment, EmailAddress, EmailBody, EmailHeaders};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Message sent to app queues (inbound)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    pub version: String,
    pub message_id: String,
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub email: InboundEmail,
    pub metadata: MessageMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundEmail {
    pub message_id: String,
    pub from: EmailAddress,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<EmailAddress>,
    pub subject: String,
    pub body: EmailBody,
    pub attachments: Vec<Attachment>,
    pub headers: EmailHeaders,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub routing_key: String,
    pub domain: String,
    #[serde(default)]
    pub spam_score: f32,
    #[serde(default)]
    pub dkim_verified: bool,
    #[serde(default)]
    pub spf_verified: bool,
}

/// Message received from outbound queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    pub version: String,
    pub correlation_id: String,
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub email: OutboundEmail,
    #[serde(default)]
    pub options: SendOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundEmail {
    pub from: EmailAddress,
    pub to: Vec<EmailAddress>,
    #[serde(default)]
    pub cc: Vec<EmailAddress>,
    #[serde(default)]
    pub bcc: Vec<EmailAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<EmailAddress>,
    pub subject: String,
    pub body: EmailBody,
    #[serde(default)]
    pub attachments: Vec<OutboundAttachment>,
    #[serde(default)]
    pub headers: EmailHeaders,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundAttachment {
    pub filename: String,
    pub content_type: String,
    pub s3_bucket: String,
    pub s3_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendOptions {
    #[serde(default = "default_priority")]
    pub priority: Priority,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_send_time: Option<DateTime<Utc>>,
    #[serde(default)]
    pub track_opens: bool,
    #[serde(default)]
    pub track_clicks: bool,
}

impl Default for SendOptions {
    fn default() -> Self {
        Self {
            priority: Priority::Normal,
            scheduled_send_time: None,
            track_opens: false,
            track_clicks: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    High,
    Normal,
    Low,
}

fn default_priority() -> Priority {
    Priority::Normal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inbound_message_serialization() {
        let msg = InboundMessage {
            version: "1.0".to_string(),
            message_id: "test-123".to_string(),
            timestamp: Utc::now(),
            source: "mailflow".to_string(),
            email: InboundEmail {
                message_id: "email-123".to_string(),
                from: EmailAddress {
                    address: "sender@example.com".to_string(),
                    name: Some("Sender".to_string()),
                },
                to: vec![],
                cc: vec![],
                reply_to: None,
                subject: "Test".to_string(),
                body: EmailBody {
                    text: Some("Body".to_string()),
                    html: None,
                },
                attachments: vec![],
                headers: EmailHeaders::default(),
                received_at: Utc::now(),
            },
            metadata: MessageMetadata {
                routing_key: "app1".to_string(),
                domain: "acme.com".to_string(),
                spam_score: 0.0,
                dkim_verified: true,
                spf_verified: true,
            },
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: InboundMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(msg.message_id, deserialized.message_id);
    }

    #[test]
    fn test_priority_serialization() {
        assert_eq!(
            serde_json::to_string(&Priority::Normal).unwrap(),
            "\"normal\""
        );
        assert_eq!(serde_json::to_string(&Priority::High).unwrap(), "\"high\"");
    }
}
