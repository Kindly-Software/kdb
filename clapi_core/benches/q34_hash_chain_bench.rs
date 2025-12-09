//! Q34 Hash Chain Benchmarks (B32 Framework)
//!
//! Performance benchmarks for hash chain operations.
//!
//! # B32 Compliance
//! - Fair baselines: Measure with and without hash chain
//! - Statistical rigor: 1000+ iterations, report percentiles
//! - Honest claims: Report actual results, no cherry-picking
//! - Reproducibility: All benchmarks committed, deterministic
//!
//! # Performance Targets
//! - PaymentCapsule256::update_hash_chain(): <50ns
//! - PaymentCapsule256::verify_chain(): <60ns
//! - OAuthSessionCapsule::revoke(): <100ns (includes hash update)
//! - OAuthSessionCapsule::verify_chain(): <100ns

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use clapi_core::capsules::{OAuthSessionCapsule, PaymentCapsule256};

// ============================================================================
// PAYMENT CAPSULE HASH CHAIN BENCHMARKS
// ============================================================================

fn bench_payment_update_hash_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("payment_hash_chain/update");

    // Configure for statistical rigor (B32)
    group.sample_size(1000);
    group.measurement_time(std::time::Duration::from_secs(5));

    // Benchmark: Single hash chain update
    group.bench_function("single_update", |b| {
        let payment = PaymentCapsule256::new(1, 2, 100_00);

        b.iter(|| {
            black_box(&payment).update_hash_chain();
        });
    });

    // Benchmark: Hash update after state transition
    group.bench_function("update_after_state_change", |b| {
        let payment = PaymentCapsule256::new(1, 2, 100_00);
        payment.start_processing().unwrap();

        b.iter(|| {
            black_box(&payment).update_hash_chain();
        });
    });

    group.finish();
}

fn bench_payment_verify_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("payment_hash_chain/verify");

    group.sample_size(1000);
    group.measurement_time(std::time::Duration::from_secs(5));

    // Benchmark: Verify valid chain
    group.bench_function("verify_valid_chain", |b| {
        let payment = PaymentCapsule256::new(1, 2, 100_00);
        payment.update_hash_chain();

        b.iter(|| {
            let result = black_box(&payment).verify_chain();
            assert!(result);
        });
    });

    // Benchmark: Verify chain after multiple updates
    group.bench_function("verify_after_10_updates", |b| {
        let payment = PaymentCapsule256::new(1, 2, 100_00);
        for _ in 0..10 {
            payment.update_hash_chain();
        }

        b.iter(|| {
            let result = black_box(&payment).verify_chain();
            assert!(result);
        });
    });

    group.finish();
}

fn bench_payment_hash_n_length_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("payment_hash_chain/n_length");

    group.sample_size(100);
    group.measurement_time(std::time::Duration::from_secs(10));

    // Benchmark: Verify chain after N updates
    for n in [10, 100, 1000] {
        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(BenchmarkId::new("verify_after", n), &n, |b, &n| {
            let payment = PaymentCapsule256::new(1, 2, 100_00);

            // Pre-compute N updates
            for _ in 0..n {
                payment.update_hash_chain();
            }

            b.iter(|| {
                let result = black_box(&payment).verify_chain();
                assert!(result);
            });
        });
    }

    group.finish();
}

fn bench_payment_hash_chain_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("payment_hash_chain/overhead");

    group.sample_size(1000);

    // Baseline: Payment operations WITHOUT hash chain
    group.bench_function("baseline_state_transition_no_hash", |b| {
        let payment = PaymentCapsule256::new(1, 2, 100_00);

        b.iter(|| {
            black_box(&payment).start_processing().unwrap();
            // Do NOT update hash chain
        });
    });

    // With hash: Payment operations WITH hash chain
    group.bench_function("with_hash_state_transition_and_update", |b| {
        let payment = PaymentCapsule256::new(1, 2, 100_00);

        b.iter(|| {
            black_box(&payment).start_processing().unwrap();
            black_box(&payment).update_hash_chain();
        });
    });

    group.finish();
}

// ============================================================================
// OAUTH SESSION HASH CHAIN BENCHMARKS
// ============================================================================

fn bench_oauth_revoke_with_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("oauth_hash_chain/revoke");

    group.sample_size(1000);
    group.measurement_time(std::time::Duration::from_secs(5));

    // Benchmark: Revoke (includes hash update)
    group.bench_function("revoke_with_hash_update", |b| {
        let session = OAuthSessionCapsule::new(456, 0xABCDEF, None);

        b.iter(|| {
            black_box(&session).revoke();
        });
    });

    // Benchmark: Mark expired (includes hash update)
    group.bench_function("mark_expired_with_hash_update", |b| {
        let session = OAuthSessionCapsule::new(456, 0xABCDEF, None);

        b.iter(|| {
            black_box(&session).mark_expired();
        });
    });

    group.finish();
}

fn bench_oauth_verify_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("oauth_hash_chain/verify");

    group.sample_size(1000);
    group.measurement_time(std::time::Duration::from_secs(5));

    // Benchmark: Verify valid chain
    group.bench_function("verify_valid_chain", |b| {
        let session = OAuthSessionCapsule::new(456, 0xABCDEF, None);

        b.iter(|| {
            let result = black_box(&session).verify_chain();
            assert!(result);
        });
    });

    // Benchmark: Verify after revoke
    group.bench_function("verify_after_revoke", |b| {
        let session = OAuthSessionCapsule::new(456, 0xABCDEF, None);
        session.revoke();

        b.iter(|| {
            let result = black_box(&session).verify_chain();
            assert!(result);
        });
    });

    group.finish();
}

fn bench_oauth_refresh_with_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("oauth_hash_chain/refresh");

    group.sample_size(1000);

    // Benchmark: Refresh (updates hash due to expiry change)
    group.bench_function("refresh_with_hash_update", |b| {
        let session = OAuthSessionCapsule::new(456, 0xABCDEF, None);

        b.iter(|| {
            black_box(&session).refresh(Some(1_000_000_000));
        });
    });

    group.finish();
}

fn bench_oauth_hash_n_length_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("oauth_hash_chain/n_length");

    group.sample_size(100);
    group.measurement_time(std::time::Duration::from_secs(10));

    // Benchmark: Verify chain after N refreshes
    for n in [10, 100, 1000] {
        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(BenchmarkId::new("verify_after_refreshes", n), &n, |b, &n| {
            let session = OAuthSessionCapsule::new(456, 0xABCDEF, None);

            // Pre-compute N refreshes
            for _ in 0..n {
                session.refresh(Some(1_000_000_000));
            }

            b.iter(|| {
                let result = black_box(&session).verify_chain();
                assert!(result);
            });
        });
    }

    group.finish();
}

// ============================================================================
// CROSS-CAPSULE BENCHMARKS
// ============================================================================

fn bench_payment_and_oauth_hash_independent(c: &mut Criterion) {
    let mut group = c.benchmark_group("cross_capsule/independent_updates");

    group.sample_size(1000);

    // Benchmark: Payment hash update
    group.bench_function("payment_update_only", |b| {
        let payment = PaymentCapsule256::new(1, 2, 100_00);

        b.iter(|| {
            black_box(&payment).update_hash_chain();
        });
    });

    // Benchmark: OAuth revoke (includes hash update)
    group.bench_function("oauth_revoke_only", |b| {
        let session = OAuthSessionCapsule::new(456, 0xABCDEF, None);

        b.iter(|| {
            black_box(&session).revoke();
        });
    });

    // Benchmark: Both updates (no interference)
    group.bench_function("both_independent", |b| {
        let payment = PaymentCapsule256::new(1, 2, 100_00);
        let session = OAuthSessionCapsule::new(456, 0xABCDEF, None);

        b.iter(|| {
            black_box(&payment).update_hash_chain();
            black_box(&session).revoke();
        });
    });

    group.finish();
}

// ============================================================================
// MEMORY ACCESS PATTERNS
// ============================================================================

fn bench_hash_cache_locality(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_chain/cache_locality");

    group.sample_size(1000);

    // Benchmark: Sequential hash updates (good cache locality)
    group.bench_function("sequential_updates", |b| {
        let payment = PaymentCapsule256::new(1, 2, 100_00);

        b.iter(|| {
            for _ in 0..10 {
                black_box(&payment).update_hash_chain();
            }
        });
    });

    // Benchmark: Interleaved payment+oauth (cache pressure)
    group.bench_function("interleaved_payment_oauth", |b| {
        let payment = PaymentCapsule256::new(1, 2, 100_00);
        let session = OAuthSessionCapsule::new(456, 0xABCDEF, None);

        b.iter(|| {
            for _ in 0..5 {
                black_box(&payment).update_hash_chain();
                black_box(&session).refresh(None);
            }
        });
    });

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    payment_benches,
    bench_payment_update_hash_chain,
    bench_payment_verify_chain,
    bench_payment_hash_n_length_verification,
    bench_payment_hash_chain_overhead,
);

criterion_group!(
    oauth_benches,
    bench_oauth_revoke_with_hash,
    bench_oauth_verify_chain,
    bench_oauth_refresh_with_hash,
    bench_oauth_hash_n_length_verification,
);

criterion_group!(
    cross_capsule_benches,
    bench_payment_and_oauth_hash_independent,
    bench_hash_cache_locality,
);

criterion_main!(payment_benches, oauth_benches, cross_capsule_benches);
