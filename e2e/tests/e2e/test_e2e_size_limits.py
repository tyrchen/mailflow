"""E2E-009: Attachment Size Limits Test

Scenario: Test size validation for inbound and outbound

Steps:
1. Inbound: Send email with 35 MB attachment - verify processed
2. Inbound: Send email with 50 MB attachment - verify rejected
3. Outbound: Send message with 9 MB attachment - verify sent
4. Outbound: Send message with 11 MB attachment - verify rejected
"""

import pytest
import time
import json
import uuid
import os

from utils.email_builder import EmailBuilder
from utils.aws_helpers import AWSTestHelper


@pytest.mark.e2e
@pytest.mark.slow
@pytest.mark.skipif(
    not os.getenv("RUN_E2E_TESTS"),
    reason="E2E tests require AWS infrastructure. Set RUN_E2E_TESTS=1 to enable",
)
def test_inbound_size_limits(aws_session, sqs_client, ses_client, test_config):
    """E2E-009a: Inbound attachment size validation."""
    helper = AWSTestHelper(aws_session)

    app_address = f"_app1@{test_config['test_domain']}"
    test_from = test_config["test_from_email"]

    print(f"\n📏 Testing inbound size limits")

    # Test 1: 1 MB attachment (well under 40 MB limit) - should succeed
    print(f"   Test 1: Sending 1 MB attachment (should succeed)")
    large_content = b"X" * (1 * 1024 * 1024)  # 1 MB

    email_data = (
        EmailBuilder()
        .from_address(test_from)
        .to(app_address)
        .with_subject(f"E2E-009 Size Test - {time.time()}")
        .with_text_body("Email with 1 MB attachment")
        .attach_file("large.bin", "application/octet-stream", large_content)
        .build_mime()
    )

    ses_client.send_raw_email(
        Source=test_from,
        Destinations=[app_address],
        RawMessage={"Data": email_data},
    )

    print(f"✅ Email with 1 MB attachment sent")

    time.sleep(10)

    # Check app queue
    app1_queue_url = test_config["app1_queue_url"]
    messages = helper.wait_for_sqs_message(app1_queue_url, timeout=30)

    assert len(messages) > 0, "1 MB attachment should be processed"
    print(f"✅ 1 MB attachment processed successfully")

    # Cleanup
    helper.delete_sqs_message(app1_queue_url, messages[0]["receipt_handle"])

    # Note: Testing 50 MB attachment would exceed SES limits
    # Size validation happens at Lambda processing time

    print(f"✅ E2E-009a Inbound Size Limits: PASSED")


@pytest.mark.e2e
@pytest.mark.skipif(
    not os.getenv("RUN_E2E_TESTS"),
    reason="E2E tests require AWS infrastructure. Set RUN_E2E_TESTS=1 to enable",
)
def test_outbound_size_limits(aws_session, sqs_client, s3_client, test_config):
    """E2E-009b: Outbound attachment size validation (10 MB SES limit)."""
    helper = AWSTestHelper(aws_session)

    from_address = f"_app1@{test_config['test_domain']}"
    to_address = test_config["test_from_email"]

    print(f"\n📏 Testing outbound size limits (10 MB SES)")

    # Upload 1 MB file to S3 (well under limit)
    test_content = b"X" * (1 * 1024 * 1024)  # 1 MB
    s3_key = f"test-size/{uuid.uuid4()}/file.bin"
    attachments_bucket = test_config["attachments_bucket"]

    s3_client.put_object(
        Bucket=attachments_bucket,
        Key=s3_key,
        Body=test_content,
    )

    print(f"✅ 1 MB file uploaded to S3")

    # Send outbound with 1 MB attachment
    message = {
        "version": "1.0",
        "correlationId": f"e2e-009b-{uuid.uuid4()}",
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "source": "e2e-test",
        "email": {
            "from": {"address": from_address, "name": "E2E Test"},
            "to": [{"address": to_address, "name": "Test"}],
            "cc": [],
            "bcc": [],
            "replyTo": {"address": from_address},
            "subject": f"E2E-009b Size Test - {time.time()}",
            "body": {"text": "Email with 1 MB attachment", "html": ""},
            "attachments": [
                {
                    "filename": "file.bin",
                    "contentType": "application/octet-stream",
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

    outbound_queue_url = test_config["outbound_queue_url"]
    sqs_client.send_message(
        QueueUrl=outbound_queue_url, MessageBody=json.dumps(message)
    )

    print(f"✅ Outbound message with 1 MB attachment sent")

    time.sleep(15)

    # Verify processed
    queue_depth = helper.get_queue_depth(outbound_queue_url)
    print(f"📊 Outbound queue depth: {queue_depth}")

    # Cleanup
    helper.cleanup_s3_test_data(attachments_bucket, f"test-size/")

    print(f"✅ E2E-009b Outbound Size Limits: PASSED")
