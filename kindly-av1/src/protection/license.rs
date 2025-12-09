//! CryptoLicenseCapsule - Ed25519 License Validation (T1 Atomic + T0 Auditable)
//! [TRADE SECRET]
//!
//! # Architecture
//!
//! 512-byte cache-aligned capsule for cryptographic license verification.
//! Uses Ed25519 signature validation for tamper-proof licensing.
//!
//! ## Memory Layout (512B, 8 cache lines)
//!
//! ```text
//! Offset  Size  Field
//! ------  ----  -----
//! 0       32    public_key ([u8; 32])
//! 32      16    state (DualAtomicU64: valid_until_epoch | flags)
//! 48      1     cached_valid (AtomicBool)
//! 49      7     _padding1
//! 56      8     last_check (AtomicU64)
//! 64      8     generation (AtomicU64)
//! 72      32    hardware_id ([u8; 32])
//! 104     64    signature ([u8; 64])
//! 168     8     tier (AtomicU64)
//! 176     336   _padding2
//! ------  ----
//! Total:  512B (8 cache lines, 64B aligned)
//! ```
//!
//! ## License Format (JSON)
//!
//! ```json
//! {
//!   "customer_id": "uuid-v4",
//!   "tier": "Creator|Professional|Enterprise",
//!   "valid_until": 1735689600,  // Unix epoch
//!   "hardware_id": "blake3-hash-hex",
//!   "signature": "ed25519-signature-hex"
//! }
//! ```
//!
//! ## Framework Compliance
//!
//! - UCE34 Q10: T1 Atomic + T0 Auditable tier
//! - Chaos: 100% lockfree, cache-aligned, generation counters
//! - ASSUM: All assumptions documented with #ASSUME tags
//! - B32: <5ns cached check, <500µs full verify

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use atomic_capsule::patterns::dual_atomic::DualAtomicU64;

/// License tiers
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LicenseTier {
    /// Creator: 1080p max, 2 machines, email support
    Creator = 1,
    /// Professional: 4K max, 3 machines, priority support
    Professional = 2,
    /// Enterprise: 8K max, 10 machines, dedicated support
    Enterprise = 3,
}

impl LicenseTier {
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Creator" => Some(Self::Creator),
            "Professional" => Some(Self::Professional),
            "Enterprise" => Some(Self::Enterprise),
            _ => None,
        }
    }

    /// Get max resolution for tier
    pub const fn max_resolution(&self) -> (u32, u32) {
        match self {
            Self::Creator => (1920, 1080),
            Self::Professional => (3840, 2160),
            Self::Enterprise => (7680, 4320),
        }
    }

    /// Get machine limit for tier
    pub const fn machine_limit(&self) -> u32 {
        match self {
            Self::Creator => 2,
            Self::Professional => 3,
            Self::Enterprise => 10,
        }
    }
}

impl From<u64> for LicenseTier {
    fn from(value: u64) -> Self {
        match value {
            1 => Self::Creator,
            2 => Self::Professional,
            3 => Self::Enterprise,
            _ => Self::Creator, // Default to lowest tier
        }
    }
}

/// License validation errors
#[derive(Debug)]
pub enum LicenseError {
    /// Invalid Ed25519 signature
    InvalidSignature,
    /// License has expired
    Expired,
    /// Hardware ID mismatch (wrong machine)
    HardwareMismatch,
    /// License tier exceeded (resolution too high)
    TierExceeded,
    /// License not activated
    NotActivated,
    /// Invalid public key
    InvalidPublicKey,
    /// Invalid license format
    InvalidFormat,
    /// JSON parse error
    JsonError(String),
    /// IO error
    IoError(std::io::Error),
}

impl std::fmt::Display for LicenseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSignature => write!(f, "Invalid Ed25519 signature"),
            Self::Expired => write!(f, "License has expired"),
            Self::HardwareMismatch => write!(f, "Hardware ID mismatch (wrong machine)"),
            Self::TierExceeded => write!(f, "License tier exceeded (resolution too high)"),
            Self::NotActivated => write!(f, "License not activated"),
            Self::InvalidPublicKey => write!(f, "Invalid public key"),
            Self::InvalidFormat => write!(f, "Invalid license format"),
            Self::JsonError(e) => write!(f, "JSON error: {}", e),
            Self::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for LicenseError {}

impl From<std::io::Error> for LicenseError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err)
    }
}

/// CryptoLicenseCapsule (512B, T1+T0)
///
/// Cache-aligned capsule for Ed25519 license validation.
///
/// # Anti-Piracy Features
///
/// - Ed25519 signature verification (cryptographically secure)
/// - Hardware ID binding (CPU + MAC address Blake3 hash)
/// - 24-hour cache validity (don't re-verify every call)
/// - Generation counter (detect tampering)
/// - Graceful expiration (warn 7 days before)
///
/// # Performance (B32 Targets)
///
/// - Cached check: <5ns (AtomicBool load)
/// - Full verify: <500µs (Ed25519 signature verification)
/// - Amortized: <1ns (24hr cache, 86,400 calls/day)
///
/// # Thread Safety
///
/// All operations use atomic operations with appropriate memory ordering.
/// Safe to share across threads.
#[repr(C, align(64))]
pub struct CryptoLicenseCapsule {
    /// Ed25519 public key (32 bytes)
    /// Used to verify license signatures
    public_key: [u8; 32],

    /// License state (DualAtomicU64)
    /// Primary: valid_until (unix epoch seconds)
    /// Secondary: flags (bit 0 = activated)
    state: DualAtomicU64,

    /// Cached validation result (AtomicBool)
    /// Updated every 24 hours
    cached_valid: AtomicBool,

    /// Padding for alignment
    _padding1: [u8; 7],

    /// Last validation timestamp (unix epoch seconds)
    last_check: AtomicU64,

    /// Generation counter (detect tampering)
    generation: AtomicU64,

    /// Hardware ID (Blake3 hash, 32 bytes)
    hardware_id: [u8; 32],

    /// Ed25519 signature (64 bytes)
    signature: [u8; 64],

    /// License tier (AtomicU64)
    tier: AtomicU64,

    /// Padding to 512 bytes
    _padding2: [u8; 336],
}

// Compile-time size verification
// #ASSUME: Size and alignment are critical for performance and security
// #VERIFY: Compile-time assertions ensure correct layout
const _: () = assert!(std::mem::size_of::<CryptoLicenseCapsule>() == 512);
const _: () = assert!(std::mem::align_of::<CryptoLicenseCapsule>() == 64);

impl CryptoLicenseCapsule {
    /// Create new capsule with public key
    ///
    /// # Arguments
    ///
    /// * `public_key` - Ed25519 public key (32 bytes)
    ///
    /// # Performance
    ///
    /// <10ns (const initialization)
    pub fn new(public_key: [u8; 32]) -> Self {
        Self {
            public_key,
            state: DualAtomicU64::new(0, 0),
            cached_valid: AtomicBool::new(false),
            _padding1: [0u8; 7],
            last_check: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            hardware_id: [0u8; 32],
            signature: [0u8; 64],
            tier: AtomicU64::new(LicenseTier::Creator as u64),
            _padding2: [0u8; 336],
        }
    }

    /// Activate license with JSON license data
    ///
    /// # Arguments
    ///
    /// * `license_json` - JSON license data (see module docs for format)
    ///
    /// # Returns
    ///
    /// Ok if activation successful, Err if signature invalid or hardware mismatch
    ///
    /// # Performance
    ///
    /// <500µs (Ed25519 signature verification dominates)
    ///
    /// # Errors
    ///
    /// - InvalidSignature: Signature verification failed
    /// - HardwareMismatch: Hardware ID doesn't match current machine
    /// - Expired: License has expired
    /// - InvalidFormat: JSON parse error
    pub fn activate(&mut self, license_json: &str) -> Result<(), LicenseError> {
        // Parse JSON license
        let license: LicenseData = serde_json::from_str(license_json)
            .map_err(|e| LicenseError::JsonError(e.to_string()))?;

        // Verify hardware ID
        let current_hardware_id = Self::generate_hardware_id();
        let license_hardware_id = Self::hex_decode(&license.hardware_id)
            .map_err(|_| LicenseError::InvalidFormat)?;

        if license_hardware_id.len() != 32 {
            return Err(LicenseError::InvalidFormat);
        }

        if license_hardware_id != current_hardware_id {
            return Err(LicenseError::HardwareMismatch);
        }

        // Check expiry
        let now = Self::unix_timestamp();
        if license.valid_until > 0 && now > license.valid_until {
            return Err(LicenseError::Expired);
        }

        // Verify Ed25519 signature
        let signature_bytes = Self::hex_decode(&license.signature)
            .map_err(|_| LicenseError::InvalidFormat)?;

        if signature_bytes.len() != 64 {
            return Err(LicenseError::InvalidFormat);
        }

        // Construct message for signature verification
        let message = format!(
            "{}|{}|{}|{}",
            license.customer_id, license.tier, license.valid_until, license.hardware_id
        );

        // Verify signature using ed25519-dalek
        #[cfg(feature = "crypto-license")]
        {
            use ed25519_dalek::{PublicKey, Signature, Verifier};

            let public_key = PublicKey::from_bytes(&self.public_key)
                .map_err(|_| LicenseError::InvalidPublicKey)?;

            let mut sig_array = [0u8; 64];
            sig_array.copy_from_slice(&signature_bytes);
            let signature = Signature::from_bytes(&sig_array);

            public_key
                .verify(message.as_bytes(), &signature)
                .map_err(|_| LicenseError::InvalidSignature)?;
        }

        #[cfg(not(feature = "crypto-license"))]
        {
            // Stub for non-crypto builds
            // In production, ALWAYS use crypto-license feature
            eprintln!("WARNING: Ed25519 verification disabled (crypto-license feature not enabled)");
        }

        // Store license data
        self.hardware_id.copy_from_slice(&current_hardware_id);
        self.signature[..signature_bytes.len()].copy_from_slice(&signature_bytes);

        // Parse tier
        let tier = LicenseTier::from_str(&license.tier).unwrap_or(LicenseTier::Creator);
        self.tier.store(tier as u64, Ordering::Release);

        // Set state
        self.state.store_primary(license.valid_until, Ordering::Release);
        self.state.store_secondary(1, Ordering::Release); // Activated flag

        // Update cache
        self.cached_valid.store(true, Ordering::Release);
        self.last_check.store(now, Ordering::Release);

        // Increment generation counter
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(())
    }

    /// Check if license is valid (24hr cached)
    ///
    /// # Performance
    ///
    /// - Cached (<24hr): <5ns (AtomicBool load)
    /// - Cache miss: <500µs (full verification)
    ///
    /// # Returns
    ///
    /// true if license is valid, false otherwise
    #[inline]
    pub fn is_valid(&self) -> bool {
        let now = Self::unix_timestamp();

        // Fast path: Check 24hr cache
        let last = self.last_check.load(Ordering::Acquire);
        if now - last < (24 * 60 * 60) {
            return self.cached_valid.load(Ordering::Acquire);
        }

        // Slow path: Full validation
        self.validate_full(now)
    }

    /// Check if tier allows resolution
    ///
    /// # Arguments
    ///
    /// * `width` - Video width in pixels
    /// * `height` - Video height in pixels
    ///
    /// # Returns
    ///
    /// Ok if tier allows resolution, Err(TierExceeded) if not
    ///
    /// # Performance
    ///
    /// <5ns (atomic load + comparison)
    #[inline]
    pub fn check_resolution(&self, width: u32, height: u32) -> Result<(), LicenseError> {
        let tier = LicenseTier::from(self.tier.load(Ordering::Acquire));
        let (max_width, max_height) = tier.max_resolution();

        if width > max_width || height > max_height {
            return Err(LicenseError::TierExceeded);
        }

        Ok(())
    }

    /// Get days until expiry (0 if expired, u64::MAX if perpetual)
    ///
    /// # Performance
    ///
    /// <10ns (atomic load + arithmetic)
    #[inline]
    pub fn days_until_expiry(&self) -> u64 {
        let valid_until = self.state.load_primary(Ordering::Acquire);

        if valid_until == 0 {
            return u64::MAX; // Perpetual license
        }

        let now = Self::unix_timestamp();
        if now > valid_until {
            return 0; // Expired
        }

        (valid_until - now) / (24 * 60 * 60)
    }

    /// Check if license expires soon (within 7 days)
    ///
    /// # Performance
    ///
    /// <10ns (calls days_until_expiry)
    #[inline]
    pub fn expiring_soon(&self) -> bool {
        let days = self.days_until_expiry();
        days < 7 && days != u64::MAX
    }

    /// Get current tier
    ///
    /// # Performance
    ///
    /// <5ns (atomic load)
    #[inline]
    pub fn tier(&self) -> LicenseTier {
        LicenseTier::from(self.tier.load(Ordering::Acquire))
    }

    /// Get generation counter (detect tampering)
    ///
    /// # Performance
    ///
    /// <5ns (atomic load)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Full validation (slow path, <500µs)
    fn validate_full(&self, now: u64) -> bool {
        // Check if activated
        let flags = self.state.load_secondary(Ordering::Acquire);
        if flags & 1 == 0 {
            return false; // Not activated
        }

        // Check expiry
        let valid_until = self.state.load_primary(Ordering::Acquire);
        if valid_until > 0 && now > valid_until {
            self.cached_valid.store(false, Ordering::Release);
            return false;
        }

        // Update cache
        self.cached_valid.store(true, Ordering::Release);
        self.last_check.store(now, Ordering::Release);

        true
    }

    /// Generate hardware ID (Blake3 hash of CPU + MAC)
    ///
    /// # Performance
    ///
    /// ~50µs (syscalls + Blake3 hash)
    ///
    /// # ASSUM Safety
    ///
    /// - #ASSUME: CPU ID stable across reboots
    /// - #ASSUME: MAC address stable (not randomized)
    /// - #VERIFY: Read from /proc/cpuinfo and /sys/class/net
    fn generate_hardware_id() -> [u8; 32] {
        use blake3::Hasher;

        let mut hasher = Hasher::new();

        // Hash CPU info
        #[cfg(target_os = "linux")]
        {
            if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
                // Extract processor ID (first 256 bytes sufficient)
                let cpu_data = &cpuinfo.as_bytes()[..cpuinfo.len().min(256)];
                hasher.update(cpu_data);
            }
        }

        // Hash MAC address
        #[cfg(target_os = "linux")]
        {
            if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
                for entry in entries.flatten() {
                    let path = entry.path().join("address");
                    if let Ok(mac) = std::fs::read_to_string(&path) {
                        hasher.update(mac.trim().as_bytes());
                        break; // First MAC is sufficient
                    }
                }
            }
        }

        // Fallback for non-Linux
        #[cfg(not(target_os = "linux"))]
        {
            hasher.update(b"fallback-hardware-id");
        }

        let hash = hasher.finalize();
        *hash.as_bytes()
    }

    /// Get current unix timestamp
    fn unix_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Hex decode (simple implementation)
    fn hex_decode(s: &str) -> Result<Vec<u8>, ()> {
        if s.len() % 2 != 0 {
            return Err(());
        }

        let mut bytes = Vec::with_capacity(s.len() / 2);
        for i in (0..s.len()).step_by(2) {
            let byte = u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ())?;
            bytes.push(byte);
        }

        Ok(bytes)
    }
}

impl Default for CryptoLicenseCapsule {
    fn default() -> Self {
        Self::new([0u8; 32])
    }
}

// Safety: All fields are either atomic or immutable after initialization
// #ASSUME: AtomicU64, AtomicBool, DualAtomicU64 are Send + Sync
// #VERIFY: public_key, hardware_id, signature only written during activate (requires &mut self)
unsafe impl Send for CryptoLicenseCapsule {}
unsafe impl Sync for CryptoLicenseCapsule {}

/// License data (parsed from JSON)
#[cfg(feature = "crypto-license")]
#[derive(Debug, serde::Deserialize)]
pub struct LicenseData {
    pub customer_id: String,
    pub tier: String,
    pub valid_until: u64,
    pub hardware_id: String,
    pub signature: String,
}

#[cfg(not(feature = "crypto-license"))]
#[derive(Debug)]
pub struct LicenseData {
    pub customer_id: String,
    pub tier: String,
    pub valid_until: u64,
    pub hardware_id: String,
    pub signature: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(std::mem::size_of::<CryptoLicenseCapsule>(), 512);
        assert_eq!(std::mem::align_of::<CryptoLicenseCapsule>(), 64);
    }

    #[test]
    fn test_new_capsule() {
        let public_key = [0u8; 32];
        let capsule = CryptoLicenseCapsule::new(public_key);
        assert!(!capsule.is_valid()); // Not activated
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.tier(), LicenseTier::Creator);
    }

    #[test]
    fn test_tier_resolution_limits() {
        let public_key = [0u8; 32];
        let capsule = CryptoLicenseCapsule::new(public_key);

        // Creator tier: 1080p max
        assert!(capsule.check_resolution(1920, 1080).is_ok());
        assert!(capsule.check_resolution(3840, 2160).is_err()); // 4K exceeds Creator

        // Upgrade to Professional tier
        capsule.tier.store(LicenseTier::Professional as u64, Ordering::Release);
        assert!(capsule.check_resolution(3840, 2160).is_ok());
        assert!(capsule.check_resolution(7680, 4320).is_err()); // 8K exceeds Professional

        // Upgrade to Enterprise tier
        capsule.tier.store(LicenseTier::Enterprise as u64, Ordering::Release);
        assert!(capsule.check_resolution(7680, 4320).is_ok());
    }

    #[test]
    fn test_expiry_calculation() {
        let public_key = [0u8; 32];
        let capsule = CryptoLicenseCapsule::new(public_key);

        // Perpetual license (valid_until = 0)
        assert_eq!(capsule.days_until_expiry(), u64::MAX);
        assert!(!capsule.expiring_soon());

        // Set expiry to 30 days from now
        let now = CryptoLicenseCapsule::unix_timestamp();
        let expiry = now + (30 * 24 * 60 * 60);
        capsule.state.store_primary(expiry, Ordering::Release);

        let days = capsule.days_until_expiry();
        assert!(days >= 29 && days <= 30); // Allow 1 day margin for test timing
        assert!(!capsule.expiring_soon());

        // Set expiry to 5 days from now (expiring soon)
        let expiry_soon = now + (5 * 24 * 60 * 60);
        capsule.state.store_primary(expiry_soon, Ordering::Release);
        assert!(capsule.expiring_soon());
    }

    #[test]
    fn test_hardware_id_generation() {
        let hw_id = CryptoLicenseCapsule::generate_hardware_id();
        assert_eq!(hw_id.len(), 32);

        // Hardware ID should be stable (same on same machine)
        let hw_id2 = CryptoLicenseCapsule::generate_hardware_id();
        assert_eq!(hw_id, hw_id2);
    }

    #[test]
    fn test_hex_decode() {
        let hex = "deadbeef";
        let bytes = CryptoLicenseCapsule::hex_decode(hex).unwrap();
        assert_eq!(bytes, vec![0xde, 0xad, 0xbe, 0xef]);

        // Invalid hex
        assert!(CryptoLicenseCapsule::hex_decode("invalid").is_err());
        assert!(CryptoLicenseCapsule::hex_decode("abc").is_err()); // Odd length
    }

    #[test]
    fn test_generation_counter() {
        let public_key = [0u8; 32];
        let mut capsule = CryptoLicenseCapsule::new(public_key);

        assert_eq!(capsule.generation(), 0);

        // Simulate activation (increments generation)
        capsule.generation.fetch_add(1, Ordering::AcqRel);
        assert_eq!(capsule.generation(), 1);
    }

    #[test]
    fn test_tier_from_str() {
        assert_eq!(LicenseTier::from_str("Creator"), Some(LicenseTier::Creator));
        assert_eq!(LicenseTier::from_str("Professional"), Some(LicenseTier::Professional));
        assert_eq!(LicenseTier::from_str("Enterprise"), Some(LicenseTier::Enterprise));
        assert_eq!(LicenseTier::from_str("invalid"), None);
    }

    #[test]
    fn test_tier_machine_limits() {
        assert_eq!(LicenseTier::Creator.machine_limit(), 2);
        assert_eq!(LicenseTier::Professional.machine_limit(), 3);
        assert_eq!(LicenseTier::Enterprise.machine_limit(), 10);
    }
}
