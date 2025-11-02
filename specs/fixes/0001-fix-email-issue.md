# Fix Design: Email Processing Issue - SES Event Handling

**Issue ID:** 0001
**Date:** 2025-11-01
**Status:** Design Complete
**Severity:** Critical - Email processing completely broken

---

## Executive Summary

Email delivery from SES to SQS is failing due to an event format mismatch. SES invokes Lambda with a SES-specific event format, but the Lambda function only handles S3 and SQS event formats, causing all inbound emails to fail silently.

---

## Root Cause Analysis

### Problem Statement
Test email sent via AWS SES successfully arrives at SES and is stored in S3, but never appears in the target SQS queue (`mailflow-app1-dev`).

### Investigation Results

#### 1. Lambda Logs Analysis
```
ERROR: Failed to parse Lambda event: data did not match any variant of untagged enum LambdaEvent
ERROR: Lambda("Invalid event type: data did not match any variant of untagged enum LambdaEvent")
```

**Finding:** Lambda is receiving an event but failing to parse it.

#### 2. Infrastructure Configuration Analysis

**File:** `infra/src/ses.ts` (lines 38-50)

The SES Receipt Rule has TWO sequential actions:
```typescript
s3Actions: [
    {
        bucketName: rawEmailsBucket.bucket,
        position: 1,  // First: Save to S3
    },
],
lambdaActions: [
    {
        functionArn: lambdaFunction.arn,
        position: 2,  // Second: Invoke Lambda
        invocationType: "Event",
    },
],
```

**Finding:** SES first saves email to S3, then invokes Lambda directly.

#### 3. Event Model Analysis

**File:** `src/models/events.rs` (lines 6-11)

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum LambdaEvent {
    S3(S3Event),
    Sqs(SqsEvent),
}
```

**Finding:** Lambda only handles S3Event and SqsEvent, but NOT SesEvent.

#### 4. AWS SES Event Format

When SES invokes Lambda directly (via `lambdaActions`), it sends a **SES Event**, not an S3 Event:

```json
{
  "Records": [
    {
      "eventSource": "aws:ses",
      "eventVersion": "1.0",
      "ses": {
        "mail": {
          "messageId": "...",
          "timestamp": "...",
          "source": "sender@example.com",
          "destination": ["_app1@domain.com"]
        },
        "receipt": {
          "timestamp": "...",
          "recipients": ["_app1@domain.com"],
          "spfVerdict": {"status": "PASS"},
          "dkimVerdict": {"status": "PASS"},
          "action": {
            "type": "Lambda",
            "functionArn": "...",
            "invocationType": "Event"
          }
        }
      }
    }
  ]
}
```

The S3 information is embedded in `ses.receipt.action.objectKey` (if S3 action preceded Lambda action).

### Root Cause

**The Lambda function expects an S3 event (with S3 bucket/key info) but SES is sending a SES event (with mail metadata and S3 reference). This mismatch causes deserialization failure and processing breakdown.**

---

## Impact Assessment

### Current State
- **Email Reception:** ✓ Working (emails arrive at SES)
- **S3 Storage:** ✓ Working (emails saved to S3)
- **Lambda Invocation:** ✓ Working (Lambda is invoked)
- **Event Parsing:** ✗ BROKEN (SES event not recognized)
- **SQS Delivery:** ✗ BROKEN (no messages sent)

### Severity
- **Critical:** 100% of inbound emails are lost
- **Silent Failure:** No alerts or visible errors to users
- **Data Loss Risk:** Emails not being routed to applications

---

## Solution Design

### Option 1: Add SES Event Support (RECOMMENDED)

**Description:** Extend Lambda to handle SES events directly.

**Changes Required:**

1. **Add SES event model** (`src/models/events.rs`):
   ```rust
   pub enum LambdaEvent {
       S3(S3Event),
       Sqs(SqsEvent),
       Ses(SesEvent),  // NEW
   }

   #[derive(Debug, Clone, Deserialize)]
   pub struct SesEvent {
       #[serde(rename = "Records")]
       pub records: Vec<SesEventRecord>,
   }

   #[derive(Debug, Clone, Deserialize)]
   pub struct SesEventRecord {
       #[serde(rename = "eventSource")]
       pub event_source: String,  // "aws:ses"
       #[serde(rename = "eventVersion")]
       pub event_version: String,
       pub ses: SesPayload,
   }

   #[derive(Debug, Clone, Deserialize)]
   pub struct SesPayload {
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
       #[serde(rename = "commonHeaders")]
       pub common_headers: SesCommonHeaders,
   }

   #[derive(Debug, Clone, Deserialize)]
   pub struct SesCommonHeaders {
       pub from: Option<Vec<String>>,
       pub to: Option<Vec<String>>,
       pub subject: Option<String>,
   }

   #[derive(Debug, Clone, Deserialize)]
   pub struct SesReceipt {
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
       #[serde(rename = "functionArn")]
       pub function_arn: Option<String>,
       #[serde(rename = "invocationType")]
       pub invocation_type: Option<String>,
       #[serde(rename = "bucketName")]
       pub bucket_name: Option<String>,
       #[serde(rename = "objectKey")]
       pub object_key: Option<String>,
   }
   ```

2. **Add SES handler** (`src/handlers/ses.rs` - NEW FILE):
   ```rust
   use crate::error::MailflowError;
   use crate::models::SesEvent;
   use crate::handlers::inbound::{InboundContext, process_s3_record};
   use tracing::{info, warn};

   pub async fn handle(event: SesEvent) -> Result<(), MailflowError> {
       info!("Processing {} SES record(s)", event.records.len());

       let ctx = InboundContext::new().await?;

       for record in event.records {
           // Extract S3 location from SES action
           let bucket = record.ses.receipt.action.bucket_name
               .ok_or_else(|| MailflowError::Lambda("S3 bucket not found in SES action".to_string()))?;
           let key = record.ses.receipt.action.object_key
               .ok_or_else(|| MailflowError::Lambda("S3 object key not found in SES action".to_string()))?;

           info!("Processing email from s3://{}/{}", bucket, key);

           // Convert SES record to S3-like processing
           process_email_from_s3(&ctx, &bucket, &key, &record).await?;
       }

       Ok(())
   }

   async fn process_email_from_s3(
       ctx: &InboundContext,
       bucket: &str,
       key: &str,
       ses_record: &crate::models::SesEventRecord,
   ) -> Result<(), MailflowError> {
       // Reuse existing S3 download and processing logic
       // Add SES metadata (SPF, DKIM, spam scores) to message
       // ... implementation details
   }
   ```

3. **Update main handler** (`src/handlers/mod.rs`):
   ```rust
   pub mod inbound;
   pub mod outbound;
   pub mod ses;  // NEW

   pub async fn handler(event: RuntimeEvent<Value>) -> Result<Value, Error> {
       let lambda_event: LambdaEvent = serde_json::from_value(event.payload.clone())?;

       match lambda_event {
           LambdaEvent::S3(s3_event) => {
               info!("Processing S3 event");
               inbound::handle(s3_event).await?;
           }
           LambdaEvent::Sqs(sqs_event) => {
               info!("Processing SQS event");
               outbound::handle(sqs_event).await?;
           }
           LambdaEvent::Ses(ses_event) => {  // NEW
               info!("Processing SES event");
               ses::handle(ses_event).await?;
           }
       }

       Ok(serde_json::json!({"statusCode": 200, "body": "OK"}))
   }
   ```

**Pros:**
- ✓ Handles SES events natively
- ✓ Can use SPF/DKIM/spam scores from SES
- ✓ No infrastructure changes required
- ✓ More flexible for future SES features

**Cons:**
- ✗ Requires code changes
- ✗ Need to test SES event parsing

---

### Option 2: Remove Lambda Action, Use S3 Event Notification (ALTERNATIVE)

**Description:** Remove direct Lambda invocation from SES rule, configure S3 to trigger Lambda on object creation.

**Changes Required:**

1. **Update SES configuration** (`infra/src/ses.ts`):
   ```typescript
   // Remove lambdaActions from receipt rule
   const receiptRule = new aws.ses.ReceiptRule(`mailflow-inbound-${environment}`, {
       name: `mailflow-inbound-${environment}`,
       ruleSetName: ruleSet.ruleSetName,
       enabled: true,
       recipients: domains,
       scanEnabled: true,
       s3Actions: [
           {
               bucketName: rawEmailsBucket.bucket,
               position: 1,
           },
       ],
       // REMOVED: lambdaActions
   });
   ```

2. **Add S3 event notification** (`infra/src/storage.ts`):
   ```typescript
   const s3Notification = new aws.s3.BucketNotification(`mailflow-s3-notification-${environment}`, {
       bucket: rawEmailsBucket.id,
       lambdaFunctions: [
           {
               lambdaFunctionArn: lambdaFunction.arn,
               events: ["s3:ObjectCreated:*"],
               filterPrefix: "",  // All objects
               filterSuffix: "",  // All objects
           },
       ],
   });
   ```

3. **Update Lambda permissions** (`infra/src/iam.ts`):
   ```typescript
   const s3LambdaPermission = new aws.lambda.Permission(`mailflow-s3-invoke-${environment}`, {
       action: "lambda:InvokeFunction",
       function: lambdaFunction.name,
       principal: "s3.amazonaws.com",
       sourceArn: rawEmailsBucket.arn,
   });
   ```

**Pros:**
- ✓ No code changes needed
- ✓ Already supports S3 events
- ✓ Simpler event flow (S3 → Lambda)

**Cons:**
- ✗ Loses SES metadata (SPF/DKIM scores)
- ✗ Requires infrastructure redeployment
- ✗ Slight delay (S3 eventually consistent)
- ✗ May trigger on non-email objects

---

### Option 3: Hybrid Approach (NOT RECOMMENDED)

Keep both SES Lambda action and S3 notification, handle both event types.

**Cons:**
- ✗ Duplicate processing risk
- ✗ More complex error handling
- ✗ Higher Lambda invocation costs

---

## Recommended Solution: Option 1

**Rationale:**
1. **Minimal disruption:** No infrastructure changes
2. **Better metadata:** Preserves SPF/DKIM/spam scores from SES
3. **Spec alignment:** Original spec (FR-1.20) includes security metadata
4. **Production-ready:** Code-only change, easier to test and rollback

---

## Implementation Plan

### Phase 1: Add SES Event Support (Code)

1. **Update event models** (`src/models/events.rs`):
   - Add `SesEvent`, `SesEventRecord`, `SesPayload` structs
   - Add `Ses` variant to `LambdaEvent` enum
   - Add tests for SES event deserialization

2. **Create SES handler** (`src/handlers/ses.rs`):
   - Extract bucket/key from SES action
   - Reuse S3 download logic from inbound handler
   - Enrich message with SPF/DKIM/spam metadata
   - Add error handling and DLQ support

3. **Update main handler** (`src/handlers/mod.rs`):
   - Add `LambdaEvent::Ses` match arm
   - Route to new SES handler

4. **Update inbound handler** (`src/handlers/inbound.rs`):
   - Refactor S3 processing logic into reusable function
   - Allow SES handler to call same logic

5. **Update message metadata** (`src/models/messages.rs`):
   - Use actual SPF/DKIM/spam scores from SES (not hardcoded zeros)

### Phase 2: Testing

1. **Unit tests:**
   - Test SES event deserialization
   - Test SPF/DKIM/spam score extraction
   - Test S3 reference extraction from SES action

2. **Integration tests:**
   - Send test email via SES
   - Verify Lambda receives and parses SES event
   - Verify message appears in target SQS queue
   - Verify SPF/DKIM metadata is populated

3. **Error cases:**
   - Missing S3 action in SES event
   - Invalid S3 bucket/key
   - Malformed SES event
   - DLQ delivery on failure

### Phase 3: Deployment

1. **Build new Lambda binary:**
   ```bash
   cargo build --release --target x86_64-unknown-linux-musl
   ```

2. **Deploy via Pulumi:**
   ```bash
   cd infra
   pulumi up
   ```

3. **Smoke test:**
   - Send test email
   - Check CloudWatch logs
   - Verify SQS message delivery

4. **Monitor:**
   - Watch error rates
   - Check DLQ for failures
   - Verify processing latency

### Phase 4: Validation

1. Send multiple test emails to different apps
2. Verify all emails processed successfully
3. Check SQS queues have correct messages
4. Validate SPF/DKIM metadata is populated
5. Test error cases (invalid recipients, etc.)

---

## Detailed Code Changes

### 1. src/models/events.rs

```rust
// Add to existing file after SesInfo struct (line 61):

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

// Update LambdaEvent enum (line 6-11):
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum LambdaEvent {
    Ses(SesEvent),   // Try SES first (most specific)
    S3(S3Event),
    Sqs(SqsEvent),
}
```

### 2. src/handlers/ses.rs (NEW FILE)

```rust
/// SES event handler - processes direct SES Lambda invocations
use crate::error::MailflowError;
use crate::handlers::inbound::InboundContext;
use crate::models::{SesEvent, SesEventRecord};
use tracing::{error, info};

pub async fn handle(event: SesEvent) -> Result<(), MailflowError> {
    info!("Processing {} SES record(s)", event.records.len());

    let ctx = InboundContext::new().await?;
    let dlq_url = std::env::var("DLQ_URL").ok();

    for record in event.records {
        if let Err(e) = process_ses_record(&ctx, &record).await {
            error!("Failed to process SES record: {}", e);

            if let Some(ref dlq) = dlq_url {
                let error_payload = serde_json::json!({
                    "error": e.to_string(),
                    "error_type": if e.is_retriable() { "retriable" } else { "permanent" },
                    "record": {
                        "message_id": record.ses.mail.message_id,
                        "recipients": record.ses.receipt.recipients,
                    },
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "handler": "ses",
                });

                if let Err(dlq_err) = ctx
                    .queue
                    .send_message(dlq, &error_payload.to_string())
                    .await
                {
                    error!("Failed to send error to DLQ: {}", dlq_err);
                }
            }
        }
    }

    Ok(())
}

async fn process_ses_record(
    ctx: &InboundContext,
    record: &SesEventRecord,
) -> Result<(), MailflowError> {
    // Extract S3 location from SES action
    let bucket = record
        .ses
        .receipt
        .action
        .bucket_name
        .as_ref()
        .ok_or_else(|| MailflowError::Lambda("S3 bucket not found in SES action".to_string()))?;

    let key = record
        .ses
        .receipt
        .action
        .object_key
        .as_ref()
        .ok_or_else(|| MailflowError::Lambda("S3 object key not found in SES action".to_string()))?;

    info!(
        "Processing SES email - message_id: {}, s3://{}/{}",
        record.ses.mail.message_id, bucket, key
    );

    // Download raw email from S3
    let raw_email = ctx.storage.download(bucket, key).await?;

    // Parse email
    let email = ctx.parser.parse(&raw_email).await?;
    info!(
        "Parsed email - from: {}, subject: {}",
        email.from.address, email.subject
    );

    // Determine routing
    let routes = ctx.router.route(&email).await?;
    info!("Determined {} route(s)", routes.len());

    // Extract security metadata from SES
    let spf_verified = record
        .ses
        .receipt
        .spf_verdict
        .as_ref()
        .map(|v| v.status == "PASS")
        .unwrap_or(false);

    let dkim_verified = record
        .ses
        .receipt
        .dkim_verdict
        .as_ref()
        .map(|v| v.status == "PASS")
        .unwrap_or(false);

    // For each route, construct and send message
    for route in routes {
        let mut inbound_message = crate::handlers::inbound::build_inbound_message(&email, &route.app_name)?;

        // Update metadata with SES security info
        inbound_message.metadata.spf_verified = spf_verified;
        inbound_message.metadata.dkim_verified = dkim_verified;

        let message_json = serde_json::to_string(&inbound_message)
            .map_err(|e| MailflowError::Queue(format!("Failed to serialize message: {}", e)))?;

        let message_id = ctx
            .queue
            .send_message(&route.queue_url, &message_json)
            .await?;

        info!(
            "Sent message to queue {} (app: {}, message_id: {})",
            route.queue_url, route.app_name, message_id
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ses_event_parsing() {
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
    }
}
```

### 3. Update src/handlers/mod.rs

```rust
// Add ses module
pub mod ses;

// Update handler function
pub async fn handler(event: RuntimeEvent<Value>) -> Result<Value, Error> {
    info!("Received Lambda event");

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
```

### 4. Update src/handlers/inbound.rs

Make `build_inbound_message` public (line 126):
```rust
pub fn build_inbound_message(  // Add 'pub'
    email: &crate::models::Email,
    routing_key: &str,
) -> Result<InboundMessage, MailflowError> {
    // ... existing implementation
}
```

---

## Testing Strategy

### Test Cases

1. **Happy Path - SES Event:**
   - Send email to `_app1@domain.com`
   - Verify Lambda receives SES event
   - Verify message in `mailflow-app1-dev` queue
   - Verify SPF/DKIM metadata populated

2. **Multiple Recipients:**
   - Send email to `_app1@domain.com` and `_app2@domain.com`
   - Verify separate messages in both queues

3. **Invalid Recipient:**
   - Send email to invalid pattern (no underscore)
   - Verify error logged, sent to DLQ

4. **S3 Reference Missing:**
   - Mock SES event without S3 bucket/key
   - Verify error handling

5. **Backward Compatibility:**
   - Trigger S3 event (if S3 notification configured)
   - Verify still works

### Manual Test Commands

```bash
# Send test email
aws ses send-email \
  --from test@yourdomain.com \
  --destination ToAddresses=_app1@yourdomain.com \
  --message "Subject={Data=Test},Body={Text={Data=Hello from SES}}" \
  --region us-east-1 \
  --profile your-aws-profile

# Check Lambda logs
aws logs tail /aws/lambda/mailflow-dev \
  --region us-east-1 \
  --profile your-aws-profile \
  --since 5m \
  --follow

# Check SQS queue
aws sqs receive-message \
  --queue-url https://sqs.us-east-1.amazonaws.com/123456789012/mailflow-app1-dev \
  --max-number-of-messages 1 \
  --region us-east-1 \
  --profile your-aws-profile

# Check DLQ (should be empty)
aws sqs receive-message \
  --queue-url https://sqs.us-east-1.amazonaws.com/123456789012/mailflow-dlq-dev \
  --max-number-of-messages 1 \
  --region us-east-1 \
  --profile your-aws-profile
```

---

## Rollback Plan

If issues arise after deployment:

1. **Code Rollback:**
   ```bash
   cd infra
   pulumi up --revert
   ```

2. **Quick Fix - Disable Lambda Action:**
   ```bash
   aws ses update-receipt-rule \
     --rule-set-name mailflow-rules-dev \
     --rule '{"Name":"mailflow-inbound-dev","Enabled":false}' \
     --region us-east-1
   ```

3. **Alternative - Switch to S3 Trigger:**
   - Deploy Option 2 (S3 event notification)
   - Loses SES metadata but restores functionality

---

## Timeline

- **Design & Review:** 30 minutes ✓
- **Implementation:** 1 hour
- **Testing:** 30 minutes
- **Deployment:** 15 minutes
- **Validation:** 15 minutes

**Total:** ~2.5 hours

---

## Success Criteria

- [ ] Lambda successfully parses SES events
- [ ] Test emails appear in target SQS queues
- [ ] SPF/DKIM metadata populated correctly
- [ ] No errors in CloudWatch logs
- [ ] DLQ remains empty
- [ ] All existing S3/SQS event handling still works
- [ ] Processing latency < 5 seconds (p95)

---

## Follow-up Items

1. **Add monitoring:**
   - CloudWatch metric for SES event processing
   - Alarm for SES event parsing failures

2. **Update documentation:**
   - Document SES event format
   - Add troubleshooting guide

3. **Consider Option 2:**
   - Evaluate S3 notification approach for future
   - May simplify architecture if SES metadata not needed

---

## References

- AWS SES Receipt Rule Actions: https://docs.aws.amazon.com/ses/latest/dg/receiving-email-action.html
- AWS SES Lambda Event Format: https://docs.aws.amazon.com/ses/latest/dg/receiving-email-action-lambda-event.html
- Product Spec: `./specs/0001-spec.md`
- Infrastructure Code: `./infra/src/ses.ts`
- Event Models: `./src/models/events.rs`

---

**Prepared by:** Claude Code
**Review Status:** Ready for Implementation
