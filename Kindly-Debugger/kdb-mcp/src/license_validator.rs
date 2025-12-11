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
    ///
    /// **Validation Modes**:
    /// 1. If admin license is set (via set_license), validates against stored hash
    /// 2. If no admin license, accepts ANY properly formatted KDB-* license
    ///
    /// **Format**: `KDB-{TIER}-{...}` where TIER is one of:
    /// - HOBBY (free tier)
    /// - PRO (was STARTER)
    /// - ENGINEER (was DEVELOPER)
    /// - TEAMS (was PROFESSIONAL)
    /// - ENTERPRISE
    /// - ADMIN (admin override)
    pub fn validate_key(&self, license_key: &str) -> bool {
        let stored_hash = self.license_hash.load(Ordering::Acquire);

        // Mode 1: Admin license override - exact hash match
        if stored_hash != 0 {
            let hash = self.fnv1a_hash(license_key.as_bytes());
            if hash == stored_hash {
                return self.validate();
            }
            // Fall through to format validation below
        }

        // Mode 2: Format-based validation (for OAuth-provisioned licenses)
        // Accept any KDB-{TIER}-* formatted license
        if self.is_valid_format(license_key) {
            self.validation_count.fetch_add(1, Ordering::Relaxed);
            self.validation_success.fetch_add(1, Ordering::Relaxed);
            eprintln!("[License] Format-based validation passed: {}...", &license_key[..license_key.len().min(20)]);
            return true;
        }

        // Invalid format or no match
        self.validation_count.fetch_add(1, Ordering::Relaxed);
        self.validation_failed.fetch_add(1, Ordering::Relaxed);
        eprintln!("[License] Validation failed: {}...", &license_key[..license_key.len().min(20)]);
        false
    }

    /// Check if license key matches valid format: `KDB-{TIER}-{...}`
    ///
    /// **Performance**: <10ns (string prefix check)
    ///
    /// **Valid Tiers**: HOBBY, PRO, ENGINEER, TEAMS, ENTERPRISE, ADMIN
    fn is_valid_format(&self, license_key: &str) -> bool {
        // Must start with "KDB-"
        if !license_key.starts_with("KDB-") {
            return false;
        }

        // Must have at least KDB-X-Y format (minimum 8 chars)
        if license_key.len() < 8 {
            return false;
        }

        // Extract tier (second component)
        let parts: Vec<&str> = license_key.splitn(3, '-').collect();
        if parts.len() < 3 {
            return false;
        }

        // Validate tier matches known tiers
        let tier = parts[1];
        matches!(
            tier,
            "HOBBY" | "PRO" | "ENGINEER" | "TEAMS" | "ENTERPRISE" | "ADMIN"
        )
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

    // ========================================================================
    // Format-Based Validation Tests (OAuth-provisioned licenses)
    // ========================================================================

    #[test]
    fn test_format_validation_hobby() {
        let validator = LicenseValidatorCapsule::new();
        // No admin license set - should accept format-based validation

        assert!(validator.validate_key("KDB-HOBBY-693ace9a-1"));
        assert!(validator.validate_key("KDB-HOBBY-abc123-456"));

        let stats = validator.get_stats();
        assert_eq!(stats.validation_success, 2);
        assert_eq!(stats.validation_failed, 0);
    }

    #[test]
    fn test_format_validation_all_tiers() {
        let validator = LicenseValidatorCapsule::new();

        // All tier formats should pass
        assert!(validator.validate_key("KDB-HOBBY-abc-123"));
        assert!(validator.validate_key("KDB-PRO-xyz-789"));
        assert!(validator.validate_key("KDB-ENGINEER-dev-456"));
        assert!(validator.validate_key("KDB-TEAMS-team-001"));
        assert!(validator.validate_key("KDB-ENTERPRISE-ent-999"));
        assert!(validator.validate_key("KDB-ADMIN-adm-root"));

        let stats = validator.get_stats();
        assert_eq!(stats.validation_success, 6);
        assert_eq!(stats.validation_failed, 0);
    }

    #[test]
    fn test_format_validation_invalid_prefix() {
        let validator = LicenseValidatorCapsule::new();

        // Wrong prefix
        assert!(!validator.validate_key("NOTDB-HOBBY-123"));
        assert!(!validator.validate_key("KD-HOBBY-123"));
        assert!(!validator.validate_key("hobby-123"));

        let stats = validator.get_stats();
        assert_eq!(stats.validation_success, 0);
        assert_eq!(stats.validation_failed, 3);
    }

    #[test]
    fn test_format_validation_invalid_tier() {
        let validator = LicenseValidatorCapsule::new();

        // Unknown tier
        assert!(!validator.validate_key("KDB-UNKNOWN-123"));
        assert!(!validator.validate_key("KDB-FREE-123"));
        assert!(!validator.validate_key("KDB-STARTER-123")); // Old tier name

        let stats = validator.get_stats();
        assert_eq!(stats.validation_success, 0);
        assert_eq!(stats.validation_failed, 3);
    }

    #[test]
    fn test_format_validation_too_short() {
        let validator = LicenseValidatorCapsule::new();

        // Too short (< 8 chars)
        assert!(!validator.validate_key("KDB-"));
        assert!(!validator.validate_key("KDB-H-1"));
        assert!(!validator.validate_key("KDB"));

        let stats = validator.get_stats();
        assert_eq!(stats.validation_success, 0);
        assert_eq!(stats.validation_failed, 3);
    }

    #[test]
    fn test_format_validation_missing_components() {
        let validator = LicenseValidatorCapsule::new();

        // Missing third component
        assert!(!validator.validate_key("KDB-HOBBY"));
        assert!(!validator.validate_key("KDB-PRO-"));

        let stats = validator.get_stats();
        assert_eq!(stats.validation_success, 0);
        assert_eq!(stats.validation_failed, 2);
    }

    #[test]
    fn test_admin_override_takes_precedence() {
        let validator = LicenseValidatorCapsule::new();

        // Set admin license
        let admin_key = "KDB-ADMIN-special-override";
        let expiry = 2000000000; // Year 2033
        validator.set_license(admin_key, expiry);

        // Admin key should pass via hash match
        assert!(validator.validate_key(admin_key));

        // Other KDB-* keys should ALSO pass via format validation (fallback)
        assert!(validator.validate_key("KDB-HOBBY-user-123"));
        assert!(validator.validate_key("KDB-PRO-user-456"));

        let stats = validator.get_stats();
        assert_eq!(stats.validation_success, 3);
        assert_eq!(stats.validation_failed, 0);
    }

    #[test]
    fn test_oauth_provisioned_license_example() {
        let validator = LicenseValidatorCapsule::new();
        // No admin license set - simulates production OAuth flow

        // OAuth provisions license like this (see mcp_sse_server.rs line 1752)
        let timestamp = 1234567890u64;
        let email_hash = 0xabcd1234u64;
        let oauth_license = format!("KDB-HOBBY-{:x}-{:x}", timestamp, email_hash);

        // Should pass format validation
        assert!(validator.validate_key(&oauth_license));

        let stats = validator.get_stats();
        assert_eq!(stats.validation_success, 1);
        assert_eq!(stats.validation_failed, 0);
    }

    #[test]
    fn test_empty_license_key() {
        let validator = LicenseValidatorCapsule::new();

        assert!(!validator.validate_key(""));

        let stats = validator.get_stats();
        assert_eq!(stats.validation_failed, 1);
    }

    #[test]
    fn test_case_sensitive_tier() {
        let validator = LicenseValidatorCapsule::new();

        // Tier must be uppercase
        assert!(validator.validate_key("KDB-HOBBY-123"));
        assert!(!validator.validate_key("KDB-hobby-123")); // Lowercase tier
        assert!(!validator.validate_key("KDB-Hobby-123")); // Mixed case

        let stats = validator.get_stats();
        assert_eq!(stats.validation_success, 1);
        assert_eq!(stats.validation_failed, 2);
    }
}
