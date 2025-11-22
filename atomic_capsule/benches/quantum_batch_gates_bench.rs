//! B32 Benchmarks: Horizontal SIMD Gate Batching (Phase 3.2)
//!
//! # Benchmark Groups
//!
//! 1. **Batch Size Scaling**: 1 gate (baseline) vs 4 gates vs 8 gates
//! 2. **Gate Type Efficiency**: Hadamard batch vs Pauli-X batch vs mixed
//! 3. **Circuit Density**: Sparse (50%+ independent) vs dense (20% independent)
//! 4. **Qubit Scaling**: 4, 8, 12, 16 qubits
//!
//! # Expected Results (B32 Conservative)
//!
//! - Sparse circuits (50%+ independent gates): **2.5× speedup**
//! - Dense circuits (20% independent gates): **1.4× speedup**
//! - Average: **2.0× speedup**
//!
//! # Combined Speedup (AVX2 + Horizontal)
//!
//! - Vertical SIMD (Phase 2): 3-4× vs scalar
//! - Horizontal SIMD (Phase 3.2): 2× additional
//! - **Total: 6-8× vs scalar baseline**

#[cfg(all(feature = "quantum-pure", feature = "portable_simd"))]
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

#[cfg(all(feature = "quantum-pure", feature = "portable_simd"))]
use atomic_capsule::quantum_pure::{QuantumCircuitCapsule, QuantumGateCapsule};

#[cfg(all(feature = "quantum-pure", feature = "portable_simd"))]
fn bench_batch_size_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_size_scaling");

    // Baseline: 1 gate (no batching)
    group.bench_function("1_gate_sequential", |b| {
        b.iter(|| {
            let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
            circuit.add_gate(QuantumGateCapsule::hadamard(0)).unwrap();
            circuit.execute().unwrap();
            black_box(circuit.execution_time_ns());
        });
    });

    // 4 gates (batch-4 SIMD)
    group.bench_function("4_gates_batched", |b| {
        b.iter(|| {
            let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
            for i in 0..4 {
                circuit.add_gate(QuantumGateCapsule::hadamard(i)).unwrap();
            }
            circuit.execute_batched().unwrap();
            black_box(circuit.execution_time_ns());
        });
    });

    // 4 gates (sequential baseline)
    group.bench_function("4_gates_sequential", |b| {
        b.iter(|| {
            let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
            for i in 0..4 {
                circuit.add_gate(QuantumGateCapsule::hadamard(i)).unwrap();
            }
            circuit.execute().unwrap();
            black_box(circuit.execution_time_ns());
        });
    });

    // 8 gates (all independent)
    group.bench_function("8_gates_batched", |b| {
        b.iter(|| {
            let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
            for i in 0..8 {
                circuit.add_gate(QuantumGateCapsule::hadamard(i)).unwrap();
            }
            circuit.execute_batched().unwrap();
            black_box(circuit.execution_time_ns());
        });
    });

    // 8 gates (sequential baseline)
    group.bench_function("8_gates_sequential", |b| {
        b.iter(|| {
            let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
            for i in 0..8 {
                circuit.add_gate(QuantumGateCapsule::hadamard(i)).unwrap();
            }
            circuit.execute().unwrap();
            black_box(circuit.execution_time_ns());
        });
    });

    group.finish();
}

#[cfg(all(feature = "quantum-pure", feature = "portable_simd"))]
fn bench_gate_type_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_type_efficiency");

    // Hadamard batch (uniform matrix)
    group.bench_function("hadamard_batch_4", |b| {
        b.iter(|| {
            let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
            for i in 0..4 {
                circuit.add_gate(QuantumGateCapsule::hadamard(i)).unwrap();
            }
            circuit.execute_batched().unwrap();
            black_box(circuit.execution_time_ns());
        });
    });

    // Pauli-X batch (different matrix)
    group.bench_function("pauli_x_batch_4", |b| {
        b.iter(|| {
            let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
            for i in 0..4 {
                circuit.add_gate(QuantumGateCapsule::pauli_x(i)).unwrap();
            }
            circuit.execute_batched().unwrap();
            black_box(circuit.execution_time_ns());
        });
    });

    // Mixed types (cannot batch across types)
    group.bench_function("mixed_types_4", |b| {
        b.iter(|| {
            let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
            circuit.add_gate(QuantumGateCapsule::hadamard(0)).unwrap();
            circuit.add_gate(QuantumGateCapsule::pauli_x(1)).unwrap();
            circuit.add_gate(QuantumGateCapsule::pauli_y(2)).unwrap();
            circuit.add_gate(QuantumGateCapsule::pauli_z(3)).unwrap();
            circuit.execute_batched().unwrap();
            black_box(circuit.execution_time_ns());
        });
    });

    group.finish();
}

#[cfg(all(feature = "quantum-pure", feature = "portable_simd"))]
fn bench_circuit_density(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_density");

    // Sparse circuit (100% independent gates)
    group.bench_function("sparse_100pct_batched", |b| {
        b.iter(|| {
            let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
            // 16 gates, all independent (8 qubits × 2 rounds)
            for _ in 0..2 {
                for i in 0..8 {
                    circuit.add_gate(QuantumGateCapsule::hadamard(i)).unwrap();
                }
            }
            circuit.execute_batched().unwrap();
            black_box(circuit.execution_time_ns());
        });
    });

    // Sparse circuit (100% independent) - sequential baseline
    group.bench_function("sparse_100pct_sequential", |b| {
        b.iter(|| {
            let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
            for _ in 0..2 {
                for i in 0..8 {
                    circuit.add_gate(QuantumGateCapsule::hadamard(i)).unwrap();
                }
            }
            circuit.execute().unwrap();
            black_box(circuit.execution_time_ns());
        });
    });

    // Dense circuit (20% independent - many dependencies)
    group.bench_function("dense_20pct_batched", |b| {
        b.iter(|| {
            let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
            // 16 gates, only 20% can batch (concentrate on few qubits)
            for _ in 0..8 {
                circuit.add_gate(QuantumGateCapsule::hadamard(0)).unwrap();
                circuit.add_gate(QuantumGateCapsule::hadamard(1)).unwrap();
            }
            circuit.execute_batched().unwrap();
            black_box(circuit.execution_time_ns());
        });
    });

    // Dense circuit (20% independent) - sequential baseline
    group.bench_function("dense_20pct_sequential", |b| {
        b.iter(|| {
            let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
            for _ in 0..8 {
                circuit.add_gate(QuantumGateCapsule::hadamard(0)).unwrap();
                circuit.add_gate(QuantumGateCapsule::hadamard(1)).unwrap();
            }
            circuit.execute().unwrap();
            black_box(circuit.execution_time_ns());
        });
    });

    group.finish();
}

#[cfg(all(feature = "quantum-pure", feature = "portable_simd"))]
fn bench_qubit_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("qubit_scaling");

    for num_qubits in [4, 8, 12, 16] {
        // Batched execution
        group.bench_with_input(
            BenchmarkId::new("batched", num_qubits),
            &num_qubits,
            |b, &nq| {
                b.iter(|| {
                    let mut circuit = QuantumCircuitCapsule::new(nq).unwrap();
                    for i in 0..nq.min(8) {
                        circuit.add_gate(QuantumGateCapsule::hadamard(i as usize)).unwrap();
                    }
                    circuit.execute_batched().unwrap();
                    black_box(circuit.execution_time_ns());
                });
            },
        );

        // Sequential baseline
        group.bench_with_input(
            BenchmarkId::new("sequential", num_qubits),
            &num_qubits,
            |b, &nq| {
                b.iter(|| {
                    let mut circuit = QuantumCircuitCapsule::new(nq).unwrap();
                    for i in 0..nq.min(8) {
                        circuit.add_gate(QuantumGateCapsule::hadamard(i as usize)).unwrap();
                    }
                    circuit.execute().unwrap();
                    black_box(circuit.execution_time_ns());
                });
            },
        );
    }

    group.finish();
}

#[cfg(all(feature = "quantum-pure", feature = "portable_simd"))]
fn bench_large_circuit(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_circuit");

    // 64-gate circuit (8 qubits × 8 gates each, all independent)
    group.bench_function("64_gates_batched", |b| {
        b.iter(|| {
            let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
            for _ in 0..8 {
                for i in 0..8 {
                    circuit.add_gate(QuantumGateCapsule::hadamard(i)).unwrap();
                }
            }
            circuit.execute_batched().unwrap();
            black_box(circuit.execution_time_ns());
        });
    });

    // 64-gate circuit - sequential baseline
    group.bench_function("64_gates_sequential", |b| {
        b.iter(|| {
            let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
            for _ in 0..8 {
                for i in 0..8 {
                    circuit.add_gate(QuantumGateCapsule::hadamard(i)).unwrap();
                }
            }
            circuit.execute().unwrap();
            black_box(circuit.execution_time_ns());
        });
    });

    group.finish();
}

#[cfg(all(feature = "quantum-pure", feature = "portable_simd"))]
criterion_group!(
    benches,
    bench_batch_size_scaling,
    bench_gate_type_efficiency,
    bench_circuit_density,
    bench_qubit_scaling,
    bench_large_circuit
);

#[cfg(all(feature = "quantum-pure", feature = "portable_simd"))]
criterion_main!(benches);

// Stub for non-SIMD builds
#[cfg(not(all(feature = "quantum-pure", feature = "portable_simd")))]
fn main() {
    eprintln!("Benchmarks require features: quantum-pure, portable_simd");
}
