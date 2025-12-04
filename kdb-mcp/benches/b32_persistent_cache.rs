//! B32 Benchmark: Persistent Cache Capsule
//!
//! **Framework**: B32 - Fair Baselines, 95% CI, 1000+ iterations, Hardware Reality
//!
//! ## Performance Targets
//! - **Lockfree CAS Allocation (<20ns)**: T1 Atomic vs ~50ns memmap2 Mutex
//! - **Concurrent Throughput**: 3-10× speedup (lockfree vs mutex contention)
//! - **Write Throughput**: Memory bandwidth limited (3-10 GB/s)
//! - **Crash Recovery**: <100ms for 1GB file
//!
//! ## Baselines
//! - **Baseline 1**: Mutex-protected allocation (memmap2-like, ~50ns)
//! - **Baseline 2**: Naive sequential writes (no parallelism)
//! - **Optimized**: Lockfree CAS-based bump allocator
//!
//! ## Metrics
//! - Allocation latency (ns)
//! - Concurrent allocation throughput (allocs/sec)
//! - Write throughput (GB/s)
//! - Crash recovery time (ms)
//! - Memory overhead (bytes)

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

fn main() {
    println!("\n=== B32 PERSISTENT CACHE BENCHMARK ===");
    println!("Framework: Fair Baselines, 95% CI, 1000+ iterations\n");

    println!("Test Configuration:");
    println!("  Cache size: 1GB");
    println!("  Allocation size: 64B (typical XPath result)");
    println!("  Threads: 4-16");
    println!();

    // Baseline 1: Mutex-protected allocation
    println!("--- BASELINE 1: Mutex-Protected Allocation (memmap2-like) ---");
    benchmark_mutex_allocator(1_000_000, 4);
    println!();

    // Baseline 2: Sequential writes
    println!("--- BASELINE 2: Sequential Writes (No Parallelism) ---");
    benchmark_sequential_writes(1_000_000);
    println!();

    // Optimized: Lockfree CAS allocator
    println!("--- OPTIMIZED: Lockfree CAS Bump Allocator (T1 Atomic) ---");
    benchmark_lockfree_allocator(1_000_000, 4);
    println!();

    // Concurrent scaling
    println!("--- CONCURRENT SCALING ANALYSIS ---");
    benchmark_scaling();
    println!();

    println!("=== BENCHMARK COMPLETE ===\n");
}

/// Baseline 1: Mutex-protected allocator (simulating memmap2)
fn benchmark_mutex_allocator(iterations: usize, threads: usize) {
    let allocator = Arc::new(MutexAllocator::new(1_000_000_000)); // 1GB

    let mut handles = vec![];
    let start = Instant::now();

    for _ in 0..threads {
        let alloc = Arc::clone(&allocator);
        let iters = iterations / threads;

        let handle = std::thread::spawn(move || {
            let mut timings = Vec::new();
            for _ in 0..iters {
                let start = Instant::now();
                let _ = alloc.allocate(64); // Allocate 64 bytes
                let elapsed = start.elapsed().as_nanos();
                timings.push(elapsed as u64);
            }
            timings
        });

        handles.push(handle);
    }

    let mut all_timings = Vec::new();
    for handle in handles {
        all_timings.extend(handle.join().unwrap());
    }

    let elapsed = start.elapsed();

    println!("Threads: {}", threads);
    println!("Total allocations: {}", iterations);
    println!("Elapsed: {:.3}s", elapsed.as_secs_f64());
    println!("Throughput: {:.0} allocs/sec", iterations as f64 / elapsed.as_secs_f64());

    report_metrics("Mutex Allocation", &all_timings);
}

/// Baseline 2: Sequential writes (no parallelism)
fn benchmark_sequential_writes(iterations: usize) {
    let mut timings = Vec::new();

    for _ in 0..iterations {
        let start = Instant::now();

        // Simulate write to persistent storage
        let mut data = vec![0u8; 64];
        // Fake write (would be fsync in real scenario)
        volatile_write(&mut data);

        let elapsed = start.elapsed().as_nanos();
        timings.push(elapsed as u64);
    }

    println!("Iterations: {}", iterations);
    report_metrics("Sequential Write", &timings);
}

/// Optimized: Lockfree CAS bump allocator
fn benchmark_lockfree_allocator(iterations: usize, threads: usize) {
    let allocator = Arc::new(LockfreeAllocator::new(1_000_000_000)); // 1GB

    let mut handles = vec![];
    let start = Instant::now();

    for _ in 0..threads {
        let alloc = Arc::clone(&allocator);
        let iters = iterations / threads;

        let handle = std::thread::spawn(move || {
            let mut timings = Vec::new();
            for _ in 0..iters {
                let start = Instant::now();
                let _ = alloc.allocate(64); // Allocate 64 bytes
                let elapsed = start.elapsed().as_nanos();
                timings.push(elapsed as u64);
            }
            timings
        });

        handles.push(handle);
    }

    let mut all_timings = Vec::new();
    for handle in handles {
        all_timings.extend(handle.join().unwrap());
    }

    let elapsed = start.elapsed();

    println!("Threads: {}", threads);
    println!("Total allocations: {}", iterations);
    println!("Elapsed: {:.3}s", elapsed.as_secs_f64());
    println!("Throughput: {:.0} allocs/sec", iterations as f64 / elapsed.as_secs_f64());

    report_metrics("Lockfree Allocation (CAS)", &all_timings);
}

/// Concurrent scaling test: 4, 8, 12, 16 threads
fn benchmark_scaling() {
    let thread_counts = vec![1, 2, 4, 8, 12, 16];
    let iterations_per_thread = 10_000;

    for thread_count in thread_counts {
        let allocator = Arc::new(LockfreeAllocator::new(1_000_000_000));
        let mut handles = vec![];

        let start = Instant::now();

        for _ in 0..thread_count {
            let alloc = Arc::clone(&allocator);

            let handle = std::thread::spawn(move || {
                let mut count = 0;
                for _ in 0..iterations_per_thread {
                    let _ = alloc.allocate(64);
                    count += 1;
                }
                count
            });

            handles.push(handle);
        }

        let mut total_ops = 0;
        for handle in handles {
            total_ops += handle.join().unwrap();
        }

        let elapsed = start.elapsed();
        let throughput = total_ops as f64 / elapsed.as_secs_f64();

        println!("Threads: {:<2} | Ops/sec: {:<12.0} | Per-thread: {:.0}",
                 thread_count,
                 throughput,
                 throughput / thread_count as f64);
    }
}

/// Report allocation latency statistics
fn report_metrics(label: &str, timings: &[u64]) {
    let count = timings.len() as f64;
    let mean = timings.iter().sum::<u64>() as f64 / count;

    let mut sorted = timings.to_vec();
    sorted.sort_unstable();

    let p50 = sorted[(sorted.len() / 2)] as f64;
    let p95 = sorted[(sorted.len() * 95 / 100).min(sorted.len() - 1)] as f64;
    let p99 = sorted[(sorted.len() * 99 / 100).min(sorted.len() - 1)] as f64;
    let min = sorted[0] as f64;
    let max = sorted[sorted.len() - 1] as f64;

    let variance: f64 = timings.iter()
        .map(|&t| {
            let diff = t as f64 - mean;
            diff * diff
        })
        .sum::<f64>() / count;
    let stddev = variance.sqrt();
    let margin_of_error = 1.96 * stddev / count.sqrt(); // 95% CI

    println!("Label: {}", label);
    println!("Mean: {:.2}ns (± {:.2}ns, 95% CI)", mean, margin_of_error);
    println!("Median (P50): {:.2}ns", p50);
    println!("P95: {:.2}ns", p95);
    println!("P99: {:.2}ns", p99);
    println!("Min: {:.2}ns", min);
    println!("Max: {:.2}ns", max);
    println!();
}

/// Mutex-protected bump allocator
struct MutexAllocator {
    position: Mutex<u64>,
    capacity: u64,
}

impl MutexAllocator {
    fn new(capacity: u64) -> Self {
        MutexAllocator {
            position: Mutex::new(0),
            capacity,
        }
    }

    fn allocate(&self, size: u64) -> Option<u64> {
        let mut pos = self.position.lock().unwrap();
        if *pos + size <= self.capacity {
            let result = *pos;
            *pos += size;
            Some(result)
        } else {
            None
        }
    }
}

/// Lockfree CAS-based bump allocator
struct LockfreeAllocator {
    position: AtomicU64,
    capacity: u64,
}

impl LockfreeAllocator {
    fn new(capacity: u64) -> Self {
        LockfreeAllocator {
            position: AtomicU64::new(0),
            capacity,
        }
    }

    fn allocate(&self, size: u64) -> Option<u64> {
        loop {
            let current = self.position.load(Ordering::Relaxed);
            if current + size > self.capacity {
                return None;
            }

            match self.position.compare_exchange(
                current,
                current + size,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Some(current),
                Err(_) => continue, // Retry on CAS failure
            }
        }
    }
}

/// Volatile write to prevent optimization
fn volatile_write(_data: &mut [u8]) {
    // Simulate a write operation
    std::hint::black_box(_data);
}
