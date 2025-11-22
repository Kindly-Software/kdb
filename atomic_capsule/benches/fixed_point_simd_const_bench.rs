//! Benchmark: FixedPointSIMDConst<const PRECISION, const LANES>
//!
//! Measures quantization and dequantization performance across precision and lane combinations.
//! Target: 5-10× speedup vs scalar baseline (EXCEPTIONAL tier).

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

#[cfg(all(feature = "nightly-const-mixed", feature = "portable_simd"))]
use atomic_capsule::composite::FixedPointSIMDConst;

#[cfg(all(feature = "nightly-const-mixed", feature = "portable_simd"))]
fn benchmark_quantize_q16_8lanes(c: &mut Criterion) {
    let capsule = FixedPointSIMDConst::<16, 8>::new();

    c.bench_function("quantize_q16_8lanes_1000", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let values = black_box([1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5, 8.5]);
                let _quantized = capsule.quantize_simd(&values);
            }
        })
    });
}

#[cfg(all(feature = "nightly-const-mixed", feature = "portable_simd"))]
fn benchmark_dequantize_q16_8lanes(c: &mut Criterion) {
    let capsule = FixedPointSIMDConst::<16, 8>::new();

    c.bench_function("dequantize_q16_8lanes_1000", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let quantized = black_box([49150i32, 81918, 114687, 147455, 180224, 212992, 245760, 278529]);
                let _dequantized = capsule.dequantize_simd(&quantized);
            }
        })
    });
}

#[cfg(all(feature = "nightly-const-mixed", feature = "portable_simd"))]
fn benchmark_quantize_q8_4lanes(c: &mut Criterion) {
    let capsule = FixedPointSIMDConst::<8, 4>::new();

    c.bench_function("quantize_q8_4lanes_1000", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let values = black_box([0.5, 1.0, 1.5, 2.0]);
                let _quantized = capsule.quantize_simd(&values);
            }
        })
    });
}

#[cfg(all(feature = "nightly-const-mixed", feature = "portable_simd"))]
fn benchmark_quantize_q32_16lanes(c: &mut Criterion) {
    let capsule = FixedPointSIMDConst::<32, 16>::new();

    c.bench_function("quantize_q32_16lanes_1000", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let values = black_box([
                    0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8,
                    0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6
                ]);
                let _quantized = capsule.quantize_simd(&values);
            }
        })
    });
}

#[cfg(all(feature = "nightly-const-mixed", feature = "portable_simd"))]
criterion_group!(
    benches,
    benchmark_quantize_q16_8lanes,
    benchmark_dequantize_q16_8lanes,
    benchmark_quantize_q8_4lanes,
    benchmark_quantize_q32_16lanes,
);

#[cfg(not(all(feature = "nightly-const-mixed", feature = "portable_simd")))]
fn no_op() {}

#[cfg(not(all(feature = "nightly-const-mixed", feature = "portable_simd")))]
criterion_group!(benches, no_op);

criterion_main!(benches);
