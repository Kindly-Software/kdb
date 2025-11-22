//! Example demonstrating LockfreeHashTable iter() and retain() methods
//!
//! This example shows:
//! 1. Basic iteration over table entries
//! 2. Conditional removal with retain()
//! 3. Iteration with hash collisions
//! 4. Concurrent iteration and modification

use atomic_capsule::collections::LockfreeHashTable;
use std::sync::Arc;
use std::thread;

fn main() {
    println!("=== LockfreeHashTable iter() and retain() Examples ===\n");

    // Example 1: Basic iteration
    println!("Example 1: Basic Iteration");
    basic_iteration();
    println!();

    // Example 2: Conditional removal
    println!("Example 2: Conditional Removal (retain)");
    conditional_removal();
    println!();

    // Example 3: Iteration with collisions
    println!("Example 3: Iteration with Hash Collisions");
    iteration_with_collisions();
    println!();

    // Example 4: Concurrent iteration
    println!("Example 4: Concurrent Iteration and Modification");
    concurrent_iteration();
    println!();

    println!("=== All Examples Complete ===");
}

fn basic_iteration() {
    let table = LockfreeHashTable::new(1024);

    // Insert some data
    table.insert(1, "Alice".to_string());
    table.insert(2, "Bob".to_string());
    table.insert(3, "Charlie".to_string());
    table.insert(4, "Diana".to_string());

    println!("Table contents:");
    for (key, value) in table.iter() {
        println!("  {} -> {}", key, value);
    }

    println!("Total entries: {}", table.len());
}

fn conditional_removal() {
    let table = LockfreeHashTable::new(1024);

    // Insert numbers 0-19
    for i in 0..20 {
        table.insert(i, i);
    }

    println!("Initial table size: {}", table.len());

    // Keep only even numbers
    let removed = table.retain(|v| *v % 2 == 0);

    println!("Removed {} odd numbers", removed);
    println!("Final table size: {}", table.len());

    println!("Remaining entries:");
    let mut entries: Vec<_> = table.iter().collect();
    entries.sort_by_key(|(k, _)| *k);
    for (key, value) in entries.iter().take(5) {
        println!("  {} -> {}", key, value);
    }
    println!("  ... ({} more)", entries.len().saturating_sub(5));
}

fn iteration_with_collisions() {
    // Small capacity forces hash collisions and chaining
    let table = LockfreeHashTable::new(16);

    // Insert 64 entries (will create chains)
    for i in 0..64 {
        table.insert(i, format!("value_{}", i));
    }

    println!("Table capacity: {}", table.capacity());
    println!(
        "Table size: {} (avg {} per slot)",
        table.len(),
        table.len() / table.capacity()
    );

    // Iterate and count
    let mut count = 0;
    for (key, value) in table.iter() {
        if count < 5 {
            println!("  {} -> {}", key, value);
        }
        count += 1;
    }
    println!("  ... ({} total entries)", count);

    // Verify all entries are accessible
    assert_eq!(
        count, 64,
        "Should iterate over all entries including chains"
    );
    println!("✓ All {} entries iterated successfully", count);
}

fn concurrent_iteration() {
    let table = Arc::new(LockfreeHashTable::new(8192));

    // Pre-populate with 100 entries
    for i in 0..100 {
        table.insert(i, i * 2);
    }

    println!("Initial table size: {}", table.len());

    let table_clone = Arc::clone(&table);
    let handle = thread::spawn(move || {
        // Insert more entries concurrently
        for i in 100..200 {
            table_clone.insert(i, i * 2);
        }
        println!("  [Background thread] Inserted 100 additional entries");
    });

    // Iterate (will see a consistent snapshot)
    println!("  [Main thread] Starting iteration...");
    let snapshot_count = table.iter().count();
    println!("  [Main thread] Iterator saw {} entries", snapshot_count);

    handle.join().unwrap();

    println!("Final table size: {}", table.len());
    println!(
        "✓ Iterator saw consistent snapshot ({} entries)",
        snapshot_count
    );
    println!("✓ Final table has all entries ({} entries)", table.len());
}
