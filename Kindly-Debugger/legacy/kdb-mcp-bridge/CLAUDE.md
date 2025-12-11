# KDB MCP HTTP Bridge

T1 Atomic stdio-to-HTTP bridge for KDB MCP debugger.

## UCE34 Compliance

| Question | Status | Details |
|----------|--------|---------|
| Q10 Tier | T1 Atomic | Single-threaded, no shared state coordination |
| Q11 Rust | 100% | Pure Rust, no unsafe |
| Q28 Interface | Simple | stdin JSON-RPC → HTTP POST → stdout JSON-RPC |
| Q33 Lockfree | N/A | Single-threaded, no locks needed |
| Q34 Audit | Yes | All requests logged to stderr |

## Architecture

```
stdin (JSON-RPC) → McpBridgeCapsule → HTTP POST → stdout (JSON-RPC)
                         │
                         ├── BridgeConfig (64B aligned)
                         │   ├── url: String
                         │   ├── license_key: String
                         │   └── timeout_secs: u64
                         │
                         └── BridgeMetrics (64B aligned, atomic)
                             ├── total_requests: AtomicU64
                             ├── successful_requests: AtomicU64
                             ├── failed_requests: AtomicU64
                             └── total_latency_us: AtomicU64
```

## Performance

- <1ms local processing (network latency dominates)
- Zero allocations in hot path after initial 64KB buffer
- ~1MB binary (stripped, LTO)
- Single dependency: ureq (no async runtime)

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `KDB_LICENSE_KEY` | Yes | - | KDB license key for authentication |
| `KDB_MCP_URL` | No | `https://mcp.kindly.software/mcp` | MCP endpoint URL |
| `KDB_TIMEOUT` | No | `30` | Request timeout in seconds |

## Build

```bash
cd /home/samuel/Primitives/Kindly-Debugger/kdb-mcp-bridge
cargo build --release
cp target/release/kdb-mcp-bridge ~/bin/
```

## Claude Code Configuration

Add to `~/.claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "kdb": {
      "command": "/home/samuel/bin/kdb-mcp-bridge",
      "env": {
        "KDB_LICENSE_KEY": "YOUR_LICENSE_KEY_HERE"
      }
    }
  }
}
```

## Test

```bash
# Manual test
export KDB_LICENSE_KEY="KDB-ENTERPRISE-..."
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"debugger_quota_status","arguments":{}}}' | ./target/release/kdb-mcp-bridge
```

## Logging

All activity logged to stderr:
```
[1733762123.456] KDB MCP Bridge started
[1733762123.456] URL: https://mcp.kindly.software/mcp
[1733762123.789] REQ [1]: 95 bytes
[1733762123.890] OK [1]: 512 bytes, 101234us
```
