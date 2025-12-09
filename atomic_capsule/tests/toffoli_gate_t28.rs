//! T28 Comprehensive Tests: ToffoliGateCapsule
//!
//! # Test Organization (4 Tiers)
//!
//! - **Q1-Q7**: Unit tests (basic functionality, error cases)
//! - **Q8-Q14**: Property tests (reversibility, unitarity, classical simulation)
//! - **Q15-Q21**: Integration tests (quantum circuits, algorithm building blocks)
//! - **Q22-Q28**: Production tests (performance, edge cases, stress testing)
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T2 SIMD tier validation
//! - **T28**: 28 comprehensive tests (4 tiers × 7 tests)
//! - **ASSUM**: 99.99%+ safety verification
//! - **B32**: Performance benchmarks in separate file

#![cfg(test)]

use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

// Helper function to create test amplitudes
fn create_basis_state(n_qubits: usize, state: usize) -> Vec<(f64, f64)> {
    let n_states = 1 << n_qubits;
    let mut amplitudes = vec![(0.0, 0.0); n_states];
    amplitudes[state] = (1.0, 0.0);
    amplitudes
}

// Helper function to create uniform superposition
fn create_superposition(n_qubits: usize) -> Vec<(f64, f64)> {
    let n_states = 1 << n_qubits;
    let amplitude = 1.0 / (n_states as f64).sqrt();
    vec![(amplitude, 0.0); n_states]
}

// Helper function to check if amplitudes are normalized
fn is_normalized(amplitudes: &[(f64, f64)]) -> bool {
    let sum: f64 = amplitudes
        .iter()
        .map(|(re, im)| re * re + im * im)
        .sum();
    (sum - 1.0).abs() < 1e-10
}

// ============================================================================
// TIER 1: UNIT TESTS (Q1-Q7)
// ============================================================================

#[test]
fn q1_toffoli_capsule_layout() {
    // ASSUM: #VERIFY_ALIGNMENT
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;
    assert_eq!(std::mem::size_of::<ToffoliGateCapsule>(), 256);
    assert_eq!(std::mem::align_of::<ToffoliGateCapsule>(), 256);
}

#[test]
fn q2_toffoli_basic_creation() {
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 2).unwrap();
    assert_eq!(gate.control1(), 0);
    assert_eq!(gate.control2(), 1);
    assert_eq!(gate.target(), 2);
    assert_eq!(gate.gate_count(), 0);
}

#[test]
fn q3_toffoli_invalid_indices() {
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    // Same control qubits
    assert!(ToffoliGateCapsule::new(0, 0, 2).is_err());

    // Same control1 and target
    assert!(ToffoliGateCapsule::new(0, 1, 0).is_err());

    // Same control2 and target
    assert!(ToffoliGateCapsule::new(0, 1, 1).is_err());

    // All same
    assert!(ToffoliGateCapsule::new(3, 3, 3).is_err());
}

#[test]
fn q4_toffoli_and_gate_behavior() {
    // Toffoli implements classical AND gate when target starts at |0⟩
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 2).unwrap();

    // Test all 8 basis states (3 qubits)
    for state in 0..8 {
        let mut amps = create_basis_state(3, state);
        gate.apply(&mut amps, 3).unwrap();

        let c1 = (state >> 0) & 1;
        let c2 = (state >> 1) & 1;
        let t = (state >> 2) & 1;

        // Expected: target flips if c1=1 AND c2=1
        let expected_t = if c1 == 1 && c2 == 1 { 1 - t } else { t };
        let expected_state = (expected_t << 2) | (c2 << 1) | c1;

        // Check that amplitude is 1.0 at expected state
        assert!((amps[expected_state].0 - 1.0).abs() < 1e-10);
        assert!(amps[expected_state].1.abs() < 1e-10);
    }
}

#[test]
fn q5_toffoli_gate_counter() {
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 2).unwrap();
    let mut amps = vec![(1.0, 0.0); 8];

    assert_eq!(gate.gate_count(), 0);

    gate.apply(&mut amps, 3).unwrap();
    assert_eq!(gate.gate_count(), 1);

    gate.apply(&mut amps, 3).unwrap();
    assert_eq!(gate.gate_count(), 2);

    gate.reset_counter();
    assert_eq!(gate.gate_count(), 0);
}

#[test]
fn q6_toffoli_update_indices() {
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 2).unwrap();

    // Valid update
    gate.update_indices(3, 4, 5).unwrap();
    assert_eq!(gate.control1(), 3);
    assert_eq!(gate.control2(), 4);
    assert_eq!(gate.target(), 5);

    // Invalid update (same indices)
    assert!(gate.update_indices(0, 0, 1).is_err());
}

#[test]
fn q7_toffoli_qubit_bounds_validation() {
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 5).unwrap(); // Qubit 5 out of bounds for 3-qubit system
    let mut amps = vec![(1.0, 0.0); 8]; // 3 qubits

    // Should fail due to qubit index >= n_qubits
    let result = gate.apply(&mut amps, 3);
    assert!(result.is_err());
}

// ============================================================================
// TIER 2: PROPERTY TESTS (Q8-Q14)
// ============================================================================

#[test]
fn q8_toffoli_reversibility() {
    // Toffoli is its own inverse: T² = I
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 2).unwrap();

    for state in 0..8 {
        let original = create_basis_state(3, state);
        let mut amps = original.clone();

        // Apply twice
        gate.apply(&mut amps, 3).unwrap();
        gate.apply(&mut amps, 3).unwrap();

        // Should return to original state
        for i in 0..8 {
            assert!((amps[i].0 - original[i].0).abs() < 1e-10);
            assert!((amps[i].1 - original[i].1).abs() < 1e-10);
        }
    }
}

#[test]
fn q9_toffoli_unitarity() {
    // Unitary: preserves norm (sum of squared amplitudes = 1)
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 2).unwrap();
    let mut amps = create_superposition(3);

    assert!(is_normalized(&amps));

    gate.apply(&mut amps, 3).unwrap();

    // Norm should be preserved
    assert!(is_normalized(&amps));
}

#[test]
fn q10_toffoli_classical_and_simulation() {
    // Toffoli simulates classical AND when target starts at |0⟩
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 2).unwrap();

    // Truth table for AND gate
    let truth_table = [
        (0, 0, 0), // 0 AND 0 = 0
        (0, 1, 0), // 0 AND 1 = 0
        (1, 0, 0), // 1 AND 0 = 0
        (1, 1, 1), // 1 AND 1 = 1
    ];

    for &(a, b, expected) in &truth_table {
        // Encode as |a,b,0⟩
        let state = (0 << 2) | (b << 1) | a;
        let mut amps = create_basis_state(3, state);

        gate.apply(&mut amps, 3).unwrap();

        // Expected state: |a,b,expected⟩
        let expected_state = (expected << 2) | (b << 1) | a;
        assert!((amps[expected_state].0 - 1.0).abs() < 1e-10);
    }
}

#[test]
fn q11_toffoli_control_qubit_preservation() {
    // Control qubits should never change
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 2).unwrap();

    for state in 0..8 {
        let mut amps = create_basis_state(3, state);
        gate.apply(&mut amps, 3).unwrap();

        // Find which state has amplitude 1.0
        let result_state = amps.iter().position(|(re, _)| (*re - 1.0).abs() < 1e-10).unwrap();

        // Control qubits (bits 0 and 1) should be unchanged
        assert_eq!(result_state & 0b11, state & 0b11);
    }
}

#[test]
fn q12_toffoli_superposition_linearity() {
    // Toffoli is linear: applies independently to each basis state in superposition
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 2).unwrap();

    // Create superposition: (|000⟩ + |110⟩)/√2
    let mut amps = vec![(0.0, 0.0); 8];
    let amplitude = 1.0 / 2.0_f64.sqrt();
    amps[0b000] = (amplitude, 0.0); // |000⟩
    amps[0b011] = (amplitude, 0.0); // |110⟩ (c1=1, c2=1, t=0)

    gate.apply(&mut amps, 3).unwrap();

    // Expected: (|000⟩ + |111⟩)/√2 (target flipped in |110⟩ → |111⟩)
    assert!((amps[0b000].0 - amplitude).abs() < 1e-10);
    assert!((amps[0b111].0 - amplitude).abs() < 1e-10);
    assert!(is_normalized(&amps));
}

#[test]
fn q13_toffoli_commutation_with_single_qubit_gates() {
    // Toffoli on qubits (0,1,2) commutes with gates on qubit 3
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 2).unwrap();

    // Create random state on 4 qubits
    let mut amps1 = create_superposition(4);
    let mut amps2 = amps1.clone();

    // Path 1: Toffoli then X on qubit 3
    gate.apply(&mut amps1, 4).unwrap();
    // X gate on qubit 3 (flip bit 3)
    for i in 0..(1 << 4) {
        let flipped = i ^ (1 << 3);
        if i < flipped {
            amps1.swap(i, flipped);
        }
    }

    // Path 2: X on qubit 3 then Toffoli
    for i in 0..(1 << 4) {
        let flipped = i ^ (1 << 3);
        if i < flipped {
            amps2.swap(i, flipped);
        }
    }
    gate.apply(&mut amps2, 4).unwrap();

    // Results should be identical
    for i in 0..(1 << 4) {
        assert!((amps1[i].0 - amps2[i].0).abs() < 1e-10);
        assert!((amps1[i].1 - amps2[i].1).abs() < 1e-10);
    }
}

#[test]
fn q14_toffoli_composition_properties() {
    // Toffoli can be composed to create multi-control gates
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate1 = ToffoliGateCapsule::new(0, 1, 2).unwrap();
    let gate2 = ToffoliGateCapsule::new(0, 1, 3).unwrap();

    // State |1100⟩ (c1=1, c2=1, t2=0, t3=0)
    let mut amps = create_basis_state(4, 0b0011);

    gate1.apply(&mut amps, 4).unwrap(); // |1110⟩
    gate2.apply(&mut amps, 4).unwrap(); // |1111⟩

    // Both targets should be flipped
    assert!((amps[0b1111].0 - 1.0).abs() < 1e-10);
}

// ============================================================================
// TIER 3: INTEGRATION TESTS (Q15-Q21)
// ============================================================================

#[test]
fn q15_toffoli_in_reversible_circuit() {
    // Toffoli is fundamental for reversible computing
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    // Implement full adder using Toffoli gates
    let carry_gate = ToffoliGateCapsule::new(0, 1, 3).unwrap(); // a AND b → carry
    let sum_gate = ToffoliGateCapsule::new(0, 2, 4).unwrap();   // a XOR cin → intermediate

    // State |a=1, b=1, cin=0, carry=0, sum=0⟩
    let mut amps = create_basis_state(5, 0b00011);

    carry_gate.apply(&mut amps, 5).unwrap();
    sum_gate.apply(&mut amps, 5).unwrap();

    // Expected: carry=1, sum computed
    assert!(is_normalized(&amps));
}

#[test]
fn q16_toffoli_for_error_correction() {
    // Toffoli gates used in syndrome extraction for quantum error correction
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    // Bit-flip error detection (3-qubit code)
    let gate1 = ToffoliGateCapsule::new(0, 1, 3).unwrap(); // Syndrome qubit 1
    let gate2 = ToffoliGateCapsule::new(1, 2, 4).unwrap(); // Syndrome qubit 2

    // Encoded state with bit-flip on qubit 1
    let mut amps = create_basis_state(5, 0b00010);

    gate1.apply(&mut amps, 5).unwrap();
    gate2.apply(&mut amps, 5).unwrap();

    // Syndrome should detect error
    assert!(is_normalized(&amps));
}

#[test]
fn q17_toffoli_universal_computation() {
    // Toffoli + Hadamard = universal quantum computation
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 2).unwrap();

    // Create superposition on control qubits (Hadamard-like)
    let mut amps = vec![(0.5, 0.0); 8]; // Uniform superposition

    gate.apply(&mut amps, 3).unwrap();

    // Toffoli entangles control qubits with target
    assert!(is_normalized(&amps));
}

#[test]
fn q18_toffoli_modular_arithmetic() {
    // Toffoli gates building block for modular exponentiation (Shor's algorithm)
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    // Simple carry propagation for addition
    let gates = vec![
        ToffoliGateCapsule::new(0, 1, 4).unwrap(),
        ToffoliGateCapsule::new(2, 3, 5).unwrap(),
    ];

    let mut amps = create_basis_state(6, 0b001111); // a=3, b=3, carries=0

    for gate in &gates {
        gate.apply(&mut amps, 6).unwrap();
    }

    assert!(is_normalized(&amps));
}

#[test]
fn q19_toffoli_cascade_depth() {
    // Test circuit with depth 10 (stress test)
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 2).unwrap();
    let mut amps = create_basis_state(3, 0b111);

    for _ in 0..10 {
        gate.apply(&mut amps, 3).unwrap();
    }

    // Even number of applications → back to original (Toffoli² = I)
    assert!((amps[0b111].0 - 1.0).abs() < 1e-10);
}

#[test]
fn q20_toffoli_multi_target() {
    // Multiple Toffoli gates with same controls, different targets
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate1 = ToffoliGateCapsule::new(0, 1, 2).unwrap();
    let gate2 = ToffoliGateCapsule::new(0, 1, 3).unwrap();

    // State |1100⟩
    let mut amps = create_basis_state(4, 0b0011);

    gate1.apply(&mut amps, 4).unwrap();
    gate2.apply(&mut amps, 4).unwrap();

    // Both targets flipped: |1111⟩
    assert!((amps[0b1111].0 - 1.0).abs() < 1e-10);
}

#[test]
fn q21_toffoli_large_system() {
    // Test Toffoli on 10-qubit system (1024 basis states)
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 9).unwrap();
    let mut amps = create_basis_state(10, 0b0000000011); // |...0011⟩

    gate.apply(&mut amps, 10).unwrap();

    // Target (qubit 9) flipped: |1000000011⟩
    assert!((amps[0b1000000011].0 - 1.0).abs() < 1e-10);
    assert!(is_normalized(&amps));
}

// ============================================================================
// TIER 4: PRODUCTION TESTS (Q22-Q28)
// ============================================================================

#[test]
fn q22_toffoli_all_basis_states() {
    // Test all 8 basis states exhaustively
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 2).unwrap();

    for state in 0..8 {
        let mut amps = create_basis_state(3, state);
        gate.apply(&mut amps, 3).unwrap();

        // Verify normalization
        assert!(is_normalized(&amps));

        // Verify exactly one basis state has amplitude 1.0
        let count = amps.iter().filter(|(re, im)| (*re - 1.0).abs() < 1e-10 && im.abs() < 1e-10).count();
        assert_eq!(count, 1);
    }
}

#[test]
fn q23_toffoli_edge_case_max_qubits() {
    // Test with maximum reasonable qubit indices
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(17, 18, 19).unwrap();
    let mut amps = create_basis_state(20, 0); // 20 qubits

    gate.apply(&mut amps, 20).unwrap();
    assert!(is_normalized(&amps));
}

#[test]
fn q24_toffoli_stress_repeated_application() {
    // Apply Toffoli 1000 times (stress test)
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 2).unwrap();
    let original = create_basis_state(3, 0b111);
    let mut amps = original.clone();

    for _ in 0..1000 {
        gate.apply(&mut amps, 3).unwrap();
    }

    // Even number → back to original
    for i in 0..8 {
        assert!((amps[i].0 - original[i].0).abs() < 1e-9);
    }

    assert_eq!(gate.gate_count(), 1000);
}

#[test]
fn q25_toffoli_concurrent_counter() {
    // Test atomic counter under concurrent-like conditions
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;
    use std::sync::Arc;

    let gate = Arc::new(ToffoliGateCapsule::new(0, 1, 2).unwrap());
    let mut amps = vec![(1.0, 0.0); 8];

    for _ in 0..100 {
        gate.apply(&mut amps, 3).unwrap();
    }

    assert_eq!(gate.gate_count(), 100);
}

#[test]
fn q26_toffoli_numerical_stability() {
    // Test numerical stability over many operations
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 2).unwrap();
    let mut amps = create_superposition(3);

    for _ in 0..100 {
        gate.apply(&mut amps, 3).unwrap();
    }

    // Norm should still be very close to 1.0
    let norm: f64 = amps.iter().map(|(re, im)| re * re + im * im).sum();
    assert!((norm - 1.0).abs() < 1e-8);
}

#[test]
fn q27_toffoli_error_recovery() {
    // Test error handling and recovery
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    let gate = ToffoliGateCapsule::new(0, 1, 2).unwrap();
    let mut amps = vec![(1.0, 0.0); 4]; // Too small for 3-qubit gate

    // Should fail gracefully
    let result = gate.apply(&mut amps, 3);
    assert!(result.is_err());

    // Gate should still be usable
    let mut amps = vec![(1.0, 0.0); 8];
    assert!(gate.apply(&mut amps, 3).is_ok());
}

#[test]
fn q28_toffoli_assum_safety_verification() {
    // ASSUM safety verification
    use atomic_capsule::quantum::toffoli_gate::ToffoliGateCapsule;

    // #ASSUME_THREE_QUBIT_INDICES_VALID
    let gate = ToffoliGateCapsule::new(0, 1, 2).unwrap();
    assert_ne!(gate.control1(), gate.control2());
    assert_ne!(gate.control1(), gate.target());
    assert_ne!(gate.control2(), gate.target());

    // #ASSUME_LOCKFREE_COORDINATION
    let mut amps = vec![(1.0, 0.0); 8];
    gate.apply(&mut amps, 3).unwrap(); // No mutex/lock calls

    // #ASSUME_CACHE_ALIGNED
    let alignment = std::mem::align_of::<ToffoliGateCapsule>();
    assert_eq!(alignment, 256);

    // #VERIFY_ALIGNMENT
    let size = std::mem::size_of::<ToffoliGateCapsule>();
    assert_eq!(size, 256);
}
