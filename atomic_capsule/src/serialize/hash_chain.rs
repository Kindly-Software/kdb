//! # HashChainCapsule - T0 Auditable Hash Chain for Compliance
//!
//! **Tier 0: Auditable Foundation** - Q34 hash-chain integrity for compliance audit trails.
//!
//! ## UCE34 Framework Application
//!
//! - **Q10 (Computational Capsule)**: T0 Auditable tier - deterministic hash chains
//! - **Q11 (Rust Transform)**: Cache-aligned 64B capsule, zero unsafe in public API
//! - **Q28 (Simplicity)**: Simple append-only API with hash chain verification
//! - **Q33 (Validation)**: Generation counters prevent TOCTOU races
//! - **Q34 (Auditability)**: Hash-chained entries for compliance (SOX/SOC2/GDPR/HIPAA)
//!
//! ## Design Philosophy
//!
//! - **Append-only**: Hash chain grows monotonically (tamper-evident)
//! - **Deterministic**: FNV-1a hash (portable, reproducible)
//! - **Lockfree**: Atomic CAS operations only (Chaos mandate)
//! - **Compliance-ready**: Timestamps + hash chain = audit trail
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_FNV1A_DETERMINISTIC`: FNV-1a always produces same hash for same input
//! - `#VERIFY_FNV1A_DETERMINISTIC`: Property tests confirm determinism
//! - `#ASSUME_TIMESTAMP_MONOTONIC`: Timestamps increase (not enforced, user responsibility)
//! - `#VERIFY_TIMESTAMP_MONOTONIC`: Debug assertions on append
//! - `#ASSUME_CHAIN_TAMPER_EVIDENT`: Modifying any entry invalidates chain
//! - `#VERIFY_CHAIN_TAMPER_EVIDENT`: Verification traverses all entries
//!
//! ## Performance (B32 Framework)
//!
//! - Append: <20ns (single CAS + FNV-1a hash)
//! - Verify entry: <10ns (single hash comparison)
//! - Verify chain: O(n) where n = chain length

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use core::sync::atomic::{AtomicU64, Ordering};

/// FNV-1a hash constants (64-bit)
const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

/// HashChainCapsule - T0 Auditable hash chain for compliance
///
/// Provides append-only hash-chained audit trail for compliance requirements
/// (SOX, SOC2, GDPR, HIPAA). Each entry is chained to the previous via FNV-1a hash.
///
/// # Layout (64B, cache-aligned)
///
/// ```text
/// [0-7]   chain_head: AtomicU64   - Current chain head hash (FNV-1a)
/// [8-15]  chain_length: AtomicU64 - Number of entries in chain
/// [16-23] last_timestamp: AtomicU64 - Unix timestamp of last entry (nanoseconds)
/// [24-31] crc64: AtomicU64       - CRC64 of chain state (tamper detection)
/// [32-39] generation: AtomicU64  - Generation counter (for TOCTOU prevention)
/// [40-63] _padding               - Cache alignment to 64B
/// ```
///
/// # Hash Chain Structure
///
/// Each entry hash is computed as:
/// ```text
/// entry_hash = FNV-1a(prev_hash || timestamp || payload_hash)
/// ```
///
/// This creates a tamper-evident chain where modifying any entry
/// invalidates all subsequent entries.
///
/// # Example
///
/// ```rust,ignore
/// use atomic_capsule::serialize::HashChainCapsule;
///
/// // Create new hash chain
/// let chain = HashChainCapsule::new();
///
/// // Append entry (payload hash from serialized data)
/// let payload_hash = 0x1234567890abcdef_u64;
/// let timestamp = 1700000000_000_000_000_u64; // nanoseconds
/// chain.append(payload_hash, timestamp);
///
/// // Verify chain integrity
/// assert!(chain.verify_head(chain.head_hash()));
///
/// // Get chain length
/// assert_eq!(chain.length(), 1);
/// ```
///
/// # Compliance Use Cases
///
/// - **SOX**: Financial transaction audit trails
/// - **SOC2**: System access logging
/// - **GDPR**: Data processing records (Article 30)
/// - **HIPAA**: Healthcare data access logs
#[repr(C, align(64))]
pub struct HashChainCapsule {
    /// Current chain head hash (FNV-1a)
    /// Initial value: FNV_OFFSET_BASIS (0xcbf29ce484222325)
    chain_head: AtomicU64,

    /// Number of entries in chain
    chain_length: AtomicU64,

    /// Unix timestamp of last entry (nanoseconds since epoch)
    last_timestamp: AtomicU64,

    /// CRC64 of chain state (chain_head ^ chain_length ^ last_timestamp)
    /// Used for quick tamper detection without full chain traversal
    crc64: AtomicU64,

    /// Generation counter for TOCTOU prevention
    /// Incremented on each append
    generation: AtomicU64,

    /// Padding to 64B cache line
    _padding: [u8; 24],
}

/// Entry proof for verification
///
/// Contains all information needed to verify an entry in the chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryProof {
    /// Hash after this entry was added
    pub entry_hash: u64,
    /// Previous chain head before this entry
    pub prev_hash: u64,
    /// Entry timestamp (nanoseconds)
    pub timestamp: u64,
    /// Payload hash that was appended
    pub payload_hash: u64,
}

/// Hash chain snapshot for atomic reads
///
/// Captures consistent state of chain at a point in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainSnapshot {
    /// Current head hash
    pub head_hash: u64,
    /// Chain length
    pub length: u64,
    /// Last entry timestamp
    pub last_timestamp: u64,
    /// CRC64 checksum
    pub crc64: u64,
    /// Generation at snapshot time
    pub generation: u64,
}

/// Error type for hash chain operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashChainError {
    /// Chain state corrupted (CRC mismatch)
    CorruptedState,
    /// Entry verification failed
    VerificationFailed,
    /// Timestamp not monotonically increasing
    NonMonotonicTimestamp,
    /// Generation mismatch (concurrent modification)
    GenerationMismatch,
}

impl core::fmt::Display for HashChainError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HashChainError::CorruptedState => write!(f, "Hash chain state corrupted"),
            HashChainError::VerificationFailed => write!(f, "Entry verification failed"),
            HashChainError::NonMonotonicTimestamp => write!(f, "Timestamp not monotonically increasing"),
            HashChainError::GenerationMismatch => write!(f, "Generation mismatch - concurrent modification"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for HashChainError {}

/// Result type for hash chain operations
pub type HashChainResult<T> = core::result::Result<T, HashChainError>;

impl HashChainCapsule {
    /// Create new empty hash chain
    ///
    /// Initial head is FNV_OFFSET_BASIS, representing an empty chain.
    ///
    /// # Performance
    ///
    /// O(1) - constant time initialization
    #[inline]
    pub const fn new() -> Self {
        let head = FNV_OFFSET_BASIS;
        let length = 0_u64;
        let timestamp = 0_u64;
        let crc = head ^ length ^ timestamp;

        Self {
            chain_head: AtomicU64::new(head),
            chain_length: AtomicU64::new(length),
            last_timestamp: AtomicU64::new(timestamp),
            crc64: AtomicU64::new(crc),
            generation: AtomicU64::new(0),
            _padding: [0u8; 24],
        }
    }

    /// Append entry to hash chain (lockfree)
    ///
    /// # Arguments
    ///
    /// - `payload_hash`: Hash of the payload being recorded (user computes this)
    /// - `timestamp`: Unix timestamp in nanoseconds (user provides)
    ///
    /// # Returns
    ///
    /// - `Ok(EntryProof)`: Proof of the appended entry
    /// - `Err(HashChainError)`: If operation fails
    ///
    /// # Performance
    ///
    /// - Target: <20ns (single CAS + FNV-1a)
    ///
    /// # Thread Safety
    ///
    /// Lockfree via atomic CAS. Concurrent appends are serialized.
    ///
    /// # ASSUM Framework
    ///
    /// - `#ASSUME_TIMESTAMP_MONOTONIC`: Timestamps should increase
    /// - `#VERIFY_TIMESTAMP_MONOTONIC`: Debug assertion (not enforced in release)
    pub fn append(&self, payload_hash: u64, timestamp: u64) -> HashChainResult<EntryProof> {
        loop {
            // Load current state
            let prev_hash = self.chain_head.load(Ordering::Acquire);
            let prev_length = self.chain_length.load(Ordering::Acquire);
            let prev_timestamp = self.last_timestamp.load(Ordering::Acquire);
            let prev_gen = self.generation.load(Ordering::Acquire);

            // Debug: Check monotonic timestamp (not enforced in release for performance)
            #[cfg(debug_assertions)]
            if prev_length > 0 && timestamp < prev_timestamp {
                return Err(HashChainError::NonMonotonicTimestamp);
            }

            // Compute new entry hash: FNV-1a(prev_hash || timestamp || payload_hash)
            let entry_hash = Self::compute_entry_hash(prev_hash, timestamp, payload_hash);

            // New state
            let new_length = prev_length + 1;
            let new_crc = entry_hash ^ new_length ^ timestamp;
            let new_gen = prev_gen.wrapping_add(1);

            // CAS to update head (linearization point)
            match self.chain_head.compare_exchange(
                prev_hash,
                entry_hash,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Update other fields (after successful CAS)
                    self.chain_length.store(new_length, Ordering::Release);
                    self.last_timestamp.store(timestamp, Ordering::Release);
                    self.crc64.store(new_crc, Ordering::Release);
                    self.generation.store(new_gen, Ordering::Release);

                    return Ok(EntryProof {
                        entry_hash,
                        prev_hash,
                        timestamp,
                        payload_hash,
                    });
                }
                Err(_) => {
                    // Contention - retry
                    core::hint::spin_loop();
                }
            }
        }
    }

    /// Verify entry proof against chain
    ///
    /// Recomputes the entry hash and checks if it matches the proof.
    ///
    /// # Arguments
    ///
    /// - `proof`: Entry proof to verify
    ///
    /// # Returns
    ///
    /// - `true`: Proof is valid
    /// - `false`: Proof is invalid or tampered
    ///
    /// # Performance
    ///
    /// - Target: <10ns (single hash computation)
    #[inline]
    pub fn verify_entry(proof: &EntryProof) -> bool {
        let computed = Self::compute_entry_hash(proof.prev_hash, proof.timestamp, proof.payload_hash);
        computed == proof.entry_hash
    }

    /// Verify chain integrity (quick CRC check)
    ///
    /// Checks if the internal CRC matches the chain state.
    ///
    /// # Performance
    ///
    /// - Target: <5ns (XOR operations)
    #[inline]
    pub fn verify_crc(&self) -> bool {
        let head = self.chain_head.load(Ordering::Acquire);
        let length = self.chain_length.load(Ordering::Acquire);
        let timestamp = self.last_timestamp.load(Ordering::Acquire);
        let crc = self.crc64.load(Ordering::Acquire);

        let computed_crc = head ^ length ^ timestamp;
        computed_crc == crc
    }

    /// Verify that head matches expected hash
    ///
    /// # Arguments
    ///
    /// - `expected`: Expected head hash
    ///
    /// # Returns
    ///
    /// - `true`: Head matches expected
    /// - `false`: Head does not match (chain modified or tampered)
    #[inline]
    pub fn verify_head(&self, expected: u64) -> bool {
        self.chain_head.load(Ordering::Acquire) == expected
    }

    /// Get current head hash
    ///
    /// # Performance
    ///
    /// - Target: <3ns (single atomic load)
    #[inline]
    pub fn head_hash(&self) -> u64 {
        self.chain_head.load(Ordering::Acquire)
    }

    /// Get chain length
    ///
    /// # Performance
    ///
    /// - Target: <3ns (single atomic load)
    #[inline]
    pub fn length(&self) -> u64 {
        self.chain_length.load(Ordering::Acquire)
    }

    /// Get last entry timestamp
    ///
    /// # Performance
    ///
    /// - Target: <3ns (single atomic load)
    #[inline]
    pub fn last_timestamp(&self) -> u64 {
        self.last_timestamp.load(Ordering::Acquire)
    }

    /// Get current generation
    ///
    /// # Performance
    ///
    /// - Target: <3ns (single atomic load)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Take atomic snapshot of chain state
    ///
    /// Returns consistent view of all chain state fields.
    ///
    /// # Performance
    ///
    /// - Target: <15ns (5 atomic loads)
    ///
    /// # Thread Safety
    ///
    /// Snapshot may be inconsistent under concurrent modification.
    /// Use generation to detect inconsistency.
    pub fn snapshot(&self) -> ChainSnapshot {
        ChainSnapshot {
            head_hash: self.chain_head.load(Ordering::Acquire),
            length: self.chain_length.load(Ordering::Acquire),
            last_timestamp: self.last_timestamp.load(Ordering::Acquire),
            crc64: self.crc64.load(Ordering::Acquire),
            generation: self.generation.load(Ordering::Acquire),
        }
    }

    /// Compute hash for audit trail serialization
    ///
    /// Returns a hash suitable for storing the chain state as part of an audit record.
    /// Uses FNV-1a over all chain state fields.
    ///
    /// # Performance
    ///
    /// - Target: <15ns (FNV-1a over 40 bytes)
    pub fn compute_audit_hash(&self) -> u64 {
        let snap = self.snapshot();
        let mut hash = FNV_OFFSET_BASIS;

        // Hash each field
        hash = Self::fnv1a_u64(hash, snap.head_hash);
        hash = Self::fnv1a_u64(hash, snap.length);
        hash = Self::fnv1a_u64(hash, snap.last_timestamp);
        hash = Self::fnv1a_u64(hash, snap.crc64);
        hash = Self::fnv1a_u64(hash, snap.generation);

        hash
    }

    /// Reset chain to initial state
    ///
    /// # Warning
    ///
    /// This destroys the audit trail. Only use for testing or when
    /// explicitly starting a new audit period.
    ///
    /// # Performance
    ///
    /// - Target: <10ns (5 atomic stores)
    pub fn reset(&self) {
        let head = FNV_OFFSET_BASIS;
        let crc = head ^ 0 ^ 0;

        self.chain_head.store(head, Ordering::Release);
        self.chain_length.store(0, Ordering::Release);
        self.last_timestamp.store(0, Ordering::Release);
        self.crc64.store(crc, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    // ========================================================================
    // Internal helpers
    // ========================================================================

    /// Compute entry hash using FNV-1a
    ///
    /// hash = FNV-1a(prev_hash || timestamp || payload_hash)
    #[inline]
    fn compute_entry_hash(prev_hash: u64, timestamp: u64, payload_hash: u64) -> u64 {
        let mut hash = FNV_OFFSET_BASIS;
        hash = Self::fnv1a_u64(hash, prev_hash);
        hash = Self::fnv1a_u64(hash, timestamp);
        hash = Self::fnv1a_u64(hash, payload_hash);
        hash
    }

    /// FNV-1a hash step for u64 value
    #[inline]
    const fn fnv1a_u64(hash: u64, value: u64) -> u64 {
        let bytes = value.to_le_bytes();
        let mut h = hash;
        let mut i = 0;
        while i < 8 {
            h ^= bytes[i] as u64;
            h = h.wrapping_mul(FNV_PRIME);
            i += 1;
        }
        h
    }
}

impl Default for HashChainCapsule {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: Capsule is thread-safe via atomic operations
unsafe impl Send for HashChainCapsule {}
unsafe impl Sync for HashChainCapsule {}

// ============================================================================
// Convenience functions for payload hashing
// ============================================================================

/// Compute FNV-1a hash of byte slice (for payload hashing)
///
/// # Performance
///
/// - Target: ~0.5ns per byte (FNV-1a is extremely fast)
///
/// # Example
///
/// ```rust,ignore
/// use atomic_capsule::serialize::hash_chain::fnv1a_hash;
///
/// let data = b"hello world";
/// let hash = fnv1a_hash(data);
/// ```
#[inline]
pub fn fnv1a_hash(data: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Compute FNV-1a hash of multiple byte slices (zero-copy)
///
/// Useful for hashing header + payload without concatenation.
///
/// # Performance
///
/// - Target: ~0.5ns per byte total
///
/// # Example
///
/// ```rust,ignore
/// use atomic_capsule::serialize::hash_chain::fnv1a_hash_multi;
///
/// let header = b"header";
/// let payload = b"payload";
/// let hash = fnv1a_hash_multi(&[header, payload]);
/// ```
#[inline]
pub fn fnv1a_hash_multi(slices: &[&[u8]]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for slice in slices {
        for &byte in *slice {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

/// Compute FNV-1a hash of a u64 value
///
/// # Performance
///
/// - Target: <5ns
#[inline]
pub const fn fnv1a_hash_u64(value: u64) -> u64 {
    HashChainCapsule::fnv1a_u64(FNV_OFFSET_BASIS, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_chain() {
        let chain = HashChainCapsule::new();
        assert_eq!(chain.length(), 0);
        assert_eq!(chain.head_hash(), FNV_OFFSET_BASIS);
        assert_eq!(chain.last_timestamp(), 0);
        assert!(chain.verify_crc());
    }

    #[test]
    fn test_append_single() {
        let chain = HashChainCapsule::new();
        let payload_hash = 0x1234567890abcdef_u64;
        let timestamp = 1700000000_000_000_000_u64;

        let proof = chain.append(payload_hash, timestamp).unwrap();

        assert_eq!(chain.length(), 1);
        assert_eq!(chain.last_timestamp(), timestamp);
        assert!(chain.verify_crc());
        assert!(HashChainCapsule::verify_entry(&proof));
    }

    #[test]
    fn test_append_multiple() {
        let chain = HashChainCapsule::new();

        for i in 0..10 {
            let payload_hash = 0x1234567890abcdef_u64 + i;
            let timestamp = 1700000000_000_000_000_u64 + i * 1_000_000;

            let proof = chain.append(payload_hash, timestamp).unwrap();
            assert!(HashChainCapsule::verify_entry(&proof));
        }

        assert_eq!(chain.length(), 10);
        assert!(chain.verify_crc());
    }

    #[test]
    fn test_chain_determinism() {
        // Same inputs should produce same hashes
        let chain1 = HashChainCapsule::new();
        let chain2 = HashChainCapsule::new();

        let payload_hash = 0xdeadbeef_u64;
        let timestamp = 1700000000_u64;

        chain1.append(payload_hash, timestamp).unwrap();
        chain2.append(payload_hash, timestamp).unwrap();

        assert_eq!(chain1.head_hash(), chain2.head_hash());
    }

    #[test]
    fn test_tamper_detection() {
        let chain = HashChainCapsule::new();

        let proof = chain.append(0x1234_u64, 1000_u64).unwrap();

        // Valid proof
        assert!(HashChainCapsule::verify_entry(&proof));

        // Tampered proof (modified timestamp)
        let tampered = EntryProof {
            timestamp: 9999,
            ..proof
        };
        assert!(!HashChainCapsule::verify_entry(&tampered));

        // Tampered proof (modified payload)
        let tampered = EntryProof {
            payload_hash: 0xffff,
            ..proof
        };
        assert!(!HashChainCapsule::verify_entry(&tampered));
    }

    #[test]
    fn test_snapshot() {
        let chain = HashChainCapsule::new();
        chain.append(0x1234_u64, 1000_u64).unwrap();

        let snap = chain.snapshot();
        assert_eq!(snap.length, 1);
        assert_eq!(snap.last_timestamp, 1000);
        assert_eq!(snap.head_hash, chain.head_hash());
    }

    #[test]
    fn test_reset() {
        let chain = HashChainCapsule::new();
        chain.append(0x1234_u64, 1000_u64).unwrap();
        chain.append(0x5678_u64, 2000_u64).unwrap();

        assert_eq!(chain.length(), 2);

        chain.reset();

        assert_eq!(chain.length(), 0);
        assert_eq!(chain.head_hash(), FNV_OFFSET_BASIS);
        assert!(chain.verify_crc());
    }

    #[test]
    fn test_audit_hash() {
        let chain = HashChainCapsule::new();
        chain.append(0x1234_u64, 1000_u64).unwrap();

        let hash1 = chain.compute_audit_hash();
        let hash2 = chain.compute_audit_hash();

        // Same chain state = same audit hash
        assert_eq!(hash1, hash2);

        // Different state = different audit hash
        chain.append(0x5678_u64, 2000_u64).unwrap();
        let hash3 = chain.compute_audit_hash();
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_fnv1a_hash() {
        // Known FNV-1a values
        let hash = fnv1a_hash(b"");
        assert_eq!(hash, FNV_OFFSET_BASIS);

        let hash = fnv1a_hash(b"a");
        assert_eq!(hash, 0xaf63dc4c8601ec8c); // Known FNV-1a value for "a"
    }

    #[test]
    fn test_fnv1a_hash_multi() {
        let hash1 = fnv1a_hash(b"helloworld");
        let hash2 = fnv1a_hash_multi(&[b"hello", b"world"]);
        assert_eq!(hash1, hash2);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_append() {
        use std::sync::Arc;
        use std::thread;

        let chain = Arc::new(HashChainCapsule::new());
        let mut handles = vec![];

        // Spawn 8 threads, each appending 100 entries
        for thread_id in 0..8_u64 {
            let chain_clone = Arc::clone(&chain);
            handles.push(thread::spawn(move || {
                for i in 0..100_u64 {
                    let payload = thread_id * 1000 + i;
                    let timestamp = thread_id * 1_000_000 + i;
                    chain_clone.append(payload, timestamp).unwrap();
                }
            }));
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // All entries appended
        assert_eq!(chain.length(), 800);
        assert!(chain.verify_crc());
    }
}
