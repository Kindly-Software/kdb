//! Debug test for Arc<T> support
//!
//! NOTE: Requires `arc_support` feature - AtomicCapsuleMap currently requires V: Copy
//! To enable: `cargo test --features arc_support`

#![cfg(all(test, feature = "arc_support"))]

use atomic_capsule_map::AtomicCapsuleMap;
use std::sync::Arc;

#[test]
fn debug_arc_insert_get() {
    let map = AtomicCapsuleMap::<u64, Arc<String>>::with_capacity(16);

    println!("Creating Arc<String>");
    let data = Arc::new(String::from("Test"));
    let data_ptr = Arc::as_ptr(&data);
    println!("Arc pointer: {:p}", data_ptr);
    println!("Arc strong_count: {}", Arc::strong_count(&data));

    println!("\nInserting into map...");
    map.insert(1, data.clone()).unwrap();
    println!("After insert, strong_count: {}", Arc::strong_count(&data));

    println!("\nAttempting get...");
    match map.get(&1) {
        Some(retrieved) => {
            println!("Got value: {}", *retrieved);
            println!(
                "Retrieved Arc strong_count: {}",
                Arc::strong_count(&retrieved)
            );
        }
        None => {
            println!("get() returned None!");

            // Check if key exists
            if map.contains_key(&1) {
                println!("But contains_key() returns true!");
            } else {
                println!("contains_key() also returns false");
            }
        }
    }
}
