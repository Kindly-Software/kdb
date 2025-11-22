//! B32 Benchmark: SIMD Quantum Gate Performance
//!
//! Measures actual speedup of SIMD implementation vs scalar baseline
//! for single-qubit gate applications across different configurations.
//!
//! Expected Results:
//! - stride = 1 (target qubit 0): 1.0× (falls back to scalar)
//! - stride >= 2 (target qubit 1+): 4-8× (SIMD optimization)
//!
//! Framework: B32 (fair baselines, 1000+ iterations, 95% CI)

use atomic_capsule::quantum_pure::{QuantumGateCapsule, QuantumState};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

/// Benchmark single-qubit Hadamard gate (H₀ - target qubit 0, stride = 1)
///
/// Expected: 1.0× speedup (SIMD falls back to scalar for stride = 1)
fn bench_hadamard_qubit0(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum_gate/hadamard_q0");

    for num_qubits in [4, 8, 12, 16].iter() {
        let mut state = QuantumState::new(*num_qubits).unwrap();
        let gate = QuantumGateCapsule::hadamard(0); // Target qubit 0 (stride = 1)

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}qubits", num_qubits)),
            num_qubits,
            |b, _| {
                b.iter(|| {
                    state.apply_gate(black_box(&gate)).unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark single-qubit Hadamard gate (H₁ - target qubit 1, stride = 2)
///
/// Expected: 4-8× speedup (SIMD processes 2 pairs/iteration)
fn bench_hadamard_qubit1(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum_gate/hadamard_q1");

    for num_qubits in [4, 8, 12, 16].iter() {
        let mut state = QuantumState::new(*num_qubits).unwrap();
        let gate = QuantumGateCapsule::hadamard(1); // Target qubit 1 (stride = 2)

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}qubits", num_qubits)),
            num_qubits,
            |b, _| {
                b.iter(|| {
                    state.apply_gate(black_box(&gate)).unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark single-qubit Hadamard gate (H₂ - target qubit 2, stride = 4)
///
/// Expected: 6-8× speedup (SIMD with larger stride, maximum benefit)
fn bench_hadamard_qubit2(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum_gate/hadamard_q2");

    for num_qubits in [4, 8, 12, 16].iter() {
        let mut state = QuantumState::new(*num_qubits).unwrap();
        let gate = QuantumGateCapsule::hadamard(2); // Target qubit 2 (stride = 4)

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}qubits", num_qubits)),
            num_qubits,
            |b, _| {
                b.iter(|| {
                    state.apply_gate(black_box(&gate)).unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark gate sequence (mixed stride workload)
///
/// Expected: 4-6× average speedup (mix of stride=1 and stride>=2)
fn bench_gate_sequence(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum_gate/sequence");

    for num_qubits in [4, 8, 12, 16].iter() {
        let mut state = QuantumState::new(*num_qubits).unwrap();
        let gates = vec![
            QuantumGateCapsule::hadamard(0),  // stride = 1 (scalar)
            QuantumGateCapsule::hadamard(1),  // stride = 2 (SIMD)
            QuantumGateCapsule::pauli_x(0),   // stride = 1 (scalar)
            QuantumGateCapsule::pauli_x(1),   // stride = 2 (SIMD)
            QuantumGateCapsule::s_gate(0),    // stride = 1 (scalar)
            QuantumGateCapsule::s_gate(1),    // stride = 2 (SIMD)
        ];

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}qubits", num_qubits)),
            num_qubits,
            |b, _| {
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

criterion_group!(
    benches,
    bench_hadamard_qubit0,
    bench_hadamard_qubit1,
    bench_hadamard_qubit2,
    bench_gate_sequence
);
criterion_main!(benches);
