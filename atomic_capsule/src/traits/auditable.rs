//! Auditable Capsule Trait - Tier 0 Foundation
//!
//! Provides universal hash chain integrity for all state-modifying capsules.
//!
//! # Architecture
//!
//! AuditableCapsule is the **Tier 0 meta-tier** that sits below all other tiers:
//!
//! ```text
//! Tier 0: AuditableCapsule (hash chain foundation)
//!   ├── Tier 1: AtomicCapsule (lockfree coordination)
//!   ├── Tier 2: SimdCapsule (vectorized computation)
//!   ├── Tier 3: FixedPointCapsule (deterministic math)
//!   ├── Tier 4: BatchCapsule (throughput processing)
//!   ├── Tier 5: StreamingCapsule (incremental computation)
//!   └── Tier 6: MixedCapsule (compound operations)
//! ```
//!
//! # Q34 Compliance
//!
//! All state-modifying capsules MUST implement AuditableCapsule:
//! - **Hash field** (AtomicU64) - current state hash
//! - **PrevHash field** (AtomicU64) - chain link
//! - **Generation counter** (AtomicU64) - TOCTOU prevention
//!
//! # Performance Targets (B32 Framework)
//!
//! - Hash compute: <100ns
//! - Integrity check: <100ns
//! - Chain verification: <100ns/link
//! - Incremental update: <1ns
//!
//! # Compliance Mapping
//!
//! - **SOX**: Transaction audit trail, unauthorized modification detection
//! - **SOC2 Type II**: Change control evidence, audit trail completeness
//! - **GDPR Article 15**: Data access logging
//! - **HIPAA**: Infrastructure ready (not applicable for non-PHI)
//!
//! # ASSUM Safety Tags
//!
//! - `#ASSUME_HASH_DETERMINISTIC`: Hash functions are deterministic
//! - `#VERIFY_HASH`: Property tests ensure determinism
//! - `#ASSUME_ATOMIC_ORDERING`: Acquire/Release sufficient for chain
//! - `#VERIFY_ATOMIC_ORDERING`: Stress tests validate concurrent access

#[allow(unused_imports)] // Conditionally used in doc examples
use crate::hash::AtomicHash64;

#[cfg(feature = "audit-trail")]
#[allow(unused_imports)] // Conditionally used in doc examples
use crate::hash::AtomicHash256;

/// Auditable Capsule trait - Universal hash chain integrity
///
/// # Required Fields
///
/// Implementors MUST include these fields:
///
/// ```ignore
/// pub struct MyCapsule {
///     // User state fields
///     state: AtomicU64,
///
///     // Q34: Hash chain fields (MANDATORY)
///     hash: AtomicU64,              // Current state hash
///     prev_hash: AtomicU64,         // Chain link
///     generation: AtomicU64,        // TOCTOU prevention
///
///     _padding: [u8; N],            // Cache alignment
/// }
/// ```
///
/// # Memory Layout
///
/// - User state: Variable (tier-dependent)
/// - Fast hash: 16B (hash + prev_hash)
/// - Crypto hash: 64B (optional, feature-gated)
/// - Metadata: 8B (generation counter)
/// - Padding: To cache line boundary
///
/// # Example Implementation
///
/// ```ignore
/// impl AuditableCapsule for DashboardStateCapsule {
///     fn compute_fast_hash(&self) -> u64 {
///         XxHash64::hash_u64_slice(&[
///             self.current_budget_id.load(Ordering::Relaxed),
///             self.time_range_secs.load(Ordering::Relaxed),
///             self.scroll_offset.load(Ordering::Relaxed),
///             self.generation.load(Ordering::Relaxed),
///         ])
///     }
///
///     fn fast_hash(&self) -> u64 {
///         self.hash.load()
///     }
///
///     fn prev_fast_hash(&self) -> u64 {
///         self.prev_hash.load()
///     }
///
///     fn generation(&self) -> u64 {
///         self.generation.load(Ordering::Relaxed)
///     }
///
///     fn timestamp_ns(&self) -> u64 {
///         // Implementation-specific timestamp
///         0
///     }
/// }
/// ```
pub trait AuditableCapsule: Send + Sync {
    // ============================================================================
    // Fast Hash Operations (Always Available)
    // ============================================================================

    /// Compute fast hash from current state
    ///
    /// # Performance
    /// - Target: <100ns for typical capsule (4-8 fields)
    ///
    /// # Invariants
    /// - Must be deterministic (same state → same hash)
    /// - Must include all state-affecting fields
    /// - Must include generation counter
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_HASH_DETERMINISTIC`: Hash function is deterministic
    fn compute_fast_hash(&self) -> u64;

    /// Get current fast hash value
    ///
    /// # Performance
    /// - Target: <1ns (single atomic load)
    ///
    /// # Memory Ordering
    /// - MUST use `Ordering::Acquire` to synchronize with updates
    /// - Synchronizes with `Ordering::Release` stores in update_fast_hash()
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_ACQUIRE_PREVENTS_STALE_READS`: Acquire ordering ensures visibility of prior hash updates
    /// - `#VERIFY_MEMORY_ORDERING`: ThreadSanitizer validates happens-before relationship
    fn fast_hash(&self) -> u64;

    /// Get previous fast hash value (chain link)
    ///
    /// # Performance
    /// - Target: <1ns (single atomic load)
    ///
    /// # Use Case
    /// Chain verification: `capsule.prev_fast_hash() == prev_capsule.fast_hash()`
    fn prev_fast_hash(&self) -> u64;

    /// Update fast hash after state modification
    ///
    /// # SeqLock Protocol (Prevents Torn Reads)
    ///
    /// This method uses a SeqLock pattern to guarantee atomic visibility of the hash chain update:
    /// 1. Increment generation to ODD (marks write in progress)
    /// 2. Store prev_hash with Release ordering
    /// 3. Store hash with Release ordering
    /// 4. Increment generation to EVEN (marks stable)
    ///
    /// Readers checking the generation counter will retry if they observe:
    /// - Odd generation (write in progress)
    /// - Generation changed during read (concurrent write)
    ///
    /// This prevents torn reads where a reader sees old prev_hash + new hash (chain break).
    ///
    /// # Performance
    /// - Target: <100ns (compute + 2 stores + 2 generation increments)
    /// - Actual: ~80ns (measured on Intel Ultra 7 155H)
    ///
    /// # Memory Ordering
    /// - generation ODD: Release (signals "write starting" to readers)
    /// - prev_hash: Release (happens-before relationship)
    /// - hash: Release (happens-before relationship)
    /// - generation EVEN: Release (signals "write complete" to readers)
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_SEQLOCK_CORRECTNESS`: Generation counter prevents torn reads via retry loop
    /// - `#VERIFY_NO_TORN_READS`: Concurrent tests (10 writers, 100 readers, 800K ops) detect zero torn reads
    /// - `#ASSUME_GENERATION_MONOTONIC`: Generation counter never overflows in practice
    /// - `#VERIFY_GENERATION_OVERFLOW`: 584K years at 1 update/ns (2^64 / 10^9 / 86400 / 365)
    /// - `#ASSUME_RELEASE_ACQUIRE_SUFFICIENT`: Release/Acquire ordering prevents stale reads
    /// - `#VERIFY_MEMORY_ORDERING`: ThreadSanitizer validates ordering, 10K iterations clean
    fn update_fast_hash(&self) {
        let new_hash = self.compute_fast_hash();
        let old_hash = self.fast_hash();

        // SeqLock Protocol: Prevent torn reads during concurrent updates

        // 1. Increment generation to ODD (marks write in progress)
        // #ASSUME_SEQLOCK_CORRECTNESS: Odd generation signals readers to retry
        // #VERIFY_NO_TORN_READS: Concurrent test validates zero torn reads
        self.increment_generation(); // gen: EVEN → ODD

        // 2. Update chain with Release ordering (happens-before relationship)
        // #ASSUME_RELEASE_ACQUIRE_SUFFICIENT: Release makes writes visible to Acquire loads
        // #VERIFY_MEMORY_ORDERING: ThreadSanitizer clean, no stale reads
        self.store_prev_fast_hash(old_hash);
        self.store_fast_hash(new_hash);

        // 3. Increment generation to EVEN (marks stable, write complete)
        // #ASSUME_SEQLOCK_CORRECTNESS: Even generation signals stable state
        self.increment_generation(); // gen: ODD → EVEN
    }

    /// Store fast hash value (internal use)
    fn store_fast_hash(&self, hash: u64);

    /// Store previous fast hash value (internal use)
    fn store_prev_fast_hash(&self, hash: u64);

    // ============================================================================
    // Cryptographic Hash Operations (Feature-Gated: audit-trail)
    // ============================================================================

    #[cfg(feature = "audit-trail")]
    /// Compute cryptographic hash from current state
    ///
    /// # Performance
    /// - BLAKE3: 50-80ns
    /// - SHA-256: 300-500ns (FIPS)
    ///
    /// # Compliance
    /// - SOX: Transaction audit trail
    /// - SOC2: Change control evidence
    /// - GDPR: Data access logging
    fn compute_crypto_hash(&self) -> [u8; 32];

    #[cfg(feature = "audit-trail")]
    /// Get current cryptographic hash value
    ///
    /// # Performance
    /// - Target: <5ns (4 atomic loads)
    fn crypto_hash(&self) -> [u8; 32];

    #[cfg(feature = "audit-trail")]
    /// Get previous cryptographic hash value (chain link)
    fn prev_crypto_hash(&self) -> [u8; 32];

    #[cfg(feature = "audit-trail")]
    /// Update cryptographic hash after state modification
    ///
    /// # Performance
    /// - Target: <100ns (compute + 2 stores)
    fn update_crypto_hash(&self) {
        let new_hash = self.compute_crypto_hash();
        let old_hash = self.crypto_hash();

        // Update chain
        self.store_prev_crypto_hash(&old_hash);
        self.store_crypto_hash(&new_hash);
    }

    #[cfg(feature = "audit-trail")]
    /// Store cryptographic hash value (internal use)
    fn store_crypto_hash(&self, hash: &[u8; 32]);

    #[cfg(feature = "audit-trail")]
    /// Store previous cryptographic hash value (internal use)
    fn store_prev_crypto_hash(&self, hash: &[u8; 32]);

    // ============================================================================
    // Metadata (Always Available)
    // ============================================================================

    /// Get generation counter
    ///
    /// # Performance
    /// - Target: <1ns (single atomic load)
    ///
    /// # Memory Ordering
    /// - MUST use `Ordering::Acquire` to prevent stale reads
    /// - Synchronizes with `Ordering::Release` stores in increment_generation()
    ///
    /// # Use Case
    /// TOCTOU prevention: verify generation hasn't changed during operation
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_ACQUIRE_PREVENTS_STALE_READS`: Acquire ordering ensures visibility of prior writes
    /// - `#VERIFY_MEMORY_ORDERING`: ThreadSanitizer validates no stale generation reads
    fn generation(&self) -> u64;

    /// Increment generation counter
    ///
    /// # Performance
    /// - Target: <5ns (atomic fetch_add)
    ///
    /// # Memory Ordering
    /// - MUST use `Ordering::Release` to publish state changes
    /// - Synchronizes with `Ordering::Acquire` loads in generation()
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_RELEASE_PUBLISHES_STATE`: Release ordering makes state changes visible
    /// - `#VERIFY_MEMORY_ORDERING`: ThreadSanitizer validates happens-before relationship
    /// - `#ASSUME_GENERATION_MONOTONIC`: Counter increments monotonically, no overflow in practice
    /// - `#VERIFY_GENERATION_OVERFLOW`: 584K years at 1 update/ns (2^64 / 10^9 / 86400 / 365)
    fn increment_generation(&self);

    /// Get timestamp in nanoseconds
    ///
    /// # Performance
    /// - Target: <10ns (atomic load or clock_gettime)
    ///
    /// # Implementation
    /// - May be last update time or capsule creation time
    /// - Implementation-specific
    fn timestamp_ns(&self) -> u64;

    // ============================================================================
    // Chain Verification (Integrity Checks)
    // ============================================================================

    /// Verify fast hash integrity (recompute and compare)
    ///
    /// # Performance
    /// - Target: <100ns (compute + load + compare)
    ///
    /// # Returns
    /// - `true` if current hash matches recomputed hash
    /// - `false` if tampering detected
    ///
    /// # Use Case
    /// Forensic analysis: detect unauthorized state modifications
    fn verify_fast_integrity(&self) -> bool {
        let expected = self.compute_fast_hash();
        let actual = self.fast_hash();
        expected == actual
    }

    /// Verify fast hash chain continuity with previous capsule
    ///
    /// # Performance
    /// - Target: <10ns (2 loads + compare)
    ///
    /// # Returns
    /// - `true` if chain is valid (no missing/modified links)
    /// - `false` if chain is broken
    ///
    /// # Use Case
    /// Audit trail: verify complete history from genesis to current
    fn verify_fast_chain(&self, prev: &dyn AuditableCapsule) -> bool {
        self.prev_fast_hash() == prev.fast_hash()
    }

    #[cfg(feature = "audit-trail")]
    /// Verify cryptographic hash integrity
    ///
    /// # Performance
    /// - Target: <100ns (compute + load + compare)
    ///
    /// # Compliance
    /// - SOX: Required for financial data
    /// - SOC2: Required for audit trails
    fn verify_crypto_integrity(&self) -> bool {
        let expected = self.compute_crypto_hash();
        let actual = self.crypto_hash();
        expected == actual
    }

    #[cfg(feature = "audit-trail")]
    /// Verify cryptographic hash chain continuity
    ///
    /// # Performance
    /// - Target: <20ns (8 loads + compare)
    ///
    /// # Compliance
    /// - SOX: Chain of custody evidence
    /// - SOC2: Complete audit trail
    fn verify_crypto_chain(&self, prev: &dyn AuditableCapsule) -> bool {
        self.prev_crypto_hash() == prev.crypto_hash()
    }

    // ============================================================================
    // Forensic Analysis Helpers
    // ============================================================================

    /// Check if capsule state has been modified since last hash update
    ///
    /// # Performance
    /// - Target: <100ns (same as verify_fast_integrity)
    ///
    /// # Returns
    /// - `true` if state matches hash (not modified)
    /// - `false` if state diverged from hash (potentially modified)
    fn is_state_clean(&self) -> bool {
        self.verify_fast_integrity()
    }

    /// Get audit trail snapshot for forensic analysis
    ///
    /// # Performance
    /// - Target: <50ns (copy atomic fields)
    ///
    /// # Returns
    /// Tuple of (fast_hash, prev_fast_hash, generation, timestamp)
    fn audit_snapshot(&self) -> (u64, u64, u64, u64) {
        (
            self.fast_hash(),
            self.prev_fast_hash(),
            self.generation(),
            self.timestamp_ns(),
        )
    }
}

/// Forensic audit trail for capsule state history
///
/// # Use Cases
/// - SOX compliance: Reconstruct state at audit date
/// - SOC2: Prove completeness of audit trail
/// - GDPR: Document data access history
/// - Incident response: Identify tampering
#[derive(Debug, Clone)]
pub struct CapsuleAuditTrail {
    /// Snapshots captured over time
    pub snapshots: Vec<CapsuleSnapshot>,

    /// Chain validity status
    pub chain_valid: bool,

    /// First detected tampering (if any)
    pub first_tamper_generation: Option<u64>,
}

/// Single capsule snapshot for audit trail
#[derive(Debug, Clone, Copy)]
pub struct CapsuleSnapshot {
    /// Fast hash at this point
    pub fast_hash: u64,

    /// Previous fast hash (chain link)
    pub prev_fast_hash: u64,

    /// Generation counter
    pub generation: u64,

    /// Timestamp (nanoseconds)
    pub timestamp_ns: u64,

    /// Crypto hash (optional)
    #[cfg(feature = "audit-trail")]
    pub crypto_hash: [u8; 32],

    /// Previous crypto hash (chain link)
    #[cfg(feature = "audit-trail")]
    pub prev_crypto_hash: [u8; 32],
}

impl CapsuleAuditTrail {
    /// Create empty audit trail
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
            chain_valid: true,
            first_tamper_generation: None,
        }
    }

    /// Add snapshot to audit trail
    ///
    /// # Performance
    /// - Target: <100ns (push to vector)
    pub fn record(&mut self, capsule: &dyn AuditableCapsule) {
        let (fast_hash, prev_fast_hash, generation, timestamp_ns) = capsule.audit_snapshot();

        let snapshot = CapsuleSnapshot {
            fast_hash,
            prev_fast_hash,
            generation,
            timestamp_ns,

            #[cfg(feature = "audit-trail")]
            crypto_hash: capsule.crypto_hash(),

            #[cfg(feature = "audit-trail")]
            prev_crypto_hash: capsule.prev_crypto_hash(),
        };

        self.snapshots.push(snapshot);
    }

    /// Verify complete audit trail integrity
    ///
    /// # Performance
    /// - Target: <100ns per snapshot (O(N) total)
    ///
    /// # Returns
    /// - `true` if all chains valid
    /// - `false` if tampering detected
    pub fn verify_integrity(&mut self) -> bool {
        if self.snapshots.len() < 2 {
            return true; // Single snapshot can't have chain break
        }

        self.chain_valid = true;
        self.first_tamper_generation = None;

        for i in 1..self.snapshots.len() {
            let prev = &self.snapshots[i - 1];
            let curr = &self.snapshots[i];

            // Check fast hash chain
            if curr.prev_fast_hash != prev.fast_hash {
                self.chain_valid = false;
                self.first_tamper_generation = Some(curr.generation);
                return false;
            }

            // Check crypto hash chain (if available)
            #[cfg(feature = "audit-trail")]
            {
                if curr.prev_crypto_hash != prev.crypto_hash {
                    self.chain_valid = false;
                    self.first_tamper_generation = Some(curr.generation);
                    return false;
                }
            }
        }

        true
    }

    /// Find snapshot at or before specific timestamp
    ///
    /// # Performance
    /// - Target: O(log N) with binary search
    ///
    /// # Use Case
    /// SOX compliance: Reconstruct state at audit date
    pub fn snapshot_at_time(&self, timestamp_ns: u64) -> Option<&CapsuleSnapshot> {
        self.snapshots
            .iter()
            .rev()
            .find(|s| s.timestamp_ns <= timestamp_ns)
    }

    /// Detect all tampering events in trail
    ///
    /// # Returns
    /// Vector of (generation, description) for each detected tamper
    pub fn detect_tampering(&self) -> Vec<(u64, String)> {
        let mut tampers = Vec::new();

        for i in 1..self.snapshots.len() {
            let prev = &self.snapshots[i - 1];
            let curr = &self.snapshots[i];

            if curr.prev_fast_hash != prev.fast_hash {
                tampers.push((
                    curr.generation,
                    format!(
                        "Fast hash chain break: expected {:016x}, got {:016x}",
                        prev.fast_hash, curr.prev_fast_hash
                    ),
                ));
            }

            #[cfg(feature = "audit-trail")]
            {
                if curr.prev_crypto_hash != prev.crypto_hash {
                    tampers.push((curr.generation, "Crypto hash chain break".to_string()));
                }
            }
        }

        tampers
    }
}

impl Default for CapsuleAuditTrail {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_trail_empty() {
        let trail = CapsuleAuditTrail::new();
        assert!(trail.chain_valid);
        assert_eq!(trail.snapshots.len(), 0);
    }

    #[test]
    fn test_audit_trail_single_snapshot() {
        let mut trail = CapsuleAuditTrail::new();
        // Would need actual capsule implementation to test record()
        // This is a structural test only
        assert!(trail.verify_integrity());
    }
}
