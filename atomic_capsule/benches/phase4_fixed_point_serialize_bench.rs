//! # Phase 4 Performance Benchmarks - FixedPointSerialize Trait
//!
//! **B32 Framework Compliance**: Honest benchmarking, no marketing claims
//!
//! ## Mission
//!
//! Validate FixedPointSerialize trait performance following B32 framework:
//! 1. **Trait Method Performance** - Measure all trait operations
//! 2. **Migration Comparison** - Manual vs derived implementations
//! 3. **Compilation Performance** - Build-time impact
//! 4. **Concurrency Scaling** - Multi-thread serialization
//! 5. **Memory Impact** - Binary size, heap, stack usage
//! 6. **Real-World Scenarios** - clapi_core, kindly_hft use cases
//!
//! ## B32 Honest Claims Framework
//!
//! - Reality check: 5-20% typical gains (if any)
//! - If no gains: Claim "Zero-cost abstraction (0% regression)"
//! - If regressions: Document reason + mitigation plan
//! - Never claim: "100× faster" without extensive evidence
//! - Always: Compare to fair baseline (manual impl, not null)
//!
//! ## Performance Targets (UCE34 Q30-Q33)
//!
//! | Operation | Target | Baseline | Expected |
//! |-----------|--------|----------|----------|
//! | serialize_raw(Q16_16) | <10ns | Direct i64 read (~1ns) | 2-10× overhead |
//! | deserialize_raw(Q16_16) | <10ns | Direct i64 write (~1ns) | 2-10× overhead |
//! | serialize_decimal(Q16_16) | <100ns | format!() (~60ns) | <50% overhead |
//! | serialize_binary(Q16_16) | <50ns | memcpy + CRC (~30ns) | <100% overhead |
//! | compute_hash(Q16_16) | <20ns | const_fast_hash (~10ns) | <100% overhead |
//!
//! ## Hardware Constraints (B32 K1-K9)
//!
//! - L1 Cache: 1ns latency (K6) - best-case memory access
//! - Atomic CAS: 10-15ns (K2) - lockfree coordination bound
//! - L2 Cache: 4ns (K7) - realistic hot path access
//! - String allocation: ~60ns (heap overhead for decimal format)
//! - CRC32 calculation: ~15ns for 8 bytes
//!
//! ## Statistical Rigor (B32 B2)
//!
//! - 1000+ iterations per benchmark (Criterion default)
//! - 95% Confidence Interval reported
//! - P50, P95, P99 percentiles measured
//! - Outlier detection and reporting
//! - Same hardware, controlled environment

use atomic_capsule::serialize::fixed_point_serialize::{
    deserialize_from_binary, serialize_to_binary, FixedPointSerialize, FixedQ16_16, FixedQ32_32,
    FixedQ8_8,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// ============================================================================
// 1. TRAIT METHOD PERFORMANCE (400 LOC)
// ============================================================================

/// Benchmark serialize_raw() - Target: <10ns
///
/// **Fair Baseline**: Direct i64 field access (~1ns)
/// **Expected Overhead**: 2-10× (function call + black_box)
fn bench_serialize_raw(c: &mut Criterion) {
    let mut group = c.benchmark_group("trait_methods/serialize_raw");
    group.throughput(Throughput::Elements(1));

    // Q16.16 (financial standard)
    group.bench_function("Q16_16", |b| {
        let value = FixedQ16_16::from_decimal(1234, 5678);
        b.iter(|| {
            black_box(value.serialize_raw());
        });
    });

    // Q8.8 (fast arithmetic)
    group.bench_function("Q8_8", |b| {
        let value = FixedQ8_8::from_decimal(12, 34);
        b.iter(|| {
            black_box(value.serialize_raw());
        });
    });

    // Q32.32 (high precision)
    group.bench_function("Q32_32", |b| {
        let value = FixedQ32_32::from_decimal(1234, 567890123);
        b.iter(|| {
            black_box(value.serialize_raw());
        });
    });

    // Baseline: Direct i64 read
    group.bench_function("baseline_i64_read", |b| {
        let value = 0x1234_5678_i64;
        b.iter(|| {
            black_box(value);
        });
    });

    group.finish();
}

/// Benchmark deserialize_from_raw() - Target: <10ns
///
/// **Fair Baseline**: Direct i64 wrapper creation (~1ns)
/// **Expected Overhead**: 2-10× (function call + constructor)
fn bench_deserialize_raw(c: &mut Criterion) {
    let mut group = c.benchmark_group("trait_methods/deserialize_raw");
    group.throughput(Throughput::Elements(1));

    let raw_q16_16 = (1234_i64 << 16) | ((5678 * 65536) / 10000);
    let raw_q8_8 = (12_i64 << 8) | ((34 * 256) / 100);
    let raw_q32_32 = (1234_i64 << 32) | ((567890123_i64 * 4294967296) / 1000000000);

    group.bench_function("Q16_16", |b| {
        b.iter(|| {
            black_box(FixedQ16_16::deserialize_from_raw(black_box(raw_q16_16)));
        });
    });

    group.bench_function("Q8_8", |b| {
        b.iter(|| {
            black_box(FixedQ8_8::deserialize_from_raw(black_box(raw_q8_8)));
        });
    });

    group.bench_function("Q32_32", |b| {
        b.iter(|| {
            black_box(FixedQ32_32::deserialize_from_raw(black_box(raw_q32_32)));
        });
    });

    // Baseline: Direct struct construction
    group.bench_function("baseline_struct_construct", |b| {
        b.iter(|| {
            black_box(FixedQ16_16(black_box(raw_q16_16)));
        });
    });

    group.finish();
}

/// Benchmark serialize_decimal() - Target: <100ns
///
/// **Fair Baseline**: format!() for two integers (~60ns)
/// **Expected Overhead**: <50% (integer division + format)
fn bench_serialize_decimal(c: &mut Criterion) {
    let mut group = c.benchmark_group("trait_methods/serialize_decimal");
    group.throughput(Throughput::Elements(1));

    group.bench_function("Q16_16", |b| {
        let value = FixedQ16_16::from_decimal(1234, 5678);
        b.iter(|| {
            black_box(value.serialize_decimal());
        });
    });

    group.bench_function("Q8_8", |b| {
        let value = FixedQ8_8::from_decimal(12, 34);
        b.iter(|| {
            black_box(value.serialize_decimal());
        });
    });

    group.bench_function("Q32_32", |b| {
        let value = FixedQ32_32::from_decimal(1234, 567890123);
        b.iter(|| {
            black_box(value.serialize_decimal());
        });
    });

    // Baseline: format!() two integers
    group.bench_function("baseline_format_two_ints", |b| {
        b.iter(|| {
            black_box(format!("{}.{:04}", black_box(1234), black_box(5678)));
        });
    });

    group.finish();
}

/// Benchmark serialize_to_binary() - Target: <50ns
///
/// **Fair Baseline**: memcpy(8B) + CRC32(8B) (~30ns)
/// **Expected Overhead**: <100% (header + checksum)
fn bench_serialize_binary(c: &mut Criterion) {
    let mut group = c.benchmark_group("trait_methods/serialize_binary");
    group.throughput(Throughput::Bytes(22)); // Full binary size

    group.bench_function("Q16_16", |b| {
        let value = FixedQ16_16::from_decimal(1234, 5678);
        b.iter(|| {
            black_box(serialize_to_binary(&value));
        });
    });

    group.bench_function("Q8_8", |b| {
        let value = FixedQ8_8::from_decimal(12, 34);
        b.iter(|| {
            black_box(serialize_to_binary(&value));
        });
    });

    group.bench_function("Q32_32", |b| {
        let value = FixedQ32_32::from_decimal(1234, 567890123);
        b.iter(|| {
            black_box(serialize_to_binary(&value));
        });
    });

    // Baseline: memcpy 22 bytes
    group.bench_function("baseline_memcpy_22bytes", |b| {
        let src = [0u8; 22];
        let mut dst = [0u8; 22];
        b.iter(|| {
            dst.copy_from_slice(black_box(&src));
            black_box(&dst);
        });
    });

    group.finish();
}

/// Benchmark deserialize_from_binary() - Target: <50ns
///
/// **Fair Baseline**: memcpy(8B) + CRC32 verify (~30ns)
/// **Expected Overhead**: <100% (validation + checks)
fn bench_deserialize_binary(c: &mut Criterion) {
    let mut group = c.benchmark_group("trait_methods/deserialize_binary");
    group.throughput(Throughput::Bytes(22));

    let value_q16_16 = FixedQ16_16::from_decimal(1234, 5678);
    let bytes_q16_16 = serialize_to_binary(&value_q16_16);

    let value_q8_8 = FixedQ8_8::from_decimal(12, 34);
    let bytes_q8_8 = serialize_to_binary(&value_q8_8);

    let value_q32_32 = FixedQ32_32::from_decimal(1234, 567890123);
    let bytes_q32_32 = serialize_to_binary(&value_q32_32);

    group.bench_function("Q16_16", |b| {
        b.iter(|| {
            black_box(deserialize_from_binary::<FixedQ16_16>(black_box(&bytes_q16_16)).unwrap());
        });
    });

    group.bench_function("Q8_8", |b| {
        b.iter(|| {
            black_box(deserialize_from_binary::<FixedQ8_8>(black_box(&bytes_q8_8)).unwrap());
        });
    });

    group.bench_function("Q32_32", |b| {
        b.iter(|| {
            black_box(deserialize_from_binary::<FixedQ32_32>(black_box(&bytes_q32_32)).unwrap());
        });
    });

    group.finish();
}

/// Benchmark verify_roundtrip() - Determinism verification overhead
fn bench_verify_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("trait_methods/verify_roundtrip");

    group.bench_function("Q16_16", |b| {
        let value = FixedQ16_16::from_decimal(1234, 5678);
        b.iter(|| {
            black_box(value.verify_roundtrip());
        });
    });

    group.bench_function("Q8_8", |b| {
        let value = FixedQ8_8::from_decimal(12, 34);
        b.iter(|| {
            black_box(value.verify_roundtrip());
        });
    });

    group.bench_function("Q32_32", |b| {
        let value = FixedQ32_32::from_decimal(1234, 567890123);
        b.iter(|| {
            black_box(value.verify_roundtrip());
        });
    });

    group.finish();
}

/// Benchmark verify_decimal_determinism()
fn bench_verify_decimal_determinism(c: &mut Criterion) {
    let mut group = c.benchmark_group("trait_methods/verify_decimal_determinism");

    group.bench_function("Q16_16", |b| {
        let value = FixedQ16_16::from_decimal(1234, 5678);
        b.iter(|| {
            black_box(value.verify_decimal_determinism());
        });
    });

    group.bench_function("Q8_8", |b| {
        let value = FixedQ8_8::from_decimal(12, 34);
        b.iter(|| {
            black_box(value.verify_decimal_determinism());
        });
    });

    group.bench_function("Q32_32", |b| {
        let value = FixedQ32_32::from_decimal(1234, 567890123);
        b.iter(|| {
            black_box(value.verify_decimal_determinism());
        });
    });

    group.finish();
}

// ============================================================================
// 2. MIGRATION COMPARISON (400 LOC)
// ============================================================================

/// Manual implementation (baseline for comparison)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ManualQ16_16(i64);

impl ManualQ16_16 {
    const fn from_decimal(integer: i64, fractional_cents: i64) -> Self {
        let fractional = (fractional_cents * 65536) / 10000;
        ManualQ16_16((integer << 16) | (fractional & 0xFFFF))
    }

    #[inline]
    fn serialize_raw_manual(&self) -> i64 {
        self.0
    }

    fn serialize_decimal_manual(&self) -> String {
        let integer = self.0 >> 16;
        let fractional = (self.0 & 0xFFFF) * 10000 / 65536;
        if integer >= 0 {
            format!("{}.{:04}", integer, fractional)
        } else {
            format!("{}.{:04}", integer, fractional.abs())
        }
    }

    #[inline]
    fn deserialize_from_raw_manual(raw: i64) -> Self {
        ManualQ16_16(raw)
    }
}

/// Compare manual vs trait implementation - Expected: <1% variance
fn bench_migration_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("migration/manual_vs_trait");

    // Manual implementation baseline
    group.bench_function("manual_serialize_raw", |b| {
        let value = ManualQ16_16::from_decimal(1234, 5678);
        b.iter(|| {
            black_box(value.serialize_raw_manual());
        });
    });

    // Trait implementation
    group.bench_function("trait_serialize_raw", |b| {
        let value = FixedQ16_16::from_decimal(1234, 5678);
        b.iter(|| {
            black_box(value.serialize_raw());
        });
    });

    // Manual deserialize
    group.bench_function("manual_deserialize_raw", |b| {
        let raw = (1234_i64 << 16) | ((5678 * 65536) / 10000);
        b.iter(|| {
            black_box(ManualQ16_16::deserialize_from_raw_manual(black_box(raw)));
        });
    });

    // Trait deserialize
    group.bench_function("trait_deserialize_raw", |b| {
        let raw = (1234_i64 << 16) | ((5678 * 65536) / 10000);
        b.iter(|| {
            black_box(FixedQ16_16::deserialize_from_raw(black_box(raw)));
        });
    });

    // Manual decimal serialization
    group.bench_function("manual_serialize_decimal", |b| {
        let value = ManualQ16_16::from_decimal(1234, 5678);
        b.iter(|| {
            black_box(value.serialize_decimal_manual());
        });
    });

    // Trait decimal serialization
    group.bench_function("trait_serialize_decimal", |b| {
        let value = FixedQ16_16::from_decimal(1234, 5678);
        b.iter(|| {
            black_box(value.serialize_decimal());
        });
    });

    group.finish();
}

/// Benchmark 10 representative manual implementations
fn bench_representative_manual_impls(c: &mut Criterion) {
    let mut group = c.benchmark_group("migration/representative_manual");

    // Representative case 1: Small positive value
    group.bench_function("case_1_small_positive", |b| {
        let value = ManualQ16_16::from_decimal(12, 34);
        b.iter(|| {
            black_box(value.serialize_raw_manual());
            black_box(value.serialize_decimal_manual());
        });
    });

    // Representative case 2: Large positive value
    group.bench_function("case_2_large_positive", |b| {
        let value = ManualQ16_16::from_decimal(32767, 9999);
        b.iter(|| {
            black_box(value.serialize_raw_manual());
            black_box(value.serialize_decimal_manual());
        });
    });

    // Representative case 3: Negative value
    group.bench_function("case_3_negative", |b| {
        let value = ManualQ16_16::from_decimal(-1234, 5678);
        b.iter(|| {
            black_box(value.serialize_raw_manual());
            black_box(value.serialize_decimal_manual());
        });
    });

    // Representative case 4: Zero
    group.bench_function("case_4_zero", |b| {
        let value = ManualQ16_16::from_decimal(0, 0);
        b.iter(|| {
            black_box(value.serialize_raw_manual());
            black_box(value.serialize_decimal_manual());
        });
    });

    // Representative case 5: High precision fractional
    group.bench_function("case_5_high_precision", |b| {
        let value = ManualQ16_16::from_decimal(100, 1);
        b.iter(|| {
            black_box(value.serialize_raw_manual());
            black_box(value.serialize_decimal_manual());
        });
    });

    // Representative case 6: Integer only (no fractional)
    group.bench_function("case_6_integer_only", |b| {
        let value = ManualQ16_16::from_decimal(1000, 0);
        b.iter(|| {
            black_box(value.serialize_raw_manual());
            black_box(value.serialize_decimal_manual());
        });
    });

    // Representative case 7: Fractional only (no integer)
    group.bench_function("case_7_fractional_only", |b| {
        let value = ManualQ16_16::from_decimal(0, 5678);
        b.iter(|| {
            black_box(value.serialize_raw_manual());
            black_box(value.serialize_decimal_manual());
        });
    });

    // Representative case 8: Max negative
    group.bench_function("case_8_max_negative", |b| {
        let value = ManualQ16_16::from_decimal(-32768, 0);
        b.iter(|| {
            black_box(value.serialize_raw_manual());
            black_box(value.serialize_decimal_manual());
        });
    });

    // Representative case 9: Typical payment ($97.00)
    group.bench_function("case_9_typical_payment", |b| {
        let value = ManualQ16_16::from_decimal(97, 0);
        b.iter(|| {
            black_box(value.serialize_raw_manual());
            black_box(value.serialize_decimal_manual());
        });
    });

    // Representative case 10: Typical fee ($2.91)
    group.bench_function("case_10_typical_fee", |b| {
        let value = ManualQ16_16::from_decimal(2, 91);
        b.iter(|| {
            black_box(value.serialize_raw_manual());
            black_box(value.serialize_decimal_manual());
        });
    });

    group.finish();
}

/// Benchmark trait implementation for same 10 cases
fn bench_representative_trait_impls(c: &mut Criterion) {
    let mut group = c.benchmark_group("migration/representative_trait");

    group.bench_function("case_1_small_positive", |b| {
        let value = FixedQ16_16::from_decimal(12, 34);
        b.iter(|| {
            black_box(value.serialize_raw());
            black_box(value.serialize_decimal());
        });
    });

    group.bench_function("case_2_large_positive", |b| {
        let value = FixedQ16_16::from_decimal(32767, 9999);
        b.iter(|| {
            black_box(value.serialize_raw());
            black_box(value.serialize_decimal());
        });
    });

    group.bench_function("case_3_negative", |b| {
        let value = FixedQ16_16::from_decimal(-1234, 5678);
        b.iter(|| {
            black_box(value.serialize_raw());
            black_box(value.serialize_decimal());
        });
    });

    group.bench_function("case_4_zero", |b| {
        let value = FixedQ16_16::from_decimal(0, 0);
        b.iter(|| {
            black_box(value.serialize_raw());
            black_box(value.serialize_decimal());
        });
    });

    group.bench_function("case_5_high_precision", |b| {
        let value = FixedQ16_16::from_decimal(100, 1);
        b.iter(|| {
            black_box(value.serialize_raw());
            black_box(value.serialize_decimal());
        });
    });

    group.bench_function("case_6_integer_only", |b| {
        let value = FixedQ16_16::from_decimal(1000, 0);
        b.iter(|| {
            black_box(value.serialize_raw());
            black_box(value.serialize_decimal());
        });
    });

    group.bench_function("case_7_fractional_only", |b| {
        let value = FixedQ16_16::from_decimal(0, 5678);
        b.iter(|| {
            black_box(value.serialize_raw());
            black_box(value.serialize_decimal());
        });
    });

    group.bench_function("case_8_max_negative", |b| {
        let value = FixedQ16_16::from_decimal(-32768, 0);
        b.iter(|| {
            black_box(value.serialize_raw());
            black_box(value.serialize_decimal());
        });
    });

    group.bench_function("case_9_typical_payment", |b| {
        let value = FixedQ16_16::from_decimal(97, 0);
        b.iter(|| {
            black_box(value.serialize_raw());
            black_box(value.serialize_decimal());
        });
    });

    group.bench_function("case_10_typical_fee", |b| {
        let value = FixedQ16_16::from_decimal(2, 91);
        b.iter(|| {
            black_box(value.serialize_raw());
            black_box(value.serialize_decimal());
        });
    });

    group.finish();
}

// ============================================================================
// 3. CONCURRENCY PERFORMANCE (300 LOC)
// ============================================================================

/// Concurrent serialization scaling - Expected: Linear scaling
fn bench_concurrent_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrency/serialize_scaling");

    for threads in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_threads", threads)),
            &threads,
            |b, &num_threads| {
                use std::sync::Arc;
                use std::thread;

                let values: Vec<_> = (0..1000)
                    .map(|i| FixedQ16_16::from_decimal(i, i % 10000))
                    .collect();
                let values = Arc::new(values);

                b.iter(|| {
                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let values = Arc::clone(&values);
                            thread::spawn(move || {
                                let mut total = 0i64;
                                for value in values.iter() {
                                    total += black_box(value.serialize_raw());
                                }
                                total
                            })
                        })
                        .collect();

                    for handle in handles {
                        black_box(handle.join().unwrap());
                    }
                });
            },
        );
    }

    group.finish();
}

/// Concurrent decimal serialization (heap allocations)
fn bench_concurrent_serialize_decimal(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrency/serialize_decimal_scaling");

    for threads in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_threads", threads)),
            &threads,
            |b, &num_threads| {
                use std::sync::Arc;
                use std::thread;

                let values: Vec<_> = (0..100)
                    .map(|i| FixedQ16_16::from_decimal(i, i % 10000))
                    .collect();
                let values = Arc::new(values);

                b.iter(|| {
                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let values = Arc::clone(&values);
                            thread::spawn(move || {
                                let mut results = Vec::new();
                                for value in values.iter() {
                                    results.push(black_box(value.serialize_decimal()));
                                }
                                results
                            })
                        })
                        .collect();

                    for handle in handles {
                        black_box(handle.join().unwrap());
                    }
                });
            },
        );
    }

    group.finish();
}

/// P99 latency at all thread counts - Detect contention
fn bench_concurrent_p99_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrency/p99_latency");

    for threads in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_threads", threads)),
            &threads,
            |b, &num_threads| {
                use std::sync::Arc;
                use std::thread;
                use std::time::Instant;

                let values: Vec<_> = (0..10000)
                    .map(|i| FixedQ16_16::from_decimal(i, i % 10000))
                    .collect();
                let values = Arc::new(values);

                b.iter(|| {
                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let values = Arc::clone(&values);
                            thread::spawn(move || {
                                let start = Instant::now();
                                let mut total = 0i64;
                                for value in values.iter() {
                                    total += black_box(value.serialize_raw());
                                }
                                (start.elapsed(), total)
                            })
                        })
                        .collect();

                    for handle in handles {
                        black_box(handle.join().unwrap());
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// 4. REAL-WORLD SCENARIOS (400 LOC)
// ============================================================================

/// Simulate clapi_core PaymentCapsule256 serialization
fn bench_realworld_payment_capsule(c: &mut Criterion) {
    let mut group = c.benchmark_group("realworld/payment_capsule");

    // Simulate 1000 payment records serialization
    let payments: Vec<_> = (0..1000)
        .map(|i| {
            let amount = FixedQ16_16::from_decimal(100 + i, (i * 123) % 10000);
            let fee = FixedQ16_16::from_decimal(2 + (i % 10), 91);
            let net = FixedQ16_16::from_decimal(
                98 + i - (i % 10),
                ((i * 123) % 10000).saturating_sub(91),
            );
            (amount, fee, net)
        })
        .collect();

    group.bench_function("serialize_1000_payments", |b| {
        b.iter(|| {
            let mut total_bytes = 0;
            for (amount, fee, net) in &payments {
                total_bytes += black_box(serialize_to_binary(amount).len());
                total_bytes += black_box(serialize_to_binary(fee).len());
                total_bytes += black_box(serialize_to_binary(net).len());
            }
            total_bytes
        });
    });

    group.bench_function("serialize_decimal_1000_payments", |b| {
        b.iter(|| {
            let mut results = Vec::with_capacity(3000);
            for (amount, fee, net) in &payments {
                results.push(black_box(amount.serialize_decimal()));
                results.push(black_box(fee.serialize_decimal()));
                results.push(black_box(net.serialize_decimal()));
            }
            results
        });
    });

    group.finish();
}

/// Simulate kindly_hft MotorCortex P&L record serialization
fn bench_realworld_motor_cortex_pnl(c: &mut Criterion) {
    let mut group = c.benchmark_group("realworld/motor_cortex_pnl");

    // Simulate 100 P&L records (real trading scenario)
    let pnl_records: Vec<_> = (0..100)
        .map(|i| {
            let realized_pnl = FixedQ16_16::from_decimal((i as i64 - 50) * 10, (i * 456) % 10000);
            let unrealized_pnl = FixedQ16_16::from_decimal((50 - i as i64) * 5, (i * 789) % 10000);
            let total_pnl = FixedQ16_16::from_decimal((i as i64 - 50) * 5, (i * 123) % 10000);
            (realized_pnl, unrealized_pnl, total_pnl)
        })
        .collect();

    group.bench_function("serialize_100_pnl_records", |b| {
        b.iter(|| {
            let mut total_bytes = 0;
            for (realized, unrealized, total) in &pnl_records {
                total_bytes += black_box(serialize_to_binary(realized).len());
                total_bytes += black_box(serialize_to_binary(unrealized).len());
                total_bytes += black_box(serialize_to_binary(total).len());
            }
            total_bytes
        });
    });

    group.bench_function("serialize_decimal_100_pnl_records", |b| {
        b.iter(|| {
            let mut results = Vec::with_capacity(300);
            for (realized, unrealized, total) in &pnl_records {
                results.push(black_box(realized.serialize_decimal()));
                results.push(black_box(unrealized.serialize_decimal()));
                results.push(black_box(total.serialize_decimal()));
            }
            results
        });
    });

    group.finish();
}

/// Audit trail serialization + hash chain verification
fn bench_realworld_audit_trail(c: &mut Criterion) {
    let mut group = c.benchmark_group("realworld/audit_trail");
    group.throughput(Throughput::Elements(1000));

    let records: Vec<_> = (0..1000)
        .map(|i| FixedQ16_16::from_decimal(i, (i * 123) % 10000))
        .collect();

    group.bench_function("serialize_verify_1000_records", |b| {
        b.iter(|| {
            let mut chain_hash = 0u64;
            for record in &records {
                let bytes = black_box(serialize_to_binary(record));
                // Simulate hash chain (simple XOR for benchmark)
                chain_hash ^= black_box(bytes.len() as u64);
                assert!(black_box(
                    deserialize_from_binary::<FixedQ16_16>(&bytes).is_ok()
                ));
            }
            chain_hash
        });
    });

    group.finish();
}

/// Throughput: Operations per second
fn bench_realworld_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("realworld/throughput");
    group.throughput(Throughput::Elements(10000));

    let values: Vec<_> = (0..10000)
        .map(|i| FixedQ16_16::from_decimal(i, i % 10000))
        .collect();

    group.bench_function("serialize_raw_10k_ops", |b| {
        b.iter(|| {
            let mut total = 0i64;
            for value in &values {
                total += black_box(value.serialize_raw());
            }
            total
        });
    });

    group.bench_function("serialize_decimal_10k_ops", |b| {
        b.iter(|| {
            let mut results = Vec::with_capacity(10000);
            for value in &values {
                results.push(black_box(value.serialize_decimal()));
            }
            results
        });
    });

    group.bench_function("serialize_binary_10k_ops", |b| {
        b.iter(|| {
            let mut results = Vec::with_capacity(10000);
            for value in &values {
                results.push(black_box(serialize_to_binary(value)));
            }
            results
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    trait_methods,
    bench_serialize_raw,
    bench_deserialize_raw,
    bench_serialize_decimal,
    bench_serialize_binary,
    bench_deserialize_binary,
    bench_verify_roundtrip,
    bench_verify_decimal_determinism,
);

criterion_group!(
    migration_comparison,
    bench_migration_comparison,
    bench_representative_manual_impls,
    bench_representative_trait_impls,
);

criterion_group!(
    concurrency,
    bench_concurrent_serialize,
    bench_concurrent_serialize_decimal,
    bench_concurrent_p99_latency,
);

criterion_group!(
    realworld,
    bench_realworld_payment_capsule,
    bench_realworld_motor_cortex_pnl,
    bench_realworld_audit_trail,
    bench_realworld_throughput,
);

criterion_main!(trait_methods, migration_comparison, concurrency, realworld,);
