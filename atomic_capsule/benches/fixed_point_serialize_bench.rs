//! # FixedPointSerialize B32 Benchmarks
//!
//! **B32 Framework Compliance**:
//! - Fair baselines (scalar hashing, manual serialization)
//! - Statistical rigor (1000+ iterations, 95% CI via Criterion)
//! - Honest claims (hardware reality: 10-50% typical, 2× exceptional)
//! - Reproducible results (all benchmarks committed)
//!
//! **Performance Targets** (AMD Ryzen 9 6900HX baseline):
//! - serialize_binary (Q16_16): <50ns
//! - deserialize_binary (Q16_16): <50ns
//! - compute_hash (FNV-1a): <20ns
//! - serialize_decimal (Q16_16): <100ns
//! - batch_serialize (100 values): <5μs (50ns/value amortized)
//!
//! **Hardware Reality Checks** (B32 § 27):
//! - L1 cache hit: 4-5 cycles (~1-2ns)
//! - Memory access: 50-100ns
//! - Function call: <1ns (inlined)
//! - Integer division: 10-20 cycles (~3-7ns)

use atomic_capsule::serialize::fixed_point_impls::{Q16_16, Q32_32, Q8_8};
use atomic_capsule::serialize::fixed_point_serialize_trait::{
    FixedPointSerialize, FixedPointSerializeExt,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

// ============================================================================
// Baseline: Manual Serialization (No Trait)
// ============================================================================

fn baseline_manual_serialize_q16_16(value: &Q16_16) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(24);
    bytes.extend_from_slice(&0x46495850u32.to_le_bytes()); // Magic
    bytes.extend_from_slice(&1u16.to_le_bytes()); // Version
    bytes.extend_from_slice(&1u16.to_le_bytes()); // Field count
    bytes.extend_from_slice(&value.to_raw().to_le_bytes()); // Raw i32
                                                            // Simplified: no checksum for baseline
    bytes.extend_from_slice(&0u64.to_le_bytes());
    bytes
}

fn baseline_manual_hash_fnv1a(value: i32) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in value.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

// ============================================================================
// Micro-Benchmarks: Single Operations
// ============================================================================

fn bench_serialize_binary(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialize_binary");

    // Q8_8 (16-bit)
    let q8 = Q8_8::from_f64(12.34);
    group.bench_function("Q8_8", |b| {
        b.iter(|| {
            black_box(q8.serialize_binary().unwrap());
        })
    });

    // Q16_16 (32-bit)
    let q16 = Q16_16::from_f64(1234.5678);
    group.bench_function("Q16_16", |b| {
        b.iter(|| {
            black_box(q16.serialize_binary().unwrap());
        })
    });

    // Q32_32 (64-bit)
    let q32 = Q32_32::from_f64(1000000.123456789);
    group.bench_function("Q32_32", |b| {
        b.iter(|| {
            black_box(q32.serialize_binary().unwrap());
        })
    });

    // Baseline: Manual serialization
    group.bench_function("Q16_16_baseline_manual", |b| {
        b.iter(|| {
            black_box(baseline_manual_serialize_q16_16(&q16));
        })
    });

    group.finish();
}

fn bench_deserialize_binary(c: &mut Criterion) {
    let mut group = c.benchmark_group("deserialize_binary");

    // Q8_8
    let q8 = Q8_8::from_f64(12.34);
    let q8_bytes = q8.serialize_binary().unwrap();
    group.bench_function("Q8_8", |b| {
        b.iter(|| {
            black_box(Q8_8::deserialize_binary(&q8_bytes).unwrap());
        })
    });

    // Q16_16
    let q16 = Q16_16::from_f64(1234.5678);
    let q16_bytes = q16.serialize_binary().unwrap();
    group.bench_function("Q16_16", |b| {
        b.iter(|| {
            black_box(Q16_16::deserialize_binary(&q16_bytes).unwrap());
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

fn bench_compute_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("compute_hash");

    // Q16_16 trait method
    let q16 = Q16_16::from_f64(1234.5678);
    group.bench_function("Q16_16_trait", |b| {
        b.iter(|| {
            black_box(q16.compute_hash());
        })
    });

    // Baseline: Manual FNV-1a
    group.bench_function("Q16_16_baseline_manual", |b| {
        b.iter(|| {
            black_box(baseline_manual_hash_fnv1a(q16.to_raw()));
        })
    });

    group.finish();
}

fn bench_serialize_decimal(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialize_decimal");

    let q16 = Q16_16::from_f64(1234.5678);

    // Different precision levels
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

fn bench_deserialize_decimal(c: &mut Criterion) {
    let mut group = c.benchmark_group("deserialize_decimal");

    let decimal_str = "1234.5678";
    group.bench_function("Q16_16", |b| {
        b.iter(|| {
            black_box(Q16_16::deserialize_decimal(decimal_str).unwrap());
        })
    });

    group.finish();
}

// ============================================================================
// Batch Benchmarks (Extension Trait)
// ============================================================================

fn bench_batch_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_serialize");

    for count in [10, 100, 1000] {
        let values: Vec<Q16_16> = (0..count)
            .map(|i| Q16_16::from_f64((i as f64) * 1.05))
            .collect();

        group.bench_with_input(BenchmarkId::new("Q16_16", count), &values, |b, vals| {
            b.iter(|| {
                black_box(Q16_16::serialize_binary_batch(vals).unwrap());
            })
        });
    }

    group.finish();
}

fn bench_batch_deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_deserialize");

    for count in [10, 100, 1000] {
        let values: Vec<Q16_16> = (0..count)
            .map(|i| Q16_16::from_f64((i as f64) * 1.05))
            .collect();
        let bytes = Q16_16::serialize_binary_batch(&values).unwrap();

        group.bench_with_input(BenchmarkId::new("Q16_16", count), &bytes, |b, data| {
            b.iter(|| {
                black_box(Q16_16::deserialize_binary_batch(data).unwrap());
            })
        });
    }

    group.finish();
}

// ============================================================================
// Roundtrip Benchmarks (Combined Operations)
// ============================================================================

fn bench_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("roundtrip");

    let q16 = Q16_16::from_f64(1234.5678);

    group.bench_function("Q16_16_binary", |b| {
        b.iter(|| {
            let bytes = q16.serialize_binary().unwrap();
            black_box(Q16_16::deserialize_binary(&bytes).unwrap());
        })
    });

    group.bench_function("Q16_16_decimal", |b| {
        b.iter(|| {
            let decimal = q16.serialize_decimal(4);
            black_box(Q16_16::deserialize_decimal(&decimal).unwrap());
        })
    });

    group.finish();
}

// ============================================================================
// Extension Trait Benchmarks
// ============================================================================

fn bench_extension_trait(c: &mut Criterion) {
    let mut group = c.benchmark_group("extension_trait");

    let value_f64 = 1234.5678;

    group.bench_function("to_f64", |b| {
        let q16 = Q16_16::from_f64(value_f64);
        b.iter(|| {
            black_box(q16.to_f64());
        })
    });

    group.bench_function("from_f64", |b| {
        b.iter(|| {
            black_box(Q16_16::from_f64(value_f64).unwrap());
        })
    });

    group.finish();
}

// ============================================================================
// Throughput Benchmarks (Ops/Second)
// ============================================================================

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    group.throughput(criterion::Throughput::Elements(1));

    let q16 = Q16_16::from_f64(1234.5678);

    group.bench_function("serialize_binary_ops_per_sec", |b| {
        b.iter(|| {
            black_box(q16.serialize_binary().unwrap());
        })
    });

    group.bench_function("compute_hash_ops_per_sec", |b| {
        b.iter(|| {
            black_box(q16.compute_hash());
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_serialize_binary,
    bench_deserialize_binary,
    bench_compute_hash,
    bench_serialize_decimal,
    bench_deserialize_decimal,
    bench_batch_serialize,
    bench_batch_deserialize,
    bench_roundtrip,
    bench_extension_trait,
    bench_throughput,
);
criterion_main!(benches);
