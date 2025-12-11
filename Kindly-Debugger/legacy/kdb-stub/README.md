# KDB Stub - MCP Schema Advertiser

This is a **schema-only stub** for the KDB (Kindly Debugger) MCP server. It exposes tool definitions for MCP directory listings (like Glama) but does not implement actual debugging functionality.

## Purpose

- Advertise KDB's tool schemas to MCP clients and directories
- Provide discovery information for the 8 debugging tools
- Direct users to https://kindly.software for full functionality

## Tools Exposed

| Tool | Description |
|------|-------------|
| `debugger/attach` | Attach to a running process by PID |
| `debugger/set_breakpoint` | Set a breakpoint at a memory address |
| `debugger/continue` | Resume process execution |
| `debugger/step_forward` | Step one instruction forward |
| `debugger/step_backward` | Time-travel backward (unique to KDB) |
| `debugger/get_stack_trace` | Get current call stack |
| `debugger/get_variables` | Read memory/variables |
| `debugger/export_trace` | Export Q34-compliant audit trail |

## Usage

### Docker (Recommended)

```bash
docker pull samuelduchaine/kdb-stub:latest
docker run -i samuelduchaine/kdb-stub:latest
```

### MCP Client Configuration

```json
{
  "mcpServers": {
    "kdb": {
      "command": "docker",
      "args": ["run", "-i", "samuelduchaine/kdb-stub:latest"]
    }
  }
}
```

## What This Is NOT

- This is NOT the full KDB debugger
- This does NOT implement actual ptrace debugging
- This does NOT connect to kindly-hub or any backend
- All tool calls return an error directing users to sign up

## Full KDB Features (Available at kindly.software)

- **Time-travel debugging**: Bidirectional execution replay (<10ns backward step)
- **Audit compliance**: Q34 hash-chain integrity for SOX/SOC2/GDPR
- **Performance**: 625x faster breakpoint lookup vs GDB, 5000x faster stack unwinding
- **Lockfree architecture**: 100% Chaos compliant, zero mutex

## License

MIT - This stub contains no proprietary code.
