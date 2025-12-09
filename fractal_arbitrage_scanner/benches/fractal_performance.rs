//! B32 Framework: Comprehensive Fractal Performance Benchmarks
//! Following UCE32 Q30 (Empirical Validation) + Q31 (Rust Transform) + Kontext27 Hardware Reality
//!
//! Benchmark Categories:
//! 1. MF-DFA calculation speed - O(N log N) complexity
//! 2. Williams Fractal detection - O(N) sliding window
//! 3. Wavelet Leaders computation - O(N log N)
//! 4. Memory allocation patterns - √N complexity validation
//! 5. Fair baseline comparisons - not strawmen

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use fractal_arbitrage_scanner::fractal_mathematics::{
    MultifractalDFA, WaveletLeaders, WilliamsFractal, efficiency_ratio, fibonacci_levels
};
use std::time::Duration;

/// Kontext27 Reality Check: Expected performance baselines
const TYPICAL_SPEEDUP: f64 = 1.5;        // 50% improvement typical
const EXCEPTIONAL_SPEEDUP: f64 = 5.0;     // 5x requires validation
const REVOLUTIONARY_THRESHOLD: f64 = 100.0; // 100x needs extensive proof

/// B32 Framework: Statistical validation requirements
const MIN_ITERATIONS: u64 = 1000;
const CONFIDENCE_INTERVAL: f64 = 0.95;

/// Generate synthetic market data for benchmarking
fn generate_market_data(size: usize, volatility: f64) -> Vec<f64> {
    let mut data = Vec::with_capacity(size);
    let mut price = 100.0;

    for i in 0..size {
        // Combine trend, volatility, and fractal behavior
        let trend = 0.0001 * (i as f64);
        let noise = volatility * ((i as f64 * 0.1).sin() + 0.5 * (i as f64 * 0.05).cos());
        let fractal = 0.001 * (i as f64 / 10.0).sin();

        price += trend + noise + fractal;
        data.push(price);
    }

    data
}

/// Fair baseline: Naive MF-DFA implementation for comparison
struct NaiveMfDfa {
    scales: Vec<f64>,
}

impl NaiveMfDfa {
    fn new() -> Self {
        Self {
            scales: vec![5.0, 15.0, 30.0, 90.0, 120.0, 240.0, 720.0],
        }
    }

    /// Unoptimized DFA calculation - baseline for comparison
    fn calculate_hurst_naive(&self, data: &[f64]) -> f64 {
        let mut log_scales = Vec::new();
        let mut log_flucts = Vec::new();

        for &scale in &self.scales {
            let scale_int = scale as usize;
            if scale_int < data.len() / 4 {
                let fluct = self.naive_fluctuation(data, scale_int);
                if fluct > 0.0 {
                    log_scales.push(scale.ln());
                    log_flucts.push(fluct.ln());
                }
            }
        }

        if log_scales.len() < 2 {
            return 0.5;
        }

        // Simple linear regression (unoptimized)
        let n = log_scales.len() as f64;
        let sum_x: f64 = log_scales.iter().sum();
        let sum_y: f64 = log_flucts.iter().sum();
        let sum_xy: f64 = log_scales.iter().zip(&log_flucts).map(|(x, y)| x * y).sum();
        let sum_xx: f64 = log_scales.iter().map(|x| x * x).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);
        slope.max(0.0).min(1.0)
    }

    fn naive_fluctuation(&self, data: &[f64], scale: usize) -> f64 {
        if data.len() < scale * 2 {
            return 0.0;
        }

        let mut sum_sq = 0.0;
        let segments = data.len() / scale;

        for seg in 0..segments {
            let start = seg * scale;
            let end = start + scale;
            let segment = &data[start..end];

            // Naive linear detrending (no optimization)
            let mut sum_x = 0.0;
            let mut sum_y = 0.0;
            let mut sum_xy = 0.0;
            let mut sum_xx = 0.0;
            let n = segment.len() as f64;

            for (i, &y) in segment.iter().enumerate() {
                let x = i as f64;
                sum_x += x;
                sum_y += y;
                sum_xy += x * y;
                sum_xx += x * x;
            }

            let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);
            let intercept = (sum_y - slope * sum_x) / n;

            for (i, &val) in segment.iter().enumerate() {
                let trend = slope * i as f64 + intercept;
                let residual = val - trend;
                sum_sq += residual * residual;
            }
        }

        (sum_sq / (segments * scale) as f64).sqrt()
    }
}

/// Benchmark MF-DFA calculation speed across different data sizes
fn bench_mf_dfa_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("mf_dfa_scaling");

    // Test data sizes from small to large
    let sizes = [100, 500, 1000, 2000, 5000, 10000];

    for size in sizes {
        let data = generate_market_data(size, 0.02);

        group.throughput(Throughput::Elements(size as u64));

        // Optimized implementation
        group.bench_with_input(
            BenchmarkId::new("optimized", size),
            &data,
            |b, data| {
                let mut mfdfa = MultifractalDFA::new();
                b.iter(|| {
                    black_box(mfdfa.calculate_hurst(black_box(data)))
                });
            },
        );

        // Fair baseline comparison
        group.bench_with_input(
            BenchmarkId::new("baseline", size),
            &data,
            |b, data| {
                let naive = NaiveMfDfa::new();
                b.iter(|| {
                    black_box(naive.calculate_hurst_naive(black_box(data)))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark Williams Fractal detection patterns
fn bench_williams_fractals(c: &mut Criterion) {
    let mut group = c.benchmark_group("williams_fractals");

    let sizes = [1000, 5000, 10000, 20000];

    for size in sizes {
        let data = generate_market_data(size, 0.01);

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(
            BenchmarkId::new("detect_high", size),
            &data,
            |b, data| {
                let fractal = WilliamsFractal::new();
                b.iter(|| {
                    black_box(fractal.detect_high(black_box(data)))
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("detect_low", size),
            &data,
            |b, data| {
                let fractal = WilliamsFractal::new();
                b.iter(|| {
                    black_box(fractal.detect_low(black_box(data)))
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("calculate_dimension", size),
            &data,
            |b, data| {
                let mut fractal = WilliamsFractal::new();
                b.iter(|| {
                    black_box(fractal.calculate_dimension(black_box(data)))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark Wavelet Leaders computation
fn bench_wavelet_leaders(c: &mut Criterion) {
    let mut group = c.benchmark_group("wavelet_leaders");

    // Power-of-2 sizes work best for wavelets
    let sizes = [128, 256, 512, 1024, 2048, 4096];

    for size in sizes {
        let data = generate_market_data(size, 0.015);

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(
            BenchmarkId::new("haar_transform", size),
            &data,
            |b, data| {
                let wl = WaveletLeaders::new();
                b.iter(|| {
                    black_box(wl.haar_transform(black_box(data)))
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("calculate_spectrum", size),
            &data,
            |b, data| {
                let wl = WaveletLeaders::new();
                b.iter(|| {
                    black_box(wl.calculate_spectrum(black_box(data)))
                });
            },
        );
    }

    group.finish();
}

/// Memory allocation pattern validation - √N complexity check
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_patterns");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(100); // Fewer samples for memory tests

    let sizes = [1000, 4000, 9000, 16000, 25000]; // √N series

    for size in sizes {
        let sqrt_size = (size as f64).sqrt() as usize;

        group.bench_with_input(
            BenchmarkId::new("sqrt_allocation", size),
            &size,
            |b, &_size| {
                b.iter(|| {
                    // Simulate √N memory allocation pattern
                    let mut buffers: Vec<Vec<f64>> = Vec::new();
                    for _i in 0..sqrt_size {
                        let buffer = vec![0.0; sqrt_size];
                        buffers.push(black_box(buffer));
                    }
                    black_box(buffers)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("linear_allocation", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    // Linear allocation for comparison
                    let buffer = vec![0.0; size];
                    black_box(buffer)
                });
            },
        );
    }

    group.finish();
}

/// Efficiency ratio calculations - simple but critical metric
fn bench_efficiency_ratio(c: &mut Criterion) {
    let mut group = c.benchmark_group("efficiency_ratio");

    let sizes = [100, 500, 1000, 2000, 5000];
    let periods = [14, 20, 50];

    for size in sizes {
        let data = generate_market_data(size, 0.02);

        for period in periods {
            if period < size {
                group.bench_with_input(
                    BenchmarkId::new(format!("period_{}", period), size),
                    &(&data, period),
                    |b, (data, period)| {
                        b.iter(|| {
                            black_box(efficiency_ratio(black_box(data), black_box(*period)))
                        });
                    },
                );
            }
        }
    }

    group.finish();
}

/// Fibonacci level calculations - constant time operations
fn bench_fibonacci_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("fibonacci_levels");

    // Different price ranges to test numerical stability
    let test_cases = [
        (100.0, 50.0),     // Normal range
        (10000.0, 5000.0), // Large numbers
        (1.5, 0.5),        // Small numbers
        (1000000.0, 999000.0), // Small relative difference
    ];

    for (i, (high, low)) in test_cases.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("calculate", i),
            &(high, low),
            |b, (high, low)| {
                b.iter(|| {
                    black_box(fibonacci_levels(black_box(**high), black_box(**low)))
                });
            },
        );
    }

    group.finish();
}

/// Comprehensive end-to-end fractal analysis pipeline
fn bench_fractal_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("fractal_pipeline");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(50);

    let data = generate_market_data(2000, 0.02);

    group.bench_function("complete_analysis", |b| {
        b.iter(|| {
            // Complete fractal analysis pipeline
            let mut mfdfa = MultifractalDFA::new();
            let mut williams = WilliamsFractal::new();
            let wl = WaveletLeaders::new();

            // MF-DFA analysis
            let hurst = mfdfa.calculate_hurst(black_box(&data));
            let spectrum = mfdfa.calculate_spectrum(black_box(&data));

            // Williams fractal detection
            let highs = williams.detect_high(black_box(&data));
            let lows = williams.detect_low(black_box(&data));
            let dimension = williams.calculate_dimension(black_box(&data));

            // Wavelet analysis
            let wl_spectrum = wl.calculate_spectrum(black_box(&data));

            // Efficiency calculations
            let eff_14 = efficiency_ratio(black_box(&data), 14);
            let eff_20 = efficiency_ratio(black_box(&data), 20);

            // Fibonacci levels for last 100 points
            let recent = &data[data.len().saturating_sub(100)..];
            let max_price = recent.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            let min_price = recent.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let fib_levels = fibonacci_levels(max_price, min_price);

            black_box((hurst, spectrum, highs, lows, dimension, wl_spectrum, eff_14, eff_20, fib_levels))
        });
    });

    group.finish();
}

/// Real-time fractal update simulation
fn bench_realtime_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("realtime_updates");

    // Simulate adding new data points to existing analysis
    let base_data = generate_market_data(1000, 0.02);
    let new_points = generate_market_data(10, 0.02);

    group.bench_function("incremental_update", |b| {
        b.iter(|| {
            let mut mfdfa = MultifractalDFA::new();

            // Initial calculation
            let _ = mfdfa.calculate_hurst(black_box(&base_data));

            // Simulate adding new points
            for &new_point in &new_points {
                let mut updated_data = base_data.clone();
                updated_data.push(new_point);

                // Recalculate with new data
                let _ = mfdfa.calculate_hurst(black_box(&updated_data));
            }

            black_box(())
        });
    });

    group.finish();
}

criterion_group!(
    fractal_benches,
    bench_mf_dfa_scaling,
    bench_williams_fractals,
    bench_wavelet_leaders,
    bench_memory_patterns,
    bench_efficiency_ratio,
    bench_fibonacci_levels,
    bench_fractal_pipeline,
    bench_realtime_updates
);

criterion_main!(fractal_benches);