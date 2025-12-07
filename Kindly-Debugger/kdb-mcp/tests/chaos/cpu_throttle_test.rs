// CPU throttle during stress test
//
// Scenario: Simulate CPU throttling during high load
// Expected: Latency increases but no failures

use super::*;
use std::time::{Duration, Instant};

#[test]
fn test_cpu_throttle_during_stress() {
    let coordinator = ChaosCoordinator::new();

    // Configure CPU chaos (30% throttle rate)
    coordinator.cpu.start();

    let start = Instant::now();
    let mut total_latency = Duration::ZERO;
    let iterations = 100;

    // Simulate 100 operations with CPU throttling
    for _ in 0..iterations {
        let op_start = Instant::now();

        // Simulate operation
        coordinator.cpu.maybe_throttle();
        std::thread::sleep(Duration::from_micros(100));

        let op_latency = op_start.elapsed();
        total_latency += op_latency;
    }

    coordinator.cpu.stop();

    let elapsed = start.elapsed();
    let avg_latency = total_latency / iterations;

    println!("CPU throttle test:");
    println!("  Total time: {:?}", elapsed);
    println!("  Avg latency: {:?}", avg_latency);
    println!("  Throttle events: {}", coordinator.cpu.stats());

    // Should take longer than baseline (100 × 100μs = 10ms)
    assert!(elapsed > Duration::from_millis(10), "Elapsed: {:?}", elapsed);

    // Some throttle events should occur
    assert!(coordinator.cpu.stats() > 10, "Throttle events: {}", coordinator.cpu.stats());
}
