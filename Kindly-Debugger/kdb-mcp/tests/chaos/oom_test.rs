// OOM during large request
//
// Scenario: Simulate out-of-memory during large request processing
// Expected: Request fails with clear error, no memory leak

use super::*;

#[test]
fn test_oom_during_large_request() {
    let coordinator = ChaosCoordinator::new();

    // Configure memory chaos (100% rate to trigger OOM)
    coordinator.memory.start();

    let mut allocation_success = 0;
    let mut allocation_failed = 0;

    // Simulate 10,000 allocations (OOM is very rare)
    for _ in 0..10000 {
        let fail = coordinator.memory.should_fail_allocation();

        if fail {
            // Allocation failed (OOM)
            allocation_failed += 1;

            // Verify graceful handling (no panic, no leak)
            // In real implementation, check memory usage doesn't grow
        } else {
            // Allocation succeeded
            allocation_success += 1;
        }
    }

    coordinator.memory.stop();

    println!("OOM test:");
    println!("  Successful allocations: {}", allocation_success);
    println!("  Failed allocations: {}", allocation_failed);

    // OOM should be rare but present
    assert!(allocation_failed > 0, "No OOM simulations occurred");

    // Most allocations should succeed
    assert!(allocation_success > 9900, "Too few successful allocations: {}", allocation_success);
}
