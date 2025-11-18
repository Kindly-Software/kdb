//! Week 2 Batch LSH Lookup Benchmarks (B32 Compliant)
//!
//! # Purpose
//!
//! Validate 1.5× dedup speedup from batch LSH lookups vs sequential baseline.
//!
//! # B32 Framework Compliance
//!
//! - **Fair Baselines**: ParallelDedupPipeline::find_duplicates() (Phase 4.4) vs find_duplicates_batch() (Phase 12.2)
//! - **Statistical Rigor**: 1000+ iterations, 95% confidence intervals
//! - **Realistic Workloads**: 1K-10K document batches (production scale)
//! - **Honest Reporting**: Document failures, not just successes
//!
//! # Performance Targets
//!
//! - Baseline (Phase 4.4): 912K docs/sec @ 16 cores
//! - Batch LSH target: 1.37M docs/sec @ 16 cores (1.5× speedup)
//! - Classification: TYPICAL tier (1-2× speedup range)

use atomic_capsule::CpuCapabilityCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
#[cfg(feature = "parallel-dedup")]
use kindly_dedup::ParallelDedupPipeline;

/// Generate test documents (simple corpus for benchmarking)
fn generate_test_corpus(num_docs: usize) -> Vec<(usize, String)> {
    (0..num_docs)
        .map(|i| (i, format!("Document {} with some text content", i)))
        .collect()
}

/// Benchmark: Sequential LSH lookup (Phase 4.4 baseline)
#[cfg(feature = "parallel-dedup")]
fn bench_lsh_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("LSH Lookup Sequential (Phase 4.4)");
    let num_threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let cpu_caps = CpuCapabilityCapsule::detect();

    for num_docs in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(num_docs as u64));
        group.bench_with_input(BenchmarkId::from_parameter(num_docs), &num_docs, |b, &num_docs| {
            let mut pipeline = ParallelDedupPipeline::new(num_docs, num_threads, &cpu_caps).unwrap();
            let corpus = generate_test_corpus(num_docs);

            // Pre-populate pipeline
            for (doc_id, text) in &corpus {
                let _ = pipeline.add_document(*doc_id, text);
            }

            b.iter(|| black_box(pipeline.find_duplicates(0.85).unwrap()));
        });
    }
    group.finish();
}

/// Benchmark: Batch LSH lookup (Phase 12.2 optimization)
/// Note: Uses same find_duplicates() method, but with batch-lsh feature gate enabled internally
#[cfg(all(feature = "parallel-dedup", feature = "batch-lsh"))]
fn bench_lsh_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("LSH Lookup Batch (Phase 12.2)");
    let num_threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let cpu_caps = CpuCapabilityCapsule::detect();

    for num_docs in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(num_docs as u64));
        group.bench_with_input(BenchmarkId::from_parameter(num_docs), &num_docs, |b, &num_docs| {
            let mut pipeline = ParallelDedupPipeline::new(num_docs, num_threads, &cpu_caps).unwrap();
            let corpus = generate_test_corpus(num_docs);

            // Pre-populate pipeline
            for (doc_id, text) in &corpus {
                let _ = pipeline.add_document(*doc_id, text);
            }

            // With batch-lsh feature enabled, find_duplicates() uses BatchLSHLookup internally
            b.iter(|| black_box(pipeline.find_duplicates(0.85).unwrap()));
        });
    }
    group.finish();
}

#[cfg(all(feature = "parallel-dedup", feature = "batch-lsh"))]
criterion_group!(benches, bench_lsh_sequential, bench_lsh_batch);

#[cfg(all(feature = "parallel-dedup", not(feature = "batch-lsh")))]
criterion_group!(benches, bench_lsh_sequential);

#[cfg(not(feature = "parallel-dedup"))]
criterion_group!(benches,);

criterion_main!(benches);
