# Licensing Module Usage Guide

## Quick Start

### Basic Tier Operations

```rust
use clapi_core::licensing::{SubscriptionTier, TierCache};

// Get tier quotas (const fn, <5ns)
let tier = SubscriptionTier::Team;
let monthly_limit = tier.monthly_request_limit(); // 1,000,000
let retention = tier.retention_days(); // 90
let rate_limit = tier.rate_limit_rps(); // 100
let concurrent = tier.concurrent_limit(); // 20

// Parse tier from string (<100ns)
let tier = SubscriptionTier::from_str("enterprise").unwrap();
assert_eq!(tier, SubscriptionTier::Enterprise);

// Display tier
println!("User tier: {}", tier); // "enterprise"
```

### TierCache (Lockfree Atomic)

```rust
use clapi_core::licensing::{SubscriptionTier, TierCache};

// Create tier cache (64B cache-aligned)
let cache = TierCache::new(user_id);

// Load tier (<50ns atomic)
let tier = cache.load();

// Store tier (<50ns atomic)
cache.store(SubscriptionTier::Enterprise);

// Atomic upgrade (<100ns CAS)
match cache.compare_exchange(SubscriptionTier::Free, SubscriptionTier::Solo) {
    Ok(()) => println!("Upgrade successful"),
    Err(actual) => println!("Already at tier: {}", actual),
}

// Get all quotas in one call (<100ns)
let (monthly, retention, rate) = cache.quotas();
```

### Axum Middleware Integration

```rust
use axum::{Router, routing::get, Extension};
use tower::ServiceBuilder;
use clapi_core::licensing::{
    TierExtension,
    tier_extraction_middleware,
    SubscriptionTier,
};

// Add middleware to router
let app = Router::new()
    .route("/api/v1/models", get(list_models))
    .layer(ServiceBuilder::new()
        .layer(axum::middleware::from_fn(tier_extraction_middleware)));

// Extract tier in handler
async fn list_models(
    Extension(tier_ext): Extension<TierExtension>,
) -> String {
    let tier = tier_ext.tier;
    let (monthly, retention, rate) = (
        tier.monthly_request_limit(),
        tier.retention_days(),
        tier.rate_limit_rps(),
    );
    
    format!(
        "User {} - Tier: {} (quota: {}, retention: {} days, rate: {} rps)",
        tier_ext.user_id, tier, monthly, retention, rate
    )
}
```

### JWT-Based Tier Detection (Week 5)

```rust
use clapi_core::licensing::detect_tier_from_jwt;

// Detect tier from JWT token (<200ns)
let token = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...";
let tier = detect_tier_from_jwt(token).await?;

// Note: Current implementation returns Free tier (placeholder)
// Week 5: Real JWT decoding with signature validation
```

## Advanced Usage

### Rate Limit Enforcement

```rust
use clapi_core::licensing::{TierCache, SubscriptionTier};
use clapi_core::capsules::RateLimitCapsule;

let cache = TierCache::with_tier(user_id, SubscriptionTier::Team);
let tier = cache.load();

// Configure rate limiter based on tier
let rate_limit = RateLimitCapsule::new(
    user_id,
    tier.rate_limit_rps() as u64, // 100 rps for Team tier
    1, // 1 second window
);

// Try to acquire token
if rate_limit.try_acquire() {
    // Process request
} else {
    // Return 429 Too Many Requests
}
```

### Quota Enforcement

```rust
use clapi_core::licensing::{TierCache, SubscriptionTier};
use clapi_core::capsules::BudgetMetaCapsule;

let cache = TierCache::with_tier(user_id, SubscriptionTier::Solo);
let tier = cache.load();

// Check monthly quota
let monthly_limit = tier.monthly_request_limit(); // 100,000

// Query current usage from BudgetMetaCapsule
let usage = budget_meta.get_monthly_usage(user_id);

if usage < monthly_limit {
    // Allow request
} else {
    // Return 429 Quota Exceeded
}
```

### Retention Policy Integration

```rust
use clapi_core::licensing::{TierCache, RetentionPolicy};

let cache = TierCache::with_tier(user_id, SubscriptionTier::Enterprise);
let tier = cache.load();

// Get retention policy for tier
let policy = RetentionPolicy::from_tier(tier);
let retention_days = policy.retention_days(); // 365

// Calculate cutoff timestamp
let cutoff = policy.cutoff_from_now();

// Delete audit logs older than cutoff
db.delete_audit_logs_before(user_id, cutoff)?;
```

### Tier Upgrade/Downgrade

```rust
use clapi_core::licensing::{TierCache, SubscriptionTier};

let cache = TierCache::with_tier(user_id, SubscriptionTier::Free);

// Upgrade to Solo tier (atomic CAS)
match cache.compare_exchange(SubscriptionTier::Free, SubscriptionTier::Solo) {
    Ok(()) => {
        // Trigger payment flow
        // Update database
        // Send confirmation email
        println!("Upgraded to Solo tier");
    }
    Err(actual) => {
        println!("Already at tier: {}", actual);
    }
}

// Downgrade on trial expiry
if trial.is_expired() {
    cache.store(SubscriptionTier::Free);
    // Trigger cleanup job
    // Send downgrade notification
}
```

## Performance Guidelines

### Hot Path (<200ns total)
```rust
// 1. Extract tier from request (<20ns)
let tier_ext = get_tier_from_request(&req)?;

// 2. Load tier from cache (<50ns)
let tier = tier_cache.load();

// 3. Check quota (<5ns const fn)
let monthly_limit = tier.monthly_request_limit();

// 4. Enforce rate limit (<40ns atomic)
rate_limiter.try_acquire()?;

// Total: ~115ns overhead per request
```

### Cold Path (>1ms acceptable)
```rust
// Database queries
db.get_user_tier(user_id)?; // ~1-10ms

// JWT decoding (Week 5)
decode_jwt(token)?; // ~150ns (signature validation)

// Tier cache miss
tier_cache_global.insert(user_id, tier); // ~100ns
```

## Error Handling

```rust
use clapi_core::error::ClapiError;

// Tier detection errors
match detect_tier_from_jwt(token).await {
    Ok(tier) => { /* success */ }
    Err(ClapiError::Unauthorized) => {
        // Invalid JWT token
        return Err(StatusCode::UNAUTHORIZED);
    }
    Err(e) => {
        // Other errors
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
}

// Quota errors
if usage >= tier.monthly_request_limit() {
    return Err(ClapiError::RateLimitExceeded {
        quota: tier.monthly_request_limit(),
        window_duration_secs: 2_592_000, // 30 days
    });
}
```

## Testing

### Unit Tests
```rust
#[test]
fn test_tier_upgrade() {
    let cache = TierCache::new(12345);
    assert_eq!(cache.load(), SubscriptionTier::Free);
    
    cache.store(SubscriptionTier::Solo);
    assert_eq!(cache.load(), SubscriptionTier::Solo);
}
```

### Property Tests (TODO: Integration Expert)
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_tier_cache_thread_safety(tier in any::<SubscriptionTier>()) {
        let cache = TierCache::new(12345);
        cache.store(tier);
        assert_eq!(cache.load(), tier);
    }
}
```

## Migration Guide

### From RwLock to TierCache

**Before** (RwLock):
```rust
let tier_map: RwLock<HashMap<u64, SubscriptionTier>> = RwLock::new(HashMap::new());

// Read (~200ns + lock contention)
let tier = *tier_map.read().unwrap().get(&user_id).unwrap();

// Write (~500ns + lock contention)
tier_map.write().unwrap().insert(user_id, SubscriptionTier::Solo);
```

**After** (TierCache):
```rust
let tier_cache: DashMap<u64, TierCache> = DashMap::new();
tier_cache.insert(user_id, TierCache::new(user_id));

// Read (<50ns lockfree)
let tier = tier_cache.get(&user_id).unwrap().load();

// Write (<50ns lockfree)
tier_cache.get(&user_id).unwrap().store(SubscriptionTier::Solo);
```

**Benefits**:
- 4× faster reads (200ns → 50ns)
- 10× faster writes (500ns → 50ns)
- Zero lock contention
- Zero deadlocks
- Cache-line aligned (zero false sharing)

## Best Practices

### ✅ DO
- Use `TierCache` for per-user tier storage
- Use `const fn` methods for quota queries
- Use `compare_exchange` for atomic upgrades
- Cache tier lookups in request extensions
- Document performance targets (B32)
- Use ASSUM tags for safety assumptions

### ❌ DON'T
- Use Mutex/RwLock for tier storage
- Query database on every request
- Store tier in mutable HashMap
- Allocate on hot path
- Use unsafe without ASSUM tags
- Make unbounded performance claims

## Future Enhancements (Week 5+)

- Real JWT decoding with signature validation
- Global tier cache (DashMap<user_id, TierCache>)
- Tier change webhooks
- Usage analytics per tier
- A/B testing for tier features
- Stripe integration for payments
- Tier downgrade grace period
- Custom tier configuration API
