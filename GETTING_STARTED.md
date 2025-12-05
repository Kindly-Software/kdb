# Getting Started with kindly-debugger

This guide will walk you through setting up kindly-debugger and running your first debugging session.

## Prerequisites

- An MCP-compatible client (Claude Code recommended)
- A process to debug (Linux x86_64)
- An API key from [kindly.services](https://kindly.services)

## Step 1: Get Your API Key

1. Visit [kindly.services](https://kindly.services)
2. Create an account or sign in
3. Navigate to **Dashboard** > **API Keys**
4. Click **Generate New Key**
5. Copy your API key (it will only be shown once)

Keep your API key secure and never commit it to version control.

## Step 2: Configure Claude Code

Add kindly-debugger to your MCP servers configuration.

### Configuration File Location

- **Linux/macOS**: `~/.config/claude-code/mcp_servers.json`
- **Windows**: `%APPDATA%\claude-code\mcp_servers.json`

### Add the Configuration

Edit the configuration file and add the kindly-debugger entry:

```json
{
  "kindly-debugger": {
    "type": "remote",
    "url": "https://api.kindly.services/mcp",
    "headers": {
      "Authorization": "Bearer YOUR_API_KEY_HERE"
    }
  }
}
```

Replace `YOUR_API_KEY_HERE` with your actual API key.

### Verify Configuration

Restart Claude Code and verify the connection:

```
Claude: Can you check my kindly-debugger connection?
```

You should see a confirmation that kindly-debugger tools are available.

## Step 3: Test the Connection

Verify your setup with a simple quota check:

```
Claude: Check my kindly-debugger quota status
```

Expected response:
```
Quota Status:
  Plan: [Your Plan]
  Used: X / Y requests
  Remaining: Z requests
```

## Step 4: Your First Debugging Session

Let's debug a simple process. First, start a test program:

```bash
# Terminal 1: Start a simple test process
sleep 3600 &
echo "PID: $!"
```

Note the PID (process ID) that's printed.

### Attach to the Process

```
Claude: Attach kindly-debugger to process [PID]
```

### Set a Breakpoint

```
Claude: Set a breakpoint at the main entry point
```

### Explore the Stack

```
Claude: Show me the current stack trace
```

### Inspect Variables

```
Claude: What are the current local variables?
```

### Time-Travel Debugging

```
Claude: Step forward 5 instructions, then step back 2
```

### Detach

When you're done:

```
Claude: Detach from the process
```

## Step 5: Enable Audit Trails (Optional)

For compliance requirements, you can retrieve audit logs:

```
Claude: Get the audit trail for today's debugging sessions
```

This returns cryptographically signed records of all debugging actions.

## Common Workflows

### Crash Investigation

1. Attach to a crashed process (if core dump available)
2. Get the stack trace to see where the crash occurred
3. Use time-travel to step backward from the crash
4. Inspect variables to find the root cause

### Breakpoint Debugging

1. Set breakpoints at suspicious locations
2. Continue execution until breakpoint hit
3. Inspect variables and stack
4. Step through code to observe behavior

### Bug Pattern Detection

1. After finding a bug, use `find_similar_bugs`
2. Review the list of similar patterns
3. Fix all related issues systematically

## Configuration Options

### Environment Variables

You can also configure the API key via environment variable:

```bash
export KDB_MCP_API_KEY="your_api_key"
```

### Multiple Environments

For different environments (dev/staging/prod), use separate API keys:

```json
{
  "kindly-debugger-dev": {
    "type": "remote",
    "url": "https://api.kindly.services/mcp",
    "headers": {
      "Authorization": "Bearer DEV_API_KEY"
    }
  },
  "kindly-debugger-prod": {
    "type": "remote",
    "url": "https://api.kindly.services/mcp",
    "headers": {
      "Authorization": "Bearer PROD_API_KEY"
    }
  }
}
```

## Troubleshooting

### "Connection refused" Error

- Verify your API key is correct
- Check your internet connection
- Ensure the service URL is correct

### "Permission denied" when attaching

- On Linux, you may need `CAP_SYS_PTRACE` capability
- Or run with elevated privileges for system processes

### "Quota exceeded" Error

- Check your current usage with `quota_status`
- Upgrade your plan at [kindly.services](https://kindly.services)
- Wait for quota reset (monthly)

### Tools not appearing in Claude Code

- Restart Claude Code after configuration changes
- Verify JSON syntax in configuration file
- Check that the API key has correct permissions

## Next Steps

- Read the [Tools Reference](TOOLS.md) for detailed documentation
- Review [Authentication](AUTHENTICATION.md) for security best practices
- Check the [FAQ](FAQ.md) for common questions

## Getting Help

- **Documentation**: This repository
- **Email**: support@kindly.software
- **Issues**: [GitHub Issues](.github/ISSUE_TEMPLATE.md)

---

Need more help? Contact [support@kindly.software](mailto:support@kindly.software)
