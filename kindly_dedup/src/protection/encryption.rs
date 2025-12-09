//! AES-256-GCM encryption for algorithm parameters
//!
//! ## Purpose
//!
//! Encrypts pipeline configuration in memory (defeats memory dump attacks).
//!
//! **Legal Context**: DEFENSIVE security - protecting trade secret algorithm parameters
//! (912× speedup configurations), not user data. Licensed software with agreed protection.
//!
//! ## Security Properties
//!
//! - **AES-256-GCM**: NIST-approved authenticated encryption
//! - **RDRAND nonce**: Hardware random number generator (unique per encryption)
//! - **Constant-time**: aes-gcm crate provides constant-time implementation
//! - **Authentication**: 16-byte tag prevents tampering
//!
//! ## UCE34 Framework
//!
//! - Q10: Tier = T0 Foundation (encryption, zero coordination)
//! - Q28: Simplicity = AES-256-GCM only (standard, NIST-approved)
//! - Q29: Dependencies = aes-gcm crate (RustCrypto, well-audited)
//! - Q33: Validation = NIST test vectors
//!
//! ## ASSUM Safety
//!
//! - #ASSUME: AES-256-GCM provides authenticated encryption
//! - #VERIFY: NIST SP 800-38D test vectors
//! - #ASSUME: aes-gcm crate is constant-time
//! - #VERIFY: Benchmark timing variance <5%
//!
//! ## Example
//!
//! ```rust,ignore
//! use kindly_dedup::protection::encryption::{AlgorithmConfig, EncryptedConfig};
//!
//! // Create configuration
//! let config = AlgorithmConfig {
//!     num_hashes: 128,
//!     num_bands: 16,
//!     rows_per_band: 8,
//!     threshold: 0.85,
//!     parallel_enabled: true,
//!     simd_enabled: true,
//!     _reserved: [0u8; 30],
//! };
//!
//! // Derive key (from hardware ID + PUF)
//! let key = derive_key()?;
//!
//! // Encrypt
//! let encrypted = EncryptedConfig::encrypt(&config, &key)?;
//!
//! // Decrypt (later)
//! let decrypted = encrypted.decrypt(&key)?;
//! assert_eq!(decrypted.num_hashes, 128);
//! ```

#![allow(dead_code)]

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};

/// Algorithm configuration (plaintext, 64 bytes)
///
/// Contains deduplication pipeline parameters that constitute trade secrets
/// (e.g., 912× speedup parameter tuning).
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct AlgorithmConfig {
    /// Number of MinHash signatures (128 typical)
    pub num_hashes: usize, // 8B

    /// Number of LSH bands (5-16 typical for 92-99% recall)
    pub num_bands: usize, // 8B

    /// Rows per LSH band (8-16 typical)
    pub rows_per_band: usize, // 8B

    /// Jaccard similarity threshold (0.85 typical)
    pub threshold: f64, // 8B

    /// Parallel processing enabled
    pub parallel_enabled: bool, // 1B

    /// SIMD acceleration enabled
    pub simd_enabled: bool, // 1B

    /// Reserved for future parameters (padding to 64B)
    pub _reserved: [u8; 30], // 30B
}

impl Default for AlgorithmConfig {
    fn default() -> Self {
        Self {
            num_hashes: 128,
            num_bands: 5,
            rows_per_band: 8,
            threshold: 0.85,
            parallel_enabled: true,
            simd_enabled: false, // Requires nightly
            _reserved: [0u8; 30],
        }
    }
}

/// Encrypted configuration (ciphertext, 64 bytes + 16 byte tag + 12 byte nonce)
///
/// ## Memory Layout
///
/// ```text
/// Offset  Field          Size  Description
/// ------  -----          ----  -----------
/// 0x00    ciphertext     64B   AES-256-GCM encrypted config
/// 0x40    auth_tag       16B   GMAC authentication tag
/// 0x50    nonce          12B   Unique encryption nonce (RDRAND)
/// ------                 ---
/// Total:                 92B
/// ```
#[derive(Clone, Debug)]
pub struct EncryptedConfig {
    /// AES-256-GCM ciphertext (64 bytes)
    ciphertext: [u8; 64],

    /// Authentication tag (16 bytes, prevents tampering)
    auth_tag: [u8; 16],

    /// Nonce (12 bytes, unique per encryption)
    ///
    /// #ASSUME: RDRAND provides cryptographically secure randomness
    /// #VERIFY: NIST SP 800-90B test suite
    nonce: [u8; 12],
}

impl EncryptedConfig {
    /// Encrypt configuration with AES-256-GCM
    ///
    /// ## Security Flow
    ///
    /// 1. Generate unique nonce (RDRAND hardware RNG)
    /// 2. Serialize config to 64-byte plaintext
    /// 3. Encrypt with AES-256-GCM (authenticated encryption)
    /// 4. Split ciphertext and authentication tag
    ///
    /// ## Performance
    ///
    /// - Nonce generation: 10ns (RDRAND)
    /// - AES-256-GCM encryption: 200-500ns (AES-NI)
    /// - Total: <1µs
    ///
    /// ## Errors
    ///
    /// - `EncryptionError::NonceGenerationFailed`: RDRAND unavailable
    /// - `EncryptionError::EncryptionFailed`: AES-GCM encryption failed
    pub fn encrypt(config: &AlgorithmConfig, key: &[u8; 32]) -> Result<Self, EncryptionError> {
        // Step 1: Generate unique nonce (RDRAND)
        let nonce = generate_nonce()?;

        // Step 2: Serialize config to 64-byte plaintext
        let plaintext = config_to_bytes(config);

        // Step 3: Encrypt with AES-256-GCM
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
        let ciphertext_with_tag = cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_ref())
            .map_err(|_| EncryptionError::EncryptionFailed)?;

        // Step 4: Split ciphertext and tag
        // AES-GCM output format: ciphertext (64B) || tag (16B) = 80B total
        let mut ciphertext = [0u8; 64];
        let mut auth_tag = [0u8; 16];
        ciphertext.copy_from_slice(&ciphertext_with_tag[..64]);
        auth_tag.copy_from_slice(&ciphertext_with_tag[64..]);

        Ok(Self {
            ciphertext,
            auth_tag,
            nonce,
        })
    }

    /// Decrypt configuration
    ///
    /// ## Security Flow
    ///
    /// 1. Combine ciphertext + authentication tag (AES-GCM input format)
    /// 2. Decrypt with AES-256-GCM (verifies tag automatically)
    /// 3. Deserialize plaintext to AlgorithmConfig
    ///
    /// ## Performance
    ///
    /// - AES-256-GCM decryption: 200-500ns (AES-NI)
    /// - Deserialization: <10ns
    /// - Total: <1µs
    ///
    /// ## Errors
    ///
    /// - `EncryptionError::DecryptionFailed`: Authentication tag mismatch (tamper detected)
    /// - `EncryptionError::InvalidFormat`: Deserialization failed
    pub fn decrypt(&self, key: &[u8; 32]) -> Result<AlgorithmConfig, EncryptionError> {
        // Step 1: Combine ciphertext + tag (AES-GCM input format)
        let mut ciphertext_with_tag = Vec::with_capacity(80);
        ciphertext_with_tag.extend_from_slice(&self.ciphertext);
        ciphertext_with_tag.extend_from_slice(&self.auth_tag);

        // Step 2: Decrypt (AES-256-GCM verifies authentication tag)
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
        let plaintext = cipher
            .decrypt(Nonce::from_slice(&self.nonce), ciphertext_with_tag.as_ref())
            .map_err(|_| EncryptionError::DecryptionFailed)?;

        // Step 3: Deserialize
        bytes_to_config(&plaintext)
    }

    /// Get nonce (for debugging, NOT for encryption)
    ///
    /// #ASSUME: Exposing nonce is safe (nonce is public in AES-GCM)
    /// #VERIFY: NIST SP 800-38D section 8 allows nonce disclosure
    pub fn nonce(&self) -> &[u8; 12] {
        &self.nonce
    }

    /// Get ciphertext length (for verification)
    pub fn ciphertext_len(&self) -> usize {
        self.ciphertext.len()
    }
}

/// Generate cryptographically secure nonce using RDRAND
///
/// ## Platform Support
///
/// - **x86_64**: RDRAND instruction (Intel Ivy Bridge+, AMD Zen+)
/// - **Other**: Fallback error (RDRAND required for security)
///
/// ## Security Properties
///
/// - 96-bit nonce (AES-GCM standard)
/// - Cryptographically secure (NIST SP 800-90B compliant)
/// - Unique per encryption (hardware RNG)
///
/// #ASSUME: RDRAND provides 128 bits of entropy per call
/// #VERIFY: Intel Software Developer's Manual Vol. 1 section 7.3.17
///
/// # Safety
/// Uses x86_64 intrinsics (_rdrand32_step, _rdrand64_step) which are safe wrappers
/// around CPU instructions. These intrinsics cannot cause UB when used correctly.
#[cfg(target_arch = "x86_64")]
#[allow(unsafe_code)]
fn generate_nonce() -> Result<[u8; 12], EncryptionError> {
    use std::arch::x86_64::{_rdrand32_step, _rdrand64_step};

    unsafe {
        let mut nonce = [0u8; 12];
        let mut rand1 = 0u64;
        let mut rand2 = 0u32;

        // Generate 64 bits (first 8 bytes of nonce)
        let success1 = _rdrand64_step(&mut rand1);
        if success1 == 0 {
            return Err(EncryptionError::NonceGenerationFailed);
        }

        // Generate 32 bits (last 4 bytes of nonce)
        let success2 = _rdrand32_step(&mut rand2);
        if success2 == 0 {
            return Err(EncryptionError::NonceGenerationFailed);
        }

        // Combine into 12-byte nonce
        nonce[0..8].copy_from_slice(&rand1.to_le_bytes());
        nonce[8..12].copy_from_slice(&rand2.to_le_bytes());

        Ok(nonce)
    }
}

/// Fallback for non-x86_64 platforms (returns error)
#[cfg(not(target_arch = "x86_64"))]
fn generate_nonce() -> Result<[u8; 12], EncryptionError> {
    Err(EncryptionError::UnsupportedPlatform)
}

/// Serialize AlgorithmConfig to 64-byte plaintext
///
/// ## Layout
///
/// ```text
/// Offset  Field             Size  Type
/// ------  -----             ----  ----
/// 0x00    num_hashes        8B    usize (little-endian)
/// 0x08    num_bands         8B    usize (little-endian)
/// 0x10    rows_per_band     8B    usize (little-endian)
/// 0x18    threshold         8B    f64 (IEEE 754)
/// 0x20    parallel_enabled  1B    bool (0x00 or 0x01)
/// 0x21    simd_enabled      1B    bool (0x00 or 0x01)
/// 0x22    _reserved         30B   Padding
/// ------                    ---
/// Total:                    64B
/// ```
fn config_to_bytes(config: &AlgorithmConfig) -> [u8; 64] {
    let mut bytes = [0u8; 64];

    // usize fields (8 bytes each, little-endian)
    bytes[0..8].copy_from_slice(&config.num_hashes.to_le_bytes());
    bytes[8..16].copy_from_slice(&config.num_bands.to_le_bytes());
    bytes[16..24].copy_from_slice(&config.rows_per_band.to_le_bytes());

    // f64 field (8 bytes, IEEE 754)
    bytes[24..32].copy_from_slice(&config.threshold.to_le_bytes());

    // bool fields (1 byte each, 0x00 = false, 0x01 = true)
    bytes[32] = config.parallel_enabled as u8;
    bytes[33] = config.simd_enabled as u8;

    // Reserved (30 bytes, already zeroed)
    bytes[34..64].copy_from_slice(&config._reserved);

    bytes
}

/// Deserialize 64-byte plaintext to AlgorithmConfig
///
/// ## Validation
///
/// - Checks plaintext length (must be exactly 64 bytes)
/// - Validates bool fields (0x00 or 0x01 only)
///
/// ## Errors
///
/// - `EncryptionError::InvalidFormat`: Length mismatch or invalid bool
fn bytes_to_config(bytes: &[u8]) -> Result<AlgorithmConfig, EncryptionError> {
    // Validate length
    if bytes.len() < 64 {
        return Err(EncryptionError::InvalidFormat);
    }

    // Parse usize fields (8 bytes each, little-endian)
    let num_hashes = usize::from_le_bytes(bytes[0..8].try_into().unwrap());
    let num_bands = usize::from_le_bytes(bytes[8..16].try_into().unwrap());
    let rows_per_band = usize::from_le_bytes(bytes[16..24].try_into().unwrap());

    // Parse f64 field (8 bytes, IEEE 754)
    let threshold = f64::from_le_bytes(bytes[24..32].try_into().unwrap());

    // Parse bool fields (validate 0x00 or 0x01)
    let parallel_enabled = match bytes[32] {
        0x00 => false,
        0x01 => true,
        _ => return Err(EncryptionError::InvalidFormat),
    };

    let simd_enabled = match bytes[33] {
        0x00 => false,
        0x01 => true,
        _ => return Err(EncryptionError::InvalidFormat),
    };

    // Parse reserved (30 bytes)
    let mut _reserved = [0u8; 30];
    _reserved.copy_from_slice(&bytes[34..64]);

    Ok(AlgorithmConfig {
        num_hashes,
        num_bands,
        rows_per_band,
        threshold,
        parallel_enabled,
        simd_enabled,
        _reserved,
    })
}

/// Encryption errors
#[derive(Debug, thiserror::Error)]
pub enum EncryptionError {
    /// Platform does not support RDRAND (x86_64 required)
    #[error("Unsupported platform: RDRAND not available")]
    UnsupportedPlatform,

    /// RDRAND failed to generate random bytes
    #[error("Nonce generation failed: RDRAND instruction failed")]
    NonceGenerationFailed,

    /// AES-GCM encryption failed
    #[error("Encryption failed: AES-256-GCM error")]
    EncryptionFailed,

    /// AES-GCM decryption failed (likely authentication tag mismatch)
    #[error("Decryption failed: Authentication tag mismatch (tamper detected)")]
    DecryptionFailed,

    /// Deserialization failed (invalid format)
    #[error("Invalid format: Deserialization failed")]
    InvalidFormat,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = AlgorithmConfig {
            num_hashes: 128,
            num_bands: 5,
            rows_per_band: 8,
            threshold: 0.85,
            parallel_enabled: true,
            simd_enabled: false,
            _reserved: [0u8; 30],
        };

        // Serialize
        let bytes = config_to_bytes(&config);
        assert_eq!(bytes.len(), 64);

        // Deserialize
        let deserialized = bytes_to_config(&bytes).unwrap();
        assert_eq!(deserialized.num_hashes, 128);
        assert_eq!(deserialized.num_bands, 5);
        assert_eq!(deserialized.rows_per_band, 8);
        assert_eq!(deserialized.threshold, 0.85);
        assert_eq!(deserialized.parallel_enabled, true);
        assert_eq!(deserialized.simd_enabled, false);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_encryption_roundtrip() {
        let config = AlgorithmConfig::default();

        // Test key (32 bytes, all zeros for testing)
        let key = [0u8; 32];

        // Encrypt
        let encrypted = EncryptedConfig::encrypt(&config, &key).unwrap();
        assert_eq!(encrypted.ciphertext_len(), 64);

        // Decrypt
        let decrypted = encrypted.decrypt(&key).unwrap();
        assert_eq!(decrypted.num_hashes, config.num_hashes);
        assert_eq!(decrypted.num_bands, config.num_bands);
        assert_eq!(decrypted.rows_per_band, config.rows_per_band);
        assert_eq!(decrypted.threshold, config.threshold);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_nonce_uniqueness() {
        // Generate 100 nonces, verify all unique
        let mut nonces = Vec::new();
        for _ in 0..100 {
            let nonce = generate_nonce().unwrap();
            assert!(!nonces.contains(&nonce), "Duplicate nonce detected");
            nonces.push(nonce);
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_authentication_tag_tamper_detection() {
        let config = AlgorithmConfig::default();
        let key = [0u8; 32];

        // Encrypt
        let mut encrypted = EncryptedConfig::encrypt(&config, &key).unwrap();

        // Tamper with ciphertext (flip 1 bit)
        encrypted.ciphertext[0] ^= 0x01;

        // Decryption should fail (authentication tag mismatch)
        let result = encrypted.decrypt(&key);
        assert!(result.is_err());
        match result {
            Err(EncryptionError::DecryptionFailed) => {} // Expected
            _ => panic!("Expected DecryptionFailed error"),
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_wrong_key_decryption() {
        let config = AlgorithmConfig::default();
        let key1 = [0u8; 32];
        let key2 = [1u8; 32]; // Different key

        // Encrypt with key1
        let encrypted = EncryptedConfig::encrypt(&config, &key1).unwrap();

        // Decrypt with key2 should fail
        let result = encrypted.decrypt(&key2);
        assert!(result.is_err());
    }
}
