# Technical Debt Tracking - clapi_core

**Purpose**: Document future improvements without cluttering code with TODO comments
**Last Updated**: 2025-10-21
**Framework**: IMPL-2 V3.0 (file preservation, no deletion, only addition/modification)

---

## Philosophy

Per IMPL-2 V3.0:
- **NO file deletion** during simplification or optimization
- **NO TODO comments** in code (use this document instead)
- **Stack optimizations** when implementation is faster than measurement
- **Build all edges** toward 99.99% reliability at 30× AI speed

---

## P0 (Critical - Ship Blocking)

### NONE ✅

All P0 issues resolved. Ready to ship P1 enhancements after P1 fixes below.

---

## P1 (High Priority - Fix Before Shipping)

### 1. Fix tracing-subscriber Dependency ❌ BLOCKER

**Issue**: `EnvFilter` import missing, `with_env_filter()` method not found
**Files**: `src/logging.rs:487, 492, 506, 512`
**Root Cause**: Missing feature flag in `Cargo.toml`

**Fix**:
```toml
[dependencies]
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
```

**Estimated Effort**: 5 minutes
**Risk**: ZERO (dependency version bump)
**Framework**: UCE-D7 (debugging framework, minimal fix)

---

### 2. Replace Hot Path Unwraps with Result<T> ⚠️ HIGH PRIORITY

**Issue**: 6 `.unwrap()` calls in hot paths (potential panics under error conditions)
**Impact**: MEDIUM (reliability risk in production)

**Locations**:

#### 2.1 `provider_router.rs:234`
```rust
// BEFORE:
let provider = provider_name.unwrap();

// AFTER:
let provider = provider_name.ok_or(ClapiError::MissingProvider)?;
```
**Estimated Effort**: 10 minutes

#### 2.2 `metrics_stream.rs:445`
```rust
// BEFORE:
let ts = timestamp.unwrap();

// AFTER:
let ts = timestamp.ok_or(ClapiError::MissingTimestamp)?;
```
**Estimated Effort**: 10 minutes

#### 2.3 `timeline_aggregation_capsule.rs:912`
```rust
// BEFORE:
let p99 = percentile.unwrap();

// AFTER:
let p99 = percentile.ok_or(ClapiError::InvalidPercentile)?;
```
**Estimated Effort**: 10 minutes

#### 2.4 `payment128.rs:567`
```rust
// BEFORE:
let stripe_resp = stripe_response.unwrap();

// AFTER:
let stripe_resp = stripe_response.map_err(|e| ClapiError::StripeError(e))?;
```
**Estimated Effort**: 15 minutes (needs error mapping)

#### 2.5 `ws.rs:789`
```rust
// BEFORE:
let conn = websocket_conn.unwrap();

// AFTER:
let conn = websocket_conn.ok_or(ClapiError::WebSocketConnectionClosed)?;
```
**Estimated Effort**: 10 minutes

**Total Effort**: 55 minutes
**Risk**: LOW (straightforward error handling)
**Framework**: UCE-D7 (minimal fix, no scope creep)

---

## P2 (Medium Priority - Future Enhancements)

### 3. E15: Extract Aggregation Helpers (Timeline Aggregation)

**Issue**: `timeline_aggregation_capsule.rs` is 1,336 lines with complex percentile computation
**Opportunity**: SIMD optimization for multi-field percentile (T2 tier)

**Refactoring**:
```
timeline_aggregation_capsule.rs (1,336 lines)
  → timeline_aggregation_capsule.rs (600 lines, capsule definition)
  + aggregation_helpers.rs (400 lines, percentile/median/sum helpers)
  + aggregation_simd.rs (336 lines, SIMD variants for 4+ fields)
```

**Benefits**:
- **2-4× speedup** for percentile computation on 4+ fields (T2 SIMD tier)
- **Better testability** (separate helper tests)
- **Code reusability** (other capsules can use aggregation helpers)

**Estimated Effort**: 3 weeks
- Week 1: Extract helpers to `aggregation_helpers.rs`
- Week 2: Implement SIMD variants (T2 tier, requires nightly)
- Week 3: B32 benchmarking + T28 testing

**Framework**: UCE34 Q10-Q12 (T2 SIMD tier selection) + B32 benchmarking
**Risk**: MEDIUM (SIMD requires careful alignment, testing)

---

### 4. Split Large Files (TOP 3)

#### 4.1 `timeline_aggregation_capsule.rs` → See E15 Above

#### 4.2 `dashboard.rs` (1,133 lines)
**Split**:
```
dashboard.rs
  → dashboard_render.rs (550 lines, UI rendering)
  + dashboard_state.rs (350 lines, state management)
  + dashboard_metrics.rs (233 lines, metrics collection)
```

**Estimated Effort**: 2 weeks
**Risk**: LOW (UI refactoring, well-defined boundaries)

#### 4.3 `ws.rs` (915 lines)
**Split**:
```
ws.rs
  → ws_protocol.rs (450 lines, WebSocket protocol handling)
  + ws_pool.rs (350 lines, connection pooling)
  + ws_handlers.rs (115 lines, message handlers)
```

**Estimated Effort**: 2 weeks
**Risk**: MEDIUM (WebSocket protocol is complex)

**Total Effort**: 7 weeks (3 + 2 + 2)
**Framework**: IMPL-2 V3.0 (simplify interfaces, preserve all files)

---

### 5. Migrate DashMap to ConcurrentMapCapsule

**Issue**: Phase 1 dependency, should use `atomic_capsule::collections::ConcurrentMapCapsule`
**Opportunity**: **3-59× speedup** (false sharing eliminated, lockfree)

**Files Affected**:
- `src/proxy/provider_router.rs` (DashMap for provider lookup)
- `src/metrics/query.rs` (DashMap for metrics cache)
- `src/cache/lru.rs` (DashMap for LRU cache)

**Migration**:
```rust
// BEFORE:
use dashmap::DashMap;
let map: DashMap<String, Provider> = DashMap::new();

// AFTER:
use atomic_capsule::collections::ConcurrentMapCapsule;
let map = ConcurrentMapCapsule::new();
```

**Benefits**:
- **3-59× speedup** (100ns insert vs 5,950ns with false sharing)
- **100% lockfree** (consistency with capsule architecture)
- **Zero dependencies** (remove DashMap from Cargo.toml)

**Estimated Effort**: 1 week
- Day 1-2: Replace `provider_router.rs` DashMap
- Day 3-4: Replace `metrics/query.rs` DashMap
- Day 5: Replace `cache/lru.rs` DashMap
- Day 6-7: T28 testing + B32 benchmarking

**Framework**: I20 Integration (Q1-Q20 integration questions) + B32 benchmarking
**Risk**: LOW (drop-in replacement, well-tested ConcurrentMapCapsule)

---

### 6. Deprecate payment.rs, Migrate to payment128.rs

**Issue**: Code duplication between `payment.rs` (legacy) and `payment128.rs` (Q16.16 fixed-point)
**Impact**: MEDIUM (maintenance burden, confusion)

**Migration**:
```rust
// Add deprecation warning to payment.rs
#[deprecated(since = "0.5.0", note = "Use payment128.rs with Q16.16 fixed-point")]
pub struct PaymentCapsule256 { ... }
```

**Steps**:
1. Mark `payment.rs` as deprecated (v0.5.0)
2. Add migration guide in `docs/PAYMENT_MIGRATION.md`
3. Migrate all callers to `payment128.rs` (v0.5.x)
4. Remove `payment.rs` in v0.6.0 (breaking change)

**Estimated Effort**: 2 weeks
- Week 1: Identify all callers, migrate one by one
- Week 2: T28 testing, remove deprecation warnings

**Framework**: I20 Integration + T28 Testing
**Risk**: LOW (well-defined migration path)

---

### 7. Add Production-Tier Tests (Q22-Q28)

**Issue**: Only 12 production-tier tests vs 42 integration tests
**Opportunity**: E24 multi-tenant stress tests, E7 concurrent builder chaos tests

**New Tests**:

#### 7.1 E24 Multi-Tenant Overhead Stress Test
```rust
#[test]
fn stress_test_multi_tenant_overhead() {
    // 1,000 tenants × 10,000 requests/tenant
    // Measure overhead: <5% per tenant isolation
}
```
**Estimated Effort**: 3 days

#### 7.2 E7 Concurrent Test Builder Chaos Test
```rust
#[test]
fn chaos_test_concurrent_builder() {
    // 100 threads × 1,000 operations
    // Random failures, retry logic, race conditions
}
```
**Estimated Effort**: 3 days

#### 7.3 E10 Budget Enforcer Load Test
```rust
#[test]
fn load_test_budget_enforcer() {
    // 1M budget checks/sec
    // Measure P99 latency <100ns
}
```
**Estimated Effort**: 2 days

**Total Effort**: 8 days
**Framework**: T28 Production Tier (Q22-Q28)
**Risk**: LOW (testing infrastructure already exists)

---

### 8. Verify Remaining ASSUM Tags (5 Unverified)

**Issue**: 5 of 197 ASSUM tags lack verification tests

#### 8.1 `budget_metacapsule.rs:412` - #ASSUME_SLOT_REUSE
```rust
// #ASSUME_SLOT_REUSE: Deallocated slots can be safely reused after generation increment
// #VERIFY: Property test validates no ABA after 1M deallocations
```
**Estimated Effort**: 1 day

#### 8.2 `timeline_aggregation_capsule.rs:856` - #ASSUME_PERCENTILE_ACCURACY
```rust
// #ASSUME_PERCENTILE_ACCURACY: Quickselect percentile is ±1% accurate for N>100
// #VERIFY: Unit test validates percentile accuracy >99% for N=1000
```
**Estimated Effort**: 1 day

#### 8.3 `payment128.rs:723` - #ASSUME_STRIPE_IDEMPOTENCY
```rust
// #ASSUME_STRIPE_IDEMPOTENCY: Stripe API guarantees idempotency for duplicate requests
// #VERIFY: Integration test validates duplicate payment handling
```
**Estimated Effort**: 2 days (needs Stripe test account)

#### 8.4 `ws.rs:445` - #ASSUME_WEBSOCKET_FRAMING
```rust
// #ASSUME_WEBSOCKET_FRAMING: WebSocket frames are delivered in order
// #VERIFY: Integration test validates frame ordering under packet loss
```
**Estimated Effort**: 2 days

#### 8.5 `doctor.rs:189` - #ASSUME_SYSTEM_CLOCK_MONOTONIC
```rust
// #ASSUME_SYSTEM_CLOCK_MONOTONIC: System clock never goes backward
// #VERIFY: Property test validates timestamp ordering across restarts
```
**Estimated Effort**: 1 day

**Total Effort**: 7 days
**Framework**: ASSUM Safety (all assumptions must be verified)
**Risk**: LOW (all assumptions are low-risk, just need test coverage)

---

## P3 (Low Priority - Nice to Have)

### 9. Replace CLI Unwraps with Better Error Messages

**Issue**: 12 `.unwrap()` calls in CLI commands (ACCEPTABLE but could be better)
**Impact**: LOW (CLI can panic, but better error messages help users)

**Example**:
```rust
// BEFORE:
let config = load_config().unwrap();

// AFTER:
let config = load_config()
    .map_err(|e| eprintln!("Failed to load config: {}", e))
    .expect("Config is required to continue");
```

**Estimated Effort**: 2 days (12 instances)
**Risk**: ZERO (CLI improvement only)

---

### 10. Add Clippy Lints for Capsule Best Practices

**Opportunity**: Custom clippy lints for capsule-specific patterns

**Examples**:
- `clippy::missing_capsule_verification` (already exists)
- `clippy::unaligned_capsule_access`
- `clippy::atomic_ordering_relaxed_in_hot_path`
- `clippy::nested_capsule_composition` (warn on >3 tiers)

**Estimated Effort**: 4 weeks
- Week 1: Research clippy lint API
- Week 2-3: Implement 4 custom lints
- Week 4: T28 testing + documentation

**Framework**: UCE34 Q33 (verification) + clippy-capsule-verify foundation
**Risk**: MEDIUM (clippy lint API is complex)

---

### 11. SIMD Optimization for Histogram (E15 Extension)

**Issue**: `profiling/histogram_simd.rs` uses scalar loops for some operations
**Opportunity**: Full SIMD coverage for histogram operations

**Benefits**:
- **2-4× speedup** for histogram updates (T2 SIMD tier)
- **Better cache utilization** (vectorized loads/stores)

**Estimated Effort**: 2 weeks
- Week 1: Implement SIMD variants
- Week 2: B32 benchmarking + T28 testing

**Framework**: UCE34 Q10-Q12 (T2 SIMD tier) + B32 benchmarking
**Risk**: MEDIUM (SIMD requires nightly, careful testing)

---

### 12. Const-Hashing for Static Provider IDs

**Issue**: Provider IDs are hashed at runtime (small cost, but could be compile-time)
**Opportunity**: Use `atomic_capsule::hash::const_hash` for 0ns runtime cost

**Example**:
```rust
// BEFORE:
let provider_id = hash_provider_name("anthropic");  // 10-20ns

// AFTER:
const ANTHROPIC_ID: u64 = const_hash!("anthropic");  // 0ns
```

**Benefits**:
- **0ns runtime cost** (100× speedup, compile-time hashing)
- **Better code clarity** (const declarations)

**Estimated Effort**: 3 days
**Risk**: LOW (const-hashing already proven in Phase 2.2)

---

## Summary

### Total Technical Debt

| Priority | Count | Estimated Effort | Risk Level |
|----------|-------|------------------|------------|
| **P0 (Critical)** | 0 | 0 | ✅ NONE |
| **P1 (High)** | 2 | 1 hour | ⚠️ LOW |
| **P2 (Medium)** | 6 | 13 weeks | ⚠️ MEDIUM |
| **P3 (Low)** | 4 | 8 weeks | ⏸️ LOW |
| **Total** | **12** | **21 weeks** | **MANAGEABLE** |

### Debt Trajectory

```
Technical Debt Over Time:

P1 Ship  →  P2 (3 months)  →  P3 (6 months)
   2          8                12 (stable)
   |          |                 |
   └──────────┴─────────────────┘
   (Fix)      (Improve)         (Polish)
```

**Recommendation**: Fix P1 (1 hour), ship P1 enhancements, then address P2 over next 3 months.

---

## Debt Prevention (Going Forward)

### Rules to Prevent New Debt

1. **NO file deletion** (IMPL-2 V3.0)
   - Simplify interfaces, not delete implementations
   - Hide complexity internally

2. **NO TODO comments in code**
   - Use this document instead
   - Track with GitHub issues if needed

3. **ALL capsules must use `#[derive(ComputationalCapsule)]`**
   - Automatic verification (0ns runtime)
   - Clippy lint safety net

4. **ALL performance claims must use B32 framework**
   - Fair baselines (not strawman comparisons)
   - Statistical rigor (1000+ iterations, 95% CI)
   - Honest claims (10-50% typical, 2-10× exceptional)

5. **ALL assumptions must be verified (ASSUM framework)**
   - Every #ASSUME needs #VERIFY
   - Unit/property/integration tests for all assumptions

6. **ALL functions <50 lines** (target: 20-30 lines)
   - Extract complex logic to helpers
   - Keep cyclomatic complexity <10

7. **ALL modules <500 lines** (target: 300 lines)
   - Split large files proactively
   - Extract reusable helpers

---

## Next Steps

1. **Read CODE_QUALITY_REPORT.md** for detailed analysis
2. **Fix P1 issues** (1 hour) before shipping
3. **Prioritize P2 items** for next quarter roadmap
4. **Update this document** as debt is resolved or new debt discovered

---

**Last Updated**: 2025-10-21
**Framework**: IMPL-2 V3.0 + UCE34 + T28 + B32 + ASSUM
**Maintainer**: Technical Debt Expert
