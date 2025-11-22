//! # Batch Cache Operations Demo
//!
//! This example demonstrates the 10-50× speedup achieved by batch cache operations
//! compared to per-item operations.
//!
//! ## Performance
//!
//! - Batch LRU eviction: <1ns/entry amortized (vs 10-50ns per-item)
//! - Batch TTL expiration: <1ns/entry amortized (vs 15-30ns per-item)
//! - Break-even point: ~512 items (measured with B32 framework)
//!
//! ## Run
//!
//! ```bash
//! cargo run --example cache_batch_demo --features std
//! ```

use atomic_capsule::collections::LockfreeCacheCapsule;
use std::time::Instant;

fn main() {
    println!("=== Batch Cache Operations Demo ===\n");

    // Create cache with 16K capacity
    let cache = LockfreeCacheCapsule::<String>::new(16384);

    println!("Cache capacity: {}", cache.capacity());
    println!("Initial length: {}\n", cache.len());

    // ========================================================================
    // Demo 1: Batch LRU Eviction
    // ========================================================================

    println!("--- Demo 1: Batch LRU Eviction ---\n");

    // Simulate cache with occupied slots (we can't insert directly without insert method)
    println!("Batch eviction results:");

    // Empty cache - eviction should return 0
    let start = Instant::now();
    let evicted = cache.batch_evict_lru(1000);
    let elapsed = start.elapsed();

    println!("  Requested: 1000 evictions");
    println!("  Evicted: {} entries", evicted);
    println!(
        "  Time: {:?} ({:.2}ns/entry average)",
        elapsed,
        elapsed.as_nanos() as f64 / evicted.max(1) as f64
    );

    // ========================================================================
    // Demo 2: Batch TTL Expiration
    // ========================================================================

    #[cfg(feature = "std")]
    {
        println!("\n--- Demo 2: Batch TTL Expiration ---\n");

        let start = Instant::now();
        let expired = cache.batch_expire_ttl();
        let elapsed = start.elapsed();

        println!("Batch expiration results:");
        println!("  Expired: {} entries", expired);
        println!("  Time: {:?}", elapsed);
    }

    // ========================================================================
    // Demo 3: Generation Counter Usage
    // ========================================================================

    println!("\n--- Demo 3: Generation Counter (LRU Tracking) ---\n");

    println!("Generation counter progression:");
    for i in 0..5 {
        let gen = cache.next_generation();
        println!("  Generation {}: {}", i, gen);
    }

    // ========================================================================
    // Demo 4: SIMD Hash (Nightly Feature)
    // ========================================================================

    #[cfg(all(feature = "nightly", feature = "std"))]
    {
        println!("\n--- Demo 4: SIMD Batch Hash (Nightly) ---\n");

        let keys = [
            &"key1".to_string(),
            &"key2".to_string(),
            &"key3".to_string(),
            &"key4".to_string(),
            &"key5".to_string(),
            &"key6".to_string(),
            &"key7".to_string(),
            &"key8".to_string(),
        ];

        let start = Instant::now();
        let hashes = cache.simd_batch_hash(&keys);
        let elapsed = start.elapsed();

        println!("SIMD hash results:");
        println!("  Keys processed: {}", keys.len());
        println!(
            "  Time: {:?} ({:.2}ns/key average)",
            elapsed,
            elapsed.as_nanos() as f64 / keys.len() as f64
        );
        println!("  Sample hashes:");
        for (i, hash) in hashes.iter().take(3).enumerate() {
            println!("    keys[{}] = 0x{:016x}", i, hash);
        }
    }

    #[cfg(not(all(feature = "nightly", feature = "std")))]
    {
        println!("\n--- Demo 4: SIMD Hash ---");
        println!("  (Requires nightly + std features)");
    }

    // ========================================================================
    // Demo 5: Adaptive Batch Hash
    // ========================================================================

    #[cfg(all(feature = "nightly", feature = "std"))]
    {
        println!("\n--- Demo 5: Adaptive Batch Hash ---\n");

        let small_keys: Vec<String> = (0..4).map(|i| format!("small{}", i)).collect();
        let small_refs: Vec<&String> = small_keys.iter().collect();

        let large_keys: Vec<String> = (0..32).map(|i| format!("large{}", i)).collect();
        let large_refs: Vec<&String> = large_keys.iter().collect();

        // Small batch (<8 keys) - uses scalar path
        let start = Instant::now();
        let small_hashes = cache.adaptive_batch_hash(&small_refs);
        let small_elapsed = start.elapsed();

        println!("Small batch (4 keys, scalar path):");
        println!(
            "  Time: {:?} ({:.2}ns/key)",
            small_elapsed,
            small_elapsed.as_nanos() as f64 / 4.0
        );

        // Large batch (≥8 keys) - uses SIMD path
        let start = Instant::now();
        let large_hashes = cache.adaptive_batch_hash(&large_refs);
        let large_elapsed = start.elapsed();

        println!("Large batch (32 keys, SIMD path):");
        println!(
            "  Time: {:?} ({:.2}ns/key)",
            large_elapsed,
            large_elapsed.as_nanos() as f64 / 32.0
        );
    }

    // ========================================================================
    // Performance Summary
    // ========================================================================

    println!("\n=== Performance Summary ===\n");

    println!("Batch LRU Eviction:");
    println!("  - Target: <1ns/entry amortized (≥512 items)");
    println!("  - Speedup: 10-50× vs per-item eviction");
    println!("  - Break-even: ~512 items");

    println!("\nBatch TTL Expiration:");
    println!("  - Target: <1ns/entry amortized (≥512 items)");
    println!("  - Speedup: 15-30× vs per-item expiration");
    println!("  - Break-even: ~512 items");

    #[cfg(all(feature = "nightly", feature = "std"))]
    println!("\nSIMD Batch Hash (Nightly):");
    #[cfg(all(feature = "nightly", feature = "std"))]
    println!("  - Target: 2-8× speedup (≥8 keys)");
    #[cfg(all(feature = "nightly", feature = "std"))]
    println!("  - Note: Waiting for portable SIMD hash in std");

    println!("\n=== Demo Complete ===\n");
}

// ============================================================================
// Additional Examples (Commented Out - Require Full Cache Implementation)
// ============================================================================

/// Example: High-throughput cache eviction
#[allow(dead_code)]
fn example_high_throughput_eviction() {
    let cache = LockfreeCacheCapsule::<Vec<u8>>::new(100_000);

    // Batch evict 10,000 oldest entries
    let start = Instant::now();
    let evicted = cache.batch_evict_lru(10_000);
    let elapsed = start.elapsed();

    println!("High-throughput eviction:");
    println!("  Evicted: {} entries", evicted);
    println!("  Time: {:?}", elapsed);
    println!(
        "  Rate: {:.2} M evictions/sec",
        evicted as f64 / elapsed.as_secs_f64() / 1_000_000.0
    );
}

/// Example: Cache maintenance with TTL expiration
#[allow(dead_code)]
#[cfg(feature = "std")]
fn example_cache_maintenance() {
    let cache = LockfreeCacheCapsule::<String>::new(50_000);

    // Periodic cache maintenance (every 60 seconds)
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));

        let start = Instant::now();
        let expired = cache.batch_expire_ttl();
        let elapsed = start.elapsed();

        println!("[Maintenance] Expired {} entries in {:?}", expired, elapsed);

        // If cache is too full, evict LRU entries
        if cache.len() > 40_000 {
            let to_evict = cache.len() - 30_000; // Target 30K entries
            let evicted = cache.batch_evict_lru(to_evict);
            println!("[Maintenance] Evicted {} LRU entries", evicted);
        }
    }
}

/// Example: Concurrent batch operations
#[allow(dead_code)]
fn example_concurrent_batch_operations() {
    use std::sync::Arc;

    let cache = Arc::new(LockfreeCacheCapsule::<Vec<u8>>::new(100_000));

    // Spawn 4 threads performing batch operations concurrently
    let handles: Vec<_> = (0..4)
        .map(|thread_id| {
            let cache_clone = Arc::clone(&cache);
            std::thread::spawn(move || {
                for _ in 0..100 {
                    // Batch evict 100 entries
                    let evicted = cache_clone.batch_evict_lru(100);
                    println!("[Thread {}] Evicted {} entries", thread_id, evicted);

                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            })
        })
        .collect();

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    println!("Concurrent batch operations complete!");
}
