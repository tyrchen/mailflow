/// Attachment processing service
use crate::constants::{MAX_ATTACHMENT_SIZE_BYTES, MAX_ATTACHMENTS_PER_EMAIL};
use crate::error::MailflowError;
use crate::models::{Attachment, AttachmentData, AttachmentStatus};
use crate::services::s3::StorageService;
use crate::utils::sanitization::{sanitize_filename_strict, sanitize_path_component};
use async_trait::async_trait;
use chrono::Utc;
use md5::{Digest as _, Md5};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// Configuration for attachment processing
#[derive(Debug, Clone)]
pub struct AttachmentConfig {
    pub bucket: String,
    pub presigned_url_expiration: Duration,
    pub max_size: usize,
    pub allowed_types: Vec<String>,
    pub blocked_types: Vec<String>,
}

impl AttachmentConfig {
    pub fn from_env() -> Result<Self, MailflowError> {
        let bucket = std::env::var("ATTACHMENTS_BUCKET")
            .unwrap_or_else(|_| std::env::var("RAW_EMAILS_BUCKET").unwrap_or_default());

        let presigned_url_expiration = Duration::from_secs(
            std::env::var("PRESIGNED_URL_EXPIRATION_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(7 * 24 * 60 * 60), // 7 days default
        );

        let max_size = std::env::var("MAX_ATTACHMENT_SIZE_BYTES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(MAX_ATTACHMENT_SIZE_BYTES);

        let allowed_types = std::env::var("ALLOWED_CONTENT_TYPES")
            .unwrap_or_else(|_| "*".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        let blocked_types = std::env::var("BLOCKED_CONTENT_TYPES")
            .unwrap_or_else(|_| "application/x-executable,application/x-msdownload".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        Ok(Self {
            bucket,
            presigned_url_expiration,
            max_size,
            allowed_types,
            blocked_types,
        })
    }
}

#[async_trait]
pub trait AttachmentProcessor: Send + Sync {
    async fn process_attachments(
        &self,
        message_id: &str,
        attachments_data: Vec<AttachmentData>,
    ) -> Result<Vec<Attachment>, MailflowError>;
}

pub struct S3AttachmentProcessor {
    storage: Arc<dyn StorageService>,
    config: AttachmentConfig,
}

impl S3AttachmentProcessor {
    pub fn new(storage: Arc<dyn StorageService>, config: AttachmentConfig) -> Self {
        Self { storage, config }
    }

    async fn process_single_attachment(
        &self,
        message_id: &str,
        data: AttachmentData,
        index: usize,
    ) -> Attachment {
        // Process attachment and handle errors gracefully
        match self.try_process_attachment(message_id, &data, index).await {
            Ok(attachment) => attachment,
            Err(e) => {
                error!("Failed to process attachment {}: {}", data.filename, e);
                // Return failed attachment with error details
                Attachment {
                    filename: data.filename.clone(),
                    sanitized_filename: sanitize_filename_strict(&data.filename),
                    content_type: data.content_type.clone(),
                    size: data.data.len(),
                    s3_bucket: self.config.bucket.clone(),
                    s3_key: String::new(),
                    presigned_url: String::new(),
                    presigned_url_expiration: Utc::now(),
                    checksum_md5: None,
                    status: AttachmentStatus::Failed,
                    error: Some(e.to_string()),
                }
            }
        }
    }

    async fn try_process_attachment(
        &self,
        message_id: &str,
        data: &AttachmentData,
        index: usize,
    ) -> Result<Attachment, MailflowError> {
        // 1. Validate size
        if data.data.len() > self.config.max_size {
            return Err(MailflowError::Validation(format!(
                "Attachment {} exceeds max size of {} bytes (actual: {} bytes)",
                data.filename,
                self.config.max_size,
                data.data.len()
            )));
        }

        // 2. Validate file type using magic bytes + extension
        let validated_mime_type = crate::utils::file_validation::validate_file_type(
            &data.filename,
            &data.data,
            &data.content_type,
        )?;

        tracing::debug!(
            filename = %data.filename,
            declared_type = %data.content_type,
            validated_type = %validated_mime_type,
            "File type validated successfully"
        );

        // 3. Sanitize filename and ensure uniqueness
        let sanitized = sanitize_filename_strict(&data.filename);
        let unique_filename = if index > 0 {
            // Add index to prevent collisions
            let parts: Vec<&str> = sanitized.rsplitn(2, '.').collect();
            if parts.len() == 2 {
                format!("{}-{}.{}", parts[1], index, parts[0])
            } else {
                format!("{}-{}", sanitized, index)
            }
        } else {
            sanitized.clone()
        };

        // 4. Generate S3 key (sanitize message_id to prevent path traversal)
        let safe_message_id = sanitize_path_component(message_id);
        let s3_key = format!("{}/{}", safe_message_id, unique_filename);

        info!(
            "Uploading attachment {} to s3://{}/{}",
            data.filename, self.config.bucket, s3_key
        );

        // 5. Calculate MD5 checksum
        let mut hasher = Md5::new();
        hasher.update(&data.data);
        let checksum_md5 = Some(format!("{:x}", hasher.finalize()));

        // 6. Upload to S3
        self.storage
            .upload(&self.config.bucket, &s3_key, &data.data)
            .await?;

        // 7. Generate presigned URL
        let presigned_url = self
            .storage
            .generate_presigned_url(
                &self.config.bucket,
                &s3_key,
                self.config.presigned_url_expiration,
            )
            .await?;

        // 8. Calculate expiration time
        let expiration = Utc::now()
            + chrono::Duration::from_std(self.config.presigned_url_expiration)
                .map_err(|e| MailflowError::Lambda(format!("Invalid duration: {}", e)))?;

        info!(
            "Successfully processed attachment {} ({} bytes, MD5: {})",
            data.filename,
            data.data.len(),
            checksum_md5.as_ref().unwrap_or(&"none".to_string())
        );

        // 9. Build metadata
        Ok(Attachment {
            filename: data.filename.clone(),
            sanitized_filename: unique_filename,
            content_type: data.content_type.clone(),
            size: data.data.len(),
            s3_bucket: self.config.bucket.clone(),
            s3_key,
            presigned_url,
            presigned_url_expiration: expiration,
            checksum_md5,
            status: AttachmentStatus::Available,
            error: None,
        })
    }
}

#[async_trait]
impl AttachmentProcessor for S3AttachmentProcessor {
    async fn process_attachments(
        &self,
        message_id: &str,
        attachments_data: Vec<AttachmentData>,
    ) -> Result<Vec<Attachment>, MailflowError> {
        if attachments_data.is_empty() {
            return Ok(vec![]);
        }

        // Security: Limit number of attachments to prevent resource exhaustion
        if attachments_data.len() > MAX_ATTACHMENTS_PER_EMAIL {
            return Err(MailflowError::Validation(format!(
                "Too many attachments: {} exceeds maximum of {}",
                attachments_data.len(),
                MAX_ATTACHMENTS_PER_EMAIL
            )));
        }

        info!(
            "Processing {} attachment(s) for message {}",
            attachments_data.len(),
            message_id
        );

        // Process attachments sequentially to avoid memory issues
        // For parallel processing, use join_all
        let mut attachments = Vec::new();
        for (index, data) in attachments_data.into_iter().enumerate() {
            let attachment = self
                .process_single_attachment(message_id, data, index)
                .await;
            attachments.push(attachment);
        }

        let successful = attachments
            .iter()
            .filter(|a| matches!(a.status, AttachmentStatus::Available))
            .count();

        let failed = attachments.len() - successful;

        if failed > 0 {
            warn!(
                "Processed {} attachments for message {}: {} successful, {} failed",
                attachments.len(),
                message_id,
                successful,
                failed
            );
        } else {
            info!(
                "Successfully processed all {} attachment(s) for message {}",
                attachments.len(),
                message_id
            );
        }

        Ok(attachments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attachment_config_from_env() {
        // Set test environment variables
        unsafe {
            std::env::set_var("ATTACHMENTS_BUCKET", "test-bucket");
            std::env::set_var("PRESIGNED_URL_EXPIRATION_SECONDS", "3600");
            std::env::set_var("MAX_ATTACHMENT_SIZE_BYTES", "10485760");
        }

        let config = AttachmentConfig::from_env().unwrap();

        assert_eq!(config.bucket, "test-bucket");
        assert_eq!(config.presigned_url_expiration, Duration::from_secs(3600));
        assert_eq!(config.max_size, 10485760);
    }

    #[test]
    fn test_unique_filename_generation() {
        let sanitized = "document.pdf";

        // First attachment (index 0)
        let filename1 = sanitized.to_string();
        assert_eq!(filename1, "document.pdf");

        // Second attachment with same name (index 1)
        let parts: Vec<&str> = sanitized.rsplitn(2, '.').collect();
        let filename2 = format!("{}-{}.{}", parts[1], 1, parts[0]);
        assert_eq!(filename2, "document-1.pdf");

        // Third attachment (index 2)
        let filename3 = format!("{}-{}.{}", parts[1], 2, parts[0]);
        assert_eq!(filename3, "document-2.pdf");
    }
}
