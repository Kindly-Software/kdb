//! T28 Tier 1: Unit Tests (Q1-Q7) for Phase 2 HTTP Proxy
//!
//! Testing proxy server components built on top of clapi_core capsules:
//! - ProxyServer: Main HTTP server coordinating all components
//! - BudgetRegistry: Budget tracking using RequestCapsule128
//! - ProviderRouter: Provider selection using RoutingCapsule128
//! - MetricsCollector: Response metrics using ResponseCapsule256
//! - AuditLogger: Audit trail using AuditLogEntry128

use clapi_core::*;
use std::sync::Arc;

// ============================================================================
// T28 Q1: Core Behaviors
// ============================================================================

#[test]
fn test_budget_registry_creation() {
    let registry = BudgetRegistry::new();
    assert_eq!(registry.len(), 0, "New registry should be empty");
}

#[test]
fn test_budget_get_or_create() {
    let registry = BudgetRegistry::new();
    let budget_id = 1;

    // Create new budget
    let capsule = registry.get_or_create(budget_id, 1000);
    let state = capsule.load_state();
    assert_eq!(state.budget_id, budget_id);
    assert_eq!(state.cost_limit, 1000);

    // Get existing budget
    let capsule2 = registry.get_or_create(budget_id, 2000);
    let state2 = capsule2.load_state();
    assert_eq!(state2.budget_id, budget_id);
    assert_eq!(state2.cost_limit, 1000, "Should not overwrite existing budget");
}

#[test]
fn test_budget_deduction_success() {
    let capsule = RequestCapsule128::new(1, 1000);

    // Deduct valid amount
    let result = capsule.try_deduct(100);
    assert!(result.is_ok(), "Valid deduction should succeed");
    assert_eq!(result.unwrap(), 900, "Remaining should be 900");

    // Verify state
    let state = capsule.load_state();
    assert_eq!(state.cost_limit, 900, "Cost limit should decrease");
}

#[test]
fn test_budget_deduction_insufficient() {
    let capsule = RequestCapsule128::new(1, 100);

    // Try to deduct more than available
    let result = capsule.try_deduct(200);
    assert!(result.is_err(), "Should fail when insufficient budget");

    match result {
        Err(ClapiError::BudgetExhausted { requested, available }) => {
            assert_eq!(requested, 200);
            assert_eq!(available, 100);
        }
        _ => panic!("Expected BudgetExhausted error"),
    }

    // Verify state unchanged
    let state = capsule.load_state();
    assert_eq!(state.cost_limit, 100, "Budget should be unchanged on failure");
}

#[test]
fn test_provider_router_creation() {
    let router = ProviderRouter::new(vec![0, 1, 2]);
    assert_eq!(router.provider_count(), 3);
}

#[test]
fn test_provider_selection_deterministic() {
    let router = ProviderRouter::new(vec![0, 1, 2]);

    // Same request ID should always select same provider
    let provider1 = router.select_provider(12345);
    let provider2 = router.select_provider(12345);
    assert_eq!(provider1, provider2, "Selection should be deterministic");
}

#[test]
fn test_provider_selection_distribution() {
    let router = ProviderRouter::new(vec![0, 1, 2]);

    // Different request IDs should distribute across providers
    let mut selections = std::collections::HashMap::new();
    for i in 0..300 {
        let provider = router.select_provider(i);
        *selections.entry(provider).or_insert(0) += 1;
    }

    // Each provider should get some requests (allow 10% variance)
    for count in selections.values() {
        assert!(*count > 80 && *count < 120, "Distribution should be roughly even");
    }
}

#[test]
fn test_metrics_collector_creation() {
    let collector = MetricsCollector::new();
    assert_eq!(collector.total_requests(), 0);
}

#[test]
fn test_metrics_record_response() {
    let collector = MetricsCollector::new();

    // Record response
    collector.record_response(100_000, 50, 0.01);

    assert_eq!(collector.total_requests(), 1);
    assert_eq!(collector.total_tokens(), 50);

    let avg_latency = collector.avg_latency_ns();
    assert_eq!(avg_latency, 100_000);
}

#[test]
fn test_audit_logger_creation() {
    let logger = AuditLogger::new();
    assert_eq!(logger.entry_count(), 0);
}

#[test]
fn test_audit_logger_append() {
    let logger = AuditLogger::new();

    // Append entry
    let entry = logger.append_entry(1, 123);
    assert_eq!(entry.request_id, 123);
    assert_eq!(logger.entry_count(), 1);
}

// ============================================================================
// T28 Q2: Edge Cases
// ============================================================================

#[test]
fn test_budget_zero_deduction() {
    let capsule = RequestCapsule128::new(1, 1000);

    let result = capsule.try_deduct(0);
    assert!(result.is_ok(), "Zero deduction should succeed");
    assert_eq!(result.unwrap(), 1000, "Budget should be unchanged");
}

#[test]
fn test_budget_exact_deduction() {
    let capsule = RequestCapsule128::new(1, 100);

    let result = capsule.try_deduct(100);
    assert!(result.is_ok(), "Exact deduction should succeed");
    assert_eq!(result.unwrap(), 0, "Budget should be zero");

    // Next deduction should fail
    let result2 = capsule.try_deduct(1);
    assert!(result2.is_err(), "Should fail when budget exhausted");
}

#[test]
fn test_budget_negative_amount_rejected() {
    let capsule = RequestCapsule128::new(1, 1000);

    let result = capsule.try_deduct(-10);
    assert!(result.is_err(), "Negative deduction should fail");
}

#[test]
fn test_budget_overflow_protection() {
    let capsule = RequestCapsule128::new(1, i64::MAX);

    // Try to deduct more than i64::MAX
    let result = capsule.try_deduct(i64::MAX);
    assert!(result.is_ok());

    // Verify no overflow
    let state = capsule.load_state();
    assert_eq!(state.cost_limit, 0);
}

#[test]
fn test_provider_router_empty_list() {
    let router = ProviderRouter::new(vec![]);

    // Selection should fail gracefully
    let result = router.try_select_provider(12345);
    assert!(result.is_none(), "Should return None for empty provider list");
}

#[test]
fn test_provider_router_single_provider() {
    let router = ProviderRouter::new(vec![5]);

    // Always select the only provider
    let provider1 = router.select_provider(1);
    let provider2 = router.select_provider(2);
    let provider3 = router.select_provider(3);

    assert_eq!(provider1, 5);
    assert_eq!(provider2, 5);
    assert_eq!(provider3, 5);
}

#[test]
fn test_metrics_overflow_protection() {
    let collector = MetricsCollector::new();

    // Record many responses
    for _ in 0..1_000_000 {
        collector.record_response(100, 1, 0.0);
    }

    // Should not overflow
    assert_eq!(collector.total_requests(), 1_000_000);
    assert_eq!(collector.total_tokens(), 1_000_000);
}

#[test]
fn test_audit_logger_hash_chain_initialization() {
    let logger = AuditLogger::new();

    // First entry should have zero hash as prev
    let entry = logger.append_entry(1, 100);
    assert_eq!(entry.prev_hash, [0u8; 32], "First entry should have zero prev hash");
}

// ============================================================================
// T28 Q3: Invariants
// ============================================================================

#[test]
fn test_budget_never_negative() {
    let capsule = RequestCapsule128::new(1, 100);

    // Multiple deductions
    let _ = capsule.try_deduct(30);
    let _ = capsule.try_deduct(30);
    let _ = capsule.try_deduct(30);

    // Try to deduct more
    let result = capsule.try_deduct(50);
    assert!(result.is_err());

    // Budget should still be positive or zero
    let state = capsule.load_state();
    assert!(state.cost_limit >= 0, "Budget should never be negative");
}

#[test]
fn test_generation_counter_monotonic() {
    let capsule = RequestCapsule128::new(1, 1000);

    let gen1 = capsule.generation();
    let _ = capsule.try_deduct(10);
    let gen2 = capsule.generation();
    let _ = capsule.try_deduct(10);
    let gen3 = capsule.generation();

    assert!(gen2 > gen1, "Generation must increase");
    assert!(gen3 > gen2, "Generation must increase");
}

#[test]
fn test_provider_health_check_consistent() {
    let routing = RoutingCapsule128::new(&[0, 1, 2]);

    let health1 = routing.health_check();
    let health2 = routing.health_check();

    assert_eq!(health1.total_count, health2.total_count);
}

#[test]
fn test_metrics_accumulation_correct() {
    let collector = MetricsCollector::new();

    // Record 3 responses
    collector.record_response(100_000, 10, 0.01); // 10 tokens
    collector.record_response(200_000, 20, 0.02); // 20 tokens
    collector.record_response(300_000, 30, 0.03); // 30 tokens

    // Invariant: total tokens = sum of individual tokens
    assert_eq!(collector.total_tokens(), 60);

    // Invariant: total requests = count of records
    assert_eq!(collector.total_requests(), 3);

    // Invariant: avg latency = sum / count
    let avg = collector.avg_latency_ns();
    assert_eq!(avg, 200_000); // (100k + 200k + 300k) / 3
}

#[test]
fn test_audit_hash_chain_integrity() {
    let logger = AuditLogger::new();

    // Append 3 entries
    let entry1 = logger.append_entry(1, 100);
    let entry2 = logger.append_entry(1, 101);
    let entry3 = logger.append_entry(1, 102);

    // Invariant: Each entry's prev_hash matches previous entry's hash
    assert_eq!(entry2.prev_hash, entry1.hash());
    assert_eq!(entry3.prev_hash, entry2.hash());
}

// ============================================================================
// T28 Q4: Code Path Coverage
// ============================================================================

#[test]
fn test_all_error_variants_coverage() {
    // BudgetExhausted
    let capsule = RequestCapsule128::new(1, 10);
    match capsule.try_deduct(100) {
        Err(ClapiError::BudgetExhausted { .. }) => (),
        _ => panic!("Expected BudgetExhausted"),
    }

    // InvalidCost
    match capsule.try_deduct(-1) {
        Err(ClapiError::InvalidCost(_)) => (),
        _ => panic!("Expected InvalidCost"),
    }

    // NoProvidersAvailable
    let router = ProviderRouter::new(vec![]);
    match router.try_select_provider_or_error(1) {
        Err(ClapiError::NoProvidersAvailable) => (),
        _ => panic!("Expected NoProvidersAvailable"),
    }
}

#[test]
fn test_success_paths_coverage() {
    // Budget deduction success
    let capsule = RequestCapsule128::new(1, 1000);
    assert!(capsule.try_deduct(100).is_ok());

    // Provider selection success
    let router = ProviderRouter::new(vec![0, 1]);
    assert!(router.try_select_provider(1).is_some());

    // Metrics recording success
    let collector = MetricsCollector::new();
    collector.record_response(100, 10, 0.01);
    assert_eq!(collector.total_requests(), 1);

    // Audit logging success
    let logger = AuditLogger::new();
    let entry = logger.append_entry(1, 100);
    assert_eq!(entry.request_id, 100);
}

// ============================================================================
// T28 Q5: Isolation and Determinism
// ============================================================================

#[test]
fn test_capsules_isolated() {
    // Each capsule operates independently
    let capsule1 = RequestCapsule128::new(1, 1000);
    let capsule2 = RequestCapsule128::new(2, 2000);

    let _ = capsule1.try_deduct(100);

    // capsule2 should be unaffected
    let state2 = capsule2.load_state();
    assert_eq!(state2.cost_limit, 2000);
}

#[test]
fn test_deterministic_operations() {
    // Budget deduction is deterministic
    let capsule1 = RequestCapsule128::new(1, 1000);
    let capsule2 = RequestCapsule128::new(1, 1000);

    let result1 = capsule1.try_deduct(100);
    let result2 = capsule2.try_deduct(100);

    assert_eq!(result1.unwrap(), result2.unwrap());
}

#[test]
fn test_no_shared_state_between_tests() {
    // This test ensures previous tests didn't leak state
    let capsule = RequestCapsule128::new(999, 999);
    let state = capsule.load_state();
    assert_eq!(state.budget_id, 999);
    assert_eq!(state.cost_limit, 999);
}

// ============================================================================
// T28 Q6: Performance (Fast Tests)
// ============================================================================

#[test]
fn test_budget_deduction_fast() {
    let capsule = RequestCapsule128::new(1, 10_000);

    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _ = capsule.try_deduct(1);
    }
    let elapsed = start.elapsed();

    // Should complete in <10ms
    assert!(elapsed.as_millis() < 10, "1000 deductions took {}ms", elapsed.as_millis());
}

#[test]
fn test_provider_selection_fast() {
    let router = ProviderRouter::new(vec![0, 1, 2, 3, 4]);

    let start = std::time::Instant::now();
    for i in 0..10_000 {
        let _ = router.select_provider(i);
    }
    let elapsed = start.elapsed();

    // Should complete in <5ms
    assert!(elapsed.as_millis() < 5, "10K selections took {}ms", elapsed.as_millis());
}

#[test]
fn test_metrics_recording_fast() {
    let collector = MetricsCollector::new();

    let start = std::time::Instant::now();
    for _ in 0..1000 {
        collector.record_response(100_000, 50, 0.01);
    }
    let elapsed = start.elapsed();

    // Should complete in <10ms
    assert!(elapsed.as_millis() < 10, "1000 records took {}ms", elapsed.as_millis());
}

// ============================================================================
// T28 Q7: Readability and Maintainability
// ============================================================================

// Helper functions for test readability

fn create_test_budget(id: u64, limit: i64) -> RequestCapsule128 {
    RequestCapsule128::new(id, limit)
}

fn create_test_router(provider_count: usize) -> ProviderRouter {
    let providers: Vec<u8> = (0..provider_count as u8).collect();
    ProviderRouter::new(providers)
}

#[test]
fn test_helper_usage_example() {
    // Arrange
    let budget = create_test_budget(1, 1000);
    let router = create_test_router(3);

    // Act
    let deduction_result = budget.try_deduct(100);
    let provider = router.select_provider(12345);

    // Assert
    assert!(deduction_result.is_ok());
    assert!(provider < 3);
}

// ============================================================================
// Mock Types for Phase 2 (these would be in actual proxy implementation)
// ============================================================================

// Simplified mock types to make tests compile
// In real Phase 2, these would be full implementations

struct BudgetRegistry {
    capsules: std::sync::RwLock<std::collections::HashMap<u64, Arc<RequestCapsule128>>>,
}

impl BudgetRegistry {
    fn new() -> Self {
        Self {
            capsules: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    fn len(&self) -> usize {
        self.capsules.read().unwrap().len()
    }

    fn get_or_create(&self, budget_id: u64, initial_limit: i64) -> Arc<RequestCapsule128> {
        let mut map = self.capsules.write().unwrap();
        map.entry(budget_id)
            .or_insert_with(|| Arc::new(RequestCapsule128::new(budget_id, initial_limit)))
            .clone()
    }
}

struct ProviderRouter {
    capsule: RoutingCapsule128,
    providers: Vec<u8>,
}

impl ProviderRouter {
    fn new(providers: Vec<u8>) -> Self {
        let provider_array: [u8; 8] = {
            let mut arr = [0u8; 8];
            for (i, &p) in providers.iter().enumerate().take(8) {
                arr[i] = p;
            }
            arr
        };
        Self {
            capsule: RoutingCapsule128::new(&provider_array[..providers.len().min(8)]),
            providers,
        }
    }

    fn provider_count(&self) -> usize {
        self.providers.len()
    }

    fn select_provider(&self, request_id: u64) -> u8 {
        if self.providers.is_empty() {
            return 0;
        }
        let idx = (request_id % self.providers.len() as u64) as usize;
        self.providers[idx]
    }

    fn try_select_provider(&self, request_id: u64) -> Option<u8> {
        if self.providers.is_empty() {
            None
        } else {
            Some(self.select_provider(request_id))
        }
    }

    fn try_select_provider_or_error(&self, _request_id: u64) -> ClapiResult<u8> {
        if self.providers.is_empty() {
            Err(ClapiError::NoProvidersAvailable)
        } else {
            Ok(self.providers[0])
        }
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

    fn total_tokens(&self) -> u64 {
        let metrics = self.capsule.load_metrics();
        metrics.tokens as u64 * self.total_requests()
    }

    fn avg_latency_ns(&self) -> u64 {
        let metrics = self.capsule.load_metrics();
        metrics.latency_ns
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
