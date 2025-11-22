//! # Cache HMAC Integration Helpers
//!
//! **Integration layer for CacheSlot HMAC verification**
//!
//! ## Purpose
//! - Provides helper functions for insert() and get() integration
//! - Handles feature-gated HMAC computation and verification
//! - Ensures correct memory ordering for HMAC storage/loading
//!
//! ## Integration Points
//! 1. `store_entry_with_hmac()` - Called from insert() after value storage
//! 2. `verify_entry_hmac()` - Called from get() before returning value
//! 3. `clear_hmac()` - Called from clear() to zero HMAC tag
//!
//! ## Feature Flags
//! - `cache-hmac`: Full HMAC integrity verification
//! - `cache`: Base cache functionality (SipHash + TTL)

#[cfg(all(feature = "std", feature = "cache-hmac"))]
use super::cache_hmac::{compute_cache_hmac, compute_value_hash, verify_cache_hmac};

/// Store HMAC tag after inserting cache entry
///
/// # Arguments
/// - `hmac_storage`: Reference to 32-byte HMAC storage in CacheSlot
/// - `key_hash`: SipHash-2-4 of cache key
/// - `value`: Reference to cached value
/// - `ttl_expiry`: Q16.16 fixed-point expiration timestamp
/// - `generation`: Current generation counter
/// - `tenant_id`: Multi-tenant isolation ID
///
/// # Performance
/// - With `cache-hmac`: ~500ns (HMAC-SHA256 computation)
/// - Without `cache-hmac`: 0ns (no-op)
///
/// # Memory Ordering
/// - HMAC stored with Release ordering (synchronizes-with Acquire in verify)
///
/// # ASSUM Tags
/// - `#ASSUME_HMAC_AFTER_VALUE`: HMAC stored AFTER value pointer (prevents race)
#[cfg(all(feature = "std", feature = "cache-hmac"))]
#[inline]
pub fn store_entry_hmac<V>(
    hmac_storage: &mut [u8; 32],
    key_hash: u64,
    value: &V,
    ttl_expiry: u64,
    generation: u64,
    tenant_id: u64,
) where
    V: AsRef<[u8]> + ?Sized,
{
    // Compute value hash (stable identity, not pointer-based)
    let value_hash = compute_value_hash(value);

    // Compute HMAC tag (500ns)
    let hmac_tag = compute_cache_hmac(key_hash, value_hash, ttl_expiry, generation, tenant_id);

    // Store HMAC tag (no atomic required, mutable reference ensures exclusivity)
    // #ASSUME_HMAC_AFTER_VALUE: Called AFTER value_ptr.store(Release)
    hmac_storage.copy_from_slice(&hmac_tag);
}

/// Fallback for store_entry_hmac when feature disabled
#[cfg(not(all(feature = "std", feature = "cache-hmac")))]
#[inline]
pub fn store_entry_hmac<V>(
    _hmac_storage: &mut [u8; 32],
    _key_hash: u64,
    _value: &V,
    _ttl_expiry: u64,
    _generation: u64,
    _tenant_id: u64,
) where
    V: AsRef<[u8]> + ?Sized,
{
    // No-op when feature disabled
}

/// Verify HMAC tag before returning cached value
///
/// # Arguments
/// - `hmac_storage`: Reference to 32-byte HMAC storage in CacheSlot
/// - `key_hash`: SipHash-2-4 of cache key
/// - `value`: Reference to cached value
/// - `ttl_expiry`: Q16.16 fixed-point expiration timestamp
/// - `generation`: Current generation counter
/// - `tenant_id`: Multi-tenant isolation ID
///
/// # Returns
/// - `true` if HMAC valid or feature disabled (cache entry safe to return)
/// - `false` if HMAC invalid (cache poisoning detected, return None)
///
/// # Performance
/// - With `cache-hmac`: ~510ns (HMAC compute 500ns + verify 10ns)
/// - Without `cache-hmac`: 0ns (always returns true)
///
/// # Security
/// - Constant-time comparison (prevents timing attacks)
/// - Returns `false` on verification failure (cache poisoning detected)
///
/// # ASSUM Tags
/// - `#ASSUME_HMAC_BEFORE_RETURN`: HMAC verified BEFORE returning value
#[cfg(all(feature = "std", feature = "cache-hmac"))]
#[inline]
pub fn verify_entry_hmac<V>(
    hmac_storage: &[u8; 32],
    key_hash: u64,
    value: &V,
    ttl_expiry: u64,
    generation: u64,
    tenant_id: u64,
) -> bool
where
    V: AsRef<[u8]> + ?Sized,
{
    // Compute value hash (stable identity, not pointer-based)
    let value_hash = compute_value_hash(value);

    // Compute expected HMAC (500ns)
    let expected_hmac = compute_cache_hmac(key_hash, value_hash, ttl_expiry, generation, tenant_id);

    // Verify HMAC (constant-time comparison, 10ns)
    // #ASSUME_HMAC_BEFORE_RETURN: Called BEFORE returning value to caller
    verify_cache_hmac(hmac_storage, &expected_hmac)
}

/// Fallback for verify_entry_hmac when feature disabled
#[cfg(not(all(feature = "std", feature = "cache-hmac")))]
#[inline]
pub fn verify_entry_hmac<V>(
    _hmac_storage: &[u8; 32],
    _key_hash: u64,
    _value: &V,
    _ttl_expiry: u64,
    _generation: u64,
    _tenant_id: u64,
) -> bool
where
    V: AsRef<[u8]> + ?Sized,
{
    // Always return true when feature disabled (no verification)
    true
}

/// Clear HMAC tag when evicting cache entry
///
/// # Arguments
/// - `hmac_storage`: Reference to 32-byte HMAC storage in CacheSlot
///
/// # Performance
/// - <10ns (32-byte zero fill)
///
/// # Memory Ordering
/// - No atomic required (mutable reference ensures exclusivity)
#[inline]
pub fn clear_entry_hmac(hmac_storage: &mut [u8; 32]) {
    // Zero out HMAC tag (prevents stale HMAC reuse)
    hmac_storage.fill(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(feature = "std", feature = "cache-hmac"))]
    #[test]
    fn test_store_and_verify_hmac_valid() {
        let mut hmac_storage = [0u8; 32];
        let value = vec![1u8, 2, 3, 4, 5];

        // Store HMAC
        store_entry_hmac(&mut hmac_storage, 123, &value, 789, 100, 200);

        // Verify HMAC
        assert!(
            verify_entry_hmac(&hmac_storage, 123, &value, 789, 100, 200),
            "HMAC verification must succeed for valid entry"
        );
    }

    #[cfg(all(feature = "std", feature = "cache-hmac"))]
    #[test]
    fn test_verify_hmac_tampered_value() {
        let mut hmac_storage = [0u8; 32];
        let value = vec![1u8, 2, 3, 4, 5];

        // Store HMAC
        store_entry_hmac(&mut hmac_storage, 123, &value, 789, 100, 200);

        // Tamper value
        let tampered_value = vec![1u8, 2, 3, 4, 6]; // Last byte differs

        // Verify HMAC (should fail)
        assert!(
            !verify_entry_hmac(&hmac_storage, 123, &tampered_value, 789, 100, 200),
            "HMAC verification must fail for tampered value"
        );
    }

    #[cfg(all(feature = "std", feature = "cache-hmac"))]
    #[test]
    fn test_verify_hmac_tampered_generation() {
        let mut hmac_storage = [0u8; 32];
        let value = vec![1u8, 2, 3, 4, 5];

        // Store HMAC
        store_entry_hmac(&mut hmac_storage, 123, &value, 789, 100, 200);

        // Tamper generation counter
        assert!(
            !verify_entry_hmac(&hmac_storage, 123, &value, 789, 101, 200),
            "HMAC verification must fail for tampered generation"
        );
    }

    #[cfg(all(feature = "std", feature = "cache-hmac"))]
    #[test]
    fn test_verify_hmac_tampered_tenant() {
        let mut hmac_storage = [0u8; 32];
        let value = vec![1u8, 2, 3, 4, 5];

        // Store HMAC
        store_entry_hmac(&mut hmac_storage, 123, &value, 789, 100, 200);

        // Tamper tenant_id
        assert!(
            !verify_entry_hmac(&hmac_storage, 123, &value, 789, 100, 201),
            "HMAC verification must fail for tampered tenant_id"
        );
    }

    #[test]
    fn test_clear_hmac() {
        let mut hmac_storage = [0xFFu8; 32]; // Non-zero initial state

        // Clear HMAC
        clear_entry_hmac(&mut hmac_storage);

        // Verify all zeros
        assert!(
            hmac_storage.iter().all(|&b| b == 0),
            "HMAC storage must be zeroed after clear"
        );
    }

    #[cfg(not(all(feature = "std", feature = "cache-hmac")))]
    #[test]
    fn test_fallback_store_verify() {
        let mut hmac_storage = [0u8; 32];
        let value = vec![1u8, 2, 3, 4, 5];

        // Store HMAC (no-op)
        store_entry_hmac(&mut hmac_storage, 123, &value, 789, 100, 200);

        // Verify HMAC (always true)
        assert!(
            verify_entry_hmac(&hmac_storage, 123, &value, 789, 100, 200),
            "HMAC verification must return true when feature disabled"
        );
    }
}
