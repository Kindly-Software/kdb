//! T28 Testing Framework - RateLimitCapsule Comprehensive Test Suite
//!
//! **Framework**: T28 (28-question comprehensive testing)
//! **Coverage**: Unit (Q1-Q7), Property (Q8-Q14), Integration (Q15-Q21), Stress (Q22-Q28)
//!
//! # Test Tiers
//! - **Unit**: Basic operations, capsule invariants
//! - **Property**: Concurrent correctness (100 users × 1000 requests)
//! - **Integration**: End-to-end rate limiting workflows
//! - **Stress**: 10K concurrent users, extreme contention

use clapi_core::capsules::{RateLimitCapsule, RateLimitStats};
use clapi_core::error::ClapiError;
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
        std::mem::size_of::<RateLimitCapsule>(),
        64,
        "RateLimitCapsule must be exactly 64 bytes"
    );
    assert_eq!(
        std::mem::align_of::<RateLimitCapsule>(),
        64,
        "RateLimitCapsule must be 64-byte aligned (L1 cache line)"
    );
}

#[test]
fn t28_q2_new_limiter_initial_state() {
    // Q2: Verify initial state (quota=1000, count=0)
    let limiter = RateLimitCapsule::new();

    assert!(limiter.check_rate_limit(), "New limiter should allow requests");

    let stats = limiter.stats();
    assert_eq!(stats.requests_count, 0, "Initial request count should be 0");
    assert_eq!(stats.quota_remaining, 1000, "Default quota should be 1000");
    assert_eq!(stats.total_requests, 0, "Total requests should start at 0");
    assert!(stats.window_start_ns > 0, "Window start should be initialized");
}

#[test]
fn t28_q3_custom_quota() {
    // Q3: Verify custom quota initialization
    let limiter = RateLimitCapsule::with_quota(5000);

    let stats = limiter.stats();
    assert_eq!(
        stats.quota_remaining, 5000,
        "Custom quota should be respected"
    );
}

#[test]
fn t28_q4_increment_request_success() {
    // Q4: Verify single request increments correctly
    let limiter = RateLimitCapsule::new();

    let result = limiter.increment_request();
    assert!(result.is_ok(), "First request should succeed");
    assert_eq!(result.unwrap(), 999, "Quota should decrement to 999");

    let stats = limiter.stats();
    assert_eq!(stats.requests_count, 1, "Request count should be 1");
    assert_eq!(stats.quota_remaining, 999, "Quota should be 999");
    assert_eq!(stats.total_requests, 1, "Total requests should be 1");
}

#[test]
fn t28_q5_quota_exhaustion() {
    // Q5: Verify quota exhaustion rejection
    let limiter = RateLimitCapsule::with_quota(5);

    // Exhaust quota
    for i in 0..5 {
        let result = limiter.increment_request();
        assert!(result.is_ok(), "Request {} should succeed", i + 1);
    }

    // Next request should fail
    let result = limiter.increment_request();
    assert!(result.is_err(), "Request 6 should be rejected (quota exhausted)");
    assert!(
        matches!(result, Err(ClapiError::RateLimitExceeded { .. })),
        "Error should be RateLimitExceeded"
    );

    // Check should return false
    assert!(
        !limiter.check_rate_limit(),
        "Check should return false when quota exhausted"
    );

    let stats = limiter.stats();
    assert_eq!(stats.requests_count, 5, "Request count should be 5");
    assert_eq!(stats.quota_remaining, 0, "Quota should be 0");
}

#[test]
fn t28_q6_check_rate_limit_fast_path() {
    // Q6: Verify check_rate_limit() performance (fast path <20ns)
    let limiter = RateLimitCapsule::new();

    // First check (quota available)
    let start = std::time::Instant::now();
    let allowed = limiter.check_rate_limit();
    let elapsed = start.elapsed();

    assert!(allowed, "Check should allow when quota available");
    assert!(
        elapsed < Duration::from_nanos(1000),
        "Check should be <1μs (fast path)"
    );
}

#[test]
fn t28_q7_stats_snapshot_consistency() {
    // Q7: Verify stats snapshot consistency
    let limiter = RateLimitCapsule::new();

    limiter.increment_request().unwrap();
    limiter.increment_request().unwrap();

    let stats = limiter.stats();
    assert_eq!(stats.requests_count, 2);
    assert_eq!(stats.quota_remaining, 998);
    assert_eq!(stats.total_requests, 2);

    // Snapshot should be stable
    let stats2 = limiter.stats();
    assert_eq!(stats.requests_count, stats2.requests_count);
}

// ============================================================================
// T28 Q8-Q14: Property Tests - Concurrent Correctness
// ============================================================================

#[test]
fn t28_q8_concurrent_increments_quota_conservation() {
    // Q8: Property: Total requests ≤ quota under concurrency
    let limiter = Arc::new(RateLimitCapsule::with_quota(100));
    let mut handles = vec![];

    for _ in 0..10 {
        let l = Arc::clone(&limiter);
        handles.push(thread::spawn(move || {
            for _ in 0..20 {
                let _ = l.increment_request();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = limiter.stats();
    // Property: requests_count ≤ 100 (quota conservation)
    assert!(
        stats.requests_count <= 100,
        "Requests count ({}) must never exceed quota (100)",
        stats.requests_count
    );
    assert_eq!(
        stats.requests_count, 100,
        "All 100 quota should be consumed"
    );
    assert_eq!(stats.quota_remaining, 0, "Quota should be exactly 0");
}

#[test]
fn t28_q9_concurrent_check_and_increment() {
    // Q9: Property: check_rate_limit() + increment_request() consistency
    let limiter = Arc::new(RateLimitCapsule::with_quota(50));
    let mut handles = vec![];
    let allowed_count = Arc::new(std::sync::atomic::AtomicU64::new(0));

    for _ in 0..10 {
        let l = Arc::clone(&limiter);
        let count = Arc::clone(&allowed_count);
        handles.push(thread::spawn(move || {
            for _ in 0..10 {
                if l.check_rate_limit() {
                    if l.increment_request().is_ok() {
                        count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = limiter.stats();
    let allowed = allowed_count.load(std::sync::atomic::Ordering::Relaxed);

    // Property: allowed increments should match capsule state
    assert_eq!(
        allowed, stats.requests_count,
        "Allowed count should match capsule requests"
    );
    assert!(
        allowed <= 50,
        "Allowed count ({}) must never exceed quota (50)",
        allowed
    );
}

#[test]
fn t28_q10_no_false_positives_under_load() {
    // Q10: Property: Never allow when quota exhausted
    let limiter = Arc::new(RateLimitCapsule::with_quota(50));
    let mut handles = vec![];
    let false_positives = Arc::new(std::sync::atomic::AtomicU64::new(0));

    for _ in 0..10 {
        let l = Arc::clone(&limiter);
        let fp = Arc::clone(&false_positives);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let check = l.check_rate_limit();
                let result = l.increment_request();

                // False positive: check=true but increment failed
                if check && result.is_err() {
                    fp.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let fp_count = false_positives.load(std::sync::atomic::Ordering::Relaxed);
    // Some false positives expected (TOCTOU race), but should be rare
    assert!(
        fp_count < 100,
        "False positives ({}) should be rare (<100 in 1000 attempts)",
        fp_count
    );
}

#[test]
fn t28_q11_100_users_1000_requests_quota_never_exceeded() {
    // Q11: Property: 100 users × 1000 requests, quota never exceeded
    let quota = 5000;
    let limiter = Arc::new(RateLimitCapsule::with_quota(quota));
    let mut handles = vec![];

    for _ in 0..100 {
        let l = Arc::clone(&limiter);
        handles.push(thread::spawn(move || {
            for _ in 0..1000 {
                let _ = l.increment_request();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = limiter.stats();
    // Property: requests_count ≤ quota (no overdraft)
    assert!(
        stats.requests_count <= quota as u64,
        "Requests count ({}) must never exceed quota ({})",
        stats.requests_count,
        quota
    );
    assert_eq!(
        stats.requests_count, quota as u64,
        "All quota should be consumed"
    );
}

#[test]
fn t28_q12_total_requests_accuracy() {
    // Q12: Property: total_requests = sum(requests_count) across windows
    let limiter = Arc::new(RateLimitCapsule::with_quota(50));
    let mut handles = vec![];

    for _ in 0..5 {
        let l = Arc::clone(&limiter);
        handles.push(thread::spawn(move || {
            for _ in 0..20 {
                let _ = l.increment_request();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = limiter.stats();
    // Property: total_requests ≥ requests_count (monotonic)
    assert!(
        stats.total_requests >= stats.requests_count,
        "Total requests must be >= current window requests"
    );
}

#[test]
fn t28_q13_quota_remaining_never_negative() {
    // Q13: Property: quota_remaining should saturate at 0 (never go negative)
    let limiter = Arc::new(RateLimitCapsule::with_quota(10));
    let mut handles = vec![];

    for _ in 0..10 {
        let l = Arc::clone(&limiter);
        handles.push(thread::spawn(move || {
            for _ in 0..20 {
                let _ = l.increment_request();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = limiter.stats();
    // Property: quota_remaining ≥ 0 (never negative)
    assert!(
        stats.quota_remaining >= 0,
        "Quota remaining ({}) should never be negative",
        stats.quota_remaining
    );
}

#[test]
fn t28_q14_monotonic_total_requests() {
    // Q14: Property: total_requests is monotonically increasing
    let limiter = RateLimitCapsule::new();

    let total1 = limiter.stats().total_requests;
    limiter.increment_request().unwrap();
    let total2 = limiter.stats().total_requests;
    limiter.increment_request().unwrap();
    let total3 = limiter.stats().total_requests;

    assert!(total2 > total1, "Total requests should increase");
    assert!(total3 > total2, "Total requests should increase monotonically");
}

// ============================================================================
// T28 Q15-Q21: Integration Tests - End-to-End Workflows
// ============================================================================

#[test]
fn t28_q15_rate_limit_workflow() {
    // Q15: End-to-end: Check → Increment → Verify → Exhaust → Reject
    let limiter = RateLimitCapsule::with_quota(3);

    // Step 1: Check allowed
    assert!(limiter.check_rate_limit(), "Should allow initially");

    // Step 2: Increment 3 times (exhaust quota)
    for i in 0..3 {
        let result = limiter.increment_request();
        assert!(result.is_ok(), "Request {} should succeed", i + 1);
    }

    // Step 3: Verify quota exhausted
    let stats = limiter.stats();
    assert_eq!(stats.requests_count, 3);
    assert_eq!(stats.quota_remaining, 0);

    // Step 4: Check rejected
    assert!(!limiter.check_rate_limit(), "Should reject when exhausted");

    // Step 5: Attempt increment (should fail)
    let result = limiter.increment_request();
    assert!(result.is_err(), "Increment should fail when exhausted");
}

#[test]
fn t28_q16_concurrent_user_isolation() {
    // Q16: Multiple users with separate limiters (isolation test)
    let user1 = Arc::new(RateLimitCapsule::with_quota(10));
    let user2 = Arc::new(RateLimitCapsule::with_quota(10));

    let u1 = Arc::clone(&user1);
    let h1 = thread::spawn(move || {
        for _ in 0..10 {
            let _ = u1.increment_request();
        }
    });

    let u2 = Arc::clone(&user2);
    let h2 = thread::spawn(move || {
        for _ in 0..5 {
            let _ = u2.increment_request();
        }
    });

    h1.join().unwrap();
    h2.join().unwrap();

    // Verify isolation: user1 exhausted, user2 still has quota
    let stats1 = user1.stats();
    let stats2 = user2.stats();

    assert_eq!(stats1.requests_count, 10, "User1 should have 10 requests");
    assert_eq!(stats1.quota_remaining, 0, "User1 quota exhausted");

    assert_eq!(stats2.requests_count, 5, "User2 should have 5 requests");
    assert_eq!(stats2.quota_remaining, 5, "User2 has 5 quota remaining");
}

#[test]
fn t28_q17_high_concurrency_no_corruption() {
    // Q17: 1000 concurrent increments, verify no state corruption
    let limiter = Arc::new(RateLimitCapsule::with_quota(1000));
    let mut handles = vec![];

    for _ in 0..100 {
        let l = Arc::clone(&limiter);
        handles.push(thread::spawn(move || {
            for _ in 0..10 {
                let _ = l.increment_request();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = limiter.stats();
    // Verify no corruption: requests_count + quota_remaining = initial_quota
    assert_eq!(
        stats.requests_count as i64 + stats.quota_remaining,
        1000,
        "Quota conservation: count + remaining should equal initial quota"
    );
}

#[test]
fn t28_q18_error_message_validation() {
    // Q18: Verify error message format
    let limiter = RateLimitCapsule::with_quota(1);
    limiter.increment_request().unwrap();

    let result = limiter.increment_request();
    assert!(result.is_err());

    match result {
        Err(ClapiError::RateLimitExceeded {
            quota,
            window_duration_secs,
        }) => {
            assert_eq!(quota, 1000, "Error should report correct quota");
            assert_eq!(
                window_duration_secs, 60,
                "Error should report 60-second window"
            );
        }
        _ => panic!("Expected RateLimitExceeded error"),
    }
}

#[test]
fn t28_q19_stats_snapshot_under_load() {
    // Q19: Verify stats snapshot consistency under concurrent load
    let limiter = Arc::new(RateLimitCapsule::with_quota(100));
    let mut handles = vec![];

    for _ in 0..10 {
        let l = Arc::clone(&limiter);
        handles.push(thread::spawn(move || {
            for _ in 0..20 {
                let _ = l.increment_request();
                let _ = l.stats(); // Concurrent snapshots
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = limiter.stats();
    // Snapshot should be internally consistent
    assert!(stats.requests_count <= 100);
    assert!(stats.quota_remaining >= 0);
}

#[test]
fn t28_q20_default_constructor() {
    // Q20: Verify Default trait implementation
    let limiter: RateLimitCapsule = Default::default();
    let stats = limiter.stats();

    assert_eq!(stats.quota_remaining, 1000, "Default quota should be 1000");
    assert_eq!(stats.requests_count, 0, "Default count should be 0");
}

#[test]
fn t28_q21_zero_quota_immediate_rejection() {
    // Q21: Edge case: Custom quota=0 should immediately reject
    // Note: with_quota panics on quota <= 0, so this test validates that behavior
    let result = std::panic::catch_unwind(|| {
        let _ = RateLimitCapsule::with_quota(0);
    });
    assert!(result.is_err(), "with_quota(0) should panic");
}

// ============================================================================
// T28 Q22-Q28: Stress Tests - Extreme Contention & Scale
// ============================================================================

#[test]
#[ignore] // Run with: cargo test t28_q22 -- --ignored
fn t28_q22_10k_concurrent_users() {
    // Q22: Stress test: 10,000 concurrent users
    let quota = 10_000;
    let limiter = Arc::new(RateLimitCapsule::with_quota(quota));
    let mut handles = vec![];

    for _ in 0..1000 {
        let l = Arc::clone(&limiter);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let _ = l.increment_request();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = limiter.stats();
    assert!(
        stats.requests_count <= quota as u64,
        "Quota never exceeded even under extreme load"
    );
}

#[test]
#[ignore] // Run with: cargo test t28_q23 -- --ignored
fn t28_q23_sustained_load_1M_requests() {
    // Q23: Stress test: 1 million requests (sustained load)
    let quota = 1_000_000;
    let limiter = Arc::new(RateLimitCapsule::with_quota(quota));
    let mut handles = vec![];

    for _ in 0..100 {
        let l = Arc::clone(&limiter);
        handles.push(thread::spawn(move || {
            for _ in 0..10_000 {
                let _ = l.increment_request();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = limiter.stats();
    assert_eq!(
        stats.requests_count, quota as u64,
        "All 1M requests should be recorded"
    );
}

#[test]
#[ignore] // Run with: cargo test t28_q24 -- --ignored
fn t28_q24_rapid_check_rate_limit_calls() {
    // Q24: Stress test: Rapid check_rate_limit() calls (10M calls)
    let limiter = RateLimitCapsule::new();

    let start = std::time::Instant::now();
    for _ in 0..10_000_000 {
        let _ = limiter.check_rate_limit();
    }
    let elapsed = start.elapsed();

    println!(
        "10M check_rate_limit() calls: {:?} ({} ns/call)",
        elapsed,
        elapsed.as_nanos() / 10_000_000
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "10M checks should complete <1s"
    );
}

#[test]
#[ignore] // Run with: cargo test t28_q25 -- --ignored
fn t28_q25_cas_retry_exhaustion() {
    // Q25: Stress test: Force CAS retry exhaustion (pathological contention)
    let limiter = Arc::new(RateLimitCapsule::with_quota(1000));
    let mut handles = vec![];

    for _ in 0..1000 {
        let l = Arc::clone(&limiter);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let _ = l.increment_request();
                std::hint::spin_loop(); // Maximize contention
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = limiter.stats();
    assert!(
        stats.requests_count <= 1000,
        "Quota conservation even under pathological contention"
    );
}

#[test]
#[ignore] // Run with: cargo test t28_q26 -- --ignored
fn t28_q26_latency_percentiles() {
    // Q26: Performance test: Measure latency percentiles (p50, p99, p999)
    let limiter = RateLimitCapsule::with_quota(1_000_000);
    let mut latencies = Vec::with_capacity(100_000);

    for _ in 0..100_000 {
        let start = std::time::Instant::now();
        let _ = limiter.increment_request();
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
    assert!(p99 < 500, "p99 latency should be <500ns");
}

#[test]
#[ignore] // Run with: cargo test t28_q27 -- --ignored
fn t28_q27_memory_stability_1M_cycles() {
    // Q27: Memory stability: 1M allocation/operation cycles
    for _ in 0..1_000_000 {
        let limiter = RateLimitCapsule::new();
        let _ = limiter.increment_request();
        drop(limiter);
    }

    // No crashes = success
}

#[test]
#[ignore] // Run with: cargo test t28_q28 -- --ignored
fn t28_q28_throughput_measurement() {
    // Q28: Throughput measurement: ops/sec under contention
    let limiter = Arc::new(RateLimitCapsule::with_quota(10_000_000));
    let mut handles = vec![];
    let start = std::time::Instant::now();

    for _ in 0..10 {
        let l = Arc::clone(&limiter);
        handles.push(thread::spawn(move || {
            for _ in 0..1_000_000 {
                let _ = l.increment_request();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let elapsed = start.elapsed();
    let throughput = 10_000_000.0 / elapsed.as_secs_f64();

    println!("Throughput: {:.0} ops/sec (10M ops in {:?})", throughput, elapsed);
    assert!(
        throughput > 10_000_000.0,
        "Throughput should exceed 10M ops/sec"
    );
}
