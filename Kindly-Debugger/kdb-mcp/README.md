# KDB - The Kindly Debugger

**Give your AI the superpower of time-travel debugging.**

Rewind execution, find what went wrong, fix the bug timeline.

---

## What is KDB?

KDB is a **time-travel debugger** that connects to Claude, Cursor, or any MCP-compatible AI assistant. Instead of reading code and guessing, your AI can now:

- **Attach** to running processes
- **Set breakpoints** and catch crashes in action
- **Step forward** through execution
- **Step BACKWARD** through time to find the root cause
- **Inspect memory** and variables at any point in history

All through natural language. Just ask Claude to debug your code.

---

## Quick Start

### 1. Get Your License Key

```
https://kindly.software
```

Free Hobby tier available. **7-day trial: ALL features unlocked (Enterprise-level, no credit card).**

### 2. Add to Claude Code

```json
{
  "mcpServers": {
    "kdb": {
      "command": "kdb-mcp",
      "args": ["--license", "YOUR_LICENSE_KEY"]
    }
  }
}
```

### 3. Debug

```
"Claude, attach to process 12345 and find why it's crashing"
```

That's it. Time-travel debugging via conversation.

---

## Features

| Feature | Description |
|---------|-------------|
| **Time-Travel** | Step backward through execution history |
| **Breakpoints** | Hardware breakpoints with hit counting |
| **Stack Traces** | SIMD-accelerated unwinding (5000x faster than GDB) |
| **Memory Inspection** | Read variables at any point in time |
| **Audit Trail** | Cryptographic hash-chain for compliance (SOX/SOC2) |

---

## Pricing

> **Launch Promo: 7-day free trial with ALL features!** No credit card required.
> Enterprise-level access (0x3FF feature mask). After trial, falls back to tier limits.

| Tier | Price | Sessions/Month | Key Features |
|------|-------|----------------|--------------|
| **Hobby** | Free | 5 | 3 step_backward/day, breakpoints, stack traces |
| **Pro** (was Starter) | $19/month | 100 | Unlimited time-travel, basic memory replay |
| **Engineer** (was Developer) | $49/month | 500 | Full memory replay, LSH bug search, read_memory_at_snapshot |
| **Teams** (was Professional) | $129/month | 2,000 | 5 seats (+$20/seat), team audit logs |
| **Enterprise** | From $999/month | Unlimited | SOX/SOC2/GDPR/HIPAA, Q34 audit trail, 7-year retention |

---

## MCP Tools

```
debugger/attach          - Attach to process
debugger/set_breakpoint  - Add breakpoint
debugger/continue        - Resume execution
debugger/step_forward    - Step one instruction
debugger/step_backward   - TIME TRAVEL
debugger/get_stack_trace - Get call stack
debugger/get_variables   - Read memory
debugger/export_trace    - Export audit trail
```

---

## Requirements

- **Platform**: Linux x86_64 (server-side)
- **Access**: Users on any OS connect via MCP
- **Permissions**: Same UID as target process (or CAP_SYS_PTRACE)

---

## Links

- **Website**: [kindly.software](https://kindly.software)
- **Sign Up**: [Get License Key](https://kindly.software)
- **Support**: support@kindly.software

---

<p align="center">
  <strong>Kindly Software</strong><br>
  <em>Debug smarter. Ship faster.</em>
</p>
