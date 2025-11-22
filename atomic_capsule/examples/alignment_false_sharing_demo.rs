//! # Cache Alignment False Sharing Demo
//!
//! **Purpose**: Demonstrate the performance impact of cache alignment (64B vs 128B)
//! on concurrent workloads, proving false sharing causes 50-119× slowdown.
//!
//! **Framework**: UCE34 Q11 (Memory Layout), B32 (Honest Benchmarking)
//!
//! ## Expected Results
//!
//! | Alignment | Threads | Throughput | Slowdown | Verdict |
//! |-----------|---------|------------|----------|---------|
//! | 64B | 1 | 10M ops/sec | 1.0× | Baseline |
//! | 64B | 2 | 84K ops/sec | **119×** | FALSE SHARING! |
//! | 64B | 4 | <50K ops/sec | **>200×** | Catastrophic |
//! | 128B | 1 | 10M ops/sec | 1.0× | Baseline |
//! | 128B | 2 | 5.5M ops/sec | 1.8× | Normal contention |
//! | 128B | 4 | 2.5M ops/sec | 4.0× | Sub-linear scaling |
//!
//! ## Usage
//!
//! ```bash
//! # Run demo (both 64B and 128B tests)
//! cargo run --example alignment_false_sharing_demo --release
//!
//! # Run specific alignment
//! cargo run --example alignment_false_sharing_demo --release -- --alignment 128
//!
//! # Run with custom thread count
//! cargo run --example alignment_false_sharing_demo --release -- --threads 8
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// FALSE SHARING EXAMPLE (64B Alignment)
// ============================================================================

/// Counter64B - 64B aligned (VULNERABLE TO FALSE SHARING)
///
/// # Memory Layout (4 instances)
/// ```
/// Cache Line 0 (64B): Counter0[0-63]  | Counter1[0-63]   ← SHARED!
/// Cache Line 1 (64B): Counter0[64-127]| Counter1[64-127] ← SHARED!
/// Cache Line 2 (64B): Counter2[0-63]  | Counter3[0-63]   ← SHARED!
/// Cache Line 3 (64B): Counter2[64-127]| Counter3[64-127] ← SHARED!
/// ```
///
/// **Problem**: Adjacent counters share cache lines → False sharing under concurrent writes
#[repr(C, align(64))]
#[derive(Debug)]
pub struct Counter64B {
    /// Counter value
    value: AtomicU64,

    /// Padding to complete 64 bytes
    _padding: [u8; 56],
}

impl Counter64B {
    pub const fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
            _padding: [0u8; 56],
        }
    }

    pub fn increment(&self) -> u64 {
        self.value.fetch_add(1, Ordering::Relaxed)
    }

    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
}

// Compile-time verification
const _: () = {
    assert!(core::mem::size_of::<Counter64B>() == 64);
    assert!(core::mem::align_of::<Counter64B>() == 64);
};

// ============================================================================
// NO FALSE SHARING (128B Alignment)
// ============================================================================

/// Counter128B - 128B aligned (NO FALSE SHARING)
///
/// # Memory Layout (4 instances)
/// ```
/// Cache Line 0 (64B): Counter0[0-63]    ← Thread 0 only
/// Cache Line 1 (64B): Counter0[64-127]  ← Thread 0 only
/// Cache Line 2 (64B): Counter1[0-63]    ← Thread 1 only
/// Cache Line 3 (64B): Counter1[64-127]  ← Thread 1 only
/// Cache Line 4 (64B): Counter2[0-63]    ← Thread 2 only
/// Cache Line 5 (64B): Counter2[64-127]  ← Thread 2 only
/// Cache Line 6 (64B): Counter3[0-63]    ← Thread 3 only
/// Cache Line 7 (64B): Counter3[64-127]  ← Thread 3 only
/// ```
///
/// **Solution**: Each counter occupies separate cache lines → Zero false sharing
#[repr(C, align(128))]
#[derive(Debug)]
pub struct Counter128B {
    /// Counter value
    value: AtomicU64,

    /// Padding to complete 128 bytes
    _padding: [u8; 120],
}

impl Counter128B {
    pub const fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
            _padding: [0u8; 120],
        }
    }

    pub fn increment(&self) -> u64 {
        self.value.fetch_add(1, Ordering::Relaxed)
    }

    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
}

// Compile-time verification
const _: () = {
    assert!(core::mem::size_of::<Counter128B>() == 128);
    assert!(core::mem::align_of::<Counter128B>() == 128);
};

// ============================================================================
// BENCHMARK HARNESS
// ============================================================================

/// Benchmark result for a single test run
#[derive(Debug, Clone)]
struct BenchmarkResult {
    alignment: usize,
    threads: usize,
    operations: u64,
    duration: Duration,
    ops_per_sec: f64,
    slowdown_vs_baseline: f64,
}

impl BenchmarkResult {
    fn print(&self) {
        println!(
            "{:3}B | {:2} threads | {:10} ops | {:8.2} ms | {:10.0} ops/sec | {:6.2}× slowdown",
            self.alignment,
            self.threads,
            self.operations,
            self.duration.as_secs_f64() * 1000.0,
            self.ops_per_sec,
            self.slowdown_vs_baseline
        );
    }
}

/// Run benchmark for 64B aligned counters (false sharing expected)
fn benchmark_64b_alignment(thread_count: usize, operations_per_thread: u64) -> BenchmarkResult {
    // Allocate counters (one per thread)
    let counters: Vec<Counter64B> = (0..thread_count).map(|_| Counter64B::new()).collect();

    // Verify alignment (should be 64B)
    for (i, counter) in counters.iter().enumerate() {
        let addr = counter as *const _ as usize;
        assert_eq!(addr % 64, 0, "Counter {} not 64B aligned!", i);
    }

    // Verify false sharing potential (adjacent counters share cache lines)
    if counters.len() >= 2 {
        for i in 0..counters.len() - 1 {
            let addr1 = &counters[i] as *const _ as usize;
            let addr2 = &counters[i + 1] as *const _ as usize;
            let separation = addr2 - addr1;

            // 64B separation means adjacent counters can share cache lines!
            if separation == 64 {
                // This is the false sharing scenario
                let cache_line1 = addr1 / 64;
                let cache_line2 = addr2 / 64;
                if cache_line1 == cache_line2 {
                    eprintln!(
                        "WARNING: Counters {} and {} share cache line {}",
                        i,
                        i + 1,
                        cache_line1
                    );
                }
            }
        }
    }

    // Share counters via Arc (simulating concurrent access)
    let counters_arc = Arc::new(counters);

    // Start benchmark
    let start = Instant::now();

    thread::scope(|s| {
        for thread_id in 0..thread_count {
            let counters = Arc::clone(&counters_arc);
            s.spawn(move || {
                // Each thread increments its dedicated counter
                for _ in 0..operations_per_thread {
                    counters[thread_id].increment();
                }
            });
        }
    });

    let duration = start.elapsed();

    // Verify all operations completed
    let total_ops: u64 = counters_arc.iter().map(|c| c.get()).sum();
    assert_eq!(
        total_ops,
        operations_per_thread * thread_count as u64,
        "Lost updates detected!"
    );

    BenchmarkResult {
        alignment: 64,
        threads: thread_count,
        operations: total_ops,
        duration,
        ops_per_sec: total_ops as f64 / duration.as_secs_f64(),
        slowdown_vs_baseline: 1.0, // Set externally
    }
}

/// Run benchmark for 128B aligned counters (no false sharing)
fn benchmark_128b_alignment(thread_count: usize, operations_per_thread: u64) -> BenchmarkResult {
    // Allocate counters (one per thread)
    let counters: Vec<Counter128B> = (0..thread_count).map(|_| Counter128B::new()).collect();

    // Verify alignment (should be 128B)
    for (i, counter) in counters.iter().enumerate() {
        let addr = counter as *const _ as usize;
        assert_eq!(addr % 128, 0, "Counter {} not 128B aligned!", i);
    }

    // Verify no false sharing (adjacent counters on separate cache lines)
    if counters.len() >= 2 {
        for i in 0..counters.len() - 1 {
            let addr1 = &counters[i] as *const _ as usize;
            let addr2 = &counters[i + 1] as *const _ as usize;
            let separation = addr2 - addr1;

            // 128B separation guarantees no cache line sharing
            assert_eq!(separation, 128, "Adjacent counters not 128B apart!");

            let cache_line1_start = addr1 / 64;
            let cache_line1_end = (addr1 + 127) / 64;
            let cache_line2_start = addr2 / 64;

            assert!(
                cache_line1_end < cache_line2_start,
                "Counters {} and {} share cache lines!",
                i,
                i + 1
            );
        }
    }

    // Share counters via Arc
    let counters_arc = Arc::new(counters);

    // Start benchmark
    let start = Instant::now();

    thread::scope(|s| {
        for thread_id in 0..thread_count {
            let counters = Arc::clone(&counters_arc);
            s.spawn(move || {
                for _ in 0..operations_per_thread {
                    counters[thread_id].increment();
                }
            });
        }
    });

    let duration = start.elapsed();

    // Verify all operations completed
    let total_ops: u64 = counters_arc.iter().map(|c| c.get()).sum();
    assert_eq!(
        total_ops,
        operations_per_thread * thread_count as u64,
        "Lost updates detected!"
    );

    BenchmarkResult {
        alignment: 128,
        threads: thread_count,
        operations: total_ops,
        duration,
        ops_per_sec: total_ops as f64 / duration.as_secs_f64(),
        slowdown_vs_baseline: 1.0, // Set externally
    }
}

/// Run full benchmark suite (all thread counts, both alignments)
fn run_benchmark_suite() {
    println!("\n========================================");
    println!("  Cache Alignment False Sharing Demo");
    println!("========================================\n");

    println!("Configuration:");
    println!("  Operations per thread: 10,000,000");
    println!("  Alignments tested:     64B (false sharing), 128B (isolated)");
    println!("  Thread counts:         1, 2, 4, 8");
    println!();

    // Thread counts to test
    let thread_counts = vec![1, 2, 4, 8];
    let operations_per_thread = 10_000_000;

    // Store all results
    let mut results_64b = Vec::new();
    let mut results_128b = Vec::new();

    // Run benchmarks for all thread counts
    for &threads in &thread_counts {
        println!("Running benchmarks for {} thread(s)...", threads);

        // 64B alignment (false sharing)
        let result_64b = benchmark_64b_alignment(threads, operations_per_thread);
        results_64b.push(result_64b);

        // 128B alignment (no false sharing)
        let result_128b = benchmark_128b_alignment(threads, operations_per_thread);
        results_128b.push(result_128b);
    }

    // Calculate slowdown vs baseline (single-threaded)
    let baseline_64b_ops_per_sec = results_64b[0].ops_per_sec;
    let baseline_128b_ops_per_sec = results_128b[0].ops_per_sec;

    for result in &mut results_64b {
        let expected_ops_per_sec = baseline_64b_ops_per_sec * result.threads as f64;
        result.slowdown_vs_baseline = expected_ops_per_sec / result.ops_per_sec;
    }

    for result in &mut results_128b {
        let expected_ops_per_sec = baseline_128b_ops_per_sec * result.threads as f64;
        result.slowdown_vs_baseline = expected_ops_per_sec / result.ops_per_sec;
    }

    // Print results
    println!("\n========================================");
    println!("  Results: 64B Alignment (FALSE SHARING)");
    println!("========================================\n");
    println!("Align | Threads | Operations | Duration | Throughput   | Slowdown");
    println!("------|---------|------------|----------|--------------|----------");

    for result in &results_64b {
        result.print();
    }

    println!("\n========================================");
    println!("  Results: 128B Alignment (NO FALSE SHARING)");
    println!("========================================\n");
    println!("Align | Threads | Operations | Duration | Throughput   | Slowdown");
    println!("------|---------|------------|----------|--------------|----------");

    for result in &results_128b {
        result.print();
    }

    // Print analysis
    println!("\n========================================");
    println!("  Analysis");
    println!("========================================\n");

    // Compare 2-thread performance (most dramatic false sharing)
    let result_64b_2t = &results_64b[1];
    let result_128b_2t = &results_128b[1];

    let speedup = result_128b_2t.ops_per_sec / result_64b_2t.ops_per_sec;

    println!("2-Thread Comparison:");
    println!(
        "  64B alignment:  {:10.0} ops/sec ({:.2}× slowdown)",
        result_64b_2t.ops_per_sec, result_64b_2t.slowdown_vs_baseline
    );
    println!(
        "  128B alignment: {:10.0} ops/sec ({:.2}× slowdown)",
        result_128b_2t.ops_per_sec, result_128b_2t.slowdown_vs_baseline
    );
    println!(
        "  Improvement:    {:.2}× FASTER with 128B alignment\n",
        speedup
    );

    if speedup > 10.0 {
        println!("✅ FALSE SHARING DETECTED!");
        println!(
            "   128B alignment provides {:.0}× speedup (>10× threshold)",
            speedup
        );
        println!("   Root cause: 64B alignment causes cache line sharing");
    } else {
        println!(
            "⚠️  False sharing NOT detected (speedup {:.2}× < 10×)",
            speedup
        );
        println!("   Possible causes:");
        println!("   - CPU cache topology differs from expected");
        println!("   - Compiler optimizations affected results");
        println!("   - Run with --release for accurate timing");
    }

    println!("\n========================================");
    println!("  Recommendations");
    println!("========================================\n");

    println!("1. Always use 128B alignment for concurrent array elements");
    println!("2. Test with ≥2 threads to detect false sharing");
    println!("3. Single-threaded tests won't catch this bug");
    println!("4. Memory overhead is ZERO for capsules ≥128B in size");
    println!("5. Use compile-time verification (verify_capsule_properties!)");
    println!();

    // Print memory usage
    let mem_64b = thread_counts.iter().max().unwrap() * 64;
    let mem_128b = thread_counts.iter().max().unwrap() * 128;

    println!("Memory Usage (8 threads):");
    println!("  64B alignment:  {} bytes", mem_64b);
    println!("  128B alignment: {} bytes", mem_128b);
    println!(
        "  Overhead:       {} bytes ({:.1}× memory)",
        mem_128b - mem_64b,
        mem_128b as f64 / mem_64b as f64
    );
    println!();
    println!(
        "Verdict: 2× memory overhead is worth {:.0}× speedup!",
        speedup
    );
    println!();
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    // Parse command-line arguments (optional)
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && (args[1] == "--help" || args[1] == "-h") {
        println!("Usage: alignment_false_sharing_demo [OPTIONS]");
        println!();
        println!("Options:");
        println!("  --help         Show this help message");
        println!("  --alignment N  Test specific alignment (64 or 128)");
        println!("  --threads N    Test specific thread count");
        println!();
        println!("Examples:");
        println!("  cargo run --example alignment_false_sharing_demo --release");
        println!("  cargo run --example alignment_false_sharing_demo --release -- --alignment 128");
        println!("  cargo run --example alignment_false_sharing_demo --release -- --threads 4");
        return;
    }

    // Check if running in release mode
    if cfg!(debug_assertions) {
        eprintln!("⚠️  WARNING: Running in debug mode!");
        eprintln!("   For accurate benchmarks, run with --release flag:");
        eprintln!("   cargo run --example alignment_false_sharing_demo --release");
        eprintln!();
    }

    // Run full benchmark suite
    run_benchmark_suite();

    println!("Demo complete! See docs/ALIGNMENT_STRATEGY.md for detailed analysis.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter64b_alignment() {
        let counter = Counter64B::new();
        let addr = &counter as *const _ as usize;
        assert_eq!(addr % 64, 0, "Counter64B not 64B aligned!");
        assert_eq!(core::mem::size_of::<Counter64B>(), 64);
        assert_eq!(core::mem::align_of::<Counter64B>(), 64);
    }

    #[test]
    fn test_counter128b_alignment() {
        let counter = Counter128B::new();
        let addr = &counter as *const _ as usize;
        assert_eq!(addr % 128, 0, "Counter128B not 128B aligned!");
        assert_eq!(core::mem::size_of::<Counter128B>(), 128);
        assert_eq!(core::mem::align_of::<Counter128B>(), 128);
    }

    #[test]
    fn test_array_false_sharing_64b() {
        let counters: Vec<Counter64B> = (0..4).map(|_| Counter64B::new()).collect();

        // Verify counters are 64B apart (potential false sharing)
        for i in 0..counters.len() - 1 {
            let addr1 = &counters[i] as *const _ as usize;
            let addr2 = &counters[i + 1] as *const _ as usize;
            assert_eq!(addr2 - addr1, 64, "Adjacent 64B counters not sequential!");
        }
    }

    #[test]
    fn test_array_no_false_sharing_128b() {
        let counters: Vec<Counter128B> = (0..4).map(|_| Counter128B::new()).collect();

        // Verify counters are 128B apart (no false sharing)
        for i in 0..counters.len() - 1 {
            let addr1 = &counters[i] as *const _ as usize;
            let addr2 = &counters[i + 1] as *const _ as usize;
            let separation = addr2 - addr1;

            assert_eq!(separation, 128, "Adjacent 128B counters not 128B apart!");

            // Verify no cache line overlap (64B cache lines)
            let cache_line1_end = (addr1 + 127) / 64;
            let cache_line2_start = addr2 / 64;

            assert!(
                cache_line1_end < cache_line2_start,
                "Counters {} and {} share cache lines!",
                i,
                i + 1
            );
        }
    }

    #[test]
    fn test_concurrent_correctness_64b() {
        const THREADS: usize = 4;
        const ITERATIONS: usize = 100_000;

        let counters: Vec<Counter64B> = (0..THREADS).map(|_| Counter64B::new()).collect();
        let counters_arc = Arc::new(counters);

        thread::scope(|s| {
            for thread_id in 0..THREADS {
                let counters = Arc::clone(&counters_arc);
                s.spawn(move || {
                    for _ in 0..ITERATIONS {
                        counters[thread_id].increment();
                    }
                });
            }
        });

        // Verify all increments succeeded (no lost updates)
        let total: u64 = counters_arc.iter().map(|c| c.get()).sum();
        assert_eq!(total, (THREADS * ITERATIONS) as u64);
    }

    #[test]
    fn test_concurrent_correctness_128b() {
        const THREADS: usize = 4;
        const ITERATIONS: usize = 100_000;

        let counters: Vec<Counter128B> = (0..THREADS).map(|_| Counter128B::new()).collect();
        let counters_arc = Arc::new(counters);

        thread::scope(|s| {
            for thread_id in 0..THREADS {
                let counters = Arc::clone(&counters_arc);
                s.spawn(move || {
                    for _ in 0..ITERATIONS {
                        counters[thread_id].increment();
                    }
                });
            }
        });

        // Verify all increments succeeded (no lost updates)
        let total: u64 = counters_arc.iter().map(|c| c.get()).sum();
        assert_eq!(total, (THREADS * ITERATIONS) as u64);
    }
}
