//! # Result Aggregator Comparison Benchmarks (B32 Framework)
//!
//! **Fair baseline comparison: V1 (Mutex<HashMap>) vs V2 (Sharded Lockfree)**
//!
//! ## B32 Benchmarking Framework Compliance
//! - **Fair baseline**: V1 uses Mutex<HashMap> with same capacity
//! - **Same hardware**: All benchmarks on same machine (Intel Ultra 7 155H)
//! - **Statistical rigor**: 1000+ iterations, 95% CI via Criterion
//! - **Honest claims**: Report actual speedups (not strawman comparisons)
//! - **Reproducibility**: All code committed, can be reproduced
//!
//! ## Performance Expectations (Hardware Reality - K4, K12, K27)
//! - **Uncontended (1 thread)**: 10-30% improvement (V2 overhead from sharding)
//! - **Light contention (2-4 threads)**: 2-3× improvement (reduced lock contention)
//! - **Heavy contention (8-16 threads)**: 5-10× improvement (16 shards vs 1 lock)
//! - **Reality Check (K27)**: Honest gains 10-50% typical, 2× exceptional, 10× suspicious
//!
//! ## Benchmark Scenarios (B3 Realistic Workloads)
//! 1. **Insert latency**: Single-threaded insert performance (baseline overhead)
//! 2. **Merge latency**: 100K results merge time (sequential scan)
//! 3. **Concurrent throughput**: 1-16 threads × 10K inserts/thread
//! 4. **Capacity stress**: 90% load factor, test graceful degradation
//! 5. **Mixed workload**: Insert + read + merge (production-like)
//!
//! ## Expected Results (K4 Mutex Costs)
//! - **V1 Mutex uncontended**: ~30ns (K4)
//! - **V1 Mutex contended**: 1-10μs (highly variable, K4)
//! - **V2 Shard lookup**: <5ns (hash + modulo)
//! - **V2 Per-shard mutex**: <50ns (reduced contention with 16 shards)

use atomic_capsule::parallel::LockfreeResultAggregator;
use criterion::{
    black_box, criterion_group, criterion_main, measurement::WallTime, BenchmarkId, Criterion,
    Throughput,
};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::thread;

// ==============================================================================
// V1 BASELINE: Single Mutex<HashMap> (Fair Comparison)
// ==============================================================================

/// V1: Simple mutex-protected HashMap (fair baseline)
///
/// # Performance
/// - **Uncontended**: ~30ns per insert (K4)
/// - **Contended**: 1-10μs per insert (lock contention, K4)
/// - **Merge**: O(n) sequential scan (same as V2)
///
/// # B32 Fairness
/// - Uses Mutex (not RwLock) for fair comparison
/// - Same capacity hint as V2
/// - Same merge algorithm as V2
struct V1MutexAggregator<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    /// Single mutex-protected HashMap
    /// - **Contention bottleneck**: All threads compete for same lock
    /// - **Uncontended**: ~30ns lock/unlock (K4)
    /// - **Contended**: 1-10μs wait time (exponential with threads, K4)
    map: Arc<Mutex<HashMap<K, Vec<V>>>>,
}

impl<K, V> V1MutexAggregator<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    /// Create new V1 aggregator with capacity hint
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            map: Arc::new(Mutex::new(HashMap::with_capacity(capacity))),
        }
    }

    /// Insert key-value pair (mutex-protected)
    ///
    /// # Performance
    /// - **Uncontended**: ~30ns (K4)
    /// - **Contended (4 threads)**: ~250ns (K4)
    /// - **Contended (16 threads)**: 5-10μs (lock storms, K4)
    pub fn insert(&self, key: K, value: V) {
        let mut map = self.map.lock().unwrap();
        map.entry(key).or_insert_with(Vec::new).push(value);
    }

    /// Merge all results (same algorithm as V2)
    ///
    /// # Performance
    /// - O(n) sequential scan (same as V2)
    /// - <10ms for 100K entries (same as V2)
    pub fn merge(&self) -> HashMap<K, Vec<V>> {
        let map = self.map.lock().unwrap();
        map.clone()
    }

    /// Get number of entries
    pub fn len(&self) -> usize {
        self.map.lock().unwrap().len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.map.lock().unwrap().is_empty()
    }
}

// ==============================================================================
// Scenario 1: Single-Threaded Insert Latency (Baseline Overhead)
// ==============================================================================

fn bench_insert_latency_single_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_latency_single_thread");
    group.throughput(Throughput::Elements(10_000));

    // V1: Mutex<HashMap> (uncontended)
    group.bench_function("V1_Mutex_baseline", |b| {
        b.iter(|| {
            let agg: V1MutexAggregator<u64, u64> = V1MutexAggregator::with_capacity(10_000);
            for i in 0..10_000 {
                black_box(agg.insert(i, i * 10));
            }
        });
    });

    // V2: Sharded lockfree (overhead from sharding)
    group.bench_function("V2_Sharded_lockfree", |b| {
        b.iter(|| {
            let agg: LockfreeResultAggregator<u64, u64> =
                LockfreeResultAggregator::with_capacity(10_000);
            for i in 0..10_000 {
                black_box(agg.insert(i, i * 10));
            }
        });
    });

    group.finish();
}

// ==============================================================================
// Scenario 2: Merge Latency (100K Results)
// ==============================================================================

fn bench_merge_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("merge_latency");

    for size in [10_000, 50_000, 100_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        // V1: Mutex<HashMap> merge
        group.bench_with_input(
            BenchmarkId::new("V1_Mutex_baseline", size),
            size,
            |b, &size| {
                let agg: V1MutexAggregator<u64, u64> = V1MutexAggregator::with_capacity(size);
                for i in 0..size {
                    agg.insert(i as u64, i as u64 * 10);
                }

                b.iter(|| {
                    let results = agg.merge();
                    black_box(results);
                });
            },
        );

        // V2: Sharded lockfree merge
        group.bench_with_input(
            BenchmarkId::new("V2_Sharded_lockfree", size),
            size,
            |b, &size| {
                let agg: LockfreeResultAggregator<u64, u64> =
                    LockfreeResultAggregator::with_capacity(size);
                for i in 0..size {
                    agg.insert(i as u64, i as u64 * 10);
                }

                b.iter(|| {
                    let results = agg.merge();
                    black_box(results);
                });
            },
        );
    }

    group.finish();
}

// ==============================================================================
// Scenario 3: Concurrent Throughput (1-16 Threads × 10K Inserts)
// ==============================================================================

fn bench_concurrent_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_throughput");

    for num_threads in [1, 2, 4, 8, 16].iter() {
        let total_ops = num_threads * 10_000;
        group.throughput(Throughput::Elements(total_ops as u64));

        // V1: Mutex<HashMap> (lock contention increases with threads)
        group.bench_with_input(
            BenchmarkId::new("V1_Mutex_baseline", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let agg = Arc::new(V1MutexAggregator::with_capacity(total_ops));
                    let mut handles = vec![];

                    for thread_id in 0..num_threads {
                        let agg_clone = Arc::clone(&agg);
                        let handle = thread::spawn(move || {
                            for i in 0..10_000 {
                                let key = (thread_id * 10_000 + i) as u64;
                                agg_clone.insert(key, thread_id as u64);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    black_box(agg.len());
                });
            },
        );

        // V2: Sharded lockfree (contention reduced by 16×)
        group.bench_with_input(
            BenchmarkId::new("V2_Sharded_lockfree", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let agg = Arc::new(LockfreeResultAggregator::with_capacity(total_ops));
                    let mut handles = vec![];

                    for thread_id in 0..num_threads {
                        let agg_clone = Arc::clone(&agg);
                        let handle = thread::spawn(move || {
                            for i in 0..10_000 {
                                let key = (thread_id * 10_000 + i) as u64;
                                agg_clone.insert(key, thread_id as u64);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    black_box(agg.len());
                });
            },
        );
    }

    group.finish();
}

// ==============================================================================
// Scenario 4: Capacity Stress (90% Load Factor)
// ==============================================================================

fn bench_capacity_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("capacity_stress_90_percent");

    // Test at 90% capacity (trigger HashMap growth)
    let capacity = 100_000;
    let load = (capacity as f64 * 0.9) as usize;
    group.throughput(Throughput::Elements(load as u64));

    // V1: Mutex<HashMap> with 90% load
    group.bench_function("V1_Mutex_baseline", |b| {
        b.iter(|| {
            let agg: V1MutexAggregator<u64, u64> = V1MutexAggregator::with_capacity(capacity);
            for i in 0..load {
                black_box(agg.insert(i as u64, i as u64 * 10));
            }
        });
    });

    // V2: Sharded lockfree with 90% load
    group.bench_function("V2_Sharded_lockfree", |b| {
        b.iter(|| {
            let agg: LockfreeResultAggregator<u64, u64> =
                LockfreeResultAggregator::with_capacity(capacity);
            for i in 0..load {
                black_box(agg.insert(i as u64, i as u64 * 10));
            }
        });
    });

    group.finish();
}

// ==============================================================================
// Scenario 5: Mixed Workload (Insert + Merge - Production-like)
// ==============================================================================

fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload_insert_merge");
    group.throughput(Throughput::Elements(100_000));

    // V1: Mutex<HashMap> insert + merge
    group.bench_function("V1_Mutex_baseline", |b| {
        b.iter(|| {
            let agg: V1MutexAggregator<u64, u64> = V1MutexAggregator::with_capacity(100_000);

            // Insert 100K entries
            for i in 0..100_000 {
                agg.insert(i, i * 10);
            }

            // Merge results
            let results = agg.merge();
            black_box(results);
        });
    });

    // V2: Sharded lockfree insert + merge
    group.bench_function("V2_Sharded_lockfree", |b| {
        b.iter(|| {
            let agg: LockfreeResultAggregator<u64, u64> =
                LockfreeResultAggregator::with_capacity(100_000);

            // Insert 100K entries
            for i in 0..100_000 {
                agg.insert(i, i * 10);
            }

            // Merge results
            let results = agg.merge();
            black_box(results);
        });
    });

    group.finish();
}

// ==============================================================================
// Scenario 6: Contention on Same Keys (Worst Case)
// ==============================================================================

fn bench_same_key_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("same_key_contention");

    for num_threads in [2, 4, 8, 16].iter() {
        let ops_per_thread = 10_000;
        let total_ops = num_threads * ops_per_thread;
        group.throughput(Throughput::Elements(total_ops as u64));

        // V1: Mutex<HashMap> (worst case - all threads contend on same lock)
        group.bench_with_input(
            BenchmarkId::new("V1_Mutex_baseline", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let agg = Arc::new(V1MutexAggregator::with_capacity(100));
                    let mut handles = vec![];

                    for thread_id in 0..num_threads {
                        let agg_clone = Arc::clone(&agg);
                        let handle = thread::spawn(move || {
                            // All threads write to same 100 keys (worst case contention)
                            for i in 0..ops_per_thread {
                                let key = (i % 100) as u64;
                                agg_clone.insert(key, thread_id as u64);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    black_box(agg.len());
                });
            },
        );

        // V2: Sharded lockfree (contention reduced by 16× even with same keys)
        group.bench_with_input(
            BenchmarkId::new("V2_Sharded_lockfree", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let agg = Arc::new(LockfreeResultAggregator::with_capacity(100));
                    let mut handles = vec![];

                    for thread_id in 0..num_threads {
                        let agg_clone = Arc::clone(&agg);
                        let handle = thread::spawn(move || {
                            // All threads write to same 100 keys (worst case contention)
                            for i in 0..ops_per_thread {
                                let key = (i % 100) as u64;
                                agg_clone.insert(key, thread_id as u64);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    black_box(agg.len());
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_insert_latency_single_thread,
    bench_merge_latency,
    bench_concurrent_throughput,
    bench_capacity_stress,
    bench_mixed_workload,
    bench_same_key_contention,
);
criterion_main!(benches);
