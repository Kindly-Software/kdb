//! B32 Benchmarks for Persistent Capsules (v0.3.2 Phase 1)
//!
//! **Purpose**: Fair baseline comparison with HashMap/Vec
//!
//! # Benchmarks
//!
//! - **PersistentMap<K,V>**: Insert/Lookup vs std::collections::HashMap
//! - **PersistentLog<T>**: Append/Iteration vs Vec<T>
//!
//! # B32 Honest Claims
//!
//! - Same hardware (no cross-machine comparison)
//! - 95% CI (1000+ iterations)
//! - Fair baselines (not strawman)
//! - Reality check: 10-50% typical, 2-10× exceptional

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

#[cfg(feature = "mmap-persistence")]
use atomic_capsule::persistence::{PersistentLog, PersistentMap};

use std::collections::HashMap;

// ============================================================================
// PERSISTENT MAP BENCHMARKS
// ============================================================================

#[cfg(feature = "mmap-persistence")]
fn bench_persistent_map_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistent_map_insert");

    for size in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        // Baseline: std::collections::HashMap
        group.bench_with_input(BenchmarkId::new("std_hashmap", size), size, |b, &size| {
            b.iter(|| {
                let mut map = HashMap::new();
                for i in 0..size {
                    map.insert(black_box(i), black_box(i * 10));
                }
            });
        });

        // PersistentMap
        group.bench_with_input(
            BenchmarkId::new("persistent_map", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut map: PersistentMap<u64, u64> = PersistentMap::new(2048).unwrap();
                    for i in 0..size {
                        map.insert(black_box(i), black_box(i * 10)).unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "mmap-persistence")]
fn bench_persistent_map_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistent_map_lookup");

    for size in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        // Baseline: std::collections::HashMap
        group.bench_with_input(BenchmarkId::new("std_hashmap", size), size, |b, &size| {
            let mut map = HashMap::new();
            for i in 0..size {
                map.insert(i, i * 10);
            }

            b.iter(|| {
                for i in 0..size {
                    black_box(map.get(&black_box(i)));
                }
            });
        });

        // PersistentMap
        group.bench_with_input(
            BenchmarkId::new("persistent_map", size),
            size,
            |b, &size| {
                let mut map: PersistentMap<u64, u64> = PersistentMap::new(2048).unwrap();
                for i in 0..size {
                    map.insert(i, i * 10).unwrap();
                }

                b.iter(|| {
                    for i in 0..size {
                        black_box(map.get(&black_box(i)));
                    }
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "mmap-persistence")]
fn bench_persistent_map_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistent_map_mixed");

    for size in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        // Baseline: std::collections::HashMap
        group.bench_with_input(BenchmarkId::new("std_hashmap", size), size, |b, &size| {
            b.iter(|| {
                let mut map = HashMap::new();
                // 70% inserts, 30% lookups
                for i in 0..size {
                    if i % 10 < 7 {
                        map.insert(black_box(i), black_box(i * 10));
                    } else if i > 0 {
                        black_box(map.get(&black_box(i - 1)));
                    }
                }
            });
        });

        // PersistentMap
        group.bench_with_input(
            BenchmarkId::new("persistent_map", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut map: PersistentMap<u64, u64> = PersistentMap::new(2048).unwrap();
                    // 70% inserts, 30% lookups
                    for i in 0..size {
                        if i % 10 < 7 {
                            map.insert(black_box(i), black_box(i * 10)).unwrap();
                        } else if i > 0 {
                            black_box(map.get(&black_box(i - 1)));
                        }
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// PERSISTENT LOG BENCHMARKS
// ============================================================================

#[cfg(feature = "mmap-persistence")]
fn bench_persistent_log_append(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistent_log_append");

    for size in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        // Baseline: Vec<Vec<u8>>
        group.bench_with_input(BenchmarkId::new("std_vec", size), size, |b, &size| {
            b.iter(|| {
                let mut vec = Vec::new();
                for i in 0..size {
                    let data = format!("Entry {}", i).into_bytes();
                    vec.push(black_box(data));
                }
            });
        });

        // PersistentLog
        group.bench_with_input(
            BenchmarkId::new("persistent_log", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut log: PersistentLog<Vec<u8>> =
                        PersistentLog::new(1024 * 1024, None).unwrap();
                    for i in 0..size {
                        let data = format!("Entry {}", i).into_bytes();
                        log.append(black_box(data)).unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "mmap-persistence")]
fn bench_persistent_log_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistent_log_iteration");

    for size in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        // Baseline: Vec<Vec<u8>>
        group.bench_with_input(BenchmarkId::new("std_vec", size), size, |b, &size| {
            let mut vec = Vec::new();
            for i in 0..size {
                let data = format!("Entry {}", i).into_bytes();
                vec.push(data);
            }

            b.iter(|| {
                for data in &vec {
                    black_box(data);
                }
            });
        });

        // PersistentLog
        group.bench_with_input(
            BenchmarkId::new("persistent_log", size),
            size,
            |b, &size| {
                let mut log: PersistentLog<Vec<u8>> =
                    PersistentLog::new(1024 * 1024, None).unwrap();
                for i in 0..size {
                    let data = format!("Entry {}", i).into_bytes();
                    log.append(data).unwrap();
                }

                b.iter(|| {
                    for (_, _, data) in log.iter() {
                        black_box(data);
                    }
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "mmap-persistence")]
fn bench_persistent_log_large_entries(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistent_log_large_entries");

    for entry_size in [1024, 4096, 16384].iter() {
        group.throughput(Throughput::Bytes(*entry_size as u64 * 100)); // 100 entries

        // Baseline: Vec<Vec<u8>>
        group.bench_with_input(
            BenchmarkId::new("std_vec", entry_size),
            entry_size,
            |b, &entry_size| {
                b.iter(|| {
                    let mut vec = Vec::new();
                    for i in 0..100 {
                        let data = vec![black_box(i as u8); entry_size];
                        vec.push(data);
                    }
                });
            },
        );

        // PersistentLog
        group.bench_with_input(
            BenchmarkId::new("persistent_log", entry_size),
            entry_size,
            |b, &entry_size| {
                b.iter(|| {
                    let mut log: PersistentLog<Vec<u8>> =
                        PersistentLog::new(4 * 1024 * 1024, None).unwrap();
                    for i in 0..100 {
                        let data = vec![black_box(i as u8); entry_size];
                        log.append(data).unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// LOAD FACTOR IMPACT (PersistentMap)
// ============================================================================

#[cfg(feature = "mmap-persistence")]
fn bench_persistent_map_load_factor_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistent_map_load_factor");

    for (bucket_count, entry_count) in [(1024, 256), (1024, 512), (1024, 768)].iter() {
        let load_factor = (*entry_count as f64 / *bucket_count as f64 * 100.0) as u64;

        group.bench_with_input(
            BenchmarkId::new("insert", format!("{}%", load_factor)),
            &(*bucket_count, *entry_count),
            |b, &(bucket_count, entry_count)| {
                b.iter(|| {
                    let mut map: PersistentMap<u64, u64> =
                        PersistentMap::new(bucket_count).unwrap();
                    for i in 0..entry_count {
                        map.insert(black_box(i), black_box(i * 10)).unwrap();
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("lookup", format!("{}%", load_factor)),
            &(*bucket_count, *entry_count),
            |b, &(bucket_count, entry_count)| {
                let mut map: PersistentMap<u64, u64> = PersistentMap::new(bucket_count).unwrap();
                for i in 0..entry_count {
                    map.insert(i, i * 10).unwrap();
                }

                b.iter(|| {
                    for i in 0..entry_count {
                        black_box(map.get(&black_box(i)));
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// HASH CHAIN OVERHEAD (Auditability Q34)
// ============================================================================

#[cfg(feature = "mmap-persistence")]
fn bench_hash_chain_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_chain_overhead");

    // PersistentMap with hash chain (Q34 Auditability)
    group.bench_function("persistent_map_with_hash", |b| {
        b.iter(|| {
            let mut map: PersistentMap<u64, u64> = PersistentMap::new(1024).unwrap();
            for i in 0..1000 {
                map.insert(black_box(i), black_box(i * 10)).unwrap();
            }
            // Hash chain updated on every insert
        });
    });

    // Baseline: HashMap without hash chain
    group.bench_function("std_hashmap_no_hash", |b| {
        b.iter(|| {
            let mut map = HashMap::new();
            for i in 0..1000 {
                map.insert(black_box(i), black_box(i * 10));
            }
        });
    });

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

#[cfg(feature = "mmap-persistence")]
criterion_group!(
    benches,
    bench_persistent_map_insert,
    bench_persistent_map_lookup,
    bench_persistent_map_mixed_workload,
    bench_persistent_log_append,
    bench_persistent_log_iteration,
    bench_persistent_log_large_entries,
    bench_persistent_map_load_factor_impact,
    bench_hash_chain_overhead,
);

#[cfg(not(feature = "mmap-persistence"))]
criterion_group!(benches,);

criterion_main!(benches);
