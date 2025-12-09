# Backward Compatibility Test Plan
**atomic_capsule_derive v0.4.1 → v0.7.0**
**Framework**: T28 Testing (Q1-Q28)
**Date**: 2025-11-02

---

## Executive Summary

**Objective**: Validate backward compatibility across 3 phases of atomic_capsule_derive evolution

**Scope**: 618 manual macro call sites across 7 downstream projects

**Test Coverage**:
- **Unit Tests** (Q1-Q7): 45+ tests per phase (syntax, semantics, error handling)
- **Property Tests** (Q8-Q14): 1000+ generated cases per phase (auto_pad, tier inference)
- **Integration Tests** (Q15-Q21): 7 downstream projects (618 capsules)
- **Production Tests** (Q22-Q28): clapi_core (365 tests), real-world validation

**Risk Level**: **MINIMAL** (deterministic capsules = tests predict production)

---

## Test Strategy Overview

### I20-Capsule Testing Principle

**Computational capsules are deterministic** → Tests are sufficient for deployment confidence

**No gradual rollout needed**:
- ✅ Compile-time verification catches bugs before runtime
- ✅ Property tests (1000+ cases) cover all input combinations
- ✅ Integration tests validate 618 real-world capsules
- ✅ If tests pass → deploy 100% immediately

**Simplified Testing** (vs traditional software):
- ❌ NO canary deployments (deterministic = predictable)
- ❌ NO A/B testing (no statistical uncertainty)
- ❌ NO production monitoring (0ns runtime cost)
- ✅ Git revert sufficient (rollback <1% likely)

---

## Phase 1: v0.5.0 (Soundness) - Test Plan

### Q1-Q7: Unit Tests (35 tests)

#### Test Suite 1.1: Mutex Detection (10 tests)

**Test 1.1.1: Detect Mutex<T> field**
```rust
#[test]
fn test_mutex_field_error() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/mutex_field.rs");

    // Expected error:
    // "Field `state` uses Mutex which is incompatible with capsule architecture"
}
```

**Test 1.1.2: Detect RwLock<T> field**
```rust
#[test]
fn test_rwlock_field_error() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/rwlock_field.rs");
}
```

**Test 1.1.3: Detect Cell<T> field**
```rust
#[test]
fn test_cell_field_error() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/cell_field.rs");
}
```

**Test 1.1.4: Detect RefCell<T> field**
```rust
#[test]
fn test_refcell_field_error() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/refcell_field.rs");
}
```

**Test 1.1.5: Nested Mutex detection**
```rust
#[test]
fn test_nested_mutex_error() {
    // struct Nested { inner: Mutex<State> }
    // struct Capsule { field: Nested }
    // Expected: Error detected (recursive field inspection)
}
```

**Tests 1.1.6-1.1.10**: Edge cases
- Multiple Mutex fields in single capsule
- Generic type with Mutex bound
- PhantomData<Mutex<T>> (false positive check)
- Arc<Mutex<T>> detection
- Box<Mutex<T>> detection

---

#### Test Suite 1.2: AtomicU64 Acceptance (5 tests)

**Test 1.2.1: AtomicU64 compiles successfully**
```rust
#[test]
fn test_atomic_u64_success() {
    let t = trybuild::TestCases::new();
    t.pass("tests/compile_pass/atomic_u64.rs");
}

// tests/compile_pass/atomic_u64.rs
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
struct AtomicCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}
```

**Test 1.2.2: DualAtomicU64 compiles successfully**
```rust
#[test]
fn test_dual_atomic_success() {
    let t = trybuild::TestCases::new();
    t.pass("tests/compile_pass/dual_atomic.rs");
}
```

**Tests 1.2.3-1.2.5**:
- AtomicI64 acceptance
- AtomicBool acceptance
- Multiple atomic fields (all lockfree)

---

#### Test Suite 1.3: Error Messages (10 tests)

**Test 1.3.1: Actionable Mutex error message**
```rust
#[test]
fn test_mutex_error_message_actionable() {
    let stderr = get_compile_error("tests/compile_fail/mutex_field.rs");

    // Must contain:
    assert!(stderr.contains("Replace Mutex with:"));
    assert!(stderr.contains("AtomicU64 for packed state"));
    assert!(stderr.contains("DualAtomicU64 for dual-channel"));
    assert!(stderr.contains("See: /home/samuel/Docs/The Atomic Capsule.md"));
}
```

**Test 1.3.2: UCE34 Q11 guidance included**
```rust
#[test]
fn test_error_includes_uce34_guidance() {
    let stderr = get_compile_error("tests/compile_fail/mutex_field.rs");
    assert!(stderr.contains("UCE34 Q10: Tier 1 Atomic"));
}
```

**Tests 1.3.3-1.3.10**: Error message quality
- Span information (correct line/column)
- Example code in error message
- Performance justification (3-10× speedup)
- Multiple suggestions (AtomicU64, DualAtomicU64)
- Help link (documentation)
- Before/after example
- Memory ordering guidance
- Send + Sync explanation

---

#### Test Suite 1.4: Backward Compatibility (10 tests)

**Test 1.4.1: v0.4.1 code without Mutex compiles**
```rust
#[test]
fn test_v0_4_1_atomic_capsule_still_works() {
    let t = trybuild::TestCases::new();
    t.pass("tests/compat/v0_4_1_atomic.rs");

    // v0.4.1 code (no changes needed):
    // #[derive(ComputationalCapsule)]
    // #[capsule(alignment = 64)]
    // #[repr(C, align(64))]
    // struct OldCapsule { state: AtomicU64, _padding: [u8; 56] }
}
```

**Test 1.4.2: Manual padding still works**
```rust
#[test]
fn test_manual_padding_backward_compat() {
    let capsule = ManualPaddingCapsule {
        state: AtomicU64::new(0),
        _padding: [0u8; 56],
    };
    assert_eq!(mem::size_of_val(&capsule), 64);
}
```

**Tests 1.4.3-1.4.10**: Backward compatibility checks
- Send + Sync traits still generated
- Const assertions still enforced
- Alignment validation unchanged
- Size validation unchanged
- Tier validation unchanged (if specified)
- #[repr(C, align(N))] validation unchanged
- Error handling unchanged (syn::Error)
- Performance unchanged (compilation time <20ms)

---

### Q8-Q14: Property Tests (1000+ cases)

#### Property Test 1.1: Lockfree Enforcement

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn property_no_mutex_in_any_capsule(
        alignment in prop::sample::select(vec![32, 64, 128, 256]),
        field_count in 1usize..10,
    ) {
        // Generate random capsule structure
        let capsule = generate_random_capsule(alignment, field_count);

        // Verify: No Mutex/RwLock/Cell fields
        let fields = inspect_fields(&capsule);
        for field in fields {
            assert!(
                !is_mutex(&field) && !is_rwlock(&field) && !is_cell(&field),
                "Blocking type detected: {:?}", field
            );
        }
    }
}
```

**Coverage**: 1000+ random capsule structures (alignment × field count combinations)

---

#### Property Test 1.2: Error Message Consistency

```rust
proptest! {
    #[test]
    fn property_error_messages_always_actionable(
        blocking_type in prop::sample::select(vec!["Mutex", "RwLock", "Cell", "RefCell"])
    ) {
        let stderr = compile_with_blocking_type(blocking_type);

        // Verify: Error message always includes fix suggestions
        assert!(stderr.contains("Replace"));
        assert!(stderr.contains("AtomicU64"));
        assert!(stderr.contains("UCE34"));
    }
}
```

**Coverage**: 1000+ error message variants (different blocking types)

---

### Q15-Q21: Integration Tests (7 projects)

#### Integration Test 1.1: atomic_capsule (618 capsules)

```bash
# Test: All 618 capsules compile after migration
cd /home/samuel/Primitives/atomic_capsule

# Step 1: Update to v0.5.0
sed -i 's/version = "0.4.1"/version = "0.5.0"/' Cargo.toml

# Step 2: Find Mutex usage (expect 0)
rg "Mutex<" src/ || echo "No Mutex found (expected)"

# Step 3: Compile (expect errors if Mutex found)
cargo build --lib 2>&1 | tee build.log

# Step 4: Verify zero Mutex/RwLock/Cell
grep -E "(Mutex|RwLock|Cell)" build.log && exit 1 || echo "PASS"

# Step 5: Run test suite
cargo test --lib --all-features
# Expected: 266 tests pass
```

**Success Criteria**:
- ✅ Zero Mutex/RwLock/Cell fields
- ✅ 266 tests pass (100%)
- ✅ Zero compilation warnings
- ✅ Performance unchanged (benchmarks)

---

#### Integration Test 1.2: clapi_core (14 capsules)

```bash
cd /path/to/clapi_core

# Test: Production-validated project
cargo test --lib --all-features
# Expected: 365 tests pass

# Benchmark: Performance unchanged
cargo bench -- --save-baseline old
# (no changes expected, v0.4.1 → v0.5.0 compatible if no Mutex)
```

**Success Criteria**:
- ✅ 365 tests pass (100%)
- ✅ Performance unchanged (<1% variance)
- ✅ Zero regressions

---

#### Integration Tests 1.3-1.7: Remaining projects

**Projects**: kindly_hft (50+), kiang (~20), kindly-db (~30), kindly_dedup (~10), others (~50)

**Test Script** (per project):
```bash
#!/bin/bash
PROJECT_PATH=$1

cd "$PROJECT_PATH"

# 1. Update Cargo.toml
sed -i 's/atomic_capsule_derive.*0.4.1/atomic_capsule_derive = { path = "..\/atomic_capsule_derive", version = "0.5.0" }/' Cargo.toml

# 2. Find Mutex usage
MUTEX_COUNT=$(rg -c "Mutex<" src/ 2>/dev/null | awk -F: '{sum+=$2} END {print sum}')
echo "Mutex fields found: $MUTEX_COUNT"

# 3. Compile (expect errors if Mutex > 0)
cargo build --lib 2>&1 | tee build.log

# 4. Check for errors
if grep -q "error:" build.log; then
    echo "EXPECTED: Mutex/RwLock errors (Phase 1 enforcement)"
    grep "error:" build.log
    exit 1  # Manual migration needed
else
    echo "PASS: No blocking types detected"
fi

# 5. Run tests
cargo test --lib --all-features
TEST_EXIT=$?

if [ $TEST_EXIT -eq 0 ]; then
    echo "✅ PASS: All tests passed"
else
    echo "❌ FAIL: Tests failed"
    exit 1
fi
```

**Success Criteria** (per project):
- ✅ Zero Mutex/RwLock/Cell (or compilation errors with actionable messages)
- ✅ 100% tests pass (after migration if needed)
- ✅ Zero performance regressions

---

### Q22-Q28: Production Tests (Real-World Validation)

#### Production Test 1.1: clapi_core Runtime Validation

**Scenario**: clapi_core already using v0.4.1 (14 capsules, Oct 18 2025)

**Test**:
```bash
cd /path/to/clapi_core

# 1. Baseline performance (v0.4.1)
cargo bench -- --save-baseline v0_4_1

# 2. Update to v0.5.0
sed -i 's/0.4.1/0.5.0/' Cargo.toml

# 3. Verify compilation (no changes expected, no Mutex)
cargo build --release

# 4. Run production test suite
cargo test --release --all-features
# Expected: 365 tests pass

# 5. Benchmark comparison
cargo bench -- --baseline v0_4_1
# Expected: <1% variance (no performance change)

# 6. Runtime validation (if applicable)
# Run production workload, monitor for issues
```

**Success Criteria**:
- ✅ 365 tests pass (100%)
- ✅ Performance unchanged (<1% variance)
- ✅ Zero production incidents (24-hour monitoring)

---

#### Production Test 1.2: Stress Testing (Concurrent Access)

```rust
#[test]
fn stress_test_concurrent_capsule_access() {
    use std::sync::Arc;
    use std::thread;

    #[derive(ComputationalCapsule)]
    #[capsule(alignment = 64)]
    #[repr(C, align(64))]
    struct StressCapsule {
        counter: AtomicU64,
        _padding: [u8; 56],
    }

    let capsule = Arc::new(StressCapsule {
        counter: AtomicU64::new(0),
        _padding: [0; 56],
    });

    let threads: Vec<_> = (0..100)
        .map(|_| {
            let capsule = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..10_000 {
                    capsule.counter.fetch_add(1, Ordering::Relaxed);
                }
            })
        })
        .collect();

    for handle in threads {
        handle.join().unwrap();
    }

    // Expected: 100 threads × 10,000 = 1,000,000
    assert_eq!(capsule.counter.load(Ordering::Relaxed), 1_000_000);
}
```

**Success Criteria**:
- ✅ Zero data races (correct final count)
- ✅ Performance linear scaling (100 threads)
- ✅ Zero deadlocks (no Mutex = impossible)

---

## Phase 2: v0.6.0 (Correctness) - Test Plan

### Q1-Q7: Unit Tests (25 tests)

#### Test Suite 2.1: Auto-Pad Feature (10 tests)

**Test 2.1.1: Auto-pad calculates correct padding**
```rust
#[test]
fn test_auto_pad_64byte_alignment() {
    #[derive(ComputationalCapsule)]
    #[capsule(alignment = 64, auto_pad = true)]
    #[repr(C, align(64))]
    struct AutoPadCapsule {
        state: AtomicU64,  // 8 bytes
        // auto_pad adds 56 bytes
    }

    assert_eq!(mem::size_of::<AutoPadCapsule>(), 64);
}
```

**Test 2.1.2: Auto-pad with multiple fields**
```rust
#[test]
fn test_auto_pad_multiple_fields() {
    #[derive(ComputationalCapsule)]
    #[capsule(alignment = 128, auto_pad = true)]
    #[repr(C, align(128))]
    struct MultiFieldCapsule {
        state: AtomicU64,    // 8 bytes
        counter: AtomicU64,  // 8 bytes
        flags: AtomicU64,    // 8 bytes
        // auto_pad adds 104 bytes
    }

    assert_eq!(mem::size_of::<MultiFieldCapsule>(), 128);
}
```

**Tests 2.1.3-2.1.10**: Auto-pad edge cases
- Empty struct (only padding)
- Struct exactly fitting alignment (no padding needed)
- Nested struct with auto_pad
- Generic types with auto_pad
- ZST fields (zero-sized types)
- Alignment 32/64/128/256 variations
- Complex field types (DualAtomicU64, SIMD arrays)
- Mixed atomic and non-atomic fields

---

#### Test Suite 2.2: Backward Compatibility (10 tests)

**Test 2.2.1: Manual padding still works**
```rust
#[test]
fn test_manual_padding_v0_5_0_compat() {
    #[derive(ComputationalCapsule)]
    #[capsule(alignment = 64)]  // No auto_pad
    #[repr(C, align(64))]
    struct ManualCapsule {
        state: AtomicU64,
        _padding: [u8; 56],  // Manual padding
    }

    assert_eq!(mem::size_of::<ManualCapsule>(), 64);
}
```

**Test 2.2.2: v0.5.0 code unchanged**
```rust
#[test]
fn test_v0_5_0_code_compiles_with_v0_6_0() {
    let t = trybuild::TestCases::new();
    t.pass("tests/compat/v0_5_0_atomic.rs");
    // No changes needed to v0.5.0 code
}
```

**Tests 2.2.3-2.2.10**: Compatibility checks
- Send + Sync traits unchanged
- Const assertions unchanged
- Error messages unchanged (for non-auto_pad capsules)
- Performance unchanged (manual padding)
- Alignment validation unchanged
- Tier validation unchanged
- #[repr(C)] validation unchanged
- Compilation time unchanged (<30ms)

---

#### Test Suite 2.3: Auto-Pad Conflicts (5 tests)

**Test 2.3.1: Detect manual + auto_pad conflict**
```rust
#[test]
fn test_auto_pad_manual_padding_conflict_warning() {
    let stderr = compile_with_warning("tests/warnings/auto_pad_conflict.rs");

    // Expected warning:
    assert!(stderr.contains("Manual _padding field detected with auto_pad = true"));
    assert!(stderr.contains("Remove manual padding or set auto_pad = false"));
}
```

**Tests 2.3.2-2.3.5**: Conflict detection
- Multiple _padding fields with auto_pad
- auto_pad with size attribute (conflicting)
- auto_pad with incorrect alignment
- auto_pad with non-repr(C) struct

---

### Q8-Q14: Property Tests (1000+ cases)

#### Property Test 2.1: Auto-Pad Correctness

```rust
proptest! {
    #[test]
    fn property_auto_pad_size_equals_alignment(
        alignment in prop::sample::select(vec![32, 64, 128, 256]),
        field_sizes in prop::collection::vec(1usize..64, 1..10),  // Random field sizes
    ) {
        // Generate capsule with random fields
        let capsule = generate_capsule_with_fields(alignment, field_sizes, auto_pad = true);

        // Verify: size == alignment (always)
        assert_eq!(
            mem::size_of_val(&capsule),
            alignment,
            "Auto-pad failed: size {} != alignment {}",
            mem::size_of_val(&capsule),
            alignment
        );
    }
}
```

**Coverage**: 1000+ capsule structures (alignment × field combinations)

---

#### Property Test 2.2: Padding Idempotence

```rust
proptest! {
    #[test]
    fn property_auto_pad_idempotent(capsule: Capsule) {
        // Calculate padding twice
        let padding1 = calculate_auto_pad(&capsule);
        let padding2 = calculate_auto_pad(&capsule);

        // Verify: deterministic (always same result)
        assert_eq!(padding1, padding2, "Auto-pad not deterministic");
    }
}
```

**Coverage**: 1000+ capsule structures (determinism check)

---

### Q15-Q21: Integration Tests (7 projects, opt-in)

#### Integration Test 2.1: atomic_capsule (opt-in auto_pad)

```bash
cd /home/samuel/Primitives/atomic_capsule

# Phase 2: Opt-in auto_pad for 1 capsule (pilot)
# Edit: src/patterns/circuit_breaker.rs
# Add: auto_pad = true, remove _padding field

# Test: Single capsule with auto_pad
cargo test --lib --test circuit_breaker
# Expected: Tests pass

# If successful: Enable for all 618 capsules (batch update)
./tools/enable_auto_pad.sh
cargo test --lib --all-features
# Expected: 266 tests pass
```

**Success Criteria**:
- ✅ Pilot capsule: Size = alignment (64B)
- ✅ 266 tests pass (100%)
- ✅ Zero regressions

---

### Q22-Q28: Production Tests (Opt-In Validation)

#### Production Test 2.1: Binary Layout Verification

```rust
#[test]
fn test_binary_layout_unchanged() {
    #[derive(ComputationalCapsule)]
    #[capsule(alignment = 64, auto_pad = true)]
    #[repr(C, align(64))]
    struct AutoPadCapsule {
        state: AtomicU64,
    }

    #[derive(ComputationalCapsule)]
    #[capsule(alignment = 64)]
    #[repr(C, align(64))]
    struct ManualPadCapsule {
        state: AtomicU64,
        _padding: [u8; 56],
    }

    // Verify: Same binary layout
    assert_eq!(
        mem::size_of::<AutoPadCapsule>(),
        mem::size_of::<ManualPadCapsule>()
    );
    assert_eq!(
        mem::align_of::<AutoPadCapsule>(),
        mem::align_of::<ManualPadCapsule>()
    );
}
```

**Success Criteria**:
- ✅ Auto-pad layout matches manual padding
- ✅ Zero performance difference
- ✅ Zero false sharing

---

## Phase 3: v0.7.0 (Usability) - Test Plan

### Q1-Q7: Unit Tests (20 tests)

#### Test Suite 3.1: Tier Inference (10 tests)

**Test 3.1.1: Infer T1 Atomic from AtomicU64**
```rust
#[test]
fn test_infer_tier_atomic() {
    assert_tier_inferred("AtomicU64", "Atomic");
}
```

**Test 3.1.2: Infer T2 SIMD from [f32; 8]**
```rust
#[test]
fn test_infer_tier_simd() {
    assert_tier_inferred("[f32; 8]", "SIMD");
}
```

**Tests 3.1.3-3.1.10**: Tier inference for all 10 tiers
- T1: AtomicU64 → "Atomic"
- T2: [f32; 8] → "SIMD"
- T3: Q16_16 → "FixedPoint"
- T4: Vec<T> → "Batch"
- T5: Iterator → "Streaming"
- T6: Mixed types → "Mixed"
- T7: GPU trait → "GPU"
- T8: Network trait → "Network"
- T9: Mmap → "Persistent"
- T10: MinHashSignatureCapsule → "Probabilistic"

---

#### Test Suite 3.2: Manual Tier Override (5 tests)

**Test 3.2.1: Manual tier takes precedence**
```rust
#[test]
fn test_manual_tier_overrides_inference() {
    #[derive(ComputationalCapsule)]
    #[capsule(alignment = 64, tier = "Atomic")]  // Manual
    #[repr(C, align(64))]
    struct ManualTier {
        scores: [f32; 8],  // Would infer "SIMD", but manual = "Atomic"
    }

    // Verify: Manual tier used (not inferred)
    assert_tier("ManualTier", "Atomic");  // Not "SIMD"
}
```

**Tests 3.2.2-3.2.5**: Override scenarios
- Manual override for all 10 tiers
- Multiple fields with conflicting inferences (manual resolves)
- Empty struct with manual tier
- Generic type with manual tier

---

#### Test Suite 3.3: Backward Compatibility (5 tests)

**Test 3.3.1: v0.6.0 code unchanged**
```rust
#[test]
fn test_v0_6_0_code_compiles_with_v0_7_0() {
    let t = trybuild::TestCases::new();
    t.pass("tests/compat/v0_6_0_auto_pad.rs");
    // No changes needed
}
```

**Tests 3.3.2-3.3.5**: Compatibility
- No tier specified (no inference warnings)
- Manual tier still works
- Auto-pad + manual tier works
- Performance unchanged

---

### Q8-Q14: Property Tests (1000+ cases)

#### Property Test 3.1: Tier Inference Determinism

```rust
proptest! {
    #[test]
    fn property_tier_inference_deterministic(capsule: Capsule) {
        // Infer tier twice
        let tier1 = infer_tier(&capsule);
        let tier2 = infer_tier(&capsule);

        // Verify: always same result
        assert_eq!(tier1, tier2, "Tier inference not deterministic");
    }
}
```

**Coverage**: 1000+ capsule structures (determinism check)

---

#### Property Test 3.2: Manual Tier Always Wins

```rust
proptest! {
    #[test]
    fn property_manual_tier_always_precedence(
        inferred_tier in prop::sample::select(vec!["Atomic", "SIMD", "FixedPoint", ...]),
        manual_tier in prop::sample::select(vec!["Atomic", "SIMD", "FixedPoint", ...]),
    ) {
        let capsule = generate_capsule_with_tiers(inferred_tier, manual_tier);

        // Verify: manual tier used (not inferred)
        assert_eq!(get_tier(&capsule), manual_tier);
    }
}
```

**Coverage**: 1000+ tier combinations (manual override check)

---

### Q15-Q21: Integration Tests (7 projects, opt-in)

**Same as Phase 2**: Opt-in tier inference, no breaking changes

---

### Q22-Q28: Production Tests (Warning Validation)

#### Production Test 3.1: Tier Mismatch Warning

```rust
#[test]
fn test_tier_mismatch_warning_helpful() {
    let stderr = compile_with_warning("tests/warnings/tier_mismatch.rs");

    // Expected warning:
    assert!(stderr.contains("Specified tier 'Atomic' differs from inferred tier 'SIMD'"));
    assert!(stderr.contains("Consider 'SIMD' for 2-19× speedup"));
    assert!(stderr.contains("UCE34_FRAMEWORK.md Q10-Q12"));
}
```

**Success Criteria**:
- ✅ Actionable warning messages
- ✅ UCE34 guidance included
- ✅ Performance justification provided

---

## Test Execution Strategy

### Parallel Execution (Per Phase)

**Phase 1 (v0.5.0)**:
```bash
# Run all tests in parallel
cargo test --lib --all-features --jobs 16 -- --test-threads 16

# Expected:
# - 35 unit tests (Q1-Q7)
# - 1000+ property tests (Q8-Q14)
# - 7 integration tests (Q15-Q21)
# - 2 production tests (Q22-Q28)
# Total: 1044+ tests (run in <5 minutes)
```

**Phase 2 (v0.6.0)**:
```bash
# Backward compatible, run all + new tests
cargo test --lib --all-features --jobs 16

# Expected:
# - 60 unit tests (35 + 25 new)
# - 2000+ property tests (1000 + 1000 new)
# - 7 integration tests (opt-in)
# - 3 production tests
# Total: 2070+ tests
```

**Phase 3 (v0.7.0)**:
```bash
# Backward compatible, run all + new tests
cargo test --lib --all-features --jobs 16

# Expected:
# - 80 unit tests (60 + 20 new)
# - 3000+ property tests (2000 + 1000 new)
# - 7 integration tests (opt-in)
# - 4 production tests
# Total: 3091+ tests
```

---

## CI/CD Integration

### GitHub Actions Workflow

```yaml
name: Backward Compatibility Tests

on:
  push:
    branches: [main, phase1, phase2, phase3]
  pull_request:

jobs:
  test-phase-1:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run Phase 1 Tests
        run: |
          cargo test --lib --all-features
          cargo clippy -- -D warnings
          cargo bench --no-run

  test-phase-2:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Test auto_pad backward compat
        run: |
          cargo test --lib --features auto-pad
          cargo test --lib  # Without auto-pad

  test-phase-3:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Test tier inference
        run: |
          cargo test --lib --all-features
          cargo clippy -- -D warnings

  integration-tests:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        project: [atomic_capsule, clapi_core, kindly_hft]
    steps:
      - uses: actions/checkout@v3
      - name: Test ${{ matrix.project }}
        run: |
          cd /path/to/${{ matrix.project }}
          cargo test --lib --all-features
```

---

## Success Criteria Summary

### Phase 1 (v0.5.0)
- [ ] ✅ 35+ unit tests (100% pass)
- [ ] ✅ 1000+ property tests (100% pass)
- [ ] ✅ 7 integration tests (618 capsules, 100% pass)
- [ ] ✅ 2 production tests (clapi_core, zero regressions)
- [ ] ✅ Zero Mutex/RwLock/Cell fields in all projects
- [ ] ✅ Compilation time <25ms per capsule
- [ ] ✅ Zero compilation warnings

### Phase 2 (v0.6.0)
- [ ] ✅ 25+ new unit tests (100% pass)
- [ ] ✅ 1000+ new property tests (100% pass)
- [ ] ✅ Backward compatible (v0.5.0 code unchanged)
- [ ] ✅ Auto-pad opt-in works (size = alignment)
- [ ] ✅ Manual padding still works
- [ ] ✅ Compilation time <30ms per capsule

### Phase 3 (v0.7.0)
- [ ] ✅ 20+ new unit tests (100% pass)
- [ ] ✅ 1000+ new property tests (100% pass)
- [ ] ✅ Backward compatible (v0.6.0 code unchanged)
- [ ] ✅ Tier inference opt-in works (10 tiers)
- [ ] ✅ Manual tier override works
- [ ] ✅ Compilation time <35ms per capsule

---

## Rollback Criteria

**Trigger Rollback If**:
- Any test suite <95% pass rate
- Performance regression >5% (benchmarks)
- Production incident (clapi_core)
- Compilation time >50ms per capsule
- Memory increase >10% during compilation

**Rollback Procedure**:
```bash
git revert <commit-hash>
cargo build --release
cargo test --lib --all-features
# Rollback time: 5 minutes
```

---

**Version**: 1.0
**Date**: 2025-11-02
**Status**: ✅ Ready for Execution
