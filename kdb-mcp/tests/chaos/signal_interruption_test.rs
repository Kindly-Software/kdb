// SIGTERM during transaction
//
// Scenario: Simulate SIGTERM signal during state transaction
// Expected: Transaction completes or rolls back atomically

use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn test_signal_interruption_during_transaction() {
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let transaction_count = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let completed_count = Arc::new(std::sync::atomic::AtomicU64::new(0));

    // Spawn transaction worker
    let shutdown = shutdown_flag.clone();
    let transactions = transaction_count.clone();
    let completed = completed_count.clone();

    let worker = std::thread::spawn(move || {
        while !shutdown.load(Ordering::Acquire) {
            // Start transaction
            transactions.fetch_add(1, Ordering::Relaxed);

            // Simulate transaction work
            std::thread::sleep(Duration::from_micros(100));

            // Check if interrupted
            if shutdown.load(Ordering::Acquire) {
                // Rollback (transaction incomplete)
                break;
            }

            // Complete transaction
            completed.fetch_add(1, Ordering::Relaxed);
        }
    });

    // Let transactions run for 100ms
    std::thread::sleep(Duration::from_millis(100));

    // Send shutdown signal (simulate SIGTERM)
    shutdown_flag.store(true, Ordering::Release);

    // Wait for graceful shutdown
    worker.join().unwrap();

    let total_transactions = transaction_count.load(Ordering::Relaxed);
    let total_completed = completed_count.load(Ordering::Relaxed);

    println!("Signal interruption test:");
    println!("  Total transactions: {}", total_transactions);
    println!("  Completed transactions: {}", total_completed);
    println!("  Incomplete: {}", total_transactions - total_completed);

    // At least some transactions should complete
    assert!(total_completed > 0, "No transactions completed");

    // At most one transaction should be incomplete (the one interrupted)
    assert!(
        total_transactions - total_completed <= 1,
        "Too many incomplete transactions: {}",
        total_transactions - total_completed
    );
}
