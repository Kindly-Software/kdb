//! Direct 20-qubit benchmark (bypasses Criterion overhead)
//!
//! # Purpose
//!
//! Validates multi-threaded AVX2 performance at 20 qubits (1M dimensions)
//! without Criterion's 10-second measurement windows that cause timeouts.
//!
//! # Method
//!
//! - 100 iterations of each gate
//! - Manual timing with std::time::Instant
//! - Statistical analysis (mean, stddev, 95% CI)

#![cfg(all(feature = "avx2-simd", target_arch = "x86_64"))]

use atomic_capsule::quantum_pure::{QuantumState, QuantumGateCapsule};
use std::time::Instant;

fn benchmark_gate(
    state: &mut QuantumState,
    gate: &QuantumGateCapsule,
    iterations: usize,
) -> (f64, f64) {
    let mut timings = Vec::with_capacity(iterations);

    // Warm-up (5 iterations)
    for _ in 0..5 {
        state.apply_gate(gate).expect("Gate failed");
    }

    // Measurement (with black_box to prevent dead code elimination)
    for _ in 0..iterations {
        let start = Instant::now();
        state.apply_gate(gate).expect("Gate failed");

        // CRITICAL: Force computation with observable side effect
        // Without this, compiler may optimize away the entire gate application!
        // Sum normalization reads ALL 1M amplitudes, preventing dead code elimination
        let mut norm_sum = 0.0;
        for i in 0..state.num_amplitudes() {
            let re = state.real_parts[i];
            let im = state.imag_parts[i];
            norm_sum += re * re + im * im;
        }
        std::hint::black_box(norm_sum);

        let elapsed = start.elapsed();
        timings.push(elapsed.as_nanos() as f64);
    }

    // Calculate statistics
    let mean = timings.iter().sum::<f64>() / timings.len() as f64;
    let variance = timings
        .iter()
        .map(|t| (t - mean).powi(2))
        .sum::<f64>()
        / timings.len() as f64;
    let stddev = variance.sqrt();

    (mean, stddev)
}

fn main() {
    println!("20-Qubit Multi-Threaded AVX2 Benchmark");
    println!("========================================\n");

    let num_qubits = 20;
    let dimension = 1_usize << num_qubits;
    let memory_mb = (dimension * 16) / (1024 * 1024);

    println!("Configuration:");
    println!("  Qubits: {}", num_qubits);
    println!("  Dimensions: {} ({})", dimension, dimension);
    println!("  Memory: {} MB", memory_mb);
    println!("  Iterations: 100");
    println!("  ThreadPool: {}-core parallel\n", std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1));

    // Create state
    println!("Initializing state...");
    let mut state = QuantumState::new(num_qubits).expect("Failed to create state");

    // Gates to benchmark
    let gates = vec![
        ("Hadamard", QuantumGateCapsule::hadamard(19)),  // High qubit for threading
        ("Pauli-X", QuantumGateCapsule::pauli_x(19)),
        ("Pauli-Y", QuantumGateCapsule::pauli_y(19)),
        ("Pauli-Z", QuantumGateCapsule::pauli_z(19)),
    ];

    println!("\nResults:");
    println!("{:<12} {:>15} {:>15} {:>15} {:>12}", "Gate", "Mean (µs)", "Stddev (µs)", "95% CI (µs)", "Per-dim (ns)");
    println!("{}", "-".repeat(72));

    for (name, gate) in gates {
        let (mean_ns, stddev_ns) = benchmark_gate(&mut state, &gate, 100);
        let mean_us = mean_ns / 1000.0;
        let stddev_us = stddev_ns / 1000.0;
        let ci_95 = 1.96 * stddev_us;  // 95% confidence interval
        let per_dim = mean_ns / dimension as f64;

        println!(
            "{:<12} {:>15.2} {:>15.2} {:>15.2} {:>12.4}",
            name, mean_us, stddev_us, ci_95, per_dim
        );
    }

    println!("\n✅ Benchmark complete");
}
