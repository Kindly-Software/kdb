# Kindly Debugger

**World's first audit-compliant time-travel debugger for AI workflows**

A commercial MCP (Model Context Protocol) server providing time-travel debugging capabilities for AI-assisted development workflows.

## Overview

Kindly Debugger brings powerful debugging capabilities directly into your AI coding assistant through the MCP protocol. Debug your applications with time-travel, set breakpoints, inspect variables, and maintain complete audit trails - all through natural language commands.

**Platform-Agnostic Access**: Users on macOS, Windows, or Linux can debug via their AI assistant (Claude Code, Cursor, etc.). The debugger runs server-side on Linux, but you never interact with it directly.

## Key Features

- **Time-Travel Debugging** - Step forward and backward through execution history
- **Audit Trails** - Cryptographically signed audit logs for compliance (SOX, SOC2, GDPR)
- **MCP Native** - Seamless integration with Claude Code and other MCP-compatible clients
- **Secure by Design** - API key authentication with rate limiting
- **Bug Pattern Detection** - Find similar bugs across your codebase

## Quick Start

### 1. Get Your API Key

Visit [kindly.software](https://kindly.software) to create an account and obtain your API key.

### 2. Configure Claude Code

Add the following to your `~/.config/claude-code/mcp_servers.json`:

```json
{
  "kindly-debugger": {
    "type": "remote",
    "url": "https://api.kindly.software/mcp",
    "headers": {
      "Authorization": "Bearer YOUR_API_KEY"
    }
  }
}
```

### 3. Start Debugging

Once configured, you can use natural language commands in Claude Code:

- "Attach to process 12345"
- "Set a breakpoint at 0x401000"
- "Step backward to find the bug"
- "Show me the stack trace"

## Tool Overview

| Category | Tool | Description |
|----------|------|-------------|
| **Debugger** | `debugger/attach` | Attach to a running process |
| | `debugger/set_breakpoint` | Set breakpoint at memory address |
| | `debugger/continue` | Continue execution until breakpoint |
| | `debugger/step_forward` | Single-step forward |
| | `debugger/step_backward` | Time-travel step backward |
| | `debugger/get_stack_trace` | Get current call stack |
| | `debugger/get_variables` | Inspect variable values |
| | `debugger/find_similar_bugs` | Find similar patterns in codebase |
| | `debugger/export_trace` | Export execution trace |
| **Admin** | `debugger/quota_status` | Check API usage quota |
| | `debugger/license_info` | Get license information |
| | `debugger/get_comprehensive_audit` | Get audit trail |

## Documentation

- [Getting Started](GETTING_STARTED.md) - Setup guide and first steps
- [Tools Reference](TOOLS.md) - Complete tool documentation
- [API Reference](API_REFERENCE.md) - Protocol details
- [Authentication](AUTHENTICATION.md) - Security and API keys
- [FAQ](FAQ.md) - Frequently asked questions
- [Changelog](CHANGELOG.md) - Version history

## Platform Support

**MCP-First Delivery Model** - Platform Agnostic Access

| What | Platform Support | Notes |
|------|------------------|-------|
| **You (MCP Client)** | macOS, Windows, Linux | Any MCP-compatible AI assistant |
| **Debugging Server** | Linux x86_64 | Server-side, you never interact directly |
| **Protocol** | MCP 2024-11-05 | Standard Model Context Protocol |

**How it works**: Your AI assistant (Claude Code, Cursor, etc.) connects to the Kindly Debugger MCP server. The server runs the debugging operations on Linux and returns results to your AI. You debug through natural language, regardless of your operating system.

## Use Cases

### Security Auditing
Track every debugging action with cryptographic audit trails. Essential for regulated industries requiring detailed access logs.

### Root Cause Analysis
Use time-travel debugging to step backward from a crash or unexpected state to find exactly when things went wrong.

### AI-Assisted Debugging
Combine the power of AI reasoning with low-level debugging. Ask Claude to analyze stack traces and suggest fixes.

### Bug Pattern Detection
Find similar bugs across your codebase by analyzing execution patterns and memory access signatures.

## Pricing

> **Launch Promo: 7-day free trial with ALL features!** No credit card required.
> Enterprise-level access (0x3FF feature mask). After trial, falls back to tier limits.

Visit [kindly.software](https://kindly.software) for pricing and signup:

| Tier | Price | Sessions/Month | Features |
|------|-------|----------------|----------|
| **Hobby** | Free | 5 | 3 step_backward/day, breakpoints, stack traces, audit trails |
| **Pro** (was Starter) | $19/month | 100 | Unlimited time-travel, basic memory replay |
| **Engineer** (was Developer) | $49/month | 500 | Full memory replay, LSH bug search, read_memory_at_snapshot |
| **Teams** (was Professional) | $129/month | 2,000 | 5 seats (+$20/seat), team audit logs |
| **Enterprise** | From $999/month | Unlimited | SOX/SOC2/GDPR/HIPAA, Q34 audit trail, 7-year retention |

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

**14-day money-back guarantee** on all paid plans.

**Sign up now**: https://api.kindly.software/api/v1/signup

## Support

- **Documentation**: This repository
- **Email**: support@kindly.software
- **Issues**: Use the [issue template](.github/ISSUE_TEMPLATE.md) for bug reports

## Security

Kindly Debugger is designed with security as a priority:

- All API communication over HTTPS
- API keys with configurable permissions
- Complete audit logging
- No persistent storage of debugged process data

For security concerns, contact security@kindly.software.

## License

**This documentation** is licensed under [CC-BY-4.0](LICENSE) (Creative Commons Attribution 4.0).

**The Kindly Debugger software** is proprietary and commercial. It is NOT open source. All rights reserved by Kindly Software.

For licensing inquiries, contact: sales@kindly.software

---

**Kindly Software** - Building tools for the next generation of AI-assisted development.

[kindly.software](https://kindly.software) | [support@kindly.software](mailto:support@kindly.software)
