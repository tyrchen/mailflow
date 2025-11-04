# Mailflow Troubleshooting Guide

## Common Issues

### 1. Dashboard Won't Load

**Symptoms:**
- Browser shows "Cannot connect" or white screen
- 404 errors in browser console

**Diagnosis:**
```bash
# Check if S3 bucket exists
aws s3 ls | grep mailflow-dashboard

# Check if files are uploaded
aws s3 ls s3://mailflow-dashboard-dev/

# Check CloudFront distribution
aws cloudfront list-distributions | grep mailflow
```

**Solutions:**
- Rebuild and redeploy dashboard: `make dashboard-build && make dashboard-deploy ENVIRONMENT=dev`
- Wait for CloudFront to propagate (15-20 minutes)
- Check browser console for specific errors

### 2. API Returns 401 Unauthorized

**Symptoms:**
- All API calls return 401
- Login works but subsequent requests fail

**Diagnosis:**
```bash
# Decode your JWT token
echo "YOUR_TOKEN" | cut -d. -f2 | base64 -d | jq

# Check required fields:
# - "iss" must match JWT_ISSUER env var
# - "teams" must include "Team Mailflow"
# - "exp" must be in the future
```

**Solutions:**
- Verify JWT issuer matches Lambda env var: `JWT_ISSUER`
- Ensure token has "Team Mailflow" in teams array (case insensitive)
- Regenerate token if expired
- Check JWKS_JSON is correctly set in Lambda

### 3. Test Emails Not Sending

**Symptoms:**
- Test email form submits but nothing happens
- No email received

**Diagnosis:**
```bash
# Check API Lambda logs
aws logs tail /aws/lambda/mailflow-api-dev --since 5m

# Check SES sending quota
aws sesv2 get-account --query 'SendQuota'

# Verify sender email is verified
aws ses list-verified-email-addresses
```

**Solutions:**
- Verify SES domain: `aws ses verify-domain-identity --domain example.com`
- Check SES is out of sandbox mode (production only)
- Verify sender address in SES
- Check Lambda has SES send permissions

### 4. Emails Not Being Received

**Symptoms:**
- Email sent to `_app@domain.com` but not in queue
- No errors in logs

**Diagnosis:**
```bash
# Check SES receipt rules
aws ses describe-active-receipt-rule-set

# Check if email arrived in S3
aws s3 ls s3://mailflow-raw-emails-dev/ --recursive | tail -10

# Check worker Lambda logs
aws logs tail /aws/lambda/mailflow-dev --since 30m

# Check SQS queue
aws sqs get-queue-attributes \
  --queue-url $(aws sqs get-queue-url --queue-name mailflow-app1 | jq -r '.QueueUrl') \
  --attribute-names ApproximateNumberOfMessages
```

**Solutions:**
- Verify SES receipt rule is active
- Check S3 bucket permissions (Lambda can read)
- Verify Lambda is triggered by SES (check SNS/S3 event)
- Check routing configuration matches recipient address

### 5. Metrics Not Showing

**Symptoms:**
- Dashboard shows 0 for all metrics
- Charts are empty

**Diagnosis:**
```bash
# Check if metrics are being emitted
aws cloudwatch list-metrics --namespace Mailflow

# Query a specific metric
aws cloudwatch get-metric-statistics \
  --namespace Mailflow \
  --metric-name InboundEmailsReceived \
  --start-time $(date -u -v-1H +%Y-%m-%dT%H:%M:%S) \
  --end-time $(date -u +%Y-%m-%dT%H:%M:%S) \
  --period 300 \
  --statistics Sum
```

**Solutions:**
- Ensure worker Lambda is actually processing emails
- Check CloudWatch PutMetricData permissions
- Verify metrics service is enabled in worker code
- Wait for metrics to propagate (up to 5 minutes)

### 6. Queue Messages Stuck

**Symptoms:**
- Messages in queue but not being processed
- Messages age keeps increasing

**Diagnosis:**
```bash
# Check Lambda event source mapping
aws lambda list-event-source-mappings \
  --function-name mailflow-dev

# Check Lambda errors
aws logs filter-log-events \
  --log-group-name /aws/lambda/mailflow-dev \
  --filter-pattern "ERROR"

# Check DLQ
aws sqs get-queue-attributes \
  --queue-url <dlq-url> \
  --attribute-names All
```

**Solutions:**
- Check if Lambda function is enabled
- Verify event source mapping is active
- Check Lambda execution role permissions
- Review error logs for specific failures
- Check concurrency limits

### 7. Build Failures

**Rust Build Fails:**
```bash
# Check Rust version
rustc --version  # Should be 1.89+

# Check target is installed
rustup target list | grep aarch64-unknown-linux-gnu

# Install if missing
rustup target add aarch64-unknown-linux-gnu

# Clean and rebuild
cargo clean
make lambda
```

**Dashboard Build Fails:**
```bash
# Check Node version
node --version  # Should be 20+

# Clear cache and rebuild
rm -rf dashboard/node_modules dashboard/yarn.lock
cd dashboard && yarn install
yarn build
```

### 8. High Lambda Costs

**Diagnosis:**
```bash
# Check invocation count
aws cloudwatch get-metric-statistics \
  --namespace AWS/Lambda \
  --metric-name Invocations \
  --dimensions Name=FunctionName,Value=mailflow-dev \
  --start-time $(date -u -v-7d +%Y-%m-%dT%H:%M:%S) \
  --end-time $(date -u +%Y-%m-%dT%H:%M:%S) \
  --period 86400 \
  --statistics Sum

# Check duration
aws cloudwatch get-metric-statistics \
  --namespace AWS/Lambda \
  --metric-name Duration \
  --dimensions Name=FunctionName,Value=mailflow-dev \
  --start-time $(date -u -v-1d +%Y-%m-%dT%H:%M:%S) \
  --end-time $(date -u +%Y-%m-%dT%H:%M:%S) \
  --period 3600 \
  --statistics Average
```

**Solutions:**
- Optimize Lambda memory allocation
- Reduce timeout if too high
- Enable Lambda provisioned concurrency only if needed
- Review code for inefficiencies
- Use ARM64 architecture (already configured)

### 9. Slow Dashboard Performance

**Diagnosis:**
- Open browser DevTools → Network tab
- Check API response times
- Look for slow queries

**Solutions:**
- Enable API Gateway caching for read-only endpoints
- Optimize CloudWatch queries (reduce time range)
- Add loading indicators for slow operations
- Implement pagination for large result sets
- Use CDN (CloudFront) for static assets

### 10. CORS Errors

**Symptoms:**
- Browser console shows CORS errors
- API calls fail from dashboard

**Diagnosis:**
```bash
# Test CORS headers
curl -X OPTIONS https://your-api-url/api/queues \
  -H "Origin: https://your-dashboard-url" \
  -H "Access-Control-Request-Method: GET" \
  -v
```

**Solutions:**
- Update CORS configuration in `crates/mailflow-api/src/lib.rs`
- Verify CloudFront allows correct origins
- Check API Gateway CORS settings
- Ensure OPTIONS method is configured

## Debugging Tips

### Enable Debug Logging

**Worker Lambda:**
```bash
# Update environment variable
aws lambda update-function-configuration \
  --function-name mailflow-dev \
  --environment "Variables={RUST_LOG=debug,...}"
```

**API Lambda:**
```bash
aws lambda update-function-configuration \
  --function-name mailflow-api-dev \
  --environment "Variables={RUST_LOG=debug,...}"
```

### Test Locally

**API Lambda:**
```bash
# Use AWS SAM CLI
sam local start-api

# Or use cargo lambda
cargo lambda watch --package mailflow-api
```

**Dashboard:**
```bash
cd dashboard
yarn dev

# Access at http://localhost:5173
```

### Trace Requests

```bash
# Follow logs in real-time
aws logs tail /aws/lambda/mailflow-dev --follow --format short

# Filter for specific message ID
aws logs filter-log-events \
  --log-group-name /aws/lambda/mailflow-dev \
  --filter-pattern "message-id-123"
```

## Performance Debugging

### Identify Slow Endpoints

```bash
# Check API Gateway metrics
aws cloudwatch get-metric-statistics \
  --namespace AWS/ApiGateway \
  --metric-name Latency \
  --dimensions Name=ApiName,Value=mailflow-api-dev \
  --start-time $(date -u -v-1H +%Y-%m-%dT%H:%M:%S) \
  --end-time $(date -u +%Y-%m-%dT%H:%M:%S) \
  --period 300 \
  --statistics Average,Maximum
```

### Profile Lambda Function

```bash
# Enable X-Ray tracing
aws lambda update-function-configuration \
  --function-name mailflow-dev \
  --tracing-config Mode=Active

# View traces
aws xray get-trace-summaries \
  --start-time $(date -u -v-1H +%s) \
  --end-time $(date -u +%s)
```

## Getting Help

### Support Channels

1. **Check logs first**: `aws logs tail /aws/lambda/mailflow-dev --follow`
2. **Review documentation**: `docs/` directory
3. **Check GitHub issues**: For known problems
4. **AWS Support**: For AWS-specific issues

### Reporting Issues

When reporting issues, include:
- Error messages (sanitized of PII)
- CloudWatch log excerpts
- Steps to reproduce
- Environment (dev/prod)
- Recent changes or deployments

### Useful Commands

```bash
# Check all Lambda functions
aws lambda list-functions | grep mailflow

# Check all SQS queues
aws sqs list-queues | grep mailflow

# Check S3 buckets
aws s3 ls | grep mailflow

# Check recent deployments
cd infra && pulumi stack history
```

## Emergency Procedures

### Stop Email Processing

```bash
# Disable SES receipt rule
aws ses set-receipt-rule \
  --rule-set-name mailflow-rules \
  --rule-name mailflow-inbound \
  --rule '{...,"Enabled":false}'

# Disable Lambda event source
aws lambda update-event-source-mapping \
  --uuid <mapping-id> \
  --no-enabled
```

### Purge Queues

```bash
# Purge outbound queue (emergency only!)
aws sqs purge-queue --queue-url <queue-url>

# Move DLQ messages back to main queue
# (Use custom script or manual reprocessing)
```

### Emergency Rollback

```bash
# Rollback infrastructure
cd infra
pulumi stack history
pulumi stack select <previous-version>
pulumi up

# Rollback dashboard
# Redeploy previous version from Git
git checkout <previous-commit> -- dashboard/
make dashboard-build
make dashboard-deploy ENVIRONMENT=dev
```
