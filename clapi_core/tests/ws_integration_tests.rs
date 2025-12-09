//! WebSocket Integration Tests (Phase 3)
//!
//! # T28 Testing Framework (Q15-Q21 Integration Tests)
//! - Q15: Integration test strategy (E2E WebSocket flow)
//! - Q16: Cross-component testing (broadcast → WebSocket → WASM)
//! - Q17: Error scenarios (connection drops, backpressure, serialization errors)
//! - Q18: Performance validation (B32 benchmarks)
//! - Q19: Stress testing (10K connections, 100K messages)
//! - Q20: Production simulation (realistic message rates)
//! - Q21: Monitoring integration (metrics validation)
//!
//! # Test Coverage
//! 1. test_single_connection_broadcast - Minimal integration test
//! 2. test_multiple_connections_broadcast - Multi-subscriber test
//! 3. test_connection_cleanup - Verify counter decrements on drop
//! 4. test_backpressure_lagging_receiver - Slow receiver handling
//! 5. test_broadcast_with_no_receivers - Edge case validation
//! 6. test_concurrent_connections - Stress test (100 connections)
//! 7. test_message_serialization_roundtrip - Binary format validation
//! 8. test_broadcast_stats_accuracy - Metrics correctness

use clapi_core::proxy::ws::{
    BroadcastState, MetricsMessage, get_broadcast_stats,
};
use clapi_core::capsules::metrics_snapshot::MetricsSnapshotData;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// T28 Q16: Minimal integration test (single connection, single broadcast)
///
/// # I20 Integration (Q16)
/// - Q16: What's the minimal integration test?
/// - Answer: Single connection, single broadcast, verify message received
#[tokio::test]
async fn test_single_connection_broadcast() {
    let broadcast_state = Arc::new(BroadcastState::new(1000));

    // Subscribe single receiver
    let mut rx = broadcast_state.subscribe();

    // Increment connection counter (simulate connection)
    broadcast_state.increment_connections();
    assert_eq!(broadcast_state.connection_count(), 1);

    // Create metrics message
    let message = MetricsMessage {
        generation: 1,
        timestamp_ns: 1234567890,
        metrics: MetricsSnapshotData {
            deductions_total: 100,
            failures_total: 10,
            circuit_trips_total: 2,
            window_deductions: 50,
            window_failures: 5,
            window_cost_cents: 500,
            latency_p50_ns: 100_000,
            latency_p99_ns: 500_000,
            success_rate_bp: 9000,
            failure_rate_bp: 1000,
        },
    };

    // Broadcast message
    let result = broadcast_state.broadcast(message.clone());
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1); // 1 active receiver

    // Receive message
    let received = rx.recv().await.unwrap();
    assert_eq!(received.generation, message.generation);
    assert_eq!(received.timestamp_ns, message.timestamp_ns);
    assert_eq!(
        received.metrics.deductions_total,
        message.metrics.deductions_total
    );

    // Cleanup
    broadcast_state.decrement_connections();
    assert_eq!(broadcast_state.connection_count(), 0);
}

/// T28 Q16: Multi-subscriber integration test
///
/// # I20 Integration (Q17)
/// - Q17: What property invariants validate composition?
/// - Answer: All subscribers receive all messages (no lost messages)
#[tokio::test]
async fn test_multiple_connections_broadcast() {
    let broadcast_state = Arc::new(BroadcastState::new(1000));

    // Subscribe 5 receivers (simulate 5 connections)
    let mut receivers = vec![];
    for _ in 0..5 {
        let rx = broadcast_state.subscribe();
        receivers.push(rx);
        broadcast_state.increment_connections();
    }

    assert_eq!(broadcast_state.connection_count(), 5);

    // Broadcast 10 messages
    for i in 0..10 {
        let message = MetricsMessage {
            generation: i,
            timestamp_ns: i as u64 * 1000,
            metrics: MetricsSnapshotData {
                deductions_total: i as u64,
                failures_total: 0,
                circuit_trips_total: 0,
                window_deductions: 0,
                window_failures: 0,
                window_cost_cents: 0,
                latency_p50_ns: 0,
                latency_p99_ns: 0,
                success_rate_bp: 10000,
                failure_rate_bp: 0,
            },
        };

        let result = broadcast_state.broadcast(message);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 5); // 5 active receivers
    }

    // All receivers should receive all 10 messages
    for rx in &mut receivers {
        for expected_gen in 0..10 {
            let received = rx.recv().await.unwrap();
            assert_eq!(received.generation, expected_gen);
        }
    }

    // Cleanup
    for _ in 0..5 {
        broadcast_state.decrement_connections();
    }
    assert_eq!(broadcast_state.connection_count(), 0);
}

/// T28 Q17: Connection cleanup test
///
/// # I20 Integration (Q13)
/// - Q13: What boundary invariants must hold?
/// - Answer: Connection count always accurate, no leaks on drop
#[tokio::test]
async fn test_connection_cleanup() {
    let broadcast_state = Arc::new(BroadcastState::new(1000));

    // Simulate 100 connections
    for _ in 0..100 {
        broadcast_state.increment_connections();
    }
    assert_eq!(broadcast_state.connection_count(), 100);

    // Simulate 100 disconnections
    for _ in 0..100 {
        broadcast_state.decrement_connections();
    }
    assert_eq!(broadcast_state.connection_count(), 0);

    // No leaks
    assert_eq!(broadcast_state.connection_count(), 0);
}

/// T28 Q17: Backpressure test (lagging receiver)
///
/// # I20 Integration (Q12)
/// - Q12: How do component failures cascade?
/// - Answer: Slow receivers drop messages (logged), no cascade to fast receivers
#[tokio::test]
async fn test_backpressure_lagging_receiver() {
    let broadcast_state = Arc::new(BroadcastState::new(10)); // Small capacity for backpressure

    // Fast receiver (reads immediately)
    let mut fast_rx = broadcast_state.subscribe();

    // Slow receiver (never reads, will lag)
    let _slow_rx = broadcast_state.subscribe();

    broadcast_state.increment_connections();
    broadcast_state.increment_connections();

    // Broadcast 20 messages (exceeds capacity of 10)
    for i in 0..20 {
        let message = MetricsMessage {
            generation: i,
            timestamp_ns: i as u64,
            metrics: MetricsSnapshotData {
                deductions_total: 0,
                failures_total: 0,
                circuit_trips_total: 0,
                window_deductions: 0,
                window_failures: 0,
                window_cost_cents: 0,
                latency_p50_ns: 0,
                latency_p99_ns: 0,
                success_rate_bp: 10000,
                failure_rate_bp: 0,
            },
        };

        let result = broadcast_state.broadcast(message);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2); // 2 active receivers
    }

    // Fast receiver should receive messages (may skip some due to backpressure)
    // With a capacity of 10 and 20 messages sent, some messages will be dropped
    let mut received_count = 0;
    loop {
        match fast_rx.try_recv() {
            Ok(_) => {
                received_count += 1;
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                break; // No more messages
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {
                // Expected with small capacity
                continue;
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Closed) => {
                break; // Channel closed
            }
        }
    }

    // Should have received at least some messages (not all 20 due to backpressure)
    assert!(received_count > 0);
    assert!(received_count <= 20);

    // Slow receiver will lag (broadcast::error::RecvError::Lagged)
    // This is tested in the actual WebSocket handler, not here
}

/// T28 Q18: Broadcast with no receivers (edge case)
///
/// # I20 Integration (Q10)
/// - Q10: What breaks at the boundaries?
/// - Answer: Broadcasting with no receivers returns Err (no panic)
#[tokio::test]
async fn test_broadcast_with_no_receivers() {
    let broadcast_state = Arc::new(BroadcastState::new(1000));

    let message = MetricsMessage {
        generation: 1,
        timestamp_ns: 0,
        metrics: MetricsSnapshotData {
            deductions_total: 0,
            failures_total: 0,
            circuit_trips_total: 0,
            window_deductions: 0,
            window_failures: 0,
            window_cost_cents: 0,
            latency_p50_ns: 0,
            latency_p99_ns: 0,
            success_rate_bp: 10000,
            failure_rate_bp: 0,
        },
    };

    // No receivers, should return Err
    let result = broadcast_state.broadcast(message);
    assert!(result.is_err());

    // Counters still updated
    assert_eq!(broadcast_state.messages_broadcast(), 1);
    assert_eq!(broadcast_state.messages_dropped(), 1);
}

/// T28 Q19: Stress test (concurrent connections)
///
/// # I20 Integration (Q18)
/// - Q18: What's the acceptable overhead budget? (B32)
/// - Answer: <10ms broadcast latency, 100K+ msg/s throughput
#[tokio::test]
async fn test_concurrent_connections() {
    let broadcast_state = Arc::new(BroadcastState::new(10_000));

    // Spawn 100 concurrent receivers
    let mut handles = vec![];
    for _ in 0..100 {
        let state = Arc::clone(&broadcast_state);
        let handle = tokio::spawn(async move {
            let mut rx = state.subscribe();
            state.increment_connections();

            // Receive 100 messages
            for expected_gen in 0..100 {
                let received = rx.recv().await.unwrap();
                assert_eq!(received.generation, expected_gen);
            }

            state.decrement_connections();
        });
        handles.push(handle);
    }

    // Wait for all receivers to be ready
    sleep(Duration::from_millis(50)).await;

    // Broadcast 100 messages
    for i in 0..100 {
        let message = MetricsMessage {
            generation: i,
            timestamp_ns: i as u64,
            metrics: MetricsSnapshotData {
                deductions_total: 0,
                failures_total: 0,
                circuit_trips_total: 0,
                window_deductions: 0,
                window_failures: 0,
                window_cost_cents: 0,
                latency_p50_ns: 0,
                latency_p99_ns: 0,
                success_rate_bp: 10000,
                failure_rate_bp: 0,
            },
        };

        let result = broadcast_state.broadcast(message);
        assert!(result.is_ok());
    }

    // Wait for all receivers to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // All connections cleaned up
    assert_eq!(broadcast_state.connection_count(), 0);

    // No dropped messages (all receivers kept up)
    assert_eq!(broadcast_state.messages_dropped(), 0);
}

/// T28 Q20: Message serialization roundtrip test
///
/// # I20 Integration (Q14)
/// - Q14: What are the new race/deadlock risks?
/// - Answer: None (tokio async coordination, bincode deterministic)
#[test]
fn test_message_serialization_roundtrip() {
    let message = MetricsMessage {
        generation: 42,
        timestamp_ns: 1234567890,
        metrics: MetricsSnapshotData {
            deductions_total: 100,
            failures_total: 10,
            circuit_trips_total: 2,
            window_deductions: 50,
            window_failures: 5,
            window_cost_cents: 500,
            latency_p50_ns: 100_000,
            latency_p99_ns: 500_000,
            success_rate_bp: 9000,
            failure_rate_bp: 1000,
        },
    };

    // Serialize to bincode
    let bytes = bincode::serialize(&message).unwrap();
    assert!(bytes.len() < 150); // Compact binary representation

    // Deserialize back
    let deserialized: MetricsMessage = bincode::deserialize(&bytes).unwrap();

    // Validate all fields
    assert_eq!(deserialized.generation, message.generation);
    assert_eq!(deserialized.timestamp_ns, message.timestamp_ns);
    assert_eq!(
        deserialized.metrics.deductions_total,
        message.metrics.deductions_total
    );
    assert_eq!(
        deserialized.metrics.failures_total,
        message.metrics.failures_total
    );
    assert_eq!(
        deserialized.metrics.circuit_trips_total,
        message.metrics.circuit_trips_total
    );
    assert_eq!(
        deserialized.metrics.window_deductions,
        message.metrics.window_deductions
    );
    assert_eq!(
        deserialized.metrics.window_failures,
        message.metrics.window_failures
    );
    assert_eq!(
        deserialized.metrics.window_cost_cents,
        message.metrics.window_cost_cents
    );
    assert_eq!(
        deserialized.metrics.latency_p50_ns,
        message.metrics.latency_p50_ns
    );
    assert_eq!(
        deserialized.metrics.latency_p99_ns,
        message.metrics.latency_p99_ns
    );
    assert_eq!(
        deserialized.metrics.success_rate_bp,
        message.metrics.success_rate_bp
    );
    assert_eq!(
        deserialized.metrics.failure_rate_bp,
        message.metrics.failure_rate_bp
    );
}

/// T28 Q21: Broadcast stats accuracy test
///
/// # I20 Integration (Q15)
/// - Q15: What are the escape hatches/circuit breakers?
/// - Answer: Drop rate monitoring, connection count tracking
#[tokio::test]
async fn test_broadcast_stats_accuracy() {
    let broadcast_state = Arc::new(BroadcastState::new(1000));

    // Initial stats
    let stats = get_broadcast_stats(&broadcast_state);
    assert_eq!(stats.connection_count, 0);
    assert_eq!(stats.messages_broadcast, 0);
    assert_eq!(stats.messages_dropped, 0);
    assert_eq!(stats.drop_rate_bp, 0);

    // Add 10 connections
    for _ in 0..10 {
        broadcast_state.increment_connections();
    }

    // Broadcast without receivers (all dropped)
    for i in 0..100 {
        let message = MetricsMessage {
            generation: i,
            timestamp_ns: i as u64,
            metrics: MetricsSnapshotData {
                deductions_total: 0,
                failures_total: 0,
                circuit_trips_total: 0,
                window_deductions: 0,
                window_failures: 0,
                window_cost_cents: 0,
                latency_p50_ns: 0,
                latency_p99_ns: 0,
                success_rate_bp: 10000,
                failure_rate_bp: 0,
            },
        };
        let _ = broadcast_state.broadcast(message);
    }

    // Updated stats
    let stats = get_broadcast_stats(&broadcast_state);
    assert_eq!(stats.connection_count, 10);
    assert_eq!(stats.messages_broadcast, 100);
    assert_eq!(stats.messages_dropped, 100);
    assert_eq!(stats.drop_rate_bp, 10000); // 100% drop rate
}
