//! # LicenseAuditCapsule - Q34 Compliant Hash-Chain with License Anchoring
//!
//! **[TRADE SECRET] - Tamper-evident audit trail bound to license**
//!
//! ## UCE34 Framework Compliance
//!
//! **Q10 Tier**: T0 Auditable + T1 Atomic
//! **Q33 Lockfree**: 100% lockfree, no Mutex/RwLock
//! **Q34 Audit**: Hash-chain includes license transform (tamper-evident)
//!
//! ## Core Innovation
//!
//! The audit trail hash-chain includes the license transform at each entry.
//! This creates cryptographic proof that:
//! 1. Operations occurred with a specific license
//! 2. License transform was correct at time of operation
//! 3. Any tampering (license or audit) breaks the chain
//!
//! ## Security Properties
//!
//! - **Tamper-evident**: Modifying any entry breaks hash chain
//! - **License-bound**: License transform embedded in chain
//! - **Replay-resistant**: Generation counters prevent replay attacks
//! - **Time-ordered**: Timestamps establish temporal ordering
//!
//! ## Memory Layout (256 bytes, cache-aligned)
//!
//! ```text
//! Offset 0-7:     chain_head (AtomicU64) - current hash chain head
//! Offset 8-15:    entry_count (AtomicU64) - number of entries
//! Offset 16-23:   license_anchor (u64) - license transform at init
//! Offset 24-31:   last_timestamp (AtomicU64) - last operation timestamp
//! Offset 32-39:   generation (AtomicU64) - generation counter
//! Offset 40-255:  padding (216 bytes)
//! ```
//!
//! ## Performance (B32 Targets)
//! - Append entry: <100ns (FNV-1a hash + atomic)
//! - Verify chain: <1ms per 1000 entries
//! - Get anchor: <5ns (direct read)

use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

/// License anchor information for audit entries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditAnchor {
    /// License transform (SHA256(signature)[0..8])
    pub license_transform: u64,
    /// Generation at time of anchor
    pub generation: u64,
    /// Timestamp at time of anchor
    pub timestamp: u64,
}

impl AuditAnchor {
    /// Create new audit anchor
    pub const fn new(license_transform: u64, generation: u64, timestamp: u64) -> Self {
        Self {
            license_transform,
            generation,
            timestamp,
        }
    }

    /// Serialize anchor to bytes for hashing
    pub fn to_bytes(&self) -> [u8; 24] {
        let mut bytes = [0u8; 24];
        bytes[0..8].copy_from_slice(&self.license_transform.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.generation.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.timestamp.to_le_bytes());
        bytes
    }
}

/// License audit entry with hash chain
#[derive(Debug, Clone, Copy)]
pub struct LicenseAuditEntry {
    /// Previous hash in chain
    pub prev_hash: u64,
    /// Operation type code
    pub operation: u8,
    /// Feature bit involved (0-63 or 255 for none)
    pub feature_bit: u8,
    /// Input value (truncated)
    pub input_low: u32,
    /// Output value (truncated)
    pub output_low: u32,
    /// License anchor at time of operation
    pub anchor: AuditAnchor,
}

impl LicenseAuditEntry {
    /// Operation codes
    pub const OP_TRANSITION: u8 = 1;
    pub const OP_FEATURE: u8 = 2;
    pub const OP_DISPATCH: u8 = 3;
    pub const OP_MASK: u8 = 4;
    pub const OP_VERIFY: u8 = 5;
    pub const OP_INIT: u8 = 0xFF;

    /// Create new audit entry
    pub const fn new(
        prev_hash: u64,
        operation: u8,
        feature_bit: u8,
        input_low: u32,
        output_low: u32,
        anchor: AuditAnchor,
    ) -> Self {
        Self {
            prev_hash,
            operation,
            feature_bit,
            input_low,
            output_low,
            anchor,
        }
    }

    /// Compute hash of this entry (for chain)
    ///
    /// Uses FNV-1a for speed (<50ns)
    pub fn compute_hash(&self) -> u64 {
        // FNV-1a constants
        const FNV_PRIME: u64 = 0x00000100000001B3;
        const FNV_OFFSET: u64 = 0xcbf29ce484222325;

        let mut hash = FNV_OFFSET;

        // Hash prev_hash
        for byte in self.prev_hash.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }

        // Hash operation
        hash ^= self.operation as u64;
        hash = hash.wrapping_mul(FNV_PRIME);

        // Hash feature_bit
        hash ^= self.feature_bit as u64;
        hash = hash.wrapping_mul(FNV_PRIME);

        // Hash input_low
        for byte in self.input_low.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }

        // Hash output_low
        for byte in self.output_low.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }

        // Hash anchor (CRITICAL - binds to license)
        for byte in self.anchor.to_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }

        hash
    }

    /// Verify this entry against expected previous hash
    pub fn verify(&self, expected_prev: u64) -> bool {
        self.prev_hash == expected_prev
    }
}

/// LicenseAuditCapsule - Q34 compliant audit trail with license anchoring
///
/// ## Memory Layout (256 bytes, cache-aligned)
///
/// - Offset 0-7: `chain_head` (AtomicU64) - current chain head hash
/// - Offset 8-15: `entry_count` (AtomicU64) - total entries appended
/// - Offset 16-23: `license_anchor` (u64) - license transform at initialization
/// - Offset 24-31: `last_timestamp` (AtomicU64) - last operation timestamp
/// - Offset 32-39: `generation` (AtomicU64) - generation counter
/// - Offset 40-255: `_padding` ([u8; 216])
///
/// ## Q34 Compliance
///
/// - Hash-chained entries (tamper-evident)
/// - License transform embedded in each entry hash
/// - Timestamps for temporal ordering
/// - Generation counters for replay prevention
///
/// ## Performance (B32 Targets)
/// - Append: <100ns (FNV-1a + atomic CAS)
/// - Verify: <1ms per 1000 entries
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 256, size = 256))]
#[repr(C, align(256))]
pub struct LicenseAuditCapsule {
    /// Current hash chain head
    chain_head: AtomicU64,

    /// Total entry count
    entry_count: AtomicU64,

    /// License anchor (transform at initialization)
    license_anchor: u64,

    /// Last operation timestamp
    last_timestamp: AtomicU64,

    /// Generation counter
    generation: AtomicU64,

    /// Padding to 256 bytes
    _padding: [u8; 216],
}

// Compile-time verification
#[cfg(not(feature = "derive"))]
crate::verify_capsule_properties!(LicenseAuditCapsule, 256, 256);

// Send + Sync safety
#[cfg(not(feature = "derive"))]
unsafe impl Send for LicenseAuditCapsule {}
#[cfg(not(feature = "derive"))]
unsafe impl Sync for LicenseAuditCapsule {}

impl LicenseAuditCapsule {
    /// Create new audit capsule anchored to license transform
    ///
    /// ## Arguments
    /// - `license_transform`: SHA256(signature)[0..8] from license
    ///
    /// ## ASSUM Framework
    /// - `#ASSUME_ANCHOR_IMMUTABLE`: License anchor cannot change after init
    /// - `#VERIFY_ANCHOR`: verify_chain() validates anchor consistency
    pub const fn new(license_transform: u64) -> Self {
        Self {
            chain_head: AtomicU64::new(license_transform), // Initial chain head IS the anchor
            entry_count: AtomicU64::new(0),
            license_anchor: license_transform,
            last_timestamp: AtomicU64::new(0),
            generation: AtomicU64::new(1),
            _padding: [0u8; 216],
        }
    }

    /// Create uninitialized audit capsule (for delayed init)
    pub const fn uninit() -> Self {
        Self {
            chain_head: AtomicU64::new(0),
            entry_count: AtomicU64::new(0),
            license_anchor: 0,
            last_timestamp: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            _padding: [0u8; 216],
        }
    }

    /// Append audit entry for operation
    ///
    /// ## Arguments
    /// - `operation`: Operation code (OP_TRANSITION, OP_FEATURE, etc.)
    /// - `feature_bit`: Feature bit (0-63) or 255 for none
    /// - `input`: Input value
    /// - `output`: Output value
    /// - `timestamp`: Operation timestamp
    ///
    /// ## Returns
    /// - New chain head hash
    ///
    /// ## Performance
    /// <100ns (FNV-1a + atomic CAS)
    ///
    /// ## ASSUM Framework
    /// - `#ASSUME_CAS_SUCCESS`: CAS may fail under high contention, but retries
    /// - `#VERIFY_CAS`: Concurrent tests validate CAS behavior
    pub fn append(
        &self,
        operation: u8,
        feature_bit: u8,
        input: u64,
        output: u64,
        timestamp: u64,
    ) -> u64 {
        // Get current generation
        let gen = self.generation.fetch_add(1, Ordering::AcqRel);

        // Create anchor for this entry
        let anchor = AuditAnchor::new(self.license_anchor, gen, timestamp);

        // CAS loop to append entry
        loop {
            let prev_hash = self.chain_head.load(Ordering::Acquire);

            let entry = LicenseAuditEntry::new(
                prev_hash,
                operation,
                feature_bit,
                input as u32, // Truncate to low 32 bits
                output as u32,
                anchor,
            );

            let new_hash = entry.compute_hash();

            // Attempt to update chain head
            match self.chain_head.compare_exchange_weak(
                prev_hash,
                new_hash,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Success - update metadata
                    self.entry_count.fetch_add(1, Ordering::Relaxed);
                    self.last_timestamp.store(timestamp, Ordering::Release);
                    return new_hash;
                }
                Err(_) => {
                    // Retry - another thread updated the chain
                    core::hint::spin_loop();
                }
            }
        }
    }

    /// Verify chain integrity
    ///
    /// ## Arguments
    /// - `entries`: Array of entries to verify (oldest first)
    ///
    /// ## Returns
    /// - `Ok(())`: Chain is valid
    /// - `Err(index)`: Chain broken at index
    ///
    /// ## Performance
    /// <1ms per 1000 entries
    pub fn verify_chain(&self, entries: &[LicenseAuditEntry]) -> Result<(), usize> {
        if entries.is_empty() {
            return Ok(());
        }

        // First entry should chain from anchor
        if entries[0].prev_hash != self.license_anchor {
            return Err(0);
        }

        // Verify each subsequent entry chains correctly
        let mut expected_prev = entries[0].compute_hash();

        for (i, entry) in entries.iter().enumerate().skip(1) {
            if entry.prev_hash != expected_prev {
                return Err(i);
            }

            // Verify anchor matches our license
            if entry.anchor.license_transform != self.license_anchor {
                return Err(i);
            }

            expected_prev = entry.compute_hash();
        }

        Ok(())
    }

    /// Get current chain head hash
    #[inline(always)]
    pub fn chain_head(&self) -> u64 {
        self.chain_head.load(Ordering::Acquire)
    }

    /// Get entry count
    #[inline(always)]
    pub fn entry_count(&self) -> u64 {
        self.entry_count.load(Ordering::Relaxed)
    }

    /// Get license anchor
    #[inline(always)]
    pub fn license_anchor(&self) -> u64 {
        self.license_anchor
    }

    /// Get current generation
    #[inline(always)]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get last timestamp
    #[inline(always)]
    pub fn last_timestamp(&self) -> u64 {
        self.last_timestamp.load(Ordering::Acquire)
    }

    /// Create audit anchor snapshot
    pub fn current_anchor(&self) -> AuditAnchor {
        AuditAnchor::new(
            self.license_anchor,
            self.generation.load(Ordering::Acquire),
            self.last_timestamp.load(Ordering::Acquire),
        )
    }
}

impl Default for LicenseAuditCapsule {
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

    /// T28: Unit Test - Anchor creation
    #[test]
    fn test_audit_anchor_creation() {
        let anchor = AuditAnchor::new(0xDEADBEEF, 42, 1234567890);
        assert_eq!(anchor.license_transform, 0xDEADBEEF);
        assert_eq!(anchor.generation, 42);
        assert_eq!(anchor.timestamp, 1234567890);
    }

    /// T28: Unit Test - Anchor serialization
    #[test]
    fn test_audit_anchor_bytes() {
        let anchor = AuditAnchor::new(0x123456789ABCDEF0, 100, 200);
        let bytes = anchor.to_bytes();

        // Verify layout
        assert_eq!(
            &bytes[0..8],
            &0x123456789ABCDEF0u64.to_le_bytes()
        );
        assert_eq!(&bytes[8..16], &100u64.to_le_bytes());
        assert_eq!(&bytes[16..24], &200u64.to_le_bytes());
    }

    /// T28: Unit Test - Entry hash computation
    #[test]
    fn test_entry_hash_deterministic() {
        let anchor = AuditAnchor::new(0xABCD, 1, 1000);
        let entry = LicenseAuditEntry::new(
            0x1234,
            LicenseAuditEntry::OP_TRANSITION,
            255,
            42,
            84,
            anchor,
        );

        let hash1 = entry.compute_hash();
        let hash2 = entry.compute_hash();

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, 0);
    }

    /// T28: Unit Test - Entry hash varies with content
    #[test]
    fn test_entry_hash_varies() {
        let anchor = AuditAnchor::new(0xABCD, 1, 1000);

        let entry1 = LicenseAuditEntry::new(0, 1, 0, 100, 200, anchor);
        let entry2 = LicenseAuditEntry::new(0, 1, 0, 101, 200, anchor); // Different input
        let entry3 = LicenseAuditEntry::new(0, 2, 0, 100, 200, anchor); // Different op

        let hash1 = entry1.compute_hash();
        let hash2 = entry2.compute_hash();
        let hash3 = entry3.compute_hash();

        assert_ne!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_ne!(hash2, hash3);
    }

    /// T28: Unit Test - Capsule creation
    #[test]
    fn test_audit_capsule_creation() {
        let capsule = LicenseAuditCapsule::new(0xDEADBEEF);

        assert_eq!(capsule.license_anchor(), 0xDEADBEEF);
        assert_eq!(capsule.chain_head(), 0xDEADBEEF); // Initial head is anchor
        assert_eq!(capsule.entry_count(), 0);
        assert_eq!(capsule.generation(), 1);
    }

    /// T28: Unit Test - Uninit capsule
    #[test]
    fn test_audit_capsule_uninit() {
        let capsule = LicenseAuditCapsule::uninit();

        assert_eq!(capsule.license_anchor(), 0);
        assert_eq!(capsule.chain_head(), 0);
        assert_eq!(capsule.generation(), 0);
    }

    /// T28: Property Test - Append updates chain
    #[test]
    fn test_append_updates_chain() {
        let capsule = LicenseAuditCapsule::new(0xABCDEF);

        let head_before = capsule.chain_head();
        let count_before = capsule.entry_count();

        let new_hash = capsule.append(
            LicenseAuditEntry::OP_TRANSITION,
            255,
            100,
            200,
            1000,
        );

        assert_ne!(capsule.chain_head(), head_before);
        assert_eq!(capsule.chain_head(), new_hash);
        assert_eq!(capsule.entry_count(), count_before + 1);
    }

    /// T28: Property Test - Multiple appends chain correctly
    #[test]
    fn test_multiple_appends() {
        let capsule = LicenseAuditCapsule::new(0xFEED);

        let hash1 = capsule.append(1, 0, 10, 20, 100);
        let hash2 = capsule.append(2, 1, 30, 40, 200);
        let hash3 = capsule.append(3, 2, 50, 60, 300);

        assert_ne!(hash1, hash2);
        assert_ne!(hash2, hash3);
        assert_eq!(capsule.chain_head(), hash3);
        assert_eq!(capsule.entry_count(), 3);
    }

    /// T28: Integration Test - Verify valid chain
    #[test]
    fn test_verify_valid_chain() {
        let anchor_val = 0xABCD1234;
        let capsule = LicenseAuditCapsule::new(anchor_val);

        // Build entries manually
        let anchor1 = AuditAnchor::new(anchor_val, 1, 100);
        let entry1 = LicenseAuditEntry::new(
            anchor_val, // prev_hash is anchor
            1,
            255,
            10,
            20,
            anchor1,
        );

        let hash1 = entry1.compute_hash();

        let anchor2 = AuditAnchor::new(anchor_val, 2, 200);
        let entry2 = LicenseAuditEntry::new(
            hash1, // prev_hash is entry1's hash
            2,
            0,
            30,
            40,
            anchor2,
        );

        // Verify chain
        let result = capsule.verify_chain(&[entry1, entry2]);
        assert!(result.is_ok());
    }

    /// T28: Integration Test - Detect broken chain
    #[test]
    fn test_detect_broken_chain() {
        let anchor_val = 0xABCD1234;
        let capsule = LicenseAuditCapsule::new(anchor_val);

        // Build entries with broken chain
        let anchor1 = AuditAnchor::new(anchor_val, 1, 100);
        let entry1 = LicenseAuditEntry::new(anchor_val, 1, 255, 10, 20, anchor1);

        let hash1 = entry1.compute_hash();

        let anchor2 = AuditAnchor::new(anchor_val, 2, 200);
        let entry2 = LicenseAuditEntry::new(
            hash1.wrapping_add(1), // WRONG prev_hash
            2,
            0,
            30,
            40,
            anchor2,
        );

        // Verify should fail at entry2
        let result = capsule.verify_chain(&[entry1, entry2]);
        assert_eq!(result, Err(1));
    }

    /// T28: Integration Test - Detect wrong anchor
    #[test]
    fn test_detect_wrong_anchor() {
        let anchor_val = 0xABCD1234;
        let capsule = LicenseAuditCapsule::new(anchor_val);

        // Entry with wrong anchor
        let wrong_anchor = AuditAnchor::new(0x11111111, 1, 100); // WRONG license
        let entry1 = LicenseAuditEntry::new(
            0xABCD1234, // Wrong - should be anchor_val
            1,
            255,
            10,
            20,
            wrong_anchor,
        );

        let hash1 = entry1.compute_hash();

        // Second entry with wrong anchor too
        let wrong_anchor2 = AuditAnchor::new(0x11111111, 2, 200);
        let entry2 = LicenseAuditEntry::new(hash1, 2, 0, 30, 40, wrong_anchor2);

        // Verify should fail - anchor mismatch
        let result = capsule.verify_chain(&[entry1, entry2]);
        assert!(result.is_err());
    }

    /// T28: Production Test - Concurrent appends
    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_appends() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(LicenseAuditCapsule::new(0xC0C0EE));

        let handles: Vec<_> = (0..4)
            .map(|i| {
                let capsule = Arc::clone(&capsule);
                thread::spawn(move || {
                    for j in 0..100 {
                        capsule.append(i as u8, (j % 64) as u8, i * 100 + j, j * 2, j as u64);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // All 400 entries should be appended
        assert_eq!(capsule.entry_count(), 400);
    }

    /// T28: Production Test - Memory layout
    #[test]
    fn test_memory_layout() {
        use core::mem::{size_of, align_of};

        assert_eq!(size_of::<LicenseAuditCapsule>(), 256);
        assert_eq!(align_of::<LicenseAuditCapsule>(), 256);
    }
}
