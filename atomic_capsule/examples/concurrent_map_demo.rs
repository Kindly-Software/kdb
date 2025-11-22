//! # ConcurrentMapCapsule Demo - Performance Validation
//!
//! Quick performance test to validate basic operation latency.

use atomic_capsule::ConcurrentMapCapsule;
use std::time::Instant;

fn main() {
    println!("=== ConcurrentMapCapsule Performance Demo ===\n");

    // Test 1: Single-threaded insert
    {
        let map: ConcurrentMapCapsule<u64, u64> = ConcurrentMapCapsule::new();
        let start = Instant::now();
        for i in 0..10000 {
            map.insert(i, i * 10);
        }
        let elapsed = start.elapsed();
        println!(
            "Insert 10K entries: {:?} ({:.1} ns/op)",
            elapsed,
            elapsed.as_nanos() as f64 / 10000.0
        );
    }

    // Test 2: Single-threaded get
    {
        let map: ConcurrentMapCapsule<u64, u64> = ConcurrentMapCapsule::new();
        for i in 0..10000 {
            map.insert(i, i * 10);
        }

        let start = Instant::now();
        for i in 0..10000 {
            let _ = map.get(&i);
        }
        let elapsed = start.elapsed();
        println!(
            "Get 10K entries: {:?} ({:.1} ns/op)",
            elapsed,
            elapsed.as_nanos() as f64 / 10000.0
        );
    }

    // Test 3: Single-threaded remove
    {
        let map: ConcurrentMapCapsule<u64, u64> = ConcurrentMapCapsule::new();
        for i in 0..10000 {
            map.insert(i, i * 10);
        }

        let start = Instant::now();
        for i in 0..10000 {
            let _ = map.remove(&i);
        }
        let elapsed = start.elapsed();
        println!(
            "Remove 10K entries: {:?} ({:.1} ns/op)",
            elapsed,
            elapsed.as_nanos() as f64 / 10000.0
        );
    }

    // Test 4: Concurrent insert (8 threads)
    {
        use std::sync::Arc;
        use std::thread;

        // 131K capacity for 80K entries (61% load factor)
        let map = Arc::new(ConcurrentMapCapsule::<u64, u64>::with_capacity(131072));
        let start = Instant::now();

        let handles: Vec<_> = (0..8)
            .map(|t| {
                let map_clone = Arc::clone(&map);
                thread::spawn(move || {
                    for i in 0..10000 {
                        let key = (t * 10000) + i;
                        map_clone.insert(key, key * 10);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let elapsed = start.elapsed();
        let total_ops = 80000;
        println!(
            "\nConcurrent insert (8 threads × 10K): {:?} ({:.1} ns/op, {:.1} Mops/s)",
            elapsed,
            elapsed.as_nanos() as f64 / total_ops as f64,
            total_ops as f64 / elapsed.as_secs_f64() / 1_000_000.0
        );
    }

    // Test 5: Concurrent get (8 threads, read-heavy)
    {
        use std::sync::Arc;
        use std::thread;

        let map = Arc::new(ConcurrentMapCapsule::<u64, u64>::new());
        for i in 0..10000 {
            map.insert(i, i * 10);
        }

        let start = Instant::now();

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let map_clone = Arc::clone(&map);
                thread::spawn(move || {
                    for _ in 0..10000 {
                        for i in 0..10 {
                            let _ = map_clone.get(&i);
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let elapsed = start.elapsed();
        let total_ops = 800000; // 8 threads × 10K iterations × 10 reads
        println!(
            "Concurrent get (8 threads, read-heavy): {:?} ({:.1} ns/op, {:.1} Mops/s)",
            elapsed,
            elapsed.as_nanos() as f64 / total_ops as f64,
            total_ops as f64 / elapsed.as_secs_f64() / 1_000_000.0
        );
    }

    println!("\n=== All tests completed successfully ===");
}
