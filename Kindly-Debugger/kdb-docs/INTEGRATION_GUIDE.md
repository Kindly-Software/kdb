# Kindly Debugger Integration Guide

Complete guide for integrating Kindly Debugger (KDB) with your MCP-compatible AI assistant.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Installation by Platform](#installation-by-platform)
  - [Windows](#windows)
  - [macOS](#macos)
  - [Linux](#linux)
- [Configuration](#configuration)
  - [Claude Code](#claude-code)
  - [Cursor](#cursor)
  - [Other MCP Clients](#other-mcp-clients)
- [Environment Variables](#environment-variables)
- [MCP Tools Reference (27 Tools)](#mcp-tools-reference-27-tools)
- [Troubleshooting](#troubleshooting)
- [Security Best Practices](#security-best-practices)

---

## Prerequisites

Before integrating KDB, you need:

1. **An MCP-compatible client** - Claude Code, Cursor, or any client supporting MCP 2024-11-05
2. **A KDB license key** - Sign up at [kindly.software](https://kindly.software) or via API
3. **Network access** to `mcp.kindly.software` (port 443)

### Get Your License Key

**Option A: Website Signup**
1. Visit [kindly.software](https://kindly.software)
2. Click "Start Free" or "Sign Up"
3. Enter your email to receive your license key

**Option B: API Signup**
```bash
curl -X POST https://api.kindly.software/api/v1/signup \
  -H "Content-Type: application/json" \
  -d '{"email": "you@example.com"}'
```

Your license key will look like:
```
HOB-2025-12-14-a1b2c3d4e5f6...
```

**7-Day Free Trial**: All new signups get unlimited Enterprise features for 7 days!

---

## Installation by Platform

### Windows

**Step 1: Set Environment Variable**

Open PowerShell as Administrator:
```powershell
# Set user environment variable (persistent)
[Environment]::SetEnvironmentVariable("KDB_LICENSE_KEY", "YOUR-LICENSE-KEY-HERE", "User")

# Verify
$env:KDB_LICENSE_KEY
```

Or via Windows Settings:
1. Press `Win + X`, select "System"
2. Click "Advanced system settings"
3. Click "Environment Variables"
4. Under "User variables", click "New"
5. Name: `KDB_LICENSE_KEY`, Value: your license key

**Step 2: Configure MCP Client**

See [Configuration](#configuration) section below.

### macOS

**Step 1: Set Environment Variable**

Add to your shell profile (`~/.zshrc` or `~/.bash_profile`):
```bash
export KDB_LICENSE_KEY="YOUR-LICENSE-KEY-HERE"
```

Apply changes:
```bash
source ~/.zshrc
```

**Step 2: Configure MCP Client**

See [Configuration](#configuration) section below.

### Linux

**Step 1: Set Environment Variable**

Add to `~/.bashrc` or `~/.profile`:
```bash
export KDB_LICENSE_KEY="YOUR-LICENSE-KEY-HERE"
```

Apply changes:
```bash
source ~/.bashrc
```

For systemd services, create `/etc/systemd/system/myservice.service.d/kdb.conf`:
```ini
[Service]
Environment="KDB_LICENSE_KEY=YOUR-LICENSE-KEY-HERE"
```

**Step 2: Configure MCP Client**

See [Configuration](#configuration) section below.

---

## Configuration

### Claude Code

**Config File Location**: `~/.claude.json`

**Recommended Configuration (using environment variable)**:
```json
{
  "mcpServers": {
    "kdb": {
      "type": "http",
      "url": "https://mcp.kindly.software/mcp",
      "headers": {
        "X-License-Key": "${KDB_LICENSE_KEY}"
      }
    }
  }
}
```

**Alternative (stdio with bridge)**:
```json
{
  "mcpServers": {
    "kdb": {
      "type": "stdio",
      "command": "/path/to/kdb-mcp-bridge",
      "env": {
        "KDB_LICENSE_KEY": "${KDB_LICENSE_KEY}"
      }
    }
  }
}
```

**Restart Claude Code** after configuration changes (or run `/mcp` to reconnect).

### Cursor

**Config Location**: Settings > MCP Servers

1. Open Cursor Settings (`Cmd/Ctrl + ,`)
2. Navigate to "MCP Servers"
3. Add new server with:
   - Name: `kdb`
   - Type: `http`
   - URL: `https://mcp.kindly.software/mcp`
   - Headers:
     ```json
     {
       "X-License-Key": "${KDB_LICENSE_KEY}"
     }
     ```

### Other MCP Clients

For any MCP 2024-11-05 compatible client, configure:

| Setting | Value |
|---------|-------|
| Server URL | `https://mcp.kindly.software/mcp` |
| SSE Endpoint | `https://mcp.kindly.software/sse` |
| Auth Header | `X-License-Key: YOUR-LICENSE-KEY` |
| Protocol | MCP 2024-11-05 |

---

## Environment Variables

KDB supports configuration via environment variables for security:

| Variable | Description | Required |
|----------|-------------|----------|
| `KDB_LICENSE_KEY` | Your license key | Yes |
| `KDB_LOG_LEVEL` | Logging level (debug/info/warn/error) | No |
| `KDB_TIMEOUT_MS` | Request timeout in milliseconds (default: 30000) | No |

**Security Note**: Always use environment variables for license keys. Never commit keys to version control.

---

## MCP Tools Reference (27 Tools)

KDB provides 27 MCP tools organized into 6 categories.

### Core Debugging (7 tools)

Essential debugging operations available to all tiers.

| Tool | Description | Latency |
|------|-------------|---------|
| `debugger/attach` | Attach to running process via ptrace | ~10us |
| `debugger/set_breakpoint` | Set breakpoint at memory address (hex format: `0x...`) | <100ns |
| `debugger/continue` | Resume execution after breakpoint hit | Variable |
| `debugger/step_forward` | Single-step forward one instruction | ~5us + 6ns snapshot |
| `debugger/step_backward` | Time-travel backward (Hobby: 3/day limit) | <10ns |
| `debugger/get_stack_trace` | SIMD-accelerated stack unwinding | <20us per 10 frames |
| `debugger/get_variables` | Read process memory at address | Variable |

### Session Management (7 tools)

Manage debugging sessions with tiered resource allocation.

| Tool | Description | Latency |
|------|-------------|---------|
| `debugger/allocate_session` | Allocate tiered session (Light: 64KB, Medium: 256KB, Heavy: 1MB) | <100ns |
| `debugger/release_session` | Release debugging session and free resources | <100ns |
| `debugger/get_session_tier` | Get current session tier | <10ns |
| `debugger/upgrade_session` | Upgrade to higher tier with data migration | <1us |
| `debugger/get_pool_stats` | Pool statistics snapshot | <50ns |
| `debugger/quota_status` | Quota tier/limits/usage | <70ns |
| `debugger/license_info` | License tier/validation/expiry | <10ns (cached) |

### Memory Replay (6 tools) [Pro+ tiers]

Copy-on-write memory tracking for time-travel debugging.

| Tool | Description | Latency | Tier |
|------|-------------|---------|------|
| `debugger/enable_memory_replay` | Enable COW memory tracking for session | <10ms init | Pro+ |
| `debugger/capture_memory_snapshot` | Capture memory snapshot | <50ms typical | Pro+ |
| `debugger/read_memory_at_snapshot` | Read memory at historical snapshot | <2ms | Engineer+ |
| `debugger/navigate_to_snapshot` | Navigate to specific snapshot | <100ns | Pro+ |
| `debugger/get_memory_replay_stats` | Memory replay statistics | <50ns | Pro+ |
| `debugger/verify_memory_integrity` | Q34 hash-chain integrity verification | O(n) | Pro+ |

### Analysis (2 tools) [Engineer+ tiers]

Advanced bug detection and trace analysis.

| Tool | Description | Tier |
|------|-------------|------|
| `debugger/find_similar_bugs` | T10 probabilistic LSH similarity search for bugs | Engineer+ |
| `debugger/export_trace` | T5 streaming export of execution trace (JSON/binary) | Engineer+ |

### Security (4 tools)

Observer/Operator access mode with Ed25519 authentication.

| Tool | Description | Latency |
|------|-------------|---------|
| `debugger/get_access_mode` | Get current Observer/Operator access mode | <10ns |
| `debugger/request_operator_challenge` | Request Ed25519 challenge for elevation | <1ms |
| `debugger/elevate_to_operator` | Submit signature to elevate to Operator mode | <1ms |
| `debugger/revoke_operator` | Drop from Operator to Observer mode | <10ns |

### Audit (1 tool)

Q34 compliance and audit trail.

| Tool | Description | Latency |
|------|-------------|---------|
| `debugger/get_comprehensive_audit` | Q34 compliance audit with BLAKE3 hash-chain | <10us |

---

## Troubleshooting

### "Connection refused" or "502 Bad Gateway"

**Causes**:
- Network connectivity issues
- Firewall blocking `mcp.kindly.software:443`
- VPN interference

**Solutions**:
1. Test connectivity: `curl -I https://mcp.kindly.software/health`
2. Check firewall rules for HTTPS (port 443)
3. Try temporarily disabling VPN

### "Authentication required" or "Invalid API key"

**Causes**:
- Missing or incorrect license key
- Environment variable not set
- Expired license

**Solutions**:
1. Verify environment variable: `echo $KDB_LICENSE_KEY`
2. Check license format: should start with tier prefix (e.g., `HOB-`, `PRO-`, `ENT-`)
3. Check license expiry with `debugger/license_info` tool
4. Re-download license from [kindly.software](https://kindly.software)

### "Permission denied" when attaching

**Causes**:
- Target process not accessible
- Insufficient permissions on server

**Solutions**:
- This is handled server-side; no local action required
- Contact support if persistent

### "Quota exceeded" Error

**Causes**:
- Monthly session limit reached
- Daily step_backward limit reached (Hobby tier)

**Solutions**:
1. Check quota: use `debugger/quota_status` tool
2. Wait for quota reset (daily/monthly)
3. Upgrade tier at [kindly.software](https://kindly.software)

### Tools not appearing in MCP client

**Causes**:
- Configuration syntax error
- Client cache stale
- Protocol handshake failure

**Solutions**:
1. Validate JSON syntax (use a JSON linter)
2. Restart MCP client
3. Check client logs for MCP errors
4. Verify with: `curl https://mcp.kindly.software/health`

### "Server not initialized" Error

**Causes**:
- Client sent tool request before protocol handshake

**Solutions**:
- Ensure client sends `initialize` before `tools/list` or `tools/call`
- This is typically a client bug; report to client vendor

---

## Security Best Practices

### License Key Protection

1. **Never commit license keys to version control**
   ```bash
   # Add to .gitignore
   .env
   *.env.local
   ```

2. **Use environment variables**
   ```json
   {
     "headers": {
       "X-License-Key": "${KDB_LICENSE_KEY}"
     }
   }
   ```

3. **Rotate keys periodically**
   - Generate new key via [kindly.software](https://kindly.software) dashboard
   - Update environment variable
   - Old key invalidated within 24 hours

### Access Control

1. **Use Observer mode by default**
   - Read-only operations don't require Operator elevation
   - Reduces risk of accidental modifications

2. **Elevate to Operator only when needed**
   - Use `debugger/request_operator_challenge` + `debugger/elevate_to_operator`
   - Revoke with `debugger/revoke_operator` when done

3. **Monitor audit trails**
   - Use `debugger/get_comprehensive_audit` regularly
   - Enterprise tier includes Q34 cryptographic hash-chain

### Network Security

1. **Use HTTPS only**
   - All KDB endpoints use TLS 1.3
   - Never configure HTTP endpoints

2. **Restrict network access**
   - Configure firewall to allow only `mcp.kindly.software:443`
   - Block outbound traffic to unknown MCP servers

---

## Support

- **Documentation**: [kindly.software/docs](https://kindly.software/docs)
- **Email**: [support@kindly.software](mailto:support@kindly.software)
- **Status Page**: [status.kindly.software](https://status.kindly.software)

### Tier-based Support

| Tier | Response Time | Support Channel |
|------|---------------|-----------------|
| Hobby | Best effort | Email |
| Pro | 48 hours | Email |
| Engineer | 24 hours | Email + Priority |
| Teams | 12 hours | Email + Priority + Slack |
| Enterprise | 4 hours | Dedicated + SLA |

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2025-12-10 | Initial release with 27 MCP tools |

---

Need more help? Contact [support@kindly.software](mailto:support@kindly.software)
