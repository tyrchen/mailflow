![Build Status](https://github.com/tyrchen/mailflow/workflows/build/badge.svg)

# Mailflow - Serverless Email Dispatching System

[English](README.md) | [中文](README.zh-CN.md)

---

A production-ready, serverless email dispatching system built with Rust and AWS Lambda. Mailflow enables applications to receive and respond to emails through a centralized routing mechanism, providing intelligent email routing based on recipient addresses.

## 🎯 What is Mailflow?

Mailflow is an AWS-based email infrastructure that acts as an email gateway for your applications:

- **Inbound Flow**: Receives emails via Amazon SES → Routes to app-specific SQS queues
- **Outbound Flow**: Processes app responses from SQS → Sends emails via SES
- **Smart Routing**: Routes emails based on recipient pattern (`_app1@acme.com` → `mailflow-app1` queue)
- **Zero Management**: Serverless architecture that scales automatically

## ✨ Key Features

- 🚀 **Serverless**: Built on AWS Lambda, scales automatically, pay only for what you use
- 💰 **Cost-Efficient**: ~$3/month for 10,000 emails (including all AWS services)
- 🔒 **Secure**: KMS encryption, IAM least-privilege, HTML sanitization
- 🎯 **Smart Routing**: Automatically routes emails to correct application queues
- 🔄 **Idempotent**: Prevents duplicate email sends using DynamoDB
- 📊 **Observable**: CloudWatch logs, metrics, and alarms
- 🏗️ **Infrastructure as Code**: Complete Pulumi deployment automation
- ⚡ **High Performance**: Processes 50-100 emails/minute, ~2-5s latency
- 🛡️ **Reliable**: Dead letter queues, error handling, at-least-once delivery

## 🏛️ Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Inbound Flow                             │
└─────────────────────────────────────────────────────────────────┘

Internet Email → SES → S3 (raw email) → Lambda (Router) → SQS (mailflow-app1)
                          ↓                                → SQS (mailflow-app2)
                    Attachments                            → SQS (mailflow-appN)

┌─────────────────────────────────────────────────────────────────┐
│                         Outbound Flow                            │
└─────────────────────────────────────────────────────────────────┘

App1 → SQS (mailflow-outbound) → Lambda (Sender) → SES → Internet Email
App2 →                ↓
AppN →         Idempotency Check
```

**AWS Services Used**:

- **Amazon SES**: Email reception and sending
- **Amazon S3**: Email and attachment storage
- **Amazon SQS**: Message queuing for routing
- **AWS Lambda**: Serverless email processing (Rust)
- **Amazon DynamoDB**: Idempotency tracking
- **Amazon CloudWatch**: Logging, metrics, and alarms

## 📋 Prerequisites

- **Rust**: 1.89+ with cargo
- **AWS Account**: With SES enabled
- **Domain**: For email reception (must be verified in SES)
- **Node.js**: 18+ (for Pulumi infrastructure)
- **Pulumi CLI**: For infrastructure deployment
- **cargo-lambda**: For building Lambda binaries

## 🚀 Quick Start

### 1. Install Dependencies

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install cargo-lambda
cargo install cargo-lambda

# Install Pulumi
brew install pulumi  # macOS
# Or visit: https://www.pulumi.com/docs/install/

# Install Node.js dependencies
cd infra
npm install
```

### 2. Build the Lambda Binary

```bash
# Build for AWS Lambda (ARM64)
cargo lambda build --release --arm64
```

This creates: `target/lambda/mailflow/bootstrap.zip`

### 3. Deploy Infrastructure

```bash
cd infra

# Initialize Pulumi stack
pulumi stack init dev

# Configure
pulumi config set aws:region us-east-1
pulumi config set mailflow:environment dev
pulumi config set mailflow:domains '["yourdomain.com"]'
pulumi config set mailflow:apps '["app1", "app2"]'

# Deploy!
pulumi up
```

### 4. Verify SES Domain

```bash
# Verify your domain in SES
aws ses verify-domain-identity --domain yourdomain.com

# Add DNS TXT record (check email for verification code)
# Wait for verification

# Check status
aws ses list-verified-email-addresses
```

### 5. Test It Out

**Send a test email to your app**:

```bash
aws ses send-email \
  --from test@yourdomain.com \
  --destination ToAddresses=_app1@yourdomain.com \
  --message "Subject={Data=Test Email},Body={Text={Data=Hello from Mailflow!}}"
```

**Check the app queue**:

```bash
# Get queue URL
APP_QUEUE=$(pulumi stack output appQueueUrls -j | jq -r '.app1')

# Receive message
aws sqs receive-message --queue-url $APP_QUEUE --max-number-of-messages 1
```

**Send a response email**:

```bash
OUTBOUND_QUEUE=$(pulumi stack output outboundQueueUrl)

aws sqs send-message --queue-url $OUTBOUND_QUEUE --message-body '{
  "version": "1.0",
  "correlation_id": "test-'$(date +%s)'",
  "timestamp": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
  "source": "app1",
  "email": {
    "from": {"address": "_app1@yourdomain.com", "name": "App1"},
    "to": [{"address": "recipient@example.com"}],
    "subject": "Test Response",
    "body": {"text": "This is a test response from Mailflow"}
  }
}'
```

## 📖 How It Works

### Inbound Email Processing

1. **Email arrives** at your domain (e.g., `_app1@yourdomain.com`)
2. **SES receives** the email and saves raw content to S3
3. **Lambda triggers** and downloads the email
4. **Parser extracts** headers, body, and metadata
5. **Router determines** destination based on recipient:
   - `_app1@yourdomain.com` → routes to `mailflow-app1` queue
   - `user@yourdomain.com` → routes to `mailflow-default` queue
6. **Message sent** to SQS queue with full email data in JSON format
7. **Your app** polls the queue and processes the email

### Outbound Email Processing

1. **Your app** sends JSON message to `mailflow-outbound` queue
2. **Lambda polls** the queue and receives messages
3. **Validator** checks message schema and required fields
4. **Idempotency** check prevents duplicate sends
5. **Composer** builds MIME email (text, HTML, or multipart)
6. **SES sends** the email to recipients
7. **Tracker** records correlation ID in DynamoDB
8. **Queue cleaned** - message deleted after successful send

## 🎨 Use Cases

- **Multi-App Email Gateway**: Route emails to different applications based on recipient
- **Support Ticketing**: Receive support emails and route to ticketing system
- **Invoice Processing**: Parse invoice emails and route to accounting app
- **Notification System**: Send transactional emails from multiple apps
- **Email Automation**: Build email workflows with routing logic

## 📊 Message Format

### Inbound Message (sent to app queues)

```json
{
  "version": "1.0",
  "messageId": "mailflow-uuid",
  "timestamp": "2025-10-31T12:34:56Z",
  "source": "mailflow",
  "email": {
    "messageId": "original-email-id",
    "from": {"address": "sender@example.com", "name": "Sender Name"},
    "to": [{"address": "_app1@yourdomain.com"}],
    "subject": "Email Subject",
    "body": {
      "text": "Plain text body",
      "html": "<html>HTML body</html>"
    },
    "headers": {
      "in-reply-to": "previous-message-id",
      "references": ["msg-1", "msg-2"]
    }
  },
  "metadata": {
    "routingKey": "app1",
    "domain": "yourdomain.com"
  }
}
```

### Outbound Message (send to mailflow-outbound queue)

```json
{
  "version": "1.0",
  "correlationId": "unique-id-123",
  "timestamp": "2025-10-31T12:34:56Z",
  "source": "app1",
  "email": {
    "from": {"address": "_app1@yourdomain.com", "name": "App1"},
    "to": [{"address": "recipient@example.com"}],
    "subject": "Response Email",
    "body": {
      "text": "Plain text response",
      "html": "<p>HTML response</p>"
    }
  }
}
```

## 🔧 Configuration

All configuration is managed through Pulumi. To add a new app:

```bash
# Update app list
pulumi config set mailflow:apps '["app1", "app2", "newapp"]'

# Deploy
pulumi up
```

This automatically:

- Creates `mailflow-newapp-dev` SQS queue
- Updates Lambda routing map
- Updates IAM permissions

**Total time**: ~2 minutes, **zero code changes**!

## 📈 Monitoring

**CloudWatch Alarms**:

- Lambda errors (>10 in 5 minutes)
- DLQ has messages (>0)
- Slow processing (>30s average)

**View Logs**:

```bash
aws logs tail /aws/lambda/mailflow-dev --follow
```

**Check Queue Depth**:

```bash
aws sqs get-queue-attributes \
  --queue-url $(pulumi stack output outboundQueueUrl) \
  --attribute-names ApproximateNumberOfMessages
```

## 🧪 Development

```bash
# Run tests
make test

# Format code
make fmt-fix

# Lint
make lint

# Full check (fmt + lint + test)
make check

# Build for local testing
make build

# Build for Lambda
make lambda-build
```

## 📚 Documentation

- **[Product Spec](specs/0001-spec.md)**: Detailed requirements and features
- **[Design Spec](specs/0002-design.md)**: Architecture and technical design
- **[Implementation Plan](specs/0003-implementation-plan.md)**: Development roadmap
- **[Review](REVIEW.md)**: Gap analysis and compliance check
- **[Status](STATUS.md)**: Implementation status
- **[Infrastructure Guide](infra/README.md)**: Complete setup instructions

## 💰 Cost Breakdown

**Monthly costs for 10,000 emails**:

- Lambda: $0.10
- S3: $0.30
- SQS: $0.02
- DynamoDB: $1.25
- SES: $1.00
- CloudWatch: $0.50

**Total: ~$3.27/month** 💸

## 🤝 Contributing

1. Fork the repository
2. Create your feature branch
3. Make your changes
4. Run tests: `make check`
5. Submit a pull request

## 📄 License

This project is distributed under the terms of MIT.

See [LICENSE](LICENSE.md) for details.

Copyright 2025 Tyr Chen
