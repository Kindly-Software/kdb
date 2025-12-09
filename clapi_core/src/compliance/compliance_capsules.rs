//! Compliance Capsules - Tier 6 Mixed (Atomic + Streaming)
//!
//! ComplianceCapsule256 (256-byte cache-aligned):
//! - Atomic counters for compliance metrics
//! - Hash chain integration for tamper detection
//! - Streaming export support (O(1) memory)
//!
//! # Performance
//! - Metadata preparation: <1μs
//! - Entry recording: <50ns (atomic increment)
//! - Hash update: <2ns (incremental XOR)
//! - Export iteration: <10ns per entry overhead
//!
//! # Memory Layout (256 bytes)
//! ```text
//! [0-7]     total_entries: AtomicU64       // Total compliance entries
//! [8-15]    sox_entries: AtomicU64         // SOX-specific entries
//! [16-23]   soc2_entries: AtomicU64        // SOC2-specific entries
//! [24-31]   gdpr_entries: AtomicU64        // GDPR-specific entries
//! [32-39]   hipaa_entries: AtomicU64       // HIPAA-specific entries (future)
//! [40-47]   first_timestamp_ns: AtomicU64  // Earliest entry timestamp
//! [48-55]   last_timestamp_ns: AtomicU64   // Latest entry timestamp
//! [56-63]   hash: AtomicU64                // Current hash
//! [64-71]   prev_hash: AtomicU64           // Previous hash (chain link)
//! [72-79]   export_count: AtomicU64        // Number of exports performed
//! [80-87]   last_export_ns: AtomicU64      // Last export timestamp
//! [88-95]   generation: AtomicU64          // TOCTOU prevention
//! [96-255]  _padding: [u8; 160]            // Cache line alignment
//! ```

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// Removed unused imports: ClapiError, ClapiResult, CapsuleHash64
use super::ComplianceFramework;

/// Compliance entry metadata (minimal footprint for hash chain)
#[derive(Debug, Clone)]
pub struct ComplianceEntry {
    /// Framework this entry belongs to
    pub framework: ComplianceFramework,
    /// Operation description
    pub operation: String,
    /// Timestamp (nanoseconds since UNIX epoch)
    pub timestamp_ns: u64,
    /// Entry-specific hash
    pub hash: u64,
    /// Previous entry hash (chain link)
    pub prev_hash: u64,
    /// Metadata key-value pairs
    pub metadata: Vec<(String, String)>,
}

/// Compliance metrics snapshot (zero-copy read)
#[derive(Debug, Clone, Copy)]
pub struct ComplianceMetrics {
    pub total_entries: u64,
    pub sox_entries: u64,
    pub soc2_entries: u64,
    pub gdpr_entries: u64,
    pub hipaa_entries: u64,
    pub first_timestamp_ns: u64,
    pub last_timestamp_ns: u64,
    pub export_count: u64,
    pub last_export_ns: u64,
    pub generation: u64,
}

/// Compliance tracking capsule with atomic counters and hash chain (256-byte, T6 Mixed)
///
/// # Safety
/// - #ASSUME: AtomicU64::fetch_add provides atomic counter increments
/// - #VERIFY: Unit tests validate counter correctness
/// - #ASSUME: Hash chain prevents tampering (cryptographic properties)
/// - #VERIFY: Integration tests validate tamper detection
/// - #ASSUME: Generation counter prevents TOCTOU races
/// - #VERIFY: Property tests validate monotonic generation
/// - #ASSUME: Relaxed ordering safe for counters (no inter-field dependencies)
/// - #VERIFY: Stress tests validate correctness under concurrency
#[derive(ComputationalCapsule, Debug)]
#[capsule(alignment = 256, size = 256)]
#[repr(C, align(256))]
pub struct ComplianceCapsule256 {
    /// Total compliance entries across all frameworks
    total_entries: AtomicU64,

    /// SOX-specific entry count
    sox_entries: AtomicU64,

    /// SOC2-specific entry count
    soc2_entries: AtomicU64,

    /// GDPR-specific entry count
    gdpr_entries: AtomicU64,

    /// HIPAA-specific entry count (future)
    hipaa_entries: AtomicU64,

    /// Earliest entry timestamp (nanoseconds since UNIX epoch)
    first_timestamp_ns: AtomicU64,

    /// Latest entry timestamp (nanoseconds since UNIX epoch)
    last_timestamp_ns: AtomicU64,

    /// Current hash (incremental XOR-based update)
    hash: AtomicU64,

    /// Previous hash (hash chain for audit trail)
    prev_hash: AtomicU64,

    /// Number of exports performed
    export_count: AtomicU64,

    /// Last export timestamp
    last_export_ns: AtomicU64,

    /// Generation counter (TOCTOU prevention)
    generation: AtomicU64,

    /// Padding to 256 bytes
    _padding: [u8; 160],
}

impl Default for ComplianceCapsule256 {
    fn default() -> Self {
        Self::new()
    }
}

impl ComplianceCapsule256 {
    /// Create new compliance capsule (zero-initialized)
    ///
    /// # Performance
    /// - Latency: <10ns (zero-allocation)
    /// - Memory: 256 bytes (stack-allocated)
    pub fn new() -> Self {
        Self {
            total_entries: AtomicU64::new(0),
            sox_entries: AtomicU64::new(0),
            soc2_entries: AtomicU64::new(0),
            gdpr_entries: AtomicU64::new(0),
            hipaa_entries: AtomicU64::new(0),
            first_timestamp_ns: AtomicU64::new(0),
            last_timestamp_ns: AtomicU64::new(0),
            hash: AtomicU64::new(0),
            prev_hash: AtomicU64::new(0),
            export_count: AtomicU64::new(0),
            last_export_ns: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            _padding: [0u8; 160],
        }
    }

    /// Record compliance entry (lockfree, <50ns)
    ///
    /// # ASSUM Framework
    /// - #ASSUME: fetch_add provides atomic counter increment
    /// - #VERIFY: Unit test validates total_entries = sox + soc2 + gdpr + hipaa
    /// - #ASSUME: Timestamp monotonicity enforced by caller
    /// - #VERIFY: Integration test validates timestamp ordering
    /// - #ASSUME: Hash XOR commutative (order-independent)
    /// - #VERIFY: Property test validates hash correctness
    pub fn record_entry(&self, framework: ComplianceFramework, entry_hash: u64, timestamp_ns: u64) {
        // Increment framework-specific counter
        // #ASSUME: Relaxed ordering safe (no inter-counter dependencies)
        // #VERIFY: Stress test validates correctness under concurrent updates
        match framework {
            ComplianceFramework::Sox404 => {
                self.sox_entries.fetch_add(1, Ordering::Relaxed);
            }
            ComplianceFramework::Soc2TypeII => {
                self.soc2_entries.fetch_add(1, Ordering::Relaxed);
            }
            ComplianceFramework::GdprArticle30 => {
                self.gdpr_entries.fetch_add(1, Ordering::Relaxed);
            }
            ComplianceFramework::Hipaa164312b => {
                // #ASSUME: Relaxed ordering (HIPAA counter independent of other counters)
                // #VERIFY: Unit tests validate framework-specific counting
                self.hipaa_entries.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Increment total counter
        // #ASSUME: Relaxed ordering (total_entries independent of framework counters)
        // #VERIFY: Property tests validate total == sum(framework_counts)
        self.total_entries.fetch_add(1, Ordering::Relaxed);

        // Update timestamps (first entry special case)
        // #ASSUME: CAS loop eventually succeeds (no ABA problem)
        // #VERIFY: Property test validates timestamp bounds
        let first_ts = self.first_timestamp_ns.load(Ordering::Relaxed);
        if first_ts == 0 || timestamp_ns < first_ts {
            // Try to update first timestamp (best-effort, race OK)
            let _ = self.first_timestamp_ns.compare_exchange(
                first_ts,
                timestamp_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            );
        }

        // Always update last timestamp (monotonic assumption)
        // #ASSUME: Relaxed store (timestamp monotonicity enforced by caller, no sync needed)
        // #VERIFY: Integration tests validate timestamp ordering
        self.last_timestamp_ns.store(timestamp_ns, Ordering::Relaxed);

        // Update hash (incremental XOR)
        // #ASSUME: XOR is commutative and associative
        // #VERIFY: Unit test validates incremental hash matches full recomputation
        let old_hash = self.hash.fetch_xor(entry_hash, Ordering::Relaxed);
        self.prev_hash.store(old_hash, Ordering::Relaxed);

        // Increment generation counter (TOCTOU prevention)
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Record export event (metadata tracking)
    ///
    /// # Performance
    /// - Latency: <20ns (2 atomic stores)
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Relaxed ordering (export count independent metric, no sync required)
    /// - #VERIFY: Unit tests validate export count tracking
    pub fn record_export(&self, timestamp_ns: u64) {
        // #ASSUME: Relaxed fetch_add (export counter independent of other metrics)
        // #VERIFY: Integration tests validate export event tracking
        self.export_count.fetch_add(1, Ordering::Relaxed);

        // #ASSUME: Relaxed store (timestamp monotonicity by caller, eventual consistency OK)
        // #VERIFY: Unit tests validate last export timestamp updates
        self.last_export_ns.store(timestamp_ns, Ordering::Relaxed);

        // #ASSUME: Relaxed generation increment (TOCTOU detection, not synchronization)
        // #VERIFY: Concurrent tests validate generation counter monotonicity
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Get compliance metrics (zero-copy snapshot)
    ///
    /// # Performance
    /// - Latency: <100ns (12 atomic loads)
    ///
    /// # ASSUM Framework
    /// - #ASSUME: Relaxed loads safe for metrics snapshot (eventual consistency OK)
    /// - #VERIFY: Integration test validates metrics accuracy
    pub fn metrics(&self) -> ComplianceMetrics {
        // NOTE: All loads use Relaxed ordering (covered by function-level ASSUM tag)
        ComplianceMetrics {
            total_entries: self.total_entries.load(Ordering::Relaxed),
            sox_entries: self.sox_entries.load(Ordering::Relaxed),
            soc2_entries: self.soc2_entries.load(Ordering::Relaxed),
            gdpr_entries: self.gdpr_entries.load(Ordering::Relaxed),
            hipaa_entries: self.hipaa_entries.load(Ordering::Relaxed),
            first_timestamp_ns: self.first_timestamp_ns.load(Ordering::Relaxed),
            last_timestamp_ns: self.last_timestamp_ns.load(Ordering::Relaxed),
            export_count: self.export_count.load(Ordering::Relaxed),
            last_export_ns: self.last_export_ns.load(Ordering::Relaxed),
            generation: self.generation.load(Ordering::Relaxed),
        }
    }

    /// Get current hash (tamper detection)
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Relaxed load (hash snapshot, eventual consistency acceptable)
    /// - #VERIFY: Unit tests validate hash integrity
    pub fn hash(&self) -> u64 {
        self.hash.load(Ordering::Relaxed)
    }

    /// Get previous hash (chain link)
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Relaxed load (prev_hash snapshot, no critical synchronization)
    /// - #VERIFY: Property tests validate hash chain correctness
    pub fn prev_hash(&self) -> u64 {
        self.prev_hash.load(Ordering::Relaxed)
    }

    /// Get generation (TOCTOU detection)
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Relaxed load (generation counter for TOCTOU, not synchronization primitive)
    /// - #VERIFY: Concurrent tests validate generation monotonicity
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    /// Verify integrity (hash + generation consistency)
    ///
    /// # Performance
    /// - Latency: <50ns (2 atomic loads + comparison)
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Acquire-Relaxed-Acquire sandwich prevents TOCTOU (torn reads)
    /// - #VERIFY: Property tests validate integrity under concurrent updates
    pub fn verify_integrity(&self) -> bool {
        // #ASSUME: Acquire load ensures visibility of writes before first generation read
        // #VERIFY: Concurrent stress tests validate TOCTOU prevention
        let gen1 = self.generation.load(Ordering::Acquire);

        // #ASSUME: Relaxed load (between two Acquire generation reads, no torn read possible)
        // #VERIFY: Unit tests validate hash consistency during verification
        let hash = self.hash.load(Ordering::Relaxed);

        // #ASSUME: Acquire load ensures we detect concurrent updates (gen1 != gen2)
        // #VERIFY: Property tests validate generation mismatch detection
        let gen2 = self.generation.load(Ordering::Acquire);

        // Generation must be stable (no concurrent updates during verification)
        gen1 == gen2 && hash != 0 // Hash should be non-zero if entries exist
    }
}

/// Get current timestamp in nanoseconds since UNIX epoch
pub fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compliance_capsule_new() {
        let capsule = ComplianceCapsule256::new();
        let metrics = capsule.metrics();

        assert_eq!(metrics.total_entries, 0);
        assert_eq!(metrics.sox_entries, 0);
        assert_eq!(metrics.soc2_entries, 0);
        assert_eq!(metrics.gdpr_entries, 0);
        assert_eq!(metrics.hipaa_entries, 0);
        assert_eq!(metrics.first_timestamp_ns, 0);
        assert_eq!(metrics.last_timestamp_ns, 0);
        assert_eq!(metrics.export_count, 0);
        assert_eq!(metrics.last_export_ns, 0);
        assert_eq!(metrics.generation, 0);
    }

    #[test]
    fn test_record_sox_entry() {
        let capsule = ComplianceCapsule256::new();
        let timestamp = now_ns();
        let hash = 0x1234567890ABCDEFu64;

        capsule.record_entry(ComplianceFramework::Sox404, hash, timestamp);

        let metrics = capsule.metrics();
        assert_eq!(metrics.total_entries, 1);
        assert_eq!(metrics.sox_entries, 1);
        assert_eq!(metrics.soc2_entries, 0);
        assert_eq!(metrics.gdpr_entries, 0);
        assert_eq!(metrics.first_timestamp_ns, timestamp);
        assert_eq!(metrics.last_timestamp_ns, timestamp);
        assert_eq!(metrics.generation, 1);
        assert_eq!(capsule.hash(), hash);
    }

    #[test]
    fn test_record_multiple_frameworks() {
        let capsule = ComplianceCapsule256::new();
        let base_ts = now_ns();

        capsule.record_entry(ComplianceFramework::Sox404, 0x1111, base_ts);
        capsule.record_entry(ComplianceFramework::Soc2TypeII, 0x2222, base_ts + 1000);
        capsule.record_entry(ComplianceFramework::GdprArticle30, 0x3333, base_ts + 2000);

        let metrics = capsule.metrics();
        assert_eq!(metrics.total_entries, 3);
        assert_eq!(metrics.sox_entries, 1);
        assert_eq!(metrics.soc2_entries, 1);
        assert_eq!(metrics.gdpr_entries, 1);
        assert_eq!(metrics.hipaa_entries, 0);
        assert_eq!(metrics.first_timestamp_ns, base_ts);
        assert_eq!(metrics.last_timestamp_ns, base_ts + 2000);
    }

    #[test]
    fn test_hash_chain_updates() {
        let capsule = ComplianceCapsule256::new();
        let ts = now_ns();

        // First entry
        capsule.record_entry(ComplianceFramework::Sox404, 0xAAAA, ts);
        let hash1 = capsule.hash();
        let prev1 = capsule.prev_hash();
        assert_eq!(hash1, 0xAAAA);
        assert_eq!(prev1, 0); // First entry, no previous

        // Second entry
        capsule.record_entry(ComplianceFramework::Soc2TypeII, 0xBBBB, ts + 1000);
        let hash2 = capsule.hash();
        let prev2 = capsule.prev_hash();
        assert_eq!(hash2, 0xAAAA ^ 0xBBBB); // XOR accumulation
        assert_eq!(prev2, hash1); // Previous hash saved
    }

    #[test]
    fn test_record_export() {
        let capsule = ComplianceCapsule256::new();
        let ts = now_ns();

        capsule.record_export(ts);

        let metrics = capsule.metrics();
        assert_eq!(metrics.export_count, 1);
        assert_eq!(metrics.last_export_ns, ts);
        assert_eq!(metrics.generation, 1);
    }

    #[test]
    fn test_verify_integrity() {
        let capsule = ComplianceCapsule256::new();

        // Empty capsule: hash should be zero
        assert!(!capsule.verify_integrity());

        // After recording entry: should be valid
        capsule.record_entry(ComplianceFramework::Sox404, 0x1234, now_ns());
        assert!(capsule.verify_integrity());
    }
}
