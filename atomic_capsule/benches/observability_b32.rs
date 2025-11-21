//! B32 Fair Benchmarking: ObservabilityCapsule vs Prometheus Mutex-Based Client
//!
//! ## B32 Framework Compliance
//!
//! - **Baseline**: Prometheus-style mutex-based metrics (RwLock<HashMap>)
//! - **Optimized**: ObservabilityCapsule (T6 Mixed: T1+T2+T5)
//! - **Hardware**: Same machine, same compiler, same workload
//! - **Iterations**: 1000+ (95% CI via Criterion.rs)
//! - **Expected Speedup**: 10-20× (T1 <15ns + T2 8× + T5 <10ns compound)
//!
//! ## Performance Reality Check
//! - 10-50% typical speedup
//! - 2-10× exceptional speedup (requires validation)
//! - 10-20× breakthrough speedup (compound optimization, extensive validation)
//!
//! ## Measurement Methodology
//! - Single-threaded: increment_metric() latency
//! - Multi-threaded: 8-thread contention stress test
//! - SIMD aggregation: 8× parallel reduction speedup
//! - Trace appending: Ring buffer vs vector append

#![cfg(feature = "observability")]

use atomic_capsule::composite::{ObservabilityCapsule, TraceEvent, TraceRingBuffer};
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

// ============================================================================
// § 1: Baseline - Prometheus-Style Mutex-Based Metrics
// ============================================================================

/// Prometheus-style metrics with mutex-based counters
#[derive(Clone)]
struct PrometheusMetrics {
    request_counter: Arc<Mutex<u64>>,
    error_counter: Arc<Mutex<u64>>,
    duration_histogram: Arc<RwLock<HashMap<usize, u64>>>, // Bucket -> count
    trace_log: Arc<Mutex<Vec<(u64, u64, u64, u32, u16, u16)>>>, // Trace events
}

impl PrometheusMetrics {
    fn new() -> Self {
        Self {
            request_counter: Arc::new(Mutex::new(0)),
            error_counter: Arc::new(Mutex::new(0)),
            duration_histogram: Arc::new(RwLock::new(HashMap::new())),
            trace_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn increment_requests(&self) {
        let mut counter = self.request_counter.lock().unwrap();
        *counter += 1;
    }

    fn increment_errors(&self) {
        let mut counter = self.error_counter.lock().unwrap();
        *counter += 1;
    }

    fn record_duration_us(&self, duration_us: u32) {
        let bucket = match duration_us {
            0..=1000 => 0,
            1001..=5000 => 1,
            5001..=10000 => 2,
            10001..=50000 => 3,
            50001..=100000 => 4,
            100001..=500000 => 5,
            500001..=1000000 => 6,
            _ => 7,
        };

        let mut histogram = self.duration_histogram.write().unwrap();
        *histogram.entry(bucket).or_insert(0) += 1;
    }

    fn append_trace(&self, trace_id_hi: u64, trace_id_lo: u64, span_id: u64, timestamp_us: u32, duration_us: u16, flags: u16) {
        let mut traces = self.trace_log.lock().unwrap();
        traces.push((trace_id_hi, trace_id_lo, span_id, timestamp_us, duration_us, flags));
    }

    fn aggregate_durations(&self) -> u64 {
        let histogram = self.duration_histogram.read().unwrap();
        histogram.values().sum()
    }

    fn load_request_count(&self) -> u64 {
        *self.request_counter.lock().unwrap()
    }

    fn load_error_count(&self) -> u64 {
        *self.error_counter.lock().unwrap()
    }
}

// ============================================================================
// § 2: Single-Threaded Benchmarks
// ============================================================================

fn bench_increment_requests_single_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("increment_requests_single_threaded");

    // Baseline: Prometheus mutex-based
    group.bench_function("prometheus_baseline", |b| {
        let metrics = PrometheusMetrics::new();
        b.iter(|| {
            metrics.increment_requests();
        });
    });

    // Optimized: ObservabilityCapsule (T1 Atomic)
    group.bench_function("observability_capsule", |b| {
        let obs = ObservabilityCapsule::new();
        b.iter(|| {
            obs.increment_requests();
        });
    });

    group.finish();
}

fn bench_record_duration_single_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_duration_single_threaded");

    // Baseline: Prometheus RwLock<HashMap>
    group.bench_function("prometheus_baseline", |b| {
        let metrics = PrometheusMetrics::new();
        b.iter(|| {
            metrics.record_duration_us(black_box(2500));
        });
    });

    // Optimized: ObservabilityCapsule (T2 SIMD histogram)
    group.bench_function("observability_capsule", |b| {
        let obs = ObservabilityCapsule::new();
        b.iter(|| {
            obs.record_duration_us(black_box(2500));
        });
    });

    group.finish();
}

fn bench_append_trace_single_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("append_trace_single_threaded");

    // Baseline: Prometheus Mutex<Vec>
    group.bench_function("prometheus_baseline", |b| {
        let metrics = PrometheusMetrics::new();
        b.iter(|| {
            metrics.append_trace(black_box(0x1234), black_box(0x5678), black_box(0xABCD), black_box(1000), black_box(100), black_box(0));
        });
    });

    // Optimized: ObservabilityCapsule (T5 Streaming ring buffer)
    group.bench_function("observability_capsule", |b| {
        let obs = ObservabilityCapsule::new();
        let mut ring_buffer = TraceRingBuffer::default();
        b.iter(|| {
            let trace = TraceEvent::new(black_box(0x1234), black_box(0x5678), black_box(0xABCD), black_box(1000), black_box(100), black_box(0));
            obs.append_trace(trace, &mut ring_buffer);
        });
    });

    group.finish();
}

// ============================================================================
// § 3: SIMD Aggregation Benchmark
// ============================================================================

fn bench_batch_aggregate_durations(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_aggregate_durations");

    // Baseline: Prometheus scalar iteration
    group.bench_function("prometheus_baseline", |b| {
        let metrics = PrometheusMetrics::new();
        // Pre-populate histogram
        for i in 0..1000 {
            metrics.record_duration_us((i % 10_000) as u32);
        }
        b.iter(|| {
            black_box(metrics.aggregate_durations());
        });
    });

    // Optimized: ObservabilityCapsule (T2 SIMD u64x8 reduction)
    group.bench_function("observability_capsule", |b| {
        let obs = ObservabilityCapsule::new();
        // Pre-populate histogram
        for i in 0..1000 {
            obs.record_duration_us((i % 10_000) as u32);
        }
        b.iter(|| {
            black_box(obs.batch_aggregate_durations());
        });
    });

    group.finish();
}

// ============================================================================
// § 4: Multi-Threaded Contention Benchmarks
// ============================================================================

fn bench_increment_requests_multi_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("increment_requests_multi_threaded");

    for thread_count in [2, 4, 8] {
        group.throughput(Throughput::Elements(thread_count as u64 * 1000));

        // Baseline: Prometheus mutex-based
        group.bench_with_input(BenchmarkId::new("prometheus_baseline", thread_count), &thread_count, |b, &threads| {
            b.iter(|| {
                let metrics = PrometheusMetrics::new();
                let handles: Vec<_> = (0..threads)
                    .map(|_| {
                        let m = metrics.clone();
                        thread::spawn(move || {
                            for _ in 0..1000 {
                                m.increment_requests();
                            }
                        })
                    })
                    .collect();
                for h in handles {
                    h.join().unwrap();
                }
            });
        });

        // Optimized: ObservabilityCapsule (T1 Atomic)
        group.bench_with_input(BenchmarkId::new("observability_capsule", thread_count), &thread_count, |b, &threads| {
            b.iter(|| {
                let obs = Arc::new(ObservabilityCapsule::new());
                let handles: Vec<_> = (0..threads)
                    .map(|_| {
                        let o = Arc::clone(&obs);
                        thread::spawn(move || {
                            for _ in 0..1000 {
                                o.increment_requests();
                            }
                        })
                    })
                    .collect();
                for h in handles {
                    h.join().unwrap();
                }
            });
        });
    }

    group.finish();
}

fn bench_record_duration_multi_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_duration_multi_threaded");

    for thread_count in [2, 4, 8] {
        group.throughput(Throughput::Elements(thread_count as u64 * 1000));

        // Baseline: Prometheus RwLock<HashMap>
        group.bench_with_input(BenchmarkId::new("prometheus_baseline", thread_count), &thread_count, |b, &threads| {
            b.iter(|| {
                let metrics = PrometheusMetrics::new();
                let handles: Vec<_> = (0..threads)
                    .map(|thread_id| {
                        let m = metrics.clone();
                        thread::spawn(move || {
                            for i in 0..1000 {
                                m.record_duration_us(((thread_id * 1000 + i) % 10_000) as u32);
                            }
                        })
                    })
                    .collect();
                for h in handles {
                    h.join().unwrap();
                }
            });
        });

        // Optimized: ObservabilityCapsule (T2 SIMD histogram)
        group.bench_with_input(BenchmarkId::new("observability_capsule", thread_count), &thread_count, |b, &threads| {
            b.iter(|| {
                let obs = Arc::new(ObservabilityCapsule::new());
                let handles: Vec<_> = (0..threads)
                    .map(|thread_id| {
                        let o = Arc::clone(&obs);
                        thread::spawn(move || {
                            for i in 0..1000 {
                                o.record_duration_us(((thread_id * 1000 + i) % 10_000) as u32);
                            }
                        })
                    })
                    .collect();
                for h in handles {
                    h.join().unwrap();
                }
            });
        });
    }

    group.finish();
}

// ============================================================================
// § 5: End-to-End Benchmark (RED Metrics + Traces)
// ============================================================================

fn bench_end_to_end_observability(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end_observability");
    group.sample_size(100); // Reduce sample size for expensive benchmark
    group.measurement_time(Duration::from_secs(10));

    // Baseline: Prometheus-style
    group.bench_function("prometheus_baseline", |b| {
        b.iter(|| {
            let metrics = PrometheusMetrics::new();
            for i in 0..10_000 {
                metrics.increment_requests();

                if i % 50 == 0 {
                    metrics.increment_errors();
                }

                metrics.record_duration_us((i % 10_000) as u32);

                if i % 10 == 0 {
                    metrics.append_trace(0x1234, i as u64, i as u64, i as u32, 100, 0);
                }
            }

            // Read metrics
            black_box(metrics.load_request_count());
            black_box(metrics.load_error_count());
            black_box(metrics.aggregate_durations());
        });
    });

    // Optimized: ObservabilityCapsule
    group.bench_function("observability_capsule", |b| {
        b.iter(|| {
            let obs = ObservabilityCapsule::new();
            let mut ring_buffer = TraceRingBuffer::default();

            for i in 0..10_000 {
                obs.increment_requests();

                if i % 50 == 0 {
                    obs.increment_errors();
                }

                obs.record_duration_us((i % 10_000) as u32);

                if i % 10 == 0 {
                    let trace = TraceEvent::new(0x1234, i as u64, i as u64, i as u32, 100, 0);
                    obs.append_trace(trace, &mut ring_buffer);
                }
            }

            // Read metrics
            black_box(obs.load_request_count());
            black_box(obs.load_error_count());
            black_box(obs.batch_aggregate_durations());
        });
    });

    group.finish();
}

// ============================================================================
// § 6: Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_increment_requests_single_threaded,
    bench_record_duration_single_threaded,
    bench_append_trace_single_threaded,
    bench_batch_aggregate_durations,
    bench_increment_requests_multi_threaded,
    bench_record_duration_multi_threaded,
    bench_end_to_end_observability,
);

criterion_main!(benches);
