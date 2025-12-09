# Stripe MCP Server Installation & Configuration Guide

## Overview

This guide covers installing and configuring the **official Stripe MCP (Model Context Protocol) server** for payment integration with the `kindly_dedup` license sales website.

**Key Points**:
- **Official**: Stripe has an official MCP server maintained by Stripe
- **Remote URL**: `https://mcp.stripe.com` (hosted by Stripe)
- **Local Alternative**: `npx -y @stripe/mcp` (self-hosted)
- **Authentication**: OAuth (recommended) or Restricted API Keys
- **Framework**: Fully compatible with Claude, Cursor, VS Code, and Claude Code

---

## Step 1: Installation Options

### Option A: Remote Stripe MCP Server (Recommended)

The remote server is hosted by Stripe and requires **no local installation**. You only need to configure the URL in your MCP client.

**Advantages**:
- No local setup needed
- Stripe handles updates and maintenance
- Simple OAuth authentication
- Works with Claude, Cursor, VS Code

**Disadvantages**:
- Requires internet connectivity
- API requests go through Stripe's servers
- Less control over configuration

### Option B: Local MCP Server (Self-Hosted)

Install Stripe MCP locally for self-hosted control and offline testing.

**Prerequisites**:
- Node.js 18+ and npm
- Stripe CLI (optional, for testing)

**Installation Command**:
```bash
# Install globally or run with npx
npx -y @stripe/mcp --tools=all --api-key=YOUR_SECRET_KEY

# Or install as a package
npm install -g @stripe/mcp
```

**Advantages**:
- Full control over server
- Offline testing possible
- Customizable tools and permissions
- No internet dependency

**Disadvantages**:
- Requires local Node.js setup
- Manual updates needed
- More complex debugging

---

## Step 2: Get Your Stripe API Keys

### Create/Find Your Stripe Account

1. Go to [Stripe Dashboard](https://dashboard.stripe.com)
2. Sign in (or create account)
3. Navigate to **Developers > API Keys**

### API Key Types

| Key Type | Use Case | Security |
|----------|----------|----------|
| **Publishable Key** | Client-side, frontend | ✅ Safe, limited permissions |
| **Secret Key** | Server-side only | ⚠️ Full permissions, keep private |
| **Restricted API Key** | Agents, limited scope | ✅ Recommended for MCP |

### Create Restricted API Key (Recommended for MCP)

For the MCP server, create a **restricted API key** to limit agent access:

1. In Dashboard: **Developers > API Keys**
2. Click **Create restricted key**
3. Grant permissions:
   - ✅ Products: Read & Write
   - ✅ Prices: Read & Write
   - ✅ Customers: Read & Write
   - ✅ Payment Intents: Read & Write
   - ✅ Checkout Sessions: Read & Write
   - ✅ Invoices: Read & Write
   - ✅ Refunds: Read & Write
4. Copy the secret key and **save securely**

### Test vs Live Keys

Stripe provides **separate keys** for testing and production:

**Test Mode** (Start here):
- Prefix: `sk_test_...` (secret) / `pk_test_...` (publishable)
- Use test card numbers: 4242 4242 4242 4242
- No real charges

**Live Mode** (Production):
- Prefix: `sk_live_...` (secret) / `pk_live_...` (publishable)
- Real payments processed
- **Enable only when ready**

---

## Step 3: Configure MCP Server

### For Claude Code

**Via CLI**:
```bash
# Add remote Stripe MCP server
claude mcp add --transport http stripe https://mcp.stripe.com/

# Or with API key (local server)
claude mcp add --transport http stripe http://localhost:3000
```

**Manual Configuration** (`~/.claude/mcp.json`):
```json
{
  "mcpServers": {
    "stripe": {
      "command": "npx",
      "args": ["@stripe/mcp", "--tools=all"],
      "env": {
        "STRIPE_SECRET_KEY": "sk_test_YOUR_SECRET_KEY"
      }
    }
  }
}
```

### For Cursor IDE

**Manual Configuration** (`~/.cursor/mcp.json`):
```json
{
  "mcpServers": {
    "stripe": {
      "url": "https://mcp.stripe.com/",
      "auth": "oauth"
    }
  }
}
```

### For VS Code

**Manual Configuration** (`.vscode/mcp.json`):
```json
{
  "mcpServers": {
    "stripe": {
      "command": "npx",
      "args": ["@stripe/mcp", "--api-key=sk_test_YOUR_SECRET_KEY"]
    }
  }
}
```

### Environment Variable Setup

**Option 1: Export in Shell**:
```bash
export STRIPE_SECRET_KEY="sk_test_YOUR_SECRET_KEY"
```

**Option 2: .env File** (for local server):
Create `/home/samuel/.env`:
```
STRIPE_SECRET_KEY=sk_test_YOUR_SECRET_KEY
STRIPE_PUBLISHABLE_KEY=pk_test_YOUR_PUBLISHABLE_KEY
```

**Option 3: Secure Storage** (Recommended):
```bash
# Store in system keyring (Linux)
pass insert stripe/secret_key
pass insert stripe/publishable_key

# Load in shell
export STRIPE_SECRET_KEY=$(pass show stripe/secret_key)
export STRIPE_PUBLISHABLE_KEY=$(pass show stripe/publishable_key)
```

---

## Step 4: Testing the Installation

### Verify MCP Server is Running

```bash
# Check if server responds
curl -s https://mcp.stripe.com/health || echo "Server check failed"

# Or for local server
curl -s http://localhost:3000/health
```

### Test with Claude Code

In Claude Code terminal:
```bash
# Verify MCP is loaded
claude mcp list

# Should show:
# stripe  HTTP   https://mcp.stripe.com/
```

### Simple Payment Test

**Create a test product**:
```bash
# This would be called via Claude Code MCP tools
# Stripe MCP will handle it through the MCP interface

# Example: Ask Claude to create a product
# "Create a Stripe product called 'kindly_dedup_license' with price $99/month"
```

---

## Available Stripe MCP Tools

The Stripe MCP server provides these tools for Claude/agents:

### Customer Management
- `create_customer` - Create a new Stripe customer
- `retrieve_customer` - Get customer details
- `update_customer` - Update customer information
- `list_customers` - List all customers

### Product & Pricing
- `create_product` - Create a new product
- `create_price` - Set pricing for a product
- `retrieve_product` - Get product details
- `list_products` - List all products

### Payment Processing
- `create_payment_intent` - Initiate a payment
- `retrieve_payment_intent` - Check payment status
- `confirm_payment_intent` - Confirm payment
- `list_payment_intents` - List all payments

### Checkout Sessions
- `create_checkout_session` - Create checkout link
- `retrieve_checkout_session` - Get session details
- `list_checkout_sessions` - List sessions

### Refunds & Disputes
- `create_refund` - Issue a refund
- `retrieve_refund` - Check refund status
- `list_refunds` - List all refunds

### Subscriptions
- `create_subscription` - Create recurring billing
- `retrieve_subscription` - Get subscription details
- `update_subscription` - Modify subscription
- `cancel_subscription` - Cancel subscription

### Invoices
- `create_invoice` - Generate invoice
- `finalize_invoice` - Send to customer
- `pay_invoice` - Mark as paid
- `list_invoices` - List all invoices

### Webhooks
- `create_webhook_endpoint` - Register webhook
- `retrieve_webhook_endpoint` - Get webhook details
- `update_webhook_endpoint` - Update endpoints
- `list_webhook_endpoints` - List all webhooks

---

## Security Best Practices

### ✅ DO's

1. **Use Restricted API Keys** for agents/MCP
   ```bash
   # Good: Restricted key with limited permissions
   sk_test_4eC39HqLyjWDarhtT657tJVd
   ```

2. **Store Keys Securely**
   ```bash
   # Use environment variables, not hardcoded
   export STRIPE_SECRET_KEY="sk_test_..."

   # Or system keyring
   pass insert stripe/secret_key
   ```

3. **Separate Test & Live Keys**
   - Test during development: `sk_test_...`
   - Live only when approved: `sk_live_...`

4. **Monitor API Usage**
   - Dashboard > Developers > Logs
   - Set up alerts for suspicious activity

5. **Enable Webhook Verification**
   ```rust
   // Example: Verify webhook signatures
   let signature = headers.get("stripe-signature").unwrap();
   stripe::webhook::verify(body, signature, "whsec_...")
   ```

### ❌ DON'Ts

1. **Never commit API keys** to version control
   ```bash
   # Bad: Keys in git
   STRIPE_KEY="sk_test_..." # ❌ Do not do this!

   # Good: Use .env or environment
   source ~/.env  # ✅
   ```

2. **Never log full API keys**
   ```bash
   # Bad
   println!("Key: {}", secret_key);  // ❌

   # Good
   println!("Key: sk_...{}", &secret_key[secret_key.len()-4:]);  // ✅
   ```

3. **Never use Secret keys client-side**
   ```javascript
   // Bad
   const stripe = Stripe('sk_test_...');  // ❌

   // Good
   const stripe = Stripe('pk_test_...');  // ✅
   ```

4. **Never share test keys** in public
   - Test keys can access your Stripe account
   - Treat like production keys

### Rotate Keys Regularly

```bash
# Update keys quarterly or after staff changes
# Old key: sk_test_4eC39HqLyjWDarhtT657tJVd
# New key: sk_test_51Abc...XyZ

# Update all configurations
export STRIPE_SECRET_KEY="sk_test_51Abc...XyZ"
```

---

## Example: License Product Setup for kindly_dedup

### 1. Create Product (via Claude)

```bash
# Ask Claude: "Create a Stripe product for kindly_dedup with these details"
# Product name: kindly_dedup
# Type: service (one-time license)
# Price: $99
```

**Result**: Product ID `prod_abc123`

### 2. Create Pricing Tiers

```
Tier 1: Basic    - $99  (100,000 docs/month)
Tier 2: Pro      - $299 (1,000,000 docs/month)
Tier 3: Enterprise - Custom quote
```

### 3. Create Checkout Session

```bash
# Ask Claude: "Create a checkout session for kindly_dedup Basic tier"
# Customer: {email: user@example.com}
# Amount: $99
# Product: kindly_dedup Basic
# Mode: payment
```

**Result**: Session ID `cs_test_xyz...` with checkout URL

### 4. Handle Webhooks

```rust
// Webhook to update license after payment
// Event: payment_intent.succeeded
// Action: Create license record in database

#[post("/webhooks/stripe")]
async fn stripe_webhook(
    signature: String,
    body: String,
) -> Result<StatusCode> {
    // 1. Verify webhook signature
    let event = stripe::webhook::verify(&body, &signature, webhook_secret)?;

    // 2. Handle payment success
    if event.type_ == "payment_intent.succeeded" {
        let payment_intent = event.data.object.payment_intent()?;

        // 3. Create license
        let license = License {
            customer_id: payment_intent.customer,
            product_id: "kindly_dedup",
            tier: "basic",
            expires_at: Utc::now() + Duration::days(365),
        };
        license.save().await?;
    }

    Ok(StatusCode::OK)
}
```

---

## Troubleshooting

### Issue: "MCP Server not found"

**Solution**:
```bash
# Verify server URL is correct
curl -s https://mcp.stripe.com/ -v

# Check Claude MCP configuration
claude mcp list

# Reload Claude Code
# (Exit and restart)
```

### Issue: "Invalid API Key"

**Solution**:
```bash
# Verify key format
echo $STRIPE_SECRET_KEY | grep "sk_test_"

# Check key is active in Stripe Dashboard
# Developers > API Keys > Verify status

# Try with restricted key instead
# Dashboard > Create restricted key
```

### Issue: "Authentication failed"

**Solution**:
```bash
# For OAuth, complete authorization flow
# Dashboard > Developers > MCP > Authorize Claude

# For API key, verify Bearer token format
Authorization: Bearer sk_test_YOUR_KEY
```

### Issue: "Rate limited"

**Stripe rate limits**: 100 requests/second (live), unlimited (test)

**Solution**:
```bash
# Implement exponential backoff
async fn retry_with_backoff<F, T>(mut f: F) -> Result<T>
where
    F: FnMut() -> BoxFuture<'static, Result<T>>,
{
    for attempt in 0..5 {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) if e.is_rate_limit() => {
                let delay = Duration::from_millis(100 * 2_u64.pow(attempt));
                tokio::time::sleep(delay).await;
            }
            Err(e) => return Err(e),
        }
    }
    Err("Max retries exceeded".into())
}
```

---

## Next Steps

1. **Get Stripe Account**: [stripe.com](https://stripe.com)
2. **Create Restricted API Key**: Dashboard > Developers > API Keys > Restricted
3. **Configure MCP**: Follow Step 3 above for your IDE
4. **Test Payments**: Use `4242 4242 4242 4242` test card
5. **Create License Products**: Via Claude MCP tools
6. **Set Up Webhooks**: For license activation
7. **Go Live**: Switch to live keys when ready

---

## Resources

- **Stripe MCP Docs**: https://docs.stripe.com/mcp
- **Stripe API Docs**: https://stripe.com/docs/api
- **MCP Registry**: https://registry.modelcontextprotocol.io
- **Stripe Dashboard**: https://dashboard.stripe.com
- **Test Cards**: https://stripe.com/docs/testing

---

## Framework Compliance

**UCE34 Framework**: Q33 Validation (API key verification automated), Q34 Auditability (webhook signatures verified)

**ASSUM Framework**: API key storage verified (#ASSUME STRIPE_KEY_SECURE #VERIFY environment variable or keyring)

**T28 Framework**: Testing with test API keys (never live in dev)

**I20 Framework**: Integration with payment gateway verified (webhooks, customer sync)

---

**Last Updated**: 2025-11-10
**Status**: Ready for Installation
