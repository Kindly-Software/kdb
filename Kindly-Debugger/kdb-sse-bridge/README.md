# KDB - The Kindly Debugger

**Give your AI the superpower of time-travel debugging.**

Rewind execution, find what went wrong, fix the bug timeline.

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
      "transport": "sse",
      "url": "https://mcp.kindly.software/sse",
      "headers": {
        "X-License-Key": "YOUR_LICENSE_KEY"
      }
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
| **Stack Traces** | Fast unwinding (<20μs per 10 frames) |
| **Memory Inspection** | Read variables at any point in time |
| **Session Management** | Tiered sessions (Light/Medium/Heavy) |
| **Memory Replay** | COW-based memory snapshot navigation |
| **Audit Trail** | Cryptographic hash-chain for compliance (SOX/SOC2) |

---

## 27 Available Tools

### Core Debugging
- `debugger_attach` - Attach to running process via ptrace
- `debugger_set_breakpoint` - Set breakpoint at memory address
- `debugger_continue` - Resume execution after breakpoint hit
- `debugger_step_forward` - Single-step forward one instruction
- `debugger_step_backward` - Time-travel: step backward
- `debugger_get_stack_trace` - Fast stack unwinding
- `debugger_get_variables` - Read process memory at address

### Analysis
- `debugger_find_similar_bugs` - LSH similarity search for bugs
- `debugger_export_trace` - Export execution trace (JSON/binary)

### Session Management
- `debugger_allocate_session` - Allocate tiered debugging session
- `debugger_release_session` - Release debugging session
- `debugger_get_session_tier` - Get current session tier
- `debugger_upgrade_session` - Upgrade to higher tier
- `debugger_get_pool_stats` - Pool statistics snapshot

### Memory Replay
- `debugger_enable_memory_replay` - Enable COW memory tracking
- `debugger_capture_memory_snapshot` - Capture memory snapshot
- `debugger_read_memory_at_snapshot` - Read memory at historical snapshot
- `debugger_navigate_to_snapshot` - Navigate to specific snapshot
- `debugger_get_memory_replay_stats` - Memory replay statistics
- `debugger_verify_memory_integrity` - Hash-chain integrity verification

### Access Control
- `debugger_get_access_mode` - Get Observer/Operator mode
- `debugger_request_operator_challenge` - Request Ed25519 challenge
- `debugger_elevate_to_operator` - Elevate to Operator mode
- `debugger_revoke_operator` - Revoke Operator mode

### Status & Compliance
- `debugger_quota_status` - Quota and usage status
- `debugger_license_info` - License information
- `debugger_get_comprehensive_audit` - Audit trail with compliance metadata

---

## Pricing

> **Launch Promo: 7-day free trial with ALL features!** No credit card required.
> Get Enterprise-level access (0x3FF feature mask) - unlimited sessions, all debugging tools.
> After trial, automatically falls back to your tier's limits.

| Tier | Price | Sessions/Month | Key Features |
|------|-------|----------------|--------------|
| **Hobby** | Free | 5 | 3 step_backward/day, breakpoints, stack traces |
| **Pro** (was Starter) | $19/month | 100 | Unlimited time-travel, unlimited step_backward, basic memory replay |
| **Engineer** (was Developer) | $49/month | 500 | Full memory replay, LSH bug search (find_similar_bugs), read_memory_at_snapshot |
| **Teams** (was Professional) | $129/month | 2,000 | Same as Engineer + 5 seats (+$20/seat), team audit logs |
| **Enterprise** | From $999/month | Unlimited | SOX/SOC2/GDPR/HIPAA compliance, Q34 audit trail, custom retention (up to 7 years) |

### Feature Comparison

| Feature | Hobby | Pro | Engineer | Teams | Enterprise |
|---------|-------|-----|----------|-------|------------|
| Time-travel | Yes | Yes | Yes | Yes | Yes |
| step_backward | 3/day | Unlimited | Unlimited | Unlimited | Unlimited |
| Memory replay | No | Basic | Full | Full | Full |
| read_memory_at_snapshot | No | No | Yes | Yes | Yes |
| find_similar_bugs (LSH) | No | No | Yes | Yes | Yes |
| Team seats | - | - | - | 5 (+$20/seat) | Unlimited |
| Compliance (SOX/SOC2/GDPR/HIPAA) | No | No | No | No | Yes |
| Custom retention | No | No | No | No | Up to 7 years |

**14-day money-back guarantee** on all paid plans.

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
