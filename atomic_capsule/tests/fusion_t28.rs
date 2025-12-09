//! T28 Comprehensive Testing: Gate Fusion Optimization
//!
//! # Test Pyramid (28 tests)
//!
//! - **Q1-Q7 (Unit)**: Pattern matching, individual fusion rules, basic correctness
//! - **Q8-Q14 (Property)**: Equivalence verification, convergence, idempotence
//! - **Q15-Q21 (Integration)**: Real circuits (Grover, QFT), multi-pattern fusion
//! - **Q22-Q28 (Production)**: Performance validation, stress testing, metrics

#![cfg(feature = "quantum-fusion")]

use atomic_capsule::quantum::fusion::{GateFusionCapsule, GateType, QuantumCircuit};
use std::f64::consts::PI;

// ============================================================================
// Q1-Q7: UNIT TESTS
// ============================================================================

#[test]
fn q1_unit_cnot_cancellation_basic() {
    let fusion = GateFusionCapsule::new();
    let mut circuit = QuantumCircuit::new(2, "cnot-cancel");
    circuit.add_gate(GateType::CNOT { control: 0, target: 1 });
    circuit.add_gate(GateType::CNOT { control: 0, target: 1 });

    let optimized = fusion.optimize(circuit).unwrap();
    assert_eq!(optimized.gates.len(), 0, "CNOT · CNOT should cancel completely");
    assert_eq!(fusion.gates_eliminated(), 2);
    assert_eq!(fusion.patterns_matched(), 1);
}

#[test]
fn q2_unit_cz_cancellation_basic() {
    let fusion = GateFusionCapsule::new();
    let mut circuit = QuantumCircuit::new(2, "cz-cancel");
    circuit.add_gate(GateType::CZ { control: 0, target: 1 });
    circuit.add_gate(GateType::CZ { control: 0, target: 1 });

    let optimized = fusion.optimize(circuit).unwrap();
    assert_eq!(optimized.gates.len(), 0, "CZ · CZ should cancel completely");
    assert_eq!(fusion.gates_eliminated(), 2);
}

#[test]
fn q3_unit_hadamard_conjugation_basic() {
    let fusion = GateFusionCapsule::new();
    let mut circuit = QuantumCircuit::new(2, "h-cnot-h");
    circuit.add_gate(GateType::H { qubit: 0 });
    circuit.add_gate(GateType::CNOT { control: 0, target: 1 });
    circuit.add_gate(GateType::H { qubit: 0 });

    let optimized = fusion.optimize(circuit).unwrap();
    assert_eq!(optimized.gates.len(), 1, "H-CNOT-H should fuse to CZ");
    assert!(matches!(optimized.gates[0], GateType::CZ { control: 0, target: 1 }));
    assert_eq!(fusion.gates_eliminated(), 2);
}

#[test]
fn q4_unit_rotation_rx_composition() {
    let fusion = GateFusionCapsule::new();
    let mut circuit = QuantumCircuit::new(1, "rx-compose");
    circuit.add_gate(GateType::Rx { qubit: 0, theta: PI / 4.0 });
    circuit.add_gate(GateType::Rx { qubit: 0, theta: PI / 4.0 });

    let optimized = fusion.optimize(circuit).unwrap();
    assert_eq!(optimized.gates.len(), 1, "Rx · Rx should compose");

    if let GateType::Rx { theta, .. } = optimized.gates[0] {
        assert!((theta - PI / 2.0).abs() < 1e-10, "Angles should sum: π/4 + π/4 = π/2");
    } else {
        panic!("Expected Rx gate");
    }
}

#[test]
fn q5_unit_rotation_ry_composition() {
    let fusion = GateFusionCapsule::new();
    let mut circuit = QuantumCircuit::new(1, "ry-compose");
    circuit.add_gate(GateType::Ry { qubit: 0, theta: PI / 3.0 });
    circuit.add_gate(GateType::Ry { qubit: 0, theta: PI / 6.0 });

    let optimized = fusion.optimize(circuit).unwrap();
    assert_eq!(optimized.gates.len(), 1);

    if let GateType::Ry { theta, .. } = optimized.gates[0] {
        assert!((theta - PI / 2.0).abs() < 1e-10);
    } else {
        panic!("Expected Ry gate");
    }
}

#[test]
fn q6_unit_rotation_rz_composition() {
    let fusion = GateFusionCapsule::new();
    let mut circuit = QuantumCircuit::new(1, "rz-compose");
    circuit.add_gate(GateType::Rz { qubit: 0, theta: PI / 8.0 });
    circuit.add_gate(GateType::Rz { qubit: 0, theta: 3.0 * PI / 8.0 });

    let optimized = fusion.optimize(circuit).unwrap();
    assert_eq!(optimized.gates.len(), 1);

    if let GateType::Rz { theta, .. } = optimized.gates[0] {
        assert!((theta - PI / 2.0).abs() < 1e-10);
    } else {
        panic!("Expected Rz gate");
    }
}

#[test]
fn q7_unit_phase_accumulation_basic() {
    let fusion = GateFusionCapsule::new();
    let mut circuit = QuantumCircuit::new(1, "phase-accum");
    circuit.add_gate(GateType::Phase { qubit: 0, phi: PI / 6.0 });
    circuit.add_gate(GateType::Phase { qubit: 0, phi: PI / 6.0 });
    circuit.add_gate(GateType::Phase { qubit: 0, phi: PI / 6.0 });

    let optimized = fusion.optimize(circuit).unwrap();
    assert_eq!(optimized.gates.len(), 1, "3 Phase gates should accumulate to 1");
    assert_eq!(fusion.gates_eliminated(), 2);
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS
// ============================================================================

#[test]
fn q8_property_no_fusion_on_different_qubits() {
    let fusion = GateFusionCapsule::new();
    let mut circuit = QuantumCircuit::new(3, "different-qubits");
    circuit.add_gate(GateType::Rx { qubit: 0, theta: PI / 4.0 });
    circuit.add_gate(GateType::Rx { qubit: 1, theta: PI / 4.0 });  // Different qubit
    circuit.add_gate(GateType::Rx { qubit: 2, theta: PI / 4.0 });  // Different qubit

    let optimized = fusion.optimize(circuit).unwrap();
    assert_eq!(optimized.gates.len(), 3, "Gates on different qubits should not fuse");
    assert_eq!(fusion.gates_eliminated(), 0);
}

#[test]
fn q9_property_convergence_finite_passes() {
    let fusion = GateFusionCapsule::new();
    let circuit = QuantumCircuit::synthetic_fusible(4);

    // Should converge in finite passes (not infinite loop)
    let optimized = fusion.optimize(circuit).unwrap();
    assert!(optimized.gates.len() < 100, "Should converge to small circuit");
}

#[test]
fn q10_property_idempotence() {
    let fusion = GateFusionCapsule::new();
    let circuit = QuantumCircuit::synthetic_fusible(2);

    let first_pass = fusion.optimize(circuit.clone()).unwrap();
    let second_pass = fusion.optimize(first_pass.clone()).unwrap();

    assert_eq!(
        first_pass.gates.len(),
        second_pass.gates.len(),
        "Re-optimizing should not change gate count (idempotent)"
    );
}

#[test]
fn q11_property_preserves_qubit_count() {
    let fusion = GateFusionCapsule::new();
    let circuit = QuantumCircuit::synthetic_fusible(5);
    let num_qubits = circuit.num_qubits;

    let optimized = fusion.optimize(circuit).unwrap();
    assert_eq!(optimized.num_qubits, num_qubits, "Qubit count should be preserved");
}

#[test]
fn q12_property_always_reduces_or_maintains() {
    let fusion = GateFusionCapsule::new();
    let circuit = QuantumCircuit::synthetic_fusible(3);
    let input_gates = circuit.gates.len();

    let optimized = fusion.optimize(circuit).unwrap();
    let output_gates = optimized.gates.len();

    assert!(
        output_gates <= input_gates,
        "Optimization should never increase gate count"
    );
}

#[test]
fn q13_property_empty_circuit() {
    let fusion = GateFusionCapsule::new();
    let circuit = QuantumCircuit::new(2, "empty");

    let optimized = fusion.optimize(circuit).unwrap();
    assert_eq!(optimized.gates.len(), 0, "Empty circuit should remain empty");
    assert_eq!(fusion.gates_eliminated(), 0);
}

#[test]
fn q14_property_single_gate_unchanged() {
    let fusion = GateFusionCapsule::new();
    let mut circuit = QuantumCircuit::new(1, "single-gate");
    circuit.add_gate(GateType::H { qubit: 0 });

    let optimized = fusion.optimize(circuit).unwrap();
    assert_eq!(optimized.gates.len(), 1, "Single gate should remain");
    assert_eq!(fusion.gates_eliminated(), 0);
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS
// ============================================================================

#[test]
fn q15_integration_grover_3qubit() {
    let fusion = GateFusionCapsule::new();
    let circuit = QuantumCircuit::grover(3);
    let input_gates = circuit.gates.len();

    let optimized = fusion.optimize(circuit).unwrap();
    let output_gates = optimized.gates.len();

    println!("Grover 3q: {} gates → {} gates", input_gates, output_gates);
    assert!(output_gates < input_gates, "Grover should have fusion opportunities");
}

#[test]
fn q16_integration_grover_5qubit() {
    let fusion = GateFusionCapsule::new();
    let circuit = QuantumCircuit::grover(5);
    let input_gates = circuit.gates.len();

    let optimized = fusion.optimize(circuit).unwrap();
    let output_gates = optimized.gates.len();

    println!("Grover 5q: {} gates → {} gates", input_gates, output_gates);
    assert!(output_gates < input_gates);
}

#[test]
fn q17_integration_qft_4qubit() {
    let fusion = GateFusionCapsule::new();
    let circuit = QuantumCircuit::qft(4);
    let input_gates = circuit.gates.len();

    let optimized = fusion.optimize(circuit).unwrap();
    let output_gates = optimized.gates.len();

    println!("QFT 4q: {} gates → {} gates", input_gates, output_gates);
    // QFT has many rotation gates that can compose
    assert!(output_gates < input_gates, "QFT should benefit from rotation composition");
}

#[test]
fn q18_integration_qft_8qubit() {
    let fusion = GateFusionCapsule::new();
    let circuit = QuantumCircuit::qft(8);
    let input_gates = circuit.gates.len();

    let optimized = fusion.optimize(circuit).unwrap();
    let output_gates = optimized.gates.len();

    println!("QFT 8q: {} gates → {} gates", input_gates, output_gates);
    assert!(output_gates < input_gates);
}

#[test]
fn q19_integration_multi_pattern_fusion() {
    let fusion = GateFusionCapsule::new();
    let mut circuit = QuantumCircuit::new(3, "multi-pattern");

    // Pattern 1: H-CNOT-H
    circuit.add_gate(GateType::H { qubit: 0 });
    circuit.add_gate(GateType::CNOT { control: 0, target: 1 });
    circuit.add_gate(GateType::H { qubit: 0 });

    // Pattern 2: CNOT-CNOT
    circuit.add_gate(GateType::CNOT { control: 1, target: 2 });
    circuit.add_gate(GateType::CNOT { control: 1, target: 2 });

    // Pattern 3: Rotation composition
    circuit.add_gate(GateType::Rx { qubit: 0, theta: PI / 3.0 });
    circuit.add_gate(GateType::Rx { qubit: 0, theta: PI / 6.0 });

    // Pattern 4: Phase accumulation
    circuit.add_gate(GateType::Phase { qubit: 2, phi: PI / 4.0 });
    circuit.add_gate(GateType::Phase { qubit: 2, phi: PI / 4.0 });

    let optimized = fusion.optimize(circuit).unwrap();
    // Should have 2 gates: 1 CZ (from H-CNOT-H), 1 Rx (from composition)
    // CNOT-CNOT eliminates to 0, Phase gates combine to 1
    assert_eq!(optimized.gates.len(), 3, "Multi-pattern fusion should work");
    assert!(fusion.patterns_matched() >= 4, "Should match all 4 patterns");
}

#[test]
fn q20_integration_synthetic_fusible() {
    let fusion = GateFusionCapsule::new();
    let circuit = QuantumCircuit::synthetic_fusible(4);
    let input_gates = circuit.gates.len();

    let optimized = fusion.optimize(circuit).unwrap();
    let output_gates = optimized.gates.len();

    println!("Synthetic 4q: {} gates → {} gates", input_gates, output_gates);
    assert!(output_gates < input_gates / 2, "Synthetic circuit should fuse heavily");
    assert!(fusion.speedup_factor() >= 2.0);
}

#[test]
fn q21_integration_chain_optimization() {
    let fusion = GateFusionCapsule::new();
    let mut circuit = QuantumCircuit::new(1, "chain");

    // Long chain of rotations
    for _ in 0..10 {
        circuit.add_gate(GateType::Rx { qubit: 0, theta: PI / 10.0 });
    }

    let optimized = fusion.optimize(circuit).unwrap();
    assert_eq!(optimized.gates.len(), 1, "10 rotations should compose to 1");
    assert_eq!(fusion.gates_eliminated(), 9);
}

// ============================================================================
// Q22-Q28: PRODUCTION TESTS
// ============================================================================

#[test]
fn q22_production_grover_speedup_target() {
    let fusion = GateFusionCapsule::new();
    let circuit = QuantumCircuit::grover(8);
    let input_gates = circuit.gates.len();

    let optimized = fusion.optimize(circuit).unwrap();
    let output_gates = optimized.gates.len();

    let reduction_pct = 100.0 * (1.0 - output_gates as f64 / input_gates as f64);
    println!(
        "Grover 8q production: {} → {} gates ({:.1}% reduction)",
        input_gates, output_gates, reduction_pct
    );

    // Target: 50%+ reduction
    assert!(
        reduction_pct >= 50.0,
        "Should achieve 50%+ reduction (got {:.1}%)",
        reduction_pct
    );
}

#[test]
fn q23_production_qft_speedup_target() {
    let fusion = GateFusionCapsule::new();
    let circuit = QuantumCircuit::qft(10);
    let input_gates = circuit.gates.len();

    let optimized = fusion.optimize(circuit).unwrap();
    let output_gates = optimized.gates.len();

    let speedup = fusion.speedup_factor();
    println!("QFT 10q production: {:.2}× speedup", speedup);

    // Target: 2.5×+ speedup (60%+ reduction)
    assert!(speedup >= 2.5, "Should achieve 2.5×+ speedup (got {:.2}×)", speedup);
}

#[test]
fn q24_production_large_circuit() {
    let fusion = GateFusionCapsule::new();
    let circuit = QuantumCircuit::synthetic_fusible(10);
    let input_gates = circuit.gates.len();

    let optimized = fusion.optimize(circuit).unwrap();
    let output_gates = optimized.gates.len();

    println!("Large circuit: {} → {} gates", input_gates, output_gates);
    assert!(input_gates >= 100, "Should test large circuit (100+ gates)");
    assert!(fusion.speedup_factor() >= 3.0, "Large circuits should achieve 3×+ speedup");
}

#[test]
fn q25_production_metrics_accuracy() {
    let fusion = GateFusionCapsule::new();
    let circuit = QuantumCircuit::synthetic_fusible(3);
    let input_gates = circuit.gates.len();

    fusion.reset_metrics();
    let optimized = fusion.optimize(circuit).unwrap();
    let output_gates = optimized.gates.len();

    // Verify metrics consistency
    assert_eq!(fusion.optimizations_applied(), 1);
    assert_eq!(
        fusion.gates_eliminated(),
        (input_gates - output_gates) as u64
    );
    assert!(fusion.patterns_matched() > 0);

    let speedup = fusion.speedup_factor();
    let expected_speedup = input_gates as f64 / output_gates as f64;
    assert!((speedup - expected_speedup).abs() < 0.01, "Speedup metric should match");
}

#[test]
fn q26_production_concurrent_optimization() {
    use std::sync::Arc;
    use std::thread;

    let fusion: Arc<GateFusionCapsule> = Arc::new(GateFusionCapsule::new());
    let mut handles = vec![];

    // Concurrent optimizations (lockfree coordination)
    for i in 0..4 {
        let fusion_clone: Arc<GateFusionCapsule> = Arc::clone(&fusion);
        let handle = thread::spawn(move || {
            let circuit = QuantumCircuit::synthetic_fusible(2 + i);
            fusion_clone.optimize(circuit).unwrap()
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join().unwrap();
    }

    assert_eq!(fusion.optimizations_applied(), 4, "4 concurrent optimizations");
    assert!(fusion.gates_eliminated() > 0);
}

#[test]
fn q27_production_stress_many_circuits() {
    let fusion = GateFusionCapsule::new();
    fusion.reset_metrics();

    for i in 2..=8 {
        let circuit = QuantumCircuit::synthetic_fusible(i);
        fusion.optimize(circuit).unwrap();
    }

    assert_eq!(fusion.optimizations_applied(), 7, "7 circuits optimized");
    assert!(fusion.gates_eliminated() > 50, "Should eliminate many gates");
    assert!(fusion.patterns_matched() > 20, "Should match many patterns");
}

#[test]
fn q28_production_reset_metrics_works() {
    let fusion = GateFusionCapsule::new();
    let circuit = QuantumCircuit::synthetic_fusible(3);

    fusion.optimize(circuit.clone()).unwrap();
    assert!(fusion.optimizations_applied() > 0);
    assert!(fusion.gates_eliminated() > 0);

    fusion.reset_metrics();
    assert_eq!(fusion.optimizations_applied(), 0);
    assert_eq!(fusion.gates_eliminated(), 0);
    assert_eq!(fusion.patterns_matched(), 0);

    // Verify metrics accumulate after reset
    fusion.optimize(circuit).unwrap();
    assert_eq!(fusion.optimizations_applied(), 1);
    assert!(fusion.gates_eliminated() > 0);
}
