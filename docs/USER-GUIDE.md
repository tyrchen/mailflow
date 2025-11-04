# Mailflow Dashboard User Guide

**Version:** 0.2.2
**Last Updated:** 2025-11-03

---

## Table of Contents

1. [Getting Started](#getting-started)
2. [Dashboard Overview](#dashboard-overview)
3. [Queue Management](#queue-management)
4. [Log Viewer](#log-viewer)
5. [Storage Browser](#storage-browser)
6. [Test Email](#test-email)
7. [Configuration](#configuration)
8. [Troubleshooting](#troubleshooting)

---

## Getting Started

### Accessing the Dashboard

1. Navigate to `https://dashboard.yourdomain.com`
2. You'll be prompted to enter a JWT token
3. Paste your JWT token and click "Login"
4. You'll be redirected to the dashboard home page

### Obtaining a JWT Token

Contact your system administrator to obtain a JWT token. The token must:
- Be signed with RS256 algorithm
- Include your email and name
- Include "Team Mailflow" in the `teams` claim
- Be valid (not expired)

---

## Dashboard Overview

### System Metrics

The dashboard home displays real-time metrics for the last 24 hours:

**Key Metrics:**
- **Total Emails:** Combined inbound and outbound emails processed
- **Processing Rate:** Emails processed per minute
- **Error Rate:** Percentage of failed emails (< 5% is healthy)
- **Active Queues:** Number of queues with pending messages

**DLQ Alert:**
- Orange warning appears if Dead Letter Queue has messages
- Click alert to view DLQ details in Queue Management

**Time-Series Chart:**
- Blue area: Inbound emails received
- Green area: Outbound emails sent
- Hover over chart for exact values
- Auto-refreshes every 30 seconds

### Navigation

**Sidebar Menu:**
- Dashboard - System overview
- Queues - SQS queue management
- Logs - CloudWatch logs viewer
- Storage - S3 storage browser
- Test Email - Send test emails
- Configuration - View system config

**Header:**
- Current page title
- Last updated timestamp
- User info and logout button

---

## Queue Management

### Viewing Queues

1. Click "Queues" in sidebar
2. All SQS queues are displayed with:
   - Queue name and type (inbound/outbound/dlq)
   - Message count
   - Messages in flight
   - Age of oldest message

### Filtering Queues

**By Type:**
- Select filter: Inbound, Outbound, or DLQ
- Only queues of that type are shown

**By Name:**
- Use search box to filter by queue name
- Case-insensitive search

### Inspecting Messages

1. Click on a queue row to view details
2. Queue information card shows:
   - Full queue URL
   - Message count and in-flight count
   - Queue type
3. Messages table shows up to 10 most recent messages:
   - Message ID
   - Preview (first 200 characters or parsed info)
   - Attributes
4. Click expand arrow to view full JSON message

**Note:** Messages are peeked, not deleted. They remain in the queue.

---

## Log Viewer

### Searching Logs

1. Click "Logs" in sidebar
2. Configure search parameters:
   - **Time Range:** Select start and end time (or use presets)
   - **Log Level:** ALL, ERROR, WARN, or INFO
   - **Search Pattern:** Message ID, correlation ID, or keywords

**Search Examples:**
```
message_id:msg-123          # Find logs for specific message
correlation_id:abc-def      # Find related logs
Failed to send              # Search error messages
```

3. Click "Query Logs" to search

### Viewing Results

- Results displayed in table with timestamp, level, and message
- Click expand arrow to view full JSON context
- Color-coded levels: Red (ERROR), Orange (WARN), Blue (INFO)

### Exporting Logs

1. Run a log query
2. Click "Export JSON" button (top-right)
3. JSON file downloads with timestamp in filename
4. Max 100 entries per export (increase limit if needed)

### From Test Email

1. Send a test email from Test Email page
2. Click "View Logs" in test history
3. Logs page opens with message ID pre-filled
4. Logs automatically queried

---

## Storage Browser

### Viewing Statistics

1. Click "Storage" in sidebar
2. Statistics cards show:
   - **Total Objects:** Number of files in bucket
   - **Total Size:** Storage used in GB
   - **Oldest Object:** Date of oldest file
   - **Newest Object:** Date of newest file

### Content Type Breakdown

**Pie Chart:**
- Visual representation of storage by file type
- Hover over segments for exact counts and sizes
- Legend shows all content types

**Breakdown Table:**
- Detailed list of content types
- Count of files per type
- Total size per type in MB

### Browsing Objects

**Recent Objects Table:**
- Shows 20 most recently uploaded files
- Columns: Key, Size, Last Modified
- Pagination available for large buckets

**Downloading Files:**
1. Find object in table
2. Click "Download" button
3. Opens presigned URL in new tab
4. File downloads automatically
5. Presigned URL valid for 7 days

### Multiple Buckets

If multiple S3 buckets exist:
- Use dropdown (top-right) to select bucket
- All stats, charts, and objects update accordingly

---

## Test Email

### Sending Inbound Test Email

1. Click "Test Email" in sidebar
2. Select "Inbound Test" tab
3. Fill out form:
   - **App:** Select destination app (app1, app2, etc.)
   - **From:** Enter sender email address
   - **Subject:** Email subject line
   - **Body:** Choose Text or HTML tab
     - **Text:** Plain text email body
     - **HTML:** HTML email body (optional)
   - **Attachments:** Click "Upload Attachment" to add files

4. Click "Send Test Email"
5. Success message appears with message ID
6. Test added to history table below

### Sending Outbound Test Email

1. Select "Outbound Test" tab
2. Fill out form:
   - **From App:** Select sender app
   - **To:** Enter recipient email address
   - **Subject:** Email subject line
   - **Body:** Text/HTML tabs (same as inbound)

3. Click "Queue Test Email"
4. Email queued to SQS outbound queue
5. Test added to history

### Attachments

**Uploading:**
- Click "Upload Attachment" button
- Select file(s) from computer
- Multiple files supported

**Size Limit:**
- Max 10 MB total across all attachments
- Current size shown below upload button
- Upload blocked if limit exceeded

**Removing:**
- Click X icon on uploaded file to remove
- Size counter updates automatically

### Test History

**Table shows:**
- Timestamp of test
- Type (inbound/outbound)
- Recipient address
- Status (success/failed)
- Message ID

**Actions:**
- Click "View Logs" to see processing logs
- Navigates to Logs page with auto-search

---

## Configuration

### Viewing System Config

1. Click "Configuration" in sidebar
2. Read-only view of current settings:
   - **Version:** System version number
   - **Source:** Where config is loaded from
   - **Routing:** App to queue mappings
   - **Security:** SPF/DKIM/DMARC settings
   - **Attachments:** Bucket and size limits

**Note:** Configuration is read-only. To change settings, update infrastructure code and redeploy.

---

## Troubleshooting

### Common Issues

#### "Unauthorized" Error

**Problem:** JWT token rejected

**Solutions:**
1. Check token hasn't expired
2. Verify token includes "Team Mailflow" in teams claim
3. Ensure issuer matches server configuration
4. Request new token from administrator

#### DLQ Messages Appearing

**Problem:** Messages in Dead Letter Queue

**Investigation:**
1. Note DLQ message count from dashboard
2. Navigate to Queues page
3. Filter to DLQ queues
4. Inspect messages to find failure reason
5. Search logs for message ID
6. Review error details in logs

**Common Causes:**
- Invalid email format
- Missing required headers
- AWS service errors
- Configuration issues

#### Logs Not Appearing

**Problem:** Log search returns no results

**Solutions:**
1. Verify time range includes expected logs
2. Check log level filter (use ALL)
3. Simplify search pattern
4. Increase time range
5. Verify Lambda function is logging correctly

#### Slow Dashboard Performance

**Problem:** Dashboard loads slowly

**Causes:**
- Large number of queue messages
- Many S3 objects
- Complex log queries

**Solutions:**
1. Use specific time ranges for logs
2. Filter queues by type
3. Limit object listing with prefix
4. Clear browser cache
5. Check network connection

#### Storage Breakdown Not Showing

**Problem:** Pie chart or table empty

**Cause:** No objects in bucket or wrong bucket selected

**Solutions:**
1. Verify bucket contains objects
2. Switch buckets using dropdown
3. Check S3 permissions
4. Reload page

---

## Best Practices

### Performance

1. **Use specific time ranges** - Avoid querying all logs
2. **Filter before searching** - Use log level filters
3. **Export large results** - Download JSON for analysis
4. **Pagination** - Use pagination for large tables

### Security

1. **Protect JWT tokens** - Never share or commit to code
2. **Logout when done** - Click logout in header
3. **Use HTTPS only** - Never use HTTP
4. **Report suspicious activity** - Contact admin immediately

### Monitoring

1. **Check dashboard daily** - Review metrics and DLQ count
2. **Set up external alerts** - Use CloudWatch alarms
3. **Monitor error rates** - Investigate spikes immediately
4. **Review test history** - Verify successful processing

### Troubleshooting Workflow

1. **Start at Dashboard** - Check overall health
2. **Check DLQ** - Look for failed messages
3. **Inspect Queues** - Verify message counts
4. **Search Logs** - Find error details
5. **Send Test Email** - Reproduce issues
6. **Export Logs** - Share with team for analysis

---

## Keyboard Shortcuts

- **Refresh:** F5 or Ctrl/Cmd+R
- **Logout:** No shortcut (use button)
- **Search:** Focus search inputs with Tab
- **Navigate:** Use browser back/forward

---

## Support

### Getting Help

1. **Documentation:** Read this guide and API docs
2. **Troubleshooting:** Follow troubleshooting section
3. **Team:** Contact team members for assistance
4. **Admin:** Contact system administrator for:
   - JWT token issues
   - Permission errors
   - Infrastructure problems

### Reporting Issues

When reporting issues, include:
1. Screenshot of error
2. JWT token user (email/sub, not full token)
3. Steps to reproduce
4. Expected vs actual behavior
5. Exported logs if relevant

---

## Appendix

### Glossary

- **DLQ:** Dead Letter Queue - holds failed messages
- **JWT:** JSON Web Token - authentication credential
- **SQS:** Simple Queue Service - AWS message queue
- **S3:** Simple Storage Service - AWS object storage
- **CloudWatch:** AWS monitoring and logging service
- **Presigned URL:** Temporary URL for downloading S3 objects
- **Message ID:** Unique identifier for email messages
- **Correlation ID:** Links related log entries

### Time Formats

All times displayed in:
- **ISO 8601 format:** `YYYY-MM-DDTHH:mm:ssZ`
- **Local timezone:** Converted from UTC
- **Relative time:** "X hours ago" in some views

### File Size Units

- **Bytes (B):** Base unit
- **Kilobytes (KB):** 1,024 bytes
- **Megabytes (MB):** 1,024 KB
- **Gigabytes (GB):** 1,024 MB

---

**Need more help?** Contact your system administrator or team lead.

**Version:** 0.2.2
**Last Updated:** 2025-11-03
