"""Email construction utilities for E2E tests."""

from email.mime.multipart import MIMEMultipart
from email.mime.text import MIMEText
from email.mime.base import MIMEBase
from email import encoders
from typing import Optional, List, Dict
from datetime import datetime
import uuid


class EmailBuilder:
    """Builder for constructing test emails."""

    def __init__(self):
        self.from_addr = None
        self.from_name = None
        self.to_addrs = []
        self.cc_addrs = []
        self.bcc_addrs = []
        self.subject = None
        self.body_text = None
        self.body_html = None
        self.attachments = []
        self.headers = {}
        self._custom_message_id = None

    def from_address(self, email: str, name: Optional[str] = None):
        """Set from address."""
        self.from_addr = email
        self.from_name = name
        return self

    def to(self, email: str, name: Optional[str] = None):
        """Add to address."""
        self.to_addrs.append((email, name))
        return self

    def cc(self, email: str, name: Optional[str] = None):
        """Add CC address."""
        self.cc_addrs.append((email, name))
        return self

    def with_subject(self, subject: str):
        """Set subject."""
        self.subject = subject
        return self

    def with_text_body(self, text: str):
        """Set text body."""
        self.body_text = text
        return self

    def with_html_body(self, html: str):
        """Set HTML body."""
        self.body_html = html
        return self

    def attach_file(self, filename: str, content_type: str, data: bytes):
        """Add file attachment."""
        self.attachments.append(
            {"filename": filename, "content_type": content_type, "data": data}
        )
        return self

    def add_header(self, name: str, value: str):
        """Add custom header.

        Note: Use with caution - some headers like Message-ID are auto-generated.
        """
        self.headers[name] = value
        return self

    def with_message_id(self, message_id: str):
        """Set Message-ID (overrides auto-generated one)."""
        self._custom_message_id = message_id
        return self

    def in_reply_to(self, message_id: str):
        """Set In-Reply-To header for threading."""
        self.headers["In-Reply-To"] = f"<{message_id}>"
        return self

    def references(self, message_ids: List[str]):
        """Set References header for threading."""
        refs = " ".join(f"<{mid}>" for mid in message_ids)
        self.headers["References"] = refs
        return self

    def build_mime(self) -> bytes:
        """Build MIME message as bytes."""
        # Create message
        if self.attachments or (self.body_text and self.body_html):
            msg = MIMEMultipart("mixed")
        else:
            msg = MIMEMultipart("alternative")

        # Set headers
        if self.from_name:
            msg["From"] = f"{self.from_name} <{self.from_addr}>"
        else:
            msg["From"] = self.from_addr

        to_list = [
            f"{name} <{email}>" if name else email for email, name in self.to_addrs
        ]
        msg["To"] = ", ".join(to_list)

        if self.cc_addrs:
            cc_list = [
                f"{name} <{email}>" if name else email for email, name in self.cc_addrs
            ]
            msg["Cc"] = ", ".join(cc_list)

        msg["Subject"] = self.subject or "Test Email"

        # Set Message-ID (custom or auto-generated)
        if self._custom_message_id:
            msg["Message-ID"] = f"<{self._custom_message_id}>"
        elif "Message-ID" not in self.headers:
            msg["Message-ID"] = f"<{uuid.uuid4()}@test.mailflow.io>"

        # Add custom headers (skip Message-ID if already set above)
        for name, value in self.headers.items():
            if name != "Message-ID":
                msg[name] = value

        # Add body
        if self.body_text or self.body_html:
            if self.body_text and self.body_html:
                # Multipart alternative
                alt = MIMEMultipart("alternative")
                if self.body_text:
                    alt.attach(MIMEText(self.body_text, "plain", "utf-8"))
                if self.body_html:
                    alt.attach(MIMEText(self.body_html, "html", "utf-8"))
                msg.attach(alt)
            elif self.body_text:
                msg.attach(MIMEText(self.body_text, "plain", "utf-8"))
            else:
                msg.attach(MIMEText(self.body_html, "html", "utf-8"))

        # Add attachments
        for attachment in self.attachments:
            part = MIMEBase(*attachment["content_type"].split("/", 1))
            part.set_payload(attachment["data"])
            encoders.encode_base64(part)
            part.add_header(
                "Content-Disposition",
                f'attachment; filename="{attachment["filename"]}"',
            )
            msg.attach(part)

        return msg.as_bytes()


def build_simple_email(
    from_email: str, to_email: str, subject: str, body: str
) -> bytes:
    """Quick helper to build simple text email."""
    return (
        EmailBuilder()
        .from_address(from_email)
        .to(to_email)
        .with_subject(subject)
        .with_text_body(body)
        .build_mime()
    )


def build_outbound_message(
    from_addr: str,
    to_addr: str,
    subject: str,
    body_text: str,
    body_html: Optional[str] = None,
    correlation_id: Optional[str] = None,
) -> Dict:
    """Build outbound message JSON."""
    return {
        "version": "1.0",
        "correlationId": correlation_id or f"test-{uuid.uuid4()}",
        "timestamp": datetime.utcnow().isoformat() + "Z",
        "source": "e2e-test-suite",
        "email": {
            "from": {"address": from_addr, "name": "E2E Test"},
            "to": [{"address": to_addr, "name": "Test Recipient"}],
            "cc": [],
            "bcc": [],
            "replyTo": {"address": from_addr},
            "subject": subject,
            "body": {"text": body_text, "html": body_html or ""},
            "attachments": [],
            "headers": {"inReplyTo": None, "references": []},
        },
        "options": {
            "priority": "normal",
            "scheduledSendTime": None,
            "trackOpens": False,
            "trackClicks": False,
        },
    }
