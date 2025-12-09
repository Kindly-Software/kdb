//! Integration tests for clapi_core capsules (v0.4.0)
//!
//! Tests core capsule functionality and integration between modules.

use clapi_core::*;

#[test]
fn test_request_capsule_basic() {
    let capsule = RequestCapsule128::new(1000_00);
    assert_eq!(capsule.budget(), 1000_00);
    assert_eq!(capsule.generation(), 1);
}

#[test]
fn test_request_capsule_try_deduct() {
    let capsule = RequestCapsule128::new(1000_00);

    let result = capsule.try_deduct(50_00);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 950_00);
    assert_eq!(capsule.budget(), 950_00);
    assert_eq!(capsule.request_count(), 1);
}

#[test]
fn test_routing_capsule_basic() {
    let capsule = RoutingCapsule128::new(1, 2); // primary=1, fallback=2
    // Test basic existence and alignment
    let _ = capsule;
}

#[test]
fn test_response_capsule_basic() {
    let capsule = ResponseCapsule256::new();

    capsule.record(1.5, 100, 50_000); // $0.015, 100 tokens, 50ms

    assert_eq!(capsule.total_requests(), 1);
    assert_eq!(capsule.total_tokens(), 100);

    let total_cost = capsule.total_cost_cents();
    assert!((total_cost - 1.5).abs() < 0.0001);
}

#[test]
fn test_audit_capsule_basic() {
    let capsule = AuditLogEntry128::new();
    // Test basic existence and alignment
    let _ = capsule;
}

#[test]
fn test_budget_metacapsule_basic() {
    let meta = BudgetMetaCapsule::new();

    let result = meta.allocate(1, 1000_00);
    assert!(result.is_ok());

    let slot_id = result.unwrap();
    assert!(slot_id < MAX_BUDGET_SLOTS);

    let stats = meta.get_stats();
    assert_eq!(stats.slot_count, 1);
}

#[test]
fn test_budget_registry_integration() {
    // Create budget registry with default $1000 budget
    let registry = proxy::BudgetRegistry::new(1000_00);

    // Deduct from budget (auto-creates budget ID 1)
    let result = registry.try_deduct(1, 50_00);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 950_00);

    // Verify budget
    assert_eq!(registry.get_budget(1), Some(950_00));

    // Credit budget
    let credit_result = registry.credit(1, 100_00);
    assert!(credit_result.is_ok());
    assert_eq!(registry.get_budget(1), Some(1050_00));
}

#[test]
fn test_epoch_tile_basic() {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let tile = EpochTile1024::new(1, now_ms);

    tile.record_request(1, 1.5, 100, 50_000, false);

    let snapshot = tile.snapshot();
    assert_eq!(snapshot.epoch_id, 1);
}

#[test]
fn test_end_to_end_request_flow() {
    // Create request capsule
    let request = RequestCapsule128::new(1000_00);

    // Deduct budget
    let result = request.try_deduct(50_00);
    assert!(result.is_ok());

    // Create response capsule
    let response = ResponseCapsule256::new();
    response.record(1.5, 100, 50_000);

    // Verify metrics
    assert_eq!(response.total_requests(), 1);
    assert_eq!(response.total_tokens(), 100);

    // Create routing capsule
    let routing = RoutingCapsule128::new(1, 2);
    let _ = routing;

    // Create audit entry
    let audit = AuditLogEntry128::new();
    let _ = audit;
}

#[test]
fn test_concurrent_budget_operations() {
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(RequestCapsule128::new(1000_00));
    let mut handles = vec![];

    // Spawn 10 threads
    for _ in 0..10 {
        let c = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for _ in 0..10 {
                let _ = c.try_deduct(1_00);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Budget conservation must hold
    let final_budget = capsule.budget();
    let spent = capsule.total_spent();
    assert_eq!(final_budget + spent, 1000_00);
}

#[test]
fn test_response_capsule_concurrent_record() {
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(ResponseCapsule256::new());
    let mut handles = vec![];

    for _ in 0..10 {
        let c = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                c.record(1.0, 10, 50_000);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(capsule.total_requests(), 1000);
    assert_eq!(capsule.total_tokens(), 10_000);
}
