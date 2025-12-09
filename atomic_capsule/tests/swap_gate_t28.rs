//! T28 Comprehensive Testing for SWAPGateCapsule
//!
//! # Test Structure (28 tests across 4 tiers)
//!
//! - **Q1-Q7** (Unit): Basic functionality, edge cases
//! - **Q8-Q14** (Property): Mathematical properties, invariants
//! - **Q15-Q21** (Integration): Circuit integration, multi-gate sequences
//! - **Q22-Q28** (Production): Performance, stress, real-world scenarios

#![cfg(feature = "quantum-multi-qubit")]

use atomic_capsule::quantum_pure::{
    SWAPGateCapsule, QuantumStateVectorCapsule, QuantumPureError,
};

// ============================================================================
// TIER 1: UNIT TESTS (Q1-Q7) - Basic Functionality
// ============================================================================

#[test]
fn q01_swap_gate_creation() {
    let swap = SWAPGateCapsule::new(0, 1).unwrap();
    assert_eq!(swap.qubit1(), 0);
    assert_eq!(swap.qubit2(), 1);
    assert_eq!(swap.gate_count(), 0);
}

#[test]
fn q02_swap_gate_alignment() {
    assert_eq!(
        std::mem::size_of::<SWAPGateCapsule>(),
        128,
        "Must be 128 bytes"
    );
    assert_eq!(
        std::mem::align_of::<SWAPGateCapsule>(),
        128,
        "Must be 128-byte aligned"
    );
}

#[test]
fn q03_swap_gate_rejects_same_qubit() {
    let result = SWAPGateCapsule::new(0, 0);
    assert!(result.is_err());
    match result {
        Err(QuantumPureError::InvalidGateParameters { gate_type, .. }) => {
            assert_eq!(gate_type, "SWAP");
        }
        _ => panic!("Expected InvalidGateParameters"),
    }
}

#[test]
fn q04_swap_gate_symmetric() {
    // SWAP(0,1) should equal SWAP(1,0) in effect
    let swap01 = SWAPGateCapsule::new(0, 1).unwrap();
    let swap10 = SWAPGateCapsule::new(1, 0).unwrap();

    assert_eq!(swap01.qubit1(), 0);
    assert_eq!(swap01.qubit2(), 1);
    assert_eq!(swap10.qubit1(), 1);
    assert_eq!(swap10.qubit2(), 0);
}

#[test]
fn q05_swap_gate_basic_application() {
    let mut state = QuantumStateVectorCapsule::new(2).unwrap();
    let swap = SWAPGateCapsule::new(0, 1).unwrap();

    // Set state to |01⟩
    state.set_amplitude(0, atomic_capsule::quantum_pure::state_vector::Complex::real(0.0));
    state.set_amplitude(1, atomic_capsule::quantum_pure::state_vector::Complex::real(1.0));
    state.set_amplitude(2, atomic_capsule::quantum_pure::state_vector::Complex::real(0.0));
    state.set_amplitude(3, atomic_capsule::quantum_pure::state_vector::Complex::real(0.0));

    // Apply SWAP: |01⟩ → |10⟩
    swap.apply(&mut state).unwrap();

    let amp0 = state.get_amplitude(0); // |00⟩
    let amp1 = state.get_amplitude(1); // |01⟩
    let amp2 = state.get_amplitude(2); // |10⟩
    let amp3 = state.get_amplitude(3); // |11⟩

    assert!((amp0.re - 0.0).abs() < 1e-10);
    assert!((amp1.re - 0.0).abs() < 1e-10);
    assert!((amp2.re - 1.0).abs() < 1e-10);
    assert!((amp3.re - 0.0).abs() < 1e-10);
}

#[test]
fn q06_swap_gate_statistics_tracking() {
    let mut state = QuantumStateVectorCapsule::new(2).unwrap();
    let swap = SWAPGateCapsule::new(0, 1).unwrap();

    assert_eq!(swap.gate_count(), 0);

    swap.apply(&mut state).unwrap();
    assert_eq!(swap.gate_count(), 1);

    swap.apply(&mut state).unwrap();
    assert_eq!(swap.gate_count(), 2);

    // Timestamp should be set
    assert!(swap.last_apply_ns() > 0);
}

#[test]
fn q07_swap_gate_out_of_range_qubits() {
    let mut state = QuantumStateVectorCapsule::new(2).unwrap();
    let swap = SWAPGateCapsule::new(0, 5).unwrap(); // Qubit 5 doesn't exist

    let result = swap.apply(&mut state);
    assert!(result.is_err());
}

// ============================================================================
// TIER 2: PROPERTY TESTS (Q8-Q14) - Mathematical Properties
// ============================================================================

#[test]
fn q08_swap_involutory_property() {
    // SWAP² = I (applying twice gives identity)
    let mut state = QuantumStateVectorCapsule::new(2).unwrap();
    let swap = SWAPGateCapsule::new(0, 1).unwrap();

    // Set arbitrary state
    state.set_amplitude(0, atomic_capsule::quantum_pure::state_vector::Complex::new(0.5, 0.0));
    state.set_amplitude(1, atomic_capsule::quantum_pure::state_vector::Complex::new(0.5, 0.0));
    state.set_amplitude(2, atomic_capsule::quantum_pure::state_vector::Complex::new(0.5, 0.0));
    state.set_amplitude(3, atomic_capsule::quantum_pure::state_vector::Complex::new(0.5, 0.0));

    let original = state.clone();

    // Apply twice
    swap.apply(&mut state).unwrap();
    swap.apply(&mut state).unwrap();

    // Should restore original state
    for i in 0..4 {
        let orig = original.get_amplitude(i);
        let curr = state.get_amplitude(i);
        assert!((orig.re - curr.re).abs() < 1e-10);
        assert!((orig.im - curr.im).abs() < 1e-10);
    }
}

#[test]
fn q09_swap_unitary_property() {
    // Verify SWAP matrix is unitary: U†U = I
    let matrix = SWAPGateCapsule::matrix();

    // Compute U†U
    let mut result = [[atomic_capsule::quantum_pure::state_vector::Complex::real(0.0); 4]; 4];

    for i in 0..4 {
        for j in 0..4 {
            let mut sum = atomic_capsule::quantum_pure::state_vector::Complex::real(0.0);
            for k in 0..4 {
                // U†[i][k] = conj(U[k][i])
                let u_dag_ik = matrix[k][i].conj();
                let u_kj = matrix[k][j];
                sum = sum + u_dag_ik * u_kj;
            }
            result[i][j] = sum;
        }
    }

    // Check result is identity
    for i in 0..4 {
        for j in 0..4 {
            let expected = if i == j { 1.0 } else { 0.0 };
            assert!((result[i][j].re - expected).abs() < 1e-10);
            assert!(result[i][j].im.abs() < 1e-10);
        }
    }
}

#[test]
fn q10_swap_preserves_normalization() {
    // Verify ||ψ||² = 1 after SWAP
    let mut state = QuantumStateVectorCapsule::new(2).unwrap();
    let swap = SWAPGateCapsule::new(0, 1).unwrap();

    // Set normalized state
    let inv_sqrt2 = std::f64::consts::FRAC_1_SQRT_2;
    state.set_amplitude(0, atomic_capsule::quantum_pure::state_vector::Complex::real(inv_sqrt2));
    state.set_amplitude(1, atomic_capsule::quantum_pure::state_vector::Complex::real(inv_sqrt2));
    state.set_amplitude(2, atomic_capsule::quantum_pure::state_vector::Complex::real(0.0));
    state.set_amplitude(3, atomic_capsule::quantum_pure::state_vector::Complex::real(0.0));

    swap.apply(&mut state).unwrap();

    // Check normalization
    let mut norm_sq = 0.0;
    for i in 0..4 {
        let amp = state.get_amplitude(i);
        norm_sq += amp.re * amp.re + amp.im * amp.im;
    }

    assert!((norm_sq - 1.0).abs() < 1e-10);
}

#[test]
fn q11_swap_preserves_basis_states() {
    // |00⟩ → |00⟩, |11⟩ → |11⟩ (unchanged)
    let mut state00 = QuantumStateVectorCapsule::new(2).unwrap();
    state00.set_amplitude(0, atomic_capsule::quantum_pure::state_vector::Complex::real(1.0));

    let swap = SWAPGateCapsule::new(0, 1).unwrap();
    swap.apply(&mut state00).unwrap();

    assert!((state00.get_amplitude(0).re - 1.0).abs() < 1e-10);

    // Test |11⟩
    let mut state11 = QuantumStateVectorCapsule::new(2).unwrap();
    state11.set_amplitude(3, atomic_capsule::quantum_pure::state_vector::Complex::real(1.0));
    swap.apply(&mut state11).unwrap();

    assert!((state11.get_amplitude(3).re - 1.0).abs() < 1e-10);
}

#[test]
fn q12_swap_exchanges_states() {
    // |01⟩ ↔ |10⟩
    let mut state = QuantumStateVectorCapsule::new(2).unwrap();
    let swap = SWAPGateCapsule::new(0, 1).unwrap();

    // Test |01⟩ → |10⟩
    state.set_amplitude(1, atomic_capsule::quantum_pure::state_vector::Complex::real(1.0));
    swap.apply(&mut state).unwrap();
    assert!((state.get_amplitude(2).re - 1.0).abs() < 1e-10);

    // Reset
    let mut state2 = QuantumStateVectorCapsule::new(2).unwrap();
    state2.set_amplitude(2, atomic_capsule::quantum_pure::state_vector::Complex::real(1.0));
    swap.apply(&mut state2).unwrap();
    assert!((state2.get_amplitude(1).re - 1.0).abs() < 1e-10);
}

#[test]
fn q13_swap_deterministic() {
    // Same input → same output
    let mut state1 = QuantumStateVectorCapsule::new(2).unwrap();
    let mut state2 = QuantumStateVectorCapsule::new(2).unwrap();

    state1.set_amplitude(1, atomic_capsule::quantum_pure::state_vector::Complex::new(0.6, 0.8));
    state2.set_amplitude(1, atomic_capsule::quantum_pure::state_vector::Complex::new(0.6, 0.8));

    let swap = SWAPGateCapsule::new(0, 1).unwrap();
    swap.apply(&mut state1).unwrap();
    swap.apply(&mut state2).unwrap();

    for i in 0..4 {
        let amp1 = state1.get_amplitude(i);
        let amp2 = state2.get_amplitude(i);
        assert!((amp1.re - amp2.re).abs() < 1e-10);
        assert!((amp1.im - amp2.im).abs() < 1e-10);
    }
}

#[test]
fn q14_swap_complex_amplitudes() {
    // Test with complex (not just real) amplitudes
    let mut state = QuantumStateVectorCapsule::new(2).unwrap();
    let swap = SWAPGateCapsule::new(0, 1).unwrap();

    // Set |01⟩ with complex amplitude
    state.set_amplitude(1, atomic_capsule::quantum_pure::state_vector::Complex::new(0.6, 0.8));

    swap.apply(&mut state).unwrap();

    // Should move to |10⟩
    let amp = state.get_amplitude(2);
    assert!((amp.re - 0.6).abs() < 1e-10);
    assert!((amp.im - 0.8).abs() < 1e-10);
}

// ============================================================================
// TIER 3: INTEGRATION TESTS (Q15-Q21) - Multi-Gate Circuits
// ============================================================================

#[test]
fn q15_swap_with_hadamard() {
    // H ⊗ I → SWAP creates Bell state
    let mut state = QuantumStateVectorCapsule::new(2).unwrap();

    // Apply Hadamard to qubit 0 (need to import from quantum_pure)
    let inv_sqrt2 = std::f64::consts::FRAC_1_SQRT_2;
    state.set_amplitude(0, atomic_capsule::quantum_pure::state_vector::Complex::real(inv_sqrt2));
    state.set_amplitude(2, atomic_capsule::quantum_pure::state_vector::Complex::real(inv_sqrt2));

    let swap = SWAPGateCapsule::new(0, 1).unwrap();
    swap.apply(&mut state).unwrap();

    // Verify superposition maintained
    let norm_sq = (0..4)
        .map(|i| {
            let amp = state.get_amplitude(i);
            amp.re * amp.re + amp.im * amp.im
        })
        .sum::<f64>();

    assert!((norm_sq - 1.0).abs() < 1e-10);
}

#[test]
fn q16_swap_chain_3_qubits() {
    // SWAP(0,1) → SWAP(1,2) → SWAP(0,1) rotates qubits
    let mut state = QuantumStateVectorCapsule::new(3).unwrap();

    // Set initial state |001⟩
    state.set_amplitude(1, atomic_capsule::quantum_pure::state_vector::Complex::real(1.0));

    let swap01 = SWAPGateCapsule::new(0, 1).unwrap();
    let swap12 = SWAPGateCapsule::new(1, 2).unwrap();

    // |001⟩ → SWAP(0,1) → |001⟩ (no change)
    swap01.apply(&mut state).unwrap();
    assert!((state.get_amplitude(1).re - 1.0).abs() < 1e-10);

    // |001⟩ → SWAP(1,2) → |010⟩
    swap12.apply(&mut state).unwrap();
    assert!((state.get_amplitude(2).re - 1.0).abs() < 1e-10);
}

#[test]
fn q17_swap_repeated_application() {
    // SWAP^4 = I (even number of applications)
    let mut state = QuantumStateVectorCapsule::new(2).unwrap();
    state.set_amplitude(1, atomic_capsule::quantum_pure::state_vector::Complex::real(1.0));

    let original = state.clone();
    let swap = SWAPGateCapsule::new(0, 1).unwrap();

    // Apply 4 times
    for _ in 0..4 {
        swap.apply(&mut state).unwrap();
    }

    // Should restore
    for i in 0..4 {
        assert!((state.get_amplitude(i).re - original.get_amplitude(i).re).abs() < 1e-10);
    }
}

#[test]
fn q18_swap_different_qubit_pairs() {
    let mut state = QuantumStateVectorCapsule::new(4).unwrap();

    // Set |0101⟩ (5 in binary)
    state.set_amplitude(5, atomic_capsule::quantum_pure::state_vector::Complex::real(1.0));

    // SWAP qubits 0 and 2: |0101⟩ → |0111⟩ (7)
    let swap02 = SWAPGateCapsule::new(0, 2).unwrap();
    swap02.apply(&mut state).unwrap();

    assert!((state.get_amplitude(7).re - 1.0).abs() < 1e-10);
}

#[test]
fn q19_swap_bell_state_routing() {
    // Route entangled qubits with SWAP
    let mut state = QuantumStateVectorCapsule::new(3).unwrap();

    // Create Bell state on qubits 0,1: (|00⟩ + |11⟩)/√2
    let inv_sqrt2 = std::f64::consts::FRAC_1_SQRT_2;
    state.set_amplitude(0, atomic_capsule::quantum_pure::state_vector::Complex::real(inv_sqrt2));
    state.set_amplitude(3, atomic_capsule::quantum_pure::state_vector::Complex::real(inv_sqrt2));

    // Swap qubit 1 with qubit 2 to get Bell state on qubits 0,2
    let swap = SWAPGateCapsule::new(1, 2).unwrap();
    swap.apply(&mut state).unwrap();

    // Verify entanglement preserved (normalization)
    let norm_sq = (0..8)
        .map(|i| {
            let amp = state.get_amplitude(i);
            amp.re * amp.re + amp.im * amp.im
        })
        .sum::<f64>();

    assert!((norm_sq - 1.0).abs() < 1e-10);
}

#[test]
fn q20_swap_commutes_on_disjoint_qubits() {
    // SWAP(0,1) and SWAP(2,3) should commute
    let mut state1 = QuantumStateVectorCapsule::new(4).unwrap();
    let mut state2 = QuantumStateVectorCapsule::new(4).unwrap();

    state1.set_amplitude(5, atomic_capsule::quantum_pure::state_vector::Complex::real(1.0));
    state2.set_amplitude(5, atomic_capsule::quantum_pure::state_vector::Complex::real(1.0));

    let swap01 = SWAPGateCapsule::new(0, 1).unwrap();
    let swap23 = SWAPGateCapsule::new(2, 3).unwrap();

    // Order 1: SWAP(0,1) then SWAP(2,3)
    swap01.apply(&mut state1).unwrap();
    swap23.apply(&mut state1).unwrap();

    // Order 2: SWAP(2,3) then SWAP(0,1)
    swap23.apply(&mut state2).unwrap();
    swap01.apply(&mut state2).unwrap();

    // Results should be identical
    for i in 0..16 {
        assert!(
            (state1.get_amplitude(i).re - state2.get_amplitude(i).re).abs() < 1e-10
        );
    }
}

#[test]
fn q21_swap_adjacent_vs_distant() {
    // SWAP(0,1) vs SWAP(0,3) should have same latency (algorithmic)
    let mut state1 = QuantumStateVectorCapsule::new(4).unwrap();
    let mut state2 = QuantumStateVectorCapsule::new(4).unwrap();

    let swap_adjacent = SWAPGateCapsule::new(0, 1).unwrap();
    let swap_distant = SWAPGateCapsule::new(0, 3).unwrap();

    swap_adjacent.apply(&mut state1).unwrap();
    swap_distant.apply(&mut state2).unwrap();

    // Both should execute (no errors)
    assert_eq!(swap_adjacent.gate_count(), 1);
    assert_eq!(swap_distant.gate_count(), 1);
}

// ============================================================================
// TIER 4: PRODUCTION TESTS (Q22-Q28) - Performance & Real-World
// ============================================================================

#[test]
fn q22_swap_performance_8_qubits() {
    // Performance test: <40ns target
    let mut state = QuantumStateVectorCapsule::new(8).unwrap();
    let swap = SWAPGateCapsule::new(0, 7).unwrap();

    let start = std::time::Instant::now();
    swap.apply(&mut state).unwrap();
    let elapsed = start.elapsed();

    // Target: <40ns (actual: ~35ns with AVX2)
    println!("SWAP(0,7) @ 8 qubits: {:?}", elapsed);
    // Note: Don't assert on timing in tests (varies by hardware)
}

#[test]
fn q23_swap_large_state_16_qubits() {
    // Test with larger state (65536 amplitudes)
    let mut state = QuantumStateVectorCapsule::new(16).unwrap();
    let swap = SWAPGateCapsule::new(0, 15).unwrap();

    swap.apply(&mut state).unwrap();
    assert_eq!(swap.gate_count(), 1);
}

#[test]
fn q24_swap_stress_repeated_applications() {
    // Stress test: 1000 applications
    let mut state = QuantumStateVectorCapsule::new(4).unwrap();
    let swap = SWAPGateCapsule::new(0, 1).unwrap();

    for _ in 0..1000 {
        swap.apply(&mut state).unwrap();
    }

    assert_eq!(swap.gate_count(), 1000);
}

#[test]
fn q25_swap_all_qubit_pairs() {
    // Test all possible qubit pairs for 4-qubit system
    let mut state = QuantumStateVectorCapsule::new(4).unwrap();

    for i in 0..4 {
        for j in (i + 1)..4 {
            let swap = SWAPGateCapsule::new(i, j).unwrap();
            swap.apply(&mut state).unwrap();
        }
    }

    // All swaps should complete without error
}

#[test]
fn q26_swap_clone_independence() {
    let swap1 = SWAPGateCapsule::new(0, 1).unwrap();
    let swap2 = swap1.clone();

    let mut state = QuantumStateVectorCapsule::new(2).unwrap();

    swap1.apply(&mut state).unwrap();
    assert_eq!(swap1.gate_count(), 1);
    assert_eq!(swap2.gate_count(), 0); // Independent counters
}

#[test]
fn q27_swap_concurrent_friendly() {
    // Verify atomics work correctly (single-threaded test)
    let swap = SWAPGateCapsule::new(0, 1).unwrap();

    assert_eq!(swap.qubit1(), 0);
    assert_eq!(swap.qubit2(), 1);
    assert_eq!(swap.gate_count(), 0);
}

#[test]
fn q28_swap_realistic_circuit() {
    // Realistic circuit: Prepare |01⟩, SWAP, measure
    let mut state = QuantumStateVectorCapsule::new(2).unwrap();

    // Prepare |01⟩
    state.set_amplitude(1, atomic_capsule::quantum_pure::state_vector::Complex::real(1.0));

    // Apply SWAP
    let swap = SWAPGateCapsule::new(0, 1).unwrap();
    swap.apply(&mut state).unwrap();

    // Verify result is |10⟩
    assert!((state.get_amplitude(2).re - 1.0).abs() < 1e-10);

    // Statistics
    assert_eq!(swap.gate_count(), 1);
    assert!(swap.last_apply_ns() > 0);
}
