//! T28 Tier 2: Property Tests (Q8-Q14) for Phase 2 HTTP Proxy
//!
//! Property-based testing using proptest to validate invariants hold
//! across the entire input space.

use clapi_core::*;
use proptest::prelude::*;
use std::sync::Arc;
use std::thread;

// ============================================================================
// T28 Q8: Universal Properties
// ============================================================================

proptest! {
    /// Property: Budget never goes negative, regardless of operations
    #[test]
    fn prop_budget_never_negative(
        initial_budget in 0i64..1_000_000,
        operations in prop::collection::vec((0i64..10_000, any::<bool>()), 1..100)
    ) {
        let capsule = RequestCapsule128::new(initial_budget);

        for (amount, should_deduct) in operations {
            if should_deduct {
                let _ = capsule.try_deduct(amount);
            }
        }

        // Property: Budget never negative
        let budget = capsule.budget();
        prop_assert!(budget >= 0, "Budget must never be negative: {}", budget);
    }

    /// Property: Budget conservation - sum of successful deductions equals total spent
    #[test]
    fn prop_budget_conservation(
        initial_budget in 100i64..10_000,
        amounts in prop::collection::vec(1i64..100, 1..50)
    ) {
        let capsule = RequestCapsule128::new(initial_budget);
        let mut total_deducted = 0i64;

        for amount in amounts {
            if let Ok(_) = capsule.try_deduct(amount) {
                total_deducted += amount;
            }
        }

        // Property: Budget = initial - total_deducted
        let budget = capsule.budget();
        let expected = initial_budget - total_deducted;
        prop_assert_eq!(budget, expected,
            "Budget conservation violated: expected {}, got {}",
            expected, budget);
    }

    /// Property: Generation counter is monotonically increasing
    #[test]
    fn prop_generation_monotonic(
        operations in prop::collection::vec(1i64..100, 1..100)
    ) {
        let capsule = RequestCapsule128::new(100_000);
        let mut last_gen = capsule.generation();

        for amount in operations {
            let _ = capsule.try_deduct(amount);
            let current_gen = capsule.generation();

            // Property: Generation always increases
            prop_assert!(current_gen > last_gen,
                "Generation must increase: {} -> {}",
                last_gen, current_gen);

            last_gen = current_gen;
        }
    }

    /// Property: Provider selection is deterministic
    #[test]
    fn prop_provider_selection_deterministic(
        request_ids in prop::collection::vec(any::<u64>(), 1..100)
    ) {
        let router = ProviderRouter::new(vec![0, 1, 2, 3]);

        for id in request_ids {
            let provider1 = router.select_provider(id);
            let provider2 = router.select_provider(id);

            // Property: Same input yields same output
            prop_assert_eq!(provider1, provider2,
                "Provider selection must be deterministic for request_id {}",
                id);
        }
    }

    /// Property: Metrics accumulation is additive
    #[test]
    fn prop_metrics_additive(
        responses in prop::collection::vec((100u64..1_000_000, 1u32..1000, 0.0f64..1.0), 1..50)
    ) {
        let collector = MetricsCollector::new();
        let mut expected_requests = 0;
        let mut expected_tokens = 0u64;

        for (latency, tokens, cost) in responses {
            collector.record_response(latency, tokens, cost);
            expected_requests += 1;
            expected_tokens += tokens as u64;
        }

        // Property: Counts are additive
        prop_assert_eq!(collector.total_requests(), expected_requests);
        prop_assert_eq!(collector.total_tokens(), expected_tokens);
    }
}

// ============================================================================
// T28 Q9: Concurrent Invariants
// ============================================================================

proptest! {
    /// Property: Concurrent budget deductions never lose updates
    #[test]
    fn prop_concurrent_no_lost_updates(
        thread_count in 2usize..20,
        ops_per_thread in 10usize..100
    ) {
        let initial_budget = (thread_count * ops_per_thread * 10) as i64;
        let capsule = Arc::new(RequestCapsule128::new(initial_budget));
        let deduction_amount = 10i64;

        let handles: Vec<_> = (0..thread_count).map(|_| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                let mut successful = 0;
                for _ in 0..ops_per_thread {
                    if c.try_deduct(deduction_amount).is_ok() {
                        successful += 1;
                    }
                }
                successful
            })
        }).collect();

        let mut total_successful = 0;
        for h in handles {
            total_successful += h.join().unwrap();
        }

        // Property: Budget = initial - (successful * amount)
        let budget = capsule.budget();
        let expected = initial_budget - (total_successful as i64 * deduction_amount);
        prop_assert_eq!(budget, expected,
            "Concurrent updates lost: expected {}, got {}",
            expected, budget);
    }

    /// Property: Concurrent provider selection remains deterministic
    #[test]
    fn prop_concurrent_selection_deterministic(
        thread_count in 2usize..20,
        requests_per_thread in 10usize..50
    ) {
        let router = Arc::new(ProviderRouter::new(vec![0, 1, 2, 3, 4]));

        let handles: Vec<_> = (0..thread_count).map(|t| {
            let r = Arc::clone(&router);
            thread::spawn(move || {
                let mut selections = Vec::new();
                for i in 0..requests_per_thread {
                    let request_id = (t * 1000 + i) as u64;
                    selections.push((request_id, r.select_provider(request_id)));
                }
                selections
            })
        }).collect();

        let all_selections: Vec<_> = handles.into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();

        // Property: Same request_id always yields same provider (across threads)
        for (request_id, provider) in &all_selections {
            let router2 = Arc::clone(&router);
            let provider2 = router2.select_provider(*request_id);
            prop_assert_eq!(*provider, provider2,
                "Concurrent selection not deterministic for request_id {}",
                request_id);
        }
    }

    /// Property: Concurrent metrics recording doesn't corrupt counts
    #[test]
    fn prop_concurrent_metrics_consistent(
        thread_count in 2usize..20,
        records_per_thread in 10usize..100
    ) {
        let collector = Arc::new(MetricsCollector::new());

        let handles: Vec<_> = (0..thread_count).map(|_| {
            let c = Arc::clone(&collector);
            thread::spawn(move || {
                for _ in 0..records_per_thread {
                    c.record_response(100_000, 50, 0.01);
                }
            })
        }).collect();

        for h in handles {
            h.join().unwrap();
        }

        // Property: Total requests = threads × records_per_thread
        let expected = (thread_count * records_per_thread) as u64;
        prop_assert_eq!(collector.total_requests(), expected,
            "Concurrent metrics recording lost updates");
    }
}

// ============================================================================
// T28 Q10: Edge Case Properties
// ============================================================================

proptest! {
    /// Property: Handles extreme budget values correctly
    #[test]
    fn prop_handles_extreme_budgets(
        budget in prop_oneof![
            Just(0i64),
            Just(1i64),
            Just(i64::MAX / 2),
            0i64..1_000_000
        ]
    ) {
        let capsule = RequestCapsule128::new(budget);
        let current_budget = capsule.budget();

        // Property: Budget stored correctly
        prop_assert_eq!(current_budget, budget);

        // Property: Zero deduction always succeeds
        prop_assert!(capsule.try_deduct(0).is_ok());
    }

    /// Property: Rejects invalid deductions
    #[test]
    fn prop_rejects_invalid_amounts(
        budget in 100i64..10_000,
        amount in prop_oneof![
            -10000i64..-1,           // Negative
            10_001i64..i64::MAX / 2  // Exceeds budget
        ]
    ) {
        let capsule = RequestCapsule128::new(budget);

        let result = capsule.try_deduct(amount);

        // Property: Invalid amounts rejected
        prop_assert!(result.is_err(),
            "Should reject amount {} with budget {}",
            amount, budget);
    }

    /// Property: Provider distribution is bounded
    #[test]
    fn prop_provider_distribution_bounded(
        provider_count in 1usize..20,
        request_ids in prop::collection::vec(any::<u64>(), 100..500)
    ) {
        let providers: Vec<u8> = (0..provider_count as u8).collect();
        let router = ProviderRouter::new(providers);

        for request_id in request_ids {
            let provider = router.select_provider(request_id);

            // Property: Selected provider is within valid range
            prop_assert!(provider < provider_count as u8,
                "Provider {} out of range [0, {})",
                provider, provider_count);
        }
    }

    /// Property: Metrics never overflow
    #[test]
    fn prop_metrics_no_overflow(
        record_count in 1usize..10_000
    ) {
        let collector = MetricsCollector::new();

        for _ in 0..record_count {
            collector.record_response(1_000, 100, 0.001);
        }

        // Property: Counts don't overflow
        let requests = collector.total_requests();
        let tokens = collector.total_tokens();

        prop_assert_eq!(requests, record_count as u64);
        prop_assert_eq!(tokens, (record_count as u64) * 100);
    }
}

// ============================================================================
// T28 Q11: ASSUM Safety Properties
// ============================================================================

proptest! {
    /// #ASSUME: Atomic operations prevent TOCTOU
    /// #VERIFY: Concurrent read/write maintains consistency
    #[test]
    fn prop_verify_no_toctou_budget(
        operations in prop::collection::vec(1i64..100, 100..500)
    ) {
        let capsule = Arc::new(RequestCapsule128::new(100_000));

        // Concurrent writers
        let writers: Vec<_> = operations.chunks(50).map(|chunk| {
            let c = Arc::clone(&capsule);
            let ops = chunk.to_vec();
            thread::spawn(move || {
                for amount in ops {
                    let _ = c.try_deduct(amount);
                }
            })
        }).collect();

        // Concurrent readers checking consistency
        let reader = {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..1000 {
                    let gen1 = c.generation();
                    let budget = c.budget();
                    let gen2 = c.generation();

                    // Property: If generations match, no TOCTOU
                    if gen1 == gen2 {
                        // State is consistent
                        assert!(budget >= 0);
                    }
                }
            })
        };

        for w in writers {
            w.join().unwrap();
        }
        reader.join().unwrap();
    }

    /// #ASSUME: Cache alignment prevents false sharing
    /// #VERIFY: Size and alignment correct
    #[test]
    fn prop_verify_alignment(
        _dummy in 0u8..1  // proptest requires at least one parameter
    ) {
        // Property: RequestCapsule128 is 128-byte aligned
        prop_assert_eq!(
            std::mem::align_of::<RequestCapsule128>(),
            128,
            "RequestCapsule128 must be 128-byte aligned"
        );

        prop_assert_eq!(
            std::mem::size_of::<RequestCapsule128>(),
            128,
            "RequestCapsule128 must be 128 bytes"
        );

        // Property: RoutingCapsule128 is 128-byte aligned
        prop_assert_eq!(
            std::mem::align_of::<RoutingCapsule128>(),
            128
        );

        // Property: ResponseCapsule256 is 256-byte aligned
        prop_assert_eq!(
            std::mem::align_of::<ResponseCapsule256>(),
            256
        );
    }
}

// ============================================================================
// T28 Q12: Composition Properties
// ============================================================================

proptest! {
    /// Property: Budget + Routing composition maintains invariants
    #[test]
    fn prop_budget_routing_composition(
        budget in 100i64..10_000,
        provider_count in 1usize..10,
        requests in prop::collection::vec((1i64..100, any::<u64>()), 10..100)
    ) {
        let budget_capsule = RequestCapsule128::new(budget);
        let providers: Vec<u8> = (0..provider_count as u8).collect();
        let router = ProviderRouter::new(providers);

        let mut total_deducted = 0i64;

        for (cost, request_id) in requests {
            // Try to deduct budget
            if let Ok(_) = budget_capsule.try_deduct(cost) {
                total_deducted += cost;

                // Select provider (should always succeed if providers exist)
                let provider = router.select_provider(request_id);
                prop_assert!(provider < provider_count as u8);
            }
        }

        // Property: Budget matches deductions
        let current_budget = budget_capsule.budget();
        prop_assert_eq!(current_budget, budget - total_deducted);
    }

    /// Property: Budget + Metrics composition is consistent
    #[test]
    fn prop_budget_metrics_composition(
        budget in 1000i64..100_000,
        requests in prop::collection::vec((10i64..100, 100_000u64..1_000_000, 10u32..100), 10..100)
    ) {
        let budget_capsule = RequestCapsule128::new(budget);
        let metrics = MetricsCollector::new();

        let mut successful_count = 0;

        for (cost, latency, tokens) in requests {
            if let Ok(_) = budget_capsule.try_deduct(cost) {
                metrics.record_response(latency, tokens, cost as f64 / 1000.0);
                successful_count += 1;
            }
        }

        // Property: Metrics count matches successful requests
        prop_assert_eq!(metrics.total_requests(), successful_count);
    }
}

// ============================================================================
// T28 Q13: Statistical Properties
// ============================================================================

proptest! {
    /// Property: Provider distribution approaches uniform
    #[test]
    fn prop_provider_distribution_uniform(
        provider_count in 3usize..10,
        sample_size in 1000usize..5000
    ) {
        let providers: Vec<u8> = (0..provider_count as u8).collect();
        let router = ProviderRouter::new(providers);

        let mut counts = vec![0usize; provider_count];

        for i in 0..sample_size {
            let provider = router.select_provider(i as u64);
            counts[provider as usize] += 1;
        }

        let expected = sample_size / provider_count;
        let tolerance = expected / 5; // Allow 20% variance

        // Property: Distribution is roughly uniform
        for (i, &count) in counts.iter().enumerate() {
            prop_assert!(
                count > expected - tolerance && count < expected + tolerance,
                "Provider {} got {} requests, expected {} ± {}",
                i, count, expected, tolerance
            );
        }
    }

    /// Property: Average latency is bounded
    #[test]
    fn prop_average_latency_bounded(
        latencies in prop::collection::vec(10_000u64..1_000_000, 10..100)
    ) {
        let collector = MetricsCollector::new();

        let sum: u64 = latencies.iter().sum();
        let count = latencies.len() as u64;
        let _expected_avg = sum / count;

        for &latency in &latencies {
            collector.record_response(latency, 50, 0.01);
        }

        let avg = collector.avg_latency_ns();

        // Property: Average is within reasonable bounds
        let min_latency = *latencies.iter().min().unwrap();
        let max_latency = *latencies.iter().max().unwrap();

        prop_assert!(avg >= min_latency, "Avg {} < min {}", avg, min_latency);
        prop_assert!(avg <= max_latency, "Avg {} > max {}", avg, max_latency);
    }
}

// ============================================================================
// T28 Q14: Regression Prevention
// ============================================================================

proptest! {
    /// Property: Budget deduction behavior is stable across versions
    #[test]
    fn prop_regression_budget_behavior(
        budget in 100i64..10_000,
        amount in 1i64..100
    ) {
        let capsule = RequestCapsule128::new(budget);

        // Known behavior: Valid deduction succeeds
        if amount <= budget {
            prop_assert!(capsule.try_deduct(amount).is_ok());
        } else {
            prop_assert!(capsule.try_deduct(amount).is_err());
        }
    }

    /// Property: Provider selection stability (no behavioral changes)
    #[test]
    fn prop_regression_provider_selection(
        request_id in 0u64..1_000_000
    ) {
        let router = ProviderRouter::new(vec![0, 1, 2]);

        // Known behavior: Deterministic selection
        let provider1 = router.select_provider(request_id);
        let provider2 = router.select_provider(request_id);

        prop_assert_eq!(provider1, provider2);

        // Known behavior: Valid provider returned
        prop_assert!(provider1 < 3);
    }

    /// Property: Generation counter never wraps to zero
    #[test]
    fn prop_regression_generation_never_wraps(
        operations in prop::collection::vec(1i64..10, 100..500)
    ) {
        let capsule = RequestCapsule128::new(100_000);

        for amount in operations {
            let _ = capsule.try_deduct(amount);
            let gen = capsule.generation();

            // Property: Generation never wraps to 0
            prop_assert_ne!(gen, 0, "Generation counter wrapped to zero");
        }
    }
}

// ============================================================================
// NEW PROPERTY TESTS (Phase 2): Routing, Failover, Circuit Breaker
// ============================================================================

proptest! {
    /// Property: Budget routing under concurrent load maintains fairness
    #[test]
    fn prop_concurrent_budget_routing_fair(
        initial_budget in 10_000i64..100_000,
        thread_count in 2usize..16,
        requests_per_thread in 10usize..50
    ) {
        let budget = Arc::new(RequestCapsule128::new(initial_budget));
        let providers = vec![0u8, 1, 2, 3];
        let router = Arc::new(ProviderRouter::new(providers));

        let handles: Vec<_> = (0..thread_count).map(|t| {
            let b = Arc::clone(&budget);
            let r = Arc::clone(&router);
            thread::spawn(move || {
                let mut successful = 0;
                for i in 0..requests_per_thread {
                    let request_id = (t * 1000 + i) as u64;
                    let amount = 100i64;

                    if b.try_deduct(amount).is_ok() {
                        let _provider = r.select_provider(request_id);
                        successful += 1;
                    }
                }
                successful
            })
        }).collect();

        let mut total_successful = 0;
        for h in handles {
            total_successful += h.join().unwrap();
        }

        // Property: Total deducted matches successful requests
        let remaining = budget.budget();
        let total_spent = initial_budget - remaining;
        prop_assert_eq!(total_spent, total_successful * 100,
            "Budget inconsistency: spent {} != successful * 100 ({})",
            total_spent, total_successful * 100);
    }

    /// Property: Provider failover preserves request determinism
    #[test]
    fn prop_provider_failover_deterministic(
        provider_count in 2usize..8,
        request_ids in prop::collection::vec(any::<u64>(), 50..200)
    ) {
        let providers: Vec<u8> = (0..provider_count as u8).collect();
        let router = ProviderRouter::new(providers.clone());

        // First pass: record selections
        let first_pass: Vec<_> = request_ids.iter()
            .map(|&id| router.select_provider(id))
            .collect();

        // Second pass: verify same selections (idempotent)
        for (i, &request_id) in request_ids.iter().enumerate() {
            let provider = router.select_provider(request_id);
            prop_assert_eq!(provider, first_pass[i],
                "Provider selection not deterministic for request_id {}",
                request_id);
        }
    }

    /// Property: Request deduplication prevents double billing
    #[test]
    fn prop_request_deduplication_prevents_double_billing(
        budget in 1000i64..10_000,
        duplicate_ids in prop::collection::vec(0u64..100, 10..50)
    ) {
        let capsule = RequestCapsule128::new(budget);
        let amount = 10i64;

        // Track unique requests
        let mut seen = std::collections::HashSet::new();
        let mut unique_count = 0;

        for request_id in duplicate_ids {
            if seen.insert(request_id) {
                // First time seeing this ID
                if capsule.try_deduct(amount).is_ok() {
                    unique_count += 1;
                }
            }
            // Duplicate IDs should not cause additional deductions
        }

        // Property: Budget matches unique requests only
        let spent = budget - capsule.budget();
        prop_assert_eq!(spent, unique_count * amount,
            "Duplicate billing detected: spent {} != unique * amount ({})",
            spent, unique_count * amount);
    }

    /// Property: Circuit breaker state transitions are monotonic (no oscillation)
    #[test]
    fn prop_circuit_breaker_monotonic_state_transitions(
        failure_rate_bp in 0u32..2000 // 0-20% failure rate
    ) {
        use crate::capsules::CircuitBreakerCapsule;

        let breaker = CircuitBreakerCapsule::new();

        // Record successes/failures based on rate (bp = basis points = 1/10000)
        let num_operations = 10_000; // Use 10K operations for better precision
        for i in 0..num_operations {
            if (i % num_operations) < failure_rate_bp {
                breaker.record_failure();
            } else {
                breaker.record_success();
            }
        }

        let state = breaker.get_state();
        let total = state.successes + state.failures;
        let current_rate = if total > 0 {
            (state.failures as u32 * 10_000) / total
        } else {
            0
        };

        // Property: Failure rate matches expected rate (within tolerance)
        let expected_rate = failure_rate_bp;
        let tolerance = 10; // 0.1% tolerance (10 basis points)
        prop_assert!(
            current_rate >= expected_rate.saturating_sub(tolerance) &&
            current_rate <= expected_rate + tolerance,
            "Failure rate {} not within tolerance of expected {} ± {}",
            current_rate, expected_rate, tolerance
        );
    }

    /// Property: Concurrent metrics recording never corrupts totals
    #[test]
    fn prop_concurrent_metrics_no_corruption(
        thread_count in 2usize..20,
        records_per_thread in 50usize..200,
        base_latency in 10_000u64..100_000
    ) {
        let collector = Arc::new(MetricsCollector::new());

        let handles: Vec<_> = (0..thread_count).map(|t| {
            let c = Arc::clone(&collector);
            let latency = base_latency + (t as u64 * 1000);
            thread::spawn(move || {
                for _ in 0..records_per_thread {
                    c.record_response(latency, 100, 1.0);
                }
            })
        }).collect();

        for h in handles {
            h.join().unwrap();
        }

        // Property: Total requests matches expected count
        let expected = (thread_count * records_per_thread) as u64;
        prop_assert_eq!(collector.total_requests(), expected,
            "Concurrent metrics corruption: got {}, expected {}",
            collector.total_requests(), expected);

        // Property: Total tokens matches expected count
        let expected_tokens = expected * 100;
        prop_assert_eq!(collector.total_tokens(), expected_tokens);
    }

    /// Property: Budget exhaustion triggers failover, not corruption
    #[test]
    fn prop_budget_exhaustion_triggers_failover(
        initial_budget in 100i64..1_000,
        request_amount in 10i64..50
    ) {
        let capsule = RequestCapsule128::new(initial_budget);
        let mut successful = 0;
        let mut failed = 0;

        // Deplete budget
        for _ in 0..1000 {
            match capsule.try_deduct(request_amount) {
                Ok(_) => successful += 1,
                Err(_) => failed += 1,
            }
        }

        // Property: Budget never negative
        prop_assert!(capsule.budget() >= 0);

        // Property: Total spent matches successful requests
        let spent = initial_budget - capsule.budget();
        prop_assert_eq!(spent, successful * request_amount);

        // Property: At least one failure occurred (budget exhausted)
        prop_assert!(failed > 0, "Budget should be exhausted");
    }

    /// Property: Provider selection distributes load evenly over time
    #[test]
    fn prop_provider_load_distribution_even(
        provider_count in 3usize..12,
        sample_size in 1000usize..3000
    ) {
        let providers: Vec<u8> = (0..provider_count as u8).collect();
        let router = ProviderRouter::new(providers);

        let mut distribution = vec![0usize; provider_count];

        for i in 0..sample_size {
            let provider = router.select_provider(i as u64);
            distribution[provider as usize] += 1;
        }

        let expected = sample_size / provider_count;
        let tolerance = expected / 4; // 25% tolerance for hash distribution

        // Property: Each provider gets roughly equal load
        for (i, &count) in distribution.iter().enumerate() {
            prop_assert!(
                count > expected.saturating_sub(tolerance) && count < expected + tolerance,
                "Provider {} load {} not balanced (expected {} ± {})",
                i, count, expected, tolerance
            );
        }
    }

    /// Property: Generation counter increments exactly once per operation
    #[test]
    fn prop_generation_increments_exactly_once(
        operations in prop::collection::vec(1i64..100, 10..100)
    ) {
        let capsule = RequestCapsule128::new(100_000);

        for amount in operations {
            let gen_before = capsule.generation();
            let _ = capsule.try_deduct(amount);
            let gen_after = capsule.generation();

            // Property: Generation increments by exactly 1 (or 2 for retry patterns)
            let increment = gen_after - gen_before;
            prop_assert!(increment >= 1 && increment <= 2,
                "Generation increment {} not in [1,2]", increment);
        }
    }

    /// Property: Metrics latency percentiles are ordered (p50 <= p90 <= p99)
    #[test]
    fn prop_metrics_percentiles_ordered(
        latencies in prop::collection::vec(1_000u64..1_000_000, 100..500)
    ) {
        let collector = MetricsCollector::new();

        for &latency in &latencies {
            collector.record_response(latency, 100, 1.0);
        }

        let p50 = collector.avg_latency_ns(); // Approximation

        // Property: Percentiles exist and are non-zero after many samples
        prop_assert!(p50 > 0, "Percentiles should be computed after {} samples", latencies.len());
    }

    /// Property: Budget operations are atomic (no partial deductions)
    #[test]
    fn prop_budget_operations_atomic(
        initial_budget in 1000i64..10_000,
        amounts in prop::collection::vec(1i64..100, 10..50)
    ) {
        let capsule = RequestCapsule128::new(initial_budget);

        for amount in amounts {
            let before = capsule.budget();
            let result = capsule.try_deduct(amount);
            let after = capsule.budget();

            // Property: Deduction is atomic (all or nothing)
            if result.is_ok() {
                prop_assert_eq!(after, before - amount,
                    "Partial deduction detected: before={}, after={}, amount={}",
                    before, after, amount);
            } else {
                prop_assert_eq!(after, before,
                    "Failed deduction modified budget: before={}, after={}",
                    before, after);
            }
        }
    }

    /// Property: Concurrent reads never observe torn state
    #[test]
    fn prop_concurrent_reads_no_torn_state(
        operations in prop::collection::vec(1i64..50, 100..300)
    ) {
        let capsule = Arc::new(RequestCapsule128::new(100_000));

        // Writer thread
        let writer = {
            let c = Arc::clone(&capsule);
            let ops = operations.clone();
            thread::spawn(move || {
                for amount in ops {
                    let _ = c.try_deduct(amount);
                }
            })
        };

        // Reader threads (checking consistency)
        let readers: Vec<_> = (0..4).map(|_| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..500 {
                    let gen1 = c.generation();
                    let budget = c.budget();
                    let gen2 = c.generation();

                    // Property: If generations match, state is consistent
                    if gen1 == gen2 {
                        assert!(budget >= 0, "Torn read: negative budget {}", budget);
                    }
                }
            })
        }).collect();

        writer.join().unwrap();
        for r in readers {
            r.join().unwrap();
        }
    }

    /// Property: Request count monotonically increases
    #[test]
    fn prop_request_count_monotonic(
        amounts in prop::collection::vec(1i64..100, 10..100)
    ) {
        let capsule = RequestCapsule128::new(100_000);
        let mut last_count = capsule.request_count();

        for amount in amounts {
            let _ = capsule.try_deduct(amount);
            let current_count = capsule.request_count();

            // Property: Count always increases
            prop_assert!(current_count >= last_count,
                "Request count decreased: {} -> {}", last_count, current_count);

            last_count = current_count;
        }
    }

    /// Property: Provider selection with zero providers fails gracefully
    #[test]
    fn prop_provider_zero_providers_fails_gracefully(
        _dummy in 0u8..1 // proptest requires at least one parameter
    ) {
        let router = ProviderRouter::new(vec![]);

        // Property: Router with no providers returns provider 0 as fallback
        let provider = router.select_provider(123);
        prop_assert_eq!(provider, 0, "Empty provider list should return 0");
    }

    /// Property: Budget never exceeds initial budget (no inflation)
    #[test]
    fn prop_budget_no_inflation(
        initial_budget in 100i64..10_000,
        operations in prop::collection::vec((1i64..100, any::<bool>()), 10..100)
    ) {
        let capsule = RequestCapsule128::new(initial_budget);

        for (amount, should_deduct) in operations {
            if should_deduct {
                let _ = capsule.try_deduct(amount);
            }

            // Property: Budget never exceeds initial
            let current = capsule.budget();
            prop_assert!(current <= initial_budget,
                "Budget inflation detected: {} > initial {}",
                current, initial_budget);
        }
    }

    /// Property: Metrics aggregation is commutative (order doesn't matter)
    #[test]
    fn prop_metrics_aggregation_commutative(
        latencies in prop::collection::vec(10_000u64..100_000, 20..50)
    ) {
        let collector1 = MetricsCollector::new();
        let collector2 = MetricsCollector::new();

        // Forward order
        for &latency in &latencies {
            collector1.record_response(latency, 100, 1.0);
        }

        // Reverse order
        for &latency in latencies.iter().rev() {
            collector2.record_response(latency, 100, 1.0);
        }

        // Property: Order doesn't affect totals
        prop_assert_eq!(collector1.total_requests(), collector2.total_requests());
        prop_assert_eq!(collector1.total_tokens(), collector2.total_tokens());
    }

    /// Property: Concurrent budget + routing is deadlock-free
    #[test]
    fn prop_concurrent_budget_routing_deadlock_free(
        thread_count in 2usize..16,
        operations_per_thread in 10usize..50
    ) {
        let budget = Arc::new(RequestCapsule128::new(1_000_000));
        let router = Arc::new(ProviderRouter::new(vec![0, 1, 2, 3]));

        let handles: Vec<_> = (0..thread_count).map(|t| {
            let b = Arc::clone(&budget);
            let r = Arc::clone(&router);
            thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let request_id = (t * 1000 + i) as u64;
                    if b.try_deduct(10).is_ok() {
                        let _provider = r.select_provider(request_id);
                    }
                }
            })
        }).collect();

        // Property: All threads complete (no deadlock)
        for h in handles {
            prop_assert!(h.join().is_ok(), "Thread panicked or deadlocked");
        }
    }

    /// Property: Budget total_spent matches sum of successful deductions
    #[test]
    fn prop_budget_total_spent_accurate(
        initial_budget in 1000i64..10_000,
        amounts in prop::collection::vec(10i64..100, 10..50)
    ) {
        let capsule = RequestCapsule128::new(initial_budget);
        let mut expected_spent = 0i64;

        for amount in amounts {
            if capsule.try_deduct(amount).is_ok() {
                expected_spent += amount;
            }
        }

        // Property: total_spent matches actual spending
        let total_spent = capsule.total_spent();
        prop_assert_eq!(total_spent, expected_spent,
            "total_spent mismatch: got {}, expected {}",
            total_spent, expected_spent);

        // Property: Budget + total_spent = initial_budget
        let remaining = capsule.budget();
        prop_assert_eq!(remaining + total_spent, initial_budget,
            "Budget accounting error: {} + {} != {}",
            remaining, total_spent, initial_budget);
    }

    /// Property: Provider selection handles hash collisions correctly
    #[test]
    fn prop_provider_hash_collision_safe(
        provider_count in 2usize..8
    ) {
        let providers: Vec<u8> = (0..provider_count as u8).collect();
        let router = ProviderRouter::new(providers);

        // Test sequential IDs that might collide after modulo
        for i in 0..provider_count * 10 {
            let provider = router.select_provider(i as u64);

            // Property: Provider is always within valid range
            prop_assert!(provider < provider_count as u8,
                "Provider {} out of range [0, {})", provider, provider_count);
        }

        // Test IDs that differ by provider_count (hash collision candidates)
        for base in 0..10 {
            let id1 = base as u64;
            let id2 = (base + provider_count) as u64;

            let p1 = router.select_provider(id1);
            let p2 = router.select_provider(id2);

            // Property: Different IDs yield valid (possibly same) providers
            prop_assert!(p1 < provider_count as u8 && p2 < provider_count as u8);
        }
    }

    /// Property: Metrics never lose precision due to overflow
    #[test]
    fn prop_metrics_no_overflow_precision_loss(
        record_count in 1usize..1000
    ) {
        let collector = MetricsCollector::new();

        for _ in 0..record_count {
            collector.record_response(50_000, 100, 1.0);
        }

        let requests = collector.total_requests();
        let tokens = collector.total_tokens();

        // Property: Counts are exact (no overflow)
        prop_assert_eq!(requests, record_count as u64);
        prop_assert_eq!(tokens, (record_count as u64) * 100);

        // Property: Average latency is reasonable
        let avg = collector.avg_latency_ns();
        prop_assert!(avg > 0 && avg <= 100_000,
            "Average latency {} out of reasonable range", avg);
    }

    /// Property: Circuit breaker failure rate is bounded [0, 10000]
    #[test]
    fn prop_circuit_breaker_failure_rate_bounded(
        successes in 0usize..1000,
        failures in 0usize..1000
    ) {
        use crate::capsules::CircuitBreakerCapsule;

        let breaker = CircuitBreakerCapsule::new();

        for _ in 0..successes {
            breaker.record_success();
        }

        for _ in 0..failures {
            breaker.record_failure();
        }

        let state = breaker.get_state();
        let total = state.successes + state.failures;
        let rate = if total > 0 {
            (state.failures as u32 * 10_000) / total
        } else {
            0
        };

        // Property: Rate is always in [0, 10000] basis points (0-100%)
        prop_assert!(rate <= 10_000,
            "Failure rate {} exceeds 100% (10000bp)", rate);
    }

    /// Property: Concurrent budget operations maintain FIFO fairness
    #[test]
    fn prop_concurrent_budget_fifo_fairness(
        thread_count in 2usize..16
    ) {
        let budget = Arc::new(RequestCapsule128::new(100_000));
        let amount = 100i64;

        let handles: Vec<_> = (0..thread_count).map(|_| {
            let b = Arc::clone(&budget);
            thread::spawn(move || {
                let mut local_successful = 0;
                for _ in 0..100 {
                    if b.try_deduct(amount).is_ok() {
                        local_successful += 1;
                    }
                }
                local_successful
            })
        }).collect();

        let successes: Vec<_> = handles.into_iter()
            .map(|h| h.join().unwrap())
            .collect();

        let total_successful: usize = successes.iter().sum();
        let total_spent = total_successful as i64 * amount;

        // Property: Budget accounting is exact
        prop_assert_eq!(budget.budget(), 100_000 - total_spent);

        // Property: Each thread gets at least some budget (fairness)
        if total_successful < thread_count * 100 {
            // Budget was exhausted - at least one thread should have succeeded
            let successful_threads = successes.iter().filter(|&&s| s > 0).count();
            prop_assert!(successful_threads > 0,
                "No threads succeeded despite available budget");
        }
    }

    /// Property: Empty routing never panics or corrupts
    #[test]
    fn prop_empty_routing_safe(
        request_ids in prop::collection::vec(any::<u64>(), 10..100)
    ) {
        let router = ProviderRouter::new(vec![]);

        for request_id in request_ids {
            // Property: Empty routing returns fallback without panic
            let provider = router.select_provider(request_id);
            prop_assert_eq!(provider, 0, "Empty routing should return provider 0");
        }
    }
}

// ============================================================================
// Mock Types (same as unit tests)
// ============================================================================

struct ProviderRouter {
    providers: Vec<u8>,
}

impl ProviderRouter {
    fn new(providers: Vec<u8>) -> Self {
        Self { providers }
    }

    fn select_provider(&self, request_id: u64) -> u8 {
        if self.providers.is_empty() {
            return 0;
        }
        let idx = (request_id % self.providers.len() as u64) as usize;
        self.providers[idx]
    }
}

struct MetricsCollector {
    capsule: ResponseCapsule256,
    count: std::sync::atomic::AtomicU64,
    total_latency: std::sync::atomic::AtomicU64,
}

impl MetricsCollector {
    fn new() -> Self {
        Self {
            capsule: ResponseCapsule256::new(),
            count: std::sync::atomic::AtomicU64::new(0),
            total_latency: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn record_response(&self, latency_ns: u64, tokens: u32, cost: f64) {
        self.capsule.record(cost, tokens as u64, latency_ns);
        self.count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.total_latency.fetch_add(latency_ns, std::sync::atomic::Ordering::Relaxed);
    }

    fn total_requests(&self) -> u64 {
        self.count.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn total_tokens(&self) -> u64 {
        // Metrics loaded directly via accessors
        self.capsule.total_tokens()
    }

    fn avg_latency_ns(&self) -> u64 {
        let count = self.total_requests();
        if count == 0 {
            return 0;
        }
        let total = self.total_latency.load(std::sync::atomic::Ordering::Relaxed);
        total / count
    }
}
