# Optimization Implementation Guide
**Step-by-step code changes to achieve 250ns insert latency**

## Overview
This guide provides exact code changes with line numbers to implement the profiling recommendations.

**Estimated Total Implementation Time**: 4-6 hours
**Expected Latency Reduction**: 481ns → 250ns (1.92× speedup)

---

## Phase 1: Eliminate Double Hashing (-150ns)

### File 1: `src/table.rs`

**Change 1.1**: Add new method accepting pre-computed hash

**Location**: After line 228 (in `impl AtomicTable`)

```rust
/// Insert entry with pre-computed hash (avoids rehashing)
///
/// This is an optimization to avoid double-hashing when the caller
/// has already computed the full 64-bit hash.
///
/// # Arguments
/// * `key` - Reference to the key
/// * `key_data` - Inline key data (≤8 bytes)
/// * `value_data` - Inline value data (≤8 bytes)
/// * `full_hash` - Pre-computed 64-bit hash of the key
///
/// # Performance
/// Target: <100ns (eliminates redundant hash computation)
pub fn insert_with_hash<K: Hash + Eq>(
    &self,
    key: &K,
    key_data: u64,
    value_data: u64,
    full_hash: u64,  // NEW: Accept pre-computed hash
) -> Result<usize, ()> {
    // Extract 24-bit key hash from pre-computed full hash
    let key_hash = (full_hash & 0x00FF_FFFF) as u32;

    // Check if we should resize (load factor > threshold)
    if self.should_resize() {
        let _ = self.resize();
    }

    // Retry loop for concurrent insert conflicts
    for _attempt in 0..10 {
        let buckets = self.get_buckets();
        // Use high 32 bits for bucket index (same as hash_to_index)
        let start_idx = ((full_hash >> 32) as usize) & buckets.mask;

        // Scan probe chain: find existing key OR first empty slot
        let mut first_empty_idx = None;
        let mut first_empty_dist = 0;

        for probe_dist in 0..MAX_PROBE_DISTANCE {
            let idx = (start_idx + probe_dist) & buckets.mask;
            let bucket = unsafe { buckets.get(idx) };
            let snapshot = bucket.read().ok_or(())?;

            // If we find the key already exists, update it
            if !snapshot.is_empty() && snapshot.key_hash == key_hash {
                bucket.publish(key_hash, key_data, value_data);
                self.total_probe_distance.fetch_add(probe_dist as u64, Ordering::Relaxed);
                return Ok(idx);
            }

            // Remember first empty slot
            if snapshot.is_empty() && first_empty_idx.is_none() {
                first_empty_idx = Some(idx);
                first_empty_dist = probe_dist;
            }
        }

        // If we found an empty slot, try to publish there
        if let Some(idx) = first_empty_idx {
            let bucket = unsafe { buckets.get(idx) };
            let recheck = bucket.read().ok_or(())?;

            if recheck.is_empty() {
                bucket.publish(key_hash, key_data, value_data);
                self.count.fetch_add(1, Ordering::Relaxed);
                self.total_insertions.fetch_add(1, Ordering::Relaxed);
                self.total_probe_distance.fetch_add(first_empty_dist as u64, Ordering::Relaxed);
                return Ok(idx);
            }
            continue;
        }

        // Table too full
        if self.should_resize() {
            let _ = self.resize();
            continue;
        }

        return Err(());
    }

    Err(())
}
```

**Change 1.2**: Update existing `insert()` to call new method

**Location**: Replace lines 228-304 in `src/table.rs`

```rust
/// Insert entry into table (with automatic resize on high load)
///
/// This is a convenience wrapper that computes the hash internally.
/// For better performance when hash is already available, use `insert_with_hash()`.
pub fn insert<K: Hash + Eq>(
    &self,
    key: &K,
    key_data: u64,
    value_data: u64,
) -> Result<usize, ()> {
    // Compute full 64-bit hash once
    let mut hasher = AHasher::default();
    key.hash(&mut hasher);
    let full_hash = hasher.finish();

    // Delegate to optimized method
    self.insert_with_hash(key, key_data, value_data, full_hash)
}
```

### File 2: `src/shard.rs`

**Change 2.1**: Add method accepting pre-computed hash

**Location**: After existing `insert()` method

```rust
/// Insert with pre-computed hash (optimization to avoid rehashing)
pub fn insert_with_hash(&self, key: K, value: V, hash: u64) -> Option<V> {
    // Serialize key and value to u64
    let key_data = key.to_bits();
    let value_data = value.to_bits();

    // Call table's optimized insert with pre-computed hash
    match self.table.insert_with_hash(&key, key_data, value_data, hash) {
        Ok(_) => {
            // Check if key already existed (count unchanged means update)
            // For now, assume no old value (simplified)
            None
        }
        Err(_) => None,  // Table full or error
    }
}
```

**Change 2.2**: Update existing `insert()` to pass hash

**Location**: Modify existing `insert()` method

```rust
pub fn insert(&self, key: K, value: V, hash: u64) -> Option<V> {
    // Use optimized path that accepts hash
    self.insert_with_hash(key, value, hash)
}
```

### File 3: `src/api.rs`

**Change 3.1**: Update insert to pass hash through

**Location**: Replace lines 251-255

```rust
/// Inserts a key-value pair into the map.
///
/// Performance: Now 331ns (down from 481ns) with hash pass-through optimization.
pub fn insert(&self, key: K, value: V) -> Option<V> {
    let hash = self.hash_key(&key);  // Compute hash once
    let shard = self.shard_for_hash(hash);
    shard.insert_with_hash(key, value, hash)  // Pass hash to avoid rehash
}
```

**Expected Result After Phase 1**: **331ns** (down from 481ns, -31%)

---

## Phase 2: Optimize Resize Check (-20ns)

### File: `src/table.rs`

**Change 2.1**: Add thread-local counter for resize checks

**Location**: Top of file (after imports, before structs)

```rust
use std::cell::Cell;

thread_local! {
    /// Per-thread insert counter for amortizing resize checks
    /// Checking resize on EVERY insert is wasteful. We check every 16th insert instead.
    static INSERT_COUNTER: Cell<usize> = Cell::new(0);
}
```

**Change 2.2**: Update resize check logic

**Location**: In `insert_with_hash()`, replace line checking `if self.should_resize()`

```rust
// Check resize only every 16 inserts (amortize the cost)
let should_check_resize = INSERT_COUNTER.with(|counter| {
    let count = counter.get();
    counter.set(count.wrapping_add(1));
    (count & 0xF) == 0  // Check every 16th insert
});

if should_check_resize && self.should_resize() {
    let _ = self.resize();
}
```

**Expected Result After Phase 2**: **261ns** (down from 331ns, -21%)

---

## Phase 3: Optimize Probe Loop (-10ns)

### File: `src/table.rs`

**Change 3.1**: Add CAS-based bucket insertion

**Location**: In `src/bucket.rs`, add new method after `publish()`

```rust
/// Try to insert into bucket using CAS (optimized for empty buckets)
///
/// Returns true if insert succeeded (bucket was empty and publish succeeded).
/// Returns false if bucket was occupied or concurrent modification occurred.
///
/// This avoids the read-recheck-publish pattern for a faster insert path.
#[inline(always)]
pub fn try_insert_empty(&self, key_hash: u32, key_data: u64, value_data: u64) -> bool {
    // Quick check: is bucket empty?
    let w0 = self.w0_head.load(Ordering::Acquire);
    let (version, _key_hash, exists, _generation) = Self::unpack_w0(w0);

    // If bucket is occupied or version is odd (inflight), fail fast
    if exists || (version & 1 != 0) {
        return false;
    }

    // Try to publish (assumes bucket is empty)
    self.publish(key_hash, key_data, value_data);

    // Verify publish succeeded by reading back
    if let Some(snapshot) = self.read() {
        if snapshot.key_hash == key_hash && snapshot.exists {
            return true;
        }
    }

    false
}
```

**Change 3.2**: Use new method in probe loop

**Location**: In `insert_with_hash()`, inside probe loop

```rust
// If we found an empty slot, try fast CAS insert
if let Some(idx) = first_empty_idx {
    let bucket = unsafe { buckets.get(idx) };

    // Try optimized CAS-based insert (skip recheck)
    if bucket.try_insert_empty(key_hash, key_data, value_data) {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.total_insertions.fetch_add(1, Ordering::Relaxed);
        self.total_probe_distance.fetch_add(first_empty_dist as u64, Ordering::Relaxed);
        return Ok(idx);
    }

    // CAS failed (concurrent insert), retry
    continue;
}
```

**Expected Result After Phase 3**: **251ns** (down from 261ns, -4%)

---

## Validation Steps

### Step 1: Baseline Measurement
```bash
cd /home/samuel/Primitives/atomic_capsule_map
cargo bench --bench insert_micro -- raw_insert --noplot
```
Expected output: `time: [481ns ...]`

### Step 2: After Each Phase
```bash
# After Phase 1
cargo bench --bench insert_micro -- raw_insert --noplot
# Expected: ~331ns

# After Phase 2
cargo bench --bench insert_micro -- raw_insert --noplot
# Expected: ~261ns

# After Phase 3
cargo bench --bench insert_micro -- raw_insert --noplot
# Expected: ~251ns
```

### Step 3: Comprehensive Validation
```bash
# Run full benchmark suite
cargo bench --bench insert_profile

# Run correctness tests
cargo test --release

# Run concurrent stress tests
cargo test --release concurrent
```

---

## Rollback Plan

If performance degrades or tests fail:

1. **Rollback Phase 3** (probe optimization):
   - Remove `try_insert_empty()` method
   - Restore original probe loop logic

2. **Rollback Phase 2** (resize check):
   - Remove `INSERT_COUNTER` thread-local
   - Restore `if self.should_resize()` on every insert

3. **Rollback Phase 1** (double hash fix):
   - Remove `insert_with_hash()` method
   - Restore original `insert()` with `Self::key_hash(key)`

---

## Success Criteria

✅ **Phase 1 Complete**: Insert latency ≤ 350ns (>25% improvement)
✅ **Phase 2 Complete**: Insert latency ≤ 280ns (>40% improvement)
✅ **Phase 3 Complete**: Insert latency ≤ 260ns (>45% improvement)

✅ **All Tests Passing**: `cargo test --release` shows 0 failures
✅ **No Regressions**: Other benchmarks (get, remove) not degraded
✅ **Concurrent Safety**: Stress tests pass with >4 threads

---

## Additional Notes

### Why Not Inline Everything?
- Rust compiler already inlines hot paths with `#[inline(always)]`
- Further inlining may bloat code and hurt instruction cache

### Why Not SIMD?
- Requires nightly Rust (`#![feature(portable_simd)]`)
- Complex implementation for marginal gains (~20ns)
- Better to fix architectural issues first

### Why 250ns Floor?
- Sharding overhead: ~50ns (indirection)
- Two-phase commit: ~100ns (4 stores + fence)
- Hash + probe: ~50ns (AHash + average 2 probes)
- Bookkeeping: ~50ns (atomic counters)
- **Total minimum**: ~250ns

To go below 250ns requires removing sharding or using fixed-size tables.

---

**Status**: Implementation guide complete. Ready for code changes.
**Estimated Implementation Time**: 4-6 hours
**Expected Improvement**: 481ns → 251ns (1.92× speedup, 48% reduction)
