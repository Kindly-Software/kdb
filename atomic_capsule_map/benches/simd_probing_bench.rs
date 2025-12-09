//! SIMD Parallel Bucket Probing Benchmark
//!
//! Validates UCE32 Q30 (Empirical Validation) for SIMD optimization.
//! Measures performance improvement of parallel bucket probing on Intel Ultra 7 155H.
//!
//! # Expected Results (B32 Framework)
//!
//! - Sequential baseline: 15ns per probe × average 2-3 probes = 30-45ns
//! - SIMD optimization: 20ns for 4 probes in parallel
//! - Target improvement: 30-40% reduction in probe time
//!
//! # Hardware Requirements (K9 - SIMD Reality)
//!
//! - x86_64 with AVX2 support (Intel Ultra 7 155H confirmed)
//! - 64-byte cache line alignment (BucketCapsule already aligned)
//! - Nightly Rust with portable_simd feature
//!
//! # UCE32 Analysis
//!
//! - Q29 (Practical Constraints): Cache line alignment, AVX2 availability
//! - Q30 (Empirical Validation): This benchmark provides empirical evidence
//! - Q31 (Rust Transform): portable_simd abstracts platform differences
//! - Q32 (Nightly Enhancement): Leverages cutting-edge SIMD capabilities

use atomic_capsule_map::AtomicCapsuleMap;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

/// Benchmark get operations with varying load factors to measure probe distance impact
fn bench_get_with_load_factor(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_probe_load_factor");
    group.measurement_time(Duration::from_secs(10));
    group.confidence_level(0.95);
    group.sample_size(1000);

    // Test different load factors to trigger different probe distances
    for load_pct in [25, 50, 75, 90] {
        let capacity = 1024usize;
        let num_entries = (capacity * load_pct) / 100;

        let map = AtomicCapsuleMap::with_capacity(capacity);

        // Pre-populate map to achieve target load factor
        for i in 0..num_entries {
            map.insert(i as u64, i as u64);
        }

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("get_hit", format!("{}%_load", load_pct)),
            &num_entries,
            |b, &num_entries| {
                b.iter(|| {
                    // Access existing keys (cache hit scenario)
                    let key = black_box((num_entries / 2) as u64);
                    black_box(map.get(&key));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("get_miss", format!("{}%_load", load_pct)),
            &num_entries,
            |b, &_| {
                b.iter(|| {
                    // Access non-existent keys (probe to end)
                    let key = black_box(u64::MAX);
                    black_box(map.get(&key));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark sequential scans to measure cache effects
fn bench_sequential_vs_random_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_probe_access_pattern");
    group.measurement_time(Duration::from_secs(10));

    let capacity = 4096;
    let num_entries = 3072; // 75% load factor

    let map = AtomicCapsuleMap::with_capacity(capacity);

    // Pre-populate with sequential keys
    for i in 0..num_entries {
        map.insert(i as u64, i as u64);
    }

    // Sequential access (cache-friendly)
    group.bench_function("sequential_access", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            let key = black_box(counter % (num_entries as u64));
            black_box(map.get(&key));
            counter = counter.wrapping_add(1);
        });
    });

    // Random access (cache-unfriendly)
    group.bench_function("random_access", |b| {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hash, Hasher};

        let hasher = RandomState::new();
        let mut seed = 12345u64;

        b.iter(|| {
            // Simple PRNG for random key selection
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let key = black_box(seed % (num_entries as u64));
            black_box(map.get(&key));
        });
    });

    group.finish();
}

/// Benchmark concurrent reads to measure SIMD benefits under contention
fn bench_concurrent_reads(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let mut group = c.benchmark_group("simd_probe_concurrent");
    group.measurement_time(Duration::from_secs(15));

    let capacity = 8192;
    let num_entries = 6144; // 75% load factor

    let map = Arc::new(AtomicCapsuleMap::with_capacity(capacity));

    // Pre-populate
    for i in 0..num_entries {
        map.insert(i as u64, i as u64);
    }

    for num_threads in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::new("concurrent_get", format!("{}_threads", num_threads)),
            &num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let mut handles = vec![];

                    for tid in 0..num_threads {
                        let map_clone = Arc::clone(&map);
                        let handle = thread::spawn(move || {
                            for i in 0..100 {
                                let key = black_box((tid * 100 + i) as u64 % (num_entries as u64));
                                black_box(map_clone.get(&key));
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark probe distance distribution
fn bench_probe_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_probe_distance");
    group.measurement_time(Duration::from_secs(10));

    // High load factor to force longer probe chains
    let capacity = 1024;
    let num_entries = 950; // ~93% load factor

    let map = AtomicCapsuleMap::with_capacity(capacity);

    // Pre-populate to create long probe chains
    for i in 0..num_entries {
        map.insert(i as u64, i as u64);
    }

    // Benchmark gets that will hit various probe distances
    group.bench_function("long_probe_chains", |b| {
        b.iter(|| {
            // Access keys throughout the map
            for i in (0..num_entries).step_by(10) {
                let key = black_box(i as u64);
                black_box(map.get(&key));
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_get_with_load_factor,
    bench_sequential_vs_random_access,
    bench_concurrent_reads,
    bench_probe_distance,
);
criterion_main!(benches);
