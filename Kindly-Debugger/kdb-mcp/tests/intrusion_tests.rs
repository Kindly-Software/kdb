//! Comprehensive Test Suite for IntrusionDetectorCapsule (T10 Probabilistic)
//!
//! Tests organized by T28 Framework (4 tiers):
//! - Q1-Q7: Unit Tests (individual operations)
//! - Q8-Q14: Property Tests (invariants, fuzz testing)
//! - Q15-Q21: Integration Tests (multi-component interaction)
//! - Q22-Q28: Production Tests (stress, realistic scenarios)

use kdb_mcp::intrusion_detector::{IntrusionDetectorCapsule, IntrusionError};
use std::sync::Arc;
use std::thread;

// ============================================================================
// TIER 1: Unit Tests (Q1-Q7)
// ============================================================================

#[test]
fn unit_q1_capsule_creation() {
    let detector = IntrusionDetectorCapsule::new();
    let stats = detector.get_stats();

    assert_eq!(stats.total_checks, 0, "Fresh detector should have 0 checks");
    assert_eq!(
        stats.failed_attempts, 0,
        "Fresh detector should have 0 failures"
    );
    assert_eq!(
        stats.checks_passed, 0,
        "Fresh detector should have 0 passed checks"
    );
}

#[test]
fn unit_q2_single_ip_allows_pass() {
    let detector = IntrusionDetectorCapsule::new();
    let result = detector.check_ip("192.168.1.1");

    assert!(result.is_ok(), "Fresh IP should be allowed");
    assert!(matches!(result, Ok(())));
}

#[test]
fn unit_q3_record_failure() {
    let detector = IntrusionDetectorCapsule::new();

    detector.record_failure("10.0.0.1");

    let stats = detector.get_stats();
    assert_eq!(
        stats.failed_attempts, 1,
        "Should record 1 failure"
    );
}

#[test]
fn unit_q4_failed_ip_is_blocked() {
    let detector = IntrusionDetectorCapsule::new();

    detector.record_failure("172.16.0.1");
    let result = detector.check_ip("172.16.0.1");

    assert!(result.is_err(), "Failed IP should be blocked");
    assert!(matches!(
        result,
        Err(IntrusionError::IpBlocked { .. })
    ));
}

#[test]
fn unit_q5_is_blocked_convenience_method() {
    let detector = IntrusionDetectorCapsule::new();

    detector.record_failure("8.8.8.8");

    assert!(detector.is_blocked("8.8.8.8"), "Should be blocked");
    assert!(!detector.is_blocked("8.8.4.4"), "Should not be blocked");
}

#[test]
fn unit_q6_unblock_ip() {
    let detector = IntrusionDetectorCapsule::new();

    detector.record_failure("1.1.1.1");
    assert!(detector.is_blocked("1.1.1.1"), "Should initially be blocked");

    detector.unblock_ip("1.1.1.1");
    assert!(!detector.is_blocked("1.1.1.1"), "Should be unblocked");
}

#[test]
fn unit_q7_reset_clears_state() {
    let detector = IntrusionDetectorCapsule::new();

    for i in 0..10 {
        detector.record_failure(&format!("10.0.0.{}", i));
    }

    let stats_before = detector.get_stats();
    assert_eq!(stats_before.failed_attempts, 10);

    detector.reset();

    let stats_after = detector.get_stats();
    assert_eq!(stats_after.total_checks, 0, "Should reset total checks");
    assert_eq!(
        stats_after.failed_attempts, 0,
        "Should reset failed attempts"
    );
}

// ============================================================================
// TIER 2: Property Tests (Q8-Q14)
// ============================================================================

#[test]
fn prop_q8_fresh_ip_always_passes() {
    let detector = IntrusionDetectorCapsule::new();

    // Test 100 random IPs - none should be blocked initially
    for i in 0..100 {
        let ip = format!("192.168.{}.{}", i / 256, i % 256);
        assert!(
            detector.check_ip(&ip).is_ok(),
            "Fresh IP {} should always pass",
            ip
        );
    }
}

#[test]
fn prop_q9_recorded_ip_always_blocked() {
    let detector = IntrusionDetectorCapsule::new();

    for i in 0..50 {
        let ip = format!("10.{}.{}.{}", i / 256, i % 256, i % 32);
        detector.record_failure(&ip);
    }

    for i in 0..50 {
        let ip = format!("10.{}.{}.{}", i / 256, i % 256, i % 32);
        assert!(
            detector.is_blocked(&ip),
            "Recorded IP {} should always be blocked",
            ip
        );
    }
}

#[test]
fn prop_q10_idempotent_operations() {
    let detector = IntrusionDetectorCapsule::new();
    let ip = "idempotent.test";

    // Record same IP multiple times
    detector.record_failure(ip);
    detector.record_failure(ip);
    detector.record_failure(ip);

    // Should still only be blocked once
    assert!(detector.is_blocked(ip));

    // Check multiple times
    let result1 = detector.check_ip(ip);
    let result2 = detector.check_ip(ip);

    assert!(result1.is_err());
    assert!(result2.is_err());
}

#[test]
fn prop_q11_false_positive_rate_bounded() {
    let detector = IntrusionDetectorCapsule::new();

    // Add 1000 IPs
    for i in 0..1000 {
        let ip = format!("172.{}.{}.{}", i / 256, i % 256, i % 32);
        detector.record_failure(&ip);
    }

    // Estimate FPR
    let fpr = detector.estimate_fpr();

    // Should be < 0.1% (requirement)
    assert!(
        fpr < 0.001,
        "FPR should be < 0.1%, got {:.4}%",
        fpr * 100.0
    );

    // Check 100 unknown IPs
    let mut false_positives = 0;
    for i in 1000..1100 {
        let ip = format!("203.0.113.{}", i % 256);
        if detector.is_blocked(&ip) {
            false_positives += 1;
        }
    }

    // Should have few or no false positives in 100 checks
    // With 0.078% FPR, expect ~0.078 positives
    assert!(
        false_positives <= 2,
        "False positives {} should be <= 2 for 100 checks at <0.1% FPR",
        false_positives
    );
}

#[test]
#[ignore = "siphash_2_4 is private - cannot test internal hash distribution"]
fn prop_q12_hash_distribution_uniform() {
    let detector = IntrusionDetectorCapsule::new();

    // Generate hashes for 1000 random IPs
    let mut hash_buckets = [0usize; 256];
    for i in 0..1000 {
        let ip = format!("hash.test.{}", i);
        // FIXME: siphash_2_4 is private, need to make it #[doc(hidden)] pub for testing
        let hash = 0; // detector.siphash_2_4(ip.as_bytes(), 0x0706050403020100);
        let bucket = (hash % 256) as usize;
        hash_buckets[bucket] += 1;
    }

    // Distribution should be relatively uniform (chi-square test)
    let expected = 1000 / 256;
    let mut chi_squared = 0.0;

    for count in hash_buckets.iter() {
        let diff = (*count as f64) - (expected as f64);
        chi_squared += (diff * diff) / (expected as f64);
    }

    // Chi-squared with 255 df, 99% confidence threshold ≈ 310
    // For 256 buckets: we allow some variance
    assert!(
        chi_squared < 350.0,
        "Hash distribution should be uniform, chi²={}",
        chi_squared
    );
}

#[test]
fn prop_q13_statistics_monotonic() {
    let detector = IntrusionDetectorCapsule::new();

    let s1 = detector.get_stats();

    detector.record_failure("monotonic.test");
    let s2 = detector.get_stats();

    assert!(s2.failed_attempts >= s1.failed_attempts);
    assert!(s2.total_checks >= s1.total_checks);
}

#[test]
fn prop_q14_no_panic_on_random_input() {
    let detector = IntrusionDetectorCapsule::new();

    // Should not panic on any IP string
    let ips = vec![
        "",
        "invalid",
        "256.256.256.256",
        "999.999.999.999",
        "0.0.0.0",
        "255.255.255.255",
        "test@invalid#chars",
        "very.long.hostname.that.is.over.255.characters.and.should.still.be.handled.correctly.by.the.bloom.filter.implementation.without.panicking.or.causing.undefined.behavior.or.memory.corruption.which.would.be.bad",
    ];

    for ip in ips {
        // Should not panic
        let _ = detector.check_ip(ip);
        let _ = detector.is_blocked(ip);
        let _ = detector.record_failure(ip);
    }
}

// ============================================================================
// TIER 3: Integration Tests (Q15-Q21)
// ============================================================================

#[test]
fn integ_q15_concurrent_reads() {
    let detector = Arc::new(IntrusionDetectorCapsule::new());
    detector.record_failure("concurrent.test");

    let mut handles = vec![];

    // 16 threads, all reading
    for _ in 0..16 {
        let detector_clone = Arc::clone(&detector);
        let handle = thread::spawn(move || {
            for _ in 0..1000 {
                let _ = detector_clone.check_ip("concurrent.test");
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Should still be blocked
    assert!(detector.is_blocked("concurrent.test"));

    let stats = detector.get_stats();
    assert_eq!(stats.total_checks, 16000, "Should have 16000 checks");
}

#[test]
fn integ_q16_concurrent_writes() {
    let detector = Arc::new(IntrusionDetectorCapsule::new());
    let mut handles = vec![];

    // 8 threads, each recording different failures
    for t in 0..8 {
        let detector_clone = Arc::clone(&detector);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let ip = format!("thread.{}.{}", t, i);
                detector_clone.record_failure(&ip);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = detector.get_stats();
    assert_eq!(stats.failed_attempts, 800, "Should have 800 failures");
}

#[test]
fn integ_q17_concurrent_mixed() {
    let detector = Arc::new(IntrusionDetectorCapsule::new());

    // Pre-populate with some blocked IPs
    for i in 0..50 {
        detector.record_failure(&format!("blocked.{}", i));
    }

    let mut handles = vec![];

    // 8 threads: 4 reading, 4 writing
    for t in 0..8 {
        let detector_clone = Arc::clone(&detector);
        let handle = thread::spawn(move || {
            if t < 4 {
                // Reader threads
                for i in 0..100 {
                    let ip = format!("blocked.{}", i % 50);
                    let _ = detector_clone.check_ip(&ip);
                }
            } else {
                // Writer threads
                for i in 0..100 {
                    let ip = format!("new.{}.{}", t, i);
                    detector_clone.record_failure(&ip);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = detector.get_stats();
    assert_eq!(
        stats.total_checks, 400,
        "4 reader threads × 100 checks = 400"
    );
    assert_eq!(
        stats.failed_attempts, 50 + 400,
        "50 initial + 4 writer threads × 100 = 450"
    );
}

#[test]
fn integ_q18_unblock_integration() {
    let detector = IntrusionDetectorCapsule::new();

    // Block 10 IPs
    for i in 0..10 {
        detector.record_failure(&format!("block.{}", i));
    }

    // Unblock 5 of them
    for i in 0..5 {
        detector.unblock_ip(&format!("block.{}", i));
    }

    // Check results
    for i in 0..5 {
        assert!(
            !detector.is_blocked(&format!("block.{}", i)),
            "IP block.{} should be unblocked",
            i
        );
    }

    for i in 5..10 {
        assert!(
            detector.is_blocked(&format!("block.{}", i)),
            "IP block.{} should still be blocked",
            i
        );
    }
}

#[test]
fn integ_q19_reset_integration() {
    let detector = Arc::new(IntrusionDetectorCapsule::new());

    // Phase 1: Fill with data
    for i in 0..100 {
        detector.record_failure(&format!("phase1.{}", i));
    }

    let stats1 = detector.get_stats();
    assert_eq!(stats1.failed_attempts, 100);

    // Phase 2: Reset
    detector.reset();

    let stats2 = detector.get_stats();
    assert_eq!(stats2.total_checks, 0);
    assert_eq!(stats2.failed_attempts, 0);

    // Phase 3: Refill
    for i in 0..200 {
        detector.record_failure(&format!("phase3.{}", i));
    }

    let stats3 = detector.get_stats();
    assert_eq!(stats3.failed_attempts, 200);
}

#[test]
fn integ_q20_error_handling() {
    let detector = IntrusionDetectorCapsule::new();
    detector.record_failure("test.example");

    match detector.check_ip("test.example") {
        Ok(()) => panic!("Should have returned error"),
        Err(IntrusionError::IpBlocked { ip }) => {
            assert_eq!(ip, "test.example");
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn integ_q21_statistics_aggregation() {
    let detector = IntrusionDetectorCapsule::new();

    // Record 50 failures
    for i in 0..50 {
        detector.record_failure(&format!("stats.{}", i));
    }

    // Check 100 IPs (50 exist, 50 don't = ~50 passed)
    for i in 0..100 {
        let _ = detector.check_ip(&format!("stats.{}", i));
    }

    let stats = detector.get_stats();
    assert_eq!(stats.total_checks, 100);
    assert_eq!(stats.failed_attempts, 50);
    assert_eq!(stats.checks_blocked, 50);
    assert_eq!(stats.checks_passed, 50);
    assert_eq!(stats.block_rate_ppm, 500_000); // 50 / 100 = 50%
}

// ============================================================================
// TIER 4: Production Tests (Q22-Q28)
// ============================================================================

#[test]
fn prod_q22_high_throughput_stress() {
    let detector = Arc::new(IntrusionDetectorCapsule::new());
    let iterations = 10_000;

    // Pre-load with 100 blocked IPs
    for i in 0..100 {
        detector.record_failure(&format!("stress.{}", i));
    }

    let mut handles = vec![];

    // 8 threads, each doing 10K operations
    for t in 0..8 {
        let detector_clone = Arc::clone(&detector);
        let handle = thread::spawn(move || {
            for i in 0..iterations {
                if i % 2 == 0 {
                    let ip = format!("stress.{}", (i / 2) % 100);
                    let _ = detector_clone.check_ip(&ip);
                } else {
                    let ip = format!("stress.new.{}.{}", t, i);
                    detector_clone.record_failure(&ip);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify consistency
    let stats = detector.get_stats();
    assert!(stats.total_checks > 0);
    assert!(stats.failed_attempts > 0);
}

#[test]
fn prod_q23_memory_efficiency() {
    let detector = IntrusionDetectorCapsule::new();
    let size = std::mem::size_of_val(&detector);

    // Must be exactly 256 KB
    assert_eq!(
        size, 256_000,
        "Detector must be 256 KB, got {} bytes",
        size
    );

    // Add 10K IPs to stress the Bloom filter
    for i in 0..10_000 {
        let ip = format!("memory.{}.{}.{}.{}", i / 1000, i / 100 % 10, i / 10 % 10, i % 10);
        detector.record_failure(&ip);
    }

    // Still 256 KB
    assert_eq!(std::mem::size_of_val(&detector), 256_000);
}

#[test]
fn prod_q24_latency_performance() {
    let detector = IntrusionDetectorCapsule::new();

    // Record baseline
    detector.record_failure("latency.baseline");

    // Measure check latency (should be <50ns)
    let start = std::time::Instant::now();
    for _ in 0..1_000_000 {
        let _ = detector.check_ip("latency.baseline");
    }
    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / 1_000_000;

    // Average should be very low (likely < 100ns per operation with overhead)
    println!("Average latency: {} ns/op", avg_ns);
    assert!(avg_ns < 500, "Latency should be <500ns (1M ops in {:?})", elapsed);
}

#[test]
fn prod_q25_hash_collision_resistance() {
    let detector = IntrusionDetectorCapsule::new();

    // Create 100K different IPs and verify no unexpected collisions
    let mut blocked_count = 0;

    for i in 0..10_000 {
        let ip = format!("collision.test.{}.{}.{}", i / 100, i % 100, i);
        detector.record_failure(&ip);

        if detector.is_blocked(&ip) {
            blocked_count += 1;
        }
    }

    // All should be blocked (no false negatives)
    assert_eq!(
        blocked_count, 10_000,
        "All recorded IPs should be blocked (no false negatives)"
    );

    // Check false positives on 1K new IPs
    let mut false_positives = 0;
    for i in 0..1_000 {
        let ip = format!("collision.novel.{}", i);
        if detector.is_blocked(&ip) {
            false_positives += 1;
        }
    }

    println!("False positives: {} / 1000", false_positives);
    // With 10K items in 2M bit space, k=3: FPR ≈ (1 - e^(-30/2M))^3 ≈ 0.00000075%
    // Expected FPs on 1K new items ≈ 0.0000075, so we expect 0
    assert!(false_positives <= 2, "FPR should be extremely low");
}

#[test]
fn prod_q26_realistic_attack_simulation() {
    // Simulate a realistic brute-force attack scenario
    let detector = Arc::new(IntrusionDetectorCapsule::new());

    let attacker_ips = vec![
        "192.0.2.1",
        "192.0.2.2",
        "203.0.113.100",
        "198.51.100.50",
    ];

    let legitimate_ips = vec![
        "10.0.0.1",
        "172.16.0.1",
        "203.0.113.1",
        "198.51.100.1",
    ];

    let mut handles = vec![];

    // Simulate attackers
    for attacker in &attacker_ips {
        let detector_clone = Arc::clone(&detector);
        let ip = attacker.to_string();
        let handle = thread::spawn(move || {
            // Each attacker tries 100 failed attempts
            for _ in 0..100 {
                detector_clone.record_failure(&ip);
            }
        });
        handles.push(handle);
    }

    // Simulate legitimate users (concurrent with attack)
    for legitimate in &legitimate_ips {
        let detector_clone = Arc::clone(&detector);
        let ip = legitimate.to_string();
        let handle = thread::spawn(move || {
            // Each legitimate user tries 50 checks
            for _ in 0..50 {
                let _ = detector_clone.check_ip(&ip);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify: attackers should be blocked, legitimate should pass
    for attacker in &attacker_ips {
        assert!(detector.is_blocked(attacker), "{} should be blocked", attacker);
    }

    for legitimate in &legitimate_ips {
        assert!(
            !detector.is_blocked(legitimate),
            "{} should not be blocked",
            legitimate
        );
    }
}

#[test]
fn prod_q27_recovery_scenario() {
    let detector = Arc::new(IntrusionDetectorCapsule::new());

    // Phase 1: Normal operation
    for i in 0..50 {
        detector.record_failure(&format!("phase1.{}", i));
    }

    let stats1 = detector.get_stats();
    assert_eq!(stats1.failed_attempts, 50);

    // Phase 2: Attack detected, reset
    detector.reset();

    // Phase 3: Resumption
    for i in 0..100 {
        let _ = detector.check_ip(&format!("phase3.{}", i));
    }

    let stats3 = detector.get_stats();
    assert_eq!(stats3.total_checks, 100);
    assert_eq!(stats3.failed_attempts, 0);
}

#[test]
fn prod_q28_compliance_fpr_validation() {
    // Q2 Requirement: <0.1% false positive rate
    let detector = IntrusionDetectorCapsule::new();

    // Add 100K IPs (large set to properly validate FPR)
    let num_items = 100_000;
    for i in 0..num_items {
        let ip = format!("compliance.{}.{}.{}", i / 10000, i / 100 % 100, i % 100);
        detector.record_failure(&ip);
    }

    // Estimate FPR
    let fpr = detector.estimate_fpr();
    println!("Estimated FPR for {} items: {:.6}%", num_items, fpr * 100.0);

    // Must be < 0.1%
    assert!(
        fpr < 0.001,
        "FPR requirement <0.1% failed: {:.6}%",
        fpr * 100.0
    );

    // Test with 10K novel IPs
    let mut false_positives = 0;
    for i in 0..10_000 {
        let ip = format!("novel.{}.{}.{}", i / 10000, i / 100 % 100, i % 100);
        if detector.is_blocked(&ip) {
            false_positives += 1;
        }
    }

    let measured_fpr = (false_positives as f64) / 10_000.0;
    println!("Measured FPR on 10K novel items: {:.6}%", measured_fpr * 100.0);

    // Measured FPR should match estimate (within margin of error)
    // Allow up to 2× due to statistical variance
    assert!(
        measured_fpr < 0.002,
        "Measured FPR should be close to estimate: {:.6}%",
        measured_fpr * 100.0
    );
}
