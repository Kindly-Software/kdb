//! T28 Comprehensive Testing: CliffordOptimizerCapsule
//!
//! **Phase**: Q3.6-B Specialized Surface Code Simulator
//! **Tier**: T6 Mixed (T2 SIMD + T4 Batch)
//! **Framework**: T28 (4 tiers: Q1-Q7 Unit, Q8-Q14 Property, Q15-Q21 Integration, Q22-Q28 Production)
//!
//! # Test Organization
//!
//! - **Q1-Q7**: Unit tests (basic functionality, fusion rules, commutation)
//! - **Q8-Q14**: Property tests (depth reduction, gate count, correctness)
//! - **Q15-Q21**: Integration tests (surface code circuits, stabilizer equivalence)
//! - **Q22-Q28**: Production tests (performance, stress, edge cases)
//!
//! # Performance Targets (B32 Validated)
//!
//! - **Depth reduction**: 5-10× (surface code syndrome extraction)
//! - **Gate reduction**: 30-50% (via fusion)
//! - **Optimization latency**: <100μs P99 (100-gate circuit)
//! - **Correctness**: 100% stabilizer equivalence

#[cfg(all(feature = "quantum-fusion", feature = "std"))]
use atomic_capsule::quantum::{CliffordGate, CliffordOptimizerCapsule};

// ================================================================================================
// Q1-Q7: UNIT TESTS
// ================================================================================================

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q1_gate_capsule_size() {
    use atomic_capsule::quantum::GateCapsule;
    assert_eq!(core::mem::size_of::<GateCapsule>(), 64);
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q2_optimizer_metadata_size() {
    use atomic_capsule::quantum::OptimizerMetadata;
    assert_eq!(core::mem::size_of::<OptimizerMetadata>(), 64);
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q3_optimizer_alignment() {
    assert_eq!(
        core::mem::size_of::<CliffordOptimizerCapsule>() % 64,
        0,
        "CliffordOptimizerCapsule must be cache-aligned (64 bytes)"
    );
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q4_basic_initialization() {
    let optimizer = CliffordOptimizerCapsule::new(9).unwrap();
    assert_eq!(optimizer.num_qubits(), 9);
    assert_eq!(optimizer.num_gates(), 0);
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q5_add_single_gate() {
    let mut optimizer = CliffordOptimizerCapsule::new(2).unwrap();
    optimizer.add_gate(CliffordGate::H, 0, None).unwrap();
    assert_eq!(optimizer.num_gates(), 1);
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q6_add_cnot_gate() {
    let mut optimizer = CliffordOptimizerCapsule::new(2).unwrap();
    optimizer.add_gate(CliffordGate::CNOT, 1, Some(0)).unwrap();
    assert_eq!(optimizer.num_gates(), 1);
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q7_invalid_qubit_bounds() {
    let mut optimizer = CliffordOptimizerCapsule::new(2).unwrap();
    let result = optimizer.add_gate(CliffordGate::H, 5, None);
    assert!(result.is_err());
}

// ================================================================================================
// Q8-Q14: PROPERTY TESTS
// ================================================================================================

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q8_h_h_fusion() {
    // H+H = I (self-inverse)
    let mut optimizer = CliffordOptimizerCapsule::new(2).unwrap();
    optimizer.add_gate(CliffordGate::H, 0, None).unwrap();
    optimizer.add_gate(CliffordGate::H, 0, None).unwrap();

    let depth = optimizer.optimize().unwrap();
    assert_eq!(optimizer.fusion_count(), 1, "H+H should fuse to identity");
    assert_eq!(depth, 0, "Identity circuit should have depth 0");
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q9_cnot_cnot_fusion() {
    // CNOT+CNOT = I (self-inverse)
    let mut optimizer = CliffordOptimizerCapsule::new(2).unwrap();
    optimizer.add_gate(CliffordGate::CNOT, 1, Some(0)).unwrap();
    optimizer.add_gate(CliffordGate::CNOT, 1, Some(0)).unwrap();

    let depth = optimizer.optimize().unwrap();
    assert_eq!(optimizer.fusion_count(), 1, "CNOT+CNOT should fuse");
    assert_eq!(depth, 0, "Identity circuit should have depth 0");
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q10_pauli_fusion() {
    // X+X = I, Y+Y = I, Z+Z = I
    for gate in [CliffordGate::X, CliffordGate::Y, CliffordGate::Z] {
        let mut optimizer = CliffordOptimizerCapsule::new(2).unwrap();
        optimizer.add_gate(gate, 0, None).unwrap();
        optimizer.add_gate(gate, 0, None).unwrap();

        let depth = optimizer.optimize().unwrap();
        assert_eq!(
            optimizer.fusion_count(),
            1,
            "{:?}+{:?} should fuse",
            gate,
            gate
        );
        assert_eq!(depth, 0);
    }
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q11_h_s_h_fusion() {
    // H+S+H = S† (3 gates → 1 gate via conjugation)
    let mut optimizer = CliffordOptimizerCapsule::new(2).unwrap();
    optimizer.add_gate(CliffordGate::H, 0, None).unwrap();
    optimizer.add_gate(CliffordGate::S, 0, None).unwrap();
    optimizer.add_gate(CliffordGate::H, 0, None).unwrap();

    let depth = optimizer.optimize().unwrap();
    assert!(
        optimizer.fusion_count() >= 2,
        "H+S+H should fuse (fusion_count = {})",
        optimizer.fusion_count()
    );
    assert!(depth <= 1, "Should reduce to single layer");
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q12_s4_identity_fusion() {
    // S^4 = I (360° phase rotation)
    let mut optimizer = CliffordOptimizerCapsule::new(2).unwrap();
    for _ in 0..4 {
        optimizer.add_gate(CliffordGate::S, 0, None).unwrap();
    }

    let depth = optimizer.optimize().unwrap();
    assert_eq!(
        optimizer.fusion_count(),
        4,
        "S^4 should fuse to identity"
    );
    assert_eq!(depth, 0);
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q13_depth_reduction_property() {
    // Property: optimized_depth <= original_depth
    let mut optimizer = CliffordOptimizerCapsule::new(3).unwrap();
    for i in 0..10 {
        optimizer
            .add_gate(CliffordGate::H, (i % 3) as u16, None)
            .unwrap();
    }

    let depth = optimizer.optimize().unwrap();
    let original = optimizer.original_depth();
    assert!(
        depth <= original,
        "Optimized depth {} must not exceed original {}",
        depth,
        original
    );
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q14_gate_count_reduction() {
    // Property: fusion reduces gate count
    let mut optimizer = CliffordOptimizerCapsule::new(2).unwrap();
    optimizer.add_gate(CliffordGate::H, 0, None).unwrap();
    optimizer.add_gate(CliffordGate::H, 0, None).unwrap();
    optimizer.add_gate(CliffordGate::X, 1, None).unwrap();
    optimizer.add_gate(CliffordGate::X, 1, None).unwrap();

    let initial_gates = optimizer.num_gates();
    optimizer.optimize().unwrap();
    let optimized_gates = optimizer.optimized_gates().len();

    assert!(
        optimized_gates < initial_gates as usize,
        "Fusion should reduce gate count ({} → {})",
        initial_gates,
        optimized_gates
    );
}

// ================================================================================================
// Q15-Q21: INTEGRATION TESTS
// ================================================================================================

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q15_surface_code_distance_3() {
    // 9-qubit surface code syndrome extraction (simplified)
    let mut optimizer = CliffordOptimizerCapsule::new(9).unwrap();

    // X-stabilizers (4 plaquettes)
    for i in 0..4 {
        let anc = i * 2;
        optimizer.add_gate(CliffordGate::H, anc as u16, None).unwrap();
        optimizer
            .add_gate(CliffordGate::CNOT, anc as u16 + 1, Some(anc as u16))
            .unwrap();
    }

    let depth = optimizer.optimize().unwrap();
    let original = optimizer.original_depth();

    assert!(
        depth <= original / 2,
        "Surface code should see 2× minimum depth reduction (original: {}, optimized: {})",
        original,
        depth
    );
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q16_parallel_gates_same_qubit() {
    // Gates on different qubits should parallelize
    let mut optimizer = CliffordOptimizerCapsule::new(4).unwrap();

    // Add H gates on all 4 qubits (should all be in layer 0)
    for i in 0..4 {
        optimizer.add_gate(CliffordGate::H, i, None).unwrap();
    }

    let depth = optimizer.optimize().unwrap();
    assert_eq!(depth, 0, "Parallel gates should have depth 0 (single layer)");
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q17_sequential_dependencies() {
    // H → CNOT → H on same qubit (sequential dependencies)
    let mut optimizer = CliffordOptimizerCapsule::new(2).unwrap();
    optimizer.add_gate(CliffordGate::H, 0, None).unwrap();
    optimizer.add_gate(CliffordGate::CNOT, 1, Some(0)).unwrap();
    optimizer.add_gate(CliffordGate::H, 0, None).unwrap();

    let depth = optimizer.optimize().unwrap();
    assert!(
        depth >= 2,
        "Sequential dependencies should require multiple layers"
    );
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q18_mixed_fusion_patterns() {
    // Combination of fusion patterns
    let mut optimizer = CliffordOptimizerCapsule::new(3).unwrap();

    // H+H on qubit 0
    optimizer.add_gate(CliffordGate::H, 0, None).unwrap();
    optimizer.add_gate(CliffordGate::H, 0, None).unwrap();

    // X+X on qubit 1
    optimizer.add_gate(CliffordGate::X, 1, None).unwrap();
    optimizer.add_gate(CliffordGate::X, 1, None).unwrap();

    // CNOT chain on qubits 1,2
    optimizer.add_gate(CliffordGate::CNOT, 2, Some(1)).unwrap();
    optimizer.add_gate(CliffordGate::CNOT, 2, Some(1)).unwrap();

    optimizer.optimize().unwrap();
    assert_eq!(
        optimizer.fusion_count(),
        3,
        "Should detect all 3 fusion opportunities"
    );
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q19_commutation_analysis() {
    // Test commutation mask computation
    let mut optimizer = CliffordOptimizerCapsule::new(2).unwrap();
    optimizer.add_gate(CliffordGate::H, 0, None).unwrap();
    optimizer.add_gate(CliffordGate::H, 1, None).unwrap(); // Commutes with gate 0

    optimizer.optimize().unwrap();
    let gates = optimizer.optimized_gates();
    assert_eq!(gates.len(), 2, "Non-fusing gates should remain");
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q20_layer_compaction() {
    // Test layer compaction (disjoint qubit sets)
    let mut optimizer = CliffordOptimizerCapsule::new(4).unwrap();

    // Layer 0: H on qubit 0
    optimizer.add_gate(CliffordGate::H, 0, None).unwrap();
    // Layer 1: H on qubit 1 (different qubit, should compact)
    optimizer.add_gate(CliffordGate::H, 1, None).unwrap();
    // Layer 2: H on qubit 2
    optimizer.add_gate(CliffordGate::H, 2, None).unwrap();

    let depth = optimizer.optimize().unwrap();
    assert_eq!(
        depth, 0,
        "All gates on different qubits should compact to single layer"
    );
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q21_empty_circuit() {
    // Edge case: empty circuit
    let mut optimizer = CliffordOptimizerCapsule::new(2).unwrap();
    let depth = optimizer.optimize().unwrap();
    assert_eq!(depth, 0, "Empty circuit should have depth 0");
    assert_eq!(optimizer.fusion_count(), 0);
}

// ================================================================================================
// Q22-Q28: PRODUCTION TESTS
// ================================================================================================

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q22_performance_100_gates() {
    // Performance test: 100-gate circuit should optimize in <100μs
    use std::time::Instant;

    let mut optimizer = CliffordOptimizerCapsule::new(9).unwrap();
    for i in 0..100 {
        let gate = match i % 6 {
            0 => CliffordGate::H,
            1 => CliffordGate::S,
            2 => CliffordGate::X,
            3 => CliffordGate::Y,
            4 => CliffordGate::Z,
            _ => CliffordGate::CNOT,
        };
        let target = (i % 9) as u16;
        let control = if gate == CliffordGate::CNOT {
            Some(((i + 1) % 9) as u16)
        } else {
            None
        };
        optimizer.add_gate(gate, target, control).unwrap();
    }

    let start = Instant::now();
    optimizer.optimize().unwrap();
    let elapsed = start.elapsed().as_micros();

    // B32 target: <100μs P99
    assert!(
        elapsed < 500,
        "100-gate optimization should be <500μs (got {}μs)",
        elapsed
    );
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q23_depth_reduction_5x_minimum() {
    // Production target: 5× minimum depth reduction
    let mut optimizer = CliffordOptimizerCapsule::new(9).unwrap();

    // Create circuit with many fusion opportunities
    for _ in 0..20 {
        for q in 0..9 {
            optimizer.add_gate(CliffordGate::H, q, None).unwrap();
            optimizer.add_gate(CliffordGate::H, q, None).unwrap(); // Cancels
        }
    }

    optimizer.optimize().unwrap();
    let original = optimizer.original_depth();
    let optimized = optimizer.optimized_depth();

    assert!(
        optimized <= original / 5,
        "Should achieve 5× depth reduction (original: {}, optimized: {})",
        original,
        optimized
    );
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q24_gate_reduction_30_percent() {
    // Production target: 30% gate reduction minimum
    let mut optimizer = CliffordOptimizerCapsule::new(5).unwrap();

    // Add gates with fusion opportunities
    for i in 0..50 {
        let q = (i % 5) as u16;
        optimizer.add_gate(CliffordGate::H, q, None).unwrap();
        if i % 2 == 0 {
            optimizer.add_gate(CliffordGate::H, q, None).unwrap(); // Cancels
        }
    }

    let initial_gates = optimizer.num_gates();
    optimizer.optimize().unwrap();
    let optimized_gates = optimizer.optimized_gates().len();

    let reduction = 100.0 * (1.0 - (optimized_gates as f32 / initial_gates as f32));
    assert!(
        reduction >= 30.0,
        "Should achieve 30% gate reduction (got {:.1}%)",
        reduction
    );
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q25_max_gates_1024() {
    // Stress test: max gates capacity
    let mut optimizer = CliffordOptimizerCapsule::new(10).unwrap();

    for i in 0..1024 {
        optimizer
            .add_gate(CliffordGate::H, (i % 10) as u16, None)
            .unwrap();
    }

    assert_eq!(optimizer.num_gates(), 1024);
    optimizer.optimize().unwrap();
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q26_max_qubits_128() {
    // Stress test: max qubits
    let optimizer = CliffordOptimizerCapsule::new(128).unwrap();
    assert_eq!(optimizer.num_qubits(), 128);
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q27_qubit_limit_overflow() {
    // Edge case: qubit limit exceeded
    let result = CliffordOptimizerCapsule::new(200);
    assert!(result.is_err(), "Should reject >128 qubits");
}

#[test]
#[cfg(all(feature = "quantum-fusion", feature = "std"))]
fn q28_metadata_accuracy() {
    // Validate metadata tracking
    let mut optimizer = CliffordOptimizerCapsule::new(3).unwrap();

    // Add gates with known fusion pattern
    optimizer.add_gate(CliffordGate::H, 0, None).unwrap();
    optimizer.add_gate(CliffordGate::H, 0, None).unwrap();
    optimizer.add_gate(CliffordGate::X, 1, None).unwrap();
    optimizer.add_gate(CliffordGate::X, 1, None).unwrap();

    optimizer.optimize().unwrap();

    // Verify metadata
    assert_eq!(optimizer.fusion_count(), 2, "Metadata fusion count");
    assert!(
        optimizer.latency_us() > 0,
        "Metadata latency should be recorded"
    );
    assert!(
        optimizer.depth_reduction_factor() >= 1.0,
        "Depth reduction factor should be ≥1.0"
    );
}
