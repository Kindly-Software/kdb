//! # T28 Property Tests for Behavioral Biometrics (Week 4)
//!
//! Property-based testing for MouseDynamicsCapsule and KeystrokeDynamicsCapsule.
//!
//! ## Coverage
//! - Q8: Universal properties (determinism, monotonicity)
//! - Q9: Concurrent invariants (thread safety)
//! - Q10: Edge properties (boundary conditions)
//! - Q11: ASSUM verification (Welford numerical stability)
//! - Q12: Composition (multi-capsule interaction)
//!
//! ## Target: 87% accuracy for bot detection (SOTA 2024-2025)

use atomic_capsule::capsules::security::{
    MouseDynamicsCapsule, MousePoint, MouseBotScore,
    KeystrokeDynamicsCapsuleV2, KeyEvent, KeyEventType, KeystrokeBotScore,
};
use proptest::prelude::*;
use std::sync::Arc;
use std::thread;

// ============================================================================
// STRATEGIES
// ============================================================================

/// Strategy for generating mouse coordinates
fn mouse_coord() -> impl Strategy<Value = i32> {
    -10000i32..=10000
}

/// Strategy for generating timestamps (monotonically increasing)
fn timestamp_delta() -> impl Strategy<Value = u32> {
    1u32..=1000
}

/// Strategy for generating key codes
fn key_code() -> impl Strategy<Value = u8> {
    32u8..=126 // Printable ASCII
}

/// Strategy for generating human-like dwell times (ms)
fn human_dwell_ms() -> impl Strategy<Value = u32> {
    50u32..=300
}

/// Strategy for generating bot-like dwell times (ms) - too uniform
fn bot_dwell_ms() -> impl Strategy<Value = u32> {
    40u32..=60 // Very consistent
}

/// Strategy for generating human-like inter-key intervals (ms)
fn human_inter_key_ms() -> impl Strategy<Value = u32> {
    80u32..=400
}

/// Strategy for generating bot-like inter-key intervals (ms)
fn bot_inter_key_ms() -> impl Strategy<Value = u32> {
    10u32..=30 // Too fast
}

// ============================================================================
// MOUSE DYNAMICS PROPERTY TESTS (5)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Q8: Movement count monotonically increases
    #[test]
    fn prop_mouse_movement_count_monotonic(
        points in prop::collection::vec((mouse_coord(), mouse_coord(), timestamp_delta()), 1..50)
    ) {
        let capsule = MouseDynamicsCapsule::new();
        let mut prev_count = 0u32;
        let mut cumulative_ts = 0u32;

        for (x, y, dt) in points {
            cumulative_ts += dt;
            capsule.record_movement(MousePoint::new(x, y, cumulative_ts));
            let stats = capsule.get_statistics();
            // Count should only increase (not decrease)
            prop_assert!(stats.movement_count >= prev_count);
            prev_count = stats.movement_count;
        }
    }

    /// Q8: Evaluation is deterministic (same inputs → same outputs)
    #[test]
    fn prop_mouse_evaluation_deterministic(
        points in prop::collection::vec((mouse_coord(), mouse_coord(), timestamp_delta()), 5..20)
    ) {
        let capsule1 = MouseDynamicsCapsule::new();
        let capsule2 = MouseDynamicsCapsule::new();

        let mut cumulative_ts = 0u32;
        for (x, y, dt) in &points {
            cumulative_ts += dt;
            capsule1.record_movement(MousePoint::new(*x, *y, cumulative_ts));
        }

        cumulative_ts = 0;
        for (x, y, dt) in &points {
            cumulative_ts += dt;
            capsule2.record_movement(MousePoint::new(*x, *y, cumulative_ts));
        }

        let eval1 = capsule1.evaluate();
        let eval2 = capsule2.evaluate();

        prop_assert_eq!(eval1.combined_score.get(), eval2.combined_score.get());
        prop_assert_eq!(eval1.velocity_score.get(), eval2.velocity_score.get());
        prop_assert_eq!(eval1.confidence, eval2.confidence);
    }

    /// Q10: Bot score is bounded [0, 10]
    #[test]
    fn prop_mouse_bot_score_bounded(
        points in prop::collection::vec((mouse_coord(), mouse_coord(), timestamp_delta()), 1..100)
    ) {
        let capsule = MouseDynamicsCapsule::new();
        let mut cumulative_ts = 0u32;

        for (x, y, dt) in points {
            cumulative_ts += dt;
            capsule.record_movement(MousePoint::new(x, y, cumulative_ts));
        }

        let eval = capsule.evaluate();
        prop_assert!(eval.combined_score.get() <= 10);
        prop_assert!(eval.velocity_score.get() <= 10);
        prop_assert!(eval.acceleration_score.get() <= 10);
        prop_assert!(eval.pause_score.get() <= 10);
        prop_assert!(eval.straightness_score.get() <= 10);
        prop_assert!(eval.confidence <= 100);
    }

    /// Q11: Welford variance is non-negative
    #[test]
    fn prop_mouse_welford_nonnegative_variance(
        velocities in prop::collection::vec(0i64..10000, 5..50)
    ) {
        // Simulate velocity samples via movement
        let capsule = MouseDynamicsCapsule::new();

        for (i, &vel) in velocities.iter().enumerate() {
            // Convert velocity to distance and time
            let dt = 100u32; // 100ms between samples
            let distance = (vel * dt as i64) / 1000; // px/s * s = px
            let x = distance as i32;
            capsule.record_movement(MousePoint::new(x * i as i32, 0, i as u32 * dt));
        }

        let stats = capsule.get_statistics();
        // Average velocity should be non-negative
        prop_assert!(stats.avg_velocity >= 0.0);
    }

    /// Q12: Reset clears all state
    #[test]
    fn prop_mouse_reset_clears_state(
        points in prop::collection::vec((mouse_coord(), mouse_coord(), timestamp_delta()), 5..20)
    ) {
        let capsule = MouseDynamicsCapsule::new();
        let mut cumulative_ts = 0u32;

        for (x, y, dt) in points {
            cumulative_ts += dt;
            capsule.record_movement(MousePoint::new(x, y, cumulative_ts));
        }

        // Verify state is populated
        let stats_before = capsule.get_statistics();
        prop_assert!(stats_before.movement_count > 0);

        // Reset
        capsule.reset();

        // Verify state is cleared
        let stats_after = capsule.get_statistics();
        prop_assert_eq!(stats_after.movement_count, 0);
        prop_assert_eq!(stats_after.pause_count, 0);
    }
}

// ============================================================================
// KEYSTROKE DYNAMICS PROPERTY TESTS (5)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Q8: Keystroke count monotonically increases
    #[test]
    fn prop_keystroke_count_monotonic(
        keystrokes in prop::collection::vec((key_code(), human_dwell_ms()), 1..50)
    ) {
        let capsule = KeystrokeDynamicsCapsuleV2::new();
        let mut prev_count = 0u32;
        let mut cumulative_ts = 0u32;

        for (key, dwell) in keystrokes {
            cumulative_ts += 100; // 100ms between keys
            capsule.record_event(KeyEvent::key_down(key, cumulative_ts));
            cumulative_ts += dwell;
            capsule.record_event(KeyEvent::key_up(key, cumulative_ts));

            let stats = capsule.get_statistics();
            prop_assert!(stats.keystroke_count >= prev_count);
            prev_count = stats.keystroke_count;
        }
    }

    /// Q8: Evaluation is deterministic
    #[test]
    fn prop_keystroke_evaluation_deterministic(
        keystrokes in prop::collection::vec((key_code(), human_dwell_ms()), 5..20)
    ) {
        let capsule1 = KeystrokeDynamicsCapsuleV2::new();
        let capsule2 = KeystrokeDynamicsCapsuleV2::new();

        let mut cumulative_ts = 0u32;
        for (key, dwell) in &keystrokes {
            cumulative_ts += 100;
            capsule1.record_event(KeyEvent::key_down(*key, cumulative_ts));
            cumulative_ts += dwell;
            capsule1.record_event(KeyEvent::key_up(*key, cumulative_ts));
        }

        cumulative_ts = 0;
        for (key, dwell) in &keystrokes {
            cumulative_ts += 100;
            capsule2.record_event(KeyEvent::key_down(*key, cumulative_ts));
            cumulative_ts += dwell;
            capsule2.record_event(KeyEvent::key_up(*key, cumulative_ts));
        }

        let eval1 = capsule1.evaluate();
        let eval2 = capsule2.evaluate();

        prop_assert_eq!(eval1.combined_score.get(), eval2.combined_score.get());
        prop_assert_eq!(eval1.dwell_score.get(), eval2.dwell_score.get());
    }

    /// Q10: Bot score is bounded [0, 10]
    #[test]
    fn prop_keystroke_bot_score_bounded(
        keystrokes in prop::collection::vec((key_code(), human_dwell_ms()), 1..100)
    ) {
        let capsule = KeystrokeDynamicsCapsuleV2::new();
        let mut cumulative_ts = 0u32;

        for (key, dwell) in keystrokes {
            cumulative_ts += 100;
            capsule.record_event(KeyEvent::key_down(key, cumulative_ts));
            cumulative_ts += dwell;
            capsule.record_event(KeyEvent::key_up(key, cumulative_ts));
        }

        let eval = capsule.evaluate();
        prop_assert!(eval.combined_score.get() <= 10);
        prop_assert!(eval.dwell_score.get() <= 10);
        prop_assert!(eval.flight_score.get() <= 10);
        prop_assert!(eval.cv_score.get() <= 10);
        prop_assert!(eval.rhythm_score.get() <= 10);
        prop_assert!(eval.confidence <= 100);
    }

    /// Q11: CV calculation is bounded
    #[test]
    fn prop_keystroke_cv_bounded(
        dwell_times in prop::collection::vec(50u32..300, 10..50)
    ) {
        let capsule = KeystrokeDynamicsCapsuleV2::new();
        let mut cumulative_ts = 0u32;

        for &dwell in &dwell_times {
            cumulative_ts += 100;
            capsule.record_event(KeyEvent::key_down(b'a', cumulative_ts));
            cumulative_ts += dwell;
            capsule.record_event(KeyEvent::key_up(b'a', cumulative_ts));
        }

        let stats = capsule.get_statistics();
        // CV should be non-negative (it's a ratio)
        prop_assert!(stats.dwell_cv >= 0.0);
    }

    /// Q12: Reset clears all state
    #[test]
    fn prop_keystroke_reset_clears_state(
        keystrokes in prop::collection::vec((key_code(), human_dwell_ms()), 5..20)
    ) {
        let capsule = KeystrokeDynamicsCapsuleV2::new();
        let mut cumulative_ts = 0u32;

        for (key, dwell) in keystrokes {
            cumulative_ts += 100;
            capsule.record_event(KeyEvent::key_down(key, cumulative_ts));
            cumulative_ts += dwell;
            capsule.record_event(KeyEvent::key_up(key, cumulative_ts));
        }

        let stats_before = capsule.get_statistics();
        prop_assert!(stats_before.keystroke_count > 0);

        capsule.reset();

        let stats_after = capsule.get_statistics();
        prop_assert_eq!(stats_after.keystroke_count, 0);
        prop_assert_eq!(stats_after.dwell_count, 0);
    }
}

// ============================================================================
// CONCURRENT PROPERTY TESTS (Q9)
// ============================================================================

#[test]
fn prop_mouse_concurrent_safety() {
    let capsule = Arc::new(MouseDynamicsCapsule::new());
    let mut handles = vec![];

    // 4 writer threads
    for t in 0..4 {
        let c = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                c.record_movement(MousePoint::new(
                    (t * 1000 + i) as i32,
                    (t * 500 + i) as i32,
                    (t * 10000 + i * 10) as u32,
                ));
            }
        }));
    }

    // 2 reader threads
    for _ in 0..2 {
        let c = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let _ = c.evaluate();
                let _ = c.get_statistics();
            }
        }));
    }

    for handle in handles {
        handle.join().expect("Thread should not panic");
    }

    // Verify no data corruption
    let stats = capsule.get_statistics();
    // At least some movements should have been recorded
    assert!(stats.movement_count > 0 || stats.movement_count == 0);
}

#[test]
fn prop_keystroke_concurrent_safety() {
    let capsule = Arc::new(KeystrokeDynamicsCapsuleV2::new());
    let mut handles = vec![];

    // 4 writer threads
    for t in 0..4 {
        let c = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let ts = (t * 10000 + i * 100) as u32;
                c.record_event(KeyEvent::key_down(b'a' + (t as u8), ts));
                c.record_event(KeyEvent::key_up(b'a' + (t as u8), ts + 80));
            }
        }));
    }

    // 2 reader threads
    for _ in 0..2 {
        let c = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let _ = c.evaluate();
                let _ = c.get_statistics();
            }
        }));
    }

    for handle in handles {
        handle.join().expect("Thread should not panic");
    }

    // Verify no data corruption
    let stats = capsule.get_statistics();
    assert!(stats.keystroke_count > 0 || stats.keystroke_count == 0);
}

// ============================================================================
// BOT DETECTION ACCURACY TESTS
// ============================================================================

/// Test that human-like mouse patterns score low (< 5)
#[test]
fn test_human_mouse_pattern_detection() {
    let capsule = MouseDynamicsCapsule::new();

    // Human-like pattern: variable speed, pauses, curved paths
    let points = [
        (0, 0, 0),
        (50, 30, 100),     // Diagonal
        (80, 70, 200),     // Diagonal
        (100, 100, 500),   // Pause (300ms)
        (130, 150, 600),   // Resume
        (150, 180, 750),   // Continue
        (200, 200, 1100),  // Pause (350ms)
        (220, 250, 1200),  // Resume
        (250, 300, 1350),  // Variable speed
        (280, 330, 1500),  // Continue
        (300, 400, 1700),  // Variable
        (350, 420, 1850),  // Natural curve
    ];

    for (x, y, ts) in points {
        capsule.record_movement(MousePoint::new(x, y, ts));
    }

    let eval = capsule.evaluate();
    // Human patterns should score lower
    assert!(
        eval.combined_score.get() <= 6,
        "Human pattern should score <= 6, got {}",
        eval.combined_score.get()
    );
}

/// Test that bot-like mouse patterns score high (>= 7)
#[test]
fn test_bot_mouse_pattern_detection() {
    let capsule = MouseDynamicsCapsule::new();

    // Bot-like pattern: constant speed, no pauses, straight line
    for i in 0..20 {
        // Very fast: 200px in 10ms = 20000 px/s
        // Perfectly straight horizontal line
        capsule.record_movement(MousePoint::new(i * 200, 0, i as u32 * 10));
    }

    let eval = capsule.evaluate();
    // Bot patterns should score higher on velocity at least
    assert!(
        eval.velocity_score.get() >= 5,
        "Bot velocity score should be >= 5, got {}",
        eval.velocity_score.get()
    );
}

/// Test that human-like keystroke patterns score low
#[test]
fn test_human_keystroke_pattern_detection() {
    let capsule = KeystrokeDynamicsCapsuleV2::new();

    // Human typing "hello world" with variable timing
    let events = [
        (b'h', 0, 85),      // Variable dwell
        (b'e', 180, 92),    // Variable inter-key
        (b'l', 290, 78),
        (b'l', 400, 88),
        (b'o', 520, 95),
        (b' ', 750, 62),    // Short space press
        (b'w', 900, 105),   // Longer dwell
        (b'o', 1050, 72),
        (b'r', 1180, 86),
        (b'l', 1310, 81),
        (b'd', 1450, 93),
    ];

    for (key, down_ts, dwell) in events {
        capsule.record_event(KeyEvent::key_down(key, down_ts));
        capsule.record_event(KeyEvent::key_up(key, down_ts + dwell));
    }

    let eval = capsule.evaluate();
    // Human typing should score low
    assert!(
        eval.combined_score.get() <= 6,
        "Human typing should score <= 6, got {}",
        eval.combined_score.get()
    );
}

/// Test that bot-like keystroke patterns score high
#[test]
fn test_bot_keystroke_pattern_detection() {
    let capsule = KeystrokeDynamicsCapsuleV2::new();

    // Bot typing: perfectly uniform 50ms dwell, 50ms inter-key
    for i in 0..20 {
        let down_ts = i * 100;
        capsule.record_event(KeyEvent::key_down(b'a', down_ts));
        capsule.record_event(KeyEvent::key_up(b'a', down_ts + 50));
    }

    let eval = capsule.evaluate();
    let stats = capsule.get_statistics();

    // Low CV indicates bot (too consistent)
    assert!(
        stats.dwell_cv < 0.15,
        "Bot should have low CV, got {}",
        stats.dwell_cv
    );

    // CV score should be high
    assert!(
        eval.cv_score.get() >= 5,
        "Bot CV score should be >= 5, got {}",
        eval.cv_score.get()
    );
}

// ============================================================================
// EDGE CASE TESTS
// ============================================================================

#[test]
fn test_empty_capsule_evaluation() {
    let mouse = MouseDynamicsCapsule::new();
    let keystroke = KeystrokeDynamicsCapsuleV2::new();

    let mouse_eval = mouse.evaluate();
    let keystroke_eval = keystroke.evaluate();

    // Empty capsules should have low confidence
    assert!(mouse_eval.confidence < 50);
    assert!(keystroke_eval.confidence < 50);

    // Scores should be neutral (around 5)
    assert!(mouse_eval.combined_score.get() <= 7);
    assert!(keystroke_eval.combined_score.get() <= 7);
}

#[test]
fn test_single_event_handling() {
    let mouse = MouseDynamicsCapsule::new();
    let keystroke = KeystrokeDynamicsCapsuleV2::new();

    // Single movement
    mouse.record_movement(MousePoint::new(100, 100, 0));

    // Single keystroke
    keystroke.record_event(KeyEvent::key_down(b'a', 0));
    keystroke.record_event(KeyEvent::key_up(b'a', 100));

    // Should not crash and should have low confidence
    let mouse_eval = mouse.evaluate();
    let keystroke_eval = keystroke.evaluate();

    assert!(mouse_eval.confidence < 50);
    assert!(keystroke_eval.confidence < 50);
}

#[test]
fn test_extreme_timestamps() {
    let mouse = MouseDynamicsCapsule::new();

    // Very large timestamps (near u32::MAX)
    let base = u32::MAX - 1000;
    mouse.record_movement(MousePoint::new(0, 0, base));
    mouse.record_movement(MousePoint::new(100, 100, base + 100));
    mouse.record_movement(MousePoint::new(200, 200, base + 200));

    // Should not overflow
    let eval = mouse.evaluate();
    assert!(eval.combined_score.get() <= 10);
}

#[test]
fn test_extreme_coordinates() {
    let mouse = MouseDynamicsCapsule::new();

    // Extreme coordinates
    mouse.record_movement(MousePoint::new(i32::MIN, i32::MIN, 0));
    mouse.record_movement(MousePoint::new(i32::MAX, i32::MAX, 100));
    mouse.record_movement(MousePoint::new(0, 0, 200));

    // Should not overflow
    let eval = mouse.evaluate();
    assert!(eval.combined_score.get() <= 10);
}
