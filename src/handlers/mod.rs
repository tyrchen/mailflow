/// Lambda event handlers
pub mod common;
pub mod inbound;
pub mod outbound;
pub mod ses;

use crate::error::MailflowError;
use crate::models::LambdaEvent;
use lambda_runtime::{Error, LambdaEvent as RuntimeEvent};
use serde_json::Value;
use tracing::{error, info};

/// Main Lambda handler - routes events to appropriate handler
pub async fn handler(event: RuntimeEvent<Value>) -> Result<Value, Error> {
    info!("Received Lambda event");

    // Try to parse as LambdaEvent (S3 or SQS)
    let lambda_event: LambdaEvent = serde_json::from_value(event.payload.clone()).map_err(|e| {
        error!("Failed to parse Lambda event: {}", e);
        MailflowError::Lambda(format!("Invalid event type: {}", e))
    })?;

    match lambda_event {
        LambdaEvent::Ses(ses_event) => {
            info!("Processing SES event (inbound via SES)");
            ses::handle(ses_event).await?;
        }
        LambdaEvent::S3(s3_event) => {
            info!("Processing S3 event (inbound via S3)");
            inbound::handle(s3_event).await?;
        }
        LambdaEvent::Sqs(sqs_event) => {
            info!("Processing SQS event (outbound)");
            outbound::handle(sqs_event).await?;
        }
    }

    Ok(serde_json::json!({
        "statusCode": 200,
        "body": "OK"
    }))
}
