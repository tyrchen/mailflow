"""Message format validation utilities."""

from typing import Dict


def validate_inbound_message(message: Dict) -> bool:
    """Validate inbound message matches spec FR-1.20.

    Note: Rust serializes with snake_case, so fields are message_id, routing_key, etc.
    """
    # Version and source
    assert (
        message["version"] == "1.0"
    ), f"Expected version 1.0, got {message.get('version')}"
    assert (
        message["source"] == "mailflow"
    ), f"Expected source mailflow, got {message.get('source')}"

    # Message ID (snake_case from Rust serde)
    assert "message_id" in message, f"Missing message_id. Keys: {list(message.keys())}"
    assert message["message_id"].startswith(
        "mailflow-"
    ), f"Message ID should start with 'mailflow-', got {message['message_id']}"

    # Email structure
    email = message["email"]
    assert "from" in email, "Missing from field"
    assert "address" in email["from"], "Missing from.address"
    assert "to" in email and len(email["to"]) > 0, "Missing or empty to field"
    assert "subject" in email, "Missing subject"
    assert "body" in email, "Missing body"
    assert (
        "text" in email["body"] or "html" in email["body"]
    ), "Body must have text or html"

    # Metadata (snake_case)
    metadata = message["metadata"]
    assert (
        "routing_key" in metadata
    ), f"Missing routing_key. Keys: {list(metadata.keys())}"
    assert "domain" in metadata, "Missing domain"
    assert "spf_verified" in metadata, "Missing spf_verified"
    assert "dkim_verified" in metadata, "Missing dkim_verified"

    # Attachments (if present)
    # Note: Field names depend on Rust serde rename settings
    if email.get("attachments"):
        for idx, attachment in enumerate(email["attachments"]):
            # Print actual keys for debugging
            print(f"DEBUG: Attachment {idx} keys: {list(attachment.keys())}")

            assert (
                "filename" in attachment
            ), f"Attachment missing filename. Keys: {list(attachment.keys())}"
            assert "size" in attachment, "Attachment missing size"
            assert "status" in attachment, "Attachment missing status"
            assert attachment["status"] in [
                "available",
                "failed",
            ], f"Invalid status: {attachment['status']}"

            # Check for either camelCase or snake_case variants
            assert (
                "content_type" in attachment or "contentType" in attachment
            ), f"Attachment missing content_type. Keys: {list(attachment.keys())}"

            assert (
                "s3_bucket" in attachment or "s3Bucket" in attachment
            ), "Attachment missing s3_bucket/s3Bucket"

            assert (
                "s3_key" in attachment or "s3Key" in attachment
            ), "Attachment missing s3_key/s3Key"

            assert (
                "presigned_url" in attachment or "presignedUrl" in attachment
            ), "Attachment missing presigned_url"

            # Checksum is optional
            checksum = attachment.get("checksum_md5") or attachment.get("checksumMd5")
            if checksum:
                assert len(checksum) == 32, "Invalid MD5 checksum length"

    return True


def validate_outbound_message(message: Dict) -> bool:
    """Validate outbound message format."""
    assert message["version"] == "1.0", "Invalid version"
    assert "correlationId" in message, "Missing correlationId"
    assert "email" in message, "Missing email"

    email = message["email"]
    assert "from" in email and "address" in email["from"], "Invalid from"
    assert "to" in email and len(email["to"]) > 0, "Invalid to"
    assert "subject" in email, "Missing subject"
    assert "body" in email, "Missing body"

    return True
