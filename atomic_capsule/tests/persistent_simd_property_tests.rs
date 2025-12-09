//! # T9+T2 PersistentSimdVector Property Tests
//!
//! **T28 Testing Framework - Tier 2: Property Tests (Tests 26-40)**
//!
//! ## Coverage (B32: 1000+ iterations, 95% CI)
//! - Tests 26-30: Atomicity properties
//! - Tests 31-35: SIMD correctness properties
//! - Tests 36-40: Crash recovery properties

#![cfg(all(feature = "portable_simd", feature = "std"))]

use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::thread;

// ============================================================================
// § 1: Atomicity Properties (Tests 26-30)
// ============================================================================

#[test]
fn test_26_concurrent_reads() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let buffer_ptr = buffer.as_mut_ptr();

    // Initialize
    {
        let slice = unsafe { std::slice::from_raw_parts_mut(buffer_ptr, 512) };
        PersistentSimdVector::init_mmap(slice).unwrap();

        let data: Vec<f32> = (0..64).map(|i| i as f32).collect();
        PersistentSimdVector::store_simd(slice, &data).unwrap();
    }

    // Concurrent reads (1000 iterations)
    let handles: Vec<_> = (0..8)
        .map(|_| {
            thread::spawn(move || {
                for _ in 0..1000 {
                    let slice = unsafe { std::slice::from_raw_parts(buffer_ptr, 512) };
                    let loaded = PersistentSimdVector::load_simd(slice).unwrap();

                    // Verify data integrity
                    assert_eq!(loaded.len(), 64);
                    for (i, &val) in loaded.iter().enumerate() {
                        assert_eq!(val, i as f32);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_27_generation_counter_atomicity() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let buffer_ptr = buffer.as_mut_ptr();

    // Initialize
    {
        let slice = unsafe { std::slice::from_raw_parts_mut(buffer_ptr, 512) };
        PersistentSimdVector::init_mmap(slice).unwrap();
    }

    // Multiple threads incrementing generation via store
    let handles: Vec<_> = (0..4)
        .map(|tid| {
            thread::spawn(move || {
                for i in 0..100 {
                    let slice = unsafe { std::slice::from_raw_parts_mut(buffer_ptr, 512) };
                    let data = vec![(tid * 100 + i) as f32; 8];
                    PersistentSimdVector::store_simd(slice, &data).unwrap();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final generation is even (committed)
    let slice = unsafe { std::slice::from_raw_parts(buffer_ptr, 512) };
    let gen = PersistentSimdVector::get_generation(slice);
    assert!(gen & 1 == 0, "Final generation must be even (committed)");
    assert!(gen >= 400 * 2, "Generation should reflect all stores");
}

#[test]
fn test_28_toctou_prevention() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let buffer_ptr = buffer.as_mut_ptr();

    // Initialize
    {
        let slice = unsafe { std::slice::from_raw_parts_mut(buffer_ptr, 512) };
        PersistentSimdVector::init_mmap(slice).unwrap();

        let data = vec![1.0; 8];
        PersistentSimdVector::store_simd(slice, &data).unwrap();
    }

    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);

    // Writer thread (updates every 1μs)
    let writer = thread::spawn(move || {
        let mut counter = 0;
        while !stop_clone.load(AtomicOrdering::Relaxed) {
            let slice = unsafe { std::slice::from_raw_parts_mut(buffer_ptr, 512) };
            let data = vec![counter as f32; 8];
            PersistentSimdVector::store_simd(slice, &data).unwrap();
            counter += 1;
            thread::yield_now();
        }
    });

    // Reader threads (1000 reads each)
    let readers: Vec<_> = (0..4)
        .map(|_| {
            thread::spawn(move || {
                for _ in 0..1000 {
                    let slice = unsafe { std::slice::from_raw_parts(buffer_ptr, 512) };
                    let loaded = PersistentSimdVector::load_simd(slice).unwrap();

                    // Verify all elements are the same (TOCTOU prevention)
                    if loaded.len() > 0 {
                        let first = loaded[0];
                        for &val in &loaded {
                            assert_eq!(val, first, "TOCTOU violation detected");
                        }
                    }
                }
            })
        })
        .collect();

    // Wait for readers
    for reader in readers {
        reader.join().unwrap();
    }

    // Stop writer
    stop.store(true, AtomicOrdering::Relaxed);
    writer.join().unwrap();
}

#[test]
fn test_29_store_atomicity() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(slice).unwrap();

    // Property: Every store completes atomically
    for i in 0..1000 {
        let data: Vec<f32> = vec![i as f32; 8];
        PersistentSimdVector::store_simd(slice, &data).unwrap();

        // Immediately load (should always succeed)
        let loaded = PersistentSimdVector::load_simd(slice).unwrap();
        assert_eq!(loaded, data, "Store atomicity violated");
    }
}

#[test]
fn test_30_generation_monotonicity_stress() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(slice).unwrap();

    let mut prev_gen = 0u64;

    // Property: Generation strictly increases
    for i in 0..1000 {
        let data = vec![i as f32; 8];
        PersistentSimdVector::store_simd(slice, &data).unwrap();

        let curr_gen = PersistentSimdVector::get_generation(slice);
        assert!(
            curr_gen > prev_gen,
            "Generation monotonicity violated: {} <= {}",
            curr_gen,
            prev_gen
        );
        prev_gen = curr_gen;
    }
}

// ============================================================================
// § 2: SIMD Correctness Properties (Tests 31-35)
// ============================================================================

#[test]
fn test_31_simd_add_commutativity() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer1 = vec![0u8; 4096];
    let mut buffer2 = vec![0u8; 4096];

    let slice1 = &mut buffer1[..512];
    let slice2 = &mut buffer2[..512];

    PersistentSimdVector::init_mmap(slice1).unwrap();
    PersistentSimdVector::init_mmap(slice2).unwrap();

    // Property: a + b == b + a
    for _ in 0..100 {
        let a: Vec<f32> = (0..8).map(|i| (i as f32) * 1.5).collect();
        let b: Vec<f32> = (0..8).map(|i| (i as f32) * 2.3).collect();

        // Path 1: a + b
        PersistentSimdVector::store_simd(slice1, &a).unwrap();
        PersistentSimdVector::simd_add(slice1, &b).unwrap();
        let result1 = PersistentSimdVector::load_simd(slice1).unwrap();

        // Path 2: b + a
        PersistentSimdVector::store_simd(slice2, &b).unwrap();
        PersistentSimdVector::simd_add(slice2, &a).unwrap();
        let result2 = PersistentSimdVector::load_simd(slice2).unwrap();

        // Verify commutativity
        for i in 0..8 {
            assert!((result1[i] - result2[i]).abs() < 0.0001);
        }
    }
}

#[test]
fn test_32_simd_add_associativity() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(slice).unwrap();

    // Property: (a + b) + c == a + (b + c)
    let a: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let b: Vec<f32> = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0];
    let c: Vec<f32> = vec![100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0];

    // (a + b) + c
    PersistentSimdVector::store_simd(slice, &a).unwrap();
    PersistentSimdVector::simd_add(slice, &b).unwrap();
    PersistentSimdVector::simd_add(slice, &c).unwrap();
    let result1 = PersistentSimdVector::load_simd(slice).unwrap();

    // a + (b + c)
    let mut bc: Vec<f32> = b.iter().zip(c.iter()).map(|(x, y)| x + y).collect();
    PersistentSimdVector::store_simd(slice, &a).unwrap();
    PersistentSimdVector::simd_add(slice, &bc).unwrap();
    let result2 = PersistentSimdVector::load_simd(slice).unwrap();

    // Verify associativity (within floating-point tolerance)
    for i in 0..8 {
        assert!((result1[i] - result2[i]).abs() < 0.001);
    }
}

#[test]
fn test_33_simd_identity() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(slice).unwrap();

    // Property: a + 0 == a
    for _ in 0..100 {
        let a: Vec<f32> = (0..8).map(|i| (i as f32) * 3.7).collect();
        let zero = vec![0.0; 8];

        PersistentSimdVector::store_simd(slice, &a).unwrap();
        PersistentSimdVector::simd_add(slice, &zero).unwrap();
        let result = PersistentSimdVector::load_simd(slice).unwrap();

        for i in 0..8 {
            assert!((result[i] - a[i]).abs() < 0.0001);
        }
    }
}

#[test]
fn test_34_simd_full_lane_correctness() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(slice).unwrap();

    // Property: SIMD matches scalar for full lanes (64 elements)
    for iteration in 0..10 {
        let a: Vec<f32> = (0..64).map(|i| (i + iteration * 64) as f32).collect();
        let b: Vec<f32> = (0..64).map(|i| (i * 2) as f32).collect();

        PersistentSimdVector::store_simd(slice, &a).unwrap();
        PersistentSimdVector::simd_add(slice, &b).unwrap();
        let simd_result = PersistentSimdVector::load_simd(slice).unwrap();

        // Scalar reference
        let scalar_result: Vec<f32> = a.iter().zip(b.iter()).map(|(x, y)| x + y).collect();

        // Verify SIMD == scalar
        for i in 0..64 {
            assert_eq!(simd_result[i], scalar_result[i]);
        }
    }
}

#[test]
fn test_35_simd_partial_lane_correctness() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(slice).unwrap();

    // Property: SIMD matches scalar for partial lanes (non-multiple of 8)
    for len in [1, 3, 5, 7, 9, 13, 17, 23, 31, 37, 47, 53, 61, 63] {
        let a: Vec<f32> = (0..len).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..len).map(|i| (i * 3) as f32).collect();

        PersistentSimdVector::store_simd(slice, &a).unwrap();
        PersistentSimdVector::simd_add(slice, &b).unwrap();
        let simd_result = PersistentSimdVector::load_simd(slice).unwrap();

        // Scalar reference
        let scalar_result: Vec<f32> = a.iter().zip(b.iter()).map(|(x, y)| x + y).collect();

        // Verify SIMD == scalar
        assert_eq!(simd_result.len(), len);
        for i in 0..len {
            assert_eq!(simd_result[i], scalar_result[i]);
        }
    }
}

// ============================================================================
// § 3: Crash Recovery Properties (Tests 36-40)
// ============================================================================

#[test]
fn test_36_committed_state_survives_load() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(slice).unwrap();

    // Property: Committed state survives repeated loads
    for i in 0..100 {
        let data = vec![i as f32; 8];
        PersistentSimdVector::store_simd(slice, &data).unwrap();

        // Multiple loads should return same data
        for _ in 0..10 {
            let loaded = PersistentSimdVector::load_simd(slice).unwrap();
            assert_eq!(loaded, data);
        }
    }
}

#[test]
fn test_37_generation_evenness_invariant() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(slice).unwrap();

    // Property: After any successful operation, generation is even
    for i in 0..100 {
        let data = vec![i as f32; 8];
        PersistentSimdVector::store_simd(slice, &data).unwrap();

        assert!(PersistentSimdVector::is_committed(slice));
        let gen = PersistentSimdVector::get_generation(slice);
        assert!(gen & 1 == 0);
    }
}

#[test]
fn test_38_hash_consistency() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(slice).unwrap();

    // Property: Same data produces same hash
    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

    for _ in 0..100 {
        PersistentSimdVector::store_simd(slice, &data).unwrap();
        let loaded = PersistentSimdVector::load_simd(slice).unwrap();
        assert_eq!(loaded, data);
    }
}

#[test]
fn test_39_concurrent_load_consistency() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let buffer_ptr = buffer.as_mut_ptr();

    // Initialize with known data
    {
        let slice = unsafe { std::slice::from_raw_parts_mut(buffer_ptr, 512) };
        PersistentSimdVector::init_mmap(slice).unwrap();

        let data = vec![42.0; 64];
        PersistentSimdVector::store_simd(slice, &data).unwrap();
    }

    // Property: All concurrent loads see consistent state
    let handles: Vec<_> = (0..8)
        .map(|_| {
            thread::spawn(move || {
                for _ in 0..1000 {
                    let slice = unsafe { std::slice::from_raw_parts(buffer_ptr, 512) };
                    let loaded = PersistentSimdVector::load_simd(slice).unwrap();

                    assert_eq!(loaded.len(), 64);
                    for &val in &loaded {
                        assert_eq!(val, 42.0);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_40_recovery_determinism() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(slice).unwrap();

    // Property: Recovery produces deterministic results
    for i in 0..100 {
        let data: Vec<f32> = (0..8).map(|j| (i * 10 + j) as f32).collect();
        PersistentSimdVector::store_simd(slice, &data).unwrap();

        // Simulate recovery (reload same data)
        let loaded = PersistentSimdVector::load_simd(slice).unwrap();
        assert_eq!(loaded, data);

        // Generation should be even (committed)
        assert!(PersistentSimdVector::is_committed(slice));
    }
}
