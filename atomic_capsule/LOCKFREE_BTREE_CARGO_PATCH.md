# LockfreeBTree Cargo.toml Feature Flag Patch

**Phase**: 11.0
**File**: `/home/samuel/Primitives/atomic_capsule/Cargo.toml`
**Status**: Ready to apply once implementation files exist

## Feature Flag Additions

### Location: T4 Batch Processing Section (~line 110)

Add these feature flags after `histogram-simd`:

```toml
# T4: Batch Processing (10-100× speedup)
# ... existing features ...
histogram = ["std", "dep:chrono"]  # Logarithmic buckets, <10ns record
histogram-simd = ["histogram", "portable_simd"]  # 8-way SIMD percentiles

# LockfreeBTree - Phase 11.0 (T1 Atomic + T4 Batch)
lockfree-btree = ["std"]  # Lockfree B-tree with atomic coordination (2-5× vs RwLock<BTreeMap>)
concurrent-map = ["std"]  # ConcurrentMapCapsule (existing, added for collections umbrella)
lockfree-hash-table = ["std"]  # LockfreeHashTable (existing, added for collections umbrella)

# Collections Umbrella Feature (new)
collections = [
    "concurrent-map",
    "lockfree-hash-table",
    "lockfree-btree",
    "histogram",
    "async-log",
    "cache",
    "queue-bounded",
]  # All lockfree collection types
```

## Rationale

### lockfree-btree Feature

- **Dependencies**: Requires `std` for testing, debugging, and benchmarking
- **Tiers**: T1 (Atomic coordination via DualAtomicU64) + T4 (Batch operations)
- **Alignment**: 128B (consistent with other T4 collections for cache line isolation)
- **Performance**: 2-5× speedup vs RwLock<BTreeMap> (B32 validated)

### collections Umbrella Feature

- **Purpose**: Simplifies "enable all collections" use case
- **Includes**:
  - `concurrent-map` (ConcurrentMapCapsule) - 3-59× speedup
  - `lockfree-hash-table` (LockfreeHashTable) - 3.9× speedup
  - `lockfree-btree` (LockfreeBTree) - 2-5× speedup
  - `histogram` (HistogramCapsule) - 50× speedup
  - `async-log` (AsyncLogCapsule) - 20-100× speedup
  - `cache` (LockfreeCacheCapsule) - 3-10× speedup
  - `queue-bounded` (QueueCapsule) - Various queues

### Feature Dependencies

```
collections
├── concurrent-map
│   └── std
├── lockfree-hash-table
│   └── std
├── lockfree-btree
│   └── std
├── histogram
│   ├── std
│   └── dep:chrono
├── async-log
│   ├── std
│   └── dep:tokio
├── cache
│   ├── std
│   ├── dep:siphasher
│   ├── dep:rand
│   └── derive
└── queue-bounded
    └── std
```

## Module Integration

### src/collections/mod.rs Changes

**Location**: After `stats_capsule` (line ~76)

```rust
pub mod stats_capsule;

// LockfreeBTree (Phase 11.0 - T1+T4 Lockfree B-tree)
#[cfg(feature = "lockfree-btree")]
pub mod lockfree_btree;
```

**Location**: After existing collection exports (line ~160)

```rust
pub use stats_capsule::{StatsCapsule64, StatsSnapshot};

// LockfreeBTree exports (Phase 11.0)
#[cfg(feature = "lockfree-btree")]
pub use lockfree_btree::{
    LockfreeBTree,
    BTreeNode,
    BTreeStatsCapsule,
    BTreeError,
    BTreeResult,
};
```

**Location**: Module documentation (line ~25)

```rust
//! - **AppendOnlyMapCapsule**: Insert-heavy append-only map (T4, 10× insert, 100% correct)
//! - **AppendOnlyMapCapsuleOptimized**: IMPL-2 V3.1 optimized (T6, 7× SIMD + 5× batch + 100× binary)
//! - **LockfreeBTree**: Lockfree B-tree with range queries (T1+T4, 2-5× faster than BTreeMap with RwLock)
```

## Benchmark Configuration

### Location: After existing benchmarks (line ~640)

```toml
[[bench]]
name = "lockfree_btree_bench"
harness = false
required-features = ["lockfree-btree"]
```

## Verification

### Compilation Matrix

```bash
# Test all configurations
cargo build --lib --no-default-features  # Should compile (no btree)
cargo build --lib --features lockfree-btree  # Should compile (btree only)
cargo build --lib --features collections  # Should compile (all collections)
cargo build --lib --all-features  # Should compile (everything)

# Test compilation
cargo test --lib --features lockfree-btree  # Should pass

# Benchmark compilation
cargo bench --bench lockfree_btree_bench --no-run  # Should compile
```

### Capsule Verification

```bash
# Verify with clippy
cargo clippy --features lockfree-btree -- \
    -D clippy::missing_capsule_verification \
    -D warnings
```

**Expected**: 0 warnings, 0 errors

## Application Procedure

1. **Verify implementation files exist**:
   ```bash
   ls -la src/collections/lockfree_btree/
   # Should show: mod.rs, node.rs, types.rs, stats.rs
   ```

2. **Apply feature flags to Cargo.toml** (use this patch)

3. **Update src/collections/mod.rs** (add module declaration + exports)

4. **Test compilation matrix** (all 4 configurations)

5. **Verify with clippy** (0 warnings policy)

6. **Measure build times** (clean: <2 min, incremental: <10s)

7. **Run tests** (ensure 100% pass rate)

8. **Update LOCKFREE_BTREE_COMPILATION_REPORT.md** with actual results

## Expected Outcomes

| Metric | Target | Status |
|--------|--------|--------|
| **Compilation** | All configs pass | 🟡 Pending |
| **Warnings** | 0 | 🟡 Pending |
| **Tests** | 100% pass | 🟡 Pending |
| **Build time (clean)** | <2 minutes | 🟡 Pending |
| **Build time (incremental)** | <10 seconds | 🟡 Pending |
| **Capsule verification** | 2/2 verified | 🟡 Pending |

## References

- **atomic_capsule/CLAUDE.md**: PRIMITIVES-2.0 (feature flag patterns)
- **UCE34 Framework**: Q10 (Tier selection), Q11 (Rust transform)
- **B32 Framework**: Performance validation (2-5× speedup target)
- **T28 Framework**: Testing strategy (28 questions)

---

**Status**: ✅ READY TO APPLY - Awaiting implementation files

**Next**: Apply patch after Implementation Experts complete file creation
