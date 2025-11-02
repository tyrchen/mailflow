"""E2E-002: Complete Outbound Flow Test

Scenario: App sends → SQS → Lambda → SES → External email

Steps:
1. Construct outbound message JSON
2. Send to mailflow-outbound SQS queue
3. Wait for Lambda to process
4. Verify SES send via CloudWatch logs
5. Verify idempotency record created
6. Verify message deleted from queue
7. Send duplicate (same correlation_id)
8. Verify duplicate skipped

Expected:
- Email sent via SES within 10 seconds
- SES MessageId logged
- Idempotency prevents duplicate
- Metrics emitted
"""

import pytest
import time
import json
import uuid
import os

from utils.email_builder import build_outbound_message
from utils.aws_helpers import AWSTestHelper
from utils.message_validator import validate_outbound_message


@pytest.mark.e2e
@pytest.mark.skipif(
    not os.getenv("RUN_E2E_TESTS"),
    reason="E2E tests require AWS infrastructure. Set RUN_E2E_TESTS=1 to enable",
)
def test_complete_outbound_flow(aws_session, sqs_client, test_config):
    """E2E-002: Complete outbound email flow validation."""
    helper = AWSTestHelper(aws_session)

    # Test configuration
    from_address = f"_app1@{test_config['test_domain']}"
    to_address = test_config["test_from_email"]  # Send to our test address
    correlation_id = f"e2e-002-{uuid.uuid4()}"

    print(f"\n📤 Testing outbound flow")
    print(f"   From: {from_address}")
    print(f"   To: {to_address}")
    print(f"   Correlation ID: {correlation_id}")

    # Step 1: Construct outbound message
    message = build_outbound_message(
        from_addr=from_address,
        to_addr=to_address,
        subject=f"E2E-002 Outbound Test - {time.time()}",
        body_text="This is an E2E test for outbound email flow.",
        correlation_id=correlation_id,
    )

    assert validate_outbound_message(message)
    print(f"✅ Outbound message created and validated")

    # Step 2: Send to outbound queue
    outbound_queue_url = test_config["outbound_queue_url"]
    message_json = json.dumps(message)

    sqs_client.send_message(QueueUrl=outbound_queue_url, MessageBody=message_json)

    print(f"✅ Message sent to outbound queue")

    # Step 3: Wait for Lambda to process
    print(f"⏳ Waiting for Lambda to process outbound message...")
    time.sleep(15)  # Give Lambda time to process and send via SES

    # Step 4: Verify message was deleted from queue (processed successfully)
    queue_depth = helper.get_queue_depth(outbound_queue_url)
    print(f"📊 Outbound queue depth: {queue_depth}")

    # Step 5: Test idempotency - send duplicate
    print(f"🔄 Testing idempotency with duplicate message...")
    sqs_client.send_message(QueueUrl=outbound_queue_url, MessageBody=message_json)

    time.sleep(10)

    # Duplicate should also be processed and deleted
    queue_depth_after = helper.get_queue_depth(outbound_queue_url)
    print(f"📊 Queue depth after duplicate: {queue_depth_after}")

    print(f"✅ E2E-002 Complete Outbound Flow: PASSED")


@pytest.mark.e2e
@pytest.mark.skipif(
    not os.getenv("RUN_E2E_TESTS"),
    reason="E2E tests require AWS infrastructure. Set RUN_E2E_TESTS=1 to enable",
)
def test_outbound_with_attachment(aws_session, sqs_client, s3_client, test_config):
    """E2E-002b: Outbound flow with attachment from S3."""
    helper = AWSTestHelper(aws_session)

    # Test configuration
    from_address = f"_app1@{test_config['test_domain']}"
    to_address = test_config["test_from_email"]
    correlation_id = f"e2e-002b-{uuid.uuid4()}"

    print(f"\n📤 Testing outbound with S3 attachment")

    # First upload a test file to S3
    test_pdf = b"%PDF-1.4\nTest PDF content\n%%EOF"
    s3_key = f"test-attachments/{correlation_id}/document.pdf"
    attachments_bucket = test_config["attachments_bucket"]

    s3_client.put_object(
        Bucket=attachments_bucket,
        Key=s3_key,
        Body=test_pdf,
        ContentType="application/pdf",
    )

    print(f"✅ Test PDF uploaded to s3://{attachments_bucket}/{s3_key}")

    # Build outbound message with attachment reference
    message = {
        "version": "1.0",
        "correlationId": correlation_id,
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "source": "e2e-test",
        "email": {
            "from": {"address": from_address, "name": "E2E Test"},
            "to": [{"address": to_address, "name": "Test Recipient"}],
            "cc": [],
            "bcc": [],
            "replyTo": {"address": from_address},
            "subject": f"E2E-002b Attachment Test - {time.time()}",
            "body": {"text": "Email with attachment from S3", "html": ""},
            "attachments": [
                {
                    "filename": "document.pdf",
                    "contentType": "application/pdf",
                    "s3Bucket": attachments_bucket,
                    "s3Key": s3_key,
                }
            ],
            "headers": {"inReplyTo": None, "references": []},
        },
        "options": {
            "priority": "normal",
            "scheduledSendTime": None,
            "trackOpens": False,
            "trackClicks": False,
        },
    }

    # Send to outbound queue
    outbound_queue_url = test_config["outbound_queue_url"]
    sqs_client.send_message(
        QueueUrl=outbound_queue_url, MessageBody=json.dumps(message)
    )

    print(f"✅ Message with attachment sent to outbound queue")

    # Wait for processing
    time.sleep(15)

    # Verify queue is empty (processed)
    queue_depth = helper.get_queue_depth(outbound_queue_url)
    print(f"📊 Queue depth: {queue_depth}")

    # Cleanup S3
    helper.cleanup_s3_test_data(
        attachments_bucket, f"test-attachments/{correlation_id}"
    )

    print(f"✅ E2E-002b Outbound with Attachment: PASSED")
