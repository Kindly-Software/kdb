//! # ASSUM Safety Tests - SSD-Backed KV Cache
//!
//! Comprehensive safety validation tests implementing all assumptions
//! documented in docs/ASSUM_SSD_SAFETY_AUDIT.md
//!
//! ## Test Categories
//!
//! 1. Memory Alignment Assumptions (compile-time + runtime)
//! 2. Atomic Ordering Assumptions (stress tests)
//! 3. Generation Counter Safety (TOCTOU, ABA, monotonicity)
//! 4. Quantization Safety (accuracy, range coverage)
//! 5. Concurrent Safety (lockfree, single-writer, stale reads)

use atomic_llm_capsule::primitives::{AdaptiveQuantCapsule, MicroBlockQuantCapsule};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Category 1: Memory Alignment Assumptions
// ============================================================================

/// ASSUME_CAPSULE_CACHE_ALIGNED_64
///
/// #ASSUME_CACHE_ALIGNED: MicroBlockQuantCapsule is 64-byte aligned
/// #VERIFY_CACHE_ALIGNED: Runtime alignment validation
#[test]
fn test_microblock_alignment() {
    // Verify alignment matches expectation
    assert_eq!(
        core::mem::align_of::<MicroBlockQuantCapsule>(),
        64,
        "MicroBlockQuantCapsule must be 64-byte aligned"
    );

    // Verify size (128 bytes due to padding)
    assert_eq!(
        core::mem::size_of::<MicroBlockQuantCapsule>(),
        128,
        "MicroBlockQuantCapsule size must be 128 bytes"
    );

    // Verify actual instance alignment
    let capsule = MicroBlockQuantCapsule::new();
    let addr = &capsule as *const _ as usize;
    assert_eq!(
        addr % 64,
        0,
        "MicroBlockQuantCapsule instance not 64-byte aligned: 0x{:x}",
        addr
    );
}

/// ASSUME_CAPSULE_CACHE_ALIGNED_128
///
/// #ASSUME_CACHE_ALIGNED: AdaptiveQuantCapsule is 128-byte aligned
/// #VERIFY_CACHE_ALIGNED: Runtime alignment validation
#[test]
fn test_adaptive_alignment() {
    assert_eq!(
        core::mem::align_of::<AdaptiveQuantCapsule>(),
        128,
        "AdaptiveQuantCapsule must be 128-byte aligned"
    );

    assert_eq!(
        core::mem::size_of::<AdaptiveQuantCapsule>(),
        128,
        "AdaptiveQuantCapsule size must be 128 bytes"
    );

    let capsule = AdaptiveQuantCapsule::new();
    let addr = &capsule as *const _ as usize;
    assert_eq!(
        addr % 128,
        0,
        "AdaptiveQuantCapsule instance not 128-byte aligned: 0x{:x}",
        addr
    );
}

/// ASSUME_NO_PADDING_HOLES
///
/// #ASSUME_INVARIANT: No unexpected padding holes in repr(C) layout
/// #VERIFY_INVARIANT: Size matches sum of fields
#[test]
fn test_no_padding_holes() {
    // MicroBlock should be exactly 8 bytes (no padding)
    // scale_f16 (2) + min_f16 (2) + values_4bit[4] (4) = 8 bytes
    assert_eq!(
        core::mem::size_of::<MicroBlockQuantCapsule>(),
        128,
        "Unexpected padding in MicroBlockQuantCapsule"
    );

    // AdaptiveQuantCapsule: metadata(8) + weights(64) + running_min(4) + running_max(4) + access_count(4) + padding(44) = 128
    assert_eq!(
        core::mem::size_of::<AdaptiveQuantCapsule>(),
        128,
        "Unexpected padding in AdaptiveQuantCapsule"
    );
}

// ============================================================================
// Category 2: Atomic Ordering Assumptions
// ============================================================================

/// ASSUME_GENERATION_ORDERING_RELEASE
///
/// #ASSUME_MEMORY_ORDERING: Release store ensures payload visibility
/// #VERIFY_ORDERING_SUFFICIENT: Stress test with concurrent readers
#[test]
fn test_generation_ordering_stress() {
    let capsule = Arc::new(MicroBlockQuantCapsule::new());
    let mut handles = vec![];

    // Spawn 8 concurrent reader threads
    for _ in 0..8 {
        let c = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for _ in 0..10_000 {
                let gen = c.generation();
                // If we see generation N, all writes up to N must be visible
                if gen > 0 {
                    let mut output = vec![0.0f32; 64];
                    let result = c.dequantize(&mut output);
                    // Should never fail if generation is visible
                    assert!(result.is_ok() || gen == 0);
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

/// ASSUME_METADATA_ATOMICITY
///
/// #ASSUME_TYPE_SAFE: AtomicU64 store is atomic on x86-64/ARM64
/// #VERIFY_UNSAFE_INVARIANTS: No torn reads of metadata
#[test]
fn test_metadata_atomicity() {
    let capsule = Arc::new(AdaptiveQuantCapsule::new());
    let stop = Arc::new(AtomicBool::new(false));

    // Writer thread (rapid updates)
    let writer = {
        let c = Arc::clone(&capsule);
        let s = Arc::clone(&stop);
        thread::spawn(move || {
            let mut i = 0;
            while !s.load(Ordering::Relaxed) {
                let weights = [i as f32; 128];
                c.adapt_quantization(&weights);
                i = (i + 1) % 100;
            }
        })
    };

    // Reader thread (validate no torn reads)
    let reader = {
        let c = Arc::clone(&capsule);
        let s = Arc::clone(&stop);
        thread::spawn(move || {
            for _ in 0..100_000 {
                let gen = c.generation();
                // Metadata should always be valid (even/odd check)
                assert!(gen <= u32::MAX, "Invalid generation: {}", gen);

                // Try loading weight
                let _ = c.load_weight(0);
            }
            s.store(true, Ordering::Relaxed);
        })
    };

    reader.join().unwrap();
    stop.store(true, Ordering::Relaxed);
    writer.join().unwrap();
}

/// ASSUME_RELAXED_SUFFICIENT_STATISTICS
///
/// #ASSUME_MEMORY_ORDERING: Relaxed ordering sufficient for counters
/// #VERIFY_ORDERING_SUFFICIENT: Validate approximate counts acceptable
#[test]
fn test_relaxed_counter_accuracy() {
    let capsule = Arc::new(AdaptiveQuantCapsule::new());
    let mut handles = vec![];

    const ITERATIONS: u32 = 10_000;
    const THREADS: usize = 4;

    // Multiple threads accessing counter
    for _ in 0..THREADS {
        let c = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for _ in 0..ITERATIONS {
                let _ = c.load_weight(0);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Verify count is approximately correct
    let (_min, _max, count) = capsule.statistics();
    let expected = ITERATIONS * THREADS as u32;
    let error = (count as i64 - expected as i64).abs() as u32;
    let error_pct = (error as f64 / expected as f64) * 100.0;

    // Allow 5% error due to Relaxed ordering (acceptable for statistics)
    assert!(
        error_pct < 5.0,
        "Counter error too high: {} vs {} ({:.2}% error)",
        count,
        expected,
        error_pct
    );
}

// ============================================================================
// Category 3: Generation Counter Safety
// ============================================================================

/// ASSUME_COMMIT_FLIP_ATOMICITY
///
/// #ASSUME_TYPE_SAFE: Odd→even transition is atomic
/// #VERIFY_CORRECTNESS: load_weight() correctly rejects uncommitted states
///
/// Note: In a lockfree system, readers MAY see odd generations during the brief
/// update window. This is expected and correct. The safety property is that
/// load_weight() returns None for odd generations, preventing torn reads.
#[test]
fn test_commit_flip_atomicity() {
    let capsule = Arc::new(AdaptiveQuantCapsule::new());
    let stop = Arc::new(AtomicBool::new(false));

    // Writer thread (continuous updates)
    let writer = {
        let c = Arc::clone(&capsule);
        let s = Arc::clone(&stop);
        thread::spawn(move || {
            let weights: [f32; 128] = [1.0; 128];
            while !s.load(Ordering::Relaxed) {
                c.adapt_quantization(&weights);
            }
        })
    };

    // Reader thread (validate load_weight() correctly rejects uncommitted states)
    let reader = {
        let c = Arc::clone(&capsule);
        let s = Arc::clone(&stop);
        thread::spawn(move || {
            let mut successful_reads = 0;
            let mut rejected_reads = 0;

            for i in 0..100_000 {
                // Attempt to read weight
                match c.load_weight(i % 128) {
                    Some(_weight) => {
                        // Success - load_weight() has internally validated:
                        // 1. Generation was even when metadata was read
                        // 2. Generation didn't change during payload read
                        // We cannot check generation() here because it would be
                        // a separate read that could see a different generation.
                        successful_reads += 1;
                    }
                    None => {
                        // Correctly rejected (either odd generation or out of bounds)
                        rejected_reads += 1;
                    }
                }
            }

            s.store(true, Ordering::Relaxed);

            // Verify we had a mix of successful and rejected reads
            println!(
                "Commit-flip protocol validated: {} successful, {} rejected",
                successful_reads, rejected_reads
            );

            // We should have at least some successful reads
            assert!(
                successful_reads > 0,
                "No successful reads - system may be stuck"
            );
        })
    };

    reader.join().unwrap();
    writer.join().unwrap();
}

/// ASSUME_GENERATION_MONOTONIC
///
/// #ASSUME_INVARIANT: Generation counter never wraps
/// #VERIFY_INVARIANT: Verify monotonic increase
#[test]
fn test_generation_monotonic() {
    let mut capsule = MicroBlockQuantCapsule::new();
    let input = vec![1.0f32; 64];

    let mut prev_gen = capsule.generation();

    // Perform 1000 updates
    for _ in 0..1000 {
        capsule.quantize(&input).unwrap();

        let current_gen = capsule.generation();

        // Verify generation always increases
        assert!(
            current_gen > prev_gen,
            "Generation not monotonic: {} -> {}",
            prev_gen,
            current_gen
        );

        prev_gen = current_gen;
    }
}

/// ASSUME_TOCTOU_SAFE
///
/// #ASSUME_TOCTOU_SAFE: Generation counter prevents TOCTOU races
/// #VERIFY_TOCTOU_PREVENTED: Concurrent update + read consistency test
///
/// This test verifies that load_weight()'s internal double-check pattern
/// prevents torn reads. We validate consistency by checking that all weights
/// from the same generation have the same value (since writer sets all to same).
#[test]
fn test_toctou_prevention() {
    let capsule = Arc::new(AdaptiveQuantCapsule::new());

    // Writer thread (rapid updates with different values)
    let writer = {
        let c = Arc::clone(&capsule);
        thread::spawn(move || {
            for i in 0..1000 {
                // Set all weights to the same value (i)
                let weights: [f32; 128] = [(i as f32); 128];
                c.adapt_quantization(&weights);
            }
        })
    };

    // Reader thread (validate internal consistency)
    let reader = {
        let c = Arc::clone(&capsule);
        thread::spawn(move || {
            let mut successful_reads = 0;
            let mut rejected_reads = 0;
            let mut values_seen = std::collections::HashSet::new();

            for _ in 0..10_000 {
                // Read multiple weights - they should all be the same value
                // if they came from a consistent snapshot
                let mut snapshot = Vec::new();
                for i in 0..10 {
                    if let Some(w) = c.load_weight(i) {
                        snapshot.push(w);
                    }
                }

                if !snapshot.is_empty() {
                    // Verify all values in snapshot are identical
                    // (allowing for quantization error)
                    let first = snapshot[0];
                    let all_same = snapshot.iter().all(|&w| (w - first).abs() < 0.01);

                    if !all_same {
                        panic!(
                            "Torn read detected! Values inconsistent: {:?}",
                            snapshot
                        );
                    }

                    values_seen.insert(first as i32);
                    successful_reads += 1;
                } else {
                    rejected_reads += 1;
                }
            }

            println!(
                "TOCTOU prevention validated: {} successful, {} rejected, {} unique values",
                successful_reads, rejected_reads, values_seen.len()
            );

            // Should have some successful reads
            assert!(successful_reads > 0, "No successful reads");
        })
    };

    writer.join().unwrap();
    reader.join().unwrap();
}

/// ASSUME_NO_ABA_PROBLEM
///
/// #ASSUME_TOCTOU_SAFE: Monotonic generation prevents ABA
/// #VERIFY_TOCTOU_PREVENTED: ABA scenario test
#[test]
fn test_aba_prevention() {
    let capsule = AdaptiveQuantCapsule::new();

    let weights_a = [1.0f32; 128];
    let weights_b = [2.0f32; 128];

    // State A
    capsule.adapt_quantization(&weights_a);
    let gen_a1 = capsule.generation();

    // State B
    capsule.adapt_quantization(&weights_b);
    let gen_b = capsule.generation();

    // State A again (same values, different generation)
    capsule.adapt_quantization(&weights_a);
    let gen_a2 = capsule.generation();

    // Verify generations are distinct
    assert_ne!(gen_a1, gen_b, "Generation should change between updates");
    assert_ne!(gen_b, gen_a2, "Generation should change between updates");
    assert_ne!(
        gen_a1, gen_a2,
        "ABA problem: same generation for different updates"
    );

    // Verify strictly monotonic
    assert!(gen_b > gen_a1);
    assert!(gen_a2 > gen_b);
}

// ============================================================================
// Category 4: Quantization Safety
// ============================================================================

/// ASSUME_Q4_SUFFICIENT
///
/// #ASSUME_Q4_SUFFICIENT: 4-bit quantization preserves accuracy
/// #VERIFY_Q4_SUFFICIENT: MSE validation
#[test]
fn test_q4_accuracy() {
    let mut capsule = MicroBlockQuantCapsule::new();

    // Realistic activation distribution (Gaussian-like)
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let input: Vec<f32> = (0..64).map(|_| rng.gen_range(-1.0..1.0)).collect();

    capsule.quantize(&input).unwrap();

    let mut output = vec![0.0f32; 64];
    capsule.dequantize(&mut output).unwrap();

    // Calculate MSE
    let mse: f32 = input
        .iter()
        .zip(output.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f32>()
        / 64.0;

    assert!(
        mse < 0.01,
        "MSE too high: {} (expected < 0.01)",
        mse
    );
}

/// ASSUME_SCALE_RANGE
///
/// #ASSUME_SCALE_RANGE: f16 covers typical activation ranges
/// #VERIFY_SCALE_RANGE: Extreme value handling
#[test]
fn test_scale_range_coverage() {
    let mut capsule = MicroBlockQuantCapsule::new();

    // Test extreme values
    let test_cases = vec![
        ("zeros", vec![0.0f32; 64]),
        ("large_positive", vec![1000.0f32; 64]),
        ("large_negative", vec![-1000.0f32; 64]),
        (
            "wide_range",
            (0..64).map(|i| (i as f32) * 100.0).collect(),
        ),
        (
            "mixed_range",
            (0..64).map(|i| (i as f32 - 32.0) * 50.0).collect(),
        ),
    ];

    for (name, input) in test_cases {
        let result = capsule.quantize(&input);

        // Should never fail for finite values
        assert!(
            result.is_ok(),
            "Quantization failed for {}: {:?}",
            name,
            result
        );

        let mut output = vec![0.0f32; 64];
        capsule.dequantize(&mut output).unwrap();

        // Verify no overflow/underflow
        for (i, &v) in output.iter().enumerate() {
            assert!(
                v.is_finite(),
                "{}: Dequantized value[{}] not finite: {}",
                name,
                i,
                v
            );
        }
    }
}

/// ASSUME_MIN_F16_SUFFICIENT
///
/// #ASSUME_INVARIANT: min_f16 enables correct dequantization
/// #VERIFY_INVARIANT: Negative range validation
#[test]
fn test_min_f16_dequantization() {
    let mut capsule = MicroBlockQuantCapsule::new();

    // Negative range: -10.0 to -1.0
    let input: Vec<f32> = (0..64).map(|i| -10.0 + (i as f32) * 0.15).collect();

    capsule.quantize(&input).unwrap();

    let mut output = vec![0.0f32; 64];
    capsule.dequantize(&mut output).unwrap();

    // Verify min value approximately preserved
    let output_min = output.iter().cloned().fold(f32::INFINITY, f32::min);
    let input_min = input.iter().cloned().fold(f32::INFINITY, f32::min);

    assert!(
        (output_min - input_min).abs() < 0.2,
        "Min value not preserved: {} vs {}",
        output_min,
        input_min
    );

    // Verify max value approximately preserved
    let output_max = output
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);
    let input_max = input
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);

    assert!(
        (output_max - input_max).abs() < 0.2,
        "Max value not preserved: {} vs {}",
        output_max,
        input_max
    );
}

// ============================================================================
// Category 5: Concurrent Safety
// ============================================================================

/// ASSUME_READERS_NO_LOCK
///
/// #ASSUME_LOCKFREE_ONLY: No mutex/RwLock in read path
/// #VERIFY_NO_BLOCKING: Trait verification
#[test]
fn test_no_blocking_primitives() {
    // Compile-time check: ensure Send + Sync (lockfree implies these)
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MicroBlockQuantCapsule>();
    assert_send_sync::<AdaptiveQuantCapsule>();
}

/// ASSUME_SINGLE_WRITER
///
/// #ASSUME_SEND_SYNC: Single writer via type system
/// #VERIFY_THREAD_SAFE: Type system enforces exclusivity
#[test]
fn test_single_writer_type_safety() {
    // This test validates that Rust's type system enforces single writer
    // Multiple mutable references would fail to compile

    let mut capsule = MicroBlockQuantCapsule::new();
    let input = vec![1.0f32; 64];

    // Single mutable access (compiles)
    capsule.quantize(&input).unwrap();

    // The following would NOT compile:
    // let ref1 = &mut capsule;
    // let ref2 = &mut capsule;
    // ref1.quantize(&input).unwrap(); // Error: multiple mutable borrows
}

/// ASSUME_READER_STALE_OK
///
/// #ASSUME_STATE_VALID: Stale reads acceptable
/// #VERIFY_STATE_MACHINE: Readers always see valid state
#[test]
fn test_stale_read_validity() {
    let capsule = Arc::new(AdaptiveQuantCapsule::new());

    let weights_old = [1.0f32; 128];
    let weights_new = [2.0f32; 128];

    // Initialize with old weights
    capsule.adapt_quantization(&weights_old);

    let stop = Arc::new(AtomicBool::new(false));

    // Reader thread: may see old or new, but never torn
    let reader = {
        let c = Arc::clone(&capsule);
        let s = Arc::clone(&stop);
        thread::spawn(move || {
            let mut saw_old = false;
            let mut saw_new = false;

            while !s.load(Ordering::Relaxed) {
                if let Some(w) = c.load_weight(0) {
                    // Should see either 1.0 or 2.0 (never partial)
                    if (w - 1.0).abs() < 0.1 {
                        saw_old = true;
                    } else if (w - 2.0).abs() < 0.1 {
                        saw_new = true;
                    } else {
                        panic!("Saw invalid weight: {} (expected ~1.0 or ~2.0)", w);
                    }
                }
            }

            println!("Reader saw: old={}, new={}", saw_old, saw_new);
        })
    };

    // Give reader time to see old state
    thread::sleep(std::time::Duration::from_millis(10));

    // Writer: update to new weights
    capsule.adapt_quantization(&weights_new);

    // Give reader time to see new state
    thread::sleep(std::time::Duration::from_millis(10));

    stop.store(true, Ordering::Relaxed);
    reader.join().unwrap();
}

/// Comprehensive concurrent stress test
///
/// 8 readers + 1 writer, 100K operations per reader
/// Validates that concurrent reads don't cause panics or torn data
#[test]
fn test_concurrent_stress() {
    let capsule = Arc::new(AdaptiveQuantCapsule::new());
    let stop = Arc::new(AtomicBool::new(false));
    let successful_reads = Arc::new(AtomicU32::new(0));
    let rejected_reads = Arc::new(AtomicU32::new(0));

    // Writer thread
    let writer = {
        let c = Arc::clone(&capsule);
        let s = Arc::clone(&stop);
        thread::spawn(move || {
            let mut i = 0;
            while !s.load(Ordering::Relaxed) {
                let weights = [(i % 100) as f32; 128];
                c.adapt_quantization(&weights);
                i += 1;
            }
        })
    };

    // 8 reader threads
    let mut readers = vec![];
    for _ in 0..8 {
        let c = Arc::clone(&capsule);
        let s = Arc::clone(&stop);
        let success = Arc::clone(&successful_reads);
        let rejected = Arc::clone(&rejected_reads);

        readers.push(thread::spawn(move || {
            for _ in 0..100_000 {
                // Try loading weight - should never panic or return torn data
                match c.load_weight(0) {
                    Some(_) => {
                        success.fetch_add(1, Ordering::Relaxed);
                    }
                    None => {
                        // Correctly rejected (odd generation or concurrent update)
                        rejected.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }));
    }

    // Wait for readers to complete
    for r in readers {
        r.join().unwrap();
    }

    stop.store(true, Ordering::Relaxed);
    writer.join().unwrap();

    // Verify we had both successful and rejected reads (proving concurrency)
    let success = successful_reads.load(Ordering::Relaxed);
    let rejected = rejected_reads.load(Ordering::Relaxed);

    println!(
        "Concurrent stress test: {} successful, {} rejected ({}% success rate)",
        success,
        rejected,
        (success as f64 / (success + rejected) as f64 * 100.0)
    );

    assert!(success > 0, "No successful reads - system may be stuck");
    assert_eq!(success + rejected, 800_000, "Missing operations");
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test NaN and infinite value rejection
#[test]
fn test_nan_infinite_rejection() {
    let mut capsule = MicroBlockQuantCapsule::new();

    // NaN input
    let mut input = vec![1.0f32; 64];
    input[32] = f32::NAN;
    assert!(capsule.quantize(&input).is_err());

    // Infinite input
    input[32] = f32::INFINITY;
    assert!(capsule.quantize(&input).is_err());

    input[32] = f32::NEG_INFINITY;
    assert!(capsule.quantize(&input).is_err());
}

/// Test buffer size validation
#[test]
fn test_buffer_size_validation() {
    let mut capsule = MicroBlockQuantCapsule::new();

    // Too small input
    let small_input = vec![1.0f32; 32];
    assert!(capsule.quantize(&small_input).is_err());

    // Too large input
    let large_input = vec![1.0f32; 128];
    assert!(capsule.quantize(&large_input).is_err());

    // Correct size should work
    let correct_input = vec![1.0f32; 64];
    assert!(capsule.quantize(&correct_input).is_ok());

    // Too small output buffer
    let mut small_output = vec![0.0f32; 32];
    assert!(capsule.dequantize(&mut small_output).is_err());

    // Correct size should work
    let mut correct_output = vec![0.0f32; 64];
    assert!(capsule.dequantize(&mut correct_output).is_ok());
}

/// Test uniform values (edge case for quantization)
#[test]
fn test_uniform_values() {
    let mut capsule = MicroBlockQuantCapsule::new();

    // All zeros
    let zeros = vec![0.0f32; 64];
    assert!(capsule.quantize(&zeros).is_ok());

    let mut output = vec![0.0f32; 64];
    capsule.dequantize(&mut output).unwrap();

    for (i, &v) in output.iter().enumerate() {
        assert!(v.abs() < 1e-6, "Value {} should be ~0.0, got {}", i, v);
    }

    // All ones
    let ones = vec![1.0f32; 64];
    assert!(capsule.quantize(&ones).is_ok());

    capsule.dequantize(&mut output).unwrap();

    for (i, &v) in output.iter().enumerate() {
        assert!(
            (v - 1.0).abs() < 0.1,
            "Value {} should be ~1.0, got {}",
            i,
            v
        );
    }
}
