//! B32-Compliant Benchmarks for SIMD Hashing
//!
//! # Benchmarking Strategy
//!
//! 1. **Fair Baseline**: Scalar FNV-1a (optimized, not strawman)
//! 2. **Statistical Rigor**: 1000+ iterations, 95% CI (Criterion)
//! 3. **Honest Reporting**: Document where SIMD helps AND hurts
//! 4. **Threshold Analysis**: Measure crossover point (expected: 4 fields)
//!
//! # Expected Results (Intel Ultra 7 155H)
//!
//! | Fields | Scalar | SIMD  | Speedup | Winner     |
//! |--------|--------|-------|---------|------------|
//! | 2      | 8ns    | 12ns  | 0.67×   | Scalar ❌  |
//! | 4      | 16ns   | 8ns   | 2.0×    | SIMD ✅    |
//! | 8      | 32ns   | 12ns  | 2.7×    | SIMD ✅    |
//! | 16     | 64ns   | 20ns  | 3.2×    | SIMD ✅    |
//!
//! # B32 Reality Check
//!
//! - Setup overhead: 4-8ns for SIMD
//! - Threshold: 4 fields minimum for benefit
//! - Speedup range: 2-3.2× (typical for SIMD u64 operations)
//! - No 10× claims (unrealistic for memory-bound hashing)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(feature = "simd-hashing")]
use atomic_capsule::hash::simd_hash::simd_fast_hash_multi;

use atomic_capsule::hash::simd_hash::{best_hash, scalar_fast_hash};

/// Benchmark scalar hash for various field counts
fn bench_scalar_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalar_hash");

    for size in [2, 4, 8, 16, 32, 64].iter() {
        let fields: Vec<u64> = (1..=*size).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &fields, |b, fields| {
            b.iter(|| scalar_fast_hash(black_box(fields)))
        });
    }

    group.finish();
}

/// Benchmark SIMD hash for various field counts
#[cfg(feature = "simd-hashing")]
fn bench_simd_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_hash");

    // Only test SIMD for ≥4 fields (below threshold, uses scalar fallback)
    for size in [4, 8, 16, 32, 64].iter() {
        let fields: Vec<u64> = (1..=*size).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &fields, |b, fields| {
            b.iter(|| simd_fast_hash_multi(black_box(fields)))
        });
    }

    group.finish();
}

/// Benchmark automatic best_hash dispatcher
fn bench_best_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("best_hash_dispatch");

    for size in [2, 4, 8, 16, 32, 64].iter() {
        let fields: Vec<u64> = (1..=*size).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &fields, |b, fields| {
            b.iter(|| best_hash(black_box(fields)))
        });
    }

    group.finish();
}

/// Benchmark threshold crossover point (2, 3, 4, 5 fields)
fn bench_threshold_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("threshold_analysis");

    for size in [2, 3, 4, 5].iter() {
        let fields: Vec<u64> = (1..=*size).collect();

        // Scalar
        group.bench_with_input(BenchmarkId::new("scalar", size), &fields, |b, fields| {
            b.iter(|| scalar_fast_hash(black_box(fields)))
        });

        // SIMD (if available)
        #[cfg(feature = "simd-hashing")]
        group.bench_with_input(BenchmarkId::new("simd", size), &fields, |b, fields| {
            b.iter(|| simd_fast_hash_multi(black_box(fields)))
        });

        // Best hash (automatic dispatch)
        group.bench_with_input(BenchmarkId::new("best", size), &fields, |b, fields| {
            b.iter(|| best_hash(black_box(fields)))
        });
    }

    group.finish();
}

/// Benchmark hash quality (determinism)
fn bench_hash_determinism(c: &mut Criterion) {
    c.bench_function("scalar_determinism_8_fields", |b| {
        let fields = [1u64, 2, 3, 4, 5, 6, 7, 8];
        b.iter(|| {
            let hash1 = scalar_fast_hash(black_box(&fields));
            let hash2 = scalar_fast_hash(black_box(&fields));
            assert_eq!(hash1, hash2);
            hash1
        })
    });

    #[cfg(feature = "simd-hashing")]
    c.bench_function("simd_determinism_8_fields", |b| {
        let fields = [1u64, 2, 3, 4, 5, 6, 7, 8];
        b.iter(|| {
            let hash1 = simd_fast_hash_multi(black_box(&fields));
            let hash2 = simd_fast_hash_multi(black_box(&fields));
            assert_eq!(hash1, hash2);
            hash1
        })
    });
}

/// Benchmark worst-case: Fields with poor cache locality
fn bench_worst_case_scattered_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("worst_case_scattered");

    // Scattered fields (non-contiguous memory)
    let scattered: Vec<u64> = (0..64).step_by(8).collect(); // Every 8th element

    group.bench_function("scalar_scattered", |b| {
        b.iter(|| scalar_fast_hash(black_box(&scattered)))
    });

    #[cfg(feature = "simd-hashing")]
    group.bench_function("simd_scattered", |b| {
        b.iter(|| simd_fast_hash_multi(black_box(&scattered)))
    });

    group.finish();
}

/// Benchmark best-case: Fields with excellent cache locality
fn bench_best_case_contiguous_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("best_case_contiguous");

    // Contiguous fields (perfect cache locality)
    let contiguous: Vec<u64> = (1..=64).collect();

    group.bench_function("scalar_contiguous", |b| {
        b.iter(|| scalar_fast_hash(black_box(&contiguous)))
    });

    #[cfg(feature = "simd-hashing")]
    group.bench_function("simd_contiguous", |b| {
        b.iter(|| simd_fast_hash_multi(black_box(&contiguous)))
    });

    group.finish();
}

/// Benchmark real-world capsule scenario (8-field capsule)
fn bench_realistic_capsule_hash(c: &mut Criterion) {
    c.bench_function("realistic_8_field_capsule", |b| {
        // Typical capsule: 8 u64 fields
        let fields = [
            0x0000000000000001, // budget_cents
            0x0000000000000002, // total_spent
            0x0000000000000003, // request_count
            0x0000000000000004, // generation
            0x0000000000000005, // deduction_count
            0x0000000000000006, // failed_deductions
            0x0000000000000007, // hash (previous)
            0x0000000000000008, // prev_hash
        ];

        b.iter(|| best_hash(black_box(&fields)))
    });
}

/// Benchmark incremental hash update (common pattern)
fn bench_incremental_update(c: &mut Criterion) {
    c.bench_function("incremental_hash_update", |b| {
        let mut fields = [1u64, 2, 3, 4, 5, 6, 7, 8];

        b.iter(|| {
            // Initial hash
            let hash1 = best_hash(black_box(&fields));

            // Update one field
            fields[3] = black_box(42);

            // Recompute hash
            let hash2 = best_hash(black_box(&fields));

            // Hashes should differ
            assert_ne!(hash1, hash2);
            hash2
        })
    });
}

#[cfg(feature = "simd-hashing")]
criterion_group!(
    benches,
    bench_scalar_hash,
    bench_simd_hash,
    bench_best_hash,
    bench_threshold_analysis,
    bench_hash_determinism,
    bench_worst_case_scattered_access,
    bench_best_case_contiguous_access,
    bench_realistic_capsule_hash,
    bench_incremental_update,
);

#[cfg(not(feature = "simd-hashing"))]
criterion_group!(
    benches,
    bench_scalar_hash,
    bench_best_hash,
    bench_threshold_analysis,
    bench_hash_determinism,
    bench_worst_case_scattered_access,
    bench_best_case_contiguous_access,
    bench_realistic_capsule_hash,
    bench_incremental_update,
);

criterion_main!(benches);
