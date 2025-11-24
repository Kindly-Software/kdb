# P0 Test Validation Matrix

**Quick Reference**: Test outcomes and validation checkpoints

---

## P0.3: CAPSULE_MISSING_GENERATION (10 tests)

| # | Test File | Expected | Tier | Has Gen Field? | Outcome | Checkpoint |
|---|-----------|----------|------|----------------|---------|------------|
| 01 | `01_atomic_without_gen.rs` | ❌ FAIL | T1 Atomic | ❌ No | Compile error | Generation required |
| 02 | `02_atomic_with_gen.rs` | ✅ PASS | T1 Atomic | ✅ Yes (generation) | Success | Correct pattern |
| 03 | `03_dual_atomic_pattern.rs` | ✅ PASS | T1 Atomic | ✅ Yes (generation) | Success | DualAtomicU64 |
| 04 | `04_non_atomic_tier_ok.rs` | ✅ PASS | T2 SIMD | ❌ No | Success | Non-T1 exempt |
| 05 | `05_abbreviated_gen.rs` | ✅ PASS | T1 Atomic | ✅ Yes (gen) | Success | Abbreviated OK |
| 06 | `06_multiple_atomics_no_gen.rs` | ❌ FAIL | T1 Atomic | ❌ No | Compile error | Generation required |
| 07 | `07_fixed_point_tier_ok.rs` | ✅ PASS | T3 Fixed-Point | ❌ No | Success | Non-T1 exempt |
| 08 | `08_generation_not_gen.rs` | ❌ FAIL | T1 Atomic | ❌ No (wrong name) | Compile error | Exact name match |
| 09 | `09_batch_tier_ok.rs` | ✅ PASS | T4 Batch | ❌ No | Success | Non-T1 exempt |
| 10 | `10_mixed_tier_with_gen.rs` | ✅ PASS | T6 Mixed | ✅ Yes (generation) | Success | Optional best practice |

**Summary**: 3 fail / 7 pass | Validates T1-specific enforcement + exact name matching

---

## P0.4: CAPSULE_NON_ATOMIC_FIELD (10 tests)

| # | Test File | Expected | Tier | Non-Atomic Fields | Outcome | Checkpoint |
|---|-----------|----------|------|-------------------|---------|------------|
| 01 | `01_u64_in_atomic.rs` | ❌ FAIL | T1 Atomic | `count: u64` | Compile error | Use AtomicU64 |
| 02 | `02_bool_in_atomic.rs` | ❌ FAIL | T1 Atomic | `active: bool` | Compile error | Use AtomicBool |
| 03 | `03_all_atomic_fields.rs` | ✅ PASS | T1 Atomic | None (all atomic) | Success | Correct pattern |
| 04 | `04_padding_allowed.rs` | ✅ PASS | T1 Atomic | `_padding: [u8; 48]` | Success | Padding exempt |
| 05 | `05_i64_in_atomic.rs` | ❌ FAIL | T1 Atomic | `delta: i64` | Compile error | Use AtomicI64 |
| 06 | `06_usize_in_atomic.rs` | ❌ FAIL | T1 Atomic | `index: usize` | Compile error | Use AtomicUsize |
| 07 | `07_non_atomic_tier_allows_u64.rs` | ✅ PASS | T2 SIMD | `count: u64` | Success | Non-T1 exempt |
| 08 | `08_atomic_i64_ok.rs` | ✅ PASS | T1 Atomic | None (AtomicI64) | Success | Atomic type OK |
| 09 | `09_multiple_violations.rs` | ❌ FAIL | T1 Atomic | `count: u64`, `active: bool` | Compile error (2×) | Multiple errors |
| 10 | `10_nested_padding_ok.rs` | ✅ PASS | T1 Atomic | Nested `Padding48` struct | Success | Padding allowed |

**Summary**: 5 fail / 5 pass | Validates T1-specific enforcement + padding exemption

---

## Validation Checkpoints

### P0.3 Key Validations
- ✅ **T1 Enforcement**: Only tier="Atomic" triggers lint (tests 04, 07, 09)
- ✅ **Exact Name Matching**: "generation" or "gen" only, not "generation_counter" (test 08)
- ✅ **DualAtomicU64 Pattern**: Production pattern accepted (test 03)
- ✅ **Optional for Non-T1**: T6 Mixed can have generation (test 10)

### P0.4 Key Validations
- ✅ **T1 Enforcement**: Only tier="Atomic" triggers lint (test 07)
- ✅ **Padding Exemption**: Arrays and nested structs allowed (tests 04, 10)
- ✅ **Primitive Coverage**: u64, i64, bool, usize all detected (tests 01, 02, 05, 06)
- ✅ **Multiple Violations**: Can report multiple errors per struct (test 09)

---

## Test Execution Commands

```bash
# P0.3 tests (generation counter)
cargo test --test compiletest -- p0_generation_violation

# P0.4 tests (atomic fields)
cargo test --test compiletest -- p0_atomic_field_violation

# All P0 tests
cargo test --test compiletest -- "p0_"

# Specific test
cargo test --test compiletest -- p0_generation_violation/01_atomic_without_gen
```

---

## Expected Error Messages

### P0.3 Error Format
```
error: T1 Atomic capsule missing generation counter field (generation or gen)
  --> tests/ui/p0_generation_violation/XX_test_name.rs:NN:N
```

### P0.4 Error Format
```
error: T1 Atomic capsule contains non-atomic field `field_name` of type `type_name`. Use `AtomicType` instead
  --> tests/ui/p0_atomic_field_violation/XX_test_name.rs:NN:N
```

---

## Coverage Matrix

| Category | Covered | Test Numbers |
|----------|---------|--------------|
| **P0.3: Tier Enforcement** | ✅ | 04, 07, 09, 10 |
| **P0.3: Name Matching** | ✅ | 05, 08 |
| **P0.3: DualAtomic Pattern** | ✅ | 03 |
| **P0.4: Primitive Types** | ✅ | 01, 02, 05, 06 |
| **P0.4: Padding Exemption** | ✅ | 04, 10 |
| **P0.4: Multiple Violations** | ✅ | 09 |
| **P0.4: Tier Enforcement** | ✅ | 07 |
| **P0.4: Atomic Equivalents** | ✅ | 03, 08 |

---

## Quick Checklist

Before implementing lints:
- [ ] All 20 test files created
- [x] 8 `.stderr` files for fail cases
- [ ] Test file syntax valid (rustfmt check)
- [ ] Error messages match `.stderr` format
- [ ] compiletest configuration updated

After implementing lints:
- [ ] All fail cases produce expected errors
- [ ] All pass cases compile cleanly
- [ ] Error messages helpful and actionable
- [ ] No false positives
- [ ] No false negatives

---

**Status**: ✅ Test Suite Complete (20/20 tests)
**Next**: Implement lint logic in `src/lib.rs`
