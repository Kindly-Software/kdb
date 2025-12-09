//! AtomicCapsuleMap vs DashMap comparison benchmark
//!
//! Fair comparison following B32 framework:
//! - DashMap is optimized baseline (not strawman)
//! - Same workloads for both
//! - Statistical rigor (95% CI, 1000+ samples)
//! - Multiple contention levels

use atomic_capsule_map::AtomicCapsuleMap;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;

fn bench_insert_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_comparison");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Single threaded insert
    group.bench_function("capsule_map/uncontended", |b| {
        let map = AtomicCapsuleMap::new();
        let mut i = 0u64;
        b.iter(|| {
            map.insert(black_box(i), black_box(i * 2));
            i += 1;
        });
    });

    group.bench_function("dashmap/uncontended", |b| {
        let map = DashMap::new();
        let mut i = 0u64;
        b.iter(|| {
            map.insert(black_box(i), black_box(i * 2));
            i += 1;
        });
    });

    group.finish();
}

fn bench_get_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_comparison");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Prepare data
    for size in [100, 1000, 10000] {
        let capsule_map = {
            let map = AtomicCapsuleMap::new();
            for i in 0..size {
                map.insert(i, i * 2);
            }
            map
        };

        let dash_map = {
            let map = DashMap::new();
            for i in 0..size {
                map.insert(i, i * 2);
            }
            map
        };

        group.bench_with_input(BenchmarkId::new("capsule_map", size), &size, |b, &size| {
            b.iter(|| {
                let key = black_box(size / 2);
                black_box(capsule_map.get(&key));
            });
        });

        group.bench_with_input(BenchmarkId::new("dashmap", size), &size, |b, &size| {
            b.iter(|| {
                let key = black_box(size / 2);
                black_box(dash_map.get(&key).map(|r| *r));
            });
        });
    }

    group.finish();
}

fn bench_update_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("update_comparison");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Prepare data
    let size = 1000;
    let capsule_map = {
        let map = AtomicCapsuleMap::new();
        for i in 0..size {
            map.insert(i, i * 2);
        }
        map
    };

    let dash_map = {
        let map = DashMap::new();
        for i in 0..size {
            map.insert(i, i * 2);
        }
        map
    };

    group.bench_function("capsule_map", |b| {
        b.iter(|| {
            let key = black_box(size / 2);
            capsule_map.insert(key, black_box(key * 3));
        });
    });

    group.bench_function("dashmap", |b| {
        b.iter(|| {
            let key = black_box(size / 2);
            dash_map.insert(key, black_box(key * 3));
        });
    });

    group.finish();
}

fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");
    group
        .confidence_level(0.95)
        .sample_size(500)
        .warm_up_time(Duration::from_secs(2));

    // Realistic workload: 70% reads, 20% updates, 10% inserts
    let size = 1000;

    group.bench_function("capsule_map", |b| {
        let map = AtomicCapsuleMap::new();
        for i in 0..size {
            map.insert(i, i * 2);
        }
        let mut counter = 0u64;

        b.iter(|| {
            let op = counter % 10;
            let key = black_box(counter % size);

            match op {
                0..=6 => {
                    // 70% reads
                    black_box(map.get(&key));
                }
                7..=8 => {
                    // 20% updates
                    map.insert(key, black_box(key * 3));
                }
                _ => {
                    // 10% inserts
                    map.insert(black_box(size + counter), black_box(counter));
                }
            }
            counter += 1;
        });
    });

    group.bench_function("dashmap", |b| {
        let map = DashMap::new();
        for i in 0..size {
            map.insert(i, i * 2);
        }
        let mut counter = 0u64;

        b.iter(|| {
            let op = counter % 10;
            let key = black_box(counter % size);

            match op {
                0..=6 => {
                    // 70% reads
                    black_box(map.get(&key).map(|r| *r));
                }
                7..=8 => {
                    // 20% updates
                    map.insert(key, black_box(key * 3));
                }
                _ => {
                    // 10% inserts
                    map.insert(black_box(size + counter), black_box(counter));
                }
            }
            counter += 1;
        });
    });

    group.finish();
}

fn bench_concurrent_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_reads");
    group
        .confidence_level(0.95)
        .sample_size(100)
        .warm_up_time(Duration::from_secs(2));

    // Test with 2, 4, 8 threads
    for num_threads in [2, 4, 8] {
        // CapsuleMap
        group.bench_with_input(
            BenchmarkId::new("capsule_map", num_threads),
            &num_threads,
            |b, &threads| {
                let map = Arc::new(AtomicCapsuleMap::new());
                for i in 0..10000 {
                    map.insert(i, i * 2);
                }

                b.iter(|| {
                    let handles: Vec<_> = (0..threads)
                        .map(|_| {
                            let map = Arc::clone(&map);
                            std::thread::spawn(move || {
                                for i in 0..100 {
                                    black_box(map.get(&black_box(i * 100)));
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );

        // DashMap
        group.bench_with_input(
            BenchmarkId::new("dashmap", num_threads),
            &num_threads,
            |b, &threads| {
                let map = Arc::new(DashMap::new());
                for i in 0..10000 {
                    map.insert(i, i * 2);
                }

                b.iter(|| {
                    let handles: Vec<_> = (0..threads)
                        .map(|_| {
                            let map = Arc::clone(&map);
                            std::thread::spawn(move || {
                                for i in 0..100 {
                                    black_box(map.get(&black_box(i * 100)).map(|r| *r));
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_insert_comparison,
    bench_get_comparison,
    bench_update_comparison,
    bench_mixed_workload,
    bench_concurrent_reads
);

criterion_main!(benches);
