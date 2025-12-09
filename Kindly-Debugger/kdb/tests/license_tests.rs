//! LicenseValidatorCapsule T28 Tests
//!
//! **Framework**: T28 5-Tier Testing (Unit/Property/Integration/Production/Determinism)
//!
//! **Coverage**:
//! - Q1-Q7: Unit tests (parse, verify, expiration, tier limits)
//! - Q8-Q14: Property tests (format invariants, hash chain integrity)
//! - Q15-Q21: Integration tests (quota integration, multi-component)
//! - Q22-Q28: Production stress tests (concurrent validation, rate limiting)
//! - Q29-Q35: Determinism tests (reproducible behavior, audit trail)
//!
//! **Target**: 100% pass rate, <10s total execution

#[cfg(target_os = "linux")]
mod license_tests {
    use kdb::ptrace::license::{LicenseError, LicenseTier, LicenseValidatorCapsule, VerificationState};
    use kdb::ptrace::quota::QuotaTrackerCapsule;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    // ========================================================================
    // Q1-Q7: Unit Tests
    // ========================================================================

    mod unit_tests {
        use super::*;

        #[test]
        fn test_capsule_size_and_alignment() {
            // T0+T1 capsule MUST be exactly 256 bytes, 64-byte aligned
            assert_eq!(
                std::mem::size_of::<LicenseValidatorCapsule>(),
                256,
                "LicenseValidatorCapsule must be 256 bytes"
            );
            assert_eq!(
                std::mem::align_of::<LicenseValidatorCapsule>(),
                64,
                "LicenseValidatorCapsule must be 64-byte aligned"
            );
        }

        #[test]
        fn test_license_tier_variants() {
            // All 5 tiers must be parseable
            assert_eq!(LicenseTier::from_str("HOB"), Some(LicenseTier::Hobby));
            assert_eq!(LicenseTier::from_str("STR"), Some(LicenseTier::Starter));
            assert_eq!(LicenseTier::from_str("DEV"), Some(LicenseTier::Developer));
            assert_eq!(LicenseTier::from_str("PRO"), Some(LicenseTier::Professional));
            assert_eq!(LicenseTier::from_str("ENT"), Some(LicenseTier::Enterprise));

            // Case insensitivity
            assert_eq!(LicenseTier::from_str("hob"), Some(LicenseTier::Hobby));
            assert_eq!(LicenseTier::from_str("Str"), Some(LicenseTier::Starter));

            // Invalid tiers
            assert_eq!(LicenseTier::from_str("INVALID"), None);
            assert_eq!(LicenseTier::from_str(""), None);
            assert_eq!(LicenseTier::from_str("PRO1"), None);
        }

        #[test]
        fn test_tier_as_str_roundtrip() {
            for tier in [
                LicenseTier::Hobby,
                LicenseTier::Starter,
                LicenseTier::Developer,
                LicenseTier::Professional,
                LicenseTier::Enterprise,
            ] {
                let s = tier.as_str();
                let parsed = LicenseTier::from_str(s);
                assert_eq!(parsed, Some(tier), "Roundtrip failed for {:?}", tier);
            }
        }

        #[test]
        fn test_tier_u8_roundtrip() {
            for i in 0u8..=4 {
                let tier = LicenseTier::from_u8(i).unwrap();
                assert_eq!(tier.as_u8(), i);
            }

            // Invalid u8 values
            assert!(LicenseTier::from_u8(5).is_none());
            assert!(LicenseTier::from_u8(255).is_none());
        }

        #[test]
        fn test_tier_limits_hobby() {
            assert_eq!(LicenseTier::Hobby.snapshots_per_day(), 50);
            assert_eq!(LicenseTier::Hobby.session_duration_secs(), 3600);
            assert_eq!(LicenseTier::Hobby.rate_limit_per_min(), 30);
            assert_eq!(LicenseTier::Hobby.monthly_price_cents(), 0);
        }

        #[test]
        fn test_tier_limits_starter() {
            assert_eq!(LicenseTier::Starter.snapshots_per_day(), 500);
            assert_eq!(LicenseTier::Starter.session_duration_secs(), 8 * 3600);
            assert_eq!(LicenseTier::Starter.rate_limit_per_min(), 120);
            assert_eq!(LicenseTier::Starter.monthly_price_cents(), 900);
        }

        #[test]
        fn test_tier_limits_developer() {
            assert_eq!(LicenseTier::Developer.snapshots_per_day(), 5000);
            assert_eq!(LicenseTier::Developer.session_duration_secs(), 24 * 3600);
            assert_eq!(LicenseTier::Developer.rate_limit_per_min(), 300);
            assert_eq!(LicenseTier::Developer.monthly_price_cents(), 2900);
        }

        #[test]
        fn test_tier_limits_professional() {
            assert_eq!(LicenseTier::Professional.snapshots_per_day(), u64::MAX);
            assert_eq!(LicenseTier::Professional.session_duration_secs(), u64::MAX);
            assert_eq!(LicenseTier::Professional.rate_limit_per_min(), 600);
            assert_eq!(LicenseTier::Professional.monthly_price_cents(), 7900);
        }

        #[test]
        fn test_tier_limits_enterprise() {
            assert_eq!(LicenseTier::Enterprise.snapshots_per_day(), u64::MAX);
            assert_eq!(LicenseTier::Enterprise.session_duration_secs(), u64::MAX);
            assert_eq!(LicenseTier::Enterprise.rate_limit_per_min(), 1200);
            assert_eq!(LicenseTier::Enterprise.monthly_price_cents(), 0); // Custom pricing
        }

        #[test]
        fn test_tier_ordering() {
            // Tiers should be orderable (for upgrade/downgrade logic)
            assert!(LicenseTier::Hobby < LicenseTier::Starter);
            assert!(LicenseTier::Starter < LicenseTier::Developer);
            assert!(LicenseTier::Developer < LicenseTier::Professional);
            assert!(LicenseTier::Professional < LicenseTier::Enterprise);
        }

        #[test]
        fn test_new_unverified() {
            let validator = LicenseValidatorCapsule::new_unverified();

            assert_eq!(validator.get_tier(), LicenseTier::Hobby);
            assert_eq!(validator.get_verification_state(), VerificationState::Pending);
            assert!(!validator.is_valid());
            assert_eq!(validator.get_validation_count(), 0);
            assert_eq!(validator.get_failure_count(), 0);
        }

        #[test]
        fn test_parse_invalid_format() {
            // Missing parts
            let result = LicenseValidatorCapsule::parse("KDB");
            assert!(matches!(result, Err(LicenseError::InvalidFormat { .. })));

            let result = LicenseValidatorCapsule::parse("KDB-PRO");
            assert!(matches!(result, Err(LicenseError::InvalidFormat { .. })));

            // Invalid prefix
            let result = LicenseValidatorCapsule::parse("XDB-PRO-12345678-ABCDEF12-sig");
            assert!(matches!(result, Err(LicenseError::InvalidFormat { .. })));
        }

        #[test]
        fn test_parse_unknown_tier() {
            let result = LicenseValidatorCapsule::parse("KDB-XXX-12345678-ABCDEF12-AAAA");
            assert!(matches!(result, Err(LicenseError::UnknownTier { .. })));
        }

        #[test]
        fn test_parse_invalid_timestamp() {
            let result = LicenseValidatorCapsule::parse("KDB-PRO-GHIJKLMN-ABCDEF12-AAAA");
            assert!(matches!(result, Err(LicenseError::InvalidTimestamp { .. })));
        }

        #[test]
        fn test_parse_invalid_org_hash() {
            let result = LicenseValidatorCapsule::parse("KDB-PRO-12345678-ZZZZZZZZ-AAAA");
            assert!(matches!(result, Err(LicenseError::InvalidOrgHash { .. })));
        }

        #[test]
        fn test_expiration_check_expired() {
            let validator = LicenseValidatorCapsule::new_unverified();

            // Set to expired using test helper (requires test-helpers feature)
            validator.set_expiration_for_test(1);

            let result = validator.check_expiration();
            assert!(matches!(result, Err(LicenseError::LicenseExpired { .. })));
            assert_eq!(validator.get_verification_state(), VerificationState::Expired);
        }

        #[test]
        fn test_expiration_check_valid() {
            let validator = LicenseValidatorCapsule::new_unverified();

            // Set to far future using test helper
            let future = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + (365 * 24 * 3600); // 1 year from now
            validator.set_expiration_for_test(future);

            let result = validator.check_expiration();
            assert!(result.is_ok());
        }

        #[test]
        fn test_days_until_expiration() {
            let validator = LicenseValidatorCapsule::new_unverified();

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            // 30 days from now
            validator.set_expiration_for_test(now + (30 * 24 * 3600));
            let days = validator.days_until_expiration();
            assert!(days >= 29 && days <= 31);

            // Already expired
            validator.set_expiration_for_test(now - 1000);
            assert_eq!(validator.days_until_expiration(), 0);
        }

        #[test]
        fn test_organization_hash_consistency() {
            // Same input should produce same hash
            let hash1 = LicenseValidatorCapsule::compute_org_hash("Kindly Software");
            let hash2 = LicenseValidatorCapsule::compute_org_hash("Kindly Software");
            assert_eq!(hash1, hash2);

            // Different inputs should produce different hashes
            let hash3 = LicenseValidatorCapsule::compute_org_hash("Other Company");
            assert_ne!(hash1, hash3);
        }

        #[test]
        fn test_organization_verification() {
            let validator = LicenseValidatorCapsule::new_unverified();
            let org_name = "Test Organization";
            let org_hash = LicenseValidatorCapsule::compute_org_hash(org_name);
            validator.set_org_hash_for_test(org_hash);

            // Correct org
            assert!(validator.verify_organization(org_name).is_ok());

            // Incorrect org
            let result = validator.verify_organization("Wrong Organization");
            assert!(matches!(result, Err(LicenseError::OrganizationMismatch { .. })));
        }

        #[test]
        fn test_base64_encoding_decoding() {
            // Test vectors
            let test_cases = vec![
                (vec![0u8; 64], "64-byte signature"),
                (vec![0xFF; 32], "32-byte all-ones"),
                ((0..64).collect::<Vec<u8>>(), "sequential bytes"),
            ];

            for (original, desc) in test_cases {
                let encoded = LicenseValidatorCapsule::encode_base64(&original);
                let decoded = LicenseValidatorCapsule::decode_base64(&encoded).unwrap();
                assert_eq!(original, decoded, "Base64 roundtrip failed for {}", desc);
            }
        }

        #[test]
        fn test_verification_state_transitions() {
            let validator = LicenseValidatorCapsule::new_unverified();

            // Initial state
            assert_eq!(validator.get_verification_state(), VerificationState::Pending);

            // After failed verify (with dev key, this will fail)
            let _ = validator.verify();
            // State should be Invalid (signature doesn't match)
            assert_eq!(validator.get_verification_state(), VerificationState::Invalid);
        }

        #[test]
        fn test_generation_counter_increments() {
            let validator = LicenseValidatorCapsule::new_unverified();
            let gen1 = validator.get_generation();

            // Trigger state change
            let _ = validator.verify();
            let gen2 = validator.get_generation();

            assert!(gen2 > gen1, "Generation should increment on state change");
        }

        #[test]
        fn test_license_status() {
            let validator = LicenseValidatorCapsule::new_unverified();
            let status = validator.get_status();

            assert_eq!(status.tier, LicenseTier::Hobby);
            assert_eq!(status.state, VerificationState::Pending);
            assert_eq!(status.validation_count, 0);
            assert_eq!(status.failure_count, 0);
        }
    }

    // ========================================================================
    // Q8-Q14: Property Tests
    // ========================================================================

    #[cfg(feature = "property-tests")]
    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn prop_tier_u8_roundtrip(tier_val in 0u8..=4) {
                let tier = LicenseTier::from_u8(tier_val).unwrap();
                assert_eq!(tier.as_u8(), tier_val);
            }

            #[test]
            fn prop_org_hash_deterministic(org_name in "[a-zA-Z0-9 ]{1,100}") {
                let hash1 = LicenseValidatorCapsule::compute_org_hash(&org_name);
                let hash2 = LicenseValidatorCapsule::compute_org_hash(&org_name);
                assert_eq!(hash1, hash2);
            }

            #[test]
            fn prop_base64_roundtrip(data in prop::collection::vec(any::<u8>(), 0..128)) {
                let encoded = LicenseValidatorCapsule::encode_base64(&data);
                let decoded = LicenseValidatorCapsule::decode_base64(&encoded).unwrap();
                assert_eq!(data, decoded);
            }

            #[test]
            fn prop_tier_limits_positive(tier_val in 0u8..=4) {
                let tier = LicenseTier::from_u8(tier_val).unwrap();
                assert!(tier.snapshots_per_day() > 0);
                assert!(tier.session_duration_secs() > 0);
                assert!(tier.rate_limit_per_min() > 0);
            }

            #[test]
            fn prop_tier_ordering_monotonic(a in 0u8..=4, b in 0u8..=4) {
                let tier_a = LicenseTier::from_u8(a).unwrap();
                let tier_b = LicenseTier::from_u8(b).unwrap();

                if a < b {
                    assert!(tier_a < tier_b);
                } else if a > b {
                    assert!(tier_a > tier_b);
                } else {
                    assert_eq!(tier_a, tier_b);
                }
            }
        }
    }

    // ========================================================================
    // Q15-Q21: Integration Tests
    // ========================================================================

    mod integration_tests {
        use super::*;

        #[test]
        fn test_quota_from_license_hobby() {
            let validator = LicenseValidatorCapsule::new_unverified();
            // Hobby tier (default for unverified)

            let quota = QuotaTrackerCapsule::new_from_license(&validator, 1);

            // Should map to Hobby limits (unverified defaults to Hobby)
            assert_eq!(quota.snapshots_limit_value(), 50);
            assert_eq!(quota.session_limit_ns_value(), 3600 * 1_000_000_000);
            assert_eq!(quota.tokens_max_value(), 30);
        }

        #[test]
        fn test_quota_from_license_verified() {
            let validator = LicenseValidatorCapsule::new_unverified();

            // Simulate verified Professional tier using test helpers
            // Note: In production, this would come from actual license verification
            #[cfg(feature = "test-helpers")]
            {
                validator.set_tier_for_test(LicenseTier::Professional);
                validator.set_verification_state_for_test(VerificationState::Valid);
            }

            // Without test-helpers feature, manually set via atomic operations
            #[cfg(not(feature = "test-helpers"))]
            {
                // Use the internal atomic field accessors that are pub in tests
                use std::sync::atomic::AtomicU8;
                // The tier field is at the start of the struct
                let tier_ptr = &validator as *const LicenseValidatorCapsule as *const AtomicU8;
                unsafe {
                    (*tier_ptr).store(LicenseTier::Professional as u8, Ordering::Relaxed);
                    // verification_state is at offset 1
                    let state_ptr = (tier_ptr as *const u8).add(1) as *const AtomicU8;
                    (*state_ptr).store(VerificationState::Valid as u8, Ordering::Relaxed);
                }
            }

            let quota = QuotaTrackerCapsule::new_from_license(&validator, 1);

            // Should map to Professional limits
            assert_eq!(quota.snapshots_limit_value(), u64::MAX);
            assert_eq!(quota.session_limit_ns_value(), u64::MAX);
            assert_eq!(quota.tokens_max_value(), 600);
        }

        #[test]
        fn test_audit_hash_chain_integrity() {
            let validator = LicenseValidatorCapsule::new_unverified();

            let hash1 = validator.get_audit_hash();

            // First event
            validator.update_audit_hash(b"EVENT_1");
            let hash2 = validator.get_audit_hash();

            // Second event
            validator.update_audit_hash(b"EVENT_2");
            let hash3 = validator.get_audit_hash();

            // All hashes should be different (chain progresses)
            assert_ne!(hash1, hash2);
            assert_ne!(hash2, hash3);
            assert_ne!(hash1, hash3);

            // Event count should increment
            assert_eq!(validator.get_audit_event_count(), 2);
        }

        #[test]
        fn test_multi_tier_quota_mapping() {
            // Test all tier mappings using the license_tier_to_quota_params helper
            use kdb::ptrace::license::license_tier_to_quota_params;

            let tiers = [
                (LicenseTier::Hobby, 50, 3600u64),
                (LicenseTier::Starter, 500, 8 * 3600),
                (LicenseTier::Developer, 5000, 24 * 3600),
                (LicenseTier::Professional, u64::MAX, u64::MAX),
                (LicenseTier::Enterprise, u64::MAX, u64::MAX),
            ];

            for (tier, expected_snapshots, expected_duration_secs) in tiers {
                // Get quota params for verified license
                let (snapshots, session_ns, _tokens, _refill) =
                    license_tier_to_quota_params(tier, true);

                assert_eq!(
                    snapshots,
                    expected_snapshots,
                    "Snapshot limit mismatch for {:?}",
                    tier
                );

                if expected_duration_secs == u64::MAX {
                    assert_eq!(
                        session_ns,
                        u64::MAX,
                        "Session limit mismatch for {:?}",
                        tier
                    );
                } else {
                    assert_eq!(
                        session_ns,
                        expected_duration_secs * 1_000_000_000,
                        "Session limit mismatch for {:?}",
                        tier
                    );
                }
            }
        }
    }

    // ========================================================================
    // Q22-Q28: Production Stress Tests
    // ========================================================================

    mod production_tests {
        use super::*;

        #[test]
        fn test_concurrent_validation() {
            let validator = Arc::new(LicenseValidatorCapsule::new_unverified());

            // Spawn multiple threads attempting validation concurrently
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let validator_clone = Arc::clone(&validator);
                    thread::spawn(move || {
                        for _ in 0..100 {
                            let _ = validator_clone.verify();
                            let _ = validator_clone.check_expiration();
                            let _ = validator_clone.get_status();
                        }
                    })
                })
                .collect();

            // All threads should complete without panic
            for handle in handles {
                handle.join().unwrap();
            }

            // Validation count should be 800 (8 threads * 100 iterations)
            assert_eq!(validator.get_validation_count(), 0); // verify() increments only on success
            assert_eq!(validator.get_failure_count(), 800); // All fail with dev key
        }

        #[test]
        fn test_rapid_state_queries() {
            let validator = LicenseValidatorCapsule::new_unverified();
            let start = std::time::Instant::now();

            // 1 million state queries
            for _ in 0..1_000_000 {
                let _ = validator.get_tier();
                let _ = validator.get_verification_state();
                let _ = validator.get_generation();
            }

            let elapsed = start.elapsed();

            // Should complete in <100ms (<33ns per query)
            assert!(
                elapsed.as_millis() < 100,
                "State queries too slow: {:?}",
                elapsed
            );
        }

        #[test]
        fn test_audit_chain_under_load() {
            let validator = LicenseValidatorCapsule::new_unverified();
            let start = std::time::Instant::now();

            // 10,000 audit events
            for i in 0..10_000 {
                let event = format!("EVENT_{}", i);
                validator.update_audit_hash(event.as_bytes());
            }

            let elapsed = start.elapsed();
            let event_count = validator.get_audit_event_count();

            assert_eq!(event_count, 10_000);

            // Should complete in <500ms (<50μs per event)
            assert!(
                elapsed.as_millis() < 500,
                "Audit chain too slow: {:?}",
                elapsed
            );
        }

        #[test]
        fn test_expiration_check_performance() {
            let validator = LicenseValidatorCapsule::new_unverified();
            let future = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + (365 * 24 * 3600);
            validator.set_expiration_for_test(future);

            let start = std::time::Instant::now();

            // 1 million expiration checks
            for _ in 0..1_000_000 {
                let _ = validator.check_expiration();
            }

            let elapsed = start.elapsed();

            // Should complete in <100ms (<100ns per check)
            assert!(
                elapsed.as_millis() < 100,
                "Expiration checks too slow: {:?}",
                elapsed
            );
        }
    }

    // ========================================================================
    // Q29-Q35: Determinism Tests
    // ========================================================================

    mod determinism_tests {
        use super::*;

        #[test]
        fn test_tier_parsing_deterministic() {
            // Same input should always produce same output
            for _ in 0..100 {
                assert_eq!(LicenseTier::from_str("HOB"), Some(LicenseTier::Hobby));
                assert_eq!(LicenseTier::from_str("PRO"), Some(LicenseTier::Professional));
            }
        }

        #[test]
        fn test_org_hash_deterministic() {
            let org_name = "Test Organization Inc.";
            let expected_hash = LicenseValidatorCapsule::compute_org_hash(org_name);

            // Same input should always produce same hash
            for _ in 0..100 {
                let hash = LicenseValidatorCapsule::compute_org_hash(org_name);
                assert_eq!(hash, expected_hash);
            }
        }

        #[test]
        fn test_generation_counter_monotonic() {
            let validator = LicenseValidatorCapsule::new_unverified();
            let mut prev_gen = validator.get_generation();

            // Generation should only increase
            for _ in 0..100 {
                let _ = validator.verify();
                let current_gen = validator.get_generation();
                assert!(
                    current_gen >= prev_gen,
                    "Generation counter went backward: {} -> {}",
                    prev_gen,
                    current_gen
                );
                prev_gen = current_gen;
            }
        }

        #[test]
        fn test_audit_event_count_monotonic() {
            let validator = LicenseValidatorCapsule::new_unverified();
            let mut prev_count = validator.get_audit_event_count();

            for i in 0..100 {
                validator.update_audit_hash(format!("EVENT_{}", i).as_bytes());
                let current_count = validator.get_audit_event_count();
                assert!(
                    current_count > prev_count,
                    "Audit event count didn't increase"
                );
                prev_count = current_count;
            }
        }

        #[test]
        fn test_status_consistency() {
            let validator = LicenseValidatorCapsule::new_unverified();
            validator.set_tier_for_test(LicenseTier::Developer);
            validator.set_verification_state_for_test(VerificationState::Valid);

            // Multiple calls should return consistent results
            let status1 = validator.get_status();
            let status2 = validator.get_status();

            assert_eq!(status1.tier, status2.tier);
            assert_eq!(status1.state, status2.state);
            assert_eq!(status1.org_hash, status2.org_hash);
            assert_eq!(status1.creation_timestamp, status2.creation_timestamp);
        }

        #[test]
        fn test_failed_verify_increments_failure_count() {
            let validator = LicenseValidatorCapsule::new_unverified();
            let initial_failures = validator.get_failure_count();

            // Verify will fail with dev key
            let _ = validator.verify();

            assert_eq!(
                validator.get_failure_count(),
                initial_failures + 1,
                "Failure count should increment after failed verify"
            );
        }
    }

    // ========================================================================
    // Error Display Tests
    // ========================================================================

    mod error_tests {
        use super::*;

        #[test]
        fn test_error_display_invalid_format() {
            let err = LicenseError::InvalidFormat {
                expected: "KDB-TIER-TIMESTAMP-ORG-SIG",
                got: "invalid".to_string(),
            };
            let display = format!("{}", err);
            assert!(display.contains("Invalid license format"));
            assert!(display.contains("invalid"));
        }

        #[test]
        fn test_error_display_unknown_tier() {
            let err = LicenseError::UnknownTier {
                tier: "XXX".to_string(),
            };
            let display = format!("{}", err);
            assert!(display.contains("Unknown license tier"));
            assert!(display.contains("XXX"));
        }

        #[test]
        fn test_error_display_expired() {
            let err = LicenseError::LicenseExpired {
                expired_at: 1000,
                current_time: 2000,
            };
            let display = format!("{}", err);
            assert!(display.contains("expired"));
            assert!(display.contains("1000"));
        }

        #[test]
        fn test_error_display_signature_failed() {
            let err = LicenseError::SignatureVerificationFailed;
            let display = format!("{}", err);
            assert!(display.contains("signature"));
            assert!(display.contains("failed") || display.contains("Invalid"));
        }

        #[test]
        fn test_error_display_org_mismatch() {
            let err = LicenseError::OrganizationMismatch {
                expected_hash: 0x12345678,
                got_hash: 0xABCDEF12,
            };
            let display = format!("{}", err);
            assert!(display.contains("Organization mismatch"));
        }
    }
}
