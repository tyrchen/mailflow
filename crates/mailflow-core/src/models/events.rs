/// AWS Lambda event types
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Lambda event wrapper - can be S3, SQS, or SES event
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum LambdaEvent {
    Ses(SesEvent), // Try SES first (most specific)
    S3(S3Event),
    Sqs(SqsEvent),
}

/// S3 event from SES
#[derive(Debug, Clone, Deserialize)]
pub struct S3Event {
    #[serde(rename = "Records")]
    pub records: Vec<S3EventRecord>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct S3EventRecord {
    #[serde(rename = "eventVersion")]
    pub event_version: String,
    #[serde(rename = "eventSource")]
    pub event_source: String,
    #[serde(rename = "awsRegion")]
    pub aws_region: String,
    #[serde(rename = "eventTime")]
    pub event_time: String,
    #[serde(rename = "eventName")]
    pub event_name: String,
    pub s3: S3Info,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ses: Option<SesInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct S3Info {
    pub bucket: S3Bucket,
    pub object: S3Object,
}

#[derive(Debug, Clone, Deserialize)]
pub struct S3Bucket {
    pub name: String,
    pub arn: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct S3Object {
    pub key: String,
    pub size: Option<i64>,
    #[serde(rename = "eTag")]
    pub etag: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SesInfo {
    pub mail: SesMail,
    pub receipt: SesReceipt,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SesMail {
    #[serde(rename = "messageId")]
    pub message_id: String,
    pub timestamp: String,
    pub source: String,
    pub destination: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SesReceipt {
    pub recipients: Vec<String>,
    #[serde(rename = "spfVerdict")]
    pub spf_verdict: Option<Verdict>,
    #[serde(rename = "dkimVerdict")]
    pub dkim_verdict: Option<Verdict>,
    #[serde(rename = "spamVerdict")]
    pub spam_verdict: Option<Verdict>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Verdict {
    pub status: String,
}

/// SES event from direct Lambda invocation
#[derive(Debug, Clone, Deserialize)]
pub struct SesEvent {
    #[serde(rename = "Records")]
    pub records: Vec<SesEventRecord>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SesEventRecord {
    #[serde(rename = "eventSource")]
    pub event_source: String,
    #[serde(rename = "eventVersion")]
    pub event_version: String,
    pub ses: SesPayload,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SesPayload {
    pub mail: SesMail,
    pub receipt: SesReceiptFull,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SesReceiptFull {
    pub timestamp: String,
    pub recipients: Vec<String>,
    #[serde(rename = "spfVerdict")]
    pub spf_verdict: Option<Verdict>,
    #[serde(rename = "dkimVerdict")]
    pub dkim_verdict: Option<Verdict>,
    #[serde(rename = "spamVerdict")]
    pub spam_verdict: Option<Verdict>,
    #[serde(rename = "virusVerdict")]
    pub virus_verdict: Option<Verdict>,
    pub action: SesAction,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SesAction {
    #[serde(rename = "type")]
    pub action_type: String,
    #[serde(rename = "bucketName")]
    pub bucket_name: Option<String>,
    #[serde(rename = "objectKey")]
    pub object_key: Option<String>,
}

/// SQS event for outbound processing
#[derive(Debug, Clone, Deserialize)]
pub struct SqsEvent {
    #[serde(rename = "Records")]
    pub records: Vec<SqsRecord>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SqsRecord {
    #[serde(rename = "messageId")]
    pub message_id: String,
    #[serde(rename = "receiptHandle")]
    pub receipt_handle: String,
    pub body: String,
    #[serde(default)]
    pub attributes: HashMap<String, String>,
    #[serde(rename = "messageAttributes", default)]
    pub message_attributes: HashMap<String, MessageAttribute>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MessageAttribute {
    #[serde(rename = "stringValue", skip_serializing_if = "Option::is_none")]
    pub string_value: Option<String>,
    #[serde(rename = "binaryValue", skip_serializing_if = "Option::is_none")]
    pub binary_value: Option<String>,
    #[serde(rename = "dataType")]
    pub data_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s3_event_deserialization() {
        let json = r#"{
            "Records": [{
                "eventVersion": "2.1",
                "eventSource": "aws:s3",
                "awsRegion": "us-east-1",
                "eventTime": "2025-10-31T12:00:00.000Z",
                "eventName": "ObjectCreated:Put",
                "s3": {
                    "bucket": {
                        "name": "test-bucket",
                        "arn": "arn:aws:s3:::test-bucket"
                    },
                    "object": {
                        "key": "test-key",
                        "size": 1024
                    }
                }
            }]
        }"#;

        let event: S3Event = serde_json::from_str(json).unwrap();
        assert_eq!(event.records.len(), 1);
        assert_eq!(event.records[0].s3.bucket.name, "test-bucket");
    }

    #[test]
    fn test_sqs_event_deserialization() {
        let json = r#"{
            "Records": [{
                "messageId": "msg-123",
                "receiptHandle": "handle-456",
                "body": "{\"test\": true}",
                "attributes": {}
            }]
        }"#;

        let event: SqsEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.records.len(), 1);
        assert_eq!(event.records[0].message_id, "msg-123");
    }

    #[test]
    fn test_ses_event_deserialization() {
        let json = r#"{
            "Records": [{
                "eventSource": "aws:ses",
                "eventVersion": "1.0",
                "ses": {
                    "mail": {
                        "messageId": "test-123",
                        "timestamp": "2025-11-01T12:00:00.000Z",
                        "source": "sender@example.com",
                        "destination": ["_app1@acme.com"]
                    },
                    "receipt": {
                        "timestamp": "2025-11-01T12:00:00.000Z",
                        "recipients": ["_app1@acme.com"],
                        "spfVerdict": {"status": "PASS"},
                        "dkimVerdict": {"status": "PASS"},
                        "spamVerdict": {"status": "PASS"},
                        "virusVerdict": {"status": "PASS"},
                        "action": {
                            "type": "Lambda",
                            "bucketName": "test-bucket",
                            "objectKey": "test-key"
                        }
                    }
                }
            }]
        }"#;

        let event: SesEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.records.len(), 1);
        assert_eq!(event.records[0].ses.mail.message_id, "test-123");
        assert_eq!(
            event.records[0].ses.receipt.action.bucket_name,
            Some("test-bucket".to_string())
        );
        assert_eq!(
            event.records[0].ses.receipt.action.object_key,
            Some("test-key".to_string())
        );
    }
}
