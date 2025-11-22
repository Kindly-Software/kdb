//! Base AuditableCapsule Trait - Object-Safe Foundation
//!
//! Provides universal hash chain integrity for all state-modifying capsules.
//!
//! # Architecture
//!
//! AuditableCapsule is the **Tier 0 meta-tier** that sits below all other tiers:
//!
//! ```text
//! Tier 0: AuditableCapsule (hash chain foundation)
//!   ├── Tier 1: AtomicCapsule (lockfree coordination)
//!   ├── Tier 2: SimdCapsule (vectorized computation)
//!   ├── Tier 3: FixedPointCapsule (deterministic math)
//!   ├── Tier 4: BatchCapsule (throughput processing)
//!   ├── Tier 5: StreamingCapsule (incremental computation)
//!   └── Tier 6: MixedCapsule (compound operations)
//! ```
//!
//! # Object Safety (NEW)
//!
//! This trait is **object-safe** to support heterogeneous capsule collections:
//!
//! ```rust,ignore
//! let capsules: Vec<Box<dyn AuditableCapsule>> = vec![
//!     Box::new(circuit_breaker_capsule),
//!     Box::new(pnl_capsule),
//!     Box::new(dashboard_state_capsule),
//! ];
//!
//! // Polymorphic audit trail
//! for capsule in &capsules {
//!     audit_trail.record("update", capsule.as_ref(), None);
//! }
//! ```
//!
//! # Q34 Compliance
//!
//! All state-modifying capsules MUST implement AuditableCapsule:
//! - **Hash field** (AtomicU64) - current state hash
//! - **PrevHash field** (AtomicU64) - chain link
//! - **Generation counter** (AtomicU64) - TOCTOU prevention
//!
//! # Performance Targets (B32 Framework)
//!
//! - Hash compute: <100ns
//! - Integrity check: <100ns
//! - Chain verification: <100ns/link
//! - Incremental update: <1ns
//!
//! # Compliance Mapping
//!
//! - **SOX**: Transaction audit trail, unauthorized modification detection
//! - **SOC2 Type II**: Change control evidence, audit trail completeness
//! - **GDPR Article 15**: Data access logging
//! - **HIPAA**: Infrastructure ready (not applicable for non-PHI)
//!
//! # ASSUM Safety Tags
//!
//! - `#ASSUME_HASH_DETERMINISTIC`: Hash functions are deterministic
//! - `#VERIFY_HASH`: Property tests ensure determinism
//! - `#ASSUME_ATOMIC_ORDERING`: Acquire/Release sufficient for chain
//! - `#VERIFY_ATOMIC_ORDERING`: Stress tests validate concurrent access

use crate::error::AuditError;
use crate::traits::sealed::Sealed;

/// Auditable Capsule trait - Universal hash chain integrity
///
/// # Object Safety
///
/// This trait is object-safe (can be used as `dyn AuditableCapsule`):
/// - All methods take `&self` (no generics, no Self return types)
/// - No associated types
/// - No `where Self: Sized` bounds
/// - No methods with `#[cfg(feature)]` attributes
///
/// # Required Fields
///
/// Implementors MUST include these fields:
///
/// ```ignore
/// pub struct MyCapsule {
///     // User state fields
///     state: AtomicU64,
///
///     // Q34: Hash chain fields (MANDATORY)
///     hash: AtomicU64,              // Current state hash
///     prev_hash: AtomicU64,         // Chain link
///     generation: AtomicU64,        // TOCTOU prevention
///
///     _padding: [u8; N],            // Cache alignment
/// }
/// ```
///
/// # Memory Layout
///
/// - User state: Variable (tier-dependent)
/// - Fast hash: 16B (hash + prev_hash)
/// - Metadata: 8B (generation counter)
/// - Padding: To cache line boundary
///
/// # Example Implementation
///
/// ```ignore
/// impl AuditableCapsule for DashboardStateCapsule {
///     fn compute_fast_hash(&self) -> u64 {
///         XxHash64::hash_u64_slice(&[
///             self.current_budget_id.load(Ordering::Relaxed),
///             self.time_range_secs.load(Ordering::Relaxed),
///             self.scroll_offset.load(Ordering::Relaxed),
///             self.generation.load(Ordering::Relaxed),
///         ])
///     }
///
///     fn fast_hash(&self) -> u64 {
///         self.hash.load(Ordering::Acquire)
///     }
///
///     fn prev_fast_hash(&self) -> u64 {
///         self.prev_hash.load(Ordering::Acquire)
///     }
///
///     fn generation(&self) -> u64 {
///         self.generation.load(Ordering::Relaxed)
///     }
///
///     fn timestamp_ns(&self) -> u64 {
///         // Implementation-specific timestamp
///         0
///     }
/// }
/// ```
///
/// # Sealed Trait
///
/// This trait is sealed to prevent external implementations.
/// Only types within the atomic_capsule crate (or using the derive macro)
/// can implement AuditableCapsule.
pub trait AuditableCapsule: Sealed + Send + Sync {
    // ============================================================================
    // Fast Hash Operations (Always Available)
    // ============================================================================

    /// Compute fast hash from current state
    ///
    /// # Performance
    /// - Target: <100ns for typical capsule (4-8 fields)
    ///
    /// # Invariants
    /// - Must be deterministic (same state → same hash)
    /// - Must include all state-affecting fields
    /// - Must include generation counter
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_HASH_DETERMINISTIC`: Hash function is deterministic
    fn compute_fast_hash(&self) -> u64;

    /// Get current fast hash value
    ///
    /// # Performance
    /// - Target: <1ns (single atomic load)
    ///
    /// # Memory Ordering
    /// - Uses `Ordering::Acquire` to synchronize with updates
    fn fast_hash(&self) -> u64;

    /// Get previous fast hash value (chain link)
    ///
    /// # Performance
    /// - Target: <1ns (single atomic load)
    ///
    /// # Use Case
    /// Chain verification: `capsule.prev_fast_hash() == prev_capsule.fast_hash()`
    fn prev_fast_hash(&self) -> u64;

    /// Get generation counter
    ///
    /// # Performance
    /// - Target: <1ns (single atomic load)
    ///
    /// # Use Case
    /// TOCTOU prevention: verify generation hasn't changed during operation
    fn generation(&self) -> u64;

    /// Get timestamp in nanoseconds
    ///
    /// # Performance
    /// - Target: <10ns (atomic load or clock_gettime)
    ///
    /// # Implementation
    /// - May be last update time or capsule creation time
    /// - Implementation-specific
    fn timestamp_ns(&self) -> u64;

    // ============================================================================
    // Convenience Methods (Default Implementations)
    // ============================================================================

    /// Verify fast hash integrity (recompute and compare)
    ///
    /// # Performance
    /// - Target: <100ns (compute + load + compare)
    ///
    /// # Returns
    /// - `true` if current hash matches recomputed hash
    /// - `false` if tampering detected
    ///
    /// # Use Case
    /// Forensic analysis: detect unauthorized state modifications
    fn verify_integrity(&self) -> bool {
        let expected = self.compute_fast_hash();
        let actual = self.fast_hash();
        expected == actual
    }

    /// Verify fast hash chain continuity with previous capsule
    ///
    /// # Performance
    /// - Target: <10ns (2 loads + compare)
    ///
    /// # Returns
    /// - `Ok(())` if chain is valid (no missing/modified links)
    /// - `Err(AuditError::ChainMismatch)` if chain is broken
    ///
    /// # Use Case
    /// Audit trail: verify complete history from genesis to current
    fn verify_chain(&self, prev: &dyn AuditableCapsule) -> Result<(), AuditError> {
        let expected = prev.fast_hash();
        let actual = self.prev_fast_hash();

        if expected == actual {
            Ok(())
        } else {
            Err(AuditError::ChainMismatch {
                pos: 0, // Position unknown in pairwise comparison
                expected: format!("{:016x}", expected),
                actual: format!("{:016x}", actual),
            })
        }
    }

    /// Get audit trail snapshot for forensic analysis
    ///
    /// # Performance
    /// - Target: <50ns (copy atomic fields)
    ///
    /// # Returns
    /// Tuple of (fast_hash, prev_fast_hash, generation, timestamp)
    fn audit_snapshot(&self) -> (u64, u64, u64, u64) {
        (
            self.fast_hash(),
            self.prev_fast_hash(),
            self.generation(),
            self.timestamp_ns(),
        )
    }

    /// Check if capsule state has been modified since last hash update
    ///
    /// # Performance
    /// - Target: <100ns (same as verify_integrity)
    ///
    /// # Returns
    /// - `true` if state matches hash (not modified)
    /// - `false` if state diverged from hash (potentially modified)
    fn is_state_clean(&self) -> bool {
        self.verify_integrity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicU64, Ordering};

    // Mock implementation for testing
    struct TestCapsule {
        hash: AtomicU64,
        prev_hash: AtomicU64,
        generation: AtomicU64,
        state_field: AtomicU64,
    }

    impl TestCapsule {
        fn new(generation: u64) -> Self {
            let state_field = generation * 100;
            let hash = generation ^ state_field; // Simple hash

            Self {
                hash: AtomicU64::new(hash),
                prev_hash: AtomicU64::new(0),
                generation: AtomicU64::new(generation),
                state_field: AtomicU64::new(state_field),
            }
        }

        fn with_prev(generation: u64, prev_hash: u64) -> Self {
            let state_field = generation * 100;
            let hash = generation ^ state_field;

            Self {
                hash: AtomicU64::new(hash),
                prev_hash: AtomicU64::new(prev_hash),
                generation: AtomicU64::new(generation),
                state_field: AtomicU64::new(state_field),
            }
        }
    }

    impl AuditableCapsule for TestCapsule {
        fn compute_fast_hash(&self) -> u64 {
            let gen = self.generation.load(Ordering::Relaxed);
            let state = self.state_field.load(Ordering::Relaxed);
            gen ^ state
        }

        fn fast_hash(&self) -> u64 {
            self.hash.load(Ordering::Acquire)
        }

        fn prev_fast_hash(&self) -> u64 {
            self.prev_hash.load(Ordering::Acquire)
        }

        fn generation(&self) -> u64 {
            self.generation.load(Ordering::Relaxed)
        }

        fn timestamp_ns(&self) -> u64 {
            0 // Simplified for testing
        }
    }

    #[test]
    fn test_object_safety_compiles() {
        // This test verifies that AuditableCapsule is object-safe
        let capsule = TestCapsule::new(1);
        let _trait_object: &dyn AuditableCapsule = &capsule;

        // If this compiles, the trait is object-safe
    }

    #[test]
    fn test_verify_integrity_valid() {
        let capsule = TestCapsule::new(1);
        assert!(capsule.verify_integrity());
    }

    #[test]
    fn test_verify_integrity_corrupted() {
        let capsule = TestCapsule::new(1);
        capsule.hash.store(0xDEADBEEF, Ordering::Release);
        assert!(!capsule.verify_integrity());
    }

    #[test]
    fn test_verify_chain_valid() {
        let prev = TestCapsule::new(1);
        let current = TestCapsule::with_prev(2, prev.fast_hash());
        assert!(current.verify_chain(&prev).is_ok());
    }

    #[test]
    fn test_verify_chain_broken() {
        let prev = TestCapsule::new(1);
        let current = TestCapsule::with_prev(2, 0xFFFFFFFF);
        assert!(current.verify_chain(&prev).is_err());
    }

    #[test]
    fn test_audit_snapshot() {
        let capsule = TestCapsule::new(1);
        let (hash, prev_hash, generation, timestamp) = capsule.audit_snapshot();

        assert_eq!(hash, capsule.fast_hash());
        assert_eq!(prev_hash, capsule.prev_fast_hash());
        assert_eq!(generation, 1);
        assert_eq!(timestamp, 0);
    }

    #[test]
    fn test_is_state_clean() {
        let capsule = TestCapsule::new(1);
        assert!(capsule.is_state_clean());

        capsule.hash.store(0xBADC0FFE, Ordering::Release);
        assert!(!capsule.is_state_clean());
    }
}
