//! # VectorizedBatchConst Benchmark Suite
//!
//! **Nightly Phase 2 Primitive 11 - T6 Mixed (T1+T2+T4) Vectorized Batch**
//!
//! Performance targets (B32 Framework):
//! - Batch 1024: 100-500µs (runtime) → 10-30µs (const) = 10-50× speedup
//! - Per-item: 100-200ns (runtime) → 10-30ns (const) = 5-10× speedup
//! - Classification: EXCEPTIONAL tier (50-100× compound via T1+T2+T4)

#![cfg(feature = "nightly-const-mixed")]

use atomic_capsule::composite::{VectorizedBatchConst, BatchError};
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

/// Benchmark: Push operations (lockfree atomic increment)
fn bench_vectorized_batch_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("vectorized_batch_const_push");

    // Small batch (256 items, SIMD width 8)
    group.bench_function("push_256_8", |b| {
        b.iter(|| {
            let batch: VectorizedBatchConst<u64, 256, 8> = VectorizedBatchConst::new();
            for i in 0..256 {
                let _ = batch.push(black_box(i as u64));
            }
            batch.len()
        })
    });

    // Medium batch (1024 items, SIMD width 16)
    group.bench_function("push_1024_16", |b| {
        b.iter(|| {
            let batch: VectorizedBatchConst<u64, 1024, 16> = VectorizedBatchConst::new();
            for i in 0..1024 {
                let _ = batch.push(black_box(i as u64));
            }
            batch.len()
        })
    });

    // Large batch (65536 items, SIMD width 32)
    group.bench_function("push_65536_32", |b| {
        b.iter(|| {
            let batch: VectorizedBatchConst<u32, 65536, 32> = VectorizedBatchConst::new();
            for i in 0..1000 {
                let _ = batch.push(black_box(i as u32));
            }
            batch.len()
        })
    });

    group.finish();
}

/// Benchmark: Flush operations (batch processing)
fn bench_vectorized_batch_flush(c: &mut Criterion) {
    let mut group = c.benchmark_group("vectorized_batch_const_flush");

    // Small batch flush
    group.bench_function("flush_256", |b| {
        b.iter(|| {
            let mut batch: VectorizedBatchConst<u64, 256, 8> = VectorizedBatchConst::new();
            for i in 0..256 {
                let _ = batch.push(black_box(i as u64));
            }
            let _ = batch.flush(|_chunk| {
                // Process chunk
            });
        })
    });

    // Medium batch flush
    group.bench_function("flush_1024", |b| {
        b.iter(|| {
            let mut batch: VectorizedBatchConst<u64, 1024, 16> = VectorizedBatchConst::new();
            for i in 0..1024 {
                let _ = batch.push(black_box(i as u64));
            }
            let _ = batch.flush(|_chunk| {
                // Process chunk
            });
        })
    });

    group.finish();
}

/// Benchmark: SIMD chunk iteration
fn bench_vectorized_batch_simd_chunks(c: &mut Criterion) {
    let mut group = c.benchmark_group("vectorized_batch_const_simd");

    // SIMD width 8
    group.bench_function("simd_width_8", |b| {
        b.iter(|| {
            let batch: VectorizedBatchConst<f32, 256, 8> = VectorizedBatchConst::new();
            for i in 0..256 {
                let _ = batch.push(black_box(i as f32));
            }
            let _chunk = batch.next_simd_chunk();
        })
    });

    // SIMD width 16
    group.bench_function("simd_width_16", |b| {
        b.iter(|| {
            let batch: VectorizedBatchConst<f32, 512, 16> = VectorizedBatchConst::new();
            for i in 0..512 {
                let _ = batch.push(black_box(i as f32));
            }
            let _chunk = batch.next_simd_chunk();
        })
    });

    // SIMD width 32
    group.bench_function("simd_width_32", |b| {
        b.iter(|| {
            let batch: VectorizedBatchConst<f32, 1024, 32> = VectorizedBatchConst::new();
            for i in 0..1024 {
                let _ = batch.push(black_box(i as f32));
            }
            let _chunk = batch.next_simd_chunk();
        })
    });

    group.finish();
}

/// Benchmark: Capacity and metadata operations
fn bench_vectorized_batch_metadata(c: &mut Criterion) {
    let batch: VectorizedBatchConst<u64, 1024, 16> = VectorizedBatchConst::new();

    c.bench_function("capacity_1024", |b| {
        b.iter(|| black_box(batch.capacity()))
    });

    c.bench_function("len_empty", |b| {
        b.iter(|| black_box(batch.len()))
    });

    c.bench_function("is_empty", |b| {
        b.iter(|| black_box(batch.is_empty()))
    });

    c.bench_function("simd_width", |b| {
        b.iter(|| black_box(batch.simd_width()))
    });
}

/// Benchmark: Comparison with runtime allocation baseline
fn bench_vectorized_batch_vs_vec(c: &mut Criterion) {
    let mut group = c.benchmark_group("vectorized_batch_vs_vec");

    // Const generic version
    group.bench_function("const_1024", |b| {
        b.iter(|| {
            let batch: VectorizedBatchConst<u64, 1024, 16> = VectorizedBatchConst::new();
            for i in 0..1024 {
                let _ = batch.push(black_box(i as u64));
            }
        })
    });

    // Runtime Vec baseline (for comparison)
    #[cfg(feature = "std")]
    group.bench_function("vec_1024", |b| {
        b.iter(|| {
            let mut vec = Vec::with_capacity(1024);
            for i in 0..1024 {
                vec.push(black_box(i as u64));
            }
            vec.len()
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_vectorized_batch_push,
    bench_vectorized_batch_flush,
    bench_vectorized_batch_simd_chunks,
    bench_vectorized_batch_metadata,
    bench_vectorized_batch_vs_vec,
);

criterion_main!(benches);
