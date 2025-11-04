# Mailflow Security Checklist

## Pre-Deployment Security Checklist

### Authentication & Authorization

- [ ] JWKS file (`infra/.jwks.json`) is gitignored
- [ ] JWT tokens expire within 24 hours
- [ ] JWT issuer validation is configured correctly
- [ ] Team membership validation ("Team Mailflow") is enforced
- [ ] API endpoints (except `/health`) require JWT authentication
- [ ] Unauthorized requests return 401 with no sensitive info

### API Security

- [ ] CORS is configured to allow only dashboard domain
- [ ] API rate limiting is enabled (100 req/min)
- [ ] Error messages don't expose stack traces or internal paths
- [ ] PII is redacted in all API responses
- [ ] Input validation on all endpoints
- [ ] SQL injection prevention (using parameterized queries)
- [ ] Path traversal prevention (sanitized file paths)

### Infrastructure Security

- [ ] S3 buckets have public access blocked
- [ ] S3 buckets use encryption at rest (AES-256 or KMS)
- [ ] CloudFront enforces HTTPS only (no HTTP)
- [ ] Lambda functions use least-privilege IAM roles
- [ ] DynamoDB tables have encryption enabled
- [ ] SQS queues use server-side encryption
- [ ] VPC configuration (if required)

### Email Security

- [ ] SES domain has DKIM enabled
- [ ] SPF records configured correctly
- [ ] DMARC policy is set
- [ ] Bounce and complaint notifications configured
- [ ] Email content is sanitized (HTML cleanup with ammonia)
- [ ] Attachment scanning enabled (if configured)
- [ ] File type validation on attachments
- [ ] Size limits enforced (35 MB attachments, 10 MB outbound)

### Secrets Management

- [ ] No secrets in source code
- [ ] JWKS stored securely (gitignored, env var in Lambda)
- [ ] AWS credentials use IAM roles (not hardcoded)
- [ ] Database credentials in AWS Secrets Manager
- [ ] API keys rotated regularly

### Logging & Monitoring

- [ ] PII redacted from CloudWatch logs
- [ ] Email addresses logged as `***@domain.com`
- [ ] Subjects logged as `Sub...[N chars]`
- [ ] Security events logged (failed auth, SPF/DKIM failures)
- [ ] CloudWatch log retention set to 30 days
- [ ] Alarms configured for security events

### Dependency Security

- [ ] Run `cargo audit` (Rust dependencies)
- [ ] Run `yarn audit` (Node dependencies)
- [ ] No high or critical vulnerabilities
- [ ] Dependencies are up to date
- [ ] Supply chain attacks mitigated (lock files committed)

### Network Security

- [ ] API Gateway uses regional endpoint
- [ ] CloudFront uses TLS 1.2+
- [ ] Security headers configured (CSP, X-Frame-Options, etc.)
- [ ] DDoS protection via CloudFront
- [ ] WAF rules configured (if required)

## Runtime Security Checklist

### Regular Security Tasks

- [ ] Review CloudWatch logs weekly for suspicious activity
- [ ] Monitor failed authentication attempts
- [ ] Check DLQ for unusual messages
- [ ] Review SES bounce/complaint rates
- [ ] Audit IAM role permissions quarterly
- [ ] Rotate JWKS keys annually
- [ ] Update dependencies monthly

### Incident Response

1. **Suspected Security Breach**
   - Immediately rotate all credentials
   - Check CloudWatch logs for unauthorized access
   - Review API Gateway access logs
   - Disable compromised Lambda functions
   - Notify security team

2. **Data Leak**
   - Identify scope of leak (check S3 access logs)
   - Revoke presigned URLs
   - Rotate encryption keys
   - Notify affected parties

3. **DDoS Attack**
   - Enable API Gateway throttling
   - Add WAF rules to block malicious IPs
   - Scale Lambda concurrency limits
   - Enable CloudFront rate limiting

## Security Testing

### Manual Tests

```bash
# Test invalid JWT
curl -H "Authorization: Bearer invalid-token" \
  https://your-api-url/api/metrics/summary

# Expected: 401 Unauthorized

# Test expired JWT
# (Generate expired token and test)

# Test missing team membership
# (Generate token without "Team Mailflow")

# Test path traversal
curl -X POST https://your-api-url/api/test/inbound \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"app":"../../../etc/passwd"}'

# Expected: Should be sanitized/rejected
```

### Automated Security Scans

```bash
# Rust security audit
cargo audit

# Node security audit
cd dashboard && yarn audit

# AWS security check
aws-vault exec prod -- \
  aws accessanalyzer list-findings
```

## Compliance

### GDPR Compliance

- [ ] PII redaction in logs implemented
- [ ] Data retention policies configured
- [ ] Right to erasure process documented
- [ ] Data processing agreement with AWS

### SOC 2 Compliance

- [ ] Access logs enabled and retained
- [ ] Encryption at rest and in transit
- [ ] Multi-factor authentication for AWS access
- [ ] Regular security audits scheduled

## Security Contacts

- **Security Team**: security@your-company.com
- **AWS Support**: Use AWS Support Center
- **Emergency**: Follow incident response plan

## References

- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [AWS Security Best Practices](https://aws.amazon.com/security/best-practices/)
- [Refine Security Guide](https://refine.dev/docs/guides-concepts/security/)
- [Rust Security Advisory Database](https://rustsec.org/)
