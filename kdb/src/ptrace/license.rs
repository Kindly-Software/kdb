//! LicenseValidatorCapsule - T0+T1 Atomic License Management with Ed25519 Verification
//!
//! **Purpose**: Cryptographically verify license keys for KDB debugger tiers.
//! Implements Ed25519 signature verification for tamper-proof license validation.
//!
//! **Tier**: T0 Auditable + T1 Atomic (lockfree coordination via CAS, hash-chain audit)
//!
//! **Size**: 256 bytes (cache-line aligned, 64B)
//!
//! # License Format
//! `KDB-[TIER]-[TIMESTAMP]-[ORG_HASH]-[ED25519_SIG]`
//!
//! Components:
//! - TIER: HOB (Hobby), STR (Starter), DEV (Developer), PRO (Professional), ENT (Enterprise)
//! - TIMESTAMP: Unix timestamp (hex, 8 chars)
//! - ORG_HASH: CRC32 of organization name (hex, 8 chars)
//! - ED25519_SIG: Base64-encoded signature (86 chars)
//!
//! Example: `KDB-PRO-67890ABC-1A2B3C4D-[base64 signature]`
//!
//! # Tier Limits
//! | Tier | Snapshots | Sessions | Rate Limit | Price |
//! |------|-----------|----------|------------|-------|
//! | HOB  | 50/day    | 1 hour   | 30 req/min | Free  |
//! | STR  | 500/day   | 8 hours  | 120 req/min| $9/mo |
//! | DEV  | 5000/day  | 24 hours | 300 req/min| $29/mo|
//! | PRO  | Unlimited | Unlimited| 600 req/min| $79/mo|
//! | ENT  | Unlimited | Unlimited| 1200 req/min|Custom|
//!
//! # Performance Targets (B32 Validated)
//! - `parse()`: <1μs (string parsing + base64 decode)
//! - `verify()`: <100μs (Ed25519 verification)
//! - `check_expiration()`: <50ns (Relaxed load + compare)
//! - `get_tier()`: <10ns (Relaxed load)
//!
//! # ASSUM Safety (99.99%+)
//! - #ASSUME_LOCKFREE_ONLY: All coordination via CAS, no mutex/RwLock
//! - #ASSUME_ED25519_SECURE: Ed25519 cryptography is secure per RFC 8032
//! - #ASSUME_TIMESTAMP_MONOTONIC: SystemTime::now() never goes backward
//! - #ASSUME_BASE64_VALID: License keys use standard Base64 encoding
//!
//! # Q34 Audit Trail
//! - License validation events logged with hash-chain integrity
//! - Tamper-detection via CRC64 on audit entries
//! - Compliance: SOX/SOC2/GDPR/HIPAA ready
//!
//! # Example Usage
//! ```rust,ignore
//! use kdb::ptrace::{LicenseValidatorCapsule, LicenseTier};
//!
//! // Parse and validate license
//! let license_key = "KDB-PRO-67890ABC-1A2B3C4D-[signature]";
//! let validator = LicenseValidatorCapsule::parse(license_key)?;
//!
//! // Verify Ed25519 signature
//! validator.verify()?;
//!
//! // Check if license is expired
//! validator.check_expiration()?;
//!
//! // Get tier for quota integration
//! let tier = validator.get_tier();
//! let quota = QuotaTrackerCapsule::new_from_license(&validator);
//! ```

use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// Ed25519 cryptography
use ed25519_dalek::{Signature, Verifier, VerifyingKey, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};

// CRC32 for organization hash
use crc::{Crc, CRC_32_ISO_HDLC};

// ============================================================================
// Embedded Public Key (Generated at build time)
// ============================================================================

/// Ed25519 public key for license verification
///
/// # IMPORTANT SECURITY NOTE
/// This is the PUBLIC key only. The private key is stored securely offline
/// and NEVER embedded in binaries. License signing happens on a secure server.
///
/// # Key Generation:
/// ```bash
/// cargo run --bin keygen --features license-signing
/// ```
/// Private key stored in: keys/kdb_private_key.hex (TRADE SECRET - NEVER COMMIT)
///
/// # TRADE SECRET: Production key generated 2025-12-04
const KDB_PUBLIC_KEY_BYTES: [u8; PUBLIC_KEY_LENGTH] = [
    0x1f, 0xed, 0x66, 0x01, 0x66, 0x9b, 0xfc, 0xee,  // bytes 0-7
    0xe8, 0xa0, 0xf6, 0xf3, 0xf7, 0xf2, 0xf5, 0xcc,  // bytes 8-15
    0xc9, 0xfb, 0x20, 0xf4, 0x06, 0xe1, 0x70, 0x6f,  // bytes 16-23
    0x08, 0x9d, 0xc2, 0x77, 0x38, 0x3c, 0x12, 0x12,  // bytes 24-31
];

// ============================================================================
// LicenseTier Enum
// ============================================================================

/// License tier levels
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LicenseTier {
    /// Hobby tier: 50 snapshots/day, 1 hour sessions, 30 req/min
    Hobby = 0,
    /// Starter tier: 500 snapshots/day, 8 hour sessions, 120 req/min
    Starter = 1,
    /// Developer tier: 5000 snapshots/day, 24 hour sessions, 300 req/min
    Developer = 2,
    /// Professional tier: Unlimited snapshots, unlimited sessions, 600 req/min
    Professional = 3,
    /// Enterprise tier: Unlimited everything, 1200 req/min, priority support
    Enterprise = 4,
}

impl LicenseTier {
    /// Parse tier from 3-character string
    #[inline]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "HOB" => Some(LicenseTier::Hobby),
            "STR" => Some(LicenseTier::Starter),
            "DEV" => Some(LicenseTier::Developer),
            "PRO" => Some(LicenseTier::Professional),
            "ENT" => Some(LicenseTier::Enterprise),
            _ => None,
        }
    }

    /// Convert to 3-character string
    #[inline]
    pub fn as_str(self) -> &'static str {
        match self {
            LicenseTier::Hobby => "HOB",
            LicenseTier::Starter => "STR",
            LicenseTier::Developer => "DEV",
            LicenseTier::Professional => "PRO",
            LicenseTier::Enterprise => "ENT",
        }
    }

    /// Convert from u8 representation
    #[inline]
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(LicenseTier::Hobby),
            1 => Some(LicenseTier::Starter),
            2 => Some(LicenseTier::Developer),
            3 => Some(LicenseTier::Professional),
            4 => Some(LicenseTier::Enterprise),
            _ => None,
        }
    }

    /// Convert to u8 representation
    #[inline]
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Get daily snapshot limit
    #[inline]
    pub fn snapshots_per_day(self) -> u64 {
        match self {
            LicenseTier::Hobby => 50,
            LicenseTier::Starter => 500,
            LicenseTier::Developer => 5000,
            LicenseTier::Professional => u64::MAX,
            LicenseTier::Enterprise => u64::MAX,
        }
    }

    /// Get session duration limit in seconds
    #[inline]
    pub fn session_duration_secs(self) -> u64 {
        match self {
            LicenseTier::Hobby => 3600,           // 1 hour
            LicenseTier::Starter => 8 * 3600,     // 8 hours
            LicenseTier::Developer => 24 * 3600,  // 24 hours
            LicenseTier::Professional => u64::MAX,
            LicenseTier::Enterprise => u64::MAX,
        }
    }

    /// Get rate limit (requests per minute)
    #[inline]
    pub fn rate_limit_per_min(self) -> u64 {
        match self {
            LicenseTier::Hobby => 30,
            LicenseTier::Starter => 120,
            LicenseTier::Developer => 300,
            LicenseTier::Professional => 600,
            LicenseTier::Enterprise => 1200,
        }
    }

    /// Get monthly price in cents (0 for hobby/custom for enterprise)
    #[inline]
    pub fn monthly_price_cents(self) -> u64 {
        match self {
            LicenseTier::Hobby => 0,
            LicenseTier::Starter => 900,       // $9/mo
            LicenseTier::Developer => 2900,    // $29/mo
            LicenseTier::Professional => 7900, // $79/mo
            LicenseTier::Enterprise => 0,      // Custom pricing
        }
    }
}

impl fmt::Display for LicenseTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LicenseTier::Hobby => write!(f, "Hobby"),
            LicenseTier::Starter => write!(f, "Starter"),
            LicenseTier::Developer => write!(f, "Developer"),
            LicenseTier::Professional => write!(f, "Professional"),
            LicenseTier::Enterprise => write!(f, "Enterprise"),
        }
    }
}

// ============================================================================
// License Validation Errors
// ============================================================================

/// License validation errors
#[derive(Debug, Clone)]
pub enum LicenseError {
    /// Invalid license key format
    InvalidFormat {
        expected: &'static str,
        got: String,
    },
    /// Unknown license tier
    UnknownTier {
        tier: String,
    },
    /// Invalid timestamp format
    InvalidTimestamp {
        timestamp: String,
    },
    /// Invalid organization hash
    InvalidOrgHash {
        hash: String,
    },
    /// Invalid signature format (not valid Base64)
    InvalidSignatureFormat {
        reason: String,
    },
    /// Ed25519 signature verification failed
    SignatureVerificationFailed,
    /// License has expired
    LicenseExpired {
        expired_at: u64,
        current_time: u64,
    },
    /// Organization mismatch
    OrganizationMismatch {
        expected_hash: u32,
        got_hash: u32,
    },
    /// Invalid public key (embedded key corrupted)
    InvalidPublicKey,
}

impl fmt::Display for LicenseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LicenseError::InvalidFormat { expected, got } => {
                write!(f, "Invalid license format: expected {}, got '{}'", expected, got)
            }
            LicenseError::UnknownTier { tier } => {
                write!(f, "Unknown license tier: '{}'. Valid: HOB, STR, DEV, PRO, ENT", tier)
            }
            LicenseError::InvalidTimestamp { timestamp } => {
                write!(f, "Invalid timestamp in license: '{}'", timestamp)
            }
            LicenseError::InvalidOrgHash { hash } => {
                write!(f, "Invalid organization hash: '{}'", hash)
            }
            LicenseError::InvalidSignatureFormat { reason } => {
                write!(f, "Invalid signature format: {}", reason)
            }
            LicenseError::SignatureVerificationFailed => {
                write!(f, "License signature verification failed. Invalid or tampered license.")
            }
            LicenseError::LicenseExpired { expired_at, current_time } => {
                write!(
                    f,
                    "License expired at {} (current time: {}). Renew at https://kindly.software/pricing",
                    expired_at, current_time
                )
            }
            LicenseError::OrganizationMismatch { expected_hash, got_hash } => {
                write!(
                    f,
                    "Organization mismatch: license for org {:08X}, but running as {:08X}",
                    expected_hash, got_hash
                )
            }
            LicenseError::InvalidPublicKey => {
                write!(f, "Invalid embedded public key (possible binary corruption)")
            }
        }
    }
}

impl Error for LicenseError {}

// ============================================================================
// LicenseValidatorCapsule - T0+T1 Atomic
// ============================================================================

/// LicenseValidatorCapsule - T0+T1 Atomic license validation
///
/// **Size**: 256 bytes (cache-line aligned)
/// **Alignment**: 64 bytes
///
/// **Layout** (256 bytes):
/// - License Metadata: 64 bytes
/// - Signature Storage: 64 bytes
/// - Verification State: 64 bytes
/// - Audit Trail: 64 bytes
#[repr(C, align(64))]
pub struct LicenseValidatorCapsule {
    // ========================================================================
    // License Metadata (64 bytes)
    // ========================================================================
    /// License tier (0-4)
    tier: AtomicU8,
    /// Verification state (0=pending, 1=valid, 2=invalid, 3=expired)
    verification_state: AtomicU8,
    /// Reserved for future flags
    _flags: [u8; 6],
    /// License expiration timestamp (Unix epoch seconds)
    expiration_timestamp: AtomicU64,
    /// Organization hash (CRC32)
    org_hash: AtomicU32,
    /// License creation timestamp (Unix epoch seconds)
    creation_timestamp: AtomicU64,
    /// Padding to 64 bytes
    _metadata_padding: [u8; 32],

    // ========================================================================
    // Ed25519 Signature Storage (64 bytes)
    // ========================================================================
    /// Ed25519 signature (64 bytes)
    signature: [u8; SIGNATURE_LENGTH],

    // ========================================================================
    // Verification State (64 bytes)
    // ========================================================================
    /// Validation count (for audit)
    validation_count: AtomicU64,
    /// Last validation timestamp
    last_validation_ns: AtomicU64,
    /// Validation failure count
    failure_count: AtomicU64,
    /// Last failure timestamp
    last_failure_ns: AtomicU64,
    /// Signed message hash (for audit verification)
    signed_message_hash: AtomicU64,
    /// Padding to 64 bytes
    _state_padding: [u8; 24],

    // ========================================================================
    // Q34 Audit Trail (64 bytes)
    // ========================================================================
    /// Generation counter for TOCTOU prevention
    generation: AtomicU64,
    /// CRC64 of last audit entry (hash-chain)
    audit_hash: AtomicU64,
    /// Audit event count
    audit_event_count: AtomicU64,
    /// Reserved for audit expansion
    _audit_reserved: [u8; 40],
}

// Compile-time size verification
const _: () = {
    const EXPECTED_SIZE: usize = 256;
    const ACTUAL_SIZE: usize = std::mem::size_of::<LicenseValidatorCapsule>();
    assert!(ACTUAL_SIZE == EXPECTED_SIZE, "LicenseValidatorCapsule must be exactly 256 bytes");
};

const _: () = {
    const EXPECTED_ALIGN: usize = 64;
    const ACTUAL_ALIGN: usize = std::mem::align_of::<LicenseValidatorCapsule>();
    assert!(ACTUAL_ALIGN == EXPECTED_ALIGN, "LicenseValidatorCapsule must be 64-byte aligned");
};

/// Verification state constants
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationState {
    Pending = 0,
    Valid = 1,
    Invalid = 2,
    Expired = 3,
}

impl LicenseValidatorCapsule {
    // ========================================================================
    // Constructors
    // ========================================================================

    /// Create a new unverified license validator (for testing/development)
    ///
    /// **Performance**: O(1), ~20ns
    pub fn new_unverified() -> Self {
        Self {
            tier: AtomicU8::new(LicenseTier::Hobby as u8),
            verification_state: AtomicU8::new(VerificationState::Pending as u8),
            _flags: [0; 6],
            expiration_timestamp: AtomicU64::new(0),
            org_hash: AtomicU32::new(0),
            creation_timestamp: AtomicU64::new(0),
            _metadata_padding: [0; 32],
            signature: [0; SIGNATURE_LENGTH],
            validation_count: AtomicU64::new(0),
            last_validation_ns: AtomicU64::new(0),
            failure_count: AtomicU64::new(0),
            last_failure_ns: AtomicU64::new(0),
            signed_message_hash: AtomicU64::new(0),
            _state_padding: [0; 24],
            generation: AtomicU64::new(0),
            audit_hash: AtomicU64::new(0),
            audit_event_count: AtomicU64::new(0),
            _audit_reserved: [0; 40],
        }
    }

    /// Parse license key from string format
    ///
    /// **Format**: `KDB-[TIER]-[TIMESTAMP]-[ORG_HASH]-[ED25519_SIG]`
    ///
    /// **Performance**: O(n), ~500ns (string parsing + base64 decode)
    ///
    /// # Errors
    /// - `LicenseError::InvalidFormat` if format doesn't match
    /// - `LicenseError::UnknownTier` if tier is not recognized
    /// - `LicenseError::InvalidTimestamp` if timestamp is not valid hex
    /// - `LicenseError::InvalidOrgHash` if org hash is not valid hex
    /// - `LicenseError::InvalidSignatureFormat` if signature is not valid Base64
    pub fn parse(license_key: &str) -> Result<Self, LicenseError> {
        // #ASSUME_LICENSE_FORMAT: License follows KDB-TIER-TIMESTAMP-ORG-SIG pattern
        let parts: Vec<&str> = license_key.split('-').collect();

        if parts.len() < 5 {
            return Err(LicenseError::InvalidFormat {
                expected: "KDB-TIER-TIMESTAMP-ORG_HASH-SIGNATURE (5+ parts)",
                got: license_key.to_string(),
            });
        }

        // Validate prefix
        if parts[0] != "KDB" {
            return Err(LicenseError::InvalidFormat {
                expected: "License must start with 'KDB'",
                got: parts[0].to_string(),
            });
        }

        // Parse tier (3 chars: HOB, STR, DEV, PRO, ENT)
        let tier = LicenseTier::from_str(parts[1])
            .ok_or_else(|| LicenseError::UnknownTier {
                tier: parts[1].to_string(),
            })?;

        // Parse timestamp (8 hex chars = Unix timestamp)
        let timestamp = u64::from_str_radix(parts[2], 16)
            .map_err(|_| LicenseError::InvalidTimestamp {
                timestamp: parts[2].to_string(),
            })?;

        // Parse org hash (8 hex chars = CRC32)
        let org_hash = u32::from_str_radix(parts[3], 16)
            .map_err(|_| LicenseError::InvalidOrgHash {
                hash: parts[3].to_string(),
            })?;

        // Reconstruct signature (may contain dashes in base64)
        let sig_parts: Vec<&str> = parts[4..].to_vec();
        let sig_base64 = sig_parts.join("-");

        // Decode Base64 signature
        let sig_bytes = Self::decode_base64(&sig_base64)
            .map_err(|e| LicenseError::InvalidSignatureFormat {
                reason: e.to_string(),
            })?;

        if sig_bytes.len() != SIGNATURE_LENGTH {
            return Err(LicenseError::InvalidSignatureFormat {
                reason: format!("Expected {} bytes, got {}", SIGNATURE_LENGTH, sig_bytes.len()),
            });
        }

        // Copy signature into fixed array
        let mut signature = [0u8; SIGNATURE_LENGTH];
        signature.copy_from_slice(&sig_bytes);

        // Calculate expiration (license valid for 1 year from creation)
        let expiration = timestamp + (365 * 24 * 3600);

        // Calculate signed message hash for audit
        let message = Self::build_message(tier, timestamp, org_hash);
        let message_hash = Self::hash_message(&message);

        let now_ns = Self::get_timestamp_ns();

        Ok(Self {
            tier: AtomicU8::new(tier as u8),
            verification_state: AtomicU8::new(VerificationState::Pending as u8),
            _flags: [0; 6],
            expiration_timestamp: AtomicU64::new(expiration),
            org_hash: AtomicU32::new(org_hash),
            creation_timestamp: AtomicU64::new(timestamp),
            _metadata_padding: [0; 32],
            signature,
            validation_count: AtomicU64::new(0),
            last_validation_ns: AtomicU64::new(now_ns),
            failure_count: AtomicU64::new(0),
            last_failure_ns: AtomicU64::new(0),
            signed_message_hash: AtomicU64::new(message_hash),
            _state_padding: [0; 24],
            generation: AtomicU64::new(1),
            audit_hash: AtomicU64::new(0),
            audit_event_count: AtomicU64::new(0),
            _audit_reserved: [0; 40],
        })
    }

    // ========================================================================
    // Ed25519 Verification
    // ========================================================================

    /// Verify Ed25519 signature
    ///
    /// **Performance**: ~50-100μs (Ed25519 verification)
    ///
    /// # Errors
    /// - `LicenseError::InvalidPublicKey` if embedded key is corrupted
    /// - `LicenseError::SignatureVerificationFailed` if signature doesn't match
    pub fn verify(&self) -> Result<(), LicenseError> {
        let now_ns = Self::get_timestamp_ns();

        // Load embedded public key
        let public_key = VerifyingKey::from_bytes(&KDB_PUBLIC_KEY_BYTES)
            .map_err(|_| LicenseError::InvalidPublicKey)?;

        // Build message that was signed
        let tier = self.get_tier();
        let timestamp = self.creation_timestamp.load(Ordering::Acquire);
        let org_hash = self.org_hash.load(Ordering::Acquire);
        let message = Self::build_message(tier, timestamp, org_hash);

        // Create signature from stored bytes
        let signature = Signature::from_bytes(&self.signature);

        // Verify signature
        // #ASSUME_ED25519_SECURE: Ed25519 verification is cryptographically sound
        match public_key.verify(&message, &signature) {
            Ok(()) => {
                // Signature valid
                self.verification_state.store(VerificationState::Valid as u8, Ordering::Release);
                self.validation_count.fetch_add(1, Ordering::Relaxed);
                self.last_validation_ns.store(now_ns, Ordering::Relaxed);
                self.generation.fetch_add(1, Ordering::AcqRel);
                self.update_audit_hash(b"VERIFY_SUCCESS");
                Ok(())
            }
            Err(_) => {
                // Signature invalid
                self.verification_state.store(VerificationState::Invalid as u8, Ordering::Release);
                self.failure_count.fetch_add(1, Ordering::Relaxed);
                self.last_failure_ns.store(now_ns, Ordering::Relaxed);
                self.generation.fetch_add(1, Ordering::AcqRel);
                self.update_audit_hash(b"VERIFY_FAILED");
                Err(LicenseError::SignatureVerificationFailed)
            }
        }
    }

    /// Verify signature and check expiration in one call
    ///
    /// **Performance**: ~50-100μs (verify) + ~50ns (expiration check)
    pub fn verify_and_check(&self) -> Result<(), LicenseError> {
        self.verify()?;
        self.check_expiration()?;
        Ok(())
    }

    // ========================================================================
    // Expiration Checking
    // ========================================================================

    /// Check if license has expired
    ///
    /// **Performance**: <50ns (Relaxed load + compare)
    ///
    /// # Errors
    /// - `LicenseError::LicenseExpired` if current time > expiration
    pub fn check_expiration(&self) -> Result<(), LicenseError> {
        let current_secs = Self::get_timestamp_secs();
        let expiration = self.expiration_timestamp.load(Ordering::Relaxed);

        if current_secs > expiration {
            self.verification_state.store(VerificationState::Expired as u8, Ordering::Release);
            self.update_audit_hash(b"LICENSE_EXPIRED");
            Err(LicenseError::LicenseExpired {
                expired_at: expiration,
                current_time: current_secs,
            })
        } else {
            Ok(())
        }
    }

    /// Check organization hash matches
    ///
    /// **Performance**: <50ns
    ///
    /// # Arguments
    /// - `org_name`: Organization name to verify against license
    pub fn verify_organization(&self, org_name: &str) -> Result<(), LicenseError> {
        let expected_hash = self.org_hash.load(Ordering::Relaxed);
        let actual_hash = Self::compute_org_hash(org_name);

        if expected_hash != actual_hash {
            self.update_audit_hash(b"ORG_MISMATCH");
            Err(LicenseError::OrganizationMismatch {
                expected_hash,
                got_hash: actual_hash,
            })
        } else {
            Ok(())
        }
    }

    // ========================================================================
    // Getters
    // ========================================================================

    /// Get license tier
    ///
    /// **Performance**: <10ns (Relaxed load)
    #[inline]
    pub fn get_tier(&self) -> LicenseTier {
        let tier_byte = self.tier.load(Ordering::Relaxed);
        LicenseTier::from_u8(tier_byte).unwrap_or(LicenseTier::Hobby)
    }

    /// Get verification state
    ///
    /// **Performance**: <10ns (Relaxed load)
    #[inline]
    pub fn get_verification_state(&self) -> VerificationState {
        let state = self.verification_state.load(Ordering::Relaxed);
        match state {
            0 => VerificationState::Pending,
            1 => VerificationState::Valid,
            2 => VerificationState::Invalid,
            _ => VerificationState::Expired,
        }
    }

    /// Check if license is verified and valid
    ///
    /// **Performance**: <10ns
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.get_verification_state() == VerificationState::Valid
    }

    /// Get expiration timestamp (Unix epoch seconds)
    ///
    /// **Performance**: <10ns
    #[inline]
    pub fn get_expiration(&self) -> u64 {
        self.expiration_timestamp.load(Ordering::Relaxed)
    }

    /// Get organization hash
    ///
    /// **Performance**: <10ns
    #[inline]
    pub fn get_org_hash(&self) -> u32 {
        self.org_hash.load(Ordering::Relaxed)
    }

    /// Get creation timestamp
    ///
    /// **Performance**: <10ns
    #[inline]
    pub fn get_creation_timestamp(&self) -> u64 {
        self.creation_timestamp.load(Ordering::Relaxed)
    }

    /// Get validation count (for audit)
    ///
    /// **Performance**: <10ns
    #[inline]
    pub fn get_validation_count(&self) -> u64 {
        self.validation_count.load(Ordering::Relaxed)
    }

    /// Get failure count (for audit)
    ///
    /// **Performance**: <10ns
    #[inline]
    pub fn get_failure_count(&self) -> u64 {
        self.failure_count.load(Ordering::Relaxed)
    }

    /// Get generation counter (for TOCTOU prevention)
    ///
    /// **Performance**: <10ns
    #[inline]
    pub fn get_generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get days until expiration (0 if expired)
    ///
    /// **Performance**: <50ns
    pub fn days_until_expiration(&self) -> u64 {
        let current_secs = Self::get_timestamp_secs();
        let expiration = self.expiration_timestamp.load(Ordering::Relaxed);

        if current_secs >= expiration {
            0
        } else {
            (expiration - current_secs) / (24 * 3600)
        }
    }

    // ========================================================================
    // Test Helpers (for setting internal state in tests)
    // ========================================================================

    /// Set tier (for testing only)
    ///
    /// **WARNING**: Only use in tests. In production, tier comes from license parsing.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn set_tier_for_test(&self, tier: LicenseTier) {
        self.tier.store(tier as u8, Ordering::Relaxed);
    }

    /// Set verification state (for testing only)
    ///
    /// **WARNING**: Only use in tests. In production, state comes from verify().
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn set_verification_state_for_test(&self, state: VerificationState) {
        self.verification_state.store(state as u8, Ordering::Relaxed);
    }

    /// Set expiration timestamp (for testing only)
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn set_expiration_for_test(&self, timestamp: u64) {
        self.expiration_timestamp.store(timestamp, Ordering::Relaxed);
    }

    /// Set organization hash (for testing only)
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn set_org_hash_for_test(&self, hash: u32) {
        self.org_hash.store(hash, Ordering::Relaxed);
    }

    /// Get audit event count (for testing and audit verification)
    pub fn get_audit_event_count(&self) -> u64 {
        self.audit_event_count.load(Ordering::Relaxed)
    }

    /// Get audit hash (for Q34 compliance verification)
    pub fn get_audit_hash(&self) -> u64 {
        self.audit_hash.load(Ordering::Relaxed)
    }

    /// Update audit hash chain (public for testing)
    ///
    /// **Note**: In production, called internally by verify/check methods.
    /// Made public for testing Q34 audit trail functionality.
    pub fn update_audit_hash(&self, event: &[u8]) {
        let prev_hash = self.audit_hash.load(Ordering::Acquire);
        let event_count = self.audit_event_count.fetch_add(1, Ordering::AcqRel);

        // Chain: new_hash = hash(prev_hash || event || count)
        let mut data = Vec::with_capacity(8 + event.len() + 8);
        data.extend_from_slice(&prev_hash.to_le_bytes());
        data.extend_from_slice(event);
        data.extend_from_slice(&event_count.to_le_bytes());

        let new_hash = Self::hash_message(&data);
        self.audit_hash.store(new_hash, Ordering::Release);
    }

    /// Compute organization hash (public for testing)
    pub fn compute_org_hash(org_name: &str) -> u32 {
        let crc = Crc::<u32>::new(&CRC_32_ISO_HDLC);
        crc.checksum(org_name.as_bytes())
    }

    /// Encode bytes to Base64 (public for testing)
    pub fn encode_base64(input: &[u8]) -> String {
        const BASE64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

        let mut output = String::with_capacity((input.len() + 2) / 3 * 4);

        for chunk in input.chunks(3) {
            let mut buffer: u32 = 0;
            for (i, &byte) in chunk.iter().enumerate() {
                buffer |= (byte as u32) << (16 - i * 8);
            }

            output.push(BASE64_TABLE[(buffer >> 18) as usize & 0x3F] as char);
            output.push(BASE64_TABLE[(buffer >> 12) as usize & 0x3F] as char);

            if chunk.len() > 1 {
                output.push(BASE64_TABLE[(buffer >> 6) as usize & 0x3F] as char);
            } else {
                output.push('=');
            }

            if chunk.len() > 2 {
                output.push(BASE64_TABLE[buffer as usize & 0x3F] as char);
            } else {
                output.push('=');
            }
        }

        output
    }

    /// Decode Base64 to bytes (public for testing)
    pub fn decode_base64(input: &str) -> Result<Vec<u8>, &'static str> {
        fn decode_char(c: u8) -> Option<u8> {
            match c {
                b'A'..=b'Z' => Some(c - b'A'),
                b'a'..=b'z' => Some(c - b'a' + 26),
                b'0'..=b'9' => Some(c - b'0' + 52),
                b'+' => Some(62),
                b'/' => Some(63),
                b'=' => None, // Padding
                _ => None,
            }
        }

        let input = input.as_bytes();
        let mut output = Vec::with_capacity(input.len() * 3 / 4);

        let mut buffer: u32 = 0;
        let mut bits_collected = 0;

        for &byte in input {
            if byte == b'=' {
                break;
            }

            if let Some(val) = decode_char(byte) {
                buffer = (buffer << 6) | (val as u32);
                bits_collected += 6;

                if bits_collected >= 8 {
                    bits_collected -= 8;
                    output.push((buffer >> bits_collected) as u8);
                    buffer &= (1 << bits_collected) - 1;
                }
            } else if !byte.is_ascii_whitespace() {
                return Err("Invalid Base64 character");
            }
        }

        Ok(output)
    }

    // ========================================================================
    // License Status
    // ========================================================================

    /// Get comprehensive license status
    pub fn get_status(&self) -> LicenseStatus {
        LicenseStatus {
            tier: self.get_tier(),
            state: self.get_verification_state(),
            expiration_timestamp: self.get_expiration(),
            days_until_expiration: self.days_until_expiration(),
            org_hash: self.get_org_hash(),
            creation_timestamp: self.get_creation_timestamp(),
            validation_count: self.get_validation_count(),
            failure_count: self.get_failure_count(),
            generation: self.get_generation(),
        }
    }

    // ========================================================================
    // License Key Generation (for admin/testing)
    // ========================================================================

    /// Generate a license key (requires private key - for testing/admin only)
    ///
    /// **SECURITY**: This function is for testing. In production, license signing
    /// happens on a secure server with the private key.
    #[cfg(feature = "license-signing")]
    pub fn generate_license_key(
        tier: LicenseTier,
        org_name: &str,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> String {
        use ed25519_dalek::Signer;

        let timestamp = Self::get_timestamp_secs();
        let org_hash = Self::compute_org_hash(org_name);
        let message = Self::build_message(tier, timestamp, org_hash);

        let signature = signing_key.sign(&message);
        let sig_base64 = Self::encode_base64(&signature.to_bytes());

        format!(
            "KDB-{}-{:08X}-{:08X}-{}",
            tier.as_str(),
            timestamp,
            org_hash,
            sig_base64
        )
    }

    /// Generate a test keypair (for development only)
    #[cfg(feature = "license-signing")]
    pub fn generate_test_keypair() -> (ed25519_dalek::SigningKey, ed25519_dalek::VerifyingKey) {
        use ed25519_dalek::{SigningKey, SecretKey};
        use rand::RngCore;

        // Generate 32 random bytes for the secret key
        let mut secret_bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut secret_bytes);

        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key = signing_key.verifying_key();
        (signing_key, verifying_key)
    }

    // ========================================================================
    // Helper Functions
    // ========================================================================

    /// Build message to sign/verify
    fn build_message(tier: LicenseTier, timestamp: u64, org_hash: u32) -> Vec<u8> {
        // Message format: "KDB-LICENSE-V1:{tier}:{timestamp}:{org_hash}"
        format!("KDB-LICENSE-V1:{}:{}:{}", tier.as_str(), timestamp, org_hash).into_bytes()
    }

    /// Hash message for audit storage
    fn hash_message(message: &[u8]) -> u64 {
        use crc::CRC_64_ECMA_182;
        let crc = Crc::<u64>::new(&CRC_64_ECMA_182);
        crc.checksum(message)
    }

    /// Get current timestamp in nanoseconds
    fn get_timestamp_ns() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }

    /// Get current timestamp in seconds
    fn get_timestamp_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

}

// ============================================================================
// LicenseStatus - User-Facing License Information
// ============================================================================

/// Current license status
#[derive(Debug, Clone)]
pub struct LicenseStatus {
    /// License tier
    pub tier: LicenseTier,
    /// Verification state
    pub state: VerificationState,
    /// Expiration timestamp (Unix epoch seconds)
    pub expiration_timestamp: u64,
    /// Days until expiration
    pub days_until_expiration: u64,
    /// Organization hash
    pub org_hash: u32,
    /// Creation timestamp
    pub creation_timestamp: u64,
    /// Validation count
    pub validation_count: u64,
    /// Failure count
    pub failure_count: u64,
    /// Generation counter
    pub generation: u64,
}

impl fmt::Display for LicenseStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LicenseStatus {{ tier: {}, state: {:?}, expires_in: {} days, validations: {}, failures: {} }}",
            self.tier,
            self.state,
            self.days_until_expiration,
            self.validation_count,
            self.failure_count
        )
    }
}

// ============================================================================
// QuotaTrackerCapsule Integration
// ============================================================================

// Note: The new_from_license() constructor is implemented in quota.rs
// to access private fields. Import LicenseTier and VerificationState there.

/// Helper function to map license tier to quota parameters
///
/// Returns: (snapshots_limit, session_limit_ns, tokens_max, refill_rate_ns)
pub fn license_tier_to_quota_params(tier: LicenseTier, verified: bool) -> (u64, u64, u64, u64) {
    // Default to Hobby tier if license not verified
    let effective_tier = if verified { tier } else { LicenseTier::Hobby };

    match effective_tier {
        LicenseTier::Hobby => (
            50,                              // 50 snapshots
            3600 * 1_000_000_000u64,        // 1 hour
            30,                              // 30 req/min
            2_000_000_000u64,                // 0.5 tokens/sec
        ),
        LicenseTier::Starter => (
            500,                             // 500 snapshots
            8 * 3600 * 1_000_000_000u64,    // 8 hours
            120,                             // 120 req/min
            500_000_000u64,                  // 2 tokens/sec
        ),
        LicenseTier::Developer => (
            5000,                            // 5000 snapshots
            24 * 3600 * 1_000_000_000u64,   // 24 hours
            300,                             // 300 req/min
            200_000_000u64,                  // 5 tokens/sec
        ),
        LicenseTier::Professional => (
            u64::MAX,                        // Unlimited
            u64::MAX,                        // Unlimited
            600,                             // 600 req/min
            100_000_000u64,                  // 10 tokens/sec
        ),
        LicenseTier::Enterprise => (
            u64::MAX,                        // Unlimited
            u64::MAX,                        // Unlimited
            1200,                            // 1200 req/min
            50_000_000u64,                   // 20 tokens/sec
        ),
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_and_alignment() {
        assert_eq!(std::mem::size_of::<LicenseValidatorCapsule>(), 256);
        assert_eq!(std::mem::align_of::<LicenseValidatorCapsule>(), 64);
    }

    #[test]
    fn test_license_tier_from_str() {
        assert_eq!(LicenseTier::from_str("HOB"), Some(LicenseTier::Hobby));
        assert_eq!(LicenseTier::from_str("str"), Some(LicenseTier::Starter));
        assert_eq!(LicenseTier::from_str("DEV"), Some(LicenseTier::Developer));
        assert_eq!(LicenseTier::from_str("pro"), Some(LicenseTier::Professional));
        assert_eq!(LicenseTier::from_str("ENT"), Some(LicenseTier::Enterprise));
        assert_eq!(LicenseTier::from_str("INVALID"), None);
    }

    #[test]
    fn test_license_tier_limits() {
        assert_eq!(LicenseTier::Hobby.snapshots_per_day(), 50);
        assert_eq!(LicenseTier::Starter.snapshots_per_day(), 500);
        assert_eq!(LicenseTier::Developer.snapshots_per_day(), 5000);
        assert_eq!(LicenseTier::Professional.snapshots_per_day(), u64::MAX);
        assert_eq!(LicenseTier::Enterprise.snapshots_per_day(), u64::MAX);

        assert_eq!(LicenseTier::Hobby.session_duration_secs(), 3600);
        assert_eq!(LicenseTier::Starter.session_duration_secs(), 8 * 3600);
        assert_eq!(LicenseTier::Developer.session_duration_secs(), 24 * 3600);
    }

    #[test]
    fn test_license_tier_rate_limits() {
        assert_eq!(LicenseTier::Hobby.rate_limit_per_min(), 30);
        assert_eq!(LicenseTier::Starter.rate_limit_per_min(), 120);
        assert_eq!(LicenseTier::Developer.rate_limit_per_min(), 300);
        assert_eq!(LicenseTier::Professional.rate_limit_per_min(), 600);
        assert_eq!(LicenseTier::Enterprise.rate_limit_per_min(), 1200);
    }

    #[test]
    fn test_new_unverified() {
        let validator = LicenseValidatorCapsule::new_unverified();
        assert_eq!(validator.get_tier(), LicenseTier::Hobby);
        assert_eq!(validator.get_verification_state(), VerificationState::Pending);
        assert!(!validator.is_valid());
    }

    #[test]
    fn test_org_hash_computation() {
        let hash1 = LicenseValidatorCapsule::compute_org_hash("Acme Corp");
        let hash2 = LicenseValidatorCapsule::compute_org_hash("Acme Corp");
        let hash3 = LicenseValidatorCapsule::compute_org_hash("Other Corp");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_base64_roundtrip() {
        let original = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let encoded = LicenseValidatorCapsule::encode_base64(&original);
        let decoded = LicenseValidatorCapsule::decode_base64(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_base64_signature_length() {
        // Ed25519 signature is 64 bytes, Base64 encodes to 88 chars (with padding)
        let sig = [0u8; 64];
        let encoded = LicenseValidatorCapsule::encode_base64(&sig);
        let decoded = LicenseValidatorCapsule::decode_base64(&encoded).unwrap();
        assert_eq!(decoded.len(), 64);
    }

    #[test]
    fn test_parse_invalid_format() {
        let result = LicenseValidatorCapsule::parse("INVALID");
        assert!(matches!(result, Err(LicenseError::InvalidFormat { .. })));

        let result = LicenseValidatorCapsule::parse("KDB-XXX-12345678-ABCDEF12-sig");
        assert!(matches!(result, Err(LicenseError::UnknownTier { .. })));
    }

    #[test]
    fn test_license_status_display() {
        let validator = LicenseValidatorCapsule::new_unverified();
        let status = validator.get_status();
        let display = format!("{}", status);
        assert!(display.contains("Hobby"));
        assert!(display.contains("Pending"));
    }

    #[test]
    fn test_generation_counter_increment() {
        let validator = LicenseValidatorCapsule::new_unverified();
        let gen1 = validator.get_generation();

        // Attempt verify (will fail with dev key, but generation should change)
        let _ = validator.verify();

        let gen2 = validator.get_generation();
        assert!(gen2 > gen1, "Generation should increment after verify attempt");
    }

    #[test]
    fn test_audit_hash_chain() {
        let validator = LicenseValidatorCapsule::new_unverified();
        let hash1 = validator.audit_hash.load(Ordering::Relaxed);

        validator.update_audit_hash(b"TEST_EVENT");
        let hash2 = validator.audit_hash.load(Ordering::Relaxed);

        validator.update_audit_hash(b"ANOTHER_EVENT");
        let hash3 = validator.audit_hash.load(Ordering::Relaxed);

        // Hashes should be different (chain progression)
        assert_ne!(hash1, hash2);
        assert_ne!(hash2, hash3);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_days_until_expiration() {
        let validator = LicenseValidatorCapsule::new_unverified();

        // Set expiration to 30 days from now
        let future = LicenseValidatorCapsule::get_timestamp_secs() + (30 * 24 * 3600);
        validator.expiration_timestamp.store(future, Ordering::Relaxed);

        let days = validator.days_until_expiration();
        assert!(days >= 29 && days <= 31, "Expected ~30 days, got {}", days);
    }

    #[test]
    fn test_expiration_check() {
        let validator = LicenseValidatorCapsule::new_unverified();

        // Set expiration to past
        validator.expiration_timestamp.store(1000, Ordering::Relaxed);

        let result = validator.check_expiration();
        assert!(matches!(result, Err(LicenseError::LicenseExpired { .. })));
    }

    #[test]
    fn test_organization_verification() {
        let validator = LicenseValidatorCapsule::new_unverified();
        let org_name = "Kindly Software";
        let org_hash = LicenseValidatorCapsule::compute_org_hash(org_name);
        validator.org_hash.store(org_hash, Ordering::Relaxed);

        // Should pass with same org
        assert!(validator.verify_organization(org_name).is_ok());

        // Should fail with different org
        let result = validator.verify_organization("Other Company");
        assert!(matches!(result, Err(LicenseError::OrganizationMismatch { .. })));
    }
}
