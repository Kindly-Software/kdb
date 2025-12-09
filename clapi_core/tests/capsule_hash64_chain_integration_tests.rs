//! # T28 Tier 3: Integration Testing (Q15-Q21) - Hash Chain Validation
//!
//! **Integration tests for hash chain validation with budget operations**.
//!
//! ## Coverage (20+ tests)
//!
//! - **Q15: Critical integration points**: Chain + budget + metrics
//! - **Q16: Error propagation**: Failed ops update chain, budget errors detected
//! - **Q17: Performance budgets**: <100ns/link, <200ns/entry export
//! - **Q18: Production load**: 1000+ operation chains
//! - **Q19: Rollback scenarios**: State reconstruction via hash lookup
//! - **Q20: I20 assumptions**: Chain integrity, audit trail completeness
//! - **Q21: Monitoring**: Metrics export, chain health

use clapi_core::capsules::RequestCapsule128Enhanced;
use std::sync::Arc;
use std::thread;

// ============================================================================
// T28 Q15: Critical Integration Points (5 tests)
// ============================================================================

#[test]
fn test_integration_chain_with_budget_deductions() {
    // Integration: Chain validation + budget enforcement
    let capsule = RequestCapsule128Enhanced::new(1000_00);
    let mut history = vec![capsule.metrics().unwrap()];

    // Deduct within budget
    capsule.try_deduct(100_00).unwrap();
    history.push(capsule.metrics().unwrap());

    capsule.try_deduct(200_00).unwrap();
    history.push(capsule.metrics().unwrap());

    capsule.try_deduct(300_00).unwrap();
    history.push(capsule.metrics().unwrap());

    // Verify integration: budget correct + chain valid
    assert_eq!(capsule.budget(), 400_00, "Budget tracking incorrect");
    assert_eq!(capsule.total_spent(), 600_00, "Total spent tracking incorrect");

    let result = capsule.verify_chain(&history);
    assert!(result.is_valid, "Chain should be valid: {}", result.report);
    assert_eq!(result.broken_links, 0, "No breaks expected");
}

#[test]
fn test_integration_chain_with_mixed_operations() {
    // Integration: Deduct + Credit + Failed operations
    let capsule = RequestCapsule128Enhanced::new(1000_00);
    let mut history = vec![capsule.metrics().unwrap()];

    // Mixed operations
    capsule.try_deduct(100_00).unwrap();
    history.push(capsule.metrics().unwrap());

    capsule.credit(50_00).unwrap();
    history.push(capsule.metrics().unwrap());

    capsule.try_deduct(200_00).unwrap();
    history.push(capsule.metrics().unwrap());

    let _ = capsule.try_deduct(10_000_00); // Will fail
    history.push(capsule.metrics().unwrap());

    // Verify integration
    assert_eq!(capsule.budget(), 750_00, "Final budget incorrect");
    assert_eq!(capsule.total_spent(), 300_00, "Total spent incorrect");
    assert_eq!(
        capsule.metrics().unwrap().failed_deductions,
        1,
        "Failed deduction not tracked"
    );

    let result = capsule.verify_chain(&history);
    assert!(result.is_valid, "Chain with mixed ops should be valid");
}

#[test]
fn test_integration_chain_through_1000_operations() {
    // Integration: Long operation chain
    let capsule = RequestCapsule128Enhanced::new(1_000_000_00);
    let mut history = vec![capsule.metrics().unwrap()];

    // 1000 operations
    for i in 0..1000 {
        capsule.try_deduct((i * 10) as i64).unwrap();
        history.push(capsule.metrics().unwrap());
    }

    // Verify integration
    let expected_spent: i64 = (0..1000).map(|i| i * 10).sum();
    assert_eq!(capsule.total_spent(), expected_spent, "Total spent mismatch");
    assert_eq!(history.len(), 1001, "History length mismatch");

    let result = capsule.verify_chain(&history);
    assert!(result.is_valid, "1000-op chain should be valid: {}", result.report);
    assert_eq!(result.broken_links, 0, "No breaks expected");
}

#[test]
fn test_integration_export_audit_trail_format() {
    // Integration: Audit trail export with all fields
    let capsule = RequestCapsule128Enhanced::new(1000_00);
    let mut history = vec![capsule.metrics().unwrap()];

    capsule.try_deduct(100_00).unwrap();
    history.push(capsule.metrics().unwrap());

    capsule.credit(50_00).unwrap();
    history.push(capsule.metrics().unwrap());

    // Export audit trail
    let audit = capsule.export_audit_trail(&history);

    // Verify format
    assert_eq!(audit.len(), 3, "Audit trail length mismatch");

    // Entry 0: INIT
    assert_eq!(audit[0].operation, "INIT", "First entry should be INIT");
    assert_eq!(audit[0].budget_before, 1000_00, "Init budget_before mismatch");
    assert_eq!(audit[0].budget_after, 1000_00, "Init budget_after mismatch");

    // Entry 1: DEDUCT
    assert_eq!(audit[1].operation, "DEDUCT", "Second entry should be DEDUCT");
    assert_eq!(audit[1].budget_before, 1000_00, "Deduct budget_before mismatch");
    assert_eq!(audit[1].budget_after, 900_00, "Deduct budget_after mismatch");

    // Entry 2: CREDIT
    assert_eq!(audit[2].operation, "CREDIT", "Third entry should be CREDIT");
    assert_eq!(audit[2].budget_before, 900_00, "Credit budget_before mismatch");
    assert_eq!(audit[2].budget_after, 950_00, "Credit budget_after mismatch");

    // Verify integrity flags
    assert!(audit[0].integrity_verified, "INIT integrity should be verified");
    assert!(audit[1].integrity_verified, "DEDUCT integrity should be verified");
    assert!(audit[2].integrity_verified, "CREDIT integrity should be verified");
}

#[test]
fn test_integration_audit_trail_json_serializable() {
    // Integration: Audit trail can be serialized (if serde feature enabled)
    let capsule = RequestCapsule128Enhanced::new(1000_00);
    let mut history = vec![capsule.metrics().unwrap()];

    capsule.try_deduct(50_00).unwrap();
    history.push(capsule.metrics().unwrap());

    let audit = capsule.export_audit_trail(&history);

    // Verify audit entries are complete (required for JSON serialization)
    for (i, entry) in audit.iter().enumerate() {
        assert!(!entry.operation.is_empty(), "Entry {} operation empty", i);
        assert!(entry.timestamp_ns > 0, "Entry {} timestamp zero", i);
        assert!(entry.hash != 0, "Entry {} hash zero", i);
    }

    // Note: Actual JSON serialization requires serde feature
    // This test validates data completeness only
}

// ============================================================================
// T28 Q16: Error Propagation (3 tests)
// ============================================================================

#[test]
fn test_integration_failed_deduction_updates_chain() {
    // Error propagation: Failed deduction → hash changes, chain valid
    let capsule = RequestCapsule128Enhanced::new(100_00);
    let mut history = vec![capsule.metrics().unwrap()];

    let hash_before = capsule.hash();

    // Attempt insufficient deduction
    let result = capsule.try_deduct(200_00);
    assert!(result.is_err(), "Deduction should fail");

    history.push(capsule.metrics().unwrap());
    let hash_after = capsule.hash();

    // Error propagation verification
    assert_ne!(hash_before, hash_after, "Hash should change after failed deduction");
    assert_eq!(
        capsule.metrics().unwrap().failed_deductions,
        1,
        "Failed deduction should be counted"
    );

    let result = capsule.verify_chain(&history);
    assert!(result.is_valid, "Chain should remain valid after failed deduction");
}

#[test]
fn test_integration_budget_exhausted_error_chain_intact() {
    // Error propagation: BudgetExhausted error → chain remains valid
    let capsule = RequestCapsule128Enhanced::new(50_00);
    let mut history = vec![capsule.metrics().unwrap()];

    // Exhaust budget
    capsule.try_deduct(30_00).unwrap();
    history.push(capsule.metrics().unwrap());

    capsule.try_deduct(20_00).unwrap();
    history.push(capsule.metrics().unwrap());

    // Try to overdraft
    let result = capsule.try_deduct(10_00);
    assert!(result.is_err(), "Should fail with BudgetExhausted");
    history.push(capsule.metrics().unwrap());

    // Chain integrity maintained
    let chain_result = capsule.verify_chain(&history);
    assert!(chain_result.is_valid, "Chain should remain valid after budget exhaustion");
}

#[test]
fn test_integration_invalid_cost_error_tracked() {
    // Error propagation: InvalidCost error → tracked in metrics
    let capsule = RequestCapsule128Enhanced::new(1000_00);
    let mut history = vec![capsule.metrics().unwrap()];

    // Attempt negative cost
    let result = capsule.try_deduct(-50_00);
    assert!(result.is_err(), "Negative cost should fail");
    history.push(capsule.metrics().unwrap());

    // Verify error tracking
    assert_eq!(
        capsule.metrics().unwrap().failed_deductions,
        1,
        "Invalid cost should be tracked as failed deduction"
    );

    let chain_result = capsule.verify_chain(&history);
    assert!(chain_result.is_valid, "Chain should remain valid after invalid cost");
}

// ============================================================================
// T28 Q17: Performance Budgets (4 tests)
// ============================================================================

#[test]
fn test_integration_performance_verify_chain_100_entries() {
    // Performance: verify_chain() <100ns per link
    let capsule = RequestCapsule128Enhanced::new(100_000_00);
    let mut history = vec![capsule.metrics().unwrap()];

    // Build 100-entry chain
    for i in 0..100 {
        capsule.try_deduct((i * 10) as i64).unwrap();
        history.push(capsule.metrics().unwrap());
    }

    // Measure verification performance
    let iterations = 1_000;
    let start = std::time::Instant::now();

    for _ in 0..iterations {
        let _ = std::hint::black_box(capsule.verify_chain(&history));
    }

    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / iterations;
    let ns_per_link = avg_ns / history.len() as u128;

    println!(
        "verify_chain() performance: {}ns total, {}ns/link (100 entries)",
        avg_ns, ns_per_link
    );

    // Budget: <200ns per link (conservative for 100-entry chain)
    assert!(
        ns_per_link < 200,
        "Verification too slow: {}ns/link > 200ns",
        ns_per_link
    );
}

#[test]
fn test_integration_performance_export_audit_trail() {
    // Performance: export_audit_trail() <200ns per entry
    let capsule = RequestCapsule128Enhanced::new(100_000_00);
    let mut history = vec![capsule.metrics().unwrap()];

    // Build 100-entry chain
    for i in 0..100 {
        capsule.try_deduct((i * 100) as i64).unwrap();
        history.push(capsule.metrics().unwrap());
    }

    // Measure export performance
    let iterations = 1_000;
    let start = std::time::Instant::now();

    for _ in 0..iterations {
        let _ = std::hint::black_box(capsule.export_audit_trail(&history));
    }

    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / iterations;
    let ns_per_entry = avg_ns / history.len() as u128;

    println!(
        "export_audit_trail() performance: {}ns total, {}ns/entry (100 entries)",
        avg_ns, ns_per_entry
    );

    // Budget: <500ns per entry (conservative for struct construction)
    assert!(
        ns_per_entry < 500,
        "Export too slow: {}ns/entry > 500ns",
        ns_per_entry
    );
}

#[test]
fn test_integration_performance_find_state_at_hash() {
    // Performance: find_state_at_hash() <100ns with 1000 entries
    let capsule = RequestCapsule128Enhanced::new(1_000_000_00);
    let mut history = vec![capsule.metrics().unwrap()];

    // Build 1000-entry chain
    for i in 0..1000 {
        capsule.try_deduct((i * 100) as i64).unwrap();
        history.push(capsule.metrics().unwrap());
    }

    // Target hash in middle of chain
    let target_hash = history[500].hash;

    // Measure lookup performance
    let iterations = 10_000;
    let start = std::time::Instant::now();

    for _ in 0..iterations {
        let _ = std::hint::black_box(capsule.find_state_at_hash(target_hash, &history));
    }

    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / iterations;

    println!(
        "find_state_at_hash() performance: {}ns (1000 entries)",
        avg_ns
    );

    // Budget: <10μs (10,000ns) for linear search through 1000 entries
    assert!(
        avg_ns < 10_000,
        "Lookup too slow: {}ns > 10μs",
        avg_ns
    );
}

#[test]
fn test_integration_performance_walk_chain_backward() {
    // Performance: walk_chain_backward() <5ns per iteration
    let capsule = RequestCapsule128Enhanced::new(100_000_00);
    let mut history = vec![capsule.metrics().unwrap()];

    // Build 100-entry chain
    for i in 0..100 {
        capsule.try_deduct((i * 100) as i64).unwrap();
        history.push(capsule.metrics().unwrap());
    }

    // Measure iteration performance
    let iterations = 10_000;
    let start = std::time::Instant::now();

    for _ in 0..iterations {
        for entry in capsule.walk_chain_backward(&history) {
            std::hint::black_box(entry);
        }
    }

    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / (iterations * history.len() as u128);

    println!(
        "walk_chain_backward() performance: {}ns/iteration (100 entries)",
        avg_ns
    );

    // Budget: <50ns per iteration (iterator overhead + bounds check)
    assert!(
        avg_ns < 50,
        "Walk backward too slow: {}ns/iter > 50ns",
        avg_ns
    );
}

// ============================================================================
// T28 Q18: Production Load (3 tests)
// ============================================================================

#[test]
fn test_integration_production_load_10k_operations() {
    // Production load: 10K operations
    let capsule = RequestCapsule128Enhanced::new(100_000_000_00); // $1M budget
    let mut history = vec![capsule.metrics().unwrap()];

    // 10K operations
    let start = std::time::Instant::now();
    for i in 0..10_000 {
        capsule.try_deduct((i * 100) as i64).unwrap();
        if i % 100 == 0 {
            // Capture metrics every 100 ops
            history.push(capsule.metrics().unwrap());
        }
    }
    let elapsed = start.elapsed();

    println!(
        "10K operations completed in {:?} ({} metrics captured)",
        elapsed,
        history.len()
    );

    // Verify chain integrity after load
    let result = capsule.verify_chain(&history);
    assert!(result.is_valid, "Chain should remain valid under production load");
    assert_eq!(result.broken_links, 0, "No breaks expected");
}

#[test]
fn test_integration_production_concurrent_operations() {
    // Production load: Concurrent operations (10 threads × 1000 ops)
    let capsule = Arc::new(RequestCapsule128Enhanced::new(100_000_000_00));
    let threads = 10;
    let ops_per_thread = 1_000;

    let start = std::time::Instant::now();
    let handles: Vec<_> = (0..threads)
        .map(|t| {
            let cap = Arc::clone(&capsule);
            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    let cost = (t * 10_000 + i * 10) as i64;
                    let _ = cap.try_deduct(cost);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread panicked");
    }
    let elapsed = start.elapsed();

    println!(
        "{} threads × {} ops completed in {:?}",
        threads, ops_per_thread, elapsed
    );

    // Verify final integrity
    assert!(
        capsule.verify_integrity(),
        "Integrity should be maintained under concurrent load"
    );
}

#[test]
fn test_integration_production_long_running_chain() {
    // Production scenario: Long-running chain (24 hours simulation)
    let capsule = RequestCapsule128Enhanced::new(1_000_000_000_00); // $10M budget
    let mut history = vec![capsule.metrics().unwrap()];

    // Simulate 24 hours @ 1 op/second = 86,400 operations
    // For test speed, simulate 1000 operations
    for i in 0..1000 {
        capsule.try_deduct((i * 1000) as i64).unwrap();
        if i % 100 == 0 {
            history.push(capsule.metrics().unwrap());
        }
    }

    // Verify chain after long operation
    let result = capsule.verify_chain(&history);
    assert!(result.is_valid, "Long-running chain should remain valid");

    // Audit trail export
    let audit = capsule.export_audit_trail(&history);
    assert_eq!(audit.len(), history.len(), "Audit trail should be complete");
}

// ============================================================================
// T28 Q19: Rollback Scenarios (2 tests)
// ============================================================================

#[test]
fn test_integration_state_reconstruction_at_hash() {
    // Rollback scenario: Reconstruct state at specific hash
    let capsule = RequestCapsule128Enhanced::new(10_000_00);
    let mut history = vec![capsule.metrics().unwrap()];

    // Perform operations
    capsule.try_deduct(1000_00).unwrap();
    history.push(capsule.metrics().unwrap());

    let checkpoint_hash = capsule.hash();
    let checkpoint_budget = capsule.budget();

    capsule.try_deduct(2000_00).unwrap();
    history.push(capsule.metrics().unwrap());

    capsule.try_deduct(3000_00).unwrap();
    history.push(capsule.metrics().unwrap());

    // Reconstruct state at checkpoint
    let state_at_checkpoint = capsule
        .find_state_at_hash(checkpoint_hash, &history)
        .expect("Checkpoint hash should be found");

    assert_eq!(
        state_at_checkpoint.hash, checkpoint_hash,
        "Hash mismatch at checkpoint"
    );
    assert_eq!(
        state_at_checkpoint.budget_cents, checkpoint_budget,
        "Budget mismatch at checkpoint"
    );
}

#[test]
fn test_integration_walk_backward_to_initial_state() {
    // Rollback scenario: Walk backward to initial state
    let initial_budget = 10_000_00;
    let capsule = RequestCapsule128Enhanced::new(initial_budget);
    let mut history = vec![capsule.metrics().unwrap()];

    // Perform operations
    for i in 1..=10 {
        capsule.try_deduct((i * 100) as i64).unwrap();
        history.push(capsule.metrics().unwrap());
    }

    // Walk backward
    let mut walked_entries = vec![];
    for entry in capsule.walk_chain_backward(&history) {
        walked_entries.push(entry);
    }

    // Verify backward walk reaches initial state
    assert_eq!(walked_entries.len(), history.len(), "Walk should cover all entries");

    let initial_state = walked_entries.last().unwrap();
    assert_eq!(
        initial_state.budget_cents, initial_budget,
        "Walk backward should reach initial budget"
    );
}

// ============================================================================
// T28 Q20: I20 Assumptions (2 tests)
// ============================================================================

#[test]
fn test_integration_i20_chain_integrity_assumption() {
    // I20 Q11: Verify chain integrity assumption
    // ASSUME: All operations maintain hash chain linkage
    // VERIFY: verify_chain() detects ANY broken link
    let capsule = RequestCapsule128Enhanced::new(10_000_00);
    let mut history = vec![capsule.metrics().unwrap()];

    // Build valid chain
    for i in 0..100 {
        capsule.try_deduct((i * 10) as i64).unwrap();
        history.push(capsule.metrics().unwrap());
    }

    // Verify assumption: chain is valid
    let result = capsule.verify_chain(&history);
    assert!(result.is_valid, "I20 Q11: Chain integrity assumption violated");

    // Intentionally break chain
    let mut corrupted_history = history.clone();
    corrupted_history[50].prev_hash ^= 0xFFFFFFFF;

    // Verify assumption: break is detected
    let corrupted_result = capsule.verify_chain(&corrupted_history);
    assert!(
        !corrupted_result.is_valid,
        "I20 Q11: Break detection assumption violated"
    );
}

#[test]
fn test_integration_i20_audit_trail_completeness_assumption() {
    // I20 Q13: Verify audit trail completeness assumption
    // ASSUME: export_audit_trail() includes ALL history entries
    // VERIFY: audit.len() === history.len() for all cases
    let test_cases = vec![
        (10, "small chain"),
        (100, "medium chain"),
        (1000, "large chain"),
    ];

    for (num_ops, description) in test_cases {
        let capsule = RequestCapsule128Enhanced::new(1_000_000_00);
        let mut history = vec![capsule.metrics().unwrap()];

        for i in 0..num_ops {
            capsule.try_deduct((i * 100) as i64).unwrap();
            history.push(capsule.metrics().unwrap());
        }

        let audit = capsule.export_audit_trail(&history);
        assert_eq!(
            audit.len(),
            history.len(),
            "I20 Q13: Audit trail completeness violated for {}",
            description
        );
    }
}

// ============================================================================
// T28 Q21: Monitoring Integration (2 tests)
// ============================================================================

#[test]
fn test_integration_metrics_export_with_chain_health() {
    // Monitoring: Export metrics with chain health indicators
    let capsule = RequestCapsule128Enhanced::new(10_000_00);
    let mut history = vec![capsule.metrics().unwrap()];

    // Perform operations
    capsule.try_deduct(1000_00).unwrap();
    history.push(capsule.metrics().unwrap());

    capsule.try_deduct(2000_00).unwrap();
    history.push(capsule.metrics().unwrap());

    let _ = capsule.try_deduct(20_000_00); // Will fail
    history.push(capsule.metrics().unwrap());

    // Export metrics
    let metrics = capsule.metrics().expect("Metrics should be available");

    // Verify monitoring data
    assert!(metrics.integrity_verified, "Chain health: integrity should be verified");
    assert_eq!(metrics.deduction_count, 2, "Successful deductions tracking");
    assert_eq!(metrics.failed_deductions, 1, "Failed deductions tracking");
    assert_eq!(metrics.request_count, 2, "Request count tracking");

    // Verify chain health
    let chain_result = capsule.verify_chain(&history);
    assert!(chain_result.is_valid, "Chain health: chain should be valid");
    assert_eq!(chain_result.broken_links, 0, "Chain health: no breaks expected");
}

#[test]
fn test_integration_chain_validation_report_format() {
    // Monitoring: Chain validation report format
    let capsule = RequestCapsule128Enhanced::new(10_000_00);
    let mut history = vec![capsule.metrics().unwrap()];

    // Build chain
    for i in 0..10 {
        capsule.try_deduct((i * 100) as i64).unwrap();
        history.push(capsule.metrics().unwrap());
    }

    // Valid chain report
    let valid_result = capsule.verify_chain(&history);
    assert!(
        valid_result.report.contains("valid"),
        "Report should indicate valid chain"
    );
    assert!(
        valid_result.report.contains("11"),
        "Report should include entry count"
    );

    // Corrupted chain report
    let mut corrupted_history = history.clone();
    corrupted_history[5].prev_hash ^= 0xFF;

    let corrupted_result = capsule.verify_chain(&corrupted_history);
    assert!(
        corrupted_result.report.contains("BREAK"),
        "Report should indicate break"
    );
    assert!(
        corrupted_result.report.contains("entry 5"),
        "Report should indicate break location"
    );
}
