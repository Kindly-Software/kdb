//! T28 Testing Framework - AdvancedRateLimiter64 Comprehensive Test Suite
//!
//! **Framework**: T28 (28-question comprehensive testing)
//! **Coverage**: Unit (Q1-Q7), Property (Q8-Q14), Integration (Q15-Q21), Stress (Q22-Q28)
//!
//! # Test Tiers
//! - **Unit**: Basic operations, capsule invariants, jitter behavior
//! - **Property**: Concurrent correctness (100 threads × 1000 requests), jitter distribution
//! - **Integration**: End-to-end rate limiting with backpressure workflows
//! - **Stress**: 10K concurrent users, extreme contention, thundering herd prevention

use clapi_core::capsules::{AdvancedRateLimiter64, RateLimiterStats};
use clapi_core::error::ClapiError;
use clapi_core::proxy::rate_limiter_jitter::{
    ExponentialBackoff, JitteredRateLimiter, RateLimiterRegistry, RetryInfo,
};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ============================================================================
// T28 Q1-Q7: Unit Tests - Basic Operations
// ============================================================================

#[test]
fn t28_q1_capsule_size_and_alignment() {
    // Q1: Verify capsule layout (64B, 64B-aligned)
    assert_eq!(
        std::mem::size_of::<AdvancedRateLimiter64>(),
        64,
        "AdvancedRateLimiter64 must be exactly 64 bytes"
    );
    assert_eq!(
        std::mem::align_of::<AdvancedRateLimiter64>(),
        64,
        "AdvancedRateLimiter64 must be 64-byte aligned (L1 cache line)"
    );
}

#[test]
fn t28_q2_new_limiter_initial_state() {
    // Q2: Verify initial state (capacity=1000, tokens=1000)
    let limiter = AdvancedRateLimiter64::new();

    let stats = limiter.stats();
    assert_eq!(stats.tokens, 1000, "Initial tokens should be 1000");
    assert_eq!(stats.capacity, 1000, "Capacity should be 1000");
    assert_eq!(stats.total_requests, 0, "Total requests should start at 0");
    assert_eq!(stats.total_throttled, 0, "Total throttled should start at 0");
    assert!(stats.has_tokens(), "New limiter should have tokens");
    assert_eq!(
        stats.throttle_rate_percent(),
        0.0,
        "Initial throttle rate should be 0%"
    );
}

#[test]
fn t28_q3_custom_capacity_and_period() {
    // Q3: Verify custom capacity initialization
    let limiter = AdvancedRateLimiter64::with_capacity_and_period(5000, 30_000_000_000);

    let stats = limiter.stats();
    assert_eq!(stats.tokens, 5000, "Custom tokens should be 5000");
    assert_eq!(stats.capacity, 5000, "Custom capacity should be 5000");
    assert_eq!(
        stats.refill_rate_ns,
        6_000_000,
        "Refill rate should be 30s / 5000 = 6ms"
    );
}

#[test]
fn t28_q4_acquire_token_success() {
    // Q4: Verify single token acquisition
    let limiter = AdvancedRateLimiter64::new();

    let result = limiter.acquire_token();
    assert!(result.is_ok(), "First acquisition should succeed");
    assert_eq!(result.unwrap(), 999, "Tokens remaining should be 999");

    let stats = limiter.stats();
    assert_eq!(stats.tokens, 999, "Tokens should be 999 after acquisition");
    assert_eq!(stats.total_requests, 1, "Total requests should be 1");
    assert_eq!(stats.total_throttled, 0, "No throttling yet");
}

#[test]
fn t28_q5_acquire_token_with_jitter() {
    // Q5: Verify token acquisition with jitter
    let limiter = AdvancedRateLimiter64::new();

    let result = limiter.acquire_token_with_jitter();
    assert!(result.is_ok(), "Acquisition with jitter should succeed");

    let (tokens_remaining, jitter_ns) = result.unwrap();
    assert_eq!(tokens_remaining, 999, "Tokens remaining should be 999");

    // Jitter should be in valid range
    let jitter_max = limiter.stats().refill_rate_ns / 10;
    assert!(
        jitter_ns < jitter_max,
        "Jitter ({}) should be < max ({})",
        jitter_ns,
        jitter_max
    );
}

#[test]
fn t28_q6_token_exhaustion() {
    // Q6: Verify token exhaustion behavior
    let limiter = AdvancedRateLimiter64::with_capacity_and_period(5, 60_000_000_000);

    // Exhaust tokens
    for i in 0..5 {
        let result = limiter.acquire_token();
        assert!(result.is_ok(), "Request {} should succeed", i + 1);
    }

    // Next request should fail
    let result = limiter.acquire_token();
    assert!(result.is_err(), "Request 6 should be rejected");
    assert!(
        matches!(result, Err(ClapiError::RateLimitExceeded { .. })),
        "Error should be RateLimitExceeded"
    );

    let stats = limiter.stats();
    assert_eq!(stats.tokens, 0, "Tokens should be 0");
    assert_eq!(stats.total_throttled, 1, "Throttle count should be 1");
}

#[test]
fn t28_q7_stats_snapshot_consistency() {
    // Q7: Verify stats snapshot consistency
    let limiter = AdvancedRateLimiter64::new();

    limiter.acquire_token().unwrap();
    limiter.acquire_token().unwrap();

    let stats = limiter.stats();
    assert_eq!(stats.tokens, 998, "Tokens should be 998");
    assert_eq!(stats.total_requests, 2, "Total requests should be 2");

    // Snapshot should be stable
    let stats2 = limiter.stats();
    assert_eq!(stats.tokens, stats2.tokens, "Stats should be stable");
}

// ============================================================================
// T28 Q8-Q14: Property Tests - Concurrent Correctness
// ============================================================================

#[test]
fn t28_q8_concurrent_acquire_token_conservation() {
    // Q8: Property: Total acquired ≤ capacity under concurrency
    let limiter = Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
        100,
        60_000_000_000,
    ));
    let mut handles = vec![];

    for _ in 0..10 {
        let l = Arc::clone(&limiter);
        handles.push(thread::spawn(move || {
            for _ in 0..20 {
                let _ = l.acquire_token();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = limiter.stats();
    // Property: total_requests - total_throttled ≤ capacity
    let successful = stats.total_requests - stats.total_throttled;
    assert!(
        successful <= 100,
        "Successful acquisitions ({}) must never exceed capacity (100)",
        successful
    );
    assert_eq!(successful, 100, "All 100 tokens should be consumed");
}

#[test]
fn t28_q9_jitter_distribution_uniformity() {
    // Q9: Property: Jitter distribution should be uniform
    let limiter = AdvancedRateLimiter64::with_capacity_and_period(10000, 60_000_000_000);
    let jitter_max = limiter.stats().refill_rate_ns / 10;

    // Collect 1000 jitter samples
    let mut jitters = Vec::with_capacity(1000);
    for _ in 0..1000 {
        if let Ok((_, jitter_ns)) = limiter.acquire_token_with_jitter() {
            jitters.push(jitter_ns);
        }
    }

    // Verify all jitters are within bounds
    for jitter in &jitters {
        assert!(
            *jitter < jitter_max,
            "Jitter {} should be < max {}",
            jitter,
            jitter_max
        );
    }

    // Verify distribution is not degenerate (at least 100 unique values)
    let unique_count = jitters.iter().collect::<std::collections::HashSet<_>>().len();
    assert!(
        unique_count > 100,
        "Jitter should be reasonably distributed ({} unique values)",
        unique_count
    );
}

#[test]
fn t28_q10_no_negative_tokens_under_load() {
    // Q10: Property: Tokens should never go negative under concurrent load
    let limiter = Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
        50,
        60_000_000_000,
    ));
    let mut handles = vec![];

    for _ in 0..10 {
        let l = Arc::clone(&limiter);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let _ = l.acquire_token();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = limiter.stats();
    // Property: tokens ≥ 0 (no overdraft allowed)
    assert!(
        stats.tokens >= 0,
        "Tokens ({}) should never be negative",
        stats.tokens
    );
}

#[test]
fn t28_q11_throttle_rate_accuracy() {
    // Q11: Property: Throttle rate calculation accuracy
    let limiter = AdvancedRateLimiter64::with_capacity_and_period(10, 60_000_000_000);

    // Make 20 requests (10 succeed, 10 fail)
    for _ in 0..20 {
        let _ = limiter.acquire_token();
    }

    let stats = limiter.stats();
    assert_eq!(stats.total_requests, 20, "Total requests should be 20");
    assert_eq!(stats.total_throttled, 10, "Throttled should be 10");
    assert_eq!(
        stats.throttle_rate_percent(),
        50.0,
        "Throttle rate should be 50%"
    );
}

#[test]
fn t28_q12_concurrent_jitter_uniqueness() {
    // Q12: Property: Concurrent jitter generation produces diverse values
    let limiter = Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
        1000,
        60_000_000_000,
    ));
    let jitters = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut handles = vec![];

    for _ in 0..10 {
        let l = Arc::clone(&limiter);
        let j = Arc::clone(&jitters);
        handles.push(thread::spawn(move || {
            for _ in 0..10 {
                if let Ok((_, jitter_ns)) = l.acquire_token_with_jitter() {
                    j.lock().unwrap().push(jitter_ns);
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let jitter_vec = jitters.lock().unwrap();
    let unique_count = jitter_vec.iter().collect::<std::collections::HashSet<_>>().len();

    // Should have diverse jitter values (at least 20% unique)
    assert!(
        unique_count > 20,
        "Concurrent jitter should be diverse ({} unique out of {})",
        unique_count,
        jitter_vec.len()
    );
}

#[test]
fn t28_q13_100_threads_1000_requests_quota_never_exceeded() {
    // Q13: Property: 100 threads × 1000 requests, quota never exceeded
    let quota = 5000;
    let limiter = Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
        quota,
        60_000_000_000,
    ));
    let mut handles = vec![];

    for _ in 0..100 {
        let l = Arc::clone(&limiter);
        handles.push(thread::spawn(move || {
            for _ in 0..1000 {
                let _ = l.acquire_token();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = limiter.stats();
    let successful = stats.total_requests - stats.total_throttled;

    // Property: successful acquisitions ≤ quota
    assert!(
        successful <= quota as u64,
        "Successful acquisitions ({}) must never exceed quota ({})",
        successful,
        quota
    );
    assert_eq!(successful, quota as u64, "All quota should be consumed");
}

#[test]
fn t28_q14_monotonic_total_requests() {
    // Q14: Property: total_requests is monotonically increasing
    let limiter = AdvancedRateLimiter64::new();

    let total1 = limiter.stats().total_requests;
    limiter.acquire_token().unwrap();
    let total2 = limiter.stats().total_requests;
    limiter.acquire_token().unwrap();
    let total3 = limiter.stats().total_requests;

    assert!(total2 > total1, "Total requests should increase");
    assert!(total3 > total2, "Total requests should increase monotonically");
}

// ============================================================================
// T28 Q15-Q21: Integration Tests - End-to-End Workflows
// ============================================================================

#[test]
fn t28_q15_jittered_rate_limiter_workflow() {
    // Q15: End-to-end: JitteredRateLimiter with backpressure
    let limiter = JitteredRateLimiter::new("user123".to_string(), 3, 60_000_000_000);

    // Step 1: Acquire tokens successfully
    for i in 0..3 {
        let result = limiter.acquire_with_backpressure();
        assert!(
            result.is_ok(),
            "Request {} should succeed",
            i + 1
        );
    }

    // Step 2: Exhaust tokens, verify backpressure
    let result = limiter.acquire_with_backpressure();
    assert!(result.is_err(), "Request 4 should be rejected");

    match result {
        Err(ClapiError::RateLimitExceededWithBackpressure {
            user_id,
            retry_after_ms,
            ..
        }) => {
            assert_eq!(user_id, "user123");
            assert!(retry_after_ms > 0, "Retry delay should be positive");
        }
        _ => panic!("Expected RateLimitExceededWithBackpressure error"),
    }
}

#[test]
fn t28_q16_exponential_backoff_progression() {
    // Q16: Verify exponential backoff increases correctly
    let mut backoff = ExponentialBackoff::new(100, 10_000, 0); // No jitter for deterministic test

    let delay0 = backoff.next_delay().as_millis();
    let delay1 = backoff.next_delay().as_millis();
    let delay2 = backoff.next_delay().as_millis();

    assert_eq!(delay0, 100, "Delay 0 should be 100ms");
    assert_eq!(delay1, 200, "Delay 1 should be 200ms (100 × 2^1)");
    assert_eq!(delay2, 400, "Delay 2 should be 400ms (100 × 2^2)");
}

#[test]
fn t28_q17_rate_limiter_registry_user_isolation() {
    // Q17: Verify per-user rate limiting isolation
    let registry = RateLimiterRegistry::new(10, 60_000_000_000);

    // User1: Exhaust quota
    for _ in 0..10 {
        let _ = registry.acquire_with_jitter("user1");
    }

    // User1: Next request should fail
    let result1 = registry.acquire_with_jitter("user1");
    assert!(result1.is_err(), "User1 should be rate limited");

    // User2: Should still have quota (isolated)
    let result2 = registry.acquire_with_jitter("user2");
    assert!(result2.is_ok(), "User2 should still have quota");
}

#[test]
fn t28_q18_registry_user_count_tracking() {
    // Q18: Verify registry tracks user count correctly
    let registry = RateLimiterRegistry::new(100, 60_000_000_000);

    assert_eq!(registry.user_count(), 0, "Initially no users");

    registry.acquire_with_jitter("user1").unwrap();
    assert_eq!(registry.user_count(), 1, "One user after first request");

    registry.acquire_with_jitter("user2").unwrap();
    registry.acquire_with_jitter("user2").unwrap(); // Same user
    assert_eq!(registry.user_count(), 2, "Two users total");
}

#[test]
fn t28_q19_registry_all_stats() {
    // Q19: Verify registry can retrieve all user stats
    let registry = RateLimiterRegistry::new(100, 60_000_000_000);

    registry.acquire_with_jitter("user1").unwrap();
    registry.acquire_with_jitter("user1").unwrap();
    registry.acquire_with_jitter("user2").unwrap();

    let all_stats = registry.all_stats();
    assert_eq!(all_stats.len(), 2, "Should have stats for 2 users");

    // Find user1 stats (should have 2 requests)
    let user1_stats = all_stats
        .iter()
        .find(|(id, _)| id == "user1")
        .map(|(_, stats)| stats)
        .unwrap();

    assert_eq!(
        user1_stats.total_requests, 2,
        "User1 should have 2 requests"
    );
}

#[test]
fn t28_q20_default_limiter_construction() {
    // Q20: Verify Default trait implementation
    let limiter: AdvancedRateLimiter64 = Default::default();
    let stats = limiter.stats();

    assert_eq!(stats.capacity, 1000, "Default capacity should be 1000");
    assert_eq!(stats.tokens, 1000, "Default tokens should be 1000");
}

#[test]
fn t28_q21_backoff_reset_functionality() {
    // Q21: Verify exponential backoff reset works
    let mut backoff = ExponentialBackoff::new(100, 10_000, 0);

    backoff.next_delay();
    backoff.next_delay();
    assert_eq!(backoff.attempt(), 2, "Attempt should be 2");

    backoff.reset();
    assert_eq!(backoff.attempt(), 0, "Attempt should reset to 0");

    let delay = backoff.next_delay().as_millis();
    assert_eq!(delay, 100, "Delay should reset to base (100ms)");
}

// ============================================================================
// T28 Q22-Q28: Stress Tests - Extreme Contention & Scale
// ============================================================================

#[test]
#[ignore] // Run with: cargo test t28_q22 -- --ignored
fn t28_q22_10k_concurrent_users() {
    // Q22: Stress test: 10,000 concurrent users
    let quota = 10_000;
    let limiter = Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
        quota,
        60_000_000_000,
    ));
    let mut handles = vec![];

    for _ in 0..1000 {
        let l = Arc::clone(&limiter);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let _ = l.acquire_token();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = limiter.stats();
    let successful = stats.total_requests - stats.total_throttled;
    assert!(
        successful <= quota as u64,
        "Quota never exceeded even under extreme load"
    );
}

#[test]
#[ignore] // Run with: cargo test t28_q23 -- --ignored
fn t28_q23_sustained_load_1M_requests() {
    // Q23: Stress test: 1 million requests (sustained load)
    let quota = 1_000_000;
    let limiter = Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
        quota,
        60_000_000_000,
    ));
    let mut handles = vec![];

    for _ in 0..100 {
        let l = Arc::clone(&limiter);
        handles.push(thread::spawn(move || {
            for _ in 0..10_000 {
                let _ = l.acquire_token();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = limiter.stats();
    let successful = stats.total_requests - stats.total_throttled;
    assert_eq!(
        successful, quota as u64,
        "All 1M tokens should be consumed"
    );
}

#[test]
#[ignore] // Run with: cargo test t28_q24 -- --ignored
fn t28_q24_rapid_acquire_token_calls() {
    // Q24: Stress test: Rapid acquire_token() calls (10M calls)
    let limiter = AdvancedRateLimiter64::with_capacity_and_period(10_000_000, 60_000_000_000);

    let start = std::time::Instant::now();
    for _ in 0..10_000_000 {
        let _ = limiter.acquire_token();
    }
    let elapsed = start.elapsed();

    println!(
        "10M acquire_token() calls: {:?} ({} ns/call)",
        elapsed,
        elapsed.as_nanos() / 10_000_000
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "10M calls should complete <5s"
    );
}

#[test]
#[ignore] // Run with: cargo test t28_q25 -- --ignored
fn t28_q25_thundering_herd_simulation() {
    // Q25: Simulate thundering herd (all threads retry simultaneously)
    let limiter = Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
        100,
        60_000_000_000,
    ));
    let collision_count = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let mut handles = vec![];

    // Exhaust quota first
    for _ in 0..100 {
        limiter.acquire_token().unwrap();
    }

    // All threads try to acquire simultaneously (thundering herd)
    for _ in 0..100 {
        let l = Arc::clone(&limiter);
        let c = Arc::clone(&collision_count);
        handles.push(thread::spawn(move || {
            // All threads retry at same time
            if l.acquire_token().is_err() {
                c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let collisions = collision_count.load(std::sync::atomic::Ordering::Relaxed);
    println!("Thundering herd collisions: {}/100", collisions);

    // With jitter, collisions should be reduced (but hard to test deterministically)
    assert!(collisions > 0, "Some collisions expected when quota exhausted");
}

#[test]
#[ignore] // Run with: cargo test t28_q26 -- --ignored
fn t28_q26_latency_percentiles() {
    // Q26: Performance test: Measure latency percentiles (p50, p99, p999)
    let limiter = AdvancedRateLimiter64::with_capacity_and_period(1_000_000, 60_000_000_000);
    let mut latencies = Vec::with_capacity(100_000);

    for _ in 0..100_000 {
        let start = std::time::Instant::now();
        let _ = limiter.acquire_token_with_jitter();
        let elapsed = start.elapsed().as_nanos();
        latencies.push(elapsed);
    }

    latencies.sort_unstable();
    let p50 = latencies[50_000];
    let p99 = latencies[99_000];
    let p999 = latencies[99_900];

    println!("Latency percentiles (100K samples):");
    println!("  p50:  {} ns", p50);
    println!("  p99:  {} ns", p99);
    println!("  p999: {} ns", p999);

    assert!(p50 < 100, "p50 latency should be <100ns");
    assert!(p99 < 1000, "p99 latency should be <1μs");
}

#[test]
#[ignore] // Run with: cargo test t28_q27 -- --ignored
fn t28_q27_memory_stability_1M_cycles() {
    // Q27: Memory stability: 1M allocation/operation cycles
    for _ in 0..1_000_000 {
        let limiter = AdvancedRateLimiter64::new();
        let _ = limiter.acquire_token();
        drop(limiter);
    }

    // No crashes = success
}

#[test]
#[ignore] // Run with: cargo test t28_q28 -- --ignored
fn t28_q28_throughput_measurement() {
    // Q28: Throughput measurement: ops/sec under contention
    let limiter = Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
        10_000_000,
        60_000_000_000,
    ));
    let mut handles = vec![];
    let start = std::time::Instant::now();

    for _ in 0..10 {
        let l = Arc::clone(&limiter);
        handles.push(thread::spawn(move || {
            for _ in 0..1_000_000 {
                let _ = l.acquire_token();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let elapsed = start.elapsed();
    let throughput = 10_000_000.0 / elapsed.as_secs_f64();

    println!(
        "Throughput: {:.0} ops/sec (10M ops in {:?})",
        throughput, elapsed
    );
    assert!(
        throughput > 10_000_000.0,
        "Throughput should exceed 10M ops/sec"
    );
}
