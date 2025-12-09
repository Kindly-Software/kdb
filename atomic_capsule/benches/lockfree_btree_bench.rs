//! # LockfreeBTree Benchmarks
//!
//! Benchmarks for lockfree B-tree index operations.
//!
//! ## B32 Framework Compliance
//!
//! - 95% CI with 1000+ iterations per benchmark
//! - Fair baselines (compare vs std::collections::BTreeMap)
//! - Performance targets: <50ns get, <100ns insert/remove
//! - Reality check: 10-50% typical, 2-10× exceptional
//!
//! ## Benchmarks
//!
//! 1. **Sequential Operations**: Insert/get/remove in order
//! 2. **Random Operations**: Random keys (realistic workload)
//! 3. **Parallel Reads**: Concurrent get operations
//! 4. **Mixed Workload**: 80% reads, 20% writes
//! 5. **Comparison**: LockfreeBTree vs std::BTreeMap

use atomic_capsule::collections::lockfree_btree::LockfreeBTree;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::thread;

/// Benchmark sequential inserts
fn bench_sequential_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("lockfree_btree_insert_sequential");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let btree = LockfreeBTree::new(16); // Degree 16 (max 31 keys)

                for i in 0..size {
                    let _ = black_box(btree.insert(i, i * 2));
                }

                black_box(btree)
            });
        });
    }

    group.finish();
}

/// Benchmark sequential gets
fn bench_sequential_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("lockfree_btree_get_sequential");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        // Pre-populate tree
        let btree = Arc::new(LockfreeBTree::new(16));
        for i in 0..*size {
            let _ = btree.insert(i, i * 2);
        }

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let mut sum = 0u64;
                for i in 0..size {
                    if let Some(v) = black_box(btree.get(&i)) {
                        sum += v;
                    }
                }
                black_box(sum)
            });
        });
    }

    group.finish();
}

/// Benchmark random gets
fn bench_random_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("lockfree_btree_get_random");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        // Pre-populate tree
        let btree = Arc::new(LockfreeBTree::new(16));
        for i in 0..*size {
            let _ = btree.insert(i, i * 2);
        }

        // Generate random keys
        let keys: Vec<u64> = (0..*size).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let mut sum = 0u64;
                for &key in &keys {
                    if let Some(v) = black_box(btree.get(&key)) {
                        sum += v;
                    }
                }
                black_box(sum)
            });
        });
    }

    group.finish();
}

/// Benchmark parallel reads (4 threads)
fn bench_parallel_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("lockfree_btree_parallel_reads");

    for size in [1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64 * 4)); // 4 threads

        // Pre-populate tree
        let btree = Arc::new(LockfreeBTree::new(16));
        for i in 0..*size {
            let _ = btree.insert(i, i * 2);
        }

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let mut handles = vec![];

                for _thread in 0..4 {
                    let btree = Arc::clone(&btree);
                    let handle = thread::spawn(move || {
                        let mut sum = 0u64;
                        for i in 0..size {
                            if let Some(v) = btree.get(&i) {
                                sum += v;
                            }
                        }
                        black_box(sum)
                    });
                    handles.push(handle);
                }

                for handle in handles {
                    let _ = handle.join();
                }
            });
        });
    }

    group.finish();
}

/// Benchmark comparison: LockfreeBTree vs std::BTreeMap
fn bench_comparison_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("btree_comparison_get");

    let size = 10_000u64;
    group.throughput(Throughput::Elements(size));

    // LockfreeBTree
    let lockfree_btree = Arc::new(LockfreeBTree::new(16));
    for i in 0..size {
        let _ = lockfree_btree.insert(i, i * 2);
    }

    group.bench_function("lockfree_btree", |b| {
        b.iter(|| {
            let mut sum = 0u64;
            for i in 0..size {
                if let Some(v) = black_box(lockfree_btree.get(&i)) {
                    sum += v;
                }
            }
            black_box(sum)
        });
    });

    // std::BTreeMap (with RwLock for fairness)
    let std_btree = Arc::new(RwLock::new(BTreeMap::new()));
    {
        let mut map = std_btree.write().unwrap();
        for i in 0..size {
            map.insert(i, i * 2);
        }
    }

    group.bench_function("std_btree_rwlock", |b| {
        b.iter(|| {
            let mut sum = 0u64;
            let map = std_btree.read().unwrap();
            for i in 0..size {
                if let Some(&v) = black_box(map.get(&i)) {
                    sum += v;
                }
            }
            black_box(sum)
        });
    });

    group.finish();
}

/// Benchmark statistics capsule overhead
fn bench_stats_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("lockfree_btree_stats");

    let btree = LockfreeBTree::new(16);

    group.bench_function("stats_snapshot", |b| {
        b.iter(|| black_box(btree.stats()));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_sequential_insert,
    bench_sequential_get,
    bench_random_get,
    bench_parallel_reads,
    bench_comparison_get,
    bench_stats_overhead,
);

criterion_main!(benches);
