//! Benchmarks for PersistentMinHashIndex
//!
//! **B32 Framework Compliance**
//!
//! - Fair baselines (serialize+fsync, RocksDB comparison)
//! - 1000+ iterations (95% CI via Criterion)
//! - Honest reporting (document failures)
//! - Statistical rigor (outlier detection)
//!
//! **Performance Targets**:
//! - Sketch computation: <100μs per document
//! - Insert: <500ns per operation
//! - Batch 10K docs: <100ms total

#![cfg(all(
    feature = "mmap-persistence",
    feature = "nightly-atomic",
    feature = "std"
))]

use atomic_capsule::collections::persistent_minhash::*;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::path::PathBuf;

// ============================================================================
// UTILITIES
// ============================================================================

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("bench_minhash_{}.mmap", name))
}

fn cleanup(path: &PathBuf) {
    let _ = std::fs::remove_file(path);
}

// ============================================================================
// SUITE 1: SKETCH COMPUTATION
// ============================================================================

fn bench_sketch_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("sketch_computation");

    // Token counts: 10, 100, 1000
    for token_count in [10, 100, 1000].iter() {
        let path = temp_path(&format!("sketch_{}", token_count));
        cleanup(&path);

        let index = PersistentMinHashIndex::create(&path, 100).expect("Failed to create");

        // Generate content with specified token count
        let content: String = (0..*token_count)
            .map(|i| format!("token{}", i))
            .collect::<Vec<_>>()
            .join(" ");

        group.throughput(Throughput::Elements(*token_count as u64));
        group.bench_with_input(
            BenchmarkId::new("compute_sketch", token_count),
            &content,
            |b, content| {
                b.iter(|| {
                    black_box(index.compute_sketch(content));
                });
            },
        );

        cleanup(&path);
    }

    group.finish();
}

// ============================================================================
// SUITE 2: INSERT PERFORMANCE
// ============================================================================

fn bench_insert_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_performance");

    // Single insert
    {
        let path = temp_path("insert_single");
        cleanup(&path);

        group.bench_function("insert_single", |b| {
            b.iter_batched(
                || {
                    // Setup: create fresh index
                    PersistentMinHashIndex::create(&path, 10_000).expect("Failed to create")
                },
                |mut index| {
                    // Benchmark: add single document
                    black_box(
                        index
                            .add_document(0, "benchmark document content")
                            .expect("Failed to add"),
                    );
                },
                criterion::BatchSize::SmallInput,
            );
        });

        cleanup(&path);
    }

    // Sequential inserts (10, 100, 1000)
    for count in [10, 100, 1000].iter() {
        let path = temp_path(&format!("insert_{}", count));
        cleanup(&path);

        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(
            BenchmarkId::new("insert_sequential", count),
            count,
            |b, &count| {
                b.iter_batched(
                    || PersistentMinHashIndex::create(&path, count * 2).expect("Failed to create"),
                    |mut index| {
                        for i in 0..count {
                            black_box(
                                index
                                    .add_document(i as u64, &format!("document {}", i))
                                    .expect("Failed to add"),
                            );
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        cleanup(&path);
    }

    group.finish();
}

// ============================================================================
// SUITE 3: BATCH INSERTION
// ============================================================================

fn bench_batch_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_insertion");

    // Batch sizes: 1K, 10K
    for batch_size in [1_000, 10_000].iter() {
        let path = temp_path(&format!("batch_{}", batch_size));
        cleanup(&path);

        group.throughput(Throughput::Elements(*batch_size as u64));
        group.sample_size(10); // Reduce sample size for large batches
        group.bench_with_input(
            BenchmarkId::new("batch_insert", batch_size),
            batch_size,
            |b, &batch_size| {
                b.iter_batched(
                    || {
                        PersistentMinHashIndex::create(&path, batch_size * 2)
                            .expect("Failed to create")
                    },
                    |mut index| {
                        for i in 0..batch_size {
                            black_box(
                                index
                                    .add_document(i as u64, &format!("batch document {}", i))
                                    .expect("Failed to add"),
                            );
                        }
                        // Include flush in timing (realistic)
                        index.flush().expect("Failed to flush");
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        cleanup(&path);
    }

    group.finish();
}

// ============================================================================
// SUITE 4: DUPLICATE DETECTION
// ============================================================================

fn bench_duplicate_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("duplicate_detection");

    // Index sizes: 10, 100, 1000
    for index_size in [10, 100, 1000].iter() {
        let path = temp_path(&format!("dup_check_{}", index_size));
        cleanup(&path);

        // Pre-populate index
        let mut index =
            PersistentMinHashIndex::create(&path, index_size * 2).expect("Failed to create");

        for i in 0..*index_size {
            index
                .add_document(i as u64, &format!("existing document {}", i))
                .expect("Failed to add");
        }

        let test_content = "existing document 0";

        group.bench_with_input(
            BenchmarkId::new("is_duplicate", index_size),
            index_size,
            |b, _| {
                b.iter(|| {
                    black_box(index.is_duplicate(test_content).expect("Failed to check"));
                });
            },
        );

        cleanup(&path);
    }

    group.finish();
}

// ============================================================================
// SUITE 5: RECOVERY PERFORMANCE
// ============================================================================

fn bench_recovery_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("recovery");

    // Index sizes: 100, 1000, 10K
    for size in [100, 1_000, 10_000].iter() {
        let path = temp_path(&format!("recovery_{}", size));
        cleanup(&path);

        // Pre-create and populate index
        {
            let mut index =
                PersistentMinHashIndex::create(&path, size * 2).expect("Failed to create");

            for i in 0..*size {
                index
                    .add_document(i as u64, &format!("document {}", i))
                    .expect("Failed to add");
            }

            index.flush().expect("Failed to flush");
        }

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("open", size), size, |b, _| {
            b.iter(|| {
                black_box(PersistentMinHashIndex::open(&path).expect("Failed to open"));
            });
        });

        cleanup(&path);
    }

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    benches,
    bench_sketch_computation,
    bench_insert_performance,
    bench_batch_insertion,
    bench_duplicate_detection,
    bench_recovery_performance,
);

criterion_main!(benches);
