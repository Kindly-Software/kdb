# kdb-mcp Public API Documentation

**Version**: 1.0.0
**Base URL**: `https://debug.kindly.dev/mcp`
**Protocol**: JSON-RPC 2.0 over HTTP/HTTPS
**Last Updated**: 2025-12-04

---

## Overview

kdb-mcp is a high-performance Model Context Protocol (MCP) server for remote debugging with time-travel capabilities. It provides <10us end-to-end latency for debugging operations, 10-100x faster than traditional debuggers.

### Key Features

- **Time-Travel Debugging**: Step forward and backward through execution
- **SIMD Stack Unwinding**: 8x faster stack traces
- **T10 Probabilistic Bug Search**: Find similar bugs using LSH
- **Q34 Audit Trail**: Cryptographic hash-chain for compliance
- **100% Lockfree**: Zero mutex, sub-microsecond latency

---

## Quick Start

### 1. Get Your License Key

Visit [https://kindly.dev/kdb-mcp](https://kindly.dev/kdb-mcp) to obtain your license key.

### 2. Configure Claude Code

Add to your `claude_code_config.json`:

```json
{
  "mcpServers": {
    "kdb-mcp": {
      "command": "kdb-mcp-server",
      "args": ["--license", "YOUR_LICENSE_KEY"],
      "env": {}
    }
  }
}
```

### 3. Start Debugging

In Claude Code, use any of the 9 debugging tools:

```
Claude, attach to process 12345 and show me the stack trace.
```

---

## Authentication

### JWT Bearer Token

All API requests require a valid JWT token in the Authorization header:

```http
Authorization: Bearer <your_jwt_token>
```

**Token Format**: Ed25519-signed JWT with the following claims:

| Claim | Type | Description |
|-------|------|-------------|
| `sub` | string | License ID |
| `exp` | number | Expiration timestamp (Unix epoch) |
| `iat` | number | Issued-at timestamp |
| `scope` | string[] | Granted permissions (e.g., `["debug:read", "debug:write"]`) |
| `tier` | string | Subscription tier (`free`, `pro`, `enterprise`) |

### License Validation

Licenses are validated on every request (<10ns cached lookup). Invalid or expired licenses receive:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32001,
    "message": "License expired or invalid"
  }
}
```

### Two-Factor Authentication (TOTP)

For high-risk operations (memory write, process attach), TOTP may be required:

```http
X-TOTP-Code: 123456
```

Supported authenticators:
- Google Authenticator
- Authy
- 1Password
- Microsoft Authenticator

---

## Rate Limits and Quotas

### Global Rate Limits

| Limit | Free Tier | Pro Tier | Enterprise |
|-------|-----------|----------|------------|
| Requests/second | 10 | 100 | 1,000 |
| Burst capacity | 50 | 500 | 5,000 |
| Concurrent sessions | 1 | 10 | 100 |
| PIDs per session | 5 | 50 | Unlimited |
| Snapshots per session | 100 | 1,000 | 10,000 |

### Per-Operation Quotas

| Operation | Free Tier | Pro Tier | Enterprise |
|-----------|-----------|----------|------------|
| `debugger/attach` | 10/hour | 100/hour | Unlimited |
| `debugger/step_*` | 1,000/hour | 10,000/hour | Unlimited |
| `debugger/get_stack_trace` | 100/hour | 1,000/hour | Unlimited |
| `debugger/find_similar_bugs` | 10/day | 100/day | Unlimited |
| `debugger/export_trace` | 1/day | 10/day | Unlimited |

### Rate Limit Headers

Every response includes rate limit information:

```http
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 87
X-RateLimit-Reset: 1733356800
```

---

## MCP Tools Reference (9 Tools)

### 1. debugger/attach

Attach to a running process for debugging.

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "debugger/attach",
  "params": {
    "pid": 12345
  }
}
```

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `pid` | number | Yes | Process ID to attach to |

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "success": true,
    "session_id": "abc123",
    "pid": 12345,
    "state": "stopped"
  }
}
```

**Errors**:
| Code | Message | Description |
|------|---------|-------------|
| -32001 | PID not whitelisted | Process not in allowlist |
| -32002 | Permission denied | Insufficient privileges |
| -32003 | Process not found | PID does not exist |

**Latency**: <10us (ptrace overhead ~5us)

---

### 2. debugger/set_breakpoint

Set a software breakpoint at the specified address.

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "debugger/set_breakpoint",
  "params": {
    "address": "0x00007f1234567890"
  }
}
```

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `address` | string | Yes | Hex address (0x prefix) |
| `condition` | string | No | Conditional expression |
| `hit_count` | number | No | Break after N hits |

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "breakpoint_id": 1,
    "address": "0x00007f1234567890",
    "enabled": true
  }
}
```

**Latency**: <1us (int3 injection)

---

### 3. debugger/continue

Resume execution until next breakpoint or signal.

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "debugger/continue",
  "params": {}
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "state": "running",
    "reason": null
  }
}
```

**Latency**: <100ns (atomic flag update)

---

### 4. debugger/step_forward

Single-step one instruction forward.

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "debugger/step_forward",
  "params": {}
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": {
    "rip": "0x00007f1234567894",
    "snapshot_id": 42,
    "instruction": "mov rax, [rbp-0x8]"
  }
}
```

**Latency**: <10us (step + snapshot capture ~6ns)

---

### 5. debugger/step_backward

Time-travel: Step one instruction backward.

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "debugger/step_backward",
  "params": {}
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "rip": "0x00007f1234567890",
    "snapshot_id": 41,
    "instruction": "push rbp"
  }
}
```

**Note**: Requires snapshots to have been captured. Returns error if at beginning of recorded history.

**Latency**: <10ns (lockfree replay)

---

### 6. debugger/get_stack_trace

Get the current call stack with SIMD-accelerated unwinding.

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "debugger/get_stack_trace",
  "params": {
    "max_frames": 50
  }
}
```

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `max_frames` | number | No | Maximum frames to return (default: 50) |
| `include_args` | boolean | No | Include function arguments |
| `include_locals` | boolean | No | Include local variables |

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "result": {
    "frames": [
      {
        "index": 0,
        "rip": "0x00007f1234567890",
        "rsp": "0x00007ffe12340000",
        "rbp": "0x00007ffe12340010",
        "function": "process_request",
        "file": "src/server.rs",
        "line": 142
      },
      {
        "index": 1,
        "rip": "0x00007f1234567abc",
        "rsp": "0x00007ffe12340020",
        "rbp": "0x00007ffe12340040",
        "function": "main",
        "file": "src/main.rs",
        "line": 15
      }
    ],
    "total_frames": 2
  }
}
```

**Latency**: <20us for 10 frames (8x faster than GDB)

---

### 7. debugger/get_variables

Read memory and variable values from the debugged process.

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 7,
  "method": "debugger/get_variables",
  "params": {
    "address": "0x00007ffe12340000",
    "length": 64
  }
}
```

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `address` | string | Yes | Memory address (hex) |
| `length` | number | Yes | Bytes to read (max 4096) |
| `format` | string | No | Output format: `hex`, `ascii`, `both` (default: `hex`) |

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 7,
  "result": {
    "address": "0x00007ffe12340000",
    "length": 64,
    "hex": "48 89 e5 48 83 ec 10 89 7d fc 48 89 75 f0...",
    "ascii": "H..H....}..u...."
  }
}
```

**Latency**: <1us (atomic coordinated read)

---

### 8. debugger/find_similar_bugs

Use T10 Probabilistic LSH to find similar bugs in the codebase.

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 8,
  "method": "debugger/find_similar_bugs",
  "params": {
    "signature": {
      "stack_hash": "0xabc123",
      "registers": {"rax": 0, "rbx": 42},
      "memory_pattern": "null pointer dereference"
    },
    "threshold": 0.8
  }
}
```

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `signature` | object | Yes | Bug signature to search for |
| `threshold` | number | No | Similarity threshold (0.0-1.0, default: 0.8) |
| `max_results` | number | No | Maximum matches (default: 10) |

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 8,
  "result": {
    "matches": [
      {
        "bug_id": "BUG-2024-1234",
        "similarity": 0.92,
        "description": "Null pointer dereference in request handler",
        "resolution": "Added null check before access",
        "file": "src/handler.rs",
        "line": 87
      }
    ],
    "total_matches": 1
  }
}
```

**Latency**: ~50us (LSH lookup)

---

### 9. debugger/export_trace

Stream the entire debugging trace for offline analysis.

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 9,
  "method": "debugger/export_trace",
  "params": {
    "format": "json",
    "include_memory": false
  }
}
```

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `format` | string | No | Export format: `json`, `binary`, `protobuf` |
| `include_memory` | boolean | No | Include memory snapshots (large) |
| `compress` | boolean | No | Gzip compression (default: true) |

**Response**:
Streaming JSON-RPC notification with progress:

```json
{"jsonrpc": "2.0", "method": "export_progress", "params": {"percent": 25}}
{"jsonrpc": "2.0", "method": "export_progress", "params": {"percent": 50}}
{"jsonrpc": "2.0", "method": "export_progress", "params": {"percent": 75}}
{"jsonrpc": "2.0", "method": "export_progress", "params": {"percent": 100}}
{
  "jsonrpc": "2.0",
  "id": 9,
  "result": {
    "file_path": "/tmp/trace-abc123.json.gz",
    "size_bytes": 1048576,
    "snapshots": 1000
  }
}
```

**Latency**: Variable (T5 streaming, ~100MB/s)

---

## Claude Code Integration

### Automatic Tool Discovery

Claude Code automatically discovers kdb-mcp tools via the MCP protocol. No manual configuration required beyond the initial setup.

### Example Conversations

**Attaching to a process**:
```
User: Debug the nginx process
Claude: I'll attach to the nginx process and show you the current state.
[Uses debugger/attach with PID from `pgrep nginx`]
```

**Time-travel debugging**:
```
User: Go back to before the crash
Claude: I'll step backward through the execution history to find the state before the crash.
[Uses debugger/step_backward repeatedly until crash point is found]
```

**Finding similar bugs**:
```
User: Have we seen this null pointer error before?
Claude: Let me search for similar bugs in your codebase.
[Uses debugger/find_similar_bugs with current error signature]
```

### Best Practices

1. **Whitelist PIDs first**: Add processes to the allowlist before debugging
2. **Use snapshots**: Enable snapshot capture for time-travel
3. **Limit memory reads**: Use targeted addresses, not full dumps
4. **Export traces offline**: For long debugging sessions, export for analysis

---

## Pricing Tiers

### Free Tier

- 10 requests/second
- 1 concurrent session
- 5 PIDs per session
- 100 snapshots
- Community support

**Price**: $0/month

### Pro Tier

- 100 requests/second
- 10 concurrent sessions
- 50 PIDs per session
- 1,000 snapshots
- Email support
- Similar bug search
- Trace export (10/day)

**Price**: $49/month or $490/year (2 months free)

### Enterprise Tier

- 1,000 requests/second
- 100 concurrent sessions
- Unlimited PIDs
- 10,000 snapshots
- Priority support
- SSO/SAML integration
- Custom SLA (99.99%)
- On-premise deployment option
- Q34 audit trail export

**Price**: Contact sales@kindly.dev

---

## Error Codes

### Standard JSON-RPC Errors

| Code | Message | Description |
|------|---------|-------------|
| -32700 | Parse error | Invalid JSON |
| -32600 | Invalid Request | Not valid JSON-RPC |
| -32601 | Method not found | Unknown method |
| -32602 | Invalid params | Invalid parameters |
| -32603 | Internal error | Server error |

### kdb-mcp Specific Errors

| Code | Message | Description |
|------|---------|-------------|
| -32001 | License error | Invalid or expired license |
| -32002 | Authentication failed | Invalid JWT or TOTP |
| -32003 | Rate limited | Quota exceeded |
| -32004 | PID not whitelisted | Process not in allowlist |
| -32005 | Permission denied | Insufficient privileges |
| -32006 | Process not found | PID does not exist |
| -32007 | Snapshot not found | Invalid snapshot ID |
| -32008 | Session expired | Session timed out |
| -32009 | Audit trail error | Integrity violation |

---

## SDKs and Libraries

### Official SDKs

- **Rust**: `kdb-mcp-client` (crates.io)
- **Python**: `kdb-mcp` (PyPI)
- **TypeScript**: `@kindly/kdb-mcp` (npm)

### Community SDKs

- **Go**: `github.com/kindly-dev/kdb-mcp-go`
- **Java**: `dev.kindly:kdb-mcp-java` (Maven Central)

---

## Support

- **Documentation**: [https://docs.kindly.dev/kdb-mcp](https://docs.kindly.dev/kdb-mcp)
- **GitHub Issues**: [https://github.com/kindly-dev/kdb-mcp/issues](https://github.com/kindly-dev/kdb-mcp/issues)
- **Discord**: [https://discord.gg/kindly-dev](https://discord.gg/kindly-dev)
- **Email**: support@kindly.dev

---

## Changelog

### v1.0.0 (2025-Q1)

- Initial public release
- 9 MCP debugging tools
- <10us latency SLA
- Time-travel debugging
- T10 similar bug search
- Q34 audit trail

---

**API Documentation Version**: 1.0.0
**Last Updated**: 2025-12-04
