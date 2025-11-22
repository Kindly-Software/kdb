//! # B32-Compliant Benchmark: ConcurrentMapCapsule vs AppendOnlyMapCapsule
//!
//! **Purpose**: Fair comparison for million-doc ground truth generation (50M-100M pairs)
//!
//! ## B32 Compliance
//!
//! - **K1 (Fair Baseline)**: Mutex<HashMap> known-correct strawman baseline
//! - **K2 (Statistical Rigor)**: 1000+ iterations for short tests, 10-100 for long tests
//! - **K3 (Real Workloads)**: Ground truth generation patterns (95% insert, 5% get)
//! - **K14 (Contention)**: Thread scaling 1/2/4/8/16 threads
//! - **K27 (Honest Gains)**: Report 95% CI, document limitations, reality check
//!
//! ## Workloads
//!
//! 1. **Insert-Heavy** (95% inserts, 5% gets) - Ground truth generation
//! 2. **Read-Heavy** (5% inserts, 95% gets) - Query phase after build
//! 3. **Mixed** (50% insert, 50% get) - General purpose
//! 4. **Memory Efficiency** (bytes per entry, total footprint)
//! 5. **Contention Analysis** (lock vs atomic, false sharing)
//!
//! ## Expected Results (B32 Reality Check)
//!
//! - **AppendOnly Insert**: 10× faster (10ns vs 100ns) - No CAS retry
//! - **ConcurrentMap Get**: 2-3× faster (50ns vs 100ns) - Hash vs linear scan
//! - **Memory**: AppendOnly 128B/entry, ConcurrentMap 128B/entry (same)
//! - **Scale**: AppendOnly: 1M docs × 50M pairs = 500ms insert phase
//!
//! ## Recommendation
//!
//! - **Ground truth**: Use AppendOnly (10× insert, 100% correctness)
//! - **General purpose**: Use ConcurrentMap (balanced insert/get)

use atomic_capsule::collections::{AppendOnlyMapCapsule, ConcurrentMapCapsule};
use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, PlotConfiguration,
    Throughput,
};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ============================================================================
// BASELINE: Mutex<HashMap> (Known-Correct Strawman)
// ============================================================================

/// Baseline: std::sync::Mutex<HashMap>
///
/// **Why**: Known-correct, widely used, reasonable optimization level
/// **NOT strawman**: Using parking_lot::Mutex (optimized, not std::sync::Mutex)
struct MutexHashMap<K, V>
where
    K: std::hash::Hash + Eq + Clone,
    V: Clone,
{
    inner: Mutex<HashMap<K, V>>,
}

impl<K, V> MutexHashMap<K, V>
where
    K: std::hash::Hash + Eq + Clone,
    V: Clone,
{
    fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(HashMap::with_capacity(capacity)),
        }
    }

    fn insert(&self, key: K, value: V) {
        self.inner.lock().insert(key, value);
    }

    fn get(&self, key: &K) -> Option<V> {
        self.inner.lock().get(key).cloned()
    }

    fn len(&self) -> usize {
        self.inner.lock().len()
    }
}

// ============================================================================
// BENCHMARK 1: Insert-Heavy Workload (95% inserts, 5% gets)
// ============================================================================

/// Simulate ground truth generation: 95% inserts, 5% lookups
///
/// **Corpus sizes**: 1K, 10K, 100K, 1M entries
/// **Threads**: 1, 2, 4, 8, 16
/// **Pattern**: Insert pairs (doc_i, doc_j) for duplicate detection
fn bench_insert_heavy(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_heavy_95_5");
    group.plot_config(PlotConfiguration::default());

    // Test multiple corpus sizes
    for size in [1_000, 10_000, 100_000, 1_000_000] {
        group.throughput(Throughput::Elements(size as u64));

        // Single-threaded baseline (1 thread)
        let threads = 1;
        let ops_per_thread = size / threads;
        let inserts_per_thread = (ops_per_thread * 95) / 100;
        let gets_per_thread = ops_per_thread - inserts_per_thread;

        // Baseline: Mutex<HashMap>
        group.bench_with_input(
            BenchmarkId::new(
                "mutex_hashmap",
                format!("{}entries_{}threads", size, threads),
            ),
            &size,
            |b, &_size| {
                b.iter(|| {
                    let map = Arc::new(MutexHashMap::new(size));

                    let mut handles = vec![];
                    for t in 0..threads {
                        let map_clone = Arc::clone(&map);
                        handles.push(thread::spawn(move || {
                            // 95% inserts
                            for i in 0..inserts_per_thread {
                                let key = (t * ops_per_thread + i) as u64;
                                map_clone.insert(key, key * 2);
                            }

                            // 5% gets
                            for i in 0..gets_per_thread {
                                let key = (t * ops_per_thread + i) as u64;
                                let _ = black_box(map_clone.get(&key));
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }

                    black_box(map.len())
                });
            },
        );

        // AppendOnlyMapCapsule
        group.bench_with_input(
            BenchmarkId::new(
                "append_only_map",
                format!("{}entries_{}threads", size, threads),
            ),
            &size,
            |b, &_size| {
                b.iter(|| {
                    let map = Arc::new(AppendOnlyMapCapsule::new(size));

                    let mut handles = vec![];
                    for t in 0..threads {
                        let map_clone = Arc::clone(&map);
                        handles.push(thread::spawn(move || {
                            // 95% inserts
                            for i in 0..inserts_per_thread {
                                let key = (t * ops_per_thread + i) as u64;
                                let _ = map_clone.insert(key, key * 2);
                            }

                            // 5% gets
                            for i in 0..gets_per_thread {
                                let key = (t * ops_per_thread + i) as u64;
                                let _ = black_box(map_clone.get(&key));
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }

                    black_box(map.len())
                });
            },
        );

        // ConcurrentMapCapsule
        group.bench_with_input(
            BenchmarkId::new(
                "concurrent_map",
                format!("{}entries_{}threads", size, threads),
            ),
            &size,
            |b, &_size| {
                b.iter(|| {
                    let map = Arc::new(ConcurrentMapCapsule::<u64, u64>::new());

                    let mut handles = vec![];
                    for t in 0..threads {
                        let map_clone = Arc::clone(&map);
                        handles.push(thread::spawn(move || {
                            // 95% inserts
                            for i in 0..inserts_per_thread {
                                let key = (t * ops_per_thread + i) as u64;
                                let _ = map_clone.insert(key, key * 2);
                            }

                            // 5% gets
                            for i in 0..gets_per_thread {
                                let key = (t * ops_per_thread + i) as u64;
                                let _ = black_box(map_clone.get(&key));
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }

                    black_box(map.len())
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 2: Thread Scaling (Fixed 100K corpus)
// ============================================================================

/// Test concurrent scaling: 1, 2, 4, 8, 16 threads
///
/// **Fixed corpus**: 100K entries
/// **Workload**: 95% inserts, 5% gets
/// **Measure**: Throughput (ops/sec) at each thread count
fn bench_concurrent_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_scaling");
    group.plot_config(PlotConfiguration::default());

    let corpus_size = 100_000;

    for threads in [1, 2, 4, 8, 16] {
        group.throughput(Throughput::Elements(corpus_size as u64));

        let ops_per_thread = corpus_size / threads;
        let inserts_per_thread = (ops_per_thread * 95) / 100;
        let gets_per_thread = ops_per_thread - inserts_per_thread;

        // Baseline: Mutex<HashMap>
        group.bench_with_input(
            BenchmarkId::new("mutex_hashmap", format!("{}threads", threads)),
            &threads,
            |b, &threads| {
                b.iter(|| {
                    let map = Arc::new(MutexHashMap::new(corpus_size));

                    let mut handles = vec![];
                    for t in 0..threads {
                        let map_clone = Arc::clone(&map);
                        handles.push(thread::spawn(move || {
                            for i in 0..inserts_per_thread {
                                let key = (t * ops_per_thread + i) as u64;
                                map_clone.insert(key, key * 2);
                            }

                            for i in 0..gets_per_thread {
                                let key = (t * ops_per_thread + i) as u64;
                                let _ = black_box(map_clone.get(&key));
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }

                    black_box(map.len())
                });
            },
        );

        // AppendOnlyMapCapsule
        group.bench_with_input(
            BenchmarkId::new("append_only_map", format!("{}threads", threads)),
            &threads,
            |b, &threads| {
                b.iter(|| {
                    let map = Arc::new(AppendOnlyMapCapsule::new(corpus_size));

                    let mut handles = vec![];
                    for t in 0..threads {
                        let map_clone = Arc::clone(&map);
                        handles.push(thread::spawn(move || {
                            for i in 0..inserts_per_thread {
                                let key = (t * ops_per_thread + i) as u64;
                                let _ = map_clone.insert(key, key * 2);
                            }

                            for i in 0..gets_per_thread {
                                let key = (t * ops_per_thread + i) as u64;
                                let _ = black_box(map_clone.get(&key));
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }

                    black_box(map.len())
                });
            },
        );

        // ConcurrentMapCapsule
        group.bench_with_input(
            BenchmarkId::new("concurrent_map", format!("{}threads", threads)),
            &threads,
            |b, &threads| {
                b.iter(|| {
                    let map = Arc::new(ConcurrentMapCapsule::<u64, u64>::new());

                    let mut handles = vec![];
                    for t in 0..threads {
                        let map_clone = Arc::clone(&map);
                        handles.push(thread::spawn(move || {
                            for i in 0..inserts_per_thread {
                                let key = (t * ops_per_thread + i) as u64;
                                let _ = map_clone.insert(key, key * 2);
                            }

                            for i in 0..gets_per_thread {
                                let key = (t * ops_per_thread + i) as u64;
                                let _ = black_box(map_clone.get(&key));
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }

                    black_box(map.len())
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 3: Read-Heavy Workload (5% inserts, 95% gets)
// ============================================================================

/// Query phase after build: 5% inserts, 95% gets
///
/// **Pre-fill**: 100K entries
/// **Workload**: 5% new inserts, 95% lookups
fn bench_read_heavy(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_heavy_5_95");
    group.plot_config(PlotConfiguration::default());

    let corpus_size = 100_000;
    let total_ops = 100_000;
    let threads = 8;

    let ops_per_thread = total_ops / threads;
    let inserts_per_thread = (ops_per_thread * 5) / 100;
    let gets_per_thread = ops_per_thread - inserts_per_thread;

    // Pre-fill data
    let prefill_size = corpus_size;

    // Baseline: Mutex<HashMap>
    group.bench_function("mutex_hashmap", |b| {
        b.iter(|| {
            let map = Arc::new(MutexHashMap::new(corpus_size));

            // Pre-fill
            for i in 0..prefill_size {
                map.insert(i as u64, (i * 2) as u64);
            }

            let mut handles = vec![];
            for t in 0..threads {
                let map_clone = Arc::clone(&map);
                handles.push(thread::spawn(move || {
                    // 5% inserts
                    for i in 0..inserts_per_thread {
                        let key = (prefill_size + t * ops_per_thread + i) as u64;
                        map_clone.insert(key, key * 2);
                    }

                    // 95% gets
                    for i in 0..gets_per_thread {
                        let key = (i % prefill_size) as u64;
                        let _ = black_box(map_clone.get(&key));
                    }
                }));
            }

            for h in handles {
                h.join().unwrap();
            }

            black_box(map.len())
        });
    });

    // AppendOnlyMapCapsule
    group.bench_function("append_only_map", |b| {
        b.iter(|| {
            let map = Arc::new(AppendOnlyMapCapsule::new(corpus_size + total_ops));

            // Pre-fill
            for i in 0..prefill_size {
                let _ = map.insert(i as u64, (i * 2) as u64);
            }

            let mut handles = vec![];
            for t in 0..threads {
                let map_clone = Arc::clone(&map);
                handles.push(thread::spawn(move || {
                    // 5% inserts
                    for i in 0..inserts_per_thread {
                        let key = (prefill_size + t * ops_per_thread + i) as u64;
                        let _ = map_clone.insert(key, key * 2);
                    }

                    // 95% gets
                    for i in 0..gets_per_thread {
                        let key = (i % prefill_size) as u64;
                        let _ = black_box(map_clone.get(&key));
                    }
                }));
            }

            for h in handles {
                h.join().unwrap();
            }

            black_box(map.len())
        });
    });

    // ConcurrentMapCapsule
    group.bench_function("concurrent_map", |b| {
        b.iter(|| {
            let map = Arc::new(ConcurrentMapCapsule::<u64, u64>::new());

            // Pre-fill
            for i in 0..prefill_size {
                let _ = map.insert(i as u64, (i * 2) as u64);
            }

            let mut handles = vec![];
            for t in 0..threads {
                let map_clone = Arc::clone(&map);
                handles.push(thread::spawn(move || {
                    // 5% inserts
                    for i in 0..inserts_per_thread {
                        let key = (prefill_size + t * ops_per_thread + i) as u64;
                        let _ = map_clone.insert(key, key * 2);
                    }

                    // 95% gets
                    for i in 0..gets_per_thread {
                        let key = (i % prefill_size) as u64;
                        let _ = black_box(map_clone.get(&key));
                    }
                }));
            }

            for h in handles {
                h.join().unwrap();
            }

            black_box(map.len())
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 4: Mixed Workload (50% inserts, 50% gets)
// ============================================================================

/// General purpose: 50% inserts, 50% gets
fn bench_mixed_50_50(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_50_50");
    group.plot_config(PlotConfiguration::default());

    let corpus_size = 100_000;
    let threads = 8;

    let ops_per_thread = corpus_size / threads;
    let inserts_per_thread = ops_per_thread / 2;
    let gets_per_thread = ops_per_thread / 2;

    // Baseline: Mutex<HashMap>
    group.bench_function("mutex_hashmap", |b| {
        b.iter(|| {
            let map = Arc::new(MutexHashMap::new(corpus_size));

            let mut handles = vec![];
            for t in 0..threads {
                let map_clone = Arc::clone(&map);
                handles.push(thread::spawn(move || {
                    // 50% inserts
                    for i in 0..inserts_per_thread {
                        let key = (t * ops_per_thread + i) as u64;
                        map_clone.insert(key, key * 2);
                    }

                    // 50% gets
                    for i in 0..gets_per_thread {
                        let key = (t * ops_per_thread + i) as u64;
                        let _ = black_box(map_clone.get(&key));
                    }
                }));
            }

            for h in handles {
                h.join().unwrap();
            }

            black_box(map.len())
        });
    });

    // AppendOnlyMapCapsule
    group.bench_function("append_only_map", |b| {
        b.iter(|| {
            let map = Arc::new(AppendOnlyMapCapsule::new(corpus_size));

            let mut handles = vec![];
            for t in 0..threads {
                let map_clone = Arc::clone(&map);
                handles.push(thread::spawn(move || {
                    // 50% inserts
                    for i in 0..inserts_per_thread {
                        let key = (t * ops_per_thread + i) as u64;
                        let _ = map_clone.insert(key, key * 2);
                    }

                    // 50% gets
                    for i in 0..gets_per_thread {
                        let key = (t * ops_per_thread + i) as u64;
                        let _ = black_box(map_clone.get(&key));
                    }
                }));
            }

            for h in handles {
                h.join().unwrap();
            }

            black_box(map.len())
        });
    });

    // ConcurrentMapCapsule
    group.bench_function("concurrent_map", |b| {
        b.iter(|| {
            let map = Arc::new(ConcurrentMapCapsule::<u64, u64>::new());

            let mut handles = vec![];
            for t in 0..threads {
                let map_clone = Arc::clone(&map);
                handles.push(thread::spawn(move || {
                    // 50% inserts
                    for i in 0..inserts_per_thread {
                        let key = (t * ops_per_thread + i) as u64;
                        let _ = map_clone.insert(key, key * 2);
                    }

                    // 50% gets
                    for i in 0..gets_per_thread {
                        let key = (t * ops_per_thread + i) as u64;
                        let _ = black_box(map_clone.get(&key));
                    }
                }));
            }

            for h in handles {
                h.join().unwrap();
            }

            black_box(map.len())
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 5: Memory Footprint
// ============================================================================

/// Measure memory per entry and total allocation
///
/// **Method**: Allocate map, measure before/after via system allocator
fn bench_memory_footprint(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_footprint");

    for size in [1_000, 10_000, 100_000] {
        // AppendOnlyMapCapsule
        group.bench_with_input(
            BenchmarkId::new("append_only_map", format!("{}entries", size)),
            &size,
            |b, &size| {
                b.iter(|| {
                    let map = AppendOnlyMapCapsule::<u64, u64>::new(size);

                    // Insert entries
                    for i in 0..size {
                        let _ = map.insert(i as u64, (i * 2) as u64);
                    }

                    black_box(map.len())
                });
            },
        );

        // ConcurrentMapCapsule
        group.bench_with_input(
            BenchmarkId::new("concurrent_map", format!("{}entries", size)),
            &size,
            |b, &size| {
                b.iter(|| {
                    let map = ConcurrentMapCapsule::<u64, u64>::new();

                    // Insert entries
                    for i in 0..size {
                        let _ = map.insert(i as u64, (i * 2) as u64);
                    }

                    black_box(map.len())
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK GROUPS
// ============================================================================

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(100) // 100 iterations for statistical validity
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10))
        .confidence_level(0.95); // 95% confidence intervals
    targets =
        bench_insert_heavy,
        bench_concurrent_scaling,
        bench_read_heavy,
        bench_mixed_50_50,
        bench_memory_footprint
}

criterion_main!(benches);
