//! B32 Benchmarks for Syndrome Extraction Capsule
//!
//! **Framework**: B32 (honest benchmarking, fair baselines, 95% CI)
//! **Target**: 3-4× SIMD speedup vs scalar baseline
//! **Validation**: 1000+ iterations, distance-3/5/7 scaling

use atomic_capsule::quantum::syndrome::SyndromeExtractionCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use num_complex::Complex64;

/// Benchmark syndrome extraction (SIMD optimized)
fn bench_syndrome_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("syndrome_extraction");

    for &distance in &[3, 5, 7] {
        let capsule = SyndromeExtractionCapsule::new(distance);
        let num_qubits = distance * distance;
        let state = vec![Complex64::new(0.5, 0.5); 1 << num_qubits];

        group.bench_with_input(
            BenchmarkId::new("simd", distance),
            &distance,
            |b, _| {
                b.iter(|| {
                    black_box(capsule.extract_syndrome(black_box(&state)))
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("scalar", distance),
            &distance,
            |b, _| {
                b.iter(|| {
                    black_box(capsule.extract_syndrome_scalar(black_box(&state)))
                })
            },
        );
    }

    group.finish();
}

/// Benchmark Pauli evaluation (core operation)
fn bench_pauli_evaluation(c: &mut Criterion) {
    use atomic_capsule::quantum::syndrome::{PauliOp, PauliString};

    let mut group = c.benchmark_group("pauli_evaluation");

    // Distance-5 surface code (25 qubits)
    let num_qubits = 25;
    let state = vec![Complex64::new(0.5, 0.5); 1 << num_qubits];

    // Pure Z stabilizer (most common in surface codes)
    let z_pauli = PauliString::from_operators(vec![PauliOp::Z; num_qubits], 0);

    group.bench_function("pure_z_simd", |b| {
        b.iter(|| {
            black_box(atomic_capsule::quantum::syndrome::simd::evaluate_pauli_simd(
                black_box(&state),
                black_box(&z_pauli),
            ))
        })
    });

    // Pure X stabilizer
    let x_pauli = PauliString::from_operators(vec![PauliOp::X; num_qubits], 0);

    group.bench_function("pure_x_simd", |b| {
        b.iter(|| {
            black_box(atomic_capsule::quantum::syndrome::simd::evaluate_pauli_simd(
                black_box(&state),
                black_box(&x_pauli),
            ))
        })
    });

    group.finish();
}

/// Benchmark stabilizer generation (one-time setup cost)
fn bench_stabilizer_generation(c: &mut Criterion) {
    use atomic_capsule::quantum::syndrome::surface_code::{
        StabilizerGenerator, SurfaceCodeTopology,
    };

    let mut group = c.benchmark_group("stabilizer_generation");

    for &distance in &[3, 5, 7] {
        group.bench_with_input(
            BenchmarkId::new("planar", distance),
            &distance,
            |b, &d| {
                b.iter(|| {
                    black_box(StabilizerGenerator::new(d, SurfaceCodeTopology::Planar))
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("toric", distance),
            &distance,
            |b, &d| {
                b.iter(|| {
                    black_box(StabilizerGenerator::new(d, SurfaceCodeTopology::Toric))
                })
            },
        );
    }

    group.finish();
}

/// Benchmark distance scaling (3 → 5 → 7)
fn bench_distance_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("distance_scaling");

    for &distance in &[3, 5, 7] {
        let capsule = SyndromeExtractionCapsule::new(distance);
        let num_qubits = distance * distance;
        let state = vec![Complex64::new(0.5, 0.5); 1 << num_qubits];

        group.bench_with_input(
            BenchmarkId::new("full_extraction", distance),
            &distance,
            |b, _| {
                b.iter(|| {
                    black_box(capsule.extract_syndrome(black_box(&state)))
                })
            },
        );
    }

    group.finish();
}

/// Benchmark throughput (extractions per second)
fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");

    let capsule = SyndromeExtractionCapsule::new(5);
    let state = vec![Complex64::new(1.0, 0.0); 1 << 25];

    group.bench_function("distance_5_throughput", |b| {
        b.iter(|| {
            for _ in 0..100 {
                black_box(capsule.extract_syndrome(black_box(&state)));
            }
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_syndrome_extraction,
    bench_pauli_evaluation,
    bench_stabilizer_generation,
    bench_distance_scaling,
    bench_throughput
);
criterion_main!(benches);
