# kindly-debugger Tools Reference

Complete documentation for all 12 MCP tools provided by kindly-debugger.

## Table of Contents

- [Debugger Tools](#debugger-tools)
  - [debugger/attach](#debuggerattach)
  - [debugger/set_breakpoint](#debuggerset_breakpoint)
  - [debugger/continue](#debuggercontinue)
  - [debugger/step_forward](#debuggerstep_forward)
  - [debugger/step_backward](#debuggerstep_backward)
  - [debugger/get_stack_trace](#debuggerget_stack_trace)
  - [debugger/get_variables](#debuggerget_variables)
  - [debugger/find_similar_bugs](#debuggerfind_similar_bugs)
  - [debugger/export_trace](#debuggerexport_trace)
- [Admin Tools](#admin-tools)
  - [debugger/quota_status](#debuggerquota_status)
  - [debugger/license_info](#debuggerlicense_info)
  - [debugger/get_comprehensive_audit](#debuggerget_comprehensive_audit)

---

## Debugger Tools

### debugger/attach

Attach the debugger to a running process.

**Parameters**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `pid` | integer | Yes | Process ID to attach to |

**Example Request**

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "debugger/attach",
    "arguments": {
      "pid": 12345
    }
  }
}
```

**Example Response**

```json
{
  "jsonrpc": "2.0",
  "id": 1,
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

**Notes**
- Requires appropriate permissions to attach to the target process
- On Linux, may require `CAP_SYS_PTRACE` capability

---

### debugger/set_breakpoint

Set a breakpoint at a specified memory address.

**Parameters**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `address` | string | Yes | Memory address in hex format (e.g., "0x401000") |
| `condition` | string | No | Optional condition expression |

**Example Request**

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "debugger/set_breakpoint",
    "arguments": {
      "address": "0x401000"
    }
  }
}
```

**Example Response**

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Breakpoint 1 set at 0x401000"
      }
    ]
  }
}
```

---

### debugger/continue

Continue execution until the next breakpoint or program exit.

**Parameters**

None

**Example Request**

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "debugger/continue",
    "arguments": {}
  }
}
```

**Example Response**

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Stopped at breakpoint 1 (0x401000)"
      }
    ]
  }
}
```

---

### debugger/step_forward

Execute a single instruction forward.

**Parameters**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `count` | integer | No | Number of steps (default: 1) |

**Example Request**

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "name": "debugger/step_forward",
    "arguments": {
      "count": 1
    }
  }
}
```

**Example Response**

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Stepped to 0x401004\nInstruction: mov rax, [rbx+8]"
      }
    ]
  }
}
```

---

### debugger/step_backward

Time-travel: step backward through execution history.

**Parameters**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `count` | integer | No | Number of steps backward (default: 1) |

**Example Request**

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "tools/call",
  "params": {
    "name": "debugger/step_backward",
    "arguments": {
      "count": 1
    }
  }
}
```

**Example Response**

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Stepped back to 0x401000\nRestored from snapshot 42"
      }
    ]
  }
}
```

**Notes**
- Requires execution history to be recorded
- History depth depends on your plan tier

---

### debugger/get_stack_trace

Retrieve the current call stack.

**Parameters**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `max_frames` | integer | No | Maximum frames to return (default: 20) |

**Example Request**

```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "tools/call",
  "params": {
    "name": "debugger/get_stack_trace",
    "arguments": {
      "max_frames": 10
    }
  }
}
```

**Example Response**

```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Stack trace:\n#0  0x401000 main+0x20\n#1  0x7f1234 __libc_start_main+0x80\n#2  0x400500 _start+0x25"
      }
    ]
  }
}
```

---

### debugger/get_variables

Inspect variable values at the current execution point.

**Parameters**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `scope` | string | No | Variable scope: "local", "global", or "all" (default: "local") |
| `filter` | string | No | Filter variables by name pattern |

**Example Request**

```json
{
  "jsonrpc": "2.0",
  "id": 7,
  "method": "tools/call",
  "params": {
    "name": "debugger/get_variables",
    "arguments": {
      "scope": "local"
    }
  }
}
```

**Example Response**

```json
{
  "jsonrpc": "2.0",
  "id": 7,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Local variables:\n  count: i32 = 42\n  buffer: *u8 = 0x7ffe1234\n  flag: bool = true"
      }
    ]
  }
}
```

---

### debugger/find_similar_bugs

Analyze the current bug pattern and find similar occurrences in the codebase.

**Parameters**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `pattern_type` | string | No | Type of pattern: "crash", "memory", "logic" (default: auto-detect) |
| `search_path` | string | No | Directory to search (default: current project) |

**Example Request**

```json
{
  "jsonrpc": "2.0",
  "id": 8,
  "method": "tools/call",
  "params": {
    "name": "debugger/find_similar_bugs",
    "arguments": {
      "pattern_type": "memory"
    }
  }
}
```

**Example Response**

```json
{
  "jsonrpc": "2.0",
  "id": 8,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Found 3 similar patterns:\n\n1. src/parser.rs:142 - Potential null pointer dereference\n   Similarity: 87%\n\n2. src/handler.rs:89 - Unchecked array index\n   Similarity: 72%\n\n3. src/utils.rs:201 - Buffer overflow risk\n   Similarity: 65%"
      }
    ]
  }
}
```

---

### debugger/export_trace

Export the execution trace for offline analysis.

**Parameters**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `format` | string | No | Export format: "json", "binary", "text" (default: "json") |
| `start_snapshot` | integer | No | Starting snapshot ID |
| `end_snapshot` | integer | No | Ending snapshot ID |

**Example Request**

```json
{
  "jsonrpc": "2.0",
  "id": 9,
  "method": "tools/call",
  "params": {
    "name": "debugger/export_trace",
    "arguments": {
      "format": "json"
    }
  }
}
```

**Example Response**

```json
{
  "jsonrpc": "2.0",
  "id": 9,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Trace exported: 1,234 snapshots\nFile: trace_20241205_123456.json\nSize: 2.4 MB"
      }
    ]
  }
}
```

---

## Admin Tools

### debugger/quota_status

Check your current API usage and quota limits.

**Parameters**

None

**Example Request**

```json
{
  "jsonrpc": "2.0",
  "id": 10,
  "method": "tools/call",
  "params": {
    "name": "debugger/quota_status",
    "arguments": {}
  }
}
```

**Example Response**

```json
{
  "jsonrpc": "2.0",
  "id": 10,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Quota Status:\n  Plan: Developer\n  Period: 2024-12-01 to 2024-12-31\n  Used: 4,521 / 10,000 requests\n  Remaining: 5,479 requests"
      }
    ]
  }
}
```

---

### debugger/license_info

Get information about your license and subscription.

**Parameters**

None

**Example Request**

```json
{
  "jsonrpc": "2.0",
  "id": 11,
  "method": "tools/call",
  "params": {
    "name": "debugger/license_info",
    "arguments": {}
  }
}
```

**Example Response**

```json
{
  "jsonrpc": "2.0",
  "id": 11,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "License Information:\n  Type: Developer\n  Status: Active\n  Expires: 2025-12-05\n  Features: Time-travel, Audit trails, Bug detection"
      }
    ]
  }
}
```

---

### debugger/get_comprehensive_audit

Retrieve the audit trail for debugging sessions.

**Parameters**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `start_time` | string | No | ISO 8601 start timestamp |
| `end_time` | string | No | ISO 8601 end timestamp |
| `session_id` | string | No | Filter by specific session |

**Example Request**

```json
{
  "jsonrpc": "2.0",
  "id": 12,
  "method": "tools/call",
  "params": {
    "name": "debugger/get_comprehensive_audit",
    "arguments": {
      "start_time": "2024-12-05T00:00:00Z"
    }
  }
}
```

**Example Response**

```json
{
  "jsonrpc": "2.0",
  "id": 12,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Audit Trail (2024-12-05):\n\n[12:34:56] Session started - user: dev@example.com\n[12:34:58] Attached to PID 12345\n[12:35:02] Breakpoint set at 0x401000\n[12:35:15] Step forward executed\n[12:35:20] Variables inspected (scope: local)\n[12:36:45] Session ended\n\nHash chain verified: SHA256:a1b2c3..."
      }
    ]
  }
}
```

**Notes**
- Audit trails are cryptographically signed
- Suitable for compliance requirements (SOX, SOC2, GDPR, HIPAA)

---

## Error Handling

All tools return errors in standard MCP format:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32602,
    "message": "Invalid params",
    "data": {
      "details": "pid must be a positive integer"
    }
  }
}
```

See [API Reference](API_REFERENCE.md) for complete error code documentation.
