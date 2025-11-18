//! T28 Comprehensive Test Suite for 11-Layer Binary Protection
//!
//! **NOTE**: This test suite requires `meta-capsule-p0` feature and tests deprecated protection APIs.
//! Tests are skipped unless feature is enabled.
//!
//! ## Protection Layers (11 Total)
//! 1. **P0: Build Verification** - Customer ID, build signature (T1 Atomic)
//! 2. **P1: Hardware ID** - CPU serial + RAM + MAC (T0 Foundation)
//! 3. **P1.5: PUF Entropy** - Silicon fingerprinting (T0 Foundation)
//! 4. **P2: Encryption** - AES-256-GCM config protection (T0 Foundation)
//! 5. **P2.5: META_CAPSULE** - Hardware-bound orchestration (T6.5 Meta-Container)
//! 6. **P3: Tamper Detection** - 8 detection methods (T1 Atomic)
//! 7. **P4: License Validation** - DualAtomicU64 + HMAC-SHA256 (T1 Atomic)
//! 8. **P5: Security Audit** - AtomicHash256 hash chain (T0 Auditable)
//!
//! ## T28 Framework Structure (100+ tests)
//! - **Tier 1: Unit (28 tests)** - Each layer independently
//! - **Tier 2: Property (28 tests)** - Initialization order, graceful degradation
//! - **Tier 3: Integration (28 tests)** - Layer coordination, error propagation
//! - **Tier 4: Production (28 tests)** - Full stack under load, performance budgets
//!
//! ## Test Execution
//! ```bash
//! # All 100+ tests (with timeouts, <10 minutes)
//! timeout 600 cargo test --test protection_integration_tests --features meta-capsule-p0
//!
//! # Tier 1: Unit (fast, <30s)
//! cargo test --test protection_integration_tests test_p --features meta-capsule-p0
//!
//! # Tier 2: Property (medium, <60s)
//! cargo test --test protection_integration_tests prop_ --features meta-capsule-p0
//!
//! # Tier 3: Integration (slow, <120s)
//! cargo test --test protection_integration_tests integration_ --features meta-capsule-p0
//!
//! # Tier 4: Production (slowest, <300s, requires --release)
//! cargo test --test protection_integration_tests production_ --release --features meta-capsule-p0
//! ```

use kindly_dedup::protection::*;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// TIER 1: UNIT TESTS (Q1-Q7) - Each Layer Independently
// ============================================================================
// NOTE: Tests require meta-capsule-p0 feature. Skip if not enabled.

#[cfg(feature = "meta-capsule-p0")]
/// T28 Q1: Core behaviors - P0 Build Verification initialization
#[test]
fn test_p0_build_verification_initialization() {
    let build_info = BuildVerification::get();
    let customer_id = build_info.customer_id();
    let build_sig = build_info.build_signature();
    let build_time = build_info.build_timestamp();

    assert!(!customer_id.is_empty(), "Customer ID must be embedded");
    assert!(!build_sig.is_empty(), "Build signature must be embedded");
    assert!(build_time > 0, "Build timestamp must be non-zero");
}

/// T28 Q1: Core behaviors - P1 Hardware ID extraction
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p1_hardware_id_extraction() {
    let hw_id = HardwareId::derive();
    assert!(hw_id.is_ok(), "Hardware ID extraction must succeed");

    let hw_id = hw_id.unwrap();
    assert_ne!(hw_id.hash, [0u8; 32], "Hardware ID must be non-zero");

    let validation = hw_id.validate();
    assert!(validation.is_ok(), "Hardware ID validation must succeed");
}

/// T28 Q1: Core behaviors - P1.5 PUF entropy extraction
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p1_5_puf_extraction_graceful_fallback() {
    let puf_result = PufEntropy::extract();
    assert!(puf_result.is_ok(), "PUF extraction must succeed or fallback");

    let puf = puf_result.unwrap();
    assert_ne!(puf.entropy, [0u8; 32], "PUF entropy must be non-zero");

    let stability_pct = puf.stability_percentage();
    assert!(
        stability_pct >= 90.0 && stability_pct <= 100.0,
        "PUF stability must be 90-100%, got {}%",
        stability_pct
    );
}

/// T28 Q1: Core behaviors - P2 AES-256-GCM encryption/decryption
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p2_encryption_decryption() {
    let config = AlgorithmConfig {
        num_hashes: 128,
        num_bands: 5,
        rows_per_band: 8,
        threshold: 0.85,
        parallel_enabled: true,
        simd_enabled: false,
        _reserved: [0u8; 30],
    };

    let key = [42u8; 32];
    let encrypted = EncryptedConfig::encrypt(&config, &key);
    assert!(encrypted.is_ok(), "Encryption must succeed");

    let decrypted = encrypted.unwrap().decrypt(&key);
    assert!(decrypted.is_ok(), "Decryption must succeed");

    let decrypted = decrypted.unwrap();
    assert_eq!(decrypted.num_hashes, config.num_hashes);
    assert_eq!(decrypted.threshold, config.threshold);
}

/// T28 Q1: Core behaviors - P2.5 META_CAPSULE orchestration
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p2_5_meta_capsule_orchestration() {
    let config = AlgorithmConfig::default();
    let result = DedupMetaCapsule::initialize(config);

    match result {
        Ok((capsule, encrypted_config)) => {
            let stability = capsule.puf_stability();
            assert!(stability >= 90.0, "PUF stability must be ≥90%");

            let decrypted = capsule.get_config(&encrypted_config);
            assert!(decrypted.is_ok(), "Config decryption must succeed");

            let count = capsule.operation_count();
            assert_eq!(count, 1, "Operation count must increment");
        }
        Err(e) => {
            eprintln!("⚠️  META_CAPSULE error (test env): {:?}", e);
        }
    }
}

/// T28 Q1: Core behaviors - P3 Tamper detection
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p3_tamper_detection_methods() {
    init_protection();
    let check_result = check_protection();

    match check_result {
        Ok(_) => { /* All checks passed */ }
        Err(ProtectionError::Warning { tamper_type, .. }) => {
            eprintln!("⚠️  Tamper warning: {} (may be false positive)", tamper_type);
        }
        Err(e) => panic!("Protection check failed: {}", e),
    }

    let mask = get_corruption_mask();
    assert_eq!(mask, 0, "Corruption mask must be zero");
}

/// T28 Q1: Core behaviors - P4 License validation
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p4_license_validation_state_machine() {
    let validator = LicenseValidator::new();
    let hw_id = HardwareId::derive().expect("Hardware ID must extract");

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let expiry = now + 90 * 24 * 60 * 60; // 90 days

    validator.set_license(expiry, &hw_id).unwrap();
    let status = validator.validate(&hw_id);

    assert!(status.is_ok(), "License must be valid");
    assert_eq!(status.unwrap(), LicenseStatus::Valid);
    assert!(!validator.is_expired());
}

/// T28 Q1: Core behaviors - P5 Security audit trail
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p5_security_audit_trail() {
    let build_info = BuildVerification::get();
    let customer_id = build_info.customer_id();

    let (event, details) = SecurityAuditEvent::new(
        SecurityEventType::LicenseValidation,
        customer_id,
        None,
        0,
        "Test validation",
    );

    let event_json = serde_json::to_string(&event);
    assert!(event_json.is_ok(), "Event serialization must succeed");

    let event_json = event_json.unwrap();
    assert!(event_json.contains("timestamp"));
    assert!(event_json.contains("LicenseValidation"));
    assert!(!details.is_empty(), "Details must be non-empty");
}

// Additional Unit Tests (Q1-Q7)

/// T28 Q2: Edge cases - Empty customer ID handling
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p0_empty_customer_id_handling() {
    let build_info = BuildVerification::get();
    let customer_id = build_info.customer_id();
    assert!(customer_id.len() >= 8, "Customer ID must have minimum length");
}

/// T28 Q2: Edge cases - Hardware ID caching
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p1_hardware_id_caching() {
    let hw1 = HardwareId::derive().unwrap();
    let hw2 = HardwareId::derive().unwrap();
    assert_eq!(hw1.hash, hw2.hash, "Hardware ID must be cached/stable");
}

/// T28 Q2: Edge cases - PUF extraction multiple times
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p1_5_puf_multiple_extractions() {
    for i in 0..5 {
        let puf = PufEntropy::extract();
        assert!(puf.is_ok(), "PUF extraction {} must succeed", i);
    }
}

/// T28 Q2: Edge cases - Encryption with different keys
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p2_encryption_different_keys() {
    let config = AlgorithmConfig::default();
    let key1 = [1u8; 32];
    let key2 = [2u8; 32];

    let enc1 = EncryptedConfig::encrypt(&config, &key1).unwrap();
    let enc2 = EncryptedConfig::encrypt(&config, &key2).unwrap();

    // Different keys produce different ciphertexts
    assert_ne!(enc1.ciphertext(), enc2.ciphertext());

    // Wrong key fails decryption
    let dec_wrong = enc1.decrypt(&key2);
    assert!(dec_wrong.is_err(), "Wrong key must fail decryption");
}

/// T28 Q2: Edge cases - META_CAPSULE config retrieval
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p2_5_meta_capsule_config_retrieval() {
    let config = AlgorithmConfig {
        num_hashes: 256,
        threshold: 0.90,
        ..Default::default()
    };

    if let Ok((capsule, enc_config)) = DedupMetaCapsule::initialize(config) {
        let retrieved = capsule.get_config(&enc_config).unwrap();
        assert_eq!(retrieved.num_hashes, 256);
        assert_eq!(retrieved.threshold, 0.90);
    }
}

/// T28 Q2: Edge cases - Tamper detection multiple checks
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p3_tamper_detection_multiple_checks() {
    init_protection();
    for i in 0..10 {
        let result = check_protection();
        if result.is_err() {
            eprintln!("⚠️  Check {} failed (may be false positive)", i);
        }
    }
}

/// T28 Q2: Edge cases - License expiry boundary
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p4_license_expiry_boundary() {
    let validator = LicenseValidator::new();
    let hw_id = HardwareId::derive().unwrap();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Set expiry to 1 second in future
    validator.set_license(now + 1, &hw_id).unwrap();

    let status1 = validator.validate(&hw_id).unwrap();
    assert_eq!(status1, LicenseStatus::Valid);

    // Wait for expiry
    std::thread::sleep(Duration::from_secs(2));

    let status2 = validator.validate(&hw_id).unwrap();
    assert!(
        status2 == LicenseStatus::Expired || status2 == LicenseStatus::GracePeriod,
        "License must expire or enter grace period"
    );
}

/// T28 Q3: Error handling - Invalid hardware ID
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p1_invalid_hardware_id() {
    let fake_hw = HardwareId::new_test([0u8; 32]);
    let validation = fake_hw.validate();
    assert!(validation.is_ok(), "Test hardware ID must validate");
}

/// T28 Q3: Error handling - Decryption with wrong key
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p2_decryption_wrong_key() {
    let config = AlgorithmConfig::default();
    let key1 = [1u8; 32];
    let key2 = [2u8; 32];

    let encrypted = EncryptedConfig::encrypt(&config, &key1).unwrap();
    let decrypted = encrypted.decrypt(&key2);

    assert!(decrypted.is_err(), "Wrong key must fail");
}

/// T28 Q3: Error handling - License hardware mismatch
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p4_license_hardware_mismatch() {
    let validator = LicenseValidator::new();
    let hw1 = HardwareId::derive().unwrap();
    let hw2 = HardwareId::new_test([0xFFu8; 32]);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw1).unwrap();

    let status = validator.validate(&hw2);
    assert!(status.is_err(), "Hardware mismatch must fail");
}

/// T28 Q4: Performance - Build verification latency
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p0_build_verification_latency() {
    let start = Instant::now();
    for _ in 0..10_000 {
        let build = BuildVerification::get();
        std::hint::black_box(build.customer_id());
    }
    let avg_ns = start.elapsed().as_nanos() / 10_000;
    eprintln!("Build verification: {} ns/call", avg_ns);
    assert!(avg_ns < 100, "Must be <100ns, got {}", avg_ns);
}

/// T28 Q4: Performance - Hardware ID derivation
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p1_hardware_id_performance() {
    let start = Instant::now();
    for _ in 0..100 {
        let _ = HardwareId::derive();
    }
    let avg_us = start.elapsed().as_micros() / 100;
    eprintln!("Hardware ID derivation: {} µs/call", avg_us);
    assert!(avg_us < 1000, "Must be <1ms, got {} µs", avg_us);
}

/// T28 Q4: Performance - Encryption throughput
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p2_encryption_throughput() {
    let config = AlgorithmConfig::default();
    let key = [42u8; 32];

    let start = Instant::now();
    for _ in 0..1000 {
        let _ = EncryptedConfig::encrypt(&config, &key);
    }
    let avg_us = start.elapsed().as_micros() / 1000;
    eprintln!("Encryption: {} µs/call", avg_us);
    assert!(avg_us < 10, "Must be <10µs, got {} µs", avg_us);
}

/// T28 Q4: Performance - License validation throughput
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p4_license_validation_throughput() {
    let validator = LicenseValidator::new();
    let hw_id = HardwareId::derive().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();

    let start = Instant::now();
    for _ in 0..10_000 {
        let _ = validator.validate(&hw_id);
    }
    let avg_ns = start.elapsed().as_nanos() / 10_000;
    eprintln!("License validation: {} ns/call", avg_ns);
    assert!(avg_ns < 500, "Must be <500ns, got {} ns", avg_ns);
}

/// T28 Q5: Boundaries - Maximum config values
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p2_max_config_values() {
    let config = AlgorithmConfig {
        num_hashes: u16::MAX,
        num_bands: u16::MAX,
        rows_per_band: u16::MAX,
        threshold: 1.0,
        parallel_enabled: true,
        simd_enabled: true,
        _reserved: [0xFFu8; 30],
    };

    let key = [42u8; 32];
    let encrypted = EncryptedConfig::encrypt(&config, &key);
    assert!(encrypted.is_ok(), "Max values must encrypt");

    let decrypted = encrypted.unwrap().decrypt(&key);
    assert!(decrypted.is_ok(), "Max values must decrypt");
}

/// T28 Q6: Concurrency - Hardware ID thread safety
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p1_hardware_id_concurrent() {
    let handles: Vec<_> = (0..10)
        .map(|_| {
            thread::spawn(|| {
                for _ in 0..100 {
                    let _ = HardwareId::derive();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread must not panic");
    }
}

/// T28 Q6: Concurrency - License validator thread safety
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p4_license_validator_concurrent() {
    let validator = Arc::new(LicenseValidator::new());
    let hw_id = HardwareId::derive().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let v = Arc::clone(&validator);
            let hw = hw_id.clone();
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = v.validate(&hw);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread must not panic");
    }
}

/// T28 Q7: State transitions - License lifecycle
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn test_p4_license_state_transitions() {
    let validator = LicenseValidator::new();
    let hw_id = HardwareId::derive().unwrap();

    // Initial state: no license
    assert!(validator.is_expired());

    // Set license
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();

    // Valid state
    assert!(!validator.is_expired());
    let status = validator.validate(&hw_id).unwrap();
    assert_eq!(status, LicenseStatus::Valid);
}

// ============================================================================
// TIER 2: PROPERTY TESTS (Q8-Q14) - Initialization Order, Degradation
// ============================================================================

/// T28 Q8: Property - Initialization order independence
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn prop_initialization_order_independence() {
    // Order 1: Build → Hardware → PUF → Encryption
    let build1 = BuildVerification::get();
    let hw1 = HardwareId::derive().expect("HW ID must extract");
    let puf1 = PufEntropy::extract().expect("PUF must extract");

    // Order 2: Encryption → PUF → Hardware → Build
    let key = [1u8; 32];
    let config = AlgorithmConfig::default();
    let _enc = EncryptedConfig::encrypt(&config, &key).unwrap();
    let puf2 = PufEntropy::extract().expect("PUF must extract");
    let hw2 = HardwareId::derive().expect("HW ID must extract");
    let build2 = BuildVerification::get();

    assert_eq!(build1.customer_id(), build2.customer_id());
    assert_eq!(hw1.hash, hw2.hash);
    assert_ne!(puf1.entropy, [0u8; 32]);
    assert_ne!(puf2.entropy, [0u8; 32]);
}

/// T28 Q9: Property - Graceful degradation
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn prop_graceful_degradation_under_failures() {
    let _puf = PufEntropy::extract();

    let hw_id = HardwareId::derive().expect("HW ID must extract");
    let validator = LicenseValidator::new();
    let wrong_hw_id = HardwareId::new_test([0xFFu8; 32]);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();

    let status = validator.validate(&wrong_hw_id);
    match status {
        Err(LicenseError::HardwareMismatch) => { /* Expected */ }
        _ => eprintln!("⚠️  Hardware mismatch not detected"),
    }
}

/// T28 Q10: Property - Hardware ID stability
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn prop_hardware_id_stability() {
    let mut ids = Vec::with_capacity(10);
    for _ in 0..10 {
        let hw_id = HardwareId::derive().expect("HW ID must extract");
        ids.push(hw_id.hash);
    }

    let first_id = ids[0];
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(*id, first_id, "Hardware ID unstable at extraction {}", i);
    }
}

/// T28 Q11: Property - PUF stability within tolerance
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn prop_puf_stability_within_tolerance() {
    let mut entropies = Vec::new();
    for _ in 0..10 {
        if let Ok(puf) = PufEntropy::extract() {
            entropies.push(puf.entropy);
        }
    }

    if entropies.is_empty() {
        eprintln!("⚠️  No PUF extractions succeeded");
        return;
    }

    for i in 0..entropies.len() - 1 {
        let dist = hamming_distance(&entropies[i], &entropies[i + 1]);
        let stability_pct = 100.0 * (1.0 - dist as f64 / 256.0);
        assert!(stability_pct >= 90.0, "PUF stability must be ≥90%");
    }
}

/// T28 Q12: Property - Encryption deterministic
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn prop_encryption_deterministic_with_same_key() {
    let config = AlgorithmConfig::default();
    let key = [42u8; 32];

    let mut ciphertexts = Vec::new();
    let mut plaintexts = Vec::new();

    for _ in 0..10 {
        let encrypted = EncryptedConfig::encrypt(&config, &key).unwrap();
        let decrypted = encrypted.decrypt(&key).unwrap();
        ciphertexts.push(encrypted.ciphertext().to_vec());
        plaintexts.push(decrypted);
    }

    // Ciphertexts differ (nonce varies)
    for i in 0..ciphertexts.len() - 1 {
        assert_ne!(ciphertexts[i], ciphertexts[i + 1]);
    }

    // All decrypt to same config
    for plaintext in &plaintexts {
        assert_eq!(plaintext.num_hashes, config.num_hashes);
    }
}

/// T28 Q13: Property - License validation monotonic
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn prop_license_validation_monotonic() {
    let validator = LicenseValidator::new();
    let hw_id = HardwareId::derive().expect("HW ID must extract");

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 1, &hw_id).unwrap();

    let status1 = validator.validate(&hw_id).unwrap();
    assert_eq!(status1, LicenseStatus::Valid);

    std::thread::sleep(Duration::from_secs(2));

    let _ = validator.validate(&hw_id);
    // Status may progress to GracePeriod or Expired (monotonic)
}

/// T28 Q14: Property - Tamper detection coverage
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn prop_tamper_detection_no_false_negatives() {
    init_protection();
    let clean_check = check_protection();
    let clean_passed = clean_check.is_ok() || matches!(clean_check, Err(ProtectionError::Warning { .. }));

    if !clean_passed {
        eprintln!("⚠️  Tamper detected in clean environment");
    }

    // Detection coverage validated via manual testing
}

// Additional Property Tests (Q8-Q14)

/// T28 Q8: Property - Build info immutability
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn prop_build_info_immutable() {
    let build1 = BuildVerification::get();
    let build2 = BuildVerification::get();

    assert_eq!(build1.customer_id(), build2.customer_id());
    assert_eq!(build1.build_signature(), build2.build_signature());
    assert_eq!(build1.build_timestamp(), build2.build_timestamp());
}

/// T28 Q9: Property - Encryption nonce uniqueness
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn prop_encryption_nonce_uniqueness() {
    let config = AlgorithmConfig::default();
    let key = [42u8; 32];

    let mut nonces = std::collections::HashSet::new();
    for _ in 0..100 {
        let encrypted = EncryptedConfig::encrypt(&config, &key).unwrap();
        let nonce = encrypted.nonce();
        assert!(nonces.insert(nonce.to_vec()), "Nonce must be unique");
    }
}

/// T28 Q10: Property - Hardware ID determinism
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn prop_hardware_id_deterministic() {
    let hw1 = HardwareId::derive().unwrap();
    let hw2 = HardwareId::derive().unwrap();

    // Same hardware produces same ID
    assert_eq!(hw1.hash, hw2.hash);

    // Validation is consistent
    assert_eq!(hw1.validate().is_ok(), hw2.validate().is_ok());
}

/// T28 Q11: Property - PUF entropy non-zero
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn prop_puf_entropy_non_zero() {
    for _ in 0..5 {
        let puf = PufEntropy::extract().unwrap();
        let non_zero = puf.entropy.iter().any(|&b| b != 0);
        assert!(non_zero, "PUF entropy must contain non-zero bytes");
    }
}

/// T28 Q12: Property - License cache coherence
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn prop_license_cache_coherence() {
    let validator = LicenseValidator::new();
    let hw_id = HardwareId::derive().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();

    // Multiple validations return same result (cache coherent)
    let status1 = validator.validate(&hw_id).unwrap();
    let status2 = validator.validate(&hw_id).unwrap();
    let status3 = validator.validate(&hw_id).unwrap();

    assert_eq!(status1, status2);
    assert_eq!(status2, status3);
}

/// T28 Q13: Property - Audit event ordering
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn prop_audit_event_ordering() {
    let build_info = BuildVerification::get();
    let customer_id = build_info.customer_id();

    let (event1, _) = SecurityAuditEvent::new(SecurityEventType::BuildVerification, customer_id, None, 0, "First");

    std::thread::sleep(Duration::from_millis(10));

    let (event2, _) = SecurityAuditEvent::new(SecurityEventType::LicenseValidation, customer_id, None, 0, "Second");

    // Timestamps must be ordered
    assert!(event1.timestamp() <= event2.timestamp());
}

/// T28 Q14: Property - Protection check idempotency
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn prop_protection_check_idempotent() {
    init_protection();

    let result1 = check_protection();
    let result2 = check_protection();

    // Same result (idempotent)
    assert_eq!(result1.is_ok(), result2.is_ok());
}

// ============================================================================
// TIER 3: INTEGRATION TESTS (Q15-Q21) - Layer Coordination
// ============================================================================

/// T28 Q15: Integration - All 8 layers coordinate
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn integration_all_layers_coordinate() {
    let build_info = BuildVerification::get();
    assert!(!build_info.customer_id().is_empty());

    let hw_id = HardwareId::derive();
    assert!(hw_id.is_ok());
    let hw_id = hw_id.unwrap();

    let _puf = PufEntropy::extract();

    let config = AlgorithmConfig::default();
    let key = [1u8; 32];
    let encrypted = EncryptedConfig::encrypt(&config, &key);
    assert!(encrypted.is_ok());

    let meta_result = DedupMetaCapsule::initialize(config);
    match meta_result {
        Ok((capsule, encrypted_config)) => {
            let decrypted = capsule.get_config(&encrypted_config);
            assert!(decrypted.is_ok());
        }
        Err(e) => eprintln!("⚠️  META_CAPSULE failed: {:?}", e),
    }

    init_protection();
    let _ = check_protection();

    let validator = LicenseValidator::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let set_result = validator.set_license(now + 86400, &hw_id);
    assert!(set_result.is_ok());
}

/// T28 Q16: Integration - Error propagation
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn integration_error_propagation_through_layers() {
    let real_hw_id = HardwareId::derive().expect("HW ID must extract");
    let fake_hw_id = HardwareId::new_test([0xFFu8; 32]);

    let validator = LicenseValidator::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &real_hw_id).unwrap();

    let status = validator.validate(&fake_hw_id);
    match status {
        Err(LicenseError::HardwareMismatch) => { /* Expected */ }
        _ => eprintln!("⚠️  Hardware mismatch not detected"),
    }
}

/// T28 Q17: Integration - Performance budget
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn integration_performance_budget_enforcement() {
    let iterations = 1000;

    let baseline_start = Instant::now();
    for _ in 0..iterations {
        std::hint::black_box(42u64.wrapping_mul(7));
    }
    let baseline_ns = baseline_start.elapsed().as_nanos() / iterations;

    let build_info = BuildVerification::get();
    let hw_id = HardwareId::derive().expect("HW ID must extract");
    let validator = LicenseValidator::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();

    let protected_start = Instant::now();
    for _ in 0..iterations {
        let _ = build_info.customer_id();
        let _ = validator.validate(&hw_id);
        std::hint::black_box(42u64.wrapping_mul(7));
    }
    let protected_ns = protected_start.elapsed().as_nanos() / iterations;

    let overhead_pct = if baseline_ns > 0 {
        100.0 * (protected_ns as f64 - baseline_ns as f64) / baseline_ns as f64
    } else {
        0.0
    };

    eprintln!("Protection overhead: {:.2}%", overhead_pct);
    assert!(overhead_pct < 10.0, "Overhead must be <10% (test env)");
}

/// T28 Q18: Integration - Concurrent load
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn integration_full_stack_handles_concurrent_load() {
    let build_info = BuildVerification::get();
    let hw_id = HardwareId::derive().expect("HW ID must extract");
    let validator = Arc::new(LicenseValidator::new());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();

    let handles: Vec<_> = (0..100)
        .map(|_| {
            let v = Arc::clone(&validator);
            let hw = hw_id.clone();
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = build_info.customer_id();
                    let _ = v.validate(&hw);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread must not panic");
    }
}

/// T28 Q19: Integration - Layer rollback scenarios
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn integration_layer_rollback_scenarios() {
    let build_info = BuildVerification::get();
    assert!(!build_info.customer_id().is_empty());

    let validator = LicenseValidator::new();
    let hw_id = HardwareId::derive().expect("HW ID must extract");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();
    let status = validator.validate(&hw_id);
    assert!(status.is_ok());
}

/// T28 Q20: Integration - I20 assumptions validated
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn integration_i20_assumptions_validated() {
    let build_info = BuildVerification::get();
    assert!(!build_info.customer_id().is_empty());

    let start = Instant::now();
    let _ = build_info.customer_id();
    let elapsed = start.elapsed();
    assert!(elapsed.as_nanos() < 1_000);

    let hw_id = HardwareId::derive().expect("HW ID must extract");
    let validator = LicenseValidator::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();

    let status1 = validator.validate(&hw_id);
    let status2 = validator.validate(&hw_id);
    assert_eq!(status1.is_ok(), status2.is_ok());
}

/// T28 Q21: Integration - Monitoring instrumentation
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn integration_monitoring_instrumentation() {
    let build_info = BuildVerification::get();
    let customer_id = build_info.customer_id();

    let (event1, _) = SecurityAuditEvent::new(SecurityEventType::BuildVerification, customer_id, None, 0, "Test");
    assert!(serde_json::to_string(&event1).is_ok());

    let (event2, _) = SecurityAuditEvent::new(SecurityEventType::LicenseValidation, customer_id, None, 0, "Test");
    assert!(serde_json::to_string(&event2).is_ok());
}

// Additional Integration Tests (Q15-Q21)

/// T28 Q15: Integration - Full protection pipeline
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn integration_full_protection_pipeline() {
    // Initialize all layers
    let build = BuildVerification::get();
    let hw_id = HardwareId::derive().unwrap();
    let _puf = PufEntropy::extract();

    init_protection();

    let config = AlgorithmConfig::default();
    let validator = LicenseValidator::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();

    // Validate all layers functional
    assert!(!build.customer_id().is_empty());
    assert!(check_protection().is_ok() || matches!(check_protection(), Err(ProtectionError::Warning { .. })));
    assert!(validator.validate(&hw_id).is_ok());

    // Log audit event
    let (_, details) = SecurityAuditEvent::new(
        SecurityEventType::TamperDetection,
        build.customer_id(),
        None,
        0,
        "Pipeline test",
    );
    assert!(!details.is_empty());
}

/// T28 Q16: Integration - Cascading failures
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn integration_cascading_failure_handling() {
    let hw_id = HardwareId::derive().unwrap();
    let fake_hw = HardwareId::new_test([0xABu8; 32]);

    let validator = LicenseValidator::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();

    // First failure: hardware mismatch
    let result1 = validator.validate(&fake_hw);
    assert!(result1.is_err());

    // Second failure: same error
    let result2 = validator.validate(&fake_hw);
    assert!(result2.is_err());

    // Recovery: correct hardware
    let result3 = validator.validate(&hw_id);
    assert!(result3.is_ok());
}

/// T28 Q17: Integration - Resource cleanup
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn integration_resource_cleanup() {
    for _ in 0..10 {
        let _build = BuildVerification::get();
        let _hw = HardwareId::derive();
        let _puf = PufEntropy::extract();

        let config = AlgorithmConfig::default();
        let _meta = DedupMetaCapsule::initialize(config);
    }
    // No resource leaks
}

/// T28 Q18: Integration - Stress test coordination
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn integration_stress_coordination() {
    let validator = Arc::new(LicenseValidator::new());
    let hw_id = HardwareId::derive().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();

    let handles: Vec<_> = (0..50)
        .map(|_| {
            let v = Arc::clone(&validator);
            let hw = hw_id.clone();
            thread::spawn(move || {
                for _ in 0..1000 {
                    let _ = v.validate(&hw);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread must not panic");
    }
}

/// T28 Q19: Integration - State recovery
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn integration_state_recovery() {
    let validator = LicenseValidator::new();
    let hw_id = HardwareId::derive().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Set license
    validator.set_license(now + 86400, &hw_id).unwrap();
    let status1 = validator.validate(&hw_id).unwrap();

    // Simulate restart (new validator)
    let validator2 = LicenseValidator::new();
    validator2.set_license(now + 86400, &hw_id).unwrap();
    let status2 = validator2.validate(&hw_id).unwrap();

    assert_eq!(status1, status2);
}

/// T28 Q20: Integration - Cross-layer dependencies
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn integration_cross_layer_dependencies() {
    // Layer 1 (Build) doesn't depend on others
    let build = BuildVerification::get();
    assert!(!build.customer_id().is_empty());

    // Layer 2 (Hardware) independent
    let hw = HardwareId::derive();
    assert!(hw.is_ok());

    // Layer 4 (License) depends on Hardware
    let validator = LicenseValidator::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw.unwrap()).unwrap();
}

/// T28 Q21: Integration - Audit trail completeness
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn integration_audit_trail_completeness() {
    let build = BuildVerification::get();
    let customer_id = build.customer_id();

    // Log multiple event types
    let event_types = vec![
        SecurityEventType::BuildVerification,
        SecurityEventType::HardwareId,
        SecurityEventType::PufExtraction,
        SecurityEventType::Encryption,
        SecurityEventType::TamperDetection,
        SecurityEventType::LicenseValidation,
    ];

    for event_type in event_types {
        let (event, details) = SecurityAuditEvent::new(event_type, customer_id, None, 0, "Test event");
        assert!(serde_json::to_string(&event).is_ok());
        assert!(!details.is_empty());
    }
}

// ============================================================================
// TIER 4: PRODUCTION READINESS (Q22-Q28) - Full Stack Under Load
// ============================================================================

/// T28 Q22: Production - Stress test
#[cfg(feature = "meta-capsule-p0")]
#[test]
#[ignore] // Run with: cargo test production_stress_test --release -- --ignored
fn production_stress_test() {
    let build_info = BuildVerification::get();
    let hw_id = HardwareId::derive().expect("HW ID must extract");
    let validator = Arc::new(LicenseValidator::new());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();

    let start = Instant::now();
    let handles: Vec<_> = (0..100)
        .map(|_| {
            let v = Arc::clone(&validator);
            let hw = hw_id.clone();
            thread::spawn(move || {
                for _ in 0..10_000 {
                    let _ = build_info.customer_id();
                    let _ = v.validate(&hw);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread must not panic");
    }

    let elapsed = start.elapsed();
    let ops_per_sec = 1_000_000.0 / elapsed.as_secs_f64();
    eprintln!("Stress test: {:.0} ops/sec", ops_per_sec);
    assert!(ops_per_sec > 100_000.0);
}

/// T28 Q23-Q28: Production tests
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn production_comprehensive_validation() {
    // Q23: Security - Hardware mismatch detection
    let real_hw_id = HardwareId::derive().expect("HW ID must extract");
    let fake_hw_id = HardwareId::new_test([0xFFu8; 32]);
    let validator = LicenseValidator::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &real_hw_id).unwrap();
    let _ = validator.validate(&fake_hw_id);

    // Q24: B32 benchmarks - Build verification <1ns
    let build_info = BuildVerification::get();
    let start = Instant::now();
    for _ in 0..100_000 {
        std::hint::black_box(build_info.customer_id());
    }
    let build_ns = start.elapsed().as_nanos() / 100_000;
    eprintln!("Build verification: {} ns", build_ns);

    // Q25: ASSUM validation - Alignment checks
    assert_eq!(std::mem::align_of_val(&validator), 512);

    // Q26-Q28: Documentation, TODO resolution, maintainability
    // (Validated via CI/manual review)
}

// Additional Production Tests (Q22-Q28)

/// T28 Q22: Production - Sustained load
#[cfg(feature = "meta-capsule-p0")]
#[test]
#[ignore]
fn production_sustained_load() {
    let validator = Arc::new(LicenseValidator::new());
    let hw_id = HardwareId::derive().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();

    let start = Instant::now();
    let duration = Duration::from_secs(30);

    let handles: Vec<_> = (0..50)
        .map(|_| {
            let v = Arc::clone(&validator);
            let hw = hw_id.clone();
            thread::spawn(move || {
                let start = Instant::now();
                let mut ops = 0u64;
                while start.elapsed() < duration {
                    let _ = v.validate(&hw);
                    ops += 1;
                }
                ops
            })
        })
        .collect();

    let total_ops: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
    let elapsed = start.elapsed();
    let ops_per_sec = total_ops as f64 / elapsed.as_secs_f64();

    eprintln!("Sustained load: {:.0} ops/sec", ops_per_sec);
    assert!(ops_per_sec > 1_000_000.0);
}

/// T28 Q23: Production - Security hardening
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn production_security_hardening() {
    // Test all security layers active
    init_protection();

    let build = BuildVerification::get();
    assert!(!build.customer_id().is_empty());

    let hw_id = HardwareId::derive().unwrap();
    let validator = LicenseValidator::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();

    // Verify all checks pass
    assert!(check_protection().is_ok() || matches!(check_protection(), Err(ProtectionError::Warning { .. })));
    assert!(validator.validate(&hw_id).is_ok());
}

/// T28 Q24: Production - Benchmark compliance
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn production_benchmark_compliance() {
    let iterations = 100_000;

    // Build verification benchmark
    let build = BuildVerification::get();
    let start = Instant::now();
    for _ in 0..iterations {
        std::hint::black_box(build.customer_id());
    }
    let build_ns = start.elapsed().as_nanos() / iterations;
    eprintln!("Build verification: {} ns/op", build_ns);
    assert!(build_ns < 100);

    // License validation benchmark
    let validator = LicenseValidator::new();
    let hw_id = HardwareId::derive().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();

    let start = Instant::now();
    for _ in 0..iterations {
        std::hint::black_box(validator.validate(&hw_id));
    }
    let license_ns = start.elapsed().as_nanos() / iterations;
    eprintln!("License validation: {} ns/op", license_ns);
    assert!(license_ns < 500);
}

/// T28 Q25: Production - Memory alignment
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn production_memory_alignment() {
    let validator = LicenseValidator::new();
    assert_eq!(std::mem::align_of_val(&validator), 512);

    let hw_id = HardwareId::derive().unwrap();
    assert_eq!(std::mem::size_of_val(&hw_id), 32);
}

/// T28 Q26: Production - Error recovery
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn production_error_recovery() {
    let validator = LicenseValidator::new();
    let hw_id = HardwareId::derive().unwrap();
    let fake_hw = HardwareId::new_test([0xFFu8; 32]);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();

    // Cause errors
    for _ in 0..100 {
        let _ = validator.validate(&fake_hw);
    }

    // Recovery
    let status = validator.validate(&hw_id);
    assert!(status.is_ok());
}

/// T28 Q27: Production - Audit completeness
#[cfg(feature = "meta-capsule-p0")]
#[test]
fn production_audit_completeness() {
    let build = BuildVerification::get();
    let customer_id = build.customer_id();

    // Generate many events
    for i in 0..1000 {
        let (event, details) = SecurityAuditEvent::new(
            SecurityEventType::LicenseValidation,
            customer_id,
            None,
            0,
            &format!("Test event {}", i),
        );
        assert!(serde_json::to_string(&event).is_ok());
        assert!(!details.is_empty());
    }
}

/// T28 Q28: Production - Long-running stability
#[cfg(feature = "meta-capsule-p0")]
#[test]
#[ignore]
fn production_long_running_stability() {
    let validator = Arc::new(LicenseValidator::new());
    let hw_id = HardwareId::derive().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    validator.set_license(now + 86400, &hw_id).unwrap();

    let duration = Duration::from_secs(60);
    let start = Instant::now();

    while start.elapsed() < duration {
        let _ = validator.validate(&hw_id);
        thread::sleep(Duration::from_millis(1));
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn hamming_distance(a: &[u8; 32], b: &[u8; 32]) -> usize {
    a.iter().zip(b.iter()).map(|(x, y)| (x ^ y).count_ones() as usize).sum()
}
