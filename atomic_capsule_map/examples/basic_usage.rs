//! Basic usage examples for AtomicCapsuleMap.
//!
//! Run with: cargo run --example basic_usage

use atomic_capsule_map::AtomicCapsuleMap;

fn main() {
    println!("=== AtomicCapsuleMap Basic Usage ===\n");

    // Create a new map
    let map = AtomicCapsuleMap::new();
    println!("Created empty map");

    // Insert some values
    println!("\nInserting values...");
    map.insert("alice", 100);
    map.insert("bob", 200);
    map.insert("charlie", 300);
    println!("  alice: 100");
    println!("  bob: 200");
    println!("  charlie: 300");

    // Get values
    println!("\nRetrieving values...");
    if let Some(value) = map.get(&"alice") {
        println!("  alice = {}", value);
    }
    if let Some(value) = map.get(&"bob") {
        println!("  bob = {}", value);
    }

    // Update a value
    println!("\nUpdating bob's value...");
    map.insert("bob", 250);
    if let Some(value) = map.get(&"bob") {
        println!("  bob = {} (was 200)", value);
    }

    // Check existence
    println!("\nChecking existence...");
    println!("  contains 'alice': {}", map.contains_key(&"alice"));
    println!("  contains 'missing': {}", map.contains_key(&"missing"));

    // Remove a value
    println!("\nRemoving charlie...");
    if let Some(value) = map.remove(&"charlie") {
        println!("  removed charlie = {}", value);
    }
    println!("  contains 'charlie': {}", map.contains_key(&"charlie"));

    // Iterate over all entries
    println!("\nAll entries:");
    for (key, value) in map.iter() {
        println!("  {} = {}", key, value);
    }

    // Map statistics
    println!("\nMap statistics:");
    println!("  length: {}", map.len());
    println!("  is_empty: {}", map.is_empty());

    // Clear the map
    println!("\nClearing map...");
    map.clear();
    println!("  length after clear: {}", map.len());
    println!("  is_empty: {}", map.is_empty());

    println!("\n=== Basic Usage Complete ===");
}
