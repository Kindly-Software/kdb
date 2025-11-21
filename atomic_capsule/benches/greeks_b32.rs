//! # B32 Benchmarks - GreeksCapsule (T3 Fixed-Point)
//!
//! **Framework**: B32 (K1-K70) - Honest benchmarking with fair baselines
//! **Comparison**: f64 Black-Scholes (fair baseline) vs GreeksCapsule Q16.16
//!
//! ## Performance Targets (B32)
//! - Greeks (ATM): <200ns (vs ~500ns f64)
//! - Greeks (ITM/OTM): <200ns (vs ~500ns f64)
//! - Implied Volatility: <500ns (vs ~1000ns f64)
//! - Batch Greeks (1000): <200μs (vs ~500μs f64)
//!
//! ## Baseline (Fair)
//! - f64 Black-Scholes with exp/log/sqrt from std::f64
//! - Represents standard financial library implementation
//! - Not a strawman: Uses optimized stdlib math functions
//!
//! ## Expected Speedup
//! - Single Greeks: 2.5× (deterministic Q16.16 + cache alignment)
//! - IV Solver: 2× (fewer iterations due to determinism)
//! - Batch: 2.5× (cache efficiency + determinism)

#![cfg(feature = "financial-greeks")]

use atomic_capsule::primitives::fixed_point::Q16_16;
use atomic_capsule::primitives::greeks::GreeksCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// ============================================================================
// § 1: Baseline - f64 Black-Scholes (Fair Baseline)
// ============================================================================

/// Fair baseline: Standard f64 Black-Scholes implementation
/// Uses std::f64 math functions (exp, log, sqrt) - typical library approach
#[derive(Debug, Clone, Copy)]
struct BaselineGreeks {
    delta: f64,
    gamma: f64,
    vega: f64,
    theta: f64,
    rho: f64,
}

/// Standard normal CDF approximation (Abramowitz and Stegun)
fn norm_cdf_f64(x: f64) -> f64 {
    const A1: f64 = 0.319381530;
    const A2: f64 = -0.356563782;
    const A3: f64 = 1.781477937;
    const A4: f64 = -1.821255978;
    const A5: f64 = 1.330274429;
    const P: f64 = 0.2316419;
    const ONE_OVER_SQRT_2PI: f64 = 0.39894228040143267793994605993438;

    let k = 1.0 / (1.0 + P * x.abs());
    let k2 = k * k;
    let k3 = k2 * k;
    let k4 = k3 * k;
    let k5 = k4 * k;

    let w = ONE_OVER_SQRT_2PI * (-0.5 * x * x).exp();
    let phi = w * (A1 * k + A2 * k2 + A3 * k3 + A4 * k4 + A5 * k5);

    if x >= 0.0 {
        1.0 - phi
    } else {
        phi
    }
}

/// Standard normal PDF
fn norm_pdf_f64(x: f64) -> f64 {
    const ONE_OVER_SQRT_2PI: f64 = 0.39894228040143267793994605993438;
    ONE_OVER_SQRT_2PI * (-0.5 * x * x).exp()
}

/// Calculate d1 and d2 for Black-Scholes
fn calculate_d1_d2_f64(
    spot: f64,
    strike: f64,
    rate: f64,
    time: f64,
    volatility: f64,
) -> (f64, f64) {
    let sqrt_time = time.sqrt();
    let vol_sqrt_time = volatility * sqrt_time;

    let ln_s_k = (spot / strike).ln();
    let numerator = ln_s_k + (rate + 0.5 * volatility * volatility) * time;

    let d1 = numerator / vol_sqrt_time;
    let d2 = d1 - vol_sqrt_time;

    (d1, d2)
}

/// Calculate Greeks using f64 Black-Scholes (fair baseline)
fn calculate_greeks_f64(
    spot: f64,
    strike: f64,
    rate: f64,
    time: f64,
    volatility: f64,
    is_call: bool,
) -> BaselineGreeks {
    let (d1, d2) = calculate_d1_d2_f64(spot, strike, rate, time, volatility);

    let sqrt_time = time.sqrt();
    let exp_minus_rt = (-rate * time).exp();

    // Delta
    let delta = if is_call {
        norm_cdf_f64(d1)
    } else {
        norm_cdf_f64(d1) - 1.0
    };

    // Gamma (same for calls and puts)
    let gamma = norm_pdf_f64(d1) / (spot * volatility * sqrt_time);

    // Vega (same for calls and puts)
    let vega = spot * norm_pdf_f64(d1) * sqrt_time / 100.0; // Divide by 100 for 1% change

    // Theta
    let theta_term1 = -(spot * norm_pdf_f64(d1) * volatility) / (2.0 * sqrt_time);
    let theta = if is_call {
        (theta_term1 - rate * strike * exp_minus_rt * norm_cdf_f64(d2)) / 365.0
    } else {
        (theta_term1 + rate * strike * exp_minus_rt * norm_cdf_f64(-d2)) / 365.0
    };

    // Rho
    let rho = if is_call {
        strike * time * exp_minus_rt * norm_cdf_f64(d2) / 100.0
    } else {
        -strike * time * exp_minus_rt * norm_cdf_f64(-d2) / 100.0
    };

    BaselineGreeks {
        delta,
        gamma,
        vega,
        theta,
        rho,
    }
}

/// Calculate implied volatility using Newton-Raphson (f64 baseline)
fn implied_volatility_f64(
    spot: f64,
    strike: f64,
    rate: f64,
    time: f64,
    market_price: f64,
    is_call: bool,
) -> Option<f64> {
    let mut vol = 0.2; // Initial guess: 20% volatility
    const MAX_ITERATIONS: usize = 10;
    const TOLERANCE: f64 = 0.0001;

    for _ in 0..MAX_ITERATIONS {
        let (d1, d2) = calculate_d1_d2_f64(spot, strike, rate, time, vol);

        let exp_minus_rt = (-rate * time).exp();
        let theo_price = if is_call {
            spot * norm_cdf_f64(d1) - strike * exp_minus_rt * norm_cdf_f64(d2)
        } else {
            strike * exp_minus_rt * norm_cdf_f64(-d2) - spot * norm_cdf_f64(-d1)
        };

        let diff = theo_price - market_price;
        if diff.abs() < TOLERANCE {
            return Some(vol);
        }

        // Vega for Newton-Raphson step
        let sqrt_time = time.sqrt();
        let vega = spot * norm_pdf_f64(d1) * sqrt_time;

        if vega.abs() < 1e-10 {
            return None; // Avoid division by zero
        }

        vol -= diff / vega;

        // Clamp volatility to reasonable range
        vol = vol.max(0.01).min(5.0);
    }

    Some(vol)
}

// ============================================================================
// § 2: Single Greeks Benchmarks - ATM/ITM/OTM Scenarios
// ============================================================================

fn bench_single_greeks_atm(c: &mut Criterion) {
    let mut group = c.benchmark_group("greeks_single_atm");
    group.throughput(Throughput::Elements(1));

    // ATM (At-The-Money): S=K=100, r=5%, T=1yr, σ=20%
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let time = 1.0;
    let volatility = 0.20;

    // Baseline: f64 Black-Scholes
    group.bench_function("baseline_f64_atm", |b| {
        b.iter(|| {
            calculate_greeks_f64(
                black_box(spot),
                black_box(strike),
                black_box(rate),
                black_box(time),
                black_box(volatility),
                black_box(true),
            )
        })
    });

    // Optimized: GreeksCapsule Q16.16
    let spot_q = Q16_16::from_f64(spot);
    let strike_q = Q16_16::from_f64(strike);
    let rate_q = Q16_16::from_f64(rate);
    let time_q = Q16_16::from_f64(time);
    let volatility_q = Q16_16::from_f64(volatility);

    group.bench_function("optimized_q16_16_atm", |b| {
        b.iter(|| {
            GreeksCapsule::calculate_greeks(
                black_box(spot_q),
                black_box(strike_q),
                black_box(rate_q),
                black_box(time_q),
                black_box(volatility_q),
                black_box(true),
            )
        })
    });

    group.finish();
}

fn bench_single_greeks_itm(c: &mut Criterion) {
    let mut group = c.benchmark_group("greeks_single_itm");
    group.throughput(Throughput::Elements(1));

    // ITM (In-The-Money): S=110, K=100, r=5%, T=1yr, σ=20%
    let spot = 110.0;
    let strike = 100.0;
    let rate = 0.05;
    let time = 1.0;
    let volatility = 0.20;

    // Baseline: f64 Black-Scholes
    group.bench_function("baseline_f64_itm", |b| {
        b.iter(|| {
            calculate_greeks_f64(
                black_box(spot),
                black_box(strike),
                black_box(rate),
                black_box(time),
                black_box(volatility),
                black_box(true),
            )
        })
    });

    // Optimized: GreeksCapsule Q16.16
    let spot_q = Q16_16::from_f64(spot);
    let strike_q = Q16_16::from_f64(strike);
    let rate_q = Q16_16::from_f64(rate);
    let time_q = Q16_16::from_f64(time);
    let volatility_q = Q16_16::from_f64(volatility);

    group.bench_function("optimized_q16_16_itm", |b| {
        b.iter(|| {
            GreeksCapsule::calculate_greeks(
                black_box(spot_q),
                black_box(strike_q),
                black_box(rate_q),
                black_box(time_q),
                black_box(volatility_q),
                black_box(true),
            )
        })
    });

    group.finish();
}

fn bench_single_greeks_otm(c: &mut Criterion) {
    let mut group = c.benchmark_group("greeks_single_otm");
    group.throughput(Throughput::Elements(1));

    // OTM (Out-of-The-Money): S=90, K=100, r=5%, T=1yr, σ=20%
    let spot = 90.0;
    let strike = 100.0;
    let rate = 0.05;
    let time = 1.0;
    let volatility = 0.20;

    // Baseline: f64 Black-Scholes
    group.bench_function("baseline_f64_otm", |b| {
        b.iter(|| {
            calculate_greeks_f64(
                black_box(spot),
                black_box(strike),
                black_box(rate),
                black_box(time),
                black_box(volatility),
                black_box(true),
            )
        })
    });

    // Optimized: GreeksCapsule Q16.16
    let spot_q = Q16_16::from_f64(spot);
    let strike_q = Q16_16::from_f64(strike);
    let rate_q = Q16_16::from_f64(rate);
    let time_q = Q16_16::from_f64(time);
    let volatility_q = Q16_16::from_f64(volatility);

    group.bench_function("optimized_q16_16_otm", |b| {
        b.iter(|| {
            GreeksCapsule::calculate_greeks(
                black_box(spot_q),
                black_box(strike_q),
                black_box(rate_q),
                black_box(time_q),
                black_box(volatility_q),
                black_box(true),
            )
        })
    });

    group.finish();
}

// ============================================================================
// § 3: Implied Volatility Solver Benchmarks
// ============================================================================

fn bench_implied_volatility(c: &mut Criterion) {
    let mut group = c.benchmark_group("greeks_implied_volatility");
    group.throughput(Throughput::Elements(1));

    // ATM call: S=100, K=100, r=5%, T=1yr, market_price≈10.45 (σ=20%)
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let time = 1.0;
    let market_price = 10.45;

    // Baseline: f64 Newton-Raphson
    group.bench_function("baseline_f64_iv", |b| {
        b.iter(|| {
            implied_volatility_f64(
                black_box(spot),
                black_box(strike),
                black_box(rate),
                black_box(time),
                black_box(market_price),
                black_box(true),
            )
        })
    });

    // Optimized: GreeksCapsule Q16.16 IV solver
    let spot_q = Q16_16::from_f64(spot);
    let strike_q = Q16_16::from_f64(strike);
    let rate_q = Q16_16::from_f64(rate);
    let time_q = Q16_16::from_f64(time);
    let market_price_q = Q16_16::from_f64(market_price);

    group.bench_function("optimized_q16_16_iv", |b| {
        b.iter(|| {
            GreeksCapsule::implied_volatility(
                black_box(spot_q),
                black_box(strike_q),
                black_box(rate_q),
                black_box(time_q),
                black_box(market_price_q),
                black_box(true),
                black_box(10), // max_iterations
            )
        })
    });

    group.finish();
}

// ============================================================================
// § 4: Batch Greeks Benchmarks (1000 options)
// ============================================================================

fn bench_batch_greeks(c: &mut Criterion) {
    let mut group = c.benchmark_group("greeks_batch_1000");
    group.throughput(Throughput::Elements(1000));

    // Generate 1000 option scenarios (mixed ATM/ITM/OTM)
    let scenarios: Vec<(f64, f64, f64, f64, f64, bool)> = (0..1000)
        .map(|i| {
            let spot = 90.0 + (i as f64 % 30.0); // 90-120 range
            let strike = 100.0;
            let rate = 0.05;
            let time = 0.25 + (i as f64 % 400.0) / 1000.0; // 0.25-0.65 years
            let volatility = 0.15 + (i as f64 % 20.0) / 100.0; // 15-35%
            let is_call = i % 2 == 0;
            (spot, strike, rate, time, volatility, is_call)
        })
        .collect();

    // Baseline: f64 batch processing
    group.bench_function("baseline_f64_batch_1000", |b| {
        b.iter(|| {
            for &(spot, strike, rate, time, volatility, is_call) in scenarios.iter() {
                black_box(calculate_greeks_f64(
                    spot, strike, rate, time, volatility, is_call,
                ));
            }
        })
    });

    // Convert to Q16.16 for optimized benchmark
    let scenarios_q: Vec<(Q16_16, Q16_16, Q16_16, Q16_16, Q16_16, bool)> = scenarios
        .iter()
        .map(|&(spot, strike, rate, time, volatility, is_call)| {
            (
                Q16_16::from_f64(spot),
                Q16_16::from_f64(strike),
                Q16_16::from_f64(rate),
                Q16_16::from_f64(time),
                Q16_16::from_f64(volatility),
                is_call,
            )
        })
        .collect();

    // Optimized: GreeksCapsule batch processing
    group.bench_function("optimized_q16_16_batch_1000", |b| {
        b.iter(|| {
            for &(spot, strike, rate, time, volatility, is_call) in scenarios_q.iter() {
                black_box(GreeksCapsule::calculate_greeks(
                    spot, strike, rate, time, volatility, is_call,
                ));
            }
        })
    });

    group.finish();
}

// ============================================================================
// § 5: Production Workload (Mixed ATM/ITM/OTM with IV)
// ============================================================================

fn bench_production_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("greeks_production_workload");
    group.throughput(Throughput::Elements(100));

    // Real-world workload: 100 options with Greeks + IV calculations
    let scenarios: Vec<(f64, f64, f64, f64, f64, bool)> = (0..100)
        .map(|i| {
            let spot = 95.0 + (i as f64 % 15.0); // 95-110 range (near ATM)
            let strike = 100.0;
            let rate = 0.05;
            let time = 0.08 + (i as f64 % 50.0) / 100.0; // 1-6 months
            let volatility = 0.18 + (i as f64 % 12.0) / 100.0; // 18-30%
            let is_call = i % 3 != 0; // 67% calls, 33% puts
            (spot, strike, rate, time, volatility, is_call)
        })
        .collect();

    // Baseline: f64 production workload
    group.bench_function("baseline_f64_production", |b| {
        b.iter(|| {
            for &(spot, strike, rate, time, volatility, is_call) in scenarios.iter() {
                // Calculate Greeks
                let greeks = calculate_greeks_f64(spot, strike, rate, time, volatility, is_call);
                black_box(greeks);

                // Calculate option price for IV solver
                let (d1, d2) = calculate_d1_d2_f64(spot, strike, rate, time, volatility);
                let exp_minus_rt = (-rate * time).exp();
                let market_price = if is_call {
                    spot * norm_cdf_f64(d1) - strike * exp_minus_rt * norm_cdf_f64(d2)
                } else {
                    strike * exp_minus_rt * norm_cdf_f64(-d2) - spot * norm_cdf_f64(-d1)
                };

                // Implied volatility solver
                let iv = implied_volatility_f64(spot, strike, rate, time, market_price, is_call);
                black_box(iv);
            }
        })
    });

    // Convert to Q16.16 for optimized benchmark
    let scenarios_q: Vec<(Q16_16, Q16_16, Q16_16, Q16_16, Q16_16, bool)> = scenarios
        .iter()
        .map(|&(spot, strike, rate, time, volatility, is_call)| {
            (
                Q16_16::from_f64(spot),
                Q16_16::from_f64(strike),
                Q16_16::from_f64(rate),
                Q16_16::from_f64(time),
                Q16_16::from_f64(volatility),
                is_call,
            )
        })
        .collect();

    // Optimized: GreeksCapsule production workload
    group.bench_function("optimized_q16_16_production", |b| {
        b.iter(|| {
            for &(spot, strike, rate, time, volatility, is_call) in scenarios_q.iter() {
                // Calculate Greeks
                let greeks =
                    GreeksCapsule::calculate_greeks(spot, strike, rate, time, volatility, is_call);
                black_box(&greeks);

                // Get price from greeks for IV solver
                let market_price = greeks.price();

                // Implied volatility solver
                let iv = GreeksCapsule::implied_volatility(
                    spot,
                    strike,
                    rate,
                    time,
                    market_price,
                    is_call,
                    10, // max_iterations
                );
                black_box(iv);
            }
        })
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_single_greeks_atm,
    bench_single_greeks_itm,
    bench_single_greeks_otm,
    bench_implied_volatility,
    bench_batch_greeks,
    bench_production_workload,
);

criterion_main!(benches);
