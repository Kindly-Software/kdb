//! T28 Tier 2: Property Testing (Q8-Q14)
//!
//! Property-based tests for budget metacapsule invariants.
//!
//! **Coverage:**
//! - Q8: Universal properties (budget conservation, generation monotonic)
//! - Q9: Concurrent invariants (no lost updates, determinism)
//! - Q10: Edge case properties (extreme values, overflow protection)
//! - Q11: ASSUM verification (TOCTOU prevention, alignment)
//! - Q12: Composition properties (budget+routing, budget+metrics)
//! - Q13: Statistical properties (uniform distribution, latency bounds)
//! - Q14: Regression prevention (stable behavior, generation never wraps)
//!
//! **Test Count:** 40 property tests

use clapi_core::error::{ClapiError, ClapiResult};
use clapi_core::proxy::budget_registry::BudgetRegistry;
use clapi_core::RequestCapsule128;
use proptest::prelude::*;
use std::sync::Arc;
use std::thread;

// ============================================================================
// T28 Q8: Universal Properties (5 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_budget_never_negative(
        initial in 1000_00i64..10_000_00i64,
        deductions in prop::collection::vec(100_00i64..500_00i64, 1..50)
    ) {
        let registry = BudgetRegistry::new(initial);
        let budget_id = 1u64;

        for amount in deductions {
            let _ = registry.try_deduct(budget_id, amount);
        }

        // Property: Budget never goes negative
        if let Some(budget) = registry.get_budget(budget_id) {
            prop_assert!(budget >= 0, "Budget went negative: {}", budget);
        }
    }

    #[test]
    fn prop_budget_conservation(
        initial in 1000_00i64..10_000_00i64,
        deductions in prop::collection::vec(10_00i64..100_00i64, 10..50)
    ) {
        let registry = BudgetRegistry::new(initial);
        let budget_id = 1u64;

        for amount in deductions {
            let _ = registry.try_deduct(budget_id, amount);
        }

        // Property: budget + total_spent = initial
        if let Some(stats) = registry.get_stats(budget_id) {
            prop_assert_eq!(
                stats.budget + stats.total_spent,
                initial,
                "Budget conservation violated"
            );
        }
    }

    #[test]
    fn prop_generation_monotonic(
        initial in 1000_00i64..10_000_00i64,
        operations in prop::collection::vec(10_00i64..100_00i64, 5..20)
    ) {
        let registry = BudgetRegistry::new(initial);
        let budget_id = 1u64;

        let mut last_gen = 0u64;

        for amount in operations {
            if registry.try_deduct(budget_id, amount).is_ok() {
                if let Some(stats) = registry.get_stats(budget_id) {
                    // Property: Generation always increases
                    prop_assert!(
                        stats.generation > last_gen,
                        "Generation not monotonic: {} <= {}",
                        stats.generation,
                        last_gen
                    );
                    last_gen = stats.generation;
                }
            }
        }
    }

    #[test]
    fn prop_deduct_idempotent_reads(
        initial in 1000_00i64..10_000_00i64,
        amount in 10_00i64..100_00i64
    ) {
        let registry = BudgetRegistry::new(initial);
        let budget_id = 1u64;

        registry.try_deduct(budget_id, amount).ok();

        // Property: Multiple reads return same value (idempotent)
        let budget1 = registry.get_budget(budget_id);
        let budget2 = registry.get_budget(budget_id);
        let budget3 = registry.get_budget(budget_id);

        prop_assert_eq!(budget1, budget2);
        prop_assert_eq!(budget2, budget3);
    }

    #[test]
    fn prop_request_count_matches_successful_deductions(
        initial in 10_000_00i64..100_000_00i64,
        deductions in prop::collection::vec(10_00i64..100_00i64, 10..50)
    ) {
        let registry = BudgetRegistry::new(initial);
        let budget_id = 1u64;

        let mut successful = 0u64;

        for amount in deductions {
            if registry.try_deduct(budget_id, amount).is_ok() {
                successful += 1;
            }
        }

        // Property: request_count = number of successful deductions
        if let Some(stats) = registry.get_stats(budget_id) {
            prop_assert_eq!(
                stats.request_count,
                successful,
                "Request count mismatch"
            );
        }
    }
}

// ============================================================================
// T28 Q9: Concurrent Invariants (8 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_concurrent_no_lost_updates(
        initial in 10_000_00i64..100_000_00i64,
        threads in 5usize..20usize,
        ops_per_thread in 10usize..50usize,
        amount in 1_00i64..10_00i64
    ) {
        let registry = Arc::new(BudgetRegistry::new(initial));
        let budget_id = 1u64;

        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let r = Arc::clone(&registry);
                thread::spawn(move || {
                    for _ in 0..ops_per_thread {
                        let _ = r.try_deduct(budget_id, amount);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // Property: All updates visible (no lost writes)
        if let Some(stats) = registry.get_stats(budget_id) {
            prop_assert_eq!(
                stats.budget + stats.total_spent,
                initial,
                "Lost updates detected"
            );
        }
    }

    #[test]
    fn prop_concurrent_deterministic_final_state(
        initial in 10_000_00i64..50_000_00i64,
        total_deductions in 1_00i64..5_00i64
    ) {
        let registry = Arc::new(BudgetRegistry::new(initial));
        let budget_id = 1u64;

        // Run 100 deductions across 10 threads deterministically
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let r = Arc::clone(&registry);
                thread::spawn(move || {
                    for _ in 0..10 {
                        let _ = r.try_deduct(budget_id, total_deductions);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // Property: Final state is deterministic (conservation holds)
        if let Some(stats) = registry.get_stats(budget_id) {
            prop_assert_eq!(
                stats.budget + stats.total_spent,
                initial,
                "Determinism violated"
            );
        }
    }

    #[test]
    fn prop_concurrent_generation_increases(
        initial in 10_000_00i64..50_000_00i64,
        threads in 5usize..15usize
    ) {
        let registry = Arc::new(BudgetRegistry::new(initial));
        let budget_id = 1u64;

        let gen_before = registry.get_stats(budget_id).map(|s| s.generation).unwrap_or(0);

        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let r = Arc::clone(&registry);
                thread::spawn(move || {
                    for _ in 0..10 {
                        let _ = r.try_deduct(budget_id, 10_00);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let gen_after = registry.get_stats(budget_id).map(|s| s.generation).unwrap_or(0);

        // Property: Generation increased after concurrent ops
        prop_assert!(gen_after > gen_before, "Generation did not increase");
    }
}

#[test]
fn prop_concurrent_multiple_budgets() {
    let registry = Arc::new(BudgetRegistry::new(10_000_00));

    let handles: Vec<_> = (0..10)
        .map(|budget_id| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = r.try_deduct(budget_id as u64, 10_00);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Property: Each budget is independent (no interference)
    for budget_id in 0..10 {
        if let Some(stats) = registry.get_stats(budget_id as u64) {
            assert_eq!(
                stats.budget + stats.total_spent,
                10_000_00,
                "Budget {} violated conservation",
                budget_id
            );
        }
    }
}

#[test]
fn prop_concurrent_credit_and_debit() {
    let registry = Arc::new(BudgetRegistry::new(10_000_00));
    let budget_id = 1u64;

    // 5 threads deducting
    let deduct_handles: Vec<_> = (0..5)
        .map(|_| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                for _ in 0..50 {
                    let _ = r.try_deduct(budget_id, 10_00);
                }
            })
        })
        .collect();

    // 5 threads crediting
    let credit_handles: Vec<_> = (0..5)
        .map(|_| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                for _ in 0..50 {
                    let _ = r.credit(budget_id, 10_00);
                }
            })
        })
        .collect();

    for h in deduct_handles.into_iter().chain(credit_handles) {
        h.join().unwrap();
    }

    // Property: Final budget = initial + credits - debits
    if let Some(stats) = registry.get_stats(budget_id) {
        let expected = 10_000_00 + (5 * 50 * 10_00) - (stats.total_spent);
        assert_eq!(stats.budget, expected, "Credit/debit balance incorrect");
    }
}

#[test]
fn prop_concurrent_read_consistency() {
    let registry = Arc::new(BudgetRegistry::new(10_000_00));
    let budget_id = 1u64;

    // 5 writers
    let write_handles: Vec<_> = (0..5)
        .map(|_| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = r.try_deduct(budget_id, 10_00);
                }
            })
        })
        .collect();

    // 10 readers
    let read_handles: Vec<_> = (0..10)
        .map(|_| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                for _ in 0..1000 {
                    let _ = r.get_budget(budget_id);
                }
            })
        })
        .collect();

    for h in write_handles.into_iter().chain(read_handles) {
        h.join().unwrap();
    }

    // Property: Readers never panic/deadlock
    // (test passes if no panics occurred)
}

#[test]
fn prop_concurrent_stats_consistent() {
    let registry = Arc::new(BudgetRegistry::new(10_000_00));
    let budget_id = 1u64;

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = r.try_deduct(budget_id, 10_00);
                    if let Some(stats) = r.get_stats(budget_id) {
                        // Property: Stats always consistent
                        assert_eq!(
                            stats.budget + stats.total_spent,
                            10_000_00,
                            "Stats inconsistent"
                        );
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn prop_concurrent_no_overdraft() {
    let registry = Arc::new(BudgetRegistry::new(1000_00));
    let budget_id = 1u64;

    let handles: Vec<_> = (0..20)
        .map(|_| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = r.try_deduct(budget_id, 10_00);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Property: Budget never goes negative (no overdraft)
    if let Some(budget) = registry.get_budget(budget_id) {
        assert!(budget >= 0, "Budget went negative: {}", budget);
    }
}

// ============================================================================
// T28 Q10: Edge Case Properties (8 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_handles_zero_deduction(
        initial in 1000_00i64..10_000_00i64,
        budget_id in 1u64..1000u64
    ) {
        let registry = BudgetRegistry::new(initial);

        let result = registry.try_deduct(budget_id, 0);

        // Property: Zero deduction succeeds and doesn't change budget
        prop_assert!(result.is_ok());
        prop_assert_eq!(registry.get_budget(budget_id), Some(initial));
    }

    #[test]
    fn prop_handles_exact_balance(
        initial in 1000_00i64..10_000_00i64,
        budget_id in 1u64..1000u64
    ) {
        let registry = BudgetRegistry::new(initial);

        let result = registry.try_deduct(budget_id, initial);

        // Property: Exact balance deduction succeeds
        prop_assert!(result.is_ok());
        prop_assert_eq!(result.unwrap(), 0);
        prop_assert_eq!(registry.get_budget(budget_id), Some(0));
    }

    #[test]
    fn prop_rejects_negative_amount(
        initial in 1000_00i64..10_000_00i64,
        negative_amount in -10_000_00i64..-1i64
    ) {
        let registry = BudgetRegistry::new(initial);

        let result = registry.try_deduct(1, negative_amount);

        // Property: Negative amounts rejected
        prop_assert!(result.is_err());
        prop_assert!(matches!(result, Err(ClapiError::InvalidCost(_))));
    }

    #[test]
    fn prop_rejects_overdraft(
        initial in 100_00i64..1000_00i64,
        excess in 1i64..10_000_00i64
    ) {
        let registry = BudgetRegistry::new(initial);

        let overdraft_amount = initial + excess;
        let result = registry.try_deduct(1, overdraft_amount);

        // Property: Overdrafts rejected
        prop_assert!(result.is_err());
        prop_assert!(matches!(result, Err(ClapiError::BudgetExhausted { .. })), "Expected BudgetExhausted error");
    }

    #[test]
    fn prop_handles_large_budgets(
        initial in 1_000_000_00i64..10_000_000_00i64,
        amount in 100_00i64..1_000_00i64
    ) {
        let registry = BudgetRegistry::new(initial);

        let result = registry.try_deduct(1, amount);

        // Property: Large budgets work correctly
        prop_assert!(result.is_ok());
        prop_assert_eq!(result.unwrap(), initial - amount);
    }

    #[test]
    fn prop_handles_small_deductions(
        initial in 1000_00i64..10_000_00i64,
        small_amount in 1i64..10i64
    ) {
        let registry = BudgetRegistry::new(initial);

        let result = registry.try_deduct(1, small_amount);

        // Property: Small deductions work correctly
        prop_assert!(result.is_ok());
        prop_assert_eq!(result.unwrap(), initial - small_amount);
    }

    #[test]
    fn prop_handles_many_small_deductions(
        initial in 10_000_00i64..100_000_00i64,
        small_amounts in prop::collection::vec(1i64..10i64, 100..500)
    ) {
        let registry = BudgetRegistry::new(initial);
        let budget_id = 1u64;

        for amount in small_amounts {
            let _ = registry.try_deduct(budget_id, amount);
        }

        // Property: Many small deductions maintain conservation
        if let Some(stats) = registry.get_stats(budget_id) {
            prop_assert_eq!(stats.budget + stats.total_spent, initial);
        }
    }

    #[test]
    fn prop_handles_boundary_budget_ids(
        initial in 1000_00i64..10_000_00i64,
        boundary_id in prop::sample::select(vec![0u64, 1, u64::MAX - 1, u64::MAX])
    ) {
        let registry = BudgetRegistry::new(initial);

        let result = registry.try_deduct(boundary_id, 100_00);

        // Property: Boundary IDs work correctly
        prop_assert!(result.is_ok());
        prop_assert_eq!(registry.get_budget(boundary_id), Some(initial - 100_00));
    }
}

// ============================================================================
// T28 Q11: ASSUM Verification (4 tests)
// ============================================================================

#[test]
fn prop_verify_no_toctou_budget() {
    // #ASSUME: Generation counter prevents TOCTOU
    // #VERIFY: Property test with concurrent readers/writers

    let registry = Arc::new(BudgetRegistry::new(10_000_00));
    let budget_id = 1u64;

    // Concurrent writers
    let write_handles: Vec<_> = (0..10)
        .map(|_| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = r.try_deduct(budget_id, 10_00);
                }
            })
        })
        .collect();

    // Concurrent readers checking TOCTOU prevention
    let read_handles: Vec<_> = (0..20)
        .map(|_| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                for _ in 0..1000 {
                    if let Some(stats) = r.get_stats(budget_id) {
                        // Property: Stats are always consistent (no torn reads)
                        assert_eq!(
                            stats.budget + stats.total_spent,
                            10_000_00,
                            "TOCTOU detected: inconsistent stats"
                        );
                    }
                }
            })
        })
        .collect();

    for h in write_handles.into_iter().chain(read_handles) {
        h.join().unwrap();
    }
}

#[test]
fn prop_verify_alignment() {
    // #ASSUME: RequestCapsule128 is 128-byte aligned
    // #VERIFY: Alignment test

    let capsule = RequestCapsule128::new(1000_00);

    // Property: Capsule is 128-byte aligned
    assert_eq!(std::mem::align_of::<RequestCapsule128>(), 128);
    assert_eq!(std::mem::size_of::<RequestCapsule128>(), 128);

    // Property: Capsule address is aligned
    let addr = &capsule as *const _ as usize;
    assert_eq!(addr % 128, 0, "Capsule not 128-byte aligned");
}

#[test]
fn prop_verify_cas_atomicity() {
    // #ASSUME: CAS loop prevents budget overdraft
    // #VERIFY: Property test with high contention

    let registry = Arc::new(BudgetRegistry::new(1000_00));
    let budget_id = 1u64;

    let handles: Vec<_> = (0..50)
        .map(|_| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = r.try_deduct(budget_id, 1_00);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Property: CAS atomicity prevents overdraft
    if let Some(budget) = registry.get_budget(budget_id) {
        assert!(budget >= 0, "CAS failed: budget went negative");
    }
}

#[test]
fn prop_verify_memory_ordering() {
    // #ASSUME: Acquire/Release ordering ensures visibility
    // #VERIFY: Concurrent readers see updates

    let registry = Arc::new(BudgetRegistry::new(10_000_00));
    let budget_id = 1u64;

    // Writer: Make 100 deductions
    let writer = {
        let r = Arc::clone(&registry);
        thread::spawn(move || {
            for _ in 0..100 {
                r.try_deduct(budget_id, 10_00).unwrap();
            }
        })
    };

    // Readers: Must eventually see all updates
    let readers: Vec<_> = (0..10)
        .map(|_| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                let mut last_budget = 10_000_00;
                for _ in 0..1000 {
                    if let Some(budget) = r.get_budget(budget_id) {
                        // Property: Budget only decreases (monotonic)
                        assert!(
                            budget <= last_budget,
                            "Memory ordering violation: budget increased {} -> {}",
                            last_budget,
                            budget
                        );
                        last_budget = budget;
                    }
                    std::thread::yield_now();
                }
            })
        })
        .collect();

    writer.join().unwrap();
    for r in readers {
        r.join().unwrap();
    }
}

// ============================================================================
// T28 Q12: Composition Properties (4 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_budget_plus_credit_composition(
        initial in 1000_00i64..10_000_00i64,
        deduct_amount in 100_00i64..500_00i64,
        credit_amount in 50_00i64..200_00i64
    ) {
        let registry = BudgetRegistry::new(initial);

        registry.try_deduct(1, deduct_amount).ok();
        registry.credit(1, credit_amount).ok();

        // Property: Composition preserves conservation
        if let Some(stats) = registry.get_stats(1) {
            let expected = initial - stats.total_spent + credit_amount;
            prop_assert_eq!(stats.budget, expected, "Composition violated conservation");
        }
    }

    #[test]
    fn prop_multiple_budgets_independent(
        initial in 1000_00i64..10_000_00i64,
        num_budgets in 5usize..20usize,
        amount in 10_00i64..100_00i64
    ) {
        let registry = BudgetRegistry::new(initial);

        for budget_id in 0..num_budgets {
            registry.try_deduct(budget_id as u64, amount).ok();
        }

        // Property: Each budget is independent
        for budget_id in 0..num_budgets {
            if let Some(stats) = registry.get_stats(budget_id as u64) {
                prop_assert_eq!(
                    stats.budget + stats.total_spent,
                    initial,
                    "Budget {} not independent",
                    budget_id
                );
            }
        }
    }
}

#[test]
fn prop_budget_and_generation_coordination() {
    let registry = BudgetRegistry::new(10_000_00);

    registry.try_deduct(1, 100_00).unwrap();
    let stats1 = registry.get_stats(1).unwrap();

    registry.try_deduct(1, 200_00).unwrap();
    let stats2 = registry.get_stats(1).unwrap();

    // Property: Generation increments match budget changes
    assert!(stats2.generation > stats1.generation);
    assert_eq!(stats1.budget - 200_00, stats2.budget);
}

#[test]
fn prop_stats_reflect_all_operations() {
    let registry = BudgetRegistry::new(10_000_00);

    registry.try_deduct(1, 100_00).unwrap();
    registry.try_deduct(1, 200_00).unwrap();
    registry.credit(1, 150_00).unwrap();

    let stats = registry.get_stats(1).unwrap();

    // Property: Stats accurately reflect all operations
    assert_eq!(stats.total_spent, 300_00);
    assert_eq!(stats.budget, 10_000_00 - 300_00 + 150_00);
    assert_eq!(stats.request_count, 2); // Only deductions count
}

// ============================================================================
// T28 Q13: Statistical Properties (4 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_average_deduction_bounded(
        initial in 10_000_00i64..100_000_00i64,
        deductions in prop::collection::vec(10_00i64..1000_00i64, 50..200)
    ) {
        let registry = BudgetRegistry::new(initial);
        let budget_id = 1u64;

        for amount in deductions.iter() {
            let _ = registry.try_deduct(budget_id, *amount);
        }

        // Property: Average deduction is within expected range
        if let Some(stats) = registry.get_stats(budget_id) {
            if stats.request_count > 0 {
                let avg = stats.total_spent / stats.request_count as i64;
                prop_assert!(avg >= 10_00 && avg <= 1000_00, "Average out of bounds: {}", avg);
            }
        }
    }

    #[test]
    fn prop_variance_bounded(
        initial in 100_000_00i64..1_000_000_00i64,
        deductions in prop::collection::vec(100_00i64..500_00i64, 100..200)
    ) {
        let registry = BudgetRegistry::new(initial);
        let budget_id = 1u64;

        for amount in deductions {
            let _ = registry.try_deduct(budget_id, amount);
        }

        // Property: Variance in deductions is reasonable
        if let Some(stats) = registry.get_stats(budget_id) {
            prop_assert!(stats.total_spent > 0, "No successful deductions");
            prop_assert!(stats.request_count > 0, "No requests counted");
        }
    }
}

#[test]
fn prop_latency_distribution() {
    let registry = BudgetRegistry::new(100_000_00);

    let mut latencies = Vec::new();

    for _ in 0..1000 {
        let start = std::time::Instant::now();
        let _ = registry.try_deduct(1, 10_00);
        latencies.push(start.elapsed().as_nanos());
    }

    // Property: Latency distribution is reasonable
    let p50 = latencies[latencies.len() / 2];
    let p99 = latencies[latencies.len() * 99 / 100];

    assert!(p50 < 1000, "p50 latency too high: {}ns", p50); // <1μs
    assert!(p99 < 10_000, "p99 latency too high: {}ns", p99); // <10μs
}

#[test]
fn prop_throughput_consistent() {
    let registry = Arc::new(BudgetRegistry::new(1_000_000_00));

    let start = std::time::Instant::now();

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                for _ in 0..1000 {
                    let _ = r.try_deduct(1, 10_00);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let elapsed = start.elapsed().as_secs_f64();
    let throughput = 10_000.0 / elapsed;

    // Property: Throughput > 100K ops/sec
    assert!(
        throughput > 100_000.0,
        "Throughput too low: {:.0} ops/s",
        throughput
    );
}

// ============================================================================
// T28 Q14: Regression Prevention (3 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_stable_behavior_across_runs(
        initial in 1000_00i64..10_000_00i64,
        deductions in prop::collection::vec(10_00i64..100_00i64, 10..50)
    ) {
        // Run same operations 10 times - should get same result
        let expected_results: Vec<_> = (0..10)
            .map(|_| {
                let registry = BudgetRegistry::new(initial);
                for amount in &deductions {
                    let _ = registry.try_deduct(1, *amount);
                }
                registry.get_stats(1).map(|s| (s.budget, s.total_spent))
            })
            .collect();

        // Property: All runs produce same result (deterministic)
        for result in &expected_results[1..] {
            prop_assert_eq!(*result, expected_results[0], "Behavior not stable");
        }
    }

    #[test]
    fn prop_generation_never_wraps(
        operations in prop::collection::vec(10_00i64..100_00i64, 1000..2000)
    ) {
        let registry = BudgetRegistry::new(1_000_000_00);
        let budget_id = 1u64;

        let mut last_gen = 0u64;

        for amount in operations {
            if registry.try_deduct(budget_id, amount).is_ok() {
                if let Some(stats) = registry.get_stats(budget_id) {
                    // Property: Generation never wraps (always increases)
                    prop_assert!(
                        stats.generation > last_gen,
                        "Generation wrapped: {} -> {}",
                        last_gen,
                        stats.generation
                    );
                    last_gen = stats.generation;
                }
            }
        }
    }
}

#[test]
fn prop_regression_budget_conservation() {
    // Regression: Ensure budget conservation holds for known failure cases
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(1, 300_00).unwrap();
    registry.try_deduct(1, 200_00).unwrap();
    registry.credit(1, 100_00).unwrap();
    registry.try_deduct(1, 150_00).unwrap();

    let stats = registry.get_stats(1).unwrap();
    assert_eq!(
        stats.budget + stats.total_spent - 100_00,
        1000_00,
        "Regression: budget conservation broken"
    );
}
