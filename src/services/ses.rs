/// SES email sending service
use crate::error::MailflowError;
use crate::utils::retry::{RetryConfig, retry_with_backoff};
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct SendQuota {
    pub max_24_hour_send: f64,
    pub max_send_rate: f64,
    pub sent_last_24_hours: f64,
}

#[async_trait]
pub trait EmailSender: Send + Sync {
    async fn send_raw_email(
        &self,
        raw_email: &[u8],
        from: &str,
        to: &[String],
    ) -> Result<String, MailflowError>;
    async fn get_send_quota(&self) -> Result<SendQuota, MailflowError>;
    async fn verify_sender_identity(&self, email: &str) -> Result<bool, MailflowError>;
}

pub struct SesEmailSender {
    client: aws_sdk_ses::Client,
}

impl SesEmailSender {
    pub fn new(client: aws_sdk_ses::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl EmailSender for SesEmailSender {
    async fn send_raw_email(
        &self,
        raw_email: &[u8],
        from: &str,
        to: &[String],
    ) -> Result<String, MailflowError> {
        use aws_sdk_ses::primitives::Blob;

        // Build message once (outside retry loop)
        let raw_message = aws_sdk_ses::types::RawMessage::builder()
            .data(Blob::new(raw_email))
            .build()
            .map_err(|e| MailflowError::Ses(format!("Failed to build raw message: {}", e)))?;

        let from_str = from.to_string();
        let to_vec = to.to_vec();

        // Retry with exponential backoff for SES rate limits and transient failures
        let response = retry_with_backoff(
            || {
                let client = self.client.clone();
                let message = raw_message.clone();
                let from = from_str.clone();
                let destinations = to_vec.clone();

                async move {
                    client
                        .send_raw_email()
                        .raw_message(message)
                        .source(from)
                        .set_destinations(Some(destinations))
                        .send()
                        .await
                        .map_err(|e| {
                            MailflowError::Ses(format!("SES send_raw_email failed: {}", e))
                        })
                }
            },
            RetryConfig::default(),
            "ses_send_raw_email",
        )
        .await?;

        let message_id = response.message_id;

        tracing::info!("Sent email via SES: {} (to: {})", message_id, to.join(", "));
        Ok(message_id)
    }

    async fn get_send_quota(&self) -> Result<SendQuota, MailflowError> {
        let response = retry_with_backoff(
            || {
                let client = self.client.clone();
                async move {
                    client.get_send_quota().send().await.map_err(|e| {
                        MailflowError::Ses(format!("SES get_send_quota failed: {}", e))
                    })
                }
            },
            RetryConfig::default(),
            "ses_get_send_quota",
        )
        .await?;

        Ok(SendQuota {
            max_24_hour_send: response.max24_hour_send(),
            max_send_rate: response.max_send_rate(),
            sent_last_24_hours: response.sent_last24_hours(),
        })
    }

    async fn verify_sender_identity(&self, email: &str) -> Result<bool, MailflowError> {
        let email_owned = email.to_string();

        let response = retry_with_backoff(
            || {
                let client = self.client.clone();
                let email = email_owned.clone();

                async move {
                    client
                        .get_identity_verification_attributes()
                        .identities(email)
                        .send()
                        .await
                        .map_err(|e| {
                            MailflowError::Ses(format!(
                                "Failed to get identity verification attributes: {}",
                                e
                            ))
                        })
                }
            },
            RetryConfig::default(),
            "ses_verify_identity",
        )
        .await?;

        let verified = response
            .verification_attributes()
            .get(email)
            .map(|attr| {
                matches!(
                    attr.verification_status(),
                    aws_sdk_ses::types::VerificationStatus::Success
                )
            })
            .unwrap_or(false);

        tracing::debug!(
            email = %email,
            verified = verified,
            "Checked sender identity verification status"
        );

        Ok(verified)
    }
}
