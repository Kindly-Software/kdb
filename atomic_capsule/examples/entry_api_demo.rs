//! # Entry API Demo - HashMap-Compatible Entry Pattern
//!
//! Demonstrates the Entry API for ConcurrentMapCapsule, providing:
//! - Atomic get-or-insert patterns (TOCTOU prevention)
//! - Composable and_modify chains
//! - Cache-like workflows
//! - Concurrent counter patterns
//!
//! **Run**: `cargo run --example entry_api_demo --features std`

use atomic_capsule::collections::{ConcurrentMapCapsule, Entry};
use std::sync::Arc;
use std::thread;

fn main() {
    println!("=== Entry API Demo ===\n");

    // Example 1: Basic or_insert
    example_basic_or_insert();

    // Example 2: and_modify chain
    example_and_modify_chain();

    // Example 3: Concurrent counter
    example_concurrent_counter();

    // Example 4: Cache pattern
    example_cache_pattern();

    // Example 5: Complex workflows
    example_complex_workflows();
}

/// Example 1: Basic or_insert
fn example_basic_or_insert() {
    println!("1. Basic or_insert:");

    let map: ConcurrentMapCapsule<u64, String> = ConcurrentMapCapsule::new();

    // First access: vacant, inserts default
    let value = map.entry(42).or_insert(String::from("default"));
    println!("   First or_insert(42): {}", value);

    // Second access: occupied, returns existing
    let value = map.entry(42).or_insert(String::from("ignored"));
    println!("   Second or_insert(42): {}", value);
    println!();
}

/// Example 2: and_modify chain
fn example_and_modify_chain() {
    println!("2. and_modify chain:");

    let map: ConcurrentMapCapsule<String, u64> = ConcurrentMapCapsule::new();

    // Increment counter pattern: modify if present, insert if absent
    for i in 0..5 {
        map.entry(String::from("counter"))
            .and_modify(|v| *v += 1)
            .or_insert(0);

        let current = map.get(&String::from("counter")).unwrap();
        println!("   Iteration {}: counter = {}", i + 1, current);
    }
    println!();
}

/// Example 3: Concurrent counter
fn example_concurrent_counter() {
    println!("3. Concurrent counter:");

    let map = Arc::new(ConcurrentMapCapsule::new());
    let mut handles = vec![];

    // 10 threads all increment the same counter
    for thread_id in 0..10 {
        let map_clone = Arc::clone(&map);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                map_clone
                    .entry(String::from("global_counter"))
                    .and_modify(|v| *v += 1)
                    .or_insert(0);
            }

            println!("   Thread {} complete", thread_id);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let final_count = map.get(&String::from("global_counter")).unwrap();
    println!(
        "   Final count (may be <1000 due to concurrent modifications): {}",
        final_count
    );
    println!();
}

/// Example 4: Cache pattern
fn example_cache_pattern() {
    println!("4. Cache pattern (get-or-compute):");

    let map: ConcurrentMapCapsule<u64, String> = ConcurrentMapCapsule::new();

    // Simulate expensive computation
    fn expensive_compute(key: u64) -> String {
        println!("   Computing value for key {}...", key);
        std::thread::sleep(std::time::Duration::from_millis(10));
        format!("computed_{}", key)
    }

    // First access: computes value
    let value = map.entry(42).or_insert_with(|| expensive_compute(42));
    println!("   First access: {}", value);

    // Second access: cached value (no computation)
    let value = map.entry(42).or_insert_with(|| {
        panic!("Should not compute again!");
    });
    println!("   Second access (cached): {}", value);
    println!();
}

/// Example 5: Complex workflows
fn example_complex_workflows() {
    println!("5. Complex workflows:");

    let map: ConcurrentMapCapsule<u64, Vec<u64>> = ConcurrentMapCapsule::new();

    // Build up vectors incrementally
    for i in 0..10 {
        if let Entry::Occupied(mut entry) = map.entry(42) {
            entry.get_mut().push(i);
        } else {
            map.entry(42).or_insert(vec![i]);
        }
    }

    let vec = map.get(&42).unwrap();
    println!("   Vector after 10 pushes: {:?}", vec);

    // Update-or-insert pattern
    map.entry(100)
        .and_modify(|v| v.push(999))
        .or_insert(vec![100]);
    println!("   Entry 100 (new): {:?}", map.get(&100).unwrap());

    map.entry(100).and_modify(|v| v.push(200)).or_insert(vec![]);
    println!("   Entry 100 (modified): {:?}", map.get(&100).unwrap());
    println!();
}
