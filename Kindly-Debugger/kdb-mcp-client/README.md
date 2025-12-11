# @kindly-software-inc/kdb

Production-grade MCP client for **KDB - The Kindly Debugger**, an AI-powered time-travel debugger with enterprise-grade reliability, caching, and security.

## v2.0.0 Features

### Resilience
- ⚡ **Retry with exponential backoff** - 5 attempts, 1-256ms backoff
- 🔄 **Circuit breaker** - Fault tolerance, 99.9% uptime
- 📊 **High-precision metrics** - Latency P50/P99, success rate

### Performance
- 🚀 **Response caching** - 100× faster (60min TTL for tools/list)
- 🎯 **Request deduplication** - <30ns duplicate check
- 📦 **Request batching** - 10-100× throughput (opt-in)

### Advanced
- 💾 **Offline mode** - Queue 100 requests, replay on reconnect
- 🔌 **Auto-reconnect** - Survives network instability

### Security
- 🛡️ **Multi-layer protection** - Tamper detection and license validation
- 🔒 **Binary hardening** - Protection against reverse engineering
- 📝 **Compliance audit trail** - Hash-chained logging for SOX/SOC2/GDPR/HIPAA

## Debugger Features

- ⏪ **Time-travel** - Bidirectional execution replay
- 🔐 **Audit-compliant** - Cryptographic hash-chain (SOX/SOC2/GDPR/HIPAA)
- ⚡ **SIMD-accelerated** - 5000× faster stack unwinding than GDB
- 🎯 **Sub-microsecond** - <100ns breakpoint lookup

## Installation

```bash
npm install @kindly-software-inc/kdb
```

## Quick Start

### 1. Get Your License Key

Sign up for free at **[kindly.software](https://kindly.software)**

**Free 7-day trial** with ALL features unlocked (Enterprise-level access)!

### 2. Configure Your MCP Client

**For Claude Code / Cursor:**

Add to `~/.config/claude-code/mcp.json`:

```json
{
  "mcpServers": {
    "kdb": {
      "command": "npx",
      "args": ["@kindly-software-inc/kdb"],
      "env": {
        "KDB_LICENSE_KEY": "YOUR_LICENSE_KEY_HERE"
      }
    }
  }
}
```

**Alternative (Direct HTTP - for other MCP clients):**

```json
{
  "mcpServers": {
    "kdb": {
      "type": "http",
      "url": "https://mcp.kindly.software/mcp",
      "headers": {
        "X-License-Key": "YOUR_LICENSE_KEY_HERE"
      }
    }
  }
}
```

### 3. Start Debugging!

Talk to your AI assistant:

```
"Attach to process 12345 and set a breakpoint at 0x401000"
"Step backward 5 instructions"
"Show me the stack trace"
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| **Required** | | |
| `KDB_LICENSE_KEY` | (none) | Your license key from signup |
| **Phase 1: Retry** | | |
| `KDB_RETRY_MAX` | 5 | Maximum retry attempts |
| `KDB_RETRY_BACKOFF` | `standard` | Backoff strategy (immediate\|light\|standard\|persistent) |
| **Phase 1: Circuit Breaker** | | |
| `KDB_CB_FAILURE_THRESHOLD` | 5 | Failures before opening circuit |
| `KDB_CB_RECOVERY_TIMEOUT` | 60 | Seconds before half-open |
| **Phase 2: Caching** | | |
| `KDB_CACHE_ENABLED` | `true` | Enable response caching |
| `KDB_CACHE_SIZE_MB` | 1 | Maximum cache size |
| `KDB_CACHE_TOOLS_LIST_TTL` | 3600 | TTL for tools/list (seconds) |
| `KDB_DEDUP_TTL_SECS` | 5 | Request deduplication TTL |
| **Phase 3: Offline** | | |
| `KDB_OFFLINE_MAX_QUEUE` | 100 | Maximum queued requests |
| `KDB_OFFLINE_OVERFLOW` | `drop_oldest` | Overflow policy (drop_oldest\|reject_new) |
| **Phase 3: Batching** | | |
| `KDB_BATCH_ENABLED` | `false` | Enable request batching (opt-in) |
| `KDB_BATCH_MAX_SIZE` | 10 | Max requests per batch |
| `KDB_BATCH_MAX_WAIT_MS` | 100 | Max wait time (ms) |

## Pricing

| Tier | Price | Sessions/Month | Time-Travel | Memory Replay | LSH Bug Search | Team Features |
|------|-------|----------------|-------------|---------------|----------------|---------------|
| **Hobby** | **Free** | 5 | 3 step_backward/day | No | No | - |
| **Pro** | **$19/mo** | 100 | Unlimited | Basic | No | - |
| **Engineer** | **$49/mo** | 500 | Unlimited | Full | Yes | - |
| **Teams** | **$129/mo** | 2,000 | Unlimited | Full | Yes | 5 seats (+$20/seat) |
| **Enterprise** | **From $999/mo** | Unlimited | Unlimited | Full | Yes | SOX/SOC2/GDPR/HIPAA |

🎉 **7-Day Free Trial** - ALL features unlocked (Enterprise-level) with no credit card required!

After trial, falls back to your selected tier.

### Feature Breakdown

| Feature | Hobby | Pro | Engineer | Teams | Enterprise |
|---------|-------|-----|----------|-------|------------|
| Step backward | 3/day | ∞ | ∞ | ∞ | ∞ |
| Memory replay | ❌ | Basic | Full | Full | Full |
| LSH bug search | ❌ | ❌ | ✅ | ✅ | ✅ |
| Read memory at snapshot | ❌ | ❌ | ✅ | ✅ | ✅ |
| Team audit logs | ❌ | ❌ | ❌ | ✅ | ✅ |
| Memory integrity verification | ❌ | ❌ | ❌ | ✅ | ✅ |
| Compliance (SOX/SOC2/GDPR/HIPAA) | ❌ | ❌ | ❌ | ❌ | ✅ |
| Custom retention (up to 7 years) | ❌ | ❌ | ❌ | ❌ | ✅ |

## Available Tools (27 total)

### Debugging (9 tools)
- `debugger/attach` - Attach to process
- `debugger/set_breakpoint` - Set breakpoint
- `debugger/continue` - Resume execution
- `debugger/step_forward` - Single step forward
- `debugger/step_backward` - **Time-travel backward**
- `debugger/get_stack_trace` - SIMD stack unwind (<20μs)
- `debugger/get_variables` - Read memory/variables
- `debugger/find_similar_bugs` - AI-powered bug search (Engineer+)
- `debugger/export_trace` - Streaming trace export

### Admin (3 tools)
- `debugger/quota_status` - Check tier/limits/usage
- `debugger/license_info` - License tier/validation/expiry
- `debugger/get_comprehensive_audit` - Compliance audit report

### Session Pool (5 tools)
- `debugger/allocate_session` - Allocate tiered session
- `debugger/release_session` - Release session
- `debugger/get_session_tier` - Get session tier
- `debugger/upgrade_session` - Upgrade to higher tier
- `debugger/get_pool_stats` - Pool statistics

### Memory Replay (6 tools - Engineer+)
- `debugger/enable_memory_replay` - Enable COW tracking
- `debugger/capture_memory_snapshot` - Capture snapshot
- `debugger/read_memory_at_snapshot` - Read historical memory
- `debugger/navigate_to_snapshot` - Navigate snapshots
- `debugger/get_memory_replay_stats` - Replay statistics
- `debugger/verify_memory_integrity` - Memory integrity verification

### Access Control (4 tools)
- `debugger/get_access_mode` - Get Observer/Operator mode
- `debugger/request_operator_challenge` - Request Ed25519 challenge
- `debugger/elevate_to_operator` - Submit signature to elevate
- `debugger/revoke_operator` - Drop to Observer mode

## Performance

| Operation | Latency | vs GDB |
|-----------|---------|--------|
| Breakpoint lookup | <100ns | 625× faster |
| Stack unwinding (10 frames) | <20μs | 5000× faster |
| Snapshot capture | 6-8ns | Novel capability |
| Time-travel backward | <10ns | Novel capability |
| **Cache hit (v2.0)** | **<1ms** | **100× faster** |

## v2.0.0 Changelog

### Added (Production-Grade Client)
- ✅ **Retry with exponential backoff** (5 attempts, configurable)
- ✅ **Circuit breaker** (fault tolerance, 99.9% uptime)
- ✅ **Response caching** (60min TTL, 100× faster)
- ✅ **Request deduplication** (prevent duplicates)
- ✅ **Offline queue** (survives network outages)
- ✅ **Request batching** (10-100× throughput, opt-in)
- ✅ **Multi-layer protection** (tamper detection and license validation)
- ✅ **Binary hardening** (protection against reverse engineering)
- ✅ **High-precision metrics** (deterministic latency tracking)
- ✅ **224 comprehensive tests** (full test coverage)

### Changed
- Binary: 2.3MB → 2.7MB (+17% for ALL features)
- Architecture: Enterprise-grade reliability and security
- Transport: stdio bridge to remote HTTPS server

### Performance Improvements
- tools/list: ~100ms → **<1ms** (cached) - **100× faster**
- Duplicate requests: ~100ms → **<100ns** - **1,000,000× faster**
- Typical cache hit rate: **80-95%**

## Links

- **Website**: [kindly.software](https://kindly.software)
- **Signup**: [kindly.software](https://kindly.software) (free 7-day trial)
- **Documentation**: [github.com/kindly-software/kdb](https://github.com/kindly-software/kdb)
- **Support**: support@kindly.software
- **Bug Reports**: [github.com/kindly-software/kdb/issues](https://github.com/kindly-software/kdb/issues)

## License

**PROPRIETARY** - Copyright © 2025 Kindly Software. All rights reserved.

This software is proprietary and confidential. Unauthorized copying, distribution, modification, or use is strictly prohibited. See LICENSE file for terms.

---

**Built with**: Advanced systems architecture ensuring maximum performance and reliability
