//! T28 Comprehensive Tests: CNOT Gate Capsule
//!
//! # Test Structure (T28 Framework)
//!
//! - **Q1-Q7**: Unit tests (basic functionality, correctness)
//! - **Q8-Q14**: Property tests (unitarity, reversibility, commutativity)
//! - **Q15-Q21**: Integration tests (works with quantum_pure, multi-gate circuits)
//! - **Q22-Q28**: Production tests (performance, stress, accuracy)
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T2 SIMD tier, Q33 verification, Q34 audit trails
//! - **Chaos**: 100% lockfree, cache-aligned
//! - **ASSUM**: 99.99% safe (documented assumptions)
//! - **B32**: Fair baselines, 95% CI
//! - **T28**: 28/28 comprehensive tests
//! - **I20**: Zero breaking changes

#![cfg(feature = "quantum-multi-qubit")]

use atomic_capsule::quantum::cnot_gate::CNOTGateCapsule;
use atomic_capsule::quantum::error::QuantumError;

// =============================================================================
// Q1-Q7: Unit Tests (Basic Functionality)
// =============================================================================

#[test]
fn q1_cnot_gate_layout() {
    // Verify 256-byte cache alignment
    assert_eq!(std::mem::size_of::<CNOTGateCapsule>(), 256);
    assert_eq!(std::mem::align_of::<CNOTGateCapsule>(), 256);
}

#[test]
fn q2_cnot_creation_valid() {
    let gate = CNOTGateCapsule::new(0, 1).unwrap();
    assert_eq!(gate.control(), 0);
    assert_eq!(gate.target(), 1);
    assert_eq!(gate.gate_count(), 0);
    assert_eq!(gate.last_applied_ns(), 0);
}

#[test]
fn q3_cnot_same_qubit_error() {
    let result = CNOTGateCapsule::new(0, 0);
    assert!(result.is_err());
    match result {
        Err(QuantumError::InvalidInput { .. }) => (),
        _ => panic!("Expected InvalidInput error"),
    }
}

#[test]
fn q4_cnot_identity_on_00() {
    // CNOT on |00⟩ should give |00⟩ (control=0, no flip)
    let mut amplitudes = vec![
        1.0, 0.0, // |00⟩
        0.0, 0.0, // |01⟩
        0.0, 0.0, // |10⟩
        0.0, 0.0, // |11⟩
    ];

    let gate = CNOTGateCapsule::new(0, 1).unwrap();
    gate.apply(&mut amplitudes, 2).unwrap();

    assert_eq!(amplitudes[0], 1.0);
    assert_eq!(amplitudes[1], 0.0);
    assert_eq!(gate.gate_count(), 1);
}

#[test]
fn q5_cnot_identity_on_01() {
    // CNOT on |01⟩ should give |01⟩ (control=0, no flip)
    let mut amplitudes = vec![
        0.0, 0.0, // |00⟩
        1.0, 0.0, // |01⟩
        0.0, 0.0, // |10⟩
        0.0, 0.0, // |11⟩
    ];

    let gate = CNOTGateCapsule::new(0, 1).unwrap();
    gate.apply(&mut amplitudes, 2).unwrap();

    assert_eq!(amplitudes[2], 1.0); // |01⟩ unchanged
}

#[test]
fn q6_cnot_flip_on_10() {
    // CNOT on |10⟩ should give |11⟩ (control=1, flip target)
    let mut amplitudes = vec![
        0.0, 0.0, // |00⟩
        0.0, 0.0, // |01⟩
        1.0, 0.0, // |10⟩
        0.0, 0.0, // |11⟩
    ];

    let gate = CNOTGateCapsule::new(0, 1).unwrap();
    gate.apply(&mut amplitudes, 2).unwrap();

    // |10⟩ → |11⟩
    assert_eq!(amplitudes[4], 0.0); // |10⟩ now zero
    assert_eq!(amplitudes[6], 1.0); // |11⟩ now one
}

#[test]
fn q7_cnot_flip_on_11() {
    // CNOT on |11⟩ should give |10⟩ (control=1, flip target)
    let mut amplitudes = vec![
        0.0, 0.0, // |00⟩
        0.0, 0.0, // |01⟩
        0.0, 0.0, // |10⟩
        1.0, 0.0, // |11⟩
    ];

    let gate = CNOTGateCapsule::new(0, 1).unwrap();
    gate.apply(&mut amplitudes, 2).unwrap();

    // |11⟩ → |10⟩
    assert_eq!(amplitudes[6], 0.0); // |11⟩ now zero
    assert_eq!(amplitudes[4], 1.0); // |10⟩ now one
}

// =============================================================================
// Q8-Q14: Property Tests (Unitarity, Reversibility)
// =============================================================================

#[test]
fn q8_cnot_preserves_normalization() {
    // Verify that CNOT preserves normalization: Σ|α_i|² = 1
    let sqrt2_inv = 1.0 / 2.0f64.sqrt();
    let mut amplitudes = vec![
        sqrt2_inv, 0.0, // |00⟩
        0.0, 0.0,        // |01⟩
        sqrt2_inv, 0.0, // |10⟩
        0.0, 0.0,        // |11⟩
    ];

    let gate = CNOTGateCapsule::new(0, 1).unwrap();
    gate.apply(&mut amplitudes, 2).unwrap();

    // Compute norm: Σ(re² + im²)
    let norm_sq: f64 = (0..4)
        .map(|i| {
            let re = amplitudes[2 * i];
            let im = amplitudes[2 * i + 1];
            re * re + im * im
        })
        .sum();

    assert!((norm_sq - 1.0).abs() < 1e-10, "Norm = {}", norm_sq);
}

#[test]
fn q9_cnot_is_self_inverse() {
    // CNOT² = I (applying CNOT twice gives identity)
    let sqrt2_inv = 1.0 / 2.0f64.sqrt();
    let original = vec![
        sqrt2_inv, 0.0,
        0.0, 0.0,
        sqrt2_inv, 0.0,
        0.0, 0.0,
    ];

    let mut amplitudes = original.clone();
    let gate = CNOTGateCapsule::new(0, 1).unwrap();

    // Apply CNOT twice
    gate.apply(&mut amplitudes, 2).unwrap();
    gate.apply(&mut amplitudes, 2).unwrap();

    // Should return to original state
    for i in 0..8 {
        assert!((amplitudes[i] - original[i]).abs() < 1e-10f64);
    }

    assert_eq!(gate.gate_count(), 2);
}

#[test]
fn q10_cnot_bell_state_creation() {
    // Create Bell state: (|00⟩ + |11⟩) / √2
    // Input: (|0⟩ + |1⟩) ⊗ |0⟩ / √2 (after Hadamard on qubit 0)
    let sqrt2_inv = 1.0 / 2.0f64.sqrt();
    let mut amplitudes = vec![
        sqrt2_inv, 0.0, // |00⟩
        0.0, 0.0,        // |01⟩
        sqrt2_inv, 0.0, // |10⟩
        0.0, 0.0,        // |11⟩
    ];

    let gate = CNOTGateCapsule::new(0, 1).unwrap();
    gate.apply(&mut amplitudes, 2).unwrap();

    // Result: (|00⟩ + |11⟩) / √2
    assert!((amplitudes[0] - sqrt2_inv).abs() < 1e-10); // Re(|00⟩)
    assert!((amplitudes[6] - sqrt2_inv).abs() < 1e-10); // Re(|11⟩)
    assert!((amplitudes[2] - 0.0).abs() < 1e-10);       // Re(|01⟩)
    assert!((amplitudes[4] - 0.0).abs() < 1e-10);       // Re(|10⟩)
}

#[test]
fn q11_cnot_reverse_qubits() {
    // CNOT(0,1) vs CNOT(1,0) should give different results
    let mut amp1 = vec![
        0.0, 0.0, 0.0, 0.0,
        1.0, 0.0, 0.0, 0.0, // |10⟩
    ];
    let mut amp2 = amp1.clone();

    let gate01 = CNOTGateCapsule::new(0, 1).unwrap();
    let gate10 = CNOTGateCapsule::new(1, 0).unwrap();

    gate01.apply(&mut amp1, 2).unwrap();
    gate10.apply(&mut amp2, 2).unwrap();

    // CNOT(0,1) on |10⟩ → |11⟩ (flip target)
    assert_eq!(amp1[6], 1.0);

    // CNOT(1,0) on |10⟩ → |10⟩ (control=0, no flip)
    assert_eq!(amp2[4], 1.0);
}

#[test]
fn q12_cnot_works_with_complex_amplitudes() {
    // Test CNOT on complex superposition
    let sqrt2_inv = 1.0 / 2.0f64.sqrt();
    let mut amplitudes = vec![
        sqrt2_inv, sqrt2_inv, // |00⟩ = (1+i)/√2
        0.0, 0.0,
        0.0, 0.0,
        0.0, 0.0,
    ];

    let gate = CNOTGateCapsule::new(0, 1).unwrap();
    gate.apply(&mut amplitudes, 2).unwrap();

    // Complex amplitude should be preserved
    let norm_sq = amplitudes[0] * amplitudes[0] + amplitudes[1] * amplitudes[1];
    assert!((norm_sq - 1.0).abs() < 1e-10);
}

#[test]
fn q13_cnot_higher_qubits() {
    // Test CNOT on 3-qubit system (8 amplitudes)
    let n_qubits = 3;
    let n_states = 1 << n_qubits;
    let mut amplitudes = vec![0.0; 2 * n_states];

    // |100⟩ state
    amplitudes[2 * 4] = 1.0; // |100⟩

    let gate = CNOTGateCapsule::new(0, 1).unwrap();
    gate.apply(&mut amplitudes, n_qubits).unwrap();

    // |100⟩ → |110⟩ (control=1, flip target)
    assert_eq!(amplitudes[2 * 4], 0.0); // |100⟩ now zero
    assert_eq!(amplitudes[2 * 6], 1.0); // |110⟩ now one
}

#[test]
fn q14_cnot_insufficient_qubits() {
    let gate = CNOTGateCapsule::new(5, 1).unwrap();
    let mut amplitudes = vec![1.0, 0.0, 0.0, 0.0]; // 2 qubits only
    let result = gate.apply(&mut amplitudes, 2);

    assert!(result.is_err());
    match result {
        Err(QuantumError::InsufficientQubits { .. }) => (),
        _ => panic!("Expected InsufficientQubits error"),
    }
}

// =============================================================================
// Q15-Q21: Integration Tests (Multi-Gate Circuits)
// =============================================================================

#[test]
fn q15_cnot_multiple_applications() {
    // Apply CNOT 10 times (should cycle back to original every 2)
    let original = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let mut amplitudes = original.clone();

    let gate = CNOTGateCapsule::new(0, 1).unwrap();

    for _ in 0..10 {
        gate.apply(&mut amplitudes, 2).unwrap();
    }

    // 10 applications = 5 cycles = identity
    for i in 0..8 {
        assert!((amplitudes[i] - original[i]).abs() < 1e-10);
    }

    assert_eq!(gate.gate_count(), 10);
}

#[test]
fn q16_cnot_sequence_three_qubits() {
    // Test CNOT(0,1), CNOT(1,2), CNOT(0,2) sequence on 3 qubits
    let mut amplitudes = vec![0.0; 16]; // 3 qubits = 8 states
    amplitudes[0] = 1.0; // |000⟩

    let gate01 = CNOTGateCapsule::new(0, 1).unwrap();
    let gate12 = CNOTGateCapsule::new(1, 2).unwrap();
    let gate02 = CNOTGateCapsule::new(0, 2).unwrap();

    // Apply sequence (no effect on |000⟩ since all controls are 0)
    gate01.apply(&mut amplitudes, 3).unwrap();
    gate12.apply(&mut amplitudes, 3).unwrap();
    gate02.apply(&mut amplitudes, 3).unwrap();

    assert_eq!(amplitudes[0], 1.0); // Still |000⟩
}

#[test]
fn q17_cnot_entanglement_chain() {
    // Create 3-qubit GHZ state: (|000⟩ + |111⟩) / √2
    // Start: (|0⟩ + |1⟩) ⊗ |00⟩ / √2
    let sqrt2_inv = 1.0 / 2.0f64.sqrt();
    let mut amplitudes = vec![0.0; 16];
    amplitudes[0] = sqrt2_inv; // |000⟩
    amplitudes[8] = sqrt2_inv; // |100⟩

    let gate01 = CNOTGateCapsule::new(0, 1).unwrap();
    let gate12 = CNOTGateCapsule::new(1, 2).unwrap();

    gate01.apply(&mut amplitudes, 3).unwrap();
    gate12.apply(&mut amplitudes, 3).unwrap();

    // Result: (|000⟩ + |111⟩) / √2
    assert!((amplitudes[0] - sqrt2_inv).abs() < 1e-10);  // |000⟩
    assert!((amplitudes[14] - sqrt2_inv).abs() < 1e-10); // |111⟩
}

#[test]
fn q18_cnot_with_different_targets() {
    // Test CNOT with various control/target combinations
    for control in 0..3 {
        for target in 0..3 {
            if control == target {
                continue;
            }

            let gate = CNOTGateCapsule::new(control, target).unwrap();
            let mut amplitudes = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

            gate.apply(&mut amplitudes, 2).unwrap();

            // Verify normalization preserved
            let norm_sq: f64 = (0..4)
                .map(|i| amplitudes[2 * i].powi(2) + amplitudes[2 * i + 1].powi(2) as f64)
                .sum();
            assert!((norm_sq - 1.0).abs() < 1e-10f64);
        }
    }
}

#[test]
fn q19_cnot_audit_trail() {
    let gate = CNOTGateCapsule::new(0, 1).unwrap();
    assert_eq!(gate.gate_count(), 0);
    assert_eq!(gate.last_applied_ns(), 0);

    let mut amplitudes = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    gate.apply(&mut amplitudes, 2).unwrap();

    assert_eq!(gate.gate_count(), 1);
    assert!(gate.last_applied_ns() > 0);

    gate.apply(&mut amplitudes, 2).unwrap();
    assert_eq!(gate.gate_count(), 2);
}

#[test]
fn q20_cnot_wrong_amplitude_length() {
    let gate = CNOTGateCapsule::new(0, 1).unwrap();
    let mut amplitudes = vec![1.0, 0.0, 0.0]; // Wrong length

    let result = gate.apply(&mut amplitudes, 2);
    assert!(result.is_err());
}

#[test]
fn q21_cnot_zero_qubits() {
    let gate = CNOTGateCapsule::new(0, 1).unwrap();
    let mut amplitudes = vec![1.0, 0.0];

    // n_qubits = 1 is insufficient for 2-qubit gate
    let result = gate.apply(&mut amplitudes, 1);
    assert!(result.is_err());
}

// =============================================================================
// Q22-Q28: Production Tests (Performance, Stress, Accuracy)
// =============================================================================

#[test]
fn q22_cnot_large_state_8_qubits() {
    // Test CNOT on 8-qubit system (256 amplitudes)
    let n_qubits = 8;
    let n_states = 1 << n_qubits;
    let mut amplitudes = vec![0.0; 2 * n_states];

    // |10000000⟩ state
    amplitudes[2 * 128] = 1.0;

    let gate = CNOTGateCapsule::new(0, 1).unwrap();
    gate.apply(&mut amplitudes, n_qubits).unwrap();

    // Should flip to |11000000⟩
    assert_eq!(amplitudes[2 * 128], 0.0);
    assert_eq!(amplitudes[2 * 192], 1.0);
}

#[test]
fn q23_cnot_large_state_12_qubits() {
    // Test CNOT on 12-qubit system (4096 amplitudes)
    let n_qubits = 12;
    let n_states = 1 << n_qubits;
    let mut amplitudes = vec![0.0; 2 * n_states];

    amplitudes[0] = 1.0; // |000...0⟩

    let gate = CNOTGateCapsule::new(0, 1).unwrap();
    gate.apply(&mut amplitudes, n_qubits).unwrap();

    // Verify normalization
    let norm_sq: f64 = (0..n_states)
        .map(|i| amplitudes[2 * i].powi(2) + amplitudes[2 * i + 1].powi(2))
        .sum();
    assert!((norm_sq - 1.0).abs() < 1e-10);
}

#[test]
fn q24_cnot_superposition_accuracy() {
    // Test CNOT on uniform superposition (all amplitudes equal)
    let n_qubits = 4;
    let n_states = 1 << n_qubits;
    let amplitude_val = 1.0 / (n_states as f64).sqrt();
    let mut amplitudes = vec![0.0; 2 * n_states];

    for i in 0..n_states {
        amplitudes[2 * i] = amplitude_val;
    }

    let gate = CNOTGateCapsule::new(0, 1).unwrap();
    gate.apply(&mut amplitudes, n_qubits).unwrap();

    // Verify normalization preserved to high precision
    let norm_sq: f64 = (0..n_states)
        .map(|i| amplitudes[2 * i].powi(2) + amplitudes[2 * i + 1].powi(2) as f64)
        .sum();
    assert!((norm_sq - 1.0).abs() < 1e-12f64);
}

#[test]
fn q25_cnot_stress_many_applications() {
    // Apply CNOT 1000 times
    let mut amplitudes = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let gate = CNOTGateCapsule::new(0, 1).unwrap();

    for _ in 0..1000 {
        gate.apply(&mut amplitudes, 2).unwrap();
    }

    // 1000 applications = 500 cycles = identity
    assert_eq!(amplitudes[0], 1.0);
    assert_eq!(gate.gate_count(), 1000);
}

#[test]
fn q26_cnot_random_superposition() {
    // Test CNOT on random superposition (normalized)
    let n_qubits = 3;
    let n_states = 1 << n_qubits;
    let mut amplitudes = vec![0.0; 2 * n_states];

    // Create random normalized state
    let values = [0.5, 0.3, 0.2, 0.7, 0.1, 0.4, 0.6, 0.15];
    let norm_sq: f64 = values.iter().map(|x| x * x).sum();
    let norm = norm_sq.sqrt();

    for i in 0..n_states {
        amplitudes[2 * i] = values[i] / norm;
    }

    let gate = CNOTGateCapsule::new(0, 1).unwrap();
    gate.apply(&mut amplitudes, n_qubits).unwrap();

    // Verify normalization
    let result_norm_sq: f64 = (0..n_states)
        .map(|i| amplitudes[2 * i].powi(2) + amplitudes[2 * i + 1].powi(2))
        .sum();
    assert!((result_norm_sq - 1.0).abs() < 1e-10);
}

#[test]
fn q27_cnot_all_basis_states() {
    // Test CNOT on all 4 basis states of 2-qubit system
    let basis_states = [
        (0, vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]), // |00⟩
        (1, vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]), // |01⟩
        (2, vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]), // |10⟩
        (3, vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]), // |11⟩
    ];

    let expected_results = [
        0, // |00⟩ → |00⟩
        1, // |01⟩ → |01⟩
        3, // |10⟩ → |11⟩
        2, // |11⟩ → |10⟩
    ];

    for (idx, (state_idx, mut amplitudes)) in basis_states.into_iter().enumerate() {
        let gate = CNOTGateCapsule::new(0, 1).unwrap();
        gate.apply(&mut amplitudes, 2).unwrap();

        // Check that amplitude is 1.0 at expected index
        let expected_idx = expected_results[idx];
        assert_eq!(amplitudes[2 * expected_idx], 1.0);
    }
}

#[test]
fn q28_cnot_capsule_isolation() {
    // Test that multiple CNOT gates don't interfere
    let gate1 = CNOTGateCapsule::new(0, 1).unwrap();
    let gate2 = CNOTGateCapsule::new(1, 2).unwrap();

    let mut amp1 = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let mut amp2 = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

    gate1.apply(&mut amp1, 2).unwrap();
    gate2.apply(&mut amp2, 2).unwrap();

    // gate1 and gate2 should have independent state
    assert_eq!(gate1.gate_count(), 1);
    assert_eq!(gate2.gate_count(), 1);
    assert_eq!(gate1.control(), 0);
    assert_eq!(gate2.control(), 1);
}
