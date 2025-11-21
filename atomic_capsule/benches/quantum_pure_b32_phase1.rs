//! B32 Benchmarking - Pure-Capsule Quantum Simulator Phase 1
//!
//! Fair comparative benchmarks against theoretical baselines.
//! Measures SIMD speedup, circuit execution, and scaling characteristics.
//!
//! Benchmark Groups:
//! 1. State initialization (4-20 qubits)
//! 2. Hadamard gate (SIMD vs theoretical scalar)
//! 3. Sequential gates (10-100 gates)
//! 4. Measurement sampling (1000 samples)
//! 5. SIMD speedup verification

use atomic_capsule::quantum_pure::{
    QuantumCircuitCapsule, QuantumGateCapsule, QuantumStateVectorCapsule,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

// ============================================================================
// Group 1: State Initialization
// ============================================================================

fn bench_state_initialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_initialization");

    for qubits in [4, 8, 12, 16, 20] {
        group.bench_with_input(BenchmarkId::new("pure_capsule", qubits), &qubits, |b, &qubits| {
            b.iter(|| {
                let state = QuantumStateVectorCapsule::new(qubits).unwrap();
                black_box(state);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Group 2: Hadamard Gate (SIMD Optimization)
// ============================================================================

fn bench_hadamard_gate(c: &mut Criterion) {
    let mut group = c.benchmark_group("hadamard_gate_simd");

    for qubits in [4, 8, 12, 16] {
        let mut state = QuantumStateVectorCapsule::new(qubits).unwrap();
        let h_gate = QuantumGateCapsule::hadamard(0);

        group.bench_with_input(
            BenchmarkId::new("simd_optimized", qubits),
            &qubits,
            |b, _| {
                b.iter(|| {
                    state.apply_gate(&h_gate).unwrap();
                    black_box(&state);
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Group 3: Sequential Gates (Circuit Depth)
// ============================================================================

fn bench_sequential_gates(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequential_gates");

    for gate_count in [10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("8_qubits", gate_count),
            &gate_count,
            |b, &gate_count| {
                b.iter_batched(
                    || {
                        let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
                        for i in 0..gate_count {
                            let gate = match i % 6 {
                                0 => QuantumGateCapsule::hadamard(i % 8),
                                1 => QuantumGateCapsule::pauli_x(i % 8),
                                2 => QuantumGateCapsule::pauli_y(i % 8),
                                3 => QuantumGateCapsule::pauli_z(i % 8),
                                4 => QuantumGateCapsule::s_gate(i % 8),
                                _ => QuantumGateCapsule::t_gate(i % 8),
                            };
                            circuit.add_gate(gate).unwrap();
                        }
                        circuit
                    },
                    |mut circuit| {
                        circuit.execute().unwrap();
                        black_box(circuit);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

// ============================================================================
// Group 4: Measurement Sampling
// ============================================================================

fn bench_measurement_sampling(c: &mut Criterion) {
    let mut group = c.benchmark_group("measurement_sampling");

    group.bench_function("1000_samples_superposition", |b| {
        b.iter_batched(
            || {
                let mut state = QuantumStateVectorCapsule::new(8).unwrap();
                state
                    .apply_gate(&QuantumGateCapsule::hadamard(0))
                    .unwrap();
                state
            },
            |mut state| {
                for _ in 0..1000 {
                    let result = state.measure(0).unwrap();
                    black_box(result);
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("measure_all_8_qubits", |b| {
        b.iter_batched(
            || {
                let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
                for i in 0..8 {
                    circuit
                        .add_gate(QuantumGateCapsule::hadamard(i))
                        .unwrap();
                }
                circuit.execute().unwrap();
                circuit
            },
            |mut circuit| {
                let result = circuit.measure_all().unwrap();
                black_box(result);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ============================================================================
// Group 5: SIMD Speedup Verification
// ============================================================================

fn bench_simd_speedup(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_speedup_verification");

    // Benchmark with 16 qubits (65K amplitudes, optimal for SIMD)
    let mut state = QuantumStateVectorCapsule::new(16).unwrap();

    group.bench_function("hadamard_16_qubits_simd", |b| {
        let h_gate = QuantumGateCapsule::hadamard(0);
        b.iter(|| {
            state.apply_gate(&h_gate).unwrap();
            black_box(&state);
        });
    });

    // Multi-gate sequence to amortize overhead
    group.bench_function("10_gates_16_qubits", |b| {
        b.iter_batched(
            || QuantumStateVectorCapsule::new(16).unwrap(),
            |mut state| {
                for i in 0..10 {
                    let gate = QuantumGateCapsule::hadamard(i % 16);
                    state.apply_gate(&gate).unwrap();
                }
                black_box(state);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ============================================================================
// Group 6: Circuit Construction Overhead
// ============================================================================

fn bench_circuit_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_overhead");

    group.bench_function("create_circuit_8_qubits", |b| {
        b.iter(|| {
            let circuit = QuantumCircuitCapsule::new(8).unwrap();
            black_box(circuit);
        });
    });

    group.bench_function("add_100_gates", |b| {
        b.iter_batched(
            || QuantumCircuitCapsule::new(8).unwrap(),
            |mut circuit| {
                for i in 0..100 {
                    circuit
                        .add_gate(QuantumGateCapsule::hadamard(i % 8))
                        .unwrap();
                }
                black_box(circuit);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("execute_empty_circuit", |b| {
        b.iter_batched(
            || QuantumCircuitCapsule::new(8).unwrap(),
            |mut circuit| {
                circuit.execute().unwrap();
                black_box(circuit);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ============================================================================
// Group 7: Scaling Characteristics
// ============================================================================

fn bench_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_characteristics");

    // Fixed 10 gates, varying qubit count
    for qubits in [4, 8, 12, 16] {
        group.bench_with_input(
            BenchmarkId::new("10_gates_varying_qubits", qubits),
            &qubits,
            |b, &qubits| {
                b.iter_batched(
                    || {
                        let mut circuit = QuantumCircuitCapsule::new(qubits).unwrap();
                        for i in 0..10 {
                            circuit
                                .add_gate(QuantumGateCapsule::hadamard(i % qubits as usize))
                                .unwrap();
                        }
                        circuit
                    },
                    |mut circuit| {
                        circuit.execute().unwrap();
                        black_box(circuit);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    // Fixed 8 qubits, varying gate count
    for gates in [10, 20, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("8_qubits_varying_gates", gates),
            &gates,
            |b, &gates| {
                b.iter_batched(
                    || {
                        let mut circuit = QuantumCircuitCapsule::new(8).unwrap();
                        for i in 0..gates {
                            circuit
                                .add_gate(QuantumGateCapsule::hadamard(i % 8))
                                .unwrap();
                        }
                        circuit
                    },
                    |mut circuit| {
                        circuit.execute().unwrap();
                        black_box(circuit);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

// ============================================================================
// Group 8: Gate Type Comparison
// ============================================================================

fn bench_gate_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_types");

    let mut state = QuantumStateVectorCapsule::new(8).unwrap();

    let gates = vec![
        ("hadamard", QuantumGateCapsule::hadamard(0)),
        ("pauli_x", QuantumGateCapsule::pauli_x(0)),
        ("pauli_y", QuantumGateCapsule::pauli_y(0)),
        ("pauli_z", QuantumGateCapsule::pauli_z(0)),
        ("s_gate", QuantumGateCapsule::s_gate(0)),
        ("t_gate", QuantumGateCapsule::t_gate(0)),
    ];

    for (name, gate) in gates {
        group.bench_function(name, |b| {
            b.iter(|| {
                state.apply_gate(&gate).unwrap();
                black_box(&state);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_state_initialization,
    bench_hadamard_gate,
    bench_sequential_gates,
    bench_measurement_sampling,
    bench_simd_speedup,
    bench_circuit_overhead,
    bench_scaling,
    bench_gate_types
);
criterion_main!(benches);
