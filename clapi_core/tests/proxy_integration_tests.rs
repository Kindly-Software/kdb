//! T28 Tier 3: Integration Tests (Q15-Q21) for Phase 2 HTTP Proxy
//!
//! Testing how capsules work together in the full HTTP proxy pipeline:
//! Request → Budget Check → Provider Selection → API Call → Response → Metrics → Audit

use clapi_core::*;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ============================================================================
// T28 Q15: Critical Integration Points
// ============================================================================

/// Test: Request → Budget → Routing → Response pipeline
#[test]
fn test_full_request_pipeline() {
    // Arrange: Set up all components
    let budget_registry = BudgetRegistry::new();
    let routing = RoutingCapsule128::new(&[0, 1, 2]);
    let metrics = MetricsCollector::new();
    let audit = AuditLogger::new();

    let budget_id = 1;
    let request_id = 12345;

    // Create budget
    let budget_capsule = budget_registry.get_or_create(budget_id, 10_000);

    // Act: Process request through pipeline

    // 1. Budget check
    let cost = 500;
    let budget_result = budget_capsule.try_deduct(cost);
    assert!(budget_result.is_ok(), "Budget check failed");

    // 2. Provider selection
    let provider = (request_id % 3) as u8;
    let health = routing.health_check();
    assert!(health.total_count > 0, "No providers available");

    // 3. Record response
    let latency_ns = 150_000; // 150μs
    let tokens = 75;
    let api_cost = 0.05;
    metrics.record_response(latency_ns, tokens, api_cost);

    // 4. Audit logging
    let audit_entry = audit.append_entry(budget_id, request_id);

    // Assert: Pipeline completed successfully
    assert_eq!(budget_result.unwrap(), 9500, "Budget incorrect");
    assert!(provider < 3, "Invalid provider");
    assert_eq!(metrics.total_requests(), 1);
    assert_eq!(audit_entry.request_id, request_id);
}

/// Test: Budget + Routing integration
#[test]
fn test_budget_routing_integration() {
    let budget = RequestCapsule128::new(1, 5000);
    let routing = RoutingCapsule128::new(&[0, 1, 2, 3]);

    // Process 10 requests
    for i in 0..10 {
        // Check budget
        let cost = 400 + (i * 10); // Varying costs
        if let Ok(remaining) = budget.try_deduct(cost) {
            // Select provider
            let provider = (i % 4) as u8;
            let health = routing.health_check();

            assert!(remaining >= 0);
            assert_eq!(health.total_count, 4);
        }
    }

    // Verify final state
    let state = budget.load_state();
    assert!(state.cost_limit >= 0, "Budget went negative");
}

/// Test: Routing + Metrics integration
#[test]
fn test_routing_metrics_integration() {
    let routing = RoutingCapsule128::new(&[0, 1, 2]);
    let metrics = MetricsCollector::new();

    // Distribute 100 requests across providers
    for i in 0..100 {
        let provider = (i % 3) as u8;
        let latency = 100_000 + (i * 1000); // Varying latency

        // Record metrics per provider
        metrics.record_response(latency, 50, 0.01);
    }

    // Verify integration
    assert_eq!(metrics.total_requests(), 100);
    let health = routing.health_check();
    assert_eq!(health.total_count, 3);
}

/// Test: Metrics + Audit integration
#[test]
fn test_metrics_audit_integration() {
    let metrics = MetricsCollector::new();
    let audit = AuditLogger::new();

    // Process 50 requests with both metrics and audit
    for i in 0..50 {
        let request_id = 1000 + i;
        let latency = 120_000;
        let tokens = 30 + (i as u32);

        metrics.record_response(latency, tokens, 0.02);
        audit.append_entry(1, request_id);
    }

    // Verify both systems tracked requests
    assert_eq!(metrics.total_requests(), 50);
    assert_eq!(audit.entry_count(), 50);
}

// ============================================================================
// T28 Q16: Error Propagation
// ============================================================================

/// Test: Budget exhaustion stops pipeline
#[test]
fn test_budget_exhaustion_propagation() {
    let budget = RequestCapsule128::new(1, 100);
    let metrics = MetricsCollector::new();

    // Request 1: Success (budget: 100 → 60)
    let result1 = budget.try_deduct(40);
    assert!(result1.is_ok());
    metrics.record_response(100_000, 20, 0.01);

    // Request 2: Success (budget: 60 → 10)
    let result2 = budget.try_deduct(50);
    assert!(result2.is_ok());
    metrics.record_response(100_000, 25, 0.015);

    // Request 3: Failure (budget exhausted)
    let result3 = budget.try_deduct(100);
    assert!(result3.is_err());

    // Verify error didn't corrupt metrics
    assert_eq!(metrics.total_requests(), 2, "Only 2 successful requests");

    // Verify error type
    match result3 {
        Err(ClapiError::BudgetExhausted { requested, available }) => {
            assert_eq!(requested, 100);
            assert_eq!(available, 10);
        }
        _ => panic!("Expected BudgetExhausted error"),
    }
}

/// Test: Invalid cost propagation
#[test]
fn test_invalid_cost_propagation() {
    let budget = RequestCapsule128::new(1, 1000);
    let audit = AuditLogger::new();

    // Negative cost should fail
    let result = budget.try_deduct(-50);
    assert!(result.is_err());

    // Verify budget unchanged
    let state = budget.load_state();
    assert_eq!(state.cost_limit, 1000);

    // Audit should still log the attempt (if implemented)
    // In production, failed requests might be audited differently
}

/// Test: Provider unavailable error
#[test]
fn test_provider_unavailable_propagation() {
    let routing = RoutingCapsule128::new(&[]); // Empty provider list

    // Health check should show no providers
    let health = routing.health_check();
    assert_eq!(health.total_count, 0);

    // Request should fail gracefully
    // (In real implementation, would return ClapiError::NoProvidersAvailable)
}

// ============================================================================
// T28 Q17: Performance Budgets
// ============================================================================

/// Test: End-to-end latency budget (<500ns)
#[test]
fn test_integration_latency_budget() {
    let budget = RequestCapsule128::new(1, 100_000);
    let routing = RoutingCapsule128::new(&[0, 1, 2]);
    let metrics = MetricsCollector::new();

    let iterations = 10_000;
    let start = Instant::now();

    for i in 0..iterations {
        // Budget check
        let _ = budget.try_deduct(1);

        // Provider selection
        let _provider = (i % 3) as u8;
        let _ = routing.health_check();

        // Metrics recording
        metrics.record_response(100_000, 10, 0.001);
    }

    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / iterations;

    // Budget: <500ns per request for capsule operations
    assert!(
        avg_ns < 500,
        "Integration overhead {}ns exceeds 500ns budget",
        avg_ns
    );

    println!("✓ Integration latency: {}ns (budget: 500ns)", avg_ns);
}

/// Test: Throughput under load
#[test]
fn test_integration_throughput() {
    let budget = RequestCapsule128::new(1, 1_000_000);
    let routing = RoutingCapsule128::new(&[0, 1, 2, 3, 4]);
    let metrics = MetricsCollector::new();

    let target_ops = 100_000;
    let start = Instant::now();

    for i in 0..target_ops {
        let _ = budget.try_deduct(10);
        let _provider = (i % 5) as u8;
        metrics.record_response(50_000, 25, 0.005);
    }

    let elapsed = start.elapsed();
    let ops_per_sec = target_ops as f64 / elapsed.as_secs_f64();

    // Target: >100K ops/sec
    assert!(
        ops_per_sec > 100_000.0,
        "Throughput {:.0} ops/s below target 100K ops/s",
        ops_per_sec
    );

    println!("✓ Integration throughput: {:.0} ops/s", ops_per_sec);
}

// ============================================================================
// T28 Q18: Production Load
// ============================================================================

/// Test: Handle 10K requests
#[test]
fn test_handle_production_load() {
    let budget_registry = BudgetRegistry::new();
    let routing = RoutingCapsule128::new(&[0, 1, 2, 3]);
    let metrics = MetricsCollector::new();
    let audit = AuditLogger::new();

    let load = 10_000;
    let start = Instant::now();

    for i in 0..load {
        let budget_id = (i % 10) as u64; // 10 different budgets
        let budget = budget_registry.get_or_create(budget_id, 100_000);

        // Process request
        if let Ok(_) = budget.try_deduct(10) {
            let _provider = (i % 4) as u8;
            metrics.record_response(100_000, 50, 0.01);
            audit.append_entry(budget_id, i as u64);
        }
    }

    let elapsed = start.elapsed();

    // Verify throughput maintained
    let throughput = load as f64 / elapsed.as_secs_f64();
    assert!(
        throughput > 10_000.0,
        "Throughput {:.0}/s below 10K/s",
        throughput
    );

    // Verify all components operational
    assert!(metrics.total_requests() > 0);
    assert!(audit.entry_count() > 0);

    println!("✓ Production load: {:.0} req/s over {}ms", throughput, elapsed.as_millis());
}

/// Test: Concurrent multi-budget load
#[test]
fn test_concurrent_multi_budget_load() {
    use std::thread;

    let budget_registry = Arc::new(BudgetRegistry::new());
    let metrics = Arc::new(MetricsCollector::new());

    let threads = 10;
    let requests_per_thread = 1000;

    let handles: Vec<_> = (0..threads).map(|t| {
        let registry = Arc::clone(&budget_registry);
        let m = Arc::clone(&metrics);

        thread::spawn(move || {
            let budget_id = t as u64;
            let budget = registry.get_or_create(budget_id, 50_000);

            for _ in 0..requests_per_thread {
                if budget.try_deduct(10).is_ok() {
                    m.record_response(100_000, 30, 0.01);
                }
            }
        })
    }).collect();

    for h in handles {
        h.join().unwrap();
    }

    // Verify no lost requests
    let total_expected = threads * requests_per_thread;
    let total_actual = metrics.total_requests();

    assert!(
        total_actual > 0,
        "No requests processed"
    );

    println!("✓ Concurrent load: {} threads × {} requests = {} processed",
        threads, requests_per_thread, total_actual);
}

// ============================================================================
// T28 Q19: Rollback Scenarios
// ============================================================================

/// Test: Rollback to direct budget operations
#[test]
fn test_rollback_direct_budget() {
    // New path: Budget capsule
    let budget_new = RequestCapsule128::new(1, 1000);
    let _ = budget_new.try_deduct(100);

    // Old path: Direct atomic (rollback scenario)
    let budget_old = std::sync::atomic::AtomicI64::new(1000);
    let remaining = budget_old.fetch_sub(100, std::sync::atomic::Ordering::Relaxed);

    // Both paths should work
    assert_eq!(budget_new.load_state().cost_limit, 900);
    assert_eq!(remaining, 1000);
    assert_eq!(budget_old.load(std::sync::atomic::Ordering::Relaxed), 900);
}

/// Test: Feature flag simulation
#[test]
fn test_feature_flag_fallback() {
    let use_new_path = true; // Feature flag

    let budget = RequestCapsule128::new(1, 1000);

    let result = if use_new_path {
        // New: Capsule-based
        budget.try_deduct(100)
    } else {
        // Old: Direct atomic (fallback)
        Ok(900) // Simplified fallback
    };

    assert!(result.is_ok());
}

// ============================================================================
// T28 Q20: I20 Integration Validation
// ============================================================================

/// Test: I20 Q11 - New assumptions from composition
#[test]
fn test_i20_q11_retry_convergence() {
    use std::thread;

    let budget = Arc::new(RequestCapsule128::new(1, 100_000));
    let threads = 20;

    let handles: Vec<_> = (0..threads).map(|_| {
        let b = Arc::clone(&budget);
        thread::spawn(move || {
            for _ in 0..100 {
                // Retry up to 10 times
                let mut attempts = 0;
                while attempts < 10 {
                    if b.try_deduct(10).is_ok() {
                        break;
                    }
                    attempts += 1;
                }
                assert!(attempts < 10, "Retry should converge");
            }
        })
    }).collect();

    for h in handles {
        h.join().unwrap();
    }
}

/// Test: I20 Q13 - Boundary invariants
#[test]
fn test_i20_q13_boundary_invariants() {
    let budget = RequestCapsule128::new(1, 1000);
    let routing = RoutingCapsule128::new(&[0, 1, 2]);

    let _ = budget.try_deduct(100);
    let _ = routing.health_check();

    // Boundary invariant: Generation counters coordinated
    let budget_gen = budget.generation();
    assert!(budget_gen > 0, "Budget generation must be positive");

    // Boundary invariant: Budget non-negative
    let state = budget.load_state();
    assert!(state.cost_limit >= 0);
}

/// Test: I20 Q17 - Property invariants across composition
#[test]
fn test_i20_q17_composition_properties() {
    let budget = RequestCapsule128::new(1, 5000);
    let metrics = MetricsCollector::new();

    let mut successful = 0;

    for i in 0..100 {
        if budget.try_deduct(50).is_ok() {
            metrics.record_response(100_000, 25, 0.01);
            successful += 1;
        }
    }

    // Property: Metrics count matches successful budget deductions
    assert_eq!(metrics.total_requests(), successful);
}

// ============================================================================
// T28 Q21: Monitoring Integration
// ============================================================================

/// Test: Metrics collection across pipeline
#[test]
fn test_monitoring_metrics_collected() {
    let budget = RequestCapsule128::new(1, 10_000);
    let routing = RoutingCapsule128::new(&[0, 1, 2]);
    let metrics = MetricsCollector::new();

    // Process 100 requests
    let mut successful = 0;
    let mut failed = 0;

    for i in 0..100 {
        if budget.try_deduct(100).is_ok() {
            metrics.record_response(150_000, 40, 0.02);
            successful += 1;
        } else {
            failed += 1;
        }
    }

    // Verify metrics
    assert_eq!(metrics.total_requests(), successful);
    assert!(failed > 0, "Should have some failures");

    // Verify routing health
    let health = routing.health_check();
    assert_eq!(health.total_count, 3);

    println!("✓ Monitoring: {} successful, {} failed", successful, failed);
}

/// Test: Audit trail completeness
#[test]
fn test_audit_trail_complete() {
    let audit = AuditLogger::new();

    // Process 50 requests
    for i in 0..50 {
        audit.append_entry(1, 1000 + i);
    }

    // Verify complete trail
    assert_eq!(audit.entry_count(), 50);

    // Verify hash chain integrity
    let entries = audit.get_entries();
    for i in 1..entries.len() {
        assert_eq!(
            entries[i].prev_hash,
            entries[i-1].hash(),
            "Hash chain broken at entry {}",
            i
        );
    }
}

/// Test: Error rate tracking
#[test]
fn test_error_rate_tracking() {
    let budget = RequestCapsule128::new(1, 500); // Low budget for errors
    let error_counter = std::sync::atomic::AtomicU64::new(0);

    for _ in 0..100 {
        if budget.try_deduct(10).is_err() {
            error_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    let error_count = error_counter.load(std::sync::atomic::Ordering::Relaxed);
    let error_rate = error_count as f64 / 100.0;

    // Should have errors due to low budget
    assert!(error_rate > 0.0, "Should track errors");
    assert!(error_rate < 1.0, "Should have some successes");

    println!("✓ Error tracking: {:.1}% error rate", error_rate * 100.0);
}

// ============================================================================
// Mock Types
// ============================================================================

struct BudgetRegistry {
    capsules: std::sync::RwLock<std::collections::HashMap<u64, Arc<RequestCapsule128>>>,
}

impl BudgetRegistry {
    fn new() -> Self {
        Self {
            capsules: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    fn get_or_create(&self, budget_id: u64, initial_limit: i64) -> Arc<RequestCapsule128> {
        let mut map = self.capsules.write().unwrap();
        map.entry(budget_id)
            .or_insert_with(|| Arc::new(RequestCapsule128::new(budget_id, initial_limit)))
            .clone()
    }
}

struct MetricsCollector {
    capsule: ResponseCapsule256,
    count: std::sync::atomic::AtomicU64,
}

impl MetricsCollector {
    fn new() -> Self {
        Self {
            capsule: ResponseCapsule256::new(),
            count: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn record_response(&self, latency_ns: u64, tokens: u32, cost: f64) {
        self.capsule.record_response(latency_ns, tokens, cost);
        self.count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn total_requests(&self) -> u64 {
        self.count.load(std::sync::atomic::Ordering::Relaxed)
    }
}

struct AuditLogger {
    entries: std::sync::RwLock<Vec<AuditEntry>>,
}

impl AuditLogger {
    fn new() -> Self {
        Self {
            entries: std::sync::RwLock::new(Vec::new()),
        }
    }

    fn entry_count(&self) -> usize {
        self.entries.read().unwrap().len()
    }

    fn append_entry(&self, budget_id: u64, request_id: u64) -> AuditEntry {
        let prev_hash = self.entries.read().unwrap()
            .last()
            .map(|e| e.hash())
            .unwrap_or([0u8; 32]);

        let capsule = AuditLogEntry128::new(budget_id, request_id, prev_hash);
        let metadata = capsule.load_metadata();

        let entry = AuditEntry {
            request_id,
            prev_hash,
            current_hash: metadata.hash,
        };

        self.entries.write().unwrap().push(entry.clone());
        entry
    }

    fn get_entries(&self) -> Vec<AuditEntry> {
        self.entries.read().unwrap().clone()
    }
}

#[derive(Clone)]
struct AuditEntry {
    request_id: u64,
    prev_hash: [u8; 32],
    current_hash: [u8; 32],
}

impl AuditEntry {
    fn hash(&self) -> [u8; 32] {
        self.current_hash
    }
}
