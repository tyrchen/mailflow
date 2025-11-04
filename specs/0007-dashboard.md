# Product Requirements Document: Mailflow Admin Dashboard

**Version:** 1.0
**Date:** 2025-11-03
**Status:** Draft

---

## 1. Executive Summary

The Mailflow Admin Dashboard is a web-based administrative interface that enables system administrators to monitor, manage, and troubleshoot the Mailflow email dispatching system. The dashboard provides real-time visibility into system health, email processing metrics, queue status, and diagnostic capabilities including log inspection and test email functionality.

This PRD outlines the requirements for a multi-crate architecture with a dedicated API backend (Rust/Axum) and a modern web frontend (React/Refine/Ant Design), secured with JWT authentication.

---

## 2. Goals and Objectives

### 2.1 Primary Goals

1. **System Observability**: Provide real-time visibility into system health, metrics, and operational status
2. **Operational Efficiency**: Enable quick diagnosis and troubleshooting of email processing issues
3. **Testing Capability**: Allow administrators to verify system functionality through test emails
4. **Resource Management**: Monitor and inspect SQS queues, S3 storage, and DynamoDB tables
5. **Security**: Protect admin access with JWT-based authentication

### 2.2 Non-Goals

- **Monitoring/Alerting**: External monitoring systems (Datadog, CloudWatch Alarms) handle alerting
- **Configuration Management**: Config changes remain in code/IaC (Pulumi)
- **User Management**: Single admin JWT, no multi-user RBAC in v1
- **Email Content Editing**: Dashboard is read-only for email content
- **Real-time WebSocket Updates**: Polling-based updates sufficient for v1

---

## 3. Architecture Overview

### 3.1 Multi-Crate Structure

```
mailflow/
├── crates/
│   ├── mailflow-core/          # Shared core library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── models/         # Shared data models
│   │       ├── services/       # AWS service traits
│   │       └── utils/          # Common utilities
│   │
│   ├── mailflow-worker/        # Email processing Lambda
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── handlers/       # SES, inbound, outbound handlers
│   │       └── email/          # Email parsing, composition
│   │
│   └── mailflow-api/           # Dashboard API Lambda
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── api/            # REST API routes
│           ├── auth/           # JWT authentication
│           └── services/       # Dashboard-specific services
│
├── dashboard/                   # React frontend
│   ├── package.json
│   ├── vite.config.ts
│   └── src/
│       ├── pages/
│       ├── components/
│       └── providers/
│
├── infra/                       # Pulumi IaC
│   └── src/
│       ├── index.ts
│       ├── lambda.ts
│       ├── dashboard.ts        # NEW: Dashboard infra
│       └── api-gateway.ts      # NEW: API Gateway
│
└── Cargo.toml                   # Workspace root
```

### 3.2 Component Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                         User Browser                             │
└──────────────────────────┬──────────────────────────────────────┘
                           │ HTTPS
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│  CloudFront CDN (dashboard.yourdomain.com)                       │
│  ├─── /          → S3 Static Website (React SPA)                │
│  └─── /api/*     → API Gateway                                  │
└──────────────────────────┬──────────────────────────────────────┘
                           │
         ┌─────────────────┴─────────────────┐
         │                                   │
         ▼                                   ▼
┌────────────────┐              ┌──────────────────────┐
│  S3 Bucket     │              │  API Gateway         │
│  (dashboard)   │              │  (REST API)          │
│                │              │  + JWT Authorizer    │
│  index.html    │              └──────┬───────────────┘
│  assets/       │                     │
└────────────────┘                     ▼
                            ┌──────────────────────────┐
                            │  Lambda: mailflow-api    │
                            │  (Rust + Axum)           │
                            │                          │
                            │  Routes:                 │
                            │  GET  /health            │
                            │  GET  /metrics           │
                            │  GET  /queues            │
                            │  GET  /queues/:name/msgs │
                            │  GET  /logs              │
                            │  POST /test-email        │
                            │  GET  /storage/stats     │
                            └──────┬───────────────────┘
                                   │
         ┌─────────────────────────┼─────────────────────┐
         │                         │                     │
         ▼                         ▼                     ▼
┌─────────────────┐   ┌─────────────────────┐   ┌──────────────┐
│  CloudWatch     │   │  SQS Queues         │   │  S3 Buckets  │
│  - Logs         │   │  - mailflow-*       │   │  - Raw emails│
│  - Metrics      │   │  - DLQ              │   │  - Attachments│
└─────────────────┘   └─────────────────────┘   └──────────────┘
                                   │
                                   ▼
                         ┌─────────────────────┐
                         │  DynamoDB Tables    │
                         │  - Config           │
                         │  - Idempotency      │
                         └─────────────────────┘
```

### 3.3 Authentication Flow

```
┌──────────┐                ┌──────────────┐                ┌────────────┐
│ Browser  │                │ API Gateway  │                │ Lambda API │
└─────┬────┘                └──────┬───────┘                └─────┬──────┘
      │                            │                              │
      │ 1. Request with JWT        │                              │
      │ Authorization: Bearer xxx  │                              │
      ├───────────────────────────►│                              │
      │                            │                              │
      │                            │ 2. JWT Authorizer            │
      │                            │ (Verify with JWKS)           │
      │                            │                              │
      │                            │ 3. If valid, invoke Lambda   │
      │                            ├─────────────────────────────►│
      │                            │                              │
      │                            │                              │
      │                            │ 4. Response                  │
      │                            │◄─────────────────────────────┤
      │                            │                              │
      │ 5. JSON Response           │                              │
      │◄───────────────────────────┤                              │
      │                            │                              │
```

**JWT Claims:**

```json
{
  "email": "xxx@example.com",
  "name": "John Doe",
  "sub": "00u5hx31ndlEVT55l358",
  "teams": [
    "Team Mailflow", # must validate if teams contains "Team Mailflow" (case insensitive)
    ...
  ],
  "resources": [
    {
      "host": "*",
      "method": "*",
      "path": "*"
    }
  ],
  "aud": "yyy", # doesn't matter
  "exp": 1762812354,
  "iat": 1762207554,
  "iss": "zzz" # must validate if it is the same as what is passed by envar in lambda function
}
```

---

## 4. Functional Requirements

### 4.1 Dashboard Pages

#### 4.1.1 Overview Page (Dashboard Home)

**FR-D1.1**: System MUST display a dashboard overview with:

- System health status (Healthy / Degraded / Down)
- Total emails processed today/this week/this month
- Current processing rate (emails/minute)
- Error rate (percentage)
- Active queue count
- DLQ message count (with alert indicator if > 0)

**FR-D1.2**: System MUST display real-time metrics charts:

- Inbound emails received (last 24 hours, time series)
- Outbound emails sent (last 24 hours, time series)
- Processing time distribution (p50, p95, p99)
- Error rate trend (last 24 hours)
- Queue depth over time

**FR-D1.3**: Dashboard MUST refresh metrics automatically every 30 seconds

#### 4.1.2 Queues Page

**FR-D2.1**: System MUST list all SQS queues with:

- Queue name
- Approximate message count
- Messages in flight
- DLQ message count (if applicable)
- Last activity timestamp
- Age of oldest message

**FR-D2.2**: System MUST allow queue inspection:

- Click queue to view messages (up to 10 most recent)
- Display message attributes:
  - Message ID
  - Timestamp
  - Body preview (first 200 chars)
  - Attributes (correlationId, source, etc.)
- Action: View full message JSON

**FR-D2.3**: System MUST support queue filtering:

- Filter by app name (e.g., show only `mailflow-app1`)
- Filter by queue type (inbound, outbound, DLQ)
- Search by queue name

**FR-D2.4**: System MUST display queue metrics:

- Message rate (msgs/min)
- Average processing time
- Redrive policy (if configured)

#### 4.1.3 Logs Page

**FR-D3.1**: System MUST fetch CloudWatch logs with:

- Time range selector (last 1h, 6h, 24h, 7d, custom)
- Log group filter (inbound, outbound, router)
- Log level filter (ERROR, WARN, INFO, DEBUG)
- Search by message ID or correlation ID

**FR-D3.2**: System MUST display logs in table format:

- Timestamp
- Level (color-coded badge)
- Message (truncated with expand option)
- Context (handler, function, message_id)
- Full log JSON (collapsible)

**FR-D3.3**: System MUST support log export:

- Download filtered logs as JSON
- Max 10,000 log entries per export

**FR-D3.4**: System MUST highlight important log patterns:

- Errors (red highlight)
- PII redaction indicators (show `***@domain.com` clearly)
- Security events (SPF/DKIM failures)

#### 4.1.4 Storage Page

**FR-D4.1**: System MUST display S3 bucket statistics:

- Bucket name
- Total objects
- Total size (GB)
- Oldest object age
- Newest object age

**FR-D4.2**: System MUST show storage breakdown:

- Raw emails storage (count, size)
- Attachments storage (count, size, breakdown by content type)
- Lifecycle policy status

**FR-D4.3**: System MUST display recent objects:

- List 20 most recent uploads
- Show: key, size, last modified, content type
- Action: Generate presigned URL for download

**FR-D4.4**: System MUST show storage trends:

- Daily upload count (chart)
- Daily storage growth (chart)
- Storage by email domain (pie chart)

#### 4.1.5 Test Email Page

**FR-D5.1**: System MUST provide test email form:

- **Send Inbound Test**:
  - To: App selector (dropdown of configured apps)
  - From: Email address input
  - Subject: Text input
  - Body: Text/HTML tabs
  - Attachments: File upload (max 10 MB total)
  - Action: Send via SES to app address

- **Send Outbound Test**:
  - From: App selector (dropdown)
  - To: Email address input
  - Subject: Text input
  - Body: Text/HTML tabs
  - Action: Send to outbound queue

**FR-D5.2**: System MUST validate test email inputs:

- Valid email addresses
- Subject not empty
- Body not empty
- Total size < 10 MB

**FR-D5.3**: System MUST display test email results:

- Success: Show message ID, queue URL, timestamp
- Failure: Show error message with details
- Link to logs for this test email

**FR-D5.4**: System MUST track recent test emails:

- List last 20 test emails sent
- Show: timestamp, type, recipient, status, message ID
- Action: View details, view logs

#### 4.1.6 Configuration Page (Read-Only)

**FR-D6.1**: System MUST display current configuration:

- Routing rules (app name → queue URL)
- Security settings (SPF/DKIM/DMARC requirements)
- Attachment settings (max size, allowed types, blocked types)
- Retention policies (raw emails, attachments, logs)

**FR-D6.2**: System MUST indicate configuration source:

- Environment variables
- DynamoDB config table
- Default values

**FR-D6.3**: Configuration MUST be read-only:

- No edit capability in dashboard
- Display note: "To change config, update Pulumi code and redeploy"

---

### 4.2 API Endpoints (mailflow-api)

#### 4.2.1 Health & Status

**API-1**: `GET /api/health`

- **Auth**: None (public health check)
- **Response**:

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

**API-2**: `GET /api/metrics/summary`

- **Auth**: Required
- **Response**:

```json
{
  "period": "24h",
  "inbound": {
    "total": 1234,
    "rate": 0.85,
    "errorRate": 0.02
  },
  "outbound": {
    "total": 1189,
    "rate": 0.82,
    "errorRate": 0.01
  },
  "processing": {
    "p50": 123,
    "p95": 456,
    "p99": 789
  },
  "queues": {
    "active": 5,
    "dlqMessages": 3
  }
}
```

**API-3**: `GET /api/metrics/timeseries?metric=inbound_received&period=24h&interval=1h`

- **Auth**: Required
- **Query Params**:
  - `metric`: inbound_received | outbound_sent | error_rate | processing_time
  - `period`: 1h | 6h | 24h | 7d | 30d
  - `interval`: 1m | 5m | 1h | 1d
- **Response**:

```json
{
  "metric": "inbound_received",
  "unit": "count",
  "datapoints": [
    {"timestamp": "2025-11-03T09:00:00Z", "value": 45},
    {"timestamp": "2025-11-03T10:00:00Z", "value": 52}
  ]
}
```

#### 4.2.2 Queue Management

**API-4**: `GET /api/queues`

- **Auth**: Required
- **Response**:

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

**API-5**: `GET /api/queues/{queueName}/messages?limit=10`

- **Auth**: Required
- **Response**:

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

#### 4.2.3 Logs

**API-6**: `POST /api/logs/query`

- **Auth**: Required
- **Request Body**:

```json
{
  "logGroup": "/aws/lambda/mailflow-dev",
  "startTime": "2025-11-03T09:00:00Z",
  "endTime": "2025-11-03T10:00:00Z",
  "filterPattern": "ERROR",
  "limit": 100
}
```

- **Response**:

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

#### 4.2.4 Storage

**API-7**: `GET /api/storage/stats`

- **Auth**: Required
- **Response**:

```json
{
  "buckets": [
    {
      "name": "mailflow-raw-emails-dev",
      "objectCount": 1234,
      "totalSizeBytes": 1073741824,
      "oldestObject": "2025-10-27T10:00:00Z",
      "newestObject": "2025-11-03T10:00:00Z"
    }
  ]
}
```

**API-8**: `GET /api/storage/{bucket}/objects?limit=20&prefix=attachments/`

- **Auth**: Required
- **Response**:

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

#### 4.2.5 Test Email

**API-9**: `POST /api/test/inbound`

- **Auth**: Required
- **Request Body**:

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

- **Response**:

```json
{
  "success": true,
  "messageId": "test-msg-123",
  "sesMessageId": "0000....-000",
  "queueUrl": "https://sqs.us-east-1.amazonaws.com/123/mailflow-app1",
  "timestamp": "2025-11-03T10:00:00Z"
}
```

**API-10**: `POST /api/test/outbound`

- **Auth**: Required
- **Request Body**:

```json
{
  "from": "app1",
  "to": "recipient@example.com",
  "subject": "Test Email",
  "body": {
    "text": "This is a test"
  }
}
```

- **Response**: Same as API-9

**API-11**: `GET /api/test/history?limit=20`

- **Auth**: Required
- **Response**:

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

#### 4.2.6 Configuration

**API-12**: `GET /api/config`

- **Auth**: Required
- **Response**:

```json
{
  "version": "1.0",
  "source": "environment",
  "routing": {
    "app1": {
      "queueUrl": "https://...",
      "enabled": true
    }
  },
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

### 4.3 Frontend Requirements

#### 4.3.1 Technology Stack

**FR-F1**: Frontend MUST use:

- **Framework**: React 18+ with TypeScript
- **Admin Framework**: Refine 4.x (<https://refine.dev>)
- **UI Library**: Ant Design 5.x
- **Styling**: Tailwind CSS 3.x
- **Build Tool**: Vite 5.x
- **State Management**: Refine's built-in data provider
- **API Client**: Axios
- **Charts**: Recharts or Ant Design Charts
- **Auth**: Refine Auth Provider with JWT

#### 4.3.2 Page Layout

**FR-F2**: Dashboard MUST have consistent layout:

- **Header**: Logo, page title, user info, logout button
- **Sidebar**: Navigation menu (collapsible)
  - Dashboard
  - Queues
  - Logs
  - Storage
  - Test Email
  - Configuration
- **Content Area**: Page-specific content
- **Footer**: Version, build timestamp, links

#### 4.3.3 Responsive Design

**FR-F3**: Dashboard MUST be responsive:

- Desktop (≥1280px): Full sidebar, 3-column layout
- Tablet (768px-1279px): Collapsible sidebar, 2-column layout
- Mobile (< 768px): Hidden sidebar (menu icon), 1-column layout

#### 4.3.4 Data Refresh

**FR-F4**: Dashboard MUST support data refresh:

- Auto-refresh every 30 seconds (configurable)
- Manual refresh button on each page
- Loading indicators during fetch
- Error handling with retry option

---

## 5. Non-Functional Requirements

### 5.1 Performance

**NFR-P1**: API response time MUST be < 500ms (p95) for all endpoints except logs
**NFR-P2**: Logs query MUST return results within 5 seconds
**NFR-P3**: Dashboard initial load MUST be < 2 seconds
**NFR-P4**: API MUST support at least 10 concurrent requests
**NFR-P5**: Frontend bundle size MUST be < 2 MB (gzipped)

### 5.2 Security

**NFR-S1**: All API endpoints (except /health) MUST require JWT authentication
**NFR-S2**: JWT MUST be signed with RS256 algorithm
**NFR-S3**: JWT MUST expire after 24 hours
**NFR-S4**: API Gateway MUST validate JWT using JWKS before invoking Lambda
**NFR-S5**: API MUST NOT expose sensitive data in error messages
**NFR-S6**: API MUST redact PII in all responses (email addresses, subjects)
**NFR-S7**: CloudFront MUST enforce HTTPS only
**NFR-S8**: S3 bucket MUST have public access blocked (CloudFront OAI only)

### 5.3 Reliability

**NFR-R1**: API MUST have 99.9% uptime (excluding AWS outages)
**NFR-R2**: API MUST handle AWS service errors gracefully (retry with exponential backoff)
**NFR-R3**: Dashboard MUST display friendly error messages on API failures
**NFR-R4**: API MUST log all errors to CloudWatch for debugging

### 5.4 Scalability

**NFR-SC1**: API Lambda MUST scale to 10 concurrent executions
**NFR-SC2**: API MUST support pagination for large result sets (logs, queues)
**NFR-SC3**: CloudFront MUST cache static assets for 1 year
**NFR-SC4**: API Gateway MUST have rate limiting (100 req/min per IP)

### 5.5 Observability

**NFR-O1**: API MUST log all requests with:

- Request ID
- Endpoint
- Duration
- Status code
- User (from JWT sub claim)

**NFR-O2**: API MUST emit CloudWatch metrics:

- Request count (by endpoint)
- Error count (by endpoint)
- Response time (by endpoint)

**NFR-O3**: Dashboard MUST track client-side errors (Sentry or CloudWatch RUM)

---

## 6. Technical Design

### 6.1 Workspace Structure

**Cargo Workspace** (`Cargo.toml` at root):

```toml
[workspace]
members = [
    "crates/mailflow-core",
    "crates/mailflow-worker",
    "crates/mailflow-api",
]
resolver = "2"

[workspace.dependencies]
# Shared dependencies
aws-config = { version = "1.8", features = ["behavior-version-latest"] }
aws-sdk-s3 = "1.110"
aws-sdk-sqs = "1.88"
aws-sdk-dynamodb = "1.97"
tokio = { version = "1.48.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tracing = "0.1"
anyhow = "1.0"
thiserror = "2.0"
```

### 6.2 mailflow-core Crate

**Purpose**: Shared types, traits, and utilities

**Exports**:

- `models::*`: Email, InboundMessage, OutboundMessage, Config
- `services::*`: StorageService, QueueService, MetricsService traits
- `utils::*`: Logging, sanitization, validation utilities
- `error::MailflowError`

### 6.3 mailflow-worker Crate

**Purpose**: Email processing Lambda (existing functionality)

**Dependencies**:

- `mailflow-core`
- `lambda_runtime`
- `mail-parser`
- `lettre`

**Main**: Handles SES events, inbound/outbound processing

### 6.4 mailflow-api Crate

**Purpose**: Dashboard API Lambda

**Dependencies**:

- `mailflow-core`
- `lambda_runtime`
- `lambda_http` (for API Gateway integration)
- `axum = "0.7"` (web framework)
- `tower = "0.4"` (middleware)
- `jsonwebtoken = "9.3"` (JWT validation)

**Structure**:

```rust
// src/main.rs
#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_http::run(service_fn(handler)).await
}

async fn handler(event: Request) -> Result<Response<Body>, Error> {
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/metrics/summary", get(metrics_summary))
        .route("/queues", get(list_queues))
        .route("/queues/:name/messages", get(get_queue_messages))
        .route("/logs/query", post(query_logs))
        .route("/storage/stats", get(storage_stats))
        .route("/test/inbound", post(test_inbound))
        .layer(JwtAuthLayer::new(jwks_from_env()?));

    // Convert Lambda event to Axum request
    // Process with Axum
    // Convert Axum response to Lambda response
}
```

**JWT Validation**:

```rust
// src/auth/jwt.rs
pub struct JwtAuthLayer {
    jwks: JsonWebKeySet,
}

impl JwtAuthLayer {
    pub fn new(jwks_json: &str) -> Result<Self, Error> {
        let jwks: JsonWebKeySet = serde_json::from_str(jwks_json)?;
        Ok(Self { jwks })
    }

    pub async fn validate(&self, token: &str) -> Result<Claims, Error> {
        // 1. Decode JWT header to get kid
        // 2. Find matching key in JWKS
        // 3. Verify signature with RS256
        // 4. Validate claims (exp, iss, aud)
        // 5. Return Claims
    }
}
```

### 6.5 Dashboard Frontend

**Structure**:

```
dashboard/
├── package.json
├── vite.config.ts
├── tailwind.config.js
├── tsconfig.json
├── index.html
└── src/
    ├── main.tsx
    ├── App.tsx
    ├── pages/
    │   ├── dashboard/
    │   │   └── index.tsx          # Overview page
    │   ├── queues/
    │   │   ├── list.tsx            # Queue list
    │   │   └── show.tsx            # Queue details
    │   ├── logs/
    │   │   └── index.tsx           # Logs viewer
    │   ├── storage/
    │   │   └── index.tsx           # Storage stats
    │   ├── test/
    │   │   └── index.tsx           # Test email form
    │   └── config/
    │       └── index.tsx           # Config viewer
    ├── components/
    │   ├── charts/
    │   │   ├── MetricChart.tsx
    │   │   └── TimeSeriesChart.tsx
    │   ├── queue/
    │   │   └── MessageCard.tsx
    │   └── common/
    │       ├── LoadingSpinner.tsx
    │       └── ErrorBoundary.tsx
    ├── providers/
    │   ├── dataProvider.ts         # Refine data provider
    │   └── authProvider.ts         # Refine auth provider
    └── utils/
        ├── api.ts                  # Axios client with JWT
        └── constants.ts
```

**Data Provider** (Refine integration):

```typescript
// src/providers/dataProvider.ts
import { DataProvider } from "@refinedev/core";
import { apiClient } from "../utils/api";

export const dataProvider: DataProvider = {
  getList: async ({ resource, pagination, filters, sorters }) => {
    // Map resource to API endpoint
    // e.g., resource="queues" -> GET /api/queues
  },

  getOne: async ({ resource, id }) => {
    // GET /api/{resource}/{id}
  },

  create: async ({ resource, variables }) => {
    // POST /api/{resource}
    // Used for test emails
  },

  // ... other CRUD methods

  custom: async ({ url, method, payload }) => {
    // For custom endpoints like /metrics/summary
    return apiClient.request({ url, method, data: payload });
  },
};
```

**Auth Provider**:

```typescript
// src/providers/authProvider.ts
import { AuthProvider } from "@refinedev/core";

export const authProvider: AuthProvider = {
  login: async ({ token }) => {
    // Store JWT in localStorage
    localStorage.setItem("jwt", token);
    return { success: true };
  },

  logout: async () => {
    localStorage.removeItem("jwt");
    return { success: true };
  },

  check: async () => {
    const token = localStorage.getItem("jwt");
    if (!token) return { authenticated: false };

    // Decode and check expiration
    const payload = JSON.parse(atob(token.split('.')[1]));
    if (payload.exp * 1000 < Date.now()) {
      return { authenticated: false };
    }

    return { authenticated: true };
  },

  getIdentity: async () => {
    const token = localStorage.getItem("jwt");
    const payload = JSON.parse(atob(token.split('.')[1]));
    return { id: payload.sub, name: payload.sub };
  },
};
```

### 6.6 Infrastructure (Pulumi)

**New Files**:

**infra/src/dashboard.ts**:

```typescript
import * as pulumi from "@pulumi/pulumi";
import * as aws from "@pulumi/aws";

export function createDashboard(config: DashboardConfig) {
  // 1. S3 Bucket for dashboard static assets
  const bucket = new aws.s3.Bucket("mailflow-dashboard", {
    website: {
      indexDocument: "index.html",
      errorDocument: "index.html", // SPA routing
    },
  });

  // 2. CloudFront OAI
  const oai = new aws.cloudfront.OriginAccessIdentity("dashboard-oai");

  // 3. Bucket policy for CloudFront
  const bucketPolicy = new aws.s3.BucketPolicy("dashboard-policy", {
    bucket: bucket.id,
    policy: {
      Version: "2012-10-17",
      Statement: [{
        Effect: "Allow",
        Principal: { AWS: oai.iamArn },
        Action: "s3:GetObject",
        Resource: pulumi.interpolate`${bucket.arn}/*`,
      }],
    },
  });

  // 4. CloudFront distribution
  const cdn = new aws.cloudfront.Distribution("dashboard-cdn", {
    origins: [
      {
        domainName: bucket.bucketRegionalDomainName,
        originId: "s3-dashboard",
        s3OriginConfig: { originAccessIdentity: oai.cloudfrontAccessIdentityPath },
      },
      {
        domainName: apiGateway.apiEndpoint,
        originId: "api-gateway",
        customOriginConfig: {
          httpPort: 80,
          httpsPort: 443,
          originProtocolPolicy: "https-only",
        },
      },
    ],
    defaultCacheBehavior: {
      targetOriginId: "s3-dashboard",
      viewerProtocolPolicy: "redirect-to-https",
      allowedMethods: ["GET", "HEAD"],
      cachedMethods: ["GET", "HEAD"],
      forwardedValues: {
        queryString: false,
        cookies: { forward: "none" },
      },
    },
    orderedCacheBehaviors: [{
      pathPattern: "/api/*",
      targetOriginId: "api-gateway",
      viewerProtocolPolicy: "https-only",
      allowedMethods: ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"],
      cachedMethods: ["GET", "HEAD"],
      forwardedValues: {
        queryString: true,
        headers: ["Authorization"],
        cookies: { forward: "all" },
      },
      minTtl: 0,
      defaultTtl: 0,
      maxTtl: 0,
    }],
    restrictions: { geoRestriction: { restrictionType: "none" } },
    viewerCertificate: { cloudfrontDefaultCertificate: true },
  });

  return { bucket, cdn };
}
```

**infra/src/api-gateway.ts**:

```typescript
import * as pulumi from "@pulumi/pulumi";
import * as aws from "@pulumi/aws";

export function createApiGateway(apiLambda: aws.lambda.Function, jwksJson: string) {
  // 1. API Gateway REST API
  const api = new aws.apigateway.RestApi("mailflow-api", {
    description: "Mailflow Dashboard API",
  });

  // 2. JWT Authorizer
  const authorizer = new aws.apigateway.Authorizer("jwt-authorizer", {
    restApi: api.id,
    type: "TOKEN",
    authorizerUri: pulumi.interpolate`arn:aws:apigateway:${region}:lambda:path/2015-03-31/functions/${jwtAuthorizerLambda.arn}/invocations`,
    identitySource: "method.request.header.Authorization",
    authorizerResultTtlInSeconds: 300,
  });

  // 3. Proxy resource for all API routes
  const proxyResource = new aws.apigateway.Resource("proxy", {
    restApi: api.id,
    parentId: api.rootResourceId,
    pathPart: "{proxy+}",
  });

  // 4. ANY method with authorization
  const proxyMethod = new aws.apigateway.Method("proxy-method", {
    restApi: api.id,
    resourceId: proxyResource.id,
    httpMethod: "ANY",
    authorization: "CUSTOM",
    authorizerId: authorizer.id,
  });

  // 5. Lambda integration
  const integration = new aws.apigateway.Integration("proxy-integration", {
    restApi: api.id,
    resourceId: proxyResource.id,
    httpMethod: proxyMethod.httpMethod,
    integrationHttpMethod: "POST",
    type: "AWS_PROXY",
    uri: apiLambda.invokeArn,
  });

  // 6. Deployment
  const deployment = new aws.apigateway.Deployment("api-deployment", {
    restApi: api.id,
    stageName: "prod",
  }, { dependsOn: [integration] });

  return { api, deployment };
}
```

---

## 7. Data Flow Examples

### 7.1 User Views Dashboard

```
1. Browser → CloudFront (dashboard.yourdomain.com)
2. CloudFront → S3 (serve index.html, app.js, app.css)
3. Browser loads React app
4. React app checks auth (JWT in localStorage)
5. If authenticated, fetch /api/metrics/summary
6. Browser → CloudFront → API Gateway → Lambda API
7. Lambda API validates JWT
8. Lambda API queries CloudWatch metrics
9. Lambda API returns JSON response
10. React app renders charts
```

### 7.2 User Sends Test Email

```
1. User fills test email form (app1, test subject, body)
2. Browser → POST /api/test/inbound (with JWT)
3. API Gateway validates JWT
4. Lambda API receives request
5. Lambda API composes test email
6. Lambda API sends email via SES to _app1@yourdomain.com
7. SES saves email to S3
8. SES triggers mailflow-worker Lambda
9. Worker processes email, routes to mailflow-app1 queue
10. Lambda API returns success response with message ID
11. React app displays success message
```

### 7.3 User Views Queue Messages

```
1. User clicks "Queues" in sidebar
2. React app → GET /api/queues
3. Lambda API calls SQS GetQueueAttributes for all queues
4. Lambda API returns list of queues with stats
5. React app renders queue table
6. User clicks "mailflow-app1" queue
7. React app → GET /api/queues/mailflow-app1/messages?limit=10
8. Lambda API calls SQS ReceiveMessage (visibility timeout = 0)
9. Lambda API returns messages (without deleting)
10. React app displays messages in table
```

---

## 8. Implementation Plan

### Phase 1: Core Infrastructure (Week 1)

**Tasks**:

1. ✅ Create Cargo workspace structure
   - Create `crates/` directory
   - Move existing code to `crates/mailflow-worker/`
   - Extract shared code to `crates/mailflow-core/`
   - Update `Cargo.toml` with workspace config

2. ✅ Create mailflow-api crate
   - Add `crates/mailflow-api/` with basic Axum setup
   - Implement JWT validation layer
   - Add health endpoint
   - Test with local lambda-runtime

3. ✅ Update Pulumi infrastructure
   - Create `infra/src/api-gateway.ts`
   - Create `infra/src/dashboard.ts`
   - Add API Gateway with JWT authorizer
   - Add S3 bucket for dashboard
   - Add CloudFront distribution

**Deliverables**:

- Working multi-crate structure
- Deployed API Lambda with /health endpoint
- API Gateway with JWT validation
- S3 + CloudFront setup (empty bucket)

---

### Phase 2: API Implementation (Week 2-3)

**Tasks**:

1. ✅ Implement metrics endpoints
   - `GET /api/metrics/summary`
   - `GET /api/metrics/timeseries`
   - Query CloudWatch metrics API
   - Aggregate and format data

2. ✅ Implement queue endpoints
   - `GET /api/queues`
   - `GET /api/queues/:name/messages`
   - Use SQS SDK to fetch queue attributes and messages

3. ✅ Implement logs endpoint
   - `POST /api/logs/query`
   - Use CloudWatch Logs Insights API
   - Support filtering and pagination

4. ✅ Implement storage endpoints
   - `GET /api/storage/stats`
   - `GET /api/storage/:bucket/objects`
   - Use S3 SDK to list objects and calculate stats

5. ✅ Implement test email endpoints
   - `POST /api/test/inbound`
   - `POST /api/test/outbound`
   - `GET /api/test/history`
   - Compose and send test emails

6. ✅ Implement config endpoint
   - `GET /api/config`
   - Load from environment or DynamoDB

**Deliverables**:

- All API endpoints functional
- Unit tests for each endpoint
- Integration tests with mocked AWS services
- API documentation

---

### Phase 3: Frontend Implementation (Week 4-5)

**Tasks**:

1. ✅ Setup React + Refine project
   - Initialize with Vite
   - Install dependencies (Refine, Ant Design, Tailwind)
   - Configure Refine with data/auth providers
   - Setup API client with JWT

2. ✅ Implement Overview page
   - System health status card
   - Metrics summary cards
   - Time series charts (Recharts)
   - Auto-refresh every 30s

3. ✅ Implement Queues page
   - Queue list table
   - Queue details modal
   - Message viewer
   - Filtering and search

4. ✅ Implement Logs page
   - Log query form (time range, filters)
   - Log table with expandable rows
   - Export to JSON button
   - Syntax highlighting for JSON

5. ✅ Implement Storage page
   - Bucket stats cards
   - Storage trend charts
   - Object list table
   - Presigned URL download

6. ✅ Implement Test Email page
   - Inbound/Outbound tabs
   - Email composition form
   - File upload for attachments
   - Test history table

7. ✅ Implement Config page
   - Read-only config viewer
   - Syntax-highlighted JSON
   - Config source indicator

**Deliverables**:

- Fully functional React dashboard
- Responsive design (desktop, tablet, mobile)
- All pages implemented
- Error handling and loading states

---

### Phase 4: Testing & Deployment (Week 6)

**Tasks**:

1. ✅ End-to-end testing
   - Test all API endpoints
   - Test JWT authentication
   - Test dashboard functionality
   - Test error scenarios

2. ✅ Performance optimization
   - Optimize frontend bundle size
   - Add CloudFront caching headers
   - Optimize API Lambda cold start
   - Add API response caching

3. ✅ Security audit
   - Review JWT validation
   - Test API authorization
   - Check for exposed secrets
   - Verify HTTPS enforcement

4. ✅ Documentation
   - API documentation (OpenAPI spec)
   - User guide for dashboard
   - Deployment guide
   - Troubleshooting guide

5. ✅ Production deployment
   - Deploy to prod environment
   - Configure custom domain
   - Setup CloudWatch alarms for API
   - Monitor for errors

**Deliverables**:

- Production-ready dashboard
- Complete documentation
- Monitoring and alerting configured

---

## 9. Success Metrics

### 9.1 Technical Metrics

- [ ] API p95 response time < 500ms
- [ ] Dashboard load time < 2 seconds
- [ ] Frontend bundle size < 2 MB gzipped
- [ ] API test coverage > 80%
- [ ] Zero security vulnerabilities (Cargo audit, npm audit)

### 9.2 Functional Metrics

- [ ] All 12 API endpoints working
- [ ] All 6 dashboard pages functional
- [ ] JWT authentication working
- [ ] Test email functionality working
- [ ] Metrics display accurately

### 9.3 User Metrics

- [ ] Admin can diagnose email processing issues in < 5 minutes
- [ ] Admin can send test email in < 1 minute
- [ ] Admin can view queue status in < 30 seconds
- [ ] Dashboard usable on mobile devices

---

## 10. Security Considerations

### 10.1 JWT Management

**Generating JWT**:

```bash
# Generate JWT using jose CLI or custom script
# Private key: infra/.jwks.json (RS256)
# Claims: sub=admin, iss=mailflow, aud=mailflow-dashboard, exp=24h

npm install -g jose-cli
jose jwt sign --iss mailflow --aud mailflow-dashboard --exp 24h \
  --key infra/.jwks.json > admin-token.jwt
```

**JWKS in Lambda**:

- Pulumi reads `infra/.jwks.json` (gitignored)
- Passes as environment variable to Lambda
- Lambda caches JWKS in memory
- No need for JWKS endpoint (single admin user)

### 10.2 API Security

- [ ] All endpoints require JWT (except /health)
- [ ] JWT validated on every request
- [ ] No sensitive data in error messages
- [ ] PII redacted in all responses
- [ ] Rate limiting at API Gateway (100 req/min)
- [ ] CORS configured (only dashboard.yourdomain.com)

### 10.3 Frontend Security

- [ ] JWT stored in localStorage (httpOnly cookies not possible with CloudFront)
- [ ] Auto-logout on JWT expiration
- [ ] XSS protection (React escaping, CSP headers)
- [ ] HTTPS only (CloudFront enforced)

---

## 11. Open Questions

1. **Multi-user support**: Should we support multiple admin users with different permissions in v2?
2. **Real-time updates**: Should we add WebSocket support for live metrics updates?
3. **Email content viewer**: Should we allow viewing full email HTML in dashboard?
4. **Queue management**: Should we allow purging queues or moving messages from dashboard?
5. **Alert configuration**: Should we allow configuring CloudWatch alarms from dashboard?
6. **Audit log**: Should we track all admin actions (test emails, config views) in DynamoDB?

---

## 12. Future Enhancements (Post-MVP)

### 12.1 Advanced Monitoring

- Custom metrics dashboards (drag-and-drop widgets)
- Anomaly detection (ML-based)
- Alerting configuration UI
- Incident management integration (PagerDuty, Slack)

### 12.2 Advanced Testing

- Email template library for test emails
- Bulk test email sending
- A/B testing support
- Load testing from dashboard

### 12.3 Management Features

- Queue management (purge, move messages)
- Routing rule editor
- Configuration management (edit config from UI)
- User management (RBAC, teams)

### 12.4 Analytics

- Email delivery analytics
- Bounce/complaint trends
- App usage statistics
- Cost analysis (Lambda, S3, SQS costs)

---

## 13. Appendices

### Appendix A: API Endpoint Summary

| Endpoint                       | Method | Auth     | Purpose                  |
|--------------------------------|--------|----------|--------------------------|
| `/api/health`                  | GET    | No       | Health check             |
| `/api/metrics/summary`         | GET    | Required | Metrics overview         |
| `/api/metrics/timeseries`      | GET    | Required | Time series metrics      |
| `/api/queues`                  | GET    | Required | List all queues          |
| `/api/queues/:name/messages`   | GET    | Required | Get queue messages       |
| `/api/logs/query`              | POST   | Required | Query CloudWatch logs    |
| `/api/storage/stats`           | GET    | Required | Storage statistics       |
| `/api/storage/:bucket/objects` | GET    | Required | List S3 objects          |
| `/api/test/inbound`            | POST   | Required | Send test inbound email  |
| `/api/test/outbound`           | POST   | Required | Send test outbound email |
| `/api/test/history`            | GET    | Required | Get test email history   |
| `/api/config`                  | GET    | Required | Get system configuration |

### Appendix B: Frontend Pages Summary

| Page          | Route      | Purpose                               |
|---------------|------------|---------------------------------------|
| Overview      | `/`        | System health and metrics overview    |
| Queues        | `/queues`  | Queue list and message inspection     |
| Logs          | `/logs`    | CloudWatch logs viewer                |
| Storage       | `/storage` | S3 storage statistics and objects     |
| Test Email    | `/test`    | Send test emails (inbound/outbound)   |
| Configuration | `/config`  | View system configuration (read-only) |

### Appendix C: Technology Stack Summary

| Component       | Technology                         |
|-----------------|------------------------------------|
| Backend API     | Rust, Axum, Lambda Runtime         |
| Frontend        | React, TypeScript, Vite            |
| Admin Framework | Refine 4.x                         |
| UI Library      | Ant Design 5.x                     |
| Styling         | Tailwind CSS 3.x                   |
| Charts          | Recharts                           |
| Auth            | JWT (RS256), jsonwebtoken          |
| Infrastructure  | Pulumi (TypeScript), AWS CDK equiv |
| Hosting         | S3 + CloudFront (static)           |
| API Gateway     | AWS API Gateway (REST)             |
| Compute         | AWS Lambda (Rust + Axum)           |

---

**Document Approval**

| Role             | Name | Date | Signature |
|------------------|------|------|-----------|
| Product Manager  |      |      |           |
| Engineering Lead |      |      |           |
| Security Lead    |      |      |           |
| DevOps Lead      |      |      |           |

---

**End of Document**
