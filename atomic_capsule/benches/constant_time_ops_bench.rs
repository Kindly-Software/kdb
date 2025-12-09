//! Constant-Time Operations Benchmarks (B32 Honest Baseline)
//!
//! # Methodology
//! - Baseline: Optimized variable-time implementations (NOT strawman)
//! - Hardware: Same CPU, same compiler flags
//! - Iterations: 1000+ per benchmark (95% CI via Criterion)
//! - Fair comparison: Both implementations highly optimized
//!
//! # Expected Results (Conservative)
//! - Constant-time = 10-30% slower than variable-time (security trade-off)
//! - Variance: <5% for constant-time, >20% for variable-time
//! - Target: <20ns for 32-byte constant-time comparison
//!
//! # B32 Reality Check
//! Constant-time code is SLOWER than variable-time (branchless overhead).
//! The value is SECURITY (prevents timing attacks), not raw speed.

use atomic_capsule::capsules::security::ConstantTimeOpsCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

// ============================================================================
// BASELINE: Variable-Time Implementations (OPTIMIZED, NOT STRAWMAN)
// ============================================================================

/// Variable-time memcmp (early return on mismatch)
/// WARNING: NOT CONSTANT-TIME (timing attack vulnerable)
#[inline(never)]
fn variable_time_memcmp(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for i in 0..a.len() {
        if a[i] != b[i] {
            return false; // Early return → timing leak
        }
    }
    true
}

/// Variable-time select (standard if-else)
/// WARNING: NOT CONSTANT-TIME (branch predictor dependent)
#[inline(never)]
fn variable_time_select(condition: bool, a: u64, b: u64) -> u64 {
    if condition {
        a
    } else {
        b
    } // Branch → timing leak
}

/// Variable-time array lookup (direct indexing)
/// WARNING: NOT CONSTANT-TIME (cache timing leak)
#[inline(never)]
fn variable_time_array_lookup(array: &[u64], index: usize) -> u64 {
    array[index] // Direct access → cache timing leak
}

// ============================================================================
// BENCHMARKS: Constant-Time vs Variable-Time
// ============================================================================

fn bench_ct_compare_32_bytes(c: &mut Criterion) {
    let mut group = c.benchmark_group("ct_compare_32_bytes");

    let ct = ConstantTimeOpsCapsule::new();
    let data_a = vec![0x12u8; 32];
    let data_b_equal = vec![0x12u8; 32];
    let data_b_unequal = vec![0x13u8; 32];

    // Constant-time (equal)
    group.bench_function("constant_time_equal", |b| {
        b.iter(|| black_box(ct.ct_compare(black_box(&data_a), black_box(&data_b_equal))));
    });

    // Constant-time (unequal, first byte differs)
    group.bench_function("constant_time_unequal_first", |b| {
        b.iter(|| black_box(ct.ct_compare(black_box(&data_a), black_box(&data_b_unequal))));
    });

    // Constant-time (unequal, last byte differs)
    let mut data_b_unequal_last = data_a.clone();
    data_b_unequal_last[31] ^= 0xFF;
    group.bench_function("constant_time_unequal_last", |b| {
        b.iter(|| black_box(ct.ct_compare(black_box(&data_a), black_box(&data_b_unequal_last))));
    });

    // Variable-time (equal) - BASELINE
    group.bench_function("variable_time_equal", |b| {
        b.iter(|| {
            black_box(variable_time_memcmp(
                black_box(&data_a),
                black_box(&data_b_equal),
            ))
        });
    });

    // Variable-time (unequal, first byte) - FAST (early return)
    group.bench_function("variable_time_unequal_first", |b| {
        b.iter(|| {
            black_box(variable_time_memcmp(
                black_box(&data_a),
                black_box(&data_b_unequal),
            ))
        });
    });

    // Variable-time (unequal, last byte) - SLOW (late mismatch)
    group.bench_function("variable_time_unequal_last", |b| {
        b.iter(|| {
            black_box(variable_time_memcmp(
                black_box(&data_a),
                black_box(&data_b_unequal_last),
            ))
        });
    });

    group.finish();
}

fn bench_ct_select(c: &mut Criterion) {
    let mut group = c.benchmark_group("ct_select");

    let ct = ConstantTimeOpsCapsule::new();

    // Constant-time select (true)
    group.bench_function("constant_time_true", |b| {
        b.iter(|| black_box(ct.ct_select(black_box(true), black_box(42), black_box(99))));
    });

    // Constant-time select (false)
    group.bench_function("constant_time_false", |b| {
        b.iter(|| black_box(ct.ct_select(black_box(false), black_box(42), black_box(99))));
    });

    // Variable-time select (true) - BASELINE
    group.bench_function("variable_time_true", |b| {
        b.iter(|| {
            black_box(variable_time_select(
                black_box(true),
                black_box(42),
                black_box(99),
            ))
        });
    });

    // Variable-time select (false) - BASELINE
    group.bench_function("variable_time_false", |b| {
        b.iter(|| {
            black_box(variable_time_select(
                black_box(false),
                black_box(42),
                black_box(99),
            ))
        });
    });

    group.finish();
}

fn bench_ct_array_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("ct_array_lookup");

    let ct = ConstantTimeOpsCapsule::new();
    let table = vec![10u64, 20, 30, 40, 50, 60, 70, 80, 90, 100];

    for idx in [0, 5, 9] {
        // Constant-time lookup
        group.bench_with_input(BenchmarkId::new("constant_time", idx), &idx, |b, &i| {
            b.iter(|| black_box(ct.ct_array_lookup(black_box(&table), black_box(i))));
        });

        // Variable-time lookup - BASELINE
        group.bench_with_input(BenchmarkId::new("variable_time", idx), &idx, |b, &i| {
            b.iter(|| black_box(variable_time_array_lookup(black_box(&table), black_box(i))));
        });
    }

    group.finish();
}

fn bench_ct_memcmp_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("ct_memcmp_sizes");

    let ct = ConstantTimeOpsCapsule::new();

    for size in [8, 16, 32, 64, 128, 256] {
        let data_a = vec![0x42u8; size];
        let data_b = vec![0x42u8; size];

        // Constant-time memcmp
        group.bench_with_input(BenchmarkId::new("constant_time", size), &size, |b, _| {
            b.iter(|| black_box(ct.ct_memcmp(black_box(&data_a), black_box(&data_b))));
        });

        // Variable-time memcmp - BASELINE
        group.bench_with_input(BenchmarkId::new("variable_time", size), &size, |b, _| {
            b.iter(|| black_box(variable_time_memcmp(black_box(&data_a), black_box(&data_b))));
        });
    }

    group.finish();
}

fn bench_hmac_verification_realistic(c: &mut Criterion) {
    // Realistic workload: HMAC-SHA256 verification (32 bytes)
    let mut group = c.benchmark_group("hmac_verification");

    let ct = ConstantTimeOpsCapsule::new();
    let hmac_computed = vec![0x12u8; 32];
    let hmac_expected = vec![0x12u8; 32];

    // Constant-time HMAC verification
    group.bench_function("constant_time_hmac_32", |b| {
        b.iter(|| black_box(ct.ct_compare(black_box(&hmac_computed), black_box(&hmac_expected))));
    });

    // Variable-time HMAC verification (INSECURE)
    group.bench_function("variable_time_hmac_32", |b| {
        b.iter(|| {
            black_box(variable_time_memcmp(
                black_box(&hmac_computed),
                black_box(&hmac_expected),
            ))
        });
    });

    group.finish();
}

fn bench_timing_variance_demonstration(c: &mut Criterion) {
    // Demonstrate variable-time timing leak (mismatch position dependent)
    let mut group = c.benchmark_group("timing_variance");

    let data_a = vec![0x12u8; 32];
    let mut data_b_first = data_a.clone();
    data_b_first[0] ^= 0xFF; // First byte differs

    let mut data_b_last = data_a.clone();
    data_b_last[31] ^= 0xFF; // Last byte differs

    // Variable-time: First byte mismatch (FAST, early return)
    group.bench_function("variable_time_first_mismatch", |b| {
        b.iter(|| {
            black_box(variable_time_memcmp(
                black_box(&data_a),
                black_box(&data_b_first),
            ))
        });
    });

    // Variable-time: Last byte mismatch (SLOW, late return)
    group.bench_function("variable_time_last_mismatch", |b| {
        b.iter(|| {
            black_box(variable_time_memcmp(
                black_box(&data_a),
                black_box(&data_b_last),
            ))
        });
    });

    // Constant-time: First byte mismatch (SAME TIME)
    let ct = ConstantTimeOpsCapsule::new();
    group.bench_function("constant_time_first_mismatch", |b| {
        b.iter(|| black_box(ct.ct_compare(black_box(&data_a), black_box(&data_b_first))));
    });

    // Constant-time: Last byte mismatch (SAME TIME)
    group.bench_function("constant_time_last_mismatch", |b| {
        b.iter(|| black_box(ct.ct_compare(black_box(&data_a), black_box(&data_b_last))));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_ct_compare_32_bytes,
    bench_ct_select,
    bench_ct_array_lookup,
    bench_ct_memcmp_sizes,
    bench_hmac_verification_realistic,
    bench_timing_variance_demonstration,
);
criterion_main!(benches);
