# Mailflow Infrastructure Guide

Complete guide for deploying the Mailflow serverless email dispatching system on AWS using Pulumi.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Step-by-Step Setup](#step-by-step-setup)
  - [1. Install Dependencies](#1-install-dependencies)
  - [2. Configure AWS Credentials](#2-configure-aws-credentials)
  - [3. Build Lambda Binary](#3-build-lambda-binary)
  - [4. Initialize Pulumi Stack](#4-initialize-pulumi-stack)
  - [5. Configure Stack](#5-configure-stack)
  - [6. Deploy Infrastructure](#6-deploy-infrastructure)
  - [7. Verify SES Domain](#7-verify-ses-domain)
  - [8. Test the System](#8-test-the-system)
- [Configuration Reference](#configuration-reference)
- [Managing Your Infrastructure](#managing-your-infrastructure)
- [Troubleshooting](#troubleshooting)
- [Cost Estimation](#cost-estimation)
- [Advanced Topics](#advanced-topics)

## Prerequisites

Before you begin, ensure you have the following:

### Required Tools

1. **Rust** (1.70+)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **cargo-lambda** (for building Lambda binaries)
   ```bash
   cargo install cargo-lambda
   ```

3. **Node.js** (18+)
   ```bash
   # macOS
   brew install node

   # Or download from https://nodejs.org/
   ```

4. **Pulumi CLI**
   ```bash
   # macOS
   brew install pulumi

   # Linux
   curl -fsSL https://get.pulumi.com | sh

   # Windows
   choco install pulumi

   # Or visit: https://www.pulumi.com/docs/install/
   ```

5. **AWS CLI** (optional but recommended)
   ```bash
   # macOS
   brew install awscli

   # Or visit: https://aws.amazon.com/cli/
   ```

### AWS Requirements

1. **AWS Account** with appropriate permissions
2. **Domain name** that you control (for receiving emails)
3. **SES enabled** in your AWS region (SES is available in specific regions)

### Verify Installation

```bash
# Check all tools are installed
rust --version      # Should be 1.70+
cargo --version
cargo lambda --version
node --version      # Should be 18+
npm --version
pulumi version
aws --version       # Optional
```

## Quick Start

If you're familiar with AWS and Pulumi, here's the TL;DR:

```bash
# 1. Build Lambda binary
make lambda-build

# 2. Install dependencies
cd infra
npm install

# 3. Configure and deploy
pulumi stack init dev
pulumi config set aws:region us-east-1
pulumi config set mailflow:environment dev
pulumi config set mailflow:domains '["yourdomain.com"]'
pulumi config set mailflow:apps '["app1", "app2"]'
pulumi up

# 4. Verify domain in SES (see detailed instructions below)
```

## Step-by-Step Setup

### 1. Install Dependencies

First, navigate to the infrastructure directory and install Node.js dependencies:

```bash
cd infra
npm install
```

This will install:
- `@pulumi/pulumi` - Pulumi SDK
- `@pulumi/aws` - AWS provider
- TypeScript and related tooling

### 2. Configure AWS Credentials

Pulumi uses the AWS SDK, which reads credentials from standard AWS credential locations.

#### Option A: AWS CLI (Recommended)

```bash
aws configure
```

You'll be prompted for:
- AWS Access Key ID
- AWS Secret Access Key
- Default region (e.g., `us-east-1`)
- Output format (press Enter for default)

#### Option B: Environment Variables

```bash
export AWS_ACCESS_KEY_ID="your-access-key"
export AWS_SECRET_ACCESS_KEY="your-secret-key"
export AWS_REGION="us-east-1"
```

#### Option C: AWS Credentials File

Create/edit `~/.aws/credentials`:

```ini
[default]
aws_access_key_id = your-access-key
aws_secret_access_key = your-secret-key
```

And `~/.aws/config`:

```ini
[default]
region = us-east-1
```

#### Verify AWS Access

```bash
# Test AWS credentials
aws sts get-caller-identity

# Expected output shows your account ID and user/role ARN
{
    "UserId": "AIDAI...",
    "Account": "123456789012",
    "Arn": "arn:aws:iam::123456789012:user/yourname"
}
```

### 3. Build Lambda Binary

Before deploying, you must build the Lambda function binary:

```bash
# From the project root directory
cd ..  # Go back to project root if you're in infra/
make lambda-build
```

This command:
1. Builds the Rust code for AWS Lambda ARM64 architecture
2. Creates a `bootstrap.zip` file
3. Copies it to `infra/assets/` for deployment

**Verify the build:**

```bash
ls -lh infra/assets/bootstrap.zip
# Should show a ~4-5MB file
```

### 4. Initialize Pulumi Stack

A Pulumi "stack" is an isolated instance of your infrastructure (e.g., dev, staging, prod).

```bash
cd infra

# Login to Pulumi (choose your backend)
pulumi login
# Options:
# - Press Enter for Pulumi Cloud (free for individuals)
# - Use `pulumi login --local` for local state storage
# - Use `pulumi login s3://bucket-name` for S3 backend

# Create a new stack
pulumi stack init dev
```

### 5. Configure Stack

Configure your stack with the necessary settings:

```bash
# Set AWS region (must be SES-supported: us-east-1, us-west-2, eu-west-1, etc.)
pulumi config set aws:region us-east-1

# Set environment name (used in resource naming)
pulumi config set mailflow:environment dev

# Set your domain(s) for receiving emails
pulumi config set mailflow:domains '["example.com"]'
# For multiple domains:
# pulumi config set mailflow:domains '["example.com", "mail.example.com"]'

# Set application names (each gets its own SQS queue)
pulumi config set mailflow:apps '["app1", "app2"]'
# Or customize for your use case:
# pulumi config set mailflow:apps '["support", "billing", "notifications"]'
```

**View current configuration:**

```bash
pulumi config
```

**Example output:**

```
KEY                      VALUE
aws:region              us-east-1
mailflow:apps           ["app1", "app2"]
mailflow:domains        ["example.com"]
mailflow:environment    dev
```

### 6. Deploy Infrastructure

Preview the changes before deployment:

```bash
pulumi preview
```

This shows you what resources will be created. You should see approximately 22 resources.

Deploy the infrastructure:

```bash
pulumi up
```

You'll see a detailed preview and be asked to confirm:

```
Do you want to perform this update? yes
```

The deployment takes about 2-3 minutes. Upon completion, you'll see outputs like:

```
Outputs:
    appQueueUrls        : {
        app1: "https://sqs.us-east-1.amazonaws.com/123456789012/mailflow-app1-dev"
        app2: "https://sqs.us-east-1.amazonaws.com/123456789012/mailflow-app2-dev"
    }
    defaultQueueUrl     : "https://sqs.us-east-1.amazonaws.com/123456789012/mailflow-default-dev"
    lambdaFunctionArn   : "arn:aws:lambda:us-east-1:123456789012:function:mailflow-dev"
    outboundQueueUrl    : "https://sqs.us-east-1.amazonaws.com/123456789012/mailflow-outbound-dev"
    rawEmailsBucketName : "mailflow-raw-emails-dev"
```

**Save these outputs** - you'll need them for testing and integration!

### 7. Verify SES Domain

For SES to receive emails at your domain, you must verify domain ownership.

#### Step 7.1: Get Verification Token

```bash
aws ses verify-domain-identity --domain example.com --region us-east-1
```

**Output:**

```json
{
    "VerificationToken": "abcdef123456..."
}
```

#### Step 7.2: Add DNS TXT Record

Add the following DNS TXT record to your domain:

| Name                     | Type | Value                                  |
|--------------------------|------|----------------------------------------|
| `_amazonses.example.com` | TXT  | `abcdef123456...` (from previous step) |

**How to add DNS records:**
- **Route 53**: AWS Console → Route 53 → Hosted Zones → Your domain → Create record
- **Cloudflare**: Dashboard → DNS → Add record
- **GoDaddy**: Domain settings → DNS → Add TXT record
- **Other providers**: Consult your DNS provider's documentation

#### Step 7.3: Add MX Record

Add an MX record to route emails to SES:

| Name          | Type | Priority | Value                                  |
|---------------|------|----------|----------------------------------------|
| `example.com` | MX   | 10       | `inbound-smtp.us-east-1.amazonaws.com` |

**Note:** Adjust the region in the MX record to match your deployment region:
- `us-east-1`: `inbound-smtp.us-east-1.amazonaws.com`
- `us-west-2`: `inbound-smtp.us-west-2.amazonaws.com`
- `eu-west-1`: `inbound-smtp.eu-west-1.amazonaws.com`

#### Step 7.4: Wait for Verification

DNS propagation can take 5 minutes to 48 hours (typically ~15 minutes).

**Check verification status:**

```bash
aws ses get-identity-verification-attributes \
  --identities example.com \
  --region us-east-1
```

**Expected output when verified:**

```json
{
    "VerificationAttributes": {
        "example.com": {
            "VerificationStatus": "Success"
        }
    }
}
```

#### Step 7.5: Request Production Access (if needed)

New AWS accounts start in **SES Sandbox mode**, which:
- Only allows sending to verified email addresses
- Limits sending volume

To remove sandbox restrictions:

1. Go to AWS Console → SES → Account Dashboard
2. Click "Request production access"
3. Fill out the form (explain your use case)
4. AWS typically approves within 24 hours

**For receiving emails (inbound), sandbox mode doesn't apply!** You can receive emails immediately after domain verification.

### 8. Test the System

Now let's verify everything works!

#### Test 8.1: Send Inbound Email

Send a test email to your app:

```bash
# Send test email
aws ses send-email \
  --from verified@example.com \
  --destination ToAddresses=_app1@example.com \
  --message "Subject={Data=Test Email},Body={Text={Data=Hello from Mailflow!}}" \
  --region us-east-1
```

**Note:** Replace `verified@example.com` with an email you've verified in SES (required in sandbox mode).

#### Test 8.2: Check SQS Queue

Verify the email was routed to the correct queue:

```bash
# Get the queue URL for app1
APP_QUEUE=$(pulumi stack output appQueueUrls -j | jq -r '.app1')

# Receive message from queue
aws sqs receive-message \
  --queue-url $APP_QUEUE \
  --max-number-of-messages 1 \
  --region us-east-1
```

You should see a JSON message with the email details!

#### Test 8.3: Send Outbound Email

Send an email through Mailflow:

```bash
# Get outbound queue URL
OUTBOUND_QUEUE=$(pulumi stack output outboundQueueUrl)

# Send message to outbound queue
aws sqs send-message \
  --queue-url $OUTBOUND_QUEUE \
  --message-body '{
    "version": "1.0",
    "correlationId": "test-'$(date +%s)'",
    "timestamp": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "source": "app1",
    "email": {
      "from": {"address": "_app1@example.com", "name": "App1"},
      "to": [{"address": "recipient@example.com"}],
      "subject": "Test from Mailflow",
      "body": {"text": "This is a test email sent via Mailflow!"}
    }
  }' \
  --region us-east-1
```

The Lambda function will process this within seconds and send the email via SES!

#### Test 8.4: Check CloudWatch Logs

Monitor Lambda execution:

```bash
# View logs
aws logs tail /aws/lambda/mailflow-dev --follow --region us-east-1
```

## Configuration Reference

All configuration is stored in your Pulumi stack settings.

### Required Configuration

| Key                    | Type   | Description                         | Example            |
|------------------------|--------|-------------------------------------|--------------------|
| `aws:region`           | string | AWS region for deployment           | `us-east-1`        |
| `mailflow:environment` | string | Environment name (dev/staging/prod) | `dev`              |
| `mailflow:domains`     | array  | Domains for receiving emails        | `["example.com"]`  |
| `mailflow:apps`        | array  | Application names for routing       | `["app1", "app2"]` |

### View All Settings

```bash
# Show configuration
pulumi config

# Show configuration with values
pulumi config --show-secrets

# Get specific value
pulumi config get mailflow:apps
```

### Update Configuration

```bash
# Update a setting
pulumi config set mailflow:apps '["app1", "app2", "app3"]'

# Apply changes
pulumi up
```

## Managing Your Infrastructure

### View Stack Outputs

```bash
# Show all outputs
pulumi stack output

# Get specific output
pulumi stack output lambdaFunctionArn

# Get output as JSON
pulumi stack output appQueueUrls -j
```

### Add a New Application

To add a new application queue:

```bash
# Update apps list
pulumi config set mailflow:apps '["app1", "app2", "newapp"]'

# Deploy changes
pulumi up
```

This automatically:
- Creates `mailflow-newapp-dev` SQS queue
- Updates Lambda routing configuration
- Updates IAM permissions

**Time:** ~2 minutes, **zero code changes!**

### Add a New Domain

```bash
# Add domain
pulumi config set mailflow:domains '["example.com", "new-domain.com"]'

# Deploy
pulumi up

# Verify the new domain in SES (see Step 7)
```

### Update Lambda Code

After making code changes:

```bash
# 1. Rebuild Lambda
cd ..  # Go to project root
make lambda-build

# 2. Redeploy
cd infra
pulumi up
```

Pulumi detects the changed binary and updates only the Lambda function.

### Destroy Infrastructure

To tear down all resources:

```bash
# Preview what will be deleted
pulumi destroy --preview

# Destroy resources
pulumi destroy

# Remove stack
pulumi stack rm dev
```

**Warning:** This deletes all resources, including S3 buckets and DynamoDB tables. Data loss is permanent!

## Troubleshooting

### Problem: Pulumi Preview Fails with TypeScript Error

**Error:**
```
error: TSError: ⨯ Unable to compile TypeScript
```

**Solution:**
```bash
# Reinstall dependencies
npm install

# Clear node modules and reinstall
rm -rf node_modules package-lock.json
npm install
```

### Problem: Lambda Deployment Package Not Found

**Error:**
```
failed to compute archive hash for "code": no such file or directory
```

**Solution:**
```bash
# Build the Lambda binary first
cd ..  # Go to project root
make lambda-build

# Verify the file exists
ls -lh infra/assets/bootstrap.zip
```

### Problem: SES Domain Verification Stuck

**Symptoms:**
- Verification status shows "Pending"
- DNS records added but not verified

**Solution:**

1. Verify DNS record is correct:
   ```bash
   dig TXT _amazonses.example.com
   ```

2. Wait 15-30 minutes for DNS propagation

3. Check for typos in the verification token

4. Ensure you added the record to the correct domain

### Problem: Emails Not Being Received

**Checklist:**

1. Is domain verified?
   ```bash
   aws ses get-identity-verification-attributes \
     --identities example.com --region us-east-1
   ```

2. Is MX record configured correctly?
   ```bash
   dig MX example.com
   ```

3. Are SES receipt rules active?
   ```bash
   aws ses describe-active-receipt-rule-set --region us-east-1
   ```

4. Check Lambda logs for errors:
   ```bash
   aws logs tail /aws/lambda/mailflow-dev --follow --region us-east-1
   ```

### Problem: Lambda Timeout or Memory Errors

**Solution:**

Edit `infra/src/lambda.ts` and increase limits:

```typescript
timeout: 120,      // seconds (default: 60)
memorySize: 512,   // MB (default: 256)
```

Then redeploy:
```bash
pulumi up
```

### Problem: SQS Queue Not Receiving Messages

**Debug steps:**

1. Check Lambda execution logs
2. Verify routing configuration:
   ```bash
   aws lambda get-function-configuration \
     --function-name mailflow-dev \
     --region us-east-1 \
     --query 'Environment.Variables.ROUTING_MAP'
   ```

3. Test with default queue (no `_` prefix):
   ```bash
   # Get default queue URL
   DEFAULT_QUEUE=$(pulumi stack output defaultQueueUrl)

   # Receive messages
   aws sqs receive-message --queue-url $DEFAULT_QUEUE
   ```

### Problem: Permission Denied Errors

**Error:**
```
User is not authorized to perform: [action]
```

**Solution:**

Ensure your AWS user/role has the required permissions:
- `s3:*` for S3 operations
- `lambda:*` for Lambda management
- `sqs:*` for SQS operations
- `ses:*` for SES configuration
- `iam:*` for creating roles and policies
- `cloudwatch:*` for logging and metrics
- `dynamodb:*` for idempotency table

### Getting Help

If you're still stuck:

1. Check CloudWatch logs:
   ```bash
   aws logs tail /aws/lambda/mailflow-dev --follow
   ```

2. Review Pulumi logs:
   ```bash
   pulumi logs --follow
   ```

3. Enable debug logging:
   ```bash
   pulumi up --logtostderr -v=9
   ```

4. Open an issue on GitHub with:
   - Your configuration (sanitized)
   - Error messages
   - Pulumi/AWS CLI versions

## Cost Estimation

### Monthly Costs (Typical Usage: 10,000 emails)

| Service             | Usage                                 | Cost             |
|---------------------|---------------------------------------|------------------|
| **AWS Lambda**      | 10K invocations, 256MB, 5s avg        | ~$0.10           |
| **Amazon S3**       | 10K emails, 1KB each, 7-day retention | ~$0.30           |
| **Amazon SQS**      | 20K requests (in + out)               | ~$0.02           |
| **Amazon DynamoDB** | 10K writes, 1KB items                 | ~$1.25           |
| **Amazon SES**      | 10K emails (1K free)                  | ~$1.00           |
| **CloudWatch**      | Logs and metrics                      | ~$0.50           |
| **Total**           |                                       | **~$3.17/month** |

### Cost Optimization Tips

1. **Reduce S3 retention**: Lower `lifecycleRules` days in `storage.ts`
2. **Batch SQS polling**: Increase `batchSize` in `lambda.ts`
3. **Right-size Lambda**: Monitor usage and adjust memory
4. **Use ARM64**: Already configured (cheaper than x86)

### Cost Scaling

| Volume                 | Monthly Cost |
|------------------------|--------------|
| 1,000 emails/month     | ~$1.50       |
| 10,000 emails/month    | ~$3.17       |
| 100,000 emails/month   | ~$15         |
| 1,000,000 emails/month | ~$85         |

## Advanced Topics

### Multi-Region Deployment

Deploy to multiple regions for redundancy:

```bash
# Create region-specific stacks
pulumi stack init us-east-1-prod
pulumi config set aws:region us-east-1
pulumi up

pulumi stack init eu-west-1-prod
pulumi config set aws:region eu-west-1
pulumi up
```

### Custom Domain Names

Use Route 53 for subdomain-based routing:

```bash
# Configure multiple domains
pulumi config set mailflow:domains '["support.example.com", "billing.example.com"]'
```

### Email Attachment Handling

Attachments are automatically stored in S3. Access them via:

```typescript
// In your application code
const s3Key = message.metadata.s3ObjectKey;
const bucket = message.metadata.s3Bucket;

// Download attachment from S3
const attachment = await s3.getObject({ Bucket: bucket, Key: s3Key });
```

### High Availability Setup

1. **Multi-AZ**: SQS and Lambda are already multi-AZ
2. **DLQ monitoring**: Set up alerts on dead letter queue
3. **Backup SES**: Configure backup MX records
4. **Lambda reserved concurrency**: Prevent throttling

```typescript
// In lambda.ts
reservedConcurrentExecutions: 100,
```

### Monitoring and Alerts

Customize CloudWatch alarms in `infra/src/monitoring.ts`:

```typescript
// Add custom metric alarm
new aws.cloudwatch.MetricAlarm("custom-alarm", {
    metricName: "Errors",
    threshold: 5,
    evaluationPeriods: 1,
    // ... more config
});
```

### Integration with Existing Systems

**Webhook Integration:**
```bash
# Poll SQS from your app
while true; do
  aws sqs receive-message --queue-url $APP_QUEUE
  # Process messages
  # Delete after processing
done
```

**Lambda Integration:**
```typescript
// Configure Lambda to trigger on queue messages
const eventSource = new aws.lambda.EventSourceMapping("trigger", {
    eventSourceArn: queue.arn,
    functionName: yourFunction.name,
});
```

## Project Structure

```
infra/
├── README.md              # This file
├── package.json           # Node.js dependencies
├── tsconfig.json          # TypeScript configuration
├── Pulumi.yaml           # Pulumi project metadata
├── assets/               # Lambda deployment packages (gitignored)
│   └── bootstrap.zip     # Built by 'make lambda-build'
└── src/
    ├── index.ts          # Main Pulumi program
    ├── storage.ts        # S3 bucket configuration
    ├── queues.ts         # SQS queue setup
    ├── iam.ts            # IAM roles and policies
    ├── lambda.ts         # Lambda function configuration
    ├── ses.ts            # SES receipt rules
    ├── database.ts       # DynamoDB table
    └── monitoring.ts     # CloudWatch alarms
```

## Next Steps

1. **Production Setup**
   - Create a `prod` stack
   - Request SES production access
   - Set up monitoring and alerts
   - Configure backup and disaster recovery

2. **Security Hardening**
   - Enable AWS CloudTrail
   - Set up AWS Config rules
   - Implement least-privilege IAM policies
   - Enable S3 bucket versioning

3. **Integration**
   - Build your application to consume SQS queues
   - Implement email processing logic
   - Add custom email templates
   - Set up webhook endpoints

4. **Optimization**
   - Monitor costs and usage
   - Right-size Lambda resources
   - Implement email filtering
   - Add caching where appropriate

---

## Resources

- [Pulumi Documentation](https://www.pulumi.com/docs/)
- [AWS SES Documentation](https://docs.aws.amazon.com/ses/)
- [AWS Lambda Documentation](https://docs.aws.amazon.com/lambda/)
- [Project GitHub Repository](https://github.com/tyrchen/mailflow)

## Support

For issues, questions, or contributions:
- Open an issue on GitHub
- Check the main [README](../README.md) for project overview
- Review [implementation status](../STATUS.md)
