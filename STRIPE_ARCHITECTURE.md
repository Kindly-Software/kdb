# kindly_dedup Stripe Integration - Architecture & Design

## System Overview

```
Customer                  kindly-web (Leptos)            kindly_dedup_stripe (Axum)       Stripe
                                 │                                │                          │
    1. Browse pricing ────────────>                              │                          │
                                 │                                │                          │
    2. Click "Buy Pro" ─────────────────────────────────────────────> Create Checkout  ────>│
                                 │<─────────────────────────────────── Session ID  <─────────│
                                 │                                │                          │
    3. Redirect to Stripe ──────────────────────────────────────────────────────────────────>│
       Checkout                  │                                │                  │      │
                                 │                                │                  │      │
    4. Enter payment │                                │                  │      │
       info          │                                │                  │      │
                     │                                │                  │      │
    5. Click Pay     │                                │                  │      │
       Now           │                                │                  │────> Process <─┐│
                     │                                │                  │      Payment  │
                     │                                │<─────────────────────── Success ──┘
                     │                                │
    6. Redirect to   <───────────────────────────────────────────────────> Webhook Event
       success page  │    (checkout.session.completed)│                   │
                     │                                │ (Verify Signature)
                     │                                │ (Generate License)
                     │                                │ (Email License)
                     │                                │ (Update Counter)
                     │                                │
    7. Check email   │                                │
       for license ←─┴────────────────────────────────┘
                     │
    8. Install CLI

    9. Validate
       license ─────────────────> CLI validates against LicenseCapsule
```

## Components

### 1. Stripe Products

Three products configured in Stripe Dashboard:

| Product | Price | Type | Limit |
|---------|-------|------|-------|
| Pro Early Adopter | $497 | One-time | 10 units |
| Pro Regular | $997 | One-time | Unlimited |
| Enterprise | Custom | Contact | Contact |

**Metadata Tags**:
- `tier`: pro, starter, enterprise
- `early_adopter`: true/false
- `limit`: number (for early adopter)

### 2. Webhook Handler (kindly_dedup_stripe)

Rust Axum microservice handling:

**Endpoints**:
- `POST /webhook/stripe` - Stripe webhook receiver (HMAC-SHA256 verified)
- `GET /api/early-adopter-remaining` - Query remaining early adopter slots
- `GET /health` - Health check

**Key Functions**:
1. **Signature Verification** (`signature.rs`)
   - HMAC-SHA256(secret, timestamp.payload)
   - Constant-time comparison (timing attack prevention)
   - Stripe-signature header parsing

2. **License Generation** (`license_service.rs`)
   - Format: `KINDLY-<TIER>-<UUID>`
   - Uses UUID v4 for uniqueness
   - Email delivery (optional)

3. **Early Adopter Counter** (`counter.rs`)
   - AtomicU64-based (T1 Atomic tier)
   - Lockfree, no mutex/RwLock
   - <10ns increment performance
   - CAS loop with retry logic

4. **Event Handling** (`main.rs`)
   - Checkout session completed event
   - Payment intent succeeded/failed tracking
   - Database persistence (optional SQLite)

### 3. Leptos Frontend (kindly-web)

**Pages**:
- `/pricing` - Pricing page with live early adopter counter
- `/success?session_id=...` - Payment confirmation
- `/cancel` - Payment cancelled

**Components**:
- `PricingPage` - Main pricing display
- `CheckoutButton` - Stripe Checkout redirect
- `EarlyAdopterBadge` - Live counter display

**API Integration** (`stripe_api.rs`):
- `GET /api/early-adopter-remaining` - Fetch counter (60-second poll)
- `POST /api/create-checkout-session` - Create Stripe checkout session

### 4. CLI License Integration (kindly_dedup)

**Module**: `src/cli/license.rs`

**Features**:
- License loading from file/env/CLI
- Format validation (KINDLY-<TIER>-<UUID>)
- LicenseCapsule integration
- Usage tracking
- Config file management (~/.kindly-dedup/license.toml)

## Data Flow

### Purchase Flow

```
1. Customer clicks "Buy Pro"
   ↓
2. Frontend calls POST /api/create-checkout-session?price_id=...
   ↓
3. Webhook handler creates Stripe Checkout session
   ↓
4. Frontend redirects to Stripe Checkout (hosted)
   ↓
5. Customer enters payment details on Stripe
   ↓
6. Stripe processes payment
   ↓
7. On success, Stripe redirects to /success?session_id=...
   ↓
8. Stripe sends webhook: checkout.session.completed
   ↓
9. Webhook handler receives event:
   a. Verifies HMAC-SHA256 signature
   b. Parses checkout session data
   c. Generates license key: KINDLY-PRO-<UUID>
   d. Checks early adopter counter (< 10?)
   e. Increments counter atomically
   f. Emails license key to customer
   g. Saves to database (optional)
   ↓
10. Customer receives email with license key
    ↓
11. Customer installs CLI: kindly_dedup --license-key KINDLY-PRO-...
    ↓
12. CLI validates license using LicenseCapsule
```

### Early Adopter Counter Logic

```
Customer purchases early adopter ($497):
  1. Webhook checks counter.get_count() < 10?
  2. If yes:
     a. Generate license key
     b. counter.increment() atomically
     c. Email license
     d. Return success
  3. If no:
     a. Return error: "Early adopter sold out"
     b. Suggest regular pricing ($997)

Counter is persistent (file-based or in-memory with file backup)
```

## Security Considerations

### 1. Webhook Signature Verification

**Critical**: HMAC-SHA256 verification prevents:
- Unauthorized webhook injection
- Payload tampering
- Replay attacks (timestamp validation could be added)

**Implementation**:
```rust
// Parse stripe-signature header
t=1614556800,v1=<hex_signature>

// Compute: HMAC-SHA256(secret, "1614556800.payload")
// Compare with constant-time comparison
```

### 2. License Key Security

**Considerations**:
- License keys are not secrets (can be shared, discussed publicly)
- Validation happens in LicenseCapsule (offline)
- No central license server (lockfree coordination)
- Tamper-proof via checksum (SHA-256)

### 3. API Keys Management

**Best Practices**:
- Store STRIPE_SECRET_KEY in environment variables
- Never commit .env file to git
- Use different keys for test vs. production
- Rotate webhook secrets periodically
- Use Stripe's IP allowlist for webhook endpoints

### 4. Early Adopter Counter

**Thread Safety**:
- AtomicU64 ensures no race conditions
- CAS loop provides linearizability
- No mutex/RwLock (zero deadlock risk)
- Relaxed ordering for performance

## Deployment Architecture

### Local Development

```
$ cargo run --bin stripe_webhook
  Listening on 0.0.0.0:3000

$ stripe listen --forward-to localhost:3000/webhook/stripe
  Webhook signing secret: whsec_test_...

# Configure in Stripe Dashboard test mode
```

### Production (Fly.io)

```
kindly_dedup_stripe/
├── fly.toml
├── Dockerfile
└── .env (secrets managed by Fly.io)

# Deploy
$ fly deploy

# View logs
$ fly logs -a kindly-dedup-stripe

# Set secrets
$ fly secrets set STRIPE_SECRET_KEY=sk_live_... STRIPE_WEBHOOK_SECRET=whsec_...
```

### Website (Fly.io)

```
kindly-web/
├── fly.toml
├── Leptos frontend
└── Environment: STRIPE_API_BASE_URL=https://stripe-webhook.kindly.software

# Build & deploy
$ trunk build --release
$ fly deploy
```

## Environment Variables

### Webhook Handler
```bash
STRIPE_SECRET_KEY=sk_test_...          # Stripe API key
STRIPE_WEBHOOK_SECRET=whsec_...        # Webhook signing secret
STRIPE_PUBLISHABLE_KEY=pk_test_...     # (optional, for frontend)
DATABASE_URL=sqlite:sales.db           # SQLite persistence
SMTP_HOST=smtp.sendgrid.net            # Email config (optional)
SMTP_PASSWORD=sg_...
APP_PORT=3000
RUST_LOG=info
```

### Frontend
```bash
STRIPE_PUBLISHABLE_KEY=pk_test_...     # Client-side Stripe.js
STRIPE_API_BASE_URL=http://localhost:3000  # Webhook handler
```

## Error Handling

### Webhook Errors

```
Request Error          → 400 Bad Request (malformed JSON)
Invalid Signature      → 401 Unauthorized (signature mismatch)
Early Adopter Sold Out → 409 Conflict (quota reached)
Server Error           → 500 Internal Server Error (logged)
```

### CLI Errors

```
License Not Found      → Prompt to provide via --license-key, env, or config file
Invalid Format         → "License key format: KINDLY-<TIER>-<UUID>"
License Expired        → "License expired, renewal required"
License Revoked        → "License revoked, contact support"
Limit Exceeded         → "GB limit exceeded for license tier"
```

## Performance Characteristics

| Operation | Latency | Notes |
|-----------|---------|-------|
| Webhook processing | <100ms | Verify sig + generate key + email |
| Counter increment | <10ns | Atomic CAS, typically 1 attempt |
| License validation (CLI) | <5ns | Atomic relaxed load |
| Usage recording | <10ns | Atomic CAS with retry |
| License creation | <1ms | SHA-256 hashing |

## Monitoring & Observability

### Key Metrics

1. **Sales Metrics**
   - Early adopter units sold (0-10)
   - Regular units sold
   - Revenue ($)
   - Conversion rate (checkout started → completed)

2. **System Metrics**
   - Webhook latency (ms)
   - Webhook error rate (%)
   - Counter increment success rate (%)
   - Email delivery rate (%)

3. **Alerts**
   - Early adopter sold out (alert via email)
   - Webhook signature errors (security incident)
   - Database errors (persistence issue)
   - High webhook latency (>500ms)

### Logging

```rust
info!("Received Stripe event: {} ({})", event.type_, event.id);
info!("Generated license key for {}: {}", customer_email, license_key);
error!("Webhook signature verification failed");
warn!("Early adopter limit reached");
```

## Testing

### Unit Tests

```bash
cargo test --lib
```

Tests cover:
- Signature verification (valid/invalid)
- License key generation (uniqueness, format)
- Counter operations (increment, limit, concurrent)
- Early adopter logic

### Integration Tests

```bash
# With Stripe test mode
stripe listen --forward-to localhost:3000/webhook/stripe

curl -X POST http://localhost:3000/webhook/stripe \
  -H "stripe-signature: t=...,v1=..." \
  -d '{"type": "checkout.session.completed", ...}'
```

### End-to-End Tests

```
1. Visit /pricing page
2. Click "Buy Pro"
3. Complete Stripe checkout (test card: 4242 4242 4242 4242)
4. Verify success page
5. Check email for license key
6. Install CLI and validate license
```

## Rollback & Recovery

### If Early Adopter Counter Gets Out of Sync

```bash
# Reset counter (only for emergencies)
$ ssh kindly-dedup-stripe
$ sqlite3 sales.db
> SELECT COUNT(*) FROM sales WHERE tier = 'pro' AND early_adopter = true;
> DELETE FROM sales WHERE id = X;  -- Remove erroneous sale

# Restart service
$ fly deploy
```

### If License Generation Breaks

```bash
# All licenses already issued are valid (they're stateless)
# Just fix code and redeploy
$ fly deploy

# Manually generate license for failed customer
$ cargo run --bin generate_license -- PRO CUSTOMER_EMAIL
```

## Future Enhancements

1. **Subscription Licenses** - Support recurring billing (annual)
2. **License Revocation API** - Remote revocation capability
3. **Usage Analytics** - Dashboard showing dedup statistics per customer
4. **Team Licenses** - Multi-user licenses with usage pooling
5. **Custom License Tiers** - Enterprise custom pricing/features
6. **License Marketplace** - Secondary market for license resale (with restrictions)

---

**[TRADE SECRET]** This implementation protects critical business logic. All commits must include [TRADE SECRET] tag.
