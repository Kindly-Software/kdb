# T9 Persistent Capsule - Verification Checklist

**Purpose**: Ensure T9 configuration is complete and correct

---

## Pre-Implementation Checks

### 1. Feature Flag Validation

**Command**:
```bash
cd /home/samuel/Primitives/atomic_capsule
cargo +nightly metadata --format-version=1 | jq '.packages[] | select(.name == "atomic_capsule") | .features' | grep -A5 persistent
```

**Expected Output**:
```json
"persistent": ["std", "dep:memmap2", "dep:bytemuck", "nightly-atomic"],
"persistent-audit": ["persistent", "audit-trail"],
"persistent-recovery": ["persistent"],
"persistent-all": ["persistent-audit", "persistent-recovery"],
"persistent-minhash": ["persistent-all", "probabilistic"]
```

**Status**: ☐ NOT YET IMPLEMENTED (configuration pending)

---

### 2. Dependency Resolution

**Command**:
```bash
cargo +nightly tree --features persistent-all | grep -E "(memmap2|bytemuck)"
```

**Expected Output**:
```
├── memmap2 v0.9.x
└── bytemuck v1.14.x
```

**Status**: ☐ NOT YET IMPLEMENTED

---

### 3. Nightly Feature Gate

**Command**:
```bash
grep -n "cfg_attr.*persistent.*atomic_from_mut" /home/samuel/Primitives/atomic_capsule/src/lib.rs
```

**Expected Output**:
```
[line number]: #![cfg_attr(feature = "persistent", feature(atomic_from_mut))]
```

**Status**: ☐ NOT YET IMPLEMENTED

---

### 4. Module Declaration

**Command**:
```bash
grep -A2 "T9 Persistent" /home/samuel/Primitives/atomic_capsule/src/lib.rs | grep "pub mod persistent"
```

**Expected Output**:
```rust
#[cfg(feature = "persistent")]
pub mod persistent;
```

**Status**: ☐ NOT YET IMPLEMENTED

---

### 5. Stable Rust Rejection

**Command**:
```bash
cargo check --features persistent-all 2>&1 | grep -i "atomic_from_mut"
```

**Expected Output**:
```
error[E0554]: `#![feature]` may not be used on the stable release channel
```

**Status**: ☐ NOT YET TESTED (feature flags not added yet)

---

### 6. Nightly Rust Acceptance

**Command**:
```bash
cargo +nightly check --features persistent-all 2>&1 | grep -E "(error|warning)" | head -5
```

**Expected Output** (before implementation):
```
error[E0433]: failed to resolve: could not find `persistent` in the crate root
```

**Expected Output** (after implementation):
```
(no errors or warnings)
```

**Status**: ☐ NOT YET TESTED

---

## Post-Implementation Checks

### 7. Compilation Success

**Command**:
```bash
cargo +nightly build --features persistent-all
```

**Expected**: Zero errors, zero warnings

**Status**: ☐ PENDING IMPLEMENTATION

---

### 8. Test Suite

**Command**:
```bash
cargo +nightly test --features persistent-all
```

**Expected**: All tests pass (T28 4-tier coverage)

**Status**: ☐ PENDING IMPLEMENTATION

---

### 9. Benchmark Compilation

**Command**:
```bash
cargo +nightly bench --features persistent-all --no-run
```

**Expected**: Benchmarks compile successfully

**Status**: ☐ PENDING IMPLEMENTATION

---

### 10. Documentation Generation

**Command**:
```bash
cargo +nightly doc --features persistent-all --no-deps
```

**Expected**: Docs generate without warnings

**Status**: ☐ PENDING IMPLEMENTATION

---

## Framework Compliance Checks

### 11. UCE34 Q1-Q34 Coverage

**Verify**: All 34 questions answered in `docs/T9_PERSISTENT_CAPSULE_UCE34.md`

**Status**: ✅ COMPLETE (see existing doc)

---

### 12. IMPL-2 V3.1 Nightly-First

**Verify**:
- ✅ Nightly features used by default (atomic_from_mut)
- ✅ Tier-maximization (T9 = T1 Atomic + mmap)
- ✅ Innovation-stacking (atomic_from_mut + memmap2 + generation counters)
- ✅ Breakthrough target (100-1000× vs serialize + write)

**Status**: ✅ COMPLETE (see T9_BUILD_CONFIGURATION.md)

---

### 13. B32 Benchmarking Framework

**Verify**:
- ☐ Fair baseline (serde + fs::write)
- ☐ 95% confidence intervals
- ☐ 1000+ iterations
- ☐ Reproducibility validation

**Status**: ☐ PENDING BENCHMARK IMPLEMENTATION

---

### 14. T28 Testing Framework

**Verify**:
- ☐ Unit tests (alignment, atomic correctness, flush success)
- ☐ Property tests (multi-process, crash recovery, concurrent access)
- ☐ Integration tests (end-to-end persistence)
- ☐ Production tests (sustained writes, disk full, corruption detection)

**Status**: ☐ PENDING TEST IMPLEMENTATION

---

### 15. ASSUM Safety Analysis

**Verify**:
- ☐ All unsafe code documented (atomic_from_mut usage)
- ☐ Safety assumptions tagged (#ASSUME_*)
- ☐ Verification strategy documented (#VERIFY_*)
- ☐ 99.5%+ safety target

**Status**: ☐ PENDING IMPLEMENTATION

---

### 16. I20 Integration Framework

**Verify** (for T9+T10 composition):
- ☐ Q1-Q5: Scope (T9 persistent + T10 probabilistic)
- ☐ Q6-Q10: Compatibility (lockfree atomic coordination)
- ☐ Q11-Q15: Safety (crash recovery, generation counters)
- ☐ Q16-Q20: Validation (100× speedup for incremental dedup)

**Status**: ☐ PENDING INTEGRATION IMPLEMENTATION

---

## Configuration File Checks

### 17. Cargo.toml Feature Section

**File**: `/home/samuel/Primitives/atomic_capsule/Cargo.toml`
**Lines**: After line 494 (before `[dependencies]`)

**Required Content**:
```toml
persistent = ["std", "dep:memmap2", "dep:bytemuck", "nightly-atomic"]
persistent-audit = ["persistent", "audit-trail"]
persistent-recovery = ["persistent"]
persistent-all = ["persistent-audit", "persistent-recovery"]
persistent-minhash = ["persistent-all", "probabilistic"]
```

**Status**: ☐ NOT ADDED

---

### 18. Cargo.toml Dependencies Section

**File**: `/home/samuel/Primitives/atomic_capsule/Cargo.toml`
**Lines**: After line 524 (with other optional dependencies)

**Required Content**:
```toml
memmap2 = { version = "0.9", optional = true }
bytemuck = { version = "1.14", optional = true, features = ["derive"] }
```

**Verification**: ✅ bytemuck not present (no conflict)

**Status**: ☐ NOT ADDED

---

### 19. lib.rs Nightly Feature Gate

**File**: `/home/samuel/Primitives/atomic_capsule/src/lib.rs`
**Line**: After line 132

**Required Content**:
```rust
#![cfg_attr(feature = "persistent", feature(atomic_from_mut))]
```

**Status**: ☐ NOT ADDED

---

### 20. lib.rs Module Declaration

**File**: `/home/samuel/Primitives/atomic_capsule/src/lib.rs`
**Line**: After line 260

**Required Content**:
```rust
#[cfg(feature = "persistent")]
pub mod persistent;
```

**Status**: ☐ NOT ADDED

---

### 21. lib.rs Re-exports

**File**: `/home/samuel/Primitives/atomic_capsule/src/lib.rs`
**Line**: After line 310

**Required Content**:
```rust
#[cfg(feature = "persistent")]
pub use persistent::{
    PersistentAtomicCapsule,
    PersistentError,
    PersistentResult,
    FlushMode,
};

#[cfg(feature = "persistent-minhash")]
pub use persistent::{
    PersistentMinHashCapsule,
    PersistentDedupIndex,
};
```

**Status**: ☐ NOT ADDED

---

## Summary

**Configuration Readiness**: 0/21 checks complete (awaiting code changes)

**Next Action**: Apply code changes from `T9_CODE_CHANGES.md`

**Estimated Time**: 15 minutes for configuration + 1 week for implementation

---

**Last Updated**: 2025-10-27
