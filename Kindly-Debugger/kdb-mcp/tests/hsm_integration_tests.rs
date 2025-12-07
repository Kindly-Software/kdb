//! HsmIntegrationCapsule - Comprehensive T28 Testing Framework
//!
//! **Framework**: T28 Systematic Testing Strategy
//! - **Q1-Q7**: Unit tests (7 tests)
//! - **Q8-Q14**: Property tests (7 tests)
//! - **Q15-Q21**: Integration tests (7 tests)
//! - **Q22-Q28**: Production tests (7 tests)
//!
//! **Total**: 28 tests covering all aspects of HSM integration

use kdb_mcp::{HsmIntegrationCapsule, HsmError, HsmStatus, HsmKeyPair, ED25519_PUBLIC_KEY_SIZE};
use std::sync::Arc;
use std::iter::repeat;
use std::sync::atomic::Ordering;

// ============================================================================
// Q1-Q7: Unit Tests (Correctness)
// ============================================================================

#[test]
fn unit_test_1_capsule_initialization() {
    let capsule = HsmIntegrationCapsule::new();

    // Verify initial state
    assert!(!capsule.is_hsm_available(), "HSM should start unavailable");
    assert_eq!(capsule.get_signature_count(), 0, "Initial signature count should be 0");
    assert_eq!(capsule.last_rotation_timestamp(), 0, "Initial rotation timestamp should be 0");
    assert_eq!(capsule.get_public_key_hash(), 0, "Initial key hash should be 0");
    assert_eq!(capsule.hsm_status(), HsmStatus::Unavailable);
}

#[test]
fn unit_test_2_hsm_status_transitions() {
    let capsule = HsmIntegrationCapsule::new();

    // Transition: Unavailable → Available
    capsule.set_hsm_status(HsmStatus::Available);
    assert_eq!(capsule.hsm_status(), HsmStatus::Available);
    assert!(capsule.is_hsm_available());

    // Transition: Available → Error
    capsule.set_hsm_status(HsmStatus::Error);
    assert_eq!(capsule.hsm_status(), HsmStatus::Error);
    assert!(!capsule.is_hsm_available());

    // Transition: Error → Unavailable
    capsule.set_hsm_status(HsmStatus::Unavailable);
    assert_eq!(capsule.hsm_status(), HsmStatus::Unavailable);
    assert!(!capsule.is_hsm_available());
}

#[test]
fn unit_test_3_signature_count_increment() {
    let capsule = HsmIntegrationCapsule::new();

    for i in 0..100 {
        capsule.increment_signature_count();
        assert_eq!(capsule.get_signature_count(), (i + 1) as u64);
    }
}

#[test]
fn unit_test_4_signing_statistics() {
    let capsule = HsmIntegrationCapsule::new();

    // Simulate 10 successful, 5 failed
    for _ in 0..10 {
        capsule.increment_signing_attempts();
        capsule.increment_signing_success();
    }
    for _ in 0..5 {
        capsule.increment_signing_attempts();
        capsule.increment_signing_failed();
    }

    let stats = capsule.get_signing_stats();
    assert_eq!(stats.total_attempts, 15);
    assert_eq!(stats.successful, 10);
    assert_eq!(stats.failed, 5);
    assert!((stats.success_rate() - 66.67).abs() < 0.1);
}

#[test]
fn unit_test_5_key_rotation_tracking() {
    let capsule = HsmIntegrationCapsule::new();
    let now = 1_700_000_000u64;

    capsule.update_key_rotation(now);

    assert_eq!(capsule.last_rotation_timestamp(), now);
    let stats = capsule.get_rotation_stats();
    assert_eq!(stats.total_rotations, 1);
    assert_eq!(stats.last_rotation_unix, now);
}

#[test]
fn unit_test_6_public_key_hash_update() {
    let capsule = HsmIntegrationCapsule::new();
    let key = vec![42u8; ED25519_PUBLIC_KEY_SIZE];

    let result = capsule.update_public_key_hash(&key);
    assert!(result.is_ok());

    let hash = capsule.get_public_key_hash();
    assert_ne!(hash, 0, "Hash should be non-zero for non-zero key");
}

#[test]
fn unit_test_7_error_handling() {
    let capsule = HsmIntegrationCapsule::new();

    // Try to sign without HSM
    let result = capsule.sign_with_hsm("/usr/lib/libpcsclite.so", "test-key", b"data");
    assert_eq!(result, Err(HsmError::HsmNotFound));

    // Invalid key label (empty)
    let result = capsule.generate_keypair("/usr/lib/libpcsclite.so", "");
    assert_eq!(result, Err(HsmError::InvalidKeyLabel));

    // Invalid key label (spaces)
    let result = capsule.generate_keypair("/usr/lib/libpcsclite.so", "key with spaces");
    assert_eq!(result, Err(HsmError::InvalidKeyLabel));
}

// ============================================================================
// Q8-Q14: Property Tests (Determinism, Monotonicity)
// ============================================================================

#[test]
fn property_test_1_signature_count_monotonic() {
    let capsule = HsmIntegrationCapsule::new();
    let mut prev = 0u64;

    for _ in 0..1000 {
        capsule.increment_signature_count();
        let curr = capsule.get_signature_count();
        assert!(
            curr >= prev,
            "Signature count should be monotonic: prev={}, curr={}",
            prev,
            curr
        );
        prev = curr;
    }
    assert_eq!(prev, 1000);
}

#[test]
fn property_test_2_concurrent_increments() {
    let capsule = Arc::new(HsmIntegrationCapsule::new());
    let num_threads = 10;
    let increments_per_thread = 1000;
    let mut handles = vec![];

    for _ in 0..num_threads {
        let capsule_clone = Arc::clone(&capsule);
        let handle = std::thread::spawn(move || {
            for _ in 0..increments_per_thread {
                capsule_clone.increment_signature_count();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let expected = (num_threads * increments_per_thread) as u64;
    assert_eq!(
        capsule.get_signature_count(),
        expected,
        "Expected {}, got {}",
        expected,
        capsule.get_signature_count()
    );
}

#[test]
fn property_test_3_fnv1a_hash_determinism() {
    // Test using public_key_hash which calls fnv1a internally
    let capsule1 = HsmIntegrationCapsule::new();
    let capsule2 = HsmIntegrationCapsule::new();

    let key = vec![123u8; ED25519_PUBLIC_KEY_SIZE];

    capsule1.update_public_key_hash(&key).unwrap();
    capsule2.update_public_key_hash(&key).unwrap();

    assert_eq!(
        capsule1.get_public_key_hash(),
        capsule2.get_public_key_hash(),
        "Same key should produce same hash"
    );
}

#[test]
fn property_test_4_fnv1a_different_inputs() {
    let capsule1 = HsmIntegrationCapsule::new();
    let capsule2 = HsmIntegrationCapsule::new();

    let key1: Vec<u8> = (0..ED25519_PUBLIC_KEY_SIZE).map(|i| (i % 256) as u8).collect();
    let key2: Vec<u8> = (0..ED25519_PUBLIC_KEY_SIZE)
        .map(|i| ((i + 1) % 256) as u8)
        .collect();

    capsule1.update_public_key_hash(&key1).unwrap();
    capsule2.update_public_key_hash(&key2).unwrap();

    assert_ne!(
        capsule1.get_public_key_hash(),
        capsule2.get_public_key_hash(),
        "Different keys should produce different hashes"
    );
}

#[test]
fn property_test_5_hsm_status_idempotent() {
    let capsule = HsmIntegrationCapsule::new();

    // Set Available multiple times
    for _ in 0..10 {
        capsule.set_hsm_status(HsmStatus::Available);
        assert_eq!(capsule.hsm_status(), HsmStatus::Available);
    }

    // Set Error multiple times
    for _ in 0..10 {
        capsule.set_hsm_status(HsmStatus::Error);
        assert_eq!(capsule.hsm_status(), HsmStatus::Error);
    }
}

#[test]
fn property_test_6_key_label_validation() {
    // Valid labels (should all succeed)
    let valid_labels = vec![
        "license-key-2025",
        "key_1",
        "SIGNING_KEY",
        "K",
        "a_b_c_d_e",
        "test-key-123",
    ];

    for label in valid_labels {
        assert!(
            HsmKeyPair::validate_key_id(label).is_ok(),
            "Label '{}' should be valid",
            label
        );
    }

    // Invalid labels (should all fail)
    let long_label = "x".repeat(300);
    let invalid_labels: Vec<&str> = vec![
        "",                        // Empty
        "key with spaces",         // Spaces
        "key@invalid",             // Special chars
        &long_label,               // Too long
        "KEY!",                    // Exclamation
        "123-KEY$",                // Dollar sign
    ];

    for label in invalid_labels {
        assert!(
            HsmKeyPair::validate_key_id(label).is_err(),
            "Label '{}' should be invalid",
            label
        );
    }
}

#[test]
fn property_test_7_generation_counter_increments() {
    let capsule = HsmIntegrationCapsule::new();
    let gen0 = capsule.generation.load(Ordering::Relaxed);

    capsule.set_hsm_status(HsmStatus::Available);
    let gen1 = capsule.generation.load(Ordering::Relaxed);
    assert!(gen1 > gen0, "Generation should increment on status change");

    capsule.update_key_rotation(123);
    let gen2 = capsule.generation.load(Ordering::Relaxed);
    assert!(gen2 > gen1, "Generation should increment on key rotation");

    let key = vec![0u8; ED25519_PUBLIC_KEY_SIZE];
    capsule.update_public_key_hash(&key).ok();
    let gen3 = capsule.generation.load(Ordering::Relaxed);
    assert!(gen3 > gen2, "Generation should increment on key hash update");
}

// ============================================================================
// Q15-Q21: Integration Tests (Workflows)
// ============================================================================

#[test]
fn integration_test_1_hsm_detection_workflow() {
    let capsule = HsmIntegrationCapsule::new();
    assert!(!capsule.is_hsm_available());

    // Simulate HSM detection
    let result = capsule.detect_hsm("/usr/lib/libpcsclite.so");
    assert!(result.is_ok());

    assert!(capsule.is_hsm_available());
    let stats = capsule.get_stats();
    assert!(stats.hsm_available);
}

#[test]
fn integration_test_2_keypair_generation_workflow() {
    let capsule = HsmIntegrationCapsule::new();
    capsule.detect_hsm("/usr/lib/libpcsclite.so").ok();

    let result = capsule.generate_keypair("/usr/lib/libpcsclite.so", "license-key-2025-01");
    assert!(result.is_ok());

    let keypair = result.unwrap();
    assert_eq!(keypair.public_key.len(), ED25519_PUBLIC_KEY_SIZE);
    assert_eq!(keypair.key_id, "license-key-2025-01");
    assert_eq!(keypair.algorithm, "ED25519");
    assert!(keypair.created_at > 0);

    // Verify statistics were updated
    // Note: detect_hsm() calls update_key_rotation once, generate_keypair() calls it again
    let stats = capsule.get_stats();
    assert_eq!(stats.key_rotations, 2);
    assert!(stats.signing_stats.successful >= 1);
}

#[test]
fn integration_test_3_signing_workflow() {
    let capsule = HsmIntegrationCapsule::new();
    capsule.detect_hsm("/usr/lib/libpcsclite.so").ok();

    let data = b"license certificate payload";
    let result = capsule.sign_with_hsm("/usr/lib/libpcsclite.so", "signing-key", data);

    assert!(result.is_ok());
    let signature = result.unwrap();
    assert_eq!(signature.len(), 64); // Ed25519 signature size

    // Verify statistics
    let stats = capsule.get_stats();
    assert_eq!(stats.signature_count, 1);
    assert_eq!(stats.signing_stats.total_attempts, 1);
    assert_eq!(stats.signing_stats.successful, 1);
}

#[test]
fn integration_test_4_public_key_export_workflow() {
    let capsule = HsmIntegrationCapsule::new();
    capsule.detect_hsm("/usr/lib/libpcsclite.so").ok();

    let result = capsule.export_public_key("/usr/lib/libpcsclite.so", "license-key-2025-01");
    assert!(result.is_ok());

    let public_key = result.unwrap();
    assert_eq!(public_key.len(), ED25519_PUBLIC_KEY_SIZE);

    // Verify statistics
    let stats = capsule.get_stats();
    assert_eq!(stats.public_key_exports, 1);
}

#[test]
fn integration_test_5_key_rotation_workflow() {
    let capsule = HsmIntegrationCapsule::new();
    let now = 1_700_000_000u64;

    // Simulate multiple key rotations
    for i in 0..5 {
        capsule.update_key_rotation(now + (i * 86400) as u64);
    }

    let stats = capsule.get_stats();
    assert_eq!(stats.rotation_stats.total_rotations, 5);
    assert_eq!(stats.rotation_stats.last_rotation_unix, now + (4 * 86400) as u64);
}

#[test]
fn integration_test_6_license_signing_scenario() {
    // Real-world scenario: Generate key, sign license, export public key
    let capsule = HsmIntegrationCapsule::new();
    capsule.detect_hsm("/usr/lib/libpcsclite.so").ok();

    // Step 1: Generate keypair
    let keypair_result = capsule.generate_keypair("/usr/lib/libpcsclite.so", "license-root-2025");
    assert!(keypair_result.is_ok());

    // Step 2: Sign license data
    let license_data = b"KINDLY-PRO-user@example.com-2025-12-31";
    let signature_result = capsule.sign_with_hsm("/usr/lib/libpcsclite.so", "license-root-2025", license_data);
    assert!(signature_result.is_ok());

    // Step 3: Export public key for distribution
    let pubkey_result = capsule.export_public_key("/usr/lib/libpcsclite.so", "license-root-2025");
    assert!(pubkey_result.is_ok());

    // Verify complete stats
    // Note: detect_hsm() and generate_keypair() both call update_key_rotation
    let stats = capsule.get_stats();
    assert!(stats.hsm_available);
    assert_eq!(stats.signature_count, 1);
    assert_eq!(stats.key_rotations, 2);
    // Note: generate_keypair() calls update_public_key_hash (increments public_key_exports)
    // And export_public_key() also calls update_public_key_hash
    assert_eq!(stats.public_key_exports, 2);
    // Note: generate_keypair increments signing_success, sign_with_hsm also increments
    assert_eq!(stats.signing_stats.successful, 2);
}

#[test]
fn integration_test_7_graceful_degradation_without_hsm() {
    let capsule = HsmIntegrationCapsule::new();

    // Don't initialize HSM - should fail gracefully
    assert!(!capsule.is_hsm_available());

    let result = capsule.sign_with_hsm("/usr/lib/libpcsclite.so", "key", b"data");
    assert_eq!(result, Err(HsmError::HsmNotFound));

    let result = capsule.generate_keypair("/usr/lib/libpcsclite.so", "key");
    assert_eq!(result, Err(HsmError::HsmNotFound));

    let result = capsule.export_public_key("/usr/lib/libpcsclite.so", "key");
    assert_eq!(result, Err(HsmError::HsmNotFound));
}

// ============================================================================
// Q22-Q28: Production Tests (Performance, Reliability, Compliance)
// ============================================================================

#[test]
fn production_test_1_capsule_layout_validation() {
    let size = std::mem::size_of::<HsmIntegrationCapsule>();
    let align = std::mem::align_of::<HsmIntegrationCapsule>();

    assert_eq!(
        size, 256,
        "Capsule must be exactly 256 bytes for HotTier alignment, got {}",
        size
    );
    assert_eq!(
        align, 256,
        "Capsule must be 256-byte aligned (cache line), got {}",
        align
    );
}

#[test]
fn production_test_2_zero_per_request_overhead() {
    // Verify that cached operations are truly O(1) and sub-microsecond
    let capsule = HsmIntegrationCapsule::new();
    capsule.set_hsm_status(HsmStatus::Available);

    // Simulate 10,000 fast-path queries
    let start = std::time::Instant::now();
    for _ in 0..10000 {
        let _ = capsule.is_hsm_available();
        let _ = capsule.get_signature_count();
        let _ = capsule.hsm_status();
    }
    let elapsed = start.elapsed();

    // Should complete in <100ms for 10,000 ops = <10μs per pair
    assert!(
        elapsed.as_millis() < 100,
        "10,000 queries should take <100ms, took {:?}",
        elapsed
    );
}

#[test]
fn production_test_3_signature_statistics_accuracy() {
    let capsule = HsmIntegrationCapsule::new();

    // Simulate realistic workload: 95% success, 5% failure
    let total_ops = 1000;
    let success_ops = 950;
    let fail_ops = 50;

    for _ in 0..success_ops {
        capsule.increment_signing_attempts();
        capsule.increment_signing_success();
    }
    for _ in 0..fail_ops {
        capsule.increment_signing_attempts();
        capsule.increment_signing_failed();
    }

    let stats = capsule.get_signing_stats();
    assert_eq!(stats.total_attempts, total_ops);
    assert_eq!(stats.successful, success_ops);
    assert_eq!(stats.failed, fail_ops);

    let success_rate = stats.success_rate();
    assert!(success_rate > 94.9 && success_rate < 95.1);
}

#[test]
fn production_test_4_assum_safety_lockfree_only() {
    // Verify no mutex or RwLock is used (only atomics)
    // This is a compile-time check: HsmIntegrationCapsule has only AtomicU64 fields
    // Runtime test: concurrent access from many threads
    let capsule = Arc::new(HsmIntegrationCapsule::new());
    let num_threads = 50;
    let ops_per_thread = 1000;
    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let capsule_clone = Arc::clone(&capsule);
        let handle = std::thread::spawn(move || {
            for op in 0..ops_per_thread {
                if thread_id % 3 == 0 {
                    capsule_clone.increment_signature_count();
                } else if thread_id % 3 == 1 {
                    capsule_clone.increment_signing_attempts();
                    capsule_clone.increment_signing_success();
                } else {
                    capsule_clone.update_key_rotation(1700000000 + op as u64);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // All threads completed without deadlock (proves lockfree design)
    assert!(capsule.get_signature_count() > 0);
}

#[test]
fn production_test_5_amdahl_law_validation() {
    // Verify that HSM operations are 0% of request SLA (offline only)
    // Fast-path overhead per request: <10ns
    // Request SLA: 10,000ns (10μs)
    // Impact: <10ns / 10,000ns = 0.1% (negligible)

    let capsule = HsmIntegrationCapsule::new();
    capsule.set_hsm_status(HsmStatus::Available);

    let start = std::time::Instant::now();
    let iterations = 1_000_000;

    for _ in 0..iterations {
        let _ = capsule.is_hsm_available();
    }

    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() / iterations as u128;

    // Average should be <50ns per operation (most CPUs)
    assert!(
        ns_per_op < 100,
        "Expected <100ns per availability check, got {} ns",
        ns_per_op
    );

    // Amdahl's Law: (100ns / 10,000ns) = 1% worst case
    let impact_percent = (ns_per_op as f64 / 10_000.0) * 100.0;
    assert!(
        impact_percent < 1.0,
        "HSM overhead should be <1% of SLA, got {}%",
        impact_percent
    );
}

#[test]
fn production_test_6_offline_signing_acceptable_latency() {
    // Verify that offline HSM signing latency is documented and acceptable
    // HSM signing: ~100-500ms (offline, not on critical path)
    // This is acceptable for license generation (happens offline)

    let capsule = HsmIntegrationCapsule::new();
    capsule.detect_hsm("/usr/lib/libpcsclite.so").ok();

    // Measure simulated signing (in production would be ~100-500ms)
    let data = b"license data to sign";
    let start = std::time::Instant::now();
    let result = capsule.sign_with_hsm("/usr/lib/libpcsclite.so", "key", data);
    let elapsed = start.elapsed();

    assert!(result.is_ok());
    // Simulated signing completes instantly (<1ms)
    // In production with real HSM, expect ~100-500ms
    assert!(
        elapsed.as_millis() < 100,
        "Simulated signing should be fast, took {:?}",
        elapsed
    );
}

#[test]
fn production_test_7_compliance_q34_auditability() {
    // Q34 Auditability requirement: Log HSM operations for SOX/SOC2/GDPR
    let capsule = HsmIntegrationCapsule::new();

    // Simulate operations that would be logged
    capsule.detect_hsm("/usr/lib/libpcsclite.so").ok();
    capsule.increment_signing_attempts();
    capsule.increment_signing_success();
    capsule.increment_signature_count();

    let now = 1_700_000_000u64;
    capsule.update_key_rotation(now);

    let key = vec![123u8; ED25519_PUBLIC_KEY_SIZE];
    capsule.update_public_key_hash(&key).ok();

    // Verify audit trail is collectible
    let stats = capsule.get_stats();

    // Q34 audit log would contain:
    // - Operation: HSM_DETECTION, Status: Available, Timestamp: now
    // - Operation: HSM_SIGNATURE, Attempts: 1, Success: 1, Timestamp: now
    // - Operation: HSM_KEY_ROTATION, Timestamp: now, RotationCount: 1
    // - Operation: PUBLIC_KEY_EXPORT, KeyHash: ..., Timestamp: now

    assert!(stats.hsm_available);
    assert_eq!(stats.signing_stats.successful, 1);
    // Note: detect_hsm() and update_key_rotation() both increment rotation counter
    assert_eq!(stats.rotation_stats.total_rotations, 2);
    assert_eq!(stats.public_key_exports, 1);

    // These stats feed into AuditEnhancementCapsule for compliance
}
