# API Reference

HTTP API documentation for Kindly Dedup.

## Base URL

```
http://localhost:8080/api/v1
```

Change host/port with `--host` and `--port` flags when starting the server.

## Authentication

All requests require a valid license key in the header:

```http
Authorization: Bearer your-license-key-here
```

## Endpoints

### POST /api/v1/deduplicate

Submit documents for deduplication.

**Request**:

```http
POST /api/v1/deduplicate HTTP/1.1
Content-Type: application/json
Authorization: Bearer your-license-key

{
  "documents": [
    {"id": 1, "text": "First document content"},
    {"id": 2, "text": "Second document content"},
    {"id": 3, "text": "First document content"}
  ],
  "threshold": 0.85,
  "options": {
    "include_stats": true,
    "persistent": false
  }
}
```

**Response** (200 OK):

```json
{
  "clusters": [
    {
      "representative_id": 1,
      "duplicate_ids": [3],
      "similarity": 1.0,
      "size": 2
    },
    {
      "representative_id": 2,
      "duplicate_ids": [],
      "similarity": 0.0,
      "size": 1
    }
  ],
  "stats": {
    "total_documents": 3,
    "unique_documents": 2,
    "duplicate_documents": 1,
    "deduplication_ratio": 0.33,
    "processing_time_ms": 12,
    "throughput_docs_per_sec": 250
  },
  "request_id": "req_abc123xyz"
}
```

**Request Parameters**:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| documents | array | Yes | Array of document objects |
| documents[].id | integer | Yes | Unique document identifier |
| documents[].text | string | Yes | Document content |
| documents[].metadata | object | No | Custom metadata (preserved in output) |
| threshold | float | No | Similarity threshold 0.0-1.0 (default: 0.85) |
| options.include_stats | boolean | No | Include statistics (default: true) |
| options.persistent | boolean | No | Use persistent storage (default: false) |

**Response Fields**:

| Field | Type | Description |
|-------|------|-------------|
| clusters | array | Array of duplicate clusters |
| clusters[].representative_id | integer | Canonical document ID for cluster |
| clusters[].duplicate_ids | array | IDs of duplicate documents |
| clusters[].similarity | float | Average similarity within cluster |
| clusters[].size | integer | Total documents in cluster |
| stats | object | Processing statistics |
| request_id | string | Unique request identifier |

**Error Response** (400 Bad Request):

```json
{
  "error": "Invalid threshold value",
  "message": "Threshold must be between 0.0 and 1.0",
  "request_id": "req_abc123xyz"
}
```

### POST /api/v1/deduplicate/batch

Process large batches asynchronously.

**Request**:

```http
POST /api/v1/deduplicate/batch HTTP/1.1
Content-Type: application/json
Authorization: Bearer your-license-key

{
  "input_url": "s3://bucket/documents.jsonl",
  "output_url": "s3://bucket/results.json",
  "threshold": 0.85,
  "callback_url": "https://your-app.com/webhook"
}
```

**Response** (202 Accepted):

```json
{
  "job_id": "job_xyz789",
  "status": "pending",
  "created_at": "2025-11-25T10:30:00Z",
  "estimated_completion": "2025-11-25T10:35:00Z"
}
```

**Request Parameters**:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| input_url | string | Yes | URL to input file (S3, HTTP, file://) |
| output_url | string | Yes | URL to write results |
| threshold | float | No | Similarity threshold (default: 0.85) |
| callback_url | string | No | Webhook URL for completion notification |

### GET /api/v1/jobs/{job_id}

Check batch job status.

**Request**:

```http
GET /api/v1/jobs/job_xyz789 HTTP/1.1
Authorization: Bearer your-license-key
```

**Response** (200 OK):

```json
{
  "job_id": "job_xyz789",
  "status": "completed",
  "progress": 100,
  "created_at": "2025-11-25T10:30:00Z",
  "completed_at": "2025-11-25T10:34:23Z",
  "result": {
    "total_documents": 1000000,
    "unique_documents": 850000,
    "duplicate_documents": 150000,
    "output_url": "s3://bucket/results.json"
  }
}
```

**Status Values**:
- `pending` - Job queued, not started
- `running` - Processing in progress
- `completed` - Successfully finished
- `failed` - Error occurred
- `cancelled` - Cancelled by user

### DELETE /api/v1/jobs/{job_id}

Cancel a running batch job.

**Request**:

```http
DELETE /api/v1/jobs/job_xyz789 HTTP/1.1
Authorization: Bearer your-license-key
```

**Response** (200 OK):

```json
{
  "job_id": "job_xyz789",
  "status": "cancelled",
  "message": "Job cancelled successfully"
}
```

### GET /health

Health check endpoint (no authentication required).

**Request**:

```http
GET /health HTTP/1.1
```

**Response** (200 OK):

```json
{
  "status": "healthy",
  "uptime_seconds": 86400,
  "version": "3.0.0",
  "timestamp": "2025-11-25T10:30:00Z"
}
```

### GET /api/v1/stats

Get service statistics.

**Request**:

```http
GET /api/v1/stats HTTP/1.1
Authorization: Bearer your-license-key
```

**Response** (200 OK):

```json
{
  "total_requests": 12345,
  "total_documents_processed": 5000000,
  "average_throughput_docs_per_sec": 60000,
  "uptime_seconds": 86400,
  "memory_usage_mb": 3500,
  "storage_usage_gb": 25
}
```

## Error Codes

| HTTP Code | Error | Description |
|-----------|-------|-------------|
| 400 | Bad Request | Invalid request parameters |
| 401 | Unauthorized | Missing or invalid license key |
| 403 | Forbidden | License expired or quota exceeded |
| 413 | Payload Too Large | Request exceeds size limit |
| 429 | Too Many Requests | Rate limit exceeded |
| 500 | Internal Server Error | Server-side processing error |
| 503 | Service Unavailable | Server overloaded or maintenance |

## Rate Limits

Default rate limits (configurable per license):

- **Standard License**: 100 requests/minute
- **Professional License**: 1000 requests/minute
- **Enterprise License**: Unlimited

Rate limit headers included in all responses:

```http
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1732531800
```

## Webhooks

Batch job completion notifications sent to `callback_url`:

**Webhook Payload**:

```json
{
  "event": "job.completed",
  "job_id": "job_xyz789",
  "status": "completed",
  "timestamp": "2025-11-25T10:34:23Z",
  "result": {
    "total_documents": 1000000,
    "unique_documents": 850000,
    "output_url": "s3://bucket/results.json"
  }
}
```

**Event Types**:
- `job.completed` - Job finished successfully
- `job.failed` - Job encountered error
- `job.progress` - Progress update (every 10%)

## Client Libraries

Official SDKs available:

- **Python**: `pip install kindly-dedup`
- **JavaScript/Node.js**: `npm install kindly-dedup`
- **Go**: `go get github.com/kindly-ai/kindly-dedup-go`
- **Java**: Maven Central (group: ai.kindly, artifact: kindly-dedup)

### Python Example

```python
from kindly_dedup import Client

client = Client(api_key="your-license-key", base_url="http://localhost:8080")

# Synchronous deduplication
result = client.deduplicate(
    documents=[
        {"id": 1, "text": "First document"},
        {"id": 2, "text": "Second document"}
    ],
    threshold=0.85
)

print(f"Found {len(result.clusters)} clusters")
print(f"Deduplication ratio: {result.stats.deduplication_ratio:.2%}")

# Batch processing
job = client.deduplicate_batch(
    input_url="s3://bucket/data.jsonl",
    output_url="s3://bucket/results.json",
    callback_url="https://myapp.com/webhook"
)

# Poll for completion
status = client.get_job_status(job.job_id)
while status.status == "running":
    time.sleep(10)
    status = client.get_job_status(job.job_id)

print(f"Job completed: {status.result.output_url}")
```

### JavaScript Example

```javascript
const KindlyDedup = require('kindly-dedup');

const client = new KindlyDedup({
  apiKey: 'your-license-key',
  baseUrl: 'http://localhost:8080'
});

// Async deduplication
const result = await client.deduplicate({
  documents: [
    { id: 1, text: 'First document' },
    { id: 2, text: 'Second document' }
  ],
  threshold: 0.85
});

console.log(`Found ${result.clusters.length} clusters`);
console.log(`Deduplication ratio: ${result.stats.deduplication_ratio}`);
```

## Performance Considerations

- **Request Size**: Maximum 10 MB per request (10,000 documents typical)
- **Batch Processing**: Use `/deduplicate/batch` for > 100K documents
- **Persistent Mode**: Recommended for datasets > 1M documents
- **Caching**: Results cached for 1 hour (same threshold + documents)

## Support

For API support and integration assistance:
- Email: api-support@kindly.ai
- Documentation: https://docs.kindly.ai/api
- Status Page: https://status.kindly.ai
