/// S3 storage service
use crate::error::MailflowError;
use async_trait::async_trait;
use std::time::Duration;

#[async_trait]
pub trait StorageService: Send + Sync {
    async fn upload(&self, bucket: &str, key: &str, data: &[u8]) -> Result<(), MailflowError>;
    async fn download(&self, bucket: &str, key: &str) -> Result<Vec<u8>, MailflowError>;
    async fn generate_presigned_url(
        &self,
        bucket: &str,
        key: &str,
        expiration: Duration,
    ) -> Result<String, MailflowError>;
    async fn delete(&self, bucket: &str, key: &str) -> Result<(), MailflowError>;
}

/// S3 storage service implementation
pub struct S3StorageService {
    client: aws_sdk_s3::Client,
}

impl S3StorageService {
    pub fn new(client: aws_sdk_s3::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StorageService for S3StorageService {
    async fn upload(&self, bucket: &str, key: &str, data: &[u8]) -> Result<(), MailflowError> {
        use aws_sdk_s3::primitives::ByteStream;

        self.client
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(ByteStream::from(data.to_vec()))
            .send()
            .await
            .map_err(|e| MailflowError::Storage(format!("S3 upload failed: {}", e)))?;

        tracing::info!("Uploaded to s3://{}/{}", bucket, key);
        Ok(())
    }

    async fn download(&self, bucket: &str, key: &str) -> Result<Vec<u8>, MailflowError> {
        let response = self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| MailflowError::Storage(format!("S3 download failed: {}", e)))?;

        let data = response
            .body
            .collect()
            .await
            .map_err(|e| MailflowError::Storage(format!("Failed to read S3 object body: {}", e)))?
            .into_bytes()
            .to_vec();

        tracing::info!(
            "Downloaded from s3://{}/{} ({} bytes)",
            bucket,
            key,
            data.len()
        );
        Ok(data)
    }

    async fn generate_presigned_url(
        &self,
        bucket: &str,
        key: &str,
        expiration: Duration,
    ) -> Result<String, MailflowError> {
        use aws_sdk_s3::presigning::PresigningConfig;

        let presigning_config = PresigningConfig::expires_in(expiration)
            .map_err(|e| MailflowError::Storage(format!("Invalid expiration duration: {}", e)))?;

        let presigned_request = self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .presigned(presigning_config)
            .await
            .map_err(|e| {
                MailflowError::Storage(format!("Failed to generate presigned URL: {}", e))
            })?;

        Ok(presigned_request.uri().to_string())
    }

    async fn delete(&self, bucket: &str, key: &str) -> Result<(), MailflowError> {
        self.client
            .delete_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| MailflowError::Storage(format!("S3 delete failed: {}", e)))?;

        tracing::info!("Deleted s3://{}/{}", bucket, key);
        Ok(())
    }
}
