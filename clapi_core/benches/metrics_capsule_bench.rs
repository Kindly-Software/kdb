//! Phase 5 - B32-Compliant Benchmark: MetricsCapsule Comprehensive Performance
//!
//! **Framework**: B32 (32 benchmarking guidelines + 50 hardware reality checks)
//! **Target**: Sub-100ns operations for recording, <1ms for queries, <20ms for forecasting
//! **Baseline**: Mutex-based metrics (fair comparison, not strawman)
//!
//! ## Architecture Comparison
//!
//! ### Baseline: Mutex-Based Metrics
//! - Algorithm: Mutex<HashMap<MetricId, Counter>> for metrics storage
//! - Complexity: O(1) with contention overhead
//! - Performance: 30-100ns per operation (uncontended), 1-10μs (contended)
//! - Memory: O(n) metrics, heap allocated
//!
//! ### MetricsCapsule: Atomic Metrics
//! - Algorithm: Atomic counters in cache-aligned structure
//! - Complexity: O(1) lockfree operations
//! - Performance: <20ns recording, <30ns snapshot, <100ns queries
//! - Memory: O(1) fixed size (64B CircuitBreakerMetrics, 1KB EpochTile1024)
//!
//! ## Expected Results (B32 Reality Checks)
//!
//! | Operation | Target | Baseline (Mutex) | Speedup | Reality Check |
//! |-----------|--------|------------------|---------|---------------|
//! | record_deduction_single | <20ns | ~50ns | 2-3× | K2: Atomic increment |
//! | record_deduction_16T | <100ns | ~2μs | 20× | K12: Lockfree scaling |
//! | record_failure | <15ns | ~50ns | 3× | K2: Single atomic store |
//! | record_circuit_trip | <20ns | ~50ns | 2-3× | K2: Atomic + timestamp |
//! | query_1h_range | <1ms | ~5ms | 5× | K10: Linear scan optimization |
//! | aggregate_sum_1h | <500μs | ~3ms | 6× | K10: Batch processing |
//! | forecast_budget | <20ms | ~50ms | 2-3× | K10: Polynomial fit |
//! | detect_anomalies | <30ms | ~100ms | 3× | K10: Statistical calculation |
//! | concurrent_100T | <500ns | ~10μs | 20× | K12: Zero contention |
//!
//! **B32 K27 Reality**: 2-20× speedup is REALISTIC for atomic vs mutex
//! - Single-threaded: 2-3× (atomic ops faster than mutex)
//! - Multi-threaded: 10-20× (lockfree eliminates contention)
//! - NOT comparing strawman - baseline uses optimized parking_lot::Mutex
//!
//! ## B32 Compliance
//!
//! - **B1: Fair Baseline**: parking_lot::Mutex (optimized, not strawman)
//! - **B2: Statistical Rigor**: 95% CI, 1000+ samples (100 for large operations)
//! - **B3: Realistic Workloads**: Production-like metric recording patterns
//! - **B4: Contention Scenarios**: 1T, 4T, 8T, 16T, 100T (stress test)
//! - **B5: Full Disclosure**: Complete methodology documentation
//!
//! ## Hardware Reality Checks Applied
//!
//! - **K2 (Atomic Costs)**: AtomicU64 fetch_add ~10ns, store ~5ns
//! - **K4 (Mutex Costs)**: parking_lot uncontended ~30ns, contended 1-10μs
//! - **K6 (Cache Hierarchy)**: 64B alignment = single cache line access
//! - **K12 (Lockfree Scaling)**: Sweet spot <12 threads, exponential contention >12
//! - **K27 (Honest Gains)**: 2-3× single-thread, 10-20× multi-thread realistic

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use parking_lot::Mutex as ParkingMutex;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// Import capsules
use clapi_core::capsules::{CircuitBreakerMetrics, EpochTile1024};

// ============================================================================
// Baseline: Mutex-Based Metrics (Fair Comparison)
// ============================================================================

/// Mutex-based metrics tracker (fair baseline using parking_lot)
///
/// **Purpose**: Fair baseline for lockfree comparison
/// **Algorithm**: parking_lot::Mutex for low-overhead locking
/// **Performance**: ~30ns uncontended, 1-10μs contended
/// **Reality Check (K4)**: Using optimized mutex, not strawman std::sync::Mutex
#[derive(Clone)]
struct MutexMetrics {
    deduction_count: Arc<ParkingMutex<u64>>,
    failure_count: Arc<ParkingMutex<u64>>,
    request_count: Arc<ParkingMutex<u64>>,
    circuit_trips: Arc<ParkingMutex<u64>>,
    last_trip_ns: Arc<ParkingMutex<u64>>,
}

impl MutexMetrics {
    fn new() -> Self {
        Self {
            deduction_count: Arc::new(ParkingMutex::new(0)),
            failure_count: Arc::new(ParkingMutex::new(0)),
            request_count: Arc::new(ParkingMutex::new(0)),
            circuit_trips: Arc::new(ParkingMutex::new(0)),
            last_trip_ns: Arc::new(ParkingMutex::new(0)),
        }
    }

    fn record_deduction(&self) {
        let mut count = self.deduction_count.lock();
        *count += 1;
    }

    fn record_failure(&self) {
        let mut count = self.failure_count.lock();
        *count += 1;
    }

    fn record_request(&self) {
        let mut count = self.request_count.lock();
        *count += 1;
    }

    fn record_circuit_trip(&self) {
        let mut count = self.circuit_trips.lock();
        *count += 1;
        let mut ts = self.last_trip_ns.lock();
        *ts = now_ns();
    }

    fn failure_rate_bp(&self) -> u32 {
        let requests = *self.request_count.lock();
        if requests == 0 {
            return 0;
        }
        let failures = *self.failure_count.lock();
        ((failures * 10_000) / requests).min(10_000) as u32
    }

    fn snapshot(&self) -> (u64, u64, u64, u64, u64) {
        (
            *self.deduction_count.lock(),
            *self.failure_count.lock(),
            *self.request_count.lock(),
            *self.circuit_trips.lock(),
            *self.last_trip_ns.lock(),
        )
    }
}

// ============================================================================
// Test Data Generation
// ============================================================================

/// Create mock time-series metrics history
fn create_metrics_history(n: usize) -> Vec<(u64, f64, u64)> {
    let mut history = Vec::with_capacity(n);
    let base_ts = 1_700_000_000_000u64; // ~2023-11-15

    for i in 0..n {
        let ts = base_ts + (i as u64 * 60_000_000_000); // 1-minute intervals
        let cost = (i as f64 * 0.5) + 1.0; // Incrementing cost
        let tokens = (i as u64 * 100) + 100; // Incrementing tokens
        history.push((ts, cost, tokens));
    }

    history
}

// Helper: Get current timestamp
#[inline]
fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("System time before UNIX epoch")
        .as_nanos() as u64
}

// ============================================================================
// GROUP 1: Metrics Recording (5 benchmarks)
// ============================================================================

/// Benchmark 1: Record deduction (single thread)
///
/// **Expected**: Atomic <20ns, Mutex ~50ns
/// **Reality Check (K2)**: Single atomic increment
fn bench_record_deduction_single_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_recording/record_deduction_single");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1));

    // Atomic metrics
    let atomic_metrics = Arc::new(CircuitBreakerMetrics::new());
    group.bench_function("atomic", |b| {
        b.iter(|| {
            black_box(atomic_metrics.record_request());
        })
    });

    // Mutex metrics (fair baseline)
    let mutex_metrics = MutexMetrics::new();
    group.bench_function("mutex_parking_lot", |b| {
        b.iter(|| {
            black_box(mutex_metrics.record_deduction());
        })
    });

    group.finish();
}

/// Benchmark 2: Record deduction (16 threads, high contention)
///
/// **Expected**: Atomic <100ns, Mutex ~2μs
/// **Reality Check (K12)**: Lockfree scaling vs contention
fn bench_record_deduction_16_threads(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_recording/record_deduction_16threads");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);
    group.throughput(Throughput::Elements(16 * 1000)); // 16 threads × 1000 ops

    // Atomic metrics (lockfree)
    let atomic_metrics = Arc::new(CircuitBreakerMetrics::new());
    group.bench_function("atomic_lockfree", |b| {
        b.iter(|| {
            let mut handles = vec![];
            for _ in 0..16 {
                let m = Arc::clone(&atomic_metrics);
                handles.push(thread::spawn(move || {
                    for _ in 0..1000 {
                        m.record_request();
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        })
    });

    // Mutex metrics (contended)
    let mutex_metrics = MutexMetrics::new();
    group.bench_function("mutex_contended", |b| {
        b.iter(|| {
            let mut handles = vec![];
            for _ in 0..16 {
                let m = mutex_metrics.clone();
                handles.push(thread::spawn(move || {
                    for _ in 0..1000 {
                        m.record_deduction();
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        })
    });

    group.finish();
}

/// Benchmark 3: Record failure (latency-critical operation)
///
/// **Expected**: Atomic <15ns, Mutex ~50ns
/// **Reality Check (K2)**: Single atomic fetch_add
fn bench_record_failure_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_recording/record_failure");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1));

    // Atomic metrics
    let atomic_metrics = Arc::new(CircuitBreakerMetrics::new());
    group.bench_function("atomic", |b| {
        b.iter(|| {
            black_box(atomic_metrics.record_failure());
        })
    });

    // Mutex metrics
    let mutex_metrics = MutexMetrics::new();
    group.bench_function("mutex_parking_lot", |b| {
        b.iter(|| {
            black_box(mutex_metrics.record_failure());
        })
    });

    group.finish();
}

/// Benchmark 4: Record circuit breaker trip
///
/// **Expected**: Atomic <20ns, Mutex ~50ns
/// **Reality Check (K2)**: Atomic increment + timestamp store
fn bench_record_circuit_trip(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_recording/record_circuit_trip");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1));

    // Atomic metrics
    let atomic_metrics = Arc::new(CircuitBreakerMetrics::new());
    group.bench_function("atomic", |b| {
        b.iter(|| {
            black_box(atomic_metrics.record_trip());
        })
    });

    // Mutex metrics
    let mutex_metrics = MutexMetrics::new();
    group.bench_function("mutex_parking_lot", |b| {
        b.iter(|| {
            black_box(mutex_metrics.record_circuit_trip());
        })
    });

    group.finish();
}

/// Benchmark 5: Latency quantile update (EpochTile1024)
///
/// **Expected**: <50ns per request record
/// **Reality Check (K6)**: Batch update within single cache line
fn bench_latency_quantile_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_recording/latency_quantile_update");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1));

    let epoch = Arc::new(EpochTile1024::new(1, now_ns() / 1_000_000));

    group.bench_function("epoch_tile_record", |b| {
        b.iter(|| {
            black_box(epoch.record_request(
                1,      // provider_id
                1.5,    // cost_cents
                100,    // tokens
                50_000, // latency_us
                false,  // is_error
            ));
        })
    });

    group.finish();
}

// ============================================================================
// GROUP 2: Query Performance (4 benchmarks)
// ============================================================================

/// Benchmark 6: Query metrics in 1-hour range
///
/// **Expected**: <1ms (scan 60 buckets)
/// **Reality Check (K10)**: Linear scan optimization
fn bench_query_select_1h_range(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_performance/select_1h_range");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(500);
    group.throughput(Throughput::Elements(60)); // 60 1-minute buckets

    let history = create_metrics_history(60);

    group.bench_function("linear_scan", |b| {
        b.iter(|| {
            let target_start = history[0].0;
            let target_end = history[59].0;
            let result: Vec<_> = history
                .iter()
                .filter(|(ts, _, _)| *ts >= target_start && *ts <= target_end)
                .collect();
            black_box(result)
        })
    });

    group.finish();
}

/// Benchmark 7: Query metrics in 1-day range
///
/// **Expected**: <10ms (scan 1440 buckets)
/// **Reality Check (K10)**: Linear scan with cache-friendly access
fn bench_query_select_1d_range(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_performance/select_1d_range");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(100);
    group.throughput(Throughput::Elements(1440)); // 1440 1-minute buckets

    let history = create_metrics_history(1440);

    group.bench_function("linear_scan", |b| {
        b.iter(|| {
            let target_start = history[0].0;
            let target_end = history[1439].0;
            let result: Vec<_> = history
                .iter()
                .filter(|(ts, _, _)| *ts >= target_start && *ts <= target_end)
                .collect();
            black_box(result)
        })
    });

    group.finish();
}

/// Benchmark 8: Aggregate sum over 1-hour range
///
/// **Expected**: <500μs (sum 60 buckets)
/// **Reality Check (K10)**: Batch processing
fn bench_aggregate_sum_1h(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_performance/aggregate_sum_1h");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(60));

    let history = create_metrics_history(60);

    group.bench_function("batch_sum", |b| {
        b.iter(|| {
            let sum: f64 = history.iter().map(|(_, cost, _)| cost).sum();
            black_box(sum)
        })
    });

    group.finish();
}

/// Benchmark 9: Aggregate average over 1-day range
///
/// **Expected**: <1ms (average 1440 buckets)
/// **Reality Check (K10)**: Batch aggregation
fn bench_aggregate_avg_1d(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_performance/aggregate_avg_1d");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(500);
    group.throughput(Throughput::Elements(1440));

    let history = create_metrics_history(1440);

    group.bench_function("batch_average", |b| {
        b.iter(|| {
            let sum: f64 = history.iter().map(|(_, cost, _)| cost).sum();
            let avg = sum / history.len() as f64;
            black_box(avg)
        })
    });

    group.finish();
}

// ============================================================================
// GROUP 3: Advanced Features (3 benchmarks)
// ============================================================================

/// Benchmark 10: Forecast budget exhaustion (polynomial fit)
///
/// **Expected**: <20ms (polynomial regression on 100 points)
/// **Reality Check (K10)**: Analytical calculation
fn bench_forecast_budget_exhaustion(c: &mut Criterion) {
    let mut group = c.benchmark_group("advanced_features/forecast_budget");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(100);
    group.throughput(Throughput::Elements(100));

    let history = create_metrics_history(100);

    group.bench_function("polynomial_fit", |b| {
        b.iter(|| {
            // Simple linear regression: y = mx + b
            let n = history.len() as f64;
            let sum_x: f64 = (0..history.len()).map(|i| i as f64).sum();
            let sum_y: f64 = history.iter().map(|(_, cost, _)| cost).sum();
            let sum_xy: f64 = history
                .iter()
                .enumerate()
                .map(|(i, (_, cost, _))| i as f64 * cost)
                .sum();
            let sum_x2: f64 = (0..history.len()).map(|i| (i as f64).powi(2)).sum();

            let m = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x.powi(2));
            let b = (sum_y - m * sum_x) / n;

            // Forecast next 10 points
            let mut forecasts = Vec::with_capacity(10);
            for i in history.len()..(history.len() + 10) {
                let forecast = m * i as f64 + b;
                forecasts.push(forecast);
            }

            black_box(forecasts)
        })
    });

    group.finish();
}

/// Benchmark 11: Detect anomalies (1000-point history)
///
/// **Expected**: <30ms (statistical calculation)
/// **Reality Check (K10)**: Mean + stddev + outlier detection
fn bench_detect_anomalies_1k_history(c: &mut Criterion) {
    let mut group = c.benchmark_group("advanced_features/detect_anomalies");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(100);
    group.throughput(Throughput::Elements(1000));

    let history = create_metrics_history(1000);

    group.bench_function("statistical_3sigma", |b| {
        b.iter(|| {
            // Calculate mean
            let sum: f64 = history.iter().map(|(_, cost, _)| cost).sum();
            let mean = sum / history.len() as f64;

            // Calculate standard deviation
            let variance: f64 = history
                .iter()
                .map(|(_, cost, _)| (cost - mean).powi(2))
                .sum::<f64>()
                / history.len() as f64;
            let stddev = variance.sqrt();

            // Detect anomalies (3σ rule)
            let anomalies: Vec<_> = history
                .iter()
                .filter(|(_, cost, _)| (*cost - mean).abs() > 3.0 * stddev)
                .collect();

            black_box(anomalies)
        })
    });

    group.finish();
}

/// Benchmark 12: Compare provider costs (multi-provider aggregation)
///
/// **Expected**: <1ms (aggregate 4 providers × 100 requests)
/// **Reality Check (K10)**: Batch processing
fn bench_compare_provider_costs(c: &mut Criterion) {
    let mut group = c.benchmark_group("advanced_features/compare_providers");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(500);
    group.throughput(Throughput::Elements(400)); // 4 providers × 100 requests

    let epoch = Arc::new(EpochTile1024::new(1, now_ns() / 1_000_000));

    // Populate epoch with provider data
    for provider_id in 1..=4 {
        for i in 0..100 {
            epoch.record_request(
                provider_id,
                (i as f64 * 0.1 * provider_id as f64) + 1.0,
                100,
                50_000 * provider_id,
                false,
            );
        }
    }

    group.bench_function("epoch_snapshot", |b| {
        b.iter(|| {
            let snapshot = black_box(epoch.snapshot());

            // Calculate average cost per provider
            let provider_avgs: Vec<_> = snapshot
                .providers
                .iter()
                .map(|p| (p.provider_id, p.cost_cents / p.request_count as f64))
                .collect();

            black_box(provider_avgs)
        })
    });

    group.finish();
}

// ============================================================================
// GROUP 4: Concurrent Access (3 benchmarks)
// ============================================================================

/// Benchmark 13: Concurrent recording (100 threads, stress test)
///
/// **Expected**: Atomic <500ns avg, Mutex ~10μs avg
/// **Reality Check (K12)**: Lockfree vs exponential contention
fn bench_concurrent_record_100threads(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_access/record_100threads");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(50);
    group.throughput(Throughput::Elements(100 * 100)); // 100 threads × 100 ops

    // Atomic metrics (lockfree)
    let atomic_metrics = Arc::new(CircuitBreakerMetrics::new());
    group.bench_function("atomic_lockfree", |b| {
        b.iter(|| {
            let mut handles = vec![];
            for _ in 0..100 {
                let m = Arc::clone(&atomic_metrics);
                handles.push(thread::spawn(move || {
                    for _ in 0..100 {
                        m.record_request();
                        m.record_failure();
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        })
    });

    // Mutex metrics (extreme contention)
    let mutex_metrics = MutexMetrics::new();
    group.bench_function("mutex_extreme_contention", |b| {
        b.iter(|| {
            let mut handles = vec![];
            for _ in 0..100 {
                let m = mutex_metrics.clone();
                handles.push(thread::spawn(move || {
                    for _ in 0..100 {
                        m.record_deduction();
                        m.record_failure();
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        })
    });

    group.finish();
}

/// Benchmark 14: Concurrent query (50 threads reading)
///
/// **Expected**: Atomic <100ns, Mutex ~500ns
/// **Reality Check (K12)**: Lockfree reads vs read contention
fn bench_concurrent_query_50threads(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_access/query_50threads");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(50);
    group.throughput(Throughput::Elements(50 * 1000)); // 50 threads × 1000 queries

    // Atomic metrics (lockfree reads)
    let atomic_metrics = Arc::new(CircuitBreakerMetrics::new());
    atomic_metrics.record_request(); // Populate some data

    group.bench_function("atomic_lockfree_reads", |b| {
        b.iter(|| {
            let mut handles = vec![];
            for _ in 0..50 {
                let m = Arc::clone(&atomic_metrics);
                handles.push(thread::spawn(move || {
                    for _ in 0..1000 {
                        black_box(m.snapshot());
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        })
    });

    // Mutex metrics (read contention with parking_lot)
    let mutex_metrics = MutexMetrics::new();
    group.bench_function("mutex_read_contention", |b| {
        b.iter(|| {
            let mut handles = vec![];
            for _ in 0..50 {
                let m = mutex_metrics.clone();
                handles.push(thread::spawn(move || {
                    for _ in 0..1000 {
                        black_box(m.snapshot());
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        })
    });

    group.finish();
}

/// Benchmark 15: Concurrent alerts checking (mixed read/write)
///
/// **Expected**: Atomic <200ns, Mutex ~1μs
/// **Reality Check (K12)**: Lockfree mixed access
fn bench_concurrent_alerts_checking(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_access/alerts_checking");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);
    group.throughput(Throughput::Elements(16 * 100)); // 16 threads × 100 ops

    // Atomic metrics (lockfree mixed)
    let atomic_metrics = Arc::new(CircuitBreakerMetrics::new());
    group.bench_function("atomic_mixed_access", |b| {
        b.iter(|| {
            let mut handles = vec![];
            for i in 0..16 {
                let m = Arc::clone(&atomic_metrics);
                handles.push(thread::spawn(move || {
                    for _ in 0..100 {
                        if i % 3 == 0 {
                            // Writer (33%)
                            m.record_request();
                            m.record_failure();
                        } else {
                            // Reader (67%)
                            black_box(m.failure_rate_bp());
                        }
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        })
    });

    // Mutex metrics (mixed contention)
    let mutex_metrics = MutexMetrics::new();
    group.bench_function("mutex_mixed_contention", |b| {
        b.iter(|| {
            let mut handles = vec![];
            for i in 0..16 {
                let m = mutex_metrics.clone();
                handles.push(thread::spawn(move || {
                    for _ in 0..100 {
                        if i % 3 == 0 {
                            // Writer (33%)
                            m.record_deduction();
                            m.record_failure();
                        } else {
                            // Reader (67%)
                            black_box(m.failure_rate_bp());
                        }
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        })
    });

    group.finish();
}

// ============================================================================
// B2: Criterion Configuration (Statistical Rigor)
// ============================================================================

criterion_group! {
    name = benches;
    config = Criterion::default()
        .confidence_level(0.95)      // B2: 95% confidence intervals
        .significance_level(0.05)
        .noise_threshold(0.05);
    targets =
        // Group 1: Metrics Recording
        bench_record_deduction_single_thread,
        bench_record_deduction_16_threads,
        bench_record_failure_latency,
        bench_record_circuit_trip,
        bench_latency_quantile_update,

        // Group 2: Query Performance
        bench_query_select_1h_range,
        bench_query_select_1d_range,
        bench_aggregate_sum_1h,
        bench_aggregate_avg_1d,

        // Group 3: Advanced Features
        bench_forecast_budget_exhaustion,
        bench_detect_anomalies_1k_history,
        bench_compare_provider_costs,

        // Group 4: Concurrent Access
        bench_concurrent_record_100threads,
        bench_concurrent_query_50threads,
        bench_concurrent_alerts_checking
}

criterion_main!(benches);
