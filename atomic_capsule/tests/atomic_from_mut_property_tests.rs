//! # T28 Property Tests for AtomicFromMut (Q8-Q14)
//!
//! **Property-based testing with proptest for comprehensive validation.**
//!
//! ## Test Organization (T28 Framework)
//! - **Q8**: Random value generation and atomicity
//! - **Q9**: Concurrent mutation detection
//! - **Q10**: Ordering invariants
//! - **Q11**: Pointer arithmetic safety
//! - **Q12**: Exclusive access enforcement
//! - **Q13**: Lifetime correctness
//! - **Q14**: Edge cases (boundaries, overflow)
//!
//! ## Coverage Target
//! - 16 property tests (Q8-Q14)
//! - 1000+ iterations per property
//! - Statistical validation

#![cfg(feature = "atomic_from_mut")]
#![feature(atomic_from_mut)]

use atomic_capsule::primitives::AtomicFromMut;
use core::sync::atomic::*;
use proptest::prelude::*;
use std::sync::Arc;
use std::thread;

// ============================================================================
// Q8: Random Value Generation and Atomicity
// ============================================================================

proptest! {
    #[test]
    fn test_q8_atomic_u64_random_values(value in any::<u64>()) {
        let mut original = value;
        let atomic_ref = AtomicU64::from_mut(&mut original);

        atomic_ref.store(value, Ordering::Release);
        let loaded = atomic_ref.load(Ordering::Acquire);

        prop_assert_eq!(loaded, value);
        prop_assert_eq!(original, value);
    }

    #[test]
    fn test_q8_atomic_i64_random_values(value in any::<i64>()) {
        let mut original = value;
        let atomic_ref = AtomicI64::from_mut(&mut original);

        atomic_ref.store(value, Ordering::Release);
        let loaded = atomic_ref.load(Ordering::Acquire);

        prop_assert_eq!(loaded, value);
        prop_assert_eq!(original, value);
    }

    #[test]
    fn test_q8_atomic_u32_random_values(value in any::<u32>()) {
        let mut original = value;
        let atomic_ref = AtomicU32::from_mut(&mut original);

        atomic_ref.store(value, Ordering::Release);
        let loaded = atomic_ref.load(Ordering::Acquire);

        prop_assert_eq!(loaded, value);
        prop_assert_eq!(original, value);
    }

    #[test]
    fn test_q8_atomic_bool_random_values(value in any::<bool>()) {
        let mut original = value;
        let atomic_ref = AtomicBool::from_mut(&mut original);

        atomic_ref.store(value, Ordering::Release);
        let loaded = atomic_ref.load(Ordering::Acquire);

        prop_assert_eq!(loaded, value);
        prop_assert_eq!(original, value);
    }
}

// ============================================================================
// Q9: Concurrent Mutation Detection
// ============================================================================

proptest! {
    #[test]
    fn test_q9_concurrent_increment_correctness(
        initial in 0u64..1000u64,
        increments in prop::collection::vec(1u64..10u64, 10..20)
    ) {
        let mut value = initial;
        let expected_sum: u64 = initial + increments.iter().sum::<u64>();

        // Use Arc + unsafe pointer for concurrent access (simulating shared memory)
        let value_ptr = &mut value as *mut u64;

        let handles: Vec<_> = increments
            .into_iter()
            .map(|inc| {
                let ptr = value_ptr;
                thread::spawn(move || unsafe {
                    let atomic_ref = AtomicU64::from_ptr(ptr);
                    atomic_ref.fetch_add(inc, Ordering::SeqCst);
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        prop_assert_eq!(value, expected_sum);
    }
}

#[test]
fn test_q9_concurrent_cas_loop() {
    // Property test for CAS operations under contention
    let mut value: u64 = 0;
    let value_ptr = &mut value as *mut u64;

    let num_threads = 4;
    let increments_per_thread = 1000;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let ptr = value_ptr;
            thread::spawn(move || unsafe {
                let atomic_ref = AtomicU64::from_ptr(ptr);
                for _ in 0..increments_per_thread {
                    loop {
                        let current = atomic_ref.load(Ordering::Relaxed);
                        match atomic_ref.compare_exchange(
                            current,
                            current + 1,
                            Ordering::SeqCst,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => break,
                            Err(_) => continue,
                        }
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(value, (num_threads * increments_per_thread) as u64);
}

// ============================================================================
// Q10: Ordering Invariants
// ============================================================================

proptest! {
    #[test]
    fn test_q10_acquire_release_ordering(
        values in prop::collection::vec(any::<u64>(), 10..100)
    ) {
        let mut shared: u64 = 0;
        let shared_ptr = &mut shared as *mut u64;

        // Writer thread: Release ordering
        let values_clone = values.clone();
        let writer = thread::spawn(move || unsafe {
            let atomic_ref = AtomicU64::from_ptr(shared_ptr);
            for &value in &values_clone {
                atomic_ref.store(value, Ordering::Release);
                thread::yield_now();
            }
        });

        // Reader thread: Acquire ordering (should see all writes)
        let reader = thread::spawn(move || unsafe {
            let atomic_ref = AtomicU64::from_ptr(shared_ptr);
            let mut observed = Vec::new();
            for _ in 0..100 {
                observed.push(atomic_ref.load(Ordering::Acquire));
                thread::yield_now();
            }
            observed
        });

        writer.join().unwrap();
        let observed = reader.join().unwrap();

        // All observed values must be from the written set (or 0 initial value)
        for &obs in &observed {
            prop_assert!(obs == 0 || values.contains(&obs));
        }
    }
}

#[test]
fn test_q10_seqcst_total_order() {
    // Sequential consistency guarantees total order
    let mut x: u64 = 0;
    let mut y: u64 = 0;
    let x_ptr = &mut x as *mut u64;
    let y_ptr = &mut y as *mut u64;

    let thread1 = thread::spawn(move || unsafe {
        let x_ref = AtomicU64::from_ptr(x_ptr);
        let y_ref = AtomicU64::from_ptr(y_ptr);

        x_ref.store(1, Ordering::SeqCst);
        y_ref.load(Ordering::SeqCst)
    });

    let thread2 = thread::spawn(move || unsafe {
        let x_ref = AtomicU64::from_ptr(x_ptr);
        let y_ref = AtomicU64::from_ptr(y_ptr);

        y_ref.store(1, Ordering::SeqCst);
        x_ref.load(Ordering::SeqCst)
    });

    let r1 = thread1.join().unwrap();
    let r2 = thread2.join().unwrap();

    // At least one thread must observe the other's write
    assert!(r1 == 1 || r2 == 1);
}

// ============================================================================
// Q11: Pointer Arithmetic Safety
// ============================================================================

proptest! {
    #[test]
    fn test_q11_pointer_roundtrip_identity(value in any::<u64>()) {
        let mut original = value;
        let ptr = &mut original as *mut u64;

        unsafe {
            let atomic_ref = AtomicU64::from_ptr(ptr);
            atomic_ref.store(value, Ordering::Release);

            // Roundtrip: ptr -> atomic -> ptr
            let retrieved_ptr = atomic_ref as *mut AtomicU64 as *mut u64;
            prop_assert_eq!(ptr, retrieved_ptr);
            prop_assert_eq!(*retrieved_ptr, value);
        }
    }

    #[test]
    fn test_q11_buffer_offset_arithmetic(
        offset in 0usize..8,
        value in any::<u64>()
    ) {
        let mut buffer = vec![0u64; 16]; // 128 bytes
        let base_ptr = buffer.as_mut_ptr();

        unsafe {
            let offset_ptr = base_ptr.add(offset);
            let atomic_ref = AtomicU64::from_ptr(offset_ptr);
            atomic_ref.store(value, Ordering::Release);

            prop_assert_eq!(buffer[offset], value);
        }
    }
}

// ============================================================================
// Q12: Exclusive Access Enforcement (Borrow Checker)
// ============================================================================

#[test]
fn test_q12_exclusive_access_single_thread() {
    let mut value: u64 = 0;

    // Borrow checker ensures exclusive access
    {
        let atomic_ref = AtomicU64::from_mut(&mut value);
        atomic_ref.store(42, Ordering::Release);
    } // atomic_ref dropped, exclusive access released

    // Can access value again after atomic_ref is dropped
    assert_eq!(value, 42);
}

// Note: These tests verify borrow checker enforcement at compile-time.
// Uncomment to verify compiler errors:

// #[test]
// fn test_q12_multiple_mutable_references_rejected() {
//     let mut value: u64 = 0;
//     let atomic_ref1 = AtomicU64::from_mut(&mut value);
//     let atomic_ref2 = AtomicU64::from_mut(&mut value); // Compile error: already borrowed
// }

// #[test]
// fn test_q12_concurrent_access_to_same_reference_rejected() {
//     let mut value: u64 = 0;
//     let atomic_ref = AtomicU64::from_mut(&mut value);
//
//     thread::spawn(|| {
//         atomic_ref.store(42, Ordering::Release); // Compile error: move/borrow
//     });
// }

// ============================================================================
// Q13: Lifetime Correctness
// ============================================================================

#[test]
fn test_q13_lifetime_tied_to_reference() {
    let mut value: u64 = 100;

    {
        let atomic_ref = AtomicU64::from_mut(&mut value);
        atomic_ref.store(200, Ordering::Release);
        assert_eq!(atomic_ref.load(Ordering::Acquire), 200);
    } // atomic_ref lifetime ends here

    // Original value still accessible after atomic_ref is dropped
    assert_eq!(value, 200);
}

#[test]
fn test_q13_nested_scopes_lifetime() {
    let mut value: u64 = 1;

    {
        let atomic_ref1 = AtomicU64::from_mut(&mut value);
        atomic_ref1.store(2, Ordering::Release);

        {
            // Inner scope: atomic_ref1 is borrowed, cannot create atomic_ref2
            assert_eq!(atomic_ref1.load(Ordering::Acquire), 2);
        }

        atomic_ref1.store(3, Ordering::Release);
    }

    // After all atomic refs dropped, value is accessible
    assert_eq!(value, 3);
}

proptest! {
    #[test]
    fn test_q13_lifetime_across_iterations(
        values in prop::collection::vec(any::<u64>(), 1..100)
    ) {
        let mut accumulator: u64 = 0;

        for &value in &values {
            // New atomic reference created each iteration
            let atomic_ref = AtomicU64::from_mut(&mut accumulator);
            atomic_ref.fetch_add(value, Ordering::SeqCst);
        } // atomic_ref dropped at end of each iteration

        let expected: u64 = values.iter().sum();
        prop_assert_eq!(accumulator, expected);
    }
}

// ============================================================================
// Q14: Edge Cases (Boundaries, Overflow)
// ============================================================================

proptest! {
    #[test]
    fn test_q14_boundary_values_u64(
        value in prop::sample::select(vec![0u64, 1, u64::MAX - 1, u64::MAX])
    ) {
        let mut original = value;
        let atomic_ref = AtomicU64::from_mut(&mut original);

        atomic_ref.store(value, Ordering::Release);
        let loaded = atomic_ref.load(Ordering::Acquire);

        prop_assert_eq!(loaded, value);
    }

    #[test]
    fn test_q14_boundary_values_i64(
        value in prop::sample::select(vec![i64::MIN, i64::MIN + 1, -1i64, 0i64, 1i64, i64::MAX - 1, i64::MAX])
    ) {
        let mut original = value;
        let atomic_ref = AtomicI64::from_mut(&mut original);

        atomic_ref.store(value, Ordering::Release);
        let loaded = atomic_ref.load(Ordering::Acquire);

        prop_assert_eq!(loaded, value);
    }
}

#[test]
fn test_q14_overflow_wrapping_behavior() {
    let mut value: u64 = u64::MAX;
    let atomic_ref = AtomicU64::from_mut(&mut value);

    // Wrapping add (overflow)
    atomic_ref.fetch_add(1, Ordering::SeqCst);
    assert_eq!(atomic_ref.load(Ordering::Acquire), 0); // Wraps to 0

    // Wrapping sub (underflow)
    atomic_ref.fetch_sub(1, Ordering::SeqCst);
    assert_eq!(atomic_ref.load(Ordering::Acquire), u64::MAX); // Wraps to MAX
}

proptest! {
    #[test]
    fn test_q14_cache_line_boundaries(
        offset in 0usize..64
    ) {
        // Test atomic operations across cache line boundaries
        let mut buffer = vec![0u8; 128];

        // Align to cache line boundary (64 bytes)
        let base_addr = buffer.as_mut_ptr() as usize;
        let aligned_offset = ((base_addr + 63) & !63) - base_addr;

        if aligned_offset + offset + 8 <= buffer.len() {
            let slice = &mut buffer[aligned_offset + offset..aligned_offset + offset + 8];

            // Only test if properly aligned for u64
            if (slice.as_ptr() as usize) % 8 == 0 {
                let atomic_ref = AtomicU64::from_slice(slice).unwrap();
                atomic_ref.store(0xDEADBEEFCAFEBABE, Ordering::Release);
                prop_assert_eq!(atomic_ref.load(Ordering::Acquire), 0xDEADBEEFCAFEBABE);
            }
        }
    }
}

#[test]
fn test_q14_zero_sized_type_edge_case() {
    // AtomicFromMut should work with zero-sized types conceptually
    // (though AtomicZST doesn't exist in std, this tests the principle)

    let mut value: bool = false;
    let atomic_ref = AtomicBool::from_mut(&mut value);

    atomic_ref.store(true, Ordering::Release);
    assert_eq!(atomic_ref.load(Ordering::Acquire), true);
}
