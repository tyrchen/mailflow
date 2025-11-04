# Mailflow Dashboard Deployment Guide

This guide covers deploying the complete Mailflow system including the dashboard.

## Prerequisites

### Software Requirements
- Rust 1.89+ with `aarch64-unknown-linux-gnu` target
- Node.js 20+
- Pulumi CLI
- AWS CLI configured with credentials
- yarn or npm

### AWS Account Setup
- AWS Account with admin access
- SES verified domain
- SES out of sandbox mode (for production)

## Quick Start

```bash
# 1. Build Lambda functions
make lambda

# 2. Build dashboard
make dashboard-build

# 3. Deploy infrastructure
cd infra && pulumi up

# 4. Deploy dashboard to S3
make dashboard-deploy ENVIRONMENT=dev
```

## Detailed Deployment Steps

### Step 1: Configure Pulumi

```bash
cd infra

# Set environment
pulumi config set environment dev  # or prod

# Set domains (SES verified)
pulumi config set domains '["example.com"]'

# Set apps
pulumi config set apps '["app1","app2","invoices"]'

# Set JWT issuer
pulumi config set jwtIssuer "your-identity-provider-domain"

# Optional: Set allowed sender domains
pulumi config set allowedSenderDomains '["trusted-domain.com"]'
```

### Step 2: Create JWKS File

Create `infra/.jwks.json` with your RS256 public keys:

```json
{
  "keys": [
    {
      "kty": "RSA",
      "kid": "your-key-id",
      "n": "modulus-base64",
      "e": "exponent-base64"
    }
  ]
}
```

**Note:** This file is gitignored for security.

### Step 3: Build Lambda Functions

```bash
# From project root
make lambda
```

This will:
- Build `mailflow-worker` for ARM64
- Build `mailflow-api` for ARM64
- Package to `assets/bootstrap.zip` and `assets/api-bootstrap.zip`

### Step 4: Deploy Infrastructure

```bash
cd infra
pulumi up
```

This creates:
- S3 buckets (raw emails, attachments, dashboard)
- SQS queues (per-app, outbound, DLQ)
- DynamoDB tables (idempotency, test-history)
- Lambda functions (worker, API)
- API Gateway
- CloudFront distribution
- IAM roles and policies
- CloudWatch log groups

**Save the outputs:**
```bash
pulumi stack output > outputs.json
```

Key outputs:
- `apiUrl`: API Gateway URL
- `dashboardUrl`: CloudFront URL
- `lambdaFunctionName`: Worker Lambda name
- `apiLambdaName`: API Lambda name

### Step 5: Configure Dashboard Environment

```bash
cd dashboard
cp .env.example .env
```

Edit `.env`:
```bash
VITE_API_URL=https://your-api-id.execute-api.us-east-1.amazonaws.com/v1/api
```

### Step 6: Build Dashboard

```bash
# From project root
make dashboard-build
```

### Step 7: Deploy Dashboard to S3

```bash
make dashboard-deploy ENVIRONMENT=dev
```

Or manually:
```bash
aws s3 sync dashboard/dist/ s3://mailflow-dashboard-dev/ --delete
```

### Step 8: Invalidate CloudFront Cache (Optional)

```bash
DISTRIBUTION_ID=$(cd infra && pulumi stack output -j | jq -r '.distributionId')
aws cloudfront create-invalidation --distribution-id $DISTRIBUTION_ID --paths "/*"
```

### Step 9: Verify Deployment

```bash
# Test API
curl https://your-api-url/api/health

# Expected response:
# {"status":"healthy","version":"0.2.2",...}

# Access dashboard
open https://your-cloudfront-url
```

## Environment-Specific Deployments

### Development Environment

```bash
pulumi stack select dev
pulumi config set environment dev
pulumi up
make dashboard-deploy ENVIRONMENT=dev
```

### Production Environment

```bash
pulumi stack select prod
pulumi config set environment prod
pulumi up
make dashboard-deploy ENVIRONMENT=prod
```

## Updating the System

### Update Lambda Code

```bash
# 1. Make code changes
# 2. Rebuild
make lambda

# 3. Redeploy
cd infra && pulumi up
```

### Update Dashboard

```bash
# 1. Make changes to dashboard/src
# 2. Rebuild and deploy
make dashboard-build
make dashboard-deploy ENVIRONMENT=dev

# 3. Invalidate CloudFront
aws cloudfront create-invalidation \
  --distribution-id YOUR_DIST_ID \
  --paths "/*"
```

### Update Configuration

```bash
# 1. Edit infra/src/*.ts
# 2. Deploy
cd infra && pulumi up
```

## Rollback

### Rollback Infrastructure

```bash
cd infra
pulumi stack history
pulumi stack select <previous-version>
pulumi up
```

### Rollback Dashboard

```bash
# Redeploy previous version from S3 versioning
aws s3api list-object-versions \
  --bucket mailflow-dashboard-dev \
  --prefix index.html

# Restore specific version
aws s3api restore-object \
  --bucket mailflow-dashboard-dev \
  --key index.html \
  --version-id <version-id>
```

## Monitoring Post-Deployment

### Check Lambda Logs

```bash
aws logs tail /aws/lambda/mailflow-dev --follow
aws logs tail /aws/lambda/mailflow-api-dev --follow
```

### Check Metrics

```bash
# Via dashboard
open https://your-dashboard-url

# Via AWS CLI
aws cloudwatch get-metric-statistics \
  --namespace Mailflow \
  --metric-name InboundEmailsReceived \
  --start-time 2025-11-03T00:00:00Z \
  --end-time 2025-11-03T23:59:59Z \
  --period 3600 \
  --statistics Sum
```

### Check Queue Status

```bash
# List queues
aws sqs list-queues | grep mailflow

# Check DLQ
aws sqs get-queue-attributes \
  --queue-url <dlq-url> \
  --attribute-names ApproximateNumberOfMessages
```

## Troubleshooting

### Lambda Functions Not Triggering

- Check SES receipt rules: `aws ses describe-receipt-rule-set`
- Check SQS event source mapping: `aws lambda list-event-source-mappings`
- Check Lambda logs for errors

### API Returns 401 Unauthorized

- Verify JWT token is valid and not expired
- Check `JWT_ISSUER` environment variable matches token issuer
- Verify user is in "Team Mailflow"
- Check `JWKS_JSON` is correctly set in Lambda environment

### Dashboard Not Loading

- Check CloudFront distribution status: `aws cloudfront get-distribution`
- Verify S3 bucket has files: `aws s3 ls s3://mailflow-dashboard-dev/`
- Check browser console for errors
- Verify VITE_API_URL in dashboard is correct

### Emails Not Being Processed

- Check SES receipt rules are active
- Verify S3 bucket permissions
- Check Lambda execution role has necessary permissions
- Review CloudWatch logs for errors

## Cost Optimization

### Lambda
- Use ARM64 architecture (already configured)
- Optimize memory allocation based on metrics
- Use provisioned concurrency only if needed

### S3
- Enable lifecycle policies for old emails
- Use S3 Intelligent-Tiering for attachments

### CloudWatch
- Adjust log retention (currently 30 days)
- Use log filtering to reduce storage

### API Gateway
- Enable caching for read-only endpoints
- Use usage plans to prevent abuse

## Security Checklist

- [ ] SES domain verified and DKIM enabled
- [ ] JWKS file is gitignored
- [ ] Lambda functions have least-privilege IAM roles
- [ ] S3 buckets have public access blocked
- [ ] CloudFront enforces HTTPS only
- [ ] API rate limiting configured
- [ ] CloudWatch logs don't contain PII
- [ ] Secrets stored in AWS Secrets Manager (not env vars)

## Production Readiness

Before deploying to production:

1. **Load Testing**
   - Test with expected email volume
   - Verify SQS queue can handle burst traffic
   - Monitor Lambda concurrency limits

2. **Security Audit**
   - Run `cargo audit` on Rust code
   - Run `yarn audit` on dashboard dependencies
   - Review IAM permissions
   - Test JWT validation with invalid tokens

3. **Backup Strategy**
   - Enable S3 versioning for dashboard bucket
   - Configure DynamoDB point-in-time recovery
   - Export Pulumi state regularly

4. **Monitoring**
   - Set up CloudWatch alarms
   - Configure SNS topics for alerts
   - Set up external monitoring (Datadog, etc.)

5. **Documentation**
   - Update team wiki with dashboard URL
   - Document JWT token generation process
   - Create runbook for common issues

## Additional Resources

- [Pulumi AWS Guide](https://www.pulumi.com/docs/clouds/aws/)
- [AWS SES Documentation](https://docs.aws.amazon.com/ses/)
- [Refine Documentation](https://refine.dev/docs/)
