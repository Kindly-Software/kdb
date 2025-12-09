//! B32 Benchmarks for CRC64SimdCapsule
//!
//! Validates performance claims for the T2 SIMD CRC64 slice-by-8 implementation.
//!
//! # Performance Targets
//!
//! | Input Size | Target | Throughput |
//! |------------|--------|------------|
//! | 2KB        | <100ns | >20 GB/s   |
//! | 1KB        | <50ns  | >20 GB/s   |
//! | 64B        | <10ns  | >6 GB/s    |
//!
//! # B32 Framework Compliance
//!
//! - Fair baseline: Compare against byte-at-a-time implementation
//! - 95% CI: Criterion provides confidence intervals
//! - Reproducibility: Deterministic inputs, multiple iterations
//! - Realistic targets: Based on slice-by-8 algorithm analysis

use atomic_capsule::hash::{crc64_hash, CRC64SimdCapsule};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

/// Byte-at-a-time reference implementation for fair comparison
fn crc64_reference(data: &[u8]) -> u64 {
    const POLY: u64 = 0x42F0E1EBA9EA3693;
    let mut crc: u64 = 0;

    for &byte in data {
        crc ^= (byte as u64) << 56;
        for _ in 0..8 {
            if crc & 0x8000000000000000 != 0 {
                crc = (crc << 1) ^ POLY;
            } else {
                crc <<= 1;
            }
        }
    }

    crc
}

/// Benchmark various input sizes
fn bench_crc64_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("CRC64-SIMD");

    // Test sizes: 64B, 256B, 512B, 1KB, 2KB, 4KB
    for size in [64, 256, 512, 1024, 2048, 4096] {
        let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

        group.throughput(Throughput::Bytes(size as u64));

        // Slice-by-8 implementation
        group.bench_with_input(BenchmarkId::new("slice-by-8", size), &data, |b, data| {
            b.iter(|| black_box(crc64_hash(black_box(data))))
        });

        // Reference implementation (byte-at-a-time)
        group.bench_with_input(BenchmarkId::new("reference", size), &data, |b, data| {
            b.iter(|| black_box(crc64_reference(black_box(data))))
        });
    }

    group.finish();
}

/// Benchmark capsule API
fn bench_crc64_capsule(c: &mut Criterion) {
    let mut group = c.benchmark_group("CRC64-Capsule-API");

    // Test incremental hashing
    let chunk_size = 512;
    let total_size = 4096;
    let data: Vec<u8> = (0..total_size).map(|i| (i % 256) as u8).collect();

    group.throughput(Throughput::Bytes(total_size as u64));

    // Single-shot
    group.bench_function("single-shot-4KB", |b| {
        b.iter(|| black_box(CRC64SimdCapsule::hash_once(black_box(&data))))
    });

    // Incremental (8 chunks of 512B)
    group.bench_function("incremental-8x512B", |b| {
        b.iter(|| {
            let capsule = CRC64SimdCapsule::new();
            for chunk in data.chunks(chunk_size) {
                capsule.update(black_box(chunk));
            }
            black_box(capsule.finalize())
        })
    });

    group.finish();
}

/// Benchmark embedding hashing (512 f32 = 2KB)
fn bench_crc64_embedding(c: &mut Criterion) {
    let mut group = c.benchmark_group("CRC64-Embedding");

    let embedding = [0.5f32; 512];

    group.throughput(Throughput::Bytes(512 * 4)); // 2KB

    group.bench_function("512-f32-embedding", |b| {
        b.iter(|| black_box(CRC64SimdCapsule::hash_embedding(black_box(&embedding))))
    });

    group.finish();
}

/// Benchmark small inputs (where overhead matters)
fn bench_crc64_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("CRC64-Small-Inputs");

    for size in [1, 4, 7, 8, 9, 15, 16, 31, 32] {
        let data: Vec<u8> = (0..size).map(|i| i as u8).collect();

        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("slice-by-8", size), &data, |b, data| {
            b.iter(|| black_box(crc64_hash(black_box(data))))
        });
    }

    group.finish();
}

/// Benchmark throughput (large input)
fn bench_crc64_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("CRC64-Throughput");

    // 1MB input for throughput measurement
    let size = 1024 * 1024;
    let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

    group.throughput(Throughput::Bytes(size as u64));
    group.sample_size(50); // Fewer samples for large input

    group.bench_function("1MB", |b| {
        b.iter(|| black_box(crc64_hash(black_box(&data))))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_crc64_sizes,
    bench_crc64_capsule,
    bench_crc64_embedding,
    bench_crc64_small,
    bench_crc64_throughput,
);

criterion_main!(benches);
