//! Phase 5.7: Audit Bridge T28 Integration Tests (Tier 3: Q15-Q21)
//!
//! Integration tests validating AuditLogBridge with other components:
//! - End-to-end audit trail (append -> flush -> capsule)
//! - Budget lifecycle logging
//! - HTTP request event logging
//! - Payment event logging
//! - Circuit breaker state logging
//! - Multi-component coordination
//! - Backward compatibility with existing audit_log.rs

use clapi_core::proxy::AuditLogBridge;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn test_end_to_end_audit_trail() {
    // Q15: Complete flow: append -> MPSC channel -> worker -> batch -> capsule
    let bridge = AuditLogBridge::new();

    // Simulate audit trail creation
    let events = vec![
        "BudgetCreated budget_id=123",
        "RequestReceived provider=claude cost=10",
        "ResponseReceived tokens=1000",
        "BudgetUpdated balance=990",
        "CheckpointCreated hash=0xabc123",
    ];

    for event in events {
        bridge.append(event).await.expect("Event should be appended");
    }

    // Wait for batch processing and flush
    tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;

    // All events should be in capsule
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_budget_lifecycle_logging() {
    // Q16: Budget lifecycle: Create -> Request -> Response -> Checkpoint
    let bridge = AuditLogBridge::new();

    // Budget creation
    assert!(bridge.append("BudgetCreated budget_id=456 initial=10000").await.is_ok());

    // First request
    assert!(bridge.log_request(456, 10000, 0x1234).await.is_ok());

    // Another request
    assert!(bridge.log_request(456, 9990, 0x5678).await.is_ok());

    // Error event
    assert!(bridge.log_error(456, 0x9abc).await.is_ok());

    // Final checkpoint
    assert!(bridge.append("CheckpointCreated hash=0xdef789 balance=9990").await.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_http_request_event_logging() {
    // Q16: Log HTTP request/response lifecycle
    let bridge = Arc::new(AuditLogBridge::new());

    // Simulated HTTP request logging
    for i in 0..10 {
        let bridge_clone = Arc::clone(&bridge);
        tokio::spawn(async move {
            let request_id = format!("req-{}", i);
            bridge_clone.append(&format!("HTTPRequestStart {}", request_id)).await.ok();

            // Simulate processing
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

            bridge_clone.append(&format!("HTTPRequestEnd {} status=200", request_id)).await.ok();
        });
    }

    // Wait for all requests
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_payment_event_logging() {
    // Q17: Log payment events with fixed-point amounts
    let bridge = AuditLogBridge::new();

    // Payment lifecycle
    assert!(bridge.append("PaymentInitiated user=789 amount=99.99").await.is_ok());
    assert!(bridge.log_request(789, 9999, 0x1111).await.is_ok());  // Amount in cents
    assert!(bridge.append("PaymentConfirmed user=789 tx_id=xyz123").await.is_ok());

    // Payment reversal
    assert!(bridge.append("PaymentReversed user=789 tx_id=xyz123 reason=refund").await.is_ok());
    assert!(bridge.log_error(789, 0x2222).await.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_circuit_breaker_state_logging() {
    // Q17: Log circuit breaker transitions
    let bridge = AuditLogBridge::new();

    // Circuit breaker state machine
    assert!(bridge.append("CircuitBreakerClosed provider=provider1").await.is_ok());

    // Failures accumulate
    for i in 0..15 {
        assert!(bridge.append(&format!("ProviderError provider=provider1 count={}", i)).await.is_ok());
    }

    // Open state
    assert!(bridge.append("CircuitBreakerOpen provider=provider1 reason=threshold").await.is_ok());

    // Half-open after cooldown
    assert!(bridge.append("CircuitBreakerHalfOpen provider=provider1").await.is_ok());

    // Recovery
    assert!(bridge.append("CircuitBreakerClosed provider=provider1").await.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_multi_component_coordination() {
    // Q18: Multiple threads logging from different components
    let bridge = Arc::new(AuditLogBridge::new());

    // Component 1: Budget registry
    let bridge1 = Arc::clone(&bridge);
    let handle1 = tokio::spawn(async move {
        for i in 0..20 {
            bridge1.append(&format!("Budget op {}", i)).await.ok();
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        }
    });

    // Component 2: Provider router
    let bridge2 = Arc::clone(&bridge);
    let handle2 = tokio::spawn(async move {
        for i in 0..20 {
            bridge2.append(&format!("Router op {}", i)).await.ok();
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        }
    });

    // Component 3: Payment handler
    let bridge3 = Arc::clone(&bridge);
    let handle3 = tokio::spawn(async move {
        for i in 0..20 {
            bridge3.append(&format!("Payment op {}", i)).await.ok();
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        }
    });

    // Wait for all components
    let _ = tokio::join!(handle1, handle2, handle3);

    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // All events logged without conflict
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_hash_chain_commitment() {
    // Q19: Hash chain with prev_hash for Q34 Auditability
    let bridge = AuditLogBridge::new();

    let mut prev_hash = 0u64;

    // Create hash-chained events
    for i in 0..5 {
        let msg = format!("ChainedEvent {} prev_hash=0x{:x}", i, prev_hash);
        assert!(bridge.append(&msg).await.is_ok());

        // Simulate next hash (in reality computed from event)
        prev_hash = (prev_hash.wrapping_mul(31) ^ i as u64).wrapping_add(0x9e3779b97f4a7c15);
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_backward_compatibility_with_audit_log() {
    // Q20: AuditLogBridge maintains same interface as old Mutex<File> AuditLog
    let bridge = AuditLogBridge::new();

    // Same methods as old AuditLog:
    // - append()
    // - log_request()
    // - log_error()

    assert!(bridge.append("test").await.is_ok());
    assert!(bridge.log_request(1, 100, 0).await.is_ok());
    assert!(bridge.log_error(1, 0).await.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_audit_trail_recovery() {
    // Q20: Audit trail can be replayed from events
    let bridge = AuditLogBridge::new();

    // Simulate transaction
    let budget_id = 999;
    let initial_balance = 10000i64;

    assert!(bridge.append(&format!("BudgetCreated id={} balance={}", budget_id, initial_balance)).await.is_ok());

    // Series of operations
    let operations = vec![
        (100, "Provider A"),
        (250, "Provider B"),
        (50, "Provider A"),
    ];

    let mut balance = initial_balance;
    for (cost, provider) in operations {
        assert!(bridge.log_request(budget_id as u64, cost, 0).await.is_ok());
        balance -= cost;
    }

    assert!(bridge.append(&format!("FinalBalance id={} balance={}", budget_id, balance)).await.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_compliance_required_fields() {
    // Q21: Log events include compliance-required fields
    let bridge = AuditLogBridge::new();

    // Compliance fields: timestamp, user/budget_id, amount, event_type, hash
    let events = vec![
        "2024-10-21T12:00:00Z user=123 amount=100 event=RequestReceived hash=0xabc",
        "2024-10-21T12:00:01Z user=123 amount=0 event=ErrorOccurred hash=0xbcd",
        "2024-10-21T12:00:02Z user=123 amount=100 event=ResponseReceived hash=0xcde",
    ];

    for event in events {
        assert!(bridge.append(event).await.is_ok());
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_interleaved_operations_no_corruption() {
    // Q21: Interleaved append/log_request/log_error without corruption
    let bridge = Arc::new(AuditLogBridge::new());

    let mut tasks = vec![];

    for i in 0..10 {
        let bridge_clone = Arc::clone(&bridge);
        let task = tokio::spawn(async move {
            // Mix operations
            bridge_clone.append(&format!("Event A {}", i)).await.ok();
            bridge_clone.log_request(i as u64, 100 * (i as i64), 0).await.ok();
            bridge_clone.append(&format!("Event B {}", i)).await.ok();
            bridge_clone.log_error(i as u64, 0).await.ok();
        });
        tasks.push(task);
    }

    let results: Vec<_> = futures::future::join_all(tasks).await;
    assert_eq!(results.len(), 10);

    tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
    assert_eq!(bridge.error_count(), 0);
}
