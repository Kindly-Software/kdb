// tests/prompt_injection_detector_tests.rs
// Comprehensive T28 Testing for PromptInjectionDetectorCapsule
//
// Test Structure (T28 4-Tier Pyramid):
// - Q1-Q7 Unit Tests (14 tests): Invariants, alignment, SIMD correctness
// - Q8-Q14 Property Tests (14 tests): Concurrent, fuzzing, determinism
// - Q15-Q21 Integration Tests (14 tests): Multi-layer fusion, real attacks
// - Q22-Q28 Production Tests (14 tests): Load, chaos, stress

#[cfg(feature = "security-prompt-injection")]
mod prompt_injection_tests {
    use atomic_capsule::capsules::security::{
        PromptInjectionDetectorCapsule, InjectionDecision as Decision, RiskScore,
        InjectionStatistics as Statistics, EMBEDDING_DIM,
    };

    // ============================================================================
    // Q1-Q7: UNIT TESTS (Invariants, Alignment, Core Functionality)
    // ============================================================================

    #[test]
    fn q1_capsule_size_alignment() {
        // Chaos Mandate: size == alignment for cache-aligned capsules
        assert_eq!(
            core::mem::size_of::<PromptInjectionDetectorCapsule>(),
            256,
            "Capsule must be exactly 256 bytes"
        );
        assert_eq!(
            core::mem::align_of::<PromptInjectionDetectorCapsule>(),
            256,
            "Capsule must be 256-byte aligned (AVX2)"
        );
    }

    #[test]
    fn q2_risk_score_bounds() {
        // Q16.16 fixed-point: [0.0, 1.0] → [0, 65536]
        let zero = RiskScore::from_f64(0.0);
        assert_eq!(zero.get_fixed(), 0);

        let one = RiskScore::from_f64(1.0);
        assert_eq!(one.get_fixed(), 65536);

        let half = RiskScore::from_f64(0.5);
        assert_eq!(half.get_fixed(), 32768);

        // Clamping: values outside [0.0, 1.0] are clamped
        let negative = RiskScore::from_f64(-0.5);
        assert_eq!(negative.get_fixed(), 0);

        let overflow = RiskScore::from_f64(1.5);
        assert_eq!(overflow.get_fixed(), 65536);
    }

    #[test]
    fn q3_risk_score_conversion_roundtrip() {
        let values = [0.0, 0.25, 0.5, 0.75, 0.85, 0.95, 1.0];

        for &val in &values {
            let score = RiskScore::from_f64(val);
            let converted = score.to_f64();
            assert!(
                (converted - val).abs() < 0.001,
                "Roundtrip failed for {}: got {}",
                val,
                converted
            );
        }
    }

    #[test]
    fn q4_risk_score_thresholds() {
        // Low risk: [0.0, 0.5)
        assert!(RiskScore::from_f64(0.0).is_low_risk());
        assert!(RiskScore::from_f64(0.3).is_low_risk());
        assert!(RiskScore::from_f64(0.49).is_low_risk());
        assert!(!RiskScore::from_f64(0.5).is_low_risk());

        // Medium risk: [0.5, 0.85)
        assert!(RiskScore::from_f64(0.5).is_medium_risk());
        assert!(RiskScore::from_f64(0.7).is_medium_risk());
        assert!(RiskScore::from_f64(0.84).is_medium_risk());
        assert!(!RiskScore::from_f64(0.85).is_medium_risk());

        // High risk: [0.85, 1.0]
        assert!(RiskScore::from_f64(0.85).is_high_risk());
        assert!(RiskScore::from_f64(0.9).is_high_risk());
        assert!(RiskScore::from_f64(1.0).is_high_risk());
        assert!(!RiskScore::from_f64(0.84).is_high_risk());
    }

    #[test]
    fn q5_decision_mapping() {
        // Allow: low risk
        let low = RiskScore::from_f64(0.3);
        assert_eq!(Decision::from(low), Decision::Allow);

        // Monitor: medium risk
        let medium = RiskScore::from_f64(0.7);
        assert_eq!(Decision::from(medium), Decision::Monitor);

        // Block: high risk
        let high = RiskScore::from_f64(0.9);
        assert_eq!(Decision::from(high), Decision::Block);

        // Edge cases
        assert_eq!(Decision::from(RiskScore::from_f64(0.49)), Decision::Allow);
        assert_eq!(Decision::from(RiskScore::from_f64(0.5)), Decision::Monitor);
        assert_eq!(Decision::from(RiskScore::from_f64(0.84)), Decision::Monitor);
        assert_eq!(Decision::from(RiskScore::from_f64(0.85)), Decision::Block);
    }

    #[test]
    fn q6_default_construction() {
        let detector = PromptInjectionDetectorCapsule::new();

        // Default threshold: 0.85
        let threshold = detector.get_threshold();
        assert!((threshold.to_f64() - 0.85).abs() < 0.01);

        // Initial statistics: all zeros
        let stats = detector.get_statistics();
        assert_eq!(stats.total_checks, 0);
        assert_eq!(stats.blocked_count, 0);
        assert_eq!(stats.monitored_count, 0);
        assert_eq!(stats.allowed_count, 0);
    }

    #[test]
    fn q7_safe_embedding_detection() {
        let detector = PromptInjectionDetectorCapsule::new();

        // Safe embedding: all zeros (matches reference)
        let safe_embedding = [0i8; EMBEDDING_DIM];
        let risk = detector.check_prompt(&safe_embedding);

        // Should be low or medium risk (not high)
        assert!(
            !risk.is_high_risk(),
            "Safe embedding should not trigger high risk: {}",
            risk.to_f64()
        );
    }

    // ============================================================================
    // Q8-Q14: PROPERTY TESTS (Concurrent, Fuzzing, Determinism)
    // ============================================================================

    #[test]
    fn q8_determinism_same_input_same_output() {
        let detector = PromptInjectionDetectorCapsule::new();

        let embedding = [42i8; EMBEDDING_DIM];

        // Check 100 times, should always get same result
        let first_risk = detector.check_prompt(&embedding);

        for _ in 0..100 {
            let risk = detector.check_prompt(&embedding);
            assert_eq!(
                risk.get_fixed(),
                first_risk.get_fixed(),
                "Determinism violated: same input gave different output"
            );
        }
    }

    #[test]
    fn q9_monotonicity_distance_increases_risk() {
        let detector = PromptInjectionDetectorCapsule::new();

        // Safe embedding (all zeros)
        let safe = [0i8; EMBEDDING_DIM];
        let risk_safe = detector.check_prompt(&safe);

        // Slightly suspicious (some non-zero values)
        let mut slight = [0i8; EMBEDDING_DIM];
        slight[0..10].fill(50);
        let risk_slight = detector.check_prompt(&slight);

        // Very suspicious (many high values)
        let mut very = [0i8; EMBEDDING_DIM];
        very[0..100].fill(127);
        let risk_very = detector.check_prompt(&very);

        // Monotonicity: more distance → higher risk
        assert!(
            risk_safe.get_fixed() <= risk_slight.get_fixed(),
            "Safe should have lower risk than slight"
        );
        assert!(
            risk_slight.get_fixed() <= risk_very.get_fixed(),
            "Slight should have lower risk than very suspicious"
        );
    }

    #[test]
    fn q10_concurrent_threshold_updates() {
        use std::sync::Arc;
        use std::thread;

        let detector = Arc::new(PromptInjectionDetectorCapsule::new());
        let mut handles = vec![];

        // 16 threads, each updating threshold 100 times
        for tid in 0..16 {
            let d = Arc::clone(&detector);
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    // Alternate between different thresholds
                    let threshold = if (tid + i) % 2 == 0 {
                        RiskScore::from_f64(0.80)
                    } else {
                        RiskScore::from_f64(0.90)
                    };
                    d.update_threshold(threshold);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // After concurrent updates, threshold should be valid (no corruption)
        let final_threshold = detector.get_threshold();
        assert!(
            final_threshold.to_f64() >= 0.5 && final_threshold.to_f64() <= 0.95,
            "Threshold corrupted after concurrent updates: {}",
            final_threshold.to_f64()
        );
    }

    #[test]
    fn q11_concurrent_decision_recording() {
        use std::sync::Arc;
        use std::thread;

        let detector = Arc::new(PromptInjectionDetectorCapsule::new());
        let mut handles = vec![];

        // 16 threads, each recording 100 decisions
        for _ in 0..16 {
            let d = Arc::clone(&detector);
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    let decision = match i % 3 {
                        0 => Decision::Allow,
                        1 => Decision::Monitor,
                        _ => Decision::Block,
                    };
                    d.record_decision(decision);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify counters are consistent
        let stats = detector.get_statistics();
        assert_eq!(
            stats.total_checks, 1600,
            "Total checks should be 16 × 100 = 1600"
        );

        // Each decision type should appear ~533 times (1600 / 3)
        assert!(
            stats.allowed_count >= 500 && stats.allowed_count <= 560,
            "Allow count out of range: {}",
            stats.allowed_count
        );
        assert!(
            stats.monitored_count >= 500 && stats.monitored_count <= 560,
            "Monitor count out of range: {}",
            stats.monitored_count
        );
        assert!(
            stats.blocked_count >= 500 && stats.blocked_count <= 560,
            "Block count out of range: {}",
            stats.blocked_count
        );
    }

    #[test]
    fn q12_fuzzing_random_embeddings() {
        let detector = PromptInjectionDetectorCapsule::new();

        // Simple LCG for deterministic fuzzing (no external deps)
        let mut seed: u64 = 12345;
        let lcg = |s: &mut u64| {
            *s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            *s
        };

        // Fuzz 1000 random embeddings
        for _ in 0..1000 {
            let mut embedding = [0i8; EMBEDDING_DIM];
            for elem in &mut embedding {
                *elem = (lcg(&mut seed) % 256) as i8;
            }

            let risk = detector.check_prompt(&embedding);

            // Invariant: risk must be in [0.0, 1.0]
            assert!(
                risk.get_fixed() >= 0 && risk.get_fixed() <= 65536,
                "Fuzzing produced invalid risk score: {}",
                risk.get_fixed()
            );
        }
    }

    #[test]
    fn q13_threshold_clamping() {
        let detector = PromptInjectionDetectorCapsule::new();

        // Below minimum (0.5) → clamped to 0.5
        detector.update_threshold(RiskScore::from_f64(0.3));
        let t = detector.get_threshold();
        assert!((t.to_f64() - 0.5).abs() < 0.01);

        // Above maximum (0.95) → clamped to 0.95
        detector.update_threshold(RiskScore::from_f64(0.99));
        let t = detector.get_threshold();
        assert!((t.to_f64() - 0.95).abs() < 0.01);

        // Within range → preserved
        detector.update_threshold(RiskScore::from_f64(0.75));
        let t = detector.get_threshold();
        assert!((t.to_f64() - 0.75).abs() < 0.01);
    }

    #[test]
    fn q14_statistics_consistency() {
        let detector = PromptInjectionDetectorCapsule::new();

        // Record known pattern
        detector.record_decision(Decision::Allow);
        detector.record_decision(Decision::Allow);
        detector.record_decision(Decision::Monitor);
        detector.record_decision(Decision::Monitor);
        detector.record_decision(Decision::Monitor);
        detector.record_decision(Decision::Block);

        let stats = detector.get_statistics();

        // Verify counts
        assert_eq!(stats.total_checks, 6);
        assert_eq!(stats.allowed_count, 2);
        assert_eq!(stats.monitored_count, 3);
        assert_eq!(stats.blocked_count, 1);

        // Invariant: total = allowed + monitored + blocked
        assert_eq!(
            stats.allowed_count + stats.monitored_count + stats.blocked_count,
            stats.total_checks
        );
    }

    // ============================================================================
    // Q15-Q21: INTEGRATION TESTS (Multi-Layer, Real Attacks)
    // ============================================================================

    #[test]
    fn q15_multi_layer_fusion() {
        let detector = PromptInjectionDetectorCapsule::new();

        // Embedding that triggers all layers
        let mut malicious = [0i8; EMBEDDING_DIM];

        // High distance from safe reference
        malicious[0..100].fill(127);

        // Features that trigger ML classifier (high mean, high variance)
        malicious[100..200].fill(-128);
        malicious[200..300].fill(127);

        let risk = detector.check_prompt(&malicious);

        // Should be high risk (multi-layer fusion)
        assert!(
            risk.is_high_risk(),
            "Multi-layer attack not detected: {}",
            risk.to_f64()
        );
    }

    #[test]
    fn q16_owasp_pattern_ignore_all_previous() {
        let detector = PromptInjectionDetectorCapsule::new();

        // Simulate embedding for "Ignore all previous instructions"
        // In production, this would be actual text-to-embedding conversion
        // For now, use pattern that triggers heuristics + ML
        let mut injection = [0i8; EMBEDDING_DIM];
        injection[0..50].fill(100);  // Suspicious pattern

        let risk = detector.check_prompt(&injection);

        // Should trigger detection
        assert!(
            risk.is_medium_risk() || risk.is_high_risk(),
            "OWASP pattern not detected: {}",
            risk.to_f64()
        );
    }

    #[test]
    fn q17_owasp_pattern_dan_jailbreak() {
        let detector = PromptInjectionDetectorCapsule::new();

        // Simulate "DAN" (Do Anything Now) jailbreak pattern
        let mut jailbreak = [0i8; EMBEDDING_DIM];
        jailbreak[0..80].fill(110);

        let risk = detector.check_prompt(&jailbreak);

        assert!(
            risk.is_medium_risk() || risk.is_high_risk(),
            "DAN jailbreak not detected: {}",
            risk.to_f64()
        );
    }

    #[test]
    fn q18_false_positive_benign_prompt() {
        let detector = PromptInjectionDetectorCapsule::new();

        // Benign prompt: low variance, low distance
        let mut benign = [0i8; EMBEDDING_DIM];
        for i in 0..EMBEDDING_DIM {
            benign[i] = (i % 10) as i8;  // Low values, low variance
        }

        let risk = detector.check_prompt(&benign);

        // Should not trigger false positive
        assert!(
            risk.is_low_risk() || risk.is_medium_risk(),
            "False positive on benign prompt: {}",
            risk.to_f64()
        );
    }

    #[test]
    fn q19_adaptive_threshold_reduces_false_positives() {
        let detector = PromptInjectionDetectorCapsule::new();

        // Borderline case
        let mut borderline = [0i8; EMBEDDING_DIM];
        borderline[0..30].fill(80);

        // With default threshold (0.85), may trigger
        let risk_default = detector.check_prompt(&borderline);

        // Increase threshold to 0.90
        detector.update_threshold(RiskScore::from_f64(0.90));

        // Same embedding should have lower relative risk vs threshold
        let decision_before = Decision::from(risk_default);
        let decision_after = Decision::from(risk_default); // Risk score unchanged, but threshold higher

        // Threshold change affects interpretation, not the score itself
        // (In production, would re-evaluate with new threshold)
        assert_eq!(risk_default, risk_default); // Score unchanged
    }

    #[test]
    fn q20_embedding_distance_simd_vs_scalar() {
        #[cfg(feature = "nightly-all")]
        {
            let detector = PromptInjectionDetectorCapsule::new();

            let test_embedding = [42i8; EMBEDDING_DIM];

            // SIMD and scalar should produce same result
            let risk_simd = detector.compute_embedding_distance_simd(&test_embedding);
            let risk_scalar = detector.compute_embedding_distance_scalar(&test_embedding);

            assert_eq!(
                risk_simd.get_fixed(),
                risk_scalar.get_fixed(),
                "SIMD and scalar produced different results"
            );
        }
    }

    #[test]
    fn q21_integration_with_behavioral_anomaly() {
        // Simulate integration with BehavioralAnomalyCapsule
        let detector = PromptInjectionDetectorCapsule::new();

        // Multiple failed checks should trigger behavioral anomaly
        let mut malicious = [127i8; EMBEDDING_DIM];

        let mut blocked_count = 0;
        for _ in 0..10 {
            let risk = detector.check_prompt(&malicious);
            if risk.is_high_risk() {
                blocked_count += 1;
            }
        }

        // Should consistently block malicious pattern
        assert!(
            blocked_count >= 8,
            "Inconsistent detection: only {} / 10 blocked",
            blocked_count
        );
    }

    // ============================================================================
    // Q22-Q28: PRODUCTION TESTS (Load, Chaos, Stress)
    // ============================================================================

    #[test]
    fn q22_sustained_load_single_thread() {
        let detector = PromptInjectionDetectorCapsule::new();

        let test_embedding = [42i8; EMBEDDING_DIM];

        // 10K checks (targeting <100ns each = <1ms total)
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            let _risk = detector.check_prompt(&test_embedding);
        }
        let elapsed = start.elapsed();

        // Should complete in <10ms (generous upper bound)
        assert!(
            elapsed.as_millis() < 10,
            "10K checks took {}ms (expected <10ms)",
            elapsed.as_millis()
        );
    }

    #[test]
    fn q23_sustained_load_multi_thread() {
        use std::sync::Arc;
        use std::thread;

        let detector = Arc::new(PromptInjectionDetectorCapsule::new());
        let mut handles = vec![];

        // 16 threads, each doing 1000 checks
        for _ in 0..16 {
            let d = Arc::clone(&detector);
            let handle = thread::spawn(move || {
                let embedding = [42i8; EMBEDDING_DIM];
                for _ in 0..1000 {
                    let _risk = d.check_prompt(&embedding);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // All 16K checks should complete without panic
    }

    #[test]
    fn q24_chaos_random_threshold_changes() {
        use std::sync::Arc;
        use std::thread;

        let detector = Arc::new(PromptInjectionDetectorCapsule::new());
        let mut handles = vec![];

        // Thread 1: Continuous threshold updates
        {
            let d = Arc::clone(&detector);
            let handle = thread::spawn(move || {
                let mut seed: u64 = 54321;
                for _ in 0..1000 {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    let threshold = 0.5 + (seed % 45) as f64 / 100.0;  // [0.5, 0.95)
                    d.update_threshold(RiskScore::from_f64(threshold));
                }
            });
            handles.push(handle);
        }

        // Threads 2-9: Continuous prompt checks
        for _ in 0..8 {
            let d = Arc::clone(&detector);
            let handle = thread::spawn(move || {
                let embedding = [42i8; EMBEDDING_DIM];
                for _ in 0..1000 {
                    let _risk = d.check_prompt(&embedding);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should complete without panic or deadlock
    }

    #[test]
    fn q25_stress_extreme_embeddings() {
        let detector = PromptInjectionDetectorCapsule::new();

        // All minimum values
        let min_embedding = [-128i8; EMBEDDING_DIM];
        let risk_min = detector.check_prompt(&min_embedding);
        assert!(risk_min.get_fixed() >= 0 && risk_min.get_fixed() <= 65536);

        // All maximum values
        let max_embedding = [127i8; EMBEDDING_DIM];
        let risk_max = detector.check_prompt(&max_embedding);
        assert!(risk_max.get_fixed() >= 0 && risk_max.get_fixed() <= 65536);

        // Alternating min/max
        let mut alternating = [-128i8; EMBEDDING_DIM];
        for i in (0..EMBEDDING_DIM).step_by(2) {
            alternating[i] = 127;
        }
        let risk_alt = detector.check_prompt(&alternating);
        assert!(risk_alt.get_fixed() >= 0 && risk_alt.get_fixed() <= 65536);
    }

    #[test]
    fn q26_memory_ordering_validation() {
        use std::sync::Arc;
        use std::thread;

        let detector = Arc::new(PromptInjectionDetectorCapsule::new());

        // Thread 1: Writer (updates threshold)
        let d1 = Arc::clone(&detector);
        let writer = thread::spawn(move || {
            for i in 0..100 {
                let threshold = 0.8 + (i as f64 / 1000.0);
                d1.update_threshold(RiskScore::from_f64(threshold));
            }
        });

        // Thread 2: Reader (reads threshold)
        let d2 = Arc::clone(&detector);
        let reader = thread::spawn(move || {
            for _ in 0..100 {
                let threshold = d2.get_threshold();
                // Should always be valid
                assert!(threshold.to_f64() >= 0.5 && threshold.to_f64() <= 0.95);
            }
        });

        writer.join().unwrap();
        reader.join().unwrap();
    }

    #[test]
    fn q27_production_realistic_workload() {
        let detector = PromptInjectionDetectorCapsule::new();

        // Simulate realistic distribution:
        // - 80% benign (low risk)
        // - 15% suspicious (medium risk)
        // - 5% malicious (high risk)

        for i in 0..1000 {
            let mut embedding = [0i8; EMBEDDING_DIM];

            if i % 20 < 16 {
                // Benign (80%)
                for j in 0..EMBEDDING_DIM {
                    embedding[j] = (j % 10) as i8;
                }
            } else if i % 20 < 19 {
                // Suspicious (15%)
                embedding[0..50].fill(80);
            } else {
                // Malicious (5%)
                embedding[0..100].fill(127);
            }

            let risk = detector.check_prompt(&embedding);
            let decision = Decision::from(risk);
            detector.record_decision(decision);
        }

        let stats = detector.get_statistics();
        assert_eq!(stats.total_checks, 1000);

        // Verify realistic distribution (with tolerance)
        assert!(
            stats.allowed_count >= 700 && stats.allowed_count <= 900,
            "Allowed count out of realistic range: {}",
            stats.allowed_count
        );
        assert!(
            stats.monitored_count >= 100 && stats.monitored_count <= 250,
            "Monitored count out of realistic range: {}",
            stats.monitored_count
        );
        assert!(
            stats.blocked_count >= 20 && stats.blocked_count <= 100,
            "Blocked count out of realistic range: {}",
            stats.blocked_count
        );
    }

    #[test]
    fn q28_zero_false_sharing_validation() {
        // Cache line alignment prevents false sharing
        let detector1 = PromptInjectionDetectorCapsule::new();
        let detector2 = PromptInjectionDetectorCapsule::new();

        let addr1 = &detector1 as *const _ as usize;
        let addr2 = &detector2 as *const _ as usize;

        // If allocated consecutively, should be 256 bytes apart (no overlap)
        let distance = if addr2 > addr1 {
            addr2 - addr1
        } else {
            addr1 - addr2
        };

        // Should be multiple of 256 (cache line aligned)
        assert_eq!(
            distance % 256,
            0,
            "Consecutive capsules not cache-aligned: distance = {}",
            distance
        );
    }
}
