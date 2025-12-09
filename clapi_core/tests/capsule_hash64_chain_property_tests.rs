//! # T28 Tier 2: Property Testing (Q8-Q14) - Hash Chain Validation
//!
//! **Property-based tests for hash chain validation in RequestCapsule128Enhanced**.
//!
//! ## Coverage (15+ tests)
//!
//! - **Q8: Universal properties**: Chain determinism, link correctness, no breaks
//! - **Q9: Concurrent properties**: Thread-safe verification, race-free updates
//! - **Q10: Edge case properties**: Boundary values preserve chain integrity
//! - **Q11: ASSUM verification**: Relaxed ordering safe, prev_hash invariant holds
//! - **Q12: Composition properties**: Multi-operation chains valid
//! - **Q13: Statistical properties**: Break detection rate (100%)
//! - **Q14: Regression tracking**: Known chains produce expected results

use clapi_core::capsules::RequestCapsule128Enhanced;
use std::sync::Arc;
use std::thread;

// ============================================================================
// T28 Q8: Universal Properties (4 tests)
// ============================================================================

#[test]
fn property_chain_always_valid_after_valid_operations() {
    // Property: Valid operations → valid chain (always)
    let iterations = 1_000;

    for seed in 0..iterations {
        let capsule = RequestCapsule128Enhanced::new(100_000_00);
        let mut history = vec![capsule.metrics().unwrap()];

        // Random sequence of operations
        for i in 0..10 {
            let cost = ((seed + i) % 100 + 1) * 100; // 100-10000 cents
            capsule.try_deduct(cost as i64).unwrap();
            history.push(capsule.metrics().unwrap());
        }

        // Verify chain integrity
        let result = capsule.verify_chain(&history);
        assert!(
            result.is_valid,
            "Chain invalid after {} operations (seed={}): {}",
            history.len() - 1,
            seed,
            result.report
        );
        assert_eq!(
            result.broken_links, 0,
            "Expected 0 breaks, found {} (seed={})",
            result.broken_links, seed
        );
    }

    println!("✅ Chain valid for {} random sequences", iterations);
}

#[test]
fn property_prev_hash_equals_previous_hash_always() {
    // Property: ∀n, history[n].prev_hash === history[n-1].hash
    let iterations = 100;

    for seed in 0..iterations {
        let capsule = RequestCapsule128Enhanced::new(10_000_00);
        let mut history = vec![capsule.metrics().unwrap()];

        for i in 0..20 {
            let cost = ((seed + i) * 10) as i64;
            capsule.try_deduct(cost).unwrap();
            history.push(capsule.metrics().unwrap());
        }

        // Verify property for all pairs
        for i in 1..history.len() {
            let prev_hash_claimed = history[i].prev_hash;
            let previous_hash_actual = history[i - 1].hash;

            assert_eq!(
                prev_hash_claimed, previous_hash_actual,
                "Link property violated at index {} (seed={}): prev_hash={:016x}, actual={:016x}",
                i, seed, prev_hash_claimed, previous_hash_actual
            );
        }
    }

    println!("✅ prev_hash property holds for {} chains", iterations);
}

#[test]
fn property_chain_deterministic_same_operations() {
    // Property: Same operations → same chain (determinism)
    let iterations = 100;

    for seed in 0..iterations {
        // First execution
        let capsule1 = RequestCapsule128Enhanced::new(10_000_00);
        let mut history1 = vec![capsule1.metrics().unwrap()];

        for i in 0..10 {
            let cost = ((seed + i) * 50) as i64;
            capsule1.try_deduct(cost).unwrap();
            history1.push(capsule1.metrics().unwrap());
        }

        // Second execution (same operations)
        let capsule2 = RequestCapsule128Enhanced::new(10_000_00);
        let mut history2 = vec![capsule2.metrics().unwrap()];

        for i in 0..10 {
            let cost = ((seed + i) * 50) as i64;
            capsule2.try_deduct(cost).unwrap();
            history2.push(capsule2.metrics().unwrap());
        }

        // Compare chains
        assert_eq!(
            history1.len(),
            history2.len(),
            "Chain lengths differ (seed={})",
            seed
        );

        for (i, (entry1, entry2)) in history1.iter().zip(history2.iter()).enumerate() {
            assert_eq!(
                entry1.hash, entry2.hash,
                "Hash mismatch at index {} (seed={})",
                i, seed
            );
            assert_eq!(
                entry1.prev_hash, entry2.prev_hash,
                "prev_hash mismatch at index {} (seed={})",
                i, seed
            );
        }
    }

    println!("✅ Chain determinism verified for {} sequences", iterations);
}

#[test]
fn property_no_hash_collisions_in_chain() {
    // Property: All hashes in chain are unique (collision-free)
    use std::collections::HashSet;

    let iterations = 100;

    for seed in 0..iterations {
        let capsule = RequestCapsule128Enhanced::new(100_000_00);
        let mut history = vec![capsule.metrics().unwrap()];
        let mut seen_hashes = HashSet::new();

        seen_hashes.insert(history[0].hash);

        for i in 0..100 {
            let cost = ((seed + i) * 10 + 1) as i64;
            capsule.try_deduct(cost).unwrap();
            let metrics = capsule.metrics().unwrap();
            history.push(metrics);

            // Check for collision
            if !seen_hashes.insert(metrics.hash) {
                panic!(
                    "HASH COLLISION at iteration {} (seed={}): hash={:016x}",
                    i, seed, metrics.hash
                );
            }
        }
    }

    println!("✅ Zero hash collisions in {} chains (100 ops each)", iterations);
}

// ============================================================================
// T28 Q9: Concurrent Properties (3 tests)
// ============================================================================

#[test]
fn property_concurrent_verification_thread_safe() {
    // Property: verify_chain() is thread-safe (no races)
    let capsule = Arc::new(RequestCapsule128Enhanced::new(1_000_000_00));
    let mut history = vec![capsule.metrics().unwrap()];

    // Build chain
    for i in 0..1000 {
        capsule.try_deduct((i * 10) as i64).unwrap();
        history.push(capsule.metrics().unwrap());
    }

    let history = Arc::new(history);
    let threads = 50;
    let iterations = 1_000;

    // Concurrent verification
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let cap = Arc::clone(&capsule);
            let hist = Arc::clone(&history);
            thread::spawn(move || {
                for _ in 0..iterations {
                    let result = cap.verify_chain(&hist);
                    assert!(result.is_valid, "Chain should remain valid");
                    assert_eq!(result.broken_links, 0, "No breaks expected");
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread panicked");
    }

    println!(
        "✅ Concurrent verification: {} threads × {} iterations",
        threads, iterations
    );
}

#[test]
fn property_concurrent_operations_preserve_chain() {
    // Property: Concurrent operations maintain chain integrity
    let capsule = Arc::new(RequestCapsule128Enhanced::new(10_000_000_00));
    let threads = 10;
    let ops_per_thread = 100;

    let handles: Vec<_> = (0..threads)
        .map(|t| {
            let cap = Arc::clone(&capsule);
            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    let cost = (t * 1000 + i * 10) as i64;
                    let _ = cap.try_deduct(cost);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread panicked");
    }

    // Final integrity check
    assert!(
        capsule.verify_integrity(),
        "Concurrent operations broke integrity"
    );

    println!(
        "✅ Chain integrity preserved: {} threads × {} ops",
        threads, ops_per_thread
    );
}

#[test]
fn property_concurrent_history_export_consistent() {
    // Property: export_audit_trail() returns consistent results under concurrency
    let capsule = Arc::new(RequestCapsule128Enhanced::new(1_000_000_00));
    let mut history = vec![capsule.metrics().unwrap()];

    // Build deterministic chain
    for i in 0..100 {
        capsule.try_deduct((i * 100) as i64).unwrap();
        history.push(capsule.metrics().unwrap());
    }

    let history = Arc::new(history);
    let threads = 20;

    // Concurrent audit trail exports
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let cap = Arc::clone(&capsule);
            let hist = Arc::clone(&history);
            thread::spawn(move || {
                let audit = cap.export_audit_trail(&hist);
                assert_eq!(audit.len(), hist.len(), "Audit trail incomplete");
                audit
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // All exports should be identical
    let reference = &results[0];
    for (i, audit) in results.iter().enumerate().skip(1) {
        assert_eq!(
            audit.len(),
            reference.len(),
            "Thread {} export length mismatch",
            i
        );
        for (j, (entry, ref_entry)) in audit.iter().zip(reference.iter()).enumerate() {
            assert_eq!(
                entry.hash, ref_entry.hash,
                "Thread {} entry {} hash mismatch",
                i, j
            );
        }
    }

    println!("✅ Concurrent audit trail exports: {} threads (consistent)", threads);
}

// ============================================================================
// T28 Q10: Edge Case Properties (2 tests)
// ============================================================================

#[test]
fn property_boundary_values_preserve_chain() {
    // Property: Boundary values (0, MAX) maintain chain integrity
    let test_cases = vec![
        (0i64, "zero_budget"),
        (1i64, "min_budget"),
        (i64::MAX / 2, "large_budget"),
    ];

    for (initial_budget, description) in test_cases {
        let capsule = RequestCapsule128Enhanced::new(initial_budget);
        let mut history = vec![capsule.metrics().unwrap()];

        // Attempt operations within budget
        if initial_budget > 1000 {
            capsule.try_deduct(100).unwrap();
            history.push(capsule.metrics().unwrap());

            capsule.try_deduct(200).unwrap();
            history.push(capsule.metrics().unwrap());
        }

        // Verify chain
        let result = capsule.verify_chain(&history);
        assert!(
            result.is_valid,
            "Chain invalid for {} (initial={}): {}",
            description,
            initial_budget,
            result.report
        );
    }

    println!("✅ Boundary values preserve chain integrity");
}

#[test]
fn property_failed_operations_update_chain() {
    // Property: Failed operations still update hash chain (failed counter changes)
    let iterations = 100;

    for seed in 0..iterations {
        let capsule = RequestCapsule128Enhanced::new(1000_00);
        let mut history = vec![capsule.metrics().unwrap()];

        // Successful operation
        capsule.try_deduct(100_00).unwrap();
        history.push(capsule.metrics().unwrap());

        // Failed operation (insufficient budget)
        let _ = capsule.try_deduct(10_000_00);
        let metrics_after_fail = capsule.metrics().unwrap();
        history.push(metrics_after_fail);

        // Property: Hash changed after failed operation
        assert_ne!(
            history[1].hash,
            history[2].hash,
            "Hash should change after failed operation (seed={})",
            seed
        );

        // Property: failed_deductions incremented
        assert_eq!(
            metrics_after_fail.failed_deductions, 1,
            "Failed deductions should increment (seed={})",
            seed
        );

        // Property: Chain still valid
        let result = capsule.verify_chain(&history);
        assert!(
            result.is_valid,
            "Chain should remain valid after failed operation (seed={})",
            seed
        );
    }

    println!("✅ Failed operations maintain chain integrity ({}  iterations)", iterations);
}

// ============================================================================
// T28 Q11: ASSUM Verification (2 tests)
// ============================================================================

#[test]
fn verify_assum_prev_hash_invariant() {
    // ASSUM: prev_hash always updated when hash changes
    // VERIFY: Capture (hash_n, prev_hash_n+1) and validate prev_hash_n+1 === hash_n
    let iterations = 1_000;

    for seed in 0..iterations {
        let capsule = RequestCapsule128Enhanced::new(10_000_00);
        let mut hash_sequence = vec![];

        hash_sequence.push(capsule.hash());

        for i in 0..20 {
            capsule.try_deduct(((seed + i) * 10) as i64).unwrap();
            hash_sequence.push(capsule.hash());
        }

        // Now rebuild history and verify
        let capsule2 = RequestCapsule128Enhanced::new(10_000_00);
        let mut history = vec![capsule2.metrics().unwrap()];

        for i in 0..20 {
            capsule2.try_deduct(((seed + i) * 10) as i64).unwrap();
            history.push(capsule2.metrics().unwrap());
        }

        // Verify ASSUM: prev_hash_n+1 === hash_n
        for i in 1..history.len() {
            assert_eq!(
                history[i].prev_hash,
                history[i - 1].hash,
                "ASSUM violated: prev_hash invariant broken at index {} (seed={})",
                i,
                seed
            );
        }
    }

    println!("✅ ASSUM verified: prev_hash invariant holds ({} chains)", iterations);
}

#[test]
fn verify_assum_relaxed_ordering_safe_for_chain() {
    // ASSUM: Relaxed ordering on hash loads safe for chain verification
    // VERIFY: Concurrent readers get consistent chain validation results
    let capsule = Arc::new(RequestCapsule128Enhanced::new(100_000_00));
    let mut history = vec![capsule.metrics().unwrap()];

    // Build chain
    for i in 0..100 {
        capsule.try_deduct((i * 100) as i64).unwrap();
        history.push(capsule.metrics().unwrap());
    }

    let history = Arc::new(history);
    let readers = 50;
    let iterations = 1_000;

    // Concurrent readers (all using Relaxed ordering internally)
    let handles: Vec<_> = (0..readers)
        .map(|_| {
            let cap = Arc::clone(&capsule);
            let hist = Arc::clone(&history);
            thread::spawn(move || {
                for _ in 0..iterations {
                    let result = cap.verify_chain(&hist);
                    // Relaxed ordering should not cause false positives/negatives
                    assert!(result.is_valid, "Relaxed ordering caused false negative");
                    assert_eq!(result.broken_links, 0, "Relaxed ordering reported phantom breaks");
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("ASSUM violated: Relaxed ordering unsafe");
    }

    println!("✅ ASSUM verified: Relaxed ordering safe for chain verification");
}

// ============================================================================
// T28 Q12: Composition Properties (2 tests)
// ============================================================================

#[test]
fn property_chain_composition_transitive() {
    // Property: Chain(A) valid ∧ Chain(B) valid ⇒ Chain(A + B) valid
    let iterations = 100;

    for seed in 0..iterations {
        // Build first chain segment
        let capsule = RequestCapsule128Enhanced::new(100_000_00);
        let mut history_a = vec![capsule.metrics().unwrap()];

        for i in 0..10 {
            capsule.try_deduct(((seed + i) * 50) as i64).unwrap();
            history_a.push(capsule.metrics().unwrap());
        }

        // Verify segment A
        let result_a = capsule.verify_chain(&history_a);
        assert!(result_a.is_valid, "Segment A invalid (seed={})", seed);

        // Build second chain segment (continuing from same capsule)
        let mut history_b = history_a.clone();

        for i in 10..20 {
            capsule.try_deduct(((seed + i) * 50) as i64).unwrap();
            history_b.push(capsule.metrics().unwrap());
        }

        // Verify combined chain
        let result_b = capsule.verify_chain(&history_b);
        assert!(result_b.is_valid, "Combined chain invalid (seed={})", seed);
    }

    println!("✅ Chain composition transitive ({} chains)", iterations);
}

#[test]
fn property_audit_trail_completeness() {
    // Property: export_audit_trail() includes all history entries
    let iterations = 100;

    for seed in 0..iterations {
        let capsule = RequestCapsule128Enhanced::new(10_000_00);
        let mut history = vec![capsule.metrics().unwrap()];

        let num_ops = (seed % 50) + 10; // 10-59 operations
        for i in 0..num_ops {
            capsule.try_deduct((i * 10) as i64).unwrap();
            history.push(capsule.metrics().unwrap());
        }

        // Export audit trail
        let audit = capsule.export_audit_trail(&history);

        // Property: audit.len() === history.len()
        assert_eq!(
            audit.len(),
            history.len(),
            "Audit trail incomplete (seed={}): expected {}, got {}",
            seed,
            history.len(),
            audit.len()
        );

        // Property: All hashes present in audit trail
        for (i, (entry, metrics)) in audit.iter().zip(history.iter()).enumerate() {
            assert_eq!(
                entry.hash, metrics.hash,
                "Audit entry {} hash mismatch (seed={})",
                i, seed
            );
            assert_eq!(
                entry.prev_hash, metrics.prev_hash,
                "Audit entry {} prev_hash mismatch (seed={})",
                i, seed
            );
        }
    }

    println!("✅ Audit trail completeness verified ({} chains)", iterations);
}

// ============================================================================
// T28 Q13: Statistical Properties (1 test)
// ============================================================================

#[test]
fn property_break_detection_rate_100_percent() {
    // Property: Intentional breaks are detected 100% of the time
    let iterations = 1_000;
    let mut breaks_detected = 0;

    for seed in 0..iterations {
        let capsule = RequestCapsule128Enhanced::new(10_000_00);
        let mut history = vec![capsule.metrics().unwrap()];

        // Build chain
        for i in 0..10 {
            capsule.try_deduct(((seed + i) * 50) as i64).unwrap();
            history.push(capsule.metrics().unwrap());
        }

        // Intentionally corrupt chain (modify prev_hash at random index)
        let break_index = ((seed as usize) % (history.len() - 1)) + 1;
        let mut corrupted_history = history.clone();
        corrupted_history[break_index].prev_hash ^= 0xFFFFFFFF; // Flip bits

        // Verify chain with break
        let result = capsule.verify_chain(&corrupted_history);

        if !result.is_valid {
            breaks_detected += 1;
        } else {
            panic!(
                "BREAK NOT DETECTED at index {} (seed={}): {}",
                break_index, seed, result.report
            );
        }
    }

    // Statistical property: 100% detection rate
    let detection_rate = (breaks_detected as f64 / iterations as f64) * 100.0;
    assert_eq!(
        breaks_detected, iterations,
        "Detection rate {:.2}% < 100%",
        detection_rate
    );

    println!(
        "✅ Break detection rate: {:.2}% ({}/{} breaks detected)",
        detection_rate, breaks_detected, iterations
    );
}

// ============================================================================
// T28 Q14: Regression Tracking (1 test)
// ============================================================================

#[test]
fn property_regression_known_chain() {
    // Property: Known operations produce known chain (regression detection)
    let capsule = RequestCapsule128Enhanced::new(10_000_00);
    let mut history = vec![capsule.metrics().unwrap()];

    // Deterministic sequence
    let operations = [100, 200, 300, 400, 500];
    for &cost in &operations {
        capsule.try_deduct(cost).unwrap();
        history.push(capsule.metrics().unwrap());
    }

    // Verify chain properties
    let result = capsule.verify_chain(&history);
    assert!(result.is_valid, "Known chain should be valid");
    assert_eq!(result.broken_links, 0, "Known chain should have zero breaks");
    assert_eq!(history.len(), operations.len() + 1, "History length mismatch");

    // Audit trail verification
    let audit = capsule.export_audit_trail(&history);
    assert_eq!(audit.len(), history.len(), "Audit trail length mismatch");

    // First entry should be INIT
    assert_eq!(audit[0].operation, "INIT", "First entry should be INIT");

    // Subsequent entries should be DEDUCT
    for (i, entry) in audit.iter().enumerate().skip(1) {
        assert_eq!(entry.operation, "DEDUCT", "Entry {} should be DEDUCT", i);
    }

    println!("✅ Regression check: known chain verified");
}
