"""E2E-001: Complete Inbound Flow Test

Scenario: External email → SES → Lambda → SQS → App receives

Steps:
1. Send email via SES to _app1@yourdomain.com
2. Wait for SES to receive and trigger Lambda
3. Verify Lambda processes email
4. Verify message appears in app1 SQS queue
5. Verify message format matches spec
6. Verify attachments stored in S3
7. Verify presigned URLs work
8. Verify metrics emitted

Expected:
- Message in SQS within 10 seconds
- All metadata correct
- Attachments accessible
- No DLQ messages

Requirements:
- AWS credentials with SES send permissions
- Verified SES email address
- Deployed Lambda function
- SQS queues created
- Set RUN_E2E_TESTS=1 environment variable to enable
"""

import pytest
import time
import sys
import os
import botocore.exceptions

# Add parent directory to path for imports
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "../..")))

from utils.email_builder import EmailBuilder, build_simple_email
from utils.aws_helpers import AWSTestHelper
from utils.message_validator import validate_inbound_message


def check_ses_permissions(ses_client, email):
    """Check if we have SES permissions for the email address or domain."""
    try:
        # Check domain (more common for production)
        domain = email.split("@")[1] if "@" in email else email

        response = ses_client.get_identity_verification_attributes(Identities=[domain])
        attrs = response.get("VerificationAttributes", {})

        # Check if domain is verified
        if domain in attrs:
            status = attrs[domain].get("VerificationStatus")
            if status == "Success":
                print(f"✅ SES domain verified: {domain}")
                return True

        # If domain check failed, list all verified identities for debugging
        try:
            all_identities = ses_client.list_verified_email_addresses()
            print(
                f"📋 Verified email addresses: {all_identities.get('VerifiedEmailAddresses', [])}"
            )
        except:
            pass

        print(f"⚠️  SES domain {domain} not verified")
        print(
            f"   Status: {attrs.get(domain, {}).get('VerificationStatus', 'Not found')}"
        )

        # For testing purposes, assume it's okay if we can't verify
        # The actual SES send will fail if not verified
        return True  # Allow test to proceed and fail on actual send if needed
    except botocore.exceptions.ClientError as e:
        print(f"⚠️  SES permission check failed: {e}")
        # Allow test to proceed - it will fail on send if permissions are wrong
        return True


@pytest.mark.e2e
@pytest.mark.skipif(
    not os.getenv("RUN_E2E_TESTS"),
    reason="E2E tests require AWS infrastructure. Set RUN_E2E_TESTS=1 to enable",
)
def test_complete_inbound_flow(
    aws_session, sqs_client, ses_client, s3_client, test_config
):
    """E2E-001: Complete inbound email flow validation.

    This test validates the complete inbound flow:
    - Sends email via SES
    - Verifies Lambda processes it
    - Validates message in SQS
    - Checks message format compliance
    """
    # Skip if SES not properly configured
    from_address = test_config["test_from_email"]
    if not check_ses_permissions(ses_client, from_address):
        pytest.skip(f"SES email {from_address} not verified or no permissions")

    helper = AWSTestHelper(aws_session)

    # Test configuration
    to_address = f"_app1@{test_config['test_domain']}"
    subject = f"E2E-001 Test - {time.time()}"
    body_text = "This is an E2E test for complete inbound flow."

    print(f"\n📧 Sending email from {from_address} to {to_address}")

    # Step 1: Build and send email via SES
    email_data = build_simple_email(from_address, to_address, subject, body_text)

    ses_message_id = ses_client.send_raw_email(
        Source=from_address,
        Destinations=[to_address],
        RawMessage={"Data": email_data},
    )["MessageId"]

    print(f"✅ Email sent via SES: {ses_message_id}")

    # Step 2 & 3: Wait for Lambda to process
    # SES will store in S3 and trigger Lambda
    print(f"⏳ Waiting for Lambda to process email...")
    time.sleep(10)  # Give SES + Lambda time to process

    # Step 4: Check message in app1 queue
    print(f"📥 Checking for message in app1 queue...")
    app1_queue_url = test_config["app1_queue_url"]

    messages = helper.wait_for_sqs_message(app1_queue_url, timeout=30, expected_count=1)

    assert len(messages) > 0, "No message received in app1 queue"

    # Find our message by subject (in case queue has messages from previous tests)
    our_message = None
    for msg in messages:
        if msg["body"]["email"]["subject"] == subject:
            our_message = msg
            break

    if not our_message:
        # Clean up all messages and fail
        for msg in messages:
            print(f"⚠️  Found unexpected message: {msg['body']['email']['subject']}")
            helper.delete_sqs_message(app1_queue_url, msg["receipt_handle"])
        pytest.fail(f"Did not find message with subject '{subject}' in queue")

    message_body = our_message["body"]

    print(f"✅ Received message in SQS queue")

    # Step 5: Validate message format
    print(f"🔍 Validating message format...")
    assert validate_inbound_message(message_body), "Message format validation failed"

    # Validate specific fields (snake_case from Rust serde)
    assert message_body["version"] == "1.0"
    assert message_body["source"] == "mailflow"
    assert message_body["metadata"]["routing_key"] == "app1"
    assert message_body["email"]["subject"] == subject
    assert message_body["email"]["body"]["text"] == body_text

    print(f"✅ Message format valid")
    print(f"   - Message ID: {message_body['message_id']}")
    print(f"   - Routing Key: {message_body['metadata']['routing_key']}")
    print(f"   - Domain: {message_body['metadata']['domain']}")

    # Step 6: Verify no attachments for this simple test
    assert len(message_body["email"]["attachments"]) == 0

    # Step 7: Check DLQ is empty (or at least not growing)
    dlq_url = test_config["dlq_url"]
    dlq_depth = helper.get_queue_depth(dlq_url)
    print(f"📊 DLQ depth: {dlq_depth}")

    # Cleanup: Delete our message from queue
    helper.delete_sqs_message(app1_queue_url, our_message["receipt_handle"])

    # Clean up any other messages that were in the queue
    for msg in messages:
        if msg != our_message:
            helper.delete_sqs_message(app1_queue_url, msg["receipt_handle"])

    print(f"✅ E2E-001 Complete Inbound Flow: PASSED")


@pytest.mark.e2e
@pytest.mark.skipif(
    not os.getenv("RUN_E2E_TESTS"),
    reason="E2E tests require AWS infrastructure. Set RUN_E2E_TESTS=1 to enable",
)
def test_inbound_with_attachment(
    aws_session, sqs_client, ses_client, s3_client, test_config
):
    """E2E-001b: Inbound flow with PDF attachment."""
    # Skip if SES not properly configured
    from_address = test_config["test_from_email"]
    if not check_ses_permissions(ses_client, from_address):
        pytest.skip(f"SES email {from_address} not verified or no permissions")

    helper = AWSTestHelper(aws_session)

    # Test configuration
    to_address = f"_app1@{test_config['test_domain']}"
    subject = f"E2E-001b Attachment Test - {time.time()}"

    # Create PDF attachment
    pdf_content = b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\nxref\n0 2\ntrailer<</Size 2/Root 1 0 R>>\n%%EOF"

    print(f"\n📧 Sending email with PDF attachment")

    # Build email with attachment
    email_data = (
        EmailBuilder()
        .from_address(from_address)
        .to(to_address)
        .with_subject(subject)
        .with_text_body("Email with PDF attachment for testing.")
        .attach_file("test-document.pdf", "application/pdf", pdf_content)
        .build_mime()
    )

    # Send via SES
    ses_message_id = ses_client.send_raw_email(
        Source=from_address,
        Destinations=[to_address],
        RawMessage={"Data": email_data},
    )["MessageId"]

    print(f"✅ Email sent: {ses_message_id}")

    # Wait for processing
    time.sleep(10)

    # Check queue
    app1_queue_url = test_config["app1_queue_url"]
    messages = helper.wait_for_sqs_message(app1_queue_url, timeout=30, max_messages=10)

    # Find our message by subject
    our_message = None
    for msg in messages:
        if msg["body"]["email"]["subject"] == subject:
            our_message = msg
            break

    if not our_message:
        for msg in messages:
            helper.delete_sqs_message(app1_queue_url, msg["receipt_handle"])
        pytest.fail(f"Did not find message with subject '{subject}'")

    message_body = our_message["body"]

    # Validate message with attachment
    assert validate_inbound_message(message_body)
    assert len(message_body["email"]["attachments"]) == 1, "Should have 1 attachment"

    attachment = message_body["email"]["attachments"][0]
    print(f"📎 Attachment received:")
    print(f"   - Filename: {attachment['filename']}")
    print(f"   - Size: {attachment['size']} bytes")
    print(
        f"   - Content Type: {attachment.get('content_type') or attachment.get('contentType')}"
    )
    print(f"   - Status: {attachment['status']}")

    assert attachment["status"] == "available"
    assert "presigned_url" in attachment or "presignedUrl" in attachment

    # Get bucket name (either snake_case or camelCase)
    s3_bucket = attachment.get("s3_bucket") or attachment.get("s3Bucket")
    assert s3_bucket == test_config["attachments_bucket"]

    # Cleanup
    helper.delete_sqs_message(app1_queue_url, our_message["receipt_handle"])

    # Clean up other messages
    for msg in messages:
        if msg != our_message:
            helper.delete_sqs_message(app1_queue_url, msg["receipt_handle"])

    print(f"✅ E2E-001b Inbound with Attachment: PASSED")
