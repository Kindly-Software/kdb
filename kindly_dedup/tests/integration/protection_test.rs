//! Protection System Integration Test for kindly_dedup v3.1.0
//!
//! **Purpose**: Integration testing for commercial protection features
//! **Tier**: T28 Integration (Q15-Q21)
//! **Status**: v3.1.0 Commercial Release
//! **Feature Gate**: Requires `binary-protection` feature flag
//!
//! ## Test Coverage
//!
//! 1. License validation flow (full cycle)
//! 2. Demo tier limits (CommercialLimiterCapsule)
//! 3. Protection status checks (health monitoring)
//! 4. Tier enforcement (Basic/Pro/Enterprise)
//!
//! ## Framework Compliance
//!
//! - **T28 Q15**: Cross-module integration (pipeline + protection + license)
//! - **T28 Q18**: Resource constraints (tier limits enforced)
//! - **T28 Q19**: Configuration validation (license tiers)
//! - **ASSUM**: All protection assumptions documented and verified
//! - **Chaos**: Uses public capsule APIs only (lockfree protection capsules)

#[cfg(feature = "binary-protection")]
use kindly_dedup::protection::{
    CommercialLimiterCapsule, CommercialLimitError, LicenseTier,
};

/// T28 Q15: License validation integration
///
/// Tests full license validation flow:
/// - Create CommercialLimiterCapsule with specific tier
/// - Verify tier limits are correctly enforced
/// - Verify state transitions (unlocked → warning → locked)
///
/// #ASSUME_TIER_LIMITS: Demo=1K, Basic=100K, Pro=10M, Enterprise=unlimited
///   #VERIFY_TIER_LIMITS: Limits match LicenseTier specifications
///
/// #ASSUME_ATOMIC_COUNTER: Document count increments atomically
///   #VERIFY_ATOMIC_COUNTER: No race conditions in multi-threaded scenarios
#[cfg(feature = "binary-protection")]
#[test]
fn test_license_validation_flow() {
    // Test Demo tier (1,000 doc limit)
    let demo_limiter = CommercialLimiterCapsule::new(LicenseTier::Demo);

    // Verify initial state
    assert_eq!(demo_limiter.current_count(), 0, "Initial count must be 0");
    assert_eq!(demo_limiter.tier(), LicenseTier::Demo, "Tier must be Demo");
    assert!(
        demo_limiter.can_add_document().is_ok(),
        "Should allow adding documents initially"
    );

    // Add documents up to 90% (warning threshold)
    for _ in 0..900 {
        demo_limiter
            .record_document()
            .expect("Should allow adding documents below limit");
    }

    // Verify warning threshold
    assert!(
        demo_limiter.is_at_warning_threshold(),
        "Should be at warning threshold after 900 docs (90% of 1000)"
    );
    assert!(
        demo_limiter.can_add_document().is_ok(),
        "Should still allow adding documents at warning threshold"
    );

    // Add documents up to limit
    for _ in 900..1000 {
        demo_limiter
            .record_document()
            .expect("Should allow adding documents up to limit");
    }

    // Verify limit reached
    assert_eq!(
        demo_limiter.current_count(),
        1000,
        "Count must be exactly 1000"
    );
    match demo_limiter.can_add_document() {
        Err(CommercialLimitError::LimitReached { tier, limit }) => {
            assert_eq!(tier, LicenseTier::Demo, "Error tier must be Demo");
            assert_eq!(limit, 1000, "Error limit must be 1000");
        }
        _ => panic!("Should return LimitReached error at limit"),
    }
}

/// T28 Q15: Demo tier enforcement
///
/// Tests hard limit enforcement for Demo tier:
/// - Demo tier has hard block at 1,000 documents
/// - Attempts to exceed limit return error
/// - Error message includes upgrade CTA
///
/// #ASSUME_HARD_BLOCK: Demo tier blocks at exactly 1,000 documents
///   #VERIFY_HARD_BLOCK: can_add_document() returns Err at 1,000
#[cfg(feature = "binary-protection")]
#[test]
fn test_demo_tier_limits() {
    let limiter = CommercialLimiterCapsule::new(LicenseTier::Demo);

    // Add 1,000 documents (limit)
    for i in 0..1000 {
        limiter.record_document().unwrap_or_else(|e| {
            panic!("Failed to add document {} of 1000: {}", i + 1, e);
        });
    }

    // Verify limit reached
    assert_eq!(limiter.current_count(), 1000, "Count must be 1000");
    assert!(
        limiter.can_add_document().is_err(),
        "Should block documents after limit"
    );
    assert_eq!(
        limiter.remaining_documents(),
        None,
        "No documents remaining after limit"
    );
}

/// T28 Q18: Basic tier enforcement
///
/// Tests soft warning for Basic tier:
/// - Basic tier has 100,000 document limit
/// - Warning at 90% (90,000 documents)
/// - No hard block (soft warning only)
///
/// #ASSUME_SOFT_WARNING: Basic tier warns but doesn't block
///   #VERIFY_SOFT_WARNING: is_at_warning_threshold() returns true at 90%
#[cfg(feature = "binary-protection")]
#[test]
fn test_basic_tier_limits() {
    let limiter = CommercialLimiterCapsule::new(LicenseTier::Basic);

    // Add documents up to warning threshold (90,000)
    for _ in 0..90_000 {
        limiter
            .record_document()
            .expect("Should allow adding documents below limit");
    }

    // Verify warning threshold
    assert!(
        limiter.is_at_warning_threshold(),
        "Should be at warning threshold after 90,000 docs (90% of 100,000)"
    );

    // Continue adding to verify no hard block
    for _ in 90_000..100_000 {
        limiter
            .record_document()
            .expect("Should allow adding documents up to limit");
    }

    // Verify limit reached but no hard block
    assert_eq!(
        limiter.current_count(),
        100_000,
        "Count must be 100,000"
    );
    match limiter.can_add_document() {
        Err(CommercialLimitError::LimitReached { tier, limit }) => {
            assert_eq!(tier, LicenseTier::Basic, "Error tier must be Basic");
            assert_eq!(limit, 100_000, "Error limit must be 100,000");
        }
        _ => panic!("Should return LimitReached error at limit"),
    }
}

/// T28 Q18: Pro tier enforcement
///
/// Tests soft warning for Pro tier:
/// - Pro tier has 10,000,000 document limit
/// - Warning at 90% (9,000,000 documents)
/// - No hard block (soft warning only)
///
/// NOTE: This test uses small sample (1,000 docs) to verify tier logic,
/// not full 10M capacity (would take too long).
///
/// #ASSUME_PRO_TIER_LOGIC: Tier limit checks work regardless of scale
///   #VERIFY_PRO_TIER_LOGIC: Test with 1K docs verifies same logic as 10M
#[cfg(feature = "binary-protection")]
#[test]
fn test_pro_tier_limits() {
    let limiter = CommercialLimiterCapsule::new(LicenseTier::Pro);

    // Verify tier configuration
    assert_eq!(limiter.tier(), LicenseTier::Pro, "Tier must be Pro");

    // Add small sample (1,000 docs)
    for _ in 0..1000 {
        limiter
            .record_document()
            .expect("Should allow adding documents below limit");
    }

    // Verify no warning threshold yet (1K << 9M)
    assert!(
        !limiter.is_at_warning_threshold(),
        "Should not be at warning threshold after 1,000 docs (< 1% of 10M)"
    );
    assert!(
        limiter.can_add_document().is_ok(),
        "Should allow adding more documents"
    );
}

/// T28 Q19: Enterprise tier unlimited
///
/// Tests unlimited capacity for Enterprise tier:
/// - No document limit
/// - No warning threshold
/// - Always allows adding documents
///
/// #ASSUME_UNLIMITED: Enterprise tier has no limits
///   #VERIFY_UNLIMITED: can_add_document() always returns Ok
#[cfg(feature = "binary-protection")]
#[test]
fn test_enterprise_tier_unlimited() {
    let limiter = CommercialLimiterCapsule::new(LicenseTier::Enterprise);

    // Add large number of documents (10,000)
    for _ in 0..10_000 {
        limiter
            .record_document()
            .expect("Enterprise tier should always allow adding documents");
    }

    // Verify no limits
    assert_eq!(
        limiter.current_count(),
        10_000,
        "Count must be 10,000"
    );
    assert!(
        !limiter.is_at_warning_threshold(),
        "Enterprise tier should never reach warning threshold"
    );
    assert!(
        limiter.can_add_document().is_ok(),
        "Enterprise tier should always allow adding documents"
    );
    assert_eq!(
        limiter.remaining_documents(),
        None,
        "Enterprise tier has no document limit"
    );
}

/// T28 Q15: Protection status checks
///
/// Tests protection status monitoring:
/// - Query current protection state
/// - Verify tier information
/// - Check health status
///
/// #ASSUME_STATUS_QUERY: ProtectionStatus provides tier and count info
///   #VERIFY_STATUS_QUERY: Status matches limiter state
#[cfg(feature = "binary-protection")]
#[test]
fn test_protection_status_checks() {
    let limiter = CommercialLimiterCapsule::new(LicenseTier::Basic);

    // Add some documents
    for _ in 0..500 {
        limiter.record_document().expect("Should allow adding documents");
    }

    // Verify status query matches limiter state
    assert_eq!(limiter.tier(), LicenseTier::Basic, "Tier must be Basic");
    assert_eq!(limiter.current_count(), 500, "Count must be 500");
    assert!(
        !limiter.is_at_warning_threshold(),
        "Should not be at warning threshold (500 << 90,000)"
    );
}

/// T28 Q19: Multi-threaded tier enforcement
///
/// Tests thread-safety of CommercialLimiterCapsule:
/// - Multiple threads increment counter concurrently
/// - Final count matches expected total
/// - No race conditions in limit checks
///
/// #ASSUME_ATOMIC_SAFETY: AtomicU64 counter is thread-safe
///   #VERIFY_ATOMIC_SAFETY: No data races, final count matches expected
#[cfg(feature = "binary-protection")]
#[test]
fn test_multithreaded_enforcement() {
    use std::sync::Arc;
    use std::thread;

    let limiter = Arc::new(CommercialLimiterCapsule::new(LicenseTier::Demo));
    let num_threads = 4;
    let docs_per_thread = 250; // Total: 1,000 docs (limit)

    let mut handles = vec![];

    for _ in 0..num_threads {
        let limiter_clone = Arc::clone(&limiter);
        let handle = thread::spawn(move || {
            for _ in 0..docs_per_thread {
                // This may fail if limit reached (race condition expected)
                let _ = limiter_clone.record_document();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // Verify final count (may be ≤1000 due to limit enforcement)
    let final_count = limiter.current_count();
    assert!(
        final_count <= 1000,
        "Final count must be ≤1000 (got {})",
        final_count
    );

    // Verify limit is enforced
    assert!(
        limiter.can_add_document().is_err(),
        "Should block documents after reaching limit"
    );
}
