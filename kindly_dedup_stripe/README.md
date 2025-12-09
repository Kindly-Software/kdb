# kindly_dedup Stripe Webhook Handler

[TRADE SECRET] Stripe payment processing for kindly_dedup license sales.

## Overview

Rust Axum microservice handling Stripe webhook events for one-time license purchases. Generates license keys, tracks early adopter sales, and manages customer information.

**Type**: T1 Atomic coordination (lockfree early adopter counter)
**Performance**: <100ms webhook processing, <10ns counter increment
**Framework**: UCE34 (Q33/Q34), ASSUM (99.99%), Chaos (100% lockfree)

## Features

- ✅ HMAC-SHA256 webhook signature verification (security-critical)
- ✅ License key generation (format: KINDLY-<TIER>-<UUID>)
- ✅ Atomic early adopter counter (0-10 units, lockfree)
- ✅ Email delivery (SendGrid optional)
- ✅ SQLite persistence (sales records)
- ✅ Production-ready error handling
- ✅ Health check endpoint

## Quick Start

### 1. Set Up Environment

```bash
cp .env.example .env

# Fill in Stripe keys from Dashboard
# STRIPE_SECRET_KEY=sk_test_...
# STRIPE_WEBHOOK_SECRET=whsec_...
```

### 2. Build & Run Locally

```bash
cargo run --bin stripe_webhook

# Test health
curl http://localhost:3000/health

# Test early adopter counter
curl http://localhost:3000/api/early-adopter-remaining
```

### 3. Deploy to Fly.io

```bash
fly deploy

# Set secrets
fly secrets set STRIPE_SECRET_KEY=sk_test_... STRIPE_WEBHOOK_SECRET=whsec_...

# Monitor
fly logs -a kindly-dedup-stripe
```

## Project Structure

```
kindly_dedup_stripe/
├── Cargo.toml
├── src/
│   ├── main.rs                 # Axum server, event handlers
│   ├── signature.rs            # HMAC-SHA256 verification
│   ├── license_service.rs      # License key generation
│   ├── counter.rs              # T1 Atomic counter (lockfree)
│   ├── error.rs                # API error types
│   └── db.rs                   # SQLite operations
├── fly.toml                    # Fly.io deployment config
├── .env.example                # Configuration template
└── README.md
```

## API Endpoints

### POST /webhook/stripe

Receives Stripe webhook events.

**Headers**:
```
stripe-signature: t=<timestamp>,v1=<signature>
Content-Type: application/json
```

**Body** (example):
```json
{
  "type": "checkout.session.completed",
  "data": {
    "object": {
      "id": "cs_test_...",
      "customer_email": "customer@example.com",
      "line_items": {
        "data": [{
          "price": {"product": "pro", "metadata": {"tier": "pro"}}
        }]
      }
    }
  }
}
```

**Response**:
```json
{
  "success": true,
  "message": "License generated successfully",
  "event_id": "evt_...",
  "license_key": "KINDLY-PRO-550e8400-e29b-41d4-a716-446655440000"
}
```

**Status Codes**:
- `200 OK` - Event processed successfully
- `400 Bad Request` - Malformed JSON or missing fields
- `401 Unauthorized` - Invalid signature
- `409 Conflict` - Early adopter quota reached
- `500 Internal Server Error` - Server error

### GET /api/early-adopter-remaining

Get remaining early adopter slots.

**Response**:
```json
{
  "sold": 5,
  "limit": 10,
  "remaining": 5,
  "sold_out": false
}
```

### GET /health

Health check endpoint.

**Response**:
```json
{
  "status": "ok",
  "service": "kindly_dedup_stripe"
}
```

## Configuration

### Environment Variables

```bash
# Stripe (required)
STRIPE_SECRET_KEY=sk_test_...
STRIPE_WEBHOOK_SECRET=whsec_...
STRIPE_PUBLISHABLE_KEY=pk_test_...

# Application
APP_ENV=test                    # test or production
APP_PORT=3000
APP_HOST=0.0.0.0

# Database (optional)
DATABASE_URL=sqlite:sales.db

# Email (optional)
SMTP_HOST=smtp.sendgrid.net
SMTP_PORT=587
SMTP_USER=apikey
SMTP_PASSWORD=sg_...
SMTP_FROM=sales@kindly.software

# Logging
RUST_LOG=info
```

## Features

### Default Features

```toml
default = ["sqlite"]
```

### Optional Features

- `sqlite`: SQLite persistence (enabled by default)
- `email`: Email delivery via SMTP
- `full`: All features

**Enable features**:
```bash
cargo run --features "sqlite,email"
```

## Performance

| Operation | Latency | Notes |
|-----------|---------|-------|
| Webhook verification | <5ms | HMAC-SHA256 + signature parse |
| License generation | <1ms | UUID v4 creation |
| Counter increment | <10ns | Atomic CAS, 1-2 attempts |
| Email send | <100ms | SendGrid API call (optional) |
| Database save | <10ms | SQLite write |
| **Total webhook** | **<100ms** | All operations combined |

## Security

### Signature Verification

Every webhook is verified with HMAC-SHA256:

```rust
// Parse header: t=<timestamp>,v1=<signature>
// Compute: HMAC-SHA256(secret, "timestamp.payload")
// Compare with constant-time function (timing attack prevention)
```

### No Secrets in Logs

- Never log STRIPE_SECRET_KEY
- Never log STRIPE_WEBHOOK_SECRET
- Safe to enable RUST_LOG=debug

### License Key Security

License keys are:
- Unique (UUID v4)
- Stateless (validate offline in CLI)
- Tamper-proof (checksum in LicenseCapsule)
- Non-guessable (2^128 entropy)

## Testing

### Unit Tests

```bash
cargo test --lib

# Test signature verification
# Test license key generation
# Test counter operations
# Test early adopter logic
```

### Integration Tests

```bash
# Start webhook handler
cargo run --bin stripe_webhook &

# Start Stripe CLI listener
stripe listen --forward-to localhost:3000/webhook/stripe

# Simulate webhook event
curl -X POST http://localhost:3000/webhook/stripe \
  -H "stripe-signature: t=...,v1=..." \
  -d '{"type": "checkout.session.completed", ...}'
```

### End-to-End Tests

```bash
1. Visit pricing page
2. Click "Buy Pro"
3. Complete checkout with test card (4242 4242 4242 4242)
4. Check success page
5. Verify license email
6. Test CLI with license key
```

## Monitoring

### Key Metrics

```
- Webhook processing latency (ms)
- Webhook error rate (%)
- Early adopter units sold (0-10)
- Counter increment success rate (%)
- Email delivery rate (%)
```

### Logs

```bash
# View logs
fly logs -a kindly-dedup-stripe

# Filter by level
fly logs -a kindly-dedup-stripe | grep ERROR
fly logs -a kindly-dedup-stripe | grep WARN

# Follow logs in real-time
fly logs -a kindly-dedup-stripe -n 50 -f
```

### Alerts

```bash
# Set up alerts in Fly.io
fly alerts add kindly-dedup-stripe \
  --type="log-events" \
  --query="error" \
  --action="email:admin@kindly.software"
```

## Troubleshooting

### Webhook Not Receiving Events

1. Check endpoint URL is correct: `fly status -a kindly-dedup-stripe`
2. Test health: `curl https://kindly-dedup-stripe.fly.dev/health`
3. View logs: `fly logs -a kindly-dedup-stripe`
4. Re-add webhook in Stripe Dashboard if needed

### Signature Verification Failing

1. Verify secret matches: `fly secrets list`
2. Check logs for mismatch error
3. Regenerate webhook secret in Stripe Dashboard

### Email Not Sending

1. Enable email feature: `cargo run --features "email"`
2. Check SMTP credentials: `fly secrets list`
3. View email logs: `fly logs | grep -i email`

## Development

### Adding New Event Types

In `main.rs`, add to the match statement:

```rust
match event.type_.as_str() {
    "checkout.session.completed" => { ... }
    "payment_intent.succeeded" => { ... }
    "your_event_type" => handle_your_event(state, event).await,
    _ => { ... }
}
```

### Modifying License Tier System

Update `LicenseTier` enum in `kindly_dedup/license_capsule.rs`:

```rust
pub enum LicenseTier {
    Trial = 1,
    Starter = 2,
    Pro = 3,
    Enterprise = 4,
    // Add new tier
    Premium = 5,
}
```

### Performance Optimization

Profile with flamegraph:

```bash
cargo install flamegraph
cargo flamegraph --bin stripe_webhook -- --example-arg

# View: flamegraph.svg
```

## Deployment

### Local Development

```bash
cargo run --bin stripe_webhook
# Listens on 0.0.0.0:3000
```

### Fly.io Production

```bash
fly deploy
fly logs -a kindly-dedup-stripe
fly status -a kindly-dedup-stripe
```

### Docker

```bash
docker build -t kindly-dedup-stripe:latest .
docker run -p 3000:3000 -e STRIPE_SECRET_KEY=... kindly-dedup-stripe:latest
```

## References

- **Architecture**: See `STRIPE_ARCHITECTURE.md`
- **Deployment**: See `STRIPE_DEPLOYMENT_GUIDE.md`
- **Pricing**: See `STRIPE_PRICING_STRATEGY.md`
- **License System**: See `kindly_dedup/src/license_capsule.rs`
- **Atomic Capsule**: See `atomic_capsule/CLAUDE.md` (T1 tier)

## License

[TRADE SECRET] - Proprietary code. All rights reserved.

## Support

Email: support@kindly.software
