# ASSUM Phase 6.2 Pre-Implementation Design Review

**Date**: 2025-11-03
**Status**: 🟡 **AWAITING IMPLEMENTATION** (Phase 6.2 code does not exist yet)
**Auditor**: ASSUM Safety Expert
**Framework**: ASSUM Safety Framework (99.99% target)

---

## Executive Summary

**Finding**: Phase 6.2 Bloom sharding **has not been implemented yet**.

**Current Reality**:
- ❌ No `ShardedBloomFilterCapsule` exists in codebase
- ❌ No `BloomShardCapsule` exists in codebase
- ✅ Only `DedupBloomFilter` (single Bloom filter, no sharding)
- ✅ Phase 6.1 (SIMD MinHash) complete and awaiting commit

**Action Required**:
1. **Implement Phase 6.2** (estimated 4-6 hours)
2. **Then perform ASSUM audit** (cannot audit non-existent code)

**This Document**: Pre-implementation safety design review to guide Phase 6.2 implementation with 99.99% safety from day one.

---

## 1. Current State Analysis

### 1.1 Existing Bloom Filter (Pre-Phase 6.2)

**File**: `src/bloom_prefilter.rs` (186 lines)

```rust
pub struct DedupBloomFilter {
    filter: BloomFilterCapsule,        // Single filter (NO sharding)
    documents_seen: usize,             // Non-atomic counter
}
```

**Current Safety**:
- ✅ Zero unsafe code (`#![deny(unsafe_code)]` at crate level)
- ✅ Uses `atomic_capsule::probabilistic::BloomFilterCapsule` (99.99% safe)
- ✅ 3 tests passing (FPR <0.1%, skip rate >85%)

**Limitations** (why Phase 6.2 needed):
- ⚠️ Single filter = contention bottleneck under parallel writes
- ⚠️ Non-atomic counter = data race in multi-threaded context
- ⚠️ No SIMD hashing = scalar-only performance

### 1.2 Phase 6.2 Specification (from PHASE_6_1_FINAL_INTEGRATION_REPORT.md)

**Scope**:
1. Shard BloomPrefilter across N shards (reduce contention)
2. ThreadLocal shard selection (lockfree access)
3. SIMD Bloom filter hash (8-wide parallel, 2-4× speedup)
4. Property tests (false positive rate <0.1%)
5. Benchmarks (validate 50-90% reduction)

**Expected Performance**:
- 50-90% reduction in duplicate checks (Bloom pre-filter)
- Compound speedup: 7.1× (SIMD MinHash) × 10× (Bloom) = 71× total

---

## 2. Pre-Implementation Safety Design

### 2.1 Proposed Architecture (Safety-First)

```rust
/// Sharded Bloom filter with lockfree coordination
///
/// # Safety Properties
/// - Zero unsafe code (enforced by #![deny(unsafe_code)])
/// - AtomicU64 coordination only (no mutex/RwLock)
/// - Fixed 16 shards (no dynamic allocation)
/// - 256B cache-aligned capsule
#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256)]
#[repr(C, align(256))]
pub struct ShardedBloomFilterCapsule {
    /// 16 independent Bloom filter shards (lockfree access)
    /// Each shard: 4096 × AtomicU64 = 32 KB × 16 = 512 KB total
    shards: [BloomShardCapsule; 16],

    /// Total documents seen (atomic counter)
    documents_seen: AtomicU64,

    /// Padding to 256B alignment
    _padding: [u8; N],  // Calculated by fix_padding_fields tool
}

/// Single Bloom filter shard (lockfree, cache-aligned)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 32768)]  // 32 KB = 4096 × 8 bytes
#[repr(C, align(128))]
pub struct BloomShardCapsule {
    /// 4096 × AtomicU64 = 32,768 bytes = 262,144 bits
    bits: [AtomicU64; 4096],
}
```

### 2.2 Safety Assumptions (Pre-Verification Checklist)

| # | Assumption | Verification Strategy | Status |
|---|------------|----------------------|--------|
| 1 | **Zero unsafe code** | Compile-time check (`#![deny(unsafe_code)]`) | 🟡 Pre-impl |
| 2 | **16 shards sufficient** | Hash distribution test (1000+ inputs, <5% variance) | 🟡 Pre-impl |
| 3 | **AtomicU64::load(Relaxed) safe** | Concurrent load/store test (no corruption) | 🟡 Pre-impl |
| 4 | **FPR <0.08%** | Empirical FPR test (10K elements, measure actual FPR) | 🟡 Pre-impl |
| 5 | **No integer overflow** | Invariant: `bit_pos = (hash >> offset) % BITS < 262144` | 🟡 Pre-impl |
| 6 | **Shard index safe** | Invariant: `shard_idx = (hash as u32) & 0xF < 16` | 🟡 Pre-impl |
| 7 | **No buffer overflow** | Fixed 4096-element arrays (bounds-checked) | 🟡 Pre-impl |
| 8 | **No use-after-free** | Owned data, no lifetimes, no raw pointers | 🟡 Pre-impl |
| 9 | **No data races** | All shared state atomic (SeqCst for visibility) | 🟡 Pre-impl |
| 10 | **No deadlocks** | No locks (atomic-only coordination) | 🟡 Pre-impl |

**Target**: 10/10 assumptions verified → 99.99% ASSUM safety (EXCEPTIONAL tier)

### 2.3 Memory Safety Invariants

```rust
// INVARIANT 1: Shard index always in bounds
#[cfg(test)]
fn verify_shard_index_safe() {
    for hash in 0u64..=u64::MAX {
        let shard_idx = (hash as u32) & 0xF;
        assert!(shard_idx < 16, "Shard index out of bounds");
    }
}

// INVARIANT 2: Bit position always in bounds
#[cfg(test)]
fn verify_bit_position_safe() {
    const BITS_PER_SHARD: u64 = 262_144;  // 4096 × 64 bits
    for hash in 0u64..=u64::MAX {
        for offset in [0, 16, 32] {  // 3 hash functions
            let bit_pos = (hash >> offset) % BITS_PER_SHARD;
            assert!(bit_pos < BITS_PER_SHARD, "Bit position overflow");
        }
    }
}

// INVARIANT 3: No alignment violations
#[cfg(test)]
fn verify_alignment() {
    use std::mem::{align_of, size_of};
    assert_eq!(align_of::<ShardedBloomFilterCapsule>(), 256);
    assert_eq!(align_of::<BloomShardCapsule>(), 128);
    assert_eq!(size_of::<ShardedBloomFilterCapsule>(), 256);
}
```

### 2.4 Concurrency Safety (Lockfree Patterns)

**Pattern 1: Shard-local atomics (zero contention)**
```rust
impl BloomShardCapsule {
    /// Insert hash into shard (lockfree, <10ns)
    pub fn insert(&self, hash: u64) {
        // SAFE: Atomic OR is commutative (no CAS loop needed)
        for offset in [0, 16, 32] {  // 3 hash functions
            let bit_pos = (hash >> offset) % BITS_PER_SHARD;
            let word_idx = (bit_pos / 64) as usize;
            let bit_mask = 1u64 << (bit_pos % 64);

            // SAFE: fetch_or with Relaxed (order doesn't matter for Bloom)
            self.bits[word_idx].fetch_or(bit_mask, Ordering::Relaxed);
        }
    }

    /// Query hash in shard (lockfree, <5ns)
    pub fn might_contain(&self, hash: u64) -> bool {
        // SAFE: load(Relaxed) safe for read-only check
        for offset in [0, 16, 32] {
            let bit_pos = (hash >> offset) % BITS_PER_SHARD;
            let word_idx = (bit_pos / 64) as usize;
            let bit_mask = 1u64 << (bit_pos % 64);

            let word = self.bits[word_idx].load(Ordering::Relaxed);
            if (word & bit_mask) == 0 {
                return false;  // Definitely not in set
            }
        }
        true  // Might be in set (or false positive)
    }
}
```

**Pattern 2: ThreadLocal shard selection (zero synchronization)**
```rust
impl ShardedBloomFilterCapsule {
    /// Get shard for hash (deterministic, <1ns)
    #[inline(always)]
    fn get_shard(&self, hash: u64) -> &BloomShardCapsule {
        // SAFE: (hash & 0xF) always < 16
        let shard_idx = (hash as u32) & 0xF;
        &self.shards[shard_idx as usize]
    }

    /// Insert document (lockfree, <10ns per shard)
    pub fn insert(&self, doc_id: usize, text: &str) {
        let hash = Self::hash_document(doc_id, text);
        let shard = self.get_shard(hash);
        shard.insert(hash);

        // SAFE: Relaxed counter (audit only, no coordination)
        self.documents_seen.fetch_add(1, Ordering::Relaxed);
    }
}
```

### 2.5 Feature Flag Safety

**Feature**: `bloom-shards` (optional, backward-compatible)

```toml
[features]
default = []
bloom-shards = ["atomic_capsule/probabilistic"]  # T10 Probabilistic tier
```

**Compilation Safety**:
- ✅ Feature disabled: Use single `DedupBloomFilter` (current behavior)
- ✅ Feature enabled: Use `ShardedBloomFilterCapsule` (Phase 6.2)
- ✅ Both configurations must compile independently
- ✅ No feature interaction bugs (atomic_capsule primitives stable)

---

## 3. Implementation Checklist (Safety-First)

### 3.1 Phase 6.2 Implementation Order

1. **Step 1: Define capsule structures** (1 hour)
   - `BloomShardCapsule` (128B aligned, 32 KB)
   - `ShardedBloomFilterCapsule` (256B aligned, 512 KB + 16 shards)
   - Use `#[derive(ComputationalCapsule)]` for automatic verification

2. **Step 2: Implement lockfree operations** (1 hour)
   - `insert()`: fetch_or (Relaxed) for each hash function
   - `might_contain()`: load (Relaxed) for query
   - `get_shard()`: Deterministic shard selection (hash & 0xF)

3. **Step 3: Write safety tests** (1 hour)
   - Shard index bounds (test all u64 values)
   - Bit position bounds (test all hash × offset combinations)
   - Concurrent insert/query (1000+ threads, no corruption)
   - Hash distribution (1000+ inputs, <5% variance)

4. **Step 4: Property tests** (30 min)
   - FPR <0.08% (10K elements, measure actual FPR)
   - Recall 99.92%+ (no false negatives except FPR)
   - Determinism (same hash → same shard)

5. **Step 5: Benchmarks** (30 min)
   - Insert latency (<10ns target)
   - Query latency (<5ns target)
   - Skip rate (50-90% on duplicate-heavy corpus)

6. **Step 6: ASSUM audit** (1 hour)
   - Verify all 10 assumptions
   - Document zero unsafe code
   - Classify safety: 99.99% EXCEPTIONAL

**Total Estimated Effort**: 5 hours (consistent with 4-6 hour estimate)

### 3.2 ASSUM Audit Checklist (Post-Implementation)

**After implementation, ASSUM audit will verify**:

```markdown
## Unsafe Code Audit
- [ ] ShardedBloomFilterCapsule: ZERO unsafe code
- [ ] BloomShardCapsule: ZERO unsafe code
- [ ] Atomic operations: Safe abstractions only (fetch_or, load)
- [ ] Hash functions: No undefined behavior (DefaultHasher)

## Assumption Verification
- [ ] #ASSUME_SHARDS_SUFFICIENT: Hash distribution test (1000+ inputs)
- [ ] #ASSUME_ATOMIC_SAFE: Concurrent load/store test (no corruption)
- [ ] #ASSUME_FPR_LOW: Empirical FPR test (<0.08%)
- [ ] #ASSUME_NO_OVERFLOW: Bit position invariant (always <262144)
- [ ] #ASSUME_SHARD_BOUNDS: Shard index invariant (always <16)

## Memory Safety
- [ ] No buffer overflows (fixed 4096-element arrays)
- [ ] No use-after-free (owned data, no lifetimes)
- [ ] No uninitialized memory (all fields zeroed or atomic)
- [ ] No alignment violations (256B/128B aligned capsules)

## Concurrency Safety
- [ ] No data races (all shared state atomic)
- [ ] No deadlocks (no locks, atomic-only)
- [ ] No ABA problems (atomic loads, no CAS loops)
- [ ] Relaxed ordering safe (counters audit-only, bits commutative)

## Feature Flag Safety
- [ ] bloom-shards feature compiles independently
- [ ] Both enabled/disabled build clean
- [ ] No feature interaction bugs
```

**Deliverable**: 150-250 line ASSUM audit report with 10/10 verified assumptions

---

## 4. Safety Properties (Guaranteed by Design)

### 4.1 Compile-Time Safety

| Property | Enforcement | Verification |
|----------|-------------|--------------|
| Zero unsafe code | `#![deny(unsafe_code)]` | Compiler error if violated |
| Capsule alignment | `#[derive(ComputationalCapsule)]` | Compile-time check (<20ms) |
| Fixed array bounds | `[AtomicU64; 4096]` | Rust bounds checking (no overflow) |
| Feature independence | Cargo feature flags | Both configs build clean |

### 4.2 Runtime Safety

| Property | Enforcement | Test Coverage |
|----------|-------------|---------------|
| Shard index bounds | `(hash & 0xF) < 16` | Property test (all u64 values) |
| Bit position bounds | `(hash % BITS) < 262144` | Property test (all hash×offset) |
| No data races | AtomicU64 only | Loom/Miri (concurrency model checking) |
| FPR <0.08% | 3 hash functions | Empirical test (10K elements) |

### 4.3 ASSUM Safety Classification

**Target**: 99.99% EXCEPTIONAL tier

**Requirements**:
- 10/10 assumptions verified with test evidence
- Zero unsafe code (enforced by compiler)
- Zero unverified assumptions
- Zero known failure modes

**Current Status**: 🟡 **AWAITING IMPLEMENTATION**

---

## 5. Recommended Next Steps

### 5.1 For Implementation Team

**Priority**: Implement Phase 6.2 with safety-first design

1. Read this pre-implementation review (this document)
2. Follow implementation checklist (Section 3.1)
3. Write tests BEFORE implementation (TDD approach)
4. Use `#[derive(ComputationalCapsule)]` for automatic verification
5. Request ASSUM audit after implementation complete

**Files to Create**:
- `src/bloom_sharding.rs` (main implementation, ~300 lines)
- `tests/bloom_sharding_tests.rs` (T28 comprehensive tests, ~200 lines)
- `benches/bloom_sharding_bench.rs` (B32 benchmarks, ~100 lines)

### 5.2 For ASSUM Auditor (Post-Implementation)

**Trigger**: When implementation complete, run ASSUM audit

**Checklist**:
1. Verify zero unsafe code (`grep -r "unsafe" src/bloom_sharding.rs`)
2. Run all tests (`cargo test --features bloom-shards`)
3. Verify 10 assumptions with test evidence
4. Document safety classification (99.99% target)
5. Write ASSUM audit report (150-250 lines)

**Deliverable**: `ASSUM_PHASE62_BLOOM_SHARDING_AUDIT.md`

---

## 6. Conclusion

**Finding**: Phase 6.2 Bloom sharding has not been implemented yet.

**This Document**: Pre-implementation safety design to guide implementation with 99.99% safety from day one.

**Action Required**:
1. ✅ **Read this pre-implementation review** (completed)
2. 🟡 **Implement Phase 6.2** (4-6 hours, safety-first design)
3. 🟡 **Request ASSUM audit** (after implementation complete)

**Safety Confidence**: 99.99% achievable with proposed design (zero unsafe code, atomic-only coordination, fixed arrays, compile-time verification).

**Next Phase**: Hand off to implementation team with safety checklist.

---

**Document Status**: ✅ COMPLETE (Pre-Implementation Design Review)

**Next Document**: `ASSUM_PHASE62_BLOOM_SHARDING_AUDIT.md` (after implementation)

---

**Generated**: 2025-11-03
**Framework**: ASSUM Safety Framework
**Classification**: Pre-Implementation Design Review
**Target Safety**: 99.99% EXCEPTIONAL (zero unsafe code)
