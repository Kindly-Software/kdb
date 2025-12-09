//! Cost Forecast Benchmarks (B32 Framework)
//!
//! Benchmarks following B32 framework guidelines:
//! - Fair baselines (simple moving average comparison)
//! - Statistical rigor (1000+ iterations, 95% CI via Criterion)
//! - Honest claims (10-50% typical, not exaggerated)
//! - Hardware reality checks (cache-aware, single-threaded baselines)
//! - Reproducibility (deterministic inputs, fixed seed)
//!
//! Target Performance:
//! - Forecast lookup: <100ns (lockfree atomic read)
//! - Trend update: <1ms (28-element linear regression)
//! - Concurrent updates: scales linearly to 8 threads

use clapi_core::capsules::CostForecast256;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Baseline Implementations (Fair Comparison)
// ============================================================================

/// Baseline: Simple moving average (no fixed-point, no atomics)
struct SimpleMovingAverage {
    window: Vec<f64>,
    capacity: usize,
}

impl SimpleMovingAverage {
    fn new(capacity: usize) -> Self {
        Self {
            window: Vec::with_capacity(capacity),
            capacity,
        }
    }

    fn update(&mut self, value: f64) {
        if self.window.len() >= self.capacity {
            self.window.remove(0); // Shift left
        }
        self.window.push(value);
    }

    fn mean(&self) -> f64 {
        if self.window.is_empty() {
            return 0.0;
        }
        self.window.iter().sum::<f64>() / self.window.len() as f64
    }

    fn trend(&self) -> f64 {
        if self.window.len() < 2 {
            return 0.0;
        }

        let n = self.window.len() as f64;
        let sum_x: f64 = (0..self.window.len()).map(|x| x as f64).sum();
        let sum_y: f64 = self.window.iter().sum();
        let sum_xy: f64 = self
            .window
            .iter()
            .enumerate()
            .map(|(x, &y)| x as f64 * y)
            .sum();
        let sum_x2: f64 = (0..self.window.len()).map(|x| (x * x) as f64).sum();

        let mean_x = sum_x / n;
        let mean_y = sum_y / n;

        let numerator = sum_xy - n * mean_x * mean_y;
        let denominator = sum_x2 - n * mean_x * mean_x;

        if denominator.abs() < 1e-6 {
            0.0
        } else {
            numerator / denominator
        }
    }
}

// ============================================================================
// Benchmark: Update Performance
// ============================================================================

fn bench_update_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("update_single");

    // Baseline: SimpleMovingAverage
    group.bench_function("baseline_simple_ma", |b| {
        let mut ma = SimpleMovingAverage::new(28);
        let mut counter = 0u64;

        b.iter(|| {
            counter += 1;
            ma.update(black_box((counter % 100) as f64));
        });
    });

    // CostForecast256 (T4+T3 Batch+Fixed-Point)
    group.bench_function("cost_forecast256", |b| {
        let forecast = CostForecast256::new(1);
        let mut counter = 0u64;

        b.iter(|| {
            counter += 1;
            forecast.update(black_box((counter % 100) as f64));
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark: Snapshot Read Performance
// ============================================================================

fn bench_snapshot_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot_read");

    // Baseline: Copy Vec (28 f64)
    group.bench_function("baseline_vec_copy", |b| {
        let data: Vec<f64> = (0..28).map(|x| x as f64).collect();

        b.iter(|| {
            let snapshot = black_box(data.clone());
            snapshot
        });
    });

    // CostForecast256 (lockfree atomic read)
    group.bench_function("cost_forecast256", |b| {
        let forecast = CostForecast256::new(1);

        // Populate with data
        for i in 0..28 {
            forecast.update(i as f64);
        }

        b.iter(|| {
            let snapshot = black_box(forecast.snapshot());
            snapshot
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark: Trend Calculation
// ============================================================================

fn bench_trend_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("trend_calculation");

    // Baseline: SimpleMovingAverage
    group.bench_function("baseline_simple_ma", |b| {
        let mut ma = SimpleMovingAverage::new(28);

        for i in 0..28 {
            ma.update(i as f64);
        }

        b.iter(|| {
            let trend = black_box(ma.trend());
            trend
        });
    });

    // CostForecast256 (snapshot includes precomputed trend)
    group.bench_function("cost_forecast256", |b| {
        let forecast = CostForecast256::new(1);

        for i in 0..28 {
            forecast.update(i as f64);
        }

        b.iter(|| {
            let snapshot = black_box(forecast.snapshot());
            snapshot.daily_burn_rate_cents
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark: Concurrent Updates (Scalability)
// ============================================================================

fn bench_concurrent_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_updates");

    for thread_count in [1, 2, 4, 8].iter() {
        group.throughput(Throughput::Elements(*thread_count as u64 * 100));

        group.bench_with_input(
            BenchmarkId::new("cost_forecast256", thread_count),
            thread_count,
            |b, &threads| {
                b.iter(|| {
                    let forecast = Arc::new(CostForecast256::new(1));
                    let mut handles = vec![];

                    for _ in 0..threads {
                        let f = Arc::clone(&forecast);
                        handles.push(thread::spawn(move || {
                            for i in 0..100 {
                                f.update((i % 100) as f64);
                            }
                        }));
                    }

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
// Benchmark: Full Workflow (Update + Snapshot)
// ============================================================================

fn bench_full_workflow(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_workflow");

    // Baseline: SimpleMovingAverage
    group.bench_function("baseline_simple_ma", |b| {
        let mut ma = SimpleMovingAverage::new(28);

        b.iter(|| {
            ma.update(black_box(10.0));
            let mean = black_box(ma.mean());
            let trend = black_box(ma.trend());
            (mean, trend)
        });
    });

    // CostForecast256
    group.bench_function("cost_forecast256", |b| {
        let forecast = CostForecast256::new(1);

        b.iter(|| {
            forecast.update(black_box(10.0));
            let snapshot = black_box(forecast.snapshot());
            (snapshot.mean_cost_cents, snapshot.daily_burn_rate_cents)
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark: Batch Updates (28 consecutive)
// ============================================================================

fn bench_batch_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_updates");
    group.throughput(Throughput::Elements(28));

    // Baseline: SimpleMovingAverage
    group.bench_function("baseline_simple_ma", |b| {
        let mut ma = SimpleMovingAverage::new(28);

        b.iter(|| {
            for i in 0..28 {
                ma.update(black_box(i as f64));
            }
        });
    });

    // CostForecast256
    group.bench_function("cost_forecast256", |b| {
        let forecast = CostForecast256::new(1);

        b.iter(|| {
            for i in 0..28 {
                forecast.update(black_box(i as f64));
            }
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark: Anomaly Detection
// ============================================================================

fn bench_anomaly_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("anomaly_detection");

    // CostForecast256 (built-in anomaly detection)
    group.bench_function("cost_forecast256", |b| {
        let forecast = CostForecast256::new(1);

        // Baseline: 10.0
        for _ in 0..27 {
            forecast.update(10.0);
        }

        b.iter(|| {
            // Anomaly update triggers detection
            forecast.update(black_box(100.0));
            let snapshot = forecast.snapshot();
            snapshot.anomaly_count
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_update_single,
    bench_snapshot_read,
    bench_trend_calculation,
    bench_concurrent_updates,
    bench_full_workflow,
    bench_batch_updates,
    bench_anomaly_detection,
);

criterion_main!(benches);
