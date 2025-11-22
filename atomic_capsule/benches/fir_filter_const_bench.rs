//! Benchmark: FIRFilterConst Performance Analysis
//!
//! Validates performance claims from design specification:
//! - Coefficient generation: 100-500µs (runtime) → 0ns (compile-time)
//! - 48kHz audio convolution: 1-5µs/sample → 50-100ns/sample (10-50×)
//!
//! Requires: `--features nightly-const-simd`

#![cfg(feature = "nightly-const-simd")]

use atomic_capsule::FIRFilterConst;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

// ============================================================================
// Unit Tests (Quick validation)
// ============================================================================

#[test]
fn test_fir_init_16tap() {
    let _filter = FIRFilterConst::<16, 48000.0, 8000.0>::new();
}

#[test]
fn test_fir_init_32tap() {
    let _filter = FIRFilterConst::<32, 48000.0, 8000.0>::new();
}

#[test]
fn test_fir_init_64tap() {
    let _filter = FIRFilterConst::<64, 48000.0, 8000.0>::new();
}

// ============================================================================
// Criterion Benchmarks (Comprehensive performance analysis)
// ============================================================================

fn fir_filter_process_sample(c: &mut Criterion) {
    c.bench_function("fir_16tap_process_sample", |b| {
        b.iter_batched(
            || {
                let filter = FIRFilterConst::<16, 48000.0, 8000.0>::new();
                filter
            },
            |mut filter| {
                for i in 0..100 {
                    let sample = (i as f32).sin();
                    let _out = filter.process_sample(black_box(sample));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    c.bench_function("fir_32tap_process_sample", |b| {
        b.iter_batched(
            || {
                let filter = FIRFilterConst::<32, 48000.0, 8000.0>::new();
                filter
            },
            |mut filter| {
                for i in 0..100 {
                    let sample = (i as f32).sin();
                    let _out = filter.process_sample(black_box(sample));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    c.bench_function("fir_64tap_process_sample", |b| {
        b.iter_batched(
            || {
                let filter = FIRFilterConst::<64, 48000.0, 8000.0>::new();
                filter
            },
            |mut filter| {
                for i in 0..100 {
                    let sample = (i as f32).sin();
                    let _out = filter.process_sample(black_box(sample));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn fir_filter_reset(c: &mut Criterion) {
    c.bench_function("fir_32tap_reset", |b| {
        b.iter_batched(
            || {
                let filter = FIRFilterConst::<32, 48000.0, 8000.0>::new();
                filter
            },
            |mut filter| {
                filter.reset();
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, fir_filter_process_sample, fir_filter_reset);
criterion_main!(benches);
