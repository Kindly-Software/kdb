//! # T28 Comprehensive Test Suite: LicenseEntangledCapsule
//!
//! **[TRADE SECRET] - Revolutionary DRM where license IS computation**
//!
//! ## T28 Framework Tiers
//!
//! - **Q1-Q7 (Unit)**: Individual component testing
//! - **Q8-Q14 (Property)**: Invariant and cryptographic property testing
//! - **Q15-Q21 (Integration)**: Multi-capsule interaction testing
//! - **Q22-Q28 (Production)**: Stress testing and real-world scenarios
//! - **Q29-Q35 (Determinism)**: Reproducibility and determinism validation
//!
//! ## Test Categories
//!
//! 1. **Entanglement Core**: Wrong license = garbage output
//! 2. **Feature Gating**: Signature-based operation dispatch
//! 3. **Audit Trail**: Q34 hash-chain with license anchoring
//! 4. **Generation Counter**: Rotation and replay detection
//! 5. **Concurrent Access**: Thread safety under contention
//! 6. **Determinism**: Same inputs = same outputs
//!
//! ## ASSUM Framework
//! - `#ASSUME_ED25519_SECURE`: Ed25519 provides 2^128 security
//! - `#VERIFY_ED25519`: RFC 8032 test vectors
//! - `#ASSUME_ENTANGLEMENT_IRREVERSIBLE`: XOR cannot be NOPed
//! - `#VERIFY_ENTANGLEMENT`: Wrong license = garbage

#![cfg(feature = "license-entanglement")]

use atomic_capsule::license::{
    LicenseEntangledCapsule, License, LicenseError, LicenseFeatures, ComputationResult,
    LicenseAuditCapsule, LicenseAuditEntry, AuditAnchor,
    EntangledGeneration, RotationSchedule,
};

// ============================================================================
// Q1-Q7: UNIT TESTS
// ============================================================================

mod unit_tests {
    use super::*;

    /// Q1: License structure creation
    #[test]
    fn test_license_creation() {
        let license = License::new(
            [0xAA; 32],     // public_key
            [0xBB; 64],     // signature
            u64::MAX,       // expiry (never)
            0xFF,           // features (basic set)
            [0xCC; 16],     // customer_id
        );

        assert_eq!(license.public_key, [0xAA; 32]);
        assert_eq!(license.signature, [0xBB; 64]);
        assert_eq!(license.expiry_timestamp, u64::MAX);
        assert_eq!(license.features, 0xFF);
        assert_eq!(license.customer_id, [0xCC; 16]);
    }

    /// Q2: License message bytes format
    #[test]
    fn test_license_message_bytes() {
        let license = License::new(
            [0; 32],
            [0; 64],
            0x123456789ABCDEF0,  // expiry
            0xFEDCBA9876543210,  // features
            [0x42; 16],          // customer_id
        );

        let bytes = license.message_bytes();

        // Structure: customer_id (16B) || expiry (8B LE) || features (8B LE)
        assert_eq!(&bytes[0..16], &[0x42; 16]);
        assert_eq!(&bytes[16..24], &0x123456789ABCDEF0u64.to_le_bytes());
        assert_eq!(&bytes[24..32], &0xFEDCBA9876543210u64.to_le_bytes());
    }

    /// Q3: LicenseFeatures bitfield operations
    #[test]
    fn test_license_features() {
        let features = LicenseFeatures::from_bits(0b10101010);

        assert!(!features.has_feature(0));
        assert!(features.has_feature(1));
        assert!(!features.has_feature(2));
        assert!(features.has_feature(3));

        // Standard feature bits
        let pro_features = LicenseFeatures::from_bits(
            (1 << LicenseFeatures::FEATURE_PROFESSIONAL)
                | (1 << LicenseFeatures::FEATURE_EXPORT)
        );
        assert!(pro_features.has_feature(LicenseFeatures::FEATURE_PROFESSIONAL));
        assert!(pro_features.has_feature(LicenseFeatures::FEATURE_EXPORT));
        assert!(!pro_features.has_feature(LicenseFeatures::FEATURE_ENTERPRISE));
    }

    /// Q4: ComputationResult validation
    #[test]
    fn test_computation_result() {
        let valid = ComputationResult::new(12345, 1, true);
        assert!(valid.is_valid());

        let unauthorized = ComputationResult::new(12345, 1, false);
        assert!(!unauthorized.is_valid());

        let zero_gen = ComputationResult::new(12345, 0, true);
        assert!(!zero_gen.is_valid());
    }

    /// Q5: LicenseError display
    #[test]
    fn test_license_error_display() {
        assert!(format!("{}", LicenseError::SignatureInvalid).contains("signature"));
        assert!(format!("{}", LicenseError::Expired).contains("expired"));
        assert!(format!("{}", LicenseError::TransformMismatch).contains("tamper"));
    }

    /// Q6: Uninit capsule state
    #[test]
    fn test_uninit_capsule() {
        let capsule = LicenseEntangledCapsule::uninit();

        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.state(), 0);
        assert!(matches!(
            capsule.verify_integrity(),
            Err(LicenseError::NotInitialized)
        ));
    }

    /// Q7: AuditAnchor serialization
    #[test]
    fn test_audit_anchor_serialization() {
        let anchor = AuditAnchor::new(0xDEADBEEF, 42, 1000);
        let bytes = anchor.to_bytes();

        assert_eq!(bytes.len(), 24);
        assert_eq!(u64::from_le_bytes(bytes[0..8].try_into().unwrap()), 0xDEADBEEF);
        assert_eq!(u64::from_le_bytes(bytes[8..16].try_into().unwrap()), 42);
        assert_eq!(u64::from_le_bytes(bytes[16..24].try_into().unwrap()), 1000);
    }
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS
// ============================================================================

mod property_tests {
    use super::*;

    /// Q8: Wrong transform produces different output
    #[test]
    fn test_wrong_transform_garbage() {
        let correct_transform = 0xCAFEBABE12345678u64;
        let wrong_transform = 0x1111111111111111u64;

        let capsule_correct = LicenseEntangledCapsule::for_testing_with_state(
            0x1000,
            correct_transform,
            0xFFFF,
            [0; 4],
        );

        let capsule_wrong = LicenseEntangledCapsule::for_testing_with_state(
            0x1000,
            wrong_transform,
            0xFFFF,
            [0; 4],
        );

        let result_correct = capsule_correct.transition(12345);
        let result_wrong = capsule_wrong.transition(12345);

        // Core property: wrong license = different (garbage) output
        assert_ne!(result_correct, result_wrong);
    }

    /// Q9: Transition modifies state atomically
    #[test]
    fn test_transition_atomic() {
        let capsule = LicenseEntangledCapsule::for_testing_with_state(
            0xAAAA,
            0xDEAD,
            0xFFFF,
            [0; 4],
        );

        let initial = capsule.state();
        let initial_gen = capsule.generation();

        let result = capsule.transition(100);

        // State changed
        assert_ne!(capsule.state(), initial);
        assert_eq!(capsule.state(), result);

        // Generation incremented
        assert_eq!(capsule.generation(), initial_gen + 1);
    }

    /// Q10: Feature gating respects feature bits
    #[test]
    fn test_feature_gating_property() {
        let capsule = LicenseEntangledCapsule::for_testing_with_state(
            0x1000,
            0xBEEF,
            0b101, // Features 0 and 2 enabled
            [0; 4],
        );

        // Feature 0 (enabled)
        assert!(capsule.feature_op(100, 0).is_some());

        // Feature 1 (disabled)
        assert!(capsule.feature_op(100, 1).is_none());

        // Feature 2 (enabled)
        assert!(capsule.feature_op(100, 2).is_some());

        // Feature 63 (disabled - high bit)
        assert!(capsule.feature_op(100, 63).is_none());
    }

    /// Q11: Dispatch path varies with signature bits
    #[test]
    fn test_dispatch_path_variation() {
        let capsule_bit0 = LicenseEntangledCapsule::for_testing_with_state(
            0x1000,
            0xABCD,
            0xFFFF,
            [0b0, 0, 0, 0], // Bit 0 = 0
        );

        let capsule_bit1 = LicenseEntangledCapsule::for_testing_with_state(
            0x1000,
            0xABCD, // Same transform
            0xFFFF,
            [0b1, 0, 0, 0], // Bit 0 = 1
        );

        let result_0 = capsule_bit0.dispatch_op(100, 0, 0);
        let result_1 = capsule_bit1.dispatch_op(100, 0, 0);

        // Different paths = different results
        assert_ne!(result_0, result_1);
    }

    /// Q12: Mask stream is deterministic
    #[test]
    fn test_mask_stream_deterministic() {
        let capsule = LicenseEntangledCapsule::for_testing_with_state(
            0,
            0xFEEDFACE,
            0,
            [0x1111, 0x2222, 0x3333, 0x4444],
        );

        // Same seed + index = same output
        let mask1 = capsule.mask_stream(42, 10);
        let mask2 = capsule.mask_stream(42, 10);
        assert_eq!(mask1, mask2);

        // Different index = different output
        let mask3 = capsule.mask_stream(42, 11);
        assert_ne!(mask1, mask3);

        // Different seed = different output
        let mask4 = capsule.mask_stream(43, 10);
        assert_ne!(mask1, mask4);
    }

    /// Q13: Generation counter monotonicity
    #[test]
    fn test_generation_monotonic() {
        let gen = EntangledGeneration::new(0x1234);

        let mut prev = gen.raw();
        for _ in 0..100 {
            gen.increment();
            let curr = gen.raw();
            assert!(curr > prev, "Generation must be monotonic");
            prev = curr;
        }
    }

    /// Q14: Audit entry hash chain property
    #[test]
    fn test_audit_hash_chain_property() {
        let anchor_val = 0xABCD1234;
        let capsule = LicenseAuditCapsule::new(anchor_val);

        // Append multiple entries
        let hash1 = capsule.append(1, 0, 100, 200, 1000);
        let hash2 = capsule.append(2, 1, 300, 400, 2000);
        let hash3 = capsule.append(3, 2, 500, 600, 3000);

        // Each hash is unique
        assert_ne!(hash1, hash2);
        assert_ne!(hash2, hash3);
        assert_ne!(hash1, hash3);

        // Chain head is latest
        assert_eq!(capsule.chain_head(), hash3);

        // Entry count correct
        assert_eq!(capsule.entry_count(), 3);
    }
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS
// ============================================================================

mod integration_tests {
    use super::*;

    /// Q15: Capsule + Audit integration
    #[test]
    fn test_capsule_audit_integration() {
        let transform = 0xDEADBEEF;
        let capsule = LicenseEntangledCapsule::for_testing(transform, 0xFFFF, [0; 4]);

        let audit = LicenseAuditCapsule::new(transform);

        // Perform operation and audit it
        let input = 12345u64;
        let output = capsule.transition(input);

        let hash = audit.append(
            LicenseAuditEntry::OP_TRANSITION,
            255,  // No feature
            input,
            output,
            1000,  // timestamp
        );

        assert_ne!(hash, 0);
        assert_eq!(audit.license_anchor(), transform);
    }

    /// Q16: Capsule + Generation integration
    #[test]
    fn test_capsule_generation_integration() {
        let transform = 0xCAFEBABE;

        // Note: LicenseEntangledCapsule.generation and EntangledGeneration serve different purposes:
        // - Capsule generation: Simple operation counter (starts at 1)
        // - EntangledGeneration: License-bound counter with rotation (starts at transform)
        let capsule = LicenseEntangledCapsule::for_testing(transform, 0xFFFF, [0; 4]);

        let gen = EntangledGeneration::new(transform);

        // Capsule starts at generation 1, EntangledGeneration starts at transform
        assert_eq!(capsule.generation(), 1);
        assert_eq!(gen.raw(), transform);

        // After operations
        capsule.transition(100);
        gen.increment();

        // Capsule is at generation 2 (1 + 1 transition)
        assert_eq!(capsule.generation(), 2);
        // EntangledGeneration is at transform + 1 (XOR increment pattern)
        assert_eq!(gen.raw(), transform + 1);

        // Both provide operation tracking, but EntangledGeneration
        // incorporates license transform for rotation/replay detection
    }

    /// Q17: Audit chain verification
    #[test]
    fn test_audit_chain_verification() {
        let anchor = 0x12345678;
        let capsule = LicenseAuditCapsule::new(anchor);

        // Build valid chain
        let anchor1 = AuditAnchor::new(anchor, 1, 100);
        let entry1 = LicenseAuditEntry::new(anchor, 1, 0, 10, 20, anchor1);
        let hash1 = entry1.compute_hash();

        let anchor2 = AuditAnchor::new(anchor, 2, 200);
        let entry2 = LicenseAuditEntry::new(hash1, 2, 1, 30, 40, anchor2);

        // Verify valid chain
        assert!(capsule.verify_chain(&[entry1, entry2]).is_ok());
    }

    /// Q18: Audit detects broken chain
    #[test]
    fn test_audit_broken_chain_detection() {
        let anchor = 0x12345678;
        let capsule = LicenseAuditCapsule::new(anchor);

        // Build broken chain
        let anchor1 = AuditAnchor::new(anchor, 1, 100);
        let entry1 = LicenseAuditEntry::new(anchor, 1, 0, 10, 20, anchor1);
        let hash1 = entry1.compute_hash();

        let anchor2 = AuditAnchor::new(anchor, 2, 200);
        let entry2 = LicenseAuditEntry::new(
            hash1.wrapping_add(1),  // WRONG prev_hash
            2,
            1,
            30,
            40,
            anchor2,
        );

        // Detect broken chain
        let result = capsule.verify_chain(&[entry1, entry2]);
        assert_eq!(result, Err(1));
    }

    /// Q19: Generation rotation
    #[test]
    fn test_generation_rotation() {
        let mut gen = EntangledGeneration::new(0xFEED);
        let schedule = RotationSchedule::new(100, 10, 0xFEED);

        let factor_before = gen.rotation_factor();

        // Not time for rotation
        assert!(gen.maybe_rotate(&schedule, 50).is_none());
        assert_eq!(gen.rotation_factor(), factor_before);

        // Time for rotation
        let new_epoch = gen.maybe_rotate(&schedule, 100).unwrap();
        assert_eq!(new_epoch, 1);
        assert_ne!(gen.rotation_factor(), factor_before);
    }

    /// Q20: Multiple feature operations
    #[test]
    fn test_multiple_feature_ops() {
        let capsule = LicenseEntangledCapsule::for_testing_with_state(
            0x1000,
            0xABCD,
            0b111111, // First 6 features
            [0; 4],
        );

        // Use different features
        let r0 = capsule.feature_op(100, 0).unwrap();
        let r1 = capsule.feature_op(100, 1).unwrap();
        let r2 = capsule.feature_op(100, 2).unwrap();

        // Each feature produces different result (due to feature_salt)
        assert_ne!(r0, r1);
        assert_ne!(r1, r2);
    }

    /// Q21: Full workflow integration
    #[test]
    fn test_full_workflow() {
        let transform = 0xCAFEBABE;

        // Create all components with same license
        let capsule = LicenseEntangledCapsule::for_testing_with_state(
            transform,
            transform,
            0xFFFFFFFF,
            [0x1111, 0x2222, 0x3333, 0x4444],
        );

        let audit = LicenseAuditCapsule::new(transform);
        let mut gen = EntangledGeneration::new(transform);

        // Workflow: compute -> audit -> rotate check
        for i in 0..5 {
            let input = i * 100;
            let output = capsule.transition(input);

            // Audit the operation
            audit.append(
                LicenseAuditEntry::OP_TRANSITION,
                255,
                input,
                output,
                i as u64 * 1000,
            );

            // Check rotation
            let schedule = RotationSchedule::new(2000, 100, transform);
            gen.maybe_rotate(&schedule, i as u64 * 1000);
        }

        // Verify state
        assert_eq!(capsule.generation(), 6);  // 1 initial + 5 transitions
        assert_eq!(audit.entry_count(), 5);
    }
}

// ============================================================================
// Q22-Q28: PRODUCTION TESTS
// ============================================================================

#[cfg(feature = "std")]
mod production_tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    /// Q22: Concurrent transitions
    #[test]
    fn test_concurrent_transitions() {
        let capsule = Arc::new(LicenseEntangledCapsule::for_testing_with_state(
            0x1000,
            0xDEADBEEF,
            0xFFFF,
            [0; 4],
        ));

        let threads = 4;
        let ops_per_thread = 1000;

        let handles: Vec<_> = (0..threads)
            .map(|i| {
                let capsule = Arc::clone(&capsule);
                thread::spawn(move || {
                    for j in 0..ops_per_thread {
                        capsule.transition(i * ops_per_thread + j);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // All operations completed
        assert_eq!(capsule.generation(), 1 + (threads * ops_per_thread) as u64);
    }

    /// Q23: Concurrent audit appends
    #[test]
    fn test_concurrent_audit_appends() {
        let audit = Arc::new(LicenseAuditCapsule::new(0xABCD));

        let threads = 4;
        let appends_per_thread = 100;

        let handles: Vec<_> = (0..threads)
            .map(|i| {
                let audit = Arc::clone(&audit);
                thread::spawn(move || {
                    for j in 0..appends_per_thread {
                        audit.append(
                            i as u8,
                            (j % 64) as u8,
                            i * 100 + j,
                            j * 2,
                            j as u64,
                        );
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(audit.entry_count(), (threads * appends_per_thread) as u64);
    }

    /// Q24: Concurrent generation increments
    #[test]
    fn test_concurrent_generation_increments() {
        let gen = Arc::new(EntangledGeneration::new(0));

        let threads = 4;
        let increments_per_thread = 1000;

        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let gen = Arc::clone(&gen);
                thread::spawn(move || {
                    for _ in 0..increments_per_thread {
                        gen.increment();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(gen.raw(), (threads * increments_per_thread) as u64);
    }

    /// Q25: High-contention feature operations
    #[test]
    fn test_high_contention_feature_ops() {
        let capsule = Arc::new(LicenseEntangledCapsule::for_testing_with_state(
            0x1000,
            0xBEEF,
            u64::MAX, // All features enabled
            [0xFFFF; 4],
        ));

        let threads = 8;
        let ops_per_thread = 500;

        let handles: Vec<_> = (0..threads)
            .map(|i| {
                let capsule = Arc::clone(&capsule);
                thread::spawn(move || {
                    for j in 0..ops_per_thread {
                        // Use different features
                        let feature = (i * ops_per_thread + j) % 64;
                        capsule.feature_op(j, feature as u64);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // Generation should account for all feature ops
        let expected_gen = 1 + (threads * ops_per_thread) as u64;
        assert_eq!(capsule.generation(), expected_gen);
    }

    /// Q26: Stress test mask stream
    #[test]
    fn test_stress_mask_stream() {
        let capsule = Arc::new(LicenseEntangledCapsule::for_testing_with_state(
            0,
            0xFEEDFACE,
            0,
            [0x1111, 0x2222, 0x3333, 0x4444],
        ));

        let threads = 4;
        let masks_per_thread = 10000;

        let handles: Vec<_> = (0..threads)
            .map(|i| {
                let capsule = Arc::clone(&capsule);
                thread::spawn(move || {
                    let mut masks = Vec::with_capacity(masks_per_thread);
                    for j in 0..masks_per_thread {
                        masks.push(capsule.mask_stream(i as u64, j as u64));
                    }
                    masks
                })
            })
            .collect();

        let all_masks: Vec<Vec<u64>> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // Verify masks are deterministic (same thread produces same sequence)
        // and different seeds produce different sequences
        for (i, masks) in all_masks.iter().enumerate() {
            for (j, other_masks) in all_masks.iter().enumerate() {
                if i != j {
                    // Different threads should have different first mask
                    assert_ne!(masks[0], other_masks[0]);
                }
            }
        }
    }

    /// Q27: Memory layout verification
    #[test]
    fn test_memory_layouts() {
        use core::mem::{size_of, align_of};

        // LicenseEntangledCapsule: 128B, 128-align
        assert_eq!(size_of::<LicenseEntangledCapsule>(), 128);
        assert_eq!(align_of::<LicenseEntangledCapsule>(), 128);

        // LicenseAuditCapsule: 256B, 256-align
        assert_eq!(size_of::<LicenseAuditCapsule>(), 256);
        assert_eq!(align_of::<LicenseAuditCapsule>(), 256);

        // EntangledGeneration: 64B, 64-align
        assert_eq!(size_of::<EntangledGeneration>(), 64);
        assert_eq!(align_of::<EntangledGeneration>(), 64);
    }

    /// Q28: Verify integrity under load
    #[test]
    fn test_verify_integrity_under_load() {
        let capsule = Arc::new(LicenseEntangledCapsule::for_testing_with_state(
            0x1234,
            0xABCD,
            0xFFFF,
            [0; 4],
        ));

        let threads = 4;

        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let capsule = Arc::clone(&capsule);
                thread::spawn(move || {
                    for i in 0..1000 {
                        capsule.transition(i);
                        // Verify integrity periodically
                        if i % 100 == 0 {
                            assert!(capsule.verify_integrity().is_ok());
                        }
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // Final integrity check
        assert!(capsule.verify_integrity().is_ok());
    }
}

// ============================================================================
// Q29-Q35: DETERMINISM TESTS
// ============================================================================

mod determinism_tests {
    use super::*;

    /// Q29: Transition determinism - same inputs = same outputs
    #[test]
    fn test_transition_determinism() {
        fn run_sequence(transform: u64, inputs: &[u64]) -> Vec<u64> {
            let capsule = LicenseEntangledCapsule::for_testing_with_state(
                transform,
                transform,
                0xFFFF,
                [0; 4],
            );

            inputs.iter().map(|&i| capsule.transition(i)).collect()
        }

        let transform = 0xCAFEBABE;
        let inputs = [100, 200, 300, 400, 500];

        let results1 = run_sequence(transform, &inputs);
        let results2 = run_sequence(transform, &inputs);

        assert_eq!(results1, results2);
    }

    /// Q30: Feature operation determinism
    #[test]
    fn test_feature_op_determinism() {
        fn run_feature_ops(transform: u64, features: u64, ops: &[(u64, u64)]) -> Vec<Option<u64>> {
            let capsule = LicenseEntangledCapsule::for_testing_with_state(
                transform,
                transform,
                features,
                [0; 4],
            );

            ops.iter().map(|&(input, feature)| capsule.feature_op(input, feature)).collect()
        }

        let transform = 0xDEAD;
        let features = 0b1111;  // First 4 features
        let ops = [(100, 0), (200, 1), (300, 2), (400, 3), (500, 4)];

        let results1 = run_feature_ops(transform, features, &ops);
        let results2 = run_feature_ops(transform, features, &ops);

        assert_eq!(results1, results2);

        // Feature 4 should be None (not enabled)
        assert!(results1[4].is_none());
    }

    /// Q31: Dispatch path determinism
    #[test]
    fn test_dispatch_determinism() {
        fn run_dispatch(sig_bits: [u64; 4], inputs: &[(u64, usize, u64)]) -> Vec<u64> {
            let capsule = LicenseEntangledCapsule::for_testing_with_state(
                0x1000,
                0xBEEF,
                0xFFFF,
                sig_bits,
            );

            inputs
                .iter()
                .map(|&(input, idx, bit)| capsule.dispatch_op(input, idx, bit))
                .collect()
        }

        let sig_bits = [0x5555, 0xAAAA, 0x5555, 0xAAAA];
        let inputs = [(100, 0, 0), (100, 0, 1), (100, 1, 0), (100, 1, 1)];

        let results1 = run_dispatch(sig_bits, &inputs);
        let results2 = run_dispatch(sig_bits, &inputs);

        assert_eq!(results1, results2);
    }

    /// Q32: Mask stream determinism
    #[test]
    fn test_mask_stream_full_determinism() {
        fn generate_masks(transform: u64, sig_bits: [u64; 4], seeds: &[(u64, u64)]) -> Vec<u64> {
            let capsule = LicenseEntangledCapsule::for_testing_with_state(
                0,
                transform,
                0,
                sig_bits,
            );

            seeds.iter().map(|&(seed, idx)| capsule.mask_stream(seed, idx)).collect()
        }

        let transform = 0xFEEDFACE;
        let sig_bits = [0x1, 0x2, 0x3, 0x4];
        let seeds: Vec<(u64, u64)> = (0..100).map(|i| (i / 10, i % 10)).collect();

        let masks1 = generate_masks(transform, sig_bits, &seeds);
        let masks2 = generate_masks(transform, sig_bits, &seeds);

        assert_eq!(masks1, masks2);
    }

    /// Q33: Audit hash chain determinism
    #[test]
    fn test_audit_hash_determinism() {
        fn compute_chain(anchor: u64, entries: &[(u8, u8, u32, u32, u64)]) -> Vec<u64> {
            let capsule = LicenseAuditCapsule::new(anchor);

            entries
                .iter()
                .map(|&(op, feat, input, output, ts)| capsule.append(op, feat, input as u64, output as u64, ts))
                .collect()
        }

        let anchor = 0xABCDEF;
        let entries = [
            (1, 0, 100, 200, 1000),
            (2, 1, 300, 400, 2000),
            (3, 2, 500, 600, 3000),
        ];

        let chain1 = compute_chain(anchor, &entries);
        let chain2 = compute_chain(anchor, &entries);

        assert_eq!(chain1, chain2);
    }

    /// Q34: Generation entanglement determinism
    #[test]
    fn test_generation_entanglement_determinism() {
        fn run_increments(factor: u64, count: usize) -> Vec<u64> {
            let gen = EntangledGeneration::new(factor);
            (0..count).map(|_| gen.increment()).collect()
        }

        let factor = 0x12345678;
        let count = 100;

        let values1 = run_increments(factor, count);
        let values2 = run_increments(factor, count);

        assert_eq!(values1, values2);
    }

    /// Q35: Rotation schedule determinism
    #[test]
    fn test_rotation_schedule_determinism() {
        let schedule = RotationSchedule::new(100, 50, 0xFEED);

        // Same epoch = same factor
        for epoch in 0..20 {
            let factor1 = schedule.factor_for_epoch(epoch);
            let factor2 = schedule.factor_for_epoch(epoch);
            assert_eq!(factor1, factor2);
        }

        // Verify rotation decisions are deterministic
        assert!(!schedule.needs_rotation(0, 50));
        assert!(!schedule.needs_rotation(0, 99));
        assert!(schedule.needs_rotation(0, 100));
        assert!(schedule.needs_rotation(0, 200));
    }
}
