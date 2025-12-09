//! B32 Benchmarks for CZGateCapsule
//!
//! # B32 Framework: Fair Baseline Benchmarking
//!
//! ## Fair Baseline Principle
//!
//! - **Baseline**: Scalar CZ gate (conditional negation, no SIMD)
//! - **Optimized**: AVX2 f64x4 vectorization (4 complex amplitudes per iteration)
//! - **Hardware**: Same CPU, same compiler, same optimization flags
//! - **Validation**: 95% CI, 1000+ iterations per measurement
//!
//! ## Performance Reality Check
//!
//! - **10-50% typical**: Most optimizations (cache, memory ordering)
//! - **2-10× exceptional**: SIMD, batching, lockfree coordination (our target)
//! - **100×+ extensive**: Requires algorithm change + validation
//!
//! ## Expected Speedup: 3-4× (EXCEPTIONAL Tier)
//!
//! - **Rationale**: Diagonal gate (simpler than CNOT) + AVX2 vectorization
//! - **Baseline**: Scalar conditional negation
//! - **SIMD**: Process 4 amplitudes per iteration (f64x4)
//! - **Target**: 3-4× speedup (validated via Criterion 1000+ iterations)
//!
//! # Benchmark Groups
//!
//! 1. **Scalar Baseline**: 8, 12, 16, 20 qubits (fair comparison)
//! 2. **AVX2 SIMD**: 8, 12, 16, 20 qubits (optimized path)
//! 3. **Speedup Validation**: Direct comparison (scalar vs AVX2)
//! 4. **Production Workload**: Realistic multi-gate circuits

use atomic_capsule::quantum_pure::cz_gate::CZGateCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

/// Helper: Create equal superposition state
fn create_equal_superposition(num_amplitudes: usize) -> (Vec<f64>, Vec<f64>) {
    let amplitude = 1.0 / (num_amplitudes as f64).sqrt();
    let real = vec![amplitude; num_amplitudes];
    let imag = vec![0.0; num_amplitudes];
    (real, imag)
}

// ============================================================================
// GROUP 1: SCALAR BASELINE (Fair Comparison)
// ============================================================================

fn bench_cz_scalar_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("cz_scalar_baseline");

    for num_qubits in [8, 12, 16, 20] {
        let num_amplitudes = 1 << num_qubits;
        let gate = CZGateCapsule::new(0, 1).unwrap();
        let (mut real, mut imag) = create_equal_superposition(num_amplitudes);

        group.throughput(Throughput::Elements(num_amplitudes as u64));

        group.bench_with_input(
            BenchmarkId::new("scalar", num_qubits),
            &num_qubits,
            |b, &nq| {
                b.iter(|| {
                    gate.apply(black_box(&mut real), black_box(&mut imag), black_box(nq))
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// GROUP 2: AVX2 SIMD OPTIMIZED
// ============================================================================

#[cfg(feature = "portable_simd")]
fn bench_cz_avx2_optimized(c: &mut Criterion) {
    let mut group = c.benchmark_group("cz_avx2_optimized");

    for num_qubits in [8, 12, 16, 20] {
        let num_amplitudes = 1 << num_qubits;
        let gate = CZGateCapsule::new(0, 1).unwrap();
        let (mut real, mut imag) = create_equal_superposition(num_amplitudes);

        group.throughput(Throughput::Elements(num_amplitudes as u64));

        group.bench_with_input(
            BenchmarkId::new("avx2", num_qubits),
            &num_qubits,
            |b, &nq| {
                b.iter(|| {
                    gate.apply_avx2(black_box(&mut real), black_box(&mut imag), black_box(nq))
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// GROUP 3: SPEEDUP VALIDATION (Scalar vs AVX2)
// ============================================================================

#[cfg(feature = "portable_simd")]
fn bench_cz_speedup_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("cz_speedup_validation");

    for num_qubits in [8, 12, 16, 20] {
        let num_amplitudes = 1 << num_qubits;
        let gate = CZGateCapsule::new(5, 10).unwrap(); // Mid-range qubits

        // Scalar baseline
        {
            let (mut real, mut imag) = create_equal_superposition(num_amplitudes);
            group.throughput(Throughput::Elements(num_amplitudes as u64));

            group.bench_with_input(
                BenchmarkId::new("scalar_baseline", num_qubits),
                &num_qubits,
                |b, &nq| {
                    b.iter(|| {
                        gate.apply(black_box(&mut real), black_box(&mut imag), black_box(nq))
                            .unwrap();
                    });
                },
            );
        }

        // AVX2 optimized
        {
            let (mut real, mut imag) = create_equal_superposition(num_amplitudes);

            group.bench_with_input(
                BenchmarkId::new("avx2_optimized", num_qubits),
                &num_qubits,
                |b, &nq| {
                    b.iter(|| {
                        gate.apply_avx2(black_box(&mut real), black_box(&mut imag), black_box(nq))
                            .unwrap();
                    });
                },
            );
        }
    }

    group.finish();
}

// ============================================================================
// GROUP 4: PRODUCTION WORKLOAD (Multi-Gate Circuits)
// ============================================================================

fn bench_cz_production_graph_state(c: &mut Criterion) {
    let mut group = c.benchmark_group("cz_production_graph_state");

    // Graph state creation: Linear chain of 10 qubits
    // Edges: (0,1), (1,2), ..., (8,9) → 9 CZ gates
    let num_qubits = 10;
    let num_amplitudes = 1 << num_qubits;

    let gates: Vec<_> = (0..9)
        .map(|i| CZGateCapsule::new(i, i + 1).unwrap())
        .collect();

    let (mut real, mut imag) = create_equal_superposition(num_amplitudes);

    group.throughput(Throughput::Elements((9 * num_amplitudes) as u64)); // 9 gates × amplitudes

    group.bench_function("graph_state_10_qubits_scalar", |b| {
        b.iter(|| {
            for gate in &gates {
                gate.apply(
                    black_box(&mut real),
                    black_box(&mut imag),
                    black_box(num_qubits),
                )
                .unwrap();
            }
        });
    });

    group.finish();
}

#[cfg(feature = "portable_simd")]
fn bench_cz_production_graph_state_avx2(c: &mut Criterion) {
    let mut group = c.benchmark_group("cz_production_graph_state_avx2");

    let num_qubits = 10;
    let num_amplitudes = 1 << num_qubits;

    let gates: Vec<_> = (0..9)
        .map(|i| CZGateCapsule::new(i, i + 1).unwrap())
        .collect();

    let (mut real, mut imag) = create_equal_superposition(num_amplitudes);

    group.throughput(Throughput::Elements((9 * num_amplitudes) as u64));

    group.bench_function("graph_state_10_qubits_avx2", |b| {
        b.iter(|| {
            for gate in &gates {
                gate.apply_avx2(
                    black_box(&mut real),
                    black_box(&mut imag),
                    black_box(num_qubits),
                )
                .unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// GROUP 5: QUBIT RANGE STRESS TEST
// ============================================================================

fn bench_cz_qubit_range(c: &mut Criterion) {
    let mut group = c.benchmark_group("cz_qubit_range");

    // Test different qubit positions (low, mid, high indices)
    let num_qubits = 16;
    let num_amplitudes = 1 << num_qubits;

    let test_cases = vec![
        ("low_qubits_0_1", 0, 1),
        ("mid_qubits_7_8", 7, 8),
        ("high_qubits_14_15", 14, 15),
        ("distant_qubits_0_15", 0, 15),
    ];

    for (name, q1, q2) in test_cases {
        let gate = CZGateCapsule::new(q1, q2).unwrap();
        let (mut real, mut imag) = create_equal_superposition(num_amplitudes);

        group.throughput(Throughput::Elements(num_amplitudes as u64));

        group.bench_function(name, |b| {
            b.iter(|| {
                gate.apply(
                    black_box(&mut real),
                    black_box(&mut imag),
                    black_box(num_qubits),
                )
                .unwrap();
            });
        });
    }

    group.finish();
}

// ============================================================================
// GROUP 6: GATE COUNTER OVERHEAD
// ============================================================================

fn bench_cz_gate_counter_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("cz_gate_counter_overhead");

    let num_qubits = 12;
    let num_amplitudes = 1 << num_qubits;
    let gate = CZGateCapsule::new(0, 1).unwrap();
    let (mut real, mut imag) = create_equal_superposition(num_amplitudes);

    group.throughput(Throughput::Elements(num_amplitudes as u64));

    // Measure gate application latency (includes counter increment)
    group.bench_function("with_counter", |b| {
        b.iter(|| {
            gate.apply(
                black_box(&mut real),
                black_box(&mut imag),
                black_box(num_qubits),
            )
            .unwrap();
        });
    });

    group.finish();
}

// ============================================================================
// GROUP 7: MEMORY ACCESS PATTERNS
// ============================================================================

fn bench_cz_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("cz_memory_patterns");

    // Test cache-friendly vs cache-unfriendly qubit positions
    let num_qubits = 18;
    let num_amplitudes = 1 << num_qubits;

    // Cache-friendly: Adjacent qubits (stride 1-2)
    {
        let gate = CZGateCapsule::new(0, 1).unwrap();
        let (mut real, mut imag) = create_equal_superposition(num_amplitudes);

        group.throughput(Throughput::Elements(num_amplitudes as u64));

        group.bench_function("cache_friendly_adjacent", |b| {
            b.iter(|| {
                gate.apply(
                    black_box(&mut real),
                    black_box(&mut imag),
                    black_box(num_qubits),
                )
                .unwrap();
            });
        });
    }

    // Cache-unfriendly: Distant qubits (large stride)
    {
        let gate = CZGateCapsule::new(0, 17).unwrap();
        let (mut real, mut imag) = create_equal_superposition(num_amplitudes);

        group.bench_function("cache_unfriendly_distant", |b| {
            b.iter(|| {
                gate.apply(
                    black_box(&mut real),
                    black_box(&mut imag),
                    black_box(num_qubits),
                )
                .unwrap();
            });
        });
    }

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    name = benches_scalar;
    config = Criterion::default()
        .sample_size(100)        // 100 iterations per measurement
        .measurement_time(std::time::Duration::from_secs(5));
    targets = bench_cz_scalar_baseline,
              bench_cz_production_graph_state,
              bench_cz_qubit_range,
              bench_cz_gate_counter_overhead,
              bench_cz_memory_patterns
);

#[cfg(feature = "portable_simd")]
criterion_group!(
    name = benches_simd;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(std::time::Duration::from_secs(5));
    targets = bench_cz_avx2_optimized,
              bench_cz_speedup_validation,
              bench_cz_production_graph_state_avx2
);

#[cfg(feature = "portable_simd")]
criterion_main!(benches_scalar, benches_simd);

#[cfg(not(feature = "portable_simd"))]
criterion_main!(benches_scalar);
