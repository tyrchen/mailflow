"""E2E-008: Multi-Recipient Routing Test

Scenario: Email to multiple apps

Steps:
1. Send email to _app1@domain.com, _app2@domain.com
2. Verify message in app1 queue
3. Verify message in app2 queue
4. Verify both have same email content
5. Verify separate routing decisions logged
"""

import pytest
import time
import os

from utils.email_builder import EmailBuilder
from utils.aws_helpers import AWSTestHelper
from utils.message_validator import validate_inbound_message


@pytest.mark.e2e
@pytest.mark.skipif(
    not os.getenv("RUN_E2E_TESTS"),
    reason="E2E tests require AWS infrastructure. Set RUN_E2E_TESTS=1 to enable",
)
def test_multi_recipient_routing(aws_session, sqs_client, ses_client, test_config):
    """E2E-008: Multi-app routing validation."""
    helper = AWSTestHelper(aws_session)

    # Test configuration
    app1_address = f"_app1@{test_config['test_domain']}"
    app2_address = f"_app2@{test_config['test_domain']}"
    test_from = test_config["test_from_email"]
    subject = f"E2E-008 Multi-Routing - {time.time()}"

    print(f"\n📨 Testing multi-recipient routing")
    print(f"   To: {app1_address}, {app2_address}")

    # Step 1: Send email to both apps
    email_data = (
        EmailBuilder()
        .from_address(test_from)
        .to(app1_address)
        .to(app2_address)
        .with_subject(subject)
        .with_text_body("Email to multiple apps for routing test.")
        .build_mime()
    )

    ses_client.send_raw_email(
        Source=test_from,
        Destinations=[app1_address, app2_address],
        RawMessage={"Data": email_data},
    )

    print(f"✅ Email sent to both apps")

    # Step 2 & 3: Wait and check both queues
    time.sleep(10)

    app1_queue_url = test_config["app1_queue_url"]
    app2_queue_url = test_config["app2_queue_url"]

    print(f"📥 Checking app1 queue...")
    app1_messages = helper.wait_for_sqs_message(
        app1_queue_url, timeout=30, expected_count=1, max_messages=10
    )

    # Find our message by subject
    app1_message = None
    for msg in app1_messages:
        if msg["body"]["email"]["subject"] == subject:
            app1_message = msg["body"]
            app1_receipt = msg["receipt_handle"]
            break

    if not app1_message:
        pytest.fail(f"Did not find message with subject '{subject}' in app1 queue")

    print(f"📥 Checking app2 queue...")
    app2_messages = helper.wait_for_sqs_message(
        app2_queue_url, timeout=30, expected_count=1, max_messages=10
    )

    # Find our message by subject
    app2_message = None
    for msg in app2_messages:
        if msg["body"]["email"]["subject"] == subject:
            app2_message = msg["body"]
            app2_receipt = msg["receipt_handle"]
            break

    if not app2_message:
        pytest.fail(f"Did not find message with subject '{subject}' in app2 queue")

    print(f"✅ Messages received in both queues")

    # Step 4: Verify same content
    assert app1_message["email"]["subject"] == app2_message["email"]["subject"]
    assert (
        app1_message["email"]["body"]["text"] == app2_message["email"]["body"]["text"]
    )

    # Step 5: Verify different routing keys (snake_case)
    assert app1_message["metadata"]["routing_key"] == "app1"
    assert app2_message["metadata"]["routing_key"] == "app2"

    print(f"✅ Routing validation:")
    print(f"   - App1 routing key: {app1_message['metadata']['routing_key']}")
    print(f"   - App2 routing key: {app2_message['metadata']['routing_key']}")
    print(
        f"   - Same subject: {app1_message['email']['subject'] == app2_message['email']['subject']}"
    )

    # Cleanup
    helper.delete_sqs_message(app1_queue_url, app1_receipt)
    helper.delete_sqs_message(app2_queue_url, app2_receipt)

    # Clean up any other messages
    for msg in app1_messages:
        if msg["body"] != app1_message:
            helper.delete_sqs_message(app1_queue_url, msg["receipt_handle"])
    for msg in app2_messages:
        if msg["body"] != app2_message:
            helper.delete_sqs_message(app2_queue_url, msg["receipt_handle"])

    print(f"✅ E2E-008 Multi-Recipient Routing: PASSED")
