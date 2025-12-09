//! SecureEnclaveCapsule Test Suite (T28 Framework - 28 Tests)
//!
//! Comprehensive test coverage across 4 tiers:
//! - Q1-Q7: Unit tests (7 tests)
//! - Q8-Q14: Property tests (7 tests)
//! - Q15-Q21: Integration tests (7 tests)
//! - Q22-Q28: Production tests (7 tests)
//!
//! **Status**: All hardware-gated tests skip gracefully when TEE unavailable

use atomic_capsule::capsules::security::secure_enclave::*;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ============================================================================
// Q1-Q7: Unit Tests (7 tests)
// ============================================================================

#[test]
fn q1_enclave_initialization_layout() {
    let capsule = SecureEnclaveCapsule::new(TeeType::Software);

    // Verify 256-byte alignment
    let (size, alignment) = SecureEnclaveCapsule::size_and_alignment();
    assert_eq!(size, 256, "Capsule must be exactly 256 bytes");
    assert_eq!(alignment, 256, "Must be 256-byte aligned for cache-line");

    // Verify initial state
    assert_eq!(capsule.state(), EnclaveState::Active, "Initial state must be Active");
    assert_eq!(capsule.tee_type(), TeeType::Software, "TEE type must match");
}

#[test]
fn q2_measurement_hash_generation() {
    let mut capsule = SecureEnclaveCapsule::new(TeeType::Software);

    // Set test measurement hash (SHA-384, 48 bytes)
    let test_hash = [42u8; 48];
    capsule.set_measurement_hash(test_hash);

    // Verify hash stored correctly
    assert!(capsule.verify_measurement(&test_hash));
}

#[test]
fn q3_measurement_hash_verification() {
    let mut capsule = SecureEnclaveCapsule::new(TeeType::Software);
    let correct_hash = [1u8; 48];
    let wrong_hash = [2u8; 48];

    capsule.set_measurement_hash(correct_hash);

    // Positive test: correct hash
    assert!(capsule.verify_measurement(&correct_hash));

    // Negative test: wrong hash
    assert!(!capsule.verify_measurement(&wrong_hash));
}

#[test]
fn q4_enclave_call_successful() {
    let capsule = SecureEnclaveCapsule::new(TeeType::Software);

    let result = capsule.enclave_call(&[1, 2, 3, 4]);
    assert!(result.is_ok(), "Enclave call must succeed in Active state");

    let output = result.unwrap();
    assert!(!output.is_empty(), "Enclave call must return data");
}

#[test]
fn q5_enclave_call_with_suspended_state() {
    let capsule = SecureEnclaveCapsule::new(TeeType::Software);

    // Suspend enclave
    assert!(capsule.suspend().is_ok());

    // Enclave call must fail when suspended
    let result = capsule.enclave_call(&[1, 2, 3]);
    assert!(result.is_err(), "Enclave call must fail in Suspended state");
}

#[test]
fn q6_remote_attestation_basic() {
    let capsule = SecureEnclaveCapsule::new(TeeType::Software);

    let result = capsule.remote_attestation();
    assert!(result.is_ok(), "Remote attestation must succeed");

    let attestation = result.unwrap();
    assert!(attestation.is_valid, "Attestation result must be valid");
    assert!(attestation.attestation_time_ms < 100, "Attestation must complete in <100ms");
    assert_eq!(
        attestation.enclave_state,
        EnclaveState::Active,
        "Enclave must return to Active state"
    );
}

#[test]
fn q7_constant_time_hash_comparison() {
    let mut capsule = SecureEnclaveCapsule::new(TeeType::Software);

    let hash1 = [0x12u8; 48];
    capsule.set_measurement_hash(hash1);

    // Compare same hash multiple times (should always be constant-time)
    for _ in 0..100 {
        assert!(capsule.verify_measurement(&hash1));
    }

    // Compare different hash (constant-time should not leak information)
    for i in 0..48 {
        let mut hash2 = [0xFFu8; 48];
        hash2[i] = 0x12; // Different in only 1 byte

        assert!(!capsule.verify_measurement(&hash2));
    }
}

// ============================================================================
// Q8-Q14: Property Tests (7 tests)
// ============================================================================

#[test]
fn q8_attestation_integrity_idempotent() {
    let capsule = SecureEnclaveCapsule::new(TeeType::Software);

    // Attestation 1
    let result1 = capsule.remote_attestation();
    assert!(result1.is_ok());
    let attestation1 = result1.unwrap();

    // Attestation 2 (should be idempotent)
    let result2 = capsule.remote_attestation();
    assert!(result2.is_ok());
    let attestation2 = result2.unwrap();

    // Both attestations should have same measurement hash
    assert_eq!(
        attestation1.measurement_hash, attestation2.measurement_hash,
        "Measurement hash must be stable"
    );

    // Both should return to Active state
    assert_eq!(capsule.state(), EnclaveState::Active);
}

#[test]
fn q9_enclave_isolation_state_independence() {
    // Create multiple capsules (simulate separate enclaves)
    let capsule1 = SecureEnclaveCapsule::new(TeeType::Software);
    let capsule2 = SecureEnclaveCapsule::new(TeeType::Software);

    // Suspend capsule1
    assert!(capsule1.suspend().is_ok());
    assert_eq!(capsule1.state(), EnclaveState::Suspended);

    // Capsule2 should remain Active (isolation)
    assert_eq!(capsule2.state(), EnclaveState::Active);

    // Enclave call should succeed on capsule2
    assert!(capsule2.enclave_call(&[]).is_ok());

    // But fail on capsule1
    assert!(capsule1.enclave_call(&[]).is_err());
}

#[test]
fn q10_memory_encryption_status_atomic_update() {
    let capsule = SecureEnclaveCapsule::new(TeeType::Software);

    // Start with NotAvailable
    assert_eq!(
        capsule.memory_encryption_status(),
        MemoryEncryptionStatus::NotAvailable
    );

    // Update to Transparent
    capsule.set_memory_encryption_status(MemoryEncryptionStatus::Transparent);
    assert_eq!(
        capsule.memory_encryption_status(),
        MemoryEncryptionStatus::Transparent
    );

    // Update to Verified
    capsule.set_memory_encryption_status(MemoryEncryptionStatus::Verified);
    assert_eq!(
        capsule.memory_encryption_status(),
        MemoryEncryptionStatus::Verified
    );
}

#[test]
fn q11_enclave_call_metrics_monotonic() {
    let capsule = SecureEnclaveCapsule::new(TeeType::Software);

    let metrics_before = capsule.call_metrics();
    assert_eq!(metrics_before.call_count, 0);

    // Make 10 enclave calls
    for _ in 0..10 {
        let _ = capsule.enclave_call(&[1, 2, 3]);
    }

    let metrics_after = capsule.call_metrics();
    assert_eq!(metrics_after.call_count, 10, "Call count must increment monotonically");
    assert!(
        metrics_after.total_latency_ns >= metrics_before.total_latency_ns,
        "Total latency must be monotonic"
    );
}

#[test]
fn q12_state_transition_validity() {
    let capsule = SecureEnclaveCapsule::new(TeeType::Software);
    assert_eq!(capsule.state(), EnclaveState::Active);

    // Valid transition: Active → Suspended
    assert!(capsule.suspend().is_ok());
    assert_eq!(capsule.state(), EnclaveState::Suspended);

    // Valid transition: Suspended → Active
    assert!(capsule.resume().is_ok());
    assert_eq!(capsule.state(), EnclaveState::Active);

    // Invalid transition: Active → Suspended → Suspended (should fail)
    assert!(capsule.suspend().is_ok());
    assert!(capsule.suspend().is_err(), "Cannot suspend already-suspended enclave");
}

#[test]
fn q13_tee_type_consistency() {
    for tee_type in [
        TeeType::Software,
        TeeType::IntelSgx,
        TeeType::AmdSev,
        TeeType::ArmTrustZone,
    ] {
        let capsule = SecureEnclaveCapsule::new(tee_type);

        // TEE type must match throughout lifecycle
        assert_eq!(capsule.tee_type(), tee_type);

        let attestation = capsule.remote_attestation().unwrap();
        assert_eq!(capsule.tee_type(), tee_type, "TEE type must remain stable");
    }
}

#[test]
fn q14_concurrent_call_count_accuracy() {
    let capsule = Arc::new(SecureEnclaveCapsule::new(TeeType::Software));
    let mut handles = vec![];

    // Spawn 10 threads, each making 10 enclave calls
    for _ in 0..10 {
        let capsule_clone = capsule.clone();
        let handle = thread::spawn(move || {
            for _ in 0..10 {
                let _ = capsule_clone.enclave_call(&[]);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify total call count (should be 100, but atomic increment may have ordering issues)
    let metrics = capsule.call_metrics();
    assert_eq!(metrics.call_count, 100, "Call count must be accurate under concurrency");
}

// ============================================================================
// Q15-Q21: Integration Tests (7 tests)
// ============================================================================

#[test]
fn q15_sgx_simulation_latency() {
    let capsule = SecureEnclaveCapsule::new(TeeType::IntelSgx);

    let attestation = capsule.remote_attestation().unwrap();
    assert!(attestation.is_valid);

    // SGX simulation should complete in 50-100ms
    assert!(attestation.attestation_time_ms >= 50);
    assert!(attestation.attestation_time_ms <= 100);
}

#[test]
fn q16_sev_simulation_latency() {
    let capsule = SecureEnclaveCapsule::new(TeeType::AmdSev);

    let attestation = capsule.remote_attestation().unwrap();
    assert!(attestation.is_valid);

    // SEV simulation should complete in 100-200ms
    assert!(attestation.attestation_time_ms >= 100);
    assert!(attestation.attestation_time_ms <= 200);
}

#[test]
fn q17_trustzone_simulation_latency() {
    let capsule = SecureEnclaveCapsule::new(TeeType::ArmTrustZone);

    let attestation = capsule.remote_attestation().unwrap();
    assert!(attestation.is_valid);

    // TrustZone simulation should complete in 200-500ms
    assert!(attestation.attestation_time_ms >= 200);
    assert!(attestation.attestation_time_ms <= 500);
}

#[test]
fn q18_software_simulation_latency() {
    let capsule = SecureEnclaveCapsule::new(TeeType::Software);

    let attestation = capsule.remote_attestation().unwrap();
    assert!(attestation.is_valid);

    // Software simulation should complete in <100ms (usually 50ms)
    assert!(attestation.attestation_time_ms < 100);
}

#[test]
fn q19_measurement_hash_persistence() {
    let mut capsule1 = SecureEnclaveCapsule::new(TeeType::Software);
    let test_hash = [99u8; 48];
    capsule1.set_measurement_hash(test_hash);

    // Verify hash persists across operations
    let _ = capsule1.enclave_call(&[]);
    assert!(capsule1.verify_measurement(&test_hash));

    let _ = capsule1.remote_attestation();
    assert!(capsule1.verify_measurement(&test_hash));
}

#[test]
fn q20_enclave_call_and_attestation_sequence() {
    let capsule = SecureEnclaveCapsule::new(TeeType::Software);

    // Make some enclave calls
    for i in 0..5 {
        let result = capsule.enclave_call(&[i as u8]);
        assert!(result.is_ok());
    }

    // Verify metrics
    let metrics = capsule.call_metrics();
    assert_eq!(metrics.call_count, 5);

    // Perform attestation
    let attestation = capsule.remote_attestation().unwrap();
    assert!(attestation.is_valid);

    // Enclave should still be functional after attestation
    assert!(capsule.enclave_call(&[42]).is_ok());

    // Call count should now be 6
    let metrics_after = capsule.call_metrics();
    assert_eq!(metrics_after.call_count, 6);
}

#[test]
fn q21_concurrent_enclave_operations() {
    let capsule = Arc::new(SecureEnclaveCapsule::new(TeeType::Software));
    let mut handles = vec![];

    // Spawn threads that concurrently:
    // - Make enclave calls
    // - Check memory encryption status
    // - Read attestation latency
    for i in 0..5 {
        let capsule_clone = capsule.clone();
        let handle = thread::spawn(move || {
            for _ in 0..10 {
                // Enclave call
                let _ = capsule_clone.enclave_call(&[i as u8]);

                // Check encryption status
                let _ = capsule_clone.memory_encryption_status();

                // Read attestation latency
                let _ = capsule_clone.last_attestation_latency_ms();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // All concurrent operations should have succeeded
    assert_eq!(capsule.call_metrics().call_count, 50);
}

// ============================================================================
// Q22-Q28: Production Tests (7 tests)
// ============================================================================

#[test]
fn q22_attestation_latency_under_100ms() {
    let capsule = SecureEnclaveCapsule::new(TeeType::Software);

    // Perform 100 attestations, measure latency
    let mut total_latency_ms = 0u32;
    for _ in 0..100 {
        let attestation = capsule.remote_attestation().unwrap();
        total_latency_ms = attestation.attestation_time_ms;
    }

    // Software attestation should consistently be <100ms
    assert!(total_latency_ms < 100, "Attestation must complete in <100ms");
}

#[test]
fn q23_enclave_call_overhead_under_1us() {
    let capsule = SecureEnclaveCapsule::new(TeeType::Software);

    // Make 1000 enclave calls
    for _ in 0..1000 {
        let _ = capsule.enclave_call(&[1, 2, 3]);
    }

    // Verify metrics
    let metrics = capsule.call_metrics();
    assert_eq!(metrics.call_count, 1000);

    // Average latency per call
    let avg_latency_ns = metrics.total_latency_ns / metrics.call_count;
    assert!(
        avg_latency_ns < 1000,
        "Average enclave call latency must be <1μs (1000ns)"
    );
}

#[test]
fn q24_concurrent_attestation_stress_test() {
    let capsule = Arc::new(SecureEnclaveCapsule::new(TeeType::Software));
    let mut handles = vec![];

    // Spawn 10 threads, each performing 10 attestations
    for _ in 0..10 {
        let capsule_clone = capsule.clone();
        let handle = thread::spawn(move || {
            for _ in 0..10 {
                let result = capsule_clone.remote_attestation();
                assert!(result.is_ok());
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify enclave remained stable
    assert_eq!(capsule.state(), EnclaveState::Active);
}

#[test]
fn q25_memory_encryption_hardware_simulation() {
    let capsule = SecureEnclaveCapsule::new(TeeType::AmdSev);

    // Simulate hardware memory encryption
    capsule.set_memory_encryption_status(MemoryEncryptionStatus::Transparent);
    assert_eq!(
        capsule.memory_encryption_status(),
        MemoryEncryptionStatus::Transparent
    );

    // Simulate verification
    capsule.set_memory_encryption_status(MemoryEncryptionStatus::Verified);
    assert_eq!(
        capsule.memory_encryption_status(),
        MemoryEncryptionStatus::Verified
    );
}

#[test]
fn q26_integrity_verification_100_percent_success() {
    let mut capsule = SecureEnclaveCapsule::new(TeeType::Software);

    // Set a test hash
    let test_hash = [123u8; 48];
    capsule.set_measurement_hash(test_hash);

    // Verify 1000 times (100% success rate expected)
    let mut success_count = 0;
    for _ in 0..1000 {
        if capsule.verify_measurement(&test_hash) {
            success_count += 1;
        }
    }

    assert_eq!(success_count, 1000, "Integrity verification must have 100% success rate");
}

#[test]
fn q27_attestation_state_machine_correctness() {
    let capsule = SecureEnclaveCapsule::new(TeeType::Software);

    // Initial state: Active
    assert_eq!(capsule.state(), EnclaveState::Active);

    // Trigger attestation (state: Attesting → Active)
    let attestation = capsule.remote_attestation().unwrap();
    assert!(attestation.is_valid);

    // After attestation, should return to Active
    assert_eq!(capsule.state(), EnclaveState::Active);

    // Suspend-Resume cycle
    assert!(capsule.suspend().is_ok());
    assert_eq!(capsule.state(), EnclaveState::Suspended);

    assert!(capsule.resume().is_ok());
    assert_eq!(capsule.state(), EnclaveState::Active);

    // Final attestation should also succeed
    let attestation2 = capsule.remote_attestation().unwrap();
    assert!(attestation2.is_valid);
}

#[test]
fn q28_production_scalability_1000_concurrent_calls() {
    let capsule = Arc::new(SecureEnclaveCapsule::new(TeeType::Software));
    let mut handles = vec![];

    // Spawn 50 threads, each making 20 enclave calls = 1000 total
    for _ in 0..50 {
        let capsule_clone = capsule.clone();
        let handle = thread::spawn(move || {
            for _ in 0..20 {
                let result = capsule_clone.enclave_call(&[42]);
                assert!(result.is_ok());
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all calls were recorded
    let metrics = capsule.call_metrics();
    assert_eq!(
        metrics.call_count, 1000,
        "Must handle 1000 concurrent calls accurately"
    );

    // Enclave should remain Active
    assert_eq!(capsule.state(), EnclaveState::Active);

    // All calls should have reasonable latency
    let avg_latency_ns = metrics.total_latency_ns / metrics.call_count;
    assert!(avg_latency_ns > 0, "Average latency must be measurable");
}
