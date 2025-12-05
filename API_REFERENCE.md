# API Reference

Technical reference for the kindly-debugger API protocol and message formats.

## Protocol Overview

kindly-debugger implements the [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) specification version **2024-11-05**.

Communication uses JSON-RPC 2.0 over HTTPS.

## Connection

### Endpoint

```
https://api.kindly.services/mcp
```

### Authentication

All requests must include an API key in the Authorization header:

```
Authorization: Bearer YOUR_API_KEY
```

See [Authentication](AUTHENTICATION.md) for details.

## Request Format

All requests follow JSON-RPC 2.0 format:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "method_name",
  "params": {
    "key": "value"
  }
}
```

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `jsonrpc` | string | Yes | Must be "2.0" |
| `id` | integer/string | Yes | Request identifier (returned in response) |
| `method` | string | Yes | Method to invoke |
| `params` | object | Depends | Method parameters |

## Response Format

### Success Response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Response text here"
      }
    ]
  }
}
```

### Error Response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32600,
    "message": "Invalid Request",
    "data": {
      "details": "Additional error information"
    }
  }
}
```

## MCP Methods

### tools/list

List all available tools.

**Request**

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/list",
  "params": {}
}
```

**Response**

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "tools": [
      {
        "name": "debugger/attach",
        "description": "Attach to a running process",
        "inputSchema": {
          "type": "object",
          "properties": {
            "pid": {
              "type": "integer",
              "description": "Process ID to attach to"
            }
          },
          "required": ["pid"]
        }
      }
    ]
  }
}
```

### tools/call

Invoke a specific tool.

**Request**

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "debugger/attach",
    "arguments": {
      "pid": 12345
    }
  }
}
```

**Response**

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Successfully attached to process 12345"
      }
    ]
  }
}
```

## Error Codes

### Standard JSON-RPC Errors

| Code | Message | Description |
|------|---------|-------------|
| -32700 | Parse error | Invalid JSON |
| -32600 | Invalid Request | Request object invalid |
| -32601 | Method not found | Method does not exist |
| -32602 | Invalid params | Invalid method parameters |
| -32603 | Internal error | Internal server error |

### kindly-debugger Custom Errors

| Code | Message | Description |
|------|---------|-------------|
| -32001 | Authentication failed | Invalid or missing API key |
| -32002 | Quota exceeded | API quota limit reached |
| -32003 | Permission denied | Insufficient permissions |
| -32004 | Process not found | Target process does not exist |
| -32005 | Not attached | No process currently attached |
| -32006 | Breakpoint failed | Could not set breakpoint |
| -32007 | History unavailable | Time-travel history not available |
| -32008 | Document not found | XML document not found |
| -32009 | Schema validation failed | Document does not match schema |
| -32010 | License expired | License has expired |

### Error Data

Error responses may include additional information in the `data` field:

```json
{
  "error": {
    "code": -32002,
    "message": "Quota exceeded",
    "data": {
      "limit": 10000,
      "used": 10000,
      "reset_at": "2024-12-01T00:00:00Z"
    }
  }
}
```

## Content Types

### Text Content

Most tool responses return text content:

```json
{
  "content": [
    {
      "type": "text",
      "text": "Response text"
    }
  ]
}
```

### Binary Content

Some tools (like `export_trace` with binary format) return base64-encoded data:

```json
{
  "content": [
    {
      "type": "blob",
      "blob": "base64_encoded_data_here",
      "mimeType": "application/octet-stream"
    }
  ]
}
```

## Rate Limiting

Requests are rate-limited based on your plan tier. Rate limit information is included in response headers:

| Header | Description |
|--------|-------------|
| `X-RateLimit-Limit` | Requests allowed per period |
| `X-RateLimit-Remaining` | Requests remaining in period |
| `X-RateLimit-Reset` | Unix timestamp when limit resets |

When rate limited, you'll receive error code -32002.

## Timeouts

- **Connection timeout**: 10 seconds
- **Request timeout**: 60 seconds
- **Long-running operations**: May return progress updates

## Versioning

The API version is indicated in the protocol specification (2024-11-05). Breaking changes will be announced with a new protocol version.

## HTTP Status Codes

| Status | Meaning |
|--------|---------|
| 200 | Success |
| 400 | Bad Request (invalid JSON-RPC) |
| 401 | Unauthorized (invalid API key) |
| 429 | Too Many Requests (rate limited) |
| 500 | Internal Server Error |
| 503 | Service Unavailable |

## Example: Complete Session

```bash
# List available tools
curl -X POST https://api.kindly.services/mcp \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'

# Attach to process
curl -X POST https://api.kindly.services/mcp \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"debugger/attach","arguments":{"pid":12345}}}'

# Get stack trace
curl -X POST https://api.kindly.services/mcp \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"debugger/get_stack_trace","arguments":{}}}'
```

## SDK Support

While kindly-debugger can be used with any HTTP client, we recommend using the MCP SDK for your platform:

- **TypeScript/JavaScript**: `@modelcontextprotocol/sdk`
- **Python**: `mcp-sdk`

See [kindly.services](https://kindly.services) for integration guides.

---

For additional help, contact [support@kindly.software](mailto:support@kindly.software)
