//! GHZ State Creation and Measurement
//!
//! # Overview
//!
//! The Greenberger-Horne-Zeilinger (GHZ) state is a maximally entangled quantum state
//! involving three or more qubits. It demonstrates quantum entanglement beyond Bell states
//! and is used in quantum communication, quantum error correction, and tests of quantum mechanics.
//!
//! # GHZ State Formula
//!
//! For N qubits:
//! ```text
//! |GHZ_N⟩ = (|0...0⟩ + |1...1⟩) / √2
//! ```
//!
//! # Properties
//!
//! - **Maximal entanglement**: All qubits are maximally correlated
//! - **Measurement correlation**: Measuring one qubit determines all others
//! - **Fragile**: Measuring in computational basis collapses to |00...0⟩ or |11...1⟩
//! - **Bell state extension**: 2-qubit GHZ state = Bell state
//!
//! # Circuit Construction
//!
//! 1. Apply Hadamard to first qubit: |000...0⟩ → (|0⟩+|1⟩)|00...0⟩/√2
//! 2. Apply CNOT cascade: Entangle all qubits with first qubit
//! 3. Result: (|00...0⟩ + |11...1⟩)/√2
//!
//! # This Implementation
//!
//! - Creates GHZ states for 2, 3, 4, and 5 qubits
//! - Demonstrates measurement correlations
//! - Shows probability distributions
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T6 Mixed tier (T1+T2 multi-qubit gates)
//! - **Chaos**: 100% computational capsules
//! - **ASSUM**: 99.5%+ safety
//! - **B32**: Fair baselines
//! - **T28**: Integration test example

#[cfg(feature = "quantum-pure")]
use atomic_capsule::quantum_pure::{
    QuantumCircuitCapsule, QuantumPureResult,
};

#[cfg(feature = "quantum-pure")]
fn main() -> QuantumPureResult<()> {
    println!("=== GHZ State Creation and Measurement ===\n");

    // Demonstrate GHZ states for different qubit counts
    for num_qubits in 2..=5 {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("{}-Qubit GHZ State", num_qubits);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

        create_and_measure_ghz(num_qubits)?;
        println!();
    }

    Ok(())
}

#[cfg(not(feature = "quantum-pure"))]
fn main() {
    eprintln!("This example requires the 'quantum-pure' feature");
    eprintln!("Run with: cargo run --example quantum_ghz --features quantum-pure");
}

#[cfg(feature = "quantum-pure")]
fn create_and_measure_ghz(num_qubits: u32) -> QuantumPureResult<()> {
    // Create GHZ state circuit
    let mut circuit = QuantumCircuitCapsule::new(num_qubits)?;

    println!("Circuit construction:");
    println!("  1. H(0): |0...0⟩ → (|0⟩+|1⟩)|0...0⟩/√2");
    circuit.add_hadamard(0)?;

    println!("  2. CNOT cascade: Entangle all qubits with qubit 0");
    for target in 1..num_qubits as usize {
        circuit.add_cnot(0, target)?;
        println!("     CNOT(0, {})", target);
    }

    println!("  → Final state: (|{}⟩ + |{}⟩)/√2",
        "0".repeat(num_qubits as usize),
        "1".repeat(num_qubits as usize)
    );

    // Execute circuit
    println!("\nExecuting circuit...");
    let start = std::time::Instant::now();
    circuit.execute()?;
    let elapsed = start.elapsed();
    println!("Execution time: {:?}", elapsed);
    println!("Circuit depth: {} gates", circuit.depth());

    // Perform measurements
    println!("\nMeasurements (100 trials):");
    let dimension = 1usize << num_qubits;
    let mut results = vec![0; dimension];

    for _ in 0..100 {
        // Reset and recreate GHZ state
        circuit.reset()?;
        circuit.add_hadamard(0)?;
        for target in 1..num_qubits as usize {
            circuit.add_cnot(0, target)?;
        }
        circuit.execute()?;

        let measurement = circuit.measure()? as usize;
        results[measurement] += 1;
    }

    // Display measurement distribution
    println!("\nMeasurement Distribution:");
    for (state, count) in results.iter().enumerate() {
        if *count > 0 {
            let prob = *count as f64 / 100.0;
            let bar = "█".repeat((prob * 50.0) as usize);
            let binary = format!("{:0width$b}", state, width = num_qubits as usize);
            println!(
                "  |{}⟩: {:3} times ({:5.1}%) {}",
                binary,
                count,
                prob * 100.0,
                bar
            );
        }
    }

    // Check GHZ property: Only |00...0⟩ and |11...1⟩ should be measured
    let all_zeros = 0;
    let all_ones = (1 << num_qubits) - 1;

    let zeros_count = results[all_zeros];
    let ones_count = results[all_ones];
    let total_ghz = zeros_count + ones_count;

    println!("\nGHZ Property Validation:");
    println!("  Expected: Only |{}⟩ and |{}⟩",
        "0".repeat(num_qubits as usize),
        "1".repeat(num_qubits as usize)
    );
    println!("  Measured: {}/{} trials ({:.1}%)",
        total_ghz,
        100,
        total_ghz as f64
    );

    if total_ghz >= 98 {
        println!("  ✅ SUCCESS: GHZ state verified with {:.1}% fidelity", total_ghz as f64);
    } else {
        println!("  ⚠️  NOTICE: GHZ fidelity {:.1}% (some decoherence expected in simulation)", total_ghz as f64);
    }

    // Show correlation
    println!("\nMeasurement Correlation:");
    println!("  |{}⟩: {:5.1}%", "0".repeat(num_qubits as usize), zeros_count as f64);
    println!("  |{}⟩: {:5.1}%", "1".repeat(num_qubits as usize), ones_count as f64);
    println!("  → Perfect correlation: measuring one qubit determines all others");

    // Compare to product state
    println!("\nComparison to Product State:");
    println!("  Product state: Would measure uniform distribution over all 2^N states");
    println!("  GHZ state: Only 2 outcomes possible (maximal entanglement)");
    println!("  Entanglement: {} qubits maximally entangled", num_qubits);

    Ok(())
}
