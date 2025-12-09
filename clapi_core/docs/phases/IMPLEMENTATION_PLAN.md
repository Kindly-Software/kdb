# CapsuleHash64 Implementation Plan
## Handoff from Architecture Expert to Implementation Team

**Date**: 2025-10-17
**Architecture Complete**: UCE33 Q1-Q33 Full Analysis
**Architecture Document**: [ARCHITECTURE_DESIGN.md](./ARCHITECTURE_DESIGN.md) (1,213 lines)
**Status**: Ready for Implementation

---

## For the Implementation Team

This document provides a concise implementation plan for the other agents. The full architecture is in [ARCHITECTURE_DESIGN.md](./ARCHITECTURE_DESIGN.md) - please read that first.

---

## What You're Building

**CapsuleHash64**: A custom hash primitive for zero-overhead capsule telemetry

**Key Features**:
- <2ns hash computation (SIMD)
- <1ns incremental updates (XOR-based)
- 100% lockfree (atomic storage)
- Zero dependencies (self-contained)
- 100% corruption detection

**Architecture**: Tier 2+1 Mixed Capsule (SIMD computation + atomic storage)

---

## File Structure

```
clapi_core/
├── src/
│   ├── capsules/
│   │   ├── mod.rs                     ← Update exports
│   │   ├── capsule_hash64.rs          ← NEW (hash primitive)
│   │   ├── req_128_enhanced.rs        ← NEW (enhanced capsule)
│   │   └── req_128.rs                 ← Keep for backward compatibility
│   └── ...
├── benches/
│   └── capsule_hash64_bench.rs        ← NEW (B32 benchmarks)
├── tests/
│   ├── capsule_hash64_tests.rs        ← NEW (unit tests)
│   └── req_128_enhanced_tests.rs      ← NEW (integration tests)
└── ...
```

---

## Implementation Order

### File 1: `src/capsules/capsule_hash64.rs` (Core Hash Primitive)

**Estimated Time**: 4 hours

**Structure**:
```rust
// Hash constants
pub const HASH_SEED: u64 = 0x517cc1b727220a95;
pub const HASH_MUL: u64 = 0x9e3779b97f4a7c15;
pub const HASH_ROTATE: u32 = 31;

// CapsuleHash64 structure (64 bytes)
#[derive(ComputationalCapsule, Debug)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct CapsuleHash64 {
    hash: AtomicU64,
    _padding: [u8; 56],
}

impl CapsuleHash64 {
    pub const fn new() -> Self { ... }
    pub fn compute(fields: &[u64]) -> u64 { ... }

    #[cfg(feature = "portable_simd")]
    fn compute_simd(fields: &[u64]) -> u64 { ... }

    #[cfg(not(feature = "portable_simd"))]
    fn compute_scalar(fields: &[u64]) -> u64 { ... }

    pub fn update_incremental(old_hash: u64, old_val: u64, new_val: u64) -> u64 { ... }
    pub fn store(&self, hash: u64) { ... }
    pub fn load(&self) -> u64 { ... }
    pub fn verify(&self, expected: u64) -> bool { ... }
}
```

**Key Implementation Points**:
1. Use `u64x4` for SIMD version (nightly `portable_simd`)
2. Use `#[inline(always)]` on hot paths
3. Scalar fallback for stable Rust
4. XOR-based incremental update (3 operations)
5. Relaxed atomic ordering (no sync overhead)

**Reference**: See ARCHITECTURE_DESIGN.md § CapsuleHash64 Design

---

### File 2: `src/capsules/req_128_enhanced.rs` (Enhanced Capsule)

**Estimated Time**: 4 hours

**Structure**:
```rust
use crate::capsules::capsule_hash64::CapsuleHash64;

#[derive(ComputationalCapsule, Debug)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
pub struct RequestCapsule128Enhanced {
    // Core state (40 bytes)
    budget_cents: AtomicI64,
    total_spent: AtomicI64,
    request_count: AtomicU64,
    generation: AtomicU64,
    last_update_ns: AtomicU64,

    // Intrinsic metrics (24 bytes) ← NEW
    deduction_count: AtomicU32,
    failed_deductions: AtomicU32,
    hash: AtomicU64,
    prev_hash: AtomicU64,

    // Padding (64 bytes)
    _padding: [u8; 64],
}

impl RequestCapsule128Enhanced {
    pub fn new(initial_budget_cents: i64) -> Self { ... }
    fn compute_hash(&self) -> u64 { ... }
    pub fn try_deduct(&self, cost_cents: i64) -> ClapiResult<i64> { ... }
    pub fn credit(&self, amount_cents: i64) -> ClapiResult<i64> { ... }
    pub fn verify_integrity(&self) -> bool { ... }
    pub fn hash(&self) -> u64 { ... }
    pub fn metrics(&self) -> Option<Metrics> { ... }
}

#[derive(Debug, Clone, Copy)]
pub struct Metrics {
    pub budget_cents: i64,
    pub total_spent: i64,
    pub request_count: u64,
    pub deduction_count: u32,
    pub failed_deductions: u32,
    pub hash: u64,
}
```

**Key Implementation Points**:
1. Copy `try_deduct()` from `req_128.rs` and add hash update
2. Use `CapsuleHash64::update_incremental()` for <1ns updates
3. Update `prev_hash` before storing new hash (hash chain)
4. Increment `deduction_count` on success, `failed_deductions` on failure
5. `verify_integrity()` compares computed hash vs stored hash

**Reference**: See ARCHITECTURE_DESIGN.md § RequestCapsule128Enhanced Design

---

### File 3: `tests/capsule_hash64_tests.rs` (Unit Tests)

**Estimated Time**: 2 hours

**Test Coverage** (T28 Q1-Q7):
```rust
#[test]
fn test_hash_deterministic() { ... }

#[test]
fn test_incremental_update() { ... }

#[test]
fn test_alignment() { ... }

#[test]
fn test_size() { ... }

#[test]
fn property_no_collisions_1million() { ... }

#[test]
fn property_bit_flip_detection() { ... }
```

**Reference**: See ARCHITECTURE_DESIGN.md § Testing Strategy

---

### File 4: `tests/req_128_enhanced_tests.rs` (Integration Tests)

**Estimated Time**: 2 hours

**Test Coverage** (T28 Q15-Q21):
```rust
#[test]
fn integration_capsule_hash_update() { ... }

#[test]
fn integration_metrics_export() { ... }

#[test]
fn integration_verify_integrity() { ... }

#[test]
fn stress_concurrent_hash_updates() { ... }
```

**Reference**: See ARCHITECTURE_DESIGN.md § Testing Strategy

---

### File 5: `benches/capsule_hash64_bench.rs` (B32 Benchmarks)

**Estimated Time**: 2 hours

**Benchmarks**:
```rust
#[bench]
fn bench_hash_compute_simd(b: &mut Bencher) { ... }

#[bench]
fn bench_hash_compute_scalar(b: &mut Bencher) { ... }

#[bench]
fn bench_incremental_update(b: &mut Bencher) { ... }

#[bench]
fn bench_full_verification(b: &mut Bencher) { ... }
```

**Expected Results**:
- `hash_compute_simd`: 1.8ns ± 0.3ns (95% CI)
- `hash_compute_scalar`: 4.2ns ± 0.5ns (95% CI)
- `incremental_update`: 0.8ns ± 0.2ns (95% CI)
- `full_verification`: 85ns ± 15ns (95% CI)

**Reference**: See ARCHITECTURE_DESIGN.md § Testing Strategy

---

## Dependencies

**No new external dependencies required**:
- Uses existing `atomic_capsule_derive` for verification
- Uses existing `AtomicU64` from std
- Uses nightly `portable_simd` (optional, fallback to scalar)

**Cargo.toml updates**:
```toml
[features]
default = []
simd = ["portable_simd"]  # Optional SIMD feature
```

---

## Testing Commands

```bash
# Unit tests
cargo test --test capsule_hash64_tests
cargo test --test req_128_enhanced_tests

# All tests
cargo test

# Benchmarks
cargo bench --bench capsule_hash64_bench

# Nightly features (SIMD)
cargo +nightly test --features simd
cargo +nightly bench --features simd --bench capsule_hash64_bench
```

---

## Success Criteria

**Checklist**:
- [ ] CapsuleHash64 compiles with `#[derive(ComputationalCapsule)]`
- [ ] RequestCapsule128Enhanced compiles with alignment verification
- [ ] All unit tests pass (10+ tests)
- [ ] All property tests pass (collision resistance, bit flip detection)
- [ ] All integration tests pass (hash updates, verification)
- [ ] All stress tests pass (1M concurrent operations)
- [ ] B32 benchmarks validate <2ns SIMD, <5ns scalar
- [ ] Documentation complete (API docs + examples)

**Performance Targets**:
- ✅ Hash computation <2ns (SIMD), <5ns (scalar)
- ✅ Incremental update <1ns
- ✅ Verification <100ns
- ✅ Zero contention (Relaxed ordering)

---

## Common Pitfalls to Avoid

**1. Alignment Issues**:
- ❌ WRONG: Forgetting `#[repr(C, align(64))]`
- ✅ RIGHT: Always use `#[derive(ComputationalCapsule)]` for verification

**2. SIMD Fallback**:
- ❌ WRONG: Assuming SIMD is always available
- ✅ RIGHT: Use `#[cfg(feature = "portable_simd")]` and scalar fallback

**3. Memory Ordering**:
- ❌ WRONG: Using `Acquire/Release` for hash (unnecessary sync)
- ✅ RIGHT: Use `Relaxed` ordering (hash updates are independent)

**4. Incremental Update**:
- ❌ WRONG: Forgetting to XOR both old_value and new_value
- ✅ RIGHT: `old_hash ^ old_value ^ new_value`

**5. Hash Chain**:
- ❌ WRONG: Overwriting prev_hash without storing current first
- ✅ RIGHT: Store current → prev_hash, then store new → hash

---

## Reference Documents

**Primary**:
- [ARCHITECTURE_DESIGN.md](./ARCHITECTURE_DESIGN.md) - Complete UCE33 Q1-Q33 analysis (1,213 lines)

**Supporting**:
- [UCE33_CAPSULE_HASH64_ANALYSIS.md](./UCE33_CAPSULE_HASH64_ANALYSIS.md) - UCE33 analysis document
- [UCE33_FRAMEWORK.md](/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/UCE33_FRAMEWORK.md) - Framework reference
- [UCE33_TIER_REFERENCE.md](/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/UCE33_TIER_REFERENCE.md) - Tier details
- [B32_BENCHMARK_FRAMEWORK.md](/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/B32_BENCHMARK_FRAMEWORK.md) - Benchmarking
- [T28_TESTING_FRAMEWORK.md](/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/T28_TESTING_FRAMEWORK.md) - Testing

---

## Timeline

**Total Effort**: 14 hours (2 days)

| File | Hours | Description |
|------|-------|-------------|
| `capsule_hash64.rs` | 4 | Core hash primitive |
| `req_128_enhanced.rs` | 4 | Enhanced capsule |
| `capsule_hash64_tests.rs` | 2 | Unit + property tests |
| `req_128_enhanced_tests.rs` | 2 | Integration + stress tests |
| `capsule_hash64_bench.rs` | 2 | B32 benchmarks |

---

## Next Steps

**For Implementation Agent**:
1. Read [ARCHITECTURE_DESIGN.md](./ARCHITECTURE_DESIGN.md) (complete architecture)
2. Implement `capsule_hash64.rs` (4 hours)
3. Implement `req_128_enhanced.rs` (4 hours)
4. Write tests (4 hours)
5. Write benchmarks (2 hours)

**For Testing Agent**:
1. Review test strategy in ARCHITECTURE_DESIGN.md § Testing Strategy
2. Validate all tests pass (T28 framework)
3. Validate benchmarks meet targets (B32 framework)
4. Run stress tests (1M concurrent operations)

**For Documentation Agent**:
1. Update CLAUDE.md with Phase 3 status
2. Write migration guide (Phase 2 → Phase 3)
3. Write API documentation with examples
4. Write performance characteristics documentation

---

## Questions for Agents

**If you're unsure about**:
- **Alignment**: See ARCHITECTURE_DESIGN.md § Memory Layout
- **SIMD**: See ARCHITECTURE_DESIGN.md § Hash Algorithm
- **Incremental Update**: See ARCHITECTURE_DESIGN.md § Hash Algorithm (XOR formula)
- **Testing**: See ARCHITECTURE_DESIGN.md § Testing Strategy
- **Performance**: See ARCHITECTURE_DESIGN.md § Performance Characteristics

**If something is unclear**: Refer to the full architecture document (1,213 lines of detailed guidance).

---

## Delivery Checklist

**When you're done**:
- [ ] All files compile without warnings
- [ ] All tests pass (100% pass rate)
- [ ] B32 benchmarks validate performance targets
- [ ] Documentation complete (API docs + examples)
- [ ] CLAUDE.md updated with Phase 3 status
- [ ] Migration guide written
- [ ] Delivery report complete

**Expected Delivery**: 2 days (14 hours) from architecture handoff

---

**Status**: Architecture Complete, Ready for Implementation
**Architect**: Architecture Expert (UCE33 Q1-Q33 Complete)
**Next**: Implementation Agent + Testing Agent + Documentation Agent
