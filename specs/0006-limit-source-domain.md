# Specification: Source Email Domain Limiting

**Document Version:** 1.0
**Date:** 2025-11-02
**Status:** Implementation Ready

---

## Overview

This specification describes the implementation of source email domain limiting for the mailflow system. The feature will allow operators to restrict which email domains can send emails through the system, providing an additional layer of security against unauthorized email sources.

---

## Use Cases

### Primary Use Case
Limit inbound emails to only trusted sender domains (e.g., only accept emails from `abc.com`, `example.org`, etc.)

### Example Scenarios

1. **Corporate Email Gateway**: Only accept emails from verified business partners
   - Allowed: sender@abc.com, user@partner.com
   - Rejected: spammer@random.com

2. **Internal System**: Only accept emails from internal domains
   - Allowed: user@internal.company.com
   - Rejected: external@gmail.com

3. **Multi-tenant System**: Different environments accept different domains
   - Dev: abc.com, test.com
   - Prod: abc.com, trusted-partner.com

---

## Requirements

### Functional Requirements

**FR-1: Domain Validation**
- The system SHALL validate the sender's email domain against a configurable allowlist
- Validation SHALL occur after email parsing but before routing
- If the sender domain is not in the allowlist, the email SHALL be rejected

**FR-2: Configuration**
- The allowlist SHALL be configurable via Pulumi infrastructure code
- The allowlist SHALL support multiple domains (comma-separated)
- An empty allowlist SHALL allow all domains (backward compatible)
- Domain matching SHALL be case-insensitive

**FR-3: Error Handling**
- Rejected emails SHALL be logged with appropriate error messages
- Rejected emails SHALL be sent to the DLQ with clear rejection reason
- PII SHALL be redacted in error logs (email addresses)

**FR-4: Metrics**
- The system SHALL emit metrics for domain validation successes
- The system SHALL emit metrics for domain validation failures
- Metrics SHALL include the rejection reason

### Non-Functional Requirements

**NFR-1: Performance**
- Domain validation SHALL add < 5ms to processing time
- Implementation SHALL use efficient string matching (no regex)

**NFR-2: Security**
- Domain validation SHALL prevent spoofing attempts
- The feature SHALL integrate with existing SPF/DKIM validation
- Validation SHALL be fail-secure (reject on error)

**NFR-3: Maintainability**
- Configuration changes SHALL require infrastructure deployment
- The feature SHALL be testable via unit and integration tests

---

## Design

### Architecture Overview

```
┌─────────────────┐
│  SES receives   │
│     email       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Lambda downloads│
│  from S3        │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Parse email    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Validate sender │  ◄── NEW: Check domain allowlist
│     domain      │
└────────┬────────┘
         │
   ┌─────┴─────┐
   │           │
  YES          NO
   │           │
   │           ▼
   │     ┌──────────┐
   │     │Send to   │
   │     │   DLQ    │
   │     └──────────┘
   │
   ▼
┌─────────────────┐
│  Route to app   │
│     queues      │
└─────────────────┘
```

### Component Changes

#### 1. Configuration Model (src/models/config.rs)

Add new field to `SecurityConfig`:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    #[serde(default)]
    pub require_spf: bool,
    #[serde(default)]
    pub require_dkim: bool,
    #[serde(default)]
    pub require_dmarc: bool,
    pub max_emails_per_sender_per_hour: u32,

    // NEW: Allowed sender domains (empty = allow all)
    #[serde(default)]
    pub allowed_sender_domains: Vec<String>,
}
```

#### 2. Security Validator (src/services/security.rs)

Add new validation method:

```rust
impl SecurityValidator {
    /// Validates sender email domain against allowlist
    ///
    /// Returns Ok(()) if domain is allowed or allowlist is empty, Err otherwise
    pub fn validate_sender_domain(&self, sender_email: &str) -> Result<(), MailflowError> {
        // If allowlist is empty, allow all domains (backward compatible)
        if self.security_config.allowed_sender_domains.is_empty() {
            return Ok(());
        }

        // Extract domain from email address
        let domain = sender_email
            .split('@')
            .nth(1)
            .ok_or_else(|| MailflowError::Validation(
                format!("Invalid email address format: {}", redact_email(sender_email))
            ))?
            .to_lowercase();

        // Check if domain is in allowlist (case-insensitive)
        let allowed = self.security_config.allowed_sender_domains
            .iter()
            .any(|allowed_domain| allowed_domain.to_lowercase() == domain);

        if allowed {
            tracing::debug!(
                domain = %domain,
                "Sender domain allowed"
            );
            Ok(())
        } else {
            tracing::warn!(
                domain = %domain,
                sender = %redact_email(sender_email),
                "Sender domain not in allowlist"
            );
            Err(MailflowError::Validation(
                format!("Sender domain '{}' is not in the allowlist", domain)
            ))
        }
    }
}
```

#### 3. Inbound Handler (src/handlers/inbound.rs)

Add domain validation after parsing:

```rust
async fn process_record(
    ctx: &InboundContext,
    record: crate::models::S3EventRecord,
) -> Result<(), MailflowError> {
    // ... existing code ...

    // 3. Parse email
    let email = ctx.parser.parse(&raw_email).await?;
    info!(
        "Parsed email - from: {}, subject: {}, size: {} bytes",
        redact_email(&email.from.address),
        redact_subject(&email.subject),
        raw_email.len()
    );

    // NEW: 3.5 Validate sender domain
    let config = ctx.config.get_config().await?;
    let security_validator = SecurityValidator::new(config.security.clone());

    security_validator.validate_sender_domain(&email.from.address)?;
    info!(
        "Sender domain validated for: {}",
        redact_email(&email.from.address)
    );

    // Emit metric for successful validation
    ctx.metrics
        .record_counter("SenderDomainValidationSuccess", 1.0, &[])
        .await;

    // 4. Check rate limit
    // ... rest of existing code ...
}
```

Add error metric emission when domain validation fails (already handled by DLQ error handling):

The existing error handling in `handle()` function will catch the validation error and send to DLQ with metrics.

#### 4. Pulumi Infrastructure (infra/src/lambda.ts)

Add new environment variable:

```typescript
environment: {
    variables: {
        // ... existing variables ...
        ALLOWED_DOMAINS: domains.join(","),

        // NEW: Allowed sender domains
        ALLOWED_SENDER_DOMAINS: config.get("allowedSenderDomains") || "",
    },
}
```

#### 5. Configuration Provider (src/services/config.rs)

Update `EnvConfigProvider` to read new environment variable:

```rust
impl EnvConfigProvider {
    pub fn new() -> Result<Self, MailflowError> {
        // ... existing code ...

        // Parse allowed sender domains from environment
        let allowed_sender_domains = std::env::var("ALLOWED_SENDER_DOMAINS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>();

        // ... existing code ...

        let security = SecurityConfig {
            require_spf: false,
            require_dkim: false,
            require_dmarc: false,
            max_emails_per_sender_per_hour: 100,
            allowed_sender_domains,
        };

        // ... rest of existing code ...
    }
}
```

---

## Configuration

### Pulumi Configuration

**File: `infra/Pulumi.dev.yaml`**

```yaml
config:
  mailflow:allowedSenderDomains:
    - abc.com
    - example.org
```

**File: `infra/Pulumi.prod.yaml`**

```yaml
config:
  mailflow:allowedSenderDomains:
    - abc.com
    - trusted-partner.com
```

### Environment Variable Format

```bash
ALLOWED_SENDER_DOMAINS="abc.com,example.org,another-domain.com"
```

**Empty value** = Allow all domains (backward compatible)

---

## Implementation Plan

### Phase 1: Core Implementation

1. **Update Configuration Model** (10 min)
   - Add `allowed_sender_domains` field to `SecurityConfig`
   - Update validation in config model

2. **Implement Domain Validator** (20 min)
   - Add `validate_sender_domain()` method to `SecurityValidator`
   - Implement case-insensitive domain matching
   - Add proper error handling and logging

3. **Integrate into Inbound Handler** (15 min)
   - Add validation call after email parsing
   - Add metrics emission
   - Update error handling

4. **Update Configuration Provider** (10 min)
   - Parse `ALLOWED_SENDER_DOMAINS` environment variable
   - Handle empty/missing values (allow all)

### Phase 2: Infrastructure

5. **Update Pulumi Configuration** (10 min)
   - Add configuration parameter to `infra/src/lambda.ts`
   - Add environment variable mapping
   - Update `Pulumi.dev.yaml` with test domains

### Phase 3: Testing

6. **Unit Tests** (30 min)
   - Test domain validation logic
   - Test case-insensitivity
   - Test empty allowlist behavior
   - Test invalid email formats

7. **Integration Tests** (20 min)
   - Test end-to-end rejection flow
   - Test DLQ message format
   - Test metrics emission

8. **Manual Testing** (30 min)
   - Deploy to dev environment
   - Send test emails from allowed domain
   - Send test emails from blocked domain
   - Verify DLQ messages
   - Verify CloudWatch metrics

**Total Estimated Time: ~2.5 hours**

---

## Testing Strategy

### Unit Tests

**File: `src/services/security.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_sender_domain_allowed() {
        let config = SecurityConfig {
            require_spf: false,
            require_dkim: false,
            require_dmarc: false,
            max_emails_per_sender_per_hour: 100,
            allowed_sender_domains: vec!["abc.com".to_string()],
        };
        let validator = SecurityValidator::new(config);

        assert!(validator.validate_sender_domain("user@abc.com").is_ok());
    }

    #[test]
    fn test_validate_sender_domain_blocked() {
        let config = SecurityConfig {
            require_spf: false,
            require_dkim: false,
            require_dmarc: false,
            max_emails_per_sender_per_hour: 100,
            allowed_sender_domains: vec!["abc.com".to_string()],
        };
        let validator = SecurityValidator::new(config);

        assert!(validator.validate_sender_domain("user@blocked.com").is_err());
    }

    #[test]
    fn test_validate_sender_domain_case_insensitive() {
        let config = SecurityConfig {
            require_spf: false,
            require_dkim: false,
            require_dmarc: false,
            max_emails_per_sender_per_hour: 100,
            allowed_sender_domains: vec!["ABC.COM".to_string()],
        };
        let validator = SecurityValidator::new(config);

        assert!(validator.validate_sender_domain("user@abc.com").is_ok());
        assert!(validator.validate_sender_domain("user@ABC.COM").is_ok());
        assert!(validator.validate_sender_domain("user@AbC.cOm").is_ok());
    }

    #[test]
    fn test_validate_sender_domain_empty_allowlist() {
        let config = SecurityConfig {
            require_spf: false,
            require_dkim: false,
            require_dmarc: false,
            max_emails_per_sender_per_hour: 100,
            allowed_sender_domains: vec![],
        };
        let validator = SecurityValidator::new(config);

        // Empty allowlist should allow all domains
        assert!(validator.validate_sender_domain("user@any-domain.com").is_ok());
    }

    #[test]
    fn test_validate_sender_domain_invalid_email() {
        let config = SecurityConfig {
            require_spf: false,
            require_dkim: false,
            require_dmarc: false,
            max_emails_per_sender_per_hour: 100,
            allowed_sender_domains: vec!["abc.com".to_string()],
        };
        let validator = SecurityValidator::new(config);

        assert!(validator.validate_sender_domain("invalid-email").is_err());
    }

    #[test]
    fn test_validate_sender_domain_multiple_allowed() {
        let config = SecurityConfig {
            require_spf: false,
            require_dkim: false,
            require_dmarc: false,
            max_emails_per_sender_per_hour: 100,
            allowed_sender_domains: vec![
                "abc.com".to_string(),
                "example.org".to_string(),
                "test.net".to_string(),
            ],
        };
        let validator = SecurityValidator::new(config);

        assert!(validator.validate_sender_domain("user@abc.com").is_ok());
        assert!(validator.validate_sender_domain("user@example.org").is_ok());
        assert!(validator.validate_sender_domain("user@test.net").is_ok());
        assert!(validator.validate_sender_domain("user@blocked.com").is_err());
    }
}
```

### Integration Test

**File: `tests/test_security.rs`** (add to existing file)

```rust
#[tokio::test]
async fn test_inbound_sender_domain_validation_blocks_email() {
    // Set allowed sender domains
    std::env::set_var("ALLOWED_SENDER_DOMAINS", "abc.com,example.org");

    // Create test email from blocked domain
    let raw_email = create_test_email("blocked@unauthorized.com", "Test Subject");

    // Upload to S3
    let s3_event = upload_test_email(&raw_email).await;

    // Process inbound email
    let result = handle(s3_event).await;

    // Should fail validation
    assert!(result.is_err());

    // Check DLQ has message
    let dlq_messages = get_dlq_messages().await;
    assert_eq!(dlq_messages.len(), 1);

    let dlq_msg: Value = serde_json::from_str(&dlq_messages[0]).unwrap();
    assert!(dlq_msg["error"]
        .as_str()
        .unwrap()
        .contains("not in the allowlist"));
}

#[tokio::test]
async fn test_inbound_sender_domain_validation_allows_email() {
    // Set allowed sender domains
    std::env::set_var("ALLOWED_SENDER_DOMAINS", "abc.com,example.org");

    // Create test email from allowed domain
    let raw_email = create_test_email("user@abc.com", "Test Subject");

    // Upload to S3
    let s3_event = upload_test_email(&raw_email).await;

    // Process inbound email
    let result = handle(s3_event).await;

    // Should succeed
    assert!(result.is_ok());

    // Check app queue has message
    let app_messages = get_app_queue_messages("app1").await;
    assert_eq!(app_messages.len(), 1);
}
```

### Manual Testing

Based on existing verification plan in `specs/0004-verification-plan.md`:

#### Test 1: Send email from allowed domain

```bash
# Set environment
export AWS_PROFILE=sandbox-account-admin
export AWS_REGION=us-east-1

# Send from allowed domain
aws ses send-email \
  --from test@abc.com \
  --destination ToAddresses=_app1@staging.sandbox.tubi.io \
  --message 'Subject={Data=Allowed Domain Test},Body={Text={Data=This should work}}' \
  --region $AWS_REGION

# Wait and check logs
sleep 5
aws logs tail /aws/lambda/mailflow-dev \
  --region $AWS_REGION \
  --since 1m \
  --format short | grep -E "(Sender domain|Parsed email)"

# Expected: "Sender domain validated"
# Expected: Message appears in app1 queue
```

#### Test 2: Send email from blocked domain

```bash
# Send from blocked domain
aws ses send-email \
  --from blocked@unauthorized.com \
  --destination ToAddresses=_app1@staging.sandbox.tubi.io \
  --message 'Subject={Data=Blocked Domain Test},Body={Text={Data=This should fail}}' \
  --region $AWS_REGION

# Wait and check logs
sleep 5
aws logs tail /aws/lambda/mailflow-dev \
  --region $AWS_REGION \
  --since 1m \
  --format short | grep -E "(Sender domain|not in allowlist)"

# Expected: "Sender domain 'unauthorized.com' is not in the allowlist"
# Expected: Message appears in DLQ
```

#### Test 3: Check DLQ for rejection message

```bash
# Check DLQ
aws sqs receive-message \
  --queue-url $QUEUE_DLQ \
  --max-number-of-messages 1 \
  --region $AWS_REGION \
  --output json | jq '.Messages[0].Body | fromjson | {
    error,
    errorType,
    handler
  }'

# Expected output:
# {
#   "error": "Validation error: Sender domain 'unauthorized.com' is not in the allowlist",
#   "errorType": "Validation",
#   "handler": "inbound"
# }
```

#### Test 4: Check CloudWatch Metrics

```bash
# Check domain validation metrics
aws cloudwatch get-metric-statistics \
  --namespace Mailflow \
  --metric-name SenderDomainValidationSuccess \
  --start-time $(date -u -v-1H '+%Y-%m-%dT%H:%M:%S') \
  --end-time $(date -u '+%Y-%m-%dT%H:%M:%S') \
  --period 300 \
  --statistics Sum \
  --region $AWS_REGION

# Check DLQ metrics for failures
aws cloudwatch get-metric-statistics \
  --namespace Mailflow \
  --metric-name DLQMessages \
  --dimensions Name=handler,Value=inbound \
  --start-time $(date -u -v-1H '+%Y-%m-%dT%H:%M:%S') \
  --end-time $(date -u '+%Y-%m-%dT%H:%M:%S') \
  --period 300 \
  --statistics Sum \
  --region $AWS_REGION
```

---

## Metrics

### New Metrics

| Metric Name | Type | Dimensions | Description |
|------------|------|------------|-------------|
| `SenderDomainValidationSuccess` | Counter | none | Number of emails that passed domain validation |
| `SenderDomainValidationFailure` | Counter | `domain` | Number of emails rejected due to domain validation |

Note: Failures are already tracked via existing `DLQMessages` metric with `handler=inbound` dimension.

---

## Error Messages

### Validation Errors

**Blocked Domain:**
```
Validation error: Sender domain 'unauthorized.com' is not in the allowlist
```

**Invalid Email Format:**
```
Validation error: Invalid email address format: ***@[domain]
```

### DLQ Message Format

```json
{
  "error": "Validation error: Sender domain 'unauthorized.com' is not in the allowlist",
  "errorType": "Validation",
  "handler": "inbound",
  "timestamp": "2025-11-02T10:30:00Z",
  "context": {
    "bucket": "mailflow-raw-emails-dev",
    "key": "message-id-123"
  }
}
```

---

## Security Considerations

1. **Domain Spoofing**: This feature checks the `From` header, which can be spoofed. It should be used in conjunction with SPF/DKIM validation for production use.

2. **Subdomain Handling**: The implementation matches exact domains only. `user@sub.abc.com` will NOT match allowlist entry `abc.com`. Operators must explicitly list subdomains.

3. **Case Sensitivity**: Domain matching is case-insensitive per RFC 5321.

4. **PII Protection**: All error logs redact email addresses using existing `redact_email()` function.

5. **Fail-Secure**: If domain extraction fails or allowlist parsing fails, the email is rejected.

---

## Backward Compatibility

- **Empty allowlist** = Allow all domains (current behavior)
- **Missing environment variable** = Allow all domains
- Existing deployments without `ALLOWED_SENDER_DOMAINS` will continue to work unchanged

---

## Rollback Plan

If issues are discovered:

1. **Immediate**: Set `ALLOWED_SENDER_DOMAINS=""` via Pulumi config
2. **Deploy**: Run `pulumi up` to update Lambda environment
3. **Verify**: Check that all emails are being processed again

No code changes needed for rollback - just configuration.

---

## Documentation Updates

Update the following files after implementation:

1. `README.md` - Add configuration section for sender domain limiting
2. `specs/0004-verification-plan.md` - Add domain validation verification steps
3. `docs/configuration.md` (if exists) - Document new environment variable

---

## Success Criteria

- [ ] Unit tests pass for all domain validation scenarios
- [ ] Integration tests pass for allowed and blocked domains
- [ ] Manual testing shows emails from allowed domains are processed
- [ ] Manual testing shows emails from blocked domains go to DLQ
- [ ] CloudWatch metrics show validation successes
- [ ] DLQ messages contain clear rejection reasons
- [ ] PII is redacted in all error logs
- [ ] Configuration is documented in Pulumi files
- [ ] Performance impact is < 5ms per email

---

## References

- RFC 5321 - Simple Mail Transfer Protocol (domain case-insensitivity)
- Existing security implementation: `src/services/security.rs`
- Existing inbound handler: `src/handlers/inbound.rs`
- Verification plan: `specs/0004-verification-plan.md`
