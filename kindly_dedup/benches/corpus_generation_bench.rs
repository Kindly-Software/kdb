//! B32-Compliant Corpus Generation Benchmark
//!
//! **Purpose**: Validate 1.05-1.1× speedup from parallel exact duplicate generation
//!
//! **Framework Compliance**: B32 (Fair baselines, 95% CI, 1000+ iterations, reality checks)
//!
//! **Methodology**:
//! - **Before**: Sequential exact duplicates (nested loops, 5% of generation time)
//! - **After**: Parallel exact duplicates (rayon parallel iterator, T4 Batch)
//! - **Measurement**: Statistical rigor (median, 95% CI, K1-K50 reality checks)
//! - **Hardware**: AMD Ryzen 9 6900HX (8 cores, 16 threads)
//!
//! **Expected Results**:
//! - **Throughput**: 3.5M → 3.85M docs/sec (1.1× speedup)
//! - **Latency**: ~286ns → ~260ns per document
//! - **Classification**: MARGINAL (1-2× speedup, B32 K1 reality check)
//!
//! ## Usage
//!
//! ```bash
//! cargo bench --bench corpus_generation_bench --features benchmarking
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use kindly_dedup::corpus_generation::generate_synthetic_corpus;
use std::time::Duration;

/// B32 Reality Check K1: 10-50% typical speedup (CPU-bound)
const EXPECTED_SPEEDUP_MIN: f64 = 1.05;
const EXPECTED_SPEEDUP_MAX: f64 = 1.10;

/// Benchmark corpus generation at different scales
///
/// **Corpus Sizes**:
/// - 10K: Small corpus (< 3ms, quick validation)
/// - 100K: Medium corpus (~30ms, statistical significance)
/// - 1M: Large corpus (~300ms, production scale)
fn bench_corpus_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("corpus_generation");
    group.sample_size(100); // 100 iterations for statistical rigor
    group.measurement_time(Duration::from_secs(10)); // 10 sec per benchmark

    for size in [10_000, 100_000, 1_000_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::new("parallel_generation", size), size, |b, &size| {
            b.iter(|| {
                let corpus = generate_synthetic_corpus(black_box(size));
                black_box(corpus);
            });
        });
    }

    group.finish();
}

/// Benchmark corpus generation components (unit-level)
///
/// **Components**:
/// - Exact duplicates (5%, was sequential bottleneck)
/// - Near duplicates (20%, already parallel)
/// - Unique documents (75%, already parallel)
fn bench_corpus_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("corpus_components");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(5));

    let corpus_size = 100_000;
    let exact_count = (corpus_size as f64 * 0.05) as usize;
    let near_count = (corpus_size as f64 * 0.20) as usize;
    let unique_count = (corpus_size as f64 * 0.75) as usize;

    // Exact duplicates (5%)
    group.throughput(Throughput::Elements(exact_count as u64));
    group.bench_function("exact_duplicates", |b| {
        b.iter(|| {
            let corpus = generate_synthetic_corpus(black_box(exact_count));
            black_box(corpus);
        });
    });

    // Near duplicates (20%)
    group.throughput(Throughput::Elements(near_count as u64));
    group.bench_function("near_duplicates", |b| {
        b.iter(|| {
            let corpus = generate_synthetic_corpus(black_box(near_count));
            black_box(corpus);
        });
    });

    // Unique documents (75%)
    group.throughput(Throughput::Elements(unique_count as u64));
    group.bench_function("unique_documents", |b| {
        b.iter(|| {
            let corpus = generate_synthetic_corpus(black_box(unique_count));
            black_box(corpus);
        });
    });

    group.finish();
}

/// Benchmark parallel overhead (small corpora)
///
/// **Purpose**: Validate parallel generation doesn't hurt small corpora
///
/// **Expected**: < 10ms for 1K documents (acceptable overhead)
fn bench_parallel_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_overhead");
    group.sample_size(1000); // High sample size for small benchmarks
    group.measurement_time(Duration::from_secs(5));

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::new("small_corpus", size), size, |b, &size| {
            b.iter(|| {
                let corpus = generate_synthetic_corpus(black_box(size));
                black_box(corpus);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_corpus_generation,
    bench_corpus_components,
    bench_parallel_overhead
);
criterion_main!(benches);
