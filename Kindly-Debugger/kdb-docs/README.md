# Kindly Debugger

A hosted MCP (Model Context Protocol) server providing time-travel debugging capabilities for AI-assisted development workflows.

## Overview

Kindly Debugger brings powerful debugging capabilities directly into your AI coding assistant through the MCP protocol. Debug your applications with time-travel, set breakpoints, inspect variables, and maintain complete audit trails - all through natural language commands.

## Key Features

- **Time-Travel Debugging** - Step forward and backward through execution history
- **Audit Trails** - Cryptographically signed audit logs for compliance (SOX, SOC2, GDPR)
- **MCP Native** - Seamless integration with Claude Code and other MCP-compatible clients
- **Secure by Design** - API key authentication with rate limiting
- **Bug Pattern Detection** - Find similar bugs across your codebase

## Quick Start

### 1. Get Your API Key

Visit [kindly.services](https://kindly.services) to create an account and obtain your API key.

### 2. Configure Claude Code

Add the following to your `~/.config/claude-code/mcp_servers.json`:

```json
{
  "kindly-debugger": {
    "type": "remote",
    "url": "https://api.kindly.services/mcp",
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

## Supported Platforms

- **Target Platforms**: Linux (x86_64), with macOS and Windows support planned
- **Client Requirements**: Any MCP-compatible client (Claude Code, etc.)
- **Protocol**: MCP 2024-11-05 specification

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

Visit [kindly.services](https://kindly.services) for pricing tiers:

- **Free Tier** - Limited quota for evaluation
- **Developer** - Individual developer usage
- **Team** - Multi-user with shared quota
- **Enterprise** - Custom limits and SLA

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

This documentation is licensed under [CC-BY-4.0](LICENSE) (Creative Commons Attribution 4.0).

The Kindly Debugger service is proprietary software provided by Kindly Software.

---

**Kindly Software** - Building tools for the next generation of AI-assisted development.

[kindly.services](https://kindly.services) | [support@kindly.software](mailto:support@kindly.software)
