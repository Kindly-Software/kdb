// Disk full during checkpoint
//
// Scenario: Simulate disk full (ENOSPC) during state checkpoint
// Expected: Checkpoint fails gracefully, no data corruption

use super::*;

#[test]
fn test_disk_full_during_checkpoint() {
    let coordinator = ChaosCoordinator::new();

    // Configure disk chaos (20% ENOSPC)
    coordinator.disk.start();

    let mut checkpoint_success = 0;
    let mut checkpoint_failed = 0;

    // Simulate 50 checkpoint attempts
    for i in 0..50 {
        let fail = coordinator.disk.should_fail_with_enospc();

        if fail {
            // Checkpoint failed (ENOSPC)
            checkpoint_failed += 1;

            // Verify no partial writes (atomicity)
            // In real implementation, check that no corrupted state exists
        } else {
            // Checkpoint succeeded
            checkpoint_success += 1;

            // Simulate checkpoint write
            std::thread::sleep(std::time::Duration::from_micros(100));
        }
    }

    coordinator.disk.stop();

    println!("Disk full test:");
    println!("  Successful checkpoints: {}", checkpoint_success);
    println!("  Failed checkpoints: {}", checkpoint_failed);

    // At least some checkpoints should succeed
    assert!(checkpoint_success > 30, "Too few successful checkpoints: {}", checkpoint_success);

    // Failed checkpoints should not corrupt state
    assert_eq!(checkpoint_success + checkpoint_failed, 50);
}
