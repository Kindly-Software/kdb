//! Arc<T> performance benchmarks
//!
//! Measures Arc<T> support overhead and validates mutex elimination benefits
//! Following B32 framework: Fair baselines, statistical rigor, real workloads
//!
//! NOTE: Requires `arc_support` feature - AtomicCapsuleMap currently requires V: Copy
//! To enable: `cargo bench --features arc_support arc_performance`

#![cfg(all(feature = "std", feature = "arc_support"))]

use atomic_capsule_map::AtomicCapsuleMap;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::sync::Arc;
use std::time::Duration;

fn bench_arc_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("arc_insert");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Arc<Vec<u8>> insert (typical use case)
    group.bench_function("arc_vec_small", |b| {
        let map = AtomicCapsuleMap::new();
        let mut key = 0u64;

        b.iter(|| {
            key = key.wrapping_add(1);
            let value = Arc::new(vec![1u8, 2, 3, 4]);
            black_box(map.insert(key, value));
        });
    });

    // Arc<Vec<u8>> with larger payload
    group.bench_function("arc_vec_medium", |b| {
        let map = AtomicCapsuleMap::new();
        let mut key = 0u64;

        b.iter(|| {
            key = key.wrapping_add(1);
            let value = Arc::new(vec![0u8; 64]);
            black_box(map.insert(key, value));
        });
    });

    // Arc<String> insert (another common use case)
    group.bench_function("arc_string", |b| {
        let map = AtomicCapsuleMap::new();
        let mut key = 0u64;

        b.iter(|| {
            key = key.wrapping_add(1);
            let value = Arc::new(format!("test_string_{}", key));
            black_box(map.insert(key, value));
        });
    });

    // Compare with non-Arc insert (baseline)
    group.bench_function("baseline_u64", |b| {
        let map = AtomicCapsuleMap::new();
        let mut key = 0u64;

        b.iter(|| {
            key = key.wrapping_add(1);
            black_box(map.insert(key, key * 2));
        });
    });

    group.finish();
}

fn bench_arc_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("arc_get");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Pre-populate map with Arc values
    let map = AtomicCapsuleMap::new();
    for i in 0..1000 {
        map.insert(i, Arc::new(vec![0u8; 64]));
    }

    group.bench_function("arc_get", |b| {
        let mut key = 0u64;
        b.iter(|| {
            key = (key + 1) % 1000;
            black_box(map.get(&key));
        });
    });

    // Compare with non-Arc get
    let baseline_map = AtomicCapsuleMap::new();
    for i in 0..1000 {
        baseline_map.insert(i, i * 2);
    }

    group.bench_function("baseline_u64_get", |b| {
        let mut key = 0u64;
        b.iter(|| {
            key = (key + 1) % 1000;
            black_box(baseline_map.get(&key));
        });
    });

    group.finish();
}

fn bench_arc_clone_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("arc_clone");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Measure Arc clone cost when getting values
    let map = AtomicCapsuleMap::new();
    for i in 0..1000 {
        map.insert(i, Arc::new(vec![0u8; 64]));
    }

    group.bench_function("arc_get_clone", |b| {
        let mut key = 0u64;
        b.iter(|| {
            key = (key + 1) % 1000;
            // This clones the Arc when we retrieve it
            let value = map.get(&key);
            black_box(value);
        });
    });

    group.finish();
}

fn bench_arc_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("arc_update");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Arc update (replace existing)
    let map = AtomicCapsuleMap::new();
    for i in 0..1000 {
        map.insert(i, Arc::new(vec![0u8; 64]));
    }

    group.bench_function("arc_replace", |b| {
        let mut key_idx = 0u64;
        b.iter(|| {
            key_idx = (key_idx + 1) % 1000;
            let new_value = Arc::new(vec![key_idx as u8; 64]);
            black_box(map.insert(key_idx, new_value));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_arc_insert,
    bench_arc_get,
    bench_arc_clone_overhead,
    bench_arc_update
);

criterion_main!(benches);
