# Migration Guide: v0.3.x → v0.4.0

**Version**: 0.4.0
**Release Date**: November 2025
**Status**: ✅ Stable Release
**Migration Time**: 2-4 hours (mostly feature flag updates)
**Breaking Changes**: 5 (all documented below)

---

## Overview

v0.4.0 is a **major consolidation release** focusing on:

1. **Automatic Verification Default** - Manual macros deprecated, derive macro mandatory
2. **Feature Preset System** - 60+ flags → 7 curated presets
3. **WASM Support** - New tier support for browser/edge computing
4. **Platform Matrix** - Explicit tier availability by target
5. **Deprecated Primitives** - 5 legacy primitives marked for v0.5.0 removal

**Backward Compatibility**:
- ✅ All v0.3.x code still compiles
- ✅ All 60+ feature flags still work (aliased to presets)
- ✅ Performance unchanged or improved
- ❌ New code should use v0.4.0 patterns

---

## Breaking Change #1: Automatic Verification (P0 - Must Fix)

### Summary
All capsules MUST use `#[derive(ComputationalCapsule)]` or `#[capsule(...)]` attributes. Manual verification macros deprecated.

### What Changed

**v0.3.x** (OLD - will be removed v0.5.0):
```rust
#[repr(C, align(64))]
struct MyCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}

verify_capsule_properties! {
    MyCapsule: {
        alignment: 64,
        size: 64,
        fields: {
            state: AtomicU64 (offset: 0, size: 8),
            _padding: [u8; 56] (offset: 8, size: 56)
        }
    }
}
```

**v0.4.0** (NEW - required):
```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct MyCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}

// Zero manual work - verification happens at compile time
```

### Migration Steps

1. **Add derive macro dependency** (if not already enabled):
   ```toml
   [dependencies]
   atomic_capsule = { version = "0.4", features = ["derive"] }
   ```

2. **Replace manual macros** (search & replace):

   ```bash
   # Find all verify_capsule_properties! usage:
   find src -name "*.rs" -exec grep -l "verify_capsule_properties" {} \;

   # Find all verify_alignment_only! usage:
   find src -name "*.rs" -exec grep -l "verify_alignment_only" {} \;
   ```

3. **Update each capsule definition**:

   ```rust
   // Before
   #[repr(C, align(64))]
   struct MyState {
       counter: AtomicU64,
       _pad: [u8; 56],
   }
   verify_capsule_properties! { MyState: { alignment: 64, size: 64 } }

   // After
   #[derive(ComputationalCapsule)]
   #[capsule(alignment = 64, size = 64)]
   #[repr(C, align(64))]
   struct MyState {
       counter: AtomicU64,
       _pad: [u8; 56],
   }
   ```

4. **Run tests**:
   ```bash
   cargo test --lib
   # All tests should pass (derive macro produces identical verification)
   ```

### Compile-Time Verification

The derive macro automatically:
- ✅ Verifies alignment (checks `align(64)` matches `#[capsule(alignment = 64)]`)
- ✅ Verifies size (checks struct size matches `#[capsule(size = 64)]`)
- ✅ Verifies field layout (checks no gaps, proper alignment)
- ✅ Emits compile-time errors (not runtime panics)

**Example Error**:
```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 63)]  // ❌ Wrong size (struct is 64)
#[repr(C, align(64))]
struct Bad {
    x: AtomicU64,
    _pad: [u8; 56],
}
// Compile error: Expected size 63, got 64
```

### Suppression (if needed)

For backward compatibility during migration:
```rust
#[allow(clippy::missing_capsule_verification)]
#[repr(C, align(64))]
struct LegacyCode {
    state: AtomicU64,
}
// NOT RECOMMENDED - only for intermediate migration
```

(Will fail in v0.5.0, so add `#[derive]` as soon as possible)

---

## Breaking Change #2: Feature Preset System (P1 - Recommended)

### Summary
Old: `cargo build --features "portable_simd,fixed-point,parallel,distributed,cache-security-full,histogram"`
New: `cargo build --features preset-high-performance`

### What Changed

**v0.3.x** (60+ individual flags):
```toml
# Old: Must manually select 10-15 features
cargo build --features "\
  nightly,\
  portable_simd,\
  fixed-point,\
  fixed-simd,\
  parallel,\
  adaptive-parallel,\
  distributed,\
  cache-security-full,\
  histogram,\
  circuit-breaker-standard64,\
  const-hashing,\
  simd-hashing"
```

**v0.4.0** (7 curated presets):
```toml
# New: Single preset handles all settings
cargo build --features preset-high-performance
```

### Available Presets

| Preset | Tiers | Use Case | Feature Count |
|--------|-------|----------|---------------|
| `preset-wasm` | T0-T3, T5, T10 | Browser/Edge | 8 |
| `preset-embedded` | T0, T1, T3 | Microcontroller | 5 |
| `preset-development` | T0-T6 | Dev workstations | 15 |
| `preset-production` | T0-T10 | Servers (no SIMD) | 25 |
| `preset-high-performance` | T0-T6 + nightly | HFT/real-time | 18 |
| `preset-compliance` | T0-T10 + FIPS | Regulated | 30 |
| `preset-full-nightly` | All + nightly | Bleeding-edge | All 60+ |

### Migration Matrix

**Mapping v0.3.x → v0.4.0 presets**:

```
v0.3.x features                    v0.4.0 preset          Rationale
─────────────────────────────────  ──────────────────────  ─────────────
None (default)                     preset-development      ~80% of dev use
+ portable_simd                    preset-high-performance HFT optimization
+ mmap-persistence                 preset-production       Server workloads
+ fips-compliant                   preset-compliance       Financial/healthcare
No features                        preset-embedded         Embedded systems
wasm32 target + minimal            preset-wasm             Browser code
```

### Step-by-Step Migration

**1. Identify your current features** (Cargo.toml):
```toml
[dependencies]
atomic_capsule = { version = "0.3", features = [
    "parallel",
    "cache-security-full",
    "distributed",
] }
```

**2. Map to preset**:
- `parallel` + `distributed` + `cache-security-full` → `preset-production`

**3. Update dependency**:
```toml
# Before
atomic_capsule = { version = "0.3", features = ["parallel", "cache-security-full", "distributed"] }

# After
atomic_capsule = { version = "0.4", features = ["preset-production"] }
```

**4. Verify behavior**:
```bash
cargo build --release
cargo test --lib
# Results should be identical
```

### Custom Feature Combinations (Still Supported)

If you need a custom feature mix:

```toml
# Old style (v0.3.x) - STILL WORKS in v0.4.0:
atomic_capsule = { version = "0.4", features = [
    "portable_simd",
    "fixed-point",
    "distributed",
] }

# New style (v0.4.0) - RECOMMENDED:
atomic_capsule = { version = "0.4", features = ["preset-production"] }
# Then selectively override if needed
```

**Backward Compatibility**: ✅ All v0.3.x feature combinations still compile identically.

---

## Breaking Change #3: WASM Support (P1 - New Platform)

### Summary
New `preset-wasm` feature for browser/edge targets. T9 persistent tier unavailable on WASM.

### What's New

**v0.3.x**: No WASM support (x86_64/aarch64 only).

**v0.4.0**: Full WASM support with tier matrix:
- ✅ T0-T3, T5, T10 fully supported
- ⚠️ T4 limited (no rayon)
- ❌ T9 unavailable (no filesystem)

### Migration: Native → WASM

If porting existing code to WASM:

```toml
# Native (v0.3.x)
atomic_capsule = "0.3"

# WASM (v0.4.0)
atomic_capsule = { version = "0.4", default-features = false, features = ["preset-wasm"] }
```

**Update code**:
```rust
// Remove T9 persistent code for WASM targets
#[cfg(not(target_arch = "wasm32"))]
use atomic_capsule::persistence::CapsuleMmapRegion;

#[cfg(not(target_arch = "wasm32"))]
fn load_persistent() -> Result<()> {
    let mmap = CapsuleMmapRegion::create("file.bin")?;
    // ... WASM won't reach here
}

#[cfg(target_arch = "wasm32")]
fn load_persistent() -> Result<()> {
    // Use in-memory alternatives
    let hll = HyperLogLogCapsule::new();
    Ok(())
}
```

**Test both targets**:
```bash
cargo test --lib                              # Native
cargo test --lib --target wasm32-unknown-unknown --features preset-wasm  # WASM
```

See **docs/WASM_COMPATIBILITY.md** for complete WASM guide.

---

## Breaking Change #4: Platform Matrix (P1 - New Documentation)

### Summary
New explicit tier support matrix by platform. See docs/PLATFORM_MATRIX.md.

### What Changed

**v0.3.x**: No platform matrix (implicit support).

**v0.4.0**: Explicit tier availability:

```
Platform     T1  T2  T3  T4  T5  T6  T7  T8  T9  T10
─────────────────────────────────────────────────────
x86_64       ✅  ✅  ✅  ✅  ✅  ✅  ⚠️  ✅  ✅  ✅
aarch64      ✅  ✅  ✅  ✅  ✅  ✅  ⚠️  ✅  ✅  ✅
wasm32       ✅  ⚠️  ✅  ⚠️  ✅  ⚠️  ❌  ❌  ❌  ✅
riscv64      ✅  ❌  ✅  ✅  ✅  ⚠️  ❌  ✅  ✅  ✅
arm-cortex   ✅  ❌  ✅  ⚠️  ✅  ⚠️  ❌  ❌  ✅  ✅
```

### Migration: Validation by Target

Ensure your code works on intended targets:

```rust
// For x86_64/aarch64 (all tiers):
#[test]
fn test_all_tiers() {
    use atomic_capsule::*;
}

// For embedded (T0-T1-T3 only):
#[cfg(target_arch = "arm")]
#[test]
fn test_embedded_tiers() {
    // Compile-time check: can't use T2 SIMD or T9 persistent
}

// For WASM (T0-T3-T5-T10):
#[cfg(target_arch = "wasm32")]
#[test]
fn test_wasm_tiers() {
    // Compile-time check: can't use T9 or T8
}
```

---

## Breaking Change #5: Deprecated Primitives (P2 - Phase Out)

### Summary
5 primitives deprecated, marked for removal in v0.5.0 (Q1 2026).

### Deprecated Primitives

| Primitive | Replaced By | Module | Action |
|-----------|-------------|--------|--------|
| `PersistentMmap` | `CapsuleMmapRegion` | T9 Persistent | Use new version |
| `LockfreeResultAggregator` | `LockfreeResultAggregatorV3` | T4 Batch | Mandatory update |
| `LockfreeResultAggregatorV2` | `LockfreeResultAggregatorV3` | T4 Batch | Mandatory update |
| `verify_capsule_properties!` | `#[derive(ComputationalCapsule)]` | T0 Verify | Mandatory update |
| `verify_alignment_only!` | `#[capsule(...)]` | T0 Verify | Mandatory update |

### Migration: PersistentMmap → CapsuleMmapRegion

**v0.3.x** (old):
```rust
use atomic_capsule::persistence::PersistentMmap;

let mmap = PersistentMmap::create("data.bin", 1024*1024)?;
mmap.write(0, &data)?;
```

**v0.4.0** (new):
```rust
use atomic_capsule::persistence::CapsuleMmapRegion;

let mmap = CapsuleMmapRegion::create("data.bin", 1024*1024)?;
mmap.write(0, &data)?;
// API identical, implementation improved
```

**Behavior Difference**: Same API, internal implementation:
- `PersistentMmap`: Legacy wrapper (still works, calls `CapsuleMmapRegion`)
- `CapsuleMmapRegion`: Native capsule (aligned, verified, faster)

### Migration: LockfreeResultAggregator → LockfreeResultAggregatorV3

**v0.3.x** (old):
```rust
use atomic_capsule::parallel::LockfreeResultAggregator;

let agg = LockfreeResultAggregator::new(100);
agg.append(0, result);
let final_result = agg.merge();
```

**v0.4.0** (new - recommended):
```rust
use atomic_capsule::parallel::LockfreeResultAggregatorV3;

let agg = LockfreeResultAggregatorV3::new(100, |results| {
    // O(1) callback merge instead of O(n) blocking merge
    results.iter().sum()
});
agg.append(0, result);
// Automatic merge on drop (no explicit merge call)
```

**Key Difference**:
- V1/V2: Blocking `merge()` call (O(n) time)
- V3: Callback-based merge (O(1) time, COCA compliant)

See **docs/PHASE15_V3_MIGRATION_GUIDE.md** for complete details.

### Migration: Manual Macros → Derive Macro

See **Breaking Change #1** (above) - covered in detail there.

---

## Deprecation Timeline

| Version | Status | Changes |
|---------|--------|---------|
| v0.3.4 | ✅ Current | Manual macros work, derive optional |
| **v0.4.0** | ✅ **Latest** | **Derive mandatory, manual deprecated** |
| v0.4.x | → | Incremental migration (Q4 2025) |
| v0.5.0 | → | Manual macros removed (breaking) |
| v1.0 | → | All legacy removed (Q2 2026) |

**Action Required**:
- ✅ v0.3.x → v0.4.0: Add `#[derive]`, keep everything else
- ⏰ v0.4.0 → v0.5.0: Remove manual macros (deadline Q1 2026)
- ❌ v0.5.0+: No manual macros support

---

## Feature-by-Feature Migration

### Hash Features

**v0.3.x**:
```toml
# Old: Individual hash features
features = ["fast-hash", "const-hashing", "simd-hashing"]
```

**v0.4.0**:
```toml
# New: Preset includes all
features = ["preset-high-performance"]
# Or keep old style (still works):
features = ["fast-hash", "const-hashing", "simd-hashing"]
```

### Parallel Features

**v0.3.x**:
```toml
features = ["parallel", "adaptive-parallel", "ultra-low-latency", "nightly-adaptive"]
```

**v0.4.0**:
```toml
# Preset approach (recommended)
features = ["preset-high-performance"]

# Or keep old style (backward compatible)
features = ["parallel", "adaptive-parallel", "ultra-low-latency", "nightly-adaptive"]
```

### Collections Features

**v0.3.x**:
```toml
features = ["cache", "cache-security-full", "histogram", "distributed"]
```

**v0.4.0**:
```toml
# Preset approach (recommended)
features = ["preset-production"]

# Or keep old style (backward compatible)
features = ["cache", "cache-security-full", "histogram", "distributed"]
```

### Persistent Features

**v0.3.x**:
```toml
features = ["mmap-persistence", "persistent-dedup", "bloom-filter-persistent"]
```

**v0.4.0**:
```toml
# Preset includes all
features = ["preset-production"]

# Or old style (calls CapsuleMmapRegion internally)
features = ["mmap-persistence", "persistent-dedup", "bloom-filter-persistent"]
```

---

## Testing Migration

### Unit Tests

Run both v0.3.x and v0.4.0 to verify identical behavior:

```bash
# Current version (v0.3.4)
cargo test --lib

# Update Cargo.toml to v0.4.0, then:
cargo test --lib

# Results should be identical
```

### Feature Tests

Verify features compile correctly:

```bash
# Old feature style (should still work)
cargo build --features "parallel,distributed,cache-security-full"

# New preset style
cargo build --features preset-production

# Verify they're equivalent
cargo build --features "parallel" -p atomic_capsule
cargo build --features preset-high-performance -p atomic_capsule
# Binary size should be similar
```

### Integration Tests

If you have integration tests with external dependencies:

```bash
# Test all targets
cargo test --lib                          # Native
cargo test --lib --target aarch64-unknown-linux-gnu  # ARM
cargo test --lib --target wasm32-unknown-unknown --features preset-wasm  # WASM (if applicable)
```

---

## Backward Compatibility Statement

**v0.4.0 is backward compatible with v0.3.x** with one caveat:

| Aspect | Compatibility | Notes |
|--------|---------------|-------|
| API | ✅ 100% | All functions, traits, types work identically |
| Performance | ✅ Same or better | Derive macro has 0ns runtime cost |
| Features | ✅ All work | 60+ old flags aliased to presets |
| Primitives | ✅ 95 supported | 5 marked deprecated (still work) |
| Macro migrations | ⚠️ Recommended | Old macros work but emit deprecation warnings |

**Example**: This v0.3.x code compiles and runs identically in v0.4.0:

```rust
// v0.3.x code (unchanged)
#[repr(C, align(64))]
struct MyState {
    counter: AtomicU64,
    _pad: [u8; 56],
}
verify_capsule_properties! { MyState: { alignment: 64, size: 64 } }

impl MyState {
    fn new() -> Self { ... }
}

#[test]
fn test_state() {
    let s = MyState::new();
    // Works in v0.4.0 (but emit deprecation warning on manual macro)
}
```

Add `#[derive(ComputationalCapsule)]` when refactoring:

```rust
// v0.4.0 code (refactored)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct MyState {
    counter: AtomicU64,
    _pad: [u8; 56],
}

impl MyState {
    fn new() -> Self { ... }
}

#[test]
fn test_state() {
    let s = MyState::new();
    // Works identically
}
```

---

## Troubleshooting

### Error: "derive feature not enabled"

```
error[E0433]: cannot find derive macro `ComputationalCapsule` in this scope
```

**Fix**:
```toml
[dependencies]
atomic_capsule = { version = "0.4", features = ["derive"] }
# Or use any preset (all include derive):
atomic_capsule = { version = "0.4", features = ["preset-production"] }
```

### Error: "manual macros no longer available"

```
error[E0425]: cannot find macro `verify_capsule_properties` in this scope
```

**Fix**: Replace with derive macro (see Breaking Change #1).

### Error: "feature combination not supported"

```
error: feature `mmap-persistence` + `wasm32` not supported
```

**Fix**: Use conditional features:
```toml
[dependencies]
atomic_capsule = { version = "0.4" }

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
atomic_capsule = { version = "0.4", features = ["preset-production"] }

[target.'cfg(target_arch = "wasm32")'.dependencies]
atomic_capsule = { version = "0.4", default-features = false, features = ["preset-wasm"] }
```

### Error: "simd-hashing requires nightly"

```
error[E0433]: feature `simd-hashing` requires `nightly` feature
```

**Fix**: Either:
- Enable nightly: `cargo build --features "simd-hashing,nightly"`
- Use preset: `cargo build --features preset-high-performance` (includes nightly)

### Performance degradation

If performance regressed v0.3.x → v0.4.0:

1. **Verify feature flags**:
   ```bash
   cargo build --release
   # Ensure optimizations enabled (-C opt-level=3)
   ```

2. **Compare profiles**:
   ```bash
   cargo build --release --features "$(cargo read-manifest | jq -r .features)"
   # Should be identical to v0.3.x with same features
   ```

3. **Check derive macro overhead**:
   - Derive macro has 0ns runtime cost (compile-time only)
   - If slowdown observed, likely related to feature flag differences

4. **Profile with benchmark suite**:
   ```bash
   cargo bench --bench primitives
   # Compare v0.3.x vs v0.4.0 results (should be same)
   ```

---

## Migration Checklist

- [ ] Update `atomic_capsule` version: `0.3.x` → `0.4.0`
- [ ] Choose migration style:
  - [ ] Preset-based (recommended): `features = ["preset-production"]`
  - [ ] Keep old style: Verify 60+ feature combination still works
- [ ] Add `#[derive(ComputationalCapsule)]` to all capsule structs
- [ ] Remove manual verification macros (search for `verify_capsule_properties!`, `verify_alignment_only!`)
- [ ] Test all targets:
  - [ ] `cargo test --lib`
  - [ ] `cargo test --target wasm32-unknown-unknown` (if WASM relevant)
- [ ] Update persistent code (T9):
  - [ ] `PersistentMmap` → `CapsuleMmapRegion`
- [ ] Update parallel code (T4):
  - [ ] `LockfreeResultAggregator` → `LockfreeResultAggregatorV3`
  - [ ] `LockfreeResultAggregatorV2` → `LockfreeResultAggregatorV3`
- [ ] Run benchmarks: `cargo bench`
- [ ] Verify performance: Same or better than v0.3.x
- [ ] Merge and deploy

---

## Next Steps

1. **Read Documentation**:
   - Main: CLAUDE.md (this file's context)
   - WASM: docs/WASM_COMPATIBILITY.md
   - Platforms: docs/PLATFORM_MATRIX.md

2. **Update Code**:
   - Cargo.toml version + features
   - Replace verification macros
   - Update deprecated primitives

3. **Test**:
   - `cargo test --lib` (all platforms)
   - `cargo bench` (performance validation)

4. **Deploy**:
   - Merge to main branch
   - Tag release (git tag v0.4.0)
   - Publish crate (if applicable)

---

## Support & Questions

- **Documentation**: See CLAUDE.md § Breaking Changes (v0.4.0)
- **Examples**: examples/ directory (updated for v0.4.0)
- **Tests**: tests/ directory (comprehensive test coverage)
- **Frameworks**: UCE34 (Q1-Q34 systematic discovery)

---

**Last Updated**: November 2025
**Next Version**: v0.5.0 (Q1 2026)
