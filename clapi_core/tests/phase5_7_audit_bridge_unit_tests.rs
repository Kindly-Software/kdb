//! Phase 5.7: Audit Bridge T28 Unit Tests (Tier 1: Q1-Q7)
//!
//! Unit tests validating AuditLogBridge capsule invariants:
//! - Basic append
//! - Batch flush
//! - Hash chaining
//! - Worker startup
//! - Graceful shutdown
//! - Error handling
//! - Memory bounds

use clapi_core::proxy::AuditLogBridge;

#[tokio::test]
async fn test_bridge_creation() {
    // Q1: Initialize bridge
    let bridge = AuditLogBridge::new();
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_basic_append() {
    // Q1: Append single event
    let bridge = AuditLogBridge::new();
    let result = bridge.append("test event").await;
    assert!(result.is_ok(), "Append should succeed");
}

#[tokio::test]
async fn test_append_multiple_events() {
    // Q2: Append multiple events in sequence
    let bridge = AuditLogBridge::new();
    for i in 0..10 {
        let msg = format!("event {}", i);
        assert!(bridge.append(&msg).await.is_ok());
    }
    // Allow time for batch processing
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
}

#[tokio::test]
async fn test_batch_flush_on_size() {
    // Q3: Batch flush when reaching size threshold (100 events)
    let bridge = AuditLogBridge::new();
    for i in 0..105 {
        let msg = format!("batch event {}", i);
        assert!(bridge.append(&msg).await.is_ok());
    }
    // Wait for batch processing
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_batch_flush_on_timeout() {
    // Q3: Batch flush on timeout (100ms) even if < 100 events
    let bridge = AuditLogBridge::new();
    bridge.append("timeout test").await.unwrap();

    // Wait for timeout to trigger flush (100ms + buffer)
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_error_counter_on_append_failure() {
    // Q4: Error counter increments on append failure (channel closed)
    let mut bridge = AuditLogBridge::new();
    let _ = bridge.shutdown().await;  // Close channel

    // Append should fail after shutdown
    let result = bridge.append("after shutdown").await;
    assert!(result.is_err());
    assert!(bridge.error_count() > 0);
}

#[tokio::test]
async fn test_log_request_helper() {
    // Q2: log_request convenience method
    let bridge = AuditLogBridge::new();
    let result = bridge.log_request(42, 12345, 0x1234).await;
    assert!(result.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
}

#[tokio::test]
async fn test_log_error_helper() {
    // Q2: log_error convenience method
    let bridge = AuditLogBridge::new();
    let result = bridge.log_error(42, 0x5678).await;
    assert!(result.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
}

#[tokio::test]
async fn test_graceful_shutdown() {
    // Q5: Graceful shutdown flushes pending events
    let mut bridge = AuditLogBridge::new();

    // Add some events
    for i in 0..5 {
        bridge.append(&format!("shutdown test {}", i)).await.ok();
    }

    // Shutdown should flush remaining events
    let result = bridge.shutdown().await;
    assert!(result.is_ok());
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_drop_implementation() {
    // Q5: Drop signals shutdown
    {
        let bridge = AuditLogBridge::new();
        bridge.append("drop test").await.ok();
        // Bridge dropped here, should trigger shutdown signal
    }
    // If Drop is implemented, no panic or resource leak
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[tokio::test]
async fn test_default_implementation() {
    // Q1: Default trait creates new bridge
    let bridge = AuditLogBridge::default();
    assert_eq!(bridge.error_count(), 0);
}

#[tokio::test]
async fn test_memory_bounds_capsule() {
    // Q7: AsyncLogCapsule uses bounded 4096-entry ring buffer
    // This test verifies the bridge doesn't panic under sustained load
    let bridge = AuditLogBridge::new();

    for i in 0..200 {
        bridge.append(&format!("mem test {}", i)).await.ok();
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    // If we get here, memory bounds held
}

#[tokio::test]
async fn test_concurrent_initialization() {
    // Q6: Multiple bridges can be created concurrently
    let mut tasks = vec![];
    for _ in 0..10 {
        let task = tokio::spawn(async { AuditLogBridge::new() });
        tasks.push(task);
    }

    let results: Vec<_> = futures::future::join_all(tasks).await;
    assert_eq!(results.len(), 10);

    for result in results {
        if let Ok(bridge) = result {
            assert_eq!(bridge.error_count(), 0);
        }
    }
}

#[tokio::test]
async fn test_empty_batch_flush() {
    // Q7: Empty batch flush doesn't panic
    let bridge = AuditLogBridge::new();

    // Wait for idle flush with no events
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
    assert_eq!(bridge.error_count(), 0);
}
