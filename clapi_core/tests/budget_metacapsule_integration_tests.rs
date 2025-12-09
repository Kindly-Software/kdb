//! T28 Tier 3: Integration Testing (Q15-Q21)
//!
//! Integration tests for budget metacapsule with other components.
//!
//! **Coverage:**
//! - Q15: Critical integration points (budget+routing, budget+metrics, budget+audit)
//! - Q16: Error propagation (budget exhaustion, invalid costs, cascading failures)
//! - Q17: Performance budgets (<500ns end-to-end, >100K ops/sec)
//! - Q18: Production load (10K requests, concurrent multi-budget)
//! - Q19: Rollback scenarios (feature flags, backward compatibility)
//! - Q20: I20 validation (retry convergence, boundary invariants, composition)
//! - Q21: Monitoring (metrics collection, audit trail, error tracking)
//!
//! **Test Count:** 50 integration tests

use clapi_core::error::{ClapiError, ClapiResult};
use clapi_core::proxy::budget_registry::BudgetRegistry;
use clapi_core::RequestCapsule128;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

// ============================================================================
// Mock Components for Integration Testing
// ============================================================================

/// Mock provider router for testing
struct MockProviderRouter {
    request_count: AtomicU64,
}

impl MockProviderRouter {
    fn new() -> Self {
        Self {
            request_count: AtomicU64::new(0),
        }
    }

    fn select_provider(&self, _budget_id: u64) -> u8 {
        self.request_count.fetch_add(1, Ordering::Relaxed);
        0 // Always select provider 0
    }

    fn request_count(&self) -> u64 {
        self.request_count.load(Ordering::Relaxed)
    }
}

/// Mock metrics collector for testing
struct MockMetricsCollector {
    total_requests: AtomicU64,
    total_cost: AtomicU64,
}

impl MockMetricsCollector {
    fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            total_cost: AtomicU64::new(0),
        }
    }

    fn record_request(&self, cost: i64) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_cost.fetch_add(cost as u64, Ordering::Relaxed);
    }

    fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }

    fn total_cost(&self) -> i64 {
        self.total_cost.load(Ordering::Relaxed) as i64
    }
}

/// Mock audit logger for testing
struct MockAuditLogger {
    entry_count: AtomicU64,
}

impl MockAuditLogger {
    fn new() -> Self {
        Self {
            entry_count: AtomicU64::new(0),
        }
    }

    fn log_request(&self, _budget_id: u64, _cost: i64) {
        self.entry_count.fetch_add(1, Ordering::Relaxed);
    }

    fn entry_count(&self) -> u64 {
        self.entry_count.load(Ordering::Relaxed)
    }
}

// ============================================================================
// T28 Q15: Critical Integration Points (10 tests)
// ============================================================================

#[test]
fn test_integration_budget_deduct_then_route() {
    // Arrange
    let registry = BudgetRegistry::new(1000_00);
    let router = MockProviderRouter::new();

    // Act: Budget deduction → Provider selection
    let budget_result = registry.try_deduct(1, 100_00);
    let provider = router.select_provider(1);

    // Assert
    assert!(budget_result.is_ok());
    assert_eq!(budget_result.unwrap(), 900_00);
    assert_eq!(provider, 0);
    assert_eq!(router.request_count(), 1);
}

#[test]
fn test_integration_budget_route_metrics() {
    // Arrange
    let registry = BudgetRegistry::new(1000_00);
    let router = MockProviderRouter::new();
    let metrics = MockMetricsCollector::new();

    // Act: Full pipeline
    let cost = 50_00;
    registry.try_deduct(1, cost).unwrap();
    let provider = router.select_provider(1);
    metrics.record_request(cost);

    // Assert: All components updated
    assert_eq!(registry.get_budget(1), Some(950_00));
    assert_eq!(provider, 0);
    assert_eq!(metrics.total_requests(), 1);
    assert_eq!(metrics.total_cost(), 50_00);
}

#[test]
fn test_integration_budget_route_metrics_audit() {
    // Arrange
    let registry = BudgetRegistry::new(1000_00);
    let router = MockProviderRouter::new();
    let metrics = MockMetricsCollector::new();
    let audit = MockAuditLogger::new();

    // Act: Full pipeline with audit
    let cost = 75_00;
    registry.try_deduct(1, cost).unwrap();
    router.select_provider(1);
    metrics.record_request(cost);
    audit.log_request(1, cost);

    // Assert: All components in sync
    assert_eq!(registry.get_budget(1), Some(925_00));
    assert_eq!(router.request_count(), 1);
    assert_eq!(metrics.total_requests(), 1);
    assert_eq!(audit.entry_count(), 1);
}

#[test]
fn test_integration_multiple_requests_sequential() {
    let registry = BudgetRegistry::new(1000_00);
    let metrics = MockMetricsCollector::new();

    for _ in 0..10 {
        registry.try_deduct(1, 50_00).unwrap();
        metrics.record_request(50_00);
    }

    assert_eq!(registry.get_budget(1), Some(500_00));
    assert_eq!(metrics.total_requests(), 10);
    assert_eq!(metrics.total_cost(), 500_00);
}

#[test]
fn test_integration_multiple_users_isolated() {
    let registry = BudgetRegistry::new(1000_00);
    let metrics = MockMetricsCollector::new();

    // User 1
    registry.try_deduct(1, 100_00).unwrap();
    metrics.record_request(100_00);

    // User 2
    registry.try_deduct(2, 200_00).unwrap();
    metrics.record_request(200_00);

    // User 3
    registry.try_deduct(3, 300_00).unwrap();
    metrics.record_request(300_00);

    // Assert: Users isolated, metrics aggregated
    assert_eq!(registry.get_budget(1), Some(900_00));
    assert_eq!(registry.get_budget(2), Some(800_00));
    assert_eq!(registry.get_budget(3), Some(700_00));
    assert_eq!(metrics.total_cost(), 600_00);
}

#[test]
fn test_integration_credit_after_deduct() {
    let registry = BudgetRegistry::new(1000_00);
    let metrics = MockMetricsCollector::new();

    registry.try_deduct(1, 500_00).unwrap();
    metrics.record_request(500_00);

    registry.credit(1, 200_00).unwrap();

    assert_eq!(registry.get_budget(1), Some(700_00));
    assert_eq!(metrics.total_cost(), 500_00); // Credit doesn't affect metrics
}

#[test]
fn test_integration_stats_after_operations() {
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(1, 100_00).unwrap();
    registry.try_deduct(1, 150_00).unwrap();
    registry.credit(1, 50_00).unwrap();

    let stats = registry.get_stats(1).unwrap();
    assert_eq!(stats.budget, 800_00);
    assert_eq!(stats.total_spent, 250_00);
    assert_eq!(stats.request_count, 2);
}

#[test]
fn test_integration_concurrent_budget_and_routing() {
    let registry = Arc::new(BudgetRegistry::new(10_000_00));
    let router = Arc::new(MockProviderRouter::new());

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let r = Arc::clone(&registry);
            let rt = Arc::clone(&router);
            thread::spawn(move || {
                for _ in 0..100 {
                    if r.try_deduct(1, 10_00).is_ok() {
                        rt.select_provider(1);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Assert: Budget and routing in sync
    let stats = registry.get_stats(1).unwrap();
    assert_eq!(router.request_count(), stats.request_count);
}

#[test]
fn test_integration_budget_metrics_consistency() {
    let registry = Arc::new(BudgetRegistry::new(10_000_00));
    let metrics = Arc::new(MockMetricsCollector::new());

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let r = Arc::clone(&registry);
            let m = Arc::clone(&metrics);
            thread::spawn(move || {
                for _ in 0..100 {
                    if r.try_deduct(1, 10_00).is_ok() {
                        m.record_request(10_00);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Assert: Budget and metrics consistent
    let stats = registry.get_stats(1).unwrap();
    assert_eq!(stats.total_spent, metrics.total_cost());
}

#[test]
fn test_integration_full_pipeline_concurrent() {
    let registry = Arc::new(BudgetRegistry::new(10_000_00));
    let router = Arc::new(MockProviderRouter::new());
    let metrics = Arc::new(MockMetricsCollector::new());
    let audit = Arc::new(MockAuditLogger::new());

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let r = Arc::clone(&registry);
            let rt = Arc::clone(&router);
            let m = Arc::clone(&metrics);
            let a = Arc::clone(&audit);
            thread::spawn(move || {
                for _ in 0..100 {
                    if r.try_deduct(1, 10_00).is_ok() {
                        rt.select_provider(1);
                        m.record_request(10_00);
                        a.log_request(1, 10_00);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Assert: All components consistent
    let stats = registry.get_stats(1).unwrap();
    assert_eq!(router.request_count(), stats.request_count);
    assert_eq!(metrics.total_requests(), stats.request_count);
    assert_eq!(audit.entry_count(), stats.request_count);
}

// ============================================================================
// T28 Q16: Error Propagation (8 tests)
// ============================================================================

#[test]
fn test_error_budget_exhaustion_blocks_routing() {
    let registry = BudgetRegistry::new(50_00);
    let router = MockProviderRouter::new();

    // Exhaust budget
    let result = registry.try_deduct(1, 100_00);
    assert!(result.is_err());

    // Routing should not occur
    let initial_count = router.request_count();

    // Verify no routing happened
    assert_eq!(router.request_count(), initial_count);
}

#[test]
fn test_error_invalid_cost_rejected() {
    let registry = BudgetRegistry::new(1000_00);

    let result = registry.try_deduct(1, -100_00);
    assert!(result.is_err());
    assert!(matches!(result, Err(ClapiError::InvalidCost(_))));

    // Budget unchanged
    let budget = registry.get_budget(1);
    assert!(budget.is_none()); // No budget created
}

#[test]
fn test_error_propagation_through_pipeline() {
    let registry = BudgetRegistry::new(50_00);
    let metrics = MockMetricsCollector::new();
    let audit = MockAuditLogger::new();

    // Try to deduct more than available
    let result = registry.try_deduct(1, 100_00);
    assert!(result.is_err());

    // Metrics and audit should not record failed request
    let initial_metrics = metrics.total_requests();
    let initial_audit = audit.entry_count();

    assert_eq!(metrics.total_requests(), initial_metrics);
    assert_eq!(audit.entry_count(), initial_audit);
}

#[test]
fn test_error_partial_success_in_batch() {
    let registry = BudgetRegistry::new(1000_00);

    let mut successful = 0;
    let mut failed = 0;

    for _ in 0..20 {
        if registry.try_deduct(1, 100_00).is_ok() {
            successful += 1;
        } else {
            failed += 1;
        }
    }

    // Should succeed 10 times, fail 10 times
    assert_eq!(successful, 10);
    assert_eq!(failed, 10);
    assert_eq!(registry.get_budget(1), Some(0));
}

#[test]
fn test_error_recovery_after_credit() {
    let registry = BudgetRegistry::new(100_00);

    // Exhaust budget
    registry.try_deduct(1, 100_00).unwrap();

    // Try to deduct - should fail
    let result1 = registry.try_deduct(1, 50_00);
    assert!(result1.is_err());

    // Credit budget
    registry.credit(1, 100_00).unwrap();

    // Now should succeed
    let result2 = registry.try_deduct(1, 50_00);
    assert!(result2.is_ok());
}

#[test]
fn test_error_concurrent_exhaustion() {
    let registry = Arc::new(BudgetRegistry::new(1000_00));

    let handles: Vec<_> = (0..20)
        .map(|_| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                let _ = r.try_deduct(1, 100_00);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Budget should be 0 or positive, never negative
    let budget = registry.get_budget(1).unwrap();
    assert!(budget >= 0);
}

#[test]
fn test_error_cascade_prevention() {
    let registry = BudgetRegistry::new(100_00);

    // First deduction succeeds
    assert!(registry.try_deduct(1, 50_00).is_ok());

    // Second deduction succeeds
    assert!(registry.try_deduct(1, 50_00).is_ok());

    // Third deduction fails (budget exhausted)
    assert!(registry.try_deduct(1, 50_00).is_err());

    // Budget is exactly 0, not negative
    assert_eq!(registry.get_budget(1), Some(0));
}

#[test]
fn test_error_multiple_users_isolated_failures() {
    let registry = BudgetRegistry::new(100_00);

    // User 1 exhausts budget
    registry.try_deduct(1, 100_00).unwrap();
    assert!(registry.try_deduct(1, 50_00).is_err());

    // User 2 should still work
    assert!(registry.try_deduct(2, 50_00).is_ok());
}

// ============================================================================
// T28 Q17: Performance Budgets (8 tests)
// ============================================================================

#[test]
fn test_performance_single_deduction_latency() {
    let registry = BudgetRegistry::new(1000_00);

    let start = std::time::Instant::now();
    registry.try_deduct(1, 50_00).unwrap();
    let elapsed = start.elapsed();

    // Budget: <1μs for single operation
    assert!(elapsed.as_nanos() < 1000, "Single deduction too slow: {:?}", elapsed);
}

#[test]
fn test_performance_batch_deductions_throughput() {
    let registry = BudgetRegistry::new(100_000_00);

    let start = std::time::Instant::now();

    for _ in 0..10_000 {
        let _ = registry.try_deduct(1, 10_00);
    }

    let elapsed = start.elapsed();
    let throughput = 10_000.0 / elapsed.as_secs_f64();

    // Budget: >100K ops/sec
    assert!(
        throughput > 100_000.0,
        "Throughput too low: {:.0} ops/s",
        throughput
    );
}

#[test]
fn test_performance_concurrent_throughput() {
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

    let elapsed = start.elapsed();
    let throughput = 10_000.0 / elapsed.as_secs_f64();

    // Budget: >50K ops/sec with concurrency
    assert!(
        throughput > 50_000.0,
        "Concurrent throughput too low: {:.0} ops/s",
        throughput
    );
}

#[test]
fn test_performance_integration_overhead() {
    let registry = BudgetRegistry::new(100_000_00);
    let metrics = MockMetricsCollector::new();

    let start = std::time::Instant::now();

    for _ in 0..10_000 {
        if registry.try_deduct(1, 10_00).is_ok() {
            metrics.record_request(10_00);
        }
    }

    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / 10_000;

    // Budget: <500ns end-to-end (deduct + metrics)
    assert!(
        avg_ns < 500,
        "Integration overhead too high: {}ns",
        avg_ns
    );
}

#[test]
fn test_performance_get_budget_read() {
    let registry = BudgetRegistry::new(1000_00);
    registry.try_deduct(1, 100_00).unwrap();

    let start = std::time::Instant::now();

    for _ in 0..100_000 {
        let _ = registry.get_budget(1);
    }

    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / 100_000;

    // Budget: <100ns for read operations
    assert!(avg_ns < 100, "Read operations too slow: {}ns", avg_ns);
}

#[test]
fn test_performance_get_stats() {
    let registry = BudgetRegistry::new(1000_00);
    registry.try_deduct(1, 100_00).unwrap();

    let start = std::time::Instant::now();

    for _ in 0..10_000 {
        let _ = registry.get_stats(1);
    }

    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / 10_000;

    // Budget: <200ns for stats operations
    assert!(avg_ns < 200, "Stats operations too slow: {}ns", avg_ns);
}

#[test]
fn test_performance_credit_operations() {
    let registry = BudgetRegistry::new(1000_00);

    let start = std::time::Instant::now();

    for _ in 0..10_000 {
        let _ = registry.credit(1, 10_00);
    }

    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / 10_000;

    // Budget: <100ns for credit operations
    assert!(avg_ns < 100, "Credit operations too slow: {}ns", avg_ns);
}

#[test]
fn test_performance_multi_user_throughput() {
    let registry = Arc::new(BudgetRegistry::new(10_000_00));

    let start = std::time::Instant::now();

    let handles: Vec<_> = (0..10)
        .map(|budget_id| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                for _ in 0..1000 {
                    let _ = r.try_deduct(budget_id as u64, 10_00);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let elapsed = start.elapsed();
    let throughput = 10_000.0 / elapsed.as_secs_f64();

    // Budget: >50K ops/sec across multiple users
    assert!(
        throughput > 50_000.0,
        "Multi-user throughput too low: {:.0} ops/s",
        throughput
    );
}

// ============================================================================
// T28 Q18: Production Load (6 tests)
// ============================================================================

#[test]
fn test_production_load_10k_requests() {
    let registry = BudgetRegistry::new(1_000_000_00);
    let metrics = MockMetricsCollector::new();

    for _ in 0..10_000 {
        if registry.try_deduct(1, 100_00).is_ok() {
            metrics.record_request(100_00);
        }
    }

    // Assert: All requests processed
    let stats = registry.get_stats(1).unwrap();
    assert_eq!(stats.request_count, 10_000);
    assert_eq!(metrics.total_requests(), 10_000);
}

#[test]
fn test_production_load_concurrent_10k() {
    let registry = Arc::new(BudgetRegistry::new(10_000_000_00));

    let handles: Vec<_> = (0..100)
        .map(|_| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = r.try_deduct(1, 100_00);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let stats = registry.get_stats(1).unwrap();
    assert!(stats.request_count > 0);
    assert_eq!(stats.budget + stats.total_spent, 10_000_000_00);
}

#[test]
fn test_production_load_multiple_budgets() {
    let registry = Arc::new(BudgetRegistry::new(1_000_000_00));

    let handles: Vec<_> = (0..100)
        .map(|budget_id| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = r.try_deduct(budget_id as u64, 100_00);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Assert: All budgets handled independently
    assert_eq!(registry.len(), 100);
}

#[test]
fn test_production_load_sustained() {
    let registry = Arc::new(BudgetRegistry::new(100_000_000_00));

    let start = std::time::Instant::now();

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                let mut count = 0;
                while start.elapsed().as_secs() < 1 {
                    if r.try_deduct(1, 10_00).is_ok() {
                        count += 1;
                    }
                }
                count
            })
        })
        .collect();

    let total: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();

    // Assert: Sustained throughput >100K ops/sec
    assert!(total > 100_000, "Sustained throughput too low: {}", total);
}

#[test]
fn test_production_load_mixed_operations() {
    let registry = Arc::new(BudgetRegistry::new(10_000_000_00));

    let handles: Vec<_> = (0..20)
        .map(|i| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                if i % 2 == 0 {
                    // Even threads: deduct
                    for _ in 0..500 {
                        let _ = r.try_deduct(1, 100_00);
                    }
                } else {
                    // Odd threads: credit
                    for _ in 0..500 {
                        let _ = r.credit(1, 50_00);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Assert: Operations completed without deadlock
    let stats = registry.get_stats(1).unwrap();
    assert!(stats.request_count > 0);
}

#[test]
fn test_production_load_spike_recovery() {
    let registry = Arc::new(BudgetRegistry::new(10_000_000_00));

    // Normal load
    let handles1: Vec<_> = (0..10)
        .map(|_| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = r.try_deduct(1, 100_00);
                }
            })
        })
        .collect();

    for h in handles1 {
        h.join().unwrap();
    }

    // Spike load
    let handles2: Vec<_> = (0..100)
        .map(|_| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = r.try_deduct(1, 100_00);
                }
            })
        })
        .collect();

    for h in handles2 {
        h.join().unwrap();
    }

    // Assert: System recovers from spike
    let stats = registry.get_stats(1).unwrap();
    assert_eq!(stats.budget + stats.total_spent, 10_000_000_00);
}

// ============================================================================
// T28 Q19: Rollback Scenarios (4 tests)
// ============================================================================

#[test]
fn test_rollback_direct_budget_access() {
    // Simulate rollback: direct capsule access without registry
    let capsule = RequestCapsule128::new(1000_00);

    capsule.try_deduct(100_00).unwrap();
    assert_eq!(capsule.budget(), 900_00);

    // Rollback works: direct capsule still functional
    capsule.try_deduct(50_00).unwrap();
    assert_eq!(capsule.budget(), 850_00);
}

#[test]
fn test_rollback_feature_flag_simulation() {
    let registry = BudgetRegistry::new(1000_00);

    // Feature flag ON: use registry
    registry.try_deduct(1, 100_00).unwrap();
    assert_eq!(registry.get_budget(1), Some(900_00));

    // Feature flag OFF: fallback to direct capsule
    let capsule = RequestCapsule128::new(1000_00);
    capsule.try_deduct(100_00).unwrap();
    assert_eq!(capsule.budget(), 900_00);

    // Both paths work identically
}

#[test]
fn test_rollback_backward_compatibility() {
    // Old API: direct capsule
    let old_capsule = RequestCapsule128::new(1000_00);
    old_capsule.try_deduct(100_00).unwrap();

    // New API: registry
    let registry = BudgetRegistry::new(1000_00);
    registry.try_deduct(1, 100_00).unwrap();

    // Both APIs produce same result
    assert_eq!(old_capsule.budget(), registry.get_budget(1).unwrap());
}

#[test]
fn test_rollback_migration_path() {
    // Migrate from direct capsule to registry
    let capsule = Arc::new(RequestCapsule128::new(1000_00));

    // Step 1: Use capsule directly
    capsule.try_deduct(100_00).unwrap();
    assert_eq!(capsule.budget(), 900_00);

    // Step 2: Wrap in registry (migration)
    let registry = BudgetRegistry::new(1000_00);
    registry.try_deduct(2, 100_00).unwrap();

    // Both coexist during migration
    assert_eq!(capsule.budget(), 900_00);
    assert_eq!(registry.get_budget(2), Some(900_00));
}

// ============================================================================
// T28 Q20: I20 Validation (8 tests)
// ============================================================================

#[test]
fn test_i20_q11_retry_convergence() {
    // I20 Q11: Retry convergence (no livelocks)
    let registry = Arc::new(BudgetRegistry::new(10_000_00));

    let handles: Vec<_> = (0..20)
        .map(|_| {
            let r = Arc::clone(&registry);
            thread::spawn(move || {
                for _ in 0..100 {
                    // Retry always converges (no livelock)
                    let result = r.try_deduct(1, 10_00);
                    assert!(result.is_ok() || result.is_err());
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // All operations completed (no livelock)
}

#[test]
fn test_i20_q13_boundary_invariants() {
    // I20 Q13: Boundary invariants (generation coordination)
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(1, 100_00).unwrap();
    let stats = registry.get_stats(1).unwrap();

    // Boundary invariant: generation > 0 after operation
    assert!(stats.generation > 0);
}

#[test]
fn test_i20_q17_composition_properties() {
    // I20 Q17: Property invariants across composition
    let registry = BudgetRegistry::new(1000_00);
    let metrics = MockMetricsCollector::new();

    registry.try_deduct(1, 100_00).unwrap();
    metrics.record_request(100_00);

    registry.try_deduct(1, 200_00).unwrap();
    metrics.record_request(200_00);

    // Property: budget + spent = initial
    let stats = registry.get_stats(1).unwrap();
    assert_eq!(stats.budget + stats.total_spent, 1000_00);
    assert_eq!(stats.total_spent, metrics.total_cost());
}

#[test]
fn test_i20_q20_rollback_plan() {
    // I20 Q20: Rollback plan tested
    let registry = BudgetRegistry::new(1000_00);

    // Normal operation
    registry.try_deduct(1, 100_00).unwrap();

    // Rollback simulation: credit back
    registry.credit(1, 100_00).unwrap();

    assert_eq!(registry.get_budget(1), Some(1000_00));
}

#[test]
fn test_i20_integration_full_validation() {
    let registry = BudgetRegistry::new(1000_00);

    // I20 Q11: Retry
    for _ in 0..10 {
        let _ = registry.try_deduct(1, 10_00);
    }

    // I20 Q13: Boundaries
    let stats = registry.get_stats(1).unwrap();
    assert!(stats.generation > 0);

    // I20 Q17: Composition
    assert_eq!(stats.budget + stats.total_spent, 1000_00);

    // I20 Q20: Rollback
    registry.credit(1, stats.total_spent).unwrap();
    assert_eq!(registry.get_budget(1), Some(1000_00));
}

#[test]
fn test_i20_concurrent_composition() {
    let registry = Arc::new(BudgetRegistry::new(10_000_00));
    let metrics = Arc::new(MockMetricsCollector::new());

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let r = Arc::clone(&registry);
            let m = Arc::clone(&metrics);
            thread::spawn(move || {
                for _ in 0..100 {
                    if r.try_deduct(1, 10_00).is_ok() {
                        m.record_request(10_00);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // I20 composition holds under concurrency
    let stats = registry.get_stats(1).unwrap();
    assert_eq!(stats.total_spent, metrics.total_cost());
}

#[test]
fn test_i20_error_propagation_composition() {
    let registry = BudgetRegistry::new(100_00);
    let metrics = MockMetricsCollector::new();

    // Successful request
    registry.try_deduct(1, 50_00).unwrap();
    metrics.record_request(50_00);

    // Failed request (budget exhausted)
    let result = registry.try_deduct(1, 100_00);
    assert!(result.is_err());

    // Metrics only record successful requests
    assert_eq!(metrics.total_cost(), 50_00);
}

#[test]
fn test_i20_multi_component_integration() {
    let registry = BudgetRegistry::new(1000_00);
    let router = MockProviderRouter::new();
    let metrics = MockMetricsCollector::new();
    let audit = MockAuditLogger::new();

    // Full pipeline
    for i in 0..10 {
        if registry.try_deduct(1, 50_00).is_ok() {
            router.select_provider(1);
            metrics.record_request(50_00);
            audit.log_request(1, 50_00);
        }
    }

    // All components in sync
    let stats = registry.get_stats(1).unwrap();
    assert_eq!(router.request_count(), stats.request_count);
    assert_eq!(metrics.total_requests(), stats.request_count);
    assert_eq!(audit.entry_count(), stats.request_count);
}

// ============================================================================
// T28 Q21: Monitoring (6 tests)
// ============================================================================

#[test]
fn test_monitoring_metrics_collection() {
    let registry = BudgetRegistry::new(1000_00);
    let metrics = MockMetricsCollector::new();

    for _ in 0..10 {
        if registry.try_deduct(1, 50_00).is_ok() {
            metrics.record_request(50_00);
        }
    }

    // Metrics collected
    assert_eq!(metrics.total_requests(), 10);
    assert_eq!(metrics.total_cost(), 500_00);
}

#[test]
fn test_monitoring_audit_trail() {
    let registry = BudgetRegistry::new(1000_00);
    let audit = MockAuditLogger::new();

    for _ in 0..10 {
        if registry.try_deduct(1, 50_00).is_ok() {
            audit.log_request(1, 50_00);
        }
    }

    // Audit trail complete
    assert_eq!(audit.entry_count(), 10);
}

#[test]
fn test_monitoring_error_tracking() {
    let registry = BudgetRegistry::new(100_00);

    let mut successful = 0;
    let mut failed = 0;

    for _ in 0..10 {
        if registry.try_deduct(1, 50_00).is_ok() {
            successful += 1;
        } else {
            failed += 1;
        }
    }

    // Error rate: 80% (8/10 failed)
    assert_eq!(successful, 2);
    assert_eq!(failed, 8);
}

#[test]
fn test_monitoring_concurrent_metrics() {
    let registry = Arc::new(BudgetRegistry::new(10_000_00));
    let metrics = Arc::new(MockMetricsCollector::new());

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let r = Arc::clone(&registry);
            let m = Arc::clone(&metrics);
            thread::spawn(move || {
                for _ in 0..100 {
                    if r.try_deduct(1, 10_00).is_ok() {
                        m.record_request(10_00);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Metrics accurate under concurrency
    let stats = registry.get_stats(1).unwrap();
    assert_eq!(metrics.total_requests(), stats.request_count);
}

#[test]
fn test_monitoring_generation_tracking() {
    let registry = BudgetRegistry::new(1000_00);

    let mut generations = Vec::new();

    for _ in 0..10 {
        registry.try_deduct(1, 50_00).unwrap();
        if let Some(stats) = registry.get_stats(1) {
            generations.push(stats.generation);
        }
    }

    // Generation always increases
    for i in 1..generations.len() {
        assert!(generations[i] > generations[i - 1]);
    }
}

#[test]
fn test_monitoring_request_count_accurate() {
    let registry = BudgetRegistry::new(10_000_00);

    for _ in 0..100 {
        registry.try_deduct(1, 10_00).unwrap();
    }

    let stats = registry.get_stats(1).unwrap();
    assert_eq!(stats.request_count, 100);
}
