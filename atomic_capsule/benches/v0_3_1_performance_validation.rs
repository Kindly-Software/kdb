//! # v0.3.1 Performance Validation Benchmarks
//!
//! **Mission**: Validate ALL v0.3.1 fixes with B32 framework compliance
//!
//! ## B32 Framework Compliance
//!
//! - **B1: Fair Baselines** - Compare against unoptimized implementations
//! - **B2: Statistical Rigor** - 1000+ iterations, 95% CI (Criterion)
//! - **B3: Realistic Workloads** - Production-scale scenarios
//! - **B5: Reporting Standards** - P50, P95, P99 percentiles
//! - **K27: Honest Gains** - 10-50% typical, 2×+ exceptional, 100×+ extensive validation
//!
//! ## v0.3.1 Fixes Validated
//!
//! 1. **Serialization Performance** - Target: <50ns serialize, <100ns decimal
//! 2. **Parallel SIGSEGV Fix** - Target: No regression from CAS + drop overhead
//! 3. **Collections Stability** - Target: Maintain 3-59× speedup vs DashMap
//!
//! ## Hardware Constraints (B32 K1-K9)
//!
//! - L1 Cache: 1ns latency - Best-case memory access
//! - Atomic CAS: 10-15ns - Lockfree coordination bound
//! - memcpy: ~2ns/8B - Data movement minimum
//!
//! ## Run Benchmarks
//!
//! ```bash
//! cargo bench --bench v0_3_1_performance_validation --features capsule-serialize
//! ```

use atomic_capsule::serialize::fixed_point_impls::{Q16_16, Q32_32, Q8_8};
use atomic_capsule::serialize::fixed_point_serialize_trait::FixedPointSerialize;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// ============================================================================
// OPTIMIZATION 1: Serialization Performance (v0.3.1 Fix)
// ============================================================================

/// Baseline: Manual serialization without trait
fn baseline_manual_serialize_q16_16(value: &Q16_16) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(24);
    bytes.extend_from_slice(&0x46495850u32.to_le_bytes()); // Magic
    bytes.extend_from_slice(&1u16.to_le_bytes()); // Version
    bytes.extend_from_slice(&1u16.to_le_bytes()); // Field count
    bytes.extend_from_slice(&value.to_raw().to_le_bytes()); // Raw i32
    bytes.extend_from_slice(&0u64.to_le_bytes()); // No checksum
    bytes
}

fn bench_serialize_binary(c: &mut Criterion) {
    let mut group = c.benchmark_group("v0.3.1_serialize_binary");
    group.throughput(Throughput::Elements(1));

    // Q16_16 (target: <50ns)
    let q16 = Q16_16::from_f64(1234.5678);
    group.bench_function("Q16_16_trait", |b| {
        b.iter(|| {
            black_box(q16.serialize_binary().unwrap());
        })
    });

    // Baseline comparison
    group.bench_function("Q16_16_baseline_manual", |b| {
        b.iter(|| {
            black_box(baseline_manual_serialize_q16_16(&q16));
        })
    });

    // Q8_8 (smaller, faster)
    let q8 = Q8_8::from_f64(12.34);
    group.bench_function("Q8_8_trait", |b| {
        b.iter(|| {
            black_box(q8.serialize_binary().unwrap());
        })
    });

    // Q32_32 (larger, slower)
    let q32 = Q32_32::from_f64(1000000.123456789);
    group.bench_function("Q32_32_trait", |b| {
        b.iter(|| {
            black_box(q32.serialize_binary().unwrap());
        })
    });

    group.finish();
}

fn bench_deserialize_binary(c: &mut Criterion) {
    let mut group = c.benchmark_group("v0.3.1_deserialize_binary");
    group.throughput(Throughput::Elements(1));

    // Q16_16 (target: <50ns)
    let q16 = Q16_16::from_f64(1234.5678);
    let q16_bytes = q16.serialize_binary().unwrap();
    group.bench_function("Q16_16", |b| {
        b.iter(|| {
            black_box(Q16_16::deserialize_binary(&q16_bytes).unwrap());
        })
    });

    // Q8_8
    let q8 = Q8_8::from_f64(12.34);
    let q8_bytes = q8.serialize_binary().unwrap();
    group.bench_function("Q8_8", |b| {
        b.iter(|| {
            black_box(Q8_8::deserialize_binary(&q8_bytes).unwrap());
        })
    });

    // Q32_32
    let q32 = Q32_32::from_f64(1000000.123456789);
    let q32_bytes = q32.serialize_binary().unwrap();
    group.bench_function("Q32_32", |b| {
        b.iter(|| {
            black_box(Q32_32::deserialize_binary(&q32_bytes).unwrap());
        })
    });

    group.finish();
}

fn bench_serialize_decimal(c: &mut Criterion) {
    let mut group = c.benchmark_group("v0.3.1_serialize_decimal");
    group.throughput(Throughput::Elements(1));

    let q16 = Q16_16::from_f64(1234.5678);

    // Target: <100ns for decimal serialization
    for precision in [0, 2, 4] {
        group.bench_with_input(
            BenchmarkId::new("Q16_16", precision),
            &precision,
            |b, &prec| {
                b.iter(|| {
                    black_box(q16.serialize_decimal(prec));
                })
            },
        );
    }

    group.finish();
}

fn bench_roundtrip_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("v0.3.1_roundtrip");
    group.throughput(Throughput::Elements(1));

    let q16 = Q16_16::from_f64(1234.5678);

    // Binary roundtrip (target: <100ns total)
    group.bench_function("Q16_16_binary", |b| {
        b.iter(|| {
            let bytes = q16.serialize_binary().unwrap();
            black_box(Q16_16::deserialize_binary(&bytes).unwrap());
        })
    });

    // Decimal roundtrip (target: <200ns total)
    group.bench_function("Q16_16_decimal", |b| {
        b.iter(|| {
            let decimal = q16.serialize_decimal(4);
            black_box(Q16_16::deserialize_decimal(&decimal).unwrap());
        })
    });

    group.finish();
}

// ============================================================================
// OPTIMIZATION 2: Parallel SIGSEGV Fix (v0.3.1)
// ============================================================================

#[cfg(feature = "std")]
fn bench_parallel_cas_overhead(c: &mut Criterion) {
    use atomic_capsule::parallel::ThreadPool;

    let mut group = c.benchmark_group("v0.3.1_parallel_sigsegv_fix");
    group.sample_size(100);

    // Test that CAS + drop sequence doesn't regress performance
    group.bench_function("thread_pool_push_cas", |b| {
        let pool = ThreadPool::new(4).unwrap();
        let counter = Arc::new(AtomicUsize::new(0));

        b.iter(|| {
            let c = Arc::clone(&counter);
            pool.push(Box::new(move || {
                c.fetch_add(1, Ordering::Relaxed);
            }))
            .unwrap();
        });

        pool.wait();
    });

    // Baseline: Direct atomic operation (no pool overhead)
    group.bench_function("direct_atomic_baseline", |b| {
        let counter = Arc::new(AtomicUsize::new(0));

        b.iter(|| {
            counter.fetch_add(1, Ordering::Relaxed);
        });
    });

    group.finish();
}

#[cfg(not(feature = "std"))]
fn bench_parallel_cas_overhead(_c: &mut Criterion) {
    // No-op on no_std
}

// ============================================================================
// OPTIMIZATION 3: Collections Stability (v0.3.1)
// ============================================================================

#[cfg(feature = "std")]
fn bench_collections_insert_stability(c: &mut Criterion) {
    use atomic_capsule::collections::ConcurrentMapCapsule;

    let mut group = c.benchmark_group("v0.3.1_collections_stability");
    group.throughput(Throughput::Elements(1));

    // ConcurrentMapCapsule insert (target: <100ns, maintain 3-59× vs DashMap)
    let map = ConcurrentMapCapsule::<String, u64>::new();
    group.bench_function("concurrent_map_insert", |b| {
        let mut i = 0u64;
        b.iter(|| {
            map.insert(format!("key_{}", i), i);
            i += 1;
        })
    });

    // DashMap baseline (fair comparison)
    let dashmap = dashmap::DashMap::<String, u64>::new();
    group.bench_function("dashmap_baseline", |b| {
        let mut i = 0u64;
        b.iter(|| {
            dashmap.insert(format!("key_{}", i), i);
            i += 1;
        })
    });

    group.finish();
}

#[cfg(not(feature = "std"))]
fn bench_collections_insert_stability(_c: &mut Criterion) {
    // No-op on no_std
}

// ============================================================================
// OPTIMIZATION 4: Edge Case Validation
// ============================================================================

fn bench_edge_cases(c: &mut Criterion) {
    let mut group = c.benchmark_group("v0.3.1_edge_cases");

    // Edge case: Zero value
    let q16_zero = Q16_16::from_f64(0.0);
    group.bench_function("serialize_zero", |b| {
        b.iter(|| {
            black_box(q16_zero.serialize_binary().unwrap());
        })
    });

    // Edge case: Maximum value
    let q16_max = Q16_16::from_raw(i32::MAX);
    group.bench_function("serialize_max", |b| {
        b.iter(|| {
            black_box(q16_max.serialize_binary().unwrap());
        })
    });

    // Edge case: Minimum value
    let q16_min = Q16_16::from_raw(i32::MIN);
    group.bench_function("serialize_min", |b| {
        b.iter(|| {
            black_box(q16_min.serialize_binary().unwrap());
        })
    });

    // Edge case: Very small value (precision test)
    let q16_tiny = Q16_16::from_f64(0.0001);
    group.bench_function("serialize_tiny", |b| {
        b.iter(|| {
            black_box(q16_tiny.serialize_binary().unwrap());
        })
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_serialize_binary,
    bench_deserialize_binary,
    bench_serialize_decimal,
    bench_roundtrip_latency,
    bench_parallel_cas_overhead,
    bench_collections_insert_stability,
    bench_edge_cases,
);
criterion_main!(benches);
