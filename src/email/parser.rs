/// Email parser using mail-parser crate
use crate::error::MailflowError;
use crate::models::{AttachmentData, Email, EmailAddress, EmailBody, EmailHeaders};
use async_trait::async_trait;
use chrono::Utc;
use mail_parser::{Addr, Address, MessageParser, MimeHeaders, PartType};

#[async_trait]
pub trait EmailParser: Send + Sync {
    async fn parse(&self, raw_email: &[u8]) -> Result<Email, MailflowError>;
}

pub struct MailParserEmailParser;

impl MailParserEmailParser {
    pub fn new() -> Self {
        Self
    }

    fn parse_addr(addr: &Addr) -> EmailAddress {
        EmailAddress {
            address: addr
                .address
                .as_ref()
                .map(|a| a.to_string())
                .unwrap_or_default(),
            name: addr.name.as_ref().map(|n| n.to_string()),
        }
    }

    fn extract_addresses(address: Option<&Address>) -> Vec<EmailAddress> {
        match address {
            Some(Address::List(list)) => list.iter().map(Self::parse_addr).collect(),
            Some(Address::Group(groups)) => groups
                .iter()
                .flat_map(|g| g.addresses.iter())
                .map(Self::parse_addr)
                .collect(),
            None => vec![],
        }
    }

    fn extract_attachments(message: &mail_parser::Message) -> Vec<AttachmentData> {
        let mut attachments = Vec::new();
        let mut inline_image_index = 0;

        for part in message.parts.iter() {
            // Extract traditional attachments (Content-Disposition: attachment)
            if let Some(filename) = part.attachment_name() {
                let content_type = part
                    .content_type()
                    .map(|ct| ct.ctype())
                    .unwrap_or("application/octet-stream")
                    .to_string();

                if let Some(body_data) = Self::get_part_body(part) {
                    attachments.push(AttachmentData {
                        filename: filename.to_string(),
                        content_type,
                        data: body_data,
                    });
                }
            }
            // Extract inline images (Content-Disposition: inline with Content-ID)
            else if part.is_content_type("image", "") {
                // Check if this is an inline image (has Content-ID or disposition: inline)
                if part.content_id().is_some() || Self::is_inline_disposition(part) {
                    let content_type = part
                        .content_type()
                        .map(|ct| format!("{}/{}", ct.ctype(), ct.subtype().unwrap_or("*")))
                        .unwrap_or("image/unknown".to_string());

                    // Generate filename from Content-ID or use generic name
                    let filename = if let Some(content_id) = part.content_id() {
                        format!("inline-{}.dat", content_id.trim_matches(&['<', '>'][..]))
                    } else {
                        inline_image_index += 1;
                        format!("inline-image-{}.dat", inline_image_index)
                    };

                    if let Some(body_data) = Self::get_part_body(part) {
                        tracing::debug!(
                            filename = %filename,
                            content_type = %content_type,
                            size = body_data.len(),
                            "Extracted inline image as attachment"
                        );

                        attachments.push(AttachmentData {
                            filename,
                            content_type,
                            data: body_data,
                        });
                    }
                }
            }
        }

        attachments
    }

    fn is_inline_disposition(part: &mail_parser::MessagePart) -> bool {
        // Check if Content-Disposition header contains "inline"
        // mail_parser doesn't expose as_text(), so we check the attachment_name
        // If no attachment_name, it might be inline
        part.attachment_name().is_none() && part.content_id().is_some()
    }

    fn get_part_body(part: &mail_parser::MessagePart) -> Option<Vec<u8>> {
        match &part.body {
            PartType::Text(text) => Some(text.as_bytes().to_vec()),
            PartType::Html(html) => Some(html.as_bytes().to_vec()),
            PartType::Binary(data) => Some(data.to_vec()),
            PartType::InlineBinary(data) => Some(data.to_vec()),
            _ => None,
        }
    }
}

impl Default for MailParserEmailParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EmailParser for MailParserEmailParser {
    async fn parse(&self, raw_email: &[u8]) -> Result<Email, MailflowError> {
        let message = MessageParser::default()
            .parse(raw_email)
            .ok_or_else(|| MailflowError::EmailParsing("Failed to parse email".to_string()))?;

        // Extract from address
        let from = message
            .from()
            .and_then(|f| f.as_list())
            .and_then(|list| list.first())
            .map(Self::parse_addr)
            .unwrap_or_else(|| EmailAddress {
                address: String::new(),
                name: None,
            });

        // Extract to addresses
        let to = Self::extract_addresses(message.to());

        // Extract cc addresses
        let cc = Self::extract_addresses(message.cc());

        // Extract bcc addresses (usually empty in received emails - SMTP strips them)
        let bcc = Self::extract_addresses(message.bcc());

        // Extract reply-to
        let reply_to = message
            .reply_to()
            .and_then(|rt| rt.as_list())
            .and_then(|list| list.first())
            .map(Self::parse_addr);

        // Extract subject
        let subject = message.subject().unwrap_or_default().to_string();

        // Extract message ID
        let message_id = message
            .message_id()
            .map(|id| id.to_string())
            .unwrap_or_else(|| format!("generated-{}", Utc::now().timestamp()));

        // Extract body
        let text_body = message.body_text(0).map(|t| t.to_string());
        let html_body = message.body_html(0).map(|h| h.to_string());

        let body = EmailBody {
            text: text_body,
            html: html_body,
        };

        // Extract headers for threading
        let in_reply_to = message.in_reply_to().as_text().map(|t| t.to_string());

        let references = message
            .references()
            .as_text_list()
            .map(|list| list.iter().map(|r| r.to_string()).collect())
            .unwrap_or_default();

        let headers = EmailHeaders {
            in_reply_to,
            references,
            custom: Default::default(),
        };

        // Extract raw attachment data from MIME parts
        let attachments_data = Self::extract_attachments(&message);

        Ok(Email {
            message_id,
            from,
            to,
            cc,
            bcc,
            reply_to,
            subject,
            body,
            attachments: vec![], // Will be populated after S3 upload
            attachments_data,    // Raw data for processing
            headers,
            received_at: Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_simple_email() {
        let raw = b"From: sender@example.com\r
To: recipient@example.com\r
Subject: Test\r
\r
Body text";

        let parser = MailParserEmailParser::new();
        let result = parser.parse(raw).await;

        assert!(result.is_ok());
        let email = result.unwrap();
        assert_eq!(email.from.address, "sender@example.com");
        assert_eq!(email.subject, "Test");
    }
}
