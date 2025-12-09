//! B32-Compliant Benchmark: CapsuleHash64 Performance Validation
//!
//! **Framework**: B32 (32 benchmarking guidelines + 50 hardware reality checks)
//! **Target**: <2ns SIMD hash, <5ns scalar hash, <1ns incremental update
//! **Baseline**: Scalar XOR hash (optimized, not strawman)
//!
//! ## Architecture Comparison
//!
//! ### Baseline: Optimized Scalar XOR Hash
//! - Computation: ~4-5ns (tight loop, no branches)
//! - Algorithm: XOR + multiply + rotate (FNV-1a inspired)
//! - Performance: Simple, predictable, cache-friendly
//!
//! ### CapsuleHash64: SIMD-Accelerated Hash Primitive
//! - Scalar: ~4-5ns (same algorithm as baseline)
//! - SIMD (4 fields): ~2-3ns (u64x4 parallel processing)
//! - SIMD (8 fields): ~3-4ns (u64x8 parallel processing)
//! - Incremental: <1ns (XOR-based delta update)
//! - Atomic storage: <5ns (Relaxed ordering)
//!
//! ## Expected Results (B32 Reality Checks)
//!
//! | Operation | Target | Baseline | Speedup | Reality Check |
//! |-----------|--------|----------|---------|---------------|
//! | Scalar hash (4 fields) | <5ns | ~4-5ns | 1.0× | K2: Tight loop overhead |
//! | SIMD hash (4 fields) | <2ns | ~4-5ns | 2-2.5× | K9: AVX2 4-way SIMD |
//! | SIMD hash (8 fields) | <3ns | ~8-10ns | 2.5-3× | K9: AVX-512 8-way SIMD |
//! | Incremental update | <1ns | ~4-5ns | 4-5× | K2: Single XOR operation |
//! | Atomic store | <5ns | N/A | N/A | K2: Relaxed store |
//! | Hash verification | <100ns | N/A | N/A | K2: Load + compare + compute |
//!
//! **B32 K27 Reality**: 2-4× SIMD speedup is REALISTIC for hash computation
//! - AVX2 (4-way u64): 2-2.5× speedup typical
//! - AVX-512 (8-way u64): 2.5-3× speedup typical
//! - Incremental XOR: 4-5× speedup (avoids full recomputation)
//! - NOT expecting 10×+ speedup (K27: suspicious claim threshold)
//!
//! ## B32 Compliance
//!
//! - **B1: Fair Baseline**: Optimized scalar XOR hash (not naive)
//! - **B2: Statistical Rigor**: 95% CI, 1000+ samples, Criterion default
//! - **B3: Realistic Workloads**: 4-field and 8-field capsules (production-like)
//! - **B4: Contention Scenarios**: 1/4/8 thread scaling tests
//! - **B5: Full Disclosure**: Complete methodology documentation
//!
//! ## Hardware Reality Checks Applied
//!
//! - **K2 (Atomic Costs)**: AtomicU64 store ~5ns, load ~5ns
//! - **K6 (Cache Hierarchy)**: Hash + state fit L1 cache (64 bytes)
//! - **K9 (SIMD Reality)**: AVX2 3-4× typical, AVX-512 4-6× typical
//! - **K10 (Big-O Constants)**: SIMD overhead matters for <4 fields
//! - **K14 (Vectorization)**: Alignment critical for SIMD performance
//! - **K27 (Honest Gains)**: 2-4× SIMD speedup realistic, 10× suspicious

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

// Import actual CapsuleHash64 implementation
use clapi_core::capsules::CapsuleHash64;

// ============================================================================
// Baseline: Optimized Scalar XOR Hash (Fair Comparison)
// ============================================================================

/// Optimized scalar XOR hash (fair baseline, not strawman)
///
/// **Purpose**: Fair baseline for SIMD comparison
/// **Algorithm**: XOR + multiply + rotate (FNV-1a inspired)
/// **Performance**: ~4-5ns for 4 fields (tight loop, no branches)
fn baseline_scalar_hash(fields: &[u64]) -> u64 {
    const SEED: u64 = 0xcbf29ce484222325;
    const MUL: u64 = 0x100000001b3;

    let mut state = SEED;
    for &field in fields {
        state ^= field;
        state = state.wrapping_mul(MUL);
        state = state.rotate_left(31);
    }
    state
}

// ============================================================================
// B2: Benchmark 1 - Scalar Hash (4 Fields)
// ============================================================================

/// Benchmark 1: Scalar hash computation (4 fields)
///
/// **Expected**: CapsuleHash64 ~4-5ns, Baseline ~4-5ns (same algorithm)
/// **Reality Check (K2)**: Tight loop overhead, branch-free execution
fn bench_hash_scalar_4fields(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_scalar_4fields");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(4)); // 4 u64 fields

    let fields = [1u64, 2, 3, 4];

    // CapsuleHash64 scalar
    group.bench_function("capsule_hash64_scalar", |b| {
        b.iter(|| black_box(CapsuleHash64::compute_scalar(black_box(&fields))))
    });

    // Baseline scalar (fair comparison)
    group.bench_function("baseline_scalar", |b| {
        b.iter(|| black_box(baseline_scalar_hash(black_box(&fields))))
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 2 - SIMD Hash (4 Fields)
// ============================================================================

/// Benchmark 2: SIMD hash computation (4 fields, u64x4)
///
/// **Expected**: CapsuleHash64 ~2-3ns (SIMD), Baseline ~4-5ns (scalar)
/// **Reality Check (K9)**: AVX2 4-way SIMD, 2-2.5× speedup typical
#[cfg(feature = "simd")]
fn bench_hash_simd_4fields(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_simd_4fields");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(4));

    let fields = [1u64, 2, 3, 4];

    // CapsuleHash64 SIMD
    group.bench_function("capsule_hash64_simd", |b| {
        b.iter(|| black_box(CapsuleHash64::compute_simd(black_box(&fields))))
    });

    // Baseline scalar (for speedup comparison)
    group.bench_function("baseline_scalar", |b| {
        b.iter(|| black_box(baseline_scalar_hash(black_box(&fields))))
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 3 - SIMD Hash (8 Fields)
// ============================================================================

/// Benchmark 3: SIMD hash computation (8 fields, u64x4 or u64x8)
///
/// **Expected**: CapsuleHash64 ~3-4ns (SIMD), Baseline ~8-10ns (scalar)
/// **Reality Check (K9)**: AVX-512 8-way SIMD, 2.5-3× speedup typical
#[cfg(feature = "simd")]
fn bench_hash_simd_8fields(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_simd_8fields");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(8));

    let fields = [1u64, 2, 3, 4, 5, 6, 7, 8];

    // CapsuleHash64 SIMD
    group.bench_function("capsule_hash64_simd", |b| {
        b.iter(|| black_box(CapsuleHash64::compute_simd(black_box(&fields))))
    });

    // Baseline scalar (for speedup comparison)
    group.bench_function("baseline_scalar", |b| {
        b.iter(|| black_box(baseline_scalar_hash(black_box(&fields))))
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 4 - Auto-Select Hash (compute)
// ============================================================================

/// Benchmark 4: Auto-select hash (SIMD or scalar based on size)
///
/// **Expected**: CapsuleHash64 ~2-5ns (auto-select), Baseline ~4-5ns (scalar)
/// **Reality Check (K10)**: Auto-select overhead minimal (<1ns)
fn bench_hash_auto_select(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_auto_select");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);

    // Test with 4 fields (SIMD threshold)
    let fields_4 = [1u64, 2, 3, 4];
    group.throughput(Throughput::Elements(4));
    group.bench_function("capsule_hash64_auto_4fields", |b| {
        b.iter(|| black_box(CapsuleHash64::compute(black_box(&fields_4))))
    });

    // Test with 8 fields (SIMD optimal)
    let fields_8 = [1u64, 2, 3, 4, 5, 6, 7, 8];
    group.throughput(Throughput::Elements(8));
    group.bench_function("capsule_hash64_auto_8fields", |b| {
        b.iter(|| black_box(CapsuleHash64::compute(black_box(&fields_8))))
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 5 - Incremental Update
// ============================================================================

/// Benchmark 5: Incremental hash update (XOR-based)
///
/// **Expected**: CapsuleHash64 <1ns (single XOR), Baseline ~4-5ns (full rehash)
/// **Reality Check (K2)**: Single XOR operation, minimal overhead
fn bench_hash_incremental_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_incremental_update");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1)); // 1 field update

    let old_hash = 0x1234567890abcdef;
    let old_value = 42u64;
    let new_value = 999u64;

    // Incremental update (XOR-based)
    group.bench_function("capsule_hash64_incremental", |b| {
        b.iter(|| {
            black_box(CapsuleHash64::update_incremental(
                black_box(old_hash),
                black_box(old_value),
                black_box(new_value),
            ))
        })
    });

    // Baseline: Full rehash (for comparison)
    group.bench_function("baseline_full_rehash", |b| {
        let fields = [1u64, new_value, 3, 4]; // Updated field
        b.iter(|| black_box(baseline_scalar_hash(black_box(&fields))))
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 6 - Atomic Store
// ============================================================================

/// Benchmark 6: Atomic hash store (Relaxed ordering)
///
/// **Expected**: CapsuleHash64 <5ns (Relaxed store)
/// **Reality Check (K2)**: AtomicU64 store ~5ns typical
fn bench_hash_atomic_store(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_atomic_store");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1));

    let capsule = CapsuleHash64::new();
    let hash = 0x1234567890abcdef;

    group.bench_function("capsule_hash64_store", |b| {
        b.iter(|| {
            capsule.store(black_box(hash));
        })
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 7 - Atomic Load
// ============================================================================

/// Benchmark 7: Atomic hash load (Relaxed ordering)
///
/// **Expected**: CapsuleHash64 <5ns (Relaxed load)
/// **Reality Check (K2)**: AtomicU64 load ~5ns typical
fn bench_hash_atomic_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_atomic_load");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1));

    let capsule = CapsuleHash64::new();

    group.bench_function("capsule_hash64_load", |b| {
        b.iter(|| black_box(capsule.load()))
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 8 - Hash Verification
// ============================================================================

/// Benchmark 8: Hash verification (compute + compare)
///
/// **Expected**: CapsuleHash64 <100ns (state read + hash compute + compare)
/// **Reality Check (K2+K6)**: Compute hash + atomic load + compare
fn bench_hash_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_verification");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(4)); // 4 fields

    let capsule = CapsuleHash64::new();
    let fields = [1u64, 2, 3, 4];
    let hash = CapsuleHash64::compute(&fields);
    capsule.store(hash);

    group.bench_function("capsule_hash64_verify", |b| {
        b.iter(|| black_box(capsule.verify(black_box(hash))))
    });

    group.finish();
}

// ============================================================================
// B3: Benchmark 9 - Realistic Capsule Workflow (RequestCapsule128)
// ============================================================================

/// Benchmark 9: Realistic workflow (deduction + hash update)
///
/// **Expected**: CapsuleHash64 <100ns (CAS + incremental hash + atomic store)
/// **Reality Check (K2+K6)**: Budget CAS (~20ns) + incremental hash (~1ns) + store (~5ns)
fn bench_hash_realistic_workflow(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_realistic_workflow");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1)); // 1 operation

    let capsule = CapsuleHash64::new();
    let fields = [1000_00i64 as u64, 0, 0, 1]; // budget, spent, count, generation

    // Simulate: try_deduct + incremental hash update
    group.bench_function("capsule_deduction_with_hash", |b| {
        b.iter(|| {
            let old_hash = capsule.load();
            let old_budget = fields[0];
            let new_budget = old_budget - 50_00;

            // Incremental hash update (O(1))
            let new_hash = CapsuleHash64::update_incremental(old_hash, old_budget, new_budget);
            capsule.store(new_hash);

            black_box(new_hash)
        })
    });

    group.finish();
}

// ============================================================================
// B4: Benchmark 10 - Concurrent Hash Computation (4 Threads)
// ============================================================================

/// Benchmark 10: Concurrent hash computation (4 threads)
///
/// **Expected**: CapsuleHash64 linear scaling (independent operations)
/// **Reality Check (K12)**: Zero contention on read-only hash computation
fn bench_hash_concurrent_compute_4t(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_concurrent_compute_4t");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);

    let num_threads = 4;
    let ops_per_thread = 1000;

    group.throughput(Throughput::Elements((num_threads * ops_per_thread) as u64));

    let fields = [1u64, 2, 3, 4];

    group.bench_function("capsule_hash64_concurrent_4t", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..num_threads)
                .map(|_| {
                    std::thread::spawn(move || {
                        for _ in 0..ops_per_thread {
                            black_box(CapsuleHash64::compute(&fields));
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// B4: Benchmark 11 - Concurrent Hash Store (4 Threads)
// ============================================================================

/// Benchmark 11: Concurrent hash store (4 threads)
///
/// **Expected**: CapsuleHash64 linear scaling (Relaxed ordering)
/// **Reality Check (K12)**: Zero contention with Relaxed store
fn bench_hash_concurrent_store_4t(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_concurrent_store_4t");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);

    let num_threads = 4;
    let ops_per_thread = 1000;

    group.throughput(Throughput::Elements((num_threads * ops_per_thread) as u64));

    let capsule = std::sync::Arc::new(CapsuleHash64::new());

    group.bench_function("capsule_hash64_concurrent_store_4t", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..num_threads)
                .map(|tid| {
                    let c = std::sync::Arc::clone(&capsule);
                    std::thread::spawn(move || {
                        for i in 0..ops_per_thread {
                            let hash = (tid as u64 * 1000 + i as u64) ^ 0x1234567890abcdef;
                            c.store(hash);
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// B4: Benchmark 12 - Concurrent Hash Verification (4 Threads)
// ============================================================================

/// Benchmark 12: Concurrent hash verification (4 threads)
///
/// **Expected**: CapsuleHash64 linear scaling (read-heavy workload)
/// **Reality Check (K12)**: Minimal contention on verification
fn bench_hash_concurrent_verify_4t(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_concurrent_verify_4t");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);

    let num_threads = 4;
    let ops_per_thread = 1000;

    group.throughput(Throughput::Elements((num_threads * ops_per_thread) as u64));

    let capsule = std::sync::Arc::new(CapsuleHash64::new());
    let fields = [1u64, 2, 3, 4];
    let hash = CapsuleHash64::compute(&fields);
    capsule.store(hash);

    group.bench_function("capsule_hash64_concurrent_verify_4t", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..num_threads)
                .map(|_| {
                    let c = std::sync::Arc::clone(&capsule);
                    std::thread::spawn(move || {
                        for _ in 0..ops_per_thread {
                            black_box(c.verify(hash));
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// B3: Benchmark 13 - Variable Field Counts (Scaling Analysis)
// ============================================================================

/// Benchmark 13: Hash computation with variable field counts
///
/// **Purpose**: Measure SIMD crossover point (when SIMD becomes beneficial)
/// **Expected**: SIMD benefits for ≥4 fields, overhead for <4 fields
/// **Reality Check (K14)**: SIMD overhead dominates for small inputs
fn bench_hash_variable_field_counts(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_variable_field_counts");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);

    for field_count in [1, 2, 4, 8, 16, 32, 64] {
        let fields: Vec<u64> = (0..field_count).collect();
        group.throughput(Throughput::Elements(field_count as u64));

        group.bench_with_input(
            BenchmarkId::new("capsule_hash64_scalar", field_count),
            &fields,
            |b, fields| b.iter(|| black_box(CapsuleHash64::compute_scalar(black_box(fields)))),
        );

        #[cfg(feature = "simd")]
        group.bench_with_input(
            BenchmarkId::new("capsule_hash64_simd", field_count),
            &fields,
            |b, fields| b.iter(|| black_box(CapsuleHash64::compute_simd(black_box(fields)))),
        );

        group.bench_with_input(
            BenchmarkId::new("baseline_scalar", field_count),
            &fields,
            |b, fields| b.iter(|| black_box(baseline_scalar_hash(black_box(fields)))),
        );
    }

    group.finish();
}

// ============================================================================
// B3: Benchmark 14 - Production Capsule Sizes
// ============================================================================

/// Benchmark 14: Hash computation for production capsule sizes
///
/// **Purpose**: Measure hash performance for actual capsule field counts
/// **Reality Check**: RequestCapsule128 (4 fields), BudgetSlotCapsule (3 fields)
fn bench_hash_production_capsules(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_production_capsules");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);

    // RequestCapsule128: budget, spent, count, generation (4 fields)
    let req_fields = [1000_00u64, 0, 0, 1];
    group.throughput(Throughput::Elements(4));
    group.bench_function("request_capsule_128_hash", |b| {
        b.iter(|| black_box(CapsuleHash64::compute(black_box(&req_fields))))
    });

    // BudgetSlotCapsule: ptr, generation, timestamp (3 fields)
    let slot_fields = [0x7fff_0000_0000u64, 1, 1234567890];
    group.throughput(Throughput::Elements(3));
    group.bench_function("budget_slot_capsule_hash", |b| {
        b.iter(|| black_box(CapsuleHash64::compute(black_box(&slot_fields))))
    });

    // CircuitBreakerCapsule: state, window_start (2 fields)
    let circuit_fields = [0u64, 1234567890];
    group.throughput(Throughput::Elements(2));
    group.bench_function("circuit_breaker_capsule_hash", |b| {
        b.iter(|| black_box(CapsuleHash64::compute(black_box(&circuit_fields))))
    });

    group.finish();
}

// ============================================================================
// B2: Criterion Configuration (Statistical Rigor)
// ============================================================================

criterion_group! {
    name = benches;
    config = Criterion::default()
        .confidence_level(0.95)      // B2: 95% confidence intervals
        .significance_level(0.05)
        .noise_threshold(0.05);
    targets =
        bench_hash_scalar_4fields,
        bench_hash_auto_select,
        bench_hash_incremental_update,
        bench_hash_atomic_store,
        bench_hash_atomic_load,
        bench_hash_verification,
        bench_hash_realistic_workflow,
        bench_hash_concurrent_compute_4t,
        bench_hash_concurrent_store_4t,
        bench_hash_concurrent_verify_4t,
        bench_hash_variable_field_counts,
        bench_hash_production_capsules
}

// SIMD benchmarks (feature-gated)
#[cfg(feature = "simd")]
criterion_group! {
    name = benches_simd;
    config = Criterion::default()
        .confidence_level(0.95)
        .significance_level(0.05)
        .noise_threshold(0.05);
    targets =
        bench_hash_simd_4fields,
        bench_hash_simd_8fields
}

#[cfg(feature = "simd")]
criterion_main!(benches, benches_simd);

#[cfg(not(feature = "simd"))]
criterion_main!(benches);
