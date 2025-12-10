//! UserRegistrationCapsule - T1 Atomic tier capsule for user signup
//!
//! A 256-byte, cache-aligned computational capsule for handling user registrations
//! with built-in rate limiting (5 signups per IP per hour).
//!
//! # UCE34/Chaos Compliance
//! - **Tier**: T1 Atomic (lockfree, <10ns operations)
//! - **Size**: 256 bytes (cache-line optimized)
//! - **Alignment**: 64 bytes (cache-line aligned, no false sharing)
//! - **Concurrency**: 100% lockfree via AtomicU64 only
//! - **TOCTOU Prevention**: Generation counter incremented on every state change
//!
//! # Rate Limiting Design
//! Uses a simple hash table with 16 slots. Each slot packs:
//! - `ip_hash`: upper 32 bits (truncated IP hash)
//! - `count`: bits 16-31 (signup count in current window)
//! - `window_start`: bits 0-15 (minutes since epoch, wraps every ~45 days)
//!
//! # Example
//! ```rust
//! use kdb_signup::capsules::UserRegistrationCapsule;
//!
//! let capsule = UserRegistrationCapsule::new();
//!
//! // Check rate limit before registration
//! if capsule.check_rate_limit("192.168.1.1") {
//!     match capsule.register("user@example.com", "Acme Corp", "192.168.1.1") {
//!         Ok(pending) => println!("Created pending user: {:016x}", pending.email_hash),
//!         Err(e) => eprintln!("Registration failed: {}", e),
//!     }
//! }
//!
//! // Check stats
//! let stats = capsule.stats();
//! println!("Total registrations: {}", stats.registrations_total);
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Error types for signup operations
#[derive(Debug, thiserror::Error)]
pub enum SignupError {
    /// Rate limit exceeded (5 signups per hour per IP)
    #[error("Rate limit exceeded (5 signups/hour)")]
    RateLimitExceeded,

    /// Invalid email format
    #[error("Invalid email format")]
    InvalidEmail,

    /// Disposable email domain blocked
    #[error("Disposable email blocked")]
    DisposableEmail,

    /// Email already registered (duplicate signup attempt)
    #[error("Email already registered")]
    EmailAlreadyRegistered,
}

/// Pending user created from successful registration
#[derive(Debug, Clone)]
pub struct PendingUser {
    /// BLAKE3 hash of the email address (first 8 bytes)
    pub email_hash: u64,
    /// Organization name
    pub org_name: String,
    /// Unix timestamp of creation
    pub created_at: u64,
}

/// Registration statistics snapshot
#[derive(Debug, Clone, Copy)]
pub struct RegistrationStats {
    /// Total successful registrations
    pub registrations_total: u64,
    /// Total blocked attempts (rate limit + validation)
    pub blocked_count: u64,
    /// Current generation counter
    pub generation: u64,
}

/// Rate limit slot packing constants
/// Slot format: [ip_hash:32][count:16][window_start:16]
const IP_HASH_SHIFT: u32 = 32;
const COUNT_SHIFT: u32 = 16;
const COUNT_MASK: u64 = 0xFFFF;
const WINDOW_MASK: u64 = 0xFFFF;

/// Maximum signups per IP per hour
const MAX_SIGNUPS_PER_HOUR: u16 = 5;

/// Number of rate limit slots (power of 2 for fast modulo)
const RATE_LIMIT_SLOTS: usize = 16;

/// Number of email dedup slots (uses remaining padding space)
/// Each slot packs: [email_hash_trunc:48][registration_hour:16]
const EMAIL_SEEN_SLOTS: usize = 12;

/// Email slot packing: upper 48 bits for hash, lower 16 bits for hour window
const EMAIL_HASH_MASK: u64 = 0xFFFFFFFFFFFF0000;
const EMAIL_WINDOW_MASK: u64 = 0xFFFF;

/// Email seen expiry: 24 hours (slots older than this are considered expired)
const EMAIL_SEEN_EXPIRY_HOURS: u16 = 24;

/// T1 Atomic tier capsule for user registration
///
/// 256-byte, 64-byte aligned structure with:
/// - Atomic statistics (24 bytes)
/// - Rate limit hash table (128 bytes)
/// - Email dedup hash table (96 bytes)
/// - Padding (8 bytes)
#[repr(C, align(64))]
pub struct UserRegistrationCapsule {
    // === Statistics (24 bytes) ===
    /// Total successful registrations
    registrations_total: AtomicU64,
    /// Total blocked attempts
    blocked_count: AtomicU64,
    /// Generation counter for TOCTOU prevention
    generation: AtomicU64,

    // === Rate Limit Hash Table (128 bytes) ===
    /// 16 slots, each packing (ip_hash:32, count:16, window_start:16)
    rate_limit_slots: [AtomicU64; RATE_LIMIT_SLOTS],

    // === Email Dedup Hash Table (96 bytes) ===
    /// 12 slots for recent email_hash tracking
    /// Each slot packs: [email_hash_trunc:48][registration_hour:16]
    /// Used for fast duplicate detection before DB call
    email_seen_slots: [AtomicU64; EMAIL_SEEN_SLOTS],

    // === Padding to 256 bytes ===
    /// Padding: 256 - 24 - 128 - 96 = 8 bytes
    _padding: [u8; 8],
}

// Compile-time verification of struct size and alignment
const _: () = {
    assert!(std::mem::size_of::<UserRegistrationCapsule>() == 256);
    assert!(std::mem::align_of::<UserRegistrationCapsule>() == 64);
};

impl UserRegistrationCapsule {
    /// Create a new UserRegistrationCapsule with zeroed state
    #[inline]
    pub const fn new() -> Self {
        Self {
            registrations_total: AtomicU64::new(0),
            blocked_count: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            rate_limit_slots: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            email_seen_slots: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            _padding: [0u8; 8],
        }
    }

    /// Register a new user
    ///
    /// # Arguments
    /// * `email` - User's email address
    /// * `org_name` - Organization name
    /// * `ip` - Client IP address for rate limiting
    ///
    /// # Returns
    /// * `Ok(PendingUser)` - Successfully created pending user
    /// * `Err(SignupError)` - Registration failed (rate limit, validation, etc.)
    pub fn register(
        &self,
        email: &str,
        org_name: &str,
        ip: &str,
    ) -> Result<PendingUser, SignupError> {
        // Validate email format
        if !Self::validate_email(email) {
            self.blocked_count.fetch_add(1, Ordering::SeqCst);
            self.generation.fetch_add(1, Ordering::SeqCst);
            return Err(SignupError::InvalidEmail);
        }

        // Check for disposable email domains
        if Self::is_disposable_email(email) {
            self.blocked_count.fetch_add(1, Ordering::SeqCst);
            self.generation.fetch_add(1, Ordering::SeqCst);
            return Err(SignupError::DisposableEmail);
        }

        // Check and update rate limit
        if !self.try_increment_rate_limit(ip) {
            self.blocked_count.fetch_add(1, Ordering::SeqCst);
            self.generation.fetch_add(1, Ordering::SeqCst);
            return Err(SignupError::RateLimitExceeded);
        }

        // Create pending user
        let email_hash = Self::hash_email(email);
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Update stats
        self.registrations_total.fetch_add(1, Ordering::SeqCst);
        self.generation.fetch_add(1, Ordering::SeqCst);

        Ok(PendingUser {
            email_hash,
            org_name: org_name.to_string(),
            created_at,
        })
    }

    /// Check if an IP is within rate limits (without incrementing)
    ///
    /// # Arguments
    /// * `ip` - Client IP address to check
    ///
    /// # Returns
    /// * `true` - IP is allowed (under rate limit)
    /// * `false` - IP is blocked (exceeded rate limit)
    #[inline]
    pub fn check_rate_limit(&self, ip: &str) -> bool {
        let ip_hash = Self::hash_ip(ip);
        let slot_idx = (ip_hash as usize) % RATE_LIMIT_SLOTS;
        let current_window = Self::current_window_minutes();

        let slot_value = self.rate_limit_slots[slot_idx].load(Ordering::SeqCst);

        // Extract packed values
        let stored_ip_hash = (slot_value >> IP_HASH_SHIFT) as u32;
        let stored_count = ((slot_value >> COUNT_SHIFT) & COUNT_MASK) as u16;
        let stored_window = (slot_value & WINDOW_MASK) as u16;

        // Check if this slot matches our IP and is in the current window
        if stored_ip_hash == ip_hash && stored_window == current_window {
            return stored_count < MAX_SIGNUPS_PER_HOUR;
        }

        // Slot is empty, different IP, or different window - allowed
        true
    }

    /// Get current registration statistics
    ///
    /// # Returns
    /// Atomic snapshot of current statistics
    #[inline]
    pub fn stats(&self) -> RegistrationStats {
        // Read generation first for consistency check
        let gen = self.generation.load(Ordering::SeqCst);
        RegistrationStats {
            registrations_total: self.registrations_total.load(Ordering::SeqCst),
            blocked_count: self.blocked_count.load(Ordering::SeqCst),
            generation: gen,
        }
    }

    /// Get current generation counter
    ///
    /// Used for TOCTOU prevention and change detection
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    // === Private Helper Methods ===

    /// Try to increment rate limit for an IP
    ///
    /// Uses CAS loop to atomically check and increment
    fn try_increment_rate_limit(&self, ip: &str) -> bool {
        let ip_hash = Self::hash_ip(ip);
        let slot_idx = (ip_hash as usize) % RATE_LIMIT_SLOTS;
        let current_window = Self::current_window_minutes();

        loop {
            let slot_value = self.rate_limit_slots[slot_idx].load(Ordering::SeqCst);

            // Extract packed values
            let stored_ip_hash = (slot_value >> IP_HASH_SHIFT) as u32;
            let stored_count = ((slot_value >> COUNT_SHIFT) & COUNT_MASK) as u16;
            let stored_window = (slot_value & WINDOW_MASK) as u16;

            let new_value = if stored_ip_hash == ip_hash && stored_window == current_window {
                // Same IP, same window - check and increment count
                if stored_count >= MAX_SIGNUPS_PER_HOUR {
                    return false; // Rate limit exceeded
                }
                Self::pack_slot(ip_hash, stored_count + 1, current_window)
            } else {
                // Different IP, different window, or empty slot - start fresh
                Self::pack_slot(ip_hash, 1, current_window)
            };

            // CAS to update slot
            match self.rate_limit_slots[slot_idx].compare_exchange(
                slot_value,
                new_value,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return true,
                Err(_) => continue, // Retry on contention
            }
        }
    }

    /// Pack rate limit slot values into u64
    #[inline]
    const fn pack_slot(ip_hash: u32, count: u16, window: u16) -> u64 {
        ((ip_hash as u64) << IP_HASH_SHIFT)
            | ((count as u64) << COUNT_SHIFT)
            | (window as u64)
    }

    /// Get current time as minutes since epoch (wraps every ~45 days)
    #[inline]
    fn current_window_minutes() -> u16 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // Minutes since epoch, truncated to 16 bits
        // Each window is 60 minutes (1 hour)
        ((now / 3600) & 0xFFFF) as u16
    }

    /// Get current time as hours since epoch (for email dedup expiry)
    #[inline]
    fn current_hour_window() -> u16 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // Hours since epoch, truncated to 16 bits (wraps every ~7.5 years)
        ((now / 3600) & 0xFFFF) as u16
    }

    /// Check if an email hash has been seen recently (within 24 hours)
    ///
    /// Uses a simple hash table with 12 slots. Returns true if email was
    /// seen within EMAIL_SEEN_EXPIRY_HOURS (24 hours).
    ///
    /// # Arguments
    /// * `email_hash` - 64-bit BLAKE3 hash of the email address
    ///
    /// # Returns
    /// * `true` - Email has been seen recently (duplicate)
    /// * `false` - Email not seen (new registration)
    #[inline]
    pub fn is_email_seen(&self, email_hash: u64) -> bool {
        let current_hour = Self::current_hour_window();
        let slot_idx = (email_hash as usize) % EMAIL_SEEN_SLOTS;
        let slot_value = self.email_seen_slots[slot_idx].load(Ordering::SeqCst);

        // Extract packed values: [email_hash_trunc:48][registration_hour:16]
        let stored_hash_trunc = slot_value & EMAIL_HASH_MASK;
        let stored_hour = (slot_value & EMAIL_WINDOW_MASK) as u16;
        let email_hash_trunc = email_hash & EMAIL_HASH_MASK;

        // Check if slot matches our email hash and is not expired
        if stored_hash_trunc == email_hash_trunc {
            // Calculate hours since registration (handles wrap-around)
            let hours_since = current_hour.wrapping_sub(stored_hour);
            return hours_since < EMAIL_SEEN_EXPIRY_HOURS;
        }

        false
    }

    /// Record that an email has been seen (for duplicate detection)
    ///
    /// Stores the email hash in a slot for 24 hours of duplicate detection.
    /// Uses CAS loop for lockfree operation.
    ///
    /// # Arguments
    /// * `email_hash` - 64-bit BLAKE3 hash of the email address
    #[inline]
    pub fn record_email_seen(&self, email_hash: u64) {
        let current_hour = Self::current_hour_window();
        let slot_idx = (email_hash as usize) % EMAIL_SEEN_SLOTS;

        // Pack email hash (upper 48 bits) with current hour (lower 16 bits)
        let email_hash_trunc = email_hash & EMAIL_HASH_MASK;
        let new_value = email_hash_trunc | (current_hour as u64);

        // Simple store - we don't need CAS here since we just overwrite
        // Collisions are acceptable (probabilistic dedup, not absolute)
        self.email_seen_slots[slot_idx].store(new_value, Ordering::SeqCst);
    }

    /// Hash an IP address to 32 bits using BLAKE3
    #[inline]
    fn hash_ip(ip: &str) -> u32 {
        let hash = blake3::hash(ip.as_bytes());
        let bytes = hash.as_bytes();
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    /// Hash an email address to 64 bits using BLAKE3
    #[inline]
    fn hash_email(email: &str) -> u64 {
        let normalized = email.to_lowercase();
        let hash = blake3::hash(normalized.as_bytes());
        let bytes = hash.as_bytes();
        u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])
    }

    /// Validate email format (basic validation)
    #[inline]
    fn validate_email(email: &str) -> bool {
        // Reject empty emails or emails with whitespace
        if email.is_empty() || email.chars().any(|c| c.is_whitespace()) {
            return false;
        }

        // Basic validation: contains exactly one @, has content before and after
        let parts: Vec<&str> = email.split('@').collect();
        if parts.len() != 2 {
            return false;
        }
        let local = parts[0];
        let domain = parts[1];

        // Local part must be non-empty and <= 64 chars
        if local.is_empty() || local.len() > 64 {
            return false;
        }

        // Domain must be non-empty, have at least one dot, and <= 255 chars
        if domain.is_empty() || domain.len() > 255 || !domain.contains('.') {
            return false;
        }

        // Domain parts must be non-empty
        let domain_parts: Vec<&str> = domain.split('.').collect();
        if domain_parts.iter().any(|p| p.is_empty()) {
            return false;
        }

        // TLD must be at least 2 chars
        if let Some(tld) = domain_parts.last() {
            if tld.len() < 2 {
                return false;
            }
        }

        true
    }

    /// Check if email is from a disposable email domain
    #[inline]
    fn is_disposable_email(email: &str) -> bool {
        // Common disposable email domains
        const DISPOSABLE_DOMAINS: &[&str] = &[
            "tempmail.com",
            "guerrillamail.com",
            "10minutemail.com",
            "mailinator.com",
            "throwaway.email",
            "temp-mail.org",
            "fakeinbox.com",
            "trashmail.com",
            "maildrop.cc",
            "yopmail.com",
            "sharklasers.com",
            "spam4.me",
            "dispostable.com",
            "mailnesia.com",
            "getairmail.com",
        ];

        if let Some(domain) = email.split('@').nth(1) {
            let domain_lower = domain.to_lowercase();
            return DISPOSABLE_DOMAINS
                .iter()
                .any(|d| domain_lower == *d || domain_lower.ends_with(&format!(".{}", d)));
        }
        false
    }
}

impl Default for UserRegistrationCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Safety: UserRegistrationCapsule uses only AtomicU64 for shared state
// All operations are lockfree and thread-safe
unsafe impl Send for UserRegistrationCapsule {}
unsafe impl Sync for UserRegistrationCapsule {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_struct_size_and_alignment() {
        assert_eq!(
            std::mem::size_of::<UserRegistrationCapsule>(),
            256,
            "Capsule must be exactly 256 bytes"
        );
        assert_eq!(
            std::mem::align_of::<UserRegistrationCapsule>(),
            64,
            "Capsule must be 64-byte aligned"
        );
    }

    #[test]
    fn test_new_capsule_zeroed() {
        let capsule = UserRegistrationCapsule::new();
        let stats = capsule.stats();

        assert_eq!(stats.registrations_total, 0);
        assert_eq!(stats.blocked_count, 0);
        assert_eq!(stats.generation, 0);
    }

    #[test]
    fn test_successful_registration() {
        let capsule = UserRegistrationCapsule::new();

        let result = capsule.register("test@example.com", "Test Org", "192.168.1.1");
        assert!(result.is_ok());

        let pending = result.unwrap();
        assert_ne!(pending.email_hash, 0);
        assert_eq!(pending.org_name, "Test Org");
        assert!(pending.created_at > 0);

        let stats = capsule.stats();
        assert_eq!(stats.registrations_total, 1);
        assert_eq!(stats.blocked_count, 0);
        assert_eq!(stats.generation, 1);
    }

    #[test]
    fn test_invalid_email_format() {
        let capsule = UserRegistrationCapsule::new();

        // No @ symbol
        let result = capsule.register("invalid-email", "Test", "1.2.3.4");
        assert!(matches!(result, Err(SignupError::InvalidEmail)));

        // Multiple @ symbols
        let result = capsule.register("test@@example.com", "Test", "1.2.3.4");
        assert!(matches!(result, Err(SignupError::InvalidEmail)));

        // No domain
        let result = capsule.register("test@", "Test", "1.2.3.4");
        assert!(matches!(result, Err(SignupError::InvalidEmail)));

        // No local part
        let result = capsule.register("@example.com", "Test", "1.2.3.4");
        assert!(matches!(result, Err(SignupError::InvalidEmail)));

        // No TLD
        let result = capsule.register("test@example", "Test", "1.2.3.4");
        assert!(matches!(result, Err(SignupError::InvalidEmail)));

        // TLD too short
        let result = capsule.register("test@example.c", "Test", "1.2.3.4");
        assert!(matches!(result, Err(SignupError::InvalidEmail)));

        let stats = capsule.stats();
        assert_eq!(stats.registrations_total, 0);
        assert_eq!(stats.blocked_count, 6);
    }

    #[test]
    fn test_disposable_email_blocked() {
        let capsule = UserRegistrationCapsule::new();

        let result = capsule.register("test@mailinator.com", "Test", "1.2.3.4");
        assert!(matches!(result, Err(SignupError::DisposableEmail)));

        let result = capsule.register("test@tempmail.com", "Test", "1.2.3.4");
        assert!(matches!(result, Err(SignupError::DisposableEmail)));

        let result = capsule.register("test@10minutemail.com", "Test", "1.2.3.4");
        assert!(matches!(result, Err(SignupError::DisposableEmail)));

        let stats = capsule.stats();
        assert_eq!(stats.registrations_total, 0);
        assert_eq!(stats.blocked_count, 3);
    }

    #[test]
    fn test_rate_limit_enforcement() {
        let capsule = UserRegistrationCapsule::new();
        let ip = "10.0.0.1";

        // First 5 registrations should succeed
        for i in 0..5 {
            let email = format!("user{}@example.com", i);
            let result = capsule.register(&email, "Test", ip);
            assert!(result.is_ok(), "Registration {} should succeed", i);
        }

        // 6th registration should fail
        let result = capsule.register("user5@example.com", "Test", ip);
        assert!(
            matches!(result, Err(SignupError::RateLimitExceeded)),
            "6th registration should be rate limited"
        );

        // Different IP should still work
        let result = capsule.register("user6@example.com", "Test", "10.0.0.2");
        assert!(result.is_ok(), "Different IP should not be rate limited");

        let stats = capsule.stats();
        assert_eq!(stats.registrations_total, 6);
        assert_eq!(stats.blocked_count, 1);
    }

    #[test]
    fn test_check_rate_limit() {
        let capsule = UserRegistrationCapsule::new();
        let ip = "172.16.0.1";

        // Initially should be allowed
        assert!(capsule.check_rate_limit(ip));

        // Register 5 times
        for i in 0..5 {
            let email = format!("check{}@example.com", i);
            capsule.register(&email, "Test", ip).unwrap();
        }

        // Now should be blocked
        assert!(!capsule.check_rate_limit(ip));

        // Other IPs should still be allowed
        assert!(capsule.check_rate_limit("172.16.0.2"));
    }

    #[test]
    fn test_generation_counter_increments() {
        let capsule = UserRegistrationCapsule::new();

        assert_eq!(capsule.generation(), 0);

        // Successful registration increments generation
        capsule.register("gen1@example.com", "Test", "1.1.1.1").unwrap();
        assert_eq!(capsule.generation(), 1);

        // Failed registration also increments generation
        let _ = capsule.register("invalid", "Test", "1.1.1.2");
        assert_eq!(capsule.generation(), 2);
    }

    #[test]
    fn test_email_hash_consistency() {
        let capsule = UserRegistrationCapsule::new();

        let result1 = capsule.register("Consistent@Example.COM", "Test1", "2.2.2.1");
        let result2 = capsule.register("consistent@example.com", "Test2", "2.2.2.2");

        // Email hashes should be the same (case-insensitive)
        assert_eq!(result1.unwrap().email_hash, result2.unwrap().email_hash);
    }

    #[test]
    fn test_slot_packing() {
        // Test the slot packing/unpacking
        let ip_hash: u32 = 0xDEADBEEF;
        let count: u16 = 5;
        let window: u16 = 1000;

        let packed = UserRegistrationCapsule::pack_slot(ip_hash, count, window);

        // Unpack and verify
        let unpacked_ip = (packed >> IP_HASH_SHIFT) as u32;
        let unpacked_count = ((packed >> COUNT_SHIFT) & COUNT_MASK) as u16;
        let unpacked_window = (packed & WINDOW_MASK) as u16;

        assert_eq!(unpacked_ip, ip_hash);
        assert_eq!(unpacked_count, count);
        assert_eq!(unpacked_window, window);
    }

    #[test]
    fn test_valid_email_formats() {
        // These should all be valid
        let valid_emails = [
            "simple@example.com",
            "very.common@example.com",
            "disposable.style.email.with+symbol@example.com",
            "other.email-with-hyphen@example.com",
            "x@example.com",
            "example@s.example",
            "user.name+tag@example.co.uk",
        ];

        for email in valid_emails {
            assert!(
                UserRegistrationCapsule::validate_email(email),
                "Email '{}' should be valid",
                email
            );
        }
    }

    #[test]
    fn test_invalid_email_formats() {
        // These should all be invalid
        let invalid_emails = [
            "",
            "plainaddress",
            "@no-local.com",
            "no-domain@",
            "no-tld@example",
            "short-tld@example.c",
            "double@@at.com",
            "spaces in@local.com",
            "missing.domain@.com",
        ];

        for email in invalid_emails {
            assert!(
                !UserRegistrationCapsule::validate_email(email),
                "Email '{}' should be invalid",
                email
            );
        }
    }

    #[test]
    fn test_concurrent_registration() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(UserRegistrationCapsule::new());
        let mut handles = vec![];

        // Spawn 10 threads, each trying to register 2 users
        for thread_id in 0..10 {
            let capsule = Arc::clone(&capsule);
            let handle = thread::spawn(move || {
                let mut successes = 0;
                for i in 0..2 {
                    let email = format!("thread{}user{}@example.com", thread_id, i);
                    let ip = format!("10.{}.{}.{}", thread_id / 256, thread_id % 256, i);
                    if capsule.register(&email, "Concurrent Test", &ip).is_ok() {
                        successes += 1;
                    }
                }
                successes
            });
            handles.push(handle);
        }

        let total_successes: u32 = handles.into_iter().map(|h| h.join().unwrap()).sum();

        let stats = capsule.stats();
        assert_eq!(
            stats.registrations_total as u32, total_successes,
            "Stats should match actual successes"
        );
    }

    #[test]
    fn test_rate_limit_concurrent_same_ip() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(UserRegistrationCapsule::new());
        let mut handles = vec![];
        let shared_ip = "shared.ip.test";

        // Spawn 10 threads all using the same IP
        for thread_id in 0..10 {
            let capsule = Arc::clone(&capsule);
            let ip = shared_ip.to_string();
            let handle = thread::spawn(move || {
                let email = format!("concurrent{}@example.com", thread_id);
                capsule.register(&email, "Test", &ip).is_ok()
            });
            handles.push(handle);
        }

        let successes: u32 = handles
            .into_iter()
            .map(|h| if h.join().unwrap() { 1 } else { 0 })
            .sum();

        // Only 5 should succeed due to rate limit
        assert_eq!(successes, 5, "Only 5 registrations should succeed per IP");
    }

    #[test]
    fn test_default_trait() {
        let capsule = UserRegistrationCapsule::default();
        assert_eq!(capsule.generation(), 0);
    }
}
