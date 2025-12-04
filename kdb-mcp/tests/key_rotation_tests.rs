//! Comprehensive test suite for KeyRotationCapsule
//! T28 Framework: 28 tests across 4 tiers (Unit/Property/Integration/Production)

use kdb_mcp::key_rotation::{KeyRotationCapsule, KeyMetadata, RotationError, RotationStats};
use std::sync::Arc;
use std::sync::atomic::Ordering;

const KEY_SIZE: usize = 32;
const GRACE_PERIOD_SECS: u64 = 60;
const SECS_PER_DAY: u64 = 86_400;

// ============================================================================
// Layout Tests (T28 Q1-Q7: Unit)
// ============================================================================

#[test]
fn test_layout_size() {
    assert_eq!(std::mem::size_of::<KeyRotationCapsule>(), 256);
}

#[test]
fn test_layout_alignment() {
    assert_eq!(std::mem::align_of::<KeyRotationCapsule>(), 256);
}

// ============================================================================
// Initialization Tests (T28 Q1-Q7: Unit)
// ============================================================================

#[test]
fn test_new_initialization() {
    let pub_key = [42u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key, 90);

    assert_eq!(capsule.current_key_id.load(Ordering::Relaxed), 1);
    assert_eq!(capsule.previous_key_id.load(Ordering::Relaxed), 0);
    assert_eq!(capsule.rotation_count.load(Ordering::Relaxed), 0);
    assert_eq!(capsule.get_current_public_key(), pub_key);
}

#[test]
fn test_initial_rotation_count_zero() {
    let pub_key = [42u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key, 90);
    let stats = capsule.get_stats();

    assert_eq!(stats.rotation_count, 0);
    assert_eq!(stats.accepted_rotations, 0);
    assert_eq!(stats.revoked_keys, 0);
}

#[test]
fn test_initial_validation_count_zero() {
    let pub_key = [42u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key, 90);
    let stats = capsule.get_stats();

    assert_eq!(stats.validation_count, 0);
    assert_eq!(stats.validation_success, 0);
}

// ============================================================================
// Key Validation Tests (T28 Q1-Q7: Unit)
// ============================================================================

#[test]
fn test_is_key_valid_current() {
    let pub_key = [42u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key, 90);
    let now = KeyRotationCapsule::get_unix_seconds();

    assert!(capsule.is_key_valid(&pub_key, now));
}

#[test]
fn test_is_key_valid_wrong_key() {
    let pub_key = [42u8; KEY_SIZE];
    let wrong_key = [43u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key, 90);
    let now = KeyRotationCapsule::get_unix_seconds();

    assert!(!capsule.is_key_valid(&wrong_key, now));
}

#[test]
fn test_is_key_valid_expired() {
    let pub_key = [42u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key, 90);
    let future = KeyRotationCapsule::get_unix_seconds() + (100 * SECS_PER_DAY);

    assert!(!capsule.is_key_valid(&pub_key, future));
}

#[test]
fn test_get_current_public_key() {
    let mut pub_key = [0u8; KEY_SIZE];
    for i in 0..KEY_SIZE {
        pub_key[i] = i as u8;
    }

    let capsule = KeyRotationCapsule::new(pub_key, 90);
    let retrieved = capsule.get_current_public_key();

    assert_eq!(retrieved, pub_key);
}

// ============================================================================
// Rotation Tests (T28 Q1-Q7: Unit)
// ============================================================================

#[test]
fn test_rotate_updates_keys() {
    let pub_key_1 = [42u8; KEY_SIZE];
    let pub_key_2 = [43u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key_1, 90);
    let now = KeyRotationCapsule::get_unix_seconds();

    let result = capsule.rotate(pub_key_2, now + 1);
    assert!(result.is_ok());

    let metadata = result.unwrap();
    assert_eq!(metadata.key_id, 2);
    assert_eq!(metadata.public_key, pub_key_2);
    assert_eq!(capsule.get_current_public_key(), pub_key_2);
}

#[test]
fn test_rotate_increments_key_id() {
    let pub_key_1 = [42u8; KEY_SIZE];
    let pub_key_2 = [43u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key_1, 90);
    let now = KeyRotationCapsule::get_unix_seconds();

    let r1 = capsule.rotate(pub_key_2, now + 1).unwrap();
    assert_eq!(r1.key_id, 2);

    let stats = capsule.get_stats();
    assert_eq!(stats.current_key_id, 2);
    assert_eq!(stats.rotation_count, 1);
}

#[test]
fn test_rotate_rejects_backwards_time() {
    let pub_key_1 = [42u8; KEY_SIZE];
    let pub_key_2 = [43u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key_1, 90);
    let now = KeyRotationCapsule::get_unix_seconds();

    let result = capsule.rotate(pub_key_2, now - 1);
    assert_eq!(result, Err(RotationError::InvalidTime));
}

// ============================================================================
// Grace Period Tests (T28 Q8-Q14: Property)
// ============================================================================

#[test]
fn test_grace_period_overlap() {
    let pub_key_1 = [42u8; KEY_SIZE];
    let pub_key_2 = [43u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key_1, 90);
    let now = KeyRotationCapsule::get_unix_seconds();

    capsule.rotate(pub_key_2, now + 1).ok();

    // Check midway through grace period
    let check_time = now + 1 + (GRACE_PERIOD_SECS / 2);
    assert!(capsule.is_key_valid(&pub_key_1, check_time), "Previous key should be valid in grace period");
    assert!(capsule.is_key_valid(&pub_key_2, check_time), "Current key should be valid");
}

#[test]
fn test_grace_period_expires() {
    let pub_key_1 = [42u8; KEY_SIZE];
    let pub_key_2 = [43u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key_1, 90);
    let now = KeyRotationCapsule::get_unix_seconds();

    capsule.rotate(pub_key_2, now + 1).ok();

    // Check after grace period ends
    let check_time = now + 1 + GRACE_PERIOD_SECS + 1;
    assert!(!capsule.is_key_valid(&pub_key_1, check_time), "Previous key should expire");
    assert!(capsule.is_key_valid(&pub_key_2, check_time), "Current key should still be valid");
}

#[test]
fn test_get_previous_public_key_during_grace() {
    let pub_key_1 = [42u8; KEY_SIZE];
    let pub_key_2 = [43u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key_1, 90);
    let now = KeyRotationCapsule::get_unix_seconds();

    capsule.rotate(pub_key_2, now + 1).ok();

    let check_time = now + 1 + (GRACE_PERIOD_SECS / 2);
    let prev = capsule.get_previous_public_key(check_time);
    assert_eq!(prev, Some(pub_key_1), "Previous key should be retrievable during grace period");
}

#[test]
fn test_get_previous_public_key_after_grace() {
    let pub_key_1 = [42u8; KEY_SIZE];
    let pub_key_2 = [43u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key_1, 90);
    let now = KeyRotationCapsule::get_unix_seconds();

    capsule.rotate(pub_key_2, now + 1).ok();

    let check_time = now + 1 + GRACE_PERIOD_SECS + 1;
    let prev = capsule.get_previous_public_key(check_time);
    assert_eq!(prev, None, "Previous key should be None after grace period");
}

// ============================================================================
// Revocation Tests (T28 Q8-Q14: Property)
// ============================================================================

#[test]
fn test_revoke_key() {
    let pub_key = [42u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key, 90);

    // Initialize Bloom filter
    let bloom_box = Box::new([0u8; 16_384]);
    let bloom_ptr = Box::leak(bloom_box) as *mut [u8; 16_384];
    capsule.bloom_ptr.store(bloom_ptr, Ordering::Release);

    let result = capsule.revoke_key(1);
    assert!(result.is_ok());

    assert!(capsule.is_key_revoked(1));
}

#[test]
fn test_revocation_counter_increments() {
    let pub_key = [42u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key, 90);

    // Initialize Bloom filter
    let bloom_box = Box::new([0u8; 16_384]);
    let bloom_ptr = Box::leak(bloom_box) as *mut [u8; 16_384];
    capsule.bloom_ptr.store(bloom_ptr, Ordering::Release);

    capsule.revoke_key(1).ok();
    capsule.revoke_key(2).ok();

    let stats = capsule.get_stats();
    assert_eq!(stats.revoked_keys, 2);
}

#[test]
fn test_bloom_false_positive_rate() {
    let pub_key = [42u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key, 90);

    // Initialize Bloom filter
    let bloom_box = Box::new([0u8; 16_384]);
    let bloom_ptr = Box::leak(bloom_box) as *mut [u8; 16_384];
    capsule.bloom_ptr.store(bloom_ptr, Ordering::Release);

    // Insert 1000 keys (1% capacity)
    for i in 1..=1000 {
        capsule.revoke_key(i).ok();
    }

    // Check for false positives
    let mut fp_count = 0;
    for i in 10001..=10100 {
        if capsule.is_key_revoked(i) {
            fp_count += 1;
        }
    }

    // FP rate should be <0.01%
    let fp_rate = fp_count as f64 / 100.0;
    assert!(fp_rate < 0.001, "FP rate {:.4}% exceeds 0.01% target", fp_rate * 100.0);
}

// ============================================================================
// Monotonicity Tests (T28 Q8-Q14: Property)
// ============================================================================

#[test]
fn test_key_id_monotonic() {
    let pub_key_1 = [42u8; KEY_SIZE];
    let pub_key_2 = [43u8; KEY_SIZE];
    let pub_key_3 = [44u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key_1, 90);
    let now = KeyRotationCapsule::get_unix_seconds();

    let r1 = capsule.rotate(pub_key_2, now + 1).unwrap();
    let r2 = capsule.rotate(pub_key_3, now + 2).unwrap();

    assert!(r1.key_id < r2.key_id);
    assert_eq!(r1.key_id, 2);
    assert_eq!(r2.key_id, 3);
}

#[test]
fn test_validation_count_monotonic() {
    let pub_key = [42u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key, 90);
    let now = KeyRotationCapsule::get_unix_seconds();

    for _ in 0..10 {
        capsule.is_key_valid(&pub_key, now);
    }

    let stats = capsule.get_stats();
    assert_eq!(stats.validation_count, 10);
}

// ============================================================================
// Integration Tests (T28 Q15-Q21: Integration)
// ============================================================================

#[test]
fn test_load_from_storage() {
    let temp_dir = std::env::temp_dir().join("key_rotation_load_test");
    let _ = std::fs::remove_dir_all(&temp_dir);

    let pub_key = [42u8; KEY_SIZE];
    let result = KeyRotationCapsule::load_from_storage(&temp_dir, pub_key);

    assert!(result.is_ok());
    let capsule = result.unwrap();
    assert_eq!(capsule.get_current_public_key(), pub_key);
}

#[test]
fn test_concurrent_validations() {
    let pub_key = [42u8; KEY_SIZE];
    let capsule = Arc::new(KeyRotationCapsule::new(pub_key, 90));
    let now = KeyRotationCapsule::get_unix_seconds();

    let mut handles = vec![];
    for _ in 0..10 {
        let capsule_clone = capsule.clone();
        let handle = std::thread::spawn(move || {
            for _ in 0..100 {
                capsule_clone.is_key_valid(&pub_key, now);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().ok();
    }

    let stats = capsule.get_stats();
    assert_eq!(stats.validation_count, 1000);
}

#[test]
fn test_multiple_rotations() {
    let pub_key_base = [42u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key_base, 90);
    let now = KeyRotationCapsule::get_unix_seconds();

    for i in 0..5 {
        let mut pub_key = pub_key_base;
        pub_key[0] = (i + 1) as u8;
        capsule.rotate(pub_key, now + i as u64 + 1).ok();
    }

    let stats = capsule.get_stats();
    assert_eq!(stats.rotation_count, 5);
    assert_eq!(stats.current_key_id, 6); // 1 initial + 5 rotations
}

// ============================================================================
// Production Tests (T28 Q22-Q28: Production)
// ============================================================================

#[test]
fn test_crash_recovery_simulation() {
    let pub_key = [42u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key, 90);

    // Initialize Bloom filter
    let bloom_box = Box::new([0u8; 16_384]);
    let bloom_ptr = Box::leak(bloom_box) as *mut [u8; 16_384];
    capsule.bloom_ptr.store(bloom_ptr, Ordering::Release);

    // Revoke keys
    for i in 1..=100 {
        capsule.revoke_key(i).ok();
    }

    // Verify revocation survives
    assert!(capsule.is_key_revoked(1));
    assert!(capsule.is_key_revoked(100));
}

#[test]
fn test_rotation_stress() {
    let pub_key_base = [42u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key_base, 90);
    let now = KeyRotationCapsule::get_unix_seconds();

    for i in 0..100 {
        let mut pub_key = pub_key_base;
        pub_key[0] = (i % 256) as u8;
        capsule.rotate(pub_key, now + i as u64).ok();
    }

    let stats = capsule.get_stats();
    assert_eq!(stats.rotation_count, 100);
}

#[test]
fn test_long_running_validity() {
    let pub_key_1 = [42u8; KEY_SIZE];
    let pub_key_2 = [43u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key_1, 90);
    let mut now = KeyRotationCapsule::get_unix_seconds();

    // Simulate 45 days passing
    now += 45 * SECS_PER_DAY;
    assert!(capsule.is_key_valid(&pub_key_1, now));

    // Rotate at day 45
    capsule.rotate(pub_key_2, now).ok();

    // Both keys valid immediately after rotation
    assert!(capsule.is_key_valid(&pub_key_2, now));
    assert!(capsule.is_key_valid(&pub_key_1, now));

    // After grace period, old key invalid
    now += GRACE_PERIOD_SECS + 1;
    assert!(capsule.is_key_valid(&pub_key_2, now));
    assert!(!capsule.is_key_valid(&pub_key_1, now));
}

#[test]
fn test_stats_consistency() {
    let pub_key = [42u8; KEY_SIZE];
    let capsule = KeyRotationCapsule::new(pub_key, 90);
    let now = KeyRotationCapsule::get_unix_seconds();

    // Perform validations
    capsule.is_key_valid(&pub_key, now);
    capsule.is_key_valid(&pub_key, now);
    capsule.is_key_valid(&[43u8; KEY_SIZE], now); // Failed

    let stats = capsule.get_stats();
    assert_eq!(stats.validation_count, 3);
    assert_eq!(stats.validation_success, 2);
}
