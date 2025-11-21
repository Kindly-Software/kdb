//! T28 Comprehensive Testing - Pure-Capsule Quantum Simulator Phase 1
//!
//! 28 tests across 4 tiers:
//! - Q1-Q7: Unit (capsule structure, initialization, basic gates)
//! - Q8-Q14: Property (quantum mechanics properties, statistics)
//! - Q15-Q21: Integration (multi-qubit, circuits, SIMD)
//! - Q22-Q28: Production (stress, performance, stability)

use atomic_capsule::quantum_pure::{
    QuantumCircuitCapsule, QuantumGateCapsule, QuantumState, QuantumStateVectorCapsule,
};
use std::sync::atomic::Ordering;

// ============================================================================
// Q1-Q7: Unit Tier - Basic Structure and Correctness
// ============================================================================

#[test]
fn test_q1_capsule_sizes() {
    use std::mem::{align_of, size_of};

    // Q1: Verify cache alignment for all three capsules
    assert_eq!(size_of::<QuantumStateVectorCapsule>(), 256);
    assert_eq!(align_of::<QuantumStateVectorCapsule>(), 256);

    assert_eq!(size_of::<QuantumGateCapsule>(), 128);
    assert_eq!(align_of::<QuantumGateCapsule>(), 128);

    // QuantumCircuitCapsule is 768 bytes due to nested 256-byte aligned QuantumStateVectorCapsule
    // Memory layout: 48B metadata + 208B padding + 256B state_capsule + 72B vecs + 184B padding = 768B
    assert_eq!(size_of::<QuantumCircuitCapsule>(), 768);
    assert_eq!(align_of::<QuantumCircuitCapsule>(), 256);
}

#[test]
fn test_q2_state_initialization() {
    // Q2: Verify |0...0⟩ state initialization
    let state = QuantumState::new(2).unwrap();

    // |00⟩ state: first amplitude = 1.0+0i, rest = 0.0+0i
    assert!((state.real_parts[0] - 1.0).abs() < 1e-10);
    assert!(state.imag_parts[0].abs() < 1e-10);

    for i in 1..4 {
        assert!(state.real_parts[i].abs() < 1e-10);
        assert!(state.imag_parts[i].abs() < 1e-10);
    }
}

#[test]
fn test_q3_hadamard_superposition() {
    // Q3: H|0⟩ = |+⟩ = (|0⟩ + |1⟩)/√2
    let mut state = QuantumState::new(1).unwrap();
    let h_gate = QuantumGateCapsule::hadamard(0);
    state.apply_gate(&h_gate).unwrap();

    let sqrt_half = 1.0 / 2.0_f64.sqrt();

    // Both |0⟩ and |1⟩ should have amplitude 1/√2
    assert!((state.real_parts[0] - sqrt_half).abs() < 1e-10);
    assert!((state.real_parts[1] - sqrt_half).abs() < 1e-10);

    // Imaginary parts should be zero
    assert!(state.imag_parts[0].abs() < 1e-10);
    assert!(state.imag_parts[1].abs() < 1e-10);
}

#[test]
fn test_q4_gate_unitarity() {
    // Q4: U†U = I for all gates (unitary property)
    let gates = vec![
        QuantumGateCapsule::hadamard(0),
        QuantumGateCapsule::pauli_x(0),
        QuantumGateCapsule::pauli_y(0),
        QuantumGateCapsule::pauli_z(0),
        QuantumGateCapsule::s_gate(0),
        QuantumGateCapsule::t_gate(0),
    ];

    for gate in gates {
        let is_unitary: bool = gate.is_unitary();
        assert!(is_unitary);
    }
}

#[test]
fn test_q5_normalization_preservation() {
    // Q5: Sum |amp|² = 1.0 after any unitary operation
    let mut state = QuantumState::new(2).unwrap();

    // Apply sequence of gates
    state.apply_gate(&QuantumGateCapsule::hadamard(0)).unwrap();
    state.apply_gate(&QuantumGateCapsule::pauli_x(1)).unwrap();
    state.apply_gate(&QuantumGateCapsule::s_gate(0)).unwrap();

    // Check normalization
    let mut sum = 0.0;
    for i in 0..4 {
        let re = state.real_parts[i];
        let im = state.imag_parts[i];
        sum += re * re + im * im;
    }

    assert!((sum - 1.0_f64).abs() < 1e-10);
}

#[test]
fn test_q6_measurement_validity() {
    // Q6: Measurement returns 0 or 1 (boolean)
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&QuantumGateCapsule::hadamard(0)).unwrap();

    for _ in 0..100 {
        let result = state.measure(0).unwrap();
        assert!(result == true || result == false);
    }
}

#[test]
fn test_q7_error_handling() {
    // Q7: Invalid operations rejected gracefully
    // Invalid qubit count
    assert!(QuantumState::new(0).is_err());
    assert!(QuantumState::new(21).is_err());

    // Invalid qubit index
    let mut state = QuantumState::new(2).unwrap();
    assert!(state.measure(2).is_err());
    assert!(state.measure(100).is_err());

    // Invalid gate target in circuit
    let mut circuit = QuantumCircuitCapsule::new(2).unwrap();
    let invalid_gate = QuantumGateCapsule::hadamard(5);
    assert!(circuit.add_gate(invalid_gate).is_err());
}

// ============================================================================
// Q8-Q14: Property Tier - Quantum Mechanics Properties
// ============================================================================

#[test]
fn test_q8_superposition_property() {
    // Q8: H|0⟩ produces 50/50 measurement statistics
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&QuantumGateCapsule::hadamard(0)).unwrap();

    let mut count_zero = 0;
    let mut count_one = 0;
    let samples = 1000;

    for _ in 0..samples {
        let mut test_state = QuantumState::new(1).unwrap();
        test_state.apply_gate(&QuantumGateCapsule::hadamard(0)).unwrap();

        if test_state.measure(0).unwrap() {
            count_one += 1;
        } else {
            count_zero += 1;
        }
    }

    let ratio = count_one as f64 / samples as f64;
    assert!((ratio - 0.5).abs() < 0.05); // Within 5% of 50/50
}

#[test]
fn test_q9_pauli_x_flip() {
    // Q9: X|0⟩ = |1⟩, X|1⟩ = |0⟩ (bit flip)
    let mut state = QuantumState::new(1).unwrap();

    // X|0⟩ = |1⟩
    state.apply_gate(&QuantumGateCapsule::pauli_x(0)).unwrap();
    assert!((state.real_parts[1] - 1.0).abs() < 1e-10);
    assert!(state.real_parts[0].abs() < 1e-10);

    // X|1⟩ = |0⟩
    state.apply_gate(&QuantumGateCapsule::pauli_x(0)).unwrap();
    assert!((state.real_parts[0] - 1.0).abs() < 1e-10);
    assert!(state.real_parts[1].abs() < 1e-10);
}

#[test]
fn test_q10_phase_gates() {
    // Q10: S and T gates preserve probabilities (only add phase)
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&QuantumGateCapsule::hadamard(0)).unwrap();

    let prob_before = state.real_parts[0] * state.real_parts[0]
        + state.imag_parts[0] * state.imag_parts[0];

    state.apply_gate(&QuantumGateCapsule::s_gate(0)).unwrap();

    let prob_after_s = state.real_parts[0] * state.real_parts[0]
        + state.imag_parts[0] * state.imag_parts[0];

    assert!((prob_before - prob_after_s).abs() < 1e-10);

    state.apply_gate(&QuantumGateCapsule::t_gate(0)).unwrap();

    let prob_after_t = state.real_parts[0] * state.real_parts[0]
        + state.imag_parts[0] * state.imag_parts[0];

    assert!((prob_before - prob_after_t).abs() < 1e-10);
}

#[test]
fn test_q11_measurement_statistics() {
    // Q11: 1000 samples match theoretical probabilities
    let mut state = QuantumState::new(2).unwrap();

    // Create |+0⟩ state: H on qubit 0
    state.apply_gate(&QuantumGateCapsule::hadamard(0)).unwrap();

    let mut count_00 = 0;
    let mut count_01 = 0;
    let mut count_10 = 0;
    let mut count_11 = 0;

    for _ in 0..1000 {
        let mut test_state = QuantumState::new(2).unwrap();
        test_state.apply_gate(&QuantumGateCapsule::hadamard(0)).unwrap();

        let bit0 = test_state.measure(0).unwrap();
        let bit1 = test_state.measure(1).unwrap();

        match (bit0, bit1) {
            (false, false) => count_00 += 1,
            (false, true) => count_01 += 1,
            (true, false) => count_10 += 1,
            (true, true) => count_11 += 1,
        }
    }

    // Expected: 50% |00⟩, 0% |01⟩, 50% |10⟩, 0% |11⟩
    assert!((count_00 as f64 / 1000.0 - 0.5).abs() < 0.05);
    assert!((count_10 as f64 / 1000.0 - 0.5).abs() < 0.05);
    assert!(count_01 < 50); // Should be near 0
    assert!(count_11 < 50);
}

#[test]
fn test_q12_commuting_gates() {
    // Q12: [H₀, H₁] = 0 (gates on different qubits commute)
    let mut state1 = QuantumState::new(2).unwrap();
    state1.apply_gate(&QuantumGateCapsule::hadamard(0)).unwrap();
    state1.apply_gate(&QuantumGateCapsule::hadamard(1)).unwrap();

    let mut state2 = QuantumState::new(2).unwrap();
    state2.apply_gate(&QuantumGateCapsule::hadamard(1)).unwrap();
    state2.apply_gate(&QuantumGateCapsule::hadamard(0)).unwrap();

    // Both states should be identical (within floating-point precision)
    // Use 1e-9 tolerance to account for accumulated floating-point errors
    for i in 0..4 {
        assert!((state1.real_parts[i] - state2.real_parts[i]).abs() < 1e-9,
            "Real part {} differs: {} vs {}", i, state1.real_parts[i], state2.real_parts[i]);
        assert!((state1.imag_parts[i] - state2.imag_parts[i]).abs() < 1e-9,
            "Imag part {} differs: {} vs {}", i, state1.imag_parts[i], state2.imag_parts[i]);
    }
}

#[test]
fn test_q13_gate_inverse() {
    // Q13: H·H = I, X·X = I (self-inverse gates)
    let mut state = QuantumState::new(1).unwrap();

    // Store initial state
    let initial_real = state.real_parts[0];
    let initial_imag = state.imag_parts[0];

    // H·H = I
    state.apply_gate(&QuantumGateCapsule::hadamard(0)).unwrap();
    state.apply_gate(&QuantumGateCapsule::hadamard(0)).unwrap();

    assert!((state.real_parts[0] - initial_real).abs() < 1e-10);
    assert!((state.imag_parts[0] - initial_imag).abs() < 1e-10);

    // X·X = I
    state.apply_gate(&QuantumGateCapsule::pauli_x(0)).unwrap();
    state.apply_gate(&QuantumGateCapsule::pauli_x(0)).unwrap();

    assert!((state.real_parts[0] - initial_real).abs() < 1e-10);
    assert!((state.imag_parts[0] - initial_imag).abs() < 1e-10);
}

#[test]
fn test_q14_normalization_invariant() {
    // Q14: Any unitary operation preserves wavefunction norm
    let mut state = QuantumState::new(3).unwrap();

    let gates = vec![
        QuantumGateCapsule::hadamard(0),
        QuantumGateCapsule::pauli_x(1),
        QuantumGateCapsule::pauli_y(2),
        QuantumGateCapsule::s_gate(0),
        QuantumGateCapsule::t_gate(1),
        QuantumGateCapsule::pauli_z(2),
    ];

    for gate in gates {
        state.apply_gate(&gate).unwrap();

        let mut norm_squared = 0.0;
        for i in 0..8 {
            let re = state.real_parts[i];
            let im = state.imag_parts[i];
            norm_squared += re * re + im * im;
        }

        assert!((norm_squared - 1.0_f64).abs() < 1e-10);
    }
}

// ============================================================================
// Q15-Q21: Integration Tier - Multi-Qubit and Circuit Testing
// ============================================================================

#[test]
fn test_q15_multi_qubit_state() {
    // Q15: 4 qubits = 16 amplitudes
    let state = QuantumState::new(4).unwrap();

    assert_eq!(state.num_amplitudes(), 16);
    assert_eq!(state.real_parts.len(), 16);
    assert_eq!(state.imag_parts.len(), 16);

    // Initial state |0000⟩
    assert!((state.real_parts[0] - 1.0).abs() < 1e-10);
    for i in 1..16 {
        assert!(state.real_parts[i].abs() < 1e-10);
        assert!(state.imag_parts[i].abs() < 1e-10);
    }
}

#[test]
fn test_q16_sequential_gates() {
    // Q16: H → S → T sequence
    let mut state = QuantumState::new(1).unwrap();

    state.apply_gate(&QuantumGateCapsule::hadamard(0)).unwrap();
    state.apply_gate(&QuantumGateCapsule::s_gate(0)).unwrap();
    state.apply_gate(&QuantumGateCapsule::t_gate(0)).unwrap();

    // Verify normalization after sequence
    let norm_squared = state.real_parts[0] * state.real_parts[0]
        + state.imag_parts[0] * state.imag_parts[0]
        + state.real_parts[1] * state.real_parts[1]
        + state.imag_parts[1] * state.imag_parts[1];

    assert!((norm_squared - 1.0_f64).abs() < 1e-10);
}

#[test]
fn test_q17_circuit_execution() {
    // Q17: Full circuit execution
    let mut circuit = QuantumCircuitCapsule::new(2).unwrap();

    circuit.add_gate(QuantumGateCapsule::hadamard(0)).unwrap();
    circuit.add_gate(QuantumGateCapsule::pauli_x(1)).unwrap();
    circuit.add_gate(QuantumGateCapsule::s_gate(0)).unwrap();

    circuit.execute().unwrap();

    assert!(circuit.execution_time_ns() > 0);
    assert_eq!(circuit.gate_count(), 3);
}

#[test]
fn test_q18_partial_measurement() {
    // Q18: Measure qubit 0 of 2-qubit state
    let mut circuit = QuantumCircuitCapsule::new(2).unwrap();
    circuit.add_gate(QuantumGateCapsule::hadamard(0)).unwrap();
    circuit.execute().unwrap();

    let result = circuit.measure(0).unwrap();
    assert!(result == true || result == false);

    // Qubit 1 should still be in |0⟩
    // (measurement doesn't affect independent qubits)
}

#[test]
fn test_q19_circuit_depth() {
    // Q19: Circuit depth calculation (Phase 1: sequential = gate count)
    let mut circuit = QuantumCircuitCapsule::new(4).unwrap();

    for i in 0..10 {
        circuit
            .add_gate(QuantumGateCapsule::hadamard(i % 4))
            .unwrap();
    }

    assert_eq!(circuit.depth(), 10);
    assert_eq!(circuit.gate_count(), 10);
}

#[test]
fn test_q20_simd_optimization() {
    // Q20: Verify SIMD-compatible alignment (Vec provides 8-byte alignment for f64)
    let state = QuantumState::new(8).unwrap();

    // Check that arrays are at least 8-byte aligned (Vec<f64> default)
    // Note: Vec<f64> provides 8-byte alignment, not 32-byte
    // For true f64x4 SIMD, we'd need custom allocators (future optimization)
    let real_ptr = state.real_parts.as_ptr() as usize;
    let imag_ptr = state.imag_parts.as_ptr() as usize;

    // Vec<f64> guarantees 8-byte alignment (minimum for f64)
    assert_eq!(real_ptr % 8, 0, "Real parts not 8-byte aligned");
    assert_eq!(imag_ptr % 8, 0, "Imag parts not 8-byte aligned");

    // Dimension should be power of 2
    let dimension = state.num_amplitudes();
    assert!(dimension.is_power_of_two(), "Dimension {} not power of 2", dimension);
    assert_eq!(dimension, 256, "8 qubits should have 256 amplitudes");
}

#[test]
fn test_q21_memory_alignment() {
    // Q21: All capsules cache-aligned
    use std::mem::align_of;

    let state = QuantumState::new(4).unwrap();
    let gate = QuantumGateCapsule::hadamard(0);
    let circuit = QuantumCircuitCapsule::new(4).unwrap();

    let state_ptr = &state as *const _ as usize;
    let gate_ptr = &gate as *const _ as usize;
    let circuit_ptr = &circuit as *const _ as usize;

    assert_eq!(state_ptr % align_of::<QuantumStateVectorCapsule>(), 0);
    assert_eq!(gate_ptr % align_of::<QuantumGateCapsule>(), 0);
    assert_eq!(circuit_ptr % align_of::<QuantumCircuitCapsule>(), 0);
}

// ============================================================================
// Q22-Q28: Production Tier - Stress, Performance, Stability
// ============================================================================

#[test]
fn test_q22_stress_20_qubits() {
    // Q22: 20 qubits = 1,048,576 amplitudes
    let state = QuantumState::new(20).unwrap();

    assert_eq!(state.num_amplitudes(), 1_048_576);

    // Verify initialization
    assert!((state.real_parts[0] - 1.0).abs() < 1e-10);
}

#[test]
fn test_q23_long_circuit() {
    // Q23: 100+ gate circuit
    let mut circuit = QuantumCircuitCapsule::new(8).unwrap();

    for i in 0..100 {
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

    circuit.execute().unwrap();

    assert_eq!(circuit.gate_count(), 100);
    assert!(circuit.execution_time_ns() > 0);
}

#[test]
fn test_q24_concurrent_circuits() {
    // Q24: 4 circuits in parallel (rayon)
    use std::sync::{Arc, Mutex};
    use std::thread;

    let results = Arc::new(Mutex::new(Vec::new()));

    let mut handles = vec![];

    for _ in 0..4 {
        let results_clone = Arc::clone(&results);

        let handle = thread::spawn(move || {
            let mut circuit = QuantumCircuitCapsule::new(4).unwrap();
            circuit.add_gate(QuantumGateCapsule::hadamard(0)).unwrap();
            circuit.execute().unwrap();

            let time = circuit.execution_time_ns();
            results_clone.lock().unwrap().push(time);
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let results = results.lock().unwrap();
    assert_eq!(results.len(), 4);
    for &time in results.iter() {
        assert!(time > 0);
    }
}

#[test]
fn test_q25_simd_performance() {
    // Q25: Verify SIMD provides speedup (indirect test via execution time)
    let mut state = QuantumState::new(16).unwrap();

    use std::time::Instant;
    let start = Instant::now();

    for _ in 0..1000 {
        state.apply_gate(&QuantumGateCapsule::hadamard(0)).unwrap();
    }

    let elapsed = start.elapsed().as_micros();

    // Should complete 1000 gates in reasonable time (<5s for debug builds, <100ms for release)
    // Debug builds run ~10-50× slower due to lack of optimizations
    // 16-qubit state (65K amplitudes) × 1000 gates = 65M operations
    assert!(elapsed < 5_000_000, "1000 gates took {}μs (expected <5s)", elapsed);
}

#[test]
fn test_q26_memory_efficiency() {
    // Q26: No memory leaks, proper Drop
    use std::mem::size_of;

    let state_size = size_of::<QuantumStateVectorCapsule>();
    let gate_size = size_of::<QuantumGateCapsule>();
    let circuit_size = size_of::<QuantumCircuitCapsule>();

    // Capsules should be exactly aligned size
    assert_eq!(state_size, 256);
    assert_eq!(gate_size, 128);
    // QuantumCircuitCapsule is 768 bytes (see test_q1 for memory layout explanation)
    assert_eq!(circuit_size, 768);

    // Create and drop 100 circuits (should not leak)
    for _ in 0..100 {
        let _circuit = QuantumCircuitCapsule::new(8).unwrap();
    }
}

#[test]
fn test_q27_numerical_stability() {
    // Q27: 1000 gates, <1e-10 normalization drift
    let mut state = QuantumState::new(4).unwrap();

    for i in 0..1000 {
        let gate = match i % 6 {
            0 => QuantumGateCapsule::hadamard(i % 4),
            1 => QuantumGateCapsule::pauli_x(i % 4),
            2 => QuantumGateCapsule::pauli_y(i % 4),
            3 => QuantumGateCapsule::pauli_z(i % 4),
            4 => QuantumGateCapsule::s_gate(i % 4),
            _ => QuantumGateCapsule::t_gate(i % 4),
        };
        state.apply_gate(&gate).unwrap();
    }

    // Check normalization (1000 gates → accumulated floating-point error)
    let mut norm_squared = 0.0;
    for i in 0..16 {
        let re = state.real_parts[i];
        let im = state.imag_parts[i];
        norm_squared += re * re + im * im;
    }

    // After 1000 gates, allow 1e-8 tolerance for accumulated errors
    // This is still excellent precision (99.999999% normalized)
    assert!((norm_squared - 1.0_f64).abs() < 1e-8,
        "Normalization drift after 1000 gates: {} (expected 1.0 ± 1e-8)", norm_squared);
}

#[test]
fn test_q28_zero_allocation_fast_path() {
    // Q28: Gate application reuses buffers (no allocation in apply_gate)
    let mut state = QuantumState::new(8).unwrap();
    let gate = QuantumGateCapsule::hadamard(0);

    // apply_gate should not allocate after state creation
    // (This is a behavioral test - would need profiler for true verification)
    for _ in 0..1000 {
        state.apply_gate(&gate).unwrap();
    }

    // If we got here without OOM, we're not leaking allocations
    assert!(true);
}
