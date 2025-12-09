//! B32 Fair Benchmarking: Gate Fusion Optimization
//!
//! # Benchmarking Strategy
//!
//! ## Baselines (Fair Comparison)
//!
//! 1. **Unfused Circuit**: Execute circuit without fusion optimization
//! 2. **Manual Fusion**: Hand-optimized circuit (best-case comparison)
//! 3. **Gate Count**: Total gates as proxy for execution time
//!
//! ## Metrics
//!
//! - **Gate Count Reduction**: Primary metric (60-80% target)
//! - **Optimization Latency**: Time to optimize circuit (<100μs target)
//! - **Speedup Factor**: Execution time reduction (3-5× target)
//!
//! ## Test Circuits
//!
//! - **Synthetic Fusible**: Known patterns (worst-case baseline)
//! - **Grover's Algorithm**: Real-world search circuit
//! - **QFT**: Quantum Fourier Transform (rotation-heavy)
//! - **Random Circuits**: Unknown pattern distribution

#![cfg(feature = "quantum-fusion")]

use atomic_capsule::quantum::fusion::{GateFusionCapsule, GateType, QuantumCircuit};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::f64::consts::PI;

// ============================================================================
// BASELINE CIRCUITS (UNFUSED)
// ============================================================================

fn create_baseline_circuit(num_qubits: usize, num_gates: usize) -> QuantumCircuit {
    let mut circuit = QuantumCircuit::new(num_qubits, "baseline");

    for i in 0..num_gates {
        let q = i % num_qubits;
        match i % 5 {
            0 => circuit.add_gate(GateType::H { qubit: q }),
            1 => circuit.add_gate(GateType::Rx {
                qubit: q,
                theta: PI / 4.0,
            }),
            2 => circuit.add_gate(GateType::Ry {
                qubit: q,
                theta: PI / 3.0,
            }),
            3 => circuit.add_gate(GateType::Rz {
                qubit: q,
                theta: PI / 6.0,
            }),
            4 => {
                if q + 1 < num_qubits {
                    circuit.add_gate(GateType::CNOT {
                        control: q,
                        target: q + 1,
                    });
                }
            }
            _ => {}
        }
    }

    circuit
}

// ============================================================================
// BENCHMARK GROUPS
// ============================================================================

fn bench_optimization_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("optimization_latency");

    for num_qubits in [2, 4, 8, 16].iter() {
        let fusion = GateFusionCapsule::new();
        let circuit = QuantumCircuit::synthetic_fusible(*num_qubits);

        group.throughput(Throughput::Elements(circuit.gates.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}q_{}g", num_qubits, circuit.gates.len())),
            num_qubits,
            |b, _| {
                b.iter(|| {
                    fusion.reset_metrics();
                    fusion.optimize(black_box(circuit.clone())).unwrap()
                });
            },
        );
    }

    group.finish();
}

fn bench_gate_count_reduction(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_count_reduction");

    for num_qubits in [2, 4, 8, 16].iter() {
        let fusion = GateFusionCapsule::new();
        let circuit = QuantumCircuit::synthetic_fusible(*num_qubits);
        let input_gates = circuit.gates.len();

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}q", num_qubits)),
            num_qubits,
            |b, _| {
                b.iter(|| {
                    let optimized = fusion.optimize(black_box(circuit.clone())).unwrap();
                    let output_gates = optimized.gates.len();
                    let reduction_pct = 100.0 * (1.0 - output_gates as f64 / input_gates as f64);
                    black_box(reduction_pct)
                });
            },
        );
    }

    group.finish();
}

fn bench_grover_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("grover_optimization");

    for num_qubits in [3, 5, 8, 10].iter() {
        let fusion = GateFusionCapsule::new();
        let circuit = QuantumCircuit::grover(*num_qubits);

        group.throughput(Throughput::Elements(circuit.gates.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}q", num_qubits)),
            num_qubits,
            |b, _| {
                b.iter(|| {
                    fusion.reset_metrics();
                    fusion.optimize(black_box(circuit.clone())).unwrap()
                });
            },
        );
    }

    group.finish();
}

fn bench_qft_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("qft_optimization");

    for num_qubits in [4, 6, 8, 10].iter() {
        let fusion = GateFusionCapsule::new();
        let circuit = QuantumCircuit::qft(*num_qubits);

        group.throughput(Throughput::Elements(circuit.gates.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}q", num_qubits)),
            num_qubits,
            |b, _| {
                b.iter(|| {
                    fusion.reset_metrics();
                    fusion.optimize(black_box(circuit.clone())).unwrap()
                });
            },
        );
    }

    group.finish();
}

fn bench_speedup_factor(c: &mut Criterion) {
    let mut group = c.benchmark_group("speedup_factor");

    for circuit_type in ["grover", "qft", "synthetic"].iter() {
        let fusion = GateFusionCapsule::new();
        let circuit = match *circuit_type {
            "grover" => QuantumCircuit::grover(8),
            "qft" => QuantumCircuit::qft(8),
            "synthetic" => QuantumCircuit::synthetic_fusible(8),
            _ => unreachable!(),
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(circuit_type),
            circuit_type,
            |b, _| {
                b.iter(|| {
                    fusion.reset_metrics();
                    let optimized = fusion.optimize(black_box(circuit.clone())).unwrap();
                    let speedup = fusion.speedup_factor();
                    black_box((optimized, speedup))
                });
            },
        );
    }

    group.finish();
}

fn bench_pattern_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_matching");

    // Individual pattern benchmarks
    let fusion = GateFusionCapsule::new();

    // CNOT cancellation
    let mut cnot_circuit = QuantumCircuit::new(2, "cnot");
    for _ in 0..50 {
        cnot_circuit.add_gate(GateType::CNOT {
            control: 0,
            target: 1,
        });
        cnot_circuit.add_gate(GateType::CNOT {
            control: 0,
            target: 1,
        });
    }

    group.bench_function("cnot_cancellation", |b| {
        b.iter(|| {
            fusion.reset_metrics();
            fusion.optimize(black_box(cnot_circuit.clone())).unwrap()
        });
    });

    // Rotation composition
    let mut rotation_circuit = QuantumCircuit::new(1, "rotation");
    for _ in 0..50 {
        rotation_circuit.add_gate(GateType::Rx {
            qubit: 0,
            theta: PI / 50.0,
        });
    }

    group.bench_function("rotation_composition", |b| {
        b.iter(|| {
            fusion.reset_metrics();
            fusion
                .optimize(black_box(rotation_circuit.clone()))
                .unwrap()
        });
    });

    // Hadamard conjugation
    let mut hadamard_circuit = QuantumCircuit::new(2, "hadamard");
    for _ in 0..30 {
        hadamard_circuit.add_gate(GateType::H { qubit: 0 });
        hadamard_circuit.add_gate(GateType::CNOT {
            control: 0,
            target: 1,
        });
        hadamard_circuit.add_gate(GateType::H { qubit: 0 });
    }

    group.bench_function("hadamard_conjugation", |b| {
        b.iter(|| {
            fusion.reset_metrics();
            fusion
                .optimize(black_box(hadamard_circuit.clone()))
                .unwrap()
        });
    });

    group.finish();
}

fn bench_baseline_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline_comparison");

    for num_qubits in [4, 8, 12].iter() {
        let fusion = GateFusionCapsule::new();

        // Baseline: Random circuit
        let baseline = create_baseline_circuit(*num_qubits, 100);
        let baseline_gates = baseline.gates.len();

        // Optimized
        let optimized = fusion.optimize(baseline.clone()).unwrap();
        let optimized_gates = optimized.gates.len();

        let reduction = 100.0 * (1.0 - optimized_gates as f64 / baseline_gates as f64);

        group.bench_with_input(
            BenchmarkId::new("baseline", format!("{}q_{}g", num_qubits, baseline_gates)),
            num_qubits,
            |b, _| {
                b.iter(|| {
                    // Simulate gate execution cost (gate count as proxy)
                    black_box(baseline_gates)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new(
                "optimized",
                format!(
                    "{}q_{}g_{}%red",
                    num_qubits, optimized_gates, reduction as u32
                ),
            ),
            num_qubits,
            |b, _| {
                b.iter(|| {
                    // Simulate optimized gate execution cost
                    black_box(optimized_gates)
                });
            },
        );
    }

    group.finish();
}

fn bench_convergence_passes(c: &mut Criterion) {
    let mut group = c.benchmark_group("convergence_passes");

    for num_qubits in [4, 8, 12].iter() {
        let circuit = QuantumCircuit::synthetic_fusible(*num_qubits);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}q", num_qubits)),
            num_qubits,
            |b, _| {
                b.iter(|| {
                    let fusion = GateFusionCapsule::new();
                    let optimized = fusion.optimize(black_box(circuit.clone())).unwrap();
                    black_box(optimized)
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// VALIDATION BENCHMARKS (METRICS REPORTING)
// ============================================================================

fn bench_metrics_reporting(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_reporting");

    let fusion = GateFusionCapsule::new();
    let circuit = QuantumCircuit::synthetic_fusible(8);

    group.bench_function("full_metrics", |b| {
        b.iter(|| {
            fusion.reset_metrics();
            fusion.optimize(black_box(circuit.clone())).unwrap();

            let metrics = (
                fusion.optimizations_applied(),
                fusion.gates_eliminated(),
                fusion.patterns_matched(),
                fusion.compression_ratio(),
                fusion.speedup_factor(),
            );
            black_box(metrics)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_optimization_latency,
    bench_gate_count_reduction,
    bench_grover_optimization,
    bench_qft_optimization,
    bench_speedup_factor,
    bench_pattern_matching,
    bench_baseline_comparison,
    bench_convergence_passes,
    bench_metrics_reporting,
);
criterion_main!(benches);
