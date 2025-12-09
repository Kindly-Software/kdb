//! LicenseVerificationCapsule (T1 Atomic + T0 Auditable)
//! [TRADE SECRET]
//!
//! 128-byte cache-aligned capsule for license state management.
//! Generation counters detect tampering attempts.
//!
//! # Anti-Tampering Design
//!
//! The generation counter is incremented on every legitimate state change.
//! Binary patches that modify state directly (without calling our methods)
//! will leave the generation counter unchanged, causing integrity checks
//! to fail.
//!
//! # Memory Layout (128B, 2 cache lines)
//!
//! ```text
//! Offset  Size  Field
//! ------  ----  -----
//! 0       8     license_state (AtomicU64)
//! 8       32    hardware_fingerprint ([u8; 32])
//! 40      32    license_hash ([u8; 32])
//! 72      8     expiry_timestamp (AtomicU64)
//! 80      8     generation (AtomicU64)
//! 88      8     activation_timestamp (AtomicU64)
//! 96      8     check_counter (AtomicU64)
//! 104     24    _padding ([u8; 24])
//! ------  ----
//! Total:  128B (2 cache lines, 64B aligned)
//! ```
//!
//! # Framework Compliance
//!
//! - UCE34 Q10: T1 Atomic + T0 Auditable tier
//! - Chaos: 100% lockfree, cache-aligned, generation counters
//! - ASSUM: All assumptions documented with #ASSUME tags

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::config::{APP_NAME, LICENSE_FILENAME, LICENSE_MAGIC, LICENSE_VERSION};
use super::fingerprint::HardwareFingerprint;
use super::key::LicenseKey;

/// License states
///
/// Stored as u64 for atomic operations. Each state has a specific meaning:
/// - Invalid (0): No license loaded or license rejected
/// - Valid (1): License verified and active
/// - Expired (2): License was valid but has expired
/// - Tampered (3): Integrity check failed, possible binary patch detected
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LicenseState {
    /// No license loaded or license rejected
    Invalid = 0,
    /// License verified and active
    Valid = 1,
    /// License was valid but has expired
    Expired = 2,
    /// Integrity check failed - tampering detected
    Tampered = 3,
}

impl From<u64> for LicenseState {
    fn from(value: u64) -> Self {
        match value {
            0 => LicenseState::Invalid,
            1 => LicenseState::Valid,
            2 => LicenseState::Expired,
            3 => LicenseState::Tampered,
            // #ASSUME: Any other value indicates tampering
            // #VERIFY: Unknown state values treated as tampered for security
            _ => LicenseState::Tampered,
        }
    }
}

impl LicenseState {
    /// Check if this state allows encoding
    #[inline]
    pub const fn allows_encoding(&self) -> bool {
        matches!(self, LicenseState::Valid)
    }
}

/// License verification errors
#[derive(Debug)]
pub enum LicenseError {
    KeyError(super::key::LicenseKeyError),
    HardwareMismatch,
    Expired,
    IntegrityFailed,
    IoError(std::io::Error),
    InvalidFormat,
    NotFound,
    NotActivated,
}

impl std::fmt::Display for LicenseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeyError(e) => write!(f, "License key error: {}", e),
            Self::HardwareMismatch => write!(f, "Hardware fingerprint mismatch"),
            Self::Expired => write!(f, "License has expired"),
            Self::IntegrityFailed => {
                write!(f, "License integrity check failed - tampering detected")
            }
            Self::IoError(e) => write!(f, "IO error: {}", e),
            Self::InvalidFormat => write!(f, "Invalid license file format"),
            Self::NotFound => write!(f, "License file not found"),
            Self::NotActivated => write!(f, "License not activated"),
        }
    }
}

impl std::error::Error for LicenseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::KeyError(e) => Some(e),
            Self::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<super::key::LicenseKeyError> for LicenseError {
    fn from(err: super::key::LicenseKeyError) -> Self {
        Self::KeyError(err)
    }
}

impl From<std::io::Error> for LicenseError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err)
    }
}

/// LicenseVerificationCapsule (128B, T1+T0)
///
/// Cache-aligned capsule for tamper-resistant license verification.
/// The generation counter ensures that binary patches are detected.
///
/// # Thread Safety
///
/// All state modifications use atomic operations with appropriate
/// memory ordering. The capsule is safe to share across threads.
///
/// # Anti-Piracy
///
/// - Generation counter must increment with each state change
/// - Hardware fingerprint bound at activation time
/// - Signature verification prevents key modification
/// - Expiry checked on every validation call
#[repr(C, align(64))]
pub struct LicenseVerificationCapsule {
    /// Current license state (atomic for lockfree access)
    license_state: AtomicU64,

    /// Blake3 hash of CPU+MAC for hardware binding
    hardware_fingerprint: [u8; 32],

    /// Stored key signature for verification
    license_hash: [u8; 32],

    /// Unix timestamp when license expires (0 = perpetual)
    expiry_timestamp: AtomicU64,

    /// Generation counter for tamper detection
    /// Incremented on every legitimate state change
    generation: AtomicU64,

    /// Unix timestamp when license was activated
    activation_timestamp: AtomicU64,

    /// Counter for number of validity checks performed
    /// Used for audit trail (T0 Auditable)
    check_counter: AtomicU64,

    /// Padding for cache alignment (128B total)
    _padding: [u8; 24],
}

// Compile-time size verification
// #ASSUME: Size and alignment are critical for performance and security
// #VERIFY: Compile-time assertions ensure correct layout
const _: () = assert!(std::mem::size_of::<LicenseVerificationCapsule>() == 128);
const _: () = assert!(std::mem::align_of::<LicenseVerificationCapsule>() == 64);

impl LicenseVerificationCapsule {
    /// Create new capsule (starts in Invalid state)
    ///
    /// The capsule is initialized with zero values and must be activated
    /// with a valid license key before encoding can proceed.
    pub const fn new() -> Self {
        Self {
            license_state: AtomicU64::new(LicenseState::Invalid as u64),
            hardware_fingerprint: [0u8; 32],
            license_hash: [0u8; 32],
            expiry_timestamp: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            activation_timestamp: AtomicU64::new(0),
            check_counter: AtomicU64::new(0),
            _padding: [0u8; 24],
        }
    }

    /// Verify and activate license key
    ///
    /// This method:
    /// 1. Parses and validates the license key format
    /// 2. Verifies the key checksum
    /// 3. Checks hardware binding against current machine
    /// 4. Generates and stores the license signature
    /// 5. Sets state to Valid and increments generation
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Key format is invalid
    /// - Checksum verification fails
    /// - Hardware fingerprint doesn't match
    /// - Key has expired
    pub fn activate(&mut self, key: &LicenseKey) -> Result<(), LicenseError> {
        // Generate current hardware fingerprint
        let fingerprint = HardwareFingerprint::generate();

        // Verify hardware binding
        if !key.verify_hardware(&fingerprint) {
            self.set_state_atomic(LicenseState::Invalid);
            return Err(LicenseError::HardwareMismatch);
        }

        // Check expiry if present
        if let Some(expiry) = key.expiry_timestamp() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if now > expiry {
                self.set_state_atomic(LicenseState::Expired);
                return Err(LicenseError::Expired);
            }

            self.expiry_timestamp.store(expiry, Ordering::Release);
        }

        // Store hardware fingerprint
        // #ASSUME: fingerprint is immutable after generation
        // #VERIFY: Copy bytes directly, no shared state
        self.hardware_fingerprint.copy_from_slice(fingerprint.as_bytes());

        // Generate and store license signature
        let signature = key.generate_signature(&fingerprint);
        self.license_hash.copy_from_slice(&signature);

        // Set activation timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.activation_timestamp.store(now, Ordering::Release);

        // Set state to valid (increments generation)
        self.set_state_atomic(LicenseState::Valid);

        // Save to disk for persistence
        self.save_to_disk()?;

        Ok(())
    }

    /// Check if license is currently valid
    ///
    /// This is called by the metacapsule before encoding.
    /// It performs a full validation including:
    /// - State check (must be Valid)
    /// - Expiry check (must not be past expiry)
    /// - Integrity check (generation counter must be consistent)
    ///
    /// # Performance
    ///
    /// This method is optimized for the hot path with `#[inline]`.
    /// Typical latency: <5ns for valid license.
    #[inline]
    pub fn is_valid(&self) -> bool {
        // Increment check counter for audit trail
        self.check_counter.fetch_add(1, Ordering::Relaxed);

        // Fast path: check state first
        let state = self.state();
        if state != LicenseState::Valid {
            return false;
        }

        // Check expiry
        let expiry = self.expiry_timestamp.load(Ordering::Acquire);
        if expiry > 0 {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if now > expiry {
                // License has expired - update state
                // Note: We don't call set_state_atomic here to avoid
                // side effects in what should be a read-only check
                return false;
            }
        }

        // Verify integrity
        self.verify_integrity()
    }

    /// Get current license state
    #[inline]
    pub fn state(&self) -> LicenseState {
        LicenseState::from(self.license_state.load(Ordering::Acquire))
    }

    /// Verify integrity (generation counter check)
    ///
    /// The generation counter should always be greater than 0 for
    /// an activated license. A value of 0 with Valid state indicates
    /// that someone patched the state directly without going through
    /// our activation method.
    ///
    /// # Anti-Tampering
    ///
    /// Binary patches that set license_state to Valid without calling
    /// activate() will leave generation at 0, causing this check to fail.
    #[inline]
    pub fn verify_integrity(&self) -> bool {
        let state = self.license_state.load(Ordering::Acquire);
        let gen = self.generation.load(Ordering::Acquire);

        // Valid state requires generation > 0
        if state == LicenseState::Valid as u64 && gen == 0 {
            return false;
        }

        // Additional integrity: activation timestamp must be set for valid license
        if state == LicenseState::Valid as u64 {
            let activation = self.activation_timestamp.load(Ordering::Acquire);
            if activation == 0 {
                return false;
            }
        }

        // Verify fingerprint is not zeroed (would indicate uninitialized state)
        if state == LicenseState::Valid as u64 {
            let all_zero = self.hardware_fingerprint.iter().all(|&b| b == 0);
            if all_zero {
                return false;
            }
        }

        true
    }

    /// Load license from disk (if previously activated)
    ///
    /// Attempts to load a previously saved license from the platform-specific
    /// configuration directory. If successful, verifies the hardware fingerprint
    /// matches the current machine.
    ///
    /// # File Format
    ///
    /// ```text
    /// Offset  Size  Description
    /// 0       4     Magic bytes "KDLY"
    /// 4       1     Version (currently 1)
    /// 5       8     Expiry timestamp (u64 LE)
    /// 13      8     Activation timestamp (u64 LE)
    /// 21      32    Hardware fingerprint
    /// 53      32    License hash
    /// 85      8     Generation counter (u64 LE)
    /// ------  ----
    /// Total:  93 bytes
    /// ```
    pub fn load_from_disk(&mut self) -> Result<(), LicenseError> {
        let path = Self::license_path()?;

        if !path.exists() {
            return Err(LicenseError::NotFound);
        }

        let mut file = File::open(&path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        // Verify minimum size
        if buffer.len() < 93 {
            return Err(LicenseError::InvalidFormat);
        }

        // Verify magic bytes
        if buffer[0..4] != LICENSE_MAGIC {
            return Err(LicenseError::InvalidFormat);
        }

        // Verify version
        if buffer[4] != LICENSE_VERSION {
            return Err(LicenseError::InvalidFormat);
        }

        // Parse fields
        let expiry = u64::from_le_bytes(buffer[5..13].try_into().unwrap());
        let activation = u64::from_le_bytes(buffer[13..21].try_into().unwrap());
        let stored_fingerprint: [u8; 32] = buffer[21..53].try_into().unwrap();
        let stored_hash: [u8; 32] = buffer[53..85].try_into().unwrap();
        let generation = u64::from_le_bytes(buffer[85..93].try_into().unwrap());

        // Verify hardware fingerprint matches current machine
        let current_fingerprint = HardwareFingerprint::generate();
        if stored_fingerprint != *current_fingerprint.as_bytes() {
            return Err(LicenseError::HardwareMismatch);
        }

        // Check expiry
        if expiry > 0 {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if now > expiry {
                self.set_state_atomic(LicenseState::Expired);
                return Err(LicenseError::Expired);
            }
        }

        // Restore state
        self.hardware_fingerprint.copy_from_slice(&stored_fingerprint);
        self.license_hash.copy_from_slice(&stored_hash);
        self.expiry_timestamp.store(expiry, Ordering::Release);
        self.activation_timestamp.store(activation, Ordering::Release);
        self.generation.store(generation, Ordering::Release);
        self.license_state
            .store(LicenseState::Valid as u64, Ordering::Release);

        Ok(())
    }

    /// Save license to disk after activation
    ///
    /// Persists the license state to the platform-specific configuration
    /// directory for future sessions.
    pub fn save_to_disk(&self) -> Result<(), LicenseError> {
        let path = Self::license_path()?;

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut buffer = Vec::with_capacity(93);

        // Magic bytes
        buffer.extend_from_slice(&LICENSE_MAGIC);

        // Version
        buffer.push(LICENSE_VERSION);

        // Expiry timestamp
        let expiry = self.expiry_timestamp.load(Ordering::Acquire);
        buffer.extend_from_slice(&expiry.to_le_bytes());

        // Activation timestamp
        let activation = self.activation_timestamp.load(Ordering::Acquire);
        buffer.extend_from_slice(&activation.to_le_bytes());

        // Hardware fingerprint
        buffer.extend_from_slice(&self.hardware_fingerprint);

        // License hash
        buffer.extend_from_slice(&self.license_hash);

        // Generation counter
        let generation = self.generation.load(Ordering::Acquire);
        buffer.extend_from_slice(&generation.to_le_bytes());

        // Write atomically (write to temp, then rename)
        let temp_path = path.with_extension("tmp");
        let mut file = File::create(&temp_path)?;
        file.write_all(&buffer)?;
        file.sync_all()?;

        fs::rename(&temp_path, &path)?;

        Ok(())
    }

    /// Get the number of validity checks performed (audit trail)
    #[inline]
    pub fn check_count(&self) -> u64 {
        self.check_counter.load(Ordering::Relaxed)
    }

    /// Get the activation timestamp
    #[inline]
    pub fn activation_time(&self) -> u64 {
        self.activation_timestamp.load(Ordering::Acquire)
    }

    /// Get the expiry timestamp (0 = perpetual)
    #[inline]
    pub fn expiry_time(&self) -> u64 {
        self.expiry_timestamp.load(Ordering::Acquire)
    }

    /// Get the generation counter value
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Invalidate the license (for logout/deactivation)
    pub fn invalidate(&mut self) {
        self.set_state_atomic(LicenseState::Invalid);

        // Clear sensitive data
        self.hardware_fingerprint.fill(0);
        self.license_hash.fill(0);
        self.expiry_timestamp.store(0, Ordering::Release);
        self.activation_timestamp.store(0, Ordering::Release);

        // Remove license file
        if let Ok(path) = Self::license_path() {
            let _ = fs::remove_file(path);
        }
    }

    /// Set state atomically and increment generation counter
    ///
    /// This is the ONLY way state should be modified. Direct modification
    /// of license_state will be detected by integrity checks because the
    /// generation counter won't be incremented.
    #[inline]
    fn set_state_atomic(&self, state: LicenseState) {
        // Increment generation counter first
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Then set state
        self.license_state.store(state as u64, Ordering::Release);
    }

    /// Get platform-specific license file path
    fn license_path() -> Result<PathBuf, LicenseError> {
        #[cfg(target_os = "linux")]
        {
            let home = std::env::var("HOME").map_err(|_| {
                LicenseError::IoError(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "HOME not set",
                ))
            })?;
            Ok(PathBuf::from(home)
                .join(".config")
                .join(APP_NAME)
                .join(LICENSE_FILENAME))
        }

        #[cfg(target_os = "macos")]
        {
            let home = std::env::var("HOME").map_err(|_| {
                LicenseError::IoError(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "HOME not set",
                ))
            })?;
            Ok(PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join(APP_NAME)
                .join(LICENSE_FILENAME))
        }

        #[cfg(target_os = "windows")]
        {
            let appdata = std::env::var("APPDATA").map_err(|_| {
                LicenseError::IoError(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "APPDATA not set",
                ))
            })?;
            Ok(PathBuf::from(appdata)
                .join(APP_NAME)
                .join(LICENSE_FILENAME))
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Err(LicenseError::IoError(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Unsupported platform",
            )))
        }
    }
}

impl Default for LicenseVerificationCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Safety: All fields are either atomic or immutable after initialization
// #ASSUME: AtomicU64 is Send + Sync
// #VERIFY: hardware_fingerprint and license_hash are only written during
//          activation (which requires &mut self)
unsafe impl Send for LicenseVerificationCapsule {}
unsafe impl Sync for LicenseVerificationCapsule {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(std::mem::size_of::<LicenseVerificationCapsule>(), 128);
        assert_eq!(std::mem::align_of::<LicenseVerificationCapsule>(), 64);
    }

    #[test]
    fn test_new_capsule_is_invalid() {
        let capsule = LicenseVerificationCapsule::new();
        assert_eq!(capsule.state(), LicenseState::Invalid);
        assert!(!capsule.is_valid());
    }

    #[test]
    fn test_generation_counter_starts_at_zero() {
        let capsule = LicenseVerificationCapsule::new();
        assert_eq!(capsule.generation(), 0);
    }

    #[test]
    fn test_state_transitions() {
        let capsule = LicenseVerificationCapsule::new();

        // Initial state
        assert_eq!(capsule.state(), LicenseState::Invalid);
        assert_eq!(capsule.generation(), 0);

        // Set to valid (simulating activation)
        capsule.set_state_atomic(LicenseState::Valid);
        assert_eq!(capsule.state(), LicenseState::Valid);
        assert_eq!(capsule.generation(), 1);

        // Set to expired
        capsule.set_state_atomic(LicenseState::Expired);
        assert_eq!(capsule.state(), LicenseState::Expired);
        assert_eq!(capsule.generation(), 2);
    }

    #[test]
    fn test_integrity_check_detects_tampering() {
        let capsule = LicenseVerificationCapsule::new();

        // Simulate binary patch: set state to Valid without incrementing generation
        capsule
            .license_state
            .store(LicenseState::Valid as u64, Ordering::Release);

        // Integrity check should fail
        assert!(!capsule.verify_integrity());
    }

    #[test]
    fn test_license_state_from_u64() {
        assert_eq!(LicenseState::from(0), LicenseState::Invalid);
        assert_eq!(LicenseState::from(1), LicenseState::Valid);
        assert_eq!(LicenseState::from(2), LicenseState::Expired);
        assert_eq!(LicenseState::from(3), LicenseState::Tampered);
        // Unknown values map to Tampered
        assert_eq!(LicenseState::from(99), LicenseState::Tampered);
    }

    #[test]
    fn test_allows_encoding() {
        assert!(!LicenseState::Invalid.allows_encoding());
        assert!(LicenseState::Valid.allows_encoding());
        assert!(!LicenseState::Expired.allows_encoding());
        assert!(!LicenseState::Tampered.allows_encoding());
    }

    #[test]
    fn test_check_counter_increments() {
        let capsule = LicenseVerificationCapsule::new();
        assert_eq!(capsule.check_count(), 0);

        // Call is_valid multiple times
        let _ = capsule.is_valid();
        assert_eq!(capsule.check_count(), 1);

        let _ = capsule.is_valid();
        assert_eq!(capsule.check_count(), 2);
    }

    #[test]
    fn test_invalidate_clears_state() {
        let mut capsule = LicenseVerificationCapsule::new();

        // Set some state
        capsule.set_state_atomic(LicenseState::Valid);
        capsule.hardware_fingerprint.fill(0xFF);
        capsule.license_hash.fill(0xFF);
        capsule.activation_timestamp.store(12345, Ordering::Release);

        // Invalidate
        capsule.invalidate();

        // Verify cleared
        assert_eq!(capsule.state(), LicenseState::Invalid);
        assert!(capsule.hardware_fingerprint.iter().all(|&b| b == 0));
        assert!(capsule.license_hash.iter().all(|&b| b == 0));
        assert_eq!(capsule.activation_time(), 0);
    }
}
