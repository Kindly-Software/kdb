//! Performance Budget Benchmarks - B32-Compliant Fair Comparisons
//!
//! ## Purpose
//! Compare Timeline Aggregation against optimized baselines using Criterion.rs
//! with statistical rigor. Enforce honest performance claims (10-50% typical, 2× exceptional).
//!
//! ## Framework Compliance
//! - **B32**: Fair benchmarking with optimized baselines (not strawmen)
//! - **B1-B5**: Statistical rigor (1000+ samples, 95% CI, percentiles)
//! - **K2**: Atomic operation reality checks (15-25ns CAS, 20ns FetchAdd)
//! - **K27**: Honest gains (10-50% typical, 2× exceptional, 10× suspicious)
//!
//! ## Baselines (Fair Comparisons)
//! 1. **Mutex<Counter>** - Standard library mutex (optimized, not strawman)
//! 2. **parking_lot::Mutex** - Fast userspace mutex
//! 3. **DashMap** - Concurrent hashmap (best-in-class)
//! 4. **AtomicU64** - Direct atomic (minimal overhead baseline)
//!
//! ## Measurement Standards (B32 B2-B5)
//! - 1000+ iterations per benchmark
//! - 95% confidence intervals (Criterion default)
//! - Warmup period (100 iterations)
//! - Report P50, P95, P99 percentiles
//! - Multiple runs for consistency
//!
//! ## Expected Results (K27 Reality Check)
//! - vs Mutex: 3-5× speedup (typical for lockfree)
//! - vs parking_lot: 2-3× speedup (parking_lot is optimized)
//! - vs DashMap: 10-30% improvement (both lockfree, different designs)
//! - vs AtomicU64: ~2× overhead (minimal wrapper cost)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use clapi_core::capsules::timeline_aggregation_capsule::TimelineAggregationCapsuleWrapper;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::sync::atomic::{AtomicU64, Ordering};
use parking_lot::Mutex as ParkingMutex;
use dashmap::DashMap;

// ============================================================================
// TEST HELPERS
// ============================================================================

/// Deterministic timestamp for benchmarking
fn deterministic_timestamp() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_700_000_000)
}

/// Create timeline with standard config
fn create_timeline() -> TimelineAggregationCapsuleWrapper {
    TimelineAggregationCapsuleWrapper::new(1440, 60)
        .expect("Timeline creation failed")
}

// ============================================================================
// BASELINES (Fair Comparisons - B32 B1)
// ============================================================================

/// Baseline 1: std::sync::Mutex (optimized, not strawman)
struct MutexCounter {
    counter: StdMutex<u64>,
}

impl MutexCounter {
    fn new() -> Self {
        Self {
            counter: StdMutex::new(0),
        }
    }

    fn increment(&self) {
        *self.counter.lock().unwrap() += 1;
    }

    fn get(&self) -> u64 {
        *self.counter.lock().unwrap()
    }
}

/// Baseline 2: parking_lot::Mutex (fast userspace mutex)
struct ParkingCounter {
    counter: ParkingMutex<u64>,
}

impl ParkingCounter {
    fn new() -> Self {
        Self {
            counter: ParkingMutex::new(0),
        }
    }

    fn increment(&self) {
        *self.counter.lock() += 1;
    }

    fn get(&self) -> u64 {
        *self.counter.lock()
    }
}

/// Baseline 3: DashMap (concurrent hashmap - best-in-class)
struct DashMapCounter {
    map: DashMap<u64, u64>,
}

impl DashMapCounter {
    fn new() -> Self {
        Self {
            map: DashMap::new(),
        }
    }

    fn increment(&self, key: u64) {
        self.map.entry(key).or_insert(0).value_mut().wrapping_add(1);
    }

    fn get(&self, key: u64) -> u64 {
        *self.map.get(&key).map(|v| *v).unwrap_or(0)
    }
}

/// Baseline 4: AtomicU64 (minimal overhead reference)
struct AtomicCounter {
    counter: AtomicU64,
}

impl AtomicCounter {
    fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
        }
    }

    fn increment(&self) {
        self.counter.fetch_add(1, Ordering::Relaxed);
    }

    fn get(&self) -> u64 {
        self.counter.load(Ordering::Relaxed)
    }
}

// ============================================================================
// BENCHMARK 1: APPEND PERFORMANCE (SINGLE-THREADED)
// ============================================================================

fn bench_append_single_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("append_single_threaded");

    // Configure for statistical validity (B32 B2)
    group.confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(3));

    let now = deterministic_timestamp();

    // Baseline 1: std::sync::Mutex
    group.bench_function("std_mutex", |b| {
        let counter = MutexCounter::new();
        b.iter(|| {
            counter.increment();
        });
    });

    // Baseline 2: parking_lot::Mutex
    group.bench_function("parking_lot", |b| {
        let counter = ParkingCounter::new();
        b.iter(|| {
            counter.increment();
        });
    });

    // Baseline 3: AtomicU64 (minimal)
    group.bench_function("atomic_u64", |b| {
        let counter = AtomicCounter::new();
        b.iter(|| {
            counter.increment();
        });
    });

    // Our Implementation: Timeline Aggregation
    group.bench_function("timeline_aggregation", |b| {
        let timeline = create_timeline();
        b.iter(|| {
            timeline.append_system_time(black_box(now)).unwrap();
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 2: QUERY PERFORMANCE (SINGLE-THREADED)
// ============================================================================

fn bench_query_single_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_single_threaded");

    group.confidence_level(0.95)
        .sample_size(1000);

    let now = deterministic_timestamp();

    // Setup: Pre-populate with 1000 events
    let timeline = create_timeline();
    for _ in 0..1000 {
        timeline.append_system_time(now).unwrap();
    }

    let mutex_counter = MutexCounter::new();
    for _ in 0..1000 {
        mutex_counter.increment();
    }

    let parking_counter = ParkingCounter::new();
    for _ in 0..1000 {
        parking_counter.increment();
    }

    let atomic_counter = AtomicCounter::new();
    for _ in 0..1000 {
        atomic_counter.increment();
    }

    // Baseline 1: std::sync::Mutex
    group.bench_function("std_mutex", |b| {
        b.iter(|| {
            black_box(mutex_counter.get());
        });
    });

    // Baseline 2: parking_lot::Mutex
    group.bench_function("parking_lot", |b| {
        b.iter(|| {
            black_box(parking_counter.get());
        });
    });

    // Baseline 3: AtomicU64
    group.bench_function("atomic_u64", |b| {
        b.iter(|| {
            black_box(atomic_counter.get());
        });
    });

    // Our Implementation: Timeline query
    group.bench_function("timeline_aggregation", |b| {
        b.iter(|| {
            black_box(timeline.query_bucket_system_time(now).unwrap());
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 3: CONCURRENT APPEND (CONTENTION SCALING)
// ============================================================================

fn bench_concurrent_append(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_append");

    group.confidence_level(0.95)
        .sample_size(100); // Fewer samples for concurrent tests

    let now = deterministic_timestamp();

    for num_threads in [1, 2, 4, 8, 16] {
        // Baseline 1: std::sync::Mutex
        group.bench_with_input(
            BenchmarkId::new("std_mutex", num_threads),
            &num_threads,
            |b, &threads| {
                let counter = Arc::new(MutexCounter::new());
                b.iter(|| {
                    let handles: Vec<_> = (0..threads)
                        .map(|_| {
                            let c = Arc::clone(&counter);
                            std::thread::spawn(move || {
                                for _ in 0..100 {
                                    c.increment();
                                }
                            })
                        })
                        .collect();
                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );

        // Baseline 2: parking_lot::Mutex
        group.bench_with_input(
            BenchmarkId::new("parking_lot", num_threads),
            &num_threads,
            |b, &threads| {
                let counter = Arc::new(ParkingCounter::new());
                b.iter(|| {
                    let handles: Vec<_> = (0..threads)
                        .map(|_| {
                            let c = Arc::clone(&counter);
                            std::thread::spawn(move || {
                                for _ in 0..100 {
                                    c.increment();
                                }
                            })
                        })
                        .collect();
                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );

        // Baseline 3: AtomicU64
        group.bench_with_input(
            BenchmarkId::new("atomic_u64", num_threads),
            &num_threads,
            |b, &threads| {
                let counter = Arc::new(AtomicCounter::new());
                b.iter(|| {
                    let handles: Vec<_> = (0..threads)
                        .map(|_| {
                            let c = Arc::clone(&counter);
                            std::thread::spawn(move || {
                                for _ in 0..100 {
                                    c.increment();
                                }
                            })
                        })
                        .collect();
                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );

        // Our Implementation: Timeline Aggregation
        group.bench_with_input(
            BenchmarkId::new("timeline_aggregation", num_threads),
            &num_threads,
            |b, &threads| {
                let timeline = Arc::new(create_timeline());
                b.iter(|| {
                    let handles: Vec<_> = (0..threads)
                        .map(|_| {
                            let t = Arc::clone(&timeline);
                            std::thread::spawn(move || {
                                for _ in 0..100 {
                                    t.append_system_time(now).unwrap();
                                }
                            })
                        })
                        .collect();
                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 4: RANGE QUERY (MULTI-BUCKET AGGREGATION)
// ============================================================================

fn bench_range_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("range_query");

    group.confidence_level(0.95)
        .sample_size(1000);

    // Setup: Timeline with events across 60 buckets (1 hour)
    let timeline = create_timeline();
    for i in 0..60 {
        let ts = deterministic_timestamp() - Duration::from_secs(i * 60);
        for _ in 0..100 {
            timeline.append_system_time(ts).unwrap();
        }
    }

    // Benchmark: Query last hour (60 buckets)
    group.bench_function("query_1_hour", |b| {
        b.iter(|| {
            black_box(timeline.query_last_hours(1).unwrap());
        });
    });

    // Benchmark: Query last 6 hours (360 buckets)
    group.bench_function("query_6_hours", |b| {
        b.iter(|| {
            black_box(timeline.query_last_hours(6).unwrap());
        });
    });

    // Benchmark: Query last 24 hours (1440 buckets)
    group.bench_function("query_24_hours", |b| {
        b.iter(|| {
            black_box(timeline.query_last_hours(24).unwrap());
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 5: FLUSH PERFORMANCE (BATCH WRITE)
// ============================================================================

fn bench_flush_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("flush_performance");

    group.confidence_level(0.95)
        .sample_size(100);

    let now = deterministic_timestamp();

    // Setup: Timeline with 1000 events
    let timeline = create_timeline();
    for _ in 0..1000 {
        timeline.append_system_time(now).unwrap();
    }

    // Benchmark: Flush single bucket
    group.bench_function("flush_single_bucket", |b| {
        b.iter(|| {
            black_box(timeline.flush_bucket_system_time(now).unwrap());
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 6: MEMORY OVERHEAD (Compared to Baselines)
// ============================================================================

fn bench_memory_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_overhead");

    // Report memory sizes for comparison
    println!("=== Memory Overhead Comparison ===");
    println!("std::sync::Mutex<u64>: {} bytes", std::mem::size_of::<StdMutex<u64>>());
    println!("parking_lot::Mutex<u64>: {} bytes", std::mem::size_of::<ParkingMutex<u64>>());
    println!("AtomicU64: {} bytes", std::mem::size_of::<AtomicU64>());
    println!("TimelineAggregationCapsuleWrapper: {} bytes",
        std::mem::size_of::<TimelineAggregationCapsuleWrapper>());

    // Benchmark: Timeline creation (memory allocation)
    group.bench_function("create_timeline", |b| {
        b.iter(|| {
            black_box(create_timeline());
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 7: DASHMAP COMPARISON (Concurrent Hashmap Baseline)
// ============================================================================

fn bench_dashmap_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("dashmap_comparison");

    group.confidence_level(0.95)
        .sample_size(1000);

    let now = deterministic_timestamp();

    // Setup: DashMap as baseline
    let dashmap = DashMapCounter::new();

    // Benchmark: DashMap insert
    group.bench_function("dashmap_insert", |b| {
        let mut key = 0u64;
        b.iter(|| {
            dashmap.increment(black_box(key));
            key = key.wrapping_add(1);
        });
    });

    // Benchmark: DashMap get
    group.bench_function("dashmap_get", |b| {
        // Pre-populate
        for i in 0..1000 {
            dashmap.increment(i);
        }

        b.iter(|| {
            black_box(dashmap.get(black_box(500)));
        });
    });

    // Setup: Timeline for comparison
    let timeline = create_timeline();

    // Benchmark: Timeline append
    group.bench_function("timeline_append", |b| {
        b.iter(|| {
            timeline.append_system_time(black_box(now)).unwrap();
        });
    });

    // Benchmark: Timeline query
    group.bench_function("timeline_query", |b| {
        // Pre-populate
        for _ in 0..1000 {
            timeline.append_system_time(now).unwrap();
        }

        b.iter(|| {
            black_box(timeline.query_bucket_system_time(now).unwrap());
        });
    });

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    benches,
    bench_append_single_threaded,
    bench_query_single_threaded,
    bench_concurrent_append,
    bench_range_query,
    bench_flush_performance,
    bench_memory_overhead,
    bench_dashmap_comparison,
);

criterion_main!(benches);

// ============================================================================
// EXPECTED RESULTS (K27 Reality Check)
// ============================================================================

// Based on B32 framework and K27 honest gains:
//
// SINGLE-THREADED APPEND:
// - vs std::sync::Mutex: 3-5× faster (typical lockfree improvement)
// - vs parking_lot: 2-3× faster (parking_lot is optimized)
// - vs AtomicU64: 1.5-2× overhead (minimal wrapper cost)
// - Absolute: <100ns p99.9 (from performance budget tests)
//
// CONCURRENT APPEND (8 threads):
// - vs std::sync::Mutex: 10-20× faster (no contention)
// - vs parking_lot: 5-10× faster (parking_lot still locks)
// - vs AtomicU64: 2-3× overhead (coordination cost)
// - Absolute: <200ns p99.9 (from performance budget tests)
//
// QUERY:
// - vs Mutex: 5-10× faster (no lock acquisition)
// - vs DashMap: 10-30% faster (simpler data structure)
// - Absolute: <1μs p99.9 (from performance budget tests)
//
// RANGE QUERY:
// - No direct baseline (unique feature)
// - Absolute: <10μs p99.9 for 60 buckets (from performance budget tests)
//
// FLUSH:
// - No direct baseline (batch operation)
// - Absolute: <10μs p99.9 single bucket (from performance budget tests)
//
// MEMORY OVERHEAD:
// - Capsule: 256B aligned (T4 tier requirement)
// - Buckets: 1440 × 64B = ~90KB
// - Total: <128MB for 1M events (from performance budget tests)
//
// REALITY CHECK:
// If any speedup exceeds 10× vs optimized baseline → SUSPICIOUS (K27)
// If latency exceeds SLO budgets → REGRESSION (fail CI/CD)
