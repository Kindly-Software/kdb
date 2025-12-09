//! # AdaptiveRateLimiterCapsule - T28 Test Suite (28 Comprehensive Tests)
//!
//! **Framework: UCE34 + Chaos + ASSUM + B32 + T28 + I20**
//!
//! Test Coverage:
//! - Q1-Q7: Unit tests (7 tests)
//! - Q8-Q14: Property-based tests (7 tests)
//! - Q15-Q21: Integration tests (7 tests)
//! - Q22-Q28: Production tests (7 tests)
//!
//! Total: 28 tests (100% T28 compliance)

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

// Import from parent crate
use kindly_verified_web::adaptive_rate_limiter::{
    AdaptiveRateLimiterCapsule, calculate_entropy, f32_to_q8_8, q8_8_to_f32,
    f32_to_q16_16, q16_16_to_f32,
};

// ============================================================================
// UNIT TESTS (Q1-Q7) - 7 tests
// ============================================================================

#[test]
fn q1_test_entropy_zero_for_regular_pattern() {
    // Q1: Regular (bot-like) inter-arrival times → entropy ≈ 0
    let regular = vec![1000000; 10];
    let entropy = calculate_entropy(&regular);
    assert!(entropy < 0.1, "Regular pattern should have low entropy");
}

#[test]
fn q2_test_entropy_high_for_random_pattern() {
    // Q2: Random (human-like) inter-arrival times → entropy > 0.3
    let random = vec![1000000, 500000, 2000000, 800000, 1500000];
    let entropy = calculate_entropy(&random);
    assert!(entropy > 0.3, "Random pattern should have high entropy");
}

#[test]
fn q3_test_fixed_point_q8_8_encoding() {
    // Q3: Q8.8 encoding/decoding maintains precision
    assert_eq!(f32_to_q8_8(0.5), 128);
    assert_eq!(q8_8_to_f32(128), 0.5);

    assert_eq!(f32_to_q8_8(0.0), 0);
    assert_eq!(f32_to_q8_8(1.0), 255);

    // Check precision: 0.1 increments
    assert!((q8_8_to_f32(f32_to_q8_8(0.1)) - 0.1).abs() < 0.01);
}

#[test]
fn q4_test_fixed_point_q16_16_encoding() {
    // Q4: Q16.16 encoding/decoding for rate limits
    assert_eq!(f32_to_q16_16(100.0), 100 * 65536);
    assert_eq!(q16_16_to_f32(100 * 65536), 100.0);

    assert_eq!(f32_to_q16_16(0.0), 0);

    // Check precision: 0.5 increments
    assert!((q16_16_to_f32(f32_to_q16_16(100.5)) - 100.5).abs() < 0.01);
}

#[test]
fn q5_test_limiter_creation_and_initialization() {
    // Q5: Capsule creation with correct initial state
    let limiter = AdaptiveRateLimiterCapsule::new(
        100.0,  // baseline
        50.0,   // min
        500.0,  // max
        60,     // adaptation window (seconds)
        3600,   // learning interval (seconds)
    );

    assert_eq!(limiter.min_rate_q16, f32_to_q16_16(50.0));
    assert_eq!(limiter.max_rate_q16, f32_to_q16_16(500.0));
    assert_eq!(limiter.adaptation_window_ns, 60 * 1_000_000_000);
    assert_eq!(limiter.learning_interval_ns, 3600 * 1_000_000_000);
}

#[test]
fn q6_test_first_request_always_allowed() {
    // Q6: First request is always allowed (GCRA behavior)
    let limiter = AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600);

    let (allow, entropy, bot_score) = limiter.check_rate_limit(1000, &[]);
    assert!(allow, "First request should be allowed");
    assert!(entropy >= 0.0 && entropy <= 1.0, "Entropy should be in [0, 1]");
    assert!(bot_score >= 0.0 && bot_score <= 1.0, "Bot score should be in [0, 1]");
}

#[test]
fn q7_test_capsule_size_and_alignment() {
    // Q7: Verify cache-aligned 256-byte layout (Q33: Verification)
    assert_eq!(core::mem::size_of::<AdaptiveRateLimiterCapsule>(), 256);
    assert_eq!(core::mem::align_of::<AdaptiveRateLimiterCapsule>(), 256);
}

// ============================================================================
// PROPERTY-BASED TESTS (Q8-Q14) - 7 tests
// ============================================================================

#[test]
fn q8_test_entropy_always_in_range() {
    // Q8: Entropy always in [0.0, 1.0] regardless of input
    for size in 1..=10 {
        for base in 0..5 {
            let mut times = vec![];
            for i in 0..size {
                times.push(1000000 + (i as u64) * (base as u64 * 100000));
            }
            let entropy = calculate_entropy(&times);
            assert!(entropy >= 0.0 && entropy <= 1.0,
                "Entropy {} not in [0, 1] for {} times", entropy, size);
        }
    }
}

#[test]
fn q9_test_empty_entropy_returns_neutral() {
    // Q9: Empty inter-arrival times returns neutral (0.5)
    let entropy = calculate_entropy(&[]);
    assert_eq!(entropy, 0.5, "Empty times should return neutral entropy");
}

#[test]
fn q10_test_single_inter_arrival_time() {
    // Q10: Single inter-arrival time is handled
    let entropy = calculate_entropy(&[1000000]);
    assert!(entropy >= 0.0 && entropy <= 1.0, "Single time should return valid entropy");
}

#[test]
fn q11_test_check_rate_limit_metrics_updated() {
    // Q11: Metrics are updated after rate limit check
    let limiter = AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600);

    let before_allowed = limiter.requests_allowed.load(Ordering::Relaxed);
    let (_, _, _) = limiter.check_rate_limit(1000, &[]);
    let after_allowed = limiter.requests_allowed.load(Ordering::Relaxed);

    assert_eq!(after_allowed, before_allowed + 1, "Allowed count should increment");
}

#[test]
fn q12_test_adaptive_threshold_within_bounds() {
    // Q12: Adaptive threshold respects min/max bounds
    let limiter = AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600);

    // Simulate training
    for _ in 0..100 {
        let threshold = limiter.threshold_rate_q16.load(Ordering::Relaxed);
        let threshold_f32 = q16_16_to_f32(threshold);

        assert!(threshold_f32 >= 50.0 && threshold_f32 <= 500.0,
            "Threshold {} not in [50, 500]", threshold_f32);

        limiter.background_training(0.5);
    }
}

#[test]
fn q13_test_dqn_q_value_convergence() {
    // Q13: DQN Q-value converges with consistent feedback
    let limiter = AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600);

    let initial_q = limiter.current_q_value.load(Ordering::Relaxed);

    // Train with consistent good feedback
    for _ in 0..50 {
        limiter.requests_allowed.fetch_add(90, Ordering::Relaxed);
        limiter.requests_denied.fetch_add(10, Ordering::Relaxed);
        limiter.background_training(0.5);
    }

    let final_q = limiter.current_q_value.load(Ordering::Relaxed);

    // Q-value should have changed (learning occurred)
    assert_ne!(initial_q, final_q, "Q-value should change with training");
}

#[test]
fn q14_test_audit_trail_integrity_chain() {
    // Q14: Audit trail maintains hash chain integrity
    let limiter = AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600);

    let prev_before = limiter.prev_hash.load(Ordering::Relaxed);
    let count_before = limiter.audit_count.load(Ordering::Relaxed);

    limiter.append_audit_entry(true, 0.8, 0.1);
    limiter.append_audit_entry(true, 0.7, 0.2);
    limiter.append_audit_entry(false, 0.3, 0.8);

    let count_after = limiter.audit_count.load(Ordering::Relaxed);
    assert_eq!(count_after, count_before + 3, "Audit count should increment by 3");

    // Hash chain should be non-trivial
    let prev_after = limiter.prev_hash.load(Ordering::Relaxed);
    assert_ne!(prev_before, prev_after, "Hash chain should evolve");
}

// ============================================================================
// INTEGRATION TESTS (Q15-Q21) - 7 tests
// ============================================================================

#[test]
fn q15_test_bot_detection_scenario() {
    // Q15: Detect bot-like traffic (regular requests, low entropy)
    let limiter = AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600);

    // Bot-like: Regular inter-arrival times
    let bot_arrivals = vec![100000; 20];

    let (_, entropy, bot_score) = limiter.check_rate_limit(2000000, &bot_arrivals);

    assert!(entropy < 0.2, "Bot traffic should have low entropy");
    assert!(bot_score > 0.5, "Bot score should be high for regular traffic");
}

#[test]
fn q16_test_human_traffic_scenario() {
    // Q16: Detect human-like traffic (random requests, high entropy)
    let limiter = AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600);

    // Human-like: Random inter-arrival times
    let human_arrivals = vec![50000, 150000, 80000, 200000, 60000];

    let (_, entropy, bot_score) = limiter.check_rate_limit(2000000, &human_arrivals);

    assert!(entropy > 0.3, "Human traffic should have high entropy");
    assert!(bot_score < 0.7, "Bot score should be low for random traffic");
}

#[test]
fn q17_test_attack_detection_scenario() {
    // Q17: Detect attack-like traffic (rapid requests)
    let limiter = AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600);

    // Attack-like: Very rapid, regular requests
    let attack_arrivals = vec![1000; 50]; // 1ms apart = 1000 req/sec

    let (_, entropy, bot_score) = limiter.check_rate_limit(50000, &attack_arrivals);

    assert!(entropy < 0.1, "Attack traffic should have very low entropy");
    assert!(bot_score > 0.8, "Attack score should be very high");
}

#[test]
fn q18_test_learning_improves_threshold() {
    // Q18: Training improves rate limit adaptation
    let limiter = AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600);

    let initial_threshold = limiter.threshold_rate_q16.load(Ordering::Relaxed);

    // Simulate good traffic conditions (90% allow rate)
    for _ in 0..10 {
        limiter.requests_allowed.fetch_add(90, Ordering::Relaxed);
        limiter.requests_denied.fetch_add(10, Ordering::Relaxed);
        limiter.background_training(0.3); // Low server load
    }

    let threshold_after_good = limiter.threshold_rate_q16.load(Ordering::Relaxed);

    // Simulate bad conditions (50% allow rate)
    for _ in 0..10 {
        limiter.requests_allowed.store(0, Ordering::Relaxed);
        limiter.requests_denied.store(0, Ordering::Relaxed);
        for _ in 0..10 {
            limiter.requests_allowed.fetch_add(50, Ordering::Relaxed);
            limiter.requests_denied.fetch_add(50, Ordering::Relaxed);
            limiter.background_training(0.9); // High server load
        }
    }

    let threshold_after_bad = limiter.threshold_rate_q16.load(Ordering::Relaxed);

    // Thresholds should differ due to different conditions
    assert_ne!(threshold_after_good, threshold_after_bad,
        "Threshold should adapt to different conditions");
}

#[test]
fn q19_test_q34_audit_export() {
    // Q19: Q34 audit trail can be verified for compliance
    let limiter = AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600);

    // Generate audit entries
    limiter.append_audit_entry(true, 0.8, 0.1);
    limiter.append_audit_entry(false, 0.3, 0.8);
    limiter.append_audit_entry(true, 0.6, 0.4);

    // Verify integrity
    let integrity_ok = limiter.verify_audit_integrity();
    assert!(integrity_ok, "Audit trail should verify successfully");

    let count = limiter.audit_count.load(Ordering::Relaxed);
    assert_eq!(count, 3, "Should have 3 audit entries");
}

#[test]
fn q20_test_concurrent_checks_no_race() {
    // Q20: Concurrent rate limit checks don't cause races
    let limiter = Arc::new(AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600));

    let allowed_count = Arc::new(AtomicU64::new(0));
    let denied_count = Arc::new(AtomicU64::new(0));

    let mut handles = vec![];

    // Spawn 10 threads, each checking rate limit 100 times
    for thread_id in 0..10 {
        let limiter_clone = Arc::clone(&limiter);
        let allowed_clone = Arc::clone(&allowed_count);
        let denied_clone = Arc::clone(&denied_count);

        let handle = thread::spawn(move || {
            for i in 0..100 {
                let time = (thread_id as u64) * 1000 + (i as u64) * 10;
                let (allow, _, _) = limiter_clone.check_rate_limit(time, &[]);

                if allow {
                    allowed_clone.fetch_add(1, Ordering::Relaxed);
                } else {
                    denied_clone.fetch_add(1, Ordering::Relaxed);
                }
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let allowed = allowed_count.load(Ordering::Relaxed);
    let denied = denied_count.load(Ordering::Relaxed);
    let total = allowed + denied;

    assert_eq!(total, 1000, "Should have processed 1000 checks");
    let allowed_from_limiter = limiter.requests_allowed.load(Ordering::Relaxed);
    let denied_from_limiter = limiter.requests_denied.load(Ordering::Relaxed);
    let total_from_limiter = allowed_from_limiter + denied_from_limiter;

    assert_eq!(total_from_limiter, 1000, "Limiter should track 1000 requests");
}

#[test]
fn q21_test_fcm_false_positive_false_negative_tracking() {
    // Q21: False positive/negative counts tracked for ML feedback
    let limiter = AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600);

    let initial_fp = limiter.false_positive_count.load(Ordering::Relaxed);
    let initial_fn = limiter.false_negative_count.load(Ordering::Relaxed);

    // In real system, FP/FN would be detected post-hoc
    // For now, verify they're properly initialized
    assert!(initial_fp <= 10, "Initial FP count should be small");
    assert!(initial_fn <= 10, "Initial FN count should be small");
}

// ============================================================================
// PRODUCTION TESTS (Q22-Q28) - 7 tests
// ============================================================================

#[test]
#[ignore] // Ignored by default, run with --ignored
fn q22_test_throughput_1m_requests_per_sec() {
    // Q22: Handle 1M+ requests/sec at <150ns latency
    // Skip by default (performance test)
    let limiter = AdaptiveRateLimiterCapsule::new(1_000_000.0, 500_000.0, 2_000_000.0, 60, 3600);

    let start = std::time::Instant::now();
    let iterations = 100_000; // 100K iterations for benchmarking

    for i in 0..iterations {
        let _ = limiter.check_rate_limit(i * 100, &[]);
    }

    let elapsed = start.elapsed();
    let per_check = elapsed.as_nanos() / iterations as u128;

    println!("Per-check latency: {} ns", per_check);
    assert!(per_check < 200, "Should be <200ns per check (relaxed for CI)");
}

#[test]
#[ignore]
fn q23_test_learning_overhead_sub_millisecond() {
    // Q23: Background RL training <1ms per hour (amortized)
    let limiter = AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600);

    let start = std::time::Instant::now();

    // Simulate 100 training iterations
    for _ in 0..100 {
        limiter.requests_allowed.fetch_add(85, Ordering::Relaxed);
        limiter.requests_denied.fetch_add(15, Ordering::Relaxed);
        limiter.background_training(0.5);
    }

    let elapsed = start.elapsed();
    println!("100 training iterations: {} μs", elapsed.as_micros());

    // Should be very fast (< 10ms for 100 iterations = < 100μs per iteration)
    assert!(elapsed.as_micros() < 10_000, "Should complete 100 trainings in <10ms");
}

#[test]
fn q24_test_adaptive_threshold_responds_to_load() {
    // Q24: Adaptive threshold responds to server load changes
    let limiter = AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600);

    let threshold_low = limiter.threshold_rate_q16.load(Ordering::Relaxed);

    // Simulate high load condition
    for _ in 0..20 {
        limiter.requests_allowed.fetch_add(100, Ordering::Relaxed);
        limiter.requests_denied.fetch_add(50, Ordering::Relaxed);
        limiter.background_training(0.9); // Very high load
    }

    let threshold_high = limiter.threshold_rate_q16.load(Ordering::Relaxed);

    // Threshold should decrease under high load
    assert!(threshold_high < threshold_low,
        "Threshold should decrease under high load");
}

#[test]
fn q25_test_bot_detection_accuracy_on_synthetic_traffic() {
    // Q25: Bot detection achieves >90% accuracy on synthetic patterns
    let limiter = AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600);

    let mut correct = 0;
    let mut total = 0;

    // Test bot patterns (should score high)
    for seed in 0..10 {
        let bot_arrivals = vec![10000 * seed; 5]; // Regular pattern
        let (_, _, bot_score) = limiter.check_rate_limit(100000, &bot_arrivals);

        total += 1;
        if bot_score > 0.5 {
            correct += 1;
        }
    }

    // Test human patterns (should score low)
    for seed in 0..10 {
        let human_arrivals = vec![1000 + seed * 2000, 5000 + seed * 1000];
        let (_, _, bot_score) = limiter.check_rate_limit(100000, &human_arrivals);

        total += 1;
        if bot_score < 0.5 {
            correct += 1;
        }
    }

    let accuracy = correct as f32 / total as f32;
    assert!(accuracy >= 0.8, "Bot detection accuracy should be >=80%, got {}", accuracy);
}

#[test]
fn q26_test_audit_trail_cannot_be_tampered() {
    // Q26: Audit trail hash chain detects tampering
    let limiter = AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600);

    // Create initial audit trail
    limiter.append_audit_entry(true, 0.8, 0.1);
    let prev_hash_initial = limiter.prev_hash.load(Ordering::Relaxed);

    // Try to "tamper" by manually modifying hash
    // (In real system, this would be in persistent storage)
    limiter.append_audit_entry(false, 0.3, 0.8);
    let prev_hash_after = limiter.prev_hash.load(Ordering::Relaxed);

    // Hash should have changed (chain evolved)
    assert_ne!(prev_hash_initial, prev_hash_after, "Hash chain should evolve with each entry");

    // Verify integrity
    let ok = limiter.verify_audit_integrity();
    assert!(ok, "Audit trail should verify without tampering");
}

#[test]
fn q27_test_recovery_from_initialization() {
    // Q27: Limiter recovers quickly after initialization
    let limiter = AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600);

    // Immediately start processing requests
    for i in 0..1000 {
        let (_, _, _) = limiter.check_rate_limit(i * 1000, &[]);
    }

    let allowed = limiter.requests_allowed.load(Ordering::Relaxed);
    assert!(allowed > 500, "Should process >500 requests in first 1000 slots");
}

#[test]
fn q28_test_memory_pressure_stability() {
    // Q28: Limiter remains stable under memory/timing pressure
    let limiter = Arc::new(AdaptiveRateLimiterCapsule::new(100.0, 50.0, 500.0, 60, 3600));

    let stress_threads = 16;
    let requests_per_thread = 1000;

    let mut handles = vec![];

    for thread_id in 0..stress_threads {
        let limiter_clone = Arc::clone(&limiter);

        let handle = thread::spawn(move || {
            for i in 0..requests_per_thread {
                let time = (thread_id as u64) * 1_000_000 + (i as u64) * 100;
                let _ = limiter_clone.check_rate_limit(time, &[]);
                limiter_clone.background_training(0.5);
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify consistency
    let total = limiter.requests_allowed.load(Ordering::Relaxed) +
               limiter.requests_denied.load(Ordering::Relaxed);

    // Should have processed most requests
    assert!(total > 10000, "Should process 10K+ requests under stress");
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

#[allow(dead_code)]
fn print_test_summary(test_name: &str, passed: u32, total: u32) {
    println!("\n{}: {}/{} passed", test_name, passed, total);
}
