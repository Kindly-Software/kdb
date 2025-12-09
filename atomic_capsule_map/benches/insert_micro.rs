//! Micro-benchmark to isolate insert performance
//! Measures ONLY the insert operation without setup/teardown overhead

use atomic_capsule_map::AtomicCapsuleMap;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Duration;

fn bench_insert_micro(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_micro");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Pre-allocated map, measure ONLY insert cost
    group.bench_function("raw_insert", |b| {
        let map = AtomicCapsuleMap::<u64, u64>::with_capacity(4096);
        let mut key = 0u64;

        b.iter(|| {
            key = key.wrapping_add(1);
            black_box(map.insert(key, key * 2));
        });
    });

    // Insert into pre-warmed map (10% load factor)
    group.bench_function("insert_10pct_load", |b| {
        let map = AtomicCapsuleMap::<u64, u64>::with_capacity(4096);
        for i in 0..400 {
            map.insert(i, i * 2);
        }
        let mut key = 10000u64;

        b.iter(|| {
            key = key.wrapping_add(1);
            black_box(map.insert(key, key * 2));
        });
    });

    // Insert into half-full map (50% load factor)
    group.bench_function("insert_50pct_load", |b| {
        let map = AtomicCapsuleMap::<u64, u64>::with_capacity(4096);
        for i in 0..2048 {
            map.insert(i, i * 2);
        }
        let mut key = 10000u64;

        b.iter(|| {
            key = key.wrapping_add(1);
            black_box(map.insert(key, key * 2));
        });
    });

    // Insert into mostly-full map (80% load factor)
    group.bench_function("insert_80pct_load", |b| {
        let map = AtomicCapsuleMap::<u64, u64>::with_capacity(4096);
        for i in 0..3276 {
            map.insert(i, i * 2);
        }
        let mut key = 10000u64;

        b.iter(|| {
            key = key.wrapping_add(1);
            black_box(map.insert(key, key * 2));
        });
    });

    group.finish();
}

criterion_group!(benches, bench_insert_micro);
criterion_main!(benches);
