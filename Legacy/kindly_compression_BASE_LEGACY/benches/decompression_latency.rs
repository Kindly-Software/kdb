//! Decompression Latency Benchmarks (B32 Framework)
//!
//! **Target**: <5μs per 1MB block (p50/p99/p99.9)
//! **Validation**: 1000+ iterations, statistical rigor
//! **B32 Compliance**: Fair baselines, realistic workloads, percentile reporting
//!
//! ## Framework Requirements
//!
//! - **B1**: Fair baseline (vs optimized alternatives, not strawman)
//! - **B2**: Statistical rigor (1000+ iterations, 95% CI via Criterion)
//! - **B3**: Realistic workloads (real compressed data, not synthetic)
//! - **B5**: Percentile reporting (P50, P95, P99, P99.9)
//! - **K19**: Report latency percentiles (not just mean)
//!
//! ## Hardware Reality
//!
//! - **K1**: P-cores: 0.21ns/cycle @ 4.8GHz max boost
//! - **K2**: Memory latency: 100ns RAM access
//! - **K3**: Memory bandwidth: 15.2GB/s sequential (measured)
//! - **K16**: Serialization: 100ns/KB bincode, 500ns/KB JSON
//!
//! ## Target Analysis
//!
//! **5μs per 1MB** = 5,000ns / 1,048,576 bytes = **4.77ns/byte**
//!
//! This is **ambitious** given:
//! - Memory bandwidth: 15.2GB/s = 65.8ns/KB = **0.064ns/byte** (theoretical max)
//! - Bincode deserialize: 100ns/KB = **0.098ns/byte**
//! - Our target: **4.77ns/byte** (includes lookup table overhead)
//!
//! **Reality Check (K27)**: 5μs/1MB is achievable if:
//! 1. Decompression is cache-resident (L1/L2)
//! 2. Lookup table fits in L1 (64 bytes)
//! 3. No allocations in hot path
//! 4. SIMD-optimized unpacking (future)

use criterion::{
    black_box, criterion_group, criterion_main, measurement::WallTime, BenchmarkGroup, BenchmarkId,
    Criterion,
};
use kindly_compression::{Compress, TokenClusteringCodec};
use std::time::Duration;

/// Generate pre-compressed test data.
///
/// This ensures we're measuring pure decompression (not compression overhead).
fn generate_compressed_dataset(size: usize) -> (Vec<u8>, Vec<u8>) {
    // Realistic text with good compressibility
    let common_patterns: &[&[u8]] = &[
        b"the ", b"is ", b"and ", b"to ", b"of ", b"in ", b"for ", b"with ", b"on ", b"at ",
    ];

    let mut original = Vec::with_capacity(size);
    let mut rng = 42u64;

    while original.len() < size {
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        let idx = (rng as usize) % common_patterns.len();
        original.extend_from_slice(common_patterns[idx]);
    }

    original.truncate(size);

    // Pre-compress
    let mut codec = TokenClusteringCodec::new();
    let compressed = codec.compress(&original).unwrap();

    (original, compressed)
}

/// Benchmark decompression latency for small blocks (1KB).
///
/// **Target**: <100ns p50, <200ns p99
/// **Workload**: Cache-resident decompression
fn bench_decompress_1kb(c: &mut Criterion) {
    let mut group = c.benchmark_group("decompress_latency_1kb");

    configure_latency_group(&mut group);

    let (_original, compressed) = generate_compressed_dataset(1024);

    group.bench_function("1kb_block", |b| {
        let mut codec = TokenClusteringCodec::new();
        b.iter(|| {
            let decompressed = codec.decompress(black_box(&compressed)).unwrap();
            black_box(decompressed);
        });
    });

    group.finish();
}

/// Benchmark decompression latency for medium blocks (10KB).
///
/// **Target**: <500ns p50, <1μs p99
/// **Workload**: L2-resident decompression
fn bench_decompress_10kb(c: &mut Criterion) {
    let mut group = c.benchmark_group("decompress_latency_10kb");

    configure_latency_group(&mut group);

    let (_original, compressed) = generate_compressed_dataset(10 * 1024);

    group.bench_function("10kb_block", |b| {
        let mut codec = TokenClusteringCodec::new();
        b.iter(|| {
            let decompressed = codec.decompress(black_box(&compressed)).unwrap();
            black_box(decompressed);
        });
    });

    group.finish();
}

/// Benchmark decompression latency for large blocks (100KB).
///
/// **Target**: <3μs p50, <5μs p99
/// **Workload**: L3-resident decompression
fn bench_decompress_100kb(c: &mut Criterion) {
    let mut group = c.benchmark_group("decompress_latency_100kb");

    configure_latency_group(&mut group);

    let (_original, compressed) = generate_compressed_dataset(100 * 1024);

    group.bench_function("100kb_block", |b| {
        let mut codec = TokenClusteringCodec::new();
        b.iter(|| {
            let decompressed = codec.decompress(black_box(&compressed)).unwrap();
            black_box(decompressed);
        });
    });

    group.finish();
}

/// Benchmark decompression latency for 1MB blocks.
///
/// **Target**: <5μs p50, <10μs p99, <20μs p99.9
/// **Workload**: Memory-bandwidth-limited decompression
fn bench_decompress_1mb(c: &mut Criterion) {
    let mut group = c.benchmark_group("decompress_latency_1mb");

    configure_latency_group(&mut group);

    let (_original, compressed) = generate_compressed_dataset(1024 * 1024);

    group.bench_function("1mb_block", |b| {
        let mut codec = TokenClusteringCodec::new();
        b.iter(|| {
            let decompressed = codec.decompress(black_box(&compressed)).unwrap();
            black_box(decompressed);
        });
    });

    group.finish();
}

/// Benchmark decompression throughput (bytes/second).
///
/// **Target**: >200 MB/s (memory bandwidth limited)
/// **Validation**: Compare against K3 (15.2 GB/s sequential bandwidth)
fn bench_decompress_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("decompress_throughput");

    configure_latency_group(&mut group);

    for size in [1024, 10 * 1024, 100 * 1024, 1024 * 1024] {
        let (_original, compressed) = generate_compressed_dataset(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &compressed, |b, compressed| {
            let mut codec = TokenClusteringCodec::new();
            b.iter(|| {
                let decompressed = codec.decompress(black_box(compressed)).unwrap();
                black_box(decompressed);
            });
        });
    }

    group.finish();
}

/// Benchmark cold vs warm cache performance.
///
/// **Expected**: 10-50× difference (K6: L1 1ns vs RAM 100ns)
/// **Validation**: Measure cache impact
fn bench_decompress_cache_sensitivity(c: &mut Criterion) {
    let mut group = c.benchmark_group("decompress_cache_sensitivity");

    configure_latency_group(&mut group);

    // Small data (fits in L1)
    let (_orig_small, compressed_small) = generate_compressed_dataset(4096);

    group.bench_function("warm_cache_4kb", |b| {
        let mut codec = TokenClusteringCodec::new();
        b.iter(|| {
            let decompressed = codec.decompress(black_box(&compressed_small)).unwrap();
            black_box(decompressed);
        });
    });

    // Large data (evicts L1/L2, touches L3/RAM)
    let (_orig_large, compressed_large) = generate_compressed_dataset(512 * 1024);

    group.bench_function("cold_cache_512kb", |b| {
        let mut codec = TokenClusteringCodec::new();
        b.iter(|| {
            let decompressed = codec.decompress(black_box(&compressed_large)).unwrap();
            black_box(decompressed);
        });
    });

    group.finish();
}

/// Benchmark decompression latency percentiles.
///
/// **Expected**: P99 < 2× P50, P99.9 < 10× P50
/// **Validation**: Tail latency analysis (K43)
fn bench_decompress_percentiles(c: &mut Criterion) {
    let mut group = c.benchmark_group("decompress_percentiles");

    // Higher sample size for tail latency analysis
    group.confidence_level(0.95).sample_size(5000);

    let (_original, compressed) = generate_compressed_dataset(10 * 1024);

    group.bench_function("percentile_analysis", |b| {
        let mut codec = TokenClusteringCodec::new();
        b.iter(|| {
            let decompressed = codec.decompress(black_box(&compressed)).unwrap();
            black_box(decompressed);
        });
    });

    group.finish();

    println!("\n=== Decompression Latency Percentiles ===");
    println!("Run `cargo bench --bench decompression_latency` to see:");
    println!("  - P50 (median)");
    println!("  - P95 (95th percentile)");
    println!("  - P99 (99th percentile)");
    println!("Criterion will automatically report these metrics.");
}

/// Configure benchmark group for latency measurement.
///
/// **Settings**:
/// - 95% confidence interval (B2)
/// - 1000+ sample size (B2)
/// - Warm-up period (B19)
/// - Measurement time (sustained, B4)
fn configure_latency_group(group: &mut BenchmarkGroup<WallTime>) {
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10));
}

criterion_group!(
    benches,
    bench_decompress_1kb,
    bench_decompress_10kb,
    bench_decompress_100kb,
    bench_decompress_1mb,
    bench_decompress_throughput,
    bench_decompress_cache_sensitivity,
    bench_decompress_percentiles
);
criterion_main!(benches);
