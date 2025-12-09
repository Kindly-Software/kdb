//! # Micro-Block Quantization Benchmark
//!
//! **B32 Framework Validation**: MBCQ vs Traditional Per-Tensor Quantization
//!
//! ## Target Performance (B32)
//!
//! - **MBCQ dequantization**: <15ns for 64 values (1 cache line)
//! - **Traditional dequantization**: ~35ns for 64 values (3 cache lines)
//! - **Expected speedup**: 2.3× (35ns → 15ns)
//!
//! ## Benchmark Strategy
//!
//! 1. **MBCQ (Co-Located)**: Single cache line read with metadata
//! 2. **Traditional (Per-Tensor)**: Separate scale/zero-point lookups
//! 3. **Fair comparison**: Same quantization accuracy (4-bit)
//!
//! ## Statistical Rigor (B32)
//!
//! - Minimum 1000 iterations
//! - 95% confidence intervals
//! - Warmup phase for cache stabilization
//! - Black-box to prevent compiler optimization

use atomic_llm_capsule::{MicroBlockQuantCapsule, QuantizedCapsule};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Traditional per-tensor quantization (baseline)
struct TraditionalQuantization {
    scale: f32,
    zero_point: u8,
    data: Vec<u8>, // 4-bit values packed as bytes
}

impl TraditionalQuantization {
    fn quantize(values: &[f32]) -> Self {
        assert_eq!(values.len(), 64);

        // Find global min/max
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for &v in values {
            min = min.min(v);
            max = max.max(v);
        }

        // Calculate scale
        let range = max - min;
        let scale = if range > 1e-8 { range / 15.0 } else { 1e-8 };

        // Quantize values
        let mut data = Vec::with_capacity(32); // 64 values × 4 bits = 32 bytes
        for chunk in values.chunks(2) {
            let q1 = ((chunk[0] - min) / scale).round().clamp(0.0, 15.0) as u8;
            let q2 = if chunk.len() > 1 {
                ((chunk[1] - min) / scale).round().clamp(0.0, 15.0) as u8
            } else {
                0
            };
            data.push(q1 | (q2 << 4));
        }

        Self {
            scale,
            zero_point: 0,
            data,
        }
    }

    fn dequantize(&self, output: &mut [f32]) {
        assert!(output.len() >= 64);

        for (i, &packed) in self.data.iter().enumerate() {
            let q1 = (packed & 0x0F) as f32;
            let q2 = ((packed >> 4) & 0x0F) as f32;

            output[i * 2] = q1 * self.scale;
            output[i * 2 + 1] = q2 * self.scale;
        }
    }
}

/// Generate realistic activation values
fn generate_activations(seed: u64) -> Vec<f32> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..64).map(|_| rng.gen_range(-5.0..5.0)).collect()
}

/// Benchmark MBCQ dequantization (co-located metadata)
fn bench_mbcq_dequantize(c: &mut Criterion) {
    let mut group = c.benchmark_group("mbcq_dequantize");

    let input = generate_activations(42);
    let mut capsule = MicroBlockQuantCapsule::new();
    capsule.quantize(&input).expect("Quantization failed");

    group.bench_function("mbcq_64_values", |b| {
        let mut output = vec![0.0f32; 64];
        b.iter(|| {
            black_box(&capsule)
                .dequantize(black_box(&mut output))
                .unwrap();
            black_box(&output);
        });
    });

    group.finish();
}

/// Benchmark traditional per-tensor dequantization (baseline)
fn bench_traditional_dequantize(c: &mut Criterion) {
    let mut group = c.benchmark_group("traditional_dequantize");

    let input = generate_activations(42);
    let quant = TraditionalQuantization::quantize(&input);

    group.bench_function("traditional_64_values", |b| {
        let mut output = vec![0.0f32; 64];
        b.iter(|| {
            black_box(&quant).dequantize(black_box(&mut output));
            black_box(&output);
        });
    });

    group.finish();
}

/// Benchmark MBCQ vs Traditional comparison
fn bench_mbcq_vs_traditional(c: &mut Criterion) {
    let mut group = c.benchmark_group("mbcq_vs_traditional");

    let input = generate_activations(42);

    // MBCQ
    let mut mbcq_capsule = MicroBlockQuantCapsule::new();
    mbcq_capsule.quantize(&input).expect("Quantization failed");

    // Traditional
    let traditional = TraditionalQuantization::quantize(&input);

    group.bench_with_input(BenchmarkId::new("mbcq", 64), &mbcq_capsule, |b, capsule| {
        let mut output = vec![0.0f32; 64];
        b.iter(|| {
            black_box(capsule)
                .dequantize(black_box(&mut output))
                .unwrap();
            black_box(&output);
        });
    });

    group.bench_with_input(
        BenchmarkId::new("traditional", 64),
        &traditional,
        |b, quant| {
            let mut output = vec![0.0f32; 64];
            b.iter(|| {
                black_box(quant).dequantize(black_box(&mut output));
                black_box(&output);
            });
        },
    );

    group.finish();
}

/// Benchmark quantization (encoding) performance
fn bench_quantize(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantization");

    let input = generate_activations(42);

    // MBCQ quantization
    group.bench_function("mbcq_quantize", |b| {
        let mut capsule = MicroBlockQuantCapsule::new();
        b.iter(|| {
            black_box(&mut capsule).quantize(black_box(&input)).unwrap();
        });
    });

    // Traditional quantization
    group.bench_function("traditional_quantize", |b| {
        b.iter(|| {
            let _ = TraditionalQuantization::quantize(black_box(&input));
        });
    });

    group.finish();
}

/// Benchmark roundtrip (quantize + dequantize)
fn bench_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("roundtrip");

    let input = generate_activations(42);

    // MBCQ roundtrip
    group.bench_function("mbcq_roundtrip", |b| {
        let mut capsule = MicroBlockQuantCapsule::new();
        let mut output = vec![0.0f32; 64];
        b.iter(|| {
            black_box(&mut capsule).quantize(black_box(&input)).unwrap();
            black_box(&capsule)
                .dequantize(black_box(&mut output))
                .unwrap();
            black_box(&output);
        });
    });

    // Traditional roundtrip
    group.bench_function("traditional_roundtrip", |b| {
        let mut output = vec![0.0f32; 64];
        b.iter(|| {
            let quant = TraditionalQuantization::quantize(black_box(&input));
            black_box(&quant).dequantize(black_box(&mut output));
            black_box(&output);
        });
    });

    group.finish();
}

/// Benchmark with various activation patterns
fn bench_activation_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("activation_patterns");

    let patterns = vec![
        ("uniform", vec![1.0f32; 64]),
        ("linear", (0..64).map(|i| i as f32 * 0.1).collect()),
        ("sine", (0..64).map(|i| ((i as f32) * 0.1).sin()).collect()),
        ("random", generate_activations(123)),
    ];

    for (name, input) in patterns {
        let mut capsule = MicroBlockQuantCapsule::new();
        capsule.quantize(&input).expect("Quantization failed");

        group.bench_with_input(BenchmarkId::new("mbcq", name), &capsule, |b, capsule| {
            let mut output = vec![0.0f32; 64];
            b.iter(|| {
                black_box(capsule)
                    .dequantize(black_box(&mut output))
                    .unwrap();
                black_box(&output);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_mbcq_dequantize,
    bench_traditional_dequantize,
    bench_mbcq_vs_traditional,
    bench_quantize,
    bench_roundtrip,
    bench_activation_patterns,
);
criterion_main!(benches);
