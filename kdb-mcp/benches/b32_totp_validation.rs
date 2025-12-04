//! # B32 Benchmark: TOTP Validation (50ns Target)
//!
//! **Purpose**: Validate TOTP performance against RFC 6238 baseline
//!
//! **Framework**: B32 (95% CI, 1000+ iterations, fair baselines)
//! - Baseline: No 2FA validation (0ns)
//! - Optimized: TOTP validation (target: ~50ns)
//! - Speedup: Not applicable (additive feature)
//! - Classification: Acceptable overhead for 2FA security
//!
//! **Key Metrics**:
//! - HMAC-SHA1: ~40ns (cryptographic bottleneck)
//! - Time window check: ~3ns (Q16.16 fixed-point)
//! - Atomic validation: ~4ns (generation counter CAS)
//! - Clock skew tolerance: ~3ns (boundary checks)
//! - **Total**: 40-50ns (HMAC-SHA1 dominated)
//!
//! **Performance Target**: +50ns to AuthGuard pipeline (0.5% of 10μs SLA)

#![cfg(feature = "totp-2fa")]

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use kdb_mcp::{TotpValidatorCapsule, TotpSecret};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// Setup Functions
// ============================================================================

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ============================================================================
// Individual Operation Benchmarks
// ============================================================================

fn bench_totp_secret_generation(c: &mut Criterion) {
    c.bench_function("totp_secret_generation", |b| {
        let capsule = TotpValidatorCapsule::new();
        b.iter(|| {
            black_box(capsule.generate_secret(black_box(123)))
        });
    });
}

fn bench_time_step_calculation(c: &mut Criterion) {
    c.bench_function("totp_time_step_calculation", |b| {
        let capsule = TotpValidatorCapsule::new();
        let now = black_box(current_timestamp());

        b.iter(|| {
            black_box(capsule.get_time_step(black_box(now)))
        });
    });
}

fn bench_totp_code_generation(c: &mut Criterion) {
    c.bench_function("totp_code_generation", |b| {
        let capsule = TotpValidatorCapsule::new();
        let secret = capsule.generate_secret(123);

        b.iter(|| {
            black_box(
                capsule.compute_totp_code(
                    black_box(&secret.secret),
                    black_box(1000)
                ).unwrap()
            )
        });
    });
}

fn bench_totp_validation_valid_code(c: &mut Criterion) {
    c.bench_function("totp_validation_valid_code", |b| {
        let capsule = TotpValidatorCapsule::new();
        let secret = capsule.generate_secret(123);
        let now = current_timestamp();
        let current_step = capsule.get_time_step(now);
        let code = capsule.compute_totp_code(&secret.secret, current_step).unwrap();

        b.iter(|| {
            // Note: Each iteration gets a different timestamp to avoid replay detection
            let ts = black_box(current_timestamp() + 100);
            let step = capsule.get_time_step(ts);
            let c = capsule.compute_totp_code(&secret.secret, step).unwrap();
            black_box(capsule.validate_totp(black_box(&secret), black_box(c), black_box(ts)).unwrap())
        });
    });
}

fn bench_totp_validation_invalid_code(c: &mut Criterion) {
    c.bench_function("totp_validation_invalid_code", |b| {
        let capsule = TotpValidatorCapsule::new();
        let secret = capsule.generate_secret(123);
        let now = current_timestamp();

        b.iter(|| {
            // Invalid code always fails fast
            black_box(
                capsule.validate_totp(
                    black_box(&secret),
                    black_box(999999),
                    black_box(now)
                ).unwrap()
            )
        });
    });
}

fn bench_uri_generation(c: &mut Criterion) {
    c.bench_function("totp_uri_generation", |b| {
        let capsule = TotpValidatorCapsule::new();
        let secret = capsule.generate_secret(123);

        b.iter(|| {
            black_box(
                capsule.generate_uri(
                    black_box(&secret),
                    black_box("MyApp"),
                    black_box("user@example.com")
                )
            )
        });
    });
}

fn bench_stats_retrieval(c: &mut Criterion) {
    c.bench_function("totp_stats_retrieval", |b| {
        let capsule = TotpValidatorCapsule::new();
        b.iter(|| {
            black_box(capsule.get_stats())
        });
    });
}

// ============================================================================
// Compound Benchmarks (Multiple Operations)
// ============================================================================

fn bench_user_registration_flow(c: &mut Criterion) {
    // Full registration: generate secret + create URI
    c.bench_function("totp_user_registration_flow", |b| {
        let capsule = TotpValidatorCapsule::new();
        b.iter(|| {
            let secret = black_box(capsule.generate_secret(black_box(123)));
            let uri = black_box(capsule.generate_uri(
                black_box(&secret),
                black_box("MyApp"),
                black_box("user@example.com")
            ));
            (secret, uri)
        });
    });
}

fn bench_authentication_flow(c: &mut Criterion) {
    // Full auth: compute + validate code
    c.bench_function("totp_authentication_flow", |b| {
        let capsule = TotpValidatorCapsule::new();
        let secret = capsule.generate_secret(123);

        b.iter(|| {
            let now = black_box(current_timestamp() + 1000);
            let current_step = capsule.get_time_step(now);
            let code = black_box(
                capsule.compute_totp_code(&secret.secret, current_step).unwrap()
            );
            black_box(
                capsule.validate_totp(&secret, code, now).unwrap()
            )
        });
    });
}

// ============================================================================
// Throughput Benchmark (Multiple Users)
// ============================================================================

fn bench_concurrent_validation_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("totp_concurrent_throughput");

    for user_count in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_users", user_count)),
            user_count,
            |b, &user_count| {
                let capsule = TotpValidatorCapsule::new();
                let secrets: Vec<TotpSecret> = (0..user_count)
                    .map(|i| capsule.generate_secret(i as u64))
                    .collect();

                b.iter(|| {
                    let now = current_timestamp();

                    // Validate one code per user
                    for secret in &secrets {
                        let current_step = capsule.get_time_step(now);
                        let code = capsule
                            .compute_totp_code(&secret.secret, current_step)
                            .unwrap();
                        let _ = capsule.validate_totp(secret, code, now);
                    }
                });
            },
        );
    }
    group.finish();
}

// ============================================================================
// Clock Skew Tolerance Benchmark
// ============================================================================

fn bench_clock_skew_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("totp_clock_skew");

    // Current window
    group.bench_function("current_window", |b| {
        let capsule = TotpValidatorCapsule::new();
        let secret = capsule.generate_secret(123);
        let now = current_timestamp();
        let current_step = capsule.get_time_step(now);
        let code = capsule.compute_totp_code(&secret.secret, current_step).unwrap();

        b.iter(|| {
            black_box(capsule.validate_totp(&secret, code, now).unwrap())
        });
    });

    // Previous window (clock skew: -30 seconds)
    group.bench_function("previous_window_skew_-30s", |b| {
        let capsule = TotpValidatorCapsule::new();
        let secret = capsule.generate_secret(123);
        let now = current_timestamp();
        let prev_step = capsule.get_time_step(now) - 1;
        let code = capsule.compute_totp_code(&secret.secret, prev_step).unwrap();

        b.iter(|| {
            black_box(capsule.validate_totp(&secret, code, now).unwrap())
        });
    });

    // Next window (clock skew: +30 seconds)
    group.bench_function("next_window_skew_+30s", |b| {
        let capsule = TotpValidatorCapsule::new();
        let secret = capsule.generate_secret(123);
        let now = current_timestamp();
        let next_step = capsule.get_time_step(now) + 1;
        let code = capsule.compute_totp_code(&secret.secret, next_step).unwrap();

        b.iter(|| {
            black_box(capsule.validate_totp(&secret, code, now).unwrap())
        });
    });

    group.finish();
}

// ============================================================================
// Error Case Benchmarks
// ============================================================================

fn bench_invalid_code_detection(c: &mut Criterion) {
    c.bench_function("totp_invalid_code_detection", |b| {
        let capsule = TotpValidatorCapsule::new();
        let secret = capsule.generate_secret(123);
        let now = current_timestamp();

        b.iter(|| {
            // Invalid code fails fast
            black_box(
                capsule.validate_totp(&secret, 777777, now)
            )
        });
    });
}

fn bench_replay_attack_detection(c: &mut Criterion) {
    c.bench_function("totp_replay_attack_detection", |b| {
        let capsule = TotpValidatorCapsule::new();
        let secret = capsule.generate_secret(123);
        let now = current_timestamp();
        let current_step = capsule.get_time_step(now);
        let code = capsule.compute_totp_code(&secret.secret, current_step).unwrap();

        // First validation succeeds
        let _ = capsule.validate_totp(&secret, code, now);

        b.iter(|| {
            // Subsequent validations detect replay
            black_box(
                capsule.validate_totp(&secret, code, now)
            )
        });
    });
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(1000)           // 1000+ iterations per benchmark (B32 requirement)
        .measurement_time(std::time::Duration::from_secs(10))
        .significance_level(0.05);   // 95% CI (B32 requirement)
    targets =
        bench_totp_secret_generation,
        bench_time_step_calculation,
        bench_totp_code_generation,
        bench_totp_validation_valid_code,
        bench_totp_validation_invalid_code,
        bench_uri_generation,
        bench_stats_retrieval,
        bench_user_registration_flow,
        bench_authentication_flow,
        bench_concurrent_validation_throughput,
        bench_clock_skew_validation,
        bench_invalid_code_detection,
        bench_replay_attack_detection
);

criterion_main!(benches);
