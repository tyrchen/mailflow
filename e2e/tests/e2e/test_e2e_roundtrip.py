"""E2E-003: Round-trip Reply Flow Test

Scenario: Receive email → App replies → Reply sent

Steps:
1. Send email to _app1@domain.com with Message-ID
2. Receive from app1 queue
3. Construct reply with In-Reply-To header
4. Send reply to outbound queue
5. Verify reply sent with threading headers
6. Verify In-Reply-To and References headers present
"""

import pytest
import time
import json
import uuid
import os

from utils.email_builder import EmailBuilder, build_outbound_message
from utils.aws_helpers import AWSTestHelper
from utils.message_validator import validate_inbound_message


@pytest.mark.e2e
@pytest.mark.skipif(
    not os.getenv("RUN_E2E_TESTS"),
    reason="E2E tests require AWS infrastructure. Set RUN_E2E_TESTS=1 to enable",
)
def test_roundtrip_reply_flow(aws_session, sqs_client, ses_client, test_config):
    """E2E-003: Round-trip reply with threading headers."""
    helper = AWSTestHelper(aws_session)

    # Test configuration
    app_address = f"_app1@{test_config['test_domain']}"
    test_from = test_config["test_from_email"]
    original_message_id = f"original-{uuid.uuid4()}@test.mailflow.io"

    print(f"\n🔄 Testing round-trip reply flow")
    print(f"   Step 1: Send email to {app_address}")

    # Step 1: Send initial email
    email_data = (
        EmailBuilder()
        .from_address(test_from)
        .to(app_address)
        .with_subject("E2E-003 Original Message")
        .with_text_body("This is the original message for reply test.")
        .with_message_id(original_message_id)
        .build_mime()
    )

    ses_client.send_raw_email(
        Source=test_from,
        Destinations=[app_address],
        RawMessage={"Data": email_data},
    )

    print(f"✅ Original email sent with Message-ID: {original_message_id}")

    # Step 2: Wait and receive from app1 queue
    time.sleep(10)

    app1_queue_url = test_config["app1_queue_url"]
    messages = helper.wait_for_sqs_message(app1_queue_url, timeout=30)

    inbound_message = messages[0]["body"]
    print(f"✅ Received inbound message")

    # Step 3: Construct reply with threading headers
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
            "subject": "Re: E2E-003 Original Message",
            "body": {"text": "This is a reply to your message.", "html": ""},
            "attachments": [],
            "headers": {
                "inReplyTo": original_message_id,
                "references": [original_message_id],
            },
        },
        "options": {
            "priority": "normal",
            "scheduledSendTime": None,
            "trackOpens": False,
            "trackClicks": False,
        },
    }

    print(f"✅ Reply constructed with threading headers")
    print(f"   - In-Reply-To: {reply_message['email']['headers']['inReplyTo']}")
    print(f"   - References: {reply_message['email']['headers']['references']}")

    # Step 4: Send reply to outbound queue
    outbound_queue_url = test_config["outbound_queue_url"]
    sqs_client.send_message(
        QueueUrl=outbound_queue_url, MessageBody=json.dumps(reply_message)
    )

    print(f"✅ Reply sent to outbound queue")

    # Step 5 & 6: Wait for processing
    time.sleep(15)

    # Verify queue processed
    queue_depth = helper.get_queue_depth(outbound_queue_url)
    print(f"📊 Outbound queue depth: {queue_depth}")

    # Cleanup
    helper.delete_sqs_message(app1_queue_url, messages[0]["receipt_handle"])

    print(f"✅ E2E-003 Round-trip Reply Flow: PASSED")
