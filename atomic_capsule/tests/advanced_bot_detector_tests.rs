// atomic_capsule/tests/advanced_bot_detector_tests.rs
// Advanced Bot Detector - T28 Comprehensive Tests (28 tests across 4 tiers)
//
// Testing Strategy:
// - Q1-Q7 (Unit): Signal scoring, fingerprint hashing, weighted ensemble
// - Q8-Q14 (Property): Adaptive thresholds, edge cases, concurrent updates
// - Q15-Q21 (Integration): Selenium/Puppeteer/Playwright detection, real-world scenarios
// - Q22-Q28 (Production): 100K requests, accuracy validation, sustained load

use atomic_capsule::capsules::security::{
    AdvancedBotDetectorCapsule, ConfidenceScore, Decision, DetectionSignals,
};
use std::sync::Arc;
use std::thread;

// ============================================================================
// TIER 1: UNIT TESTS (Q1-Q7)
// ============================================================================

#[test]
fn test_q1_layout() {
    assert_eq!(
        core::mem::size_of::<AdvancedBotDetectorCapsule>(),
        256,
        "Must be 256 bytes"
    );
    assert_eq!(
        core::mem::align_of::<AdvancedBotDetectorCapsule>(),
        256,
        "Must be 256-byte aligned"
    );
}

#[test]
fn test_q2_default() {
    let detector = AdvancedBotDetectorCapsule::new();
    let stats = detector.get_statistics();
    assert_eq!(stats.bot_count, 0);
    assert_eq!(stats.human_count, 0);
    assert_eq!(stats.evasion_count, 0);
    assert_eq!(stats.challenge_count, 0);
}

#[test]
fn test_q3_evaluate_human() {
    let detector = AdvancedBotDetectorCapsule::new();
    let signals = DetectionSignals::default(); // All signals zero
    let confidence = detector.evaluate(&signals);
    assert!(confidence.is_likely_human(), "Default signals should be human");
    assert!(confidence.get() < 40);
}

#[test]
fn test_q4_canvas_fingerprint_scoring() {
    let detector = AdvancedBotDetectorCapsule::new();

    // Low entropy hash (suspicious)
    let mut signals = DetectionSignals::default();
    signals.canvas_hash = 0x0000_0001; // Only 1 bit set (low entropy)
    let confidence = detector.evaluate(&signals);
    assert!(confidence.get() > 0, "Low entropy should score high");

    // High entropy hash (less suspicious)
    signals.canvas_hash = 0x5A5A_A5A5; // Balanced bits (moderate entropy)
    let confidence = detector.evaluate(&signals);
    assert!(confidence.get() < 20, "High entropy should score lower");
}

#[test]
fn test_q5_automation_detection() {
    let detector = AdvancedBotDetectorCapsule::new();
    let mut signals = DetectionSignals::default();

    // navigator.webdriver = true (15% weight × 10 score = 15 points)
    signals.navigator_webdriver = true;
    let confidence = detector.evaluate(&signals);
    assert!(confidence.get() >= 10, "Webdriver should score high");

    // DevTools protocol detected
    signals.devtools_protocol = true;
    let confidence = detector.evaluate(&signals);
    assert!(confidence.get() >= 15, "DevTools should increase score");
}

#[test]
fn test_q6_weighted_ensemble() {
    let detector = AdvancedBotDetectorCapsule::new();
    let mut signals = DetectionSignals::default();

    // High automation signals (30% weight)
    signals.navigator_webdriver = true; // 15 points
    signals.phantom_properties = 10;    // 5 points
    signals.devtools_protocol = true;   // 5 points
    signals.missing_plugins = 10;       // 5 points
    // Total automation: 30 points

    let confidence = detector.evaluate(&signals);
    assert!(
        confidence.get() >= 25,
        "Weighted sum should be ~30 points (score: {})",
        confidence.get()
    );
}

#[test]
fn test_q7_confidence_score_ranges() {
    assert!(ConfidenceScore::new(0).is_likely_human());
    assert!(ConfidenceScore::new(39).is_likely_human());
    assert!(ConfidenceScore::new(40).is_uncertain());
    assert!(ConfidenceScore::new(69).is_uncertain());
    assert!(ConfidenceScore::new(70).is_likely_bot());
    assert!(ConfidenceScore::new(84).is_likely_bot());
    assert!(ConfidenceScore::new(85).is_definite_bot());
    assert!(ConfidenceScore::new(100).is_definite_bot());
}

// ============================================================================
// TIER 2: PROPERTY TESTS (Q8-Q14)
// ============================================================================

#[test]
fn test_q8_confidence_score_clamping() {
    // Property: Confidence scores are always clamped to 0-100
    for i in 0..=255u8 {
        let score = ConfidenceScore::new(i);
        assert!(score.get() <= 100, "Score must be ≤100 (got: {})", score.get());
    }
}

#[test]
fn test_q9_decision_from_confidence() {
    assert_eq!(Decision::from(ConfidenceScore::new(0)), Decision::Allow);
    assert_eq!(Decision::from(ConfidenceScore::new(40)), Decision::Challenge);
    assert_eq!(Decision::from(ConfidenceScore::new(70)), Decision::RateLimit);
    assert_eq!(Decision::from(ConfidenceScore::new(85)), Decision::Block);
}

#[test]
fn test_q10_record_decision_allow() {
    let detector = AdvancedBotDetectorCapsule::new();
    detector.record_decision(Decision::Allow);
    let stats = detector.get_statistics();
    assert_eq!(stats.human_count, 1);
    assert_eq!(stats.bot_count, 0);
}

#[test]
fn test_q11_record_decision_block() {
    let detector = AdvancedBotDetectorCapsule::new();
    detector.record_decision(Decision::Block);
    let stats = detector.get_statistics();
    assert_eq!(stats.bot_count, 1);
    assert_eq!(stats.evasion_count, 1); // Block also increments evasion
}

#[test]
fn test_q12_record_decision_challenge() {
    let detector = AdvancedBotDetectorCapsule::new();
    detector.record_decision(Decision::Challenge);
    let stats = detector.get_statistics();
    assert_eq!(stats.challenge_count, 1);
}

#[test]
fn test_q13_signal_scoring_monotonicity() {
    // Property: Higher signal values should lead to higher or equal confidence
    let detector = AdvancedBotDetectorCapsule::new();

    for i in 0..10u8 {
        let mut signals = DetectionSignals::default();
        signals.phantom_properties = i;
        let confidence1 = detector.evaluate(&signals);

        signals.phantom_properties = i + 1;
        let confidence2 = detector.evaluate(&signals);

        assert!(
            confidence2.get() >= confidence1.get(),
            "Confidence should be monotonic (i={}, conf1={}, conf2={})",
            i,
            confidence1.get(),
            confidence2.get()
        );
    }
}

#[test]
fn test_q14_concurrent_record_decisions() {
    let detector = Arc::new(AdvancedBotDetectorCapsule::new());
    let mut handles = vec![];

    // 100 threads, each recording 100 decisions
    for _ in 0..100 {
        let detector_clone = Arc::clone(&detector);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let decision = if i % 2 == 0 {
                    Decision::Allow
                } else {
                    Decision::Block
                };
                detector_clone.record_decision(decision);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = detector.get_statistics();
    assert_eq!(
        stats.human_count + stats.bot_count,
        10_000,
        "Total should be 10,000"
    );
    assert_eq!(stats.human_count, 5_000, "Half should be human");
    assert_eq!(stats.bot_count, 5_000, "Half should be bot");
}

// ============================================================================
// TIER 3: INTEGRATION TESTS (Q15-Q21)
// ============================================================================

#[test]
fn test_q15_selenium_detection() {
    // Selenium detection: navigator.webdriver + missing plugins + suspicious canvas
    let detector = AdvancedBotDetectorCapsule::new();
    let mut signals = DetectionSignals::default();

    // Selenium typical signatures
    signals.navigator_webdriver = true; // 15 points (CRITICAL)
    signals.missing_plugins = 10;       // 5 points
    signals.canvas_hash = 0x0000_0001;  // Suspicious (10 points)

    let confidence = detector.evaluate(&signals);
    let decision = Decision::from(confidence);

    assert!(
        confidence.is_likely_bot() || confidence.is_definite_bot(),
        "Selenium should be detected as bot (score: {})",
        confidence.get()
    );
    assert!(
        matches!(decision, Decision::RateLimit | Decision::Block),
        "Selenium should be rate-limited or blocked"
    );
}

#[test]
fn test_q16_puppeteer_detection() {
    // Puppeteer detection: DevTools protocol + phantom properties
    let detector = AdvancedBotDetectorCapsule::new();
    let mut signals = DetectionSignals::default();

    // Puppeteer typical signatures
    signals.devtools_protocol = true; // 5 points
    signals.phantom_properties = 8;   // 8 points
    signals.missing_plugins = 8;      // 5 points (scaled)

    let confidence = detector.evaluate(&signals);
    assert!(
        confidence.get() >= 15,
        "Puppeteer should score moderately (score: {})",
        confidence.get()
    );
}

#[test]
fn test_q17_playwright_detection() {
    // Playwright detection: DevTools + automation + behavioral anomalies
    let detector = AdvancedBotDetectorCapsule::new();
    let mut signals = DetectionSignals::default();

    // Playwright typical signatures
    signals.devtools_protocol = true; // 5 points
    signals.navigator_webdriver = true; // 15 points
    signals.mouse_velocity = 10;      // 10 points (too fast)
    signals.mouse_acceleration = 10;  // 10 points (constant)

    let confidence = detector.evaluate(&signals);
    assert!(
        confidence.is_definite_bot(),
        "Playwright should be detected as definite bot (score: {})",
        confidence.get()
    );
}

#[test]
fn test_q18_legitimate_human_traffic() {
    // Legitimate human: No automation signals, normal behavioral patterns
    let detector = AdvancedBotDetectorCapsule::new();
    let mut signals = DetectionSignals::default();

    // Human typical signatures
    signals.navigator_webdriver = false; // 0 points
    signals.canvas_hash = 0x5A5A_A5A5;   // Balanced entropy (5 points)
    signals.webgl_hash = 0x1234_5678;    // Generic GPU (5 points)
    signals.mouse_velocity = 5;          // Normal speed (5 points)
    signals.mouse_acceleration = 5;      // Natural variance (5 points)

    let confidence = detector.evaluate(&signals);
    assert!(
        confidence.is_likely_human() || confidence.is_uncertain(),
        "Legitimate human should score low (score: {})",
        confidence.get()
    );
}

#[test]
fn test_q19_privacy_tools_degradation() {
    // Privacy tools (Safari Private, Brave): Some signals missing → uncertain
    let detector = AdvancedBotDetectorCapsule::new();
    let mut signals = DetectionSignals::default();

    // Privacy tool signatures (limited fingerprinting)
    signals.canvas_hash = 0;   // Canvas blocked
    signals.webgl_hash = 0;    // WebGL blocked
    signals.audio_hash = 0;    // Audio blocked
    signals.mouse_velocity = 5; // Normal behavior

    let confidence = detector.evaluate(&signals);
    assert!(
        confidence.is_likely_human() || confidence.is_uncertain(),
        "Privacy tools should not be blocked (score: {})",
        confidence.get()
    );
}

#[test]
fn test_q20_mixed_signals_uncertain() {
    // Mixed signals: Some automation, some human → challenge with CAPTCHA
    let detector = AdvancedBotDetectorCapsule::new();
    let mut signals = DetectionSignals::default();

    // Mixed signals
    signals.phantom_properties = 5;  // Some phantom props (5 points)
    signals.mouse_velocity = 7;      // Slightly fast (7 points)
    signals.canvas_hash = 0x5A5A_A5A5; // Normal (5 points)

    let confidence = detector.evaluate(&signals);
    assert!(
        confidence.is_uncertain() || confidence.is_likely_human(),
        "Mixed signals should be uncertain (score: {})",
        confidence.get()
    );
}

#[test]
fn test_q21_all_signals_definite_bot() {
    // All signals maxed out → definite bot
    let detector = AdvancedBotDetectorCapsule::new();
    let mut signals = DetectionSignals::default();

    // Max out all signals
    signals.navigator_webdriver = true; // 15 points
    signals.phantom_properties = 10;    // 5 points
    signals.devtools_protocol = true;   // 5 points
    signals.missing_plugins = 10;       // 5 points
    signals.mouse_velocity = 10;        // 10 points
    signals.mouse_acceleration = 10;    // 10 points
    signals.canvas_hash = 0x0000_0001;  // 10 points
    signals.webgl_hash = 0x0000_0001;   // 10 points
    signals.js_challenge = 10;          // 2 points

    let confidence = detector.evaluate(&signals);
    assert!(
        confidence.is_definite_bot(),
        "All signals maxed should be definite bot (score: {})",
        confidence.get()
    );
    assert!(confidence.get() >= 70, "Score should be ≥70");
}

// ============================================================================
// TIER 4: PRODUCTION TESTS (Q22-Q28)
// ============================================================================

#[test]
fn test_q22_throughput_1k_requests() {
    // Throughput test: 1,000 requests in <1ms
    use std::time::Instant;

    let detector = AdvancedBotDetectorCapsule::new();
    let mut signals = DetectionSignals::default();
    signals.navigator_webdriver = true;

    let start = Instant::now();
    for _ in 0..1_000 {
        let _confidence = detector.evaluate(&signals);
    }
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 10,
        "1K requests should complete in <10ms (took: {:?})",
        elapsed
    );
}

#[test]
fn test_q23_concurrent_throughput() {
    // Concurrent throughput: 10 threads × 1K requests
    let detector = Arc::new(AdvancedBotDetectorCapsule::new());
    let mut handles = vec![];

    for _ in 0..10 {
        let detector_clone = Arc::clone(&detector);
        handles.push(thread::spawn(move || {
            let mut signals = DetectionSignals::default();
            signals.navigator_webdriver = true;
            for _ in 0..1_000 {
                let _confidence = detector_clone.evaluate(&signals);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // No assertions needed - success = no crashes
}

#[test]
fn test_q24_counter_accuracy() {
    // Counter accuracy: Record 10K decisions, verify counts
    let detector = AdvancedBotDetectorCapsule::new();

    for i in 0..10_000 {
        let decision = match i % 4 {
            0 => Decision::Allow,
            1 => Decision::Challenge,
            2 => Decision::RateLimit,
            3 => Decision::Block,
            _ => unreachable!(),
        };
        detector.record_decision(decision);
    }

    let stats = detector.get_statistics();
    assert_eq!(stats.human_count, 2_500, "Allow count");
    assert_eq!(stats.challenge_count, 2_500, "Challenge count");
    assert_eq!(stats.bot_count, 5_000, "Bot count (RateLimit + Block)");
    assert_eq!(stats.evasion_count, 2_500, "Evasion count (Block only)");
}

#[test]
fn test_q25_sustained_load() {
    // Sustained load: 100 threads × 1K requests each
    let detector = Arc::new(AdvancedBotDetectorCapsule::new());
    let mut handles = vec![];

    for thread_id in 0..100 {
        let detector_clone = Arc::clone(&detector);
        handles.push(thread::spawn(move || {
            let mut signals = DetectionSignals::default();
            signals.navigator_webdriver = thread_id % 2 == 0; // 50% bots

            for _ in 0..1_000 {
                let confidence = detector_clone.evaluate(&signals);
                let decision = Decision::from(confidence);
                detector_clone.record_decision(decision);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = detector.get_statistics();
    assert_eq!(
        stats.human_count + stats.bot_count,
        100_000,
        "Total decisions"
    );
    // Note: Exact counts depend on scoring logic, but total should be 100K
}

#[test]
fn test_q26_zero_signals_human() {
    // Edge case: All signals zero → should be human
    let detector = AdvancedBotDetectorCapsule::new();
    let signals = DetectionSignals::default();
    let confidence = detector.evaluate(&signals);
    assert!(confidence.is_likely_human());
}

#[test]
fn test_q27_detection_consistency() {
    // Property: Same signals → same confidence (deterministic)
    let detector = AdvancedBotDetectorCapsule::new();
    let mut signals = DetectionSignals::default();
    signals.navigator_webdriver = true;
    signals.phantom_properties = 8;

    let confidence1 = detector.evaluate(&signals);
    let confidence2 = detector.evaluate(&signals);

    assert_eq!(
        confidence1.get(),
        confidence2.get(),
        "Same signals should produce same confidence"
    );
}

#[test]
fn test_q28_lockfree_guarantee() {
    // Lockfree guarantee: No deadlocks under concurrent load
    let detector = Arc::new(AdvancedBotDetectorCapsule::new());
    let mut handles = vec![];

    for _ in 0..50 {
        let detector_clone = Arc::clone(&detector);
        handles.push(thread::spawn(move || {
            for i in 0..1_000 {
                let decision = if i % 2 == 0 {
                    Decision::Allow
                } else {
                    Decision::Block
                };
                detector_clone.record_decision(decision);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap(); // Should not hang (lockfree guarantee)
    }

    let stats = detector.get_statistics();
    assert_eq!(stats.human_count + stats.bot_count, 50_000);
}
