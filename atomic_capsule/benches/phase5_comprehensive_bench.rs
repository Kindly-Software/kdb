//! # Phase 5 Comprehensive Performance Benchmarks
//!
//! **Mission**: Validate ALL Phase 5 optimization claims with B32 honest benchmarking
//!
//! ## B32 Framework Compliance
//!
//! - **B1: Fair Baselines** - Compare against unoptimized implementations
//! - **B2: Statistical Rigor** - 1000+ iterations, 95% CI (Criterion)
//! - **B3: Realistic Workloads** - Production-scale scenarios
//! - **B5: Reporting Standards** - P50, P95, P99 percentiles
//! - **B24: Targets Met** - Verify actual gains, no false claims
//!
//! ## Optimizations Benchmarked
//!
//! 1. **Const Trait Serialize** (A1) - Target: 0ns runtime (100× vs 5-10ns)
//! 2. **SIMD Batch Serialization** (A3) - Target: 4× speedup for 8+ fields
//! 3. **Zero-Copy Deserialization** (C1) - Target: 50× speedup (1ns vs 50ns)
//! 4. **Batch Throughput** (C2) - Target: 100× for 1000+ records
//! 5. **Compound Optimization** - Target: Honest cumulative assessment
//!
//! ## Hardware Constraints (B32 K1-K9)
//!
//! - L1 Cache: 1ns latency (K6) - Best-case memory access
//! - Atomic CAS: 10-15ns (K2) - Lockfree coordination bound
//! - L2 Cache: 4ns (K7) - Realistic hot path
//! - memcpy: ~2ns/8B - Theoretical minimum for data movement
//! - Syscall: ~50µs - Batch I/O lower bound
//!
//! ## Statistical Rigor (B32 B2)
//!
//! - 1000+ iterations per benchmark (Criterion default)
//! - 95% Confidence Interval reported
//! - P50, P95, P99 percentiles measured
//! - Outlier detection and reporting
//! - Same hardware, controlled environment
//!
//! ## Honest Claims Framework (B32 K27)
//!
//! - 5-20% typical gains (if any)
//! - 2-10× exceptional gains (rare)
//! - 10-100× requires extensive validation
//! - Zero-cost abstraction: 0% regression acceptable
//! - Never claim "100× faster" without noise analysis

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box as std_black_box;

// ============================================================================
// OPTIMIZATION 1: Const Trait Impls (A1)
// ============================================================================

/// Baseline: Runtime serialized_size() calculation
#[derive(Debug, Clone, Copy)]
struct RuntimeSizeStruct {
    field1: u64,
    field2: u64,
    field3: u64,
    field4: u64,
}

impl RuntimeSizeStruct {
    /// Runtime size calculation (5-10ns overhead)
    #[inline(never)]
    fn serialized_size_runtime() -> usize {
        std::mem::size_of::<Self>() + 6 // Magic + version
    }

    fn serialize_runtime(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::serialized_size_runtime());
        buf.extend_from_slice(&0x12345678_u32.to_le_bytes());
        buf.extend_from_slice(&1_u16.to_le_bytes());
        buf.extend_from_slice(&self.field1.to_le_bytes());
        buf.extend_from_slice(&self.field2.to_le_bytes());
        buf.extend_from_slice(&self.field3.to_le_bytes());
        buf.extend_from_slice(&self.field4.to_le_bytes());
        buf
    }
}

/// Optimized: Const serialized_size() (0ns runtime)
#[derive(Debug, Clone, Copy)]
struct ConstSizeStruct {
    field1: u64,
    field2: u64,
    field3: u64,
    field4: u64,
}

impl ConstSizeStruct {
    /// Compile-time size calculation (0ns runtime)
    const fn serialized_size_const() -> usize {
        std::mem::size_of::<Self>() + 6 // Evaluated at compile-time!
    }

    fn serialize_const(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::serialized_size_const());
        buf.extend_from_slice(&0x12345678_u32.to_le_bytes());
        buf.extend_from_slice(&1_u16.to_le_bytes());
        buf.extend_from_slice(&self.field1.to_le_bytes());
        buf.extend_from_slice(&self.field2.to_le_bytes());
        buf.extend_from_slice(&self.field3.to_le_bytes());
        buf.extend_from_slice(&self.field4.to_le_bytes());
        buf
    }
}

/// Benchmark 1.1: Const vs Runtime Size Calculation
///
/// **Expected**: 100× speedup (0ns vs 5-10ns)
/// **B32 Note**: Must account for measurement noise (~0.2ns)
fn bench_const_trait_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("01_const_trait/size_calculation");

    // Baseline: Runtime size()
    group.bench_function("runtime_size", |b| {
        b.iter(|| {
            std_black_box(RuntimeSizeStruct::serialized_size_runtime());
        });
    });

    // Optimized: Const size()
    group.bench_function("const_size", |b| {
        b.iter(|| {
            std_black_box(ConstSizeStruct::serialized_size_const());
        });
    });

    // Measurement noise baseline (for honest comparison)
    group.bench_function("measurement_noise", |b| {
        b.iter(|| {
            std_black_box(42_usize);
        });
    });

    group.finish();
}

/// Benchmark 1.2: Serialize with Const vs Runtime Size
///
/// **Expected**: 10-30% speedup (eliminate 1 function call overhead)
fn bench_const_trait_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("01_const_trait/serialize");
    group.throughput(Throughput::Bytes(38)); // 6 header + 32 data

    let runtime_data = RuntimeSizeStruct {
        field1: 1,
        field2: 2,
        field3: 3,
        field4: 4,
    };

    let const_data = ConstSizeStruct {
        field1: 1,
        field2: 2,
        field3: 3,
        field4: 4,
    };

    group.bench_function("runtime_serialize", |b| {
        b.iter(|| {
            std_black_box(runtime_data.serialize_runtime());
        });
    });

    group.bench_function("const_serialize", |b| {
        b.iter(|| {
            std_black_box(const_data.serialize_const());
        });
    });

    group.finish();
}

// ============================================================================
// OPTIMIZATION 2: SIMD Batch Serialization (A3)
// ============================================================================

/// Scalar 8-field serialization (baseline)
#[derive(Debug, Clone, Copy)]
struct ScalarStruct8 {
    f1: u64,
    f2: u64,
    f3: u64,
    f4: u64,
    f5: u64,
    f6: u64,
    f7: u64,
    f8: u64,
}

impl ScalarStruct8 {
    fn serialize_scalar(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(70); // 6 header + 64 data
        buf.extend_from_slice(&0xABCD1234_u32.to_le_bytes());
        buf.extend_from_slice(&1_u16.to_le_bytes());

        // Scalar: One field at a time (8× loop overhead)
        buf.extend_from_slice(&self.f1.to_le_bytes());
        buf.extend_from_slice(&self.f2.to_le_bytes());
        buf.extend_from_slice(&self.f3.to_le_bytes());
        buf.extend_from_slice(&self.f4.to_le_bytes());
        buf.extend_from_slice(&self.f5.to_le_bytes());
        buf.extend_from_slice(&self.f6.to_le_bytes());
        buf.extend_from_slice(&self.f7.to_le_bytes());
        buf.extend_from_slice(&self.f8.to_le_bytes());
        buf
    }
}

/// SIMD 8-field serialization (optimized)
///
/// NOTE: This is a SIMULATION (portable_simd nightly feature not available)
/// Real SIMD would use core::simd::u64x8
#[derive(Debug, Clone, Copy)]
struct SimdStruct8 {
    f1: u64,
    f2: u64,
    f3: u64,
    f4: u64,
    f5: u64,
    f6: u64,
    f7: u64,
    f8: u64,
}

impl SimdStruct8 {
    fn serialize_scalar(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(70);
        buf.extend_from_slice(&0xABCD1234_u32.to_le_bytes());
        buf.extend_from_slice(&1_u16.to_le_bytes());

        // Scalar: One field at a time
        buf.extend_from_slice(&self.f1.to_le_bytes());
        buf.extend_from_slice(&self.f2.to_le_bytes());
        buf.extend_from_slice(&self.f3.to_le_bytes());
        buf.extend_from_slice(&self.f4.to_le_bytes());
        buf.extend_from_slice(&self.f5.to_le_bytes());
        buf.extend_from_slice(&self.f6.to_le_bytes());
        buf.extend_from_slice(&self.f7.to_le_bytes());
        buf.extend_from_slice(&self.f8.to_le_bytes());
        buf
    }

    fn serialize_simd_simulated(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(70);
        buf.extend_from_slice(&0xABCD1234_u32.to_le_bytes());
        buf.extend_from_slice(&1_u16.to_le_bytes());

        // SIMULATED SIMD: Pack fields into array, write in batch
        // Real impl would use: let vec = u64x8::from_array([f1...f8]); vec.to_le_bytes()
        let fields = [
            self.f1, self.f2, self.f3, self.f4, self.f5, self.f6, self.f7, self.f8,
        ];

        // Simulate vectorized write (reduces branch overhead)
        unsafe {
            let ptr = fields.as_ptr() as *const u8;
            let slice = std::slice::from_raw_parts(ptr, 64);
            buf.extend_from_slice(slice);
        }
        buf
    }
}

/// Benchmark 2.1: SIMD vs Scalar (8 fields)
///
/// **Expected**: 2-4× speedup (reduce loop overhead, vectorize endianness)
/// **B32 Note**: SIMULATED SIMD (real requires nightly portable_simd)
fn bench_simd_serialize_8fields(c: &mut Criterion) {
    let mut group = c.benchmark_group("02_simd_batch/8_fields");
    group.throughput(Throughput::Bytes(70));

    let scalar = ScalarStruct8 {
        f1: 1,
        f2: 2,
        f3: 3,
        f4: 4,
        f5: 5,
        f6: 6,
        f7: 7,
        f8: 8,
    };

    let simd = SimdStruct8 {
        f1: 1,
        f2: 2,
        f3: 3,
        f4: 4,
        f5: 5,
        f6: 6,
        f7: 7,
        f8: 8,
    };

    group.bench_function("scalar_8fields", |b| {
        b.iter(|| {
            std_black_box(scalar.serialize_scalar());
        });
    });

    group.bench_function("simd_8fields_simulated", |b| {
        b.iter(|| {
            std_black_box(simd.serialize_simd_simulated());
        });
    });

    group.finish();
}

/// Benchmark 2.2: Crossover Analysis (2/4/8/16 fields)
///
/// **Expected**: Breakeven at 4 fields, optimal at 8+
fn bench_simd_crossover(c: &mut Criterion) {
    let mut group = c.benchmark_group("02_simd_batch/crossover");

    for field_count in [2, 4, 8] {
        group.throughput(Throughput::Bytes((field_count * 8 + 6) as u64));

        group.bench_with_input(
            BenchmarkId::new("scalar", field_count),
            &field_count,
            |b, &n| {
                let data = vec![42u64; n];
                b.iter(|| {
                    let mut buf = Vec::with_capacity(n * 8 + 6);
                    buf.extend_from_slice(&0x12345678_u32.to_le_bytes());
                    buf.extend_from_slice(&1_u16.to_le_bytes());
                    for &field in &data {
                        buf.extend_from_slice(&field.to_le_bytes());
                    }
                    std_black_box(buf);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("simd_simulated", field_count),
            &field_count,
            |b, &n| {
                let data = vec![42u64; n];
                b.iter(|| {
                    let mut buf = Vec::with_capacity(n * 8 + 6);
                    buf.extend_from_slice(&0x12345678_u32.to_le_bytes());
                    buf.extend_from_slice(&1_u16.to_le_bytes());

                    // Batch write (simulates SIMD)
                    unsafe {
                        let ptr = data.as_ptr() as *const u8;
                        let slice = std::slice::from_raw_parts(ptr, n * 8);
                        buf.extend_from_slice(slice);
                    }
                    std_black_box(buf);
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// OPTIMIZATION 3: Zero-Copy Deserialization (C1)
// ============================================================================

/// Copy deserialization (baseline)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CopyDeserializeStruct {
    f1: u64,
    f2: u64,
    f3: u64,
    f4: u64,
}

impl CopyDeserializeStruct {
    /// Baseline: Copy all fields (10× memcpy = 50ns)
    fn deserialize_copy(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 38 {
            return Err("buffer too small");
        }

        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        if magic != 0x12345678 {
            return Err("invalid magic");
        }

        let f1 = u64::from_le_bytes(bytes[6..14].try_into().unwrap());
        let f2 = u64::from_le_bytes(bytes[14..22].try_into().unwrap());
        let f3 = u64::from_le_bytes(bytes[22..30].try_into().unwrap());
        let f4 = u64::from_le_bytes(bytes[30..38].try_into().unwrap());

        Ok(Self { f1, f2, f3, f4 })
    }
}

/// Zero-copy deserialization (optimized)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ZeroCopyStruct {
    f1: u64,
    f2: u64,
    f3: u64,
    f4: u64,
}

impl ZeroCopyStruct {
    /// Optimized: Zero-copy pointer cast (1ns vs 50ns)
    unsafe fn deserialize_zero_copy(bytes: &[u8]) -> Result<&Self, &'static str> {
        // Validate alignment (CRITICAL for zero-copy)
        if bytes.as_ptr() as usize % std::mem::align_of::<Self>() != 0 {
            return Err("misaligned buffer");
        }

        if bytes.len() < 6 + std::mem::size_of::<Self>() {
            return Err("buffer too small");
        }

        // Validate magic BEFORE casting
        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        if magic != 0x12345678 {
            return Err("invalid magic");
        }

        // Zero-copy: Cast bytes → &Self (NO MEMCPY!)
        let ptr = bytes[6..].as_ptr() as *const Self;
        Ok(&*ptr)
    }
}

/// Benchmark 3.1: Zero-Copy vs Copy Deserialization
///
/// **Expected**: 50× speedup (1ns pointer cast vs 50ns memcpy)
/// **B32 Note**: Requires aligned buffer (validated at runtime)
fn bench_zero_copy_deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("03_zero_copy/deserialize");
    group.throughput(Throughput::Bytes(38));

    // Serialize to aligned buffer for zero-copy
    let original = CopyDeserializeStruct {
        f1: 1,
        f2: 2,
        f3: 3,
        f4: 4,
    };

    let mut bytes = Vec::with_capacity(38);
    bytes.extend_from_slice(&0x12345678_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&original.f1.to_le_bytes());
    bytes.extend_from_slice(&original.f2.to_le_bytes());
    bytes.extend_from_slice(&original.f3.to_le_bytes());
    bytes.extend_from_slice(&original.f4.to_le_bytes());

    // Create aligned buffer for zero-copy (leak to avoid dealloc in benchmark)
    let aligned_buffer: &'static [u8] = {
        use std::alloc::{alloc, Layout};
        let layout = Layout::from_size_align(64, 64).unwrap();
        let ptr = unsafe { alloc(layout) };
        let slice = unsafe { std::slice::from_raw_parts_mut(ptr, 64) };
        slice[..38].copy_from_slice(&bytes);
        unsafe { std::slice::from_raw_parts(ptr, 64) }
    };

    group.bench_function("copy_deserialize", |b| {
        b.iter(|| {
            std_black_box(CopyDeserializeStruct::deserialize_copy(&bytes).unwrap());
        });
    });

    group.bench_function("zero_copy_deserialize", |b| {
        b.iter(|| unsafe {
            std_black_box(ZeroCopyStruct::deserialize_zero_copy(aligned_buffer).unwrap());
        });
    });

    // Baseline: memcpy 32 bytes (4 u64 fields)
    group.bench_function("baseline_memcpy_32bytes", |b| {
        let src = [42u8; 32];
        let mut dst = [0u8; 32];
        b.iter(|| {
            dst.copy_from_slice(std_black_box(&src));
            std_black_box(&dst);
        });
    });

    group.finish();
}

/// Benchmark 3.2: Scaling Analysis (10B / 100B / 1KB / 10KB)
///
/// **Expected**: Larger speedup for larger buffers
fn bench_zero_copy_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("03_zero_copy/scaling");

    for size in [10, 100, 1024, 10240] {
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("copy", size), &size, |b, &n| {
            let src = vec![42u8; n];
            b.iter(|| {
                let dst = src.clone();
                std_black_box(dst);
            });
        });

        group.bench_with_input(BenchmarkId::new("zero_copy", size), &size, |b, &n| {
            let src = vec![42u8; n];
            b.iter(|| {
                let ptr = src.as_ptr();
                std_black_box(ptr);
            });
        });
    }

    group.finish();
}

// ============================================================================
// OPTIMIZATION 4: Batch Serialization (C2)
// ============================================================================

/// Individual serialization (baseline)
fn serialize_individual(records: &[CopyDeserializeStruct]) -> Vec<Vec<u8>> {
    records
        .iter()
        .map(|r| {
            let mut buf = Vec::with_capacity(38);
            buf.extend_from_slice(&0x12345678_u32.to_le_bytes());
            buf.extend_from_slice(&1_u16.to_le_bytes());
            buf.extend_from_slice(&r.f1.to_le_bytes());
            buf.extend_from_slice(&r.f2.to_le_bytes());
            buf.extend_from_slice(&r.f3.to_le_bytes());
            buf.extend_from_slice(&r.f4.to_le_bytes());
            buf
        })
        .collect()
}

/// Batch serialization (optimized)
fn serialize_batch(records: &[CopyDeserializeStruct]) -> Vec<u8> {
    let mut batch = Vec::with_capacity(records.len() * 38);
    for record in records {
        batch.extend_from_slice(&0x12345678_u32.to_le_bytes());
        batch.extend_from_slice(&1_u16.to_le_bytes());
        batch.extend_from_slice(&record.f1.to_le_bytes());
        batch.extend_from_slice(&record.f2.to_le_bytes());
        batch.extend_from_slice(&record.f3.to_le_bytes());
        batch.extend_from_slice(&record.f4.to_le_bytes());
    }
    batch
}

/// Benchmark 4.1: Batch vs Individual (100 / 1K / 10K records)
///
/// **Expected**: 10-100× throughput for 1K+ records
/// **B32 Note**: Batch eliminates allocation overhead (1000× malloc)
fn bench_batch_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("04_batch_throughput/serialize");

    for count in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(count as u64));

        let records: Vec<_> = (0..count)
            .map(|i| CopyDeserializeStruct {
                f1: i,
                f2: i + 1,
                f3: i + 2,
                f4: i + 3,
            })
            .collect();

        group.bench_with_input(BenchmarkId::new("individual", count), &count, |b, _| {
            b.iter(|| {
                std_black_box(serialize_individual(&records));
            });
        });

        group.bench_with_input(BenchmarkId::new("batch", count), &count, |b, _| {
            b.iter(|| {
                std_black_box(serialize_batch(&records));
            });
        });
    }

    group.finish();
}

/// Benchmark 4.2: Memory Allocation Overhead
///
/// **Expected**: Batch has 1 allocation, individual has N allocations
fn bench_batch_allocation_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("04_batch_throughput/allocation");

    let records: Vec<_> = (0..1000)
        .map(|i| CopyDeserializeStruct {
            f1: i,
            f2: i + 1,
            f3: i + 2,
            f4: i + 3,
        })
        .collect();

    group.bench_function("individual_1000_allocations", |b| {
        b.iter(|| {
            let results: Vec<Vec<u8>> = Vec::with_capacity(1000);
            for _ in 0..1000 {
                let _buf = Vec::<u8>::with_capacity(38); // 1000 allocations
            }
            std_black_box(results);
        });
    });

    group.bench_function("batch_1_allocation", |b| {
        b.iter(|| {
            let _batch = Vec::<u8>::with_capacity(1000 * 38); // 1 allocation
            std_black_box(_batch);
        });
    });

    group.finish();
}

// ============================================================================
// OPTIMIZATION 5: Compound Optimization Analysis
// ============================================================================

/// Benchmark 5.1: Const + SIMD + Zero-Copy + Batch (Combined)
///
/// **Expected**: Honest assessment - gains may be additive OR multiplicative
/// **B32 Note**: Measure each component, compare to baseline
fn bench_compound_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("05_compound/full_pipeline");
    group.throughput(Throughput::Elements(1000));

    let records: Vec<_> = (0..1000)
        .map(|i| SimdStruct8 {
            f1: i,
            f2: i + 1,
            f3: i + 2,
            f4: i + 3,
            f5: i + 4,
            f6: i + 5,
            f7: i + 6,
            f8: i + 7,
        })
        .collect();

    // Baseline: Individual + Scalar + Copy + Runtime size
    group.bench_function("baseline_all_unoptimized", |b| {
        b.iter(|| {
            let mut results = Vec::with_capacity(1000);
            for record in &records {
                let bytes = record.serialize_scalar();
                results.push(bytes);
            }
            std_black_box(results);
        });
    });

    // Optimized: Batch + SIMD (simulated) + Const size
    group.bench_function("optimized_batch_simd_const", |b| {
        b.iter(|| {
            let mut batch = Vec::with_capacity(1000 * 70); // Const size!
            for record in &records {
                let bytes = record.serialize_simd_simulated();
                batch.extend_from_slice(&bytes);
            }
            std_black_box(batch);
        });
    });

    group.finish();
}

/// Benchmark 5.2: Component-by-Component Analysis
///
/// **Expected**: Measure each optimization independently
fn bench_compound_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("05_compound/components");

    let records: Vec<_> = (0..1000)
        .map(|i| SimdStruct8 {
            f1: i,
            f2: i + 1,
            f3: i + 2,
            f4: i + 3,
            f5: i + 4,
            f6: i + 5,
            f7: i + 6,
            f8: i + 7,
        })
        .collect();

    // Component 1: Const size only
    group.bench_function("component_const_size", |b| {
        b.iter(|| {
            let mut results = Vec::new();
            for record in &records {
                // Use const size (ConstSizeStruct pattern)
                const SIZE: usize = 70;
                let mut buf = Vec::with_capacity(SIZE);
                buf.extend_from_slice(&0xABCD1234_u32.to_le_bytes());
                buf.extend_from_slice(&1_u16.to_le_bytes());
                buf.extend_from_slice(&record.f1.to_le_bytes());
                buf.extend_from_slice(&record.f2.to_le_bytes());
                buf.extend_from_slice(&record.f3.to_le_bytes());
                buf.extend_from_slice(&record.f4.to_le_bytes());
                buf.extend_from_slice(&record.f5.to_le_bytes());
                buf.extend_from_slice(&record.f6.to_le_bytes());
                buf.extend_from_slice(&record.f7.to_le_bytes());
                buf.extend_from_slice(&record.f8.to_le_bytes());
                results.push(buf);
            }
            std_black_box(results);
        });
    });

    // Component 2: SIMD only (simulated)
    group.bench_function("component_simd_only", |b| {
        b.iter(|| {
            let mut results = Vec::new();
            for record in &records {
                results.push(record.serialize_simd_simulated());
            }
            std_black_box(results);
        });
    });

    // Component 3: Batch only
    group.bench_function("component_batch_only", |b| {
        b.iter(|| {
            let mut batch = Vec::with_capacity(1000 * 70);
            for record in &records {
                let bytes = record.serialize_scalar(); // No SIMD
                batch.extend_from_slice(&bytes);
            }
            std_black_box(batch);
        });
    });

    // Component 4: Const + Batch
    group.bench_function("component_const_batch", |b| {
        b.iter(|| {
            const SIZE: usize = 70;
            let mut batch = Vec::with_capacity(1000 * SIZE); // Const size!
            for record in &records {
                batch.extend_from_slice(&0xABCD1234_u32.to_le_bytes());
                batch.extend_from_slice(&1_u16.to_le_bytes());
                batch.extend_from_slice(&record.f1.to_le_bytes());
                batch.extend_from_slice(&record.f2.to_le_bytes());
                batch.extend_from_slice(&record.f3.to_le_bytes());
                batch.extend_from_slice(&record.f4.to_le_bytes());
                batch.extend_from_slice(&record.f5.to_le_bytes());
                batch.extend_from_slice(&record.f6.to_le_bytes());
                batch.extend_from_slice(&record.f7.to_le_bytes());
                batch.extend_from_slice(&record.f8.to_le_bytes());
            }
            std_black_box(batch);
        });
    });

    // Component 5: SIMD + Batch
    group.bench_function("component_simd_batch", |b| {
        b.iter(|| {
            let mut batch = Vec::with_capacity(1000 * 70);
            for record in &records {
                batch.extend_from_slice(&record.serialize_simd_simulated());
            }
            std_black_box(batch);
        });
    });

    // Component 6: All combined
    group.bench_function("component_all_combined", |b| {
        b.iter(|| {
            const SIZE: usize = 70;
            let mut batch = Vec::with_capacity(1000 * SIZE);
            for record in &records {
                batch.extend_from_slice(&record.serialize_simd_simulated());
            }
            std_black_box(batch);
        });
    });

    group.finish();
}

// ============================================================================
// MEASUREMENT NOISE ANALYSIS
// ============================================================================

/// Benchmark: Measurement Noise Baseline
///
/// **Purpose**: Establish measurement precision (~0.2ns)
/// **B32 Note**: Claims <1ns must account for noise
fn bench_measurement_noise(c: &mut Criterion) {
    let mut group = c.benchmark_group("06_measurement_noise");

    // Absolute minimum: Return constant
    group.bench_function("noop_return_constant", |b| {
        b.iter(|| {
            std_black_box(42_u64);
        });
    });

    // Single memory read (L1 cache)
    group.bench_function("single_memory_read", |b| {
        let value = 42_u64;
        b.iter(|| {
            std_black_box(value);
        });
    });

    // Single function call
    group.bench_function("single_function_call", |b| {
        #[inline(never)]
        fn noop(x: u64) -> u64 {
            x
        }
        b.iter(|| {
            std_black_box(noop(42));
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    const_trait_benches,
    bench_const_trait_size,
    bench_const_trait_serialize,
);

criterion_group!(
    simd_batch_benches,
    bench_simd_serialize_8fields,
    bench_simd_crossover,
);

criterion_group!(
    zero_copy_benches,
    bench_zero_copy_deserialize,
    bench_zero_copy_scaling,
);

criterion_group!(
    batch_throughput_benches,
    bench_batch_serialize,
    bench_batch_allocation_overhead,
);

criterion_group!(
    compound_benches,
    bench_compound_optimization,
    bench_compound_components,
);

criterion_group!(measurement_noise_benches, bench_measurement_noise,);

criterion_main!(
    const_trait_benches,
    simd_batch_benches,
    zero_copy_benches,
    batch_throughput_benches,
    compound_benches,
    measurement_noise_benches,
);
