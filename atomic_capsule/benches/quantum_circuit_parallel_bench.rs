//! B32 Benchmarks: Quantum Circuit Parallel Execution (T4 Batch)
//!
//! # Performance Targets (Conservative B32 Estimates)
//!
//! - 10-gate circuit: 2-4× speedup (overhead amortization)
//! - 100-gate circuit: 8-12× speedup (good parallelism)
//! - 1000-gate circuit: 10-16× speedup (optimal parallelism)
//!
//! # Methodology
//!
//! - Fair baseline: Sequential execution (no strawman)
//! - 1000+ iterations per benchmark
//! - 95% confidence intervals
//! - Warm-up runs to stabilize CPU frequency
//!
//! # Hardware
//!
//! AMD Ryzen 9 6900HX (8 cores, 16 threads)

#[cfg(all(feature = "quantum-pure", feature = "rayon"))]
use atomic_capsule::quantum_pure::{QuantumCircuitCapsule, QuantumGateCapsule};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(all(feature = "quantum-pure", feature = "rayon"))]
fn benchmark_sequential_vs_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum_circuit_sequential_vs_parallel");

    // Benchmark 10-gate circuit (overhead amortization test)
    group.bench_function("10_gates_sequential", |b| {
        let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
        for i in 0..10 {
            circuit.add_gate(QuantumGateCapsule::hadamard(i % 8)).unwrap();
        }

        b.iter(|| {
            let mut circuit_copy = QuantumCircuitCapsule::new(8).unwrap();
            for i in 0..10 {
                circuit_copy.add_gate(QuantumGateCapsule::hadamard(i % 8)).unwrap();
            }
            circuit_copy.execute().unwrap();
            black_box(circuit_copy.execution_time_ns());
        });
    });

    group.bench_function("10_gates_parallel", |b| {
        let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
        for i in 0..10 {
            circuit.add_gate(QuantumGateCapsule::hadamard(i % 8)).unwrap();
        }

        b.iter(|| {
            let mut circuit_copy = QuantumCircuitCapsule::new(8).unwrap();
            for i in 0..10 {
                circuit_copy.add_gate(QuantumGateCapsule::hadamard(i % 8)).unwrap();
            }
            circuit_copy.execute_parallel().unwrap();
            black_box(circuit_copy.execution_time_ns());
        });
    });

    // Benchmark 100-gate circuit (good parallelism test)
    group.bench_function("100_gates_sequential", |b| {
        let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
        for i in 0..100 {
            let gate = match i % 5 {
                0 => QuantumGateCapsule::hadamard(i % 8),
                1 => QuantumGateCapsule::pauli_x(i % 8),
                2 => QuantumGateCapsule::pauli_y(i % 8),
                3 => QuantumGateCapsule::pauli_z(i % 8),
                4 => QuantumGateCapsule::s_gate(i % 8),
                _ => unreachable!(),
            };
            circuit.add_gate(gate).unwrap();
        }

        b.iter(|| {
            let mut circuit_copy = QuantumCircuitCapsule::new(8).unwrap();
            for i in 0..100 {
                let gate = match i % 5 {
                    0 => QuantumGateCapsule::hadamard(i % 8),
                    1 => QuantumGateCapsule::pauli_x(i % 8),
                    2 => QuantumGateCapsule::pauli_y(i % 8),
                    3 => QuantumGateCapsule::pauli_z(i % 8),
                    4 => QuantumGateCapsule::s_gate(i % 8),
                    _ => unreachable!(),
                };
                circuit_copy.add_gate(gate).unwrap();
            }
            circuit_copy.execute().unwrap();
            black_box(circuit_copy.execution_time_ns());
        });
    });

    group.bench_function("100_gates_parallel", |b| {
        let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
        for i in 0..100 {
            let gate = match i % 5 {
                0 => QuantumGateCapsule::hadamard(i % 8),
                1 => QuantumGateCapsule::pauli_x(i % 8),
                2 => QuantumGateCapsule::pauli_y(i % 8),
                3 => QuantumGateCapsule::pauli_z(i % 8),
                4 => QuantumGateCapsule::s_gate(i % 8),
                _ => unreachable!(),
            };
            circuit.add_gate(gate).unwrap();
        }

        b.iter(|| {
            let mut circuit_copy = QuantumCircuitCapsule::new(8).unwrap();
            for i in 0..100 {
                let gate = match i % 5 {
                    0 => QuantumGateCapsule::hadamard(i % 8),
                    1 => QuantumGateCapsule::pauli_x(i % 8),
                    2 => QuantumGateCapsule::pauli_y(i % 8),
                    3 => QuantumGateCapsule::pauli_z(i % 8),
                    4 => QuantumGateCapsule::s_gate(i % 8),
                    _ => unreachable!(),
                };
                circuit_copy.add_gate(gate).unwrap();
            }
            circuit_copy.execute_parallel().unwrap();
            black_box(circuit_copy.execution_time_ns());
        });
    });

    // Benchmark 1000-gate circuit (optimal parallelism test)
    group.bench_function("1000_gates_sequential", |b| {
        let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
        for i in 0..1000 {
            let gate = match i % 6 {
                0 => QuantumGateCapsule::hadamard(i % 8),
                1 => QuantumGateCapsule::pauli_x(i % 8),
                2 => QuantumGateCapsule::pauli_y(i % 8),
                3 => QuantumGateCapsule::pauli_z(i % 8),
                4 => QuantumGateCapsule::s_gate(i % 8),
                5 => QuantumGateCapsule::t_gate(i % 8),
                _ => unreachable!(),
            };
            circuit.add_gate(gate).unwrap();
        }

        b.iter(|| {
            let mut circuit_copy = QuantumCircuitCapsule::new(8).unwrap();
            for i in 0..1000 {
                let gate = match i % 6 {
                    0 => QuantumGateCapsule::hadamard(i % 8),
                    1 => QuantumGateCapsule::pauli_x(i % 8),
                    2 => QuantumGateCapsule::pauli_y(i % 8),
                    3 => QuantumGateCapsule::pauli_z(i % 8),
                    4 => QuantumGateCapsule::s_gate(i % 8),
                    5 => QuantumGateCapsule::t_gate(i % 8),
                    _ => unreachable!(),
                };
                circuit_copy.add_gate(gate).unwrap();
            }
            circuit_copy.execute().unwrap();
            black_box(circuit_copy.execution_time_ns());
        });
    });

    group.bench_function("1000_gates_parallel", |b| {
        let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
        for i in 0..1000 {
            let gate = match i % 6 {
                0 => QuantumGateCapsule::hadamard(i % 8),
                1 => QuantumGateCapsule::pauli_x(i % 8),
                2 => QuantumGateCapsule::pauli_y(i % 8),
                3 => QuantumGateCapsule::pauli_z(i % 8),
                4 => QuantumGateCapsule::s_gate(i % 8),
                5 => QuantumGateCapsule::t_gate(i % 8),
                _ => unreachable!(),
            };
            circuit.add_gate(gate).unwrap();
        }

        b.iter(|| {
            let mut circuit_copy = QuantumCircuitCapsule::new(8).unwrap();
            for i in 0..1000 {
                let gate = match i % 6 {
                    0 => QuantumGateCapsule::hadamard(i % 8),
                    1 => QuantumGateCapsule::pauli_x(i % 8),
                    2 => QuantumGateCapsule::pauli_y(i % 8),
                    3 => QuantumGateCapsule::pauli_z(i % 8),
                    4 => QuantumGateCapsule::s_gate(i % 8),
                    5 => QuantumGateCapsule::t_gate(i % 8),
                    _ => unreachable!(),
                };
                circuit_copy.add_gate(gate).unwrap();
            }
            circuit_copy.execute_parallel().unwrap();
            black_box(circuit_copy.execution_time_ns());
        });
    });

    group.finish();
}

#[cfg(all(feature = "quantum-pure", feature = "rayon"))]
fn benchmark_dependency_layering(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum_circuit_dependency_layering");

    // Benchmark dependency graph construction overhead
    for gate_count in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(gate_count),
            gate_count,
            |b, &gate_count| {
                let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
                for i in 0..gate_count {
                    circuit.add_gate(QuantumGateCapsule::hadamard(i % 8)).unwrap();
                }

                b.iter(|| {
                    black_box(circuit.build_dependency_layers());
                });
            },
        );
    }

    group.finish();
}

#[cfg(all(feature = "quantum-pure", feature = "rayon"))]
fn benchmark_parallelism_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum_circuit_parallelism_efficiency");

    // Benchmark different qubit counts (affects parallelism)
    for num_qubits in [4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::new("sequential", num_qubits),
            num_qubits,
            |b, &num_qubits| {
                let mut circuit = QuantumCircuitCapsule::new(num_qubits).unwrap();
                for i in 0..100 {
                    circuit.add_gate(QuantumGateCapsule::hadamard(i % num_qubits)).unwrap();
                }

                b.iter(|| {
                    let mut circuit_copy = QuantumCircuitCapsule::new(num_qubits).unwrap();
                    for i in 0..100 {
                        circuit_copy.add_gate(QuantumGateCapsule::hadamard(i % num_qubits)).unwrap();
                    }
                    circuit_copy.execute().unwrap();
                    black_box(circuit_copy.execution_time_ns());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("parallel", num_qubits),
            num_qubits,
            |b, &num_qubits| {
                let mut circuit = QuantumCircuitCapsule::new(num_qubits).unwrap();
                for i in 0..100 {
                    circuit.add_gate(QuantumGateCapsule::hadamard(i % num_qubits)).unwrap();
                }

                b.iter(|| {
                    let mut circuit_copy = QuantumCircuitCapsule::new(num_qubits).unwrap();
                    for i in 0..100 {
                        circuit_copy.add_gate(QuantumGateCapsule::hadamard(i % num_qubits)).unwrap();
                    }
                    circuit_copy.execute_parallel().unwrap();
                    black_box(circuit_copy.execution_time_ns());
                });
            },
        );
    }

    group.finish();
}

#[cfg(all(feature = "quantum-pure", feature = "rayon"))]
criterion_group!(
    benches,
    benchmark_sequential_vs_parallel,
    benchmark_dependency_layering,
    benchmark_parallelism_efficiency
);

#[cfg(not(all(feature = "quantum-pure", feature = "rayon")))]
criterion_group!(benches,);

criterion_main!(benches);
