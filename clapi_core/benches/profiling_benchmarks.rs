//! B32 Framework Benchmarks for Latency Profiling
//!
//! # B32 Compliance
//!
//! - **Fair Baseline**: Compare against mutex-protected histogram
//! - **Statistical Rigor**: 1000+ iterations, 95% CI via Criterion
//! - **Honest Reporting**: Document where optimization helps AND hurts
//! - **Reality Check**: 10-50% typical, 2-10× exceptional
//!
//! # Benchmarks
//!
//! 1. **record()**: Lockfree vs Mutex (target: 3-10× speedup)
//! 2. **percentile()**: O(1) bucket scan vs sorted array (target: 10-50× speedup)
//! 3. **stats()**: Full snapshot (target: <100ns)

use clapi_core::profiling::capsule::LatencyHistogramCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::{Arc, Mutex};
use std::thread;

// ============================================================================
// FAIR BASELINE: Mutex-Protected Histogram
// ============================================================================

struct MutexHistogram {
    data: Mutex<Vec<u64>>,
}

impl MutexHistogram {
    fn new() -> Self {
        Self {
            data: Mutex::new(Vec::new()),
        }
    }

    fn record(&self, latency: u64) {
        let mut data = self.data.lock().unwrap();
        data.push(latency);
    }

    fn percentile(&self, p: f64) -> u64 {
        let mut data = self.data.lock().unwrap();
        if data.is_empty() {
            return 0;
        }
        data.sort_unstable();
        let index = ((data.len() as f64 * p / 100.0).ceil() as usize).min(data.len() - 1);
        data[index]
    }

    fn count(&self) -> usize {
        self.data.lock().unwrap().len()
    }
}

// ============================================================================
// BENCHMARK 1: record() - Lockfree vs Mutex
// ============================================================================

fn bench_record_lockfree(c: &mut Criterion) {
    c.bench_function("record_lockfree", |b| {
        let histogram = LatencyHistogramCapsule::new();
        let mut counter = 0u64;
        b.iter(|| {
            counter += 1;
            histogram.record(black_box(counter % 10000));
        });
    });
}

fn bench_record_mutex(c: &mut Criterion) {
    c.bench_function("record_mutex", |b| {
        let histogram = MutexHistogram::new();
        let mut counter = 0u64;
        b.iter(|| {
            counter += 1;
            histogram.record(black_box(counter % 10000));
        });
    });
}

// ============================================================================
// BENCHMARK 2: percentile() - Lockfree vs Sorted Array
// ============================================================================

fn bench_percentile_lockfree(c: &mut Criterion) {
    let histogram = LatencyHistogramCapsule::new();
    for i in 1..=10000 {
        histogram.record(i);
    }

    c.bench_function("percentile_lockfree_p99", |b| {
        b.iter(|| {
            black_box(histogram.percentile(99.0));
        });
    });
}

fn bench_percentile_mutex(c: &mut Criterion) {
    let histogram = MutexHistogram::new();
    for i in 1..=10000 {
        histogram.record(i);
    }

    c.bench_function("percentile_mutex_p99", |b| {
        b.iter(|| {
            black_box(histogram.percentile(99.0));
        });
    });
}

// ============================================================================
// BENCHMARK 3: stats() - Full Snapshot
// ============================================================================

fn bench_stats_snapshot(c: &mut Criterion) {
    let histogram = LatencyHistogramCapsule::new();
    for i in 1..=10000 {
        histogram.record(i);
    }

    c.bench_function("stats_snapshot", |b| {
        b.iter(|| {
            black_box(histogram.stats());
        });
    });
}

// ============================================================================
// BENCHMARK 4: Concurrent Recording (Scalability)
// ============================================================================

fn bench_concurrent_recording(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_recording");

    for num_threads in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::new("lockfree", num_threads),
            &num_threads,
            |b, &threads| {
                b.iter(|| {
                    let histogram = Arc::new(LatencyHistogramCapsule::new());
                    let mut handles = vec![];

                    for _ in 0..threads {
                        let hist = Arc::clone(&histogram);
                        handles.push(thread::spawn(move || {
                            for i in 0..1000 {
                                hist.record(i);
                            }
                        }));
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    black_box(histogram.count());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("mutex", num_threads),
            &num_threads,
            |b, &threads| {
                b.iter(|| {
                    let histogram = Arc::new(MutexHistogram::new());
                    let mut handles = vec![];

                    for _ in 0..threads {
                        let hist = Arc::clone(&histogram);
                        handles.push(thread::spawn(move || {
                            for i in 0..1000 {
                                hist.record(i);
                            }
                        }));
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    black_box(histogram.count());
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 5: Mean Calculation
// ============================================================================

fn bench_mean_calculation(c: &mut Criterion) {
    let histogram = LatencyHistogramCapsule::new();
    for i in 1..=10000 {
        histogram.record(i);
    }

    c.bench_function("mean_calculation", |b| {
        b.iter(|| {
            black_box(histogram.mean_ns());
        });
    });
}

// ============================================================================
// BENCHMARK 6: Multiple Percentiles
// ============================================================================

fn bench_multiple_percentiles(c: &mut Criterion) {
    let histogram = LatencyHistogramCapsule::new();
    for i in 1..=10000 {
        histogram.record(i);
    }

    c.bench_function("multiple_percentiles", |b| {
        b.iter(|| {
            black_box(histogram.percentile(50.0));
            black_box(histogram.percentile(90.0));
            black_box(histogram.percentile(95.0));
            black_box(histogram.percentile(99.0));
            black_box(histogram.percentile(99.9));
        });
    });
}

criterion_group!(
    benches,
    bench_record_lockfree,
    bench_record_mutex,
    bench_percentile_lockfree,
    bench_percentile_mutex,
    bench_stats_snapshot,
    bench_concurrent_recording,
    bench_mean_calculation,
    bench_multiple_percentiles,
);

criterion_main!(benches);
