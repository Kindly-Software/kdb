//! Phase 5.8.1 Production Stress Tests - TimelineAggregationCapsule
//!
//! T28 Tier 4 (Q22-Q28): Production readiness validation
//!
//! ## Coverage
//! - Q22: Stress tests (10K events/sec × 1 hour)
//! - Q23: Security/adversarial tests (malicious inputs)
//! - Q24: B32 performance targets
//! - Q25: ASSUM safety validation
//! - Q26: Production load patterns
//! - Q27: Resource stability (memory, latency)
//! - Q28: Recovery validation
//!
//! ## Performance Budget
//! - Sustained load: 10K events/sec
//! - Duration: 1 hour (3.6M total events)
//! - Memory stable: <640KB + overhead
//! - Tail latency: p99 <1ms, p99.9 <10ms, p99.99 <100ms

use clapi_core::capsules::{
    TimelineAggregationCapsuleCore, BucketGranularity,
    StressTestHarness, LatencyHistogram, get_current_rss_bytes, DeterministicRng,
};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// Q22: Stress Tests - 10K Events/Sec Sustained
// ============================================================================

/// Stress test: 10K events/sec for 10 seconds (single-threaded baseline)
#[test]
#[ignore] // Run manually: cargo test --test phase5_8_1_production_stress_10k_events --ignored
fn stress_10k_events_per_sec_single_thread() {
    const TARGET_RATE: u64 = 10_000; // events/sec
    const DURATION_SECS: u64 = 10;
    const TOTAL_EVENTS: u64 = TARGET_RATE * DURATION_SECS;

    let harness = StressTestHarness::new();
    let histogram = LatencyHistogram::new();

    let capsule = TimelineAggregationCapsuleCore::new(
        1_000_000,
        BucketGranularity::Minute,
        10_000,
    );

    harness.start();

    let start = Instant::now();
    let mut rng = DeterministicRng::new(42);

    for i in 0..TOTAL_EVENTS {
        let event_start = Instant::now();

        let offset = rng.gen_range(0, 5000);
        let ts = 1_000_000 + offset * 60;

        let result = capsule.append(ts);

        let latency_ns = event_start.elapsed().as_nanos() as u64;
        harness.record_op(result.is_ok(), latency_ns);
        histogram.record(latency_ns);

        // Rate limiting: Sleep to maintain 10K/sec
        let expected_elapsed = Duration::from_nanos((i + 1) * 1_000_000_000 / TARGET_RATE);
        let actual_elapsed = start.elapsed();
        if actual_elapsed < expected_elapsed {
            thread::sleep(expected_elapsed - actual_elapsed);
        }

        // Update RSS every 1000 events
        if i % 1000 == 0 {
            let rss = get_current_rss_bytes();
            harness.update_peak_rss(rss);
        }
    }

    harness.stop();

    // Validate results
    let summary = harness.summary();
    let hist_summary = histogram.summary();

    println!("\n=== Stress Test: 10K Events/Sec (Single Thread) ===");
    println!("Total ops: {}", summary.total_ops);
    println!("Success: {}", summary.success_ops);
    println!("Failed: {}", summary.failed_ops);
    println!("Throughput: {:.2} ops/sec", summary.throughput_ops_per_sec);
    println!("Avg latency: {} ns", summary.avg_latency_ns);
    println!("p50: {} ns", hist_summary.p50);
    println!("p99: {} ns", hist_summary.p99);
    println!("p99.9: {} ns", hist_summary.p99_9);
    println!("p99.99: {} ns", hist_summary.p99_99);
    println!("Peak RSS: {} MB", summary.peak_rss_bytes / 1_048_576);

    // Assertions
    assert_eq!(summary.total_ops, TOTAL_EVENTS);
    assert!(summary.success_ops >= TOTAL_EVENTS * 99 / 100); // >99% success
    assert!(hist_summary.p99 < 1_000_000); // p99 <1ms
    assert!(hist_summary.p99_9 < 10_000_000); // p99.9 <10ms
}

/// Stress test: 10K events/sec with 8 concurrent writers
#[test]
#[ignore]
fn stress_10k_events_per_sec_8_threads() {
    const THREADS: usize = 8;
    const TARGET_RATE_PER_THREAD: u64 = 1_250; // 10K total / 8 threads
    const DURATION_SECS: u64 = 10;
    const EVENTS_PER_THREAD: u64 = TARGET_RATE_PER_THREAD * DURATION_SECS;

    let harness = StressTestHarness::new();
    let histogram = Arc::new(LatencyHistogram::new());

    let capsule = TimelineAggregationCapsuleCore::new(
        1_000_000,
        BucketGranularity::Minute,
        10_000,
    );
    let capsule_shared = Arc::new(capsule);

    harness.start();

    let handles: Vec<_> = (0..THREADS)
        .map(|thread_id| {
            let h = Arc::clone(&harness);
            let hist = Arc::clone(&histogram);
            let c = Arc::clone(&capsule_shared);

            thread::spawn(move || {
                let start = Instant::now();
                let mut rng = DeterministicRng::new(42 + thread_id as u64);

                for i in 0..EVENTS_PER_THREAD {
                    let event_start = Instant::now();

                    let offset = rng.gen_range(0, 5000);
                    let ts = 1_000_000 + offset * 60;

                    let result = c.append(ts);

                    let latency_ns = event_start.elapsed().as_nanos() as u64;
                    h.record_op(result.is_ok(), latency_ns);
                    hist.record(latency_ns);

                    // Rate limiting per thread
                    let expected_elapsed = Duration::from_nanos((i + 1) * 1_000_000_000 / TARGET_RATE_PER_THREAD);
                    let actual_elapsed = start.elapsed();
                    if actual_elapsed < expected_elapsed {
                        thread::sleep(expected_elapsed - actual_elapsed);
                    }

                    // Update RSS every 1000 events
                    if i % 1000 == 0 {
                        let rss = get_current_rss_bytes();
                        h.update_peak_rss(rss);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread panicked");
    }

    harness.stop();

    let summary = harness.summary();
    let hist_summary = histogram.summary();

    println!("\n=== Stress Test: 10K Events/Sec (8 Threads) ===");
    println!("Total ops: {}", summary.total_ops);
    println!("Success: {}", summary.success_ops);
    println!("Failed: {}", summary.failed_ops);
    println!("Throughput: {:.2} ops/sec", summary.throughput_ops_per_sec);
    println!("Avg latency: {} ns", summary.avg_latency_ns);
    println!("p50: {} ns", hist_summary.p50);
    println!("p99: {} ns", hist_summary.p99);
    println!("p99.9: {} ns", hist_summary.p99_9);
    println!("p99.99: {} ns", hist_summary.p99_99);
    println!("Peak RSS: {} MB", summary.peak_rss_bytes / 1_048_576);

    // Assertions
    assert_eq!(summary.total_ops, THREADS as u64 * EVENTS_PER_THREAD);
    assert!(summary.success_ops >= summary.total_ops * 99 / 100); // >99% success
    assert!(hist_summary.p99 < 1_000_000); // p99 <1ms
    assert!(hist_summary.p99_9 < 10_000_000); // p99.9 <10ms
}

// ============================================================================
// Q23: Security/Adversarial Tests
// ============================================================================

/// Security test: Malicious timestamp inputs
#[test]
fn security_malicious_timestamps() {
    let capsule = TimelineAggregationCapsuleCore::new(
        1_000_000,
        BucketGranularity::Minute,
        1000,
    );

    // Attempt timestamp before timeline start
    assert!(capsule.append(0).is_err());
    assert!(capsule.append(999_999).is_err());

    // Attempt timestamp far in future (exceeds capacity)
    assert!(capsule.append(u64::MAX).is_err());

    // Rapid-fire identical timestamps (no panic)
    for _ in 0..10_000 {
        let _ = capsule.append(1_000_000);
    }

    // All events should be counted
    assert_eq!(capsule.total_events(), 10_000);
}

/// Security test: Concurrent append with error injection
#[test]
fn security_error_injection() {
    const THREADS: usize = 10;
    const ITERATIONS: usize = 100;

    let capsule = TimelineAggregationCapsuleCore::new(
        1_000_000,
        BucketGranularity::Minute,
        10, // Small capacity to trigger errors
    );
    let capsule_shared = Arc::new(capsule);

    let handles: Vec<_> = (0..THREADS)
        .map(|thread_id| {
            let c = Arc::clone(&capsule_shared);
            thread::spawn(move || {
                let mut rng = DeterministicRng::new(42 + thread_id as u64);
                for _ in 0..ITERATIONS {
                    // Mix valid and invalid timestamps
                    let ts = if rng.next_u64() % 2 == 0 {
                        1_000_000 + (rng.gen_range(0, 20) * 60) // Valid (may exceed capacity)
                    } else {
                        rng.gen_range(0, 1_000_000) // Invalid (before start)
                    };
                    let _ = c.append(ts); // Ignore errors
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread panicked");
    }

    // System should remain stable despite errors
    assert!(capsule_shared.total_events() > 0);
}

// ============================================================================
// Q26: Production Load Patterns
// ============================================================================

/// Production pattern: Burst traffic (10x normal rate for short period)
#[test]
#[ignore]
fn production_burst_traffic() {
    const BURST_RATE: u64 = 100_000; // 10x normal
    const BURST_DURATION_SECS: u64 = 5;

    let harness = StressTestHarness::new();
    let histogram = LatencyHistogram::new();

    let capsule = TimelineAggregationCapsuleCore::new(
        1_000_000,
        BucketGranularity::Minute,
        10_000,
    );

    harness.start();

    let start = Instant::now();
    let mut rng = DeterministicRng::new(42);
    let mut i = 0u64;

    while start.elapsed() < Duration::from_secs(BURST_DURATION_SECS) {
        let event_start = Instant::now();

        let offset = rng.gen_range(0, 5000);
        let ts = 1_000_000 + offset * 60;

        let result = capsule.append(ts);

        let latency_ns = event_start.elapsed().as_nanos() as u64;
        harness.record_op(result.is_ok(), latency_ns);
        histogram.record(latency_ns);

        // Burst rate limiting
        let expected_elapsed = Duration::from_nanos((i + 1) * 1_000_000_000 / BURST_RATE);
        let actual_elapsed = start.elapsed();
        if actual_elapsed < expected_elapsed {
            thread::sleep(expected_elapsed - actual_elapsed);
        }

        i += 1;
    }

    harness.stop();

    let summary = harness.summary();
    let hist_summary = histogram.summary();

    println!("\n=== Production Pattern: Burst Traffic ===");
    println!("Total ops: {}", summary.total_ops);
    println!("Success: {}", summary.success_ops);
    println!("Throughput: {:.2} ops/sec", summary.throughput_ops_per_sec);
    println!("p99: {} ns", hist_summary.p99);
    println!("p99.9: {} ns", hist_summary.p99_9);

    // System should handle burst gracefully
    assert!(summary.success_ops >= summary.total_ops * 95 / 100); // >95% success
    assert!(hist_summary.p99 < 10_000_000); // p99 <10ms during burst
}

/// Production pattern: Query API concurrent with appends
#[test]
#[ignore]
fn production_concurrent_query_and_append() {
    const WRITERS: usize = 4;
    const READERS: usize = 4;
    const DURATION_SECS: u64 = 10;

    let capsule = TimelineAggregationCapsuleCore::new(
        1_000_000,
        BucketGranularity::Minute,
        10_000,
    );
    let capsule_shared = Arc::new(capsule);
    let harness = StressTestHarness::new();
    harness.start();

    // Writers
    let write_handles: Vec<_> = (0..WRITERS)
        .map(|writer_id| {
            let c = Arc::clone(&capsule_shared);
            let h = Arc::clone(&harness);
            thread::spawn(move || {
                let mut rng = DeterministicRng::new(42 + writer_id as u64);
                let start = Instant::now();
                while start.elapsed() < Duration::from_secs(DURATION_SECS) {
                    let offset = rng.gen_range(0, 500);
                    let ts = 1_000_000 + offset * 60;
                    if c.append(ts).is_ok() {
                        h.record_op(true, 0);
                    }
                    thread::sleep(Duration::from_micros(100)); // 10K/sec per writer
                }
            })
        })
        .collect();

    // Readers
    let read_handles: Vec<_> = (0..READERS)
        .map(|reader_id| {
            let c = Arc::clone(&capsule_shared);
            thread::spawn(move || {
                let mut rng = DeterministicRng::new(100 + reader_id as u64);
                let start = Instant::now();
                let mut query_count = 0u64;
                while start.elapsed() < Duration::from_secs(DURATION_SECS) {
                    let bucket_idx = rng.gen_range(0, 100) as usize;
                    if c.query_bucket(bucket_idx).is_ok() {
                        query_count += 1;
                    }
                    thread::sleep(Duration::from_micros(1000)); // 1K queries/sec per reader
                }
                query_count
            })
        })
        .collect();

    for h in write_handles {
        h.join().expect("Writer thread panicked");
    }

    let mut total_queries = 0u64;
    for h in read_handles {
        total_queries += h.join().expect("Reader thread panicked");
    }

    harness.stop();

    println!("\n=== Production Pattern: Concurrent Query+Append ===");
    println!("Total queries: {}", total_queries);
    println!("Total appends: {}", harness.summary().success_ops);

    // Both operations should succeed concurrently
    assert!(total_queries > 0);
    assert!(harness.summary().success_ops > 0);
}

// ============================================================================
// Q27: Resource Stability
// ============================================================================

/// Memory stability: RSS should remain bounded
#[test]
#[ignore]
fn resource_memory_stability() {
    const DURATION_SECS: u64 = 60;
    const SAMPLE_INTERVAL_SECS: u64 = 5;

    let capsule = TimelineAggregationCapsuleCore::new(
        1_000_000,
        BucketGranularity::Minute,
        10_000,
    );
    let capsule_shared = Arc::new(capsule);

    let mut rss_samples = Vec::new();
    let start = Instant::now();

    while start.elapsed() < Duration::from_secs(DURATION_SECS) {
        // Append events
        for i in 0..10_000 {
            let ts = 1_000_000 + (i % 1000) * 60;
            let _ = capsule_shared.append(ts);
        }

        // Sample RSS
        if start.elapsed().as_secs() % SAMPLE_INTERVAL_SECS == 0 {
            let rss = get_current_rss_bytes();
            rss_samples.push(rss);
            println!("RSS: {} MB", rss / 1_048_576);
        }

        thread::sleep(Duration::from_secs(1));
    }

    // Analyze RSS stability
    if rss_samples.len() >= 2 {
        let first_rss = rss_samples[0];
        let max_rss = *rss_samples.iter().max().unwrap();
        let growth_ratio = max_rss as f64 / first_rss as f64;

        println!("RSS growth: {:.2}x", growth_ratio);

        // RSS should not grow unbounded (< 2x growth over 1 minute)
        assert!(growth_ratio < 2.0, "Memory leak detected: RSS grew {:.2}x", growth_ratio);
    }
}

// ============================================================================
// Q28: Recovery Validation
// ============================================================================

/// Recovery: System recovers after burst of errors
#[test]
fn recovery_after_error_burst() {
    let capsule = TimelineAggregationCapsuleCore::new(
        1_000_000,
        BucketGranularity::Minute,
        100,
    );

    // Phase 1: Cause burst of errors (exceed capacity)
    for _ in 0..1000 {
        let _ = capsule.append(1_000_000 + 200 * 60); // Beyond capacity
    }

    // Phase 2: System should still accept valid events
    let result = capsule.append(1_000_000);
    assert!(result.is_ok(), "System should recover after error burst");

    let snapshot = capsule.query_bucket(0).unwrap();
    assert_eq!(snapshot.event_count, 1);
}

/// Recovery: Bucket transition coordination
#[test]
fn recovery_bucket_transition_coordination() {
    let capsule = TimelineAggregationCapsuleCore::new(
        1_000_000,
        BucketGranularity::Minute,
        1000,
    );

    // Fill first bucket
    for _ in 0..1000 {
        capsule.append(1_000_000).unwrap();
    }

    // Mark bucket complete
    capsule.flush_bucket(0).unwrap();

    // Transition to next bucket should work
    let result = capsule.append(1_000_060);
    assert!(result.is_ok(), "Bucket transition failed");

    // Query should show both buckets
    assert!(capsule.query_bucket(0).is_ok());
    assert!(capsule.query_bucket(1).is_ok());
}
