//! # LockfreeHashTable Benchmarks
//!
//! **B32 Framework: Compare LockfreeHashTable vs RwLock<HashMap>**
//!
//! Baseline: RwLock<HashMap> (standard Rust concurrent map)
//! Optimized: LockfreeHashTable (100% lockfree)
//!
//! Expected improvements:
//! - Read (get): 3-10× faster (no lock contention)
//! - Write (insert): 2-5× faster (CAS-based coordination)
//! - Remove: 2-4× faster (lockfree deletion)

use atomic_capsule::LockfreeHashTable;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;

fn bench_single_thread_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_thread_insert");

    for size in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        // Baseline: RwLock<HashMap>
        group.bench_with_input(
            BenchmarkId::new("rwlock_hashmap", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let map = Arc::new(RwLock::new(HashMap::new()));
                    for i in 0..size {
                        map.write().unwrap().insert(i as u64, i);
                    }
                });
            },
        );

        // Optimized: LockfreeHashTable
        group.bench_with_input(
            BenchmarkId::new("lockfree_table", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let table = Arc::new(LockfreeHashTable::new(16384));
                    for i in 0..size {
                        table.insert(i as u64, i);
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_single_thread_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_thread_get");

    for size in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        // Baseline: RwLock<HashMap>
        {
            let map = Arc::new(RwLock::new(HashMap::new()));
            for i in 0..size {
                map.write().unwrap().insert(i as u64, i);
            }

            group.bench_with_input(
                BenchmarkId::new("rwlock_hashmap", size),
                &size,
                |b, &size| {
                    b.iter(|| {
                        for i in 0..size {
                            black_box(map.read().unwrap().get(&(i as u64)));
                        }
                    });
                },
            );
        }

        // Optimized: LockfreeHashTable
        {
            let table = Arc::new(LockfreeHashTable::new(16384));
            for i in 0..size {
                table.insert(i as u64, i);
            }

            group.bench_with_input(
                BenchmarkId::new("lockfree_table", size),
                &size,
                |b, &size| {
                    b.iter(|| {
                        for i in 0..size {
                            black_box(table.get(&(i as u64)));
                        }
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_concurrent_inserts(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_inserts");

    for threads in [2, 4, 8] {
        let per_thread = 1000;
        group.throughput(Throughput::Elements((threads * per_thread) as u64));

        // Baseline: RwLock<HashMap>
        group.bench_with_input(
            BenchmarkId::new("rwlock_hashmap", threads),
            &threads,
            |b, &threads| {
                b.iter(|| {
                    let map = Arc::new(RwLock::new(HashMap::new()));
                    let mut handles = vec![];

                    for thread_id in 0..threads {
                        let map_clone = Arc::clone(&map);
                        handles.push(thread::spawn(move || {
                            for i in 0..per_thread {
                                let key = (thread_id * per_thread + i) as u64;
                                map_clone.write().unwrap().insert(key, i);
                            }
                        }));
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );

        // Optimized: LockfreeHashTable
        group.bench_with_input(
            BenchmarkId::new("lockfree_table", threads),
            &threads,
            |b, &threads| {
                b.iter(|| {
                    let table = Arc::new(LockfreeHashTable::new(16384));
                    let mut handles = vec![];

                    for thread_id in 0..threads {
                        let table_clone = Arc::clone(&table);
                        handles.push(thread::spawn(move || {
                            for i in 0..per_thread {
                                let key = (thread_id * per_thread + i) as u64;
                                table_clone.insert(key, i);
                            }
                        }));
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

fn bench_concurrent_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_reads");

    for threads in [2, 4, 8] {
        let per_thread = 1000;
        group.throughput(Throughput::Elements((threads * per_thread) as u64));

        // Baseline: RwLock<HashMap>
        {
            let map = Arc::new(RwLock::new(HashMap::new()));
            for i in 0..per_thread {
                map.write().unwrap().insert(i as u64, i);
            }

            group.bench_with_input(
                BenchmarkId::new("rwlock_hashmap", threads),
                &threads,
                |b, &threads| {
                    b.iter(|| {
                        let map_clone = Arc::clone(&map);
                        let mut handles = vec![];

                        for _ in 0..threads {
                            let map_clone2 = Arc::clone(&map_clone);
                            handles.push(thread::spawn(move || {
                                for i in 0..per_thread {
                                    black_box(map_clone2.read().unwrap().get(&(i as u64)));
                                }
                            }));
                        }

                        for handle in handles {
                            handle.join().unwrap();
                        }
                    });
                },
            );
        }

        // Optimized: LockfreeHashTable
        {
            let table = Arc::new(LockfreeHashTable::new(16384));
            for i in 0..per_thread {
                table.insert(i as u64, i);
            }

            group.bench_with_input(
                BenchmarkId::new("lockfree_table", threads),
                &threads,
                |b, &threads| {
                    b.iter(|| {
                        let table_clone = Arc::clone(&table);
                        let mut handles = vec![];

                        for _ in 0..threads {
                            let table_clone2 = Arc::clone(&table_clone);
                            handles.push(thread::spawn(move || {
                                for i in 0..per_thread {
                                    black_box(table_clone2.get(&(i as u64)));
                                }
                            }));
                        }

                        for handle in handles {
                            handle.join().unwrap();
                        }
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_mixed_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_operations");

    for threads in [2, 4, 8] {
        let per_thread = 500;
        group.throughput(Throughput::Elements((threads * per_thread * 2) as u64));

        // Baseline: RwLock<HashMap>
        group.bench_with_input(
            BenchmarkId::new("rwlock_hashmap", threads),
            &threads,
            |b, &threads| {
                b.iter(|| {
                    let map = Arc::new(RwLock::new(HashMap::new()));
                    let mut handles = vec![];

                    for thread_id in 0..threads {
                        let map_clone = Arc::clone(&map);
                        handles.push(thread::spawn(move || {
                            for i in 0..per_thread {
                                let key = (thread_id * per_thread + i) as u64;
                                map_clone.write().unwrap().insert(key, i);
                                black_box(map_clone.read().unwrap().get(&key));
                            }
                        }));
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );

        // Optimized: LockfreeHashTable
        group.bench_with_input(
            BenchmarkId::new("lockfree_table", threads),
            &threads,
            |b, &threads| {
                b.iter(|| {
                    let table = Arc::new(LockfreeHashTable::new(16384));
                    let mut handles = vec![];

                    for thread_id in 0..threads {
                        let table_clone = Arc::clone(&table);
                        handles.push(thread::spawn(move || {
                            for i in 0..per_thread {
                                let key = (thread_id * per_thread + i) as u64;
                                table_clone.insert(key, i);
                                black_box(table_clone.get(&key));
                            }
                        }));
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

criterion_group!(
    benches,
    bench_single_thread_insert,
    bench_single_thread_get,
    bench_concurrent_inserts,
    bench_concurrent_reads,
    bench_mixed_operations,
);

// ========================================================================
// GENERIC KEY BENCHMARKS (Phase 2.3)
// ========================================================================

fn bench_string_keys_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_keys_insert");

    for size in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        // Baseline: u64 keys
        group.bench_with_input(BenchmarkId::new("u64_baseline", size), &size, |b, &size| {
            b.iter(|| {
                let table = LockfreeHashTable::<u64, String>::new(16384);
                for i in 0..size {
                    table.insert(i as u64, format!("value{}", i));
                }
            });
        });

        // String keys
        group.bench_with_input(BenchmarkId::new("string_keys", size), &size, |b, &size| {
            b.iter(|| {
                let table = LockfreeHashTable::<String, String>::new(16384);
                for i in 0..size {
                    table.insert(format!("key{}", i), format!("value{}", i));
                }
            });
        });
    }

    group.finish();
}

fn bench_string_keys_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_keys_get");

    for size in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        // Baseline: u64 keys
        {
            let table = LockfreeHashTable::<u64, String>::new(16384);
            for i in 0..size {
                table.insert(i as u64, format!("value{}", i));
            }

            group.bench_with_input(BenchmarkId::new("u64_baseline", size), &size, |b, &size| {
                b.iter(|| {
                    for i in 0..size {
                        black_box(table.get(&(i as u64)));
                    }
                });
            });
        }

        // String keys
        {
            let table = LockfreeHashTable::<String, String>::new(16384);
            for i in 0..size {
                table.insert(format!("key{}", i), format!("value{}", i));
            }

            let keys: Vec<String> = (0..size).map(|i| format!("key{}", i)).collect();

            group.bench_with_input(BenchmarkId::new("string_keys", size), &size, |b, &_size| {
                b.iter(|| {
                    for key in &keys {
                        black_box(table.get(key));
                    }
                });
            });
        }
    }

    group.finish();
}

fn bench_custom_struct_keys(c: &mut Criterion) {
    #[derive(Hash, Eq, PartialEq, Clone)]
    struct CustomKey {
        id: u64,
        category: u32,
    }

    let mut group = c.benchmark_group("custom_struct_keys");

    for size in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        // Baseline: u64 keys
        group.bench_with_input(BenchmarkId::new("u64_baseline", size), &size, |b, &size| {
            b.iter(|| {
                let table = LockfreeHashTable::<u64, i32>::new(16384);
                for i in 0..size {
                    table.insert(i as u64, i);
                }
            });
        });

        // Custom struct keys
        group.bench_with_input(
            BenchmarkId::new("custom_struct", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let table = LockfreeHashTable::<CustomKey, i32>::new(16384);
                    for i in 0..size {
                        let key = CustomKey {
                            id: i as u64,
                            category: (i % 10) as u32,
                        };
                        table.insert(key, i);
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_key_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("key_type_overhead");
    group.throughput(Throughput::Elements(1000));

    // u64 baseline (8 bytes)
    group.bench_function("u64_8bytes", |b| {
        let table = LockfreeHashTable::<u64, i32>::new(8192);
        b.iter(|| {
            for i in 0..1000 {
                table.insert(i as u64, i);
            }
        });
    });

    // String (50 bytes avg)
    group.bench_function("string_50bytes", |b| {
        let table = LockfreeHashTable::<String, i32>::new(8192);
        b.iter(|| {
            for i in 0..1000 {
                table.insert(format!("key_{:040}", i), i); // 50 byte keys
            }
        });
    });

    // Custom struct (16 bytes)
    #[derive(Hash, Eq, PartialEq, Clone)]
    struct CustomKey {
        id: u64,
        version: u64,
    }

    group.bench_function("custom_16bytes", |b| {
        let table = LockfreeHashTable::<CustomKey, i32>::new(8192);
        b.iter(|| {
            for i in 0..1000 {
                let key = CustomKey {
                    id: i as u64,
                    version: 1,
                };
                table.insert(key, i);
            }
        });
    });

    group.finish();
}

criterion_group!(
    generic_benches,
    bench_string_keys_insert,
    bench_string_keys_get,
    bench_custom_struct_keys,
    bench_key_overhead
);
criterion_main!(benches, generic_benches);
