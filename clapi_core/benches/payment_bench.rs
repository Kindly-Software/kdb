//! B32 Payment Benchmarks
//!
//! Honest benchmarking with:
//! - Fair baseline (no strawman comparisons)
//! - Statistical rigor (1000+ iterations, 95% CI)
//! - Hardware reality (10-50% typical, 2-10× exceptional)
//! - Reproducibility (committed results)
//!
//! ## Performance Targets
//! - record_payment(): <100ns (atomic writes + fee calculation)
//! - confirm_payment(): <100ns (atomic CAS state transition)
//! - refund_payment(): <100ns (atomic CAS state transition)
//! - verify_arithmetic(): <50ns (3 atomic loads + subtraction)
//! - snapshot(): <150ns (11 atomic loads)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use clapi_core::capsules::{PaymentCapsule256, PaymentStatus};
use std::sync::Arc;
use std::thread;

/// Benchmark payment capsule creation
fn bench_payment_creation(c: &mut Criterion) {
    c.bench_function("payment_creation", |b| {
        b.iter(|| {
            let payment = PaymentCapsule256::new(
                black_box(123),
                black_box(456),
                black_box(1_000_00),
            );
            black_box(payment);
        });
    });
}

/// Benchmark fee calculation (deterministic fixed-point)
fn bench_fee_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("fee_calculation");

    for amount in [100_00, 1_000_00, 10_000_00, 100_000_00].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(amount), amount, |b, &amount| {
            b.iter(|| {
                let payment = PaymentCapsule256::new(1, 1, black_box(amount));
                black_box(payment.fee());
            });
        });
    }

    group.finish();
}

/// Benchmark state transitions
fn bench_state_transitions(c: &mut Criterion) {
    c.bench_function("start_processing", |b| {
        b.iter(|| {
            let payment = PaymentCapsule256::new(1, 1, 1_000_00);
            payment.start_processing().unwrap();
            black_box(&payment);
        });
    });

    c.bench_function("confirm_payment", |b| {
        b.iter(|| {
            let payment = PaymentCapsule256::new(1, 1, 1_000_00);
            payment.start_processing().unwrap();
            payment.confirm_payment().unwrap();
            black_box(&payment);
        });
    });

    c.bench_function("refund_payment", |b| {
        b.iter(|| {
            let payment = PaymentCapsule256::new(1, 1, 1_000_00);
            payment.start_processing().unwrap();
            payment.confirm_payment().unwrap();
            payment.refund_payment().unwrap();
            black_box(&payment);
        });
    });
}

/// Benchmark arithmetic verification
fn bench_verify_arithmetic(c: &mut Criterion) {
    let payment = PaymentCapsule256::new(1, 1, 1_000_00);

    c.bench_function("verify_arithmetic", |b| {
        b.iter(|| {
            black_box(payment.verify_arithmetic());
        });
    });
}

/// Benchmark snapshot creation
fn bench_snapshot(c: &mut Criterion) {
    let payment = PaymentCapsule256::new(1, 1, 1_000_00);

    c.bench_function("snapshot", |b| {
        b.iter(|| {
            black_box(payment.snapshot());
        });
    });
}

/// Benchmark retry increment
fn bench_retry_increment(c: &mut Criterion) {
    c.bench_function("increment_retry", |b| {
        b.iter(|| {
            let payment = PaymentCapsule256::new(1, 1, 1_000_00);
            for _ in 0..PaymentCapsule256::MAX_RETRY_COUNT {
                payment.increment_retry().unwrap();
            }
            black_box(&payment);
        });
    });
}

/// Benchmark concurrent state transitions (realistic contention)
fn bench_concurrent_confirm(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_confirm");

    for thread_count in [2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let payment = Arc::new(PaymentCapsule256::new(1, 1, 1_000_00));
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
    c.bench_function("full_lifecycle", |b| {
        b.iter(|| {
            let payment = PaymentCapsule256::new(
                black_box(1),
                black_box(123),
                black_box(1_000_00),
            );

            // Full lifecycle: Pending → Processing → Success → Refunded
            payment.start_processing().unwrap();
            payment.confirm_payment().unwrap();
            payment.refund_payment().unwrap();

            black_box(&payment);
        });
    });
}

/// Benchmark Stripe ID hashing
fn bench_stripe_id_hash(c: &mut Criterion) {
    let payment = PaymentCapsule256::new(1, 1, 1_000_00);
    let stripe_id = "pi_3N1234567890abcdefghijklmnopqrstuvwxyz";

    c.bench_function("stripe_id_hash", |b| {
        b.iter(|| {
            payment.record_stripe_id(black_box(stripe_id)).unwrap();
        });
    });
}

/// Benchmark fixed-point arithmetic vs float (comparison)
fn bench_fixed_point_vs_float(c: &mut Criterion) {
    let mut group = c.benchmark_group("fixed_point_vs_float");

    // Fixed-point (Q0.64)
    group.bench_function("fixed_point_fee", |b| {
        b.iter(|| {
            let amount: i64 = black_box(1_000_00);
            let fee = (amount * 3) / 100;
            black_box(fee);
        });
    });

    // Floating-point (baseline)
    group.bench_function("float_fee", |b| {
        b.iter(|| {
            let amount: f64 = black_box(1_000.00);
            let fee = amount * 0.03;
            black_box(fee);
        });
    });

    group.finish();
}

/// Benchmark large batch payment processing
fn bench_batch_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_processing");

    for batch_size in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    let payments: Vec<_> = (0..batch_size)
                        .map(|i| PaymentCapsule256::new(i, i, i as i64 * 100_00))
                        .collect();

                    for payment in &payments {
                        payment.start_processing().unwrap();
                        payment.confirm_payment().unwrap();
                    }

                    black_box(payments);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark generation counter increments
fn bench_generation_counter(c: &mut Criterion) {
    let payment = PaymentCapsule256::new(1, 1, 1_000_00);

    c.bench_function("generation_counter_read", |b| {
        b.iter(|| {
            black_box(payment.generation());
        });
    });

    c.bench_function("generation_counter_increment", |b| {
        b.iter(|| {
            let payment = PaymentCapsule256::new(1, 1, 1_000_00);
            payment.start_processing().unwrap(); // Increments generation
            black_box(&payment);
        });
    });
}

/// Benchmark memory layout (cache efficiency)
fn bench_cache_efficiency(c: &mut Criterion) {
    let payments: Vec<_> = (0..1000)
        .map(|i| PaymentCapsule256::new(i, i, i as i64 * 100_00))
        .collect();

    c.bench_function("sequential_snapshot", |b| {
        b.iter(|| {
            for payment in &payments {
                black_box(payment.snapshot());
            }
        });
    });
}

criterion_group!(
    benches,
    bench_payment_creation,
    bench_fee_calculation,
    bench_state_transitions,
    bench_verify_arithmetic,
    bench_snapshot,
    bench_retry_increment,
    bench_concurrent_confirm,
    bench_full_lifecycle,
    bench_stripe_id_hash,
    bench_fixed_point_vs_float,
    bench_batch_processing,
    bench_generation_counter,
    bench_cache_efficiency,
);

criterion_main!(benches);
