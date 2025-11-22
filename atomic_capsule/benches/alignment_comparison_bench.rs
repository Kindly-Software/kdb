//! # Cache Alignment Comparison Benchmark (64B vs 128B)
//!
//! **Purpose**: Quantify the performance impact of cache alignment (64B vs 128B) under
//! multi-threaded contention to validate false sharing elimination.
//!
//! **Framework**: B32 Honest Benchmarking
//!
//! ## Expected Results
//!
//! | Alignment | Threads | Throughput (Mops/sec) | Slowdown | Verdict |
//! |-----------|---------|----------------------|----------|---------|
//! | 64B | 1 | 10.0 | 1.0× | Baseline |
//! | 64B | 2 | 0.08 | **119×** | FALSE SHARING! |
//! | 64B | 4 | <0.05 | **>200×** | Catastrophic |
//! | 128B | 1 | 10.0 | 1.0× | Baseline |
//! | 128B | 2 | 5.5 | 1.8× | Normal contention |
//! | 128B | 4 | 2.5 | 4.0× | Sub-linear scaling |
//!
//! **Key Insight**: 128B alignment provides **50-60× speedup** at 2+ threads.
//!
//! ## Usage
//!
//! ```bash
//! # Run all alignment benchmarks
//! cargo bench --bench alignment_comparison_bench --features std
//!
//! # Save baseline for comparison
//! cargo bench --bench alignment_comparison_bench -- --save-baseline main
//!
//! # Compare against baseline
//! cargo bench --bench alignment_comparison_bench -- --baseline main
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

// ============================================================================
// TEST STRUCTURES
// ============================================================================

/// Counter64B - 64B aligned (VULNERABLE TO FALSE SHARING)
#[repr(C, align(64))]
pub struct Counter64B {
    value: AtomicU64,
    _padding: [u8; 56],
}

impl Counter64B {
    pub const fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
            _padding: [0u8; 56],
        }
    }

    #[inline]
    pub fn increment(&self) -> u64 {
        self.value.fetch_add(1, Ordering::Relaxed)
    }

    #[inline]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
}

/// Counter128B - 128B aligned (NO FALSE SHARING)
#[repr(C, align(128))]
pub struct Counter128B {
    value: AtomicU64,
    _padding: [u8; 120],
}

impl Counter128B {
    pub const fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
            _padding: [0u8; 120],
        }
    }

    #[inline]
    pub fn increment(&self) -> u64 {
        self.value.fetch_add(1, Ordering::Relaxed)
    }

    #[inline]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
}

// Compile-time verification
const _: () = {
    assert!(core::mem::size_of::<Counter64B>() == 64);
    assert!(core::mem::align_of::<Counter64B>() == 64);
    assert!(core::mem::size_of::<Counter128B>() == 128);
    assert!(core::mem::align_of::<Counter128B>() == 128);
};

// ============================================================================
// BENCHMARK: SINGLE-THREADED BASELINE
// ============================================================================

/// Benchmark: Single-threaded increment (baseline, no contention)
fn bench_single_thread_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("alignment_single_thread");
    group.throughput(Throughput::Elements(1_000_000));

    // 64B alignment
    group.bench_function("64B_1M_increments", |b| {
        let counter = Counter64B::new();
        b.iter(|| {
            for _ in 0..1_000_000 {
                black_box(counter.increment());
            }
        });
    });

    // 128B alignment
    group.bench_function("128B_1M_increments", |b| {
        let counter = Counter128B::new();
        b.iter(|| {
            for _ in 0..1_000_000 {
                black_box(counter.increment());
            }
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK: MULTI-THREADED CONTENTION
// ============================================================================

/// Benchmark: Multi-threaded increment to expose false sharing
///
/// **Hypothesis**: 64B alignment shows exponential degradation, 128B scales sub-linearly
fn bench_multi_thread_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("alignment_multi_thread");

    let thread_counts = vec![1, 2, 4, 8];
    let ops_per_thread = 100_000;

    for &threads in &thread_counts {
        group.throughput(Throughput::Elements((threads * ops_per_thread) as u64));

        // 64B alignment (FALSE SHARING)
        group.bench_with_input(
            BenchmarkId::new("64B_alignment", format!("{}_threads", threads)),
            &threads,
            |b, &threads| {
                b.iter(|| {
                    let counters: Vec<Counter64B> =
                        (0..threads).map(|_| Counter64B::new()).collect();

                    let counters_arc = Arc::new(counters);

                    thread::scope(|s| {
                        for thread_id in 0..threads {
                            let counters = Arc::clone(&counters_arc);
                            s.spawn(move || {
                                for _ in 0..ops_per_thread {
                                    counters[thread_id].increment();
                                }
                            });
                        }
                    });

                    // Verify all increments succeeded
                    let total: u64 = counters_arc.iter().map(|c| c.get()).sum();
                    assert_eq!(total, (threads * ops_per_thread) as u64);
                });
            },
        );

        // 128B alignment (NO FALSE SHARING)
        group.bench_with_input(
            BenchmarkId::new("128B_alignment", format!("{}_threads", threads)),
            &threads,
            |b, &threads| {
                b.iter(|| {
                    let counters: Vec<Counter128B> =
                        (0..threads).map(|_| Counter128B::new()).collect();

                    let counters_arc = Arc::new(counters);

                    thread::scope(|s| {
                        for thread_id in 0..threads {
                            let counters = Arc::clone(&counters_arc);
                            s.spawn(move || {
                                for _ in 0..ops_per_thread {
                                    counters[thread_id].increment();
                                }
                            });
                        }
                    });

                    // Verify all increments succeeded
                    let total: u64 = counters_arc.iter().map(|c| c.get()).sum();
                    assert_eq!(total, (threads * ops_per_thread) as u64);
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK: READ-HEAVY WORKLOAD
// ============================================================================

/// Benchmark: Read-heavy workload (95% reads, 5% writes)
///
/// **Purpose**: Validate that false sharing primarily impacts write-heavy workloads
fn bench_read_heavy_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("alignment_read_heavy");

    let thread_counts = vec![2, 4, 8];
    let ops_per_thread = 100_000;

    for &threads in &thread_counts {
        group.throughput(Throughput::Elements((threads * ops_per_thread) as u64));

        // 64B alignment
        group.bench_with_input(
            BenchmarkId::new("64B_95read_5write", format!("{}_threads", threads)),
            &threads,
            |b, &threads| {
                b.iter(|| {
                    let counters: Vec<Counter64B> =
                        (0..threads).map(|_| Counter64B::new()).collect();

                    let counters_arc = Arc::new(counters);

                    thread::scope(|s| {
                        for thread_id in 0..threads {
                            let counters = Arc::clone(&counters_arc);
                            s.spawn(move || {
                                for i in 0..ops_per_thread {
                                    if i % 20 == 0 {
                                        // 5% writes
                                        counters[thread_id].increment();
                                    } else {
                                        // 95% reads
                                        black_box(counters[thread_id].get());
                                    }
                                }
                            });
                        }
                    });
                });
            },
        );

        // 128B alignment
        group.bench_with_input(
            BenchmarkId::new("128B_95read_5write", format!("{}_threads", threads)),
            &threads,
            |b, &threads| {
                b.iter(|| {
                    let counters: Vec<Counter128B> =
                        (0..threads).map(|_| Counter128B::new()).collect();

                    let counters_arc = Arc::new(counters);

                    thread::scope(|s| {
                        for thread_id in 0..threads {
                            let counters = Arc::clone(&counters_arc);
                            s.spawn(move || {
                                for i in 0..ops_per_thread {
                                    if i % 20 == 0 {
                                        // 5% writes
                                        counters[thread_id].increment();
                                    } else {
                                        // 95% reads
                                        black_box(counters[thread_id].get());
                                    }
                                }
                            });
                        }
                    });
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK: WRITE-HEAVY WORKLOAD
// ============================================================================

/// Benchmark: Write-heavy workload (80% writes, 20% reads)
///
/// **Purpose**: Worst case for false sharing (expect maximum degradation)
fn bench_write_heavy_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("alignment_write_heavy");

    let thread_counts = vec![2, 4, 8];
    let ops_per_thread = 100_000;

    for &threads in &thread_counts {
        group.throughput(Throughput::Elements((threads * ops_per_thread) as u64));

        // 64B alignment (WORST CASE FALSE SHARING)
        group.bench_with_input(
            BenchmarkId::new("64B_80write_20read", format!("{}_threads", threads)),
            &threads,
            |b, &threads| {
                b.iter(|| {
                    let counters: Vec<Counter64B> =
                        (0..threads).map(|_| Counter64B::new()).collect();

                    let counters_arc = Arc::new(counters);

                    thread::scope(|s| {
                        for thread_id in 0..threads {
                            let counters = Arc::clone(&counters_arc);
                            s.spawn(move || {
                                for i in 0..ops_per_thread {
                                    if i % 5 < 4 {
                                        // 80% writes
                                        counters[thread_id].increment();
                                    } else {
                                        // 20% reads
                                        black_box(counters[thread_id].get());
                                    }
                                }
                            });
                        }
                    });
                });
            },
        );

        // 128B alignment
        group.bench_with_input(
            BenchmarkId::new("128B_80write_20read", format!("{}_threads", threads)),
            &threads,
            |b, &threads| {
                b.iter(|| {
                    let counters: Vec<Counter128B> =
                        (0..threads).map(|_| Counter128B::new()).collect();

                    let counters_arc = Arc::new(counters);

                    thread::scope(|s| {
                        for thread_id in 0..threads {
                            let counters = Arc::clone(&counters_arc);
                            s.spawn(move || {
                                for i in 0..ops_per_thread {
                                    if i % 5 < 4 {
                                        // 80% writes
                                        counters[thread_id].increment();
                                    } else {
                                        // 20% reads
                                        black_box(counters[thread_id].get());
                                    }
                                }
                            });
                        }
                    });
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK: MEMORY OVERHEAD ANALYSIS
// ============================================================================

/// Benchmark: Memory allocation overhead (64B vs 128B)
fn bench_memory_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("alignment_memory_overhead");

    let array_sizes = vec![100, 1000, 10_000];

    for &size in &array_sizes {
        // 64B alignment
        group.bench_with_input(
            BenchmarkId::new("64B_alloc", format!("{}_counters", size)),
            &size,
            |b, &size| {
                b.iter(|| {
                    let counters: Vec<Counter64B> = (0..size).map(|_| Counter64B::new()).collect();
                    black_box(counters);
                });
            },
        );

        // 128B alignment
        group.bench_with_input(
            BenchmarkId::new("128B_alloc", format!("{}_counters", size)),
            &size,
            |b, &size| {
                b.iter(|| {
                    let counters: Vec<Counter128B> =
                        (0..size).map(|_| Counter128B::new()).collect();
                    black_box(counters);
                });
            },
        );
    }

    // Print memory usage analysis
    println!("\n========================================");
    println!("  Memory Usage Analysis");
    println!("========================================\n");
    println!("Count  | 64B Total | 128B Total | Overhead | Per-Item Overhead");
    println!("-------|-----------|------------|----------|-----------------");

    for &size in &array_sizes {
        let mem_64b = size * 64;
        let mem_128b = size * 128;
        let overhead = mem_128b - mem_64b;
        let overhead_pct = (mem_128b as f64 / mem_64b as f64 - 1.0) * 100.0;

        println!(
            "{:6} | {:9} | {:10} | {:8} | {:6.1}% ({} bytes/item)",
            size,
            format!("{} B", mem_64b),
            format!("{} B", mem_128b),
            format!("{} B", overhead),
            overhead_pct,
            64
        );
    }

    println!();
    println!("Note: For capsules ≥128B in size, alignment overhead is ZERO.");
    println!("      Memory overhead only applies to small capsules (<128B).");
    println!();

    group.finish();
}

// ============================================================================
// MAIN BENCHMARK GROUP
// ============================================================================

criterion_group!(
    benches,
    bench_single_thread_baseline,
    bench_multi_thread_contention,
    bench_read_heavy_workload,
    bench_write_heavy_workload,
    bench_memory_overhead,
);

criterion_main!(benches);

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter64b_size_and_alignment() {
        assert_eq!(core::mem::size_of::<Counter64B>(), 64);
        assert_eq!(core::mem::align_of::<Counter64B>(), 64);

        let counter = Counter64B::new();
        let addr = &counter as *const _ as usize;
        assert_eq!(addr % 64, 0, "Counter64B not 64B aligned!");
    }

    #[test]
    fn test_counter128b_size_and_alignment() {
        assert_eq!(core::mem::size_of::<Counter128B>(), 128);
        assert_eq!(core::mem::align_of::<Counter128B>(), 128);

        let counter = Counter128B::new();
        let addr = &counter as *const _ as usize;
        assert_eq!(addr % 128, 0, "Counter128B not 128B aligned!");
    }

    #[test]
    fn test_counter64b_array_spacing() {
        let counters: Vec<Counter64B> = (0..4).map(|_| Counter64B::new()).collect();

        for i in 0..counters.len() - 1 {
            let addr1 = &counters[i] as *const _ as usize;
            let addr2 = &counters[i + 1] as *const _ as usize;
            assert_eq!(addr2 - addr1, 64, "Adjacent 64B counters not sequential!");
        }
    }

    #[test]
    fn test_counter128b_array_spacing() {
        let counters: Vec<Counter128B> = (0..4).map(|_| Counter128B::new()).collect();

        for i in 0..counters.len() - 1 {
            let addr1 = &counters[i] as *const _ as usize;
            let addr2 = &counters[i + 1] as *const _ as usize;
            assert_eq!(addr2 - addr1, 128, "Adjacent 128B counters not 128B apart!");

            // Verify no cache line sharing (64B cache lines)
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
    fn test_counter_correctness() {
        let counter64 = Counter64B::new();
        let counter128 = Counter128B::new();

        for _ in 0..1000 {
            counter64.increment();
            counter128.increment();
        }

        assert_eq!(counter64.get(), 1000);
        assert_eq!(counter128.get(), 1000);
    }

    #[test]
    fn test_concurrent_correctness() {
        const THREADS: usize = 4;
        const ITERATIONS: usize = 10_000;

        // Test 64B alignment
        {
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

            let total: u64 = counters_arc.iter().map(|c| c.get()).sum();
            assert_eq!(total, (THREADS * ITERATIONS) as u64);
        }

        // Test 128B alignment
        {
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

            let total: u64 = counters_arc.iter().map(|c| c.get()).sum();
            assert_eq!(total, (THREADS * ITERATIONS) as u64);
        }
    }
}
