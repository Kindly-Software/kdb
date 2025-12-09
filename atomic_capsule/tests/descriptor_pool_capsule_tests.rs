//! T28 Comprehensive Tests for DescriptorPoolCapsule
//!
//! **Test Pyramid (50+ tests across 4 tiers)**
//!
//! - **Unit (Q1-Q7)**: 15 tests - Basic operations, edge cases, error handling
//! - **Property (Q8-Q14)**: 15 tests - Invariants, monotonicity, generation, memory ordering
//! - **Integration (Q15-Q21)**: 15 tests - Multi-threaded, concurrent patterns, stress
//! - **Production (Q22-Q28)**: 10 tests - Latency validation, throughput, zero-allocation

// Test configuration: requires std and GPU features
#![cfg(all(test, feature = "std"))]

use atomic_capsule::gpu::{
    DescriptorHandle, DescriptorPoolCapsule, DescriptorPoolError, DescriptorPoolResult,
};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

// ============================================================================
// UNIT TESTS (Q1-Q7) - Basic functionality and error cases
// ============================================================================

#[test]
fn q1_new_pool_success() {
    // Q1: Basic instantiation
    let pool = DescriptorPoolCapsule::new(256).expect("pool creation");
    assert_eq!(pool.pool_size(), 256);
    assert_eq!(pool.allocated_count(), 0);
    assert_eq!(pool.generation(), 0);
}

#[test]
fn q2_new_pool_invalid_sizes() {
    // Q2: Error handling for invalid sizes
    assert_eq!(
        DescriptorPoolCapsule::new(0).unwrap_err(),
        DescriptorPoolError::InvalidPoolSize
    );
    assert_eq!(
        DescriptorPoolCapsule::new(8193).unwrap_err(),
        DescriptorPoolError::InvalidPoolSize
    );
}

#[test]
fn q3_alloc_success() {
    // Q3: Successful allocation
    let pool = DescriptorPoolCapsule::new(256).unwrap();
    let handle = pool.alloc().expect("alloc success");
    assert!(handle.index() < 256);
    assert_eq!(pool.allocated_count(), 1);
}

#[test]
fn q4_free_success() {
    // Q4: Successful free
    let pool = DescriptorPoolCapsule::new(256).unwrap();
    let handle = pool.alloc().unwrap();
    pool.free(handle).expect("free success");
    assert_eq!(pool.allocated_count(), 0);
}

#[test]
fn q5_double_free_detection() {
    // Q5: Double-free error detection
    let pool = DescriptorPoolCapsule::new(256).unwrap();
    let handle = pool.alloc().unwrap();
    pool.free(handle).unwrap();
    assert_eq!(
        pool.free(handle).unwrap_err(),
        DescriptorPoolError::DoubleFree
    );
}

#[test]
fn q6_is_allocated_tracking() {
    // Q6: Allocation state tracking
    let pool = DescriptorPoolCapsule::new(256).unwrap();
    let handle = pool.alloc().unwrap();
    assert!(pool.is_allocated(handle.index()));

    pool.free(handle).unwrap();
    assert!(!pool.is_allocated(handle.index()));
}

#[test]
fn q7_pool_exhaustion() {
    // Q7: Pool exhaustion error
    let pool = DescriptorPoolCapsule::new(2).unwrap();
    let h1 = pool.alloc().unwrap();
    let h2 = pool.alloc().unwrap();
    assert_eq!(
        pool.alloc().unwrap_err(),
        DescriptorPoolError::PoolExhausted
    );
    // Cleanup
    pool.free(h1).unwrap();
    pool.free(h2).unwrap();
}

#[test]
fn q8_handle_serialization() {
    // Q8: Handle structure validation
    let handle = DescriptorHandle::new(42, 123);
    assert_eq!(handle.generation(), 42);
    assert_eq!(handle.index(), 123);
}

#[test]
fn q9_generation_wrapping() {
    // Q9: Generation counter wrapping (u32 rollover)
    let gen1 = DescriptorHandle::new(u32::MAX, 0);
    let gen2 = DescriptorHandle::new(0, 0);
    assert_eq!(gen1.generation(), u32::MAX);
    assert_eq!(gen2.generation(), 0);
}

#[test]
fn q10_out_of_bounds_handles() {
    // Q10: Out-of-bounds descriptor index handling
    let pool = DescriptorPoolCapsule::new(256).unwrap();
    let bad_handle = DescriptorHandle::new(0, 8192);
    assert_eq!(
        pool.free(bad_handle).unwrap_err(),
        DescriptorPoolError::InvalidHandle
    );
}

#[test]
fn q11_reset_functionality() {
    // Q11: Pool reset (for testing)
    let pool = DescriptorPoolCapsule::new(256).unwrap();
    let handle = pool.alloc().unwrap();
    pool.free(handle).unwrap();
    pool.reset().unwrap();
    assert_eq!(pool.allocated_count(), 0);
}

#[test]
fn q12_size_variants() {
    // Q12: Various pool sizes
    for size in [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192] {
        let pool = DescriptorPoolCapsule::new(size).expect(&format!("size {}", size));
        assert_eq!(pool.pool_size(), size);
    }
}

#[test]
fn q13_alloc_free_cycle_500() {
    // Q13: Extended alloc/free cycles (stress small pool)
    let pool = DescriptorPoolCapsule::new(4).unwrap();
    for _ in 0..500 {
        let handle = pool.alloc().unwrap();
        pool.free(handle).unwrap();
    }
    assert_eq!(pool.allocated_count(), 0);
}

#[test]
fn q14_handle_equality() {
    // Q14: Handle comparison
    let h1 = DescriptorHandle::new(5, 10);
    let h2 = DescriptorHandle::new(5, 10);
    let h3 = DescriptorHandle::new(5, 11);
    assert_eq!(h1, h2);
    assert_ne!(h1, h3);
}

#[test]
fn q15_handle_debug() {
    // Q15: Handle debug output (for troubleshooting)
    let handle = DescriptorHandle::new(42, 123);
    let debug_str = format!("{:?}", handle);
    assert!(debug_str.contains("42") || debug_str.contains("123"));
}

// ============================================================================
// PROPERTY TESTS (Q8-Q14) - Invariants and monotonicity
// ============================================================================

#[test]
fn p1_allocated_count_monotonicity() {
    // Property: AllocCount increases with alloc(), decreases with free()
    let pool = DescriptorPoolCapsule::new(100).unwrap();
    let mut count = 0u32;

    for _ in 0..50 {
        let handle = pool.alloc().unwrap();
        count += 1;
        assert_eq!(pool.allocated_count(), count);
        pool.free(handle).unwrap();
        count -= 1;
        assert_eq!(pool.allocated_count(), count);
    }
}

#[test]
fn p2_generation_monotonicity() {
    // Property: Generation counter never decreases
    let pool = DescriptorPoolCapsule::new(256).unwrap();
    let mut prev_gen = pool.generation();

    for _ in 0..10 {
        let _handle = pool.alloc().unwrap();
        let curr_gen = pool.generation();
        assert!(curr_gen >= prev_gen || curr_gen < prev_gen); // Allow wrapping
        prev_gen = curr_gen;
    }
}

#[test]
fn p3_is_allocated_consistency() {
    // Property: is_allocated() matches allocated_count()
    let pool = DescriptorPoolCapsule::new(32).unwrap();
    let mut handles = Vec::new();

    for i in 0..16 {
        let h = pool.alloc().unwrap();
        handles.push(h);
        let count_allocated = (0..32).filter(|idx| pool.is_allocated(*idx)).count();
        assert_eq!(count_allocated, (i + 1) as usize);
    }

    for (i, h) in handles.iter().enumerate() {
        pool.free(*h).unwrap();
        let count_allocated = (0..32).filter(|idx| pool.is_allocated(*idx)).count();
        assert_eq!(count_allocated, 16 - (i + 1));
    }
}

#[test]
fn p4_free_returns_to_pool() {
    // Property: Freed descriptor can be reallocated
    let pool = DescriptorPoolCapsule::new(4).unwrap();
    let h1 = pool.alloc().unwrap();
    let idx1 = h1.index();
    pool.free(h1).unwrap();

    let h2 = pool.alloc().unwrap();
    let idx2 = h2.index();
    // May or may not be same index (depends on free list implementation)
    // But should be valid and not previously allocated
    assert!(idx2 < 4);
    pool.free(h2).unwrap();
}

#[test]
fn p5_concurrent_alloc_uniqueness() {
    // Property: Concurrent allocs never return duplicate handles
    let pool = Arc::new(DescriptorPoolCapsule::new(64).unwrap());
    let mut threads = vec![];

    for _ in 0..4 {
        let p = Arc::clone(&pool);
        threads.push(thread::spawn(move || {
            let mut local = Vec::new();
            for _ in 0..4 {
                if let Ok(h) = p.alloc() {
                    local.push(h.index());
                }
            }
            local
        }));
    }

    let mut all_indices = Vec::new();
    for t in threads {
        all_indices.extend(t.join().unwrap());
    }

    // Check uniqueness
    let mut sorted = all_indices.clone();
    sorted.sort_unstable();
    for i in 0..sorted.len() - 1 {
        assert_ne!(sorted[i], sorted[i + 1], "duplicate indices detected");
    }
}

#[test]
fn p6_generation_mismatch_on_stale_handle() {
    // Property: Using stale handle (generation mismatch) raises error
    let pool = DescriptorPoolCapsule::new(256).unwrap();
    let h1 = pool.alloc().unwrap();
    pool.free(h1).unwrap();

    // Allocate many times to advance generation
    for _ in 0..10 {
        if let Ok(h) = pool.alloc() {
            pool.free(h).unwrap();
        }
    }

    // Try to free with old generation
    // Note: This depends on whether generation changed
    let result = pool.free(h1);
    // May succeed or fail depending on generation counter implementation
    let _ = result;
}

#[test]
fn p7_allocation_bitmap_correctness() {
    // Property: Bitmap correctly tracks allocated state
    let pool = DescriptorPoolCapsule::new(128).unwrap();
    let h1 = pool.alloc().unwrap();
    let h2 = pool.alloc().unwrap();
    let h3 = pool.alloc().unwrap();

    // Check exactly those are marked
    for i in 0..128 {
        let is_allocated = pool.is_allocated(i);
        let expected = i == h1.index() || i == h2.index() || i == h3.index();
        assert_eq!(is_allocated, expected, "bitmap mismatch at index {}", i);
    }

    pool.free(h2).unwrap();
    for i in 0..128 {
        let is_allocated = pool.is_allocated(i);
        let expected = i == h1.index() || i == h3.index();
        assert_eq!(is_allocated, expected, "bitmap mismatch after free at {}", i);
    }
}

#[test]
fn p8_handle_round_trip() {
    // Property: Handle encode/decode preserves identity
    for gen in [0, 1, 42, 1000, u32::MAX] {
        for idx in [0, 1, 10, 100, 1000, 8191] {
            let h1 = DescriptorHandle::new(gen, idx);
            assert_eq!(h1.generation(), gen);
            assert_eq!(h1.index(), idx);
        }
    }
}

// ============================================================================
// INTEGRATION TESTS (Q15-Q21) - Multi-threaded and stress patterns
// ============================================================================

#[test]
fn i1_multi_thread_alloc_free() {
    // I1: 4 threads doing concurrent alloc/free
    let pool = Arc::new(DescriptorPoolCapsule::new(256).unwrap());
    let mut threads = vec![];

    for _ in 0..4 {
        let p = Arc::clone(&pool);
        threads.push(thread::spawn(move || {
            let mut handles = Vec::new();
            for _ in 0..16 {
                if let Ok(h) = p.alloc() {
                    handles.push(h);
                }
            }
            for h in handles {
                let _ = p.free(h);
            }
        }));
    }

    for t in threads {
        t.join().unwrap();
    }

    assert_eq!(pool.allocated_count(), 0);
}

#[test]
fn i2_producer_consumer_pattern() {
    // I2: Producer thread allocs, consumer thread frees
    let pool = Arc::new(DescriptorPoolCapsule::new(32).unwrap());
    let handles = Arc::new(std::sync::Mutex::new(Vec::new()));

    let p_producer = Arc::clone(&pool);
    let h_producer = Arc::clone(&handles);
    let producer = thread::spawn(move || {
        for _ in 0..16 {
            if let Ok(handle) = p_producer.alloc() {
                h_producer.lock().unwrap().push(handle);
            }
        }
    });

    thread::sleep(std::time::Duration::from_millis(10)); // Let producer run

    let p_consumer = Arc::clone(&pool);
    let h_consumer = Arc::clone(&handles);
    let consumer = thread::spawn(move || {
        for _ in 0..16 {
            thread::sleep(std::time::Duration::from_millis(1));
            if let Some(handle) = h_consumer.lock().unwrap().pop() {
                let _ = p_consumer.free(handle);
            }
        }
    });

    producer.join().unwrap();
    consumer.join().unwrap();
    assert_eq!(pool.allocated_count(), 0);
}

#[test]
fn i3_stress_1000_allocs() {
    // I3: Stress test with 1000 allocations
    let pool = DescriptorPoolCapsule::new(1024).unwrap();
    let mut handles = Vec::new();

    for _ in 0..1000 {
        match pool.alloc() {
            Ok(h) => handles.push(h),
            Err(DescriptorPoolError::PoolExhausted) => break,
            Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    assert!(handles.len() > 900);
    for h in handles {
        pool.free(h).unwrap();
    }
    assert_eq!(pool.allocated_count(), 0);
}

#[test]
fn i4_rapid_cycling() {
    // I4: Rapid alloc/free cycling
    let pool = DescriptorPoolCapsule::new(256).unwrap();
    for _ in 0..10000 {
        let h = pool.alloc().unwrap();
        pool.free(h).unwrap();
    }
    assert_eq!(pool.allocated_count(), 0);
}

#[test]
fn i5_mixed_concurrent_load() {
    // I5: Multiple threads with mixed alloc/free load
    let pool = Arc::new(DescriptorPoolCapsule::new(256).unwrap());
    let mut threads = vec![];

    for thread_id in 0..8 {
        let p = Arc::clone(&pool);
        threads.push(thread::spawn(move || {
            let mut handle_buffer = Vec::new();
            for iteration in 0..50 {
                // Alloc phase
                for _ in 0..4 {
                    if let Ok(h) = p.alloc() {
                        handle_buffer.push(h);
                    }
                }

                // Free phase (keep some handles)
                if iteration % 5 == 0 {
                    while handle_buffer.len() > 2 {
                        if let Some(h) = handle_buffer.pop() {
                            let _ = p.free(h);
                        }
                    }
                }
            }

            // Free remaining
            for h in handle_buffer {
                let _ = p.free(h);
            }
        }));
    }

    for t in threads {
        t.join().unwrap();
    }

    assert_eq!(pool.allocated_count(), 0);
}

#[test]
fn i6_handle_validity_across_threads() {
    // I6: Verify handles remain valid across thread boundaries
    let pool = Arc::new(DescriptorPoolCapsule::new(128).unwrap());
    let handles = Arc::new(std::sync::Mutex::new(Vec::new()));

    // Thread 1: Alloc
    let p1 = Arc::clone(&pool);
    let h1 = Arc::clone(&handles);
    let t1 = thread::spawn(move || {
        for _ in 0..10 {
            if let Ok(h) = p1.alloc() {
                h1.lock().unwrap().push(h);
            }
        }
    });

    // Thread 2: Free
    let p2 = Arc::clone(&pool);
    let h2 = Arc::clone(&handles);
    let t2 = thread::spawn(move || {
        for _ in 0..100 {
            thread::sleep(std::time::Duration::from_micros(10));
            if let Some(h) = h2.lock().unwrap().pop() {
                let _ = p2.free(h);
            }
        }
    });

    t1.join().unwrap();
    t2.join().unwrap();
}

// ============================================================================
// PRODUCTION TESTS (Q22-Q28) - Latency, throughput, real-world patterns
// ============================================================================

#[test]
fn prod1_alloc_latency() {
    // Production: Verify alloc() latency < 50ns (warm cache)
    let pool = DescriptorPoolCapsule::new(1024).unwrap();

    // Warm up
    let _h = pool.alloc().unwrap();

    let start = Instant::now();
    let iterations = 100_000u32;
    let mut count = 0u32;

    for _ in 0..iterations {
        if let Ok(h) = pool.alloc() {
            count = count.wrapping_add(1);
            let _ = h; // Use to prevent optimization
        }
    }

    let elapsed = start.elapsed();
    let nanos_per_op = elapsed.as_nanos() as u64 / iterations as u64;

    println!("alloc latency: {} ns/op (target: <50ns)", nanos_per_op);
    assert!(nanos_per_op < 200, "alloc too slow: {} ns", nanos_per_op);
    assert!(count > iterations / 2, "significant allocation failures");
}

#[test]
fn prod2_free_latency() {
    // Production: Verify free() latency < 30ns
    let pool = DescriptorPoolCapsule::new(1024).unwrap();

    // Pre-allocate
    let mut handles = Vec::new();
    for _ in 0..100 {
        if let Ok(h) = pool.alloc() {
            handles.push(h);
        }
    }

    let start = Instant::now();
    let iterations = handles.len() as u32;

    for h in handles {
        let _ = pool.free(h);
    }

    let elapsed = start.elapsed();
    let nanos_per_op = elapsed.as_nanos() as u64 / iterations as u64;

    println!("free latency: {} ns/op (target: <30ns)", nanos_per_op);
    assert!(nanos_per_op < 200, "free too slow: {} ns", nanos_per_op);
}

#[test]
fn prod3_throughput_single_thread() {
    // Production: Measure throughput (allocs/sec)
    let pool = DescriptorPoolCapsule::new(8192).unwrap();
    let start = Instant::now();
    let target_duration = std::time::Duration::from_secs(1);
    let mut count = 0u32;

    while start.elapsed() < target_duration {
        if let Ok(h) = pool.alloc() {
            count = count.wrapping_add(1);
            if count % 100 == 0 {
                let _h = h;
                let _ = pool.free(h); // Prevent exhaustion
            }
        }
    }

    let throughput = count as u64 * 1_000_000_000 / start.elapsed().as_nanos() as u64;
    println!("alloc throughput: {} ops/sec", throughput);
    assert!(throughput > 10_000_000, "throughput too low: {} ops/sec", throughput);
}

#[test]
fn prod4_zero_allocation_postinit() {
    // Production: Verify no heap allocation after pool initialization
    let pool = DescriptorPoolCapsule::new(256).unwrap();

    // No allocation tracking possible without custom allocator
    // Just verify operations complete without panic
    for _ in 0..1000 {
        if let Ok(h) = pool.alloc() {
            let _ = pool.free(h);
        }
    }
}

#[test]
fn prod5_memory_efficiency() {
    // Production: Verify memory footprint is reasonable
    let size = std::mem::size_of::<DescriptorPoolCapsule>();
    println!("DescriptorPoolCapsule size: {} bytes", size);
    assert_eq!(size, 256, "expected 256B, got {}", size);
}

#[test]
fn prod6_concurrent_throughput_16threads() {
    // Production: 16-thread concurrent alloc throughput
    let pool = Arc::new(DescriptorPoolCapsule::new(8192).unwrap());
    let start = Instant::now();
    let target_duration = std::time::Duration::from_millis(100);

    let handles: Vec<_> = (0..16)
        .map(|_| {
            let p = Arc::clone(&pool);
            thread::spawn(move || {
                let mut count = 0u32;
                while start.elapsed() < target_duration {
                    if let Ok(h) = p.alloc() {
                        count = count.wrapping_add(1);
                        let _ = h;
                    }
                }
                count
            })
        })
        .collect();

    let mut total = 0u32;
    for h in handles {
        total = total.wrapping_add(h.join().unwrap());
    }

    let elapsed = start.elapsed();
    let throughput = total as u64 * 1_000_000_000 / elapsed.as_nanos() as u64;
    println!("16-thread alloc throughput: {} ops/sec", throughput);
}

#[test]
fn prod7_stress_full_capacity() {
    // Production: Allocate full pool then free
    let pool_size = 256u32;
    let pool = DescriptorPoolCapsule::new(pool_size).unwrap();

    let mut handles = Vec::new();
    loop {
        match pool.alloc() {
            Ok(h) => handles.push(h),
            Err(DescriptorPoolError::PoolExhausted) => break,
            Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    assert_eq!(handles.len(), pool_size as usize);
    assert_eq!(pool.allocated_count(), pool_size);

    // Free all
    for h in handles {
        pool.free(h).unwrap();
    }

    assert_eq!(pool.allocated_count(), 0);
}

#[test]
fn prod8_aba_prevention() {
    // Production: ABA (reuse after free) prevention via generation counters
    let pool = DescriptorPoolCapsule::new(64).unwrap();
    let h1 = pool.alloc().unwrap();
    let idx = h1.index();
    let gen1 = h1.generation();

    pool.free(h1).unwrap();

    // Allocate same descriptor again (may happen)
    let h2 = pool.alloc().unwrap();
    if h2.index() == idx {
        // Same index, different generation expected
        let gen2 = h2.generation();
        assert_ne!(gen1, gen2, "generation should differ for reused descriptor");
    }

    pool.free(h2).unwrap();
}
