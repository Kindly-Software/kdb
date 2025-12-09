//! Profiling benchmark for insert operation
//! Designed to run with `perf record` for flamegraph analysis
//!
//! Run with:
//!   cargo bench --bench insert_profile -- --profile-time=10

use atomic_capsule_map::AtomicCapsuleMap;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

fn bench_insert_single_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_profile");

    // Configure for profiling (longer runs, fewer samples)
    group
        .confidence_level(0.95)
        .sample_size(100) // Fewer samples for profiling
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(10)); // Long measurement for perf

    // Test 1: Empty map insert (minimal probing, focus on publish cost)
    group.bench_function("empty_map", |b| {
        b.iter_batched(
            || AtomicCapsuleMap::<u64, u64>::with_capacity(4096),
            |map| {
                for i in 0..1000u64 {
                    black_box(map.insert(i, i * 2));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Test 2: Pre-filled map (50% load, realistic probing)
    group.bench_function("prefilled_50pct", |b| {
        b.iter_batched(
            || {
                let map = AtomicCapsuleMap::<u64, u64>::with_capacity(4096);
                for i in 0..2048 {
                    map.insert(i, i * 2);
                }
                map
            },
            |map| {
                for i in 10000..11000u64 {
                    black_box(map.insert(i, i * 2));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Test 3: High contention scenario (80% load, max probing)
    group.bench_function("high_load_80pct", |b| {
        b.iter_batched(
            || {
                let map = AtomicCapsuleMap::<u64, u64>::with_capacity(4096);
                for i in 0..3276 {
                    map.insert(i, i * 2);
                }
                map
            },
            |map| {
                for i in 10000..11000u64 {
                    black_box(map.insert(i, i * 2));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_insert_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_components");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(1));

    // Isolate hash computation cost
    group.bench_function("hash_only", |b| {
        use ahash::AHasher;
        use std::hash::{Hash, Hasher};

        let mut key = 0u64;
        b.iter(|| {
            key = key.wrapping_add(1);
            let mut hasher = AHasher::default();
            black_box(&key).hash(&mut hasher);
            black_box(hasher.finish());
        });
    });

    // Isolate probe cost (linear search simulation)
    group.bench_function("probe_simulation", |b| {
        let map = AtomicCapsuleMap::<u64, u64>::with_capacity(4096);
        // Fill to 50% to simulate realistic probing
        for i in 0..2048 {
            map.insert(i, i * 2);
        }

        b.iter(|| {
            // Simulate probe loop (read-only, no insert)
            for i in 10000..10100u64 {
                black_box(map.get(&i));
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_insert_single_threaded,
    bench_insert_components
);
criterion_main!(benches);
