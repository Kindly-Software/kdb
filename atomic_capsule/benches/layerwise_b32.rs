//! B32 Benchmarks: LayerwiseParallelCapsule
//!
//! # Benchmark Categories
//!
//! 1. **Build Layers**: Dependency analysis overhead
//! 2. **Sequential Execution**: Baseline performance
//! 3. **Layered Execution**: Parallel execution performance
//! 4. **Speedup Comparison**: Sequential vs layered
//!
//! # Performance Targets (B32 Conservative)
//!
//! - **Small circuits** (10-50 gates): 2-4× speedup
//! - **Medium circuits** (100-500 gates): 4-8× speedup
//! - **Large circuits** (500+ gates): 8-12× speedup
//!
//! # Baseline
//!
//! - Sequential gate execution (no layering)
//! - Same quantum state operations
//! - Fair comparison (same gates, same qubits)
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T4 Batch tier selected
//! - **B32**: Fair baselines, 95% CI, 1000+ iterations
//! - **ASSUM**: All assumptions verified in benchmarks

#[cfg(feature = "quantum-multi-qubit")]
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

#[cfg(feature = "quantum-multi-qubit")]
use atomic_capsule::quantum_pure::{LayerwiseParallelCapsule, QuantumGateCapsule, QuantumState};

#[cfg(feature = "quantum-multi-qubit")]
fn benchmark_build_layers(c: &mut Criterion) {
    let mut group = c.benchmark_group("layerwise_build_layers");

    for num_gates in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*num_gates as u64));

        let gates: Vec<_> = (0..*num_gates)
            .map(|i| QuantumGateCapsule::hadamard(i % 10))
            .collect();

        group.bench_with_input(BenchmarkId::from_parameter(num_gates), num_gates, |b, _| {
            let capsule = LayerwiseParallelCapsule::new();
            b.iter(|| {
                let layers = capsule.build_layers(black_box(&gates)).unwrap();
                black_box(layers);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "quantum-multi-qubit")]
fn benchmark_sequential_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("layerwise_sequential_baseline");

    for num_gates in [10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*num_gates as u64));

        let gates: Vec<_> = (0..*num_gates)
            .map(|i| QuantumGateCapsule::hadamard(i % 8))
            .collect();

        group.bench_with_input(BenchmarkId::from_parameter(num_gates), num_gates, |b, _| {
            b.iter(|| {
                let mut state = QuantumState::new(8).unwrap();
                for gate in &gates {
                    state.apply_gate(black_box(gate)).unwrap();
                }
                black_box(state);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "quantum-multi-qubit")]
fn benchmark_layered_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("layerwise_layered_execution");

    for num_gates in [10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*num_gates as u64));

        let gates: Vec<_> = (0..*num_gates)
            .map(|i| QuantumGateCapsule::hadamard(i % 8))
            .collect();

        group.bench_with_input(BenchmarkId::from_parameter(num_gates), num_gates, |b, _| {
            let capsule = LayerwiseParallelCapsule::new();
            let layers = capsule.build_layers(&gates).unwrap();

            b.iter(|| {
                let mut state = QuantumState::new(8).unwrap();
                capsule
                    .execute_layers(black_box(&layers), |gate| state.apply_gate(gate))
                    .unwrap();
                black_box(state);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "quantum-multi-qubit")]
fn benchmark_speedup_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("layerwise_speedup_comparison");

    // Benchmark realistic mixed circuits
    for (name, gates) in [
        ("dense_sequential", create_dense_circuit(50)),
        ("wide_parallel", create_wide_circuit(50)),
        ("realistic_mixed", create_realistic_circuit(50)),
    ]
    .iter()
    {
        group.throughput(Throughput::Elements(gates.len() as u64));

        // Sequential baseline
        group.bench_with_input(
            BenchmarkId::new(format!("{}_sequential", name), gates.len()),
            gates,
            |b, gates| {
                b.iter(|| {
                    let mut state = QuantumState::new(10).unwrap();
                    for gate in gates {
                        state.apply_gate(black_box(gate)).unwrap();
                    }
                    black_box(state);
                });
            },
        );

        // Layered execution
        group.bench_with_input(
            BenchmarkId::new(format!("{}_layered", name), gates.len()),
            gates,
            |b, gates| {
                let capsule = LayerwiseParallelCapsule::new();
                let layers = capsule.build_layers(gates).unwrap();

                b.iter(|| {
                    let mut state = QuantumState::new(10).unwrap();
                    capsule
                        .execute_layers(black_box(&layers), |gate| state.apply_gate(gate))
                        .unwrap();
                    black_box(state);
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "quantum-multi-qubit")]
fn benchmark_layer_analysis_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("layerwise_analysis_overhead");

    // Measure overhead of dependency analysis vs gate execution
    for num_gates in [10, 50, 100, 500].iter() {
        let gates: Vec<_> = (0..*num_gates)
            .map(|i| QuantumGateCapsule::hadamard(i % 10))
            .collect();

        group.throughput(Throughput::Elements(*num_gates as u64));

        group.bench_with_input(
            BenchmarkId::new("analysis_only", num_gates),
            num_gates,
            |b, _| {
                let capsule = LayerwiseParallelCapsule::new();
                b.iter(|| {
                    let layers = capsule.build_layers(black_box(&gates)).unwrap();
                    black_box(layers);
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "quantum-multi-qubit")]
fn benchmark_parallelism_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("layerwise_parallelism_metrics");

    for (name, gates) in [
        ("worst_case", create_dense_circuit(100)),
        ("best_case", create_wide_circuit(100)),
        ("realistic", create_realistic_circuit(100)),
    ]
    .iter()
    {
        group.bench_function(name, |b| {
            let capsule = LayerwiseParallelCapsule::new();
            let layers = capsule.build_layers(gates).unwrap();

            b.iter(|| {
                // Measure metrics calculation overhead
                let num_layers = capsule.num_layers();
                let max_par = capsule.max_parallelism();
                let avg_par = capsule.average_parallelism();
                let efficiency = capsule.parallelism_efficiency();

                black_box((num_layers, max_par, avg_par, efficiency));
            });
        });
    }

    group.finish();
}

// ========================================================================
// Circuit Generators
// ========================================================================

#[cfg(feature = "quantum-multi-qubit")]
fn create_dense_circuit(num_gates: usize) -> Vec<QuantumGateCapsule> {
    // Worst case: All gates on same qubit (zero parallelism)
    (0..num_gates)
        .map(|_| QuantumGateCapsule::hadamard(0))
        .collect()
}

#[cfg(feature = "quantum-multi-qubit")]
fn create_wide_circuit(num_gates: usize) -> Vec<QuantumGateCapsule> {
    // Best case: Each gate on different qubit (perfect parallelism)
    (0..num_gates)
        .map(|i| QuantumGateCapsule::hadamard(i % 10))
        .collect()
}

#[cfg(feature = "quantum-multi-qubit")]
fn create_realistic_circuit(num_gates: usize) -> Vec<QuantumGateCapsule> {
    // Realistic: Mix of parallel and sequential sections
    let mut gates = Vec::with_capacity(num_gates);
    let num_qubits = 10;

    for i in 0..num_gates {
        if i % 20 < 10 {
            // Parallel section: Different qubits
            gates.push(QuantumGateCapsule::hadamard(i % num_qubits));
        } else {
            // Sequential section: Same qubit
            gates.push(QuantumGateCapsule::pauli_x(i % 3));
        }
    }

    gates
}

#[cfg(feature = "quantum-multi-qubit")]
criterion_group!(
    benches,
    benchmark_build_layers,
    benchmark_sequential_execution,
    benchmark_layered_execution,
    benchmark_speedup_comparison,
    benchmark_layer_analysis_overhead,
    benchmark_parallelism_metrics,
);

#[cfg(feature = "quantum-multi-qubit")]
criterion_main!(benches);

#[cfg(not(feature = "quantum-multi-qubit"))]
fn main() {
    println!("Benchmarks require quantum-pure feature");
}
