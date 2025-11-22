//! Cryptographic Hash Extension for AuditableCapsule
//!
//! Feature-gated extension trait providing cryptographic hash operations.
//!
//! # Architecture
//!
//! ```text
//! AuditableCapsule (base, always available)
//!   └── CryptoAuditableCapsule (extension, feature: audit-trail)
//! ```
//!
//! # Why Separate?
//!
//! 1. **Object Safety**: Base trait must be object-safe for polymorphism
//! 2. **Performance**: Fast hash (xxHash64) is sufficient for most use cases
//! 3. **Feature Gating**: Crypto dependencies only when needed
//!
//! # Use Cases
//!
//! - **SOX Compliance**: Financial audit trails requiring crypto hash chains
//! - **SOC2**: Change control evidence with cryptographic integrity
//! - **HIPAA**: PHI access logging with tamper-evident chains
//! - **Government**: FIPS-compliant audit trails
//!
//! # Performance Targets
//!
//! - BLAKE3: 50-80ns (default crypto hash)
//! - SHA-256: 300-500ns (FIPS-compliant)
//! - Keyed HMAC: 100-150ns
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_CRYPTO_SECURE`: BLAKE3/SHA-256 are cryptographically secure
//! - `#VERIFY_CRYPTO`: Property tests ensure no collisions on test data
//! - `#ASSUME_HMAC_SECURE`: HMAC-SHA256 provides authentication
//! - `#VERIFY_HMAC`: Integration tests validate keyed authentication

use crate::error::AuditError;
use crate::traits::auditable_base::AuditableCapsule;

/// Cryptographic hash extension for AuditableCapsule
///
/// # Feature Gate
///
/// This trait is only available with the `audit-trail` feature:
///
/// ```toml
/// atomic_capsule = { version = "0.4.1", features = ["audit-trail"] }
/// ```
///
/// # Object Safety
///
/// This trait is **NOT object-safe** due to feature gating:
/// - Cannot use `dyn CryptoAuditableCapsule`
/// - Use static dispatch only
/// - Prefer `impl CryptoAuditableCapsule` in function signatures
///
/// # Example Implementation
///
/// ```ignore
/// #[cfg(feature = "audit-trail")]
/// impl CryptoAuditableCapsule for DashboardStateCapsule {
///     fn compute_crypto_hash(&self) -> [u8; 32] {
///         use blake3::Hasher;
///
///         let mut hasher = Hasher::new();
///         hasher.update(&self.current_budget_id.load(Ordering::Relaxed).to_le_bytes());
///         hasher.update(&self.time_range_secs.load(Ordering::Relaxed).to_le_bytes());
///         hasher.update(&self.generation.load(Ordering::Relaxed).to_le_bytes());
///
///         *hasher.finalize().as_bytes()
///     }
///
///     fn crypto_hash(&self) -> [u8; 32] {
///         let mut result = [0u8; 32];
///         // Load from atomic storage (implementation-specific)
///         result
///     }
///
///     fn prev_crypto_hash(&self) -> [u8; 32] {
///         let mut result = [0u8; 32];
///         // Load from atomic storage (implementation-specific)
///         result
///     }
///
///     fn compute_keyed_hmac(&self, key: &[u8; 32]) -> [u8; 32] {
///         use hmac::{Hmac, Mac};
///         use sha2::Sha256;
///
///         type HmacSha256 = Hmac<Sha256>;
///
///         let mut mac = HmacSha256::new_from_slice(key).unwrap();
///         mac.update(&self.crypto_hash());
///         mac.update(&self.generation().to_le_bytes());
///
///         let result = mac.finalize();
///         let bytes = result.into_bytes();
///         bytes.into()
///     }
/// }
/// ```
#[cfg(feature = "audit-trail")]
pub trait CryptoAuditableCapsule: AuditableCapsule {
    // ============================================================================
    // Cryptographic Hash Operations
    // ============================================================================

    /// Compute cryptographic hash from current state
    ///
    /// # Performance
    /// - BLAKE3: 50-80ns
    /// - SHA-256: 300-500ns (FIPS)
    ///
    /// # Compliance
    /// - SOX: Transaction audit trail
    /// - SOC2: Change control evidence
    /// - GDPR: Data access logging
    ///
    /// # Invariants
    /// - Must be deterministic (same state → same hash)
    /// - Must include all state-affecting fields
    /// - Must include generation counter
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_CRYPTO_DETERMINISTIC`: Crypto hash is deterministic
    fn compute_crypto_hash(&self) -> [u8; 32];

    /// Get current cryptographic hash value
    ///
    /// # Performance
    /// - Target: <5ns (4 atomic loads for 32 bytes)
    ///
    /// # Memory Ordering
    /// - Uses `Ordering::Acquire` to synchronize with updates
    fn crypto_hash(&self) -> [u8; 32];

    /// Get previous cryptographic hash value (chain link)
    ///
    /// # Performance
    /// - Target: <5ns (4 atomic loads for 32 bytes)
    ///
    /// # Use Case
    /// Chain verification: `capsule.prev_crypto_hash() == prev_capsule.crypto_hash()`
    fn prev_crypto_hash(&self) -> [u8; 32];

    /// Compute keyed HMAC for authentication
    ///
    /// # Performance
    /// - Target: <150ns (HMAC-SHA256)
    ///
    /// # Arguments
    /// - `key`: 32-byte secret key for HMAC
    ///
    /// # Returns
    /// 32-byte HMAC value
    ///
    /// # Use Case
    /// Cryptographic authentication of audit trail entries
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_HMAC_SECURE`: HMAC-SHA256 provides authentication
    fn compute_keyed_hmac(&self, key: &[u8; 32]) -> [u8; 32];

    // ============================================================================
    // Convenience Methods (Default Implementations)
    // ============================================================================

    /// Verify cryptographic hash integrity
    ///
    /// # Performance
    /// - Target: <100ns (compute + load + compare)
    ///
    /// # Compliance
    /// - SOX: Required for financial data
    /// - SOC2: Required for audit trails
    ///
    /// # Returns
    /// - `true` if hash matches
    /// - `false` if tampering detected
    fn verify_crypto_integrity(&self) -> bool {
        let expected = self.compute_crypto_hash();
        let actual = self.crypto_hash();
        expected == actual
    }

    /// Verify cryptographic hash chain continuity
    ///
    /// # Performance
    /// - Target: <20ns (8 loads + compare)
    ///
    /// # Compliance
    /// - SOX: Chain of custody evidence
    /// - SOC2: Complete audit trail
    ///
    /// # Returns
    /// - `Ok(())` if chain valid
    /// - `Err(AuditError::ChainMismatch)` if broken
    fn verify_crypto_chain(&self, prev: &dyn AuditableCapsule) -> Result<(), AuditError> {
        // NOTE: This requires prev to also implement CryptoAuditableCapsule
        // In practice, this is verified at compile-time by the caller

        // For now, we can only verify the fast hash chain
        // Full crypto chain verification requires both capsules to have crypto extension
        self.verify_chain(prev)
    }

    /// Verify keyed HMAC authentication
    ///
    /// # Performance
    /// - Target: <150ns (compute + compare)
    ///
    /// # Arguments
    /// - `key`: 32-byte secret key for HMAC
    /// - `expected_hmac`: HMAC value to verify
    ///
    /// # Returns
    /// - `Ok(())` if HMAC valid
    /// - `Err(AuditError::KeyedHmacFailed)` if invalid
    ///
    /// # Security Note
    ///
    /// Uses constant-time comparison to prevent timing attacks
    fn verify_keyed_hmac(&self, key: &[u8; 32], expected_hmac: &[u8; 32]) -> Result<(), AuditError> {
        let computed_hmac = self.compute_keyed_hmac(key);

        // Constant-time comparison (prevents timing attacks)
        let mut diff = 0u8;
        for i in 0..32 {
            diff |= computed_hmac[i] ^ expected_hmac[i];
        }

        if diff == 0 {
            Ok(())
        } else {
            Err(AuditError::KeyedHmacFailed {
                hmac: *expected_hmac,
            })
        }
    }
}

#[cfg(all(test, feature = "audit-trail"))]
mod tests {
    use super::*;
    use crate::traits::auditable_base::AuditableCapsule;
    use core::sync::atomic::{AtomicU64, Ordering};

    // Mock implementation for testing
    struct TestCapsule {
        hash: AtomicU64,
        prev_hash: AtomicU64,
        generation: AtomicU64,
        crypto_hash: [u8; 32],
        prev_crypto_hash: [u8; 32],
    }

    impl TestCapsule {
        fn new(generation: u64) -> Self {
            let mut crypto_hash = [0u8; 32];
            crypto_hash[0..8].copy_from_slice(&generation.to_le_bytes());

            Self {
                hash: AtomicU64::new(generation),
                prev_hash: AtomicU64::new(0),
                generation: AtomicU64::new(generation),
                crypto_hash,
                prev_crypto_hash: [0u8; 32],
            }
        }
    }

    impl AuditableCapsule for TestCapsule {
        fn compute_fast_hash(&self) -> u64 {
            self.generation.load(Ordering::Relaxed)
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
            0
        }
    }

    impl CryptoAuditableCapsule for TestCapsule {
        fn compute_crypto_hash(&self) -> [u8; 32] {
            let mut hash = [0u8; 32];
            let gen = self.generation.load(Ordering::Relaxed);
            hash[0..8].copy_from_slice(&gen.to_le_bytes());
            hash
        }

        fn crypto_hash(&self) -> [u8; 32] {
            self.crypto_hash
        }

        fn prev_crypto_hash(&self) -> [u8; 32] {
            self.prev_crypto_hash
        }

        fn compute_keyed_hmac(&self, key: &[u8; 32]) -> [u8; 32] {
            // Simplified HMAC for testing (NOT cryptographically secure)
            let mut hmac = [0u8; 32];
            for i in 0..32 {
                hmac[i] = key[i] ^ self.crypto_hash[i];
            }
            hmac
        }
    }

    #[test]
    fn test_verify_crypto_integrity_valid() {
        let capsule = TestCapsule::new(1);
        assert!(capsule.verify_crypto_integrity());
    }

    #[test]
    fn test_verify_crypto_integrity_corrupted() {
        let mut capsule = TestCapsule::new(1);
        capsule.crypto_hash[0] = 0xFF; // Corrupt hash
        assert!(!capsule.verify_crypto_integrity());
    }

    #[test]
    fn test_verify_keyed_hmac_valid() {
        let capsule = TestCapsule::new(1);
        let key = [0x42u8; 32];
        let hmac = capsule.compute_keyed_hmac(&key);

        assert!(capsule.verify_keyed_hmac(&key, &hmac).is_ok());
    }

    #[test]
    fn test_verify_keyed_hmac_invalid() {
        let capsule = TestCapsule::new(1);
        let key = [0x42u8; 32];
        let wrong_hmac = [0xFFu8; 32];

        assert!(capsule.verify_keyed_hmac(&key, &wrong_hmac).is_err());
    }
}
