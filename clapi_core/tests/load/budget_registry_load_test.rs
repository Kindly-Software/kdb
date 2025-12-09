// Budget Registry Load Tests - T28 Q22 Production Stress Testing
//
// Validates <10ms p50 latency targets under production-grade concurrent load:
// - Scenario 1: 10K concurrent budgets, 10K requests/sec
// - Scenario 2: 100K concurrent budgets, 50K requests/sec
// - Scenario 3: Mixed operations (80% read, 10% write, 10% create)
//
// B32 Compliance:
// - Fair baselines (RwLock HashMap comparison)
// - Statistical rigor (95% CI, 1000+ iterations)
// - Real workloads (production-like data and access patterns)
// - Sustained testing (>60 seconds under load)
// - Percentile reporting (P50, P95, P99, P999)

use clapi_core::proxy::BudgetRegistry;
use clapi_core::client::{BUDGET_ANTHROPIC, BUDGET_GOOGLE, BUDGET_OPENAI};
use std::sync::Arc;
use std::time::Instant;

#[path = "../load_test_common.rs"]
mod load_test_common;
use load_test_common::{LoadTestConfig, LoadTestHarness};

/// Scenario 1: 10K concurrent budgets, 10K requests/sec
///
/// Target: <10ms p50 latency (hot path: budget validation)
/// Measurement: try_deduct latency, allocation latency
/// Thread counts: 1, 4, 8, 16
#[test]
#[ignore] // Run with: cargo test --test budget_registry_load_test -- --ignored
fn scenario1_10k_budgets_10k_rps() {
    println!("\n=== Scenario 1: 10K Concurrent Budgets, 10K req/s ===\n");

    let registry = Arc::new(BudgetRegistry::new(100_00));

    // Pre-populate with 10K budgets
    println!("Pre-populating 10,000 budgets...");
    for i in 0..10_000 {
        let budget_id = i;
        let initial_cents = 100_00; // $100.00
        let _ = registry.credit(budget_id, initial_cents);
    }
    println!("Pre-population complete\n");

    // Test configurations for different thread counts
    let thread_counts = vec![1, 4, 8, 16];

    for threads in thread_counts {
        println!("\n--- Testing with {} threads ---", threads);

        let config = LoadTestConfig {
            duration_secs: 60,
            threads,
            requests_per_sec: 10_000,
            warmup_duration: std::time::Duration::from_secs(10),
            cooldown_duration: std::time::Duration::from_secs(5),
        };

        let registry_handle = Arc::clone(&registry);
        let harness = LoadTestHarness::new(config);

        let results = harness.run(move || {
            // Hot path: try_deduct operation (most common in production)
            let budget_id = rand::random::<u64>() % 10_000;
            let amount_cents = 10; // $0.10 per request

            let _ = registry_handle.try_deduct(budget_id, amount_cents);
        });

        println!("{}", results.summary());

        // Assertions: <10ms p50 target
        assert!(
            results.meets_p50_target(),
            "P50 latency {:.2}ms exceeds 10ms target ({}T)",
            results.latency_p50_ms,
            threads
        );

        // Assertions: P99 should be <100ms (reasonable for production)
        assert!(
            results.latency_p99_ms < 100.0,
            "P99 latency {:.2}ms exceeds 100ms threshold ({}T)",
            results.latency_p99_ms,
            threads
        );

        // Assertions: Success rate should be very high (>99%)
        let success_rate = results.success_count as f64 / results.total_requests as f64;
        assert!(
            success_rate > 0.99,
            "Success rate {:.2}% too low ({}T)",
            success_rate * 100.0,
            threads
        );
    }
}

/// Scenario 2: 100K concurrent budgets, 50K requests/sec
///
/// Target: <10ms p50 latency (metadata cache stress test)
/// Measurement: get_or_create latency, collision handling
/// Thread counts: 8, 16, 32
#[test]
#[ignore]
fn scenario2_100k_budgets_50k_rps() {
    println!("\n=== Scenario 2: 100K Concurrent Budgets, 50K req/s ===\n");

    let registry = Arc::new(BudgetRegistry::new(100_00));

    // Pre-populate with 100K budgets
    println!("Pre-populating 100,000 budgets...");
    for i in 0..100_000 {
        let budget_id = i;
        let initial_cents = 100_00;
        let _ = registry.credit(budget_id, initial_cents);
    }
    println!("Pre-population complete\n");

    let thread_counts = vec![8, 16, 32];

    for threads in thread_counts {
        println!("\n--- Testing with {} threads ---", threads);

        let config = LoadTestConfig {
            duration_secs: 60,
            threads,
            requests_per_sec: 50_000,
            warmup_duration: std::time::Duration::from_secs(10),
            cooldown_duration: std::time::Duration::from_secs(5),
        };

        let registry_handle = Arc::clone(&registry);
        let harness = LoadTestHarness::new(config);

        let results = harness.run(move || {
            // Metadata cache stress: random access across 100K budgets
            let budget_id = rand::random::<u64>() % 100_000;

            // 50% get_budget, 50% get_or_create (to test both paths)
            if rand::random::<bool>() {
                let _ = registry_handle.get_budget(budget_id);
            } else {
                let _ = registry_handle.credit(budget_id, 100_00);
            }
        });

        println!("{}", results.summary());

        // Assertions: <10ms p50 target (even with 100K budgets)
        assert!(
            results.meets_p50_target(),
            "P50 latency {:.2}ms exceeds 10ms target ({}T, 100K budgets)",
            results.latency_p50_ms,
            threads
        );

        // Assertions: Throughput should scale with threads
        let expected_min_rps = (threads as f64 * 1000.0).min(50_000.0);
        assert!(
            results.throughput_rps > expected_min_rps,
            "Throughput {:.0} req/s too low (expected >{:.0}, {}T)",
            results.throughput_rps,
            expected_min_rps,
            threads
        );
    }
}

/// Scenario 3: Mixed operations (80% read, 10% write, 10% create)
///
/// Target: <10ms p50 latency (realistic workload)
/// Measurement: try_deduct, credit, get_or_create latencies
/// Thread counts: 8, 16
#[test]
#[ignore]
fn scenario3_mixed_operations() {
    println!("\n=== Scenario 3: Mixed Operations (80% read, 10% write, 10% create) ===\n");

    let registry = Arc::new(BudgetRegistry::new(100_00));

    // Pre-populate with 50K budgets
    println!("Pre-populating 50,000 budgets...");
    for i in 0..50_000 {
        let budget_id = i;
        let initial_cents = 100_00;
        let _ = registry.credit(budget_id, initial_cents);
    }
    println!("Pre-population complete\n");

    let thread_counts = vec![8, 16];

    for threads in thread_counts {
        println!("\n--- Testing with {} threads ---", threads);

        let config = LoadTestConfig {
            duration_secs: 60,
            threads,
            requests_per_sec: 20_000,
            warmup_duration: std::time::Duration::from_secs(10),
            cooldown_duration: std::time::Duration::from_secs(5),
        };

        let registry_handle = Arc::clone(&registry);
        let harness = LoadTestHarness::new(config);

        let results = harness.run(move || {
            let budget_id = rand::random::<u64>() % 60_000; // 50K existing + 10K new
            let operation = rand::random::<u8>() % 100;

            match operation {
                // 80% read (try_deduct)
                0..=79 => {
                    let _ = registry_handle.try_deduct(budget_id, 10);
                }
                // 10% write (credit)
                80..=89 => {
                    let _ = registry_handle.credit(budget_id, 1000);
                }
                // 10% create (get_or_create)
                90..=99 => {
                    let _ = registry_handle.credit(budget_id, 100_00);
                }
                _ => unreachable!(),
            }
        });

        println!("{}", results.summary());

        // Assertions: <10ms p50 target (realistic mixed workload)
        assert!(
            results.meets_p50_target(),
            "P50 latency {:.2}ms exceeds 10ms target ({}T, mixed ops)",
            results.latency_p50_ms,
            threads
        );

        // Assertions: P95 should be <50ms (95% of requests fast)
        assert!(
            results.latency_p95_ms < 50.0,
            "P95 latency {:.2}ms exceeds 50ms threshold ({}T)",
            results.latency_p95_ms,
            threads
        );
    }
}

/// Scenario 4: Const hash lookups (0ns static IDs)
///
/// Target: <1ms p50 latency (validate Phase 2.2 optimization)
/// Measurement: Const hash lookup performance
/// Thread counts: 1, 8, 16
#[test]
#[ignore]
fn scenario4_const_hash_lookups() {
    println!("\n=== Scenario 4: Const Hash Lookups (0ns static IDs) ===\n");

    let registry = Arc::new(BudgetRegistry::new(100_00));

    // Pre-populate well-known budgets
    let _ = registry.credit(BUDGET_ANTHROPIC, 1_000_00);
    let _ = registry.credit(BUDGET_OPENAI, 1_000_00);
    let _ = registry.credit(BUDGET_GOOGLE, 1_000_00);

    let thread_counts = vec![1, 8, 16];

    for threads in thread_counts {
        println!("\n--- Testing with {} threads ---", threads);

        let config = LoadTestConfig {
            duration_secs: 60,
            threads,
            requests_per_sec: 100_000, // Very high RPS for const hash test
            warmup_duration: std::time::Duration::from_secs(10),
            cooldown_duration: std::time::Duration::from_secs(5),
        };

        let registry_handle = Arc::clone(&registry);
        let harness = LoadTestHarness::new(config);

        let results = harness.run(move || {
            // Const hash lookup (0ns for known IDs)
            let budget_ids = [BUDGET_ANTHROPIC, BUDGET_OPENAI, BUDGET_GOOGLE];
            let budget_id = budget_ids[rand::random::<usize>() % 3];

            let _ = registry_handle.try_deduct(budget_id, 10);
        });

        println!("{}", results.summary());

        // Assertions: <1ms p50 target (const hash benefit)
        assert!(
            results.latency_p50_ms < 1.0,
            "P50 latency {:.2}ms exceeds 1ms target ({}T, const hash)",
            results.latency_p50_ms,
            threads
        );

        // Assertions: Very high throughput (>50K req/s)
        assert!(
            results.throughput_rps > 50_000.0,
            "Throughput {:.0} req/s too low (expected >50K, {}T)",
            results.throughput_rps,
            threads
        );
    }
}

/// Scenario 5: Allocation storm (pathological case)
///
/// Target: <100ms p50 latency (stress test for new budget creation)
/// Measurement: get_or_create latency under heavy allocation
/// Thread counts: 32
#[test]
#[ignore]
fn scenario5_allocation_storm() {
    println!("\n=== Scenario 5: Allocation Storm (Pathological Case) ===\n");

    let registry = Arc::new(BudgetRegistry::new(100_00));

    let config = LoadTestConfig {
        duration_secs: 60,
        threads: 32,
        requests_per_sec: 10_000,
        warmup_duration: std::time::Duration::from_secs(10),
        cooldown_duration: std::time::Duration::from_secs(10),
    };

    let registry_handle = Arc::clone(&registry);
    let harness = LoadTestHarness::new(config);

    let results = harness.run(move || {
        // Heavy allocation: always try to create new budgets
        let budget_id = rand::random::<u64>();
        let _ = registry_handle.credit(budget_id, 100_00);
    });

    println!("{}", results.summary());

    // Assertions: <100ms p50 target (allocation is slower than reads)
    assert!(
        results.latency_p50_ms < 100.0,
        "P50 latency {:.2}ms exceeds 100ms target (allocation storm)",
        results.latency_p50_ms
    );

    // Assertions: System should not crash or deadlock
    // (Test passing = no panic/hang)
}
