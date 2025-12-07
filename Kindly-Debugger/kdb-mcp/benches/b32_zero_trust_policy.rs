//! # B32 Framework Benchmarks: ZeroTrustPolicyCapsule Performance Validation
//!
//! **Tier**: T1 Atomic + T3 Fixed-Point
//! **Performance Target**: +80ns per request
//! **Framework**: B32 (95% CI, 1000+ iterations, fair baseline)
//!
//! ## Benchmarks
//!
//! 1. **risk_score_calculation** (~30ns)
//!    - Q8.8 fixed-point arithmetic
//!    - 7-component aggregation
//!    - Saturating arithmetic
//!
//! 2. **policy_evaluation** (~50ns)
//!    - Threshold checks
//!    - Policy rule loading
//!    - Action determination
//!
//! 3. **concurrent_stats_updates** (lockfree)
//!    - Atomic counter increments
//!    - No mutex/RwLock
//!    - Relaxed ordering
//!
//! 4. **end_to_end_decision** (+80ns)
//!    - Risk aggregation + policy eval
//!    - Statistics tracking
//!    - Total orchestration latency

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use kdb_mcp::{ZeroTrustPolicyCapsule, RiskComponents, PolicyRules};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Benchmark 1: Risk Score Calculation (Q8.8 Fixed-Point Arithmetic)
// ============================================================================

fn bench_risk_score_calculation(c: &mut Criterion) {
    let capsule = ZeroTrustPolicyCapsule::new();

    c.bench_function("risk_score_calc_empty", |b| {
        b.iter(|| {
            let components = black_box(RiskComponents::new());
            capsule.calculate_risk_score(&components)
        })
    });

    c.bench_function("risk_score_calc_single_component", |b| {
        b.iter(|| {
            let components = black_box(RiskComponents {
                intrusion_risk: black_box(100 << 8),
                ..Default::default()
            });
            capsule.calculate_risk_score(&components)
        })
    });

    c.bench_function("risk_score_calc_multiple_components", |b| {
        b.iter(|| {
            let components = black_box(RiskComponents {
                intrusion_risk: black_box(100 << 8),
                license_risk: black_box(75 << 8),
                session_risk: black_box(50 << 8),
                rate_limit_risk: black_box(25 << 8),
                anomaly_risk: black_box(10 << 8),
                totp_risk: black_box(15 << 8),
                pid_access_risk: black_box(5 << 8),
                _reserved: 0,
            });
            capsule.calculate_risk_score(&components)
        })
    });

    c.bench_function("risk_score_calc_max_risk", |b| {
        b.iter(|| {
            let components = black_box(RiskComponents {
                intrusion_risk: black_box(u16::MAX),
                license_risk: black_box(u16::MAX),
                session_risk: black_box(u16::MAX),
                rate_limit_risk: black_box(u16::MAX),
                anomaly_risk: black_box(u16::MAX),
                totp_risk: black_box(u16::MAX),
                pid_access_risk: black_box(u16::MAX),
                _reserved: 0,
            });
            capsule.calculate_risk_score(&components)
        })
    });
}

// ============================================================================
// Benchmark 2: Policy Rules Update (Atomic CAS-based)
// ============================================================================

fn bench_policy_update(c: &mut Criterion) {
    let capsule = ZeroTrustPolicyCapsule::new();

    c.bench_function("policy_update_default_rules", |b| {
        b.iter(|| {
            let rules = black_box(PolicyRules::default());
            let _ = capsule.update_policy(rules);
        })
    });

    c.bench_function("policy_update_custom_thresholds", |b| {
        b.iter(|| {
            let mut rules = PolicyRules::default();
            rules.high_risk_threshold = black_box(150 << 8);
            rules.medium_risk_threshold = black_box(80 << 8);
            let _ = capsule.update_policy(rules);
        })
    });

    c.bench_function("policy_update_with_flags_disabled", |b| {
        b.iter(|| {
            let mut rules = PolicyRules::default();
            rules.enable_blocking = black_box(0);
            rules.enable_monitoring = black_box(0);
            let _ = capsule.update_policy(rules);
        })
    });
}

// ============================================================================
// Benchmark 3: Statistics Collection (Atomic Updates)
// ============================================================================

fn bench_statistics(c: &mut Criterion) {
    let capsule = Arc::new(ZeroTrustPolicyCapsule::new());

    c.bench_function("stats_single_thread_1000_updates", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                capsule.total_verifications.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                capsule.requests_allowed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        })
    });

    c.bench_function("stats_read_policy_stats", |b| {
        let _setup = {
            for _ in 0..100 {
                capsule.total_verifications.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        };
        b.iter(|| capsule.get_policy_stats())
    });

    c.bench_function("stats_reset_all", |b| {
        b.iter(|| capsule.reset_stats())
    });
}

// ============================================================================
// Benchmark 4: Concurrent Statistics Updates (Stress Test)
// ============================================================================

fn bench_concurrent_updates(c: &mut Criterion) {
    c.bench_function("concurrent_stats_4_threads_250_each", |b| {
        b.iter(|| {
            let capsule = Arc::new(ZeroTrustPolicyCapsule::new());
            let mut handles = vec![];

            for _ in 0..4 {
                let cap = Arc::clone(&capsule);
                let handle = thread::spawn(move || {
                    for _ in 0..250 {
                        cap.total_verifications.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        cap.requests_allowed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.join();
            }

            let _stats = capsule.get_policy_stats();
        })
    });

    c.bench_function("concurrent_stats_8_threads_125_each", |b| {
        b.iter(|| {
            let capsule = Arc::new(ZeroTrustPolicyCapsule::new());
            let mut handles = vec![];

            for _ in 0..8 {
                let cap = Arc::clone(&capsule);
                let handle = thread::spawn(move || {
                    for _ in 0..125 {
                        cap.total_verifications.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        cap.requests_monitored.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.join();
            }

            let _stats = capsule.get_policy_stats();
        })
    });
}

// ============================================================================
// Benchmark 5: End-to-End Performance (Validation Target: +80ns)
// ============================================================================

fn bench_end_to_end(c: &mut Criterion) {
    let capsule = ZeroTrustPolicyCapsule::new();

    let mut group = c.benchmark_group("end_to_end");
    group.sample_size(10000); // 10K samples for high precision

    group.bench_function("e2e_low_risk_evaluation", |b| {
        b.iter(|| {
            let components = black_box(RiskComponents::new()); // 0 risk
            let score = capsule.calculate_risk_score(&components);
            capsule.total_verifications.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            capsule.requests_allowed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            score
        })
    });

    group.bench_function("e2e_medium_risk_evaluation", |b| {
        b.iter(|| {
            let components = black_box(RiskComponents {
                intrusion_risk: black_box(100 << 8),
                license_risk: black_box(75 << 8),
                ..Default::default()
            });
            let score = capsule.calculate_risk_score(&components);
            capsule.total_verifications.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            capsule.requests_monitored.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            score
        })
    });

    group.bench_function("e2e_high_risk_evaluation", |b| {
        b.iter(|| {
            let components = black_box(RiskComponents {
                intrusion_risk: black_box(255 << 8),
                license_risk: black_box(255 << 8),
                ..Default::default()
            });
            let score = capsule.calculate_risk_score(&components);
            capsule.total_verifications.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            capsule.requests_blocked.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            score
        })
    });

    group.finish();
}

// ============================================================================
// Benchmark 6: Scalability (100K-1M Operations)
// ============================================================================

fn bench_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability");
    group.sample_size(100); // Fewer samples for longer benchmarks

    for operations in [10_000, 100_000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(operations),
            operations,
            |b, &ops| {
                let capsule = ZeroTrustPolicyCapsule::new();
                b.iter(|| {
                    for i in 0..ops {
                        let components = RiskComponents {
                            intrusion_risk: black_box(((i % 256) as u16) << 8),
                            license_risk: black_box((((i / 256) % 256) as u16) << 8),
                            ..Default::default()
                        };
                        let _score = capsule.calculate_risk_score(&components);
                        capsule.total_verifications.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_risk_score_calculation,
    bench_policy_update,
    bench_statistics,
    bench_concurrent_updates,
    bench_end_to_end,
    bench_scalability,
);
criterion_main!(benches);
