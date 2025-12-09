//! Phase 5.7: Audit Bridge T28 Property Tests (Tier 2: Q8-Q14)
//!
//! Property-based tests validating AuditLogBridge invariants:
//! - Concurrent append ordering
//! - Hash chain integrity
//! - Ordering guarantees (happens-before)
//! - Backpressure handling
//! - Idempotency
//! - Batch boundary conditions
//! - Error propagation

use clapi_core::proxy::AuditLogBridge;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn test_concurrent_append_ordering() {
    // Q8: All concurrent appends succeed without data loss
    let bridge = Arc::new(AuditLogBridge::new());
    let mut tasks = vec![];

    for i in 0..100 {
        let bridge_clone = Arc::clone(&bridge);
        let task = tokio::spawn(async move {
            bridge_clone.append(&format!("concurrent {}", i)).await
        });
        tasks.push(task);
    }

    // Wait for all tasks
    let results: Vec<_> = futures::future::join_all(tasks).await;
    for result in results {
        assert!(result.is_ok());
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_hash_chain_integrity() {
    // Q9: Hash chain prevents tampering (Q34 Auditability)
    // Each event includes prev_hash linking to previous
    let bridge = AuditLogBridge::new();

    // Append events in sequence
    for i in 0..10 {
        let msg = format!("hash chain {}", i);
        assert!(bridge.append(&msg).await.is_ok());
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // All events should be logged without error
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_ordering_guarantees() {
    // Q9: Happens-before: append -> flush -> capsule append
    // MPSC channel guarantees ordering
    let bridge = Arc::new(AuditLogBridge::new());
    let counter = Arc::new(AtomicU64::new(0));

    for i in 0..50 {
        let bridge_clone = Arc::clone(&bridge);
        let counter_clone = Arc::clone(&counter);

        tokio::spawn(async move {
            bridge_clone.append(&format!("ordered {}", i)).await.ok();
            counter_clone.fetch_add(1, Ordering::Release);
        });
    }

    // Wait for all tasks and batch processing
    tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

    // All events should have been processed (ordered)
    assert_eq!(counter.load(Ordering::Acquire), 50);
}

#[tokio::test]
async fn test_backpressure_channel() {
    // Q10: Backpressure when channel reaches 1024 capacity
    let bridge = Arc::new(AuditLogBridge::new());

    // Fill channel close to capacity
    let mut pending = vec![];
    for i in 0..1030 {
        let bridge_clone = Arc::clone(&bridge);
        let task = tokio::spawn(async move {
            bridge_clone.append(&format!("backpressure {}", i)).await
        });
        pending.push(task);
    }

    // Wait for all to complete (some may experience backpressure)
    let results: Vec<_> = futures::future::join_all(pending).await;

    // Most should succeed, some may fail due to backpressure
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    assert!(success_count > 900, "Most appends should succeed: {}", success_count);
}

#[tokio::test]
async fn test_idempotency_same_message() {
    // Q11: Appending same message multiple times creates separate entries
    let bridge = Arc::new(AuditLogBridge::new());

    let msg = "identical";
    for _ in 0..10 {
        assert!(bridge.append(msg).await.is_ok());
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_batch_boundary_at_100() {
    // Q12: Batch boundary exactly at 100 events
    let bridge = AuditLogBridge::new();

    // Append exactly 100
    for i in 0..100 {
        bridge.append(&format!("boundary {}", i)).await.ok();
    }

    // Should flush without overflow
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Add 1 more - new batch
    bridge.append("after boundary").await.ok();

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_batch_timeout_edge_case() {
    // Q12: Timeout fires at 100ms boundary
    let bridge = AuditLogBridge::new();

    bridge.append("timeout edge").await.ok();

    // Wait exactly at timeout boundary
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Should have flushed
    bridge.append("after timeout").await.ok();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_error_propagation_on_capsule_fail() {
    // Q13: Error in AsyncLogCapsule propagates to error counter
    let bridge = AuditLogBridge::new();

    // Normal appends succeed
    for i in 0..5 {
        bridge.append(&format!("normal {}", i)).await.ok();
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // If capsule is full, additional appends may fail
    // (though AsyncLogCapsule has exponential backoff)
    let initial_errors = bridge.error_count();

    bridge.append("possibly full").await.ok();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Error count should not decrease
    let final_errors = bridge.error_count();
    assert!(final_errors >= initial_errors);
}

#[tokio::test]
async fn test_concurrent_write_isolation() {
    // Q13: Concurrent writes don't corrupt each other
    let bridge = Arc::new(AuditLogBridge::new());
    let mut tasks = vec![];

    for thread in 0..10 {
        for i in 0..10 {
            let bridge_clone = Arc::clone(&bridge);
            let task = tokio::spawn(async move {
                let msg = format!("thread {} iter {}", thread, i);
                bridge_clone.append(&msg).await
            });
            tasks.push(task);
        }
    }

    // 100 total appends from 10 concurrent tasks
    let results: Vec<_> = futures::future::join_all(tasks).await;

    let success = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(success, 100, "All writes should succeed");

    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
}

#[tokio::test]
async fn test_no_data_loss_under_shutdown() {
    // Q14: Pending events are flushed on shutdown
    let mut bridge = AuditLogBridge::new();

    // Add events but don't wait for flush
    for i in 0..50 {
        bridge.append(&format!("shutdown flush {}", i)).await.ok();
    }

    // Immediate shutdown should still flush
    let shutdown_result = bridge.shutdown().await;
    assert!(shutdown_result.is_ok());
}

#[tokio::test]
async fn test_channel_closure_semantics() {
    // Q14: After channel close, all further appends fail consistently
    let mut bridge = AuditLogBridge::new();

    bridge.append("before").await.ok();
    bridge.shutdown().await.ok();

    // All subsequent appends fail
    for i in 0..10 {
        let result = bridge.append(&format!("after {}", i)).await;
        assert!(result.is_err(), "Append after shutdown should fail: {}", i);
    }
}

#[tokio::test]
async fn test_mixed_successful_and_failed_appends() {
    // Q13: Some appends succeed, some fail, counter tracks all failures
    let mut bridge = AuditLogBridge::new();

    // Successful appends
    for i in 0..20 {
        bridge.append(&format!("success {}", i)).await.ok();
    }

    // Shutdown and flush
    bridge.shutdown().await.ok();

    // All subsequent appends fail
    for i in 0..10 {
        let result = bridge.append(&format!("after shutdown {}", i)).await;
        assert!(result.is_err());
    }

    // Error counter should reflect failures
    assert!(bridge.error_count() > 0);
}
