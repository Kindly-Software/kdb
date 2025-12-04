// Clock skew during latency tracking
//
// Scenario: Simulate backwards clock during latency measurement
// Expected: Latency tracking handles negative durations gracefully

use super::*;
use std::time::Duration;

#[test]
fn test_clock_skew_during_latency_tracking() {
    let coordinator = ChaosCoordinator::new();

    // Configure clock chaos (50% rate)
    coordinator.clock.start();

    let mut valid_measurements = 0;
    let mut invalid_measurements = 0;

    // Simulate 1000 latency measurements
    for _ in 0..1000 {
        let start = coordinator.clock.now_with_chaos();
        std::thread::sleep(Duration::from_micros(100));
        let end = coordinator.clock.now_with_chaos();

        // Calculate duration (may be negative if clock went backwards)
        if end >= start {
            valid_measurements += 1;
        } else {
            invalid_measurements += 1;

            // Verify graceful handling (no panic, saturate to 0)
            let duration = end.saturating_sub(start);
            assert_eq!(duration, Duration::ZERO, "Negative duration not saturated");
        }
    }

    coordinator.clock.stop();

    println!("Clock skew test:");
    println!("  Valid measurements: {}", valid_measurements);
    println!("  Invalid measurements: {}", invalid_measurements);
    println!("  Total skew: {} ns", coordinator.clock.total_skew_ns());

    // Some measurements should be invalid (clock went backwards)
    assert!(invalid_measurements > 0, "No clock skew detected");

    // Most measurements should be valid
    assert!(valid_measurements > 900, "Too many invalid measurements: {}", invalid_measurements);
}
