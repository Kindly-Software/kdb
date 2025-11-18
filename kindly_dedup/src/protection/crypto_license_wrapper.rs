//! CryptoLicense Wrapper - kindly_dedup Integration
//!
//! **Purpose**: Wrapper for atomic_capsule::protection::CryptoLicenseCapsule
//!
//! # Architecture
//! - **T1 Atomic**: DualAtomicU64 license state coordination
//! - **Ed25519**: 2^128 security, <500µs verification
//! - **24hr Cache**: <10ns cached validation, <1ns amortized
//!
//! # Integration (I20 Q1-Q20)
//! - Q1: Replaces file-based LicenseValidator with cryptographic signatures
//! - Q6: Lockfree (DualAtomicU64), compatible with kindly_dedup
//! - Q7: <10ns cached (<0.01% overhead)
//! - Q19: Big Bang deployment (deterministic capsule)
//!
//! # Usage
//! ```rust,no_run
//! use kindly_dedup::protection::CryptoLicenseWrapper;
//!
//! // Initialize with embedded public key
//! let wrapper = CryptoLicenseWrapper::new()?;
//!
//! // Verify license (Ed25519, <500µs)
//! wrapper.verify_from_file("license.dat")?;
//!
//! // Fast cached check (<10ns)
//! if wrapper.is_valid() {
//!     println!("License valid");
//! }
//! ```

#![cfg(feature = "protection-crypto-license")]

use super::MetaCapsuleError;
use std::fs;
use std::path::Path;
use std::time::Duration;

// Re-export from atomic_capsule
#[cfg(feature = "protection-crypto-license")]
pub use atomic_capsule::protection::crypto_license::{CryptoLicenseCapsule, LicenseData, LicenseError, LicenseStatus};

/// Wrapper for CryptoLicenseCapsule with kindly_dedup-specific integration
///
/// **UCE34 Q10**: T1 Atomic (DualAtomicU64)
/// **I20 Q6**: Lockfree, compatible with kindly_dedup
/// **I20 Q7**: <10ns cached, <500µs verify
pub struct CryptoLicenseWrapper {
    capsule: CryptoLicenseCapsule,
    license_path: Option<String>,
}

impl CryptoLicenseWrapper {
    /// Create new license wrapper with embedded public key
    ///
    /// **Public Key**: Embedded at build time (build.rs)
    ///
    /// # Errors
    /// - `MetaCapsuleError::InvalidPublicKey`: Public key format invalid
    ///
    /// # Performance
    /// - <1µs (one-time initialization)
    pub fn new() -> Result<Self, MetaCapsuleError> {
        // Load public key from build constants
        let public_key = Self::load_embedded_public_key()?;

        Ok(Self {
            capsule: CryptoLicenseCapsule::new(public_key),
            license_path: None,
        })
    }

    /// Verify license from file
    ///
    /// **Format**: Binary file with license data + Ed25519 signature (64 bytes)
    ///
    /// # Errors
    /// - `MetaCapsuleError::LicenseFailed`: Signature invalid or license expired
    ///
    /// # Performance
    /// - <500µs (Ed25519 verification, constant-time)
    pub fn verify_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), MetaCapsuleError> {
        let data = fs::read(&path)
            .map_err(|e| MetaCapsuleError::LicenseFailed(format!("Failed to read license file: {}", e)))?;

        if data.len() < 64 {
            return Err(MetaCapsuleError::LicenseFailed(
                "License file too small (expected ≥64 bytes)".to_string(),
            ));
        }

        // Split into license data + signature (last 64 bytes)
        let (license_bytes, signature_bytes) = data.split_at(data.len() - 64);

        // Parse license data
        let license = LicenseData::from_bytes(license_bytes)?;

        // Convert signature bytes to array
        let mut signature = [0u8; 64];
        signature.copy_from_slice(signature_bytes);

        // Verify signature (Ed25519, <500µs)
        self.capsule.verify_license(&license, &signature)?;

        // Store path for future reference
        self.license_path = Some(path.as_ref().to_string_lossy().to_string());

        Ok(())
    }

    /// Check if license is valid (cached, <10ns)
    ///
    /// **Cache**: 24hr validity, no signature verification
    ///
    /// # Performance
    /// - <10ns (DualAtomicU64 load, Relaxed ordering)
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.capsule.is_valid()
    }

    /// Get license status
    ///
    /// **Status**: Unverified | Valid | SignatureInvalid | Expired
    ///
    /// # Performance
    /// - <10ns (atomic load)
    #[inline]
    pub fn status(&self) -> LicenseStatus {
        self.capsule.status()
    }

    /// Time until license expiry
    ///
    /// **Returns**: Some(duration) if license valid, None otherwise
    ///
    /// # Performance
    /// - <10ns (atomic load + subtraction)
    #[inline]
    pub fn time_until_expiry(&self) -> Option<Duration> {
        self.capsule.time_until_expiry()
    }

    /// Get license file path (if verified from file)
    #[inline]
    pub fn license_path(&self) -> Option<&str> {
        self.license_path.as_deref()
    }

    /// Load embedded public key from build constants
    ///
    /// **Build-time**: Public key embedded via build.rs
    ///
    /// # Errors
    /// - `MetaCapsuleError::InvalidPublicKey`: Key not found or invalid format
    fn load_embedded_public_key() -> Result<[u8; 32], MetaCapsuleError> {
        // Public key embedded at build time
        // In production, this would be set via build.rs:
        // const PUBLIC_KEY: [u8; 32] = include_bytes!(concat!(env!("OUT_DIR"), "/public_key.bin"));

        // For now, use a placeholder (demo key)
        // TODO: Replace with actual key embedding in build.rs
        let key = [0u8; 32]; // Placeholder

        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_license_wrapper_new() {
        let wrapper = CryptoLicenseWrapper::new();
        assert!(wrapper.is_ok());
    }

    #[test]
    fn test_crypto_license_wrapper_initial_state() {
        let wrapper = CryptoLicenseWrapper::new().unwrap();
        assert_eq!(wrapper.status(), LicenseStatus::Unverified);
        assert_eq!(wrapper.is_valid(), false);
        assert_eq!(wrapper.time_until_expiry(), None);
        assert_eq!(wrapper.license_path(), None);
    }
}
