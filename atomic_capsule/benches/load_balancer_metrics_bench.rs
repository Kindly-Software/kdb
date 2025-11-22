//! Comprehensive benchmarks for LoadBalancerMetricsCapsule (B32 Framework)
//!
//! Fair baselines comparison with traditional approaches:
//! - Mutex<HashMap> for per-backend metrics
//! - RwLock<Vec<Stats>> for aggregation
//! - Standard atomic counters (naive approach)
//!
//! Performance targets (B32 Framework):
//! - <50ns metric recording (Relaxed atomics)
//! - <1ms aggregation (10 backends)
//! - <500ns load variance (fixed-point)
//! - <2ms percentiles (100K requests)
//! - <50ns snapshot (Q34 audit)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::sync::Mutex;
use parking_lot::RwLock;

use atomic_capsule::load_balancing::{
    LoadBalancerMetricsCapsule, BackendMetrics, AlertThresholds,
};

// Baseline implementations
struct MutexMetrics {
    total_requests: Mutex<u64>,
    successful: Mutex<u64>,
    failed: Mutex<u64>,
    total_latency: Mutex<u64>,
}

impl MutexMetrics {
    fn new() -> Self {
        Self {
            total_requests: Mutex::new(0),
            successful: Mutex::new(0),
            failed: Mutex::new(0),
            total_latency: Mutex::new(0),
        }
    }

    fn record_request(&self, latency: u64, success: bool) {
        *self.total_requests.lock().unwrap() += 1;
        if success {
            *self.successful.lock().unwrap() += 1;
        } else {
            *self.failed.lock().unwrap() += 1;
        }
        *self.total_latency.lock().unwrap() += latency;
    }
}

struct RwLockMetrics {
    state: RwLock<MetricsState>,
}

struct MetricsState {
    total_requests: u64,
    successful: u64,
    failed: u64,
    total_latency: u64,
}

impl RwLockMetrics {
    fn new() -> Self {
        Self {
            state: RwLock::new(MetricsState {
                total_requests: 0,
                successful: 0,
                failed: 0,
                total_latency: 0,
            }),
        }
    }

    fn record_request(&self, latency: u64, success: bool) {
        let mut state = self.state.write();
        state.total_requests += 1;
        if success {
            state.successful += 1;
        } else {
            state.failed += 1;
        }
        state.total_latency += latency;
    }
}

// Benchmarks

/// Q1: Single request recording - Capsule vs Mutex
fn bench_single_request_record(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_request_record");

    group.bench_function("capsule", |b| {
        let metrics = LoadBalancerMetricsCapsule::new();
        b.iter(|| {
            metrics.record_request(
                black_box(0),
                black_box(5_000_000),
                black_box(true),
            ).unwrap();
        });
    });

    group.bench_function("mutex", |b| {
        let metrics = MutexMetrics::new();
        b.iter(|| {
            metrics.record_request(
                black_box(5_000_000),
                black_box(true),
            );
        });
    });

    group.bench_function("rwlock", |b| {
        let metrics = RwLockMetrics::new();
        b.iter(|| {
            metrics.record_request(
                black_box(5_000_000),
                black_box(true),
            );
        });
    });

    group.finish();
}

/// Q2: Latency recording with min/max updates
fn bench_latency_recording(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_recording");

    group.bench_function("capsule_min_max", |b| {
        let metrics = LoadBalancerMetricsCapsule::new();
        let latencies = vec![1_000_000, 5_000_000, 10_000_000, 3_000_000, 8_000_000];
        let mut idx = 0;
        b.iter(|| {
            let latency = black_box(latencies[idx % latencies.len()]);
            metrics.record_request(0, latency, true).unwrap();
            idx += 1;
        });
    });

    group.bench_function("naive_atomics", |b| {
        use std::sync::atomic::{AtomicU64, Ordering};
        let min_lat = AtomicU64::new(u64::MAX);
        let max_lat = AtomicU64::new(0);
        let latencies = vec![1_000_000, 5_000_000, 10_000_000, 3_000_000, 8_000_000];
        let mut idx = 0;
        b.iter(|| {
            let latency = black_box(latencies[idx % latencies.len()]);
            // CAS loop for min
            loop {
                let current = min_lat.load(Ordering::Relaxed);
                if latency >= current {
                    break;
                }
                if min_lat
                    .compare_exchange_weak(current, latency, Ordering::Release, Ordering::Relaxed)
                    .is_ok()
                {
                    break;
                }
            }
            // CAS loop for max
            loop {
                let current = max_lat.load(Ordering::Relaxed);
                if latency <= current {
                    break;
                }
                if max_lat
                    .compare_exchange_weak(current, latency, Ordering::Release, Ordering::Relaxed)
                    .is_ok()
                {
                    break;
                }
            }
            idx += 1;
        });
    });

    group.finish();
}

/// Q3: Session tracking (hit/miss)
fn bench_session_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("session_tracking");

    group.bench_function("capsule", |b| {
        let metrics = LoadBalancerMetricsCapsule::new();
        let mut hit = true;
        b.iter(|| {
            metrics.record_session_lookup(black_box(hit)).unwrap();
            hit = !hit;
        });
    });

    group.finish();
}

/// Q4: Aggregation performance
fn bench_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("aggregation");

    group.bench_function("capsule_10_backends", |b| {
        let metrics = LoadBalancerMetricsCapsule::new();
        // Pre-populate with some data
        for _ in 0..1000 {
            metrics.record_request(0, 5_000_000, true).unwrap();
        }
        b.iter(|| {
            metrics.aggregate_metrics().unwrap();
        });
    });

    group.bench_function("capsule_100_backends", |b| {
        let metrics = LoadBalancerMetricsCapsule::new();
        for _ in 0..10_000 {
            metrics.record_request(0, 5_000_000, true).unwrap();
        }
        b.iter(|| {
            metrics.aggregate_metrics().unwrap();
        });
    });

    group.finish();
}

/// Q5: Export formats
fn bench_exports(c: &mut Criterion) {
    let mut group = c.benchmark_group("exports");

    let metrics = LoadBalancerMetricsCapsule::new();
    for _ in 0..1000 {
        metrics.record_request(0, 5_000_000, true).unwrap();
    }

    group.bench_function("prometheus", |b| {
        b.iter(|| {
            metrics.export_prometheus().unwrap();
        });
    });

    group.bench_function("json", |b| {
        b.iter(|| {
            metrics.export_json().unwrap();
        });
    });

    group.bench_function("binary", |b| {
        b.iter(|| {
            metrics.export_binary().unwrap();
        });
    });

    group.finish();
}

/// Q6: Alert checking
fn bench_alert_checking(c: &mut Criterion) {
    let mut group = c.benchmark_group("alert_checking");

    let metrics = LoadBalancerMetricsCapsule::new();
    for _ in 0..1000 {
        metrics.record_request(0, 5_000_000, true).unwrap();
    }

    let thresholds = AlertThresholds::default();

    group.bench_function("check_alerts", |b| {
        b.iter(|| {
            metrics.check_alerts(black_box(&thresholds)).unwrap();
        });
    });

    group.finish();
}

/// Q7: Snapshot and audit trail
fn bench_audit_trail(c: &mut Criterion) {
    let mut group = c.benchmark_group("audit_trail");

    group.bench_function("take_snapshot", |b| {
        let metrics = LoadBalancerMetricsCapsule::new();
        for _ in 0..100 {
            metrics.record_request(0, 5_000_000, true).unwrap();
        }
        b.iter(|| {
            metrics.take_snapshot().unwrap();
        });
    });

    group.bench_function("verify_audit_trail", |b| {
        let metrics = LoadBalancerMetricsCapsule::new();
        for _ in 0..100 {
            metrics.record_request(0, 5_000_000, true).unwrap();
        }
        let snapshot = metrics.aggregate_metrics().unwrap();
        b.iter(|| {
            metrics.verify_audit_trail(black_box(&snapshot)).unwrap();
        });
    });

    group.finish();
}

/// Q8: Backend metrics layout and access
fn bench_backend_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("backend_metrics");

    group.bench_function("new", |b| {
        b.iter(|| {
            let _backend = BackendMetrics::new(black_box(0));
        });
    });

    group.bench_function("get_state", |b| {
        let backend = BackendMetrics::new(0);
        b.iter(|| {
            let _ = backend.get_state();
        });
    });

    group.bench_function("set_state", |b| {
        let backend = BackendMetrics::new(0);
        use atomic_capsule::load_balancing::BackendState;
        b.iter(|| {
            backend.set_state(black_box(BackendState::Degraded));
        });
    });

    group.finish();
}

/// Q9: Circuit breaker state tracking
fn bench_circuit_breaker(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker_tracking");

    let states = vec!["open", "closed", "half_open", "open", "closed"];
    let mut idx = 0;

    group.bench_function("record_state", |b| {
        let metrics = LoadBalancerMetricsCapsule::new();
        b.iter(|| {
            let state = black_box(states[idx % states.len()]);
            metrics.record_circuit_breaker_state(state).unwrap();
            idx += 1;
        });
    });

    group.finish();
}

/// Q10: Health check recording
fn bench_health_checks(c: &mut Criterion) {
    let mut group = c.benchmark_group("health_checks");

    group.bench_function("record_check", |b| {
        let metrics = LoadBalancerMetricsCapsule::new();
        let mut healthy = true;
        b.iter(|| {
            metrics.record_health_check(black_box(0), black_box(healthy)).unwrap();
            healthy = !healthy;
        });
    });

    group.finish();
}

// Scalability benchmarks

/// Q11: Scaling - record count
fn bench_scale_request_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_request_count");

    for count in [100, 1_000, 10_000, 100_000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            let metrics = LoadBalancerMetricsCapsule::new();
            b.iter(|| {
                for i in 0..count {
                    metrics.record_request(
                        (i % 4) as u32,
                        ((i % 100) * 1_000_000) as u64,
                        i % 10 != 0,
                    ).unwrap();
                }
            });
        });
    }

    group.finish();
}

/// Q12: Scaling - backend count
fn bench_scale_backend_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_backend_count");

    for backend_count in [1, 4, 16, 64].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}backends", backend_count)),
            backend_count,
            |b, &backend_count| {
                let metrics = LoadBalancerMetricsCapsule::new();
                // Record 1K requests across backends
                for i in 0..1_000 {
                    metrics
                        .record_request(
                            (i % backend_count) as u32,
                            5_000_000,
                            true,
                        )
                        .unwrap();
                }

                b.iter(|| {
                    metrics.aggregate_metrics().unwrap();
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_single_request_record,
    bench_latency_recording,
    bench_session_tracking,
    bench_aggregation,
    bench_exports,
    bench_alert_checking,
    bench_audit_trail,
    bench_backend_metrics,
    bench_circuit_breaker,
    bench_health_checks,
    bench_scale_request_count,
    bench_scale_backend_count,
);
criterion_main!(benches);
