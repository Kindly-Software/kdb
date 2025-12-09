# Getting Started with Kindly Debugger

This guide will walk you through setting up Kindly Debugger and running your first debugging session.

## Prerequisites

- An MCP-compatible client (Claude Code, Cursor, or any MCP client)
- Your operating system: macOS, Windows, or Linux (any platform works!)
- A process to debug on a Linux server (the debugger handles this server-side)

## Step 1: Sign Up and Get Your License Key

### Option A: Quick Signup via API

```bash
curl -X POST https://api.kindly.software/api/v1/signup \
  -H "Content-Type: application/json" \
  -d '{"email": "you@example.com"}'
```

You'll receive an Ed25519-signed license key for the **Hobby tier** (free, 5 sessions/month).

**Launch Promo**: During the first 7 days after signup, you get **unlimited sessions**!

### Option B: Website Signup

1. Visit [kindly.software](https://kindly.software)
2. Click **Get Started** or **Sign Up**
3. Enter your email to receive your license key

### License Key Format

Your license key will look like:
```
HOB-2025-12-14-a1b2c3d4...
```

Keep your license key secure and never commit it to version control.

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
    "url": "https://api.kindly.software/mcp",
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
    "url": "https://api.kindly.software/mcp",
    "headers": {
      "Authorization": "Bearer DEV_API_KEY"
    }
  },
  "kindly-debugger-prod": {
    "type": "remote",
    "url": "https://api.kindly.software/mcp",
    "headers": {
      "Authorization": "Bearer PROD_API_KEY"
    }
  }
}
```

## Troubleshooting

### "Connection refused" Error

- Verify your license key is correct
- Check your internet connection
- Ensure the service URL is `https://api.kindly.software/mcp`

### "Permission denied" when attaching

This happens server-side (you don't need to do anything locally):
- The debugger server needs `CAP_SYS_PTRACE` for the target process
- This is handled by our hosted infrastructure

### "Quota exceeded" Error

- Check your current usage with `quota_status` tool
- **Hobby tier**: 5 sessions/month (unlimited during 7-day promo)
- Upgrade to Pro or Enterprise at [kindly.software](https://kindly.software)
- Wait for monthly reset

### Tools not appearing in Claude Code

- Restart Claude Code after configuration changes
- Verify JSON syntax in configuration file (use a JSON validator)
- Check that your license key is valid and not expired
- Try `curl https://api.kindly.software/health` to verify connectivity

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
