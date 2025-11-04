# Mailflow API Documentation

**Version:** 0.2.2
**Base URL:** `https://api.yourdomain.com`
**Authentication:** JWT Bearer Token

---

## Authentication

All endpoints (except `/api/health`) require JWT authentication.

**Header:**
```
Authorization: Bearer <JWT_TOKEN>
```

**JWT Claims Required:**
- `iss`: Must match `JWT_ISSUER` environment variable
- `exp`: Token must not be expired
- `teams`: Must include "Team Mailflow" (case-insensitive)

---

## Endpoints

### Health Check

#### `GET /api/health`

Check API health and AWS service connectivity.

**Auth:** None (public endpoint)

**Response:** `200 OK`
```json
{
  "status": "healthy",
  "version": "0.2.2",
  "timestamp": "2025-11-03T10:00:00Z",
  "checks": {
    "sqs": "ok",
    "s3": "ok",
    "dynamodb": "ok",
    "cloudwatch": "ok"
  }
}
```

---

### Metrics

#### `GET /api/metrics/summary`

Get 24-hour metrics summary.

**Auth:** Required

**Response:** `200 OK`
```json
{
  "period": "24h",
  "inbound": {
    "total": 1234.0,
    "rate": 0.85,
    "errorRate": 0.02
  },
  "outbound": {
    "total": 1189.0,
    "rate": 0.82,
    "errorRate": 0.01
  },
  "processing": {
    "p50": 123.0,
    "p95": 456.0,
    "p99": 789.0
  },
  "queues": {
    "active": 5,
    "dlqMessages": 3
  }
}
```

#### `GET /api/metrics/timeseries`

Get time-series metrics data.

**Auth:** Required

**Query Parameters:**
- `metric` (required): `inbound_received` | `outbound_sent` | `error_rate` | `processing_time`
- `period` (optional): `1h` | `6h` | `24h` | `7d` | `30d` (default: `24h`)
- `interval` (optional): `1m` | `5m` | `1h` | `1d` (default: `1h`)

**Response:** `200 OK`
```json
{
  "metric": "inbound_received",
  "unit": "count",
  "datapoints": [
    {"timestamp": "2025-11-03T09:00:00Z", "value": 45.0},
    {"timestamp": "2025-11-03T10:00:00Z", "value": 52.0}
  ]
}
```

---

### Queues

#### `GET /api/queues`

List all SQS queues with statistics.

**Auth:** Required

**Response:** `200 OK`
```json
{
  "queues": [
    {
      "name": "mailflow-app1",
      "url": "https://sqs.us-east-1.amazonaws.com/123/mailflow-app1",
      "type": "inbound",
      "messageCount": 12,
      "messagesInFlight": 3,
      "oldestMessageAge": 45,
      "lastActivity": "2025-11-03T10:05:00Z"
    }
  ]
}
```

#### `GET /api/queues/:name/messages`

Peek at messages in a queue (non-destructive).

**Auth:** Required

**Query Parameters:**
- `limit` (optional): 1-10 (default: 10)

**Response:** `200 OK`
```json
{
  "queueName": "mailflow-app1",
  "messages": [
    {
      "messageId": "abc123",
      "receiptHandle": "...",
      "body": "{...}",
      "attributes": {
        "SentTimestamp": "1730620800000",
        "ApproximateReceiveCount": "1"
      },
      "preview": "Email from: john@example.com, Subject: Hello"
    }
  ],
  "totalCount": 12
}
```

---

### Logs

#### `POST /api/logs/query`

Query CloudWatch logs.

**Auth:** Required

**Request Body:**
```json
{
  "logGroup": "/aws/lambda/mailflow-dev",
  "startTime": "2025-11-03T09:00:00Z",
  "endTime": "2025-11-03T10:00:00Z",
  "filterPattern": "ERROR",
  "limit": 100
}
```

**Response:** `200 OK`
```json
{
  "logs": [
    {
      "timestamp": "2025-11-03T09:15:23Z",
      "message": "Error processing email",
      "level": "ERROR",
      "context": {
        "handler": "inbound",
        "messageId": "msg-123"
      }
    }
  ],
  "nextToken": "..."
}
```

---

### Storage

#### `GET /api/storage/stats`

Get S3 bucket statistics.

**Auth:** Required

**Response:** `200 OK`
```json
{
  "buckets": [
    {
      "name": "mailflow-raw-emails-dev",
      "objectCount": 1234,
      "totalSizeBytes": 1073741824,
      "oldestObject": "2025-10-27T10:00:00Z",
      "newestObject": "2025-11-03T10:00:00Z",
      "contentTypeBreakdown": [
        {
          "contentType": "application/pdf",
          "count": 500,
          "totalSizeBytes": 524288000
        },
        {
          "contentType": "image/jpeg",
          "count": 300,
          "totalSizeBytes": 314572800
        }
      ]
    }
  ]
}
```

#### `GET /api/storage/:bucket/objects`

List objects in an S3 bucket.

**Auth:** Required

**Query Parameters:**
- `limit` (optional): 1-100 (default: 20)
- `prefix` (optional): Filter by key prefix

**Response:** `200 OK`
```json
{
  "bucket": "mailflow-raw-emails-dev",
  "objects": [
    {
      "key": "msg-123/attachments/doc.pdf",
      "size": 12345,
      "lastModified": "2025-11-03T10:00:00Z",
      "contentType": "application/pdf",
      "presignedUrl": "https://..."
    }
  ]
}
```

---

### Test Email

#### `POST /api/test/inbound`

Send a test inbound email via SES.

**Auth:** Required

**Request Body:**
```json
{
  "app": "app1",
  "from": "test@example.com",
  "subject": "Test Email",
  "body": {
    "text": "This is a test",
    "html": "<p>This is a test</p>"
  },
  "attachments": [
    {
      "filename": "test.txt",
      "contentType": "text/plain",
      "data": "base64-encoded-content"
    }
  ]
}
```

**Response:** `200 OK`
```json
{
  "success": true,
  "messageId": "test-msg-123",
  "sesMessageId": "0000....-000",
  "queueUrl": "https://sqs.us-east-1.amazonaws.com/123/mailflow-app1",
  "timestamp": "2025-11-03T10:00:00Z"
}
```

#### `POST /api/test/outbound`

Queue a test outbound email.

**Auth:** Required

**Request Body:**
```json
{
  "from": "app1",
  "to": "recipient@example.com",
  "subject": "Test Email",
  "body": {
    "text": "This is a test",
    "html": "<p>This is a test</p>"
  }
}
```

**Response:** `200 OK` (same as inbound)

#### `GET /api/test/history`

Get test email history.

**Auth:** Required

**Response:** `200 OK`
```json
{
  "tests": [
    {
      "id": "test-123",
      "type": "inbound",
      "timestamp": "2025-11-03T10:00:00Z",
      "recipient": "_app1@yourdomain.com",
      "status": "success",
      "messageId": "msg-123"
    }
  ]
}
```

---

### Configuration

#### `GET /api/config`

Get system configuration (read-only).

**Auth:** Required

**Response:** `200 OK`
```json
{
  "version": "1.0",
  "source": "environment",
  "routing": {},
  "security": {
    "requireSpf": false,
    "requireDkim": false,
    "requireDmarc": false
  },
  "attachments": {
    "bucket": "mailflow-raw-emails-dev",
    "presignedUrlExpiration": 604800,
    "maxSize": 36700160
  }
}
```

---

## Error Responses

All errors return JSON with `error` and `code` fields:

**401 Unauthorized:**
```json
{
  "error": "Invalid token: ...",
  "code": "UNAUTHORIZED"
}
```

**403 Forbidden:**
```json
{
  "error": "Access denied",
  "code": "FORBIDDEN"
}
```

**404 Not Found:**
```json
{
  "error": "Resource not found",
  "code": "NOT_FOUND"
}
```

**400 Bad Request:**
```json
{
  "error": "Invalid request: ...",
  "code": "BAD_REQUEST"
}
```

**500 Internal Server Error:**
```json
{
  "error": "An internal error occurred",
  "code": "INTERNAL_ERROR"
}
```

**500 Service Error:**
```json
{
  "error": "A service error occurred",
  "code": "SERVICE_ERROR"
}
```

---

## Rate Limiting

API Gateway enforces rate limiting:
- **100 requests per minute** per IP address
- Exceeding the limit returns `429 Too Many Requests`

---

## CORS

CORS is restricted to the configured dashboard origin:
- `ALLOWED_ORIGIN` environment variable
- Default: `https://dashboard.example.com`

Allowed methods: `GET`, `POST`, `OPTIONS`
Allowed headers: `Authorization`, `Content-Type`

---

## CloudWatch Metrics

The API emits custom CloudWatch metrics:

**Namespace:** `Mailflow/API` (configurable via `CLOUDWATCH_NAMESPACE`)

**Metrics:**
- `RequestCount` - Count of requests (by endpoint and status code)
- `ResponseTime` - Response time in milliseconds
- `ErrorCount` - Count of 4xx/5xx errors

**Dimensions:**
- `Endpoint` - HTTP method + path (e.g., "GET /api/queues")
- `StatusCode` - HTTP status code

---

## Request Logging

All requests are logged with:
- `request_id` - Unique UUID
- `method` - HTTP method
- `path` - Request path
- `user` - Email and user ID from JWT (or "anonymous")
- `status` - Response status code
- `duration_ms` - Request duration in milliseconds

**Log Format:** JSON (structured logging via tracing)

---

## Examples

### cURL

```bash
# Health check
curl https://api.yourdomain.com/api/health

# Get metrics (with auth)
curl -H "Authorization: Bearer $TOKEN" \
  https://api.yourdomain.com/api/metrics/summary

# Query logs
curl -X POST \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "logGroup": "/aws/lambda/mailflow-dev",
    "startTime": "2025-11-03T09:00:00Z",
    "endTime": "2025-11-03T10:00:00Z",
    "filterPattern": "ERROR",
    "limit": 100
  }' \
  https://api.yourdomain.com/api/logs/query

# Send test email
curl -X POST \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "app": "app1",
    "from": "test@example.com",
    "subject": "Test",
    "body": {"text": "Test email"}
  }' \
  https://api.yourdomain.com/api/test/inbound
```

### JavaScript/TypeScript

```typescript
const API_URL = 'https://api.yourdomain.com';
const token = 'your-jwt-token';

// Fetch metrics
const response = await fetch(`${API_URL}/api/metrics/summary`, {
  headers: {
    'Authorization': `Bearer ${token}`,
  },
});
const metrics = await response.json();

// Send test email
const result = await fetch(`${API_URL}/api/test/inbound`, {
  method: 'POST',
  headers: {
    'Authorization': `Bearer ${token}`,
    'Content-Type': 'application/json',
  },
  body: JSON.stringify({
    app: 'app1',
    from: 'test@example.com',
    subject: 'Test Email',
    body: { text: 'This is a test' },
  }),
});
```

---

**Last Updated:** 2025-11-03
