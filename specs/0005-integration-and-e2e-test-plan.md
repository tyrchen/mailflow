# Integration and E2E Test Plan: Mail Dispatcher

**Document Version:** 1.0
**Date:** 2025-11-02
**Purpose:** Comprehensive integration and end-to-end testing strategy
**Test Framework:** Python + pytest + boto3

---

## Overview

This document defines comprehensive integration and E2E tests for the mailflow system to validate all flows work correctly in a real AWS environment.

---

## 1. Test Strategy

### 1.1 Test Pyramid

```
        ┌─────────────────┐
        │   E2E Tests     │  ← 10 tests (Full system flows)
        │    (Python)     │
        ├─────────────────┤
        │ Integration     │  ← 20 tests (Component interactions)
        │     Tests       │
        ├─────────────────┤
        │  Unit Tests     │  ← 68 tests (Already implemented in Rust)
        │    (Rust)       │
        └─────────────────┘
```

### 1.2 Test Environments

- **Local:** Mock AWS services (localstack)
- **Dev:** Real AWS dev environment (yourdomain.com)
- **Staging:** Production-like environment
- **Production:** Smoke tests only

---

## 2. Integration Test Categories

### 2.1 Inbound Email Flow Tests

**Test ID:** INT-001 to INT-008

| Test ID | Test Name                       | Description                               | Validates             |
|---------|---------------------------------|-------------------------------------------|-----------------------|
| INT-001 | Simple email routing            | Send plain text email, verify in SQS      | Basic flow            |
| INT-002 | Email with single attachment    | Email + PDF, verify attachment in S3      | Attachment processing |
| INT-003 | Email with multiple attachments | Email + 3 files, verify all extracted     | Multi-attachment      |
| INT-004 | Email with inline images        | HTML + embedded images, verify extraction | FR-1.8                |
| INT-005 | Multi-app routing               | Email to 2 apps, verify both queues       | FR-1.13               |
| INT-006 | Large email handling            | 35 MB email, verify processing            | FR-1.3                |
| INT-007 | Email with special characters   | UTF-8, emojis, unicode                    | FR-1.7                |
| INT-008 | Email threading                 | Reply with In-Reply-To, verify headers    | FR-2.11               |

### 2.2 Outbound Email Flow Tests

**Test ID:** INT-009 to INT-015

| Test ID | Test Name                          | Description                           | Validates        |
|---------|------------------------------------|---------------------------------------|------------------|
| INT-009 | Simple outbound send               | SQS → Lambda → SES                    | Basic flow       |
| INT-010 | Outbound with attachment           | Fetch from S3, compose, send          | FR-2.8, FR-2.9   |
| INT-011 | Outbound with multiple attachments | 3 attachments from S3                 | Multi-attachment |
| INT-012 | Outbound size validation           | >10 MB attachments, verify rejection  | FR-2.9           |
| INT-013 | Idempotency check                  | Send same correlation_id twice        | NFR-2.4          |
| INT-014 | SES quota handling                 | Exceed quota, verify rejection        | FR-2.14, FR-2.15 |
| INT-015 | Threading headers                  | Reply chain, verify headers preserved | FR-2.11          |

### 2.3 Security & Validation Tests

**Test ID:** INT-016 to INT-022

| Test ID | Test Name                 | Description                                 | Validates |
|---------|---------------------------|---------------------------------------------|-----------|
| INT-016 | Blocked file type         | Send .exe file, verify rejected             | FR-1.17   |
| INT-017 | Magic byte validation     | PDF with wrong magic bytes, verify rejected | FR-1.17   |
| INT-018 | Path traversal protection | Filename with ../, verify sanitized         | SEC-001   |
| INT-019 | Rate limiting             | Send >100 emails/hour, verify blocked       | NFR-3.7   |
| INT-020 | PII redaction             | Check logs for email addresses              | NFR-3.9   |
| INT-021 | Unverified sender         | Send from unverified address, verify error  | FR-2.13   |
| INT-022 | Non-existent queue        | Route to missing queue, verify DLQ          | FR-1.12   |

### 2.4 Error Handling Tests

**Test ID:** INT-023 to INT-027

| Test ID | Test Name                  | Description                              | Validates |
|---------|----------------------------|------------------------------------------|-----------|
| INT-023 | DLQ routing                | Trigger error, verify DLQ message        | NFR-2.6   |
| INT-024 | Retry on transient failure | Simulate throttle, verify retry          | FR-2.16   |
| INT-025 | S3 download failure        | Missing S3 object, verify error handling | -         |
| INT-026 | Invalid email format       | Malformed MIME, verify error             | -         |
| INT-027 | Queue deletion failure     | Simulate failure, verify logged          | CRIT-003  |

### 2.5 Metrics & Observability Tests

**Test ID:** INT-028 to INT-030

| Test ID | Test Name        | Description                              | Validates |
|---------|------------------|------------------------------------------|-----------|
| INT-028 | Metrics emission | Process email, verify CloudWatch metrics | NFR-5.2   |
| INT-029 | Log correlation  | Verify Lambda request ID in logs         | NFR-5.5   |
| INT-030 | Tracing spans    | Verify span creation and context         | NFR-5.5   |

---

## 3. End-to-End Test Scenarios

### 3.1 E2E-001: Complete Inbound Flow

**Scenario:** External email → SES → Lambda → SQS → App receives

**Steps:**

1. Send email via SES to `_app1@yourdomain.com`
2. Wait for SES to receive and trigger Lambda
3. Verify Lambda processes email
4. Verify message appears in app1 SQS queue
5. Verify message format matches spec
6. Verify attachments stored in S3
7. Verify presigned URLs work
8. Verify metrics emitted

**Expected:**

- Message in SQS within 10 seconds
- All metadata correct
- Attachments accessible
- No DLQ messages

**Asserts:**

- `message.version == "1.0"`
- `message.source == "mailflow"`
- `message.metadata.routing_key == "app1"`
- `len(message.email.attachments) == expected_count`
- All presigned URLs return 200

---

### 3.2 E2E-002: Complete Outbound Flow

**Scenario:** App sends → SQS → Lambda → SES → External email

**Steps:**

1. Construct outbound message JSON
2. Send to mailflow-outbound SQS queue
3. Wait for Lambda to process
4. Verify SES send via CloudWatch logs
5. Verify idempotency record created
6. Verify message deleted from queue
7. Send duplicate (same correlation_id)
8. Verify duplicate skipped

**Expected:**

- Email sent via SES within 10 seconds
- SES MessageId logged
- Idempotency prevents duplicate
- Metrics emitted

**Asserts:**

- Log contains "Sent email via SES"
- DynamoDB has idempotency record
- Second send skipped
- SQS queue empty

---

### 3.3 E2E-003: Round-trip Reply Flow

**Scenario:** Receive email → App replies → Reply sent

**Steps:**

1. Send email to `_app1@domain.com` with Message-ID
2. Receive from app1 queue
3. Construct reply with In-Reply-To header
4. Send reply to outbound queue
5. Verify reply sent with threading headers
6. Verify In-Reply-To and References headers present

**Expected:**

- Full email conversation chain maintained
- Threading headers preserved
- Reply-To address correct

**Asserts:**

- Reply has `In-Reply-To: <original-message-id>`
- `References` header contains chain
- From address is `_app1@domain.com`

---

### 3.4 E2E-004: Attachment Round-trip

**Scenario:** Receive email with PDF → App replies with same PDF

**Steps:**

1. Send email with document.pdf attachment
2. Receive message, get presigned URL
3. Download PDF via presigned URL
4. Verify PDF integrity (MD5 checksum)
5. Construct reply referencing same S3 object
6. Send outbound with attachment
7. Verify email sent with attachment

**Expected:**

- Attachment preserved exactly
- MD5 matches original
- Presigned URL works
- Outbound fetch succeeds
- SES sends <10 MB

**Asserts:**

- `attachment.checksum_md5 == original_md5`
- Presigned URL download works
- Outbound email has attachment
- Total size < 10 MB

---

### 3.5 E2E-005: Security Validation Flow

**Scenario:** Test file type security

**Steps:**

1. Send email with allowed file (PDF)
2. Verify processed successfully
3. Send email with blocked file (.exe)
4. Verify rejected and in DLQ
5. Send email with fake PDF (wrong magic bytes)
6. Verify rejected
7. Check logs for PII redaction

**Expected:**

- PDF: Success
- EXE: Rejected with clear error
- Fake PDF: Rejected
- No PII in logs

**Asserts:**

- PDF in SQS
- EXE in DLQ with validation error
- Fake PDF in DLQ
- Logs show `***@domain` not full emails

---

### 3.6 E2E-006: Rate Limiting Flow

**Scenario:** Test sender rate limits

**Steps:**

1. Send 50 emails from same sender rapidly
2. Verify all processed (under limit)
3. Send 60 more emails (total 110 > 100 limit)
4. Verify last 10 rejected
5. Check DLQ for rate limit errors
6. Wait 1 hour
7. Send 1 email, verify succeeds

**Expected:**

- First 100: Success
- Next 10: Rate limited
- After window: Success

**Asserts:**

- 100 messages in app queue
- 10 messages in DLQ with "Rate limit exceeded"
- After reset: New message succeeds

---

### 3.7 E2E-007: Error Recovery Flow

**Scenario:** Test retry and error handling

**Steps:**

1. Create queue that doesn't exist in routing
2. Send email to non-existent app
3. Verify error in DLQ
4. Create the queue
5. Resend email
6. Verify now succeeds

**Expected:**

- First send: DLQ with routing error
- After queue created: Success

**Asserts:**

- DLQ message contains queue URL
- Error is non-retriable
- Second attempt succeeds

---

### 3.8 E2E-008: Multi-Recipient Routing

**Scenario:** Email to multiple apps

**Steps:**

1. Send email to `_app1@domain.com, _app2@domain.com`
2. Verify message in app1 queue
3. Verify message in app2 queue
4. Verify both have same email content
5. Verify separate routing decisions logged

**Expected:**

- 2 messages (one per app)
- Same content
- Different routing_key

**Asserts:**

- `app1_msg.metadata.routing_key == "app1"`
- `app2_msg.metadata.routing_key == "app2"`
- `app1_msg.email.subject == app2_msg.email.subject`

---

### 3.9 E2E-009: Attachment Size Limits

**Scenario:** Test size validation

**Steps:**

1. **Inbound:** Send email with 35 MB attachment
2. Verify processed successfully
3. **Inbound:** Send email with 50 MB attachment
4. Verify rejected (>40 MB limit)
5. **Outbound:** Send message with 9 MB attachment
6. Verify sent successfully
7. **Outbound:** Send message with 11 MB attachment
8. Verify rejected (>10 MB SES limit)

**Expected:**

- 35 MB inbound: Success
- 50 MB inbound: Rejected
- 9 MB outbound: Success
- 11 MB outbound: Rejected

**Asserts:**

- Error messages contain size limits
- Successful attachments have correct size in metadata

---

### 3.10 E2E-010: Performance & Scalability

**Scenario:** Load testing

**Steps:**

1. Send 100 emails within 1 minute
2. Verify all processed within 5 seconds p95
3. Check Lambda memory usage < 128 MB
4. Verify no throttling errors
5. Check DLQ is empty
6. Verify metrics show correct counts

**Expected:**

- All 100 processed
- p95 latency < 5s
- Memory efficient
- Zero errors

**Asserts:**

- 100 messages in queues
- CloudWatch p95 latency < 5000ms
- Max memory < 128 MB
- DLQ count == 0

---

## 4. Test Implementation Structure

### 4.1 Directory Structure

```
e2e/
├── pyproject.toml              # uv project config
├── pytest.ini                  # pytest config
├── conftest.py                 # pytest fixtures
├── requirements.txt            # Python dependencies
├── README.md                   # Test documentation
├── tests/
│   ├── test_e2e_inbound.py       # E2E-001
│   ├── test_e2e_outbound.py      # E2E-002
│   ├── test_e2e_roundtrip.py     # E2E-003
│   ├── test_e2e_attachments.py   # E2E-004
│   ├── test_e2e_security.py      # E2E-005
│   ├── test_e2e_rate_limiting.py # E2E-006
│   ├── test_e2e_error_recovery.py # E2E-007
│   ├── test_e2e_multi_routing.py # E2E-008
│   ├── test_e2e_size_limits.py   # E2E-009
│   └── test_e2e_performance.py   # E2E-010
├── utils/
│   ├── __init__.py
│   ├── aws_helpers.py          # AWS SDK wrappers
│   ├── email_builder.py        # Email construction
│   ├── message_validator.py    # Message format validation
│   └── test_helpers.py         # Common test utilities
└── fixtures/
    ├── emails/
    │   ├── simple.eml
    │   ├── with-attachment.eml
    │   ├── html-inline-images.eml
    │   ├── multipart-alternative.eml
    │   └── threading-reply.eml
    ├── attachments/
    │   ├── test.pdf
    │   ├── test.jpg
    │   ├── large-35mb.bin
    │   └── blocked.exe
    └── messages/
        ├── outbound-simple.json
        ├── outbound-with-attachment.json
        └── outbound-reply.json
```

---

## 5. Test Framework Setup

### 5.1 Dependencies (pyproject.toml)

```toml
[project]
name = "mailflow-e2e-tests"
version = "0.1.0"
description = "End-to-end tests for mailflow"
requires-python = ">=3.11"

dependencies = [
    "boto3>=1.34.0",
    "pytest>=8.0.0",
    "pytest-asyncio>=0.23.0",
    "pytest-timeout>=2.2.0",
    "pytest-xdist>=3.5.0",  # Parallel test execution
    "email-validator>=2.1.0",
    "faker>=22.0.0",  # Generate test data
    "tenacity>=8.2.0",  # Retry logic for waiting
]

[project.optional-dependencies]
dev = [
    "pytest-cov>=4.1.0",
    "pytest-html>=4.1.0",
    "black>=24.0.0",
    "ruff>=0.1.0",
]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"
```

### 5.2 Environment Variables

```bash
# Required for tests
export AWS_PROFILE=your-aws-profile
export AWS_REGION=us-east-1
export TEST_ENVIRONMENT=dev

# Test targets
export LAMBDA_FUNCTION=mailflow-dev
export APP1_QUEUE_URL=https://sqs.us-east-1.amazonaws.com/123456789012/mailflow-app1-dev
export APP2_QUEUE_URL=https://sqs.us-east-1.amazonaws.com/123456789012/mailflow-app2-dev
export OUTBOUND_QUEUE_URL=https://sqs.us-east-1.amazonaws.com/123456789012/mailflow-outbound-dev
export DLQ_URL=https://sqs.us-east-1.amazonaws.com/123456789012/mailflow-dlq-dev
export RAW_EMAILS_BUCKET=mailflow-raw-emails-dev
export ATTACHMENTS_BUCKET=mailflow-attachments-dev
export TEST_DOMAIN=yourdomain.com
export TEST_FROM_EMAIL=test@yourdomain.com
```

---

## 6. Key Test Patterns

### 6.1 Pattern: Send Email and Wait for Processing

```python
async def send_and_wait_for_processing(
    ses_client,
    logs_client,
    to_address: str,
    subject: str,
    body: str,
    timeout: int = 30
) -> dict:
    """Send email and wait for Lambda processing"""
    # 1. Send email
    response = await ses_client.send_email(
        Source=TEST_FROM_EMAIL,
        Destination={'ToAddresses': [to_address]},
        Message={
            'Subject': {'Data': subject},
            'Body': {'Text': {'Data': body}}
        }
    )
    message_id = response['MessageId']

    # 2. Wait for Lambda log entry
    start_time = time.time()
    while time.time() - start_time < timeout:
        logs = await get_recent_logs(logs_client, 'Parsed email')
        if any(message_id in log for log in logs):
            return {'message_id': message_id, 'processed': True}
        await asyncio.sleep(1)

    raise TimeoutError(f"Email {message_id} not processed within {timeout}s")
```

### 6.2 Pattern: Receive and Validate SQS Message

```python
async def receive_and_validate_message(
    sqs_client,
    queue_url: str,
    expected_subject: str,
    timeout: int = 30
) -> dict:
    """Receive message from SQS and validate format"""
    # 1. Poll queue
    start_time = time.time()
    while time.time() - start_time < timeout:
        response = await sqs_client.receive_message(
            QueueUrl=queue_url,
            MaxNumberOfMessages=1,
            WaitTimeSeconds=10
        )

        if 'Messages' in response:
            message = json.loads(response['Messages'][0]['Body'])

            # 2. Validate message format
            assert message['version'] == '1.0'
            assert message['source'] == 'mailflow'
            assert 'email' in message
            assert 'metadata' in message

            # 3. Validate content
            if expected_subject:
                assert message['email']['subject'] == expected_subject

            return message

    raise TimeoutError(f"No message received in {timeout}s")
```

### 6.3 Pattern: Verify Attachment Accessible

```python
async def verify_attachment_accessible(
    attachment_metadata: dict
) -> bytes:
    """Download attachment via presigned URL and verify"""
    import requests

    url = attachment_metadata['presignedUrl']

    # 1. Download via presigned URL (no auth needed)
    response = requests.get(url)
    assert response.status_code == 200, f"Failed to download: {response.status_code}"

    # 2. Verify size matches
    content = response.content
    assert len(content) == attachment_metadata['size']

    # 3. Verify MD5 checksum
    import hashlib
    actual_md5 = hashlib.md5(content).hexdigest()
    expected_md5 = attachment_metadata.get('checksum_md5')
    if expected_md5:
        assert actual_md5 == expected_md5, "MD5 mismatch"

    return content
```

### 6.4 Pattern: Check Metrics Emitted

```python
async def verify_metrics_emitted(
    cloudwatch_client,
    metric_name: str,
    namespace: str = 'Mailflow',
    expected_min: int = 1,
    lookback_minutes: int = 5
) -> bool:
    """Verify CloudWatch metric was emitted"""
    from datetime import datetime, timedelta

    end_time = datetime.utcnow()
    start_time = end_time - timedelta(minutes=lookback_minutes)

    response = await cloudwatch_client.get_metric_statistics(
        Namespace=namespace,
        MetricName=metric_name,
        StartTime=start_time,
        EndTime=end_time,
        Period=300,
        Statistics=['Sum']
    )

    datapoints = response['Datapoints']
    if not datapoints:
        return False

    total = sum(dp['Sum'] for dp in datapoints)
    return total >= expected_min
```

---

## 7. Test Data Management

### 7.1 Email Fixtures

**Simple Email (fixtures/emails/simple.eml):**

```
From: test@yourdomain.com
To: _app1@yourdomain.com
Subject: Simple Test Email
MIME-Version: 1.0
Content-Type: text/plain; charset=UTF-8

This is a simple test email body.
```

**Email with Attachment (fixtures/emails/with-attachment.eml):**

```
From: test@yourdomain.com
To: _app1@yourdomain.com
Subject: Email with PDF Attachment
MIME-Version: 1.0
Content-Type: multipart/mixed; boundary="boundary123"

--boundary123
Content-Type: text/plain

Email with attachment.

--boundary123
Content-Type: application/pdf; name="test.pdf"
Content-Disposition: attachment; filename="test.pdf"
Content-Transfer-Encoding: base64

JVBERi0xLjQKJeLjz9MKMSAwIG9iago8PAovVHlwZSAvQ2F0YWxvZwovUGFnZXMgMiAwIFIKPj4K
...
--boundary123--
```

### 7.2 Message Fixtures

**Outbound Message (fixtures/messages/outbound-simple.json):**

```json
{
  "version": "1.0",
  "correlationId": "test-{uuid}",
  "timestamp": "{iso8601}",
  "source": "test-suite",
  "email": {
    "from": {
      "address": "_app1@yourdomain.com",
      "name": "Test App"
    },
    "to": [
      {
        "address": "recipient@example.com",
        "name": "Test Recipient"
      }
    ],
    "cc": [],
    "bcc": [],
    "replyTo": {
      "address": "_app1@yourdomain.com"
    },
    "subject": "Test Response",
    "body": {
      "text": "This is a test response",
      "html": "<p>This is a test response</p>"
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
}
```

---

## 8. Test Execution Strategy

### 8.1 Test Phases

**Phase 1: Integration Tests (Local Dev)**

- Run against dev environment
- Fast feedback (5-10 minutes)
- Covers individual components
- Can run in parallel

**Phase 2: E2E Tests (Dev Environment)**

- Run against dev environment
- Comprehensive scenarios (15-20 minutes)
- Sequential execution (to avoid interference)
- Full system validation

**Phase 3: Load Tests (Staging)**

- Performance validation
- Scalability testing
- Run separately (30 minutes)
- Requires dedicated environment

### 8.2 CI/CD Integration

```yaml
# .github/workflows/integration-tests.yml
name: Integration Tests

on:
  push:
    branches: [main, develop]
  pull_request:

jobs:
  integration-tests:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Setup Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.11'

      - name: Install uv
        run: curl -LsSf https://astral.sh/uv/install.sh | sh

      - name: Install dependencies
        run: cd e2e && uv pip install -r requirements.txt

      - name: Run integration tests
        run: cd e2e && pytest tests/ -v --junit-xml=results.xml
        env:
          AWS_PROFILE: ${{ secrets.AWS_PROFILE }}
          AWS_REGION: us-east-1
          TEST_ENVIRONMENT: dev

      - name: Upload test results
        uses: actions/upload-artifact@v4
        with:
          name: test-results
          path: e2e/results.xml
```

### 8.3 Test Execution Commands

```bash
# Run all integration tests
cd e2e
uv run pytest tests/ -v
# Run with coverage
uv run pytest tests/ --cov=utils --cov-report=html

# Run in parallel (fast)
uv run pytest tests/ -n 4

# Run with timeout protection
uv run pytest tests/ --timeout=300
```

---

## 9. Assertions & Validations

### 9.1 Message Format Validation

```python
def validate_inbound_message(message: dict):
    """Validate inbound message matches spec FR-1.20"""
    # Version and source
    assert message['version'] == '1.0'
    assert message['source'] == 'mailflow'
    assert 'messageId' in message
    assert message['messageId'].startswith('mailflow-')

    # Email structure
    email = message['email']
    assert 'from' in email
    assert 'address' in email['from']
    assert 'to' in email and len(email['to']) > 0
    assert 'subject' in email
    assert 'body' in email
    assert 'text' in email['body'] or 'html' in email['body']

    # Metadata
    metadata = message['metadata']
    assert 'routingKey' in metadata
    assert 'domain' in metadata
    assert 'spfVerified' in metadata
    assert 'dkimVerified' in metadata

    # Attachments (if present)
    if email.get('attachments'):
        for attachment in email['attachments']:
            assert 'filename' in attachment
            assert 'sanitizedFilename' in attachment
            assert 'contentType' in attachment
            assert 'size' in attachment
            assert 's3Bucket' in attachment
            assert 's3Key' in attachment
            assert 'presignedUrl' in attachment
            assert 'presignedUrlExpiration' in attachment
            assert 'status' in attachment
            assert attachment['status'] in ['available', 'failed']

            if attachment.get('checksumMd5'):
                # Validate MD5 format
                assert len(attachment['checksumMd5']) == 32
```

### 9.2 Performance Assertions

```python
def validate_performance(duration_ms: float, memory_mb: float):
    """Validate performance meets NFR requirements"""
    # NFR-1.1: Inbound processing < 5s p95
    assert duration_ms < 5000, f"Duration {duration_ms}ms exceeds 5s limit"

    # NFR-1.5: Memory < 128 MB
    assert memory_mb < 128, f"Memory {memory_mb}MB exceeds 128MB limit"

    # Good performance targets
    if duration_ms < 1000:
        print(f"✅ Excellent performance: {duration_ms}ms")
    elif duration_ms < 3000:
        print(f"✅ Good performance: {duration_ms}ms")
    else:
        print(f"⚠️  Acceptable but slow: {duration_ms}ms")
```

### 9.3 Security Assertions

```python
def validate_pii_redaction(log_entries: list):
    """Validate no PII in logs (NFR-3.9)"""
    import re

    # Email pattern
    email_pattern = re.compile(r'[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}')

    for log in log_entries:
        # Find all email-like strings
        matches = email_pattern.findall(log['message'])

        for match in matches:
            # Should be redacted (***@domain.com)
            assert match.startswith('***@'), \
                f"Found unredacted email in logs: {match}"
```

---

## 10. Test Utilities

### 10.1 AWS Helpers (utils/aws_helpers.py)

```python
class AWSTestHelper:
    """Helper class for AWS operations in tests"""

    async def send_raw_email(self, email_file: str) -> str:
        """Send raw email from file"""
        with open(email_file, 'rb') as f:
            email_data = f.read()

        response = await self.ses.send_raw_email(
            RawMessage={'Data': email_data}
        )
        return response['MessageId']

    async def wait_for_sqs_message(
        self,
        queue_url: str,
        timeout: int = 30
    ) -> dict:
        """Wait for message to appear in queue"""
        # Implementation with tenacity retry

    async def get_lambda_logs(
        self,
        log_group: str,
        filter_pattern: str,
        minutes: int = 5
    ) -> list:
        """Get recent Lambda logs"""

    async def get_cloudwatch_metric(
        self,
        metric_name: str,
        namespace: str = 'Mailflow',
        minutes: int = 5
    ) -> float:
        """Get metric value from CloudWatch"""

    async def purge_queue(self, queue_url: str):
        """Purge SQS queue for clean test state"""

    async def cleanup_s3_test_data(self, bucket: str, prefix: str):
        """Clean up S3 test data"""
```

### 10.2 Email Builder (utils/email_builder.py)

```python
class EmailBuilder:
    """Builder for test emails"""

    def __init__(self):
        self.from_addr = None
        self.to_addrs = []
        self.subject = None
        self.body_text = None
        self.body_html = None
        self.attachments = []

    def from_address(self, email: str, name: str = None):
        self.from_addr = {'email': email, 'name': name}
        return self

    def to(self, email: str, name: str = None):
        self.to_addrs.append({'email': email, 'name': name})
        return self

    def with_subject(self, subject: str):
        self.subject = subject
        return self

    def with_text_body(self, text: str):
        self.body_text = text
        return self

    def with_html_body(self, html: str):
        self.body_html = html
        return self

    def attach_file(self, filename: str, content_type: str, data: bytes):
        self.attachments.append({
            'filename': filename,
            'content_type': content_type,
            'data': data
        })
        return self

    def build_mime(self) -> bytes:
        """Build MIME message"""
        from email.mime.multipart import MIMEMultipart
        from email.mime.text import MIMEText
        from email.mime.application import MIMEApplication

        # Implementation...
```

---

## 11. Expected Test Coverage

### 11.1 Coverage Targets

| Category       | Integration Tests | E2E Tests | Total  |
|----------------|-------------------|-----------|--------|
| Inbound Flow   | 8                 | 3         | 11     |
| Outbound Flow  | 7                 | 3         | 10     |
| Security       | 7                 | 2         | 9      |
| Error Handling | 5                 | 1         | 6      |
| Observability  | 3                 | 1         | 4      |
| **Total**      | **30**            | **10**    | **40** |

### 11.2 Test Execution Time

| Phase             | Tests | Duration  | Run Frequency           |
|-------------------|-------|-----------|-------------------------|
| Unit Tests (Rust) | 68    | 1.1s      | Every commit            |
| Integration Tests | 30    | 10-15 min | Every PR                |
| E2E Tests         | 10    | 15-20 min | Daily / Before release  |
| Load Tests        | 3     | 30 min    | Weekly / Before release |

---

## 12. Success Criteria

### 12.1 Test Success Criteria

**All tests must:**

- ✅ Pass consistently (no flakiness)
- ✅ Complete within timeout
- ✅ Clean up resources after run
- ✅ Provide clear failure messages
- ✅ Be idempotent (can run multiple times)

**Integration tests must validate:**

- ✅ All spec functional requirements (FR-*)
- ✅ Message format compliance
- ✅ Error handling
- ✅ Performance within limits

**E2E tests must validate:**

- ✅ Complete user workflows
- ✅ Cross-component integration
- ✅ Real AWS service interaction
- ✅ Production-like scenarios

### 12.2 Acceptance Criteria

**Before production deployment:**

- [ ] All 68 unit tests passing
- [ ] All 30 integration tests passing
- [ ] All 10 E2E tests passing
- [ ] Load test: 100 emails/min for 10 minutes
- [ ] Zero DLQ messages during test run
- [ ] p95 latency < 5 seconds
- [ ] Memory usage < 128 MB
- [ ] No security vulnerabilities found

---

## 13. Test Maintenance

### 13.1 Flaky Test Handling

If a test fails intermittently:

1. Add retry logic with tenacity
2. Increase timeouts if needed
3. Add better wait conditions
4. Log more context for debugging
5. Mark as `@pytest.mark.flaky(reruns=3)` temporarily

### 13.2 Test Data Cleanup

**After each test:**

- Purge test SQS queues
- Delete S3 test objects
- Clear idempotency records (if test table)
- Reset rate limiter counters (if test table)

**Cleanup utility:**

```python
@pytest.fixture(autouse=True)
async def cleanup_test_data():
    """Auto cleanup after each test"""
    yield  # Run test

    # Cleanup
    await purge_all_test_queues()
    await delete_s3_test_prefix()
    await clear_test_dynamodb_records()
```

---

## 14. Monitoring Test Health

### 14.1 Test Metrics to Track

- Test pass rate (target: 100%)
- Test execution time (track trends)
- Flaky test count (target: 0)
- Coverage percentage (target: >85%)

### 14.2 Test Dashboards

Create dashboard showing:

- Test runs per day
- Pass/fail trends
- Performance metrics
- Coverage over time

---

## 15. Implementation Checklist

### 15.1 Setup Tasks

- [ ] Create `e2e/` directory structure
- [ ] Set up `pyproject.toml` with uv
- [ ] Create pytest configuration
- [ ] Implement AWS helper utilities
- [ ] Create email builder utility
- [ ] Generate test fixtures

### 15.2 Test Implementation Tasks

- [ ] Implement 8 inbound integration tests
- [ ] Implement 7 outbound integration tests
- [ ] Implement 7 security tests
- [ ] Implement 5 error handling tests
- [ ] Implement 3 observability tests
- [ ] Implement 10 E2E test scenarios

### 15.3 Verification Tasks

- [ ] All tests pass in dev environment
- [ ] Tests are not flaky (3 runs, all pass)
- [ ] Test cleanup works (no leaked resources)
- [ ] Documentation complete
- [ ] CI/CD integration working

---

## 16. Risk Mitigation

### 16.1 Test Risks

| Risk                            | Mitigation                                          |
|---------------------------------|-----------------------------------------------------|
| Tests interfere with each other | Use unique message IDs, separate queues             |
| AWS API rate limits             | Implement backoff, run tests sequentially if needed |
| Test data accumulation          | Auto cleanup after each test                        |
| Flaky network issues            | Retry logic, generous timeouts                      |
| Cost of running tests           | Use dev environment, cleanup promptly               |

### 16.2 Environment Isolation

- Use dedicated test queues (`mailflow-app1-test`)
- Use test S3 prefix (`test-runs/{timestamp}/`)
- Use test DynamoDB tables
- Clean up after test suite completes

---

## 17. Documentation

### 17.1 Test Documentation

Each test file should include:

- Module docstring explaining test category
- Individual test docstrings with:
  - What is being tested
  - Test data used
  - Expected behavior
  - Related spec requirements

### 17.2 Troubleshooting Guide

**Common test failures:**

- Timeout waiting for message → Check Lambda logs, increase timeout
- Assertion error on message format → Check spec version, validate JSON schema
- S3 access denied → Verify IAM permissions
- SQS queue not found → Check environment variables
- Rate limit errors → Wait between test runs

---

## 18. Next Steps

**Phase 1: Setup (Day 1)**

1. Create e2e directory structure
2. Set up Python project with uv
3. Implement AWS helper utilities
4. Create test fixtures

**Phase 2: Integration Tests (Day 2-3)**

1. Implement inbound flow tests
2. Implement outbound flow tests
3. Implement security tests
4. Implement error handling tests

**Phase 3: E2E Tests (Day 4-5)**

1. Implement 10 E2E scenarios
2. Run and debug in dev environment
3. Optimize for speed and reliability
4. Document any limitations

**Phase 4: CI/CD Integration (Day 6)**

1. Set up GitHub Actions workflow
2. Configure test reporting
3. Add to PR checks
4. Document for team

---

## Appendix A: AWS Permissions Required for Tests

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "ses:SendEmail",
        "ses:SendRawEmail",
        "sqs:SendMessage",
        "sqs:ReceiveMessage",
        "sqs:DeleteMessage",
        "sqs:PurgeQueue",
        "sqs:GetQueueAttributes",
        "s3:GetObject",
        "s3:PutObject",
        "s3:DeleteObject",
        "s3:ListBucket",
        "logs:FilterLogEvents",
        "logs:GetLogEvents",
        "cloudwatch:GetMetricStatistics",
        "dynamodb:PutItem",
        "dynamodb:GetItem",
        "dynamodb:DeleteItem",
        "lambda:InvokeFunction",
        "lambda:GetFunction"
      ],
      "Resource": "*"
    }
  ]
}
```

---

## Appendix B: Test Execution Checklist

**Before running tests:**

- [ ] AWS credentials configured
- [ ] Environment variables set
- [ ] Lambda function deployed
- [ ] SQS queues exist
- [ ] S3 buckets exist
- [ ] Test email address verified in SES

**After test suite:**

- [ ] Review test results
- [ ] Check for flaky tests
- [ ] Verify all resources cleaned up
- [ ] Update test documentation if needed
- [ ] Report any bugs found

---

**Document Prepared By:** Test Strategy Analysis
**Status:** Ready for Implementation
**Next Action:** Begin Phase 1 - Setup e2e directory and utilities
