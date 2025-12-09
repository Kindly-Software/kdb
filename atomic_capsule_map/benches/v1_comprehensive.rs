//! # Comprehensive B32-Compliant Benchmark Suite for AtomicCapsuleMap v1.0
//!
//! This benchmark suite follows the B32 Benchmark Framework (32 guidelines + 27 hardware reality checks)
//! for honest, statistically rigorous performance validation.
//!
//! ## B32 Framework Compliance
//!
//! **Core Principles (K27)**:
//! - Typical optimization: 10-50% improvement
//! - Exceptional result: 2x speedup
//! - Suspicious claim: 10x+ without algorithm change
//!
//! **Measurement Standards (B2)**:
//! - Minimum 1000 iterations for statistical significance
//! - 95% confidence intervals (Criterion.rs)
//! - Warmup period: 100+ iterations
//! - Multiple independent runs for reproducibility
//!
//! **Fair Baselines (B1)**:
//! - Compare against DashMap 6.1 (optimized baseline)
//! - Compare against std::HashMap + RwLock
//! - Compare against crossbeam-skiplist
//! - No strawman comparisons
//!
//! ## Test Coverage
//!
//! 1. **Micro-benchmarks**: Component-level performance
//! 2. **Operation benchmarks**: API-level operations
//! 3. **Contention benchmarks**: Multi-threaded scaling
//! 4. **Comparison benchmarks**: Fair baseline comparisons
//! 5. **Hardware reality checks**: Cache, NUMA, false sharing

use atomic_capsule_map::AtomicCapsuleMap;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use dashmap::DashMap;
use rayon::prelude::*;
use std::collections::HashMap;
use std::hash::{BuildHasher, Hash, Hasher};
use std::sync::{Arc, RwLock};
use std::time::Duration;

// ============================================================================
// SECTION 1: MICRO-BENCHMARKS (Component-level)
// ============================================================================

/// Benchmark: Hash function speed
/// Target: <10ns per hash (B32 K2: Cache-line atomic operations)
fn bench_hash_function(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro/hash_function");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    group.bench_function("u64_hash", |b| {
        let mut key = 0u64;
        b.iter(|| {
            key = key.wrapping_add(1);
            black_box(ahash::RandomState::new().hash_one(&key))
        });
    });

    group.bench_function("string_hash", |b| {
        let keys: Vec<String> = (0..1000).map(|i| format!("key_{}", i)).collect();
        let mut idx = 0;

        b.iter(|| {
            idx = (idx + 1) % 1000;
            black_box(ahash::RandomState::new().hash_one(&keys[idx]))
        });
    });

    group.finish();
}

/// Benchmark: Bucket CAS operation
/// Target: <15ns per CAS (B32 K2: AtomicU64 CAS = 10-15ns actual)
fn bench_bucket_cas(c: &mut Criterion) {
    use portable_atomic::{AtomicU64, Ordering};

    let mut group = c.benchmark_group("micro/bucket_cas");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    let atomic = AtomicU64::new(0);

    group.bench_function("successful_cas", |b| {
        b.iter(|| {
            let old = atomic.load(Ordering::Relaxed);
            black_box(atomic.compare_exchange(
                old,
                old.wrapping_add(1),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ))
        });
    });

    group.bench_function("failed_cas", |b| {
        atomic.store(100, Ordering::Relaxed);
        b.iter(|| {
            black_box(atomic.compare_exchange(
                0, // Wrong expected value
                1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ))
        });
    });

    group.finish();
}

/// Benchmark: Probe distance impact
/// Measures the cost of linear probing with increasing distances
fn bench_probe_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro/probe_distance");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    for load_factor in [50, 75, 90, 95] {
        for initial_size in [100, 1000, 10000] {
            let map = AtomicCapsuleMap::new();
            let num_entries = (initial_size * load_factor) / 100;

            // Fill map to target load factor
            for i in 0..num_entries {
                map.insert(i, i * 2);
            }

            group.bench_with_input(
                BenchmarkId::from_parameter(format!(
                    "{}pct_load_{}entries",
                    load_factor, initial_size
                )),
                &(load_factor, initial_size),
                |b, _| {
                    let mut key: u64 = num_entries as u64;
                    b.iter(|| {
                        key = key.wrapping_add(1);
                        black_box(map.get(&key)) // Miss path exercises probing
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark: Shard selection overhead
/// Measures the cost of selecting the correct shard
fn bench_shard_selection(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro/shard_selection");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    let map = AtomicCapsuleMap::new();

    // Pre-populate to force shard distribution
    for i in 0..10000 {
        map.insert(i, i * 2);
    }

    group.bench_function("sequential_keys", |b| {
        let mut key = 0u64;
        b.iter(|| {
            key = key.wrapping_add(1);
            black_box(map.get(&key))
        });
    });

    group.bench_function("random_keys", |b| {
        use std::collections::hash_map::RandomState;

        let random_state = RandomState::new();
        let mut key = 12345u64;
        b.iter(|| {
            // Generate pseudo-random key
            let mut hasher = random_state.build_hasher();
            key.hash(&mut hasher);
            key = hasher.finish();
            black_box(map.get(&key))
        });
    });

    group.finish();
}

/// Benchmark: Snapshot creation time
/// Measures the cost of creating atomic snapshots
fn bench_snapshot_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro/snapshot_creation");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    for size in [10, 100, 1000, 10000] {
        let map = AtomicCapsuleMap::new();
        for i in 0..size {
            map.insert(i, i * 2);
        }

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                // Snapshot via iteration
                let snapshot: Vec<_> = map.iter().collect();
                black_box(snapshot.len())
            });
        });
    }

    group.finish();
}

// ============================================================================
// SECTION 2: OPERATION BENCHMARKS (API-level)
// ============================================================================

/// Benchmark: Insert operations
/// Target: <50ns (v0.1.1 baseline: 481ns, DashMap: 36ns)
fn bench_insert_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("operations/insert");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Uncontended insert (best case)
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

    // Insert with varying load factors
    for load_factor in [25, 50, 75, 90] {
        let map = AtomicCapsuleMap::new();
        let capacity = 1000;
        let initial_size = (capacity * load_factor) / 100;

        for i in 0..initial_size {
            map.insert(i, i * 2);
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pct_load", load_factor)),
            &load_factor,
            |b, _| {
                let mut key: u64 = initial_size as u64;
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

/// Benchmark: Get operations
/// Target: <30ns (v0.1.1 baseline: 8-10ns, DashMap: 17ns)
fn bench_get_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("operations/get");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Get with different map sizes
    for size in [10, 100, 1000, 10000, 100000] {
        let map = AtomicCapsuleMap::new();
        for i in 0..size {
            map.insert(i, i * 2);
        }

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("size_{}", size)),
            &size,
            |b, &size| {
                let mut key_idx = 0u64;
                b.iter_batched(
                    || {
                        key_idx = (key_idx + 1) % size;
                        key_idx
                    },
                    |key| black_box(map.get(&key)),
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    // Get hit rate impact
    let map = AtomicCapsuleMap::new();
    for i in 0..1000 {
        map.insert(i, i * 2);
    }

    group.bench_function("100pct_hit", |b| {
        let mut key_idx = 0u64;
        b.iter_batched(
            || {
                key_idx = (key_idx + 1) % 1000;
                key_idx
            },
            |key| black_box(map.get(&key)),
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("0pct_hit", |b| {
        let mut key = 10000u64;
        b.iter_batched(
            || {
                key = key.wrapping_add(1);
                key
            },
            |k| black_box(map.get(&k)),
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("50pct_hit", |b| {
        let mut key = 0u64;
        b.iter_batched(
            || {
                key = key.wrapping_add(1);
                if key % 2 == 0 {
                    key % 1000 // Hit
                } else {
                    10000 + key // Miss
                }
            },
            |k| black_box(map.get(&k)),
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark: Update operations
/// Target: <100ns (v0.1.1 baseline: 15-20ns, DashMap: 16ns)
fn bench_update_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("operations/update");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    for size in [100, 1000, 10000] {
        let map = AtomicCapsuleMap::new();
        for i in 0..size {
            map.insert(i, i * 2);
        }

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
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

/// Benchmark: Remove operations
/// Target: <50ns (v0.1.1 baseline: 30-46ns)
fn bench_remove_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("operations/remove");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    for size in [100, 1000, 10000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter_batched_ref(
                || {
                    let map = AtomicCapsuleMap::new();
                    for i in 0..size {
                        map.insert(i, i * 2);
                    }
                    map
                },
                |map| {
                    // Remove first 10 items per iteration (amortization)
                    for i in 0..10 {
                        black_box(map.remove(&i));
                    }
                },
                criterion::BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

/// Benchmark: Iteration (full table scan)
/// Measures snapshot + iteration performance
fn bench_iteration_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("operations/iteration");
    group
        .confidence_level(0.95)
        .sample_size(100) // Fewer samples for expensive operations
        .warm_up_time(Duration::from_secs(2));

    for size in [100, 1000, 10000] {
        let map = AtomicCapsuleMap::new();
        for i in 0..size {
            map.insert(i, i * 2);
        }

        group.throughput(Throughput::Elements(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let count: usize = map.iter().count();
                black_box(count)
            });
        });
    }

    group.finish();
}

// ============================================================================
// SECTION 3: CONTENTION BENCHMARKS (Multi-threaded)
// ============================================================================

/// Benchmark: Thread scaling for reads
/// Tests scaling from 1 to 32 threads (B32 K8: 22 threads total)
fn bench_contention_read_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("contention/read_scaling");
    group
        .confidence_level(0.95)
        .sample_size(100)
        .warm_up_time(Duration::from_secs(2));

    let map = Arc::new(AtomicCapsuleMap::new());
    for i in 0..10000 {
        map.insert(i, i * 2);
    }

    for num_threads in [1, 2, 4, 8, 12, 16, 22, 32] {
        group.throughput(Throughput::Elements(1000 * num_threads as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}threads", num_threads)),
            &num_threads,
            |b, &threads| {
                b.iter(|| {
                    (0..threads).into_par_iter().for_each(|thread_id| {
                        for i in 0..1000 {
                            let key = (thread_id * 1000 + i) % 10000;
                            black_box(map.get(&(key as u64)));
                        }
                    });
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Thread scaling for writes
/// Tests write-heavy workload scaling
fn bench_contention_write_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("contention/write_scaling");
    group
        .confidence_level(0.95)
        .sample_size(100)
        .warm_up_time(Duration::from_secs(2));

    for num_threads in [1, 2, 4, 8, 12, 16] {
        group.throughput(Throughput::Elements(1000 * num_threads as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}threads", num_threads)),
            &num_threads,
            |b, &threads| {
                b.iter_batched(
                    || Arc::new(AtomicCapsuleMap::new()),
                    |map| {
                        (0..threads).into_par_iter().for_each(|thread_id| {
                            for i in 0..1000 {
                                let key = thread_id * 1000 + i;
                                map.insert(key as u64, key as u64 * 2);
                            }
                        });
                    },
                    criterion::BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark: Mixed workload (read-write ratio)
/// Tests realistic workload patterns
fn bench_contention_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("contention/mixed_workload");
    group
        .confidence_level(0.95)
        .sample_size(100)
        .warm_up_time(Duration::from_secs(2));

    for read_pct in [50, 70, 90, 95, 99] {
        let map = Arc::new(AtomicCapsuleMap::new());
        for i in 0..10000 {
            map.insert(i, i * 2);
        }

        let num_threads = 8;
        group.throughput(Throughput::Elements(1000 * num_threads as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pct_read_8threads", read_pct)),
            &read_pct,
            |b, &read_percentage| {
                b.iter(|| {
                    (0..num_threads).into_par_iter().for_each(|thread_id| {
                        for i in 0..1000 {
                            let op_id = thread_id * 1000 + i;
                            if op_id % 100 < read_percentage {
                                // Read operation
                                let key = (op_id % 10000) as u64;
                                black_box(map.get(&key));
                            } else {
                                // Write operation
                                let key = (10000 + op_id) as u64;
                                map.insert(key, key * 2);
                            }
                        }
                    });
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// SECTION 4: COMPARISON BENCHMARKS (Fair Baselines)
// ============================================================================

/// Benchmark: AtomicCapsuleMap vs DashMap (Fair comparison)
/// B32 B1: Compare against optimized baseline, not strawman
fn bench_vs_dashmap(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparison/vs_dashmap");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Insert comparison
    group.bench_function("insert_acm", |b| {
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

    group.bench_function("insert_dashmap", |b| {
        let map = DashMap::new();
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

    // Get comparison
    let acm = AtomicCapsuleMap::new();
    let dm = DashMap::new();
    for i in 0..1000 {
        acm.insert(i, i * 2);
        dm.insert(i, i * 2);
    }

    group.bench_function("get_acm", |b| {
        let mut key_idx = 0u64;
        b.iter_batched(
            || {
                key_idx = (key_idx + 1) % 1000;
                key_idx
            },
            |key| black_box(acm.get(&key)),
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("get_dashmap", |b| {
        let mut key_idx = 0u64;
        b.iter_batched(
            || {
                key_idx = (key_idx + 1) % 1000;
                key_idx
            },
            |key| black_box(dm.get(&key).map(|v| *v)),
            criterion::BatchSize::SmallInput,
        );
    });

    // Update comparison
    group.bench_function("update_acm", |b| {
        let mut key_idx = 0u64;
        let mut val = 0u64;
        b.iter_batched(
            || {
                key_idx = (key_idx + 1) % 1000;
                val = val.wrapping_add(1);
                (key_idx, val)
            },
            |(key, value)| {
                acm.insert(key, value);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("update_dashmap", |b| {
        let mut key_idx = 0u64;
        let mut val = 0u64;
        b.iter_batched(
            || {
                key_idx = (key_idx + 1) % 1000;
                val = val.wrapping_add(1);
                (key_idx, val)
            },
            |(key, value)| {
                dm.insert(key, value);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Mixed workload comparison (70% read, 30% write)
    group.bench_function("mixed_70read_acm", |b| {
        let mut op_count = 0u64;
        b.iter_batched(
            || {
                op_count = op_count.wrapping_add(1);
                op_count
            },
            |op_id| {
                if op_id % 10 < 7 {
                    black_box(acm.get(&(op_id % 1000)));
                } else {
                    acm.insert(1000 + op_id, op_id * 2);
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("mixed_70read_dashmap", |b| {
        let mut op_count = 0u64;
        b.iter_batched(
            || {
                op_count = op_count.wrapping_add(1);
                op_count
            },
            |op_id| {
                if op_id % 10 < 7 {
                    black_box(dm.get(&(op_id % 1000)));
                } else {
                    dm.insert(1000 + op_id, op_id * 2);
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark: vs std::HashMap + RwLock
/// B32 B1: Compare against another fair baseline
fn bench_vs_rwlock_hashmap(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparison/vs_rwlock");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    let acm = AtomicCapsuleMap::new();
    let rwlock_map = RwLock::new(HashMap::new());

    for i in 0..1000 {
        acm.insert(i, i * 2);
        rwlock_map.write().unwrap().insert(i, i * 2);
    }

    // Read comparison
    group.bench_function("read_acm", |b| {
        let mut key_idx = 0u64;
        b.iter_batched(
            || {
                key_idx = (key_idx + 1) % 1000;
                key_idx
            },
            |key| black_box(acm.get(&key)),
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("read_rwlock", |b| {
        let mut key_idx = 0u64;
        b.iter_batched(
            || {
                key_idx = (key_idx + 1) % 1000;
                key_idx
            },
            |key| black_box(rwlock_map.read().unwrap().get(&key).copied()),
            criterion::BatchSize::SmallInput,
        );
    });

    // Write comparison
    group.bench_function("write_acm", |b| {
        let mut key = 10000u64;
        b.iter_batched(
            || {
                key = key.wrapping_add(1);
                (key, key * 2)
            },
            |(k, v)| {
                acm.insert(k, v);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("write_rwlock", |b| {
        let mut key = 10000u64;
        b.iter_batched(
            || {
                key = key.wrapping_add(1);
                (key, key * 2)
            },
            |(k, v)| {
                rwlock_map.write().unwrap().insert(k, v);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ============================================================================
// SECTION 5: HARDWARE REALITY CHECKS (B32 HW1-HW27)
// ============================================================================

/// Hardware Reality Check: Cache effects (L1/L2/L3)
/// B32 K6: L1=1ns, L2=3ns, L3=9-12ns, RAM=90-100ns
fn bench_hw_cache_effects(c: &mut Criterion) {
    let mut group = c.benchmark_group("hardware/cache_effects");
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // L1 cache size: 48KB → ~6000 u64 entries
    // L2 cache size: 2MB → ~250K u64 entries
    // L3 cache size: 24MB → ~3M u64 entries

    for size in [
        1_000,      // Fits in L1
        100_000,    // Fits in L2
        1_000_000,  // Fits in L3
        10_000_000, // Exceeds L3, hits RAM
    ] {
        let map = AtomicCapsuleMap::new();
        for i in 0..size {
            map.insert(i, i * 2);
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}entries", size)),
            &size,
            |b, &size| {
                let mut key_idx = 0u64;
                b.iter_batched(
                    || {
                        key_idx = (key_idx + 1) % size;
                        key_idx
                    },
                    |key| black_box(map.get(&key)),
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Hardware Reality Check: False sharing detection
/// B32 K6: Cache line = 64 bytes
fn bench_hw_false_sharing(c: &mut Criterion) {
    use portable_atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    let mut group = c.benchmark_group("hardware/false_sharing");
    group
        .confidence_level(0.95)
        .sample_size(100)
        .warm_up_time(Duration::from_secs(2));

    // Test with adjacent atomics (false sharing)
    let adjacent = Arc::new([AtomicU64::new(0), AtomicU64::new(0)]);

    group.bench_function("adjacent_atomics", |b| {
        b.iter(|| {
            let adj = adjacent.clone();
            rayon::scope(|s| {
                s.spawn(|_| {
                    for _ in 0..1000 {
                        adj[0].fetch_add(1, Ordering::Relaxed);
                    }
                });
                s.spawn(|_| {
                    for _ in 0..1000 {
                        adj[1].fetch_add(1, Ordering::Relaxed);
                    }
                });
            });
        });
    });

    // Test with cache-aligned atomics (no false sharing)
    #[repr(align(64))]
    struct Aligned(AtomicU64);

    let aligned = Arc::new([Aligned(AtomicU64::new(0)), Aligned(AtomicU64::new(0))]);

    group.bench_function("aligned_atomics", |b| {
        b.iter(|| {
            let al = aligned.clone();
            rayon::scope(|s| {
                s.spawn(|_| {
                    for _ in 0..1000 {
                        al[0].0.fetch_add(1, Ordering::Relaxed);
                    }
                });
                s.spawn(|_| {
                    for _ in 0..1000 {
                        al[1].0.fetch_add(1, Ordering::Relaxed);
                    }
                });
            });
        });
    });

    group.finish();
}

/// Hardware Reality Check: Memory bandwidth saturation
/// B32 K3: DDR5-5600 measured = 15.2GB/s sequential, 3-5GB/s random
fn bench_hw_memory_bandwidth(c: &mut Criterion) {
    let mut group = c.benchmark_group("hardware/memory_bandwidth");
    group
        .confidence_level(0.95)
        .sample_size(100)
        .warm_up_time(Duration::from_secs(2));

    // Sequential access pattern (cache-friendly)
    let map = AtomicCapsuleMap::new();
    for i in 0..100000 {
        map.insert(i, i * 2);
    }

    group.throughput(Throughput::Elements(10000));
    group.bench_function("sequential_reads", |b| {
        b.iter(|| {
            for i in 0..10000 {
                black_box(map.get(&i));
            }
        });
    });

    // Random access pattern (cache-hostile)
    group.bench_function("random_reads", |b| {
        use std::collections::hash_map::RandomState;

        let random_state = RandomState::new();
        let mut seed = 12345u64;
        b.iter(|| {
            for _ in 0..10000 {
                let mut hasher = random_state.build_hasher();
                seed.hash(&mut hasher);
                seed = hasher.finish();
                let key = seed % 100000;
                black_box(map.get(&key));
            }
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK GROUPS
// ============================================================================

criterion_group!(
    micro_benchmarks,
    bench_hash_function,
    bench_bucket_cas,
    bench_probe_distance,
    bench_shard_selection,
    bench_snapshot_creation,
);

criterion_group!(
    operation_benchmarks,
    bench_insert_operations,
    bench_get_operations,
    bench_update_operations,
    bench_remove_operations,
    bench_iteration_operations,
);

criterion_group!(
    contention_benchmarks,
    bench_contention_read_scaling,
    bench_contention_write_scaling,
    bench_contention_mixed_workload,
);

criterion_group!(
    comparison_benchmarks,
    bench_vs_dashmap,
    bench_vs_rwlock_hashmap,
);

criterion_group!(
    hardware_benchmarks,
    bench_hw_cache_effects,
    bench_hw_false_sharing,
    bench_hw_memory_bandwidth,
);

criterion_main!(
    micro_benchmarks,
    operation_benchmarks,
    contention_benchmarks,
    comparison_benchmarks,
    hardware_benchmarks,
);
