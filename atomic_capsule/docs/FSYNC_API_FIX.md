# fsync() API Fix: &mut self -> &self (v0.8.1)

## Executive Summary

Changed `MmapManager::fsync()` and the `Durable` trait from `&mut self` to `&self` to enable Chaos-compliant usage patterns. This is a **BREAKING CHANGE** that improves ergonomics for capsules using `Arc<MmapManager>`.

## Problem Statement

The `MmapManager::fsync()` method required `&mut self`, but capsules store `Arc<MmapManager>` for shared ownership. This created a conflict:

```rust
// BEFORE: Could not call fsync without Arc::get_mut()
pub struct MmapLshBucketCapsule {
    mmap_manager: Arc<MmapManager>,
}

impl MmapLshBucketCapsule {
    pub fn sync(&self) -> io::Result<()> {
        // ERROR: fsync requires &mut self, but we only have &self
        // self.mmap_manager.fsync()?;  // Won't compile!
        Ok(())
    }
}
```

## Root Cause Analysis

Examined the `fsync()` implementation in `/home/samuel/Primitives/atomic_capsule/src/mmap/manager.rs`:

```rust
pub fn fsync(&mut self) -> Result<(), MmapError> {
    // Bump generation before fsync (Q34 audit trail)
    self.generation.fetch_add(1, Ordering::Release);  // AtomicU64 - uses &self!

    // Platform-specific fsync
    unix::platform_fsync(self.ptr, self.size)?;  // Just a syscall - no mutation!

    Ok(())
}
```

**Findings**:
1. `generation.fetch_add()` is atomic - works with `&self` (interior mutability)
2. `platform_fsync()` is just an OS syscall (`msync`/`FlushViewOfFile`) - no Rust state mutation
3. The `&mut self` was unnecessary and overly restrictive

## Solution

Changed signature from `&mut self` to `&self` in:

| File | Change |
|------|--------|
| `src/mmap/manager.rs` | `pub fn fsync(&self)` |
| `src/platform/native/persistence/manager.rs` | `pub fn fsync(&self)` |
| `src/persistence/mod.rs` | `trait Durable { fn fsync(&self) }` |
| `src/persistence/mmap_manager.rs` | `impl Durable { fn fsync(&self) }` |
| `src/persistence/persistent_map.rs` | `impl Durable { fn fsync(&self) }` |
| `src/persistence/persistent_log.rs` | `impl Durable { fn fsync(&self) }` |

## Chaos Compliance

This change improves Chaos (Computational Capsule) compliance:

1. **Interior Mutability**: Generation counter uses `AtomicU64` (lockfree)
2. **&self Methods**: Capsules can call fsync without exclusive access
3. **Arc<T> Compatibility**: `Arc<MmapManager>` can call fsync directly
4. **Concurrent Access**: Multiple threads can call fsync simultaneously (safe - OS handles serialization)

## UCE34/ASSUM Documentation

### Assumptions
- `#ASSUME_FLUSH_DURABILITY`: Platform fsync guarantees persistence to disk
- `#ASSUME_ATOMIC_GENERATION`: Generation counter uses Release ordering for visibility

### Framework Compliance
- **Q10 (Tier)**: T9 Persistent tier (mmap-backed storage)
- **Q11 (ASSUM)**: fsync is a syscall, no Rust memory mutation needed
- **Q33 (Chaos)**: Maintained 100% lockfree guarantee
- **Q34 (Audit)**: Generation counter incremented for audit trail

## Test Results

All tests pass:

```
running 36 tests (mmap module)
test mmap::manager::tests::test_manager_fsync ... ok
...
test result: ok. 36 passed; 0 failed

running 45 tests (persistence module)
test persistence::mmap_manager::tests::test_mmap_manager_initialization ... ok
...
test result: ok. 45 passed; 0 failed
```

## Migration Guide

### Callers Using &mut
No change needed - calling `method(&self)` on `&mut T` works.

```rust
// This still works:
let mut manager = MmapManager::new(&path, &layout)?;
manager.fsync()?;  // OK - &mut T coerces to &self
```

### Callers Using Arc<T>
Now works without `Arc::get_mut()`:

```rust
// BEFORE: Required workaround
let manager = Arc::new(MmapManager::new(&path, &layout)?);
// manager.fsync()?;  // ERROR: cannot borrow as mutable

// AFTER: Works directly
let manager = Arc::new(MmapManager::new(&path, &layout)?);
manager.fsync()?;  // OK!
```

### Trait Implementors
Update `Durable` implementations to use `&self`:

```rust
// BEFORE
impl Durable for MyType {
    fn fsync(&mut self) -> Result<(), MmapError> { ... }
}

// AFTER
impl Durable for MyType {
    fn fsync(&self) -> Result<(), MmapError> { ... }
}
```

## Impact on kindly_dedup

The `MmapLshBucketCapsule` in `/home/samuel/Primitives/kindly_dedup/src/streaming/mmap_lsh_bucket_capsule.rs` can now properly call fsync:

```rust
// BEFORE (workaround)
pub fn sync(&self) -> io::Result<()> {
    // MmapManager::fsync requires &mut, but we have &self
    // For now, skip fsync - it will be called on Drop
    // TODO: Add Arc::get_mut() pattern or make fsync take &self
    Ok(())
}

// AFTER (fixed)
pub fn sync(&self) -> io::Result<()> {
    // Now works directly since fsync takes &self
    self.mmap_manager.fsync()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{:?}", e)))
}
```

## Files Modified

1. `/home/samuel/Primitives/atomic_capsule/src/mmap/manager.rs` (lines 314-331)
2. `/home/samuel/Primitives/atomic_capsule/src/platform/native/persistence/manager.rs` (lines 314-331)
3. `/home/samuel/Primitives/atomic_capsule/src/persistence/mod.rs` (lines 183-265)
4. `/home/samuel/Primitives/atomic_capsule/src/persistence/mmap_manager.rs` (lines 646-679)
5. `/home/samuel/Primitives/atomic_capsule/src/persistence/persistent_map.rs` (lines 851-881)
6. `/home/samuel/Primitives/atomic_capsule/src/persistence/persistent_log.rs` (lines 821-850)

## Version

- **atomic_capsule**: v0.8.1 (BREAKING CHANGE from v0.8.0)
- **Date**: 2025-11-24
- **Author**: Claude (Opus 4.5)

## See Also

- `/home/samuel/CLAUDE.md` - UCE34 Framework configuration
- `/home/samuel/Docs/The Computational Capsule.md` - Chaos architecture
- `/home/samuel/Primitives/kindly_dedup/src/streaming/mmap_lsh_bucket_capsule.rs` - Consumer of this API
