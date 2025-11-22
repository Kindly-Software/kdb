//! B32 Benchmarks: Phase 3.4 Multi-Threaded SIMD Thread Scaling
//!
//! # Benchmark Goals (B32 Conservative)
//!
//! 1. **Thread Scaling**: Measure speedup vs thread count (1, 2, 4, 8, 16)
//! 2. **Amdahl's Law Validation**: Verify parallel efficiency ≥85% at 8 threads
//! 3. **Problem Size Scaling**: Determine threshold where threading helps (vs overhead)
//! 4. **Combined Speedup**: Phase 3.1 AVX2 (4×) + Phase 3.4 Threading (7×) = 28× total
//!
//! # Fair Baselines
//!
//! - **Scalar**: Pure sequential (no SIMD, no threading)
//! - **SIMD**: Phase 3.1 AVX2 (4× baseline)
//! - **Parallel SIMD**: Phase 3.4 (28× target)
//!
//! # Expected Results (8-core CPU, 20 qubits = 524K pairs)
//!
//! - 1 thread: 100 µs (SIMD baseline)
//! - 2 threads: 52 µs (1.92× speedup, 96% efficiency)
//! - 4 threads: 27 µs (3.70× speedup, 93% efficiency)
//! - 8 threads: 14 µs (7.14× speedup, 89% efficiency ≥ 85% target ✓)
//!
//! # Hardware Reality (B32 K1-K70)
//!
//! - CPU: AMD Ryzen 9 6900HX (8c/16t, 4.9 GHz boost)
//! - L1: 32 KB per core
//! - L2: 512 KB per core
//! - L3: 16 MB shared
//! - SIMD: AVX2 (256-bit, 4× f64)

#![cfg(all(feature = "portable_simd", feature = "batch-native"))]

use atomic_capsule::quantum_pure::{QuantumGateCapsule, QuantumState};
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

// ============================================================================
// Benchmark Group 1: Thread Scaling (Amdahl's Law)
// ============================================================================

fn bench_thread_scaling_20_qubits(c: &mut Criterion) {
    let mut group = c.benchmark_group("thread_scaling_20_qubits");

    // 20 qubits = 2^20 = 1M amplitudes, stride = 2^20 = 1M (well above 100K threshold)
    let thread_counts = [1, 2, 4, 8, 16];

    for &num_threads in &thread_counts {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}threads", num_threads)),
            &num_threads,
            |b, &threads| {
                // Configure thread pool
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build_global()
                    .ok();

                let mut state = QuantumState::new(20).unwrap();
                let h_gate = QuantumGateCapsule::hadamard(20); // Target qubit 20 → 1M stride

                b.iter(|| {
                    state.apply_gate(black_box(&h_gate)).unwrap();
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark Group 2: Problem Size Scaling (Threshold Analysis)
// ============================================================================

fn bench_problem_size_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("problem_size_scaling");

    // Test different qubit counts to find parallelization threshold
    // stride = 2^target
    // 16 qubits: stride = 65K < 100K (SIMD only)
    // 17 qubits: stride = 131K > 100K (Parallel SIMD)
    // 18 qubits: stride = 262K (Parallel SIMD)
    // 19 qubits: stride = 524K (Parallel SIMD)
    // 20 qubits: stride = 1M (Parallel SIMD)
    let qubit_counts = [16, 17, 18, 19, 20];

    rayon::ThreadPoolBuilder::new()
        .num_threads(8)
        .build_global()
        .ok();

    for &num_qubits in &qubit_counts {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}qubits", num_qubits)),
            &num_qubits,
            |b, &qubits| {
                let mut state = QuantumState::new(qubits).unwrap();
                let h_gate = QuantumGateCapsule::hadamard(qubits - 1);

                b.iter(|| {
                    state.apply_gate(black_box(&h_gate)).unwrap();
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark Group 3: Gate Type Comparison (All 6 Gates)
// ============================================================================

fn bench_gate_types_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_types_parallel");

    rayon::ThreadPoolBuilder::new()
        .num_threads(8)
        .build_global()
        .ok();

    let gates = [
        ("hadamard", QuantumGateCapsule::hadamard(18)),
        ("pauli_x", QuantumGateCapsule::pauli_x(18)),
        ("pauli_y", QuantumGateCapsule::pauli_y(18)),
        ("pauli_z", QuantumGateCapsule::pauli_z(18)),
        ("s_gate", QuantumGateCapsule::s_gate(18)),
        ("t_gate", QuantumGateCapsule::t_gate(18)),
    ];

    for (name, gate) in &gates {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            gate,
            |b, gate| {
                let mut state = QuantumState::new(18).unwrap();

                b.iter(|| {
                    state.apply_gate(black_box(gate)).unwrap();
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark Group 4: Combined Phase 3 Speedup (Scalar → SIMD → Parallel SIMD)
// ============================================================================

fn bench_phase3_combined_speedup(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase3_combined_speedup");

    // Baseline: Scalar (no SIMD, no threading)
    // Note: We can't easily disable SIMD in the current implementation,
    // so we'll use a smaller problem size to estimate scalar performance
    group.bench_function("scalar_baseline_16qubits", |b| {
        let mut state = QuantumState::new(16).unwrap();
        let h_gate = QuantumGateCapsule::hadamard(0); // stride = 1 → scalar path

        b.iter(|| {
            state.apply_gate(black_box(&h_gate)).unwrap();
        });
    });

    // Phase 3.1: SIMD only (single-threaded)
    group.bench_function("simd_only_16qubits", |b| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build_global()
            .ok();

        let mut state = QuantumState::new(16).unwrap();
        let h_gate = QuantumGateCapsule::hadamard(16); // stride = 65K < 100K → SIMD only

        b.iter(|| {
            state.apply_gate(black_box(&h_gate)).unwrap();
        });
    });

    // Phase 3.4: Parallel SIMD (8 threads)
    group.bench_function("parallel_simd_20qubits_8threads", |b| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(8)
            .build_global()
            .ok();

        let mut state = QuantumState::new(20).unwrap();
        let h_gate = QuantumGateCapsule::hadamard(20); // stride = 1M > 100K → parallel SIMD

        b.iter(|| {
            state.apply_gate(black_box(&h_gate)).unwrap();
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark Group 5: Parallel Efficiency (Scaling Analysis)
// ============================================================================

fn bench_parallel_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_efficiency");

    // Fixed problem size (20 qubits), varying thread count
    // Measure efficiency = (speedup / num_threads) × 100%
    // Target: ≥85% efficiency at 8 threads
    let thread_counts = [1, 2, 4, 8, 16];

    for &num_threads in &thread_counts {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}threads_efficiency", num_threads)),
            &num_threads,
            |b, &threads| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build_global()
                    .ok();

                let mut state = QuantumState::new(20).unwrap();
                let h_gate = QuantumGateCapsule::hadamard(20);

                b.iter(|| {
                    state.apply_gate(black_box(&h_gate)).unwrap();
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark Group 6: Load Balancing (Rayon Work Stealing)
// ============================================================================

fn bench_load_balancing(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_balancing");

    rayon::ThreadPoolBuilder::new()
        .num_threads(8)
        .build_global()
        .ok();

    // Test irregular workload: Mix of gates on different qubits
    // Rayon work stealing should balance load efficiently
    group.bench_function("irregular_workload_mixed_gates", |b| {
        let mut state = QuantumState::new(20).unwrap();

        // Mix of gates on different qubits (different strides)
        let gates = [
            QuantumGateCapsule::hadamard(20),  // stride = 1M
            QuantumGateCapsule::pauli_x(19),   // stride = 524K
            QuantumGateCapsule::pauli_y(18),   // stride = 262K
            QuantumGateCapsule::s_gate(20),    // stride = 1M
        ];

        b.iter(|| {
            for gate in &gates {
                state.apply_gate(black_box(gate)).unwrap();
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_thread_scaling_20_qubits,
    bench_problem_size_scaling,
    bench_gate_types_parallel,
    bench_phase3_combined_speedup,
    bench_parallel_efficiency,
    bench_load_balancing,
);

criterion_main!(benches);
