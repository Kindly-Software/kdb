# kindly_dedup Stripe Integration - Deployment Guide

## Quick Start (5 minutes)

### 1. Create Stripe Products

Go to Stripe Dashboard → Products:

```
Create 3 Products:

1. Pro License - Early Adopter
   Price: $497.00 USD
   Metadata: tier=pro, early_adopter=true, limit=10

2. Pro License - Regular
   Price: $997.00 USD
   Metadata: tier=pro, early_adopter=false

3. Enterprise License
   No price (contact sales)
```

**Save the price IDs**:
- `price_early_adopter` = (copy from Stripe Dashboard)
- `price_regular` = (copy from Stripe Dashboard)

### 2. Get Stripe API Keys

Dashboard → Developers → API Keys:

```
Publishable key: pk_test_xxxxx...
Secret key:      sk_test_xxxxx...
```

### 3. Create Webhook Signing Secret

Dashboard → Developers → Webhooks → Add endpoint:

```
Endpoint URL: https://your-server.com/webhook/stripe
Events to send:
  ✓ checkout.session.completed
  ✓ payment_intent.succeeded
  ✓ payment_intent.payment_failed
```

**Save the signing secret**:
```
Signing secret: whsec_test_xxxxx...
```

### 4. Deploy Webhook Handler (Fly.io)

```bash
cd /home/samuel/Primitives/kindly_dedup_stripe

# Create Fly.io app (first time only)
fly apps create kindly-dedup-stripe

# Deploy
fly deploy

# Set environment secrets
fly secrets set \
  STRIPE_SECRET_KEY=sk_test_xxxxx \
  STRIPE_WEBHOOK_SECRET=whsec_test_xxxxx \
  STRIPE_PUBLISHABLE_KEY=pk_test_xxxxx

# Verify health check
curl https://kindly-dedup-stripe.fly.dev/health
# Expected: {"status":"ok","service":"kindly_dedup_stripe"}
```

### 5. Update Website

In `/home/samuel/Primitives/kindly-web/`:

```rust
// src/utils/stripe_api.rs
fn get_api_base_url() -> String {
    "https://kindly-dedup-stripe.fly.dev".to_string()
}

// Update price IDs in src/pages/pricing_stripe.rs
stripe_price_id: "price_xxxxx_early_adopter".to_string(),
stripe_price_id: "price_xxxxx_regular".to_string(),
```

### 6. Deploy Website

```bash
cd /home/samuel/Primitives/kindly-web

trunk build --release
fly deploy -a kindly-web
```

### 7. Test Payment Flow

1. Visit https://your-website.com/pricing
2. Click "Buy Pro"
3. Use Stripe test card: `4242 4242 4242 4242`
4. Check success page
5. Verify license email (or check logs: `fly logs -a kindly-dedup-stripe`)

---

## Detailed Setup

### A. Stripe Account Setup

#### 1. Create Stripe Account

```
https://stripe.com
Sign up → Verify email → Complete profile
```

#### 2. Switch to Test Mode

Dashboard top-left: toggle "Test mode"

#### 3. Create Products

**Product 1: Pro Early Adopter**
```
Name: kindly_dedup Pro License - Early Adopter
Type: Service
Description: Unlimited deduplication, lifetime updates

Price:
  Amount: $497.00
  Currency: USD
  Billing period: One-time

Additional settings:
  Metadata key=tier, value=pro
  Metadata key=early_adopter, value=true
  Metadata key=limit, value=10
```

**Product 2: Pro Regular**
```
Name: kindly_dedup Pro License
Type: Service
Description: Unlimited deduplication, lifetime updates

Price:
  Amount: $997.00
  Currency: USD
  Billing period: One-time

Additional settings:
  Metadata key=tier, value=pro
  Metadata key=early_adopter, value=false
```

**Product 3: Enterprise**
```
Name: kindly_dedup Enterprise License
Type: Service
Description: Custom pricing, dedicated support

No price set (use contact form instead)
```

### B. Webhook Handler Deployment

#### 1. Prepare .env File

```bash
# kindly_dedup_stripe/.env
STRIPE_SECRET_KEY=sk_test_xxxxx
STRIPE_WEBHOOK_SECRET=whsec_test_xxxxx
STRIPE_PUBLISHABLE_KEY=pk_test_xxxxx
DATABASE_URL=sqlite:sales.db
SMTP_HOST=smtp.sendgrid.net
SMTP_PORT=587
SMTP_USER=apikey
SMTP_PASSWORD=sg_xxxxx
SMTP_FROM=sales@kindly.software
APP_ENV=test
APP_PORT=3000
RUST_LOG=info,kindly_dedup_stripe=debug
```

#### 2. Test Locally

```bash
cd kindly_dedup_stripe

# Start webhook handler
cargo run --bin stripe_webhook

# In another terminal, start Stripe CLI listener
brew install stripe/stripe-cli/stripe
stripe login

stripe listen --forward-to localhost:3000/webhook/stripe
# Output: Ready! Your webhook signing secret is: whsec_test_xxxxx...

# Update .env with this secret

# Test webhook
curl -X POST http://localhost:3000/webhook/stripe \
  -H "Content-Type: application/json" \
  -d '{"type": "checkout.session.completed", "data": {"object": {"customer_email": "test@example.com"}}}'
```

#### 3. Create Fly.io Configuration

```toml
# kindly_dedup_stripe/fly.toml
app = "kindly-dedup-stripe"
primary_region = "sjc"

[build]
builder = "paketobuildpacks"

[env]
APP_ENV = "production"
APP_PORT = "3000"

[[services]]
internal_port = 3000
processes = ["app"]

[services.http_checks]
enabled = true
grace_period = "5s"
interval = 10000
method = "GET"
min_response_code = 200
path = "/health"
protocol = "http"
timeout = 5000
```

#### 4. Deploy to Fly.io

```bash
cd kindly_dedup_stripe

# First time
fly auth login
fly apps create kindly-dedup-stripe

# Deploy
fly deploy

# Set secrets (Stripe keys)
fly secrets set \
  STRIPE_SECRET_KEY=sk_test_xxxxx \
  STRIPE_WEBHOOK_SECRET=whsec_test_xxxxx

# Verify deployment
fly logs -a kindly-dedup-stripe
fly status -a kindly-dedup-stripe

# Test health endpoint
curl https://kindly-dedup-stripe.fly.dev/health
```

#### 5. Configure Webhook in Stripe

Dashboard → Developers → Webhooks → Add endpoint:

```
Endpoint URL: https://kindly-dedup-stripe.fly.dev/webhook/stripe
Events: checkout.session.completed, payment_intent.succeeded, payment_intent.payment_failed
Version: Latest API version
```

**Get the signing secret** and update Fly.io:

```bash
fly secrets set STRIPE_WEBHOOK_SECRET=whsec_live_xxxxx
fly deploy
```

### C. Website Deployment

#### 1. Update Configuration

```rust
// src/utils/stripe_api.rs
fn get_api_base_url() -> String {
    "https://kindly-dedup-stripe.fly.dev".to_string()
}

// src/pages/pricing_stripe.rs
// Update Stripe price IDs
stripe_price_id: "price_xxxxx_early".to_string(),
stripe_price_id: "price_xxxxx_regular".to_string(),
```

#### 2. Update index.html

```html
<!-- kindly-web/index.html -->
<script src="https://js.stripe.com/v3/"></script>

<!-- In your app script, initialize Stripe -->
<script>
  const stripe = Stripe('pk_test_xxxxx'); // Your publishable key
</script>
```

#### 3. Add Routes

```rust
// src/lib.rs
use leptos_router::*;

#[component]
fn App() -> impl IntoView {
    view! {
        <Router>
            <Routes>
                <Route path="/" view=HomePage />
                <Route path="/pricing" view=PricingPage />
                <Route path="/success" view=SuccessPage />
                <Route path="/cancel" view=CancelPage />
            </Routes>
        </Router>
    }
}
```

#### 4. Deploy

```bash
cd kindly-web

# Build WASM
trunk build --release

# Deploy to Fly.io
fly deploy -a kindly-web

# Set environment
fly secrets set STRIPE_API_BASE_URL=https://kindly-dedup-stripe.fly.dev
fly deploy -a kindly-web

# Verify
fly logs -a kindly-web
```

---

## Testing Checklist

### Pre-Launch Testing

- [ ] Stripe test account created
- [ ] 3 products configured with correct metadata
- [ ] Webhook handler deployed and health check passes
- [ ] Stripe webhook endpoint added and verified
- [ ] Environment variables set correctly
- [ ] Website deployed with price IDs and API base URL updated
- [ ] Early adopter counter initialized to 0
- [ ] Email sending configured (or deferred to Stripe)
- [ ] SSL/TLS certificates valid
- [ ] CORS headers configured correctly

### Payment Flow Testing

- [ ] Visit /pricing page - 2 pricing cards visible
- [ ] Click "Buy Pro" - redirects to Stripe Checkout
- [ ] Enter test card (4242 4242 4242 4242) - payment processes
- [ ] Redirect to /success?session_id=... - page displays correctly
- [ ] Check webhook logs - event received and processed
- [ ] Check email - license key received (or logs show it was attempted)
- [ ] Copy license key - format is KINDLY-PRO-<UUID>
- [ ] Test CLI - kindly_dedup --license-key KINDLY-PRO-... works

### Early Adopter Counter Testing

- [ ] Early adopter counter starts at 0
- [ ] After 1 purchase, counter = 1
- [ ] Badge shows "9 of 10 remaining"
- [ ] After 10 purchases, counter = 10
- [ ] 11th purchase returns 409 error
- [ ] GET /api/early-adopter-remaining returns correct count

### Error Handling Testing

- [ ] Invalid signature → 401 Unauthorized
- [ ] Missing stripe-signature header → 400 Bad Request
- [ ] Malformed JSON → 400 Bad Request
- [ ] Early adopter sold out → 409 Conflict with helpful message
- [ ] Network error in webhook → logged and retried by Stripe
- [ ] Email failure → logged but doesn't fail webhook

### Security Testing

- [ ] Webhook signature verification works
- [ ] Constant-time comparison prevents timing attacks
- [ ] No secrets logged to stdout
- [ ] License keys are unique (no collisions)
- [ ] License format validation in CLI works
- [ ] Tamper detection in LicenseCapsule works

### Performance Testing

- [ ] Webhook processes in < 100ms
- [ ] Counter increment in < 10ns
- [ ] Early adopter page loads in < 2s
- [ ] Checkout button responds immediately
- [ ] Email delivery within 1 minute (or deferred to Stripe)

### Load Testing

```bash
# Simulate 100 concurrent checkout sessions
ab -n 100 -c 10 https://kindly-dedup-stripe.fly.dev/health

# Monitor: fly logs -a kindly-dedup-stripe
```

---

## Troubleshooting

### Webhook Not Receiving Events

```bash
# 1. Verify webhook URL is correct
fly status -a kindly-dedup-stripe

# 2. Check health endpoint
curl https://kindly-dedup-stripe.fly.dev/health

# 3. View logs
fly logs -a kindly-dedup-stripe

# 4. Re-add webhook endpoint in Stripe Dashboard
# (Sometimes Stripe needs a fresh registration)
```

### Signature Verification Failing

```bash
# Verify secret is correct
fly secrets list -a kindly-dedup-stripe

# Check logs for mismatch
fly logs -a kindly-dedup-stripe | grep "Signature"

# Regenerate webhook signing secret in Stripe Dashboard
```

### License Email Not Sending

```bash
# Check if email feature is enabled
grep "email" kindly_dedup_stripe/Cargo.toml

# If not, enable:
cargo run --release --features "email"
fly deploy

# Check SMTP credentials
fly secrets list -a kindly-dedup-stripe | grep SMTP

# Send test email
curl -X POST https://kindly-dedup-stripe.fly.dev/test-email \
  -H "X-Admin-Token: secret" \
  -d '{"email": "test@example.com"}'
```

### Early Adopter Counter Wrong

```bash
# Check database
fly ssh console -a kindly-dedup-stripe
sqlite3 sales.db
SELECT COUNT(*) FROM sales WHERE tier = 'pro';

# Reset if needed (DANGEROUS!)
DELETE FROM sales WHERE id > 10;
```

### Pricing Page Not Showing Counter

```bash
# Check API endpoint
curl https://kindly-dedup-stripe.fly.dev/api/early-adopter-remaining

# Should return:
{"sold": X, "limit": 10, "remaining": Y, "sold_out": false}

# Check frontend logs (browser console)
console.log('Early adopter count:', response);
```

---

## Moving to Production

### 1. Switch to Live Stripe Keys

```bash
# Get live keys from Stripe Dashboard (toggle "Live mode")
# Publishable: pk_live_xxxxx
# Secret: sk_live_xxxxx

fly secrets set \
  STRIPE_SECRET_KEY=sk_live_xxxxx \
  STRIPE_WEBHOOK_SECRET=whsec_live_xxxxx \
  STRIPE_PUBLISHABLE_KEY=pk_live_xxxxx \
  -a kindly-dedup-stripe

fly deploy -a kindly-dedup-stripe
```

### 2. Add Live Webhook Endpoint

Stripe Dashboard → Developers → Webhooks → Add endpoint:

```
Endpoint URL: https://kindly-dedup-stripe.fly.dev/webhook/stripe
Events: checkout.session.completed, payment_intent.succeeded, payment_intent.payment_failed
Live: ✓
```

### 3. Enable Production Monitoring

```bash
# Sentry (error tracking)
fly secrets set SENTRY_DSN=https://...

# Datadog (metrics)
fly secrets set DATADOG_API_KEY=...

# Update code to initialize monitoring
```

### 4. Set Up Alerts

```bash
# Early adopter sold out email
fly alerts add kindly-dedup-stripe \
  --type="log-events" \
  --query="sold_out" \
  --action="email:admin@kindly.software"

# High error rate
fly alerts add kindly-dedup-stripe \
  --type="http-errors" \
  --threshold=5 \
  --window=5m \
  --action="pagerduty:incident"
```

### 5. Test Live Payment

```
1. Visit live website: https://kindly.software/pricing
2. Click "Buy Pro"
3. Use real credit card (or Stripe test card)
4. Verify payment processes and license is delivered
5. Monitor logs in real-time
```

---

## Monitoring & Maintenance

### Daily Checks

```bash
# Health check
curl https://kindly-dedup-stripe.fly.dev/health

# Early adopter count
curl https://kindly-dedup-stripe.fly.dev/api/early-adopter-remaining

# Recent logs
fly logs -a kindly-dedup-stripe -n 50
```

### Weekly Reports

```bash
# Sales count
sqlite3 sales.db <<EOF
SELECT tier, COUNT(*) as count, SUM(amount_cents)/100.0 as revenue
FROM sales
WHERE created_at >= datetime('now', '-7 days')
GROUP BY tier;
EOF

# Error rate
fly logs -a kindly-dedup-stripe | grep -i error | wc -l
```

### Monthly Maintenance

```bash
# Update dependencies
cargo update
cargo audit fix

# Review and rotate secrets
fly secrets list

# Check SSL certificate expiration
curl -vI https://kindly-dedup-stripe.fly.dev

# Archive old logs
```

---

## Backup & Recovery

### Database Backup

```bash
# Scheduled backup (daily)
fly ssh console -a kindly-dedup-stripe <<EOF
sqlite3 sales.db ".backup /tmp/sales-$(date +%Y-%m-%d).db"
# Upload to S3 or Cloud Storage
EOF
```

### Secret Rotation

```bash
# 1. Generate new Stripe webhook secret
# Stripe Dashboard → Webhooks → Rotate

# 2. Update Fly.io secret
fly secrets set STRIPE_WEBHOOK_SECRET=whsec_new_xxxxx

# 3. Deploy
fly deploy
```

---

**[TRADE SECRET]** Deployment procedures are confidential.
