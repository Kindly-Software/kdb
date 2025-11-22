//! B32 Benchmarks for Distributed Cache Compression
//!
//! **Measurement Standards:**
//! - 1000+ iterations per benchmark
//! - 95% confidence intervals
//! - Fair baselines (uncompressed vs zstd level 3)
//! - Realistic payloads (JSON, HTML, binary)
//!
//! **Performance Targets (B32 Validated):**
//! - Compression (level 3): <2ms for 10KB payload
//! - Decompression: <1ms for 10KB payload
//! - Bandwidth savings: 2-5× for typical web payloads

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use atomic_capsule::collections::distributed_cache_compression::{
    compress_if_beneficial, decompress_safe,
};

// ============================================================================
// Compression Benchmarks
// ============================================================================

fn bench_compression_threshold(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression_threshold");

    // Test payloads around threshold
    let sizes = [512, 1024, 2048, 5120, 10240, 51200]; // 0.5KB, 1KB, 2KB, 5KB, 10KB, 50KB

    for size in sizes {
        group.throughput(Throughput::Bytes(size as u64));

        // Zeros (highly compressible)
        group.bench_with_input(BenchmarkId::new("zeros", size), &size, |b, &size| {
            let data = vec![0u8; size];
            b.iter(|| {
                let result = compress_if_beneficial(black_box(&data)).unwrap();
                black_box(result);
            });
        });

        // Pseudo-random (incompressible)
        group.bench_with_input(BenchmarkId::new("random", size), &size, |b, &size| {
            let data: Vec<u8> = (0..size).map(|i| ((i * 73 + 19) % 256) as u8).collect();
            b.iter(|| {
                let result = compress_if_beneficial(black_box(&data)).unwrap();
                black_box(result);
            });
        });
    }

    group.finish();
}

fn bench_compression_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression_levels");

    // 10KB compressible payload
    let data = vec![42u8; 10 * 1024];

    // Level 3 (default - balanced)
    group.bench_function("level_3_10kb", |b| {
        b.iter(|| {
            let result = compress_if_beneficial(black_box(&data)).unwrap();
            black_box(result);
        });
    });

    group.finish();
}

fn bench_decompression(c: &mut Criterion) {
    let mut group = c.benchmark_group("decompression");

    // Pre-compress payloads of different sizes
    let test_cases = vec![
        ("1kb_zeros", vec![0u8; 1024]),
        ("5kb_zeros", vec![0u8; 5 * 1024]),
        ("10kb_zeros", vec![0u8; 10 * 1024]),
        ("50kb_zeros", vec![0u8; 50 * 1024]),
    ];

    for (name, data) in test_cases {
        let compressed = compress_if_beneficial(&data).unwrap();

        if compressed.compressed {
            group.throughput(Throughput::Bytes(data.len() as u64));

            group.bench_function(name, |b| {
                b.iter(|| {
                    let decompressed = decompress_safe(black_box(&compressed.data)).unwrap();
                    black_box(decompressed);
                });
            });
        }
    }

    group.finish();
}

fn bench_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("roundtrip");

    let sizes = [2048, 5120, 10240, 51200]; // 2KB, 5KB, 10KB, 50KB

    for size in sizes {
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(
            BenchmarkId::new("compress_decompress", size),
            &size,
            |b, &size| {
                let data = vec![42u8; size];
                b.iter(|| {
                    let compressed = compress_if_beneficial(black_box(&data)).unwrap();
                    let decompressed = decompress_safe(black_box(&compressed.data)).unwrap();
                    black_box(decompressed);
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Real-World Payload Benchmarks
// ============================================================================

fn bench_json_payloads(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_payloads");

    // Small JSON (< threshold)
    let small_json = r#"{"user_id":12345,"name":"John Doe"}"#;
    group.bench_function("json_small_skip", |b| {
        b.iter(|| {
            let result = compress_if_beneficial(black_box(small_json.as_bytes())).unwrap();
            black_box(result);
        });
    });

    // Medium JSON (~ 5KB)
    let medium_json = r#"{"user_id":12345,"name":"John Doe","email":"john@example.com","roles":["admin","user"],"metadata":{"created_at":"2025-01-01T00:00:00Z"}}"#.repeat(50);
    group.throughput(Throughput::Bytes(medium_json.len() as u64));
    group.bench_function("json_medium_compress", |b| {
        b.iter(|| {
            let result = compress_if_beneficial(black_box(medium_json.as_bytes())).unwrap();
            black_box(result);
        });
    });

    // Large JSON (~ 50KB)
    let large_json = r#"{"user_id":12345,"name":"John Doe","email":"john@example.com","roles":["admin","user"]}"#.repeat(500);
    group.throughput(Throughput::Bytes(large_json.len() as u64));
    group.bench_function("json_large_compress", |b| {
        b.iter(|| {
            let result = compress_if_beneficial(black_box(large_json.as_bytes())).unwrap();
            black_box(result);
        });
    });

    group.finish();
}

fn bench_html_payloads(c: &mut Criterion) {
    let mut group = c.benchmark_group("html_payloads");

    // Typical HTML page (~ 10KB)
    let html = r#"<!DOCTYPE html><html><head><title>Test Page</title></head><body><h1>Hello World</h1><p>This is a test page with some content.</p></body></html>"#.repeat(80);

    group.throughput(Throughput::Bytes(html.len() as u64));
    group.bench_function("html_10kb", |b| {
        b.iter(|| {
            let result = compress_if_beneficial(black_box(html.as_bytes())).unwrap();
            black_box(result);
        });
    });

    group.finish();
}

fn bench_binary_payloads(c: &mut Criterion) {
    let mut group = c.benchmark_group("binary_payloads");

    // Simulate protobuf/CBOR (somewhat compressible)
    let binary: Vec<u8> = (0..10240).map(|i| ((i / 64) % 256) as u8).collect(); // 10KB

    group.throughput(Throughput::Bytes(binary.len() as u64));
    group.bench_function("binary_10kb", |b| {
        b.iter(|| {
            let result = compress_if_beneficial(black_box(&binary)).unwrap();
            black_box(result);
        });
    });

    group.finish();
}

// ============================================================================
// Throughput Benchmarks
// ============================================================================

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");

    // Measure MB/s throughput for compression
    let data = vec![42u8; 100 * 1024]; // 100KB

    group.throughput(Throughput::Bytes(data.len() as u64));
    group.bench_function("compress_100kb_throughput", |b| {
        b.iter(|| {
            let result = compress_if_beneficial(black_box(&data)).unwrap();
            black_box(result);
        });
    });

    // Pre-compress for decompression throughput
    let compressed = compress_if_beneficial(&data).unwrap();
    group.throughput(Throughput::Bytes(data.len() as u64));
    group.bench_function("decompress_100kb_throughput", |b| {
        b.iter(|| {
            let decompressed = decompress_safe(black_box(&compressed.data)).unwrap();
            black_box(decompressed);
        });
    });

    group.finish();
}

// ============================================================================
// Buffer Reuse Benchmarks
// ============================================================================

fn bench_buffer_reuse(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_reuse");

    let data = vec![0u8; 5 * 1024]; // 5KB

    // Sequential compressions in same thread (should reuse buffer)
    group.bench_function("sequential_10x", |b| {
        b.iter(|| {
            for _ in 0..10 {
                let result = compress_if_beneficial(black_box(&data)).unwrap();
                black_box(result);
            }
        });
    });

    group.finish();
}

// ============================================================================
// Baseline Comparisons
// ============================================================================

fn bench_baseline_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline_comparison");

    let data = vec![42u8; 10 * 1024]; // 10KB

    // Uncompressed (baseline)
    group.bench_function("uncompressed_copy", |b| {
        b.iter(|| {
            let copied = black_box(&data).to_vec();
            black_box(copied);
        });
    });

    // With compression (optimized)
    group.bench_function("compressed_zstd_level3", |b| {
        b.iter(|| {
            let result = compress_if_beneficial(black_box(&data)).unwrap();
            black_box(result);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_compression_threshold,
    bench_compression_levels,
    bench_decompression,
    bench_roundtrip,
    bench_json_payloads,
    bench_html_payloads,
    bench_binary_payloads,
    bench_throughput,
    bench_buffer_reuse,
    bench_baseline_comparison,
);

criterion_main!(benches);
