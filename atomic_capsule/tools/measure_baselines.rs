//! Baseline Performance Measurement Tool
//!
//! Measures actual performance to generate realistic regression test baselines.
//! Run with: cargo run --release --bin measure_baselines

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::thread;
use std::time::Instant;

fn measure_atomic_operations() {
    println!("\n=== Atomic Operations ===");

    // Atomic load
    {
        let value = AtomicU64::new(42);
        let iterations = 100_000;
        let start = Instant::now();
        for _ in 0..iterations {
            std::hint::black_box(value.load(Ordering::Relaxed));
        }
        let avg_ns = start.elapsed().as_nanos() / iterations as u128;
        println!("load_ns = {}", avg_ns);
    }

    // Atomic store
    {
        let value = AtomicU64::new(0);
        let iterations = 100_000;
        let start = Instant::now();
        for i in 0..iterations {
            value.store(i as u64, Ordering::Relaxed);
            std::hint::black_box(());
        }
        let avg_ns = start.elapsed().as_nanos() / iterations as u128;
        println!("store_ns = {}", avg_ns);
    }

    // Atomic CAS
    {
        let value = AtomicU64::new(0);
        let iterations = 10_000;
        let start = Instant::now();
        for i in 0..iterations {
            value
                .compare_exchange(
                    i as u64,
                    (i + 1) as u64,
                    Ordering::Release,
                    Ordering::Relaxed,
                )
                .ok();
        }
        let avg_ns = start.elapsed().as_nanos() / iterations as u128;
        println!("cas_ns = {}", avg_ns);
    }
}

#[cfg(feature = "std")]
fn measure_concurrent_map() {
    use atomic_capsule::collections::ConcurrentMapCapsule;

    println!("\n=== ConcurrentMapCapsule ===");

    // Insert
    {
        let map: ConcurrentMapCapsule<u64, u64> = ConcurrentMapCapsule::new();
        let iterations = 10_000;
        let start = Instant::now();
        for i in 0..iterations {
            let _ = std::hint::black_box(map.insert(i, i * 10));
        }
        let avg_ns = start.elapsed().as_nanos() / iterations as u128;
        println!("insert_ns = {}", avg_ns);
    }

    // Get
    {
        let map: ConcurrentMapCapsule<u64, u64> = ConcurrentMapCapsule::new();
        for i in 0..10_000 {
            let _ = map.insert(i, i * 10);
        }
        let iterations = 10_000;
        let start = Instant::now();
        for i in 0..iterations {
            let _ = std::hint::black_box(map.get(&i));
        }
        let avg_ns = start.elapsed().as_nanos() / iterations as u128;
        println!("get_ns = {}", avg_ns);
    }

    // Remove
    {
        let iterations = 10_000;
        let runs = 10;
        let mut total_ns = 0u128;

        for _ in 0..runs {
            let map: ConcurrentMapCapsule<u64, u64> = ConcurrentMapCapsule::new();
            for i in 0..iterations {
                let _ = map.insert(i, i * 10);
            }

            let start = Instant::now();
            for i in 0..iterations {
                let _ = std::hint::black_box(map.remove(&i));
            }
            total_ns += start.elapsed().as_nanos();
        }

        let avg_ns = (total_ns / runs as u128) / iterations as u128;
        println!("remove_ns = {}", avg_ns);
    }
}

#[cfg(feature = "std")]
fn measure_lockfree_table() {
    use atomic_capsule::LockfreeHashTable;

    println!("\n=== LockfreeHashTable ===");

    // Insert
    {
        let table = Arc::new(LockfreeHashTable::new(16384));
        let iterations = 10_000;
        let start = Instant::now();
        for i in 0..iterations {
            let _ = std::hint::black_box(table.insert(i as u64, i));
        }
        let avg_ns = start.elapsed().as_nanos() / iterations as u128;
        println!("insert_ns = {}", avg_ns);
    }

    // Get
    {
        let table = Arc::new(LockfreeHashTable::new(16384));
        for i in 0..10_000 {
            let _ = table.insert(i as u64, i);
        }
        let iterations = 10_000;
        let start = Instant::now();
        for i in 0..iterations {
            let _ = std::hint::black_box(table.get(&(i as u64)));
        }
        let avg_ns = start.elapsed().as_nanos() / iterations as u128;
        println!("get_ns = {}", avg_ns);
    }
}

#[cfg(feature = "std")]
fn measure_concurrent_operations() {
    use atomic_capsule::collections::ConcurrentMapCapsule;

    println!("\n=== Concurrent Operations (8 threads) ===");

    // Concurrent insert
    {
        let num_threads = 8;
        let per_thread = 1_000;
        let total_ops = num_threads * per_thread;

        let start = Instant::now();
        let map = Arc::new(ConcurrentMapCapsule::<u64, u64>::new());
        let mut handles = vec![];

        for t in 0..num_threads {
            let map_clone = Arc::clone(&map);
            handles.push(thread::spawn(move || {
                for i in 0..per_thread {
                    let key = (t * per_thread) + i;
                    let _ = map_clone.insert(key, key * 10);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let avg_ns = start.elapsed().as_nanos() / total_ops as u128;
        println!("insert_per_op_ns = {}", avg_ns);
    }

    // Concurrent get
    {
        let num_threads = 8;
        let per_thread = 1_000;
        let total_ops = num_threads * per_thread;

        let map = Arc::new({
            let m: ConcurrentMapCapsule<u64, u64> = ConcurrentMapCapsule::new();
            for i in 0..total_ops {
                let _ = m.insert(i, i * 10);
            }
            m
        });

        let start = Instant::now();
        let mut handles = vec![];

        for _ in 0..num_threads {
            let map_clone = Arc::clone(&map);
            handles.push(thread::spawn(move || {
                for i in 0..per_thread {
                    let _ = std::hint::black_box(map_clone.get(&i));
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let avg_ns = start.elapsed().as_nanos() / total_ops as u128;
        println!("get_per_op_ns = {}", avg_ns);
    }

    // Concurrent remove
    {
        let num_threads = 8;
        let per_thread = 1_000;
        let total_ops = num_threads * per_thread;

        let map = Arc::new({
            let m: ConcurrentMapCapsule<u64, u64> = ConcurrentMapCapsule::new();
            for t in 0..num_threads {
                for i in 0..per_thread {
                    let key = (t * per_thread) + i;
                    let _ = m.insert(key, key * 10);
                }
            }
            m
        });

        let start = Instant::now();
        let mut handles = vec![];

        for t in 0..num_threads {
            let map_clone = Arc::clone(&map);
            handles.push(thread::spawn(move || {
                for i in 0..per_thread {
                    let key = (t * per_thread) + i;
                    let _ = std::hint::black_box(map_clone.remove(&key));
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let avg_ns = start.elapsed().as_nanos() / total_ops as u128;
        println!("remove_per_op_ns = {}", avg_ns);
    }
}

fn main() {
    println!("=== Baseline Performance Measurement ===");
    println!("Measuring actual performance for regression test baselines...\n");

    measure_atomic_operations();

    #[cfg(feature = "std")]
    {
        measure_concurrent_map();
        measure_lockfree_table();
        measure_concurrent_operations();
    }

    println!("\n=== Copy these values to benches/BASELINE_PERFORMANCE.toml ===");
}
