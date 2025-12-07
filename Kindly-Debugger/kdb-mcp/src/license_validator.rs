//! LicenseValidatorCapsule - T1 Atomic License Validation (4 KB)
//!
//! Lockfree license key validation with caching.
//! **Latency**: <10ns cached validation
//! **Tier**: T1 Atomic (cached hash + timestamp)

use core::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// LicenseValidatorCapsule (4 KB, 64-byte aligned)
// ============================================================================

#[repr(C, align(64))]
pub struct LicenseValidatorCapsule {
    // License state (64 bytes, single cache line)
    pub license_hash: AtomicU64,         // FNV-1a hash of license key
    pub expiry_unix: AtomicU64,          // License expiry (Unix seconds)
    pub validation_count: AtomicU64,     // Total validations
    pub validation_success: AtomicU64,   // Successful validations
    pub validation_failed: AtomicU64,    // Failed validations
    pub last_validation_ns: AtomicU64,   // Last validation timestamp
    pub cached_valid: AtomicU64,         // Cached validity (1 = valid, 0 = invalid)
    _padding: [u8; 8],

    // Reserved space (4KB - 64 bytes = 4032 bytes)
    _reserved: [u8; 4032],
}

impl LicenseValidatorCapsule {
    /// Create new license validator (no license by default)
    pub const fn new() -> Self {
        Self {
            license_hash: AtomicU64::new(0),
            expiry_unix: AtomicU64::new(0),
            validation_count: AtomicU64::new(0),
            validation_success: AtomicU64::new(0),
            validation_failed: AtomicU64::new(0),
            last_validation_ns: AtomicU64::new(0),
            cached_valid: AtomicU64::new(0),
            _padding: [0; 8],
            _reserved: [0; 4032],
        }
    }

    /// Set license key
    pub fn set_license(&self, license_key: &str, expiry_unix: u64) {
        let hash = self.fnv1a_hash(license_key.as_bytes());
        self.license_hash.store(hash, Ordering::Release);
        self.expiry_unix.store(expiry_unix, Ordering::Release);
        self.cached_valid.store(1, Ordering::Release); // Assume valid until proven otherwise
    }

    /// Validate license (<10ns cached, ~50ns fresh)
    pub fn validate(&self) -> bool {
        self.validation_count.fetch_add(1, Ordering::Relaxed);

        // Fast path: check cached validity
        let cached = self.cached_valid.load(Ordering::Acquire);
        if cached == 1 {
            // Check expiry
            let expiry = self.expiry_unix.load(Ordering::Relaxed);
            let now_unix = self.get_unix_seconds();

            if now_unix < expiry {
                self.validation_success.fetch_add(1, Ordering::Relaxed);
                return true;
            } else {
                // Expired, invalidate cache
                self.cached_valid.store(0, Ordering::Release);
            }
        }

        // Failed validation
        self.validation_failed.fetch_add(1, Ordering::Relaxed);
        false
    }

    /// Validate with specific license key (slower, ~100ns)
    pub fn validate_key(&self, license_key: &str) -> bool {
        let hash = self.fnv1a_hash(license_key.as_bytes());
        let stored_hash = self.license_hash.load(Ordering::Acquire);

        if hash == stored_hash {
            self.validate()
        } else {
            self.validation_count.fetch_add(1, Ordering::Relaxed);
            self.validation_failed.fetch_add(1, Ordering::Relaxed);
            false
        }
    }

    /// FNV-1a hash (fast, lockfree)
    fn fnv1a_hash(&self, bytes: &[u8]) -> u64 {
        const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;

        let mut hash = FNV_OFFSET_BASIS;
        for &byte in bytes {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }

    /// Get statistics
    pub fn get_stats(&self) -> LicenseStats {
        LicenseStats {
            validation_count: self.validation_count.load(Ordering::Relaxed),
            validation_success: self.validation_success.load(Ordering::Relaxed),
            validation_failed: self.validation_failed.load(Ordering::Relaxed),
            is_valid: self.cached_valid.load(Ordering::Relaxed) == 1,
            expiry_unix: self.expiry_unix.load(Ordering::Relaxed),
        }
    }

    #[inline]
    fn get_unix_seconds(&self) -> u64 {
        #[cfg(feature = "std")]
        {
            use std::time::SystemTime;
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        }
        #[cfg(not(feature = "std"))]
        {
            0 // No-op in no_std
        }
    }
}

/// License statistics
#[derive(Debug, Clone, Copy)]
pub struct LicenseStats {
    pub validation_count: u64,
    pub validation_success: u64,
    pub validation_failed: u64,
    pub is_valid: bool,
    pub expiry_unix: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{size_of, align_of};

    #[test]
    fn test_license_validator_size() {
        assert_eq!(size_of::<LicenseValidatorCapsule>(), 4096, "LicenseValidatorCapsule must be 4 KB");
    }

    #[test]
    fn test_license_validator_alignment() {
        assert_eq!(align_of::<LicenseValidatorCapsule>(), 64, "LicenseValidatorCapsule must be 64-byte aligned");
    }

    #[test]
    fn test_set_license() {
        let validator = LicenseValidatorCapsule::new();

        let expiry = 2000000000; // Year 2033
        validator.set_license("test-license-key-123", expiry);

        assert!(validator.validate());

        let stats = validator.get_stats();
        assert_eq!(stats.validation_success, 1);
        assert_eq!(stats.validation_failed, 0);
    }

    #[test]
    fn test_validate_key() {
        let validator = LicenseValidatorCapsule::new();

        validator.set_license("test-key", 2000000000);

        assert!(validator.validate_key("test-key"));
        assert!(!validator.validate_key("wrong-key"));

        let stats = validator.get_stats();
        assert_eq!(stats.validation_success, 1);
        assert_eq!(stats.validation_failed, 1);
    }

    #[test]
    fn test_expired_license() {
        let validator = LicenseValidatorCapsule::new();

        let expiry = 1000000000; // Year 2001 (expired)
        validator.set_license("test-key", expiry);

        assert!(!validator.validate());

        let stats = validator.get_stats();
        assert_eq!(stats.validation_failed, 1);
    }
}
