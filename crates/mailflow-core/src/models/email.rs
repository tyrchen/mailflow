/// Email domain models
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parsed email representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email {
    pub message_id: String,
    pub from: EmailAddress,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub bcc: Vec<EmailAddress>,
    pub reply_to: Option<EmailAddress>,
    pub subject: String,
    pub body: EmailBody,
    pub attachments: Vec<Attachment>,
    pub headers: EmailHeaders,
    pub received_at: DateTime<Utc>,

    // Transient field - raw attachment data before S3 upload
    #[serde(skip)]
    pub attachments_data: Vec<AttachmentData>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmailAddress {
    pub address: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmailBody {
    pub text: Option<String>,
    pub html: Option<String>,
}

/// Raw attachment data before processing (not serialized to SQS)
#[derive(Debug, Clone)]
pub struct AttachmentData {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub filename: String,
    #[serde(rename = "sanitizedFilename")]
    pub sanitized_filename: String,
    #[serde(rename = "contentType")]
    pub content_type: String,
    pub size: usize,
    #[serde(rename = "s3Bucket")]
    pub s3_bucket: String,
    #[serde(rename = "s3Key")]
    pub s3_key: String,
    #[serde(rename = "presignedUrl")]
    pub presigned_url: String,
    #[serde(rename = "presignedUrlExpiration")]
    pub presigned_url_expiration: DateTime<Utc>,
    #[serde(rename = "checksumMd5", skip_serializing_if = "Option::is_none")]
    pub checksum_md5: Option<String>,
    pub status: AttachmentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentStatus {
    Available,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmailHeaders {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<String>,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(flatten)]
    pub custom: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_address_serialization() {
        let addr = EmailAddress {
            address: "test@example.com".to_string(),
            name: Some("Test User".to_string()),
        };

        let json = serde_json::to_string(&addr).unwrap();
        let deserialized: EmailAddress = serde_json::from_str(&json).unwrap();

        assert_eq!(addr, deserialized);
    }

    #[test]
    fn test_email_body() {
        let body = EmailBody {
            text: Some("Plain text".to_string()),
            html: Some("<p>HTML</p>".to_string()),
        };

        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains("Plain text"));
        assert!(json.contains("<p>HTML</p>"));
    }
}
