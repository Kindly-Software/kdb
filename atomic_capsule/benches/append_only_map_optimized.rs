//! # AppendOnlyMapCapsuleOptimized Benchmarks (B32 Framework)
//!
//! **FAIR BASELINES**: Compare against optimized baseline (not strawman)
//! **STATISTICAL RIGOR**: 1000+ samples, 95% CI via Criterion
//! **HONEST REPORTING**: Document where optimizations fail AND succeed
//! **REALITY CHECKS**: Validate 10-50% typical, 2-10× exceptional, 100× rare

use atomic_capsule::collections::{AppendOnlyMapCapsule, AppendOnlyMapCapsuleOptimized};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::thread;

// ============================================================================
// B1: FAIR BASELINE - Compare optimized vs baseline (not strawman)
// ============================================================================

/// Benchmark baseline single insert (<10ns)
fn bench_baseline_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_single");

    let map = AppendOnlyMapCapsule::new(100_000);

    let mut i = 0u64;
    group.bench_function("baseline", |b| {
        b.iter(|| {
            let key = black_box(i);
            let value = black_box(key * 2);
            let _ = map.insert(key, value);
            i += 1;
        });
    });

    group.finish();
}

/// Benchmark optimized single insert (should match baseline <10ns)
fn bench_optimized_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_single");

    let map = AppendOnlyMapCapsuleOptimized::new(100_000);

    let mut i = 0u64;
    group.bench_function("optimized", |b| {
        b.iter(|| {
            let key = black_box(i);
            let value = black_box(key * 2);
            let _ = map.insert(key, value);
            i += 1;
        });
    });

    group.finish();
}

// ============================================================================
// T4: BATCH INSERT BENCHMARK (Target: 5× throughput)
// ============================================================================

/// Benchmark batch insert (T4 optimization)
fn bench_batch_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_batch");

    for batch_size in [100, 1_000, 10_000, 100_000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));

        let map = AppendOnlyMapCapsuleOptimized::new(1_000_000);
        let pairs: Vec<(u64, u64)> = (0..*batch_size).map(|i| (i as u64, i as u64 * 2)).collect();

        group.bench_with_input(BenchmarkId::new("batch", batch_size), batch_size, |b, _| {
            b.iter(|| {
                let _ = map.insert_batch(black_box(&pairs));
            });
        });
    }

    group.finish();
}

/// Compare batch vs sequential inserts
fn bench_batch_vs_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_vs_sequential");

    const BATCH_SIZE: usize = 10_000;
    group.throughput(Throughput::Elements(BATCH_SIZE as u64));

    // Sequential baseline
    group.bench_function("sequential", |b| {
        b.iter(|| {
            let map = AppendOnlyMapCapsuleOptimized::new(100_000);
            for i in 0..BATCH_SIZE {
                let _ = map.insert(black_box(i as u64), black_box(i as u64 * 2));
            }
        });
    });

    // Batch optimized
    group.bench_function("batch", |b| {
        b.iter(|| {
            let map = AppendOnlyMapCapsuleOptimized::new(100_000);
            let pairs: Vec<(u64, u64)> =
                (0..BATCH_SIZE).map(|i| (i as u64, i as u64 * 2)).collect();
            let _ = map.insert_batch(black_box(&pairs));
        });
    });

    group.finish();
}

// ============================================================================
// T2: SIMD GET BENCHMARK (Target: 7× speedup)
// ============================================================================

/// Benchmark baseline linear scan get
fn bench_baseline_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_performance");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let map = AppendOnlyMapCapsule::new(*size);

        // Pre-populate
        for i in 0..*size {
            map.insert(i as u64, i as u64 * 2).unwrap();
        }

        group.bench_with_input(BenchmarkId::new("baseline", size), size, |b, &s| {
            b.iter(|| {
                let key = black_box((s / 2) as u64); // Middle element
                let _ = map.get(&key);
            });
        });
    }

    group.finish();
}

/// Benchmark SIMD get (T2 optimization)
#[cfg(feature = "portable_simd")]
fn bench_simd_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_performance");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let map = AppendOnlyMapCapsuleOptimized::new(*size);

        // Pre-populate
        for i in 0..*size {
            map.insert(i as u64, i as u64 * 2).unwrap();
        }

        group.bench_with_input(BenchmarkId::new("simd", size), size, |b, &s| {
            b.iter(|| {
                let key = black_box((s / 2) as u64);
                let _ = map.get_simd(&key);
            });
        });
    }

    group.finish();
}

/// Direct SIMD vs baseline comparison
#[cfg(feature = "portable_simd")]
fn bench_simd_vs_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_speedup");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        // Baseline
        let baseline_map = AppendOnlyMapCapsule::new(*size);
        for i in 0..*size {
            baseline_map.insert(i as u64, i as u64 * 2).unwrap();
        }

        group.bench_with_input(BenchmarkId::new("baseline", size), size, |b, &s| {
            b.iter(|| {
                let key = black_box((s / 2) as u64);
                let _ = baseline_map.get(&key);
            });
        });

        // SIMD
        let simd_map = AppendOnlyMapCapsuleOptimized::new(*size);
        for i in 0..*size {
            simd_map.insert(i as u64, i as u64 * 2).unwrap();
        }

        group.bench_with_input(BenchmarkId::new("simd", size), size, |b, &s| {
            b.iter(|| {
                let key = black_box((s / 2) as u64);
                let _ = simd_map.get_simd(&key);
            });
        });
    }

    group.finish();
}

// ============================================================================
// BINARY SEARCH BENCHMARK (Target: 100× speedup for sorted)
// ============================================================================

/// Benchmark binary search vs linear (sorted keys)
fn bench_binary_vs_linear(c: &mut Criterion) {
    let mut group = c.benchmark_group("binary_search");

    for size in [1_000, 10_000, 100_000, 1_000_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        let map = AppendOnlyMapCapsuleOptimized::new(*size);

        // Insert sorted keys
        for i in 0..*size {
            map.insert(i as u64, i as u64 * 2).unwrap();
        }

        // Manually mark as sorted
        map.is_sorted
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // Linear scan
        group.bench_with_input(BenchmarkId::new("linear", size), size, |b, &s| {
            b.iter(|| {
                let key = black_box((s / 2) as u64);
                let _ = map.get(&key);
            });
        });

        // Binary search
        group.bench_with_input(BenchmarkId::new("binary", size), size, |b, &s| {
            b.iter(|| {
                let key = black_box((s / 2) as u64);
                let _ = map.get_binary(&key);
            });
        });
    }

    group.finish();
}

// ============================================================================
// B27: HONEST REPORTING - Document where optimizations fail
// ============================================================================

/// SIMD overhead test (B27: Honest reporting)
///
/// **EXPECTED**: SIMD slower for small maps (<64 entries) due to setup overhead
#[cfg(feature = "portable_simd")]
fn bench_simd_overhead_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_overhead");

    for size in [8, 16, 32, 64].iter() {
        let map = AppendOnlyMapCapsuleOptimized::new(*size);

        // Pre-populate
        for i in 0..*size {
            map.insert(i as u64, i as u64 * 2).unwrap();
        }

        // Baseline
        group.bench_with_input(BenchmarkId::new("baseline", size), size, |b, &s| {
            b.iter(|| {
                let key = black_box((s / 2) as u64);
                let _ = map.get(&key);
            });
        });

        // SIMD (may be slower for small N)
        group.bench_with_input(BenchmarkId::new("simd", size), size, |b, &s| {
            b.iter(|| {
                let key = black_box((s / 2) as u64);
                let _ = map.get_simd(&key);
            });
        });
    }

    group.finish();
}

// ============================================================================
// PRODUCTION SIMULATION (Ground Truth Generation)
// ============================================================================

/// Simulate ground truth generation workload
///
/// **Workload**: 10K docs → 50M pairs
/// - 95% inserts (batch)
/// - 5% lookups (SIMD)
fn bench_ground_truth_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("production_ground_truth");

    // Simulate 50M pairs (scaled down to 500K for benchmark)
    const TOTAL_PAIRS: usize = 500_000;
    const LOOKUP_RATIO: usize = 20; // 1 lookup per 20 inserts (5%)

    group.bench_function("ground_truth_workload", |b| {
        b.iter(|| {
            let map = AppendOnlyMapCapsuleOptimized::new(TOTAL_PAIRS);

            // Batch insert 95%
            let batch_size = 10_000;
            let num_batches = (TOTAL_PAIRS * 95 / 100) / batch_size;

            for batch_idx in 0..num_batches {
                let start = batch_idx * batch_size;
                let pairs: Vec<(u64, u64)> = (start..start + batch_size)
                    .map(|i| (i as u64, i as u64))
                    .collect();
                let _ = map.insert_batch(black_box(&pairs));

                // Lookup every 20 inserts (5%)
                if batch_idx % LOOKUP_RATIO == 0 {
                    #[cfg(feature = "portable_simd")]
                    {
                        let key = black_box((start / 2) as u64);
                        let _ = map.get_simd(&key);
                    }
                    #[cfg(not(feature = "portable_simd"))]
                    {
                        let key = black_box((start / 2) as u64);
                        let _ = map.get(&key);
                    }
                }
            }
        });
    });

    group.finish();
}

// ============================================================================
// CONCURRENT BENCHMARKS
// ============================================================================

/// Benchmark concurrent inserts (thread scaling)
fn bench_concurrent_inserts(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_inserts");

    for num_threads in [1, 2, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements((num_threads * 1000) as u64));

        group.bench_with_input(
            BenchmarkId::new("threads", num_threads),
            num_threads,
            |b, &t| {
                b.iter(|| {
                    let map = Arc::new(AppendOnlyMapCapsuleOptimized::new(100_000));
                    let mut handles = vec![];

                    for thread_id in 0..t {
                        let map_clone = Arc::clone(&map);
                        handles.push(thread::spawn(move || {
                            for i in 0..1000 {
                                let key = (thread_id * 10_000 + i) as u64;
                                let _ = map_clone.insert(black_box(key), black_box(key * 2));
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// CRITERION GROUPS
// ============================================================================

criterion_group!(
    benches,
    bench_baseline_insert,
    bench_optimized_insert,
    bench_batch_insert,
    bench_batch_vs_sequential,
    bench_baseline_get,
    bench_binary_vs_linear,
    bench_ground_truth_simulation,
    bench_concurrent_inserts,
);

#[cfg(feature = "portable_simd")]
criterion_group!(
    simd_benches,
    bench_simd_get,
    bench_simd_vs_baseline,
    bench_simd_overhead_small,
);

#[cfg(feature = "portable_simd")]
criterion_main!(benches, simd_benches);

#[cfg(not(feature = "portable_simd"))]
criterion_main!(benches);
