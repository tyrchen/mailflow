/// Test data builders and helpers
use base64::Engine;
use chrono::Utc;
use serde_json::json;

/// Build a simple test email message
pub fn build_simple_email(from: &str, to: &str, subject: &str, body: &str) -> String {
    format!(
        "From: {}\r\nTo: {}\r\nSubject: {}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\n{}",
        from, to, subject, body
    )
}

/// Build an HTML email message
pub fn build_html_email(from: &str, to: &str, subject: &str, text: &str, html: &str) -> String {
    let boundary = "----=_Part_0_123456789.1234567890";
    format!(
        "From: {}\r\n\
         To: {}\r\n\
         Subject: {}\r\n\
         MIME-Version: 1.0\r\n\
         Content-Type: multipart/alternative; boundary=\"{}\"\r\n\
         \r\n\
         --{}\r\n\
         Content-Type: text/plain; charset=UTF-8\r\n\
         \r\n\
         {}\r\n\
         \r\n\
         --{}\r\n\
         Content-Type: text/html; charset=UTF-8\r\n\
         \r\n\
         {}\r\n\
         \r\n\
         --{}--\r\n",
        from, to, subject, boundary, boundary, text, boundary, html, boundary
    )
}

/// Build an email with attachment
pub fn build_email_with_attachment(
    from: &str,
    to: &str,
    subject: &str,
    body: &str,
    filename: &str,
    content_type: &str,
    attachment_data: &[u8],
) -> String {
    let boundary = "----=_Part_0_987654321.1234567890";
    let encoded = base64::engine::general_purpose::STANDARD.encode(attachment_data);

    format!(
        "From: {}\r\n\
         To: {}\r\n\
         Subject: {}\r\n\
         MIME-Version: 1.0\r\n\
         Content-Type: multipart/mixed; boundary=\"{}\"\r\n\
         \r\n\
         --{}\r\n\
         Content-Type: text/plain; charset=UTF-8\r\n\
         \r\n\
         {}\r\n\
         \r\n\
         --{}\r\n\
         Content-Type: {}; name=\"{}\"\r\n\
         Content-Disposition: attachment; filename=\"{}\"\r\n\
         Content-Transfer-Encoding: base64\r\n\
         \r\n\
         {}\r\n\
         \r\n\
         --{}--\r\n",
        from,
        to,
        subject,
        boundary,
        boundary,
        body,
        boundary,
        content_type,
        filename,
        filename,
        encoded,
        boundary
    )
}

/// Build an outbound message JSON
pub fn build_outbound_message(
    from: &str,
    to: &str,
    subject: &str,
    body_text: &str,
    body_html: Option<&str>,
) -> serde_json::Value {
    json!({
        "version": "1.0",
        "correlationId": format!("test-{}", uuid::Uuid::new_v4()),
        "timestamp": Utc::now().to_rfc3339(),
        "source": "test-suite",
        "email": {
            "from": {
                "address": from,
                "name": "Test Sender"
            },
            "to": [{
                "address": to,
                "name": "Test Recipient"
            }],
            "cc": [],
            "bcc": [],
            "replyTo": {
                "address": from
            },
            "subject": subject,
            "body": {
                "text": body_text,
                "html": body_html.unwrap_or("")
            },
            "attachments": [],
            "headers": {
                "inReplyTo": null,
                "references": []
            }
        },
        "options": {
            "priority": "normal",
            "scheduledSendTime": null,
            "trackOpens": false,
            "trackClicks": false
        }
    })
}

/// Build an outbound message with attachments
pub fn build_outbound_message_with_attachments(
    from: &str,
    to: &str,
    subject: &str,
    body_text: &str,
    attachments: Vec<serde_json::Value>,
) -> serde_json::Value {
    json!({
        "version": "1.0",
        "correlationId": format!("test-{}", uuid::Uuid::new_v4()),
        "timestamp": Utc::now().to_rfc3339(),
        "source": "test-suite",
        "email": {
            "from": {
                "address": from,
                "name": "Test Sender"
            },
            "to": [{
                "address": to,
                "name": "Test Recipient"
            }],
            "cc": [],
            "bcc": [],
            "replyTo": {
                "address": from
            },
            "subject": subject,
            "body": {
                "text": body_text,
                "html": ""
            },
            "attachments": attachments,
            "headers": {
                "inReplyTo": null,
                "references": []
            }
        },
        "options": {
            "priority": "normal",
            "scheduledSendTime": null,
            "trackOpens": false,
            "trackClicks": false
        }
    })
}

/// Build an attachment reference for outbound messages
pub fn build_attachment_ref(
    s3_bucket: &str,
    s3_key: &str,
    filename: &str,
    content_type: &str,
    size: usize,
) -> serde_json::Value {
    json!({
        "filename": filename,
        "contentType": content_type,
        "size": size,
        "s3Bucket": s3_bucket,
        "s3Key": s3_key
    })
}

/// Generate test PDF content
pub fn generate_test_pdf() -> Vec<u8> {
    // Minimal valid PDF
    b"%PDF-1.4\n\
      1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n\
      2 0 obj<</Type/Pages/Count 1/Kids[3 0 R]>>endobj\n\
      3 0 obj<</Type/Page/Parent 2 0 R/Resources<<>>/MediaBox[0 0 612 792]>>endobj\n\
      xref\n\
      0 4\n\
      0000000000 65535 f\n\
      0000000009 00000 n\n\
      0000000056 00000 n\n\
      0000000115 00000 n\n\
      trailer<</Size 4/Root 1 0 R>>\n\
      startxref\n\
      203\n\
      %%EOF\n"
        .to_vec()
}

/// Generate test image content (1x1 PNG)
pub fn generate_test_png() -> Vec<u8> {
    // Minimal 1x1 PNG
    vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15,
        0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01,
        0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
        0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ]
}

/// Generate test JPEG content
pub fn generate_test_jpeg() -> Vec<u8> {
    // Minimal valid JPEG
    vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00,
        0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0x08, 0x06, 0x06, 0x07, 0x06,
        0x05, 0x08, 0x07, 0x07, 0x07, 0x09, 0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B, 0x0B,
        0x0C, 0x19, 0x12, 0x13, 0x0F, 0xFF, 0xD9,
    ]
}

/// Generate large binary data for size testing
pub fn generate_large_binary(size_mb: usize) -> Vec<u8> {
    vec![0x42; size_mb * 1024 * 1024]
}

/// Generate executable-like data (for blocked file testing)
pub fn generate_fake_exe() -> Vec<u8> {
    // PE header signature
    let mut data = vec![0x4D, 0x5A]; // MZ signature
    data.extend_from_slice(b"This is not a real executable");
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_simple_email() {
        let email = build_simple_email(
            "test@example.com",
            "recipient@example.com",
            "Test Subject",
            "Test Body",
        );
        assert!(email.contains("From: test@example.com"));
        assert!(email.contains("Subject: Test Subject"));
        assert!(email.contains("Test Body"));
    }

    #[test]
    fn test_build_outbound_message() {
        let msg = build_outbound_message(
            "sender@example.com",
            "recipient@example.com",
            "Test",
            "Body",
            None,
        );
        assert_eq!(msg["version"], "1.0");
        assert_eq!(msg["email"]["from"]["address"], "sender@example.com");
        assert_eq!(msg["email"]["subject"], "Test");
    }

    #[test]
    fn test_generate_test_pdf() {
        let pdf = generate_test_pdf();
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.len() > 100);
    }

    #[test]
    fn test_generate_test_png() {
        let png = generate_test_png();
        assert_eq!(
            &png[0..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
    }
}
