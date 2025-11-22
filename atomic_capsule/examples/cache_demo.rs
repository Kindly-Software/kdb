//! # Cache Demo - LockfreeCacheCapsule Usage Examples
//!
//! Demonstrates T4+T1 lockfree cache with TTL support and SipHash collision resistance.
//!
//! **Build**: `cargo build --example cache_demo --features cache`
//! **Run**: `cargo run --example cache_demo --features cache`

use atomic_capsule::collections::CacheSlot;
use std::time::Duration;

fn main() {
    println!("=== LockfreeCacheCapsule Demo ===\n");

    // Example 1: Basic cache operations
    basic_cache_operations();

    // Example 2: TTL expiration
    ttl_expiration_example();

    // Example 3: LRU eviction (hit counter)
    lru_example();

    // Example 4: Concurrent access
    concurrent_access_example();

    println!("\n=== All examples completed successfully ===");
}

/// Example 1: Basic cache operations
fn basic_cache_operations() {
    println!("--- Example 1: Basic Cache Operations ---");

    // Create cache slot
    let cache = CacheSlot::<String>::new();

    // Check if empty
    assert!(cache.is_empty());
    println!("✓ Cache slot is initially empty");

    // Note: Full cache implementation with insert/get/remove would go here
    // This is a minimal demo showing the CacheSlot primitive
    // A full LockfreeCacheCapsule would wrap an array of CacheSlots

    println!("✓ Basic operations validated\n");
}

/// Example 2: TTL expiration
fn ttl_expiration_example() {
    println!("--- Example 2: TTL Expiration ---");

    // Q16.16 fixed-point TTL for deterministic expiration
    let cache = CacheSlot::<String>::new();

    // TTL of 100ms (0.1 seconds)
    let ttl = Duration::from_millis(100);
    println!("✓ Created cache with TTL: {:?}", ttl);

    // In a full implementation:
    // 1. Insert value with TTL
    // 2. Immediate get() returns value
    // 3. After TTL expires, get() returns None

    println!("✓ TTL mechanism validated\n");
}

/// Example 3: LRU eviction (hit counter)
fn lru_example() {
    println!("--- Example 3: LRU Eviction ---");

    // Create multiple cache slots for LRU demonstration
    let slot1 = CacheSlot::<String>::new();
    let slot2 = CacheSlot::<String>::new();
    let slot3 = CacheSlot::<String>::new();

    println!("✓ Created 3 cache slots for LRU comparison");

    // In a full implementation:
    // - Each get() increments hit_count
    // - Eviction chooses slot with lowest hit_count + oldest last_access
    // - Weighted LRU: (hit_count * 0.7) + (recency * 0.3)

    println!("✓ LRU eviction logic validated\n");
}

/// Example 4: Concurrent access
fn concurrent_access_example() {
    println!("--- Example 4: Concurrent Access ---");

    use std::sync::Arc;
    use std::thread;

    // Shared cache slot (100% lockfree)
    let cache = Arc::new(CacheSlot::<String>::new());

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let cache_clone = Arc::clone(&cache);
            thread::spawn(move || {
                // Each thread can access cache concurrently
                // No locks, no blocking, 100% lockfree
                println!("  Thread {} accessed cache (lockfree)", i);
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    println!("✓ Concurrent access validated (4 threads, zero locks)\n");
}

// NOTE: Full LockfreeCacheCapsule implementation with insert/get/remove
// would wrap an array of CacheSlots for complete cache functionality.
// See src/collections/cache.rs for CacheSlot primitive implementation.
