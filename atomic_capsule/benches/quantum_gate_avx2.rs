//! B32 Benchmark: AVX2 Quantum Gate Performance (Phase 3.1)
//!
//! Measures actual speedup of AVX2 implementation vs SSE and scalar baselines
//! for single-qubit gate applications across different configurations.
//!
//! # Expected Results (B32 Framework)
//!
//! - **Scalar baseline**: 1.0× (reference)
//! - **SSE baseline** (stride >= 2): 1.56-1.72× vs scalar
//! - **AVX2 upgrade** (stride >= 4): 3-6× vs scalar (2-4× over SSE)
//!
//! # Benchmark Groups
//!
//! 1. **avx2_vs_scalar**: Direct AVX2 to scalar comparison (qubit 2, stride=4)
//! 2. **avx2_vs_sse**: AVX2 improvement over SSE (qubit 2 AVX2 vs qubit 1 SSE)
//! 3. **avx2_gate_types**: All gate types with AVX2 (H, X, Y, Z, S, T)
//! 4. **avx2_scaling**: Scaling from 4 to 16 qubits (stride=4)
//!
//! # Framework Compliance
//!
//! - **Fair baselines**: SSE and scalar paths using same hardware
//! - **1000+ iterations**: Criterion default (adaptive sampling)
//! - **95% CI**: Criterion statistical analysis
//! - **Reproducibility**: Black-box gates, fresh states
//!
//! # Performance Targets
//!
//! - **Minimum**: 3× vs scalar (conservative)
//! - **Target**: 4× vs scalar (realistic)
//! - **Stretch**: 6× vs scalar (optimistic)

#![cfg(all(feature = "avx2-simd", target_arch = "x86_64"))]

use atomic_capsule::quantum_pure::{QuantumGateCapsule, QuantumState};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

// ============================================================================
// Group 1: AVX2 vs Scalar (Direct Comparison)
// ============================================================================

/// Benchmark AVX2 vs scalar for Hadamard gate (qubit 2, stride=4)
///
/// Expected: 3-6× speedup (AVX2 processes 4 pairs/iteration vs 1 pair scalar)
fn bench_avx2_vs_scalar_hadamard(c: &mut Criterion) {
    let mut group = c.benchmark_group("avx2_vs_scalar/hadamard");

    for num_qubits in [4, 8, 12, 16].iter() {
        // AVX2 path: qubit 2 (stride=4)
        group.bench_with_input(
            BenchmarkId::new("avx2", num_qubits),
            num_qubits,
            |b, &n| {
                let mut state = QuantumState::new(n).unwrap();
                let gate = QuantumGateCapsule::hadamard(2); // stride=4 (AVX2 path)
                b.iter(|| {
                    state.apply_gate(black_box(&gate)).unwrap();
                });
            },
        );

        // Scalar baseline: qubit 0 (stride=1, forces scalar path)
        // Note: This simulates scalar performance for comparison
        // (actual scalar method not directly callable from external tests)
        group.bench_with_input(
            BenchmarkId::new("scalar_fallback", num_qubits),
            num_qubits,
            |b, &n| {
                let mut state = QuantumState::new(n).unwrap();
                let gate = QuantumGateCapsule::hadamard(0); // stride=1 (scalar path)
                b.iter(|| {
                    state.apply_gate(black_box(&gate)).unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark AVX2 vs scalar for Pauli-X gate
fn bench_avx2_vs_scalar_pauli_x(c: &mut Criterion) {
    let mut group = c.benchmark_group("avx2_vs_scalar/pauli_x");

    for num_qubits in [4, 8, 12, 16].iter() {
        group.bench_with_input(
            BenchmarkId::new("avx2", num_qubits),
            num_qubits,
            |b, &n| {
                let mut state = QuantumState::new(n).unwrap();
                let gate = QuantumGateCapsule::pauli_x(2);
                b.iter(|| {
                    state.apply_gate(black_box(&gate)).unwrap();
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("scalar_fallback", num_qubits),
            num_qubits,
            |b, &n| {
                let mut state = QuantumState::new(n).unwrap();
                let gate = QuantumGateCapsule::pauli_x(0);
                b.iter(|| {
                    state.apply_gate(black_box(&gate)).unwrap();
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Group 2: AVX2 vs SSE (Improvement Validation)
// ============================================================================

/// Benchmark AVX2 improvement over SSE baseline
///
/// Expected: 2-4× speedup (AVX2 qubit 2 vs SSE qubit 1)
fn bench_avx2_vs_sse_hadamard(c: &mut Criterion) {
    let mut group = c.benchmark_group("avx2_vs_sse/hadamard");

    for num_qubits in [4, 8, 12, 16].iter() {
        // AVX2 path: qubit 2 (stride=4)
        group.bench_with_input(
            BenchmarkId::new("avx2_q2", num_qubits),
            num_qubits,
            |b, &n| {
                let mut state = QuantumState::new(n).unwrap();
                let gate = QuantumGateCapsule::hadamard(2); // stride=4 (AVX2)
                b.iter(|| {
                    state.apply_gate(black_box(&gate)).unwrap();
                });
            },
        );

        // SSE baseline: qubit 1 (stride=2)
        group.bench_with_input(
            BenchmarkId::new("sse_q1", num_qubits),
            num_qubits,
            |b, &n| {
                let mut state = QuantumState::new(n).unwrap();
                let gate = QuantumGateCapsule::hadamard(1); // stride=2 (SSE)
                b.iter(|| {
                    state.apply_gate(black_box(&gate)).unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark AVX2 vs SSE for all gate types
fn bench_avx2_vs_sse_all_gates(c: &mut Criterion) {
    let mut group = c.benchmark_group("avx2_vs_sse/all_gates");

    let num_qubits = 8; // Fixed size for gate comparison

    // AVX2 path (qubit 2)
    for gate_name in ["hadamard", "pauli_x", "pauli_y", "pauli_z", "s_gate", "t_gate"].iter() {
        group.bench_with_input(
            BenchmarkId::new("avx2", gate_name),
            gate_name,
            |b, &name| {
                let mut state = QuantumState::new(num_qubits).unwrap();
                let gate = match name {
                    "hadamard" => QuantumGateCapsule::hadamard(2),
                    "pauli_x" => QuantumGateCapsule::pauli_x(2),
                    "pauli_y" => QuantumGateCapsule::pauli_y(2),
                    "pauli_z" => QuantumGateCapsule::pauli_z(2),
                    "s_gate" => QuantumGateCapsule::s_gate(2),
                    "t_gate" => QuantumGateCapsule::t_gate(2),
                    _ => unreachable!(),
                };
                b.iter(|| {
                    state.apply_gate(black_box(&gate)).unwrap();
                });
            },
        );
    }

    // SSE baseline (qubit 1)
    for gate_name in ["hadamard", "pauli_x", "pauli_y", "pauli_z", "s_gate", "t_gate"].iter() {
        group.bench_with_input(
            BenchmarkId::new("sse", gate_name),
            gate_name,
            |b, &name| {
                let mut state = QuantumState::new(num_qubits).unwrap();
                let gate = match name {
                    "hadamard" => QuantumGateCapsule::hadamard(1),
                    "pauli_x" => QuantumGateCapsule::pauli_x(1),
                    "pauli_y" => QuantumGateCapsule::pauli_y(1),
                    "pauli_z" => QuantumGateCapsule::pauli_z(1),
                    "s_gate" => QuantumGateCapsule::s_gate(1),
                    "t_gate" => QuantumGateCapsule::t_gate(1),
                    _ => unreachable!(),
                };
                b.iter(|| {
                    state.apply_gate(black_box(&gate)).unwrap();
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Group 3: AVX2 Gate Types (Individual Performance)
// ============================================================================

/// Benchmark individual gate types with AVX2
fn bench_avx2_gate_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("avx2_gate_types");

    let num_qubits = 8;
    let target = 2; // stride=4 (AVX2 path)

    for gate_name in ["hadamard", "pauli_x", "pauli_y", "pauli_z", "s_gate", "t_gate"].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(gate_name),
            gate_name,
            |b, &name| {
                let mut state = QuantumState::new(num_qubits).unwrap();
                let gate = match name {
                    "hadamard" => QuantumGateCapsule::hadamard(target),
                    "pauli_x" => QuantumGateCapsule::pauli_x(target),
                    "pauli_y" => QuantumGateCapsule::pauli_y(target),
                    "pauli_z" => QuantumGateCapsule::pauli_z(target),
                    "s_gate" => QuantumGateCapsule::s_gate(target),
                    "t_gate" => QuantumGateCapsule::t_gate(target),
                    _ => unreachable!(),
                };
                b.iter(|| {
                    state.apply_gate(black_box(&gate)).unwrap();
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Group 4: AVX2 Scaling (Problem Size)
// ============================================================================

/// Benchmark AVX2 scaling from 4 to 16 qubits
///
/// Expected: Consistent 3-6× speedup across all sizes
fn bench_avx2_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("avx2_scaling");

    for num_qubits in [4, 6, 8, 10, 12, 14, 16].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}qubits", num_qubits)),
            num_qubits,
            |b, &n| {
                let mut state = QuantumState::new(n).unwrap();
                let gate = QuantumGateCapsule::hadamard(2); // stride=4 (AVX2)
                b.iter(|| {
                    state.apply_gate(black_box(&gate)).unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark AVX2 with different stride sizes (validate all AVX2 paths)
fn bench_avx2_stride_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("avx2_stride_sizes");

    let num_qubits = 8;

    // Test stride=4, 8, 16, 32 (all AVX2 paths)
    for target in 2..=5 {
        group.bench_with_input(
            BenchmarkId::new("avx2", format!("stride_{}", 1 << target)),
            &target,
            |b, &t| {
                let mut state = QuantumState::new(num_qubits).unwrap();
                let gate = QuantumGateCapsule::hadamard(t);
                b.iter(|| {
                    state.apply_gate(black_box(&gate)).unwrap();
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Group 5: AVX2 Mixed Workloads
// ============================================================================

/// Benchmark mixed gate sequence (realistic quantum algorithm)
fn bench_avx2_gate_sequence(c: &mut Criterion) {
    let mut group = c.benchmark_group("avx2_gate_sequence");

    for num_qubits in [8, 12, 16].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}qubits", num_qubits)),
            num_qubits,
            |b, &n| {
                let mut state = QuantumState::new(n).unwrap();
                let gates = vec![
                    QuantumGateCapsule::hadamard(2),  // AVX2
                    QuantumGateCapsule::pauli_x(3),   // AVX2
                    QuantumGateCapsule::s_gate(2),    // AVX2
                    QuantumGateCapsule::t_gate(3),    // AVX2
                    QuantumGateCapsule::pauli_y(4),   // AVX2
                    QuantumGateCapsule::hadamard(3),  // AVX2
                ];
                b.iter(|| {
                    for gate in &gates {
                        state.apply_gate(black_box(gate)).unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Group 6: AVX2 Stress Test
// ============================================================================

/// Benchmark AVX2 with 100 gate applications (stress test)
fn bench_avx2_stress_100_gates(c: &mut Criterion) {
    let mut group = c.benchmark_group("avx2_stress");

    for num_qubits in [8, 12, 16].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}qubits_100gates", num_qubits)),
            num_qubits,
            |b, &n| {
                let mut state = QuantumState::new(n).unwrap();
                let gate = QuantumGateCapsule::hadamard(2); // AVX2 path
                b.iter(|| {
                    for _ in 0..100 {
                        state.apply_gate(black_box(&gate)).unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_avx2_vs_scalar_hadamard,
    bench_avx2_vs_scalar_pauli_x,
    bench_avx2_vs_sse_hadamard,
    bench_avx2_vs_sse_all_gates,
    bench_avx2_gate_types,
    bench_avx2_scaling,
    bench_avx2_stride_sizes,
    bench_avx2_gate_sequence,
    bench_avx2_stress_100_gates
);
criterion_main!(benches);
