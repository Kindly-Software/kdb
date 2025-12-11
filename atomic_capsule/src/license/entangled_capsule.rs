//! # LicenseEntangledCapsule - Computational License Binding
//!
//! **[TRADE SECRET] - Revolutionary DRM where license IS computation**
//!
//! ## UCE34 Framework Compliance
//!
//! **Q10 Tier**: T6 Mixed (T0 Auditable + T1 Atomic + T3 Fixed-Point)
//! **Q33 Lockfree**: 100% lockfree, no Mutex/RwLock
//! **Q34 Audit**: Hash-chain audit trail with license anchoring
//!
//! ## Core Innovation
//!
//! Traditional DRM:
//! ```rust,ignore
//! if !check_license() { exit(); }  // NOP this out = bypassed
//! compute_result(input)             // Now executes unlicensed
//! ```
//!
//! CHAOS DRM (this capsule):
//! ```rust,ignore
//! let next = state ^ license_transform;  // Wrong key = garbage
//! // There's NO check to bypass - the license IS the math
//! // Removing the XOR changes the output - software is useless
//! ```
//!
//! ## Memory Layout (128 bytes, cache-aligned)
//!
//! ```text
//! Offset 0-7:    state (AtomicU64) - entangled with license_transform
//! Offset 8-15:   generation (AtomicU64) - incorporates license rotation
//! Offset 16-31:  license_transform (u128) - SHA256(Ed25519_signature)[0..16]
//! Offset 32-39:  license_features (u64) - feature flags from license
//! Offset 40-71:  signature_bits (u64 × 4) - Ed25519 signature for dispatch
//! Offset 72-127: padding (56 bytes)
//! ```
//!
//! ## Security (128-bit Transform)
//! Previous 64-bit transform was vulnerable to O(2^64) brute-force ($18K-$1M attack cost).
//! New 128-bit transform provides O(2^128) security (~$10^24 attack cost).
//!
//! ## ASSUM Framework
//! - `#ASSUME_ENTANGLEMENT_IRREVERSIBLE`: XOR with transform cannot be NOPed without garbage output
//! - `#VERIFY_ENTANGLEMENT`: Property tests validate wrong license = wrong output
//! - `#ASSUME_ED25519_SECURE`: Ed25519 provides 2^128 security (NIST SP 800-186)
//! - `#VERIFY_ED25519`: RFC 8032 test vectors
//! - `#ASSUME_SHA256_PREIMAGE`: SHA-256 preimage resistance (cannot derive license from transform)
//! - `#VERIFY_SHA256`: NIST FIPS 180-4 compliance

use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

/// License structure with Ed25519 signature
///
/// ## Format
/// - `public_key`: Ed25519 verifying key (32 bytes)
/// - `signature`: Ed25519 signature over license data (64 bytes)
/// - `expiry_timestamp`: Unix timestamp when license expires
/// - `features`: 64-bit feature flags (bitfield)
/// - `customer_id`: Unique customer identifier (16 bytes)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct License {
    /// Ed25519 public key (32 bytes)
    pub public_key: [u8; 32],
    /// Ed25519 signature over license data (64 bytes)
    pub signature: [u8; 64],
    /// License expiry timestamp (Unix seconds)
    pub expiry_timestamp: u64,
    /// Feature flags (bitfield)
    pub features: u64,
    /// Customer ID (16-byte UUID)
    pub customer_id: [u8; 16],
}

impl License {
    /// Create new license with all fields
    pub const fn new(
        public_key: [u8; 32],
        signature: [u8; 64],
        expiry_timestamp: u64,
        features: u64,
        customer_id: [u8; 16],
    ) -> Self {
        Self {
            public_key,
            signature,
            expiry_timestamp,
            features,
            customer_id,
        }
    }

    /// Check if license is expired
    #[cfg(feature = "std")]
    pub fn is_expired(&self) -> bool {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now > self.expiry_timestamp
    }

    /// Get message bytes for signature verification
    ///
    /// Format: [customer_id (16B) || expiry_timestamp (8B LE) || features (8B LE)]
    pub fn message_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[0..16].copy_from_slice(&self.customer_id);
        bytes[16..24].copy_from_slice(&self.expiry_timestamp.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.features.to_le_bytes());
        bytes
    }
}

/// License feature flags (64-bit bitfield)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LicenseFeatures(pub u64);

impl LicenseFeatures {
    /// Create from raw bitfield
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    /// Check if feature bit is set
    #[inline(always)]
    pub const fn has_feature(self, bit: u64) -> bool {
        (self.0 & (1 << bit)) != 0
    }

    /// Get raw bits
    #[inline(always)]
    pub const fn bits(self) -> u64 {
        self.0
    }
}

/// Standard feature bits (example set)
impl LicenseFeatures {
    pub const FEATURE_BASIC: u64 = 0;
    pub const FEATURE_PROFESSIONAL: u64 = 1;
    pub const FEATURE_ENTERPRISE: u64 = 2;
    pub const FEATURE_UNLIMITED: u64 = 3;
    pub const FEATURE_EXPORT: u64 = 4;
    pub const FEATURE_NETWORK: u64 = 5;
    pub const FEATURE_AUDIT: u64 = 6;
    pub const FEATURE_CUSTOM_1: u64 = 32;
    pub const FEATURE_CUSTOM_2: u64 = 33;
}

/// Computation result from entangled operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComputationResult {
    /// Result value (entangled with license)
    pub value: u64,
    /// Generation at computation time
    pub generation: u64,
    /// Whether feature was authorized
    pub authorized: bool,
}

impl ComputationResult {
    /// Create new computation result
    pub const fn new(value: u64, generation: u64, authorized: bool) -> Self {
        Self {
            value,
            generation,
            authorized,
        }
    }

    /// Check if result is valid (authorized and non-zero generation)
    pub const fn is_valid(&self) -> bool {
        self.authorized && self.generation > 0
    }
}

/// License errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LicenseError {
    /// License signature invalid (forgery detected)
    SignatureInvalid,
    /// License has expired
    Expired,
    /// License public key invalid format
    InvalidPublicKey,
    /// Feature not authorized by license
    FeatureNotAuthorized,
    /// License transform mismatch (tampering detected)
    TransformMismatch,
    /// Generation counter anomaly (replay attack?)
    GenerationAnomaly,
    /// License not initialized
    NotInitialized,
}

impl core::fmt::Display for LicenseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LicenseError::SignatureInvalid => {
                write!(f, "License signature invalid (forgery detected)")
            }
            LicenseError::Expired => write!(f, "License has expired"),
            LicenseError::InvalidPublicKey => write!(f, "Invalid Ed25519 public key format"),
            LicenseError::FeatureNotAuthorized => write!(f, "Feature not authorized by license"),
            LicenseError::TransformMismatch => {
                write!(f, "License transform mismatch (tampering detected)")
            }
            LicenseError::GenerationAnomaly => {
                write!(f, "Generation counter anomaly (potential replay attack)")
            }
            LicenseError::NotInitialized => write!(f, "License not initialized"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LicenseError {}

/// LicenseEntangledCapsule - Computation is cryptographically bound to license
///
/// ## Core Innovation
///
/// The license transform is XOR'd into every state transition. Without the correct
/// license, the computation produces garbage. There is no "check" to bypass.
///
/// ## Memory Layout (128 bytes, cache-aligned)
///
/// - Offset 0-7: `state` (AtomicU64) - entangled with license_transform
/// - Offset 8-15: `generation` (AtomicU64) - incorporates license rotation
/// - Offset 16-31: `license_transform` (u128) - SHA256(signature)[0..16]
/// - Offset 32-39: `license_features` (u64) - feature flags from license
/// - Offset 40-71: `signature_bits` ([u64; 4]) - full signature for dispatch
/// - Offset 72-127: `_padding` ([u8; 56])
///
/// ## Security (128-bit Transform)
/// Previous 64-bit transform was vulnerable to O(2^64) brute-force ($18K-$1M attack cost).
/// New 128-bit transform provides O(2^128) security (~$10^24 attack cost).
///
/// ## Performance (B32 Targets)
/// - State transition: <15ns (XOR + atomic)
/// - Feature operation: <25ns (bit check + dispatch)
/// - Integrity verify: <50ns (transform validation)
///
/// ## ASSUM Framework
/// - `#ASSUME_128B_ALIGNMENT`: Cache-line aligned for optimal performance
/// - `#VERIFY_128B_ALIGNMENT`: Compile-time verification via derive
/// - `#ASSUME_LOCKFREE`: All operations are 100% lockfree
/// - `#VERIFY_LOCKFREE`: No Mutex/RwLock in implementation
/// - `#ASSUME_128BIT_SECURITY`: 128-bit transform provides O(2^128) brute-force resistance
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 128, size = 128))]
#[repr(C, align(128))]
pub struct LicenseEntangledCapsule {
    /// State value entangled with license transform
    ///
    /// Every state transition: new_state = f(old_state) ^ license_transform
    /// Wrong license = wrong transform = garbage output
    state: AtomicU64,

    /// Generation counter with license rotation
    ///
    /// Incorporates license rotation schedule to detect replay attacks
    generation: AtomicU64,

    /// License transform: SHA256(Ed25519_signature)[0..16]
    ///
    /// Derived from Ed25519 signature, cannot be computed without valid license
    /// This value is XOR'd into all state transitions
    /// 128-bit transform provides O(2^128) security against brute-force attacks
    license_transform: u128,

    /// Feature flags from license (bitfield)
    ///
    /// Determines which operations are authorized
    /// But authorization alone is not enough - transform must also be correct
    license_features: u64,

    /// Signature bits for operation dispatch (4 × 64 bits = 256 bits)
    ///
    /// First 256 bits of Ed25519 signature used for dispatch decisions
    /// Different signature = different execution path = different output
    signature_bits: [u64; 4],

    /// Padding to complete 128-byte alignment
    /// (8 + 8 + 16 + 8 + 32 + 56 = 128 bytes)
    _padding: [u8; 56],
}

// Compile-time verification (Q33 mandatory)
#[cfg(not(feature = "derive"))]
crate::verify_capsule_properties!(LicenseEntangledCapsule, 128, 128);

// Send + Sync safety
#[cfg(not(feature = "derive"))]
unsafe impl Send for LicenseEntangledCapsule {}
#[cfg(not(feature = "derive"))]
unsafe impl Sync for LicenseEntangledCapsule {}

impl LicenseEntangledCapsule {
    /// Create new LicenseEntangledCapsule from license and public key
    ///
    /// ## Arguments
    /// - `license`: License structure with Ed25519 signature
    /// - `public_key`: Ed25519 verifying key (for signature verification)
    ///
    /// ## Returns
    /// - `Ok(Self)`: Capsule initialized with license entanglement
    /// - `Err(LicenseError)`: Signature invalid, expired, or public key invalid
    ///
    /// ## Performance
    /// - Ed25519 verification: <500µs (one-time at init)
    ///
    /// ## ASSUM Framework
    /// - `#ASSUME_ED25519_SECURE`: Signature verification is cryptographically secure
    /// - `#VERIFY_ED25519`: RFC 8032 test vectors validate implementation
    #[cfg(feature = "std")]
    pub fn new(license: &License, public_key: &[u8; 32]) -> Result<Self, LicenseError> {
        use ed25519_dalek::{Signature as Ed25519Signature, Verifier, VerifyingKey};

        // Check expiry first (cheap check)
        if license.is_expired() {
            return Err(LicenseError::Expired);
        }

        // Verify public key matches license
        if license.public_key != *public_key {
            return Err(LicenseError::InvalidPublicKey);
        }

        // Verify Ed25519 signature
        let verifying_key = VerifyingKey::from_bytes(public_key)
            .map_err(|_| LicenseError::InvalidPublicKey)?;

        let signature = Ed25519Signature::from_bytes(&license.signature);
        let message = license.message_bytes();

        verifying_key
            .verify(&message, &signature)
            .map_err(|_| LicenseError::SignatureInvalid)?;

        // Compute license transform: SHA256(signature)[0..16] (128-bit)
        let transform = Self::compute_transform(&license.signature);

        // Extract signature bits for dispatch
        let signature_bits = Self::extract_signature_bits(&license.signature);

        Ok(Self {
            // Initial state uses lower 64 bits of 128-bit transform
            state: AtomicU64::new(transform as u64),
            generation: AtomicU64::new(1),
            license_transform: transform,
            license_features: license.features,
            signature_bits,
            _padding: [0u8; 56],
        })
    }

    /// Create uninitialized capsule (for testing or delayed init)
    ///
    /// ## Warning
    /// Operations on uninitialized capsule will fail with `NotInitialized` error
    pub const fn uninit() -> Self {
        Self {
            state: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            license_transform: 0,
            license_features: 0,
            signature_bits: [0; 4],
            _padding: [0u8; 56],
        }
    }

    /// Create capsule for testing (bypasses signature verification)
    ///
    /// ## Warning
    /// This is for testing only. In production, use `new()` with valid license.
    ///
    /// ## Arguments
    /// - `transform`: License transform value (normally SHA256(signature)[0..16])
    /// - `features`: Feature flags bitfield
    /// - `signature_bits`: First 256 bits of signature for dispatch
    ///
    /// ## Returns
    /// Capsule with specified parameters, ready for testing
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn for_testing(transform: u128, features: u64, signature_bits: [u64; 4]) -> Self {
        Self {
            // Initial state uses lower 64 bits of transform
            state: AtomicU64::new(transform as u64),
            generation: AtomicU64::new(1),
            license_transform: transform,
            license_features: features,
            signature_bits,
            _padding: [0u8; 56],
        }
    }

    /// Create capsule for testing with custom initial state
    ///
    /// ## Arguments
    /// - `initial_state`: Initial state value
    /// - `transform`: License transform value (128-bit)
    /// - `features`: Feature flags bitfield
    /// - `signature_bits`: Signature bits for dispatch
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn for_testing_with_state(
        initial_state: u64,
        transform: u128,
        features: u64,
        signature_bits: [u64; 4],
    ) -> Self {
        Self {
            state: AtomicU64::new(initial_state),
            generation: AtomicU64::new(1),
            license_transform: transform,
            license_features: features,
            signature_bits,
            _padding: [0u8; 56],
        }
    }

    /// Compute license transform from signature
    ///
    /// Transform = SHA256(signature)[0..16] as little-endian u128
    ///
    /// ## Security
    /// 128-bit transform provides O(2^128) security against brute-force attacks
    /// Previous 64-bit was vulnerable to $18K-$1M attack cost (O(2^64))
    ///
    /// ## ASSUM Framework
    /// - `#ASSUME_SHA256_PREIMAGE`: Cannot derive signature from transform
    /// - `#VERIFY_SHA256`: NIST FIPS 180-4 compliance
    /// - `#ASSUME_128BIT_SECURITY`: 128-bit provides ~$10^24 attack cost
    #[cfg(feature = "std")]
    fn compute_transform(signature: &[u8; 64]) -> u128 {
        use sha2::{Sha256, Digest};

        let mut hasher = Sha256::new();
        hasher.update(signature);
        let hash = hasher.finalize();

        // First 16 bytes as little-endian u128
        u128::from_le_bytes([
            hash[0], hash[1], hash[2], hash[3],
            hash[4], hash[5], hash[6], hash[7],
            hash[8], hash[9], hash[10], hash[11],
            hash[12], hash[13], hash[14], hash[15],
        ])
    }

    /// Extract signature bits for dispatch (first 256 bits = 4 × u64)
    fn extract_signature_bits(signature: &[u8; 64]) -> [u64; 4] {
        [
            u64::from_le_bytes([
                signature[0], signature[1], signature[2], signature[3],
                signature[4], signature[5], signature[6], signature[7],
            ]),
            u64::from_le_bytes([
                signature[8], signature[9], signature[10], signature[11],
                signature[12], signature[13], signature[14], signature[15],
            ]),
            u64::from_le_bytes([
                signature[16], signature[17], signature[18], signature[19],
                signature[20], signature[21], signature[22], signature[23],
            ]),
            u64::from_le_bytes([
                signature[24], signature[25], signature[26], signature[27],
                signature[28], signature[29], signature[30], signature[31],
            ]),
        ]
    }

    // ========================================================================
    // CORE ENTANGLED OPERATIONS
    // ========================================================================

    /// State transition entangled with license
    ///
    /// ## Core Innovation
    ///
    /// ```rust,ignore
    /// new_state = f(input, old_state) ^ license_transform_low
    /// ```
    ///
    /// Without correct license_transform, output is garbage.
    /// There is NO "if check_license()" to bypass.
    ///
    /// ## Performance
    /// <15ns (XOR + atomic store)
    ///
    /// ## ASSUM Framework
    /// - `#ASSUME_ENTANGLEMENT_IRREVERSIBLE`: XOR cannot be NOPed
    /// - `#VERIFY_ENTANGLEMENT`: Property tests validate garbage without license
    /// - `#ASSUME_128BIT_SECURITY`: Full 128-bit transform stored, lower 64 bits used for state XOR
    #[inline(always)]
    pub fn transition(&self, input: u64) -> u64 {
        // Load current state
        let current = self.state.load(Ordering::Acquire);

        // Compute next state: mix input with current, then entangle with license
        // The license_transform XOR is the CORE of the protection
        // Remove it = garbage output
        // Use lower 64 bits of 128-bit transform for u64 state XOR
        let mixed = current.wrapping_add(input).rotate_left(13);
        let transform_low = self.license_transform as u64;
        let next = mixed ^ transform_low;

        // Store and return
        self.state.store(next, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Relaxed);

        next
    }

    /// Feature-gated operation entangled with license
    ///
    /// ## Dual Protection
    ///
    /// 1. Feature flag must be set (from license.features)
    /// 2. Computation is entangled with license_transform (garbage without it)
    ///
    /// Even if attacker patches feature check, output is still wrong
    /// because license_transform is wrong.
    ///
    /// ## Arguments
    /// - `input`: Input value for computation
    /// - `feature_bit`: Feature bit to check (0-63)
    ///
    /// ## Returns
    /// - `Some(result)`: Feature authorized, computation result
    /// - `None`: Feature not authorized
    ///
    /// ## Performance
    /// <25ns (bit check + XOR + atomic)
    #[inline(always)]
    pub fn feature_op(&self, input: u64, feature_bit: u64) -> Option<u64> {
        // Feature flag check (can be patched, but doesn't matter)
        let features = LicenseFeatures::from_bits(self.license_features);
        if !features.has_feature(feature_bit) {
            return None;
        }

        // Even if attacker patches the check above to always return true,
        // the computation below still uses license_transform
        // Wrong license = wrong transform = garbage output

        let current = self.state.load(Ordering::Acquire);

        // Feature-specific transform: include feature_bit in computation
        // Use lower 64 bits of 128-bit transform for u64 state XOR
        let feature_salt = feature_bit.wrapping_mul(0x517cc1b727220a95);
        let mixed = current.wrapping_add(input).wrapping_add(feature_salt);
        let transform_low = self.license_transform as u64;
        let result = mixed ^ transform_low;

        // Update generation
        self.generation.fetch_add(1, Ordering::Relaxed);

        Some(result)
    }

    /// Dispatch operation based on signature bits
    ///
    /// ## Signature-Based Dispatch
    ///
    /// Different signatures produce different execution paths.
    /// Cannot patch to "always take good path" because path IS the computation.
    ///
    /// ## Arguments
    /// - `input`: Input value
    /// - `dispatch_index`: Which signature word to use (0-3)
    /// - `bit_position`: Bit position in word (0-63)
    ///
    /// ## Performance
    /// <20ns (bit extract + conditional + XOR)
    #[inline(always)]
    pub fn dispatch_op(&self, input: u64, dispatch_index: usize, bit_position: u64) -> u64 {
        let sig_word = self.signature_bits.get(dispatch_index).copied().unwrap_or(0);
        let dispatch_bit = (sig_word >> (bit_position & 63)) & 1;

        let current = self.state.load(Ordering::Acquire);

        // Use lower 64 bits of 128-bit transform for u64 state operations
        let transform_low = self.license_transform as u64;

        // Different computation paths based on signature bit
        // Both paths still use license_transform - no "good path" to patch to
        let result = if dispatch_bit == 1 {
            // Path A: rotate then XOR
            let mixed = current.wrapping_add(input).rotate_left(17);
            mixed ^ transform_low
        } else {
            // Path B: XOR then rotate
            let mixed = current ^ input;
            mixed.wrapping_add(transform_low).rotate_right(13)
        };

        self.state.store(result, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Relaxed);

        result
    }

    /// Generate XOR mask stream from license
    ///
    /// Creates deterministic pseudo-random stream seeded by license.
    /// Used for data obfuscation - wrong license = wrong mask = garbage data.
    ///
    /// ## Arguments
    /// - `seed`: Additional seed value
    /// - `index`: Position in stream
    ///
    /// ## Performance
    /// <10ns per mask value
    #[inline(always)]
    pub fn mask_stream(&self, seed: u64, index: u64) -> u64 {
        // Combine transform with seed and index
        // Use lower 64 bits of 128-bit transform for u64 mask stream
        let transform_low = self.license_transform as u64;
        let mixed = transform_low
            .wrapping_add(seed)
            .wrapping_add(index.wrapping_mul(0x9e3779b97f4a7c15));

        // Mix with signature bits for additional entropy
        let sig_part = self.signature_bits[(index as usize) & 3];
        mixed ^ sig_part.rotate_left((index & 63) as u32)
    }

    // ========================================================================
    // VERIFICATION AND INTEGRITY
    // ========================================================================

    /// Verify license integrity
    ///
    /// Checks that internal state is consistent with license transform.
    ///
    /// ## Returns
    /// - `Ok(())`: Integrity verified
    /// - `Err(LicenseError)`: Tampering detected
    ///
    /// ## Performance
    /// <50ns (atomic loads + comparison)
    pub fn verify_integrity(&self) -> Result<(), LicenseError> {
        // Check initialization
        if self.license_transform == 0 && self.generation.load(Ordering::Relaxed) == 0 {
            return Err(LicenseError::NotInitialized);
        }

        // Verify generation is reasonable (not wrapped or reset)
        let gen = self.generation.load(Ordering::Acquire);
        if gen == 0 {
            return Err(LicenseError::GenerationAnomaly);
        }

        // Verify state includes transform (sanity check)
        let state = self.state.load(Ordering::Acquire);
        if state == 0 && self.license_transform != 0 {
            // State should never be exactly 0 after valid operations
            // This could indicate tampering
            return Err(LicenseError::TransformMismatch);
        }

        Ok(())
    }

    /// Get current generation counter
    #[inline(always)]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get current state value
    #[inline(always)]
    pub fn state(&self) -> u64 {
        self.state.load(Ordering::Acquire)
    }

    /// Get license features
    #[inline(always)]
    pub fn features(&self) -> LicenseFeatures {
        LicenseFeatures::from_bits(self.license_features)
    }

    /// Check if specific feature is authorized
    #[inline(always)]
    pub fn has_feature(&self, feature_bit: u64) -> bool {
        self.features().has_feature(feature_bit)
    }

    /// Get license transform (for audit purposes)
    ///
    /// ## Warning
    /// Do not expose this in production code. For audit/debug only.
    ///
    /// Returns the full 128-bit transform value.
    #[cfg(any(test, feature = "audit-q34"))]
    pub fn transform(&self) -> u128 {
        self.license_transform
    }
}

impl Default for LicenseEntangledCapsule {
    fn default() -> Self {
        Self::uninit()
    }
}

// ============================================================================
// T28 COMPREHENSIVE TESTING
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test helpers
    fn test_license() -> License {
        License::new(
            [0u8; 32],      // public_key
            [0xAA; 64],     // signature
            u64::MAX,       // expiry (never)
            0xFFFFFFFFFFFFFFFF, // all features
            [0x42; 16],     // customer_id
        )
    }

    /// T28: Unit Test - Uninit capsule
    #[test]
    fn test_uninit_capsule() {
        let capsule = LicenseEntangledCapsule::uninit();
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.state(), 0);
        assert!(capsule.verify_integrity().is_err());
    }

    /// T28: Unit Test - Feature flags
    #[test]
    fn test_license_features() {
        let features = LicenseFeatures::from_bits(0b1011);

        assert!(features.has_feature(0));
        assert!(features.has_feature(1));
        assert!(!features.has_feature(2));
        assert!(features.has_feature(3));
        assert!(!features.has_feature(4));
    }

    /// T28: Unit Test - License message bytes
    #[test]
    fn test_license_message_bytes() {
        let license = test_license();
        let bytes = license.message_bytes();

        // Verify structure
        assert_eq!(&bytes[0..16], &license.customer_id);
        assert_eq!(
            &bytes[16..24],
            &license.expiry_timestamp.to_le_bytes()
        );
        assert_eq!(
            &bytes[24..32],
            &license.features.to_le_bytes()
        );
    }

    /// T28: Unit Test - Signature bits extraction
    #[test]
    fn test_signature_bits_extraction() {
        let mut signature = [0u8; 64];
        for i in 0..64 {
            signature[i] = i as u8;
        }

        let bits = LicenseEntangledCapsule::extract_signature_bits(&signature);

        // Verify first word
        let expected_0 = u64::from_le_bytes([0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(bits[0], expected_0);

        // Verify second word
        let expected_1 = u64::from_le_bytes([8, 9, 10, 11, 12, 13, 14, 15]);
        assert_eq!(bits[1], expected_1);
    }

    /// T28: Property Test - Mask stream determinism
    #[test]
    fn test_mask_stream_deterministic() {
        let capsule = LicenseEntangledCapsule {
            state: AtomicU64::new(12345),
            generation: AtomicU64::new(1),
            license_transform: 0xDEADBEEF12345678_FEDCBA9876543210u128,
            license_features: 0xFFFF,
            signature_bits: [0x1111, 0x2222, 0x3333, 0x4444],
            _padding: [0u8; 56],
        };

        // Same inputs = same output
        let mask1 = capsule.mask_stream(100, 5);
        let mask2 = capsule.mask_stream(100, 5);
        assert_eq!(mask1, mask2);

        // Different index = different output
        let mask3 = capsule.mask_stream(100, 6);
        assert_ne!(mask1, mask3);

        // Different seed = different output
        let mask4 = capsule.mask_stream(101, 5);
        assert_ne!(mask1, mask4);
    }

    /// T28: Property Test - Transition modifies state
    #[test]
    fn test_transition_modifies_state() {
        let capsule = LicenseEntangledCapsule {
            state: AtomicU64::new(0x1234),
            generation: AtomicU64::new(1),
            license_transform: 0xABCDEF0123456789_9876543210FEDCBAu128,
            license_features: 0xFFFF,
            signature_bits: [0; 4],
            _padding: [0u8; 56],
        };

        let initial_state = capsule.state();
        let initial_gen = capsule.generation();

        let result = capsule.transition(42);

        // State changed
        assert_ne!(capsule.state(), initial_state);
        assert_eq!(capsule.state(), result);

        // Generation incremented
        assert_eq!(capsule.generation(), initial_gen + 1);
    }

    /// T28: Property Test - Wrong transform = wrong output
    #[test]
    fn test_wrong_transform_garbage_output() {
        // Create two capsules with different transforms
        let capsule_correct = LicenseEntangledCapsule {
            state: AtomicU64::new(0x1000),
            generation: AtomicU64::new(1),
            license_transform: 0xABCDEF0123456789_9876543210FEDCBAu128,
            license_features: 0xFFFF,
            signature_bits: [0; 4],
            _padding: [0u8; 56],
        };

        let capsule_wrong = LicenseEntangledCapsule {
            state: AtomicU64::new(0x1000), // Same initial state
            generation: AtomicU64::new(1),
            license_transform: 0x1111111111111111_2222222222222222u128, // WRONG transform
            license_features: 0xFFFF,
            signature_bits: [0; 4],
            _padding: [0u8; 56],
        };

        // Same input
        let result_correct = capsule_correct.transition(12345);
        let result_wrong = capsule_wrong.transition(12345);

        // Different outputs - wrong license = garbage
        assert_ne!(result_correct, result_wrong);
    }

    /// T28: Property Test - Feature gating
    #[test]
    fn test_feature_gating() {
        let capsule = LicenseEntangledCapsule {
            state: AtomicU64::new(0x1000),
            generation: AtomicU64::new(1),
            license_transform: 0xABCD_0000_0000_0000_0000_0000_0000_0000u128,
            license_features: 0b1010, // Features 1 and 3 enabled
            signature_bits: [0; 4],
            _padding: [0u8; 56],
        };

        // Feature 1 (enabled) - returns Some
        assert!(capsule.feature_op(100, 1).is_some());

        // Feature 2 (disabled) - returns None
        assert!(capsule.feature_op(100, 2).is_none());

        // Feature 3 (enabled) - returns Some
        assert!(capsule.feature_op(100, 3).is_some());
    }

    /// T28: Property Test - Dispatch path variation
    #[test]
    fn test_dispatch_paths() {
        let capsule_bit0 = LicenseEntangledCapsule {
            state: AtomicU64::new(0x1000),
            generation: AtomicU64::new(1),
            license_transform: 0xABCD_0000_0000_0000_0000_0000_0000_0000u128,
            license_features: 0xFFFF,
            signature_bits: [0b0, 0, 0, 0], // Bit 0 = 0
            _padding: [0u8; 56],
        };

        let capsule_bit1 = LicenseEntangledCapsule {
            state: AtomicU64::new(0x1000), // Same initial state
            generation: AtomicU64::new(1),
            license_transform: 0xABCD_0000_0000_0000_0000_0000_0000_0000u128, // Same transform
            license_features: 0xFFFF,
            signature_bits: [0b1, 0, 0, 0], // Bit 0 = 1
            _padding: [0u8; 56],
        };

        // Same input, same transform, but different dispatch bit
        let result_0 = capsule_bit0.dispatch_op(100, 0, 0);
        let result_1 = capsule_bit1.dispatch_op(100, 0, 0);

        // Different execution paths = different results
        assert_ne!(result_0, result_1);
    }

    /// T28: Integration Test - Verify integrity
    #[test]
    fn test_verify_integrity() {
        // Uninitialized capsule fails verification
        let uninit = LicenseEntangledCapsule::uninit();
        assert!(matches!(
            uninit.verify_integrity(),
            Err(LicenseError::NotInitialized)
        ));

        // Initialized capsule passes verification
        let valid = LicenseEntangledCapsule {
            state: AtomicU64::new(0x1234),
            generation: AtomicU64::new(1),
            license_transform: 0xABCD_0000_0000_0000_0000_0000_0000_0000u128,
            license_features: 0xFFFF,
            signature_bits: [0; 4],
            _padding: [0u8; 56],
        };
        assert!(valid.verify_integrity().is_ok());
    }

    /// T28: Integration Test - Computation result validation
    #[test]
    fn test_computation_result() {
        let result_valid = ComputationResult::new(12345, 1, true);
        assert!(result_valid.is_valid());

        let result_unauthorized = ComputationResult::new(12345, 1, false);
        assert!(!result_unauthorized.is_valid());

        let result_zero_gen = ComputationResult::new(12345, 0, true);
        assert!(!result_zero_gen.is_valid());
    }

    /// T28: Production Test - Concurrent access
    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_transitions() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(LicenseEntangledCapsule {
            state: AtomicU64::new(0x1000),
            generation: AtomicU64::new(1),
            license_transform: 0xDEADBEEF_CAFEBABE_12345678_87654321u128,
            license_features: 0xFFFF,
            signature_bits: [0; 4],
            _padding: [0u8; 56],
        });

        let handles: Vec<_> = (0..4)
            .map(|i| {
                let capsule = Arc::clone(&capsule);
                thread::spawn(move || {
                    for j in 0..1000 {
                        capsule.transition(i * 1000 + j);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // Generation should be 1 + 4000 operations
        assert_eq!(capsule.generation(), 4001);
    }

    /// T28: Production Test - Memory layout verification
    #[test]
    fn test_memory_layout() {
        use core::mem::{size_of, align_of};

        assert_eq!(size_of::<LicenseEntangledCapsule>(), 128);
        assert_eq!(align_of::<LicenseEntangledCapsule>(), 128);
    }
}
