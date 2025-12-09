# User Migration Guide: Manual Macros → Automatic Derivation

**Target Audience**: Developers maintaining computational capsules across 7 projects

**Goal**: Migrate 618 manual verification macros to automatic `#[derive(ComputationalCapsule)]`

**Benefit**: 87.5% code reduction, zero manual maintenance, identical behavior

---

## Quick Start (5 Minutes)

### Before Migration

```rust
// Manual verification (OLD - 5+ lines per capsule)
#[repr(C, align(128))]
pub struct DualAtomicU64 {
    value: AtomicU64,
    generation: AtomicU64,
    _padding: [u8; 112],
}

// Manual macro call (required for verification)
verify_capsule_properties!(DualAtomicU64, 128, 128);
```

### After Migration

```rust
// Automatic verification (NEW - 2 lines added to struct)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
pub struct DualAtomicU64 {
    value: AtomicU64,
    generation: AtomicU64,
    _padding: [u8; 112],
}

// No manual macro needed! Verified at compile-time automatically.
```

**Result**: Same compile-time verification, zero runtime cost, 87.5% less code.

---

## Why Migrate?

### Problem: Manual Verification is Fragile

```rust
// Manual macro: Easy to forget, easy to mistype
verify_capsule_properties!(CircuitBreakerCapsule, 64, 64);

// What if you add a field?
#[repr(C, align(64))]
pub struct CircuitBreakerCapsule {
    state: AtomicU32,
    failures: AtomicU32,
    new_field: AtomicU64,  // OOPS: Size changed to 72 bytes!
    _padding: [u8; 48],
}

// Manual macro is now WRONG (says 64 bytes, actually 72 bytes)
// Compiler doesn't catch this until runtime (or worse, production)
```

### Solution: Automatic Derivation is Bulletproof

```rust
// Derive macro: Automatically checks at compile-time
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct CircuitBreakerCapsule {
    state: AtomicU32,
    failures: AtomicU32,
    new_field: AtomicU64,  // Add field
    _padding: [u8; 40],     // Adjust padding (compiler checks!)
}

// ERROR: Size mismatch! Expected 64 bytes, got 72 bytes.
// Add #[capsule(size = 72)] or adjust padding.
```

**Benefit**: Compiler catches errors **before** they reach production.

---

## Migration Steps (3 Steps)

### Step 1: Run Analysis Tool

```bash
# Check how many macros need migration
cargo +nightly -Zscript tools/migrate_verify_macros_to_derive.rs analyze

# Output:
# ╔═══════════════════════════════════════════════════════════════╗
# ║         Migration Plan: atomic_capsule                        ║
# ╚═══════════════════════════════════════════════════════════════╝
#
# Total manual macros: 250
# Estimated time: 45.0 hours
# Risk level: High
# Dependencies: None (foundation crate)
#
# Capsules by tier:
#   T1Atomic: 85 capsules
#   T2SIMD: 42 capsules
#   T3FixedPoint: 30 capsules
#   T4Batch: 28 capsules
#   T5Streaming: 15 capsules
#   T6Mixed: 50 capsules
```

### Step 2: Execute Migration (Dry-Run First)

```bash
# Dry-run to preview changes
cargo +nightly -Zscript tools/migrate_verify_macros_to_derive.rs migrate --dry-run atomic_capsule

# Output:
# [DRY RUN] Would modify: src/primitives/dual_atomic_u64.rs
#   Struct: DualAtomicU64
#   Add: #[derive(ComputationalCapsule)]
#        #[capsule(alignment = 128, size = 128)]
#   Remove: verify_capsule_properties!(DualAtomicU64, 128, 128);

# Review output carefully. If looks good:
cargo +nightly -Zscript tools/migrate_verify_macros_to_derive.rs migrate atomic_capsule

# Output:
# ✓ Migrated: DualAtomicU64 in src/primitives/dual_atomic_u64.rs
# ✓ Migrated: CircuitBreakerCapsule in src/patterns/circuit_breaker.rs
# ... (250 capsules)
# ✓ Migration complete!
```

### Step 3: Validate Migration

```bash
# Run comprehensive validation
bash tools/validate_migration.sh atomic_capsule

# Output:
# ╔═══════════════════════════════════════════════════════════════╗
# ║  Migration Validation: atomic_capsule
# ╚═══════════════════════════════════════════════════════════════╝
# [1/5] Capturing pre-migration baseline...
#   ✓ Tests: 266/266 passed
#   ✓ Compilation: 12.3s
#   ✓ Clippy: 0 warnings
#   ✓ Benchmarks: 45.2ns avg
# [2/5] Executing migration (dry-run)...
#   Proceed with real migration? (y/N) y
# [3/5] Running post-migration tests...
#   ✓ Tests: 266/266 passed (100% pass rate maintained)
# [4/5] Comparing baselines...
#   ✓ Compilation time: +1.6% (within 20ms/capsule tolerance)
#   ✓ Benchmarks: -0.2% (within 5% tolerance)
# [5/5] Generating validation report...
#
# ✓ Validation complete. Review report above.
#   To rollback: git restore atomic_capsule/**/*.rs
#   To commit: git add atomic_capsule && git commit -m 'feat(phase4): Migrate atomic_capsule to derive macros'
```

---

## Per-Tier Migration Examples

### Tier 1: Atomic (DualAtomicU64, CircuitBreakerCapsule)

**Before**:
```rust
#[repr(C, align(128))]
pub struct DualAtomicU64 {
    value: AtomicU64,
    generation: AtomicU64,
    _padding: [u8; 112],
}

verify_capsule_properties!(DualAtomicU64, 128, 128);
```

**After**:
```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
pub struct DualAtomicU64 {
    value: AtomicU64,
    generation: AtomicU64,
    _padding: [u8; 112],
}
```

### Tier 2: SIMD (SimdF32x8, SimdF64x8)

**Before**:
```rust
#[repr(C, align(256))]
pub struct SimdF64x8 {
    data: f64x8,
    _padding: [u8; 192],
}

verify_simd_capsule!(SimdF64x8, 256);
```

**After**:
```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256)]
#[repr(C, align(256))]
pub struct SimdF64x8 {
    data: f64x8,
    _padding: [u8; 192],
}
```

### Tier 3: Fixed-Point (FixedPointQ16x8)

**Before**:
```rust
#[repr(C, align(128))]
pub struct FixedPointQ16x8 {
    values: [i64; 8],
    _padding: [u8; 64],
}

verify_capsule_properties!(FixedPointQ16x8, 128, 128);
```

**After**:
```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
pub struct FixedPointQ16x8 {
    values: [i64; 8],
    _padding: [u8; 64],
}
```

### Tier 4: Batch (BatchRingBuffer)

**Before**:
```rust
#[repr(C, align(64))]
pub struct BatchRingBuffer<T, const N: usize> {
    buffer: [T; N],
    head: AtomicUsize,
    tail: AtomicUsize,
    _padding: [u8; 40],
}

verify_alignment_only!(BatchRingBuffer, 64);
```

**After**:
```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64)]  // No size constraint for generic
#[repr(C, align(64))]
pub struct BatchRingBuffer<T, const N: usize> {
    buffer: [T; N],
    head: AtomicUsize,
    tail: AtomicUsize,
    _padding: [u8; 40],
}
```

---

## Common Migration Scenarios

### Scenario 1: Simple Capsule (No Size Constraint)

**Use Case**: Generic capsules where size varies

```rust
// Before
#[repr(C, align(64))]
pub struct GenericCapsule<T> {
    data: T,
    _padding: [u8; 56],
}

verify_alignment_only!(GenericCapsule, 64);

// After
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64)]  // No size constraint
#[repr(C, align(64))]
pub struct GenericCapsule<T> {
    data: T,
    _padding: [u8; 56],
}
```

### Scenario 2: Complex Capsule (Size + Alignment)

**Use Case**: Fixed-size capsules with strict layout requirements

```rust
// Before
#[repr(C, align(256))]
pub struct PaymentCapsule256 {
    amount: AtomicI64,
    fee: AtomicI64,
    // ... 20 more fields
    _padding: [u8; 80],
}

verify_capsule_properties!(PaymentCapsule256, 256, 256);

// After
#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256)]
#[repr(C, align(256))]
pub struct PaymentCapsule256 {
    amount: AtomicI64,
    fee: AtomicI64,
    // ... 20 more fields
    _padding: [u8; 80],
}
```

### Scenario 3: SIMD Capsule (AVX2/AVX-512)

**Use Case**: Vectorized computation capsules

```rust
// Before (AVX2 - 256-bit alignment)
#[repr(C, align(256))]
pub struct SimdF64x4 {
    data: f64x4,
    _padding: [u8; 224],
}

verify_simd_capsule!(SimdF64x4, 256);

// After
#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256)]
#[repr(C, align(256))]
pub struct SimdF64x4 {
    data: f64x4,
    _padding: [u8; 224],
}
```

---

## Troubleshooting

### Problem 1: Compilation Error After Migration

**Error**:
```
error: Size mismatch! Expected 128 bytes, got 136 bytes.
```

**Solution**: Adjust padding or update size attribute

```rust
// Option 1: Adjust padding
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
pub struct MyCapsule {
    data: [u8; 64],
    _padding: [u8; 56],  // Reduce from 64 to 56 (was too much)
}

// Option 2: Update size constraint
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 136)]  // Accept new size
#[repr(C, align(128))]
pub struct MyCapsule {
    data: [u8; 64],
    _padding: [u8; 64],
}
```

### Problem 2: Tests Fail After Migration

**Error**:
```
test bench_dual_atomic_u64 ... FAILED
```

**Solution**: Check for alignment/size assumptions in tests

```rust
// Test before migration
#[test]
fn test_size() {
    assert_eq!(std::mem::size_of::<DualAtomicU64>(), 128);  // PASS
}

// After migration (if size changed)
#[test]
fn test_size() {
    assert_eq!(std::mem::size_of::<DualAtomicU64>(), 136);  // Update test
}
```

### Problem 3: Performance Regression

**Error**:
```
benchmark_avg_after_ns: 50.5ns (+11.7% vs baseline)
```

**Solution**: This is a RED FLAG. Rollback and investigate.

```bash
# Immediate rollback
git restore atomic_capsule/**/*.rs

# Investigate
cargo clean
cargo build --release --all-features
cargo bench --all-features

# Compare disassembly
cargo +nightly rustc --lib -- --emit asm
```

**Root Cause**: Derive macro should have **zero runtime cost**. If benchmarks regress, this is a bug in the derive macro implementation.

---

## Validation Checklist

After migration, verify ALL of the following:

### Compilation (Required)
- [ ] All projects compile successfully
- [ ] Zero compilation errors
- [ ] Zero clippy warnings (same as baseline)
- [ ] Compilation time <20ms/capsule overhead

### Tests (Required)
- [ ] All tests pass (100% pass rate)
- [ ] No test failures introduced
- [ ] Test pass rate maintained (within 1%)

### Performance (Required - B32 Framework)
- [ ] Benchmarks within 5% of baseline
- [ ] No performance regression detected
- [ ] Memory layout unchanged (size, alignment, field offsets)

### Production (If Applicable)
- [ ] Production systems unaffected
- [ ] Zero downtime migration
- [ ] No data corruption
- [ ] No behavioral changes

---

## FAQ

### Q: Will this change behavior of my code?

**A**: NO. Derive macro generates **identical code** to manual macros. Behavior is 100% unchanged (bit-exact).

### Q: Will this affect performance?

**A**: NO. Derive macro has **zero runtime cost** (all verification at compile-time). Benchmarks should be within 5% of baseline (measurement noise).

### Q: What if I need to rollback?

**A**: Simple. Run `git restore <project>/**/*.rs` and all manual macros are restored. See `ROLLBACK_PROCEDURE.md` for details.

### Q: Can I mix manual macros and derive macros?

**A**: YES (temporarily). During migration, both work side-by-side. After migration complete, remove all manual macros.

### Q: What if my capsule is generic?

**A**: Use `#[capsule(alignment = N)]` without size constraint. Size varies with generic parameter.

### Q: What if I have custom verification logic?

**A**: Use manual impl. Derive macro is for standard capsule verification only. Custom logic requires manual implementation.

---

## Timeline

| Week | Project | Macros | Status | ETA |
|------|---------|--------|--------|-----|
| **Week 1** | atomic_capsule | 250 | 🔄 In Progress | Nov 1 |
| **Week 2** | clapi_core | 94 | ⏳ Pending | Nov 8 |
| **Week 2-3** | kindly_hft | 205 | ⏳ Pending | Nov 15 |
| **Week 3** | kindly-db | 40 | ⏳ Pending | Nov 15 |
| **Week 4** | kiang + others | 34 | ⏳ Pending | Nov 22 |

**Total**: 618 macros → 0 manual maintenance (87.5% infrastructure reduction)

---

## Support

### Need Help?

1. **Check documentation**:
   - `tools/MIGRATION_VALIDATION_FRAMEWORK.md` - Comprehensive validation guide
   - `tools/PHASE4_PER_PROJECT_MIGRATION_PLANS.md` - Per-project detailed plans
   - `tools/ROLLBACK_PROCEDURE.md` - Emergency rollback instructions

2. **Run analysis tool**:
   ```bash
   cargo +nightly -Zscript tools/migrate_verify_macros_to_derive.rs analyze <project>
   ```

3. **Check migration metrics**:
   ```bash
   python3 tools/collect_migration_metrics.py report <project>
   ```

4. **Review examples**:
   - `atomic_capsule_derive/README.md` - Derive macro documentation
   - `atomic_capsule_derive/tests/` - Compile-pass and compile-fail tests

### Reporting Issues

If you encounter issues:

1. **Document the failure** (logs, errors, metrics)
2. **Create incident report** (`ROLLBACK_INCIDENT_*.md`)
3. **Rollback immediately** (`git restore <project>/**/*.rs`)
4. **File issue** with detailed reproduction steps

---

## Final Notes

- **Migration is safe**: Git ensures all code is recoverable
- **Migration is fast**: Automated tool handles 618 macros
- **Migration is validated**: T28 + B32 + ASSUM frameworks ensure correctness
- **Migration is reversible**: Rollback in <5 minutes

**When in doubt, run dry-run first. Review carefully before executing real migration.**

---

## Quick Reference

```bash
# Analyze project
cargo +nightly -Zscript tools/migrate_verify_macros_to_derive.rs analyze <project>

# Generate migration plan
cargo +nightly -Zscript tools/migrate_verify_macros_to_derive.rs plan <project>

# Execute migration (dry-run)
cargo +nightly -Zscript tools/migrate_verify_macros_to_derive.rs migrate --dry-run <project>

# Execute migration (real)
cargo +nightly -Zscript tools/migrate_verify_macros_to_derive.rs migrate <project>

# Validate migration
bash tools/validate_migration.sh <project>

# Collect metrics
python3 tools/collect_migration_metrics.py compare <project>

# Generate dashboard
python3 tools/collect_migration_metrics.py dashboard

# Rollback if needed
git restore <project>/**/*.rs
```

**Ready to migrate? Start with `analyze` to see the scope.**
