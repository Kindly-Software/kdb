//! Basic operations benchmark
//!
//! Measures fundamental operations: insert, get, remove, update
//! Following B32 framework: Fair baselines, statistical rigor, real workloads

use atomic_capsule_map::AtomicCapsuleMap;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

fn bench_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Uncontended insert (single thread)
    group.bench_function("uncontended", |b| {
        let map = AtomicCapsuleMap::new();
        let mut key = 0u64;

        b.iter_batched(
            || {
                key = key.wrapping_add(1);
                (key, key * 2)
            },
            |(k, v)| {
                map.insert(k, v);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Insert with moderate initial size
    for initial_size in [100, 1000, 10000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("preloaded_{}", initial_size)),
            &initial_size,
            |b, &size| {
                let map = AtomicCapsuleMap::new();
                for i in 0..size {
                    map.insert(i, i * 2);
                }
                let mut key: u64 = size;

                b.iter_batched(
                    || {
                        key = key.wrapping_add(1);
                        (key, key * 2)
                    },
                    |(k, v)| {
                        map.insert(k, v);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("get");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Get from different sizes
    for size in [10, 100, 1000, 10000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let map = AtomicCapsuleMap::new();
            for i in 0..size {
                map.insert(i, i * 2);
            }
            let mut key_idx = 0u64;

            b.iter_batched(
                || {
                    key_idx = (key_idx + 1) % size;
                    key_idx
                },
                |key| {
                    black_box(map.get(&key));
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    // Get hit vs miss
    let map = AtomicCapsuleMap::new();
    for i in 0..1000 {
        map.insert(i, i * 2);
    }

    group.bench_function("hit", |b| {
        let mut key_idx = 0u64;
        b.iter_batched(
            || {
                key_idx = (key_idx + 1) % 1000;
                key_idx
            },
            |key| {
                black_box(map.get(&key));
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("miss", |b| {
        let mut key = 10000u64;
        b.iter_batched(
            || {
                key = key.wrapping_add(1);
                key
            },
            |k| {
                black_box(map.get(&k));
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_remove(c: &mut Criterion) {
    let mut group = c.benchmark_group("remove");
    group
        .confidence_level(0.95)
        .sample_size(1000) // Increased: amortization enables more samples
        .warm_up_time(Duration::from_secs(2));

    // Small map (100 entries) - Remove 10 items per iteration
    group.bench_function("remove_100", |b| {
        b.iter_batched_ref(
            || {
                let map = AtomicCapsuleMap::<u64, u64>::new();
                for i in 0..100 {
                    map.insert(i, i * 2);
                }
                map
            },
            |map| {
                // Amortize: Remove 10 items per iteration
                for i in 0..10 {
                    black_box(map.remove(&i));
                }
            },
            criterion::BatchSize::LargeInput,
        );
    });

    // Medium map (1000 entries) - Remove 50 items per iteration
    group.bench_function("remove_1000", |b| {
        b.iter_batched_ref(
            || {
                let map = AtomicCapsuleMap::<u64, u64>::new();
                for i in 0..1000 {
                    map.insert(i, i * 2);
                }
                map
            },
            |map| {
                // Amortize: Remove 50 items per iteration
                for i in 0..50 {
                    black_box(map.remove(&i));
                }
            },
            criterion::BatchSize::LargeInput,
        );
    });

    // Large map (10000 entries) - Remove 100 items per iteration
    group.bench_function("remove_10000", |b| {
        b.iter_batched_ref(
            || {
                let map = AtomicCapsuleMap::<u64, u64>::new();
                for i in 0..10000 {
                    map.insert(i, i * 2);
                }
                map
            },
            |map| {
                // Amortize: Remove 100 items per iteration
                for i in 0..100 {
                    black_box(map.remove(&i));
                }
            },
            criterion::BatchSize::LargeInput,
        );
    });

    group.finish();
}

fn bench_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("update");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Update existing keys
    for size in [100, 1000, 10000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let map = AtomicCapsuleMap::new();
            for i in 0..size {
                map.insert(i, i * 2);
            }
            let mut key_idx = 0u64;
            let mut val = 0u64;

            b.iter_batched(
                || {
                    key_idx = (key_idx + 1) % size;
                    val = val.wrapping_add(1);
                    (key_idx, val)
                },
                |(key, value)| {
                    map.insert(key, value);
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_get_or_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_or_insert");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    let map = AtomicCapsuleMap::new();
    for i in 0..1000 {
        map.insert(i, i * 2);
    }

    // Existing key (fast path)
    group.bench_function("existing", |b| {
        let mut key_idx = 0u64;
        b.iter_batched(
            || {
                key_idx = (key_idx + 1) % 1000;
                (key_idx, 9999)
            },
            |(key, val)| {
                black_box(map.get_or_insert(key, val));
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // New key (insert path)
    group.bench_function("new", |b| {
        let mut key = 10000u64;
        b.iter_batched(
            || {
                key = key.wrapping_add(1);
                (key, key * 2)
            },
            |(k, v)| {
                black_box(map.get_or_insert(k, v));
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_compare_and_swap(c: &mut Criterion) {
    let mut group = c.benchmark_group("compare_and_swap");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    let map = AtomicCapsuleMap::new();
    for i in 0..1000 {
        map.insert(i, i * 2);
    }

    // Success case
    group.bench_function("success", |b| {
        let mut key_idx = 0u64;
        b.iter_batched(
            || {
                key_idx = (key_idx + 1) % 1000;
                let old_val = map.get(&key_idx).unwrap();
                (key_idx, old_val, old_val + 1)
            },
            |(key, old, new)| {
                black_box(map.compare_and_swap(&key, old, new));
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Failure case (expected value doesn't match)
    group.bench_function("failure", |b| {
        let mut key_idx = 0u64;
        b.iter_batched(
            || {
                key_idx = (key_idx + 1) % 1000;
                (key_idx, 0u64, 9999u64)
            },
            |(key, old, new)| {
                black_box(map.compare_and_swap(&key, old, new));
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_insert,
    bench_get,
    bench_remove,
    bench_update,
    bench_get_or_insert,
    bench_compare_and_swap
);

criterion_main!(benches);
