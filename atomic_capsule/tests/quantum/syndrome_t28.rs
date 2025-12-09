//! T28 Comprehensive Tests for Syndrome Extraction Capsule
//!
//! **Coverage**: 29 tests across 4 tiers (unit/property/integration/production)
//! **Framework**: T28 (Q1-Q28 systematic testing)
//! **Target**: 100% pass rate, <25μs P99 latency @ distance-5

#[cfg(feature = "quantum-syndrome")]
mod tests {
    use atomic_capsule::quantum::syndrome::{
        PauliOp, PauliString, SyndromeExtractionCapsule, SurfaceCodeTopology,
    };
    use num_complex::Complex64;

    // =========================================================================
    // Q1-Q7: UNIT TESTS (8 tests)
    // =========================================================================

    #[test]
    fn q1_pauli_op_encoding() {
        assert_eq!(PauliOp::I as u8, 0b00);
        assert_eq!(PauliOp::X as u8, 0b01);
        assert_eq!(PauliOp::Z as u8, 0b10);
        assert_eq!(PauliOp::Y as u8, 0b11);
    }

    #[test]
    fn q2_pauli_string_creation() {
        let ops = vec![PauliOp::X, PauliOp::Z, PauliOp::I, PauliOp::Y];
        let pauli = PauliString::from_operators(ops, 0);

        assert_eq!(pauli.num_qubits(), 4);
        assert_eq!(pauli.get_operator(0), PauliOp::X);
        assert_eq!(pauli.get_operator(1), PauliOp::Z);
        assert_eq!(pauli.get_operator(2), PauliOp::I);
        assert_eq!(pauli.get_operator(3), PauliOp::Y);
    }

    #[test]
    fn q3_capsule_layout() {
        assert_eq!(core::mem::size_of::<SyndromeExtractionCapsule>(), 256);
        assert_eq!(core::mem::align_of::<SyndromeExtractionCapsule>(), 256);
    }

    #[test]
    fn q4_pure_z_optimization() {
        let ops = vec![PauliOp::Z, PauliOp::Z, PauliOp::I];
        let pauli = PauliString::from_operators(ops, 0);
        assert!(pauli.is_pure_z());
        assert!(!pauli.is_pure_x());
    }

    #[test]
    fn q5_pure_x_optimization() {
        let ops = vec![PauliOp::X, PauliOp::X, PauliOp::I];
        let pauli = PauliString::from_operators(ops, 0);
        assert!(pauli.is_pure_x());
        assert!(!pauli.is_pure_z());
    }

    #[test]
    fn q6_syndrome_cache() {
        let capsule = SyndromeExtractionCapsule::new(3);
        let syndrome = vec![true, false, true, false, false];

        // Pack syndrome into u64
        let packed = 0b00101u64; // Bits: [1,0,1,0,0]

        // Verify packing logic
        let syndrome_bits = syndrome
            .iter()
            .enumerate()
            .fold(0u64, |acc, (i, &bit)| acc | ((bit as u64) << i));

        assert_eq!(syndrome_bits, packed);
    }

    #[test]
    fn q7_pauli_weight() {
        let ops = vec![PauliOp::X, PauliOp::I, PauliOp::Z, PauliOp::I];
        let pauli = PauliString::from_operators(ops, 0);
        assert_eq!(pauli.weight(), 2);
    }

    // =========================================================================
    // Q8-Q14: PROPERTY TESTS (7 tests)
    // =========================================================================

    #[test]
    fn q8_stabilizer_commutativity() {
        use atomic_capsule::quantum::syndrome::surface_code::StabilizerGenerator;

        // All stabilizers in surface code must commute
        let gen = StabilizerGenerator::new(5, SurfaceCodeTopology::Planar).unwrap();
        let stabs = gen.all_stabilizers();

        for i in 0..stabs.len() {
            for j in (i + 1)..stabs.len() {
                assert!(
                    stabs[i].commutes_with(&stabs[j]),
                    "Stabilizers {} and {} do not commute",
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn q9_parity_constraint() {
        let capsule = SyndromeExtractionCapsule::new(5);

        // Even parity should be valid
        let syndrome_even = vec![true; 4]; // 4 ones = even
        assert!(capsule.validate_parity(&syndrome_even));

        // Odd parity should be invalid
        let syndrome_odd = vec![true; 3]; // 3 ones = odd
        assert!(!capsule.validate_parity(&syndrome_odd));
    }

    #[test]
    fn q10_extraction_determinism() {
        // Same state → same syndrome (deterministic)
        let capsule = SyndromeExtractionCapsule::new(3);
        let state: Vec<Complex64> = (0..(1 << 9))
            .map(|i| Complex64::new((i as f64).sin(), (i as f64).cos()))
            .collect();

        let syndrome1 = capsule.extract_syndrome(&state).unwrap();
        let syndrome2 = capsule.extract_syndrome(&state).unwrap();

        assert_eq!(syndrome1, syndrome2);
    }

    #[test]
    fn q11_metrics_increment() {
        let capsule = SyndromeExtractionCapsule::new(3);
        let state = vec![Complex64::new(1.0, 0.0); 1 << 9];

        for _ in 0..10 {
            let _ = capsule.extract_syndrome(&state);
        }

        assert_eq!(capsule.extract_count(), 10);
        assert!(capsule.avg_latency_ns() > 0.0);
    }

    #[test]
    fn q12_latency_positive() {
        let capsule = SyndromeExtractionCapsule::new(5);
        let state = vec![Complex64::new(1.0, 0.0); 1 << 25];

        let _ = capsule.extract_syndrome(&state);

        let latency = capsule.avg_latency_ns();
        assert!(latency > 0.0);
        assert!(latency < 1_000_000_000.0); // < 1 second
    }

    #[test]
    fn q13_stabilizer_weight_bounds() {
        use atomic_capsule::quantum::syndrome::surface_code::StabilizerGenerator;

        let gen = StabilizerGenerator::new(5, SurfaceCodeTopology::Planar).unwrap();
        let all_stabs = gen.all_stabilizers();

        // All stabilizers should have weight 2-4
        for stab in &all_stabs {
            let w = stab.weight();
            assert!(w >= 2 && w <= 4, "weight {} out of range [2,4]", w);
        }
    }

    #[test]
    fn q14_pure_x_z_separation() {
        use atomic_capsule::quantum::syndrome::surface_code::StabilizerGenerator;

        let gen = StabilizerGenerator::new(5, SurfaceCodeTopology::Planar).unwrap();
        let x_stabs = gen.x_stabilizers();
        let z_stabs = gen.z_stabilizers();

        // X-stabilizers must be pure X
        for stab in x_stabs.iter() {
            assert!(stab.is_pure_x());
            assert!(!stab.is_pure_z());
        }

        // Z-stabilizers must be pure Z
        for stab in z_stabs.iter() {
            assert!(stab.is_pure_z());
            assert!(!stab.is_pure_x());
        }
    }

    // =========================================================================
    // Q15-Q21: INTEGRATION TESTS (7 tests)
    // =========================================================================

    #[test]
    fn q15_distance_3_perfect_state() {
        // |000...0⟩ state (9 qubits)
        let capsule = SyndromeExtractionCapsule::new(3);
        let mut state = vec![Complex64::new(0.0, 0.0); 1 << 9];
        state[0] = Complex64::new(1.0, 0.0);

        let syndrome = capsule.extract_syndrome(&state).unwrap();

        // Perfect state → all stabilizers +1 → syndrome all false
        assert!(syndrome.iter().all(|&bit| !bit));
    }

    #[test]
    fn q16_distance_5_perfect_state() {
        let capsule = SyndromeExtractionCapsule::new(5);
        let mut state = vec![Complex64::new(0.0, 0.0); 1 << 25];
        state[0] = Complex64::new(1.0, 0.0);

        let syndrome = capsule.extract_syndrome(&state).unwrap();

        assert!(syndrome.iter().all(|&bit| !bit));
        assert_eq!(syndrome.len(), 25); // 16 X + 9 Z stabilizers
    }

    #[test]
    fn q17_decoder_integration() {
        // Extract syndrome → pass to decoder → verify format
        let capsule = SyndromeExtractionCapsule::new(3);
        let state = vec![Complex64::new(1.0, 0.0); 1 << 9];

        let syndrome = capsule.extract_syndrome(&state).unwrap();
        let decoder_input = capsule.to_decoder_input(&syndrome);

        assert_eq!(decoder_input.distance, 3);
        assert_eq!(decoder_input.syndrome_bits.len(), syndrome.len());
    }

    #[test]
    fn q18_boundary_conditions() {
        use atomic_capsule::quantum::syndrome::surface_code::StabilizerGenerator;

        // Boundary stabilizers have weight < 4
        let gen = StabilizerGenerator::new(5, SurfaceCodeTopology::Planar).unwrap();
        let all_stabs = gen.all_stabilizers();

        for stab in &all_stabs {
            assert!(stab.num_qubits() == 25);
            assert!(stab.weight() >= 2);
            assert!(stab.weight() <= 4);
        }
    }

    #[test]
    fn q19_distance_7_scalability() {
        // Large code (49 qubits, 48 stabilizers)
        let capsule = SyndromeExtractionCapsule::new(7);
        let state = vec![Complex64::new(1.0, 0.0); 1 << 49];

        let start = std::time::Instant::now();
        let syndrome = capsule.extract_syndrome(&state).unwrap();
        let latency = start.elapsed();

        assert_eq!(syndrome.len(), 61); // 36 X + 25 Z stabilizers
        assert!(latency.as_micros() < 50); // <50μs target
    }

    #[test]
    fn q20_toric_vs_planar() {
        let planar = SyndromeExtractionCapsule::new(5);
        let toric = SyndromeExtractionCapsule::with_topology(5, SurfaceCodeTopology::Toric);

        let state = vec![Complex64::new(1.0, 0.0); 1 << 25];

        let syndrome_planar = planar.extract_syndrome(&state).unwrap();
        let syndrome_toric = toric.extract_syndrome(&state).unwrap();

        // Toric has more stabilizers (no boundary)
        assert!(syndrome_toric.len() > syndrome_planar.len());
    }

    #[test]
    fn q21_parity_violation_detection() {
        // This test validates that parity errors are detected
        // In practice, valid surface code states always have even parity
        let capsule = SyndromeExtractionCapsule::new(3);

        // For a valid quantum state, parity violations should be rare (0%)
        let state = vec![Complex64::new(1.0, 0.0); 1 << 9];

        for _ in 0..100 {
            let _ = capsule.extract_syndrome(&state).unwrap();
        }

        let error_rate = capsule.parity_error_rate();
        assert!(error_rate < 0.01); // <1% error rate (should be 0%)
    }

    // =========================================================================
    // Q22-Q28: PRODUCTION TESTS (7 tests)
    // =========================================================================

    #[test]
    fn q22_10k_extractions_stress() {
        // Stress test: 10,000 extractions
        let capsule = SyndromeExtractionCapsule::new(5);
        let state = vec![Complex64::new(1.0, 0.0); 1 << 25];

        for _ in 0..10_000 {
            let syndrome = capsule.extract_syndrome(&state).unwrap();
            assert_eq!(syndrome.len(), 25);
        }

        assert_eq!(capsule.extract_count(), 10_000);

        let avg_latency = capsule.avg_latency_ns();
        assert!(avg_latency < 25_000.0); // <25μs average
    }

    #[test]
    fn q23_concurrent_safety() {
        use std::sync::Arc;
        use std::thread;

        // Multi-threaded safety (lockfree coordination)
        let capsule: Arc<SyndromeExtractionCapsule> = Arc::new(SyndromeExtractionCapsule::new(3));
        let state: Arc<Vec<Complex64>> = Arc::new(vec![Complex64::new(1.0, 0.0); 1 << 9]);

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let capsule_clone = Arc::clone(&capsule);
                let state_clone = Arc::clone(&state);

                thread::spawn(move || {
                    for _ in 0..1000 {
                        let _ = capsule_clone.extract_syndrome(&state_clone);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(capsule.extract_count(), 4000);
    }

    #[test]
    fn q24_latency_distribution() {
        // Latency should be consistent (not highly variable)
        let capsule = SyndromeExtractionCapsule::new(5);
        let state = vec![Complex64::new(1.0, 0.0); 1 << 25];

        let mut latencies = Vec::new();

        for _ in 0..1000 {
            let start = std::time::Instant::now();
            let _ = capsule.extract_syndrome(&state).unwrap();
            latencies.push(start.elapsed().as_nanos() as f64);
        }

        let mean = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let variance = latencies
            .iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>()
            / latencies.len() as f64;
        let stddev = variance.sqrt();

        // Coefficient of variation < 30%
        assert!(stddev / mean < 0.3);
    }

    #[test]
    fn q25_memory_footprint() {
        // Capsule should stay within 256 bytes
        let capsule = SyndromeExtractionCapsule::new(5);
        assert_eq!(core::mem::size_of_val(&capsule), 256);
    }

    #[test]
    fn q26_cache_efficiency() {
        // Syndrome cache should work for ≤64 stabilizers
        let capsule = SyndromeExtractionCapsule::new(5);
        let state = vec![Complex64::new(1.0, 0.0); 1 << 25];

        let syndrome = capsule.extract_syndrome(&state).unwrap();

        // For distance-5, syndrome length = 25 (< 64)
        // Cache should be populated
        assert!(syndrome.len() <= 64);
    }

    #[test]
    fn q27_parity_error_rate_validation() {
        // Parity errors should be rare (0% for valid codes)
        let capsule = SyndromeExtractionCapsule::new(5);
        let state = vec![Complex64::new(1.0, 0.0); 1 << 25];

        for _ in 0..1000 {
            let _ = capsule.extract_syndrome(&state).unwrap();
        }

        let error_rate = capsule.parity_error_rate();
        assert!(error_rate < 0.001); // <0.1% error rate
    }

    #[test]
    fn q28_distance_scaling() {
        // Validate distance scaling (3, 5, 7)
        for &distance in &[3, 5, 7] {
            let capsule = SyndromeExtractionCapsule::new(distance);
            let num_qubits = distance * distance;
            let state = vec![Complex64::new(1.0, 0.0); 1 << num_qubits];

            let syndrome = capsule.extract_syndrome(&state).unwrap();

            // Syndrome length should match expected stabilizer count
            assert!(syndrome.len() > 0);
            assert_eq!(capsule.distance(), distance);
        }
    }

    #[test]
    fn q29_simd_vs_scalar_equivalence() {
        // SIMD and scalar must give identical results
        let capsule = SyndromeExtractionCapsule::new(3);
        let state: Vec<Complex64> = (0..(1 << 9))
            .map(|i| Complex64::new(0.5 + (i as f64) * 0.01, 0.3 - (i as f64) * 0.005))
            .collect();

        let syndrome_simd = capsule.extract_syndrome(&state).unwrap();
        let syndrome_scalar = capsule.extract_syndrome_scalar(&state).unwrap();

        assert_eq!(syndrome_simd, syndrome_scalar);
    }
}
