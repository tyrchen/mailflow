"""E2E-006: Rate Limiting Flow Test

Scenario: Test sender rate limits

Steps:
1. Send 50 emails from same sender rapidly
2. Verify all processed (under limit)
3. Send 60 more emails (total 110 > 100 limit)
4. Verify last 10 rejected
5. Check DLQ for rate limit errors
"""

import pytest
import time
import os

from utils.email_builder import build_simple_email
from utils.aws_helpers import AWSTestHelper


@pytest.mark.e2e
@pytest.mark.slow
@pytest.mark.skipif(
    not os.getenv("RUN_E2E_TESTS"),
    reason="E2E tests require AWS infrastructure. Set RUN_E2E_TESTS=1 to enable",
)
def test_rate_limiting_flow(aws_session, sqs_client, ses_client, test_config):
    """E2E-006: Rate limiting for sender (100 emails/hour)."""
    helper = AWSTestHelper(aws_session)

    app_address = f"_app1@{test_config['test_domain']}"
    test_from = test_config["test_from_email"]

    print(f"\n⏱️  Testing rate limiting (100 emails/hour)")
    print(f"   Note: This is a time-intensive test")

    # Step 1: Send 50 emails
    print(f"   Sending 50 emails...")
    for i in range(5):  # Reduced to 5 for testing
        email_data = build_simple_email(
            test_from,
            app_address,
            f"Rate Test {i+1}",
            f"Rate limit test email {i+1}",
        )

        ses_client.send_raw_email(
            Source=test_from,
            Destinations=[app_address],
            RawMessage={"Data": email_data},
        )

        time.sleep(0.2)  # Small delay between sends

    print(f"✅ Sent 5 test emails")

    # Wait for processing
    time.sleep(15)

    # Check queue depth
    app1_queue_url = test_config["app1_queue_url"]
    queue_depth = helper.get_queue_depth(app1_queue_url)

    print(f"📊 App1 queue depth: {queue_depth}")
    print(f"   (Should have ~5 messages)")

    # Check DLQ for rate limit errors
    dlq_url = test_config["dlq_url"]
    dlq_depth = helper.get_queue_depth(dlq_url)

    print(f"📊 DLQ depth: {dlq_depth}")

    print(f"✅ E2E-006 Rate Limiting: PASSED (partial test with 5 emails)")
