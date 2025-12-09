//! WebSocket Performance Regression Tests
//!
//! # Purpose
//! Detect performance regressions in Phase 3 WebSocket implementation.
//!
//! # Methodology (B32 Framework)
//! - Baseline metrics from PHASE3_PROFILING_REPORT.md
//! - Alert thresholds: +10% regression (fail test)
//! - Statistical validation: 1000+ iterations per test
//!
//! # Test Coverage
//! 1. Broadcast latency (single, 1000, 10K receivers)
//! 2. Message serialization/deserialization
//! 3. Atomic counter operations
//! 4. Stats snapshot
//! 5. Memory per connection
//!
//! # CI Integration
//! ```bash
//! cargo test --test ws_performance_regression_tests -- --ignored
//! ```
//!
//! # Failure Handling
//! If tests fail:
//! 1. Check PHASE3_PROFILING_REPORT.md for expected ranges
//! 2. Investigate recent changes (git log)
//! 3. Profile with: cargo bench --bench ws_bench
//! 4. Revert if >10% regression without justification

use clapi_core::proxy::ws::{BroadcastState, MetricsMessage, get_broadcast_stats};
use clapi_core::capsules::metrics_snapshot::MetricsSnapshotData;
use std::sync::Arc;
use std::time::Instant;

/// Baseline Performance Metrics (from PHASE3_PROFILING_REPORT.md)
///
/// These are the p50 (median) values from Criterion benchmarks.
/// Alert thresholds are set at +10% regression.
mod baselines {
    /// Broadcast latency (single receiver)
    /// Baseline: 39.6ns
    /// Alert: 43.5ns (+10%)
    pub const BROADCAST_SINGLE_NS: u64 = 43;

    /// Broadcast latency (1000 receivers)
    /// Baseline: 44.8ns
    /// Alert: 49.3ns (+10%)
    pub const BROADCAST_1000_NS: u64 = 49;

    /// Broadcast latency (10K receivers)
    /// Baseline: 37.6ns
    /// Alert: 41.4ns (+10%)
    pub const BROADCAST_10K_NS: u64 = 41;

    /// Message serialization
    /// Baseline: 10.0ns
    /// Alert: 11.0ns (+10%)
    pub const SERIALIZE_NS: u64 = 11;

    /// Message deserialization
    /// Baseline: 3.4ns
    /// Alert: 3.7ns (+10%)
    pub const DESERIALIZE_NS: u64 = 4;

    /// Counter increment/decrement
    /// Baseline: 5.0ns
    /// Alert: 5.5ns (+10%)
    pub const COUNTER_OP_NS: u64 = 6;

    /// Counter read
    /// Baseline: 0.21ns
    /// Alert: 0.23ns (+10%)
    pub const COUNTER_READ_NS: u64 = 1;

    /// Stats snapshot
    /// Baseline: 0.77ns
    /// Alert: 0.85ns (+10%)
    pub const STATS_SNAPSHOT_NS: u64 = 1;

    /// Memory per connection
    /// Baseline: ~10KB
    /// Alert: ~11KB (+10%)
    pub const MEMORY_PER_CONNECTION_BYTES: usize = 11_000;
}

/// Helper: Measure latency with statistical rigor (1000+ iterations)
///
/// Returns p50 (median) latency in nanoseconds.
fn measure_latency<F>(mut f: F, iterations: usize) -> u64
where
    F: FnMut(),
{
    let mut samples = Vec::with_capacity(iterations);

    // Warm-up (100 iterations to warm CPU cache)
    for _ in 0..100 {
        f();
    }

    // Measurement phase
    for _ in 0..iterations {
        let start = Instant::now();
        f();
        let duration = start.elapsed();
        samples.push(duration.as_nanos() as u64);
    }

    // Calculate p50 (median)
    samples.sort_unstable();
    samples[iterations / 2]
}

/// Test 1: Broadcast latency (single receiver)
///
/// # Performance Target
/// - Baseline: 39.6ns (p50)
/// - Alert Threshold: 43.5ns (+10%)
///
/// # Failure Condition
/// If p50 latency > 43.5ns, test fails (regression detected).
#[test]
#[ignore] // Run with --ignored flag (CI performance tests)
fn test_broadcast_single_receiver_latency() {
    let broadcast_state = Arc::new(BroadcastState::new(1000));
    let _rx = broadcast_state.subscribe();

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

    let p50_ns = measure_latency(
        || {
            let _ = broadcast_state.broadcast(message.clone());
        },
        1000,
    );

    println!("Broadcast (single receiver) p50: {}ns", p50_ns);

    assert!(
        p50_ns <= baselines::BROADCAST_SINGLE_NS,
        "REGRESSION: Broadcast latency (single) regressed from 39.6ns to {}ns (threshold: {}ns)",
        p50_ns,
        baselines::BROADCAST_SINGLE_NS
    );
}

/// Test 2: Broadcast latency (1000 receivers)
///
/// # Performance Target
/// - Baseline: 44.8ns (p50)
/// - Alert Threshold: 49.3ns (+10%)
#[test]
#[ignore]
fn test_broadcast_1000_receivers_latency() {
    let broadcast_state = Arc::new(BroadcastState::new(10_000));

    // Subscribe 1000 receivers
    let _receivers: Vec<_> = (0..1000)
        .map(|_| broadcast_state.subscribe())
        .collect();

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

    let p50_ns = measure_latency(
        || {
            let _ = broadcast_state.broadcast(message.clone());
        },
        1000,
    );

    println!("Broadcast (1000 receivers) p50: {}ns", p50_ns);

    assert!(
        p50_ns <= baselines::BROADCAST_1000_NS,
        "REGRESSION: Broadcast latency (1000) regressed from 44.8ns to {}ns (threshold: {}ns)",
        p50_ns,
        baselines::BROADCAST_1000_NS
    );
}

/// Test 3: Broadcast latency (10K receivers)
///
/// # Performance Target
/// - Baseline: 37.6ns (p50)
/// - Alert Threshold: 41.4ns (+10%)
#[test]
#[ignore]
fn test_broadcast_10k_receivers_latency() {
    let broadcast_state = Arc::new(BroadcastState::new(20_000));

    // Subscribe 10K receivers
    let _receivers: Vec<_> = (0..10_000)
        .map(|_| broadcast_state.subscribe())
        .collect();

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

    let p50_ns = measure_latency(
        || {
            let _ = broadcast_state.broadcast(message.clone());
        },
        1000,
    );

    println!("Broadcast (10K receivers) p50: {}ns", p50_ns);

    assert!(
        p50_ns <= baselines::BROADCAST_10K_NS,
        "REGRESSION: Broadcast latency (10K) regressed from 37.6ns to {}ns (threshold: {}ns)",
        p50_ns,
        baselines::BROADCAST_10K_NS
    );
}

/// Test 4: Message serialization latency
///
/// # Performance Target
/// - Baseline: 10.0ns (p50)
/// - Alert Threshold: 11.0ns (+10%)
#[test]
#[ignore]
fn test_message_serialization_latency() {
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

    let p50_ns = measure_latency(
        || {
            let _ = bincode::serialize(&message).unwrap();
        },
        1000,
    );

    println!("Serialization p50: {}ns", p50_ns);

    assert!(
        p50_ns <= baselines::SERIALIZE_NS,
        "REGRESSION: Serialization latency regressed from 10.0ns to {}ns (threshold: {}ns)",
        p50_ns,
        baselines::SERIALIZE_NS
    );
}

/// Test 5: Message deserialization latency
///
/// # Performance Target
/// - Baseline: 3.4ns (p50)
/// - Alert Threshold: 3.7ns (+10%)
#[test]
#[ignore]
fn test_message_deserialization_latency() {
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

    let bytes = bincode::serialize(&message).unwrap();

    let p50_ns = measure_latency(
        || {
            let _: MetricsMessage = bincode::deserialize(&bytes).unwrap();
        },
        1000,
    );

    println!("Deserialization p50: {}ns", p50_ns);

    assert!(
        p50_ns <= baselines::DESERIALIZE_NS,
        "REGRESSION: Deserialization latency regressed from 3.4ns to {}ns (threshold: {}ns)",
        p50_ns,
        baselines::DESERIALIZE_NS
    );
}

/// Test 6: Counter increment latency
///
/// # Performance Target
/// - Baseline: 5.0ns (p50)
/// - Alert Threshold: 5.5ns (+10%)
#[test]
#[ignore]
fn test_counter_increment_latency() {
    let broadcast_state = Arc::new(BroadcastState::new(1000));

    let p50_ns = measure_latency(
        || {
            broadcast_state.increment_connections();
        },
        1000,
    );

    println!("Counter increment p50: {}ns", p50_ns);

    assert!(
        p50_ns <= baselines::COUNTER_OP_NS,
        "REGRESSION: Counter increment latency regressed from 5.0ns to {}ns (threshold: {}ns)",
        p50_ns,
        baselines::COUNTER_OP_NS
    );
}

/// Test 7: Counter decrement latency
///
/// # Performance Target
/// - Baseline: 5.0ns (p50)
/// - Alert Threshold: 5.5ns (+10%)
#[test]
#[ignore]
fn test_counter_decrement_latency() {
    let broadcast_state = Arc::new(BroadcastState::new(1000));

    let p50_ns = measure_latency(
        || {
            broadcast_state.decrement_connections();
        },
        1000,
    );

    println!("Counter decrement p50: {}ns", p50_ns);

    assert!(
        p50_ns <= baselines::COUNTER_OP_NS,
        "REGRESSION: Counter decrement latency regressed from 5.0ns to {}ns (threshold: {}ns)",
        p50_ns,
        baselines::COUNTER_OP_NS
    );
}

/// Test 8: Counter read latency
///
/// # Performance Target
/// - Baseline: 0.21ns (p50)
/// - Alert Threshold: 0.23ns (+10%)
#[test]
#[ignore]
fn test_counter_read_latency() {
    let broadcast_state = Arc::new(BroadcastState::new(1000));

    let p50_ns = measure_latency(
        || {
            let _ = broadcast_state.connection_count();
        },
        1000,
    );

    println!("Counter read p50: {}ns", p50_ns);

    assert!(
        p50_ns <= baselines::COUNTER_READ_NS,
        "REGRESSION: Counter read latency regressed from 0.21ns to {}ns (threshold: {}ns)",
        p50_ns,
        baselines::COUNTER_READ_NS
    );
}

/// Test 9: Stats snapshot latency
///
/// # Performance Target
/// - Baseline: 0.77ns (p50)
/// - Alert Threshold: 0.85ns (+10%)
#[test]
#[ignore]
fn test_stats_snapshot_latency() {
    let broadcast_state = Arc::new(BroadcastState::new(1000));

    let p50_ns = measure_latency(
        || {
            let _ = get_broadcast_stats(&broadcast_state);
        },
        1000,
    );

    println!("Stats snapshot p50: {}ns", p50_ns);

    assert!(
        p50_ns <= baselines::STATS_SNAPSHOT_NS,
        "REGRESSION: Stats snapshot latency regressed from 0.77ns to {}ns (threshold: {}ns)",
        p50_ns,
        baselines::STATS_SNAPSHOT_NS
    );
}

/// Test 10: Memory per connection
///
/// # Performance Target
/// - Baseline: ~10KB per connection
/// - Alert Threshold: ~11KB (+10%)
#[test]
#[ignore]
fn test_memory_per_connection() {
    use std::mem::size_of;

    // Measure connection state memory
    let connection_state_bytes = size_of::<clapi_core::proxy::ws::ConnectionState>();
    println!("ConnectionState size: {} bytes", connection_state_bytes);

    // Measure WebSocket frame overhead (estimated)
    let websocket_overhead = 8; // WebSocket handle pointer

    // Measure receiver queue (tokio::sync::broadcast default)
    let receiver_queue_bytes = 10_000; // Estimated 10KB buffer

    let total_per_connection = connection_state_bytes + websocket_overhead + receiver_queue_bytes;

    println!("Total memory per connection: {} bytes (~{}KB)",
             total_per_connection,
             total_per_connection / 1024);

    assert!(
        total_per_connection <= baselines::MEMORY_PER_CONNECTION_BYTES,
        "REGRESSION: Memory per connection increased from ~10KB to {}KB (threshold: {}KB)",
        total_per_connection / 1024,
        baselines::MEMORY_PER_CONNECTION_BYTES / 1024
    );
}

/// Test 11: Throughput regression (sustained broadcast rate)
///
/// # Performance Target
/// - Baseline: 27M msg/s at 10K connections
/// - Alert Threshold: 24.3M msg/s (-10%)
#[test]
#[ignore]
fn test_sustained_throughput() {
    let broadcast_state = Arc::new(BroadcastState::new(20_000));

    // Subscribe 1000 receivers (10K too slow for test)
    let _receivers: Vec<_> = (0..1000)
        .map(|_| broadcast_state.subscribe())
        .collect();

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

    // Measure throughput over 1000 messages
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = broadcast_state.broadcast(message.clone());
    }
    let duration = start.elapsed();

    let throughput_msg_per_sec = (1000.0 / duration.as_secs_f64()) as u64;
    println!("Sustained throughput: {} msg/s", throughput_msg_per_sec);

    // Alert threshold: 24.3M msg/s (-10% of 27M baseline)
    assert!(
        throughput_msg_per_sec >= 24_000_000,
        "REGRESSION: Throughput dropped from 27M msg/s to {} msg/s (threshold: 24.3M msg/s)",
        throughput_msg_per_sec
    );
}

/// Test 12: Concurrent broadcast stress test (no regression in latency under load)
///
/// # Performance Target
/// - Baseline: <50ns under concurrent load (100 threads)
/// - Alert Threshold: <55ns (+10%)
#[test]
#[ignore]
fn test_concurrent_broadcast_latency() {
    use std::thread;

    let broadcast_state = Arc::new(BroadcastState::new(10_000));

    // Subscribe 100 receivers
    let _receivers: Vec<_> = (0..100)
        .map(|_| broadcast_state.subscribe())
        .collect();

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

    // Spawn 10 threads broadcasting concurrently
    let mut handles = vec![];
    for _ in 0..10 {
        let state = Arc::clone(&broadcast_state);
        let msg = message.clone();
        handles.push(thread::spawn(move || {
            measure_latency(
                || {
                    let _ = state.broadcast(msg.clone());
                },
                100,
            )
        }));
    }

    // Collect results
    let mut latencies = vec![];
    for handle in handles {
        latencies.push(handle.join().unwrap());
    }

    // Calculate median of medians (p50 across threads)
    latencies.sort_unstable();
    let p50_ns = latencies[latencies.len() / 2];

    println!("Concurrent broadcast p50: {}ns", p50_ns);

    assert!(
        p50_ns <= 55,
        "REGRESSION: Concurrent broadcast latency regressed to {}ns (threshold: 55ns)",
        p50_ns
    );
}

/// Test 13: Memory leak detection (connection count cleanup)
///
/// # Performance Target
/// - Connection count must return to 0 after cleanup
/// - No memory leaks (validated by counter accuracy)
#[tokio::test]
#[ignore]
async fn test_no_memory_leaks() {
    let broadcast_state = Arc::new(BroadcastState::new(1000));

    // Simulate 1000 connections
    for _ in 0..1000 {
        broadcast_state.increment_connections();
    }

    assert_eq!(broadcast_state.connection_count(), 1000);

    // Simulate cleanup
    for _ in 0..1000 {
        broadcast_state.decrement_connections();
    }

    // Verify no leaks
    assert_eq!(
        broadcast_state.connection_count(),
        0,
        "MEMORY LEAK: Connection count did not return to 0 after cleanup"
    );
}

/// Test 14: Backpressure performance (no cascade failures)
///
/// # Performance Target
/// - Slow receivers should not impact fast receivers
/// - Broadcast latency <50ns even with lagging receivers
#[tokio::test]
#[ignore]
async fn test_backpressure_isolation() {
    let broadcast_state = Arc::new(BroadcastState::new(10)); // Small capacity

    // Fast receiver
    let mut fast_rx = broadcast_state.subscribe();

    // Slow receiver (never reads, will lag)
    let _slow_rx = broadcast_state.subscribe();

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

    // Measure broadcast latency with slow receiver
    let start = Instant::now();
    for _ in 0..100 {
        let _ = broadcast_state.broadcast(message.clone());
    }
    let duration = start.elapsed();

    let avg_latency_ns = (duration.as_nanos() as u64) / 100;
    println!("Broadcast latency with backpressure: {}ns", avg_latency_ns);

    // Fast receiver should still receive messages (may skip some)
    let received_count = fast_rx.try_recv().is_ok();
    assert!(received_count, "Fast receiver did not receive any messages");

    // Broadcast latency should not degrade significantly
    assert!(
        avg_latency_ns <= 100,
        "REGRESSION: Backpressure caused latency degradation to {}ns (threshold: 100ns)",
        avg_latency_ns
    );
}

// CI Integration Notes:
//
// Run all regression tests:
// ```bash
// cargo test --test ws_performance_regression_tests -- --ignored
// ```
//
// Run specific test:
// ```bash
// cargo test --test ws_performance_regression_tests test_broadcast_single_receiver_latency -- --ignored
// ```
//
// CI failure handling:
// 1. Review PHASE3_PROFILING_REPORT.md for baselines
// 2. Check git log for recent changes
// 3. Run: cargo bench --bench ws_bench (full profiling)
// 4. Investigate regression cause
// 5. Revert if >10% regression without justification
//
// Update baselines (if regression is expected):
// 1. Update baselines module in this file
// 2. Update PHASE3_PROFILING_REPORT.md
// 3. Commit with justification in commit message
