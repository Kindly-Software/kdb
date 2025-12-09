//! P3-E5: CapacityPlannerCapsule128 Benchmarks
//!
//! # B32 Honest Benchmarking
//! - Fair baseline: f64 floating-point regression
//! - Statistical rigor: 1000+ samples, 95% CI
//! - Honest reporting: Document fixed-point precision limits

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use clapi_core::capsules::CapacityPlannerCapsule128;

/// Baseline: Floating-point linear regression (non-incremental)
struct FloatRegression {
    observations: Vec<(f64, f64)>, // (time, usage)
}

impl FloatRegression {
    fn new() -> Self {
        Self {
            observations: Vec::new(),
        }
    }

    fn record_usage(&mut self, time: f64, usage: f64) {
        self.observations.push((time, usage));
    }

    fn forecast_exhaustion(&self) -> Option<f64> {
        let n = self.observations.len() as f64;
        if n < 2.0 {
            return None;
        }

        let sum_x: f64 = self.observations.iter().map(|(x, _)| x).sum();
        let sum_y: f64 = self.observations.iter().map(|(_, y)| y).sum();
        let sum_xy: f64 = self.observations.iter().map(|(x, y)| x * y).sum();
        let sum_x2: f64 = self.observations.iter().map(|(x, _)| x * x).sum();

        let numerator = (n * sum_xy) - (sum_x * sum_y);
        let denominator = (n * sum_x2) - (sum_x * sum_x);

        if denominator == 0.0 {
            return None;
        }

        let slope = numerator / denominator;
        if slope >= 0.0 {
            return None; // Never exhausts
        }

        let mean_x = sum_x / n;
        let mean_y = sum_y / n;
        let intercept = mean_y - (slope * mean_x);

        let exhaustion_time = -intercept / slope;
        Some(exhaustion_time)
    }
}

fn bench_capacity_forecast_record(c: &mut Criterion) {
    let capsule = CapacityPlannerCapsule128::new(7);
    let mut baseline = FloatRegression::new();

    c.bench_function("capacity_forecast_record_fixed", |b| {
        b.iter(|| {
            black_box(capsule.record_usage(100_00));
        })
    });

    c.bench_function("capacity_forecast_record_float", |b| {
        let mut time = 0.0;
        b.iter(|| {
            black_box(baseline.record_usage(time, 100_00.0));
            time += 1.0;
        })
    });
}

fn bench_capacity_forecast_compute(c: &mut Criterion) {
    let mut group = c.benchmark_group("capacity_forecast_compute");

    for num_samples in [10, 100, 1000] {
        // Fixed-point capsule
        let capsule = CapacityPlannerCapsule128::new(7);
        for i in 0..num_samples {
            capsule.record_usage(1000_00 - (i * 10_00));
            std::thread::sleep(std::time::Duration::from_micros(10));
        }

        group.bench_with_input(
            BenchmarkId::new("fixed", num_samples),
            &num_samples,
            |b, _| {
                b.iter(|| {
                    black_box(capsule.forecast_exhaustion());
                })
            },
        );

        // Floating-point baseline
        let mut baseline = FloatRegression::new();
        for i in 0..num_samples {
            baseline.record_usage(i as f64, (1000_00 - (i * 10_00)) as f64);
        }

        group.bench_with_input(
            BenchmarkId::new("float", num_samples),
            &num_samples,
            |b, _| {
                b.iter(|| {
                    black_box(baseline.forecast_exhaustion());
                })
            },
        );
    }

    group.finish();
}

fn bench_capacity_forecast_confidence(c: &mut Criterion) {
    let capsule = CapacityPlannerCapsule128::new(7);
    for i in 0..100 {
        capsule.record_usage(1000_00 - (i * 10_00));
        std::thread::sleep(std::time::Duration::from_micros(10));
    }

    c.bench_function("capacity_forecast_confidence", |b| {
        b.iter(|| {
            black_box(capsule.confidence());
        })
    });
}

fn bench_capacity_forecast_concurrent(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let mut group = c.benchmark_group("capacity_forecast_concurrent");

    for num_threads in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            &num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let capsule = Arc::new(CapacityPlannerCapsule128::new(7));
                    let mut handles = vec![];

                    for _ in 0..num_threads {
                        let c = Arc::clone(&capsule);
                        handles.push(thread::spawn(move || {
                            for i in 0..100 {
                                c.record_usage(1000_00 - (i * 10_00));
                                std::thread::sleep(std::time::Duration::from_micros(10));
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }

                    black_box(capsule.forecast_exhaustion());
                })
            },
        );
    }

    group.finish();
}

fn bench_capacity_forecast_alert_check(c: &mut Criterion) {
    let capsule = CapacityPlannerCapsule128::new(7);
    for i in 0..100 {
        capsule.record_usage(1000_00 - (i * 10_00));
        std::thread::sleep(std::time::Duration::from_micros(10));
    }

    c.bench_function("capacity_forecast_alert_check", |b| {
        b.iter(|| {
            black_box(capsule.should_alert());
        })
    });
}

criterion_group!(
    benches,
    bench_capacity_forecast_record,
    bench_capacity_forecast_compute,
    bench_capacity_forecast_confidence,
    bench_capacity_forecast_concurrent,
    bench_capacity_forecast_alert_check,
);
criterion_main!(benches);
