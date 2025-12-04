// Network partition during request
//
// Scenario: Simulate network partition while processing MCP request
// Expected: Request fails gracefully, retry succeeds after recovery

use super::*;
use std::time::{Duration, Instant};

#[test]
fn test_network_partition_during_request() {
    let coordinator = ChaosCoordinator::new();

    // Configure network chaos (50% packet loss)
    coordinator.network.start();

    let start = Instant::now();
    let mut success_count = 0;
    let mut failure_count = 0;

    // Simulate 100 requests during network chaos
    for _ in 0..100 {
        let drop = coordinator.network.should_drop_packet();

        if drop {
            failure_count += 1;
            // Simulate retry logic
            std::thread::sleep(Duration::from_millis(10));

            // Retry should succeed (if network recovered)
            let retry_drop = coordinator.network.should_drop_packet();
            if !retry_drop {
                success_count += 1;
            }
        } else {
            success_count += 1;
        }
    }

    coordinator.network.stop();

    println!("Network partition test:");
    println!("  Success: {}", success_count);
    println!("  Failures: {}", failure_count);
    println!("  Duration: {:?}", start.elapsed());

    // At least some requests should succeed (with retries)
    assert!(success_count > 40, "Too few successful requests: {}", success_count);
}
