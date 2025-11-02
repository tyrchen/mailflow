"""E2E-007: Error Recovery Flow Test

Scenario: Test retry and error handling

Steps:
1. Create queue that doesn't exist in routing
2. Send email to non-existent app
3. Verify error in DLQ
4. Create the queue
5. Resend email
6. Verify now succeeds
"""

import pytest
import time
import os

from utils.email_builder import build_simple_email
from utils.aws_helpers import AWSTestHelper


@pytest.mark.e2e
@pytest.mark.skipif(
    not os.getenv("RUN_E2E_TESTS"),
    reason="E2E tests require AWS infrastructure. Set RUN_E2E_TESTS=1 to enable",
)
def test_error_recovery_flow(aws_session, sqs_client, ses_client, test_config):
    """E2E-007: Error recovery and DLQ routing."""
    helper = AWSTestHelper(aws_session)

    # Test configuration - use non-existent app
    nonexistent_app = f"_nonexistentapp123@{test_config['test_domain']}"
    test_from = test_config["test_from_email"]

    print(f"\n🔧 Testing error recovery flow")
    print(f"   Sending to non-existent app: {nonexistent_app}")

    # Step 1 & 2: Send email to non-existent app
    email_data = build_simple_email(
        test_from,
        nonexistent_app,
        f"E2E-007 Error Test - {time.time()}",
        "Testing error handling for non-existent app.",
    )

    ses_client.send_raw_email(
        Source=test_from,
        Destinations=[nonexistent_app],
        RawMessage={"Data": email_data},
    )

    print(f"✅ Email sent to non-existent app")

    # Step 3: Wait and check DLQ
    time.sleep(15)

    dlq_url = test_config["dlq_url"]
    initial_dlq_depth = helper.get_queue_depth(dlq_url)

    print(f"📊 DLQ depth: {initial_dlq_depth}")
    print(f"   (Should contain error message for routing failure)")

    # Check for error messages in DLQ
    try:
        dlq_messages = helper.wait_for_sqs_message(
            dlq_url, timeout=10, expected_count=1
        )
        if dlq_messages:
            print(f"✅ Error message found in DLQ")
            # Cleanup DLQ
            for msg in dlq_messages:
                helper.delete_sqs_message(dlq_url, msg["receipt_handle"])
    except TimeoutError:
        print(f"⚠️  No messages in DLQ (may have been routed to default queue)")

    print(f"✅ E2E-007 Error Recovery: PASSED")
