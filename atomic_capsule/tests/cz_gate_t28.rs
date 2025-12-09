//! T28 Comprehensive Tests for CZGateCapsule
//!
//! # T28 Framework: 4-Tier Testing Pyramid
//!
//! - **Q1-Q7 (Unit)**: Basic functionality, edge cases, error handling
//! - **Q8-Q14 (Property)**: Mathematical properties, invariants
//! - **Q15-Q21 (Integration)**: Multi-gate circuits, state composition
//! - **Q22-Q28 (Production)**: Performance, concurrency, stress testing
//!
//! # Total: 28 Tests (7 per tier)

use atomic_capsule::quantum_pure::cz_gate::CZGateCapsule;
use atomic_capsule::quantum_pure::error::QuantumPureError;

/// Helper: Compute normalization (Σ|amplitude|²)
fn compute_norm(real: &[f64], imag: &[f64]) -> f64 {
    real.iter()
        .zip(imag.iter())
        .map(|(r, i)| r * r + i * i)
        .sum()
}

/// Helper: Create equal superposition state (all amplitudes = 1/√N)
fn create_equal_superposition(num_amplitudes: usize) -> (Vec<f64>, Vec<f64>) {
    let amplitude = 1.0 / (num_amplitudes as f64).sqrt();
    let real = vec![amplitude; num_amplitudes];
    let imag = vec![0.0; num_amplitudes];
    (real, imag)
}

// ============================================================================
// Q1-Q7: UNIT TESTS (Basic Functionality, Edge Cases, Error Handling)
// ============================================================================

#[test]
fn q1_cz_gate_layout() {
    // Verify 128-byte cache-aligned layout
    assert_eq!(std::mem::size_of::<CZGateCapsule>(), 128);
    assert_eq!(std::mem::align_of::<CZGateCapsule>(), 128);
}

#[test]
fn q2_cz_creation_valid() {
    // Valid gate creation
    let gate = CZGateCapsule::new(0, 1).unwrap();
    assert_eq!(gate.qubit1(), 0);
    assert_eq!(gate.qubit2(), 1);
    assert_eq!(gate.gate_count(), 0);
    assert!(gate.is_symmetric());
}

#[test]
fn q3_cz_creation_invalid_same_qubit() {
    // Error: qubits must be different
    let result = CZGateCapsule::new(0, 0);
    assert!(result.is_err());
    match result {
        Err(QuantumPureError::InvalidGateParameters { gate_type, reason }) => {
            assert_eq!(gate_type, "CZ");
            assert!(reason.contains("different"));
        }
        _ => panic!("Expected InvalidGateParameters error"),
    }
}

#[test]
fn q4_cz_apply_00_state() {
    // CZ|00⟩ = |00⟩ (no change)
    let gate = CZGateCapsule::new(0, 1).unwrap();
    let mut real = vec![1.0, 0.0, 0.0, 0.0]; // |00⟩
    let mut imag = vec![0.0; 4];

    gate.apply(&mut real, &mut imag, 2).unwrap();

    assert_eq!(real, vec![1.0, 0.0, 0.0, 0.0]);
    assert_eq!(imag, vec![0.0; 4]);
    assert_eq!(gate.gate_count(), 1);
}

#[test]
fn q5_cz_apply_11_state() {
    // CZ|11⟩ = -|11⟩ (phase flip)
    let gate = CZGateCapsule::new(0, 1).unwrap();
    let mut real = vec![0.0, 0.0, 0.0, 1.0]; // |11⟩
    let mut imag = vec![0.0; 4];

    gate.apply(&mut real, &mut imag, 2).unwrap();

    assert_eq!(real, vec![0.0, 0.0, 0.0, -1.0]); // Phase flip
    assert_eq!(imag, vec![0.0; 4]);
    assert_eq!(gate.gate_count(), 1);
}

#[test]
fn q6_cz_apply_invalid_qubit_index() {
    // Error: qubit index >= num_qubits
    let gate = CZGateCapsule::new(0, 5).unwrap();
    let mut real = vec![1.0, 0.0, 0.0, 0.0];
    let mut imag = vec![0.0; 4];

    let result = gate.apply(&mut real, &mut imag, 2); // num_qubits = 2, but gate uses qubit 5
    assert!(result.is_err());
    match result {
        Err(QuantumPureError::InvalidQubitIndex { index, num_qubits }) => {
            assert_eq!(index, 5);
            assert_eq!(num_qubits, 2);
        }
        _ => panic!("Expected InvalidQubitIndex error"),
    }
}

#[test]
fn q7_cz_gate_counter_increment() {
    // Gate counter increments on each application
    let gate = CZGateCapsule::new(0, 1).unwrap();
    let mut real = vec![1.0, 0.0, 0.0, 0.0];
    let mut imag = vec![0.0; 4];

    assert_eq!(gate.gate_count(), 0);

    gate.apply(&mut real, &mut imag, 2).unwrap();
    assert_eq!(gate.gate_count(), 1);

    gate.apply(&mut real, &mut imag, 2).unwrap();
    assert_eq!(gate.gate_count(), 2);

    gate.apply(&mut real, &mut imag, 2).unwrap();
    assert_eq!(gate.gate_count(), 3);
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS (Mathematical Properties, Invariants)
// ============================================================================

#[test]
fn q8_cz_preserves_normalization() {
    // Property: CZ is unitary → preserves normalization
    let gate = CZGateCapsule::new(0, 1).unwrap();

    // Superposition: (|00⟩ + |01⟩ + |10⟩ + |11⟩) / 2
    let mut real = vec![0.5, 0.5, 0.5, 0.5];
    let mut imag = vec![0.0; 4];

    let initial_norm = compute_norm(&real, &imag);
    gate.apply(&mut real, &mut imag, 2).unwrap();
    let final_norm = compute_norm(&real, &imag);

    assert!((initial_norm - final_norm).abs() < 1e-10);
    assert!((final_norm - 1.0).abs() < 1e-10);
}

#[test]
fn q9_cz_symmetry_property() {
    // Property: CZ(i,j) = CZ(j,i) (symmetric gate)
    let gate_01 = CZGateCapsule::new(0, 1).unwrap();
    let gate_10 = CZGateCapsule::new(1, 0).unwrap();

    let mut real1 = vec![0.5, 0.5, 0.5, 0.5];
    let mut imag1 = vec![0.0; 4];

    let mut real2 = real1.clone();
    let mut imag2 = imag1.clone();

    gate_01.apply(&mut real1, &mut imag1, 2).unwrap();
    gate_10.apply(&mut real2, &mut imag2, 2).unwrap();

    // Results should be identical
    for i in 0..4 {
        assert!((real1[i] - real2[i]).abs() < 1e-10_f64);
        assert!((imag1[i] - imag2[i]).abs() < 1e-10_f64);
    }
}

#[test]
fn q10_cz_idempotent_property() {
    // Property: CZ² = I (applying CZ twice returns original state)
    let gate = CZGateCapsule::new(0, 1).unwrap();

    let original_real = vec![0.5, 0.3, 0.2, 0.1];
    let original_imag = vec![0.1, 0.2, 0.3, 0.4];

    let mut real = original_real.clone();
    let mut imag = original_imag.clone();

    // Apply CZ twice
    gate.apply(&mut real, &mut imag, 2).unwrap();
    gate.apply(&mut real, &mut imag, 2).unwrap();

    // Should return to original state
    for i in 0..4 {
        assert!((real[i] - original_real[i]).abs() < 1e-10_f64);
        assert!((imag[i] - original_imag[i]).abs() < 1e-10_f64);
    }
}

#[test]
fn q11_cz_diagonal_property() {
    // Property: CZ is diagonal → only affects |11⟩ basis state
    let gate = CZGateCapsule::new(0, 1).unwrap();

    // Test all 4 basis states
    let basis_states = vec![
        (vec![1.0, 0.0, 0.0, 0.0], vec![0.0; 4], 0), // |00⟩
        (vec![0.0, 1.0, 0.0, 0.0], vec![0.0; 4], 1), // |01⟩
        (vec![0.0, 0.0, 1.0, 0.0], vec![0.0; 4], 2), // |10⟩
        (vec![0.0, 0.0, 0.0, 1.0], vec![0.0; 4], 3), // |11⟩
    ];

    for (mut real, mut imag, idx) in basis_states {
        let original_real = real.clone();
        let original_imag = imag.clone();

        gate.apply(&mut real, &mut imag, 2).unwrap();

        if idx == 3 {
            // |11⟩ should be negated
            assert_eq!(real[3], -original_real[3]);
        } else {
            // Other basis states unchanged
            assert_eq!(real, original_real);
            assert_eq!(imag, original_imag);
        }
    }
}

#[test]
fn q12_cz_phase_only_property() {
    // Property: CZ only affects phases, not amplitudes magnitudes
    let gate = CZGateCapsule::new(0, 1).unwrap();

    let mut real = vec![0.5, 0.3, 0.2, 0.1];
    let mut imag = vec![0.1, 0.2, 0.3, 0.4];

    // Compute magnitudes before
    let magnitudes_before: Vec<f64> = real
        .iter()
        .zip(imag.iter())
        .map(|(r, i)| (*r * *r + *i * *i).sqrt())
        .collect();

    gate.apply(&mut real, &mut imag, 2).unwrap();

    // Compute magnitudes after
    let magnitudes_after: Vec<f64> = real
        .iter()
        .zip(imag.iter())
        .map(|(r, i)| (*r * *r + *i * *i).sqrt())
        .collect();

    // Magnitudes should be unchanged (only phases affected)
    for i in 0..4 {
        assert!((magnitudes_before[i] - magnitudes_after[i]).abs() < 1e-10_f64);
    }
}

#[test]
fn q13_cz_commutes_with_itself() {
    // Property: CZ commutes with itself on same qubits
    let gate1 = CZGateCapsule::new(0, 1).unwrap();
    let gate2 = CZGateCapsule::new(0, 1).unwrap();

    let mut real1 = vec![0.5, 0.3, 0.2, 0.1];
    let mut imag1 = vec![0.1, 0.2, 0.3, 0.4];

    let mut real2 = real1.clone();
    let mut imag2 = imag1.clone();

    // Apply gate1 then gate2
    gate1.apply(&mut real1, &mut imag1, 2).unwrap();
    gate2.apply(&mut real1, &mut imag1, 2).unwrap();

    // Apply gate2 then gate1
    gate2.apply(&mut real2, &mut imag2, 2).unwrap();
    gate1.apply(&mut real2, &mut imag2, 2).unwrap();

    // Results should be identical
    for i in 0..4 {
        assert!((real1[i] - real2[i]).abs() < 1e-10_f64);
        assert!((imag1[i] - imag2[i]).abs() < 1e-10_f64);
    }
}

#[test]
fn q14_cz_hermitian_property() {
    // Property: CZ is Hermitian (CZ† = CZ)
    // This means CZ is self-inverse (tested in q10)
    let gate = CZGateCapsule::new(0, 1).unwrap();

    let mut real = vec![0.5, 0.3, 0.2, 0.1];
    let mut imag = vec![0.1, 0.2, 0.3, 0.4];

    let original_real = real.clone();
    let original_imag = imag.clone();

    // CZ†CZ = I → applying twice returns original
    gate.apply(&mut real, &mut imag, 2).unwrap();
    gate.apply(&mut real, &mut imag, 2).unwrap();

    for i in 0..4 {
        assert!((real[i] - original_real[i]).abs() < 1e-10_f64);
        assert!((imag[i] - original_imag[i]).abs() < 1e-10_f64);
    }
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS (Multi-Gate Circuits, State Composition)
// ============================================================================

#[test]
fn q15_cz_multi_qubit_system() {
    // Integration: CZ on 4-qubit system (16 amplitudes)
    let gate = CZGateCapsule::new(0, 1).unwrap();

    let (mut real, mut imag) = create_equal_superposition(16);

    let initial_norm = compute_norm(&real, &imag);
    gate.apply(&mut real, &mut imag, 4).unwrap();
    let final_norm = compute_norm(&real, &imag);

    // Normalization preserved
    assert!((initial_norm - final_norm).abs() < 1e-10);

    // Amplitudes where q0=1 AND q1=1 should be negated
    // Indices: 3, 7, 11, 15 (binary: 0011, 0111, 1011, 1111)
    let negated_indices = vec![3, 7, 11, 15];
    for i in 0..16 {
        if negated_indices.contains(&i) {
            assert!(real[i] < 0.0); // Should be negative
        } else {
            assert!(real[i] > 0.0); // Should be positive
        }
    }
}

#[test]
fn q16_cz_sequential_different_qubits() {
    // Integration: Apply CZ on different qubit pairs sequentially
    let gate01 = CZGateCapsule::new(0, 1).unwrap();
    let gate12 = CZGateCapsule::new(1, 2).unwrap();

    // 3-qubit system (8 amplitudes)
    let (mut real, mut imag) = create_equal_superposition(8);

    gate01.apply(&mut real, &mut imag, 3).unwrap();
    gate12.apply(&mut real, &mut imag, 3).unwrap();

    // Normalization preserved
    let final_norm = compute_norm(&real, &imag);
    assert!((final_norm - 1.0).abs() < 1e-10);
}

#[test]
fn q17_cz_graph_state_creation() {
    // Integration: Create 3-qubit graph state (linear cluster)
    // Graph: 0—1—2 (edges: 0-1, 1-2)
    let gate01 = CZGateCapsule::new(0, 1).unwrap();
    let gate12 = CZGateCapsule::new(1, 2).unwrap();

    // Start with equal superposition
    let (mut real, mut imag) = create_equal_superposition(8);

    // Apply CZ gates to create graph state
    gate01.apply(&mut real, &mut imag, 3).unwrap();
    gate12.apply(&mut real, &mut imag, 3).unwrap();

    // Verify normalization
    let norm = compute_norm(&real, &imag);
    assert!((norm - 1.0).abs() < 1e-10);

    // Verify specific phase pattern (graph state structure)
    // Indices with odd number of 1s in positions 0-1 and 1-2 should be negated
    let expected_signs = vec![1.0, 1.0, 1.0, -1.0, 1.0, -1.0, -1.0, 1.0];
    for i in 0..8 {
        assert_eq!(real[i].signum(), expected_signs[i]);
    }
}

#[test]
fn q18_cz_bell_state_preparation() {
    // Integration: CZ can create Bell states (after Hadamard)
    // |Φ+⟩ = (|00⟩ + |11⟩)/√2 (maximally entangled)
    let gate = CZGateCapsule::new(0, 1).unwrap();

    // Simulate post-Hadamard state: (|00⟩ + |01⟩ + |10⟩ + |11⟩)/2
    let mut real = vec![0.5, 0.5, 0.5, 0.5];
    let mut imag = vec![0.0; 4];

    gate.apply(&mut real, &mut imag, 2).unwrap();

    // Result: (|00⟩ + |01⟩ + |10⟩ - |11⟩)/2
    assert_eq!(real[0], 0.5);
    assert_eq!(real[1], 0.5);
    assert_eq!(real[2], 0.5);
    assert_eq!(real[3], -0.5);

    // Normalization preserved
    let norm = compute_norm(&real, &imag);
    assert!((norm - 1.0).abs() < 1e-10);
}

#[test]
fn q19_cz_complex_amplitudes() {
    // Integration: CZ on state with complex amplitudes
    let gate = CZGateCapsule::new(0, 1).unwrap();

    let mut real = vec![0.5, 0.3, 0.2, 0.1];
    let mut imag = vec![0.1, 0.2, 0.3, 0.4];

    let initial_norm = compute_norm(&real, &imag);

    gate.apply(&mut real, &mut imag, 2).unwrap();

    // Verify |11⟩ amplitude (index 3) is negated
    assert_eq!(real[3], -0.1);
    assert_eq!(imag[3], -0.4);

    // Verify normalization preserved
    let final_norm = compute_norm(&real, &imag);
    assert!((initial_norm - final_norm).abs() < 1e-10);
}

#[test]
fn q20_cz_large_qubit_count() {
    // Integration: CZ on 8-qubit system (256 amplitudes)
    let gate = CZGateCapsule::new(3, 5).unwrap();

    let (mut real, mut imag) = create_equal_superposition(256);

    gate.apply(&mut real, &mut imag, 8).unwrap();

    // Verify normalization
    let norm = compute_norm(&real, &imag);
    assert!((norm - 1.0).abs() < 1e-9);

    // Verify specific amplitudes are negated
    // Indices where q3=1 AND q5=1: (bit 3 = 1, bit 5 = 1)
    // Example: 0b00101000 = 40
    let idx = (1 << 3) | (1 << 5); // 8 + 32 = 40
    assert!(real[idx] < 0.0);
}

#[test]
fn q21_cz_reverse_qubit_order() {
    // Integration: CZ with reversed qubit indices (symmetry test)
    let gate_01 = CZGateCapsule::new(0, 1).unwrap();
    let gate_10 = CZGateCapsule::new(1, 0).unwrap();

    let (mut real1, mut imag1) = create_equal_superposition(16);
    let (mut real2, mut imag2) = (real1.clone(), imag1.clone());

    gate_01.apply(&mut real1, &mut imag1, 4).unwrap();
    gate_10.apply(&mut real2, &mut imag2, 4).unwrap();

    // Results should be identical (symmetry)
    for i in 0..16 {
        assert!((real1[i] - real2[i]).abs() < 1e-10);
        assert!((imag1[i] - imag2[i]).abs() < 1e-10);
    }
}

// ============================================================================
// Q22-Q28: PRODUCTION TESTS (Performance, Concurrency, Stress Testing)
// ============================================================================

#[test]
fn q22_cz_20_qubit_stress_test() {
    // Production: Stress test with 20 qubits (1M amplitudes)
    let gate = CZGateCapsule::new(10, 15).unwrap();

    let (mut real, mut imag) = create_equal_superposition(1 << 20); // 1M amplitudes

    let start = std::time::Instant::now();
    gate.apply(&mut real, &mut imag, 20).unwrap();
    let duration = start.elapsed();

    // Performance target: <10ms @ 20 qubits (scalar baseline)
    assert!(duration.as_millis() < 10);

    // Verify normalization
    let norm = compute_norm(&real, &imag);
    assert!((norm - 1.0).abs() < 1e-8);
}

#[test]
#[cfg(feature = "portable_simd")]
fn q23_cz_avx2_correctness() {
    // Production: AVX2 matches scalar results
    let gate = CZGateCapsule::new(0, 1).unwrap();

    let (mut real_scalar, mut imag_scalar) = create_equal_superposition(256);
    let (mut real_avx2, mut imag_avx2) = (real_scalar.clone(), imag_scalar.clone());

    // Apply scalar
    gate.apply(&mut real_scalar, &mut imag_scalar, 8).unwrap();

    // Apply AVX2
    gate.apply_avx2(&mut real_avx2, &mut imag_avx2, 8).unwrap();

    // Results should match exactly
    for i in 0..256 {
        assert!((real_scalar[i] - real_avx2[i]).abs() < 1e-10);
        assert!((imag_scalar[i] - imag_avx2[i]).abs() < 1e-10);
    }
}

#[test]
#[cfg(feature = "portable_simd")]
fn q24_cz_avx2_performance() {
    // Production: AVX2 speedup verification
    let gate = CZGateCapsule::new(5, 10).unwrap();

    let (mut real, mut imag) = create_equal_superposition(1 << 16); // 64K amplitudes

    // Warm-up
    gate.apply_avx2(&mut real, &mut imag, 16).unwrap();

    // Benchmark
    let start = std::time::Instant::now();
    for _ in 0..100 {
        gate.apply_avx2(&mut real, &mut imag, 16).unwrap();
    }
    let duration = start.elapsed();

    // Performance target: <5ms for 100 iterations @ 16 qubits
    assert!(duration.as_millis() < 5);
}

#[test]
fn q25_cz_concurrent_read_access() {
    // Production: Concurrent reads of gate properties (thread-safe)
    use std::sync::Arc;
    use std::thread;

    let gate = Arc::new(CZGateCapsule::new(0, 1).unwrap());

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let gate_clone: Arc<CZGateCapsule> = Arc::clone(&gate);
            thread::spawn(move || {
                // Concurrent reads should be safe
                assert_eq!(gate_clone.qubit1(), 0);
                assert_eq!(gate_clone.qubit2(), 1);
                assert!(gate_clone.is_symmetric());
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn q26_cz_sequential_applications() {
    // Production: Multiple sequential applications
    let gate = CZGateCapsule::new(0, 1).unwrap();

    let (mut real, mut imag) = create_equal_superposition(256);

    // Apply 1000 times (should return to original state due to idempotency)
    for _ in 0..1000 {
        gate.apply(&mut real, &mut imag, 8).unwrap();
    }

    // 1000 applications (even number) → original state
    let expected = 1.0 / (256.0_f64).sqrt();
    for i in 0..256 {
        assert!((real[i].abs() - expected).abs() < 1e-8);
    }

    assert_eq!(gate.gate_count(), 1000);
}

#[test]
fn q27_cz_error_recovery() {
    // Production: Error handling doesn't corrupt state
    let gate = CZGateCapsule::new(0, 10).unwrap(); // qubit 10 invalid for 2-qubit system

    let mut real = vec![1.0, 0.0, 0.0, 0.0];
    let mut imag = vec![0.0; 4];

    let original_real = real.clone();
    let original_imag = imag.clone();

    // Apply with invalid num_qubits (should error)
    let result = gate.apply(&mut real, &mut imag, 2);
    assert!(result.is_err());

    // State should be unchanged (error recovery)
    assert_eq!(real, original_real);
    assert_eq!(imag, original_imag);

    // Gate counter should not increment on error
    assert_eq!(gate.gate_count(), 0);
}

#[test]
fn q28_cz_memory_alignment_verification() {
    // Production: Verify cache alignment in practice
    let gate = CZGateCapsule::new(0, 1).unwrap();

    // Get raw pointer
    let ptr = &gate as *const CZGateCapsule as usize;

    // Verify 128-byte alignment
    assert_eq!(ptr % 128, 0, "CZGateCapsule not aligned to 128 bytes");

    // Verify fields are at expected offsets
    unsafe {
        let qubit1_ptr = &gate.qubit1 as *const _ as usize;
        let qubit2_ptr = &gate.qubit2 as *const _ as usize;
        let gate_count_ptr = &gate.gate_count as *const _ as usize;

        assert_eq!(qubit1_ptr - ptr, 0); // Offset 0
        assert_eq!(qubit2_ptr - ptr, 4); // Offset 4
        assert_eq!(gate_count_ptr - ptr, 8); // Offset 8
    }
}
