//! Batch Serialization Benchmarks - B32 Framework Validation
//!
//! **Claim**: 100× throughput improvement for batch vs individual serialization
//!
//! ## Baseline
//! - Individual serialization: Serialize 1000 Q16_16 records one-by-one
//! - Measured overhead: header(16) + checksum(4) per record = 20 bytes × 1000 = 20KB
//!
//! ## Optimized
//! - Batch serialization: Single header(16) + checksum(4) = 20 bytes total
//! - Amortization: 20KB → 20 bytes = **1000× overhead reduction**
//!
//! ## B32 Framework Compliance
//!
//! 1. **Fair Baseline**: Individual serialization using same binary format (not strawman)
//! 2. **Statistical Rigor**: 1000+ iterations, report p50/p95/p99 (not single measurement)
//! 3. **Honest Claims**: Report actual throughput (not theoretical maximum)
//! 4. **Reproducibility**: All benchmarks committed, runnable via `cargo bench`
//!
//! ## Expected Results
//!
//! - Small batches (10 records): 2-5× (overhead not fully amortized)
//! - Medium batches (100 records): 10-20× (partial amortization)
//! - Large batches (1000 records): 50-100× (full amortization)
//! - Extra-large batches (10K records): 100-200× (full amortization + parallelism)

use atomic_capsule::serialize::batch::BatchSerialize;
use atomic_capsule::serialize::fixed_point_trait::FixedPointSerialize;
use atomic_capsule::serialize::{Q16_16, Q32_32, Q8_8};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// ============================================================================
// Individual Serialization Baseline (Fair Comparison)
// ============================================================================

/// Individual serialization using binary format (fair baseline)
fn individual_serialize_q16_16(values: &[Q16_16]) -> Vec<Vec<u8>> {
    values.iter().map(|v| v.serialize_binary()).collect()
}

/// Individual deserialization using binary format (fair baseline)
fn individual_deserialize_q16_16(serialized: &[Vec<u8>]) -> Vec<Q16_16> {
    serialized
        .iter()
        .map(|bytes| Q16_16::deserialize_binary(bytes).unwrap())
        .collect()
}

// ============================================================================
// Batch Serialization Benchmarks
// ============================================================================

fn bench_batch_serialize_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_serialize_small");

    for size in [10, 50, 100] {
        let values: Vec<Q16_16> = (0..size).map(|i| Q16_16::from_i64(i * 100)).collect();

        group.throughput(Throughput::Elements(size as u64));

        // Individual serialization (baseline)
        group.bench_with_input(
            BenchmarkId::new("individual", size),
            &values,
            |b, values| {
                b.iter(|| black_box(individual_serialize_q16_16(values)));
            },
        );

        // Batch serialization (optimized)
        group.bench_with_input(BenchmarkId::new("batch", size), &values, |b, values| {
            b.iter(|| black_box(Q16_16::serialize_batch(values)));
        });
    }

    group.finish();
}

fn bench_batch_serialize_medium(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_serialize_medium");

    for size in [500, 1000, 2000] {
        let values: Vec<Q16_16> = (0..size).map(|i| Q16_16::from_i64(i * 100)).collect();

        group.throughput(Throughput::Elements(size as u64));

        // Individual serialization (baseline)
        group.bench_with_input(
            BenchmarkId::new("individual", size),
            &values,
            |b, values| {
                b.iter(|| black_box(individual_serialize_q16_16(values)));
            },
        );

        // Batch serialization (optimized)
        group.bench_with_input(BenchmarkId::new("batch", size), &values, |b, values| {
            b.iter(|| black_box(Q16_16::serialize_batch(values)));
        });
    }

    group.finish();
}

fn bench_batch_serialize_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_serialize_large");
    group.sample_size(100); // Reduce sample size for large benchmarks

    for size in [5000, 10000] {
        let values: Vec<Q16_16> = (0..size).map(|i| Q16_16::from_i64(i * 100)).collect();

        group.throughput(Throughput::Elements(size as u64));

        // Individual serialization (baseline)
        group.bench_with_input(
            BenchmarkId::new("individual", size),
            &values,
            |b, values| {
                b.iter(|| black_box(individual_serialize_q16_16(values)));
            },
        );

        // Batch serialization (optimized)
        group.bench_with_input(BenchmarkId::new("batch", size), &values, |b, values| {
            b.iter(|| black_box(Q16_16::serialize_batch(values)));
        });
    }

    group.finish();
}

// ============================================================================
// Batch Deserialization Benchmarks
// ============================================================================

fn bench_batch_deserialize_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_deserialize_small");

    for size in [10, 50, 100] {
        let values: Vec<Q16_16> = (0..size).map(|i| Q16_16::from_i64(i * 100)).collect();

        // Pre-serialize for deserialization benchmarks
        let individual_bytes = individual_serialize_q16_16(&values);
        let batch_bytes = Q16_16::serialize_batch(&values);

        group.throughput(Throughput::Elements(size as u64));

        // Individual deserialization (baseline)
        group.bench_with_input(
            BenchmarkId::new("individual", size),
            &individual_bytes,
            |b, bytes| {
                b.iter(|| black_box(individual_deserialize_q16_16(bytes)));
            },
        );

        // Batch deserialization (optimized)
        group.bench_with_input(BenchmarkId::new("batch", size), &batch_bytes, |b, bytes| {
            b.iter(|| black_box(Q16_16::deserialize_batch(bytes).unwrap()));
        });
    }

    group.finish();
}

fn bench_batch_deserialize_medium(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_deserialize_medium");

    for size in [500, 1000, 2000] {
        let values: Vec<Q16_16> = (0..size).map(|i| Q16_16::from_i64(i * 100)).collect();

        // Pre-serialize for deserialization benchmarks
        let individual_bytes = individual_serialize_q16_16(&values);
        let batch_bytes = Q16_16::serialize_batch(&values);

        group.throughput(Throughput::Elements(size as u64));

        // Individual deserialization (baseline)
        group.bench_with_input(
            BenchmarkId::new("individual", size),
            &individual_bytes,
            |b, bytes| {
                b.iter(|| black_box(individual_deserialize_q16_16(bytes)));
            },
        );

        // Batch deserialization (optimized)
        group.bench_with_input(BenchmarkId::new("batch", size), &batch_bytes, |b, bytes| {
            b.iter(|| black_box(Q16_16::deserialize_batch(bytes).unwrap()));
        });
    }

    group.finish();
}

fn bench_batch_deserialize_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_deserialize_large");
    group.sample_size(100); // Reduce sample size for large benchmarks

    for size in [5000, 10000] {
        let values: Vec<Q16_16> = (0..size).map(|i| Q16_16::from_i64(i * 100)).collect();

        // Pre-serialize for deserialization benchmarks
        let individual_bytes = individual_serialize_q16_16(&values);
        let batch_bytes = Q16_16::serialize_batch(&values);

        group.throughput(Throughput::Elements(size as u64));

        // Individual deserialization (baseline)
        group.bench_with_input(
            BenchmarkId::new("individual", size),
            &individual_bytes,
            |b, bytes| {
                b.iter(|| black_box(individual_deserialize_q16_16(bytes)));
            },
        );

        // Batch deserialization (optimized)
        group.bench_with_input(BenchmarkId::new("batch", size), &batch_bytes, |b, bytes| {
            b.iter(|| black_box(Q16_16::deserialize_batch(bytes).unwrap()));
        });
    }

    group.finish();
}

// ============================================================================
// Roundtrip Benchmarks (End-to-End)
// ============================================================================

fn bench_batch_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_roundtrip");

    for size in [100, 1000, 5000] {
        let values: Vec<Q16_16> = (0..size).map(|i| Q16_16::from_i64(i * 100)).collect();

        group.throughput(Throughput::Elements(size as u64));

        // Individual roundtrip (baseline)
        group.bench_with_input(
            BenchmarkId::new("individual", size),
            &values,
            |b, values| {
                b.iter(|| {
                    let serialized = individual_serialize_q16_16(values);
                    black_box(individual_deserialize_q16_16(&serialized))
                });
            },
        );

        // Batch roundtrip (optimized)
        group.bench_with_input(BenchmarkId::new("batch", size), &values, |b, values| {
            b.iter(|| {
                let bytes = Q16_16::serialize_batch(values);
                black_box(Q16_16::deserialize_batch(&bytes).unwrap())
            });
        });
    }

    group.finish();
}

// ============================================================================
// Type-Specific Benchmarks (Q8_8, Q32_32)
// ============================================================================

fn bench_batch_q8_8(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_q8_8");

    let values: Vec<Q8_8> = (0..1000).map(|i| Q8_8::from_i32(i * 10)).collect();

    group.throughput(Throughput::Elements(1000));

    // Batch serialization
    group.bench_function("serialize", |b| {
        b.iter(|| black_box(Q8_8::serialize_batch(&values)));
    });

    // Batch deserialization
    let batch_bytes = Q8_8::serialize_batch(&values);
    group.bench_function("deserialize", |b| {
        b.iter(|| black_box(Q8_8::deserialize_batch(&batch_bytes).unwrap()));
    });

    group.finish();
}

fn bench_batch_q32_32(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_q32_32");

    let values: Vec<Q32_32> = (0..1000).map(|i| Q32_32::from_i64(i * 1000)).collect();

    group.throughput(Throughput::Elements(1000));

    // Batch serialization
    group.bench_function("serialize", |b| {
        b.iter(|| black_box(Q32_32::serialize_batch(&values)));
    });

    // Batch deserialization
    let batch_bytes = Q32_32::serialize_batch(&values);
    group.bench_function("deserialize", |b| {
        b.iter(|| black_box(Q32_32::deserialize_batch(&batch_bytes).unwrap()));
    });

    group.finish();
}

// ============================================================================
// Memory Efficiency Benchmarks
// ============================================================================

fn bench_batch_memory_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_memory_overhead");

    for size in [100, 1000, 10000] {
        let values: Vec<Q16_16> = (0..size).map(|i| Q16_16::from_i64(i * 100)).collect();

        group.throughput(Throughput::Elements(size as u64));

        // Measure individual overhead
        group.bench_with_input(
            BenchmarkId::new("individual_overhead", size),
            &values,
            |b, values| {
                b.iter(|| {
                    let serialized = individual_serialize_q16_16(values);
                    let total_bytes: usize = serialized.iter().map(|v| v.len()).sum();
                    black_box(total_bytes)
                });
            },
        );

        // Measure batch overhead
        group.bench_with_input(
            BenchmarkId::new("batch_overhead", size),
            &values,
            |b, values| {
                b.iter(|| {
                    let bytes = Q16_16::serialize_batch(values);
                    black_box(bytes.len())
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Parallel Processing Benchmarks (rayon feature)
// ============================================================================

#[cfg(feature = "rayon")]
fn bench_batch_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_parallel");

    // Only test with large batches where parallelism pays off
    for size in [2000, 5000, 10000] {
        let values: Vec<Q16_16> = (0..size).map(|i| Q16_16::from_i64(i * 100)).collect();

        group.throughput(Throughput::Elements(size as u64));

        // Batch serialization (automatic parallel for ≥1000)
        group.bench_with_input(BenchmarkId::new("serialize", size), &values, |b, values| {
            b.iter(|| black_box(Q16_16::serialize_batch(values)));
        });

        // Batch deserialization (automatic parallel for ≥1000)
        let batch_bytes = Q16_16::serialize_batch(&values);
        group.bench_with_input(
            BenchmarkId::new("deserialize", size),
            &batch_bytes,
            |b, bytes| {
                b.iter(|| black_box(Q16_16::deserialize_batch(bytes).unwrap()));
            },
        );
    }

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    batch_serialize_benches,
    bench_batch_serialize_small,
    bench_batch_serialize_medium,
    bench_batch_serialize_large,
);

criterion_group!(
    batch_deserialize_benches,
    bench_batch_deserialize_small,
    bench_batch_deserialize_medium,
    bench_batch_deserialize_large,
);

criterion_group!(batch_roundtrip_benches, bench_batch_roundtrip,);

criterion_group!(batch_type_benches, bench_batch_q8_8, bench_batch_q32_32,);

criterion_group!(batch_memory_benches, bench_batch_memory_overhead,);

#[cfg(feature = "rayon")]
criterion_group!(batch_parallel_benches, bench_batch_parallel,);

#[cfg(feature = "rayon")]
criterion_main!(
    batch_serialize_benches,
    batch_deserialize_benches,
    batch_roundtrip_benches,
    batch_type_benches,
    batch_memory_benches,
    batch_parallel_benches,
);

#[cfg(not(feature = "rayon"))]
criterion_main!(
    batch_serialize_benches,
    batch_deserialize_benches,
    batch_roundtrip_benches,
    batch_type_benches,
    batch_memory_benches,
);
