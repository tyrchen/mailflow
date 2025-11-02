"""Pytest configuration and shared fixtures for E2E tests."""

import os
import pytest
import boto3
from dotenv import load_dotenv

# Load environment variables from .env file
# override=True ensures .env values take precedence over existing env vars
load_dotenv(override=True)


@pytest.fixture(scope="session")
def aws_region():
    """AWS region for tests."""
    return os.getenv("AWS_REGION", "us-east-1")


@pytest.fixture(scope="session")
def aws_session(aws_region):
    """Create AWS session with configured profile."""
    profile = os.getenv("AWS_PROFILE")
    print(f"🔧 Creating AWS session with profile='{profile}', region='{aws_region}'")

    if profile:
        session = boto3.Session(profile_name=profile, region_name=aws_region)
    else:
        print("⚠️  WARNING: No AWS_PROFILE set, using default credentials")
        session = boto3.Session(region_name=aws_region)

    # Verify the session is using correct account
    sts = session.client("sts")
    identity = sts.get_caller_identity()
    print(f"✅ AWS Identity: Account={identity['Account']}, ARN={identity['Arn']}")

    return session


@pytest.fixture
def s3_client(aws_session):
    """S3 client for tests."""
    return aws_session.client("s3")


@pytest.fixture
def sqs_client(aws_session):
    """SQS client for tests."""
    return aws_session.client("sqs")


@pytest.fixture
def ses_client(aws_session):
    """SES client for tests."""
    return aws_session.client("ses")


@pytest.fixture
def lambda_client(aws_session):
    """Lambda client for tests."""
    return aws_session.client("lambda")


@pytest.fixture
def cloudwatch_client(aws_session):
    """CloudWatch client for tests."""
    return aws_session.client("cloudwatch")


@pytest.fixture
def logs_client(aws_session):
    """CloudWatch Logs client for tests."""
    return aws_session.client("logs")


@pytest.fixture
def dynamodb_client(aws_session):
    """DynamoDB client for tests."""
    return aws_session.client("dynamodb")


@pytest.fixture
def test_config():
    """Test configuration from environment."""
    return {
        "lambda_function": os.getenv("LAMBDA_FUNCTION", "mailflow-dev"),
        "app1_queue_url": os.getenv("APP1_QUEUE_URL"),
        "app2_queue_url": os.getenv("APP2_QUEUE_URL"),
        "outbound_queue_url": os.getenv("OUTBOUND_QUEUE_URL"),
        "dlq_url": os.getenv("DLQ_URL"),
        "raw_emails_bucket": os.getenv("RAW_EMAILS_BUCKET"),
        "attachments_bucket": os.getenv("ATTACHMENTS_BUCKET"),
        "test_domain": os.getenv("TEST_DOMAIN", "yourdomain.com"),
        "test_from_email": os.getenv("TEST_FROM_EMAIL", "test@yourdomain.com"),
        "default_timeout": int(os.getenv("DEFAULT_TIMEOUT", "30")),
    }


@pytest.fixture(autouse=True, scope="function")
def cleanup_test_queues(sqs_client, test_config):
    """Purge test queues before each test to ensure clean state."""
    # Only purge if E2E tests are enabled
    if not os.getenv("RUN_E2E_TESTS"):
        yield
        return

    print(f"\n🧹 Purging test queues before test...")

    # Purge queues BEFORE test to ensure clean state
    for queue_key in ["app1_queue_url", "app2_queue_url"]:
        queue_url = test_config.get(queue_key)
        if queue_url:
            try:
                # Try to purge - may fail if purged < 60s ago
                sqs_client.purge_queue(QueueUrl=queue_url)
                print(f"✅ Purged {queue_url.split('/')[-1]}")
            except sqs_client.exceptions.PurgeQueueInProgress:
                # Queue was purged recently, manually drain instead
                print(
                    f"⏳ Queue {queue_url.split('/')[-1]} purged recently, draining manually..."
                )
                drained = 0
                while True:
                    response = sqs_client.receive_message(
                        QueueUrl=queue_url,
                        MaxNumberOfMessages=10,
                        WaitTimeSeconds=1,
                    )
                    if "Messages" not in response:
                        break
                    for msg in response["Messages"]:
                        sqs_client.delete_message(
                            QueueUrl=queue_url, ReceiptHandle=msg["ReceiptHandle"]
                        )
                        drained += 1
                    if len(response["Messages"]) < 10:
                        break
                print(f"✅ Drained {drained} messages from {queue_url.split('/')[-1]}")
            except Exception as e:
                print(f"⚠️  Failed to purge {queue_key}: {e}")

    # Small delay after purge
    import time

    time.sleep(2)

    yield  # Run the test

    # Note: Could add post-test cleanup here if needed
