"""AWS helper utilities for E2E tests."""

import time
import json
from typing import Dict, List, Optional
from datetime import datetime, timedelta
import boto3


class AWSTestHelper:
    """Helper class for AWS operations in tests."""

    def __init__(self, session: boto3.Session):
        self.ses = session.client("ses")
        self.sqs = session.client("sqs")
        self.s3 = session.client("s3")
        self.logs = session.client("logs")
        self.cloudwatch = session.client("cloudwatch")

    async def send_raw_email(
        self, email_data: bytes, source: str, destinations: List[str]
    ) -> str:
        """Send raw email via SES."""
        response = self.ses.send_raw_email(
            Source=source,
            Destinations=destinations,
            RawMessage={"Data": email_data},
        )
        return response["MessageId"]

    def wait_for_sqs_message(
        self,
        queue_url: str,
        timeout: int = 30,
        expected_count: int = 1,
        max_messages: int = 10,
    ) -> List[Dict]:
        """Wait for messages to appear in SQS queue.

        Returns up to max_messages, useful when queue might have old messages.
        """
        messages = []
        start_time = time.time()
        seen_message_ids = set()

        while time.time() - start_time < timeout and len(messages) < max_messages:
            response = self.sqs.receive_message(
                QueueUrl=queue_url,
                MaxNumberOfMessages=min(10, max_messages),
                WaitTimeSeconds=5,
                AttributeNames=["All"],
            )

            if "Messages" in response:
                for msg in response["Messages"]:
                    # Avoid duplicates
                    if msg["MessageId"] in seen_message_ids:
                        continue

                    seen_message_ids.add(msg["MessageId"])

                    # Parse message body
                    body = json.loads(msg["Body"])
                    messages.append(
                        {
                            "body": body,
                            "receipt_handle": msg["ReceiptHandle"],
                            "message_id": msg["MessageId"],
                            "attributes": msg.get("Attributes", {}),
                        }
                    )

            if len(messages) >= expected_count:
                break

            time.sleep(1)

        if len(messages) < expected_count:
            raise TimeoutError(
                f"Expected at least {expected_count} messages but got {len(messages)} within {timeout}s"
            )

        return messages

    def get_lambda_logs(
        self, log_group: str, filter_pattern: str, minutes: int = 5
    ) -> List[str]:
        """Get recent Lambda logs matching pattern."""
        end_time = int(datetime.utcnow().timestamp() * 1000)
        start_time = int(
            (datetime.utcnow() - timedelta(minutes=minutes)).timestamp() * 1000
        )

        try:
            response = self.logs.filter_log_events(
                logGroupName=log_group,
                startTime=start_time,
                endTime=end_time,
                filterPattern=filter_pattern,
            )

            return [event["message"] for event in response.get("events", [])]
        except self.logs.exceptions.ResourceNotFoundException:
            return []

    def get_cloudwatch_metric(
        self, metric_name: str, namespace: str = "Mailflow", minutes: int = 5
    ) -> float:
        """Get metric value from CloudWatch."""
        end_time = datetime.utcnow()
        start_time = end_time - timedelta(minutes=minutes)

        response = self.cloudwatch.get_metric_statistics(
            Namespace=namespace,
            MetricName=metric_name,
            StartTime=start_time,
            EndTime=end_time,
            Period=300,
            Statistics=["Sum"],
        )

        datapoints = response.get("Datapoints", [])
        if not datapoints:
            return 0.0

        return sum(dp["Sum"] for dp in datapoints)

    def purge_queue(self, queue_url: str):
        """Purge all messages from SQS queue."""
        try:
            self.sqs.purge_queue(QueueUrl=queue_url)
        except Exception as e:
            print(f"Warning: Failed to purge queue {queue_url}: {e}")

    def cleanup_s3_test_data(self, bucket: str, prefix: str):
        """Clean up S3 test objects."""
        try:
            response = self.s3.list_objects_v2(Bucket=bucket, Prefix=prefix)

            if "Contents" in response:
                objects = [{"Key": obj["Key"]} for obj in response["Contents"]]
                if objects:
                    self.s3.delete_objects(Bucket=bucket, Delete={"Objects": objects})
        except Exception as e:
            print(f"Warning: Failed to cleanup S3 {bucket}/{prefix}: {e}")

    def delete_sqs_message(self, queue_url: str, receipt_handle: str):
        """Delete a message from SQS queue."""
        self.sqs.delete_message(QueueUrl=queue_url, ReceiptHandle=receipt_handle)

    def get_queue_depth(self, queue_url: str) -> int:
        """Get approximate number of messages in queue."""
        response = self.sqs.get_queue_attributes(
            QueueUrl=queue_url, AttributeNames=["ApproximateNumberOfMessages"]
        )
        return int(response["Attributes"]["ApproximateNumberOfMessages"])
