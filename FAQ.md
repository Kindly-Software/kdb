# Frequently Asked Questions

Common questions about kindly-debugger.

## General

### What is kindly-debugger?

kindly-debugger is a hosted debugging service that integrates with AI coding assistants through the Model Context Protocol (MCP). It provides time-travel debugging, audit trails, and bug pattern detection.

### What is MCP?

The Model Context Protocol is a standard for connecting AI assistants to external tools and data sources. kindly-debugger implements MCP to work seamlessly with Claude Code and other compatible clients.

### Which platforms are supported?

- **Target debugging**: Linux x86_64 (macOS and Windows planned)
- **Clients**: Any MCP-compatible client (Claude Code, etc.)

## Setup

### How do I get an API key?

Visit [kindly.services](https://kindly.services), create an account, and generate a key from the dashboard.

### Where do I put the configuration file?

- **Linux/macOS**: `~/.config/claude-code/mcp_servers.json`
- **Windows**: `%APPDATA%\claude-code\mcp_servers.json`

### Do I need to install anything?

No. kindly-debugger is a hosted service. You only need to configure your MCP client with your API key.

### Can I use kindly-debugger without Claude Code?

Yes. Any MCP-compatible client will work. You can also make direct HTTP requests to the API.

## Debugging

### How does time-travel debugging work?

kindly-debugger records execution snapshots as your program runs. You can step backward through these snapshots to see previous program states.

### What's the limit on execution history?

History depth depends on your plan tier. Check your plan details at [kindly.services](https://kindly.services).

### Can I debug remote processes?

The debugger attaches to processes on the system where kindly-debugger is running. For remote debugging, contact support about enterprise options.

### Does debugging affect my program's performance?

Yes, debugging introduces overhead. The exact impact depends on your usage (breakpoint density, snapshot frequency, etc.).

## Security

### Is my code sent to your servers?

kindly-debugger needs to read memory and registers from debugged processes. We do not store your program code or data beyond the active debugging session.

### Are audit trails secure?

Yes. Audit trails are cryptographically signed and suitable for compliance requirements (SOX, SOC2, GDPR, HIPAA).

### How do I revoke a compromised API key?

Go to **Dashboard** > **API Keys** in [kindly.services](https://kindly.services) and click **Revoke** on the compromised key.

## Pricing & Limits

### Is there a free tier?

Yes. The free tier includes limited API requests per month for evaluation purposes.

### What happens when I hit my quota?

API requests will return a quota exceeded error. You can upgrade your plan or wait for the monthly reset.

### How do I check my usage?

Use the `debugger/quota_status` tool or check the dashboard at [kindly.services](https://kindly.services).

## Troubleshooting

### "Permission denied" when attaching to a process

On Linux, you may need `CAP_SYS_PTRACE` capability or elevated privileges to attach to certain processes.

### Tools not showing in Claude Code

1. Verify your configuration JSON syntax
2. Restart Claude Code
3. Check that your API key is valid

### Slow response times

- Check your internet connection
- Reduce the complexity of queries
- Contact support if issues persist

### "History unavailable" when stepping backward

Time-travel requires execution history. Step forward first to build up history, then step backward.

## Support

### How do I report a bug?

Use the [issue template](.github/ISSUE_TEMPLATE.md) in this repository or email support@kindly.software.

### How do I request a feature?

Email support@kindly.software with your feature request and use case.

### Is there enterprise support?

Yes. Enterprise plans include dedicated support, custom SLAs, and priority access to new features. Contact sales@kindly.software.

---

Can't find your answer? Contact [support@kindly.software](mailto:support@kindly.software)
