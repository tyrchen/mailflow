# Production Deployment Checklist

Use this checklist before deploying Mailflow to production.

## Pre-Deployment

### Code Quality
- [ ] All tests passing: `cargo test --workspace`
- [ ] No clippy warnings: `cargo clippy --workspace -- -D warnings`
- [ ] Code formatted: `cargo fmt --check`
- [ ] Dashboard builds: `make dashboard-build`
- [ ] No TypeScript errors
- [ ] No console errors in browser

### Security
- [ ] Run security audits: `make audit-all`
- [ ] No high or critical vulnerabilities
- [ ] JWKS file is gitignored (`infra/.jwks.json`)
- [ ] Reviewed `docs/SECURITY.md` checklist
- [ ] JWT issuer configured correctly
- [ ] Team validation enabled

### Configuration
- [ ] Pulumi stack created: `pulumi stack init prod`
- [ ] Environment set: `pulumi config set environment prod`
- [ ] Domains configured and SES verified
- [ ] Apps list configured
- [ ] JWT issuer set
- [ ] Allowed sender domains configured (if applicable)

### AWS Setup
- [ ] SES domain verified and out of sandbox
- [ ] DKIM enabled for domain
- [ ] SPF record configured
- [ ] DMARC policy set
- [ ] AWS account limits checked (Lambda concurrency, SES quota)
- [ ] IAM roles reviewed

### Infrastructure
- [ ] Review `infra/Pulumi.prod.yaml` configuration
- [ ] DynamoDB tables have backups enabled
- [ ] S3 buckets have lifecycle policies
- [ ] CloudWatch log retention configured (30 days)
- [ ] CloudWatch alarms configured

## Deployment Steps

### 1. Build Artifacts

```bash
# Build Lambda functions
make lambda

# Verify build
ls -lh assets/*.zip

# Expected:
# - bootstrap.zip (worker)
# - api-bootstrap.zip (API)
```

### 2. Build Dashboard

```bash
make dashboard-build

# Verify bundle size
ls -lh dashboard/dist/assets/*.js

# Should be under 2 MB total (gzipped)
```

### 3. Deploy Infrastructure

```bash
cd infra
pulumi stack select prod
pulumi preview  # Review changes
pulumi up

# Save outputs
pulumi stack output > prod-outputs.json
```

### 4. Verify Lambda Deployment

```bash
# Test worker Lambda
aws lambda invoke \
  --function-name mailflow-prod \
  --payload '{}' \
  /tmp/response.json

# Test API Lambda
curl https://$(cd infra && pulumi stack output apiUrl)/api/health
```

### 5. Deploy Dashboard

```bash
# Update .env with prod API URL
cd dashboard
echo "VITE_API_URL=https://$(cd ../infra && pulumi stack output apiUrl)/api" > .env

# Rebuild with prod config
yarn build

# Deploy to S3
cd ..
make dashboard-deploy ENVIRONMENT=prod
```

### 6. Configure DNS (Optional)

```bash
# Create Route53 records for custom domain
aws route53 change-resource-record-sets \
  --hosted-zone-id Z123456 \
  --change-batch file://dns-changes.json
```

### 7. Test End-to-End

```bash
# Send test email
aws ses send-email \
  --from verified@example.com \
  --destination ToAddresses=_app1@example.com \
  --message "Subject={Data=Production Test},Body={Text={Data=Testing}}"

# Check queue
aws sqs receive-message \
  --queue-url $(cd infra && pulumi stack output -j | jq -r '.appQueueUrls.app1')

# Test dashboard
open https://$(cd infra && pulumi stack output dashboardUrl)
```

## Post-Deployment

### Monitoring Setup

- [ ] Configure CloudWatch alarms
- [ ] Set up SNS topics for alerts
- [ ] Configure PagerDuty/Slack integration
- [ ] Create CloudWatch dashboard
- [ ] Enable AWS X-Ray tracing (optional)

### Documentation

- [ ] Update team wiki with URLs
- [ ] Document JWT token generation process
- [ ] Share dashboard access instructions
- [ ] Create runbook for on-call engineers

### Backup Strategy

- [ ] Enable S3 versioning for dashboard bucket
- [ ] Configure DynamoDB point-in-time recovery
- [ ] Export Pulumi state: `pulumi stack export > backup.json`
- [ ] Document rollback procedures

### Load Testing

- [ ] Test with expected email volume
- [ ] Monitor Lambda concurrency
- [ ] Check SQS queue depths
- [ ] Verify SES sending quota
- [ ] Test burst traffic (1000 emails/min)

### Security Validation

- [ ] Test invalid JWT tokens (should return 401)
- [ ] Test expired tokens (should redirect to login)
- [ ] Test missing team membership (should return 403)
- [ ] Verify HTTPS enforcement (HTTP redirects to HTTPS)
- [ ] Test API rate limiting
- [ ] Verify PII redaction in logs

### Performance Validation

- [ ] Dashboard loads in < 2 seconds
- [ ] API p95 response time < 500ms
- [ ] Email processing p95 < 5 seconds
- [ ] No Lambda timeouts in first 24 hours

## Week 1 Post-Production

### Daily Checks (First Week)

- [ ] Check DLQ message count (should be 0)
- [ ] Review error logs
- [ ] Monitor Lambda invocations
- [ ] Check SES bounce/complaint rates
- [ ] Verify metrics are being collected

### Week 1 Review

- [ ] Review all CloudWatch alarms
- [ ] Analyze costs (Lambda, API Gateway, S3, CloudWatch)
- [ ] Gather user feedback
- [ ] Identify performance bottlenecks
- [ ] Plan optimizations

## Rollback Plan

If critical issues occur:

```bash
# 1. Disable email processing
aws ses set-receipt-rule \
  --rule-set-name mailflow-rules-prod \
  --rule-name mailflow-inbound \
  --rule '{...,"Enabled":false}'

# 2. Rollback infrastructure
cd infra
pulumi stack select prod
pulumi stack history
# Find working version
pulumi rollback <version>

# 3. Redeploy previous dashboard
git checkout <previous-tag> -- dashboard/
make dashboard-build
make dashboard-deploy ENVIRONMENT=prod

# 4. Verify rollback
curl https://your-api-url/api/health

# 5. Re-enable email processing
aws ses set-receipt-rule \
  --rule-set-name mailflow-rules-prod \
  --rule-name mailflow-inbound \
  --rule '{...,"Enabled":true}'
```

## Success Criteria

Production deployment is successful when:

- ✅ All health checks return "healthy"
- ✅ Test email flows through system in < 10 seconds
- ✅ Dashboard loads and displays metrics
- ✅ JWT authentication works
- ✅ All 12 API endpoints functional
- ✅ No errors in CloudWatch logs
- ✅ DLQ message count = 0
- ✅ SES bounce rate < 5%
- ✅ Costs within budget

## Sign-Off

Before marking deployment as complete:

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Engineering Lead | | | |
| DevOps Lead | | | |
| Security Lead | | | |
| Product Manager | | | |

## Post-Deployment Report Template

```markdown
# Mailflow Production Deployment Report

**Date:** YYYY-MM-DD
**Deployed By:** Name
**Version:** 0.2.2

## Deployment Summary
- Infrastructure: ✅/❌
- Worker Lambda: ✅/❌
- API Lambda: ✅/❌
- Dashboard: ✅/❌

## Validation Results
- Health Check: ✅/❌
- Test Email: ✅/❌
- API Endpoints: ✅/❌
- Dashboard Pages: ✅/❌

## Performance Metrics (First Hour)
- Email Processing: X emails
- Average Processing Time: Xms
- Error Rate: X%
- API Response Time p95: Xms

## Issues Encountered
- None / List issues

## Next Steps
- Monitor for 24 hours
- Review metrics daily for first week
- Collect user feedback
```
