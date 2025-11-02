# Fix Design: Attachment Metadata Parsing

**Issue ID:** 0002
**Date:** 2025-11-01
**Status:** Design Complete
**Priority:** High
**Related to:** 0001-fix-email-issue.md

---

## Executive Summary

Implement attachment metadata extraction and processing to enable applications consuming from SQS queues to easily access email attachments. The system will extract attachments from parsed emails, upload them to dedicated S3 storage, generate presigned URLs, and include comprehensive metadata in SQS messages.

---

## Current State Analysis

### What Works ✅
- Emails with attachments are received and stored in S3 by SES
- Lambda downloads and parses email bodies correctly
- Email routing and SQS delivery functional
- Attachment utility functions exist (`sanitize_filename`, `validate_file_type`)

### What's Missing ❌
- Attachment extraction from MIME multipart emails
- Attachment upload to S3 with organized structure
- Presigned URL generation for secure access
- Attachment metadata in SQS messages
- Size validation and content-type checking

### Code Evidence

**parser.rs:117-118**
```rust
// For now, attachments are empty - will be processed separately
let attachments = vec![];
```

**Current Message Format** (attachments array is empty):
```json
{
  "email": {
    "attachments": []  // Always empty!
  }
}
```

---

## Requirements

### Functional Requirements

**FR-1: Attachment Detection**
- Extract all attachments from MIME multipart emails
- Detect inline images and treat as attachments
- Support common MIME types (application/pdf, image/*, text/*, etc.)
- Handle base64, quoted-printable, and binary encodings

**FR-2: Attachment Storage**
- Upload attachments to S3 with organized structure
- Store in separate location from raw emails
- Preserve original filename and content-type
- Generate unique keys to avoid collisions

**FR-3: Attachment Metadata**
- Include in SQS message:
  - Filename (original and sanitized)
  - Content-Type
  - Size in bytes
  - S3 bucket and key
  - Presigned URL with configurable expiration
  - MD5/SHA256 checksum for integrity

**FR-4: Security & Validation**
- Validate attachment sizes (default max: 35 MB per file)
- Check file types against allow/block lists
- Sanitize filenames to prevent path traversal
- Scan for malware (future: integration point)
- Generate time-limited presigned URLs (default: 7 days)

**FR-5: Error Handling**
- Continue processing email if attachment upload fails
- Include error details in metadata
- Log attachment processing failures
- Send to DLQ if all attachments fail

### Non-Functional Requirements

**NFR-1: Performance**
- Process attachments in parallel when multiple exist
- Limit individual attachment size to 35 MB
- Limit total email size to 40 MB (SES limit)
- Complete attachment processing within Lambda timeout

**NFR-2: Storage Efficiency**
- Use separate S3 bucket/prefix for attachments
- Apply lifecycle policies (default: 30 days retention)
- Use S3 Intelligent-Tiering for cost optimization
- Deduplicate identical attachments (optional, future)

**NFR-3: Usability**
- Apps can download attachments via presigned URLs
- No AWS credentials needed by consuming apps
- Metadata sufficient to render attachment list in UI
- Clear error messages if attachment unavailable

---

## Design

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Attachment Processing Flow                    │
└─────────────────────────────────────────────────────────────────┘

1. Raw Email in S3
   └─> Lambda downloads
       └─> mail-parser extracts MIME parts
           └─> Identify attachments (Content-Disposition: attachment/inline)
               └─> For each attachment:
                   ├─> Validate size & type
                   ├─> Sanitize filename
                   ├─> Upload to S3 (attachments bucket)
                   ├─> Generate presigned URL
                   └─> Build metadata object
               └─> Add to email.attachments array
                   └─> Serialize to SQS message
```

### S3 Storage Structure

```
s3://mailflow-attachments-{env}/
├── {message-id}/
│   ├── {sanitized-filename-1}
│   ├── {sanitized-filename-2}
│   └── {sanitized-filename-n}
```

**Rationale:**
- Group by message ID for easy cleanup
- Sanitized filenames prevent collisions and security issues
- Flat structure within message for simplicity

**Alternative Considered:**
```
s3://mailflow-attachments-{env}/
├── {YYYY}/
│   ├── {MM}/
│   │   ├── {DD}/
│   │   │   ├── {message-id}/
│   │   │   │   ├── {filename}
```
**Rejected:** Over-engineered for initial implementation, adds complexity

### Attachment Metadata Schema

```json
{
  "filename": "document.pdf",              // Original filename
  "sanitized_filename": "document.pdf",    // Safe filename used in S3
  "content_type": "application/pdf",       // MIME type
  "size": 123456,                          // Size in bytes
  "s3_bucket": "mailflow-attachments-dev", // S3 bucket name
  "s3_key": "msg123/document.pdf",         // S3 object key
  "presigned_url": "https://...",          // Time-limited download URL
  "presigned_url_expiration": "2025-11-08T12:00:00Z", // URL expiry
  "checksum_md5": "d41d8cd98f00b204e...",  // MD5 hash (optional)
  "status": "available",                   // Status: available|failed|processing
  "error": null                            // Error message if failed
}
```

### Configuration

**Environment Variables:**
```bash
ATTACHMENTS_BUCKET=mailflow-attachments-dev
ATTACHMENTS_PREFIX=                          # Optional prefix
PRESIGNED_URL_EXPIRATION_SECONDS=604800      # 7 days
MAX_ATTACHMENT_SIZE_BYTES=36700160           # 35 MB
ALLOWED_CONTENT_TYPES=*                      # Comma-separated or * for all
BLOCKED_CONTENT_TYPES=application/x-executable,application/x-msdownload
```

**Pulumi Infrastructure:**
```typescript
// New S3 bucket for attachments
const attachmentsBucket = new aws.s3.Bucket(`mailflow-attachments-${env}`, {
    lifecycleRules: [{
        enabled: true,
        expiration: { days: 30 }  // Auto-delete old attachments
    }],
    serverSideEncryptionConfiguration: {
        rule: {
            applyServerSideEncryptionByDefault: {
                sseAlgorithm: "AES256"
            }
        }
    },
    corsRules: [{  // Allow browser downloads via presigned URLs
        allowedMethods: ["GET"],
        allowedOrigins: ["*"],
        allowedHeaders: ["*"],
        maxAgeSeconds: 3600
    }]
});
```

---

## Implementation Plan

### Phase 1: Parser Enhancement

**File:** `src/email/parser.rs`

**Changes:**
1. Import mail-parser attachment APIs
2. Iterate through message parts
3. Identify attachments by Content-Disposition header
4. Extract attachment data and metadata
5. Return attachment descriptors (not yet uploaded)

**Code Structure:**
```rust
// New helper function
fn extract_attachments(message: &Message) -> Vec<AttachmentData> {
    let mut attachments = Vec::new();

    // Iterate through all parts
    for part in message.parts.iter() {
        // Check if this is an attachment
        if part.is_attachment() || part.is_inline_attachment() {
            let filename = part.attachment_name()
                .or_else(|| part.inline_attachment_name())
                .unwrap_or("unnamed");

            let content_type = part.content_type()
                .and_then(|ct| ct.c_type)
                .unwrap_or("application/octet-stream");

            let body = part.body.as_ref()?;

            attachments.push(AttachmentData {
                filename: filename.to_string(),
                content_type: content_type.to_string(),
                data: body.clone(),
            });
        }
    }

    attachments
}
```

### Phase 2: Attachment Processing Service

**File:** `src/services/attachments.rs` (NEW)

**Responsibilities:**
- Validate attachment size and type
- Sanitize filenames
- Upload to S3
- Generate presigned URLs
- Build attachment metadata

**Interface:**
```rust
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
    bucket: String,
    presigned_url_expiration: Duration,
    max_size: usize,
    allowed_types: Vec<String>,
    blocked_types: Vec<String>,
}

impl S3AttachmentProcessor {
    pub fn new(
        storage: Arc<dyn StorageService>,
        config: AttachmentConfig,
    ) -> Self { ... }

    async fn process_single_attachment(
        &self,
        message_id: &str,
        data: AttachmentData,
    ) -> Result<Attachment, MailflowError> {
        // 1. Validate size
        if data.data.len() > self.max_size {
            return Err(MailflowError::Validation(
                format!("Attachment {} exceeds max size", data.filename)
            ));
        }

        // 2. Validate content type
        validate_file_type(
            &data.content_type,
            &self.allowed_types,
            &self.blocked_types
        )?;

        // 3. Sanitize filename
        let sanitized = sanitize_filename(&data.filename);

        // 4. Generate S3 key
        let s3_key = format!("{}/{}", message_id, sanitized);

        // 5. Upload to S3
        self.storage.upload(&self.bucket, &s3_key, &data.data).await?;

        // 6. Generate presigned URL
        let presigned_url = self.storage.generate_presigned_url(
            &self.bucket,
            &s3_key,
            self.presigned_url_expiration
        ).await?;

        // 7. Calculate expiration time
        let expiration = Utc::now() + self.presigned_url_expiration;

        // 8. Build metadata
        Ok(Attachment {
            filename: data.filename,
            sanitized_filename: sanitized,
            content_type: data.content_type,
            size: data.data.len(),
            s3_bucket: self.bucket.clone(),
            s3_key,
            presigned_url,
            presigned_url_expiration: expiration,
            checksum_md5: None, // Optional enhancement
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
        // Process attachments in parallel using join_all
        let futures = attachments_data.into_iter().map(|data| {
            self.process_single_attachment(message_id, data)
        });

        let results = futures::future::join_all(futures).await;

        // Collect successful attachments, log failures
        let mut attachments = Vec::new();
        for result in results {
            match result {
                Ok(attachment) => attachments.push(attachment),
                Err(e) => {
                    tracing::error!("Failed to process attachment: {}", e);
                    // Optionally add failed attachment with error status
                }
            }
        }

        Ok(attachments)
    }
}
```

### Phase 3: Integration

**File:** `src/handlers/inbound.rs` and `src/handlers/ses.rs`

**Changes:**
1. Add attachment processor to context
2. Call attachment processing after email parsing
3. Update email object with processed attachments
4. Handle errors gracefully

**Updated Context:**
```rust
pub struct InboundContext {
    pub storage: Arc<dyn StorageService>,
    pub queue: Arc<dyn QueueService>,
    pub parser: Arc<dyn EmailParser>,
    pub router: Arc<dyn Router>,
    pub attachment_processor: Arc<dyn AttachmentProcessor>, // NEW
    config: Arc<dyn ConfigProvider>,
}
```

**Updated Processing Flow:**
```rust
async fn process_ses_record(
    ctx: &InboundContext,
    record: &SesEventRecord,
) -> Result<(), MailflowError> {
    // ... existing code ...

    // Parse email
    let mut email = ctx.parser.parse(&raw_email).await?;

    // NEW: Process attachments if any
    if !email.attachments_data.is_empty() {
        let processed_attachments = ctx.attachment_processor
            .process_attachments(&email.message_id, email.attachments_data)
            .await?;

        email.attachments = processed_attachments;

        info!(
            "Processed {} attachment(s) for message {}",
            email.attachments.len(),
            email.message_id
        );
    }

    // ... rest of existing code ...
}
```

### Phase 4: Infrastructure Updates

**File:** `infra/src/storage.ts`

**Add Attachments Bucket:**
```typescript
export function createStorage(environment: string) {
    // ... existing rawEmailsBucket ...

    // New: Attachments bucket
    const attachmentsBucket = new aws.s3.Bucket(
        `mailflow-attachments-${environment}`,
        {
            bucket: `mailflow-attachments-${environment}`,
            lifecycleRules: [
                {
                    enabled: true,
                    expiration: {
                        days: 30, // Auto-delete after 30 days
                    },
                },
            ],
            serverSideEncryptionConfiguration: {
                rule: {
                    applyServerSideEncryptionByDefault: {
                        sseAlgorithm: "AES256",
                    },
                },
            },
            corsRules: [
                {
                    allowedMethods: ["GET"],
                    allowedOrigins: ["*"],
                    allowedHeaders: ["*"],
                    maxAgeSeconds: 3600,
                },
            ],
            tags: {
                Environment: environment,
                Service: "mailflow",
            },
        }
    );

    return {
        bucket: rawEmailsBucket,
        attachmentsBucket,  // NEW
        bucketPolicy,
        publicAccessBlock,
    };
}
```

**File:** `infra/src/iam.ts`

**Add S3 Permissions:**
```typescript
{
    Sid: "AttachmentsBucketAccess",
    Effect: "Allow",
    Action: ["s3:GetObject", "s3:PutObject", "s3:DeleteObject"],
    Resource: `${attachmentsBucketArn}/*`,
}
```

**File:** `infra/src/lambda.ts`

**Add Environment Variables:**
```typescript
environment: {
    variables: {
        // ... existing ...
        ATTACHMENTS_BUCKET: attachmentsBucket.bucket,
        PRESIGNED_URL_EXPIRATION_SECONDS: "604800", // 7 days
        MAX_ATTACHMENT_SIZE_BYTES: "36700160", // 35 MB
        ALLOWED_CONTENT_TYPES: "*",
        BLOCKED_CONTENT_TYPES: "application/x-executable,application/x-msdownload",
    },
}
```

### Phase 5: Model Updates

**File:** `src/models/email.rs`

**Add AttachmentData (intermediate):**
```rust
/// Raw attachment data before processing
#[derive(Debug, Clone)]
pub struct AttachmentData {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
}
```

**Update Email struct:**
```rust
pub struct Email {
    // ... existing fields ...
    pub attachments: Vec<Attachment>,

    // Transient field for processing
    #[serde(skip)]
    pub attachments_data: Vec<AttachmentData>,
}
```

**Update Attachment struct:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub filename: String,
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
    pub presigned_url_expiration: chrono::DateTime<Utc>,
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
    Processing,
}
```

---

## Error Handling Strategy

### Attachment Processing Failures

**Scenario 1: Single Attachment Fails**
```rust
// Continue processing other attachments
// Include failed attachment with error status
Attachment {
    filename: "corrupted.pdf",
    status: AttachmentStatus::Failed,
    error: Some("File type validation failed"),
    presigned_url: "",  // Empty
    // ... other fields with safe defaults
}
```

**Scenario 2: All Attachments Fail**
```rust
// Still send email to SQS with empty attachments array
// Log error but don't fail entire email processing
tracing::warn!(
    "All {} attachments failed for message {}",
    count, message_id
);
```

**Scenario 3: S3 Upload Fails**
```rust
// Retry up to 3 times with exponential backoff
// If still failing, mark as failed and continue
```

**Scenario 4: Size Limit Exceeded**
```rust
// Skip attachment and log warning
// Include in metadata with error status
Attachment {
    filename: "huge-file.zip",
    status: AttachmentStatus::Failed,
    error: Some("Exceeds max size of 35 MB"),
    size: 50_000_000,
    // ... other fields
}
```

---

## Testing Strategy

### Unit Tests

**File:** `src/email/parser.rs`
```rust
#[tokio::test]
async fn test_parse_email_with_attachment() {
    let raw = include_bytes!("../../tests/fixtures/email-with-pdf.eml");
    let parser = MailParserEmailParser::new();
    let email = parser.parse(raw).await.unwrap();

    assert_eq!(email.attachments_data.len(), 1);
    assert_eq!(email.attachments_data[0].filename, "test.pdf");
    assert_eq!(email.attachments_data[0].content_type, "application/pdf");
    assert!(!email.attachments_data[0].data.is_empty());
}
```

**File:** `src/services/attachments.rs`
```rust
#[tokio::test]
async fn test_process_single_attachment() {
    // Mock storage service
    // Test successful upload and presigned URL generation
}

#[tokio::test]
async fn test_attachment_size_validation() {
    // Test that oversized attachments are rejected
}

#[tokio::test]
async fn test_filename_sanitization() {
    // Test "../etc/passwd" -> "_etc_passwd"
}
```

### Integration Tests

**File:** `tests/attachment_integration_test.rs`
```rust
#[tokio::test]
async fn test_end_to_end_attachment_processing() {
    // 1. Send email with attachment via SES
    // 2. Wait for Lambda processing
    // 3. Check SQS message
    // 4. Verify attachment metadata
    // 5. Download via presigned URL
    // 6. Verify content matches original
}
```

### Manual Testing

```bash
# 1. Send email with various attachment types
aws ses send-raw-email --raw-message file://test-email-pdf.eml
aws ses send-raw-email --raw-message file://test-email-images.eml
aws ses send-raw-email --raw-message file://test-email-multiple.eml

# 2. Check Lambda logs
aws logs tail /aws/lambda/mailflow-dev --follow

# 3. Check SQS message
aws sqs receive-message --queue-url $QUEUE_URL | jq '.Messages[0].Body | fromjson | .email.attachments'

# 4. Verify S3 objects
aws s3 ls s3://mailflow-attachments-dev/ --recursive

# 5. Test presigned URL
curl -o downloaded.pdf "$(echo $PRESIGNED_URL)"
```

---

## Performance Considerations

### Parallel Processing
```rust
// Process multiple attachments concurrently
let futures = attachments_data.into_iter().map(|data| {
    tokio::spawn(async move {
        process_single_attachment(data).await
    })
});

let results = futures::future::join_all(futures).await;
```

### Memory Management
```rust
// Stream large attachments to S3 instead of loading in memory
// Use multipart upload for files > 5MB
if data.len() > 5 * 1024 * 1024 {
    self.storage.multipart_upload(&bucket, &key, data).await?;
} else {
    self.storage.upload(&bucket, &key, data).await?;
}
```

### Lambda Timeout Protection
```rust
// Set timeout for attachment processing
const MAX_ATTACHMENT_PROCESSING_TIME: Duration = Duration::from_secs(45);

tokio::time::timeout(
    MAX_ATTACHMENT_PROCESSING_TIME,
    process_attachments(...)
).await??;
```

---

## Migration & Rollout

### Phase 1: Deploy Infrastructure (No Code Changes)
- Create attachments S3 bucket
- Update IAM policies
- Add environment variables
- **Risk:** Low, no behavior change

### Phase 2: Deploy Code with Feature Flag
```rust
const ENABLE_ATTACHMENT_PROCESSING: bool =
    std::env::var("ENABLE_ATTACHMENTS")
        .unwrap_or("false".to_string()) == "true";

if ENABLE_ATTACHMENT_PROCESSING {
    // Process attachments
} else {
    // Skip (current behavior)
}
```
- Deploy with flag disabled
- Monitor for errors
- **Risk:** Low, feature disabled

### Phase 3: Enable for Test Queue
- Set `ENABLE_ATTACHMENTS=true` for one test app
- Send test emails
- Verify SQS messages
- Check consuming app can download
- **Risk:** Medium, limited blast radius

### Phase 4: Gradual Rollout
- Enable for 10% of apps
- Monitor metrics: error rate, latency, S3 costs
- If stable, increase to 50%, then 100%
- **Risk:** Medium, gradual rollout

### Phase 5: Remove Feature Flag
- Remove conditional code
- Make attachment processing default
- **Risk:** Low, proven in production

---

## Success Metrics

### Functional Metrics
- [ ] 100% of emails with attachments have non-empty attachments array
- [ ] Presigned URLs are valid and downloadable
- [ ] Attachment metadata accurate (filename, size, type)
- [ ] No increase in email processing failures

### Performance Metrics
- [ ] Lambda execution time increase < 200ms for emails with attachments
- [ ] Memory usage remains < 128 MB
- [ ] S3 upload latency < 100ms per attachment
- [ ] Presigned URL generation < 10ms

### Cost Metrics
- [ ] S3 storage cost < $0.023 per GB per month
- [ ] S3 request cost tracked and acceptable
- [ ] Lambda duration increase cost < 10% of total

---

## Monitoring & Alerts

### CloudWatch Metrics
```typescript
new aws.cloudwatch.MetricAlarm("attachment-processing-errors", {
    metricName: "AttachmentProcessingErrors",
    namespace: "Mailflow",
    statistic: "Sum",
    period: 300,
    evaluationPeriods: 1,
    threshold: 10,
    comparisonOperator: "GreaterThanThreshold",
    alarmActions: [snsTopicArn],
});
```

### Custom Metrics to Emit
```rust
// In attachment processor
metrics::counter!("attachments.processed", 1);
metrics::histogram!("attachments.size_bytes", size as f64);
metrics::histogram!("attachments.upload_duration_ms", duration.as_millis() as f64);
metrics::counter!("attachments.failures", 1, "reason" => error_type);
```

### Log Queries
```
# Failed attachments
fields @timestamp, @message
| filter @message like /Failed to process attachment/
| stats count() by error_reason

# Large attachments
fields @timestamp, @message
| filter @message like /attachment size/
| parse @message "size: * bytes" as size
| stats avg(size), max(size), p99(size)
```

---

## Security Considerations

### Filename Sanitization
```rust
// Prevent path traversal
assert_eq!(sanitize_filename("../../../etc/passwd"), "_etc_passwd");

// Prevent command injection
assert_eq!(sanitize_filename("file; rm -rf /"), "file_rm__rf_");

// Allow normal filenames
assert_eq!(sanitize_filename("Document-2024.pdf"), "Document-2024.pdf");
```

### Content-Type Validation
```rust
// Block executables
let blocked = vec![
    "application/x-executable",
    "application/x-msdownload",
    "application/x-msdos-program",
    "application/x-sh",
];
```

### Presigned URL Security
```rust
// Short expiration (7 days default)
// No public read permissions
// HTTPS only
// Audit access via CloudTrail
```

### Malware Scanning (Future)
```rust
// Integration point for AWS GuardDuty or ClamAV
async fn scan_for_malware(data: &[u8]) -> Result<bool, Error> {
    // Call scanning service
    // Return true if safe, false if malware detected
}
```

---

## Documentation for App Developers

### SQS Message Format

```json
{
  "email": {
    "attachments": [
      {
        "filename": "invoice-2024.pdf",
        "sanitized_filename": "invoice-2024.pdf",
        "contentType": "application/pdf",
        "size": 245632,
        "s3Bucket": "mailflow-attachments-dev",
        "s3Key": "msg-123/invoice-2024.pdf",
        "presignedUrl": "https://mailflow-attachments-dev.s3.amazonaws.com/msg-123/invoice-2024.pdf?X-Amz-...",
        "presignedUrlExpiration": "2025-11-08T12:00:00Z",
        "status": "available"
      }
    ]
  }
}
```

### Downloading Attachments (Python Example)

```python
import requests
import json

# Parse SQS message
message = json.loads(sqs_message['Body'])
attachments = message['email']['attachments']

for attachment in attachments:
    if attachment['status'] == 'available':
        # Download via presigned URL
        response = requests.get(attachment['presignedUrl'])

        if response.status_code == 200:
            # Save file
            with open(attachment['filename'], 'wb') as f:
                f.write(response.content)

            print(f"Downloaded {attachment['filename']} ({attachment['size']} bytes)")
        else:
            print(f"Failed to download {attachment['filename']}")
    else:
        print(f"Attachment {attachment['filename']} not available: {attachment.get('error')}")
```

### Handling Expired URLs

```python
from datetime import datetime, timezone

def is_url_expired(expiration_str):
    expiration = datetime.fromisoformat(expiration_str.replace('Z', '+00:00'))
    return expiration < datetime.now(timezone.utc)

# Check before download
for attachment in attachments:
    if is_url_expired(attachment['presignedUrlExpiration']):
        # URL expired - need to request new email or store attachments
        print(f"Presigned URL for {attachment['filename']} has expired")
        # Option 1: Store attachments when received
        # Option 2: Request email resend
        # Option 3: Use S3 SDK with app credentials (not recommended)
```

---

## Future Enhancements

### 1. Attachment Deduplication
```rust
// Store attachments by content hash
// Reuse existing attachment if hash matches
let content_hash = sha256(&data);
let s3_key = format!("shared/{}", content_hash);

// Reference same S3 object from multiple emails
// Reduces storage costs
```

### 2. Virus Scanning Integration
```rust
// Integrate with AWS GuardDuty or ClamAV
async fn scan_attachment(data: &[u8]) -> Result<ScanResult, Error> {
    // Send to scanning service
    // Wait for result
    // Mark attachment as quarantined if malware detected
}
```

### 3. Thumbnail Generation
```rust
// For images and PDFs, generate thumbnails
async fn generate_thumbnail(
    content_type: &str,
    data: &[u8]
) -> Option<Vec<u8>> {
    if content_type.starts_with("image/") {
        // Use image library to create thumbnail
        Some(create_thumbnail(data, 200, 200))
    } else {
        None
    }
}

// Include in metadata
attachment.thumbnail_url = Some(presigned_thumbnail_url);
```

### 4. Compression
```rust
// Compress large text files before upload
if content_type.starts_with("text/") && data.len() > 1_000_000 {
    let compressed = compress_gzip(&data)?;
    // Upload compressed version
    // Include compression info in metadata
}
```

### 5. OCR for Scanned Documents
```rust
// Use AWS Textract for scanned PDFs and images
async fn extract_text(attachment: &Attachment) -> Option<String> {
    if attachment.content_type == "application/pdf" {
        let text = aws_textract::analyze_document(&attachment.s3_key).await?;
        Some(text)
    } else {
        None
    }
}

// Include in metadata
attachment.extracted_text = Some(text);
```

---

## Appendix

### A. mail-parser API Reference

```rust
use mail_parser::{Message, MessageParser, PartType};

// Parse email
let message = MessageParser::default().parse(raw_bytes)?;

// Iterate parts
for part in message.parts.iter() {
    // Check if attachment
    let is_attachment = matches!(
        part.body,
        PartType::Text(_) | PartType::Binary(_) | PartType::Message(_)
    ) && part.is_attachment();

    // Get filename
    let filename = part.attachment_name();

    // Get content type
    let content_type = part.content_type();

    // Get body
    let body = part.body();
}
```

### B. S3 Presigned URL Generation

```rust
use aws_sdk_s3::presigning::PresigningConfig;
use std::time::Duration;

// Generate presigned GET URL
let presigning_config = PresigningConfig::expires_in(
    Duration::from_secs(7 * 24 * 60 * 60)  // 7 days
)?;

let presigned_request = s3_client
    .get_object()
    .bucket(bucket)
    .key(key)
    .presigned(presigning_config)
    .await?;

let url = presigned_request.uri().to_string();
```

### C. Cost Estimation

**Assumptions:**
- 1000 emails/day with attachments
- Average 2 attachments per email
- Average attachment size: 500 KB
- Presigned URL expiration: 7 days
- Attachment retention: 30 days

**S3 Storage:**
- Daily storage: 1000 emails * 2 attachments * 500 KB = 1 GB/day
- Monthly storage (30 days): 30 GB
- Cost: 30 GB * $0.023/GB = **$0.69/month**

**S3 Requests:**
- PUT requests: 2000/day * 30 = 60,000/month
- GET requests (presigned URL): 2000/day * 30 = 60,000/month
- Cost: (60,000 + 60,000) * $0.005/1000 = **$0.60/month**

**Lambda:**
- Additional execution time per email: 200ms
- Additional cost: 1000 * 30 * 0.2s * $0.0000166667 = **$0.10/month**

**Total Additional Cost: ~$1.39/month** for 1000 emails/day with attachments

---

**End of Design Document**

**Next Steps:**
1. Review and approve design
2. Create feature branch
3. Implement Phase 1 (Parser)
4. Implement Phase 2 (Attachment Service)
5. Implement Phase 3 (Integration)
6. Test thoroughly
7. Deploy with feature flag
8. Gradual rollout
9. Monitor and iterate

**Prepared by:** Claude Code
**Review Status:** Ready for Implementation
