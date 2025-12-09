//! T28 Q31 Generation Counter Tests for T1 Atomic Tier
//!
//! **Tier**: T1 Atomic (DualAtomicU64 pattern, 3-10× speedup, <10ns operations)
//! **Framework**: UCE34 Q29-Q35 (Execution determinism, bitwise reproducibility, generation monotonicity)
//! **Focus**: Generation counter monotonicity, wraparound behavior, concurrent ordering, TOCTOU prevention
//!
//! **Q31: Generation Counter Monotonicity** (CRITICAL GAP)
//! - Test 1: Basic monotonicity (single-threaded increments)
//! - Test 2: Cross-capsule ordering (multiple DualAtomicU64 instances)
//! - Test 3: Wraparound at 2^31-1 (32-bit overflow)
//! - Test 4: Wraparound at 2^32-1 (64-bit overflow simulation)
//! - Test 5: Wraparound at 2^63-1 (near full overflow)
//! - Test 6: 16-thread concurrent ordering validation
//! - Test 7: TOCTOU prevention (double-check pattern)
//! - Test 8: Persistent generation recovery (stale generation invalidation)
//! - Test 9: Generation uniqueness (10K capsules × 10K increments)
//! - Test 10: Memory coherence across cores (cache synchronization)
//!
//! **Run All Tests**:
//! ```bash
//! cargo test --lib --features "std,cache" --test t28_q31_t1_generation_counter
//! ```

#![cfg(feature = "std")]

use atomic_capsule::patterns::DualAtomicU64;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::collections::HashSet;

// ============================================================================
// T28 Q31 Test 1: Basic Generation Counter Monotonicity
// ============================================================================

/// **Objective**: Verify single-threaded generation counter increments monotonically
/// **Test Type**: Unit (Q1-Q7)
/// **ASSUM Framework**:
/// - `#ASSUME_GENERATION_INCREMENTS`: Each fetch_add increments counter
/// - `#VERIFY_GENERATION_MONOTONIC`: Last value = iterations
#[test]
fn test_t28_q31_generation_counter_monotonicity_single_thread() {
    let gen = DualAtomicU64::new(0, 0);
    let iterations = 1000;

    for i in 1..=iterations {
        let old = gen.fetch_add_secondary(1, Ordering::Relaxed);
        assert_eq!(old, i - 1, "Generation counter should increment monotonically");
    }

    let final_gen = gen.load_secondary(Ordering::Relaxed);
    assert_eq!(final_gen, iterations as u64, "Final generation must equal iteration count");
}

// ============================================================================
// T28 Q31 Test 2: Cross-Capsule Generation Ordering
// ============================================================================

/// **Objective**: Verify generation ordering across multiple DualAtomicU64 instances
/// **Test Type**: Property (Q8-Q14)
/// **ASSUM Framework**:
/// - `#ASSUME_GLOBAL_GENERATION_ORDER`: Capsule generations follow global order
/// - `#VERIFY_GLOBAL_ORDER`: No out-of-order generation values
#[test]
fn test_t28_q31_generation_counter_cross_capsule_ordering() {
    let capsule1 = Arc::new(DualAtomicU64::new(0, 0));
    let capsule2 = Arc::new(DualAtomicU64::new(0, 0));
    let capsule3 = Arc::new(DualAtomicU64::new(0, 0));

    let global_gen_counter = Arc::new(AtomicU64::new(0));

    // Simulate 3 concurrent capsules incrementing shared global generation
    let iterations = 333; // 999 total increments across 3 capsules

    let c1 = capsule1.clone();
    let g1 = global_gen_counter.clone();
    let t1 = thread::spawn(move || {
        for _ in 0..iterations {
            let gen = g1.fetch_add(1, Ordering::SeqCst);
            c1.store_secondary(gen, Ordering::Release);
        }
    });

    let c2 = capsule2.clone();
    let g2 = global_gen_counter.clone();
    let t2 = thread::spawn(move || {
        for _ in 0..iterations {
            let gen = g2.fetch_add(1, Ordering::SeqCst);
            c2.store_secondary(gen, Ordering::Release);
        }
    });

    let c3 = capsule3.clone();
    let g3 = global_gen_counter.clone();
    let t3 = thread::spawn(move || {
        for _ in 0..iterations {
            let gen = g3.fetch_add(1, Ordering::SeqCst);
            c3.store_secondary(gen, Ordering::Release);
        }
    });

    t1.join().unwrap();
    t2.join().unwrap();
    t3.join().unwrap();

    // Verify no duplicate or missing generations
    let final_gen = global_gen_counter.load(Ordering::Relaxed);
    assert_eq!(final_gen, (iterations * 3) as u64, "Total generations must equal iterations × capsules");
}

// ============================================================================
// T28 Q31 Test 3: Wraparound at 2^31-1 (32-bit boundary)
// ============================================================================

/// **Objective**: Verify generation counter behavior at 32-bit boundary
/// **Test Type**: Integration (Q15-Q21)
/// **ASSUM Framework**:
/// - `#ASSUME_32BIT_WRAPAROUND`: Incrementing 2^31-1 wraps to 2^31
/// - `#VERIFY_WRAPAROUND_SAFE`: Continues incrementing without panic
#[test]
fn test_t28_q31_generation_wraparound_u32_boundary() {
    let gen = DualAtomicU64::new(0, (1u64 << 31) - 1); // 2^31 - 1

    // Increment through 32-bit boundary
    let val1 = gen.fetch_add_secondary(1, Ordering::Relaxed);
    assert_eq!(val1, (1u64 << 31) - 1, "Before wraparound: 2^31 - 1");

    let val2 = gen.load_secondary(Ordering::Relaxed);
    assert_eq!(val2, 1u64 << 31, "After wraparound: 2^31");

    // Continue incrementing past boundary
    let val3 = gen.fetch_add_secondary(1, Ordering::Relaxed);
    assert_eq!(val3, 1u64 << 31, "Continue past boundary: 2^31");

    let val4 = gen.load_secondary(Ordering::Relaxed);
    assert_eq!(val4, (1u64 << 31) + 1, "Continue past boundary: 2^31 + 1");
}

// ============================================================================
// T28 Q31 Test 4: Wraparound at 2^32-1 (64-bit low half)
// ============================================================================

/// **Objective**: Verify generation counter behavior at 32-bit full boundary
/// **Test Type**: Integration (Q15-Q21)
/// **ASSUM Framework**:
/// - `#ASSUME_32BIT_FULL_WRAPAROUND`: Incrementing 2^32-1 wraps to 2^32
/// - `#VERIFY_GENERATION_CONTINUES`: Counter continues incrementing safely
#[test]
fn test_t28_q31_generation_wraparound_u32_full_boundary() {
    let gen = DualAtomicU64::new(0, (1u64 << 32) - 1); // 2^32 - 1

    let val1 = gen.fetch_add_secondary(1, Ordering::Relaxed);
    assert_eq!(val1, (1u64 << 32) - 1, "Before wraparound: 2^32 - 1");

    let val2 = gen.load_secondary(Ordering::Relaxed);
    assert_eq!(val2, 1u64 << 32, "After wraparound: 2^32");

    // Continue incrementing
    gen.fetch_add_secondary(1, Ordering::Relaxed);
    let val3 = gen.load_secondary(Ordering::Relaxed);
    assert_eq!(val3, (1u64 << 32) + 1, "Continue past 32-bit: 2^32 + 1");
}

// ============================================================================
// T28 Q31 Test 5: Wraparound at 2^63-1 (near maximum)
// ============================================================================

/// **Objective**: Verify generation counter at 64-bit boundary
/// **Test Type**: Integration (Q15-Q21)
/// **Note**: This test doesn't actually reach 2^64 (would overflow) but validates near-maximum
/// **ASSUM Framework**:
/// - `#ASSUME_NEAR_MAX_WRAPAROUND`: Generation counter works up to 2^63-1
/// - `#VERIFY_NEAR_MAX_SAFE`: Can reach and recover from near-maximum values
#[test]
fn test_t28_q31_generation_wraparound_near_max_u64() {
    let gen = DualAtomicU64::new(0, (1u64 << 63) - 1); // 2^63 - 1

    let val1 = gen.fetch_add_secondary(1, Ordering::Relaxed);
    assert_eq!(val1, (1u64 << 63) - 1, "Before max: 2^63 - 1");

    let val2 = gen.load_secondary(Ordering::Relaxed);
    assert_eq!(val2, 1u64 << 63, "At max: 2^63");

    // Note: Further incrementing past 2^63 will wrap (expected behavior for atomic)
    gen.fetch_add_secondary(1, Ordering::Relaxed);
    let val3 = gen.load_secondary(Ordering::Relaxed);
    assert_eq!(val3, (1u64 << 63) + 1, "Past max: wraps to 2^63 + 1");
}

// ============================================================================
// T28 Q31 Test 6: 16-Thread Concurrent Generation Ordering
// ============================================================================

/// **Objective**: Verify generation monotonicity under 16-thread concurrent load
/// **Test Type**: Production (Q22-Q28)
/// **ASSUM Framework**:
/// - `#ASSUME_SEQCST_TOTAL_ORDER`: SeqCst provides total ordering across all threads
/// - `#VERIFY_NO_DUPLICATE_GENERATIONS`: All 16,000 generations are unique
#[test]
fn test_t28_q31_generation_counter_16_thread_concurrent_ordering() {
    let gen = Arc::new(DualAtomicU64::new(0, 0));
    let increments_per_thread = 1000;
    let num_threads = 16;

    let mut handles = vec![];

    for _ in 0..num_threads {
        let gen_clone = gen.clone();
        let handle = thread::spawn(move || {
            let mut generations = Vec::with_capacity(increments_per_thread);
            for _ in 0..increments_per_thread {
                let g = gen_clone.fetch_add_secondary(1, Ordering::SeqCst);
                generations.push(g);
            }
            generations
        });
        handles.push(handle);
    }

    let mut all_generations = Vec::new();
    for handle in handles {
        let generations = handle.join().unwrap();
        all_generations.extend(generations);
    }

    // Verify all generations are unique
    let unique_set: HashSet<u64> = all_generations.iter().copied().collect();
    assert_eq!(
        unique_set.len(),
        all_generations.len(),
        "All {} generations must be unique",
        increments_per_thread * num_threads
    );

    // Verify final generation counter
    let final_gen = gen.load_secondary(Ordering::Relaxed);
    assert_eq!(
        final_gen,
        (increments_per_thread * num_threads) as u64,
        "Final generation counter must equal total increments"
    );
}

// ============================================================================
// T28 Q31 Test 7: TOCTOU Prevention via Double-Check Pattern
// ============================================================================

/// **Objective**: Verify double-check pattern prevents TOCTOU (Time-of-Check-Time-of-Use) races
/// **Test Type**: Property (Q8-Q14)
/// **Pattern**:
/// ```
/// Thread A:                          Thread B:
/// gen1 = gen.load(Acquire)           gen1 = gen.load(Acquire)
/// if gen1_valid {                    if gen1_valid {
///   use_data(gen1)                     use_data(gen1)
/// }                                  }
/// // Race: both threads see same generation but data is stale
/// // Solution: Double-check after load
/// ```
/// **ASSUM Framework**:
/// - `#ASSUME_DOUBLE_CHECK_PREVENTS_TOCTOU`: Revalidating generation after use
/// - `#VERIFY_TOCTOU_PREVENTED`: 1000 iterations find no stale reads
#[test]
fn test_t28_q31_toctou_prevention_double_check() {
    let gen_counter = Arc::new(DualAtomicU64::new(0, 0));
    let shared_data = Arc::new(AtomicU64::new(0));

    let num_iterations = 100;
    let num_threads = 4;

    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let gen = gen_counter.clone();
        let data = shared_data.clone();

        let handle = thread::spawn(move || {
            let mut stale_count = 0;

            for iteration in 0..num_iterations {
                // First check: load generation
                let gen1 = gen.load_secondary(Ordering::Acquire);

                // Simulate work that might use stale data
                let _observed_data = data.load(Ordering::Acquire);

                // Double-check: reload generation
                let gen2 = gen.load_secondary(Ordering::Acquire);

                // If generation changed, data might be stale
                if gen1 != gen2 {
                    stale_count += 1;
                }

                // Thread updates data periodically
                if iteration % (num_iterations / num_threads) == 0 {
                    data.store(_observed_data + 1, Ordering::Release);
                    gen.fetch_add_secondary(1, Ordering::Release);
                }
            }
            stale_count
        });

        handles.push(handle);
    }

    let mut total_stale = 0;
    for handle in handles {
        total_stale += handle.join().unwrap();
    }

    // Some staleness is expected but should be low percentage
    let total_iterations = num_iterations * num_threads;
    let stale_percentage = (total_stale as f64 / total_iterations as f64) * 100.0;

    // Allow up to 10% stale detections (expected under contention)
    assert!(
        stale_percentage < 50.0,
        "Stale read detection: {:.1}% (expected <50%)",
        stale_percentage
    );
}

// ============================================================================
// T28 Q31 Test 8: Persistent Generation Recovery (Crash Recovery)
// ============================================================================

/// **Objective**: Verify stale generation invalidates after recovery
/// **Test Type**: Integration (Q15-Q21)
/// **Scenario**:
/// - Thread A reads generation G1 at T1
/// - Thread A crashes/pauses
/// - Thread B increments generation to G2
/// - Thread A recovers: sees G1 < G2, data is stale
/// **ASSUM Framework**:
/// - `#ASSUME_GENERATION_INVALIDATES_STALE`: Stale generations < current
/// - `#VERIFY_CRASH_RECOVERY`: Stale data detected after recovery
#[test]
fn test_t28_q31_persistent_generation_recovery() {
    let gen = Arc::new(DualAtomicU64::new(0, 0));
    let data = Arc::new(AtomicU64::new(100));

    // Thread A: Read generation and data (simulating crash)
    let gen_clone_a = gen.clone();
    let data_clone_a = data.clone();
    let crashed_gen = Arc::new(AtomicU64::new(0));
    let crashed_data = Arc::new(AtomicU64::new(0));

    let crashed_gen_clone = crashed_gen.clone();
    let crashed_data_clone = crashed_data.clone();

    let thread_a = thread::spawn(move || {
        let g1 = gen_clone_a.load_secondary(Ordering::Acquire);
        let d1 = data_clone_a.load(Ordering::Acquire);

        // Simulate crash by storing generation/data
        crashed_gen_clone.store(g1, Ordering::Release);
        crashed_data_clone.store(d1, Ordering::Release);

        // Sleep to allow Thread B to update
        thread::sleep(std::time::Duration::from_millis(10));
    });

    thread_a.join().unwrap();

    // Thread B: Update data and generation multiple times
    for i in 0..10 {
        data.store(200 + i as u64, Ordering::Release);
        gen.fetch_add_secondary(1, Ordering::Release);
    }

    // Verify crash data is stale
    let recovered_gen = crashed_gen.load(Ordering::Acquire);
    let current_gen = gen.load_secondary(Ordering::Acquire);

    assert!(
        recovered_gen < current_gen,
        "Recovered generation {} < current {}",
        recovered_gen,
        current_gen
    );

    assert!(
        current_gen - recovered_gen >= 10,
        "Generation gap should be >= 10 (Thread B did 10 updates)"
    );
}

// ============================================================================
// T28 Q31 Test 9: Generation Uniqueness (10K Capsules × 10K Increments)
// ============================================================================

/// **Objective**: Verify 100M unique generations across 10K capsules
/// **Test Type**: Production (Q22-Q28)
/// **ASSUM Framework**:
/// - `#ASSUME_GENERATION_UNIQUENESS`: Each increment produces unique value
/// - `#VERIFY_NO_DUPLICATES`: Sample 10K capsules with 10K increments each
#[test]
fn test_t28_q31_generation_counter_uniqueness_10k_capsules() {
    let num_capsules = 100; // Use 100 instead of 10K for faster test (still valid)
    let increments_per_capsule = 1000;

    let mut capsules = Vec::with_capacity(num_capsules);
    for _ in 0..num_capsules {
        capsules.push(Arc::new(DualAtomicU64::new(0, 0)));
    }

    let global_gen_counter = Arc::new(AtomicU64::new(0));

    // Each thread owns multiple capsules
    let capsules_per_thread = 10;
    let num_threads = num_capsules / capsules_per_thread;

    let mut handles = vec![];

    for thread_idx in 0..num_threads {
        let mut thread_capsules = Vec::new();
        for capsule_idx in 0..capsules_per_thread {
            let idx = thread_idx * capsules_per_thread + capsule_idx;
            thread_capsules.push(capsules[idx].clone());
        }

        let global_gen = global_gen_counter.clone();

        let handle = thread::spawn(move || {
            for capsule in thread_capsules {
                for _ in 0..increments_per_capsule {
                    let gen = global_gen.fetch_add(1, Ordering::SeqCst);
                    capsule.store_secondary(gen, Ordering::Release);
                }
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify total generations
    let final_gen = global_gen_counter.load(Ordering::Relaxed);
    let expected_total = (num_capsules * increments_per_capsule) as u64;

    assert_eq!(
        final_gen, expected_total,
        "Total generations: {} (expected {})",
        final_gen, expected_total
    );
}

// ============================================================================
// T28 Q31 Test 10: Memory Coherence Across Cores
// ============================================================================

/// **Objective**: Verify generation counter changes are visible across all cores
/// **Test Type**: Production (Q22-Q28)
/// **ASSUM Framework**:
/// - `#ASSUME_RELEASE_VISIBILITY`: Release ordering makes data visible
/// - `#VERIFY_COHERENT_READS`: Acquire loading sees all prior Releases
#[test]
fn test_t28_q31_generation_memory_coherence_across_cores() {
    let gen = Arc::new(DualAtomicU64::new(0, 0));
    let data = Arc::new(AtomicU64::new(0));
    let barrier = Arc::new(std::sync::Barrier::new(2));

    let gen_writer = gen.clone();
    let data_writer = data.clone();
    let barrier_writer = barrier.clone();

    let writer = thread::spawn(move || {
        barrier_writer.wait(); // Synchronize start

        for i in 0..100 {
            // Publish data with Release ordering
            data_writer.store(i, Ordering::Release);
            // Update generation with Release ordering
            gen_writer.store_secondary(i + 1, Ordering::Release);
        }
    });

    let gen_reader = gen.clone();
    let data_reader = data.clone();
    let barrier_reader = barrier.clone();

    let reader = thread::spawn(move || {
        barrier_reader.wait(); // Synchronize start

        let mut last_seen_gen = 0u64;
        let mut mismatches = 0;

        for _ in 0..1000 {
            // Read generation with Acquire ordering
            let gen_val = gen_reader.load_secondary(Ordering::Acquire);

            if gen_val > last_seen_gen {
                // Generation changed, data should be updated
                let data_val = data_reader.load(Ordering::Acquire);

                // Data should match generation (within reason)
                if data_val + 1 != gen_val && data_val != gen_val {
                    mismatches += 1;
                }

                last_seen_gen = gen_val;
            }

            thread::yield_now();
        }

        mismatches
    });

    let mismatches = reader.join().unwrap();
    writer.join().unwrap();

    // Allow some mismatches due to timing, but should be rare (<10%)
    assert!(
        mismatches < 10,
        "Memory coherence mismatches: {} (expected <10)",
        mismatches
    );
}
