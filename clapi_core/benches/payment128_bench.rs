//! B32 PaymentCapsule128 Benchmarks (Agent 2 - Fixed-Point Precision Fix)
//!
//! Benchmarks for PaymentCapsule128 with Q16.8 fixed-point arithmetic.
//!
//! ## Performance Targets (B32 Framework)
//! - new(): <80ns (atomic writes + fee calculation + hash chain initialization)
//! - verify_arithmetic(): <100ns (7 atomic loads + arithmetic + ±2 cent tolerance)
//! - update_hash_chain(): <50ns (5 atomic loads + XOR)
//! - verify_chain(): <50ns (recompute hash + compare)
//! - 10-20% faster than PaymentCapsule256 (single cache line vs two)
//!
//! ## B32 Requirements
//! - Fair baseline (compare to PaymentCapsule256, not strawman)
//! - Statistical rigor (1000+ iterations, 95% CI via Criterion)
//! - Hardware reality (10-50% typical improvement, not marketing hype)
//! - Reproducibility (committed results, consistent across runs)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use clapi_core::capsules::PaymentCapsule128;
use std::sync::Arc;
use std::thread;

/// Benchmark payment capsule creation (includes hash chain initialization)
fn bench_payment_creation(c: &mut Criterion) {
    c.bench_function("payment128_creation", |b| {
        b.iter(|| {
            let payment = PaymentCapsule128::new(
                black_box(123),
                black_box(456),
                black_box(1_000_00),
            )
            .unwrap();
            black_box(payment);
        });
    });
}

/// Benchmark arithmetic verification with ±2 cent tolerance
fn bench_verify_arithmetic(c: &mut Criterion) {
    let payment = PaymentCapsule128::new(1, 1, 1_000_00).unwrap();

    c.bench_function("payment128_verify_arithmetic", |b| {
        b.iter(|| {
            black_box(payment.verify_arithmetic());
        });
    });
}

/// Benchmark arithmetic verification across different amount ranges
fn bench_verify_arithmetic_ranges(c: &mut Criterion) {
    let mut group = c.benchmark_group("payment128_verify_arithmetic_ranges");

    let test_cases = vec![
        ("small", 100),        // $1.00 (edge case for Q16.8 precision)
        ("medium", 1_000_00),  // $1,000
        ("large", 10_000_00),  // $10,000
    ];

    for (label, amount) in test_cases {
        group.bench_with_input(BenchmarkId::new("verify", label), &amount, |b, &amount| {
            let payment = PaymentCapsule128::new(1, 1, amount).unwrap();
            b.iter(|| {
                black_box(payment.verify_arithmetic());
            });
        });
    }

    group.finish();
}

/// Benchmark hash chain update
fn bench_update_hash_chain(c: &mut Criterion) {
    let payment = PaymentCapsule128::new(1, 1, 1_000_00).unwrap();

    c.bench_function("payment128_update_hash_chain", |b| {
        b.iter(|| {
            payment.update_hash_chain();
            black_box(&payment);
        });
    });
}

/// Benchmark hash chain verification
fn bench_verify_chain(c: &mut Criterion) {
    let payment = PaymentCapsule128::new(1, 1, 1_000_00).unwrap();

    c.bench_function("payment128_verify_chain", |b| {
        b.iter(|| {
            black_box(payment.verify_chain());
        });
    });
}

/// Benchmark hash chain after state transitions
fn bench_hash_chain_with_transitions(c: &mut Criterion) {
    c.bench_function("payment128_hash_chain_lifecycle", |b| {
        b.iter(|| {
            let payment = PaymentCapsule128::new(1, 1, 1_000_00).unwrap();

            // Verify initial chain
            black_box(payment.verify_chain());

            // State transition + update
            payment.start_processing().unwrap();
            payment.update_hash_chain();
            black_box(payment.verify_chain());

            // Another transition + update
            payment.confirm_payment().unwrap();
            payment.update_hash_chain();
            black_box(payment.verify_chain());
        });
    });
}

/// Benchmark Q16.8 bit packing roundtrip
fn bench_bit_packing_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("payment128_bit_packing");

    let test_cases = vec![
        ("small", 100),        // $1.00
        ("medium", 1_000_00),  // $1,000
        ("large", 10_000_00),  // $10,000
    ];

    for (label, amount) in test_cases {
        group.bench_with_input(BenchmarkId::new("pack_unpack", label), &amount, |b, &amount| {
            b.iter(|| {
                let payment = PaymentCapsule128::new(1, 1, black_box(amount)).unwrap();
                let _fee = payment.fee();
                let _net = payment.net();
                black_box(&payment);
            });
        });
    }

    group.finish();
}

/// Benchmark state transitions
fn bench_state_transitions(c: &mut Criterion) {
    c.bench_function("payment128_start_processing", |b| {
        b.iter(|| {
            let payment = PaymentCapsule128::new(1, 1, 1_000_00).unwrap();
            payment.start_processing().unwrap();
            black_box(&payment);
        });
    });

    c.bench_function("payment128_confirm_payment", |b| {
        b.iter(|| {
            let payment = PaymentCapsule128::new(1, 1, 1_000_00).unwrap();
            payment.start_processing().unwrap();
            payment.confirm_payment().unwrap();
            black_box(&payment);
        });
    });

    c.bench_function("payment128_refund_payment", |b| {
        b.iter(|| {
            let payment = PaymentCapsule128::new(1, 1, 1_000_00).unwrap();
            payment.start_processing().unwrap();
            payment.confirm_payment().unwrap();
            payment.refund_payment().unwrap();
            black_box(&payment);
        });
    });
}

/// Benchmark snapshot creation
fn bench_snapshot(c: &mut Criterion) {
    let payment = PaymentCapsule128::new(1, 1, 1_000_00).unwrap();

    c.bench_function("payment128_snapshot", |b| {
        b.iter(|| {
            black_box(payment.snapshot());
        });
    });
}

/// Benchmark concurrent state transitions (realistic contention)
fn bench_concurrent_confirm(c: &mut Criterion) {
    let mut group = c.benchmark_group("payment128_concurrent_confirm");

    for thread_count in [2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let payment = Arc::new(PaymentCapsule128::new(1, 1, 1_000_00).unwrap());
                    payment.start_processing().unwrap();

                    let handles: Vec<_> = (0..thread_count)
                        .map(|_| {
                            let p = Arc::clone(&payment);
                            thread::spawn(move || p.confirm_payment())
                        })
                        .collect();

                    for h in handles {
                        let _ = h.join();
                    }

                    black_box(&payment);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark full payment lifecycle
fn bench_full_lifecycle(c: &mut Criterion) {
    c.bench_function("payment128_full_lifecycle", |b| {
        b.iter(|| {
            let payment = PaymentCapsule128::new(
                black_box(1),
                black_box(123),
                black_box(1_000_00),
            )
            .unwrap();

            // Full lifecycle: Pending → Processing → Success → Refunded
            payment.start_processing().unwrap();
            payment.update_hash_chain();

            payment.confirm_payment().unwrap();
            payment.update_hash_chain();

            payment.refund_payment().unwrap();
            payment.update_hash_chain();

            // Final verification
            black_box(payment.verify_chain());
            black_box(&payment);
        });
    });
}

/// Benchmark cache efficiency (128B single cache line)
fn bench_cache_efficiency(c: &mut Criterion) {
    let payments: Vec<_> = (0..1000)
        .map(|i| PaymentCapsule128::new(i, i, (i as i64) * 100_00).unwrap())
        .collect();

    c.bench_function("payment128_sequential_snapshot", |b| {
        b.iter(|| {
            for payment in &payments {
                black_box(payment.snapshot());
            }
        });
    });
}

/// Benchmark Q16.8 precision edge cases
fn bench_precision_edge_cases(c: &mut Criterion) {
    let mut group = c.benchmark_group("payment128_precision");

    // Test small amounts where Q16.8 loses precision
    group.bench_function("small_amount_verify", |b| {
        let payment = PaymentCapsule128::new(1, 1, 100).unwrap(); // $1.00
        b.iter(|| {
            black_box(payment.verify_arithmetic());
        });
    });

    // Test large amounts near limits
    group.bench_function("large_amount_verify", |b| {
        let payment = PaymentCapsule128::new(1, 1, 15_000_00).unwrap(); // $15,000
        b.iter(|| {
            black_box(payment.verify_arithmetic());
        });
    });

    // Test negative amounts
    group.bench_function("negative_amount_verify", |b| {
        let payment = PaymentCapsule128::new(1, 1, -1_000_00).unwrap(); // -$1,000
        b.iter(|| {
            black_box(payment.verify_arithmetic());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_payment_creation,
    bench_verify_arithmetic,
    bench_verify_arithmetic_ranges,
    bench_update_hash_chain,
    bench_verify_chain,
    bench_hash_chain_with_transitions,
    bench_bit_packing_roundtrip,
    bench_state_transitions,
    bench_snapshot,
    bench_concurrent_confirm,
    bench_full_lifecycle,
    bench_cache_efficiency,
    bench_precision_edge_cases,
);

criterion_main!(benches);
