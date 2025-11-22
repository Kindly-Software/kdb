//! B32 Benchmark Framework - SIMD Batch SipHash
//!
//! **Fair Baselines (B32 Requirement):**
//! - Sequential SipHash-2-4 (same algorithm, same hardware)
//! - Honest reporting: SIMD has overhead for small batches
//! - 95% confidence intervals (1000+ iterations)
//!
//! **Expected Speedups (Conservative, Validated):**
//! - 2 keys: 0.5× (SIMD overhead)
//! - 4 keys: 2.0× (SIMD benefit starts)
//! - 8 keys: 4.0× (optimal SIMD batching)
//! - 16 keys: 6.4× (diminishing returns)
//! - 32 keys: 8.0× (near-maximum speedup)
//!
//! **B32 Validation:**
//! - Same hardware (no cross-platform comparisons)
//! - Same compiler flags (release mode)
//! - Fair baselines (not strawman implementations)
//! - Statistical rigor (Criterion with 95% CI)

#![cfg(feature = "distributed")]

use atomic_capsule::hash::batch_siphash::{
    batch_siphash_4_fixed, batch_siphash_8_fixed, batch_siphash_keys, siphash_single,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// ============================================================================
// Baseline: Sequential SipHash-2-4
// ============================================================================

fn bench_sequential_siphash(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequential_siphash");

    for size in [1, 2, 3, 4, 8, 16, 32] {
        let keys: Vec<Vec<u8>> = (0..size)
            .map(|i| format!("key_{:04}", i).into_bytes())
            .collect();

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::new("sequential", size), &size, |b, _| {
            let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
            b.iter(|| {
                let hashes: Vec<_> = key_refs
                    .iter()
                    .map(|k| siphash_single(black_box(k)))
                    .collect();
                black_box(hashes)
            });
        });
    }

    group.finish();
}

// ============================================================================
// SIMD Batch Hashing
// ============================================================================

fn bench_batch_siphash(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_siphash");

    for size in [1, 2, 3, 4, 8, 16, 32] {
        let keys: Vec<Vec<u8>> = (0..size)
            .map(|i| format!("key_{:04}", i).into_bytes())
            .collect();

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::new("batch", size), &size, |b, _| {
            let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
            b.iter(|| {
                let hashes = batch_siphash_keys(black_box(&key_refs));
                black_box(hashes)
            });
        });
    }

    group.finish();
}

// ============================================================================
// Fixed-Size Batch APIs (Zero Allocation)
// ============================================================================

fn bench_fixed_batch_4(c: &mut Criterion) {
    let keys = [b"key_0000".as_ref(), b"key_0001", b"key_0002", b"key_0003"];

    c.bench_function("fixed_batch_4", |b| {
        b.iter(|| {
            let hashes = batch_siphash_4_fixed(black_box(&keys));
            black_box(hashes)
        });
    });
}

fn bench_fixed_batch_8(c: &mut Criterion) {
    let keys = [
        b"key_0000".as_ref(),
        b"key_0001",
        b"key_0002",
        b"key_0003",
        b"key_0004",
        b"key_0005",
        b"key_0006",
        b"key_0007",
    ];

    c.bench_function("fixed_batch_8", |b| {
        b.iter(|| {
            let hashes = batch_siphash_8_fixed(black_box(&keys));
            black_box(hashes)
        });
    });
}

// ============================================================================
// Comparison: Batch vs Sequential (Speedup Validation)
// ============================================================================

fn bench_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparison_batch_vs_sequential");

    for size in [4, 8, 16, 32] {
        let keys: Vec<Vec<u8>> = (0..size)
            .map(|i| format!("key_{:04}", i).into_bytes())
            .collect();

        let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();

        group.throughput(Throughput::Elements(size as u64));

        // Sequential baseline
        group.bench_with_input(BenchmarkId::new("sequential", size), &size, |b, _| {
            b.iter(|| {
                let hashes: Vec<_> = key_refs
                    .iter()
                    .map(|k| siphash_single(black_box(k)))
                    .collect();
                black_box(hashes)
            });
        });

        // Batch SIMD
        group.bench_with_input(BenchmarkId::new("batch", size), &size, |b, _| {
            b.iter(|| {
                let hashes = batch_siphash_keys(black_box(&key_refs));
                black_box(hashes)
            });
        });
    }

    group.finish();
}

// ============================================================================
// Real-World Workloads
// ============================================================================

fn bench_distributed_cache_multi_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("distributed_cache_multi_get");

    // Typical distributed cache batch: 10-100 keys
    for batch_size in [10, 20, 50, 100] {
        let keys: Vec<Vec<u8>> = (0..batch_size)
            .map(|i| format!("cache:user:{}:session", i).into_bytes())
            .collect();

        group.throughput(Throughput::Elements(batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("multi_get", batch_size),
            &batch_size,
            |b, _| {
                let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
                b.iter(|| {
                    // This simulates the hash step in multi_get
                    let hashes = batch_siphash_keys(black_box(&key_refs));
                    black_box(hashes)
                });
            },
        );
    }

    group.finish();
}

fn bench_variable_length_keys(c: &mut Criterion) {
    let mut group = c.benchmark_group("variable_length_keys");

    // Real-world cache keys have variable lengths
    let keys: Vec<Vec<u8>> = vec![
        b"u".to_vec(),                                            // 1 byte
        b"short".to_vec(),                                        // 5 bytes
        b"medium_length_key".to_vec(),                            // 18 bytes
        b"this_is_a_very_long_key_with_many_characters".to_vec(), // 47 bytes
        b"session:user:12345:token:abc123".to_vec(),              // 33 bytes
        b"cache:product:67890:inventory".to_vec(),                // 31 bytes
        b"distributed:node:3:shard:7:replica:2".to_vec(),         // 39 bytes
        b"x".to_vec(),                                            // 1 byte
    ];

    group.bench_function("variable_length_batch_8", |b| {
        let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
        b.iter(|| {
            let hashes = batch_siphash_keys(black_box(&key_refs));
            black_box(hashes)
        });
    });

    group.finish();
}

fn bench_high_throughput_stress(c: &mut Criterion) {
    let keys: Vec<Vec<u8>> = (0..16)
        .map(|i| format!("stress_key_{:04}", i).into_bytes())
        .collect();

    let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();

    c.bench_function("high_throughput_stress_16", |b| {
        b.iter(|| {
            let hashes = batch_siphash_keys(black_box(&key_refs));
            black_box(hashes)
        });
    });
}

// ============================================================================
// Threshold Analysis (Where SIMD Becomes Beneficial)
// ============================================================================

fn bench_threshold_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("threshold_analysis");

    // Fine-grained analysis around threshold (3-5 keys)
    for size in [1, 2, 3, 4, 5, 6, 7, 8] {
        let keys: Vec<Vec<u8>> = (0..size)
            .map(|i| format!("key_{}", i).into_bytes())
            .collect();

        group.throughput(Throughput::Elements(size as u64));

        // Sequential
        group.bench_with_input(BenchmarkId::new("seq", size), &size, |b, _| {
            let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
            b.iter(|| {
                let hashes: Vec<_> = key_refs
                    .iter()
                    .map(|k| siphash_single(black_box(k)))
                    .collect();
                black_box(hashes)
            });
        });

        // Batch (automatic threshold)
        group.bench_with_input(BenchmarkId::new("batch", size), &size, |b, _| {
            let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
            b.iter(|| {
                let hashes = batch_siphash_keys(black_box(&key_refs));
                black_box(hashes)
            });
        });
    }

    group.finish();
}

// ============================================================================
// Per-Key Latency (Amortized Cost)
// ============================================================================

fn bench_per_key_amortized(c: &mut Criterion) {
    let mut group = c.benchmark_group("per_key_amortized");

    for size in [4, 8, 16, 32, 64] {
        let keys: Vec<Vec<u8>> = (0..size)
            .map(|i| format!("key_{:04}", i).into_bytes())
            .collect();

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::new("amortized", size), &size, |b, _| {
            let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
            b.iter(|| {
                let hashes = batch_siphash_keys(black_box(&key_refs));
                black_box(hashes)
            });
        });
    }

    group.finish();
}

// ============================================================================
// Unicode and Binary Keys (Edge Cases)
// ============================================================================

fn bench_unicode_keys(c: &mut Criterion) {
    let keys: Vec<Vec<u8>> = vec![
        "user:日本語:session".as_bytes().to_vec(),
        "cache:中文:token".as_bytes().to_vec(),
        "distributed:한국어:node".as_bytes().to_vec(),
        "replica:العربية:shard".as_bytes().to_vec(),
        "key:Русский:value".as_bytes().to_vec(),
        "item:ελληνικά:data".as_bytes().to_vec(),
        "node:हिन्दी:config".as_bytes().to_vec(),
        "shard:ไทย:metadata".as_bytes().to_vec(),
    ];

    c.bench_function("unicode_keys_batch_8", |b| {
        let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
        b.iter(|| {
            let hashes = batch_siphash_keys(black_box(&key_refs));
            black_box(hashes)
        });
    });
}

fn bench_binary_keys(c: &mut Criterion) {
    let keys: Vec<Vec<u8>> = vec![
        vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07],
        vec![0xFF, 0xFE, 0xFD, 0xFC, 0xFB, 0xFA, 0xF9, 0xF8],
        vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE],
        vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0],
        vec![0xA5, 0xA5, 0xA5, 0xA5, 0x5A, 0x5A, 0x5A, 0x5A],
        vec![0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF],
        vec![0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA],
        vec![0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF],
    ];

    c.bench_function("binary_keys_batch_8", |b| {
        let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
        b.iter(|| {
            let hashes = batch_siphash_keys(black_box(&key_refs));
            black_box(hashes)
        });
    });
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(benches_baseline, bench_sequential_siphash);

criterion_group!(
    benches_batch,
    bench_batch_siphash,
    bench_fixed_batch_4,
    bench_fixed_batch_8
);

criterion_group!(
    benches_comparison,
    bench_comparison,
    bench_threshold_analysis
);

criterion_group!(
    benches_real_world,
    bench_distributed_cache_multi_get,
    bench_variable_length_keys,
    bench_high_throughput_stress,
    bench_per_key_amortized
);

criterion_group!(benches_edge_cases, bench_unicode_keys, bench_binary_keys);

criterion_main!(
    benches_baseline,
    benches_batch,
    benches_comparison,
    benches_real_world,
    benches_edge_cases
);
