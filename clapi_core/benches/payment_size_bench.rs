//! PaymentCapsule Size Comparison Benchmarks
//!
//! B32 Framework benchmarks comparing PaymentCapsule128 (128B) vs PaymentCapsule256 (256B):
//! - Creation latency
//! - Field access latency
//! - State transition latency
//! - Snapshot latency
//! - Memory footprint
//!
//! Expected Results (B32 Framework):
//! - 10-20% speedup for PaymentCapsule128 (single cache line vs two)
//! - Fair baseline: Both capsules tested under identical conditions
//! - Statistical rigor: 1000+ iterations per benchmark

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use clapi_core::capsules::{PaymentCapsule128, PaymentCapsule256};

// ============================================================================
// Creation Benchmarks
// ============================================================================

fn bench_payment_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("payment_creation");

    group.bench_function("PaymentCapsule256::new", |b| {
        b.iter(|| {
            let payment = PaymentCapsule256::new(
                black_box(123),
                black_box(456),
                black_box(1_000_00),
            );
            black_box(payment);
        })
    });

    group.bench_function("PaymentCapsule128::new", |b| {
        b.iter(|| {
            let payment = PaymentCapsule128::new(
                black_box(123),
                black_box(456),
                black_box(1_000_00),
            ).unwrap();
            black_box(payment);
        })
    });

    group.finish();
}

// ============================================================================
// Field Access Benchmarks
// ============================================================================

fn bench_payment_field_access(c: &mut Criterion) {
    let p256 = PaymentCapsule256::new(123, 456, 1_000_00);
    let p128 = PaymentCapsule128::new(123, 456, 1_000_00).unwrap();

    let mut group = c.benchmark_group("field_access");

    // Amount access (identical for both - direct load)
    group.bench_function("PaymentCapsule256::amount", |b| {
        b.iter(|| black_box(p256.amount()))
    });

    group.bench_function("PaymentCapsule128::amount", |b| {
        b.iter(|| black_box(p128.amount()))
    });

    // Fee access (PaymentCapsule256: direct load, PaymentCapsule128: unpack)
    group.bench_function("PaymentCapsule256::fee", |b| {
        b.iter(|| black_box(p256.fee()))
    });

    group.bench_function("PaymentCapsule128::fee", |b| {
        b.iter(|| black_box(p128.fee()))
    });

    // Net access (PaymentCapsule256: direct load, PaymentCapsule128: unpack)
    group.bench_function("PaymentCapsule256::net", |b| {
        b.iter(|| black_box(p256.net()))
    });

    group.bench_function("PaymentCapsule128::net", |b| {
        b.iter(|| black_box(p128.net()))
    });

    // Status access (PaymentCapsule256: load+convert, PaymentCapsule128: unpack+convert)
    group.bench_function("PaymentCapsule256::status", |b| {
        b.iter(|| black_box(p256.status()))
    });

    group.bench_function("PaymentCapsule128::status", |b| {
        b.iter(|| black_box(p128.status()))
    });

    group.finish();
}

// ============================================================================
// State Transition Benchmarks
// ============================================================================

fn bench_payment_state_transitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_transitions");

    // Pending → Processing
    group.bench_function("PaymentCapsule256::start_processing", |b| {
        b.iter_batched(
            || PaymentCapsule256::new(1, 1, 1_000_00),
            |p| {
                p.start_processing().unwrap();
                black_box(p);
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.bench_function("PaymentCapsule128::start_processing", |b| {
        b.iter_batched(
            || PaymentCapsule128::new(1, 1, 1_000_00).unwrap(),
            |p| {
                p.start_processing().unwrap();
                black_box(p);
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Processing → Success
    group.bench_function("PaymentCapsule256::confirm_payment", |b| {
        b.iter_batched(
            || {
                let p = PaymentCapsule256::new(1, 1, 1_000_00);
                p.start_processing().unwrap();
                p
            },
            |p| {
                p.confirm_payment().unwrap();
                black_box(p);
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.bench_function("PaymentCapsule128::confirm_payment", |b| {
        b.iter_batched(
            || {
                let p = PaymentCapsule128::new(1, 1, 1_000_00).unwrap();
                p.start_processing().unwrap();
                p
            },
            |p| {
                p.confirm_payment().unwrap();
                black_box(p);
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Success → Refunded
    group.bench_function("PaymentCapsule256::refund_payment", |b| {
        b.iter_batched(
            || {
                let p = PaymentCapsule256::new(1, 1, 1_000_00);
                p.start_processing().unwrap();
                p.confirm_payment().unwrap();
                p
            },
            |p| {
                p.refund_payment().unwrap();
                black_box(p);
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.bench_function("PaymentCapsule128::refund_payment", |b| {
        b.iter_batched(
            || {
                let p = PaymentCapsule128::new(1, 1, 1_000_00).unwrap();
                p.start_processing().unwrap();
                p.confirm_payment().unwrap();
                p
            },
            |p| {
                p.refund_payment().unwrap();
                black_box(p);
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

// ============================================================================
// Snapshot Benchmarks
// ============================================================================

fn bench_payment_snapshot(c: &mut Criterion) {
    let p256 = PaymentCapsule256::new(123, 456, 1_000_00);
    let p128 = PaymentCapsule128::new(123, 456, 1_000_00).unwrap();

    let mut group = c.benchmark_group("snapshot");

    group.bench_function("PaymentCapsule256::snapshot", |b| {
        b.iter(|| black_box(p256.snapshot()))
    });

    group.bench_function("PaymentCapsule128::snapshot", |b| {
        b.iter(|| black_box(p128.snapshot()))
    });

    group.finish();
}

// ============================================================================
// Hash Chain Benchmarks
// ============================================================================

fn bench_payment_hash_chain(c: &mut Criterion) {
    let p256 = PaymentCapsule256::new(123, 456, 1_000_00);
    let p128 = PaymentCapsule128::new(123, 456, 1_000_00).unwrap();

    let mut group = c.benchmark_group("hash_chain");

    group.bench_function("PaymentCapsule256::update_hash_chain", |b| {
        b.iter(|| {
            p256.update_hash_chain();
            black_box(&p256);
        })
    });

    group.bench_function("PaymentCapsule128::update_hash_chain", |b| {
        b.iter(|| {
            p128.update_hash_chain();
            black_box(&p128);
        })
    });

    group.bench_function("PaymentCapsule256::verify_chain", |b| {
        b.iter(|| black_box(p256.verify_chain()))
    });

    group.bench_function("PaymentCapsule128::verify_chain", |b| {
        b.iter(|| black_box(p128.verify_chain()))
    });

    group.finish();
}

// ============================================================================
// Arithmetic Verification Benchmarks
// ============================================================================

fn bench_payment_arithmetic(c: &mut Criterion) {
    let p256 = PaymentCapsule256::new(123, 456, 1_000_00);
    let p128 = PaymentCapsule128::new(123, 456, 1_000_00).unwrap();

    let mut group = c.benchmark_group("arithmetic");

    group.bench_function("PaymentCapsule256::verify_arithmetic", |b| {
        b.iter(|| black_box(p256.verify_arithmetic()))
    });

    group.bench_function("PaymentCapsule128::verify_arithmetic", |b| {
        b.iter(|| black_box(p128.verify_arithmetic()))
    });

    group.finish();
}

// ============================================================================
// Retry Count Benchmarks
// ============================================================================

fn bench_payment_retry(c: &mut Criterion) {
    let mut group = c.benchmark_group("retry_count");

    group.bench_function("PaymentCapsule256::increment_retry", |b| {
        b.iter_batched(
            || PaymentCapsule256::new(1, 1, 1_000_00),
            |p| {
                p.increment_retry().unwrap();
                black_box(p);
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.bench_function("PaymentCapsule128::increment_retry", |b| {
        b.iter_batched(
            || PaymentCapsule128::new(1, 1, 1_000_00).unwrap(),
            |p| {
                p.increment_retry().unwrap();
                black_box(p);
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

// ============================================================================
// Memory Footprint Benchmarks
// ============================================================================

fn bench_payment_array_access(c: &mut Criterion) {
    // Simulate array of payments (cache effects)
    const ARRAY_SIZE: usize = 1000;

    let payments256: Vec<PaymentCapsule256> = (0..ARRAY_SIZE)
        .map(|i| PaymentCapsule256::new(i as u64, i as u64, 1_000_00))
        .collect();

    let payments128: Vec<PaymentCapsule128> = (0..ARRAY_SIZE)
        .map(|i| PaymentCapsule128::new(i as u64, i as u64, 1_000_00).unwrap())
        .collect();

    let mut group = c.benchmark_group("array_access");

    // Sequential access (measures cache line efficiency)
    group.bench_function("PaymentCapsule256_array_sequential", |b| {
        b.iter(|| {
            let mut sum = 0i64;
            for p in &payments256 {
                sum += p.amount();
                sum += p.fee();
                sum += p.net();
            }
            black_box(sum);
        })
    });

    group.bench_function("PaymentCapsule128_array_sequential", |b| {
        b.iter(|| {
            let mut sum = 0i64;
            for p in &payments128 {
                sum += p.amount();
                sum += p.fee();
                sum += p.net();
            }
            black_box(sum);
        })
    });

    // Random access (measures TLB/cache misses)
    let indices: Vec<usize> = (0..100).map(|i| (i * 13) % ARRAY_SIZE).collect();

    group.bench_function("PaymentCapsule256_array_random", |b| {
        b.iter(|| {
            let mut sum = 0i64;
            for &idx in &indices {
                sum += payments256[idx].amount();
                sum += payments256[idx].fee();
                sum += payments256[idx].net();
            }
            black_box(sum);
        })
    });

    group.bench_function("PaymentCapsule128_array_random", |b| {
        b.iter(|| {
            let mut sum = 0i64;
            for &idx in &indices {
                sum += payments128[idx].amount();
                sum += payments128[idx].fee();
                sum += payments128[idx].net();
            }
            black_box(sum);
        })
    });

    group.finish();
}

// ============================================================================
// Amount Variation Benchmarks
// ============================================================================

fn bench_payment_amount_variations(c: &mut Criterion) {
    let amounts = vec![
        1_00,       // $1
        10_00,      // $10
        100_00,     // $100
        1_000_00,   // $1,000
        5_000_00,   // $5,000
        10_000_00,  // $10,000
    ];

    let mut group = c.benchmark_group("amount_variations");

    for amount in amounts {
        group.bench_with_input(
            BenchmarkId::new("PaymentCapsule256", amount),
            &amount,
            |b, &amount| {
                b.iter(|| {
                    let p = PaymentCapsule256::new(1, 1, amount);
                    black_box(p.fee());
                    black_box(p.net());
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("PaymentCapsule128", amount),
            &amount,
            |b, &amount| {
                b.iter(|| {
                    let p = PaymentCapsule128::new(1, 1, amount).unwrap();
                    black_box(p.fee());
                    black_box(p.net());
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
    bench_payment_creation,
    bench_payment_field_access,
    bench_payment_state_transitions,
    bench_payment_snapshot,
    bench_payment_hash_chain,
    bench_payment_arithmetic,
    bench_payment_retry,
    bench_payment_array_access,
    bench_payment_amount_variations,
);

criterion_main!(benches);
