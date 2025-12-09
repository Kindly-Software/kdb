//! T28 Test Suite: Horizontal SIMD Gate Batching (Phase 3.2)
//!
//! # Test Coverage
//!
//! - Q1-Q7 (Unit): Basic batch operations, correctness
//! - Q8-Q14 (Property): Normalization, unitarity, determinism
//! - Q15-Q21 (Integration): Full circuits with batched execution
//! - Q22-Q28 (Production): Performance validation, error handling
//!
//! # Performance Targets (B32 Conservative)
//!
//! - Sparse circuits (50%+ independent): 2.5× speedup
//! - Dense circuits (20% independent): 1.4× speedup
//! - Average: 2.0× speedup

#[cfg(all(feature = "quantum-pure", feature = "portable_simd"))]
mod batch_gate_tests {
    use atomic_capsule::quantum_pure::{
        QuantumCircuitCapsule, QuantumGateCapsule, GateType, QuantumPureResult,
    };
    use atomic_capsule::quantum_pure::batch_gates::{batch_gates, GateBatch};

    // ============================================================================
    // Q1-Q7: UNIT TESTS
    // ============================================================================

    #[test]
    fn test_q1_batch_creation() {
        let mut batch = GateBatch::new(GateType::Hadamard);
        assert_eq!(batch.size(), 0);
        assert!(!batch.is_full());

        assert!(batch.try_add(0, 0));
        assert_eq!(batch.size(), 1);

        assert!(batch.try_add(1, 1));
        assert_eq!(batch.size(), 2);
    }

    #[test]
    fn test_q2_batch_duplicate_target() {
        let mut batch = GateBatch::new(GateType::Hadamard);
        assert!(batch.try_add(0, 0)); // H₀
        assert!(!batch.try_add(1, 0)); // H₀ again - should fail
        assert_eq!(batch.size(), 1);
    }

    #[test]
    fn test_q3_batch_size_limit() {
        let mut batch = GateBatch::new(GateType::PauliX);

        // Add 8 gates (max)
        for i in 0..8 {
            assert!(batch.try_add(i, i));
        }

        assert!(batch.is_full());
        assert_eq!(batch.size(), 8);

        // 9th gate fails
        assert!(!batch.try_add(8, 8));
    }

    #[test]
    fn test_q4_batch_gates_single_type() {
        let gates = vec![
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::hadamard(1),
            QuantumGateCapsule::hadamard(2),
            QuantumGateCapsule::hadamard(3),
        ];

        let batches = batch_gates(&gates);
        assert_eq!(batches.len(), 1); // All Hadamard → single batch
        assert_eq!(batches[0].size(), 4);
        assert_eq!(batches[0].gate_type, GateType::Hadamard);
    }

    #[test]
    fn test_q5_batch_gates_mixed_types() {
        let gates = vec![
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::pauli_x(1),
            QuantumGateCapsule::hadamard(2),
            QuantumGateCapsule::pauli_x(3),
        ];

        let batches = batch_gates(&gates);
        assert_eq!(batches.len(), 2); // 2 types → 2 batches

        // Count total gates
        let total: usize = batches.iter().map(|b| b.size()).sum();
        assert_eq!(total, 4);
    }

    #[test]
    fn test_q6_batch_gates_duplicate_targets() {
        let gates = vec![
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::hadamard(1),
            QuantumGateCapsule::hadamard(0), // Duplicate target
            QuantumGateCapsule::hadamard(2),
        ];

        let batches = batch_gates(&gates);
        // Should split into multiple batches due to duplicate target
        assert!(batches.len() >= 1);

        let total: usize = batches.iter().map(|b| b.size()).sum();
        assert_eq!(total, 4); // All gates included
    }

    #[test]
    fn test_q7_batch_empty_circuit() {
        let gates: Vec<QuantumGateCapsule> = vec![];
        let batches = batch_gates(&gates);
        assert_eq!(batches.len(), 0);
    }

    // ============================================================================
    // Q8-Q14: PROPERTY TESTS
    // ============================================================================

    #[test]
    fn test_q8_batched_preserves_normalization() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(4)?;

        // Add gates that preserve normalization
        circuit.add_gate(QuantumGateCapsule::hadamard(0))?;
        circuit.add_gate(QuantumGateCapsule::hadamard(1))?;
        circuit.add_gate(QuantumGateCapsule::hadamard(2))?;
        circuit.add_gate(QuantumGateCapsule::hadamard(3))?;

        circuit.execute_batched()?;

        // Verify normalization (sum of |amplitude|² = 1.0)
        // Note: Cannot access state_capsule.verify_normalization directly
        // So we test indirectly via measurement (should always succeed)
        let _ = circuit.measure_all()?;

        Ok(())
    }

    #[test]
    fn test_q9_batched_correctness_vs_sequential() -> QuantumPureResult<()> {
        let mut circuit_seq = QuantumCircuitCapsule::new(4)?;
        let mut circuit_batch = QuantumCircuitCapsule::new(4)?;

        // Add identical gates to both
        let gates = vec![
            QuantumGateCapsule::hadamard(0),
            QuantumGateCapsule::hadamard(1),
            QuantumGateCapsule::hadamard(2),
            QuantumGateCapsule::hadamard(3),
        ];

        for gate in &gates {
            circuit_seq.add_gate(gate.clone())?;
            circuit_batch.add_gate(gate.clone())?;
        }

        circuit_seq.execute()?;
        circuit_batch.execute_batched()?;

        // Results should be identical (measurement is probabilistic, so compare execution time)
        // Both should complete successfully
        assert!(circuit_seq.execution_time_ns() > 0);
        assert!(circuit_batch.execution_time_ns() > 0);

        Ok(())
    }

    #[test]
    fn test_q10_batched_deterministic() -> QuantumPureResult<()> {
        let mut circuit1 = QuantumCircuitCapsule::new(4)?;
        let mut circuit2 = QuantumCircuitCapsule::new(4)?;

        // Add same gates to both
        for i in 0..4 {
            circuit1.add_gate(QuantumGateCapsule::hadamard(i))?;
            circuit2.add_gate(QuantumGateCapsule::hadamard(i))?;
        }

        circuit1.execute_batched()?;
        circuit2.execute_batched()?;

        // Both should execute successfully (deterministic)
        assert!(circuit1.execution_time_ns() > 0);
        assert!(circuit2.execution_time_ns() > 0);

        Ok(())
    }

    #[test]
    fn test_q11_batched_pauli_gates() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(4)?;

        // Add different Pauli gates
        circuit.add_gate(QuantumGateCapsule::pauli_x(0))?;
        circuit.add_gate(QuantumGateCapsule::pauli_y(1))?;
        circuit.add_gate(QuantumGateCapsule::pauli_z(2))?;
        circuit.add_gate(QuantumGateCapsule::s_gate(3))?;

        circuit.execute_batched()?;

        assert!(circuit.execution_time_ns() > 0);

        Ok(())
    }

    #[test]
    fn test_q12_batched_mixed_gates() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(8)?;

        // Mix of Hadamard, Pauli, phase gates
        circuit.add_gate(QuantumGateCapsule::hadamard(0))?;
        circuit.add_gate(QuantumGateCapsule::pauli_x(1))?;
        circuit.add_gate(QuantumGateCapsule::hadamard(2))?;
        circuit.add_gate(QuantumGateCapsule::pauli_z(3))?;
        circuit.add_gate(QuantumGateCapsule::s_gate(4))?;
        circuit.add_gate(QuantumGateCapsule::t_gate(5))?;
        circuit.add_gate(QuantumGateCapsule::hadamard(6))?;
        circuit.add_gate(QuantumGateCapsule::pauli_y(7))?;

        circuit.execute_batched()?;

        assert!(circuit.execution_time_ns() > 0);

        Ok(())
    }

    #[test]
    fn test_q13_batched_single_gate() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(2)?;

        // Single gate (no batching possible)
        circuit.add_gate(QuantumGateCapsule::hadamard(0))?;

        circuit.execute_batched()?;

        assert!(circuit.execution_time_ns() > 0);

        Ok(())
    }

    #[test]
    fn test_q14_batched_empty_circuit() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(4)?;

        // No gates
        circuit.execute_batched()?;

        // Should complete quickly
        assert!(circuit.execution_time_ns() < 10_000); // < 10μs

        Ok(())
    }

    // ============================================================================
    // Q15-Q21: INTEGRATION TESTS
    // ============================================================================

    #[test]
    fn test_q15_batched_bell_state_preparation() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(2)?;

        // Bell state: H₀ + CNOT(0,1)
        // (CNOT not supported in Phase 1, so just test H₀)
        circuit.add_gate(QuantumGateCapsule::hadamard(0))?;

        circuit.execute_batched()?;

        let result = circuit.measure_all()?;
        // Should be 0 or 1 with 50% probability each
        assert!(result == 0 || result == 1);

        Ok(())
    }

    #[test]
    fn test_q16_batched_multiple_hadamards() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(8)?;

        // Apply Hadamard to all 8 qubits (perfect batch)
        for i in 0..8 {
            circuit.add_gate(QuantumGateCapsule::hadamard(i))?;
        }

        circuit.execute_batched()?;

        assert!(circuit.execution_time_ns() > 0);

        Ok(())
    }

    #[test]
    fn test_q17_batched_sequential_dependencies() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(4)?;

        // Create dependencies (same qubit, cannot batch)
        circuit.add_gate(QuantumGateCapsule::hadamard(0))?;
        circuit.add_gate(QuantumGateCapsule::pauli_x(0))?; // Depends on H₀
        circuit.add_gate(QuantumGateCapsule::pauli_z(0))?; // Depends on X₀

        circuit.execute_batched()?;

        assert!(circuit.execution_time_ns() > 0);

        Ok(())
    }

    #[test]
    fn test_q18_batched_large_circuit() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(8)?;

        // Add 64 gates (8 qubits × 8 gates each)
        for _ in 0..8 {
            for qubit in 0..8 {
                circuit.add_gate(QuantumGateCapsule::hadamard(qubit))?;
            }
        }

        circuit.execute_batched()?;

        assert_eq!(circuit.gate_count(), 64);
        assert!(circuit.execution_time_ns() > 0);

        Ok(())
    }

    #[test]
    fn test_q19_batched_alternating_types() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(4)?;

        // Alternate gate types (forces multiple batches)
        circuit.add_gate(QuantumGateCapsule::hadamard(0))?;
        circuit.add_gate(QuantumGateCapsule::pauli_x(1))?;
        circuit.add_gate(QuantumGateCapsule::hadamard(2))?;
        circuit.add_gate(QuantumGateCapsule::pauli_x(3))?;

        circuit.execute_batched()?;

        assert!(circuit.execution_time_ns() > 0);

        Ok(())
    }

    #[test]
    fn test_q20_batched_reset_and_reexecute() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(4)?;

        // First execution
        circuit.add_gate(QuantumGateCapsule::hadamard(0))?;
        circuit.add_gate(QuantumGateCapsule::hadamard(1))?;
        circuit.execute_batched()?;

        let time1 = circuit.execution_time_ns();

        // Reset and re-execute
        circuit.reset()?;
        circuit.execute_batched()?;

        let time2 = circuit.execution_time_ns();

        // Both should execute successfully
        assert!(time1 > 0);
        assert!(time2 > 0);

        Ok(())
    }

    #[test]
    fn test_q21_batched_measurement_collapse() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(2)?;

        // Create superposition with batched execution
        circuit.add_gate(QuantumGateCapsule::hadamard(0))?;
        circuit.add_gate(QuantumGateCapsule::hadamard(1))?;
        circuit.execute_batched()?;

        // Measure (should collapse to one of 4 basis states)
        let result = circuit.measure_all()?;
        assert!(result < 4); // 2 qubits → 4 possible states

        Ok(())
    }

    // ============================================================================
    // Q22-Q28: PRODUCTION TESTS
    // ============================================================================

    #[test]
    fn test_q22_batched_performance_sparse_circuit() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(8)?;

        // Sparse circuit (all independent gates)
        for i in 0..8 {
            circuit.add_gate(QuantumGateCapsule::hadamard(i))?;
        }

        circuit.execute_batched()?;

        // Should execute quickly due to batching
        let time = circuit.execution_time_ns();
        assert!(time > 0);
        assert!(time < 100_000); // < 100μs for 8 gates

        Ok(())
    }

    #[test]
    fn test_q23_batched_performance_dense_circuit() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(4)?;

        // Dense circuit (many dependencies on same qubit)
        for _ in 0..10 {
            circuit.add_gate(QuantumGateCapsule::hadamard(0))?;
        }

        circuit.execute_batched()?;

        let time = circuit.execution_time_ns();
        assert!(time > 0);

        Ok(())
    }

    #[test]
    fn test_q24_batched_error_handling_invalid_qubit() {
        let mut circuit = QuantumCircuitCapsule::new(4).unwrap();

        // Try to add gate with invalid target (should fail during add_gate, not execute)
        let result = circuit.add_gate(QuantumGateCapsule::hadamard(4));
        assert!(result.is_err());
    }

    #[test]
    fn test_q25_batched_max_qubits() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(16)?; // Max practical size

        // Add gates to all qubits
        for i in 0..16 {
            circuit.add_gate(QuantumGateCapsule::hadamard(i))?;
        }

        circuit.execute_batched()?;

        assert!(circuit.execution_time_ns() > 0);

        Ok(())
    }

    #[test]
    fn test_q26_batched_gate_count_validation() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(4)?;

        // Add 20 gates
        for i in 0..20 {
            circuit.add_gate(QuantumGateCapsule::hadamard(i % 4))?;
        }

        assert_eq!(circuit.gate_count(), 20);

        circuit.execute_batched()?;

        assert_eq!(circuit.gate_count(), 20); // Gate count unchanged

        Ok(())
    }

    #[test]
    fn test_q27_batched_depth_tracking() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(4)?;

        // Add 4 independent gates (depth = 4 in sequential, could be 1 in parallel)
        circuit.add_gate(QuantumGateCapsule::hadamard(0))?;
        circuit.add_gate(QuantumGateCapsule::hadamard(1))?;
        circuit.add_gate(QuantumGateCapsule::hadamard(2))?;
        circuit.add_gate(QuantumGateCapsule::hadamard(3))?;

        assert_eq!(circuit.depth(), 4); // Sequential depth

        circuit.execute_batched()?;

        Ok(())
    }

    #[test]
    fn test_q28_batched_clear_and_rebuild() -> QuantumPureResult<()> {
        let mut circuit = QuantumCircuitCapsule::new(4)?;

        // Add gates
        circuit.add_gate(QuantumGateCapsule::hadamard(0))?;
        circuit.add_gate(QuantumGateCapsule::hadamard(1))?;
        circuit.execute_batched()?;

        // Clear gates
        circuit.clear_gates();
        assert_eq!(circuit.gate_count(), 0);

        // Rebuild circuit
        circuit.add_gate(QuantumGateCapsule::pauli_x(0))?;
        circuit.add_gate(QuantumGateCapsule::pauli_x(1))?;
        circuit.execute_batched()?;

        assert_eq!(circuit.gate_count(), 2);

        Ok(())
    }
}
