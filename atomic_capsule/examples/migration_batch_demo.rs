//! Migration Batch Demo
//!
//! Demonstrates lockfree batch task migration between NUMA domains.

use atomic_capsule::parallel::{AdaptiveWorkQueue, MigrationBatch, MigrationStats};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn main() {
    println!("Migration Batch Demo - Tier 4 Batch Capsule\n");

    // Create source and target queues (simulating NUMA domains)
    let source_queue = AdaptiveWorkQueue::new(8);
    let target_queue = AdaptiveWorkQueue::new(8);

    // Create migration statistics
    let stats = MigrationStats::new();

    // Create migration batch (NUMA 0 → NUMA 1)
    let mut batch = MigrationBatch::new(0, 1);

    println!("Batch capacity: 64 tasks");
    println!("Source NUMA: {}", batch.source_numa());
    println!("Target NUMA: {}", batch.target_numa());
    println!("Initial count: {}\n", batch.count());

    // Add 32 tasks to batch
    let counter = Arc::new(AtomicUsize::new(0));
    println!("Adding 32 tasks to batch...");
    for i in 0..32 {
        let c = Arc::clone(&counter);
        let success = batch.add_task(Box::new(move || {
            c.fetch_add(i + 1, Ordering::Relaxed);
        }));
        assert!(success, "Failed to add task {}", i);
    }

    println!("Batch count after filling: {}", batch.count());
    println!("Batch full: {}", batch.is_full());
    println!("Batch empty: {}\n", batch.is_empty());

    // Execute migration
    println!("Executing migration...");
    let migrated = batch
        .execute(&source_queue, &target_queue)
        .expect("Migration failed");

    println!("Tasks migrated: {}", migrated);
    println!("Generation after migration: {}\n", batch.generation());

    // Record migration statistics
    stats.record_migration(32, migrated);

    // Execute all tasks in target queue
    println!("Executing tasks from target queue...");
    let mut executed = 0;
    while let Some(task) = target_queue.pop() {
        task();
        executed += 1;
    }

    println!("Tasks executed: {}", executed);
    println!(
        "Counter sum (1+2+...+32): {}\n",
        counter.load(Ordering::Relaxed)
    );

    // Display migration statistics
    println!("Migration Statistics:");
    println!("  Total migrations: {}", stats.total_migrations());
    println!("  Total tasks migrated: {}", stats.total_tasks_migrated());
    println!("  Failed migrations: {}", stats.failed_migrations());
    println!("  Partial migrations: {}", stats.partial_migrations());
    println!("  Success rate: {:.2}%", stats.success_rate() * 100.0);

    println!("\nDemo completed successfully!");
}
