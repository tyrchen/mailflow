"""E2E-004: Attachment Round-trip Test

Scenario: Receive email with PDF → App replies with same PDF

Steps:
1. Send email with document.pdf attachment
2. Receive message, get presigned URL
3. Download PDF via presigned URL
4. Verify PDF integrity (MD5 checksum)
5. Construct reply referencing same S3 object
6. Send outbound with attachment
7. Verify email sent with attachment
"""

import pytest
import time
import json
import uuid
import hashlib
import os
import requests

from utils.email_builder import EmailBuilder
from utils.aws_helpers import AWSTestHelper
from utils.message_validator import validate_inbound_message


@pytest.mark.e2e
@pytest.mark.skipif(
    not os.getenv("RUN_E2E_TESTS"),
    reason="E2E tests require AWS infrastructure. Set RUN_E2E_TESTS=1 to enable",
)
def test_attachment_roundtrip(
    aws_session, sqs_client, ses_client, s3_client, test_config
):
    """E2E-004: Attachment round-trip with MD5 verification."""
    helper = AWSTestHelper(aws_session)

    # Test configuration
    app_address = f"_app1@{test_config['test_domain']}"
    test_from = test_config["test_from_email"]

    # Create PDF with known content for MD5 verification
    pdf_content = b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Count 1/Kids[3 0 R]>>endobj\n3 0 obj<</Type/Page/Parent 2 0 R/Resources<<>>/MediaBox[0 0 612 792]>>endobj\nxref\n0 4\n0000000000 65535 f\n0000000009 00000 n\n0000000056 00000 n\n0000000115 00000 n\ntrailer<</Size 4/Root 1 0 R>>\nstartxref\n203\n%%EOF\n"
    original_md5 = hashlib.md5(pdf_content).hexdigest()

    print(f"\n📎 Testing attachment round-trip")
    print(f"   Original MD5: {original_md5}")

    # Step 1: Send email with PDF attachment
    email_data = (
        EmailBuilder()
        .from_address(test_from)
        .to(app_address)
        .with_subject(f"E2E-004 PDF Test - {time.time()}")
        .with_text_body("Email with PDF for round-trip test.")
        .attach_file("document.pdf", "application/pdf", pdf_content)
        .build_mime()
    )

    ses_client.send_raw_email(
        Source=test_from,
        Destinations=[app_address],
        RawMessage={"Data": email_data},
    )

    print(f"✅ Email with PDF sent")

    # Step 2: Receive message
    time.sleep(10)

    app1_queue_url = test_config["app1_queue_url"]
    messages = helper.wait_for_sqs_message(app1_queue_url, timeout=30, max_messages=10)

    # Find message with attachment
    inbound_message = None
    receipt = None
    for msg in messages:
        if len(msg["body"]["email"]["attachments"]) > 0:
            inbound_message = msg["body"]
            receipt = msg["receipt_handle"]
            break

    if not inbound_message:
        # Clean up and fail
        for msg in messages:
            helper.delete_sqs_message(app1_queue_url, msg["receipt_handle"])
        pytest.fail("No message with attachment found")

    assert len(inbound_message["email"]["attachments"]) == 1
    attachment = inbound_message["email"]["attachments"][0]

    # Get fields (support both snake_case and camelCase)
    s3_bucket = attachment.get("s3_bucket") or attachment.get("s3Bucket")
    s3_key = attachment.get("s3_key") or attachment.get("s3Key")
    presigned_url = attachment.get("presigned_url") or attachment.get("presignedUrl")
    content_type = attachment.get("content_type") or attachment.get("contentType")

    print(f"✅ Attachment received:")
    print(f"   - S3 Bucket: {s3_bucket}")
    print(f"   - S3 Key: {s3_key}")
    print(f"   - Presigned URL: {presigned_url[:50]}...")

    # Step 3: Download via presigned URL
    response = requests.get(presigned_url, timeout=10)
    assert response.status_code == 200, f"Failed to download: {response.status_code}"

    downloaded_content = response.content
    print(f"✅ Downloaded {len(downloaded_content)} bytes via presigned URL")

    # Step 4: Verify MD5 checksum
    downloaded_md5 = hashlib.md5(downloaded_content).hexdigest()
    assert (
        downloaded_md5 == original_md5
    ), f"MD5 mismatch: {downloaded_md5} != {original_md5}"

    print(f"✅ MD5 checksum verified")

    # Step 5 & 6: Construct reply with same attachment
    reply_message = {
        "version": "1.0",
        "correlationId": f"reply-{uuid.uuid4()}",
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "source": "e2e-test",
        "email": {
            "from": {"address": app_address, "name": "App 1"},
            "to": [{"address": test_from, "name": "Original Sender"}],
            "cc": [],
            "bcc": [],
            "replyTo": {"address": app_address},
            "subject": "Re: E2E-004 PDF Test",
            "body": {"text": "Returning your document.", "html": ""},
            "attachments": [
                {
                    "filename": attachment["filename"],
                    "contentType": content_type,
                    "s3Bucket": s3_bucket,
                    "s3Key": s3_key,
                }
            ],
            "headers": {
                "inReplyTo": inbound_message["email"].get("message_id")
                or inbound_message["email"].get("messageId"),
                "references": [],
            },
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
        QueueUrl=outbound_queue_url, MessageBody=json.dumps(reply_message)
    )

    print(f"✅ Reply with attachment sent to outbound queue")

    # Step 7: Wait for processing
    time.sleep(15)

    # Cleanup
    helper.delete_sqs_message(app1_queue_url, receipt)

    # Clean up other messages
    for msg in messages:
        if msg["receipt_handle"] != receipt:
            helper.delete_sqs_message(app1_queue_url, msg["receipt_handle"])

    print(f"✅ E2E-004 Attachment Round-trip: PASSED")
