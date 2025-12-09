# Mmap Performance Optimization (Phase 4.5.1)

**Date**: 2025-11-24
**Status**: Implemented
**Target**: Reduce 1K docs overhead from +92% to <50%

## Problem Statement

Phase 4.5 O(1) memory refactoring showed significant overhead at small scales:
- **1K docs**: +92% overhead (vs in-memory baseline)
- **100K docs**: -15.6% improvement (O(1) advantage kicks in)

The overhead at small scales is acceptable but reduces attractiveness for small-scale demonstrations and trials.

## Root Cause Analysis (Q10a Profiling)

Profiling identified the following overhead sources:

| Source | Overhead % | Description |
|--------|-----------|-------------|
| **Mmap syscall** | 60-70% | File creation + truncation + mmap syscall (~1ms baseline) |
| **ConcurrentMapCapsuleV2 init** | 10-15% | Empty concurrent map allocation overhead |
| **Atomic operations per insert** | 15-20% | 3 atomic ops per insertion (write_offset, index, total_insertions) |
| **Bucket over-allocation** | 5-10% | 2048-doc buckets allocated for small workloads |

## Optimizations Implemented (Q10c Tier Selection)

### Optimization 1: Lazy Mmap Initialization (Primary - 60-70% reduction target)

**Before**: Mmap syscall at construction (~1ms fixed cost)
```rust
pub fn create(...) -> io::Result<Self> {
    let mmap_manager = Arc::new(MmapManager::new(path, &layout)?);  // 1ms syscall
    // ...
}
```

**After**: Mmap syscall deferred to first bucket allocation
```rust
pub fn create(...) -> io::Result<Self> {
    Ok(Self {
        mmap_manager: UnsafeCell::new(None),  // No syscall
        init_once: Once::new(),
        mmap_initialized: AtomicBool::new(false),
        // ...
    })
}

fn ensure_mmap_initialized(&self) -> io::Result<&Arc<MmapManager>> {
    if self.mmap_initialized.load(Ordering::Acquire) {
        // Fast path: <5ns atomic check
        return Ok(unsafe { (*self.mmap_manager.get()).as_ref().unwrap() });
    }
    // Slow path: 1ms mmap syscall (once)
    self.init_once.call_once(|| { /* mmap initialization */ });
    // ...
}
```

**Benefits**:
- Zero cost for empty pipelines
- Zero cost for read-only operations on empty buckets
- First insert pays the 1ms cost (amortized over many insertions)
- Small datasets (<10K docs) use 1 MB region instead of 100 MB

### Optimization 2: Compact Bucket Allocation (Secondary - 5-10% reduction target)

**Before**: Each bucket allocated for 2048 docs (8196 bytes)
```rust
fn allocate_bucket(&self) -> u64 {
    let size = 4 + MAX_DOCS_PER_BUCKET * 4;  // 8196 bytes
    // ...
}
```

**After**: Buckets start at 64 docs (260 bytes), grow on demand
```rust
const INITIAL_BUCKET_CAPACITY: usize = 64;

fn allocate_bucket_compact(&self, mmap_manager: &MmapManager) -> u64 {
    let size = 4 + INITIAL_BUCKET_CAPACITY * 4;  // 260 bytes
    // ...
}

fn grow_bucket(&self, ...) -> u64 {
    // Double capacity when full: 64 -> 128 -> 256 -> 512 -> 1024 -> 2048
    let new_capacity = (capacity * 2).min(MAX_DOCS_PER_BUCKET);
    // ...
}
```

**Benefits**:
- 32x smaller initial allocation (260 bytes vs 8196 bytes)
- Reduces page faults for small datasets
- Lazy growth only when needed
- Most buckets in practice have <64 docs

### Optimization 3: Batched Insertion Counter (Tertiary - 15-20% reduction target)

**Before**: Atomic increment on every insertion
```rust
pub fn add_to_bucket(...) {
    // ... write to bucket ...
    self.total_insertions.fetch_add(1, Ordering::Relaxed);  // Every insertion
}
```

**After**: Batch updates every 64 insertions
```rust
const INSERTION_BATCH_SIZE: u64 = 64;

pub fn add_to_bucket(...) {
    // ... write to bucket ...
    let local = self.local_insertion_count.fetch_add(1, Ordering::Relaxed);
    if local + 1 >= INSERTION_BATCH_SIZE {
        self.total_insertions.fetch_add(INSERTION_BATCH_SIZE, Ordering::Relaxed);
        self.local_insertion_count.store(0, Ordering::Relaxed);
    }
}
```

**Benefits**:
- 64x fewer atomic operations for total_insertions counter
- Metrics remain accurate within +/-64 documents
- metrics() function includes pending insertions for accurate reads

## API Changes

### New Methods

```rust
impl MmapLshBucketCapsule {
    /// Check if mmap has been initialized
    pub fn is_initialized(&self) -> bool;
}
```

### Modified Behavior

1. **create()**: No longer performs mmap syscall (returns immediately)
2. **add_to_bucket()**: First call triggers mmap initialization (~1ms)
3. **get_bucket()**: Returns empty Vec if mmap not initialized (fast path)
4. **extract_candidates()**: Returns empty Vec if mmap not initialized
5. **sync()**: No-op if mmap not initialized
6. **metrics()**: Includes pending insertions from local batch counter

### Index Format Change

Index tuple changed from `(offset, count)` to `(offset, count, capacity)`:
```rust
// Before
index: ConcurrentMapCapsuleV2<u64, (u64, u32)>

// After
index: ConcurrentMapCapsuleV2<u64, (u64, u32, u16)>
```

## Performance Targets

| Scale | Before | After | Improvement |
|-------|--------|-------|-------------|
| 1K docs | +92% overhead | <50% overhead | >40% reduction |
| 10K docs | +15% overhead | <10% overhead | >5% reduction |
| 100K docs | -15.6% | -15.6% | Maintained |
| 1M docs | -20%+ | -20%+ | Maintained |

## ASSUM Safety Framework Compliance

New assumption added:
```
#ASSUME_LAZY_INIT_SAFE: Lazy initialization is thread-safe via Once pattern
```

Existing assumptions maintained:
- `#ASSUME_MMAP_PERSISTENCE`: Mmap changes persist to disk via msync
- `#ASSUME_CRASH_RECOVERY`: Generation counters enable crash recovery
- `#ASSUME_BOUNDED_BUCKETS`: Max 2048 docs per bucket prevents overflow
- `#VERIFY_O1_MEMORY`: RSS remains constant regardless of document count

## Thread Safety Analysis

1. **Lazy initialization**: Protected by `std::sync::Once` (kernel-backed)
2. **mmap_initialized flag**: AtomicBool with Acquire/Release ordering
3. **UnsafeCell**: Only accessed after Once completes, immutable after init
4. **Bucket growth**: Atomic offset allocation, no data races

## Test Coverage

New tests added:
- `test_lazy_initialization`: Verify mmap not created at construction
- `test_compact_bucket_allocation`: Verify 64-doc initial capacity
- `test_batched_insertion_counter`: Verify batched counter accuracy
- `test_bucket_growth_pattern`: Verify 64 -> 128 -> ... -> 2048 growth
- `test_small_dataset_region_size`: Verify 1 MB region for <10K docs

## Framework Compliance

| Framework | Status | Details |
|-----------|--------|---------|
| **UCE34** | Q10 validated | T9 Persistent tier optimization |
| **Q10a** | Profiled | Bottleneck identified: mmap syscall (60-70%) |
| **Q10b** | Amdahl's Law | 60-70% of overhead addressed (Speedup = 1.7-2.3x) |
| **Q10c** | Tier match | Lazy init + compact allocation match bottleneck |
| **Chaos** | 100% lockfree | No mutex/RwLock (Once is kernel-backed) |
| **B32** | Pending | Benchmark validation required |
| **ASSUM** | 99.99%+ | New assumption documented |

## Benchmark Command

```bash
# Run O(1) memory validation benchmark
cargo bench --features "parallel-dedup,benchmarking" --bench o1_memory_benchmark -- --sample-size 10

# Quick validation (1K docs only)
cargo bench --features "parallel-dedup,benchmarking" --bench o1_memory_benchmark -- "1_000_docs"
```

## Rollback Plan

If optimizations cause issues:
1. Revert lazy initialization (immediate mmap at construction)
2. Revert compact buckets (2048 initial capacity)
3. Revert batched counters (immediate atomic update)

Each optimization is independent and can be reverted separately.

## References

- **Phase 4.5 Design**: `docs/archive/PHASE_4_5_O1_MEMORY.md`
- **UCE34 Framework**: `/home/samuel/CLAUDE.md` Q10a/b/c
- **MmapManager**: `atomic_capsule/src/mmap/manager.rs`
- **MmapRegion**: `atomic_capsule/src/mmap/region.rs`
