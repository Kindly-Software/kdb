# @kindly-software-inc/kdb

MCP client configuration for **KDB - The Kindly Debugger**, an AI-powered time-travel debugger.

## Features

- ⏪ **Time-travel debugging** - Bidirectional execution replay
- 🔐 **Audit-compliant** - Cryptographic hash-chain logging (SOX/SOC2)
- ⚡ **SIMD-accelerated** - 5000× faster stack unwinding than GDB
- 🎯 **Sub-microsecond** - <100ns breakpoint lookup

## Installation

```bash
npm install @kindly-software-inc/kdb
```

## Setup

### 1. Get your license key

Sign up for free at [kindly.software](https://kindly.software)

### 2. Configure your MCP client

**Claude Code / Cursor:**

Add to your MCP configuration:

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

## Available Tools

| Tool | Description |
|------|-------------|
| `attach` | Attach to a running process |
| `detach` | Detach from process |
| `breakpoint_set` | Set a breakpoint |
| `breakpoint_remove` | Remove a breakpoint |
| `step` | Single-step execution |
| `continue` | Continue execution |
| `snapshot` | Capture execution state |
| `back` | Step backward in time |
| `stack` | Get stack trace |
| `memory_read` | Read process memory |
| `registers` | Get CPU registers |

## Pricing

| Tier | Sessions/Month | Price |
|------|----------------|-------|
| **Hobby** | 5 | Free |
| **Pro** | 100 | Coming soon |
| **Enterprise** | Unlimited | Contact us |

🎉 **Launch Promo**: Unlimited sessions during launch week!

## Links

- Website: [kindly.software](https://kindly.software)
- Documentation: [github.com/kindly-software/kdb](https://github.com/kindly-software/kdb)
- Support: support@kindly.software

## License

MIT - This package is MIT licensed. The KDB service itself is proprietary.
