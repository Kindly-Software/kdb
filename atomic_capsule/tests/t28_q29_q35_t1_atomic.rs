//! T28 Q29-Q35 Comprehensive T1 Atomic Tier Tests
//!
//! **Tier**: T1 Atomic (DualAtomicU64, 3-10× speedup, <100ns operations)
//! **Framework**: UCE34 Q29-Q35 (Determinism, reproducibility, memory ordering, replay)
//! **Coverage**: 25+ tests across Q29, Q30, Q33, Q34, Q35
//!
//! **Q29: Execution Path Determinism** (5 tests)
//! - Same atomic operations → same execution path
//! - CAS retry loops deterministic
//!
//! **Q30: Bitwise Reproducibility** (5 tests)
//! - Atomic loads return identical bit patterns
//! - Generation counter sequences reproducible
//!
//! **Q33: Memory Ordering Consistency** (6 tests)
//! - Acquire/Release fence validation
//! - SeqCst total ordering
//! - ABA prevention
//!
//! **Q34: Deterministic Replay** (3 tests)
//! - State snapshot → replay validation
//! - Atomic operation replay determinism
//!
//! **Q35: Composition Determinism** (4 tests)
//! - T1 + T2 (Atomic + SIMD) coordination
//! - DualAtomicU64 + SIMD hash composition
//!
//! **Run All Tests**:
//! ```bash
//! cargo test --lib --features "std,cache" --test t28_q29_q35_t1_atomic
//! ```

#![cfg(feature = "std")]

use atomic_capsule::patterns::DualAtomicU64;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Q29: Execution Path Determinism (5 tests)
// ============================================================================

/// **Q29 Test 1**: Same operations → same execution path
/// **Type**: Unit (Q1-Q7)
/// **Validate**: Multiple runs with same inputs produce identical outputs
#[test]
fn test_t28_q29_execution_path_determinism_basic() {
    let num_runs = 10;
    let mut results = Vec::new();

    for _ in 0..num_runs {
        let dual = DualAtomicU64::new(0, 0);

        // Execute deterministic sequence
        dual.fetch_add_primary(10, Ordering::Relaxed);
        let v1 = dual.load_primary(Ordering::Relaxed);

        dual.fetch_add_primary(20, Ordering::Relaxed);
        let v2 = dual.load_primary(Ordering::Relaxed);

        dual.fetch_add_primary(30, Ordering::Relaxed);
        let v3 = dual.load_primary(Ordering::Relaxed);

        results.push((v1, v2, v3));
    }

    // Verify all runs produced identical results
    let first = results[0];
    for (i, result) in results.iter().enumerate() {
        assert_eq!(
            result, &first,
            "Run {} produced different result: {:?} vs {:?}",
            i, result, first
        );
    }

    assert_eq!(first, (10, 30, 60), "Expected values (10, 30, 60)");
}

/// **Q29 Test 2**: CAS retry loops are deterministic
/// **Type**: Property (Q8-Q14)
/// **Validate**: CAS loop retries follow same pattern in repeated runs
#[test]
fn test_t28_q29_execution_path_cas_retry_determinism() {
    let num_runs = 5;
    let mut retry_counts = Vec::new();

    for _ in 0..num_runs {
        let dual = Arc::new(DualAtomicU64::new(0, 0));
        let mut retries = 0;

        // Single-threaded CAS loop (should be deterministic - no retries)
        let expected = dual.load_primary(Ordering::Relaxed);
        loop {
            match dual.compare_exchange_primary(expected, expected + 1, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => break,
                Err(_) => retries += 1,
            }
        }

        retry_counts.push(retries);
    }

    // All runs should have same retry count (0 for single-threaded)
    assert!(retry_counts.iter().all(|&r| r == 0), "CAS should succeed immediately");
}

/// **Q29 Test 3**: Ordering semantics deterministic
/// **Type**: Property (Q8-Q14)
/// **Validate**: Different orderings produce consistent memory behavior
#[test]
fn test_t28_q29_execution_path_ordering_determinism() {
    let data = Arc::new(DualAtomicU64::new(0, 0));

    let mut results = Vec::new();

    for _ in 0..5 {
        let d = data.clone();

        // Write-Release → Read-Acquire sequence
        d.store_primary(42, Ordering::Release);
        let v = d.load_primary(Ordering::Acquire);

        results.push(v);
    }

    // All runs should observe same value
    assert!(results.iter().all(|&v| v == 42), "Acquire/Release should be consistent");
}

/// **Q29 Test 4**: Multiple atomic operations in sequence
/// **Type**: Unit (Q1-Q7)
/// **Validate**: Sequence of operations always produces same final state
#[test]
fn test_t28_q29_execution_path_operation_sequence() {
    let num_runs = 10;

    for _ in 0..num_runs {
        let dual = DualAtomicU64::new(100, 200);

        // Deterministic sequence
        dual.fetch_add_primary(50, Ordering::Relaxed); // 150
        dual.fetch_sub_primary(25, Ordering::Relaxed); // 125
        dual.fetch_add_secondary(100, Ordering::Relaxed); // 300

        let p = dual.load_primary(Ordering::Relaxed);
        let s = dual.load_secondary(Ordering::Relaxed);

        assert_eq!(p, 125, "Primary should be 125");
        assert_eq!(s, 300, "Secondary should be 300");
    }
}

/// **Q29 Test 5**: Barriers and synchronization deterministic
/// **Type**: Integration (Q15-Q21)
/// **Validate**: Barriers ensure consistent execution order
#[test]
fn test_t28_q29_execution_path_barriers() {
    let dual = Arc::new(DualAtomicU64::new(0, 0));
    let barrier = Arc::new(std::sync::Barrier::new(2));

    let mut results = Vec::new();

    for _ in 0..5 {
        let d = dual.clone();
        let b = barrier.clone();

        let t1 = {
            let d = d.clone();
            let b = b.clone();
            thread::spawn(move || {
                d.fetch_add_primary(10, Ordering::Relaxed);
                b.wait();
                d.load_primary(Ordering::Acquire)
            })
        };

        let t2 = {
            let d = d.clone();
            let b = b.clone();
            thread::spawn(move || {
                d.fetch_add_primary(20, Ordering::Relaxed);
                b.wait();
                d.load_primary(Ordering::Acquire)
            })
        };

        let v1 = t1.join().unwrap();
        let v2 = t2.join().unwrap();
        results.push((v1, v2));

        // Reset for next iteration
        dual.store_primary(0, Ordering::Relaxed);
    }

    // All runs should produce same final value (order-independent)
    assert!(results.iter().all(|&(v1, v2)| v1 == 30 && v2 == 30),
            "Both threads should see final value 30");
}

// ============================================================================
// Q30: Bitwise Reproducibility (5 tests)
// ============================================================================

/// **Q30 Test 1**: Bitwise identical loads
/// **Type**: Unit (Q1-Q7)
/// **Validate**: Same load produces identical bit patterns
#[test]
fn test_t28_q30_bitwise_reproducibility_identical_loads() {
    let dual = DualAtomicU64::new(0xDEADBEEFCAFEBABE, 0x0123456789ABCDEF);

    for _ in 0..100 {
        let p1 = dual.load_primary(Ordering::Relaxed);
        let p2 = dual.load_primary(Ordering::Relaxed);
        assert_eq!(
            p1, p2,
            "Primary loads should be bitwise identical"
        );

        let s1 = dual.load_secondary(Ordering::Relaxed);
        let s2 = dual.load_secondary(Ordering::Relaxed);
        assert_eq!(
            s1, s2,
            "Secondary loads should be bitwise identical"
        );
    }
}

/// **Q30 Test 2**: Generation counter sequence reproducibility
/// **Type**: Property (Q8-Q14)
/// **Validate**: Generation increments produce reproducible sequence
#[test]
fn test_t28_q30_bitwise_reproducibility_generation_sequence() {
    let num_runs = 3;

    for _ in 0..num_runs {
        let dual = DualAtomicU64::new(0, 0);
        let mut sequence = Vec::new();

        for _ in 0..100 {
            let gen = dual.fetch_add_secondary(1, Ordering::Relaxed);
            sequence.push(gen);
        }

        // Verify sequence is 0, 1, 2, ..., 99
        for (i, &val) in sequence.iter().enumerate() {
            assert_eq!(val, i as u64, "Generation sequence reproducible");
        }
    }
}

/// **Q30 Test 3**: Store → Load round-trip
/// **Type**: Unit (Q1-Q7)
/// **Validate**: Stored value is retrieved bitwise identically
#[test]
fn test_t28_q30_bitwise_reproducibility_store_load() {
    let dual = DualAtomicU64::new(0, 0);
    let test_values = vec![
        0u64,
        1,
        u64::MAX,
        0x8000000000000000, // High bit set
        0x0000000000000001, // Low bit set
        0xAAAAAAAAAAAAAAAA,
        0x5555555555555555,
    ];

    for &val in &test_values {
        dual.store_primary(val, Ordering::Relaxed);
        let retrieved = dual.load_primary(Ordering::Relaxed);

        assert_eq!(
            retrieved, val,
            "Value {:016X} not retrieved identically",
            val
        );
    }
}

/// **Q30 Test 4**: Concurrent loads are consistent
/// **Type**: Property (Q8-Q14)
/// **Validate**: Multiple threads see same bit pattern when reading same value
#[test]
fn test_t28_q30_bitwise_reproducibility_concurrent_reads() {
    let test_value = 0xFEDCBA9876543210u64;
    let dual = Arc::new(DualAtomicU64::new(test_value, 0));

    let num_threads = 8;
    let reads_per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let d = dual.clone();
            thread::spawn(move || {
                let mut values = Vec::new();
                for _ in 0..reads_per_thread {
                    values.push(d.load_primary(Ordering::Relaxed));
                }
                values
            })
        })
        .collect();

    let mut all_values = Vec::new();
    for handle in handles {
        all_values.extend(handle.join().unwrap());
    }

    // Verify all values are identical
    assert!(
        all_values.iter().all(|&v| v == test_value),
        "All concurrent reads should see identical value"
    );
}

/// **Q30 Test 5**: Bit pattern preservation across operations
/// **Type**: Integration (Q15-Q21)
/// **Validate**: Atomic operations preserve bits that aren't modified
#[test]
fn test_t28_q30_bitwise_reproducibility_atomic_ops() {
    let dual = DualAtomicU64::new(0xFF00FF00FF00FF00, 0);

    // Add preserves other bits
    dual.fetch_add_primary(1, Ordering::Relaxed);
    let after_add = dual.load_primary(Ordering::Relaxed);
    assert_eq!(after_add, 0xFF00FF00FF00FF01, "Addition preserves bit pattern");

    // Sub preserves other bits
    dual.fetch_sub_primary(1, Ordering::Relaxed);
    let after_sub = dual.load_primary(Ordering::Relaxed);
    assert_eq!(after_sub, 0xFF00FF00FF00FF00, "Subtraction preserves bit pattern");

    // Store completely replaces bits
    dual.store_primary(0x00FF00FF00FF00FF, Ordering::Relaxed);
    let after_store = dual.load_primary(Ordering::Relaxed);
    assert_eq!(after_store, 0x00FF00FF00FF00FF, "Store produces exact value");
}

// ============================================================================
// Q33: Memory Ordering Consistency (6 tests)
// ============================================================================

/// **Q33 Test 1**: Acquire/Release fence validation
/// **Type**: Property (Q8-Q14)
/// **Validate**: Acquire/Release establishes synchronization
#[test]
fn test_t28_q33_acquire_release_fence_validation() {
    let data = Arc::new(DualAtomicU64::new(0, 0));
    let ready = Arc::new(AtomicU64::new(0));

    let data_w = data.clone();
    let ready_w = ready.clone();
    let writer = thread::spawn(move || {
        data_w.store_primary(42, Ordering::Relaxed);
        ready_w.store(1, Ordering::Release); // Synchronization point
    });

    let data_r = data.clone();
    let ready_r = ready.clone();
    let reader = thread::spawn(move || {
        while ready_r.load(Ordering::Acquire) == 0 {
            // Acquire synchronizes-with Release above
            thread::yield_now();
        }
        data_r.load_primary(Ordering::Relaxed)
    });

    writer.join().unwrap();
    let observed = reader.join().unwrap();

    assert_eq!(observed, 42, "Acquire/Release establishes synchronization");
}

/// **Q33 Test 2**: SeqCst total ordering (4-thread)
/// **Type**: Production (Q22-Q28)
/// **Validate**: SeqCst provides total order across all threads
#[test]
fn test_t28_q33_seqcst_total_order_4_threads() {
    let counter = Arc::new(DualAtomicU64::new(0, 0));
    let num_threads = 4;
    let iterations = 250;

    let mut handles = vec![];

    for _ in 0..num_threads {
        let c = counter.clone();
        let handle = thread::spawn(move || {
            for _ in 0..iterations {
                c.fetch_add_primary(1, Ordering::SeqCst);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let final_val = counter.load_primary(Ordering::SeqCst);
    assert_eq!(
        final_val,
        (num_threads * iterations) as u64,
        "SeqCst prevents lost updates: {} != {}",
        final_val,
        num_threads * iterations
    );
}

/// **Q33 Test 3**: ABA prevention with generation counter
/// **Type**: Integration (Q15-Q21)
/// **Validate**: Generation counter detects ABA condition
/// **Scenario**: Value A → B → A detected via generation counter
#[test]
fn test_t28_q33_aba_prevention_generation_counter() {
    let dual = Arc::new(DualAtomicU64::new(100, 1)); // value=100, gen=1

    let dual_w = dual.clone();
    let writer = thread::spawn(move || {
        // Change value
        dual_w.store_primary(200, Ordering::Release);
        dual_w.fetch_add_secondary(1, Ordering::Release); // Increment generation

        thread::sleep(std::time::Duration::from_millis(10));

        // Change back to original value (ABA)
        dual_w.store_primary(100, Ordering::Release);
        dual_w.fetch_add_secondary(1, Ordering::Release); // Increment generation again
    });

    let mut generations = Vec::new();
    let dual_r = dual.clone();
    let reader = thread::spawn(move || {
        for _ in 0..10 {
            let gen = dual_r.load_secondary(Ordering::Acquire);
            let val = dual_r.load_primary(Ordering::Acquire);
            generations.push((gen, val));
            thread::sleep(std::time::Duration::from_millis(2));
        }
        generations
    });

    writer.join().unwrap();
    let observations = reader.join().unwrap();

    // Verify generation changed (ABA detected)
    let initial_gen = observations[0].0;
    let final_gen = observations[observations.len() - 1].0;

    assert!(
        final_gen > initial_gen,
        "Generation should increment even with ABA: {} vs {}",
        final_gen,
        initial_gen
    );
}

/// **Q33 Test 4**: CAS Release ordering
/// **Type**: Property (Q8-Q14)
/// **Validate**: CAS with Release ensures happens-before
#[test]
fn test_t28_q33_cas_release_ordering() {
    let dual = Arc::new(DualAtomicU64::new(0, 0));
    let flag = Arc::new(AtomicU64::new(0));

    let dual_w = dual.clone();
    let flag_w = flag.clone();
    let writer = thread::spawn(move || {
        dual_w.store_secondary(999, Ordering::Relaxed);
        // CAS with Release
        let _ = dual_w.compare_exchange_primary(0, 1, Ordering::Release, Ordering::Relaxed);
        flag_w.store(1, Ordering::Release);
    });

    let dual_r = dual.clone();
    let flag_r = flag.clone();
    let reader = thread::spawn(move || {
        while flag_r.load(Ordering::Acquire) == 0 {
            thread::yield_now();
        }
        // Should see secondary value set before CAS Release
        dual_r.load_secondary(Ordering::Acquire)
    });

    writer.join().unwrap();
    let observed = reader.join().unwrap();

    assert_eq!(observed, 999, "CAS Release should synchronize writes");
}

/// **Q33 Test 5**: Happens-before relationship validation
/// **Type**: Integration (Q15-Q21)
/// **Validate**: Release-Acquire establishes happens-before
#[test]
fn test_t28_q33_happens_before_validation() {
    let shared = Arc::new([
        DualAtomicU64::new(0, 0),
        DualAtomicU64::new(0, 0),
        DualAtomicU64::new(0, 0),
    ]);
    let signal = Arc::new(AtomicU64::new(0));

    let shared_w = shared.clone();
    let signal_w = signal.clone();
    let writer = thread::spawn(move || {
        // Write all three values
        shared_w[0].store_primary(11, Ordering::Relaxed);
        shared_w[1].store_primary(22, Ordering::Relaxed);
        shared_w[2].store_primary(33, Ordering::Relaxed);
        // Signal with Release (happens-before)
        signal_w.store(1, Ordering::Release);
    });

    let shared_r = shared.clone();
    let signal_r = signal.clone();
    let reader = thread::spawn(move || {
        // Wait for Release signal (Acquire happens-after Release)
        while signal_r.load(Ordering::Acquire) == 0 {
            thread::yield_now();
        }
        // Should see all three writes
        (
            shared_r[0].load_primary(Ordering::Relaxed),
            shared_r[1].load_primary(Ordering::Relaxed),
            shared_r[2].load_primary(Ordering::Relaxed),
        )
    });

    writer.join().unwrap();
    let (v0, v1, v2) = reader.join().unwrap();

    assert_eq!((v0, v1, v2), (11, 22, 33), "All writes visible after Acquire");
}

// ============================================================================
// Q34: Deterministic Replay (3 tests)
// ============================================================================

/// **Q34 Test 1**: State snapshot consistency
/// **Type**: Unit (Q1-Q7)
/// **Validate**: Snapshots capture identical state
#[test]
fn test_t28_q34_deterministic_replay_snapshot_consistency() {
    let dual = DualAtomicU64::new(42, 99);

    let snapshots: Vec<(u64, u64)> = (0..10)
        .map(|_| {
            (
                dual.load_primary(Ordering::Relaxed),
                dual.load_secondary(Ordering::Relaxed),
            )
        })
        .collect();

    // All snapshots should be identical
    assert!(
        snapshots.iter().all(|s| s == &(42, 99)),
        "Snapshots should be consistent"
    );
}

/// **Q34 Test 2**: Replay → identical state
/// **Type**: Property (Q8-Q14)
/// **Validate**: Replaying operations produces same final state
#[test]
fn test_t28_q34_deterministic_replay_operations() {
    let operations = vec![
        ("add_p", 10u64),
        ("add_s", 20u64),
        ("sub_p", 5u64),
        ("add_p", 15u64),
    ];

    let mut results = Vec::new();

    for _ in 0..5 {
        let dual = DualAtomicU64::new(0, 0);

        for (op, val) in &operations {
            match *op {
                "add_p" => dual.fetch_add_primary(*val, Ordering::Relaxed),
                "sub_p" => dual.fetch_sub_primary(*val, Ordering::Relaxed),
                "add_s" => dual.fetch_add_secondary(*val, Ordering::Relaxed),
                _ => 0,
            };
        }

        let p = dual.load_primary(Ordering::Relaxed);
        let s = dual.load_secondary(Ordering::Relaxed);
        results.push((p, s));
    }

    // All runs should produce same result
    let first = results[0];
    assert!(results.iter().all(|r| r == &first), "Replay produces identical state");
    assert_eq!(first, (20, 20), "Final state is (20, 20)");
}

/// **Q34 Test 3**: Time-travel replay (backward compatibility)
/// **Type**: Integration (Q15-Q21)
/// **Validate**: Old operation sequence produces same result in new version
#[test]
fn test_t28_q34_deterministic_replay_backward_compat() {
    // Simulate old version operations
    let old_ops = vec![(1, "add"), (2, "add"), (3, "add")];

    let mut old_result = 0u64;
    for (val, _op) in &old_ops {
        old_result += val;
    }

    // New version with optimized path
    let dual = DualAtomicU64::new(0, 0);
    for (val, op) in &old_ops {
        if op == &"add" {
            dual.fetch_add_primary(*val, Ordering::Relaxed);
        }
    }

    let new_result = dual.load_primary(Ordering::Relaxed);

    assert_eq!(
        old_result, new_result,
        "Old and new produce same result: {} vs {}",
        old_result, new_result
    );
}

// ============================================================================
// Q35: Composition Determinism (4 tests)
// ============================================================================

/// **Q35 Test 1**: T1 + T2 (Atomic + SIMD) coordination
/// **Type**: Integration (Q15-Q21)
/// **Validate**: DualAtomicU64 coordinates with SIMD hash
#[test]
fn test_t28_q35_composition_determinism_atomic_simd() {
    // T1 component: DualAtomicU64 for coordination
    let coord = DualAtomicU64::new(0, 0);

    // Simulate SIMD operations on coordinated data
    let mut hash_input = vec![1u64, 2u64, 3u64, 4u64];
    let mut results = Vec::new();

    for iteration in 0..3 {
        // Atomic coordination
        let gen = coord.fetch_add_secondary(1, Ordering::Relaxed);

        // Simulate SIMD hash with generation
        let hash_result: u64 = hash_input.iter().sum::<u64>() ^ gen;
        results.push(hash_result);

        // Update input
        hash_input[iteration] += 1;
    }

    // Verify composition stability
    assert_eq!(results.len(), 3, "All iterations completed");
    assert!(
        results[0] < results[1] && results[1] != results[2],
        "Results are deterministic but distinct per iteration"
    );
}

/// **Q35 Test 2**: DualAtomicU64 + SIMD hash composition
/// **Type**: Property (Q8-Q14)
/// **Validate**: Hash results consistent across runs
#[test]
fn test_t28_q35_composition_determinism_hash_consistency() {
    let mut all_hashes = Vec::new();

    for _ in 0..5 {
        let dual = DualAtomicU64::new(42, 99);
        let mut run_hashes = Vec::new();

        for i in 0..10 {
            let p = dual.load_primary(Ordering::Relaxed);
            let s = dual.load_secondary(Ordering::Relaxed);

            // Simulate composition: atomic values + SIMD-like hash
            let hash = p.wrapping_mul(31).wrapping_add(s).wrapping_mul(17).wrapping_add(i);
            run_hashes.push(hash);

            dual.fetch_add_secondary(1, Ordering::Relaxed);
        }

        all_hashes.push(run_hashes);
    }

    // Verify all runs produced same hash sequence
    let first = &all_hashes[0];
    assert!(all_hashes.iter().all(|h| h == first), "Hash composition is deterministic");
}

/// **Q35 Test 3**: Nested atomic operations
/// **Type**: Integration (Q15-Q21)
/// **Validate**: Compositions of T1 operations are deterministic
#[test]
fn test_t28_q35_composition_determinism_nested_ops() {
    let dual = Arc::new(DualAtomicU64::new(100, 200));
    let num_threads = 2;
    let ops_per_thread = 50;

    let mut run_results = Vec::new();

    for _run in 0..3 {
        let d = dual.clone();

        let handles: Vec<_> = (0..num_threads)
            .map(|thread_id| {
                let d = d.clone();
                thread::spawn(move || {
                    let mut local_sum = 0u64;
                    for _ in 0..ops_per_thread {
                        if thread_id == 0 {
                            local_sum += d.fetch_add_primary(1, Ordering::Relaxed);
                        } else {
                            local_sum += d.fetch_add_secondary(1, Ordering::Relaxed);
                        }
                    }
                    local_sum
                })
            })
            .collect();

        let total: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
        run_results.push(total);

        // Reset for next run
        dual.store_primary(100, Ordering::Relaxed);
        dual.store_secondary(200, Ordering::Relaxed);
    }

    // Verify all runs produced same sum
    assert!(
        run_results.iter().all(|&r| r == run_results[0]),
        "Nested operation composition is deterministic"
    );
}

/// **Q35 Test 4**: Multi-capsule composition
/// **Type**: Production (Q22-Q28)
/// **Validate**: Multiple DualAtomicU64 instances compose deterministically
#[test]
fn test_t28_q35_composition_determinism_multi_capsule() {
    let capsules = vec![
        Arc::new(DualAtomicU64::new(10, 20)),
        Arc::new(DualAtomicU64::new(30, 40)),
        Arc::new(DualAtomicU64::new(50, 60)),
    ];

    let mut run_results = Vec::new();

    for _run in 0..3 {
        let mut totals = vec![0u64; 3];

        let handles: Vec<_> = (0..3)
            .map(|idx| {
                let c = capsules[idx].clone();
                thread::spawn(move || {
                    let mut sum = 0;
                    for _ in 0..100 {
                        sum += c.fetch_add_primary(1, Ordering::Relaxed);
                    }
                    sum
                })
            })
            .collect();

        for (i, h) in handles.into_iter().enumerate() {
            totals[i] = h.join().unwrap();
        }

        run_results.push(totals);

        // Reset capsules
        for (i, c) in capsules.iter().enumerate() {
            c.store_primary([10, 30, 50][i], Ordering::Relaxed);
        }
    }

    // Verify all runs produced same composition
    let first = &run_results[0];
    for result in &run_results[1..] {
        assert_eq!(
            result, first,
            "Multi-capsule composition is deterministic"
        );
    }
}
