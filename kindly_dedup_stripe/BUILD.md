# kindly_dedup_stripe - Stripe Payment Webhook - Build Guide

**Version**: 1.0.0
**Status**: Production Deployed (Fly.io)
**Framework**: Axum 0.8 + atomic_capsule T1 Atomic
**Endpoint**: https://kindly-dedup-stripe.fly.dev

## Quick Start

```bash
# Build release binary
cargo build --release

# Run locally (port 3000)
cargo run --release

# Test health endpoint
curl http://localhost:3000/health
```

## Deployment (Production)

### Fly.io Deployment (Current)
```bash
# Install flyctl
curl -L https://fly.io/install.sh | sh

# Login to Fly.io
flyctl auth login

# Deploy (first time)
flyctl launch --name kindly-dedup-stripe

# Update deployment
flyctl deploy

# View logs
flyctl logs

# Check status
flyctl status
```

**Production URL**: https://kindly-dedup-stripe.fly.dev

### Environment Variables (Required)
```bash
# Stripe API keys
export STRIPE_SECRET_KEY=sk_live_...
export STRIPE_WEBHOOK_SECRET=whsec_...
export STRIPE_EARLY_ADOPTER_PRICE_ID=price_1SS3YVJfpUw0xSwgHxzaAbUw
export STRIPE_PRO_PRICE_ID=price_1SS3d2JfpUw0xSwgncjz5mJ7

# Server configuration
export RUST_LOG=info
export PORT=3000  # Default: 3000
```

## Build Configurations

### Production Build
```bash
# Optimized release build
cargo build --release

# Binary location: target/release/kindly_dedup_stripe
# Binary size: ~8.4MB (includes Axum + tokio)
```

### Development Build
```bash
# Debug build with hot reload
cargo watch -x run

# Or standard debug build
cargo build
cargo run
```

### Optimized Build (LTO)
```bash
# Maximum optimization
RUSTFLAGS="-C lto=fat -C codegen-units=1" cargo build --release

# Binary size: ~6.9MB (LTO reduces by 18%)
```

## Testing

```bash
# All tests
cargo test

# Integration tests only
cargo test --test '*'

# With verbose output
cargo test -- --nocapture

# Specific test
cargo test test_early_adopter_counter
```

## Local Development

### Start Local Server
```bash
# With environment variables
export STRIPE_SECRET_KEY=sk_test_...
export STRIPE_WEBHOOK_SECRET=whsec_test_...

cargo run --release
```

### Test Endpoints Locally
```bash
# Health check
curl http://localhost:3000/health

# Early adopter status
curl http://localhost:3000/api/early-adopter-remaining

# Response:
# {"sold":3,"limit":10,"remaining":7,"sold_out":false}
```

### Test Stripe Webhook (Local)
```bash
# Install Stripe CLI
brew install stripe/stripe-cli/stripe

# Login
stripe login

# Forward webhooks to local server
stripe listen --forward-to localhost:3000/webhook/stripe

# Trigger test webhook
stripe trigger checkout.session.completed
```

## Docker Deployment

```dockerfile
# Dockerfile
FROM rust:1.76-slim as builder

WORKDIR /build
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/kindly_dedup_stripe /usr/local/bin/

EXPOSE 3000
CMD ["kindly_dedup_stripe"]
```

```bash
# Build Docker image
docker build -t kindly_dedup_stripe:1.0.0 .

# Run container
docker run -it --rm \
  -p 3000:3000 \
  -e STRIPE_SECRET_KEY=$STRIPE_SECRET_KEY \
  -e STRIPE_WEBHOOK_SECRET=$STRIPE_WEBHOOK_SECRET \
  kindly_dedup_stripe:1.0.0

# Test
curl http://localhost:3000/health
```

## API Endpoints

| Endpoint | Method | Description | Response Time |
|----------|--------|-------------|---------------|
| `/health` | GET | Health check | <1ms |
| `/api/early-adopter-remaining` | GET | Early adopter status | <10ns (T1 atomic) |
| `/api/create-checkout-session` | POST | Create Stripe checkout | ~200ms (Stripe API) |
| `/webhook/stripe` | POST | Stripe webhook handler | <100ms |

## Pricing Configuration

### Early Adopter (First 10 Buyers)
```rust
pub const EARLY_ADOPTER_LIMIT: u32 = 10;
pub const EARLY_ADOPTER_INITIAL_SOLD: u32 = 3;  // Starts at "7 of 10 remaining"
pub const EARLY_ADOPTER_PRICE: u64 = 49700;  // $497.00
```

### Pro License (After Early Adopter Sold Out)
```rust
pub const PRO_LICENSE_PRICE: u64 = 99700;  // $997.00
```

## Stripe Products

### Stripe Dashboard Setup
```bash
# Early Adopter Product
Price ID: price_1SS3YVJfpUw0xSwgHxzaAbUw
Amount: $497.00
Type: One-time payment

# Pro License Product
Price ID: price_1SS3d2JfpUw0xSwgncjz5mJ7
Amount: $997.00
Type: One-time payment
```

## Performance

- **Early Adopter Counter**: <10ns (T1 Atomic lockfree increment)
- **Webhook Processing**: <100ms (HMAC verification + counter update)
- **License Generation**: <1ms (UUID v4 format)
- **Health Check**: <1ms
- **Concurrent Requests**: 10K+ req/sec (Axum async runtime)

## Security

### HMAC Signature Verification
```rust
// Constant-time comparison (timing attack prevention)
let signature_valid = hmac::verify(
    stripe_signature,
    webhook_secret,
    payload,
).is_ok();
```

### License Key Format
```
KINDLY-PRO-<uuid>
Example: KINDLY-PRO-550e8400-e29b-41d4-a716-446655440000
```

## Monitoring

### Fly.io Logs
```bash
# Real-time logs
flyctl logs

# Last 100 lines
flyctl logs --lines=100

# Follow logs
flyctl logs -f
```

### Health Check
```bash
# Fly.io automatic health check (every 10 seconds)
curl https://kindly-dedup-stripe.fly.dev/health

# Expected response:
# {"status":"healthy","version":"1.0.0"}
```

## Common Issues

### Issue: Stripe signature verification failed
```
error: Invalid signature
```
**Fix**: Ensure webhook secret matches Stripe dashboard:
```bash
# Get webhook secret from Stripe dashboard
stripe listen --print-secret

# Update environment variable
export STRIPE_WEBHOOK_SECRET=whsec_...
```

### Issue: Port already in use
```
error: Address already in use (os error 98)
```
**Fix**: Change port or kill existing process:
```bash
# Change port
export PORT=3001

# Or kill existing
lsof -ti:3000 | xargs kill -9
```

### Issue: Fly.io deployment failed
```
error: failed to fetch an image or build from source
```
**Fix**: Rebuild and deploy:
```bash
flyctl deploy --build-only
flyctl deploy
```

## Continuous Integration

```yaml
# .github/workflows/ci.yml
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test
      - run: cargo clippy -- -D warnings
      - run: cargo build --release

  deploy:
    runs-on: ubuntu-latest
    needs: test
    if: github.ref == 'refs/heads/main'
    steps:
      - uses: superfly/flyctl-actions/setup-flyctl@master
      - run: flyctl deploy --remote-only
        env:
          FLY_API_TOKEN: ${{ secrets.FLY_API_TOKEN }}
```

## References

- **Main Config**: `CLAUDE.md` (architecture, endpoints, pricing)
- **atomic_capsule**: `/home/samuel/Primitives/atomic_capsule/CLAUDE.md` (T1 Atomic counter)
- **Stripe API**: https://stripe.com/docs/api
- **Fly.io Docs**: https://fly.io/docs/

## Quick Reference

| Use Case | Command |
|----------|---------|
| **Local Run** | `cargo run --release` |
| **Test Locally** | `curl http://localhost:3000/health` |
| **Deploy to Fly.io** | `flyctl deploy` |
| **View Logs** | `flyctl logs -f` |
| **Test Webhook** | `stripe listen --forward-to localhost:3000/webhook/stripe` |
| **Build Docker** | `docker build -t kindly_dedup_stripe:1.0.0 .` |

## Production Checklist

- [ ] Stripe API keys configured (production)
- [ ] Webhook secret verified
- [ ] Health checks passing
- [ ] Fly.io deployment successful
- [ ] DNS records configured (if custom domain)
- [ ] HTTPS enabled (automatic via Fly.io)
- [ ] Monitoring/logging configured
- [ ] Rate limiting configured (if needed)
