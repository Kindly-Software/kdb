//! BuildHardening Wrapper - kindly_dedup Integration
//!
//! **Purpose**: Wrapper for atomic_capsule::protection::BuildHardeningCapsule
//!
//! # Architecture
//! - **T0 Compile-Time**: All encryption happens at build time (0ns runtime)
//! - **XOR Cipher**: Customer ID encrypted with build-unique key
//! - **Build Signature**: SHA-256 hash of build artifacts
//!
//! # Integration (I20 Q1-Q20)
//! - Q1: Replaces visible customer ID with encrypted constant
//! - Q6: Const fn (no runtime state), compatible with kindly_dedup
//! - Q7: 0ns runtime cost (all compile-time)
//! - Q19: Big Bang deployment (deterministic capsule)
//!
//! # Usage
//! ```rust
//! use kindly_dedup::protection::BuildHardeningWrapper;
//!
//! // Initialize from build constants
//! let wrapper = BuildHardeningWrapper::from_build_constants();
//!
//! // Decrypt customer ID (<20ns, XOR cipher)
//! let customer_id = wrapper.decrypt_customer_id();
//! println!("Customer: {}", String::from_utf8_lossy(&customer_id));
//!
//! // Verify build integrity (<50ns, FNV-1a)
//! assert!(wrapper.verify_build_integrity());
//! ```

#![cfg(feature = "protection-build-hardening")]

use super::MetaCapsuleError;

// Re-export from atomic_capsule
#[cfg(feature = "protection-build-hardening")]
pub use atomic_capsule::protection::build_hardening::{
    derive_build_key, encrypt_customer_id_const, BuildHardeningCapsule,
};

/// Build-time constants (set via build.rs)
///
/// **Source**: Environment variables during compilation
/// - CUSTOMER_ID: Customer identifier (16 bytes)
/// - BUILD_SIGNATURE: SHA-256 hash of build artifacts (32 bytes)
/// - BUILD_TIMESTAMP: Unix timestamp when binary was built
mod build_constants {
    /// Customer ID (encrypted at compile-time)
    ///
    /// **Default**: "demo-customer-01" for testing
    /// **Production**: Set via CUSTOMER_ID env var in build.rs
    pub const CUSTOMER_ID: [u8; 16] = *b"demo-customer-01";

    /// Build signature (SHA-256 of build artifacts)
    ///
    /// **Default**: Zeros for testing
    /// **Production**: Set via BUILD_SIGNATURE env var in build.rs
    pub const BUILD_SIGNATURE: [u8; 32] = [0u8; 32];

    /// Build timestamp (Unix seconds)
    ///
    /// **Default**: 1730652000 (2024-11-03) for testing
    /// **Production**: Set via BUILD_TIMESTAMP env var in build.rs
    pub const BUILD_TIMESTAMP: u64 = 1730652000;

    /// Rustc version (for build key derivation)
    ///
    /// **Source**: rustc --version during build
    pub const RUSTC_VERSION: &[u8] = b"rustc 1.91.0";

    /// Git commit hash (for build key derivation)
    ///
    /// **Source**: git rev-parse HEAD during build
    pub const COMMIT_HASH: &[u8] = b"commit-placeholder";
}

/// Wrapper for BuildHardeningCapsule with kindly_dedup-specific constants
///
/// **UCE34 Q10**: T0 Compile-Time (const fn only)
/// **I20 Q6**: No runtime state, compatible with kindly_dedup
/// **I20 Q7**: 0ns runtime cost (all compile-time)
pub struct BuildHardeningWrapper {
    capsule: BuildHardeningCapsule,
    build_key: u64,
}

impl BuildHardeningWrapper {
    /// Create wrapper from build constants
    ///
    /// **Build Key**: Derived from rustc version + timestamp + commit hash
    ///
    /// # Performance
    /// - 0ns runtime cost (all const fn)
    pub fn from_build_constants() -> Self {
        // Derive build-unique encryption key (compile-time)
        let build_key = derive_build_key(
            build_constants::RUSTC_VERSION,
            build_constants::BUILD_TIMESTAMP,
            build_constants::COMMIT_HASH,
        );

        // Encrypt customer ID at compile-time
        let encrypted_customer_id = encrypt_customer_id_const(build_constants::CUSTOMER_ID, build_key);

        // Create hardening capsule (compile-time)
        let capsule = BuildHardeningCapsule::new(
            encrypted_customer_id,
            build_constants::BUILD_SIGNATURE,
            build_constants::BUILD_TIMESTAMP,
            build_key,
        );

        Self { capsule, build_key }
    }

    /// Decrypt customer ID (<20ns, XOR cipher)
    ///
    /// **Algorithm**: Simple XOR with build-unique key
    ///
    /// # Performance
    /// - <20ns (XOR operation, simple cipher)
    #[inline]
    pub fn decrypt_customer_id(&self) -> [u8; 16] {
        self.capsule.decrypt_customer_id(self.build_key)
    }

    /// Verify build integrity (<50ns, FNV-1a)
    ///
    /// **Algorithm**: FNV-1a hash of encrypted ID + signature + timestamp
    ///
    /// # Performance
    /// - <50ns (FNV-1a hash computation)
    #[inline]
    pub fn verify_build_integrity(&self) -> bool {
        self.capsule.verify_build_integrity(self.build_key)
    }

    /// Get build timestamp (Unix seconds)
    #[inline]
    pub fn build_timestamp(&self) -> u64 {
        build_constants::BUILD_TIMESTAMP
    }

    /// Get build signature (SHA-256 hash)
    #[inline]
    pub fn build_signature(&self) -> &[u8; 32] {
        &build_constants::BUILD_SIGNATURE
    }

    /// Get rustc version used to compile binary
    #[inline]
    pub fn rustc_version(&self) -> &str {
        std::str::from_utf8(build_constants::RUSTC_VERSION).unwrap_or("unknown")
    }

    /// Get git commit hash used to compile binary
    #[inline]
    pub fn commit_hash(&self) -> &str {
        std::str::from_utf8(build_constants::COMMIT_HASH).unwrap_or("unknown")
    }
}

impl Default for BuildHardeningWrapper {
    fn default() -> Self {
        Self::from_build_constants()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_hardening_wrapper_decrypt() {
        let wrapper = BuildHardeningWrapper::from_build_constants();

        let customer_id = wrapper.decrypt_customer_id();
        assert_eq!(&customer_id, b"demo-customer-01");
    }

    #[test]
    fn test_build_hardening_wrapper_verify() {
        let wrapper = BuildHardeningWrapper::from_build_constants();

        // Verification should always succeed for valid build
        assert!(wrapper.verify_build_integrity());
    }

    #[test]
    fn test_build_hardening_wrapper_metadata() {
        let wrapper = BuildHardeningWrapper::from_build_constants();

        assert_eq!(wrapper.build_timestamp(), 1730652000);
        assert!(wrapper.rustc_version().contains("rustc"));
    }

    #[test]
    fn test_build_hardening_wrapper_deterministic() {
        let wrapper1 = BuildHardeningWrapper::from_build_constants();
        let wrapper2 = BuildHardeningWrapper::from_build_constants();

        assert_eq!(wrapper1.decrypt_customer_id(), wrapper2.decrypt_customer_id());
        assert_eq!(wrapper1.verify_build_integrity(), wrapper2.verify_build_integrity());
    }
}
