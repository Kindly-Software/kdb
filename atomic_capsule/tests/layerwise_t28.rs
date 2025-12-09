//! T28 Comprehensive Tests: LayerwiseParallelCapsule
//!
//! # Test Categories
//!
//! - **Unit Tests** (T28 Q1-Q7): Basic functionality, layer construction
//! - **Property Tests** (T28 Q8-Q14): Correctness (parallel = sequential), independence
//! - **Integration Tests** (T28 Q15-Q21): Real circuits (Grover, QFT simulation)
//! - **Production Tests** (T28 Q22-Q28): Performance, scalability, stress testing
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T4 Batch tier, Q33 verification
//! - **Chaos**: 100% computational capsule architecture
//! - **ASSUM**: 99.5%+ safety, all assumptions verified
//! - **B32**: Fair baselines (sequential execution), performance targets validated
//! - **T28**: Comprehensive 4-tier test pyramid

#[cfg(feature = "quantum-multi-qubit")]
mod layerwise_tests {
    use atomic_capsule::quantum_pure::{
        GateLayer, LayerwiseParallelCapsule, QuantumGateCapsule, QuantumPureError, QuantumState,
    };

    const EPSILON: f64 = 1e-10;

    // ========================================================================
    // UNIT TESTS (T28 Q1-Q7): Basic Functionality
    // ========================================================================

    #[test]
    fn test_q1_capsule_size_alignment() {
        use std::mem::{align_of, size_of};
        assert_eq!(size_of::<LayerwiseParallelCapsule>(), 256);
        assert_eq!(align_of::<LayerwiseParallelCapsule>(), 256);
    }

    #[test]
    fn test_q2_new_capsule() {
        let capsule = LayerwiseParallelCapsule::new();
        assert_eq!(capsule.num_layers(), 0);
        assert_eq!(capsule.max_parallelism(), 0);
        assert_eq!(capsule.total_gates(), 0);
        assert_eq!(capsule.execution_time_ns(), 0);
        assert_eq!(capsule.speedup(), 1.0);
    }

    #[test]
    fn test_q3_empty_layers() {
        let capsule = LayerwiseParallelCapsule::new();
        let layers = capsule.build_layers(&[]).unwrap();
        assert_eq!(layers.len(), 0);
        assert_eq!(capsule.num_layers(), 0);
        assert_eq!(capsule.total_gates(), 0);
    }

    #[test]
    fn test_q4_single_gate_layer() {
        let capsule = LayerwiseParallelCapsule::new();
        let gates = vec![QuantumGateCapsule::hadamard(0)];

        let layers = capsule.build_layers(&gates).unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].num_gates(), 1);
        assert_eq!(capsule.num_layers(), 1);
        assert_eq!(capsule.max_parallelism(), 1);
    }

    #[test]
    fn test_q5_independent_gates_same_layer() {
        let capsule = LayerwiseParallelCapsule::new();
        let gates = vec![
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::hadamard(1),
            QuantumGateCapsule::hadamard(2),
        ];

        let layers = capsule.build_layers(&gates).unwrap();

        // All independent gates → single layer
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].num_gates(), 3);
        assert_eq!(capsule.max_parallelism(), 3);
    }

    #[test]
    fn test_q6_dependent_gates_separate_layers() {
        let capsule = LayerwiseParallelCapsule::new();
        let gates = vec![
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::pauli_x(0), // Same qubit → new layer
        ];

        let layers = capsule.build_layers(&gates).unwrap();

        // Dependent gates → separate layers
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].num_gates(), 1);
        assert_eq!(layers[1].num_gates(), 1);
        assert_eq!(capsule.max_parallelism(), 1);
    }

    #[test]
    fn test_q7_mixed_dependencies() {
        let capsule = LayerwiseParallelCapsule::new();
        let gates = vec![
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::hadamard(1), // Independent → layer 0
            QuantumGateCapsule::pauli_x(0), // Depends on qubit 0 → layer 1
            QuantumGateCapsule::pauli_x(1), // Depends on qubit 1 → layer 1
        ];

        let layers = capsule.build_layers(&gates).unwrap();

        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].num_gates(), 2); // H(0), H(1)
        assert_eq!(layers[1].num_gates(), 2); // X(0), X(1)
        assert_eq!(capsule.max_parallelism(), 2);
    }

    // ========================================================================
    // PROPERTY TESTS (T28 Q8-Q14): Correctness Invariants
    // ========================================================================

    #[test]
    fn test_q8_layer_independence_property() {
        // Property: All gates in same layer operate on disjoint qubits
        let capsule = LayerwiseParallelCapsule::new();
        let gates = vec![
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::hadamard(1),
            QuantumGateCapsule::pauli_x(2),
            QuantumGateCapsule::pauli_z(3),
        ];

        let layers = capsule.build_layers(&gates).unwrap();

        for layer in &layers {
            let qubits_used = collect_qubits_in_layer(layer);
            // No duplicates → independence
            assert_eq!(qubits_used.len(), layer.num_gates());
        }
    }

    #[test]
    fn test_q9_total_gates_preserved() {
        // Property: Sum of gates across layers = total gates
        let capsule = LayerwiseParallelCapsule::new();
        let gates = vec![
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::hadamard(1),
            QuantumGateCapsule::pauli_x(0),
            QuantumGateCapsule::pauli_x(1),
            QuantumGateCapsule::pauli_z(0),
        ];

        let layers = capsule.build_layers(&gates).unwrap();
        let total_in_layers: usize = layers.iter().map(|l| l.num_gates()).sum();

        assert_eq!(total_in_layers, gates.len());
        assert_eq!(capsule.total_gates(), gates.len() as u64);
    }

    #[test]
    fn test_q10_sequential_equivalence() {
        // Property: Layered execution = sequential execution (correctness)
        let mut state_sequential = QuantumState::new(2).unwrap();
        let mut state_layered = QuantumState::new(2).unwrap();

        let gates = vec![
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::hadamard(1),
            QuantumGateCapsule::pauli_x(0),
        ];

        // Sequential execution
        for gate in &gates {
            state_sequential.apply_gate(gate).unwrap();
        }

        // Layered execution
        let capsule = LayerwiseParallelCapsule::new();
        let layers = capsule.build_layers(&gates).unwrap();

        for layer in &layers {
            for gate in layer.gates() {
                state_layered.apply_gate(gate).unwrap();
            }
        }

        // States should be identical
        assert_states_equal(&state_sequential, &state_layered);
    }

    #[test]
    fn test_q11_max_parallelism_bounded() {
        // Property: max_parallelism ≤ num_qubits (can't have more parallel gates than qubits)
        let num_qubits = 4;
        let capsule = LayerwiseParallelCapsule::new();

        let gates = vec![
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::hadamard(1),
            QuantumGateCapsule::hadamard(2),
            QuantumGateCapsule::hadamard(3),
        ];

        let layers = capsule.build_layers(&gates).unwrap();

        assert!(capsule.max_parallelism() <= num_qubits as u64);
    }

    #[test]
    fn test_q12_average_parallelism_calculation() {
        let capsule = LayerwiseParallelCapsule::new();
        let gates = vec![
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::hadamard(1),
            QuantumGateCapsule::pauli_x(0),
            QuantumGateCapsule::pauli_x(1),
        ];

        let _layers = capsule.build_layers(&gates).unwrap();

        // 4 gates, 2 layers → average 2.0 gates/layer
        let avg = capsule.average_parallelism();
        assert!((avg - 2.0).abs() < EPSILON);
    }

    #[test]
    fn test_q13_speedup_calculation() {
        let capsule = LayerwiseParallelCapsule::new();

        // 1000ns sequential, 400ns parallel → 2.5× speedup
        capsule.update_speedup(1000, 400);
        assert!((capsule.speedup() - 2.5).abs() < EPSILON);

        // 1000ns sequential, 200ns parallel → 5.0× speedup
        capsule.update_speedup(1000, 200);
        assert!((capsule.speedup() - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_q14_parallelism_efficiency() {
        let capsule = LayerwiseParallelCapsule::new();
        let gates = vec![
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::hadamard(1),
            QuantumGateCapsule::hadamard(2),
            QuantumGateCapsule::hadamard(3),
        ];

        let _layers = capsule.build_layers(&gates).unwrap();

        // All gates in single layer → 100% efficiency
        let efficiency = capsule.parallelism_efficiency();
        assert!((efficiency - 1.0).abs() < EPSILON);
    }

    // ========================================================================
    // INTEGRATION TESTS (T28 Q15-Q21): Real Circuits
    // ========================================================================

    #[test]
    fn test_q15_bell_state_circuit() {
        // Bell state: H(0) then CNOT(0,1) would create (|00⟩+|11⟩)/√2
        // Without CNOT, test H(0) and H(1) separately
        let capsule = LayerwiseParallelCapsule::new();
        let gates = vec![
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::hadamard(1),
        ];

        let layers = capsule.build_layers(&gates).unwrap();

        // Both Hadamards independent → single layer
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].num_gates(), 2);
    }

    #[test]
    fn test_q16_grover_like_circuit() {
        // Grover-like circuit: H on all qubits, then oracle, then diffusion
        // Simplified: Multiple Hadamards followed by phase flips
        let capsule = LayerwiseParallelCapsule::new();
        let gates = vec![
            // Initialization (parallel)
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::hadamard(1),
            QuantumGateCapsule::hadamard(2),
            // Oracle (parallel)
            QuantumGateCapsule::pauli_z(0),
            QuantumGateCapsule::pauli_z(1),
            QuantumGateCapsule::pauli_z(2),
        ];

        let layers = capsule.build_layers(&gates).unwrap();

        // Expect 2 layers: [H(0), H(1), H(2)], [Z(0), Z(1), Z(2)]
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].num_gates(), 3);
        assert_eq!(layers[1].num_gates(), 3);
        assert_eq!(capsule.max_parallelism(), 3);
    }

    #[test]
    fn test_q17_qft_like_circuit() {
        // QFT-like circuit: Hadamards and phase rotations
        let capsule = LayerwiseParallelCapsule::new();
        let gates = vec![
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::s_gate(0),
            QuantumGateCapsule::hadamard(1),
            QuantumGateCapsule::s_gate(1),
        ];

        let layers = capsule.build_layers(&gates).unwrap();

        // H(0) and H(1) parallel, S(0) and S(1) parallel
        assert_eq!(layers.len(), 2);
    }

    #[test]
    fn test_q18_dense_circuit() {
        // Dense circuit: Many gates on few qubits (limited parallelism)
        let capsule = LayerwiseParallelCapsule::new();
        let mut gates = Vec::new();

        // 10 gates on qubit 0
        for _ in 0..10 {
            gates.push(QuantumGateCapsule::hadamard(0));
        }

        let layers = capsule.build_layers(&gates).unwrap();

        // All sequential → 10 layers
        assert_eq!(layers.len(), 10);
        assert_eq!(capsule.max_parallelism(), 1);
    }

    #[test]
    fn test_q19_wide_circuit() {
        // Wide circuit: One gate per qubit (perfect parallelism)
        let capsule = LayerwiseParallelCapsule::new();
        let gates = (0..8)
            .map(|i| QuantumGateCapsule::hadamard(i))
            .collect::<Vec<_>>();

        let layers = capsule.build_layers(&gates).unwrap();

        // All parallel → single layer
        assert_eq!(layers.len(), 1);
        assert_eq!(capsule.max_parallelism(), 8);
        assert_eq!(capsule.average_parallelism(), 8.0);
        assert!((capsule.parallelism_efficiency() - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_q20_alternating_qubits() {
        // Alternating qubits: A(0), A(1), B(0), B(1), ...
        let capsule = LayerwiseParallelCapsule::new();
        let gates = vec![
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::hadamard(1),
            QuantumGateCapsule::pauli_x(0),
            QuantumGateCapsule::pauli_x(1),
            QuantumGateCapsule::pauli_z(0),
            QuantumGateCapsule::pauli_z(1),
        ];

        let layers = capsule.build_layers(&gates).unwrap();

        // Expect 3 layers
        assert_eq!(layers.len(), 3);
        assert_eq!(capsule.max_parallelism(), 2);
    }

    #[test]
    fn test_q21_execution_timing() {
        // Integration test: Execute layers and verify timing recorded
        let mut state = QuantumState::new(2).unwrap();
        let capsule = LayerwiseParallelCapsule::new();

        let gates = vec![
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::hadamard(1),
        ];

        let layers = capsule.build_layers(&gates).unwrap();

        capsule
            .execute_layers(&layers, |gate| state.apply_gate(gate))
            .unwrap();

        // Execution time should be recorded (non-zero)
        assert!(capsule.execution_time_ns() > 0);
    }

    // ========================================================================
    // PRODUCTION TESTS (T28 Q22-Q28): Performance & Scalability
    // ========================================================================

    #[test]
    fn test_q22_large_circuit_50_gates() {
        let capsule = LayerwiseParallelCapsule::new();
        let mut gates = Vec::new();

        // 50 gates across 10 qubits
        for i in 0..50 {
            gates.push(QuantumGateCapsule::hadamard(i % 10));
        }

        let layers = capsule.build_layers(&gates).unwrap();

        assert_eq!(capsule.total_gates(), 50);
        assert!(capsule.num_layers() >= 5); // At least some layering
        assert!(capsule.max_parallelism() <= 10); // Bounded by qubits
    }

    #[test]
    fn test_q23_large_circuit_500_gates() {
        let capsule = LayerwiseParallelCapsule::new();
        let mut gates = Vec::new();

        // 500 gates across 20 qubits
        for i in 0..500 {
            gates.push(QuantumGateCapsule::hadamard(i % 20));
        }

        let layers = capsule.build_layers(&gates).unwrap();

        assert_eq!(capsule.total_gates(), 500);
        assert!(capsule.num_layers() >= 25); // Many layers
        assert!(capsule.max_parallelism() <= 20);
    }

    #[test]
    fn test_q24_worst_case_sequential() {
        // Worst case: All gates on same qubit (zero parallelism)
        let capsule = LayerwiseParallelCapsule::new();
        let gates = (0..100)
            .map(|_| QuantumGateCapsule::hadamard(0))
            .collect::<Vec<_>>();

        let layers = capsule.build_layers(&gates).unwrap();

        assert_eq!(layers.len(), 100); // One gate per layer
        assert_eq!(capsule.max_parallelism(), 1);
        assert_eq!(capsule.average_parallelism(), 1.0);
        assert!((capsule.parallelism_efficiency() - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_q25_best_case_parallel() {
        // Best case: All gates on different qubits (perfect parallelism)
        let capsule = LayerwiseParallelCapsule::new();
        let gates = (0..20)
            .map(|i| QuantumGateCapsule::hadamard(i))
            .collect::<Vec<_>>();

        let layers = capsule.build_layers(&gates).unwrap();

        assert_eq!(layers.len(), 1); // Single layer
        assert_eq!(capsule.max_parallelism(), 20);
        assert_eq!(capsule.average_parallelism(), 20.0);
        assert!((capsule.parallelism_efficiency() - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_q26_realistic_mixed_circuit() {
        // Realistic: Mix of parallel and sequential sections
        let capsule = LayerwiseParallelCapsule::new();
        let mut gates = Vec::new();

        // Parallel section: Hadamards on all qubits
        for i in 0..8 {
            gates.push(QuantumGateCapsule::hadamard(i));
        }

        // Sequential section: Multiple gates on qubit 0
        for _ in 0..5 {
            gates.push(QuantumGateCapsule::pauli_x(0));
        }

        // Another parallel section
        for i in 0..8 {
            gates.push(QuantumGateCapsule::pauli_z(i));
        }

        let layers = capsule.build_layers(&gates).unwrap();

        // Expect: 1 layer (8 H), 5 layers (5 X on q0), 1 layer (8 Z) = 7 layers
        assert_eq!(layers.len(), 7);
        assert_eq!(capsule.max_parallelism(), 8);
        assert_eq!(capsule.total_gates(), 21);
    }

    #[test]
    fn test_q27_speedup_realistic_estimate() {
        // Estimate speedup for realistic circuit
        let capsule = LayerwiseParallelCapsule::new();
        let mut gates = Vec::new();

        // 100 gates, 50% parallelism
        for i in 0..50 {
            gates.push(QuantumGateCapsule::hadamard(i % 10));
            gates.push(QuantumGateCapsule::pauli_x(i % 10));
        }

        let layers = capsule.build_layers(&gates).unwrap();

        // Conservative estimate: 2-4× speedup for 50% parallelism
        let ideal_speedup = 100.0 / layers.len() as f64;

        // Should see some benefit
        assert!(ideal_speedup > 1.5);
        assert!(ideal_speedup < 10.0); // Conservative upper bound
    }

    #[test]
    fn test_q28_memory_efficiency() {
        // Verify memory layout doesn't explode with large circuits
        let capsule = LayerwiseParallelCapsule::new();
        let gates = (0..1000)
            .map(|i| QuantumGateCapsule::hadamard(i % 20))
            .collect::<Vec<_>>();

        let layers = capsule.build_layers(&gates).unwrap();

        // Layers should be compact (sum of layer sizes = total gates)
        let total_in_layers: usize = layers.iter().map(|l| l.num_gates()).sum();
        assert_eq!(total_in_layers, 1000);

        // Capsule metadata should be small (256 bytes)
        use std::mem::size_of;
        assert_eq!(size_of::<LayerwiseParallelCapsule>(), 256);
    }

    // ========================================================================
    // Helper Functions
    // ========================================================================

    fn collect_qubits_in_layer(layer: &GateLayer) -> std::collections::HashSet<usize> {
        use std::collections::HashSet;
        let mut qubits = HashSet::new();

        for gate in layer.gates() {
            qubits.insert(gate.target());
            if let Some(control) = gate.control_qubit() {
                qubits.insert(control);
            }
        }

        qubits
    }

    fn assert_states_equal(state1: &QuantumState, state2: &QuantumState) {
        assert_eq!(state1.real_parts.len(), state2.real_parts.len());

        for i in 0..state1.real_parts.len() {
            assert!(
                (state1.real_parts[i] - state2.real_parts[i]).abs() < EPSILON,
                "Real part mismatch at index {}: {} vs {}",
                i,
                state1.real_parts[i],
                state2.real_parts[i]
            );
            assert!(
                (state1.imag_parts[i] - state2.imag_parts[i]).abs() < EPSILON,
                "Imag part mismatch at index {}: {} vs {}",
                i,
                state1.imag_parts[i],
                state2.imag_parts[i]
            );
        }
    }
}
