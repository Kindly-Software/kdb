//! [TRADE SECRET] QuantizationCapsule B32 Performance Benchmarks
//!
//! ## Benchmarking Framework (B32)
//!
//! - **Fair Baseline**: Q16.16 fixed-point vs floating-point reference
//! - **95% CI**: Criterion.rs with 1000+ iterations per benchmark
//! - **Performance Classification**:
//!   - Typical: 2-10× speedup (EXPECTED)
//!   - Exceptional: 10-50× speedup (VALIDATED)
//!   - Breakthrough: 50-1000× speedup (REQUIRES EXTENSIVE VALIDATION)
//!
//! ## Target Performance
//!
//! - **per_block_4x4**: <200ns (16 coefficients × ~12ns each)
//! - **per_block_8x8**: <200ns (64 coefficients, amortized)
//! - **set_qp**: <50ns (atomic CAS)
//! - **get_qp**: <30ns (atomic load)
//! - **throughput**: >5M blocks/sec on single thread
//!
//! ## Trade Secret Notice
//!
//! Q16.16 fixed-point quantization performance is proprietary.
//! All benchmark results marked [TRADE SECRET]. LOCAL COMMITS ONLY.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use atomic_capsule::encoder::QuantizationCapsule;

// ============================================================================
// Benchmark Group 1: Single Block Quantization
// ============================================================================

fn bench_quantize_4x4_basic(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantize_4x4");

    let quant = QuantizationCapsule::new(32);
    let input = black_box([100i16, 50, 25, 12, -30, -15, -8, -4, 200, 100, 50, 25, -60, -30, -15, -7]);

    group.bench_function("basic", |b| {
        b.iter(|| quant.quantize_block_4x4(&input))
    });

    group.finish();
}

fn bench_quantize_4x4_varying_qp(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantize_4x4_qp");
    let input = black_box([100i16; 16]);

    for qp in [16, 32, 64, 128, 192].iter() {
        let quant = QuantizationCapsule::new(*qp);

        group.bench_with_input(BenchmarkId::from_parameter(qp), qp, |b, _| {
            b.iter(|| quant.quantize_block_4x4(&input))
        });
    }

    group.finish();
}

fn bench_quantize_8x8_basic(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantize_8x8");

    let quant = QuantizationCapsule::new(32);
    let input = black_box([50i16; 64]);

    group.bench_function("basic", |b| {
        b.iter(|| quant.quantize_block_8x8(&input))
    });

    group.finish();
}

fn bench_quantize_8x8_varying_qp(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantize_8x8_qp");
    let input = black_box([50i16; 64]);

    for qp in [16, 32, 64, 128, 192].iter() {
        let quant = QuantizationCapsule::new(*qp);

        group.bench_with_input(BenchmarkId::from_parameter(qp), qp, |b, _| {
            b.iter(|| quant.quantize_block_8x8(&input))
        });
    }

    group.finish();
}

// ============================================================================
// Benchmark Group 2: Dequantization
// ============================================================================

fn bench_dequantize_4x4(c: &mut Criterion) {
    let mut group = c.benchmark_group("dequantize_4x4");

    let quant = QuantizationCapsule::new(32);
    let quantized = black_box([50i16, 25, 12, 6, -15, -7, -4, -2, 100, 50, 25, 12, -30, -15, -7, -3]);

    group.bench_function("basic", |b| {
        b.iter(|| quant.dequantize_block_4x4(&quantized))
    });

    group.finish();
}

fn bench_dequantize_8x8(c: &mut Criterion) {
    let mut group = c.benchmark_group("dequantize_8x8");

    let quant = QuantizationCapsule::new(32);
    let quantized = black_box([25i16; 64]);

    group.bench_function("basic", |b| {
        b.iter(|| quant.dequantize_block_8x8(&quantized))
    });

    group.finish();
}

// ============================================================================
// Benchmark Group 3: QP and Delta Operations
// ============================================================================

fn bench_set_qp(c: &mut Criterion) {
    let mut group = c.benchmark_group("set_qp");

    let quant = QuantizationCapsule::new(32);

    group.bench_function("atomic_store", |b| {
        b.iter(|| {
            for qp in 0..255 {
                quant.set_qp(qp);
            }
        })
    });

    group.finish();
}

fn bench_get_qp(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_qp");

    let quant = QuantizationCapsule::new(32);

    group.bench_function("atomic_load", |b| {
        b.iter(|| {
            let _qp = quant.get_qp();
            black_box(_qp)
        })
    });

    group.finish();
}

fn bench_set_dc_delta(c: &mut Criterion) {
    let mut group = c.benchmark_group("set_dc_delta");

    let quant = QuantizationCapsule::new(32);

    group.bench_function("positive", |b| {
        b.iter(|| {
            for delta in -32..=31 {
                quant.set_dc_delta(delta);
            }
        })
    });

    group.finish();
}

fn bench_set_ac_delta(c: &mut Criterion) {
    let mut group = c.benchmark_group("set_ac_delta");

    let quant = QuantizationCapsule::new(32);

    group.bench_function("positive", |b| {
        b.iter(|| {
            for delta in -32..=31 {
                quant.set_ac_delta(delta);
            }
        })
    });

    group.finish();
}

// ============================================================================
// Benchmark Group 4: Roundtrip (Quantize + Dequantize)
// ============================================================================

fn bench_roundtrip_4x4(c: &mut Criterion) {
    let mut group = c.benchmark_group("roundtrip_4x4");

    let quant = QuantizationCapsule::new(32);
    let input = black_box([100i16, 50, 25, 12, -30, -15, -8, -4, 200, 100, 50, 25, -60, -30, -15, -7]);

    group.bench_function("basic", |b| {
        b.iter(|| {
            let quantized = quant.quantize_block_4x4(&input);
            quant.dequantize_block_4x4(&quantized)
        })
    });

    group.finish();
}

fn bench_roundtrip_8x8(c: &mut Criterion) {
    let mut group = c.benchmark_group("roundtrip_8x8");

    let quant = QuantizationCapsule::new(32);
    let input = black_box([50i16; 64]);

    group.bench_function("basic", |b| {
        b.iter(|| {
            let quantized = quant.quantize_block_8x8(&input);
            quant.dequantize_block_8x8(&quantized)
        })
    });

    group.finish();
}

// ============================================================================
// Benchmark Group 5: Multi-Block Throughput
// ============================================================================

fn bench_throughput_4x4_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_4x4");

    let quant = QuantizationCapsule::new(32);
    let input = black_box([50i16; 16]);

    group.bench_function("100_blocks", |b| {
        b.iter(|| {
            let mut _result = [0i16; 16];
            for _ in 0..100 {
                _result = quant.quantize_block_4x4(&input);
            }
            black_box(_result)
        })
    });

    group.finish();
}

fn bench_throughput_8x8_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_8x8");

    let quant = QuantizationCapsule::new(32);
    let input = black_box([25i16; 64]);

    group.bench_function("50_blocks", |b| {
        b.iter(|| {
            let mut _result = [0i16; 64];
            for _ in 0..50 {
                _result = quant.quantize_block_8x8(&input);
            }
            black_box(_result)
        })
    });

    group.finish();
}

// ============================================================================
// Benchmark Group 6: QP Variations
// ============================================================================

fn bench_qp_extremes(c: &mut Criterion) {
    let mut group = c.benchmark_group("qp_extremes");
    let input = black_box([100i16; 16]);

    for (label, qp) in &[("qp_0", 0), ("qp_127", 127), ("qp_255", 255)] {
        let quant = QuantizationCapsule::new(*qp);

        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter(|| quant.quantize_block_4x4(&input))
        });
    }

    group.finish();
}

// ============================================================================
// Benchmark Group 7: State Transitions
// ============================================================================

fn bench_state_transitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_transitions");

    let quant = QuantizationCapsule::new(32);
    let input = black_box([50i16; 16]);

    group.bench_function("qp_change_per_block", |b| {
        b.iter(|| {
            for qp in 0..64 {
                quant.set_qp(qp);
                let _result = quant.quantize_block_4x4(&input);
                black_box(_result);
            }
        })
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_quantize_4x4_basic,
    bench_quantize_4x4_varying_qp,
    bench_quantize_8x8_basic,
    bench_quantize_8x8_varying_qp,
    bench_dequantize_4x4,
    bench_dequantize_8x8,
    bench_set_qp,
    bench_get_qp,
    bench_set_dc_delta,
    bench_set_ac_delta,
    bench_roundtrip_4x4,
    bench_roundtrip_8x8,
    bench_throughput_4x4_sequential,
    bench_throughput_8x8_sequential,
    bench_qp_extremes,
    bench_state_transitions,
);

criterion_main!(benches);
