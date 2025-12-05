# Authentication

Guide to API key authentication and security for kindly-debugger.

## API Key Authentication

kindly-debugger uses API key authentication. All requests must include a valid API key.

### Obtaining an API Key

1. Visit [kindly.services](https://kindly.services)
2. Create an account or sign in
3. Navigate to **Dashboard** > **API Keys**
4. Click **Generate New Key**
5. Optionally, add a description and set permissions
6. Copy the key immediately (it will only be shown once)

### Using Your API Key

Include the API key in the `Authorization` header:

```
Authorization: Bearer YOUR_API_KEY
```

### Configuration Example

In Claude Code's `mcp_servers.json`:

```json
{
  "kindly-debugger": {
    "type": "remote",
    "url": "https://api.kindly.services/mcp",
    "headers": {
      "Authorization": "Bearer kdb_live_a1b2c3d4e5f6g7h8i9j0"
    }
  }
}
```

## API Key Formats

| Prefix | Environment | Usage |
|--------|-------------|-------|
| `kdb_live_` | Production | Live debugging sessions |
| `kdb_test_` | Testing | Testing and development |

## Rate Limits

Rate limits vary by plan tier:

| Tier | Requests/Month | Burst Rate |
|------|---------------|------------|
| Free | 100 | 10/minute |
| Developer | 10,000 | 60/minute |
| Team | 100,000 | 300/minute |
| Enterprise | Custom | Custom |

For current pricing and limits, visit [kindly.services](https://kindly.services).

### Rate Limit Headers

Responses include rate limit information:

```
X-RateLimit-Limit: 10000
X-RateLimit-Remaining: 9500
X-RateLimit-Reset: 1733097600
```

### Handling Rate Limits

When rate limited, you'll receive:

```json
{
  "error": {
    "code": -32002,
    "message": "Quota exceeded",
    "data": {
      "limit": 10000,
      "used": 10000,
      "reset_at": "2024-12-01T00:00:00Z"
    }
  }
}
```

Wait until the reset time or upgrade your plan.

## Security Best Practices

### Do

- Store API keys in environment variables or secure vaults
- Use different keys for development and production
- Rotate keys periodically
- Monitor usage through the dashboard
- Revoke compromised keys immediately

### Don't

- Commit API keys to version control
- Share keys between team members (use team accounts)
- Log API keys in application logs
- Embed keys in client-side code
- Use production keys for testing

## Key Management

### Viewing Keys

In the dashboard, you can:
- See all active keys
- View last used timestamp
- See usage statistics

### Rotating Keys

1. Generate a new key
2. Update your configuration
3. Verify the new key works
4. Revoke the old key

### Revoking Keys

Immediately revoke compromised keys:
1. Go to **Dashboard** > **API Keys**
2. Find the key to revoke
3. Click **Revoke**
4. Confirm the action

Revocation is immediate and cannot be undone.

## Permissions

API keys can have restricted permissions:

| Permission | Description |
|------------|-------------|
| `debug:read` | Read debugging state (stack, variables) |
| `debug:write` | Modify state (breakpoints, stepping) |
| `audit:read` | Access audit trails |
| `document:read` | Query XML documents |
| `document:write` | Modify document cache |

Default keys have all permissions. Create restricted keys for specific use cases.

## IP Allowlisting

Enterprise plans support IP allowlisting:

1. Go to **Dashboard** > **Security**
2. Add allowed IP ranges (CIDR notation)
3. Enable allowlisting

Requests from non-allowed IPs will be rejected.

## Audit Logging

All API key usage is logged:

- Request timestamp
- Client IP address
- Tool called
- Success/failure status

View audit logs in the dashboard or via the `get_comprehensive_audit` tool.

## Multi-User Access

### Team Accounts

Team plans support multiple users:
- Each user gets their own API key
- Usage is aggregated for billing
- Admins can view all team usage

### Service Accounts

For CI/CD and automated systems:
1. Create a service account
2. Generate a key for the service account
3. Use descriptive names for tracking

## Troubleshooting

### "Authentication failed" Error

- Verify the key is correct (no extra spaces)
- Check the key hasn't been revoked
- Ensure the `Bearer ` prefix is included
- Confirm the key format matches the environment

### "Permission denied" Error

- Check the key has required permissions
- Verify IP allowlisting (if enabled)
- Contact support if permissions seem correct

### Key Not Working After Generation

- Keys are active immediately
- Try regenerating if issues persist
- Check for copy/paste errors

## Support

For authentication issues:
- Email: support@kindly.software
- Include your account email (not your API key!)

---

For complete documentation, return to the [README](README.md).
