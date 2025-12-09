//! Hash benchmarks for kindly_dash
//!
//! This file provides minimal benchmarks for the hash module re-exports.
//! Full hash benchmarks live in atomic_capsule foundation crate.
//!
//! # UCE34 Framework Compliance
//!
//! - **Q1-Q9**: Problem = Verify hash re-exports work correctly
//! - **Q10**: Tier = N/A (benchmark stub only)
//! - **Q11**: Rust = Criterion benchmarks
//! - **Q12**: Nightly = Optional (const-hashing, simd-hashing features)
//! - **Q28-Q30**: Performance validation via B32 framework
//! - **Q33**: Validation = Ensure re-exports compile and function
//!
//! # B32 Benchmarking Framework
//!
//! These benchmarks validate that hash re-exports work correctly.
//! For comprehensive hash performance benchmarks, see:
//! - `/home/samuel/Primitives/atomic_capsule/benches/hash_bench.rs`
//!
//! Benchmark targets:
//! - const_fast_hash: 0ns (compile-time evaluation)
//! - AtomicHash64 load/store: <50ns (lockfree atomic)
//! - best_hash: Auto-select SIMD or scalar (8-20ns SIMD, ~50ns scalar)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use kindly_dash::hash::{const_fast_hash, AtomicHash64, best_hash, scalar_fast_hash};

/// Benchmark compile-time hash (const evaluation)
///
/// Expected: 0ns runtime (hash computed at compile-time)
///
/// #ASSUME: Const evaluation happens at compile-time
/// #VERIFY: This benchmark only measures const access overhead (should be negligible)
fn bench_const_hash(c: &mut Criterion) {
    c.bench_function("const_hash/static_id", |b| {
        const HASH: u64 = const_fast_hash(b"openai");
        b.iter(|| {
            black_box(HASH)
        })
    });
}

/// Benchmark AtomicHash64 load/store
///
/// Expected: <50ns (lockfree atomic operations)
///
/// #ASSUME: Relaxed ordering sufficient for benchmark
/// #VERIFY: B32 validated in atomic_capsule benches
fn bench_atomic_hash(c: &mut Criterion) {
    let hash = AtomicHash64::new();

    c.bench_function("atomic_hash64/store", |b| {
        b.iter(|| {
            hash.store(black_box(0x123456789ABCDEF0));
        })
    });

    c.bench_function("atomic_hash64/load", |b| {
        hash.store(0x123456789ABCDEF0);
        b.iter(|| {
            black_box(hash.load());
        })
    });
}

/// Benchmark runtime hash (auto-select SIMD or scalar)
///
/// Expected:
/// - Scalar: ~50ns (baseline)
/// - SIMD: 8-20ns (2-8× speedup for 4+ fields)
///
/// #ASSUME: best_hash selects optimal implementation
/// #VERIFY: SIMD enabled with nightly-all feature
fn bench_runtime_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime_hash");

    // Single field (no SIMD benefit)
    group.bench_function("best_hash/1_field", |b| {
        let data = [1u64];
        b.iter(|| {
            black_box(best_hash(black_box(&data)));
        })
    });

    // 4 fields (SIMD threshold)
    group.bench_function("best_hash/4_fields", |b| {
        let data = [1u64, 2, 3, 4];
        b.iter(|| {
            black_box(best_hash(black_box(&data)));
        })
    });

    // 8 fields (optimal SIMD)
    group.bench_function("best_hash/8_fields", |b| {
        let data = [1u64, 2, 3, 4, 5, 6, 7, 8];
        b.iter(|| {
            black_box(best_hash(black_box(&data)));
        })
    });

    group.finish();
}

/// Benchmark scalar hash (baseline comparison)
///
/// Expected: ~50ns (baseline, no SIMD)
///
/// #ASSUME: Scalar hash matches SIMD output
/// #VERIFY: Property tests in atomic_capsule validate equivalence
fn bench_scalar_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalar_hash");

    for size in [1, 4, 8, 16] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let data: Vec<u64> = (0..size).collect();
            b.iter(|| {
                black_box(scalar_fast_hash(black_box(&data)));
            })
        });
    }

    group.finish();
}

/// Benchmark SIMD hash (if available)
///
/// Expected: 8-20ns for 4+ fields (2-8× vs scalar)
///
/// #ASSUME: portable_simd available on nightly
/// #VERIFY: Feature-gated, only runs with nightly-all feature
#[cfg(feature = "simd-hashing")]
fn bench_simd_hash(c: &mut Criterion) {
    use kindly_dash::hash::simd_fast_hash_multi;

    let mut group = c.benchmark_group("simd_hash");

    for size in [4, 8, 16] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let data: Vec<u64> = (0..size).collect();
            b.iter(|| {
                black_box(simd_fast_hash_multi(black_box(&data)));
            })
        });
    }

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    hash_benches,
    bench_const_hash,
    bench_atomic_hash,
    bench_runtime_hash,
    bench_scalar_hash,
);

// Add SIMD bench group if feature enabled
#[cfg(feature = "simd-hashing")]
criterion_group!(simd_benches, bench_simd_hash);

// Main entry point
#[cfg(feature = "simd-hashing")]
criterion_main!(hash_benches, simd_benches);

#[cfg(not(feature = "simd-hashing"))]
criterion_main!(hash_benches);
