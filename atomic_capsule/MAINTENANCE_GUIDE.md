# Atomic Capsule Maintenance Guide

**Version**: 1.0
**Date**: 2025-10-20
**Scope**: Production-ready maintenance procedures
**Target**: Developers maintaining atomic_capsule ecosystem

---

## Table of Contents

1. [Quick Reference](#quick-reference)
2. [Common Tasks](#common-tasks)
3. [Debugging Procedures](#debugging-procedures)
4. [Performance Monitoring](#performance-monitoring)
5. [Migration Procedures](#migration-procedures)
6. [Emergency Procedures](#emergency-procedures)

---

## Quick Reference

### Essential Commands

```bash
# Development workflow
cargo test --lib --all-features                    # Run all tests
cargo clippy --all-features -- -D warnings        # Zero warnings
cargo bench --bench nightly_optimizations_bench    # Performance validation
cargo +nightly miri test                          # UB detection

# Feature testing
cargo test --no-default-features                   # Minimal (no_std)
cargo test --features "std"                        # Standard library only
cargo test --features "nightly-all"                # All nightly features
cargo test --features "profile-production"         # Production preset

# Documentation
cargo doc --no-deps --all-features --open         # Generate docs
cargo readme > README.md                           # Update README

# Migration
capsule-migrate --dry-run .                        # Preview migration
capsule-migrate --apply .                          # Apply migration

# Performance regression
cargo bench -- --save-baseline main                # Baseline
cargo bench -- --baseline main                     # Compare
```

### File Structure

```
atomic_capsule/
├── src/
│   ├── lib.rs                    # Crate root (re-exports)
│   ├── alignment.rs              # Alignment tiers (HotTier, WarmTier, ColdTier)
│   ├── arch.rs                   # Architecture detection
│   ├── retry.rs                  # Retry policies
│   ├── verification.rs           # Compile-time macros
│   ├── traits.rs                 # Capsule trait hierarchy (Q33)
│   ├── primitives/               # SIMD capsules (T2)
│   ├── hash/                     # Hash module (T0 auditable)
│   ├── collections/              # Lockfree data structures (T1+T4)
│   ├── serialize/                # Deterministic serialization (T0)
│   └── forensics/                # Compliance audit trails
├── tests/                        # Integration tests
├── benches/                      # Criterion benchmarks
├── examples/                     # Usage examples
└── docs/                         # Documentation
    ├── PHASE*.md                 # Phase deliverables
    ├── ARCHITECTURE.md           # 6-tier taxonomy
    └── MIGRATION.md              # Migration guide
```

### Contact Points

| Component | Owner | Documentation |
|-----------|-------|---------------|
| **Core Foundation** | atomic_capsule crate | `/home/samuel/Primitives/atomic_capsule/` |
| **Derive Macro** | atomic_capsule_derive | `/home/samuel/Primitives/atomic_capsule_derive/` |
| **Clippy Lint** | clippy-capsule-verify | `/home/samuel/Primitives/clippy-capsule-verify/` |
| **Serialization** | serialize module | `atomic_capsule/src/serialize/` |
| **Collections** | collections module | `atomic_capsule/src/collections/` |

---

## Common Tasks

### Task 1: Adding a New FixedPoint Type

**Scenario**: Add Q24.40 (high-precision financial calculations)

**Steps**:

1. **Define Type** (src/fixed_point.rs):

```rust
/// Q24.40 fixed-point (24 integer bits, 40 fractional bits)
///
/// # Range
/// - Integer: -8,388,608 to 8,388,607 (±8M)
/// - Precision: 1 / 2^40 ≈ 0.0000000009 (sub-nanosecond)
///
/// # Use Cases
/// - High-frequency trading (sub-tick precision)
/// - Scientific calculations (high precision required)
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct FixedQ24_40(i64);

impl FixedQ24_40 {
    pub const FRACTIONAL_BITS: u32 = 40;
    pub const SCALE: i64 = 1 << 40;  // 2^40

    pub fn from_raw(raw: i64) -> Self {
        Self(raw)
    }

    pub fn to_raw(self) -> i64 {
        self.0
    }

    pub fn from_f64(value: f64) -> Self {
        Self((value * (Self::SCALE as f64)) as i64)
    }

    pub fn to_f64(self) -> f64 {
        (self.0 as f64) / (Self::SCALE as f64)
    }
}
```

2. **Implement CapsuleSerialize** (src/serialize/impls.rs):

```rust
impl CapsuleSerialize for FixedQ24_40 {
    fn serialize_deterministic(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(14);
        buf.extend_from_slice(&0x51323440_u32.to_le_bytes());  // "Q24@" magic
        buf.extend_from_slice(&1_u16.to_le_bytes());           // Version 1
        buf.extend_from_slice(&self.to_raw().to_le_bytes());   // i64 raw value
        buf
    }

    fn deserialize_from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
        // Validate buffer size
        if bytes.len() < 14 {
            return Err(SerializeError::BufferTooSmall {
                required: 14,
                actual: bytes.len(),
            });
        }

        // Validate magic number
        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        if magic != 0x51323440 {
            return Err(SerializeError::InvalidMagic {
                expected: 0x51323440,
                actual: magic,
            });
        }

        // Validate version
        let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        if version != 1 {
            return Err(SerializeError::VersionMismatch {
                expected: 1,
                actual: version,
            });
        }

        // Deserialize raw value
        let raw = i64::from_le_bytes(bytes[6..14].try_into().unwrap());
        Ok(Self::from_raw(raw))
    }

    fn serialized_size() -> usize {
        14  // 4 (magic) + 2 (version) + 8 (i64)
    }

    fn verify_roundtrip(&self) -> bool {
        let bytes = self.serialize_deterministic();
        if let Ok(restored) = Self::deserialize_from_bytes(&bytes) {
            restored.to_raw() == self.to_raw()
        } else {
            false
        }
    }

    fn verify_determinism(&self) -> bool {
        let bytes1 = self.serialize_deterministic();
        let bytes2 = self.serialize_deterministic();
        bytes1 == bytes2
    }
}
```

3. **Add Tests** (src/serialize/impls.rs):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_q24_40_roundtrip() {
        let value = FixedQ24_40::from_f64(1234.56789);
        let bytes = value.serialize_deterministic();
        let restored = FixedQ24_40::deserialize_from_bytes(&bytes).unwrap();
        assert_eq!(value.to_raw(), restored.to_raw());
    }

    #[test]
    fn test_fixed_q24_40_determinism() {
        let value = FixedQ24_40::from_f64(9999.99999);
        let bytes1 = value.serialize_deterministic();
        let bytes2 = value.serialize_deterministic();
        assert_eq!(bytes1, bytes2);
    }

    #[test]
    fn test_fixed_q24_40_precision() {
        // Test sub-nanosecond precision (1e-9)
        let value = FixedQ24_40::from_f64(0.0000000001);  // 1e-10
        let bytes = value.serialize_deterministic();
        let restored = FixedQ24_40::deserialize_from_bytes(&bytes).unwrap();
        assert!((restored.to_f64() - 0.0000000001).abs() < 1e-12);
    }

    #[test]
    fn test_fixed_q24_40_range() {
        // Test maximum positive value
        let max = FixedQ24_40::from_raw(i64::MAX);
        assert!(max.verify_roundtrip());

        // Test maximum negative value
        let min = FixedQ24_40::from_raw(i64::MIN);
        assert!(min.verify_roundtrip());
    }
}
```

4. **Update Documentation**:
   - Add entry to `PHASE1_IMPLEMENTATION_COMPLETE.md`
   - Update `serialize/mod.rs` doc comment
   - Add usage example to README.md

**Effort**: 30-45 minutes
**Testing**: `cargo test --lib --features "capsule-serialize"`

---

### Task 2: Migrating from Manual Macro to Derive

**Scenario**: Migrate BudgetSlotCapsule from clapi_core

**Before** (manual verification):
```rust
// clapi_core/src/types.rs
use atomic_capsule::verify_capsule_properties;

#[repr(C, align(64))]
pub struct BudgetSlotCapsule {
    state: AtomicU64,
    generation: AtomicU64,
    _padding: [u8; 48],
}

// Manual verification (Phase 1 pattern)
verify_capsule_properties!(BudgetSlotCapsule, 64, 64);
```

**After** (automatic derive):
```rust
// clapi_core/src/types.rs
use atomic_capsule_derive::ComputationalCapsule;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct BudgetSlotCapsule {
    state: AtomicU64,
    generation: AtomicU64,
    _padding: [u8; 48],
}

// Verification now automatic at compile-time!
```

**Migration Steps**:

1. **Add Dependency** (Cargo.toml):
```toml
[dependencies]
atomic_capsule = { version = "0.2", features = ["derive"] }
atomic_capsule_derive = "0.1"
```

2. **Replace Macro**:
```diff
- verify_capsule_properties!(BudgetSlotCapsule, 64, 64);
+ #[derive(ComputationalCapsule)]
+ #[capsule(alignment = 64, size = 64)]
```

3. **Add Attribute**:
```diff
+ #[derive(ComputationalCapsule)]
+ #[capsule(alignment = 64, size = 64)]
  #[repr(C, align(64))]
  pub struct BudgetSlotCapsule {
```

4. **Validate**:
```bash
cargo test --lib
cargo clippy -- -D warnings
```

**Automated Migration** (recommended):
```bash
# Install tool
cargo install capsule-migrate

# Preview changes
capsule-migrate --dry-run clapi_core/

# Apply migration
capsule-migrate --apply clapi_core/

# Validate
cd clapi_core && cargo test
```

**Effort**: 2 minutes per capsule (manual), 5 seconds per project (automated)

---

### Task 3: Adding a New Feature Flag

**Scenario**: Add `profile-embedded` for IoT/embedded systems

**Steps**:

1. **Define Feature** (Cargo.toml):
```toml
[features]
# Phase X: Embedded Systems Profile
# #ASSUME_EMBEDDED: CRC32 sufficient for integrity (no crypto needed)
# #VERIFY_EMBEDDED: Binary size <10KB overhead validated via CI
#
# Performance claims (B32 framework):
# - Binary size: +5KB (crc32fast only, measured with cargo-bloat)
# - Compile time: <10ms (no heavy deps, measured with cargo build --timings)
# - Runtime cost: <20ns per CRC32 (hardware-accelerated on ARM Cortex-M4+)
#
# Use cases:
# - Embedded systems (STM32, ESP32, nRF52)
# - IoT devices (battery-powered, limited RAM)
# - no_std environments (bare metal, RTOS)
#
# Recommended hardware:
# - ARM Cortex-M4+ (hardware CRC32)
# - RISC-V with Bitmanip extension
# - Avoid: Cortex-M0/M0+ (software CRC32 only)
profile-embedded = ["crc32fast"]
```

2. **Add Conditional Compilation** (src/lib.rs):
```rust
#[cfg(feature = "profile-embedded")]
pub mod embedded {
    //! Embedded systems utilities
    //!
    //! This module provides minimal-overhead serialization for embedded systems.
    //! All features use `no_std` compatible implementations.

    pub use crate::serialize::CapsuleSerialize;
    pub use crc32fast;

    /// Validate binary size budget for embedded deployment
    ///
    /// # Example
    /// ```
    /// use atomic_capsule::embedded::validate_binary_size;
    ///
    /// let size = std::mem::size_of::<MyCapsule>();
    /// assert!(validate_binary_size(size, 10_000));  // Max 10KB
    /// ```
    pub fn validate_binary_size(actual: usize, budget: usize) -> bool {
        actual <= budget
    }
}
```

3. **Add Tests** (tests/feature_profiles.rs):
```rust
#[test]
#[cfg(feature = "profile-embedded")]
fn test_embedded_profile_binary_size() {
    use atomic_capsule::embedded::validate_binary_size;

    // Validate capsule sizes for embedded constraints
    let size = std::mem::size_of::<crate::types::SmallCapsule>();
    assert!(
        validate_binary_size(size, 128),
        "SmallCapsule too large for embedded ({}B > 128B)",
        size
    );
}

#[test]
#[cfg(feature = "profile-embedded")]
fn test_embedded_profile_no_std() {
    // Verify no_std compatibility
    // This test compiles without std, validating embedded support
    #[cfg(not(feature = "std"))]
    {
        use atomic_capsule::embedded::CapsuleSerialize;
        // Test basic serialization without heap allocation
        let value = 42_u64;
        let bytes = value.serialize_deterministic();
        assert_eq!(bytes.len(), 14);
    }
}
```

4. **CI Integration** (.github/workflows/ci.yml):
```yaml
- name: Test embedded profile
  run: |
    cargo test --no-default-features --features "profile-embedded"
    cargo build --release --no-default-features --features "profile-embedded"
    cargo bloat --release --features "profile-embedded" > bloat.txt
    # Validate binary size <10KB overhead
```

5. **Update Documentation** (README.md, CLAUDE.md):
```markdown
## Feature Presets

| Preset | Dependencies | Binary Size | Use Case |
|--------|--------------|-------------|----------|
| `profile-embedded` | crc32fast | +5KB | IoT/Embedded (no crypto) |
| `profile-development` | xxhash-rust | +8KB | Development (fast hash) |
| `profile-production` | xxhash+blake3 | +23KB | Production (audit trail) |
| `profile-government` | All hashes | +50KB | Regulated (FIPS compliance) |

### Embedded Systems Example
```rust
// Cargo.toml
[dependencies]
atomic_capsule = { version = "0.2", default-features = false, features = ["profile-embedded"] }

// main.rs
#![no_std]
use atomic_capsule::embedded::CapsuleSerialize;

let sensor_data = 42_u64;
let bytes = sensor_data.serialize_deterministic();
// Send to cloud via LoRaWAN...
```
```

**Effort**: 2-3 hours (definition + tests + docs + CI)
**Validation**: Binary size measurement, no_std compilation

---

## Debugging Procedures

### Procedure 1: Compilation Error - Type Mismatch

**Symptom**:
```
error[E0308]: mismatched types
 --> src/collections/concurrent_map.rs:1073:24
  |
  | map.insert(i, i * 10);
  |            ^ expected `u64`, found `usize`
```

**Root Cause**: Loop variable type mismatch

**Fix**:
```rust
// BEFORE (incorrect)
for i in 0..count {
    map.insert(i, i * 10);  // ERROR: i is usize
}

// AFTER (correct)
for i in 0..count {
    map.insert(i as u64, (i * 10) as u64);  // FIXED
}
```

**Checklist**:
- [ ] Identify loop variable type (`usize`, `u32`, `u64`)
- [ ] Check map key/value type (`ConcurrentMapCapsule<K, V>`)
- [ ] Add explicit casts (`as u64`)
- [ ] Run `cargo test --lib` to validate

**Estimated Fix Time**: 2 minutes

---

### Procedure 2: Alignment Mismatch Error

**Symptom**:
```
Capsule alignment mismatch for MyCapsule
  Expected: 64 bytes
  Actual: 8 bytes
  Help: Update #[repr(C, align(64))] attribute
```

**Root Cause**: Missing or incorrect `#[repr(C, align(N))]` attribute

**Debug Steps**:

1. **Check Current Alignment**:
```rust
println!("Alignment: {}", std::mem::align_of::<MyCapsule>());
println!("Size: {}", std::mem::size_of::<MyCapsule>());
```

2. **Fix Alignment**:
```rust
// BEFORE (incorrect)
#[repr(C)]  // Missing align!
struct MyCapsule {
    state: AtomicU64,
}

// AFTER (correct)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]  // Explicit alignment
struct MyCapsule {
    state: AtomicU64,
    _padding: [u8; 56],  // Pad to 64 bytes
}
```

3. **Validate**:
```bash
cargo test --lib
# Should pass if alignment correct
```

**Estimated Fix Time**: 5 minutes

---

### Procedure 3: Size Mismatch Error

**Symptom**:
```
Capsule size mismatch for MyCapsule
  Expected: 64 bytes
  Actual: 72 bytes (8 bytes padding added by compiler)
```

**Root Cause**: Compiler added padding for alignment

**Debug Steps**:

1. **Calculate Expected Size**:
```rust
// Field sizes:
// - AtomicU64: 8 bytes
// - AtomicU64: 8 bytes
// Total: 16 bytes
//
// With 64-byte alignment:
// - Compiler pads to next multiple of 64 = 64 bytes
// - Actual padding: 64 - 16 = 48 bytes
```

2. **Fix** (Option 1: Explicit Padding):
```rust
#[repr(C, align(64))]
struct MyCapsule {
    state: AtomicU64,     // 8 bytes
    generation: AtomicU64, // 8 bytes
    _padding: [u8; 48],    // 48 bytes (explicit)
}  // Total: 64 bytes ✓
```

3. **Fix** (Option 2: Adjust Size Expectation):
```rust
// Accept compiler-generated size
verify_capsule_properties!(MyCapsule, 64, 72);  // 72 instead of 64
```

4. **Validate**:
```rust
assert_eq!(std::mem::size_of::<MyCapsule>(), 64);
```

**Estimated Fix Time**: 5 minutes

---

### Procedure 4: Benchmark Regression Detection

**Symptom**: Performance degradation after code change

**Steps**:

1. **Establish Baseline** (before change):
```bash
cd atomic_capsule
git checkout main
cargo bench --bench nightly_optimizations_bench -- --save-baseline main
```

2. **Make Code Changes**:
```bash
git checkout feature-branch
# ... edit code ...
```

3. **Measure New Performance**:
```bash
cargo bench --bench nightly_optimizations_bench -- --baseline main
```

4. **Interpret Results**:
```
const_hash_static_id  time:   [0.00 ns 0.00 ns 0.00 ns]
                      change: [+0.00% +0.00% +0.00%] (no change detected)

simd_hash_4_fields    time:   [9.8 ns 10.1 ns 10.4 ns]
                      change: [+15.3% +18.2% +21.1%] (regression detected)
                      Performance has regressed.
```

5. **Investigate Regression**:
```bash
# Diff the changed code
git diff main feature-branch src/hash/simd_hash.rs

# Profile the hot path
cargo flamegraph --bench simd_hash_bench

# Analyze assembly
cargo asm atomic_capsule::hash::simd_hash::hash_fields --rust
```

6. **Decision Tree**:
```
Is regression >5%?
├─ NO: Accept (within noise threshold)
└─ YES: Is regression intentional (e.g., safety fix)?
    ├─ YES: Document trade-off, update B32 claims
    └─ NO: Investigate root cause
        ├─ Compiler optimization disabled?
        ├─ SIMD intrinsic changed?
        ├─ Cache alignment broken?
        └─ Fix or revert
```

**Estimated Time**: 30 minutes investigation, 1-4 hours fix

---

## Performance Monitoring

### Continuous Benchmarking

**Setup** (one-time):
```bash
# Install criterion
cargo install cargo-criterion

# Configure CI (`.github/workflows/benchmark.yml`)
name: Benchmark
on: [push, pull_request]
jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run benchmarks
        run: cargo bench --all-features
      - name: Upload results
        uses: actions/upload-artifact@v3
        with:
          name: benchmark-results
          path: target/criterion/
```

**Monitoring Targets** (B32 framework):
```toml
[target.bench]
# Tier 0: Auditable Foundation
const_hash_static_id        < 0.5 ns   # Compile-time evaluated
crc32_checksum              < 20 ns    # Hardware-accelerated

# Tier 1: Atomic Operations
atomic_load_acquire         < 5 ns     # Single cache line
dual_atomic_cas             < 15 ns    # Two cache lines

# Tier 2: SIMD Operations
simd_hash_4_fields          < 10 ns    # u64x4 vectorization
simd_hash_8_fields          < 15 ns    # u64x8 vectorization

# Tier 4: Batch Operations
batch_serialize_1k          < 50 µs    # 1000 records batched
concurrent_map_insert_p99   < 20 µs    # 99th percentile latency
```

**Alert Thresholds**:
- **WARNING**: >5% regression
- **CRITICAL**: >10% regression
- **INVESTIGATE**: >2× speedup (likely measurement error)

---

## Migration Procedures

### Migrating from Phase 1 to Phase 2 (Manual → Automatic Verification)

**Scope**: 618 manual macro instances across 7 projects

**Automated Tool** (recommended):
```bash
# Install
cargo install capsule-migrate

# Migrate all projects
cd /home/samuel/Primitives
capsule-migrate --apply atomic_capsule
capsule-migrate --apply clapi_core
capsule-migrate --apply kiang
capsule-migrate --apply kindly-db
capsule-migrate --apply kindly_hft
capsule-migrate --apply atomic_hedge_capsule
capsule-migrate --apply atomic_venue_snapshot

# Validate
for project in atomic_capsule clapi_core kiang kindly-db kindly_hft atomic_hedge_capsule atomic_venue_snapshot; do
    cd $project
    cargo test --lib
    cd ..
done
```

**Manual Migration** (if tool unavailable):

1. **Update Cargo.toml**:
```toml
[dependencies]
atomic_capsule = { version = "0.2", features = ["derive"] }
atomic_capsule_derive = "0.1"
```

2. **Find All Manual Macros**:
```bash
rg "verify_capsule_properties!" src/ -A 1
```

3. **Replace Each Instance**:
```rust
// BEFORE
verify_capsule_properties!(MyCapsule, 64, 64);

// AFTER
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
```

4. **Validate**:
```bash
cargo test --lib
cargo clippy -- -D warnings
```

**Effort**:
- Automated: 5 seconds per project (618 instances in 30 seconds)
- Manual: 2 minutes per capsule (618 × 2min ≈ 20 hours)

**Timeline**: Phase 2 complete → v0.5.0 (manual deprecated) → v0.6.0 (manual removed)

---

## Emergency Procedures

### Emergency 1: Production Panic (Zero Panics Policy Violation)

**Symptom**: Panic in production code (UNACCEPTABLE)

**Immediate Response** (within 5 minutes):

1. **Isolate Failure**:
```bash
# Get panic backtrace
RUST_BACKTRACE=full cargo run 2>&1 | tee panic.log

# Identify panic location
grep "panicked at" panic.log
```

2. **Hotfix** (if trivial):
```rust
// BEFORE (panicking)
let value = map.get(&key).unwrap();  // NEVER DO THIS!

// AFTER (Result propagation)
let value = map.get(&key).ok_or(Error::KeyNotFound)?;
```

3. **Deploy Fix**:
```bash
git commit -m "HOTFIX: Remove panic in production path"
cargo test --lib
cargo build --release
# Deploy immediately
```

**Root Cause Analysis** (within 24 hours):

1. **Why did this panic occur?**
   - Missing error handling (unwrap/expect)
   - Violated invariant (debug_assert! in release)
   - Hardware failure (OOM, disk full)

2. **How did it pass tests?**
   - Missing test case for this path
   - Test coverage gap
   - Integration test needed

3. **Preventive Measures**:
   - Add `#![deny(clippy::unwrap_used)]` to crate root
   - Add property tests for all error paths
   - Review all `.unwrap()` calls (should be ZERO in hot paths)

**Documentation**:
```markdown
# Incident Report: Production Panic

**Date**: 2025-10-20
**Severity**: CRITICAL (zero panics policy violation)
**Component**: concurrent_map.rs:insert()
**Root Cause**: unwrap() on None value (missing key check)

## Timeline
- 14:23 UTC: Panic reported in production
- 14:28 UTC: Hotfix deployed (unwrap → ok_or)
- 14:30 UTC: Service restored
- 15:00 UTC: Root cause identified (missing test)
- 16:00 UTC: Property test added (prevent recurrence)

## Preventive Actions
- Added clippy::unwrap_used lint
- Added 20 property tests for error paths
- Updated test coverage from 0.69 → 0.75 ratio
```

**Effort**: 5 minutes hotfix, 24 hours RCA, 4 hours prevention

---

### Emergency 2: Undefined Behavior Detected (Miri Failure)

**Symptom**: `cargo +nightly miri test` reports UB

**Immediate Response** (within 1 hour):

1. **Reproduce Locally**:
```bash
cargo +nightly miri test --test <failing_test>
```

2. **Identify UB Type**:
```
error: Undefined Behavior: dereferencing pointer failed: 0x12345678 is not a valid pointer
   --> src/collections/concurrent_map.rs:628
    |
628 | return Some(unsafe { &*ptr });
    |                      ^^^^^ dereferencing pointer failed
```

**Common UB Patterns**:

| UB Type | Cause | Fix |
|---------|-------|-----|
| **Invalid pointer dereference** | ptr was freed | Validate ptr before deref |
| **Data race** | Relaxed ordering on sync | Use Acquire/Release |
| **Misaligned access** | Unaligned struct overlay | Validate alignment runtime |
| **Uninitialized memory** | Reading padding bytes | Zero-initialize padding |

**Fix Template**:
```rust
// BEFORE (UB)
unsafe { &*ptr }  // No validation!

// AFTER (SAFE)
// SAFETY: Validated in insert() that ptr != null, ptr is aligned,
// ptr points to valid Box allocation, lifetime tied to map.
unsafe {
    debug_assert!(!ptr.is_null());
    debug_assert!(ptr as usize % std::mem::align_of::<V>() == 0);
    &*ptr
}
```

3. **Add ASSUM Documentation**:
```rust
/// # ASSUM Framework
/// - `#ASSUME_PTR_VALID`: ptr was allocated via Box::into_raw, never freed
/// - `#VERIFY_PTR_VALID`: Validated at allocation in insert(), not freed until Drop
/// - `#ASSUME_PTR_ALIGNED`: Box guarantees alignment matches std::mem::align_of::<V>()
/// - `#VERIFY_PTR_ALIGNED`: Box allocator enforces this invariant
```

4. **Validate Fix**:
```bash
cargo +nightly miri test --test <failing_test>
# Should pass now
```

**Effort**: 1-4 hours investigation, 30 minutes fix, 1 hour ASSUM documentation

---

### Emergency 3: Performance Cliff (10× Slowdown)

**Symptom**: Sudden 10× performance degradation

**Immediate Response** (within 2 hours):

1. **Bisect to Find Regression**:
```bash
git bisect start
git bisect bad HEAD  # Current (slow)
git bisect good v0.1.9  # Last known good
# Git will checkout commits for testing
cargo bench --bench <slow_bench>
git bisect good  # or bad
# Repeat until bad commit found
```

2. **Profile Hot Path**:
```bash
# Flamegraph
cargo flamegraph --bench <slow_bench>
# Opens flamegraph.svg in browser

# Cachegrind (cache misses)
valgrind --tool=cachegrind target/release/<bench>
```

3. **Common Causes**:

| Symptom | Cause | Fix |
|---------|-------|-----|
| **10-100× slower** | Debug build in production | Use `--release` |
| **2-10× slower** | Cache misalignment | Fix alignment (64/128B) |
| **1.5-5× slower** | Disabled SIMD | Enable portable_simd feature |
| **10-50% slower** | Lock contention | Check concurrent access pattern |

4. **Fix Template**:
```rust
// Example: Cache misalignment (2× slowdown)
// BEFORE (misaligned)
#[repr(C)]  // Missing align!
struct HotStruct {
    data: [u8; 64],
}

// AFTER (aligned)
#[repr(C, align(64))]  // Cache-aligned
struct HotStruct {
    data: [u8; 64],
}
```

**Effort**: 2 hours bisect, 30 minutes profile, 1 hour fix

---

## Appendix A: Tool Reference

### A1: Clippy Lints

```bash
# Zero warnings (mandatory)
cargo clippy --all-features -- -D warnings

# Specific lints for capsules
cargo clippy -- -D clippy::unwrap_used         # No unwrap() in production
cargo clippy -- -D clippy::panic               # No panic!() in production
cargo clippy -- -W clippy::missing_docs_in_private_items  # Document all items
```

### A2: Miri (UB Detection)

```bash
# Full test suite
cargo +nightly miri test

# Specific test
cargo +nightly miri test --test concurrent_map_stress

# Ignore leaks (for benchmarks)
MIRIFLAGS="-Zmiri-ignore-leaks" cargo +nightly miri test
```

### A3: Cargo-Bloat (Binary Size)

```bash
# Install
cargo install cargo-bloat

# Analyze binary size
cargo bloat --release --features "profile-production"

# Top 10 functions by size
cargo bloat --release -n 10
```

### A4: Cargo-ASM (Assembly Inspection)

```bash
# Install
cargo install cargo-asm

# View assembly for function
cargo asm atomic_capsule::hash::const_hash::hash_bytes --rust

# Verify SIMD codegen
cargo asm atomic_capsule::hash::simd_hash::hash_fields --intel
```

---

**Document Version**: 1.0
**Date**: 2025-10-20
**Author**: Technical Debt Expert
**Frameworks Applied**: UCE34, IMPL-2 V3.0, ASSUM, B32, T28
**Coverage**: Common tasks, debugging, performance, migration, emergencies
