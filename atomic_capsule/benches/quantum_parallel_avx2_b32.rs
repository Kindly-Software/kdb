//! Phase 3.2: Multi-threaded AVX2 Benchmark with ThreadPool (B32 Framework)
//!
//! # Objective
//!
//! Validate **4-8× threading speedup** on top of Phase 3.1's 2.0× AVX2 baseline using ThreadPool.
//!
//! # Benchmarks
//!
//! Compare:
//! - **Baseline**: Single-threaded AVX2 (Phase 3.1, ~2.0× vs scalar)
//! - **Phase 3.2**: Multi-threaded AVX2 (ThreadPool + f64x4)
//! - **Target**: 8-16× total vs scalar (2.0 AVX2 × 4-8 threading)
//!
//! # ThreadPool Migration
//!
//! - **OLD**: Rayon (~300ns spawn overhead, dynamic work-stealing)
//! - **NEW**: atomic_capsule ThreadPool (<20ns spawn, static chunking)
//! - **Threshold**: 18 qubits (262K dims) - below uses single AVX2, above uses parallel
//!
//! # Methodology (B32 Framework)
//!
//! - **Fair baseline**: Single-threaded AVX2 (NOT scalar, to measure threading gain)
//! - **1000+ iterations**: Criterion 95% CI
//! - **Realistic workload**: 20 qubits (1M amplitudes, above 262K threshold)
//! - **Thread scaling**: Fixed by std::thread::available_parallelism()

#![cfg(all(feature = "avx2-simd", target_arch = "x86_64"))]

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use atomic_capsule::quantum_pure::{QuantumState, QuantumGateCapsule};

/// Benchmark single-threaded AVX2 (Phase 3.1 baseline)
///
/// # Method
///
/// - Disable parallelization by using small stride (<100K threshold)
/// - This forces `apply_single_qubit_gate` to use AVX2 non-parallel path
/// - Baseline: ~2.0× vs scalar (from Phase 3.1 results)
fn bench_single_threaded_avx2(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase3.2/single_threaded_avx2");

    // Use 16 qubits = 65K amplitudes
    // Qubit 2 (stride=4) triggers AVX2 but NOT parallelization (stride < 100K)
    let num_qubits = 16;
    let mut state = QuantumState::new(num_qubits).expect("Failed to create state");

    // Standard gate types
    let gates = vec![
        ("hadamard", QuantumGateCapsule::hadamard(2)),
        ("pauli_x", QuantumGateCapsule::pauli_x(2)),
        ("pauli_y", QuantumGateCapsule::pauli_y(2)),
        ("pauli_z", QuantumGateCapsule::pauli_z(2)),
        ("s_gate", QuantumGateCapsule::s_gate(2)),
        ("t_gate", QuantumGateCapsule::t_gate(2)),
    ];

    for (name, gate) in gates {
        group.bench_with_input(
            BenchmarkId::new("avx2_qubit2", name),
            &gate,
            |b, g| {
                b.iter(|| {
                    state.apply_gate(black_box(g)).expect("Gate failed");
                });
            },
        );
    }

    group.finish();
}

/// Benchmark multi-threaded AVX2 (Phase 3.2)
///
/// # Method
///
/// - Use 18 qubits (262K dims, at threshold boundary)
/// - Qubit 17 (stride=131K) → Below 262K threshold, uses single-threaded AVX2
/// - This validates threshold logic works correctly
fn bench_multi_threaded_avx2(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase3.2/multi_threaded_avx2");

    // Use 16 qubits, target qubit 17 would require 17 qubits
    // Instead use 18 qubits (262K amplitudes) with qubit 17 (stride=131K)
    let num_qubits = 18;
    let mut state = QuantumState::new(num_qubits).expect("Failed to create state");

    // Target qubit 17: stride = 2^17 = 131,072 (> 100K threshold)
    let target_qubit = 17;

    let gates = vec![
        ("hadamard", QuantumGateCapsule::hadamard(target_qubit)),
        ("pauli_x", QuantumGateCapsule::pauli_x(target_qubit)),
        ("pauli_y", QuantumGateCapsule::pauli_y(target_qubit)),
        ("pauli_z", QuantumGateCapsule::pauli_z(target_qubit)),
        ("s_gate", QuantumGateCapsule::s_gate(target_qubit)),
        ("t_gate", QuantumGateCapsule::t_gate(target_qubit)),
    ];

    for (name, gate) in gates {
        group.bench_with_input(
            BenchmarkId::new("parallel_avx2_qubit17", name),
            &gate,
            |b, g| {
                b.iter(|| {
                    state.apply_gate(black_box(g)).expect("Gate failed");
                });
            },
        );
    }

    group.finish();
}

/// Benchmark threading scalability with ThreadPool
///
/// # Method
///
/// - Fix gate type (Hadamard) at 18 qubits (262K dims, below threshold)
/// - ThreadPool uses std::thread::available_parallelism() (automatic)
/// - This benchmark validates single-threaded path below threshold
fn bench_thread_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase3.2/thread_scaling");

    let num_qubits = 18; // 262K amplitudes
    let target_qubit = 17; // stride = 131K (triggers parallelization)
    let mut state = QuantumState::new(num_qubits).expect("Failed to create state");
    let gate = QuantumGateCapsule::hadamard(target_qubit);

    // Note: ThreadPool uses std::thread::available_parallelism() automatically
    // Below 262K threshold, this should use single-threaded AVX2 path

    group.bench_function("hadamard_qubit17_threshold_test", |b| {
        b.iter(|| {
            state.apply_gate(black_box(&gate)).expect("Gate failed");
        });
    });

    group.finish();
}

/// Benchmark 100-gate circuit (realistic quantum algorithm)
///
/// # Method
///
/// - Apply 100 Hadamard gates sequentially to different qubits
/// - Half use low-index qubits (AVX2 only), half use high-index (parallel AVX2)
/// - Measure end-to-end latency for realistic workload
fn bench_circuit_100_gates(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase3.2/circuit");

    let num_qubits = 18; // 262K amplitudes
    let mut state = QuantumState::new(num_qubits).expect("Failed to create state");

    // Create 100-gate circuit (50 low-index + 50 high-index qubits)
    let mut circuit = Vec::new();
    for i in 0..50 {
        circuit.push(QuantumGateCapsule::hadamard(i % 10)); // Low-index (AVX2 only)
        circuit.push(QuantumGateCapsule::hadamard(10 + i % 8)); // High-index (parallel AVX2)
    }

    group.bench_function("100_gates_mixed_qubits", |b| {
        b.iter(|| {
            for gate in &circuit {
                state.apply_gate(black_box(gate)).expect("Gate failed");
            }
        });
    });

    group.finish();
}

/// Benchmark 20-qubit state (1M dimensions, ThreadPool validation)
///
/// # Objective
///
/// Validate ThreadPool delivers 4-8× threading gain at 1M dimensions (above 262K threshold).
///
/// # Method
///
/// - 20 qubits = 1,048,576 dimensions (8MB state vector)
/// - Qubit 19 (stride = 512K) → Multi-threaded AVX2 path
/// - Expected: ~250µs @ 8 threads (8× vs single AVX2 ~2ms)
/// - Compare: Single AVX2 ~2ms (2× vs scalar ~4ms)
/// - Total speedup: 16× vs scalar (2.0 AVX2 × 8 threading)
fn bench_multi_threaded_20_qubits(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase3.2/multi_threaded_20qubits");

    let num_qubits = 20; // 1,048,576 dimensions
    let mut state = QuantumState::new(num_qubits).expect("Failed to create state");

    let gates = vec![
        ("hadamard", QuantumGateCapsule::hadamard(19)),
        ("pauli_x", QuantumGateCapsule::pauli_x(19)),
        ("pauli_y", QuantumGateCapsule::pauli_y(19)),
        ("pauli_z", QuantumGateCapsule::pauli_z(19)),
    ];

    for (name, gate) in gates {
        group.bench_with_input(
            BenchmarkId::new("parallel_avx2_qubit19", name),
            &gate,
            |b, g| {
                b.iter(|| {
                    state.apply_gate(black_box(g)).expect("Gate failed");
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_single_threaded_avx2,
    bench_multi_threaded_avx2,
    bench_multi_threaded_20_qubits,
    bench_thread_scaling,
    bench_circuit_100_gates
);
criterion_main!(benches);
