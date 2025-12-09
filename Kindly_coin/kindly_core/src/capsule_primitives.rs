//! Capsule Primitives - Shared types across all capsules
//!
//! Common primitives used by all atomic capsules in Kindly Coin.

use serde::{Deserialize, Serialize};

/// Capsule header (standard across all capsules)
///
/// Layout (64 bits):
/// - commit:1 | stale:1 | version:8 | payload:54
#[derive(Debug, Clone, Copy)]
pub struct CapsuleHeader {
    /// Commit flag (1 = committed, 0 = uncommitted)
    pub commit: bool,
    /// Stale flag (1 = stale data, 0 = fresh)
    pub stale: bool,
    /// Version number (odd = uncommitted, even = committed)
    pub version: u8,
    /// Payload (capsule-specific)
    pub payload: u64,
}

impl CapsuleHeader {
    /// Pack header into 64-bit value
    pub fn pack(&self) -> u64 {
        let commit_bit = if self.commit { 1u64 << 63 } else { 0 };
        let stale_bit = if self.stale { 1u64 << 62 } else { 0 };
        let version_bits = (self.version as u64) << 54;
        let payload_bits = self.payload & 0x3F_FFFF_FFFF_FFFF;

        commit_bit | stale_bit | version_bits | payload_bits
    }

    /// Unpack header from 64-bit value
    pub fn unpack(packed: u64) -> Self {
        Self {
            commit: (packed >> 63) & 1 == 1,
            stale: (packed >> 62) & 1 == 1,
            version: ((packed >> 54) & 0xFF) as u8,
            payload: packed & 0x3F_FFFF_FFFF_FFFF,
        }
    }

    /// Check if header is valid (committed, not stale, even version)
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.commit && !self.stale && self.version % 2 == 0
    }
}

/// Capsule status (shared status codes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CapsuleStatus {
    /// Uncommitted (being written)
    Uncommitted = 0,
    /// Committed and valid
    Valid = 1,
    /// Invalid or corrupted
    Invalid = 2,
    /// Stale (needs refresh)
    Stale = 3,
}

/// Protection level (circuit breaker integration)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum ProtectionLevel {
    /// L0: Normal operation
    Normal = 0,
    /// L1: Elevated - reduce size
    Level1 = 1,
    /// L2: Critical - emergency mode
    Level2 = 2,
    /// L3: Halt - pause all operations
    Level3 = 3,
}

impl ProtectionLevel {
    /// Get size multiplier for this protection level
    ///
    /// Uses phi-based scaling:
    /// - L0: 1.0× (normal)
    /// - L1: 1/φ ≈ 0.618× (reduce by golden ratio)
    /// - L2: 1/φ² ≈ 0.382× (reduce by phi squared)
    /// - L3: 0.0× (halt)
    pub fn size_multiplier(&self) -> f64 {
        const PHI: f64 = 1.6180339887498948;
        match self {
            ProtectionLevel::Normal => 1.0,
            ProtectionLevel::Level1 => 1.0 / PHI,
            ProtectionLevel::Level2 => 1.0 / (PHI * PHI),
            ProtectionLevel::Level3 => 0.0,
        }
    }

    /// Check if trading/operations are allowed
    pub fn allows_operations(&self) -> bool {
        *self != ProtectionLevel::Level3
    }
}

/// Calculate checksum for capsule data
///
/// Simple XOR-based checksum for fast validation
#[inline]
pub fn calculate_checksum(data: &[u64]) -> u16 {
    let mut checksum = 0u64;
    for &word in data {
        checksum ^= word;
    }
    (checksum & 0xFFFF) as u16
}

/// Verify checksum matches expected value
#[inline]
pub fn verify_checksum(data: &[u64], expected: u16) -> bool {
    calculate_checksum(data) == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_header_pack_unpack() {
        let header = CapsuleHeader {
            commit: true,
            stale: false,
            version: 42,
            payload: 0x1234_5678_9ABC,
        };

        let packed = header.pack();
        let unpacked = CapsuleHeader::unpack(packed);

        assert_eq!(header.commit, unpacked.commit);
        assert_eq!(header.stale, unpacked.stale);
        assert_eq!(header.version, unpacked.version);
        assert_eq!(header.payload, unpacked.payload);
    }

    #[test]
    fn test_capsule_header_is_valid() {
        let valid = CapsuleHeader {
            commit: true,
            stale: false,
            version: 2, // even
            payload: 0,
        };
        assert!(valid.is_valid());

        let uncommitted = CapsuleHeader {
            commit: false,
            stale: false,
            version: 2,
            payload: 0,
        };
        assert!(!uncommitted.is_valid());

        let stale = CapsuleHeader {
            commit: true,
            stale: true,
            version: 2,
            payload: 0,
        };
        assert!(!stale.is_valid());

        let odd_version = CapsuleHeader {
            commit: true,
            stale: false,
            version: 3, // odd
            payload: 0,
        };
        assert!(!odd_version.is_valid());
    }

    #[test]
    fn test_protection_level_size_multiplier() {
        let phi = 1.6180339887498948;
        assert!((ProtectionLevel::Normal.size_multiplier() - 1.0).abs() < 0.001);
        assert!((ProtectionLevel::Level1.size_multiplier() - (1.0 / phi)).abs() < 0.001);
        assert!((ProtectionLevel::Level2.size_multiplier() - (1.0 / (phi * phi))).abs() < 0.001);
        assert_eq!(ProtectionLevel::Level3.size_multiplier(), 0.0);
    }

    #[test]
    fn test_checksum() {
        let data = vec![0x1234_5678_9ABC_DEF0, 0xFEDC_BA98_7654_3210];
        let checksum = calculate_checksum(&data);
        assert!(verify_checksum(&data, checksum));
        assert!(!verify_checksum(&data, checksum.wrapping_add(1)));
    }
}
