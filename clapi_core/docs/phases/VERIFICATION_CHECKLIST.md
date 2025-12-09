# ASSUM Verification Checklist - Clapi Core

**Purpose**: Quick reference for implementing and verifying ASSUM safety tags
**Status**: Pre-Implementation Guidance
**Framework**: ASSUM Safety (10 Categories)

## Per-Capsule Verification

### ✅ RequestCapsule128 (Budget Enforcement)

#### ASSUM Tags Required
- [ ] `#ASSUME_BUDGET_ATOMIC`: CAS loop ensures atomic deduction
- [ ] `#ASSUME_TOCTOU_ELIMINATED`: Generation counter prevents ABA
- [ ] `#ASSUME_OVERFLOW_SAFE`: u64 accommodates $18 quintillion
- [ ] `#ASSUME_MEMORY_ORDERING`: Acquire/Release prevents races

#### Verification Methods
- [ ] Property test: Budget never negative (1000 threads × 10K ops)
- [ ] Stress test: 1M concurrent deductions, zero overdrafts
- [ ] Miri: Zero UB, zero data races
- [ ] ThreadSanitizer: Clean
- [ ] Loom: All interleavings safe
- [ ] Benchmark: <100ns deduction (2× speedup vs baseline)

#### Critical Code Sections
```rust
// Budget deduction CAS loop (line ~45)
// #ASSUME_BUDGET_ATOMIC: CAS loop ensures atomic budget deduction
// #VERIFY_BUDGET_CORRECTNESS: Property test validates no overdrafts

// Generation counter increment (line ~65)
// #ASSUME_TOCTOU_ELIMINATED: Generation counter prevents ABA
// #VERIFY_ABA_FREE: Concurrent stress test validates monotonicity

// Memory ordering (line ~50, ~55)
// #ASSUME_MEMORY_ORDERING: Acquire/Release prevents data races
// #VERIFY_ORDERING_STRESS: Tested under 100 threads, no races
```

#### Residual Risk
- **Target**: <0.001%
- **Sources**: Hardware bugs (<0.0001%), compiler bugs (<0.0001%), memory corruption (<0.0009%)

---

### ✅ RoutingCapsule128 (Provider Selection)

#### ASSUM Tags Required
- [ ] `#ASSUME_DETERMINISTIC`: Same input → same provider
- [ ] `#ASSUME_STATE_VALID`: FSM prevents invalid transitions
- [ ] `#ASSUME_GENERATION_ABA`: Gen counter prevents ABA

#### Verification Methods
- [ ] Property test: Routing consistency (1M iterations)
- [ ] Statistical test: Chi-squared validates uniform distribution
- [ ] Unit test: All FSM transitions checked
- [ ] Integration test: Failover cascade works
- [ ] Loom: All ABA scenarios prevented
- [ ] Benchmark: <100ns selection (3× speedup)

#### Critical Code Sections
```rust
// Provider selection hash (line ~80)
// #ASSUME_DETERMINISTIC: Hash-based selection is reproducible
// #VERIFY_DETERMINISTIC_ROUTING: Same request hash → same provider

// Circuit breaker FSM (line ~110)
// #ASSUME_STATE_VALID: Circuit breaker state machine is correct
// #VERIFY_STATE_MACHINE: Model checking validates FSM

// Generation counter (line ~95)
// #ASSUME_GENERATION_ABA: 64-bit generation counter prevents ABA
// #VERIFY_ABA_FREE: Concurrent stress test validates monotonicity
```

#### Residual Risk
- **Target**: <0.01%
- **Sources**: Hash collision (<0.001%), circuit breaker false positive (<0.005%), health check race (<0.004%)

---

### ✅ ResponseCapsule256 (Cost Tracking)

#### ASSUM Tags Required
- [ ] `#ASSUME_PRECISION_SUFFICIENT`: Q16.16 provides $0.000015 precision
- [ ] `#ASSUME_NO_DRIFT`: Fixed-point eliminates FP errors
- [ ] `#ASSUME_ATOMIC_UPDATE`: Readers never see partial updates

#### Verification Methods
- [ ] Math proof: Q16.16 precision ±$0.000015
- [ ] Property test: 10K updates, zero drift vs expected
- [ ] ThreadSanitizer: No data races in cost accumulation
- [ ] Loom: All interleavings validate atomic visibility
- [ ] Benchmark: <500ns tracking (4× speedup), fixed-point 10× faster than f64

#### Critical Code Sections
```rust
// Cost accumulation (line ~135)
// #ASSUME_PRECISION_SUFFICIENT: Q16.16 provides $0.000015 precision
// #VERIFY_PRECISION_ADEQUATE: $65,535.99998 max cost

// Fixed-point arithmetic (line ~150)
// #ASSUME_NO_DRIFT: Fixed-point arithmetic is exact
// #VERIFY_NO_ACCUMULATION_ERROR: 1M additions, zero drift

// Atomic update (line ~145)
// #ASSUME_ATOMIC_UPDATE: AtomicU64 ensures visibility
// #VERIFY_ATOMIC_VISIBILITY: ThreadSanitizer + Loom validate
```

#### Residual Risk
- **Target**: <0.0001%
- **Sources**: Fixed-point overflow (<0.00001%), atomic visibility bug (<0.00001%), SIMD alignment (<0.00008%)

---

### ✅ AuditLogEntry128 (Hash Chain)

#### ASSUM Tags Required
- [ ] `#ASSUME_APPEND_ONLY`: Generation ensures immutability
- [ ] `#ASSUME_HASH_SECURE`: SHA256 collision resistance 2^-128
- [ ] `#ASSUME_CHAIN_INTEGRITY`: Prev hash links validated

#### Verification Methods
- [ ] Unit test: Hash chain integrity after 10K appends
- [ ] Property test: Append order preserved (monotonic generations)
- [ ] Integration test: Merkle root validates entire chain
- [ ] Property test: Tampering detection (any modification breaks chain)
- [ ] Cryptographic proof: SHA256 NIST standard (FIPS 180-4)
- [ ] Benchmark: <50ns append (10× speedup)

#### Critical Code Sections
```rust
// Append operation (line ~180)
// #ASSUME_APPEND_ONLY: Generation counter prevents modification
// #VERIFY_IMMUTABILITY: Property test validates no entry changes

// Hash computation (line ~200)
// #ASSUME_HASH_SECURE: SHA256 provides 2^-128 collision resistance
// #VERIFY_COLLISION_RESISTANCE: Cryptographic proof (NIST standard)

// Chain validation (line ~220)
// #ASSUME_CHAIN_INTEGRITY: Each entry links to previous via hash
// #VERIFY_CHAIN_VALIDATION: Merkle root invariant tested
```

#### Residual Risk
- **Target**: <0.00001%
- **Sources**: SHA256 collision (<0.00000001%), generation overflow (<0.00000001%), memory corruption (<0.00001%)

---

### ✅ EpochTile1024 (Cost Aggregation)

#### ASSUM Tags Required
- [ ] `#ASSUME_BATCH_ATOMIC`: Tile updates indivisible
- [ ] `#ASSUME_FIXED_POINT`: Q8.8 sufficient for aggregation
- [ ] `#ASSUME_EPOCH_ALIGNED`: Timestamps bucket correctly

#### Verification Methods
- [ ] Unit test: Batch flush atomicity (all or nothing)
- [ ] Property test: No partial updates observed by readers
- [ ] Math proof: Q8.8 precision ±$0.0039
- [ ] Property test: Hourly aggregation exact vs expected
- [ ] Unit test: Epoch boundaries correct (hour transitions)
- [ ] Benchmark: <100μs aggregation (10× speedup)

#### Critical Code Sections
```rust
// Batch flush (line ~250)
// #ASSUME_BATCH_ATOMIC: Tile flush is atomic (all or nothing)
// #VERIFY_BATCH_ATOMICITY: Property test validates no partial updates

// Fixed-point aggregation (line ~270)
// #ASSUME_FIXED_POINT: Q8.8 provides sufficient precision
// #VERIFY_PRECISION_ADEQUATE: $16,777,215.99609 max cost per epoch

// Epoch bucketing (line ~290)
// #ASSUME_EPOCH_ALIGNED: Timestamp bucketing is deterministic
// #VERIFY_EPOCH_BOUNDARIES: Unit test validates bucket assignment
```

#### Residual Risk
- **Target**: <0.01%
- **Sources**: Concurrent flush (<0.001%), Q8.8 precision loss (<0.005%), clock skew (<0.004%)

---

## Cross-Capsule Verification

### Integration Tests
- [ ] Full request flow: Budget → Routing → Response → Audit
- [ ] Budget exhaustion handling (reject request, refund budget)
- [ ] Provider failover (circuit breaker transitions)
- [ ] Audit chain validation (Merkle root after 10K appends)
- [ ] Cost aggregation (epoch rollover, batch flush)

### Stress Tests
- [ ] 1000 threads × 10K budget deductions (never negative)
- [ ] 100 threads × 1K audit appends (chain integrity)
- [ ] 1M concurrent requests (full system, all invariants)

### Static Analysis
- [ ] Miri: Zero undefined behavior
- [ ] ThreadSanitizer: Zero data races
- [ ] Loom: All interleavings safe
- [ ] Clippy: Zero warnings (with clippy::missing_capsule_verification)

---

## Quick ASSUM Tagging Template

For every atomic operation:

```rust
// #ASSUME_<CATEGORY>: <Assumption>
//   - Detail 1
//   - Detail 2
// #VERIFY_<CATEGORY>: <Verification>
//   - Method 1 (e.g., Property test)
//   - Method 2 (e.g., Miri)
let value = self.state.load(Ordering::Acquire);
```

### Common Categories

1. **BUDGET_ATOMIC** / **BUDGET_CORRECTNESS**
2. **TOCTOU_ELIMINATED** / **ABA_FREE**
3. **OVERFLOW_SAFE** / **OVERFLOW_IMPOSSIBLE**
4. **MEMORY_ORDERING** / **ORDERING_STRESS**
5. **DETERMINISTIC** / **DETERMINISTIC_ROUTING**
6. **STATE_VALID** / **STATE_MACHINE**
7. **PRECISION_SUFFICIENT** / **PRECISION_ADEQUATE**
8. **NO_DRIFT** / **NO_ACCUMULATION_ERROR**
9. **ATOMIC_UPDATE** / **ATOMIC_VISIBILITY**
10. **HASH_SECURE** / **COLLISION_RESISTANCE**
11. **CHAIN_INTEGRITY** / **CHAIN_VALIDATION**
12. **BATCH_ATOMIC** / **BATCH_ATOMICITY**

---

## Testing Commands

```bash
# Unit tests
cargo test

# Property tests
cargo test --features proptest

# Miri validation (UB detection)
cargo +nightly miri test

# ThreadSanitizer (data race detection)
RUSTFLAGS="-Z sanitizer=thread" cargo +nightly test

# Loom model checking
cargo test --features loom

# Benchmarks (B32 rigor)
cargo bench

# Full validation suite
./scripts/validate_safety.sh
```

---

## Overall Verification Status

### Phase 1: Foundation (Week 1)
- [x] Safety audit document created (37KB, 800+ lines)
- [x] Verification checklist created (this document)
- [ ] Implement capsules with ASSUM tags
- [ ] Add #[derive(ComputationalCapsule)] to all structs

### Phase 2: Unit Tests (Week 2)
- [ ] RequestCapsule128 tests (budget deduction)
- [ ] RoutingCapsule128 tests (provider selection)
- [ ] ResponseCapsule256 tests (cost tracking)
- [ ] AuditLogEntry128 tests (hash chain)
- [ ] EpochTile1024 tests (cost aggregation)

### Phase 3: Property Tests (Week 2)
- [ ] Budget never negative (arbitrary inputs)
- [ ] Routing consistency (same input → same provider)
- [ ] Cost accumulation exact (zero drift)
- [ ] Audit chain tamper-proof (any modification breaks)
- [ ] Epoch aggregation correct (batch atomicity)

### Phase 4: Integration Tests (Week 3)
- [ ] Full request flow (budget → routing → response → audit)
- [ ] Budget exhaustion handling
- [ ] Provider failover cascade
- [ ] Audit chain validation
- [ ] Cost aggregation (epoch rollover)

### Phase 5: Stress Tests (Week 3)
- [ ] 1000 threads × 10K operations (budget)
- [ ] 100 threads × 1K appends (audit)
- [ ] 1M concurrent requests (full system)

### Phase 6: Static Analysis (Week 4)
- [ ] Miri: Zero UB
- [ ] ThreadSanitizer: Zero data races
- [ ] Loom: All interleavings safe
- [ ] Clippy: Zero warnings

### Phase 7: Benchmarking (Week 4)
- [ ] Budget deduction: <100ns (2× baseline)
- [ ] Provider selection: <100ns (3× baseline)
- [ ] Response metrics: <500ns (4× baseline)
- [ ] Audit append: <50ns (10× baseline)
- [ ] Cost aggregation: <100μs (10× baseline)

---

## Success Criteria

### All Tests Pass
- ✅ Zero test failures
- ✅ Zero Miri warnings
- ✅ Zero ThreadSanitizer warnings
- ✅ Zero Loom failures

### Performance Targets Met
- ✅ All benchmarks within 10% of targets
- ✅ 95% confidence intervals (1000+ iterations)
- ✅ Reproducible results (±5% variance)

### Safety Rating Achieved
- ✅ Overall: 99.99%+ safety rating
- ✅ Per-capsule: <0.01% residual risk
- ✅ All ASSUM tags documented and verified

---

## Document Maintenance

**Update when**:
- New capsules added
- Atomic operations modified
- Performance targets changed
- Safety assumptions updated

**Review cadence**: Every major version (v0.x.0)
**Owner**: Security Expert + Implementation Team
**Sign-off**: Required before production deployment

---

**Document Version**: 1.0.0
**Last Updated**: 2025-10-16
**Status**: Pre-Implementation Checklist ✅
