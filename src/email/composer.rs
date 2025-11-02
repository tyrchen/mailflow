/// Email composer using lettre crate
use crate::constants::SES_MAX_ATTACHMENT_SIZE_BYTES;
use crate::error::MailflowError;
use crate::models::OutboundEmail;
use crate::utils::retry::{RetryConfig, retry_with_backoff};
use async_trait::async_trait;
use aws_sdk_s3::Client as S3Client;
use lettre::message::{Attachment, Mailbox, Message, MultiPart, SinglePart};
use std::str::FromStr;

#[async_trait]
pub trait EmailComposer: Send + Sync {
    async fn compose(&self, email: &OutboundEmail) -> Result<Vec<u8>, MailflowError>;
}

pub struct LettreEmailComposer {
    s3_client: S3Client,
}

impl LettreEmailComposer {
    pub fn new(s3_client: S3Client) -> Self {
        Self { s3_client }
    }

    fn to_mailbox(addr: &crate::models::EmailAddress) -> Result<Mailbox, MailflowError> {
        let mailbox = if let Some(name) = &addr.name {
            Mailbox::new(
                Some(name.clone()),
                addr.address.parse().map_err(|e| {
                    MailflowError::EmailParsing(format!("Invalid email address: {}", e))
                })?,
            )
        } else {
            Mailbox::from_str(&addr.address)
                .map_err(|e| MailflowError::EmailParsing(format!("Invalid email address: {}", e)))?
        };
        Ok(mailbox)
    }

    /// Fetch attachment data from S3 with retry logic
    async fn fetch_attachment_from_s3(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<Vec<u8>, MailflowError> {
        tracing::debug!(bucket = %bucket, key = %key, "Fetching attachment from S3");

        let bucket_owned = bucket.to_string();
        let key_owned = key.to_string();

        let output = retry_with_backoff(
            || {
                let client = self.s3_client.clone();
                let bucket = bucket_owned.clone();
                let key = key_owned.clone();

                async move {
                    client
                        .get_object()
                        .bucket(bucket.clone())
                        .key(key.clone())
                        .send()
                        .await
                        .map_err(|e| {
                            MailflowError::Storage(format!(
                                "Failed to fetch attachment from s3://{}/{}: {}",
                                bucket, key, e
                            ))
                        })
                }
            },
            RetryConfig::default(),
            "s3_get_attachment",
        )
        .await?;

        let bytes = output
            .body
            .collect()
            .await
            .map_err(|e| {
                MailflowError::Storage(format!(
                    "Failed to read attachment body from s3://{}/{}: {}",
                    bucket, key, e
                ))
            })?
            .into_bytes();

        let data = bytes.to_vec();
        tracing::debug!(
            bucket = %bucket,
            key = %key,
            size = data.len(),
            "Successfully fetched attachment from S3"
        );

        Ok(data)
    }
}

// Note: No Default implementation since S3Client is required

#[async_trait]
impl EmailComposer for LettreEmailComposer {
    async fn compose(&self, email: &OutboundEmail) -> Result<Vec<u8>, MailflowError> {
        let mut message_builder = Message::builder()
            .from(Self::to_mailbox(&email.from)?)
            .subject(&email.subject);

        // Add To recipients
        for to in &email.to {
            message_builder = message_builder.to(Self::to_mailbox(to)?);
        }

        // Add CC recipients
        for cc in &email.cc {
            message_builder = message_builder.cc(Self::to_mailbox(cc)?);
        }

        // Add BCC recipients
        for bcc in &email.bcc {
            message_builder = message_builder.bcc(Self::to_mailbox(bcc)?);
        }

        // Add Reply-To if present
        if let Some(reply_to) = &email.reply_to {
            message_builder = message_builder.reply_to(Self::to_mailbox(reply_to)?);
        }

        // Note: Threading headers (In-Reply-To, References) are not directly supported by lettre
        // They would need to be added manually after message creation or via custom Header impl
        // For now, logging their presence for awareness
        if email.headers.in_reply_to.is_some() || !email.headers.references.is_empty() {
            tracing::debug!(
                "Email has threading headers (In-Reply-To: {:?}, References: {} items)",
                email.headers.in_reply_to,
                email.headers.references.len()
            );
            // Enhancement: Could add via post-processing of formatted() output
        }

        // Build message body with attachments support
        let message = if email.attachments.is_empty() {
            // No attachments - build simple message
            match (&email.body.text, &email.body.html) {
                (Some(text), Some(html)) => {
                    // Both text and HTML - create multipart/alternative
                    message_builder
                        .multipart(
                            MultiPart::alternative()
                                .singlepart(SinglePart::plain(text.clone()))
                                .singlepart(SinglePart::html(html.clone())),
                        )
                        .map_err(|e| {
                            MailflowError::EmailParsing(format!(
                                "Failed to build multipart message: {}",
                                e
                            ))
                        })?
                }
                (Some(text), None) => {
                    // Text only
                    message_builder.body(text.clone()).map_err(|e| {
                        MailflowError::EmailParsing(format!("Failed to build text message: {}", e))
                    })?
                }
                (None, Some(html)) => {
                    // HTML only
                    message_builder
                        .singlepart(SinglePart::html(html.clone()))
                        .map_err(|e| {
                            MailflowError::EmailParsing(format!(
                                "Failed to build HTML message: {}",
                                e
                            ))
                        })?
                }
                (None, None) => {
                    // No body
                    message_builder.body(String::new()).map_err(|e| {
                        MailflowError::EmailParsing(format!("Failed to build empty message: {}", e))
                    })?
                }
            }
        } else {
            // Has attachments - build multipart/mixed
            tracing::info!(
                attachment_count = email.attachments.len(),
                "Building email with attachments"
            );

            // Fetch all attachments from S3 and validate total size
            let mut total_size = 0u64;
            let mut attachment_data = Vec::new();

            for attachment_ref in &email.attachments {
                tracing::debug!(
                    filename = %attachment_ref.filename,
                    bucket = %attachment_ref.s3_bucket,
                    key = %attachment_ref.s3_key,
                    "Fetching attachment from S3"
                );

                let data = self
                    .fetch_attachment_from_s3(&attachment_ref.s3_bucket, &attachment_ref.s3_key)
                    .await?;

                total_size += data.len() as u64;
                attachment_data.push((attachment_ref, data));
            }

            // Validate total size (SES limit is 10 MB for outbound)
            if total_size > SES_MAX_ATTACHMENT_SIZE_BYTES as u64 {
                return Err(MailflowError::Validation(format!(
                    "Total attachment size {} bytes exceeds SES limit of {} bytes (10 MB)",
                    total_size, SES_MAX_ATTACHMENT_SIZE_BYTES
                )));
            }

            tracing::info!(
                attachment_count = email.attachments.len(),
                total_size = total_size,
                "All attachments fetched successfully, total size within limit"
            );

            // Build multipart/mixed with body + attachments
            // Start with body part
            let mut multipart = match (&email.body.text, &email.body.html) {
                (Some(text), Some(html)) => MultiPart::mixed().multipart(
                    MultiPart::alternative()
                        .singlepart(SinglePart::plain(text.clone()))
                        .singlepart(SinglePart::html(html.clone())),
                ),
                (Some(text), None) => {
                    MultiPart::mixed().singlepart(SinglePart::plain(text.clone()))
                }
                (None, Some(html)) => MultiPart::mixed().singlepart(SinglePart::html(html.clone())),
                (None, None) => MultiPart::mixed().singlepart(SinglePart::plain(String::new())),
            };

            // Add attachments
            for (attachment_ref, data) in attachment_data {
                tracing::debug!(
                    filename = %attachment_ref.filename,
                    size = data.len(),
                    content_type = %attachment_ref.content_type,
                    "Adding attachment to email"
                );

                let attachment = Attachment::new(attachment_ref.filename.clone()).body(
                    data,
                    attachment_ref.content_type.parse().map_err(|e| {
                        MailflowError::EmailParsing(format!(
                            "Invalid content type '{}': {}",
                            attachment_ref.content_type, e
                        ))
                    })?,
                );

                multipart = multipart.singlepart(attachment);
            }

            message_builder.multipart(multipart).map_err(|e| {
                MailflowError::EmailParsing(format!(
                    "Failed to build multipart message with attachments: {}",
                    e
                ))
            })?
        };

        // Convert to raw email bytes
        let mut raw_email = message.formatted();

        // Add threading headers if present (post-process since lettre doesn't support them directly)
        if email.headers.in_reply_to.is_some() || !email.headers.references.is_empty() {
            let email_str = String::from_utf8_lossy(&raw_email);
            let (headers_part, body_part) = if let Some(pos) = email_str.find("\r\n\r\n") {
                (&email_str[..pos], &email_str[pos + 4..])
            } else {
                (email_str.as_ref(), "")
            };

            let mut updated_headers = headers_part.to_string();

            // Add In-Reply-To header
            if let Some(ref in_reply_to) = email.headers.in_reply_to {
                updated_headers.push_str(&format!("\r\nIn-Reply-To: <{}>", in_reply_to));
            }

            // Add References header
            if !email.headers.references.is_empty() {
                let refs = email
                    .headers
                    .references
                    .iter()
                    .map(|r| format!("<{}>", r))
                    .collect::<Vec<_>>()
                    .join(" ");
                updated_headers.push_str(&format!("\r\nReferences: {}", refs));
            }

            // Reconstruct email
            raw_email = format!("{}\r\n\r\n{}", updated_headers, body_part)
                .as_bytes()
                .to_vec();

            tracing::debug!(
                "Added threading headers (In-Reply-To: {:?}, References: {} items)",
                email.headers.in_reply_to,
                email.headers.references.len()
            );
        }

        tracing::info!(
            "Composed email: subject='{}', to={} recipients",
            email.subject,
            email.to.len()
        );

        Ok(raw_email)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{EmailAddress, EmailBody, EmailHeaders, OutboundEmail};

    // Helper function to create a test S3 client
    async fn create_test_s3_client() -> S3Client {
        let config = aws_config::from_env().load().await;
        S3Client::new(&config)
    }

    #[tokio::test]
    async fn test_compose_simple_email() {
        let email = OutboundEmail {
            from: EmailAddress {
                address: "sender@example.com".to_string(),
                name: Some("Sender".to_string()),
            },
            to: vec![EmailAddress {
                address: "recipient@example.com".to_string(),
                name: None,
            }],
            cc: vec![],
            bcc: vec![],
            reply_to: None,
            subject: "Test Subject".to_string(),
            body: EmailBody {
                text: Some("Test body".to_string()),
                html: None,
            },
            attachments: vec![],
            headers: EmailHeaders::default(),
        };

        let s3_client = create_test_s3_client().await;
        let composer = LettreEmailComposer::new(s3_client);
        let result = composer.compose(&email).await;

        assert!(result.is_ok());
        let raw_email = result.unwrap();
        let email_str = String::from_utf8_lossy(&raw_email);

        assert!(email_str.contains("From: Sender <sender@example.com>"));
        assert!(email_str.contains("To: recipient@example.com"));
        assert!(email_str.contains("Subject: Test Subject"));
        assert!(email_str.contains("Test body"));
    }

    #[tokio::test]
    async fn test_compose_multipart_email() {
        let email = OutboundEmail {
            from: EmailAddress {
                address: "sender@example.com".to_string(),
                name: None,
            },
            to: vec![EmailAddress {
                address: "recipient@example.com".to_string(),
                name: None,
            }],
            cc: vec![],
            bcc: vec![],
            reply_to: None,
            subject: "Test".to_string(),
            body: EmailBody {
                text: Some("Plain text".to_string()),
                html: Some("<p>HTML</p>".to_string()),
            },
            attachments: vec![],
            headers: EmailHeaders::default(),
        };

        let s3_client = create_test_s3_client().await;
        let composer = LettreEmailComposer::new(s3_client);
        let result = composer.compose(&email).await;

        assert!(result.is_ok());
        let raw_email = result.unwrap();
        let email_str = String::from_utf8_lossy(&raw_email);

        assert!(email_str.contains("multipart/alternative"));
        assert!(email_str.contains("Plain text"));
        assert!(email_str.contains("<p>HTML</p>"));
    }
}
