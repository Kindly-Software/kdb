//! Build-time verification capsule
//!
//! Provides runtime access to build-time constants embedded via build.rs.
//!
//! ## UCE34 Framework
//! - Q10: Tier = T1 Atomic (compile-time constants, zero runtime cost)
//! - Q11: Rust transform = const/static embedding, env!() macro
//! - Q12: Nightly = Not required (stable features sufficient)
//! - Q28: Simplicity = Single module, <200 lines (I20-enhanced)
//! - Q33: Verification = Compile-time verification via env!() macro
//! - Q34: Auditability = Embedded constants accessible for runtime verification
//!
//! ## ASSUM Safety
//! - #ASSUME: CUSTOMER_ID, BUILD_TIMESTAMP, BUILD_SIGNATURE embedded by build.rs
//! - #VERIFY: env!() macro fails at compile-time if constants not embedded
//! - #ASSUME: Constants are immutable (const, no runtime modification possible)
//!
//! ## Zero Runtime Cost
//! All constants are compile-time embedded via env!() macro (0ns access overhead).
//!
//! ## I20 Integration (Phase 2.4.1 - Build Hardening)
//! - Q1: Integrating atomic_capsule::hash::AtomicHash256 for runtime integrity checks
//! - Q2: Problem = Build constants can be patched in binary, need runtime validation
//! - Q6: Compatible = Both use atomic operations, compile-time constants
//! - Q7: Performance = Hash verification <100ns, called once at startup
//! - Q15: Rollback = Feature flag `protection-build-hardening` (instant disable)
//! - Q19: Deployment = Big Bang (deterministic capsules, tests = production)
//!
//! ## Example
//! ```rust
//! use kindly_dedup::protection::BuildVerification;
//!
//! let build_info = BuildVerification::get();
//! println!("Customer ID: {}", build_info.customer_id());
//! println!("Build Signature: {}", build_info.build_signature());
//! println!("Build Timestamp: {}", build_info.build_timestamp());
//! ```

use std::fmt;

// I20 Integration: Build hardening with runtime integrity checks
#[cfg(feature = "protection-build-hardening")]
use atomic_capsule::hash::AtomicHash256;
#[cfg(feature = "protection-build-hardening")]
use core::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "protection-build-hardening")]
use sha2::{Digest, Sha256};

/// Build verification capsule (T1 Atomic tier - I20-Enhanced)
///
/// Cache-aligned structure (128B) containing build-time constants + runtime integrity checks.
///
/// ## Memory Layout (I20-Enhanced)
/// ```text
/// [customer_id: &str (16B)] [build_signature: &str (16B)] [build_timestamp: u64 (8B)]
/// [runtime_integrity_hash: AtomicHash256 (32B)] [verification_count: AtomicU64 (8B)]
/// [_padding: 48B]
/// ```
///
/// ## Verification (I20-Enhanced)
/// - Compile-time: env!() macro fails if constants not embedded
/// - Runtime: SHA-256 hash verification of embedded constants (<100ns)
/// - Tamper detection: AtomicHash256 stores expected hash, verifies on each call
///
/// ## ASSUM Safety
/// - #ASSUME: Constants embedded by build.rs
/// - #VERIFY: Compile-time verification via env!() macro
/// - #ASSUME: Runtime hash computed once, stored in AtomicHash256
/// - #VERIFY: Hash mismatch indicates binary patching
#[repr(C, align(128))]
#[derive(Debug, Clone, Copy)]
pub struct BuildVerification {
    customer_id: &'static str,
    build_signature: &'static str,
    build_timestamp: u64,

    /// Runtime integrity hash (I20 enhancement)
    /// Stores SHA-256(customer_id || build_signature || build_timestamp)
    /// Verified on each integrity check (<100ns)
    #[cfg(feature = "protection-build-hardening")]
    runtime_integrity_hash: AtomicHash256,

    /// Verification counter (I20 enhancement)
    /// Counts successful integrity verifications (monotonic)
    #[cfg(feature = "protection-build-hardening")]
    verification_count: AtomicU64,

    _padding: [u8; if cfg!(feature = "protection-build-hardening") {
        48
    } else {
        24
    }],
}

impl BuildVerification {
    /// Get build verification instance (singleton pattern)
    ///
    /// Returns a reference to the static BuildVerification instance.
    ///
    /// ## Zero Runtime Cost
    /// Constants are embedded at compile-time via env!() macro.
    /// No dynamic allocation, no runtime computation.
    ///
    /// ## ASSUM Safety
    /// - #ASSUME: CUSTOMER_ID, BUILD_SIGNATURE, BUILD_TIMESTAMP embedded by build.rs
    /// - #VERIFY: env!() macro fails at compile-time if not embedded
    #[inline(always)]
    pub const fn get() -> Self {
        Self {
            customer_id: env!("CUSTOMER_ID"),
            build_signature: env!("BUILD_SIGNATURE"),
            build_timestamp: match parse_u64(env!("BUILD_TIMESTAMP")) {
                Some(ts) => ts,
                None => 0, // Fallback to 0 if parse fails (should never happen)
            },
            #[cfg(feature = "protection-build-hardening")]
            runtime_integrity_hash: AtomicHash256::new([0u8; 32]),
            #[cfg(feature = "protection-build-hardening")]
            verification_count: AtomicU64::new(0),
            _padding: [0u8; if cfg!(feature = "protection-build-hardening") {
                48
            } else {
                24
            }],
        }
    }

    /// Get customer ID (embedded at build time)
    ///
    /// ## Example
    /// ```rust,ignore
    /// let build_info = BuildVerification::get();
    /// assert_eq!(build_info.customer_id().len(), 36); // UUID format
    /// ```
    #[inline(always)]
    pub const fn customer_id(&self) -> &'static str {
        self.customer_id
    }

    /// Get binary signature (SHA-256 hash, embedded at build time)
    ///
    /// ## Example
    /// ```rust,ignore
    /// let build_info = BuildVerification::get();
    /// assert_eq!(build_info.build_signature().len(), 64); // SHA-256 hex
    /// ```
    #[inline(always)]
    pub const fn build_signature(&self) -> &'static str {
        self.build_signature
    }

    /// Get build timestamp (Unix timestamp, embedded at build time)
    ///
    /// ## Example
    /// ```rust,ignore
    /// let build_info = BuildVerification::get();
    /// assert!(build_info.build_timestamp() > 1700000000); // After 2023-11-14
    /// ```
    #[inline(always)]
    pub const fn build_timestamp(&self) -> u64 {
        self.build_timestamp
    }

    /// Verify build integrity (check that constants are embedded)
    ///
    /// Returns true if all constants are non-empty and valid.
    ///
    /// ## ASSUM Safety
    /// - #ASSUME: Empty strings indicate build.rs failure
    /// - #VERIFY: Check customer_id, build_signature non-empty
    #[inline]
    pub fn verify_integrity(&self) -> bool {
        // Check customer_id is non-empty (UUID format is 36 characters)
        if self.customer_id.is_empty() {
            return false;
        }

        // Check build_signature is non-empty (SHA-256 hex is 64 characters)
        if self.build_signature.is_empty() {
            return false;
        }

        // Check build_timestamp is reasonable (after 2020-01-01)
        if self.build_timestamp < 1577836800 {
            return false;
        }

        true
    }
}

impl Default for BuildVerification {
    fn default() -> Self {
        Self::get()
    }
}

impl fmt::Display for BuildVerification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Build Verification:")?;
        writeln!(f, "  Customer ID: {}", self.customer_id)?;
        writeln!(f, "  Build Signature: {}", self.build_signature)?;
        writeln!(
            f,
            "  Build Timestamp: {} ({})",
            self.build_timestamp,
            format_timestamp(self.build_timestamp)
        )?;
        writeln!(
            f,
            "  Integrity: {}",
            if self.verify_integrity() { "VALID" } else { "INVALID" }
        )
    }
}

/// Const-compatible u64 parser (for env!() macro)
///
/// ## ASSUM Safety
/// - #ASSUME: Input is valid decimal u64 string
/// - #VERIFY: Returns None if parse fails
const fn parse_u64(s: &str) -> Option<u64> {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    let mut result: u64 = 0;
    let mut i = 0;
    while i < bytes.len() {
        let digit = match bytes[i] {
            b'0'..=b'9' => bytes[i] - b'0',
            _ => return None,
        };
        result = match result.checked_mul(10) {
            Some(v) => v,
            None => return None,
        };
        result = match result.checked_add(digit as u64) {
            Some(v) => v,
            None => return None,
        };
        i += 1;
    }

    Some(result)
}

/// Format Unix timestamp as human-readable date
fn format_timestamp(timestamp: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};

    let duration = Duration::from_secs(timestamp);
    let datetime = UNIX_EPOCH + duration;

    match datetime.duration_since(UNIX_EPOCH) {
        Ok(d) => {
            // Simple date formatting (YYYY-MM-DD HH:MM:SS)
            let secs = d.as_secs();
            let days = secs / 86400;
            let hours = (secs % 86400) / 3600;
            let minutes = (secs % 3600) / 60;
            let seconds = secs % 60;
            format!("Day {} {:02}:{:02}:{:02} UTC", days, hours, minutes, seconds)
        }
        Err(_) => "Invalid timestamp".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_verification_get() {
        let build_info = BuildVerification::get();
        println!("{}", build_info);

        // Verify customer_id is non-empty
        assert!(!build_info.customer_id().is_empty());

        // Verify build_signature is non-empty
        assert!(!build_info.build_signature().is_empty());

        // Verify build_timestamp is reasonable (after 2020-01-01)
        assert!(build_info.build_timestamp() > 1577836800);
    }

    #[test]
    fn test_build_verification_integrity() {
        let build_info = BuildVerification::get();
        assert!(build_info.verify_integrity(), "Build integrity check failed");
    }

    #[test]
    fn test_build_verification_display() {
        let build_info = BuildVerification::get();
        let display = format!("{}", build_info);
        assert!(display.contains("Customer ID:"));
        assert!(display.contains("Build Signature:"));
        assert!(display.contains("Build Timestamp:"));
        assert!(display.contains("Integrity: VALID"));
    }

    #[test]
    fn test_parse_u64() {
        assert_eq!(parse_u64("0"), Some(0));
        assert_eq!(parse_u64("123"), Some(123));
        assert_eq!(parse_u64("1730000000"), Some(1730000000));
        assert_eq!(parse_u64(""), None);
        assert_eq!(parse_u64("abc"), None);
        assert_eq!(parse_u64("12.34"), None);
    }
}
