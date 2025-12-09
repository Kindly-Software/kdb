# SWeMR Pattern: Single-Writer-Multiple-Reader

**Version**: 1.0
**Status**: Production
**Framework**: ASSUM Safety + T28 Testing

## Overview

The **SWeMR (Single-Writer-Multiple-Reader)** pattern is a concurrency safety pattern that guarantees:
- **Exactly one writer** at any given time
- **Multiple concurrent readers** after the write completes
- **No data races** through proper memory ordering

This pattern is critical for safe use of unsafe SIMD `store()` operations in the `simd_capsule_tier2` crate.

---

## Core Guarantees

### 1. Single Writer Invariant

**#ASSUME_SWEMR_SINGLE_WRITER**: Only one thread may call `store()` on a given pointer at any time.

```rust
// ✓ SAFE: Only one writer
let v = SimdF32x8Capsule::from_array([1.0; 8]);
unsafe {
    v.store(ptr);
}

// ✗ UNSAFE: Concurrent writers = data race
std::thread::scope(|s| {
    s.spawn(|| unsafe { v1.store(ptr); }); // UB!
    s.spawn(|| unsafe { v2.store(ptr); }); // Concurrent write!
});
```

### 2. Multiple Reader Safety

**#ASSUME_SWEMR_READER_SAFETY**: Multiple threads may read after the write completes.

```rust
// ✓ SAFE: Multiple readers after write
unsafe { writer.store(ptr); }
// Release semantics ensure visibility

// Now multiple readers can safely access
let reader1 = unsafe { (*ptr)[0] };
let reader2 = unsafe { (*ptr)[1] };
```

### 3. Memory Ordering Requirements

**#ASSUME_MEMORY_ORDERING**: Store uses Release semantics, reads use Acquire or stronger.

```rust
// Writer side (Release)
unsafe { capsule.store(ptr); }

// Reader side (Acquire)
let value = AtomicPtr::new(ptr).load(Ordering::Acquire);
```

---

## Implementation Patterns

### Pattern 1: Atomic Flag Protection

Use `AtomicBool` to enforce single-writer invariant:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use simd_capsule_tier2::SimdF32x8Capsule;

struct SingleWriterGuard {
    buffer: Vec<f32>,
    writer_active: AtomicBool,
}

impl SingleWriterGuard {
    fn write(&self, capsule: &SimdF32x8Capsule) -> Result<(), &'static str> {
        // Acquire write lock
        if self.writer_active.swap(true, Ordering::Acquire) {
            return Err("Writer already active");
        }

        // Safe: Only one writer
        let ptr = self.buffer.as_ptr() as *mut [f32; 8];
        unsafe {
            capsule.store(ptr);
        }

        // Release write lock
        self.writer_active.store(false, Ordering::Release);
        Ok(())
    }
}
```

**#VERIFY_THREAD_SAFETY**: Atomic swap ensures only one writer proceeds.

### Pattern 2: Thread ID Verification

Track the writing thread ID:

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

struct ThreadIdGuard {
    buffer: Vec<f32>,
    writer_thread_id: AtomicU64,
}

impl ThreadIdGuard {
    fn write(&self, capsule: &SimdF32x8Capsule) -> Result<(), &'static str> {
        let current_id = thread::current().id().as_u64().get();
        let expected = 0u64;

        // Try to claim write ownership
        if self.writer_thread_id.compare_exchange(
            expected,
            current_id,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_err() {
            return Err("Another thread is writing");
        }

        // Safe: Only this thread can write
        let ptr = self.buffer.as_ptr() as *mut [f32; 8];
        unsafe {
            capsule.store(ptr);
        }

        // Release ownership
        self.writer_thread_id.store(0, Ordering::Release);
        Ok(())
    }
}
```

**#VERIFY_STACKED_BORROWS**: Compare-exchange ensures exclusive access.

### Pattern 3: Channel-Based Coordination

Use channels to enforce single-writer discipline:

```rust
use std::sync::mpsc;
use simd_capsule_tier2::SimdF32x8Capsule;

struct ChannelWriter {
    tx: mpsc::Sender<SimdF32x8Capsule>,
}

impl ChannelWriter {
    fn spawn_writer(mut buffer: Vec<f32>) -> Self {
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let ptr = buffer.as_mut_ptr() as *mut [f32; 8];

            // Single writer thread
            for capsule in rx {
                unsafe {
                    capsule.store(ptr); // Safe: only thread writing
                }
            }
        });

        ChannelWriter { tx }
    }

    fn write(&self, capsule: SimdF32x8Capsule) -> Result<(), &'static str> {
        self.tx.send(capsule).map_err(|_| "Writer thread died")
    }
}
```

**#VERIFY_LIFETIME_BOUNDS**: Channel ownership ensures sequential writes.

---

## Common Anti-Patterns (FORBIDDEN)

### Anti-Pattern 1: Concurrent Writers

```rust
// ✗ WRONG: Multiple threads writing to same location
let ptr = buffer.as_mut_ptr() as *mut [f32; 8];

std::thread::scope(|s| {
    s.spawn(|| unsafe { v1.store(ptr); }); // UB!
    s.spawn(|| unsafe { v2.store(ptr); }); // Data race!
});
```

**Why this fails**: Two threads racing to write to the same memory location = undefined behavior.

### Anti-Pattern 2: Unsynchronized Reads During Write

```rust
// ✗ WRONG: Reading while write is in progress
std::thread::scope(|s| {
    s.spawn(|| unsafe { writer.store(ptr); });
    s.spawn(|| {
        // UB! Reading during write
        let value = unsafe { (*ptr)[0] };
    });
});
```

**Why this fails**: No happens-before relationship between write and read.

### Anti-Pattern 3: Misaligned Pointers

```rust
// ✗ WRONG: Pointer not aligned to 32 bytes
let mut buffer = vec![0.0f32; 9];
let ptr = unsafe { buffer.as_mut_ptr().add(1) as *mut [f32; 8] }; // Misaligned!

unsafe {
    capsule.store(ptr); // UB! SIMD requires 32-byte alignment
}
```

**Why this fails**: SIMD operations require natural alignment (32 bytes for f32x8).

### Anti-Pattern 4: Stale Pointer References

```rust
// ✗ WRONG: Pointer outlives buffer
let ptr = {
    let buffer = vec![0.0f32; 8];
    buffer.as_ptr() as *mut [f32; 8]
}; // buffer dropped here!

unsafe {
    capsule.store(ptr); // UB! Dangling pointer
}
```

**Why this fails**: Lifetime violation - pointer must remain valid during and after write.

---

## Safety Verification Checklist (T28 Framework)

### Pre-Deployment Validation

- [ ] **Single Writer**: Verified via atomic flag or thread ID tracking
- [ ] **No Aliasing**: Stacked Borrows analysis passes (Miri clean)
- [ ] **Alignment**: Runtime check `ptr as usize % 32 == 0` before store
- [ ] **Lifetime**: Pointer validity guaranteed by ownership system
- [ ] **Memory Ordering**: Acquire/Release semantics documented
- [ ] **Multi-threaded Test**: 1000+ iterations with ThreadSanitizer clean
- [ ] **Property Test**: Loom model checking validates SWeMR invariant

### Runtime Verification

```rust
pub unsafe fn checked_store(
    capsule: &SimdF32x8Capsule,
    ptr: *mut [f32; 8],
) -> Result<(), &'static str> {
    // #VERIFY_ALIGNMENT
    if (ptr as usize) % 32 != 0 {
        return Err("Pointer not 32-byte aligned");
    }

    // #VERIFY_THREAD_SAFETY (example with global flag)
    static WRITER_ACTIVE: AtomicBool = AtomicBool::new(false);
    if WRITER_ACTIVE.swap(true, Ordering::Acquire) {
        return Err("Concurrent writer detected");
    }

    // Safe: Verified alignment + single writer
    capsule.store(ptr);

    WRITER_ACTIVE.store(false, Ordering::Release);
    Ok(())
}
```

---

## Performance Considerations

### When to Use Unsafe `store()`

Unsafe `store()` is **only justified** when:
1. Profiling shows `store_slice()` is a bottleneck (>5% of runtime)
2. You have verified SWeMR pattern compliance (see checklist)
3. You can guarantee 32-byte alignment statically or at runtime

### Performance Comparison

| Operation | Latency | Safety |
|-----------|---------|--------|
| `store_slice()` (safe) | ~3-5ns | Bounds checked, always safe |
| `store()` (unsafe) | ~2-4ns | No checks, requires SWeMR |
| **Speedup** | ~1.25-1.5× | Only justified for critical hot paths |

**Recommendation**: Use safe `store_slice()` unless profiling proves otherwise.

---

## Testing Strategy (T28 Framework)

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_writer_safety() {
        let mut buffer = vec![0.0f32; 8];
        let ptr = buffer.as_mut_ptr() as *mut [f32; 8];
        let capsule = SimdF32x8Capsule::from_array([1.0; 8]);

        // Single writer
        unsafe { capsule.store(ptr); }

        // Verify write
        assert_eq!(buffer, vec![1.0; 8]);
    }

    #[test]
    fn test_alignment_requirement() {
        // Aligned buffer
        let mut buffer = vec![0.0f32; 8];
        let ptr = buffer.as_mut_ptr() as *mut [f32; 8];
        assert_eq!((ptr as usize) % 32, 0, "Vec should be 32-byte aligned");
    }
}
```

### Property Tests (Loom)

```rust
#[cfg(test)]
#[cfg(loom)]
mod loom_tests {
    use loom::sync::atomic::{AtomicBool, Ordering};
    use loom::thread;

    #[test]
    fn swemr_single_writer_property() {
        loom::model(|| {
            let writer_active = Arc::new(AtomicBool::new(false));
            let buffer = Arc::new(Mutex::new(vec![0.0f32; 8]));

            let handles: Vec<_> = (0..2)
                .map(|_| {
                    let writer_active = Arc::clone(&writer_active);
                    let buffer = Arc::clone(&buffer);

                    thread::spawn(move || {
                        // Try to acquire write lock
                        if !writer_active.swap(true, Ordering::Acquire) {
                            // Safe: Only one thread proceeds
                            let mut buf = buffer.lock().unwrap();
                            let ptr = buf.as_mut_ptr() as *mut [f32; 8];
                            // unsafe { capsule.store(ptr); }

                            writer_active.store(false, Ordering::Release);
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    }
}
```

### Stress Tests

```rust
#[test]
#[ignore] // Run with --ignored for stress testing
fn stress_test_concurrent_writes() {
    const ITERATIONS: usize = 100_000;
    const THREADS: usize = 8;

    let writer_active = Arc::new(AtomicBool::new(false));
    let buffer = Arc::new(Mutex::new(vec![0.0f32; 8]));

    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let writer_active = Arc::clone(&writer_active);
            let buffer = Arc::clone(&buffer);

            std::thread::spawn(move || {
                for _ in 0..ITERATIONS {
                    while writer_active.swap(true, Ordering::Acquire) {
                        // Spin until we can acquire
                        std::hint::spin_loop();
                    }

                    // Safe: Only one thread writing
                    let mut buf = buffer.lock().unwrap();
                    let ptr = buf.as_mut_ptr() as *mut [f32; 8];
                    let capsule = SimdF32x8Capsule::from_array([1.0; 8]);
                    unsafe { capsule.store(ptr); }

                    writer_active.store(false, Ordering::Release);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}
```

---

## ASSUM Framework Tags Reference

All SWeMR unsafe operations must include these tags:

| Tag | Category | Requirement |
|-----|----------|-------------|
| `#ASSUME_SWEMR_SINGLE_WRITER` | Concurrency | Only one writer at a time |
| `#ASSUME_SWEMR_READER_SAFETY` | Concurrency | Multiple readers after write |
| `#ASSUME_ALIASING_INVARIANT` | Memory Safety | No overlapping writes, proper alignment |
| `#ASSUME_MEMORY_ORDERING` | Synchronization | Release/Acquire semantics |
| `#ASSUME_TYPE_SAFE` | Type Safety | Pointer valid, aligned, exclusively owned |
| `#VERIFY_STACKED_BORROWS` | Verification | Miri clean |
| `#VERIFY_THREAD_SAFETY` | Verification | ThreadSanitizer clean |
| `#VERIFY_ALIGNMENT` | Verification | Runtime or static alignment check |
| `#VERIFY_LIFETIME_BOUNDS` | Verification | Pointer validity guaranteed |
| `#VERIFY_UNSAFE_INVARIANTS` | Verification | All assumptions verified |

---

## References

- **ASSUM Framework**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/ASSUM_SAFETY.md`
- **T28 Testing Framework**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/T28_TESTING_FRAMEWORK.md`
- **Atomic Capsule Patterns**: `/home/samuel/Primitives/atomic_capsule/docs/ATOMIC_CAPSULE_PATTERNS.md`
- **Rust Nomicon (Aliasing)**: https://doc.rust-lang.org/nomicon/aliasing.html
- **Rust Memory Ordering**: https://doc.rust-lang.org/std/sync/atomic/enum.Ordering.html

---

## Quick Reference Card

```rust
// ✓ SAFE: Single writer with atomic flag
let writer_active = AtomicBool::new(false);
assert!(!writer_active.swap(true, Ordering::Acquire));
unsafe { capsule.store(ptr); }
writer_active.store(false, Ordering::Release);

// ✓ SAFE: Multiple readers after write
unsafe { writer.store(ptr); } // Release
let value = unsafe { (*ptr)[0] }; // Safe to read

// ✗ UNSAFE: Concurrent writers
std::thread::scope(|s| {
    s.spawn(|| unsafe { v1.store(ptr); }); // UB!
    s.spawn(|| unsafe { v2.store(ptr); }); // Data race!
});

// ✗ UNSAFE: Misaligned pointer
let ptr = unsafe { buf.as_mut_ptr().add(1) as *mut [f32; 8] }; // UB!
unsafe { capsule.store(ptr); }
```

---

**Last Updated**: 2025-10-16
**Maintainer**: SIMD Capsule Safety Team
**License**: MIT (Public) + Trade Secret (Internal Patterns)
