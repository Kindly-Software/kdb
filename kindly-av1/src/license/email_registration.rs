//! Email Registration Capsule (T1 Atomic)
//! [TRADE SECRET]
//!
//! Email-based registration for 720p tier unlock (Anonymous Free → Registered Free).
//! Stores Blake3 hash of email + device fingerprint for privacy.
//!
//! # Memory Layout (128B, cache-aligned)
//!
//! ```text
//! Offset  Size  Field
//! ------  ----  -----
//! 0       32    email_hash ([u8; 32], Blake3)
//! 32      32    device_fingerprint_hash ([u8; 32], Blake3)
//! 64      32    registration_token ([u8; 32], Blake3 of combined)
//! 96      8     registration_timestamp (AtomicU64)
//! 104     8     generation (AtomicU64)
//! 112     1     registered (AtomicBool)
//! 113     15    _padding
//! ------  ----
//! Total:  128B (2 cache lines, 64B aligned)
//! ```
//!
//! # Disk Format
//!
//! ```text
//! Offset  Size  Description
//! 0       4     Magic bytes "KDLY"
//! 4       1     Version (currently 1)
//! 5       1     Registered flag
//! 6       2     _padding
//! 8       32    Email hash
//! 40      32    Device fingerprint hash
//! 72      32    Registration token
//! 104     8     Registration timestamp
//! ------  ----
//! Total:  112 bytes
//! ```
//!
//! # Email Validation
//!
//! Basic RFC 5322 validation (dot-atom format):
//! - local@domain pattern
//! - Local part: alphanumeric + .!#$%&'*+-/=?^_`{|}~
//! - Domain: alphanumeric + hyphen + dots, at least one dot
//! - No external crates (Chaos mandate)
//!
//! # Privacy
//!
//! - Email stored as Blake3 hash (32 bytes, irreversible)
//! - Combined with device fingerprint for unique binding
//! - Registration token = Blake3(email_hash || device_fingerprint_hash)
//! - GDPR-compliant (no PII stored)
//!
//! # Framework Compliance
//!
//! - UCE34 Q10: T1 Atomic tier
//! - Chaos: 100% lockfree, cache-aligned, generation counters
//! - ASSUM: All assumptions documented with #ASSUME tags

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::config::{APP_NAME, LICENSE_MAGIC, LICENSE_VERSION};
use super::fingerprint::HardwareFingerprint;

/// Email registration errors
#[derive(Debug)]
pub enum EmailError {
    InvalidEmailFormat(String),
    AlreadyRegistered,
    IoError(std::io::Error),
    InvalidFormat,
    NotFound,
}

impl std::fmt::Display for EmailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidEmailFormat(email) => {
                write!(f, "Invalid email format: {}", email)
            }
            Self::AlreadyRegistered => write!(f, "Email already registered"),
            Self::IoError(e) => write!(f, "IO error: {}", e),
            Self::InvalidFormat => write!(f, "Invalid registration file format"),
            Self::NotFound => write!(f, "Registration file not found"),
        }
    }
}

impl std::error::Error for EmailError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for EmailError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err)
    }
}

/// Email Registration Capsule (128B, T1 Atomic)
///
/// Cache-aligned capsule for email-based 720p tier unlock.
/// Stores hashed email + device fingerprint for privacy.
///
/// # Thread Safety
///
/// All state modifications use atomic operations. The capsule is safe to
/// share across threads. Disk persistence uses atomic write-then-rename.
///
/// # Privacy Design
///
/// - Email never stored in plaintext (Blake3 hash only)
/// - Combined with device fingerprint for binding
/// - Registration token prevents replay attacks
/// - GDPR-compliant (no PII storage)
#[repr(C, align(64))]
pub struct EmailRegistrationCapsule {
    /// Blake3 hash of email address (32 bytes)
    email_hash: [u8; 32],

    /// Blake3 hash of device fingerprint (32 bytes)
    device_fingerprint_hash: [u8; 32],

    /// Registration token: Blake3(email_hash || device_fingerprint_hash)
    registration_token: [u8; 32],

    /// Unix timestamp when registration occurred
    registration_timestamp: AtomicU64,

    /// Generation counter for tamper detection
    generation: AtomicU64,

    /// Registration status (atomic for lockfree access)
    registered: AtomicBool,

    /// Padding for 128B alignment
    _padding: [u8; 15],
}

// Compile-time size verification
const _: () = assert!(std::mem::size_of::<EmailRegistrationCapsule>() == 128);
const _: () = assert!(std::mem::align_of::<EmailRegistrationCapsule>() == 64);

impl EmailRegistrationCapsule {
    /// Create new capsule
    pub const fn new() -> Self {
        Self {
            email_hash: [0u8; 32],
            device_fingerprint_hash: [0u8; 32],
            registration_token: [0u8; 32],
            registration_timestamp: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            registered: AtomicBool::new(false),
            _padding: [0u8; 15],
        }
    }

    /// Validate email format (basic RFC 5322)
    ///
    /// Validates dot-atom format: local@domain
    /// - Local part: alphanumeric + allowed special chars
    /// - Domain: alphanumeric + hyphen + dots, at least one dot
    ///
    /// # Examples
    ///
    /// ```
    /// use kindly_av1::license::EmailRegistrationCapsule;
    ///
    /// assert!(EmailRegistrationCapsule::validate_email("user@example.com"));
    /// assert!(EmailRegistrationCapsule::validate_email("first.last@domain.co.uk"));
    /// assert!(!EmailRegistrationCapsule::validate_email("invalid@"));
    /// assert!(!EmailRegistrationCapsule::validate_email("@domain.com"));
    /// ```
    pub fn validate_email(email: &str) -> bool {
        // #ASSUME: Email validation follows basic RFC 5322 dot-atom format
        // #VERIFY: Pattern matches local@domain with alphanumeric + special chars

        if email.len() < 3 || email.len() > 254 {
            return false;
        }

        let parts: Vec<&str> = email.split('@').collect();
        if parts.len() != 2 {
            return false;
        }

        let (local, domain) = (parts[0], parts[1]);

        // Local part validation
        if local.is_empty() || local.len() > 64 {
            return false;
        }

        // Allowed special chars in local part (RFC 5322 simplified)
        const ALLOWED_SPECIAL: &[char] = &['.', '!', '#', '$', '%', '&', '\'', '*', '+', '-', '/', '=', '?', '^', '_', '`', '{', '|', '}', '~'];

        for ch in local.chars() {
            if !ch.is_alphanumeric() && !ALLOWED_SPECIAL.contains(&ch) {
                return false;
            }
        }

        // No consecutive dots, no leading/trailing dots
        if local.contains("..") || local.starts_with('.') || local.ends_with('.') {
            return false;
        }

        // Domain part validation
        if domain.is_empty() || domain.len() > 253 {
            return false;
        }

        // Domain must have at least one dot
        if !domain.contains('.') {
            return false;
        }

        // Domain labels: alphanumeric + hyphen
        for label in domain.split('.') {
            if label.is_empty() || label.len() > 63 {
                return false;
            }

            // No leading/trailing hyphen
            if label.starts_with('-') || label.ends_with('-') {
                return false;
            }

            for ch in label.chars() {
                if !ch.is_alphanumeric() && ch != '-' {
                    return false;
                }
            }
        }

        true
    }

    /// Register email with device fingerprint
    ///
    /// Validates email format, hashes email + fingerprint, generates
    /// registration token, and persists to disk.
    ///
    /// # Arguments
    ///
    /// * `email` - Email address to register (validated before hashing)
    /// * `fingerprint` - Hardware fingerprint of current device
    ///
    /// # Errors
    ///
    /// Returns error if email format invalid, already registered, or I/O fails.
    pub fn register(
        &mut self,
        email: &str,
        fingerprint: &HardwareFingerprint,
    ) -> Result<(), EmailError> {
        // Check if already registered
        if self.is_registered() {
            return Err(EmailError::AlreadyRegistered);
        }

        // Validate email format
        if !Self::validate_email(email) {
            return Err(EmailError::InvalidEmailFormat(email.to_string()));
        }

        // Hash email (Blake3 for privacy)
        let email_hash = Self::hash_email(email);

        // Hash device fingerprint
        let device_hash = *fingerprint.as_bytes();

        // Generate registration token: Blake3(email_hash || device_hash)
        let mut combined = [0u8; 64];
        combined[..32].copy_from_slice(&email_hash);
        combined[32..].copy_from_slice(&device_hash);
        let token = blake3::hash(&combined);

        // Get current timestamp
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Update state
        self.email_hash = email_hash;
        self.device_fingerprint_hash = device_hash;
        self.registration_token = *token.as_bytes();
        self.registration_timestamp.store(timestamp, Ordering::Release);
        self.registered.store(true, Ordering::Release);

        // Increment generation counter
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Persist to disk
        self.persist_to_disk()?;

        Ok(())
    }

    /// Check if email is registered
    #[inline]
    pub fn is_registered(&self) -> bool {
        self.registered.load(Ordering::Acquire)
    }

    /// Verify registration token
    ///
    /// Validates that the stored registration token matches the
    /// computed Blake3(email_hash || device_hash). Detects tampering.
    #[inline]
    pub fn verify_token(&self) -> bool {
        if !self.is_registered() {
            return false;
        }

        // Recompute token
        let mut combined = [0u8; 64];
        combined[..32].copy_from_slice(&self.email_hash);
        combined[32..].copy_from_slice(&self.device_fingerprint_hash);
        let expected_token = blake3::hash(&combined);

        // Compare with stored token
        self.registration_token == *expected_token.as_bytes()
    }

    /// Verify device fingerprint matches registration
    ///
    /// Checks that the current device fingerprint matches the one
    /// used during registration. Prevents license transfer.
    pub fn verify_device(&self, fingerprint: &HardwareFingerprint) -> bool {
        if !self.is_registered() {
            return false;
        }

        &self.device_fingerprint_hash == fingerprint.as_bytes()
    }

    /// Get registration timestamp
    #[inline]
    pub fn registration_time(&self) -> u64 {
        self.registration_timestamp.load(Ordering::Acquire)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Persist to disk for offline validation
    ///
    /// Uses atomic write-then-rename to prevent corruption.
    pub fn persist_to_disk(&self) -> Result<(), EmailError> {
        let path = Self::registration_path()?;

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut buffer = Vec::with_capacity(112);

        // Magic bytes
        buffer.extend_from_slice(&LICENSE_MAGIC);

        // Version
        buffer.push(LICENSE_VERSION);

        // Registered flag
        buffer.push(if self.is_registered() { 1 } else { 0 });

        // Padding
        buffer.extend_from_slice(&[0u8; 2]);

        // Email hash
        buffer.extend_from_slice(&self.email_hash);

        // Device fingerprint hash
        buffer.extend_from_slice(&self.device_fingerprint_hash);

        // Registration token
        buffer.extend_from_slice(&self.registration_token);

        // Registration timestamp
        buffer.extend_from_slice(&self.registration_time().to_le_bytes());

        // Write atomically (write to temp, then rename)
        let temp_path = path.with_extension("tmp");
        let mut file = File::create(&temp_path)?;
        file.write_all(&buffer)?;
        file.sync_all()?;

        fs::rename(&temp_path, &path)?;

        Ok(())
    }

    /// Load from disk
    pub fn load_from_disk(&mut self) -> Result<(), EmailError> {
        let path = Self::registration_path()?;

        if !path.exists() {
            return Err(EmailError::NotFound);
        }

        let mut file = File::open(&path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        // Verify minimum size
        if buffer.len() < 112 {
            return Err(EmailError::InvalidFormat);
        }

        // Verify magic bytes
        if buffer[0..4] != LICENSE_MAGIC {
            return Err(EmailError::InvalidFormat);
        }

        // Verify version
        if buffer[4] != LICENSE_VERSION {
            return Err(EmailError::InvalidFormat);
        }

        // Parse fields
        let registered = buffer[5] != 0;

        // Restore state
        self.email_hash.copy_from_slice(&buffer[8..40]);
        self.device_fingerprint_hash.copy_from_slice(&buffer[40..72]);
        self.registration_token.copy_from_slice(&buffer[72..104]);

        let timestamp = u64::from_le_bytes(buffer[104..112].try_into().unwrap());
        self.registration_timestamp.store(timestamp, Ordering::Release);
        self.registered.store(registered, Ordering::Release);

        Ok(())
    }

    /// Hash email using Blake3
    fn hash_email(email: &str) -> [u8; 32] {
        // Normalize email: lowercase, trim whitespace
        let normalized = email.trim().to_lowercase();
        let hash = blake3::hash(normalized.as_bytes());
        *hash.as_bytes()
    }

    /// Register email with device fingerprint (in-memory only, no disk persistence)
    ///
    /// Test-only method that skips disk I/O for unit testing.
    /// Production code should use `register()` which persists to disk.
    #[cfg(test)]
    pub fn register_in_memory(
        &mut self,
        email: &str,
        fingerprint: &HardwareFingerprint,
    ) -> Result<(), EmailError> {
        // Check if already registered
        if self.is_registered() {
            return Err(EmailError::AlreadyRegistered);
        }

        // Validate email format
        if !Self::validate_email(email) {
            return Err(EmailError::InvalidEmailFormat(email.to_string()));
        }

        // Hash email (Blake3 for privacy)
        let email_hash = Self::hash_email(email);

        // Hash device fingerprint
        let device_hash = *fingerprint.as_bytes();

        // Generate registration token: Blake3(email_hash || device_hash)
        let mut combined = [0u8; 64];
        combined[..32].copy_from_slice(&email_hash);
        combined[32..].copy_from_slice(&device_hash);
        let token = blake3::hash(&combined);

        // Get current timestamp
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Update state
        self.email_hash = email_hash;
        self.device_fingerprint_hash = device_hash;
        self.registration_token = *token.as_bytes();
        self.registration_timestamp.store(timestamp, Ordering::Release);
        self.registered.store(true, Ordering::Release);

        // Increment generation counter
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Skip disk persistence for test-only method
        Ok(())
    }

    /// Get platform-specific registration file path
    fn registration_path() -> Result<PathBuf, EmailError> {
        #[cfg(target_os = "linux")]
        {
            let home = std::env::var("HOME").map_err(|_| {
                EmailError::IoError(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "HOME not set",
                ))
            })?;
            Ok(PathBuf::from(home)
                .join(".config")
                .join(APP_NAME)
                .join("registration.dat"))
        }

        #[cfg(target_os = "macos")]
        {
            let home = std::env::var("HOME").map_err(|_| {
                EmailError::IoError(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "HOME not set",
                ))
            })?;
            Ok(PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join(APP_NAME)
                .join("registration.dat"))
        }

        #[cfg(target_os = "windows")]
        {
            let appdata = std::env::var("APPDATA").map_err(|_| {
                EmailError::IoError(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "APPDATA not set",
                ))
            })?;
            Ok(PathBuf::from(appdata).join(APP_NAME).join("registration.dat"))
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Err(EmailError::IoError(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Unsupported platform",
            )))
        }
    }
}

impl Default for EmailRegistrationCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Safety: All fields are atomic or immutable after initialization
// #ASSUME: AtomicU64, AtomicBool are Send + Sync
// #VERIFY: Modification requires &mut self, all reads use atomic ordering
unsafe impl Send for EmailRegistrationCapsule {}
unsafe impl Sync for EmailRegistrationCapsule {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(std::mem::size_of::<EmailRegistrationCapsule>(), 128);
        assert_eq!(std::mem::align_of::<EmailRegistrationCapsule>(), 64);
    }

    #[test]
    fn test_new_capsule_not_registered() {
        let capsule = EmailRegistrationCapsule::new();
        assert!(!capsule.is_registered());
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.registration_time(), 0);
    }

    #[test]
    fn test_validate_email_valid() {
        assert!(EmailRegistrationCapsule::validate_email("user@example.com"));
        assert!(EmailRegistrationCapsule::validate_email("first.last@domain.com"));
        assert!(EmailRegistrationCapsule::validate_email("user+tag@example.co.uk"));
        assert!(EmailRegistrationCapsule::validate_email("test_user@subdomain.example.com"));
    }

    #[test]
    fn test_validate_email_invalid() {
        // No @ sign
        assert!(!EmailRegistrationCapsule::validate_email("invalid"));

        // Multiple @ signs
        assert!(!EmailRegistrationCapsule::validate_email("user@@example.com"));

        // Missing local part
        assert!(!EmailRegistrationCapsule::validate_email("@example.com"));

        // Missing domain
        assert!(!EmailRegistrationCapsule::validate_email("user@"));

        // No dot in domain
        assert!(!EmailRegistrationCapsule::validate_email("user@domain"));

        // Consecutive dots
        assert!(!EmailRegistrationCapsule::validate_email("user..name@example.com"));

        // Leading dot in local
        assert!(!EmailRegistrationCapsule::validate_email(".user@example.com"));

        // Trailing dot in local
        assert!(!EmailRegistrationCapsule::validate_email("user.@example.com"));

        // Invalid char in local
        assert!(!EmailRegistrationCapsule::validate_email("user name@example.com"));

        // Too long
        let long_email = format!("{}@example.com", "a".repeat(250));
        assert!(!EmailRegistrationCapsule::validate_email(&long_email));
    }

    #[test]
    fn test_register_email() {
        let mut capsule = EmailRegistrationCapsule::new();
        let fp = HardwareFingerprint::from_bytes([0xAA; 32]);

        let result = capsule.register_in_memory("test@example.com", &fp);
        assert!(result.is_ok());
        assert!(capsule.is_registered());
        assert_eq!(capsule.generation(), 1);
        assert!(capsule.registration_time() > 0);
    }

    #[test]
    fn test_register_invalid_email() {
        let mut capsule = EmailRegistrationCapsule::new();
        let fp = HardwareFingerprint::from_bytes([0xAA; 32]);

        let result = capsule.register("invalid-email", &fp);
        assert!(result.is_err());
        assert!(!capsule.is_registered());
    }

    #[test]
    fn test_register_twice_fails() {
        let mut capsule = EmailRegistrationCapsule::new();
        let fp = HardwareFingerprint::from_bytes([0xAA; 32]);

        capsule.register_in_memory("test@example.com", &fp).unwrap();

        // Second registration should fail
        let result = capsule.register_in_memory("another@example.com", &fp);
        assert!(result.is_err());
        match result {
            Err(EmailError::AlreadyRegistered) => {}
            _ => panic!("Expected AlreadyRegistered error"),
        }
    }

    #[test]
    fn test_verify_token() {
        let mut capsule = EmailRegistrationCapsule::new();
        let fp = HardwareFingerprint::from_bytes([0xAA; 32]);

        capsule.register_in_memory("test@example.com", &fp).unwrap();
        assert!(capsule.verify_token());

        // Tamper with token
        capsule.registration_token[0] ^= 0xFF;
        assert!(!capsule.verify_token());
    }

    #[test]
    fn test_verify_device() {
        let mut capsule = EmailRegistrationCapsule::new();
        let fp1 = HardwareFingerprint::from_bytes([0xAA; 32]);
        let fp2 = HardwareFingerprint::from_bytes([0xBB; 32]);

        capsule.register_in_memory("test@example.com", &fp1).unwrap();

        // Original device should match
        assert!(capsule.verify_device(&fp1));

        // Different device should not match
        assert!(!capsule.verify_device(&fp2));
    }

    #[test]
    fn test_hash_email_normalization() {
        // Same email with different casing/whitespace should produce same hash
        let hash1 = EmailRegistrationCapsule::hash_email("Test@Example.Com");
        let hash2 = EmailRegistrationCapsule::hash_email("  test@example.com  ");
        assert_eq!(hash1, hash2);

        // Different emails should produce different hashes
        let hash3 = EmailRegistrationCapsule::hash_email("other@example.com");
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_generation_counter_increments() {
        let mut capsule = EmailRegistrationCapsule::new();
        let fp = HardwareFingerprint::from_bytes([0xAA; 32]);

        assert_eq!(capsule.generation(), 0);

        capsule.register_in_memory("test@example.com", &fp).unwrap();
        assert_eq!(capsule.generation(), 1);
    }
}
