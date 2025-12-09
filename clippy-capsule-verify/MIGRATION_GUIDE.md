# Migration Guide: Adopting clippy-capsule-verify

**Safe, gradual adoption of Chaos compliance lints for existing codebases.**

## Overview

This guide walks through adopting `clippy-capsule-verify` in existing projects with
minimal disruption. Follow the phased approach to fix violations incrementally while
maintaining development velocity.

## Prerequisites

- **Rust nightly**: Required for clippy plugin (rustc_private)
- **Existing codebase**: Using computational capsule architecture
- **CI/CD pipeline**: For automated enforcement

```bash
# Install nightly Rust
rustup toolchain install nightly
rustup component add clippy --toolchain nightly
```

## Phase 1: Assessment (Week 1)

**Goal**: Understand current violations without breaking builds.

### Step 1: Run Lints in Warn Mode

```bash
# All lints as warnings (non-blocking)
cargo +nightly clippy --all-features --all-targets -- \
    -W clippy::capsule_mutex_violation \
    -W clippy::capsule_unaligned_violation \
    -W clippy::capsule_non_atomic_field \
    -W clippy::capsule_missing_generation \
    -W clippy::missing_capsule_verification \
    -W clippy::capsule_scattered_atomics \
    -W clippy::capsule_incorrect_padding \
    2>&1 | tee coca-assessment.log
```

### Step 2: Categorize Violations

```bash
# Count violations by type
grep "capsule_mutex_violation" coca-assessment.log | wc -l
grep "capsule_unaligned_violation" coca-assessment.log | wc -l
grep "capsule_non_atomic_field" coca-assessment.log | wc -l
grep "capsule_missing_generation" coca-assessment.log | wc -l
```

Example output:
```
capsule_mutex_violation: 3 occurrences
capsule_unaligned_violation: 12 occurrences
capsule_non_atomic_field: 7 occurrences
capsule_missing_generation: 18 occurrences
```

### Step 3: Prioritize Fixes

**Critical (P0)**: Fix immediately (data races, undefined behavior)
- `capsule_mutex_violation`: 100% lockfree mandate
- `capsule_unaligned_violation`: False sharing, SIMD crashes

**High (P0/P1)**: Fix within 2 weeks
- `capsule_missing_generation`: TOCTOU prevention
- `capsule_non_atomic_field`: Data race potential

**Medium (P1)**: Fix within 4 weeks
- `missing_capsule_verification`: Compile-time safety
- `capsule_scattered_atomics`: Complexity reduction
- `capsule_incorrect_padding`: Performance degradation

## Phase 2: Fix Critical Violations (Week 2-3)

### Violation: capsule_mutex_violation

**Problem**: Mutex/RwLock in capsule (violates lockfree mandate)

```rust
// ❌ BEFORE: Mutex in capsule
#[repr(C, align(64))]
struct BadCapsule {
    data: Mutex<HashMap<u64, u64>>,
    count: AtomicU64,
}
```

**Fix**: Replace with lockfree alternative

```rust
// ✅ AFTER: Lockfree hash table
use atomic_capsule::collections::LockfreeHashTable;

#[repr(C, align(64))]
struct GoodCapsule {
    data: LockfreeHashTable<u64, u64>,
    count: AtomicU64,
}
```

**Alternatives**:
- `LockfreeHashTable`: Concurrent maps (3.9× speedup)
- `AtomicU64`: Simple counters (<5ns)
- `DualAtomicU64`: Complex state coordination
- `RingBufferBroadcast`: Streaming updates (10ns append)

### Violation: capsule_unaligned_violation

**Problem**: Size not multiple of alignment (false sharing)

```rust
// ❌ BEFORE: 72 bytes with 64B alignment
#[repr(C, align(64))]
struct BadCapsule {
    state: AtomicU64,     // 8 bytes
    counter: AtomicU64,   // 8 bytes
    // Total: 16 bytes (not multiple of 64)
}
```

**Fix**: Add padding to align size

```rust
// ✅ AFTER: 64 bytes aligned
#[repr(C, align(64))]
struct GoodCapsule {
    state: AtomicU64,     // 8 bytes
    counter: AtomicU64,   // 8 bytes
    _padding: [u8; 48],   // 48 bytes padding (16 + 48 = 64)
}
```

**Formula**: `padding = alignment - (field_sizes % alignment)`

### Violation: capsule_missing_generation

**Problem**: No generation counter (TOCTOU races)

```rust
// ❌ BEFORE: Single atomic (TOCTOU vulnerable)
#[repr(C, align(64))]
struct BadCapsule {
    state: AtomicU64,
}
```

**Fix**: Add generation counter or use DualAtomicU64

```rust
// ✅ AFTER: DualAtomicU64 with generation
use atomic_capsule::patterns::DualAtomicU64;

#[repr(C, align(64))]
struct GoodCapsule {
    dual: DualAtomicU64,  // 16 bytes (primary + secondary with gen)
    _padding: [u8; 48],
}

// Access pattern
let (primary, secondary) = self.dual.load(Ordering::Acquire);
// primary: State(8) | Phase(8) | Count(16) | Gen(32)
// secondary: Data(32) | Gen(32)
```

**Alternative**: Explicit generation field

```rust
#[repr(C, align(64))]
struct GoodCapsule {
    state: AtomicU64,
    generation: AtomicU32,  // Explicit generation counter
    _padding: [u8; 52],
}
```

## Phase 3: Enable CI/CD Enforcement (Week 4)

### Step 1: Add P0 Critical Lints (Deny Level)

```yaml
# .github/workflows/coca-compliance.yml
- name: Chaos P0 Critical Checks
  run: |
    cargo +nightly clippy --all-features --all-targets -- \
      -D clippy::capsule_mutex_violation \
      -D clippy::capsule_unaligned_violation \
      -D clippy::capsule_non_atomic_field \
      -D clippy::capsule_missing_generation
```

**Why Deny Level?**
- Prevents new violations from entering codebase
- Catches bugs at compile-time (before runtime)
- Enforces 100% lockfree mandate

### Step 2: Add P1 Warnings (Non-blocking)

```yaml
- name: Chaos P1 Best Practices
  run: |
    cargo +nightly clippy --all-features --all-targets -- \
      -W clippy::missing_capsule_verification \
      -W clippy::capsule_scattered_atomics \
      -W clippy::capsule_incorrect_padding
  continue-on-error: true
```

**Why Warnings?**
- Non-blocking (builds still succeed)
- Gradual cleanup over time
- Developer awareness without disruption

## Phase 4: Fix Remaining Violations (Week 5-8)

### Violation: missing_capsule_verification

**Problem**: No compile-time verification macro

```rust
// ❌ BEFORE: No verification
#[repr(C, align(64))]
struct UnverifiedCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}
```

**Fix Option 1**: Add derive macro (recommended)

```rust
// ✅ AFTER: Automatic verification
#[derive(ComputationalCapsule)]
#[repr(C, align(64))]
struct VerifiedCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}
```

**Fix Option 2**: Manual verification macro

```rust
#[repr(C, align(64))]
struct VerifiedCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}

verify_capsule_properties!(VerifiedCapsule, 64, 64);
//                         ^^^^^^^^^^^^^^^^  ^^  ^^
//                         Struct name       align size
```

### Violation: capsule_scattered_atomics

**Problem**: Multiple atomic fields instead of DualAtomicU64

```rust
// ❌ BEFORE: Scattered atomics (complexity)
#[repr(C, align(64))]
struct ScatteredCapsule {
    state: AtomicU64,
    count: AtomicU64,
    phase: AtomicU32,
    generation: AtomicU32,
}
```

**Fix**: Consolidate into DualAtomicU64

```rust
// ✅ AFTER: DualAtomicU64 (single coordination point)
#[repr(C, align(64))]
struct ConsolidatedCapsule {
    dual: DualAtomicU64,
    _padding: [u8; 48],
}

// Pack state into dual atomic
// primary: State(8) | Phase(8) | Count(16) | Gen(32)
// secondary: Reserved(32) | Gen(32)
```

**Benefits**:
- Single atomic load captures full state
- Atomic snapshot (<10ns vs 4× separate loads)
- Generation counter prevents TOCTOU races

### Violation: capsule_incorrect_padding

**Problem**: Wrong padding calculation

```rust
// ❌ BEFORE: Incorrect padding (false sharing risk)
#[repr(C, align(64))]
struct BadPadding {
    state: AtomicU64,     // 8 bytes
    _padding: [u8; 52],   // 52 bytes (total: 60, not 64!)
}
```

**Fix**: Correct padding calculation

```rust
// ✅ AFTER: Correct padding
#[repr(C, align(64))]
struct GoodPadding {
    state: AtomicU64,     // 8 bytes
    _padding: [u8; 56],   // 56 bytes (total: 64 ✓)
}
```

**Formula**:
```
total_size = sum(field_sizes) + padding
padding = alignment - (sum(field_sizes) % alignment)

Example: 64B alignment, 8B field
padding = 64 - (8 % 64) = 64 - 8 = 56 bytes
```

## Phase 5: Cleanup Legacy Code (Ongoing)

### Strategy: Gradual Suppression Removal

**Step 1**: Tag legacy code with suppressions

```rust
// Legacy module under migration
#[allow(clippy::capsule_mutex_violation)]  // TECH DEBT: Refactor to lockfree
mod legacy {
    // ... existing code with Mutex ...
}
```

**Step 2**: Create migration tickets

```markdown
# Tech Debt: Migrate legacy module to lockfree

**Violation**: `capsule_mutex_violation` in `legacy::CapsuleName`

**Impact**: 30-100ns overhead, non-deterministic latency

**Fix**: Replace Mutex<HashMap> with LockfreeHashTable

**Effort**: 2 hours (low risk, high value)

**Priority**: P1 (fix within 4 weeks)
```

**Step 3**: Track suppression count over time

```bash
# Count suppressions
grep -r "#\[allow(clippy::capsule" src/ | wc -l

# Goal: Reduce by 20% per month
# Month 1: 50 suppressions
# Month 2: 40 suppressions (-20%)
# Month 3: 32 suppressions (-20%)
# Month 6: 0 suppressions (100% compliant)
```

## Common Migration Patterns

### Pattern 1: Mutex → LockfreeHashTable

```rust
// Before
struct Cache {
    data: Mutex<HashMap<u64, Vec<u8>>>,
}

// After
use atomic_capsule::collections::LockfreeHashTable;

struct Cache {
    data: LockfreeHashTable<u64, Vec<u8>>,
}
```

**Speedup**: 3.9× (measured on atomic_capsule benchmarks)

### Pattern 2: Multiple Atomics → DualAtomicU64

```rust
// Before
struct Coordinator {
    state: AtomicU64,
    generation: AtomicU32,
    phase: AtomicU32,
}

// After
use atomic_capsule::patterns::DualAtomicU64;

struct Coordinator {
    dual: DualAtomicU64,
}

// Pack state
impl Coordinator {
    pub fn pack_state(state: u8, gen: u32, phase: u8) -> (u64, u64) {
        let primary = (state as u64) << 56 | (gen as u64) << 24 | phase as u64;
        let secondary = gen as u64;
        (primary, secondary)
    }

    pub fn unpack_state(primary: u64, secondary: u64) -> (u8, u32, u8) {
        let state = (primary >> 56) as u8;
        let gen = ((primary >> 24) & 0xFFFFFFFF) as u32;
        let phase = (primary & 0xFF) as u8;
        (state, gen, phase)
    }
}
```

### Pattern 3: Unaligned Struct → Cache-Aligned

```rust
// Before (72 bytes, 64B align)
struct Unaligned {
    field1: AtomicU64,
    field2: AtomicU64,
    field3: AtomicU64,
    field4: AtomicU64,
    field5: AtomicU64,
    field6: AtomicU64,
    field7: AtomicU64,
    field8: AtomicU64,
    field9: AtomicU64,
}

// After (128 bytes, 128B align for >64B)
#[repr(C, align(128))]
struct Aligned {
    field1: AtomicU64,
    field2: AtomicU64,
    field3: AtomicU64,
    field4: AtomicU64,
    field5: AtomicU64,
    field6: AtomicU64,
    field7: AtomicU64,
    field8: AtomicU64,
    field9: AtomicU64,
    _padding: [u8; 56],  // 72 + 56 = 128
}
```

**Rule**: Use 128B alignment if struct size > 64B.

## Troubleshooting

### False Positive: T2+ Tier Flagged for Non-Atomic Fields

**Problem**: SIMD capsule flagged as T1 (Atomic) tier.

```rust
// T2 SIMD capsule incorrectly flagged
#[repr(C, align(64))]
struct SIMDCapsule {
    simd_data: [f32; 16],  // Warning: non-atomic field (FALSE POSITIVE)
}
```

**Workaround**: Add suppression with justification

```rust
#[allow(clippy::capsule_non_atomic_field)]  // T2 SIMD tier, not T1 Atomic
#[repr(C, align(64))]
struct SIMDCapsule {
    simd_data: [f32; 16],
}
```

**Future Fix**: Phase 2 will add explicit tier attributes:

```rust
#[tier = "T2"]  // Explicit SIMD tier (planned)
#[repr(C, align(64))]
struct SIMDCapsule {
    simd_data: [f32; 16],  // No warning
}
```

### Build Time Increases

**Problem**: Clippy adds 5-10% compilation overhead.

**Solution 1**: Enable caching

```yaml
# GitHub Actions
- uses: actions/cache@v3
  with:
    path: ~/.cargo
    key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
```

**Solution 2**: Incremental builds

```bash
# Don't clean before clippy
cargo clippy  # Uses incremental build cache
```

**Solution 3**: Parallel linting

```bash
cargo clippy --jobs $(nproc)  # Utilize all CPU cores
```

## Success Metrics

Track migration progress:

```bash
# Violations over time
echo "Week,P0,P1,Total" > migration-progress.csv
echo "1,22,45,67" >> migration-progress.csv
echo "2,15,40,55" >> migration-progress.csv
echo "3,8,35,43" >> migration-progress.csv
echo "4,0,30,30" >> migration-progress.csv
```

**Target**: Zero P0 violations within 4 weeks.

## Best Practices

1. **Start with P0 Critical**: Fix lockfree/alignment violations first
2. **Weekly reviews**: Track violation count, celebrate reductions
3. **Pre-commit hooks**: Instant feedback (prevent new violations)
4. **Documentation**: Update Chaos patterns as you fix violations
5. **Team training**: Share learnings, common patterns
6. **Gradual enforcement**: Warnings → Deny over 4-8 weeks

## Support

- **Questions**: Consult `/home/samuel/Docs/The Computational Capsule.md`
- **Patterns**: See `/home/samuel/Docs/The Atomic Capsule.md`
- **Examples**: Browse `atomic_capsule/src/` for production patterns
- **Issues**: File bug reports with minimal reproduction cases

## Timeline Summary

| Week | Phase | Focus | Outcome |
|------|-------|-------|---------|
| 1 | Assessment | Run lints, categorize violations | Violation count baseline |
| 2-3 | Critical Fixes | Fix P0 (Mutex, alignment, generation) | Zero P0 violations |
| 4 | CI/CD | Enable deny level for P0 | Automated enforcement |
| 5-8 | Cleanup | Fix P1, remove suppressions | 100% Chaos compliant |
| Ongoing | Maintenance | Review suppressions, track metrics | Zero tech debt |

**Total Time**: 4-8 weeks for full migration (varies by codebase size).

**ROI**: 100× faster violation detection, zero runtime bugs, deterministic latency.
