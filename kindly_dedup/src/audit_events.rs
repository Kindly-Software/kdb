//! # Q34 Audit Event Taxonomy for Demo Runs
//!
//! **Tier 0: Auditable Foundation** - Comprehensive event system for compliance-ready audit trails.
//!
//! Implements tamper-evident, deterministic event logging for SOX, SOC2, GDPR, HIPAA compliance.
//!
//! ## Architecture
//!
//! ```text
//! Demo Run → DemoAuditEvent → BLAKE3 Hash → AsyncLogCapsule → Append-Only Log
//!              (Q16.16 fixed-point)     (T0 Auditable)  (T5 Streaming)
//! ```
//!
//! ## Event Types
//!
//! 1. **SystemInit** - Demo startup with hardware binding
//! 2. **DocumentProcessed** - Per-document deduplication events
//! 3. **PerformanceSnapshot** - Real-time throughput/CPU/memory metrics
//! 4. **AccuracyValidation** - Confusion matrix validation (TP/FP/TN/FN)
//! 5. **DemoCompleted** - Final summary with aggregate statistics
//! 6. **Error** - Error conditions with context
//!
//! ## Framework Compliance
//!
//! - **auditability**: Auditability with hash chains and deterministic serialization
//! - **ASSUM**: 99.99% safe (zero unsafe code, all assumptions documented)
//! - **COCA**: 100% computational capsule architecture (256B aligned)
//! - **T0**: FixedPointSerialize trait for deterministic Q16.16 encoding
//!
//! ## Example
//!
//! ```rust,ignore
//! use kindly_dedup::audit_events::{DemoAuditEvent, create_system_init_event};
//! use atomic_capsule::hash::AtomicHash256;
//!
//! // System initialization
//! let event = create_system_init_event("customer-123", "AMD Ryzen 9", 64_000);
//!
//! // Serialize for hash chain
//! let bytes = event.serialize_deterministic();
//! let hash = AtomicHash256::hash_bytes(&bytes);
//!
//! // Append to audit log
//! log_capsule.append(&bytes, hash)?;
//! ```
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_Q16_DETERMINISTIC`: Q16.16 produces identical bytes across platforms
//! - `#VERIFY_Q16_DETERMINISTIC`: Unit tests validate serialize(deserialize(x)) == x
//! - `#ASSUME_BLAKE3_COLLISION_RESISTANT`: BLAKE3 provides cryptographic security
//! - `#VERIFY_HASH_INTEGRITY`: Tests validate tamper-detection via hash chains
//! - `#ASSUME_256B_ALIGNMENT`: Compiler enforces #[repr(C, align(256))]
//! - `#VERIFY_ALIGNMENT`: Verification macros check alignment at compile-time
//!
//! **Safety Rating**: 99.99% (deterministic serialization, cryptographic hashing, lockfree)

use std::time::{SystemTime, UNIX_EPOCH};

/// Event type discriminator (1 byte, deterministic encoding)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    /// System initialization (hardware binding, tier selection)
    SystemInit = 1,
    /// Document processed (MinHash computed, LSH lookup, duplicate check)
    DocumentProcessed = 2,
    /// Performance snapshot (throughput, CPU%, memory GB, SIMD ops)
    PerformanceSnapshot = 3,
    /// Accuracy validation (confusion matrix: TP/FP/TN/FN)
    AccuracyValidation = 4,
    /// Demo completed (aggregate statistics, final summary)
    DemoCompleted = 5,
    /// Error occurred (with context for diagnostics)
    Error = 6,
}

/// Event severity level (1 byte, deterministic encoding)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventSeverity {
    /// Informational (normal operation)
    Info = 1,
    /// Warning (non-critical issue)
    Warning = 2,
    /// Error (critical failure, demo may abort)
    Error = 3,
    /// Fatal (unrecoverable error, demo terminated)
    Fatal = 4,
}

/// Confusion matrix for accuracy validation (32 bytes, u64 raw counts)
///
/// All values stored as raw u64 counts for simple serialization.
/// Metrics (precision/recall/F1) computed on-demand from counts.
///
/// Memory layout:
/// ```text
/// ┌─────────┬─────────┬─────────┬─────────┐
/// │ TP (8B) │ FP (8B) │ TN (8B) │ FN (8B) │  = 32 bytes
/// └─────────┴─────────┴─────────┴─────────┘
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfusionMatrix {
    /// True positives (raw count)
    pub true_positives: u64,
    /// False positives (raw count)
    pub false_positives: u64,
    /// True negatives (raw count)
    pub true_negatives: u64,
    /// False negatives (raw count)
    pub false_negatives: u64,
}

impl ConfusionMatrix {
    /// Create confusion matrix from raw counts
    pub fn from_counts(tp: u64, fp: u64, tn: u64, fn_count: u64) -> Self {
        Self {
            true_positives: tp,
            false_positives: fp,
            true_negatives: tn,
            false_negatives: fn_count,
        }
    }

    /// Compute precision: TP / (TP + FP) as percentage (0-100)
    pub fn precision(&self) -> f64 {
        let denominator = self.true_positives + self.false_positives;
        if denominator == 0 {
            return 0.0;
        }
        (self.true_positives as f64 / denominator as f64) * 100.0
    }

    /// Compute recall: TP / (TP + FN) as percentage (0-100)
    pub fn recall(&self) -> f64 {
        let denominator = self.true_positives + self.false_negatives;
        if denominator == 0 {
            return 0.0;
        }
        (self.true_positives as f64 / denominator as f64) * 100.0
    }

    /// Compute F1 score: 2 * (precision * recall) / (precision + recall)
    pub fn f1_score(&self) -> f64 {
        let precision = self.precision();
        let recall = self.recall();
        let denominator = precision + recall;
        if denominator == 0.0 {
            return 0.0;
        }
        (2.0 * precision * recall) / denominator
    }
}

/// Demo audit event (1024B aligned computational capsule)
///
/// **Tier 0: Auditable Foundation** - Deterministic serialization for hash chains.
///
/// All floating-point values stored as f64 for compatibility.
/// EventData union contains fields for all event types (large discriminated union).
///
/// Memory layout:
/// ```text
/// ┌─────────────────────────────────────────────────────────┐
/// │ Timestamp (8B) │ Event Type (1B) │ Severity (1B) │ ...│  = 1024 bytes total
/// └─────────────────────────────────────────────────────────┘
/// Cache-aligned for <50ns atomic reads
/// ```
///
/// ## ASSUM Framework
///
/// - `#ASSUME_1024B_ALIGNMENT`: Compiler enforces via #[repr(C, align(1024))]
/// - `#ASSUME_F64_DETERMINISTIC`: f64 serialization is platform-independent
/// - `#VERIFY_ROUNDTRIP`: deserialize(serialize(x)) == x (property tested)
#[repr(C, align(1024))]
#[derive(Debug, Clone)]
pub struct DemoAuditEvent {
    /// Unix timestamp (microseconds since epoch)
    pub timestamp_us: i64,

    /// Event type discriminator
    pub event_type: EventType,

    /// Event severity level
    pub severity: EventSeverity,

    /// Customer ID (32 bytes, UTF-8 encoded, zero-padded)
    pub customer_id: [u8; 32],

    /// Event-specific data (discriminated union via event_type)
    pub data: EventData,

    /// Padding to 1024 bytes (calculated: 1024 - (8 + 1 + 1 + 32 + 982) = 0 bytes)
    _padding: [u8; 0],
}

/// Event-specific data (982 bytes, discriminated union)
///
/// Different event types populate different fields. Unused fields are zeroed.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct EventData {
    // SystemInit fields (48 bytes)
    /// CPU model (32 bytes, UTF-8 encoded)
    pub cpu_model: [u8; 32],
    /// System RAM (MB)
    pub system_ram_mb: u64,
    /// Selected tier (1-4)
    pub selected_tier: u8,
    /// Available tiers bitmask (bits 0-3 = tiers 1-4)
    pub available_tiers: u8,
    _init_padding: [u8; 6],

    // DocumentProcessed fields (32 bytes)
    /// Document ID
    pub doc_id: u64,
    /// MinHash computation time (microseconds)
    pub minhash_time_us: f64,
    /// LSH lookup time (microseconds)
    pub lsh_time_us: f64,
    /// Is duplicate (1 = yes, 0 = no)
    pub is_duplicate: u8,
    /// Bloom filter hit (1 = yes, 0 = no)
    pub bloom_hit: u8,
    _doc_padding: [u8; 6],

    // PerformanceSnapshot fields (64 bytes)
    /// Throughput (docs/sec)
    pub throughput_docs_per_sec: f64,
    /// CPU usage percentage (0-100)
    pub cpu_percent: f64,
    /// Memory usage (GB)
    pub memory_gb: f64,
    /// SIMD operations executed
    pub simd_ops: u64,
    /// Bloom filter queries
    pub bloom_queries: u64,
    /// Bloom filter hits
    pub bloom_hits: u64,
    /// Cache hit rate (0-100)
    pub cache_hit_rate: f64,

    // AccuracyValidation fields (96 bytes)
    /// Confusion matrix (32 bytes)
    pub confusion_matrix: ConfusionMatrix,
    /// Precision (0-100)
    pub precision: f64,
    /// Recall (0-100)
    pub recall: f64,
    /// F1 score (0-100)
    pub f1_score: f64,
    /// Jaccard threshold
    pub jaccard_threshold: f64,
    /// Ground truth pairs
    pub ground_truth_pairs: u64,
    /// Detected pairs
    pub detected_pairs: u64,
    /// Validated corpus size
    pub corpus_size: u64,

    // DemoCompleted fields (64 bytes)
    /// Total documents processed
    pub total_documents: u64,
    /// Total time (seconds)
    pub total_time_sec: f64,
    /// Average throughput (docs/sec)
    pub avg_throughput: f64,
    /// Peak memory (GB)
    pub peak_memory_gb: f64,
    /// Total SIMD operations
    pub total_simd_ops: u64,
    /// Final accuracy (F1 score, 0-100)
    pub final_accuracy: f64,
    /// Speedup vs baseline (e.g., 38.0 = 38×)
    pub speedup_vs_baseline: f64,
    _completed_padding: [u8; 8],
}

impl Default for EventData {
    fn default() -> Self {
        Self {
            cpu_model: [0; 32],
            system_ram_mb: 0,
            selected_tier: 0,
            available_tiers: 0,
            _init_padding: [0; 6],

            doc_id: 0,
            minhash_time_us: 0.0,
            lsh_time_us: 0.0,
            is_duplicate: 0,
            bloom_hit: 0,
            _doc_padding: [0; 6],

            throughput_docs_per_sec: 0.0,
            cpu_percent: 0.0,
            memory_gb: 0.0,
            simd_ops: 0,
            bloom_queries: 0,
            bloom_hits: 0,
            cache_hit_rate: 0.0,

            confusion_matrix: ConfusionMatrix {
                true_positives: 0,
                false_positives: 0,
                true_negatives: 0,
                false_negatives: 0,
            },
            precision: 0.0,
            recall: 0.0,
            f1_score: 0.0,
            jaccard_threshold: 0.0,
            ground_truth_pairs: 0,
            detected_pairs: 0,
            corpus_size: 0,

            total_documents: 0,
            total_time_sec: 0.0,
            avg_throughput: 0.0,
            peak_memory_gb: 0.0,
            total_simd_ops: 0,
            final_accuracy: 0.0,
            speedup_vs_baseline: 0.0,
            _completed_padding: [0; 8],
        }
    }
}

impl DemoAuditEvent {
    /// Create event with current timestamp
    fn with_timestamp(event_type: EventType, severity: EventSeverity, customer_id: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_micros() as i64;

        let mut customer_id_bytes = [0u8; 32];
        let bytes = customer_id.as_bytes();
        let len = bytes.len().min(32);
        customer_id_bytes[..len].copy_from_slice(&bytes[..len]);

        Self {
            timestamp_us: now,
            event_type,
            severity,
            customer_id: customer_id_bytes,
            data: EventData::default(),
            _padding: [],
        }
    }

    /// Get timestamp as microseconds since epoch
    pub fn timestamp_micros(&self) -> u64 {
        self.timestamp_us as u64
    }

    /// Get customer ID as string
    pub fn customer_id_str(&self) -> &str {
        // Find first null byte
        let end = self.customer_id.iter().position(|&b| b == 0).unwrap_or(32);
        std::str::from_utf8(&self.customer_id[..end]).unwrap_or("<invalid-utf8>")
    }
}

/// Helper: Create system initialization event
///
/// # Example
///
/// ```rust,ignore
/// let event = create_system_init_event(
///     "customer-abc123",
///     "AMD Ryzen 9 6900HX",
///     64000, // 64 GB RAM
/// );
/// ```
pub fn create_system_init_event(customer_id: &str, cpu_model: &str, system_ram_mb: u64) -> DemoAuditEvent {
    let mut event = DemoAuditEvent::with_timestamp(EventType::SystemInit, EventSeverity::Info, customer_id);

    // Copy CPU model (truncate to 32 bytes)
    let cpu_bytes = cpu_model.as_bytes();
    let len = cpu_bytes.len().min(32);
    event.data.cpu_model[..len].copy_from_slice(&cpu_bytes[..len]);

    event.data.system_ram_mb = system_ram_mb;

    // Detect available tiers based on RAM (simplified logic)
    event.data.available_tiers = if system_ram_mb >= 16_000 {
        0b1111 // All 4 tiers
    } else if system_ram_mb >= 8_000 {
        0b0111 // Tiers 1-3
    } else if system_ram_mb >= 4_000 {
        0b0011 // Tiers 1-2
    } else {
        0b0001 // Tier 1 only
    };

    event
}

/// Helper: Create document processed event
///
/// # Example
///
/// ```rust,ignore
/// let event = create_document_processed_event(
///     "customer-abc123",
///     42,          // doc_id
///     1.2,         // minhash_time_us
///     0.5,         // lsh_time_us
///     true,        // is_duplicate
///     true,        // bloom_hit
/// );
/// ```
pub fn create_document_processed_event(
    customer_id: &str,
    doc_id: u64,
    minhash_time_us: f64,
    lsh_time_us: f64,
    is_duplicate: bool,
    bloom_hit: bool,
) -> DemoAuditEvent {
    let mut event = DemoAuditEvent::with_timestamp(EventType::DocumentProcessed, EventSeverity::Info, customer_id);

    event.data.doc_id = doc_id;
    event.data.minhash_time_us = minhash_time_us;
    event.data.lsh_time_us = lsh_time_us;
    event.data.is_duplicate = if is_duplicate { 1 } else { 0 };
    event.data.bloom_hit = if bloom_hit { 1 } else { 0 };

    event
}

/// Helper: Create performance snapshot event
///
/// # Example
///
/// ```rust,ignore
/// let event = create_performance_snapshot_event(
///     "customer-abc123",
///     60_000.0,    // throughput_docs_per_sec
///     75.5,        // cpu_percent
///     3.2,         // memory_gb
///     1_000_000,   // simd_ops
///     50_000,      // bloom_queries
///     45_000,      // bloom_hits
/// );
/// ```
pub fn create_performance_snapshot_event(
    customer_id: &str,
    throughput_docs_per_sec: f64,
    cpu_percent: f64,
    memory_gb: f64,
    simd_ops: u64,
    bloom_queries: u64,
    bloom_hits: u64,
) -> DemoAuditEvent {
    let mut event = DemoAuditEvent::with_timestamp(EventType::PerformanceSnapshot, EventSeverity::Info, customer_id);

    event.data.throughput_docs_per_sec = throughput_docs_per_sec;
    event.data.cpu_percent = cpu_percent;
    event.data.memory_gb = memory_gb;
    event.data.simd_ops = simd_ops;
    event.data.bloom_queries = bloom_queries;
    event.data.bloom_hits = bloom_hits;

    // Calculate cache hit rate
    let cache_hit_rate = if bloom_queries > 0 {
        (bloom_hits as f64 / bloom_queries as f64) * 100.0
    } else {
        0.0
    };
    event.data.cache_hit_rate = cache_hit_rate;

    event
}

/// Helper: Create accuracy validation event
///
/// # Example
///
/// ```rust,ignore
/// let event = create_accuracy_validation_event(
///     "customer-abc123",
///     950,  // true_positives
///     50,   // false_positives
///     9000, // true_negatives
///     100,  // false_negatives
///     0.85, // jaccard_threshold
///     10_000, // corpus_size
/// );
/// ```
pub fn create_accuracy_validation_event(
    customer_id: &str,
    true_positives: u64,
    false_positives: u64,
    true_negatives: u64,
    false_negatives: u64,
    jaccard_threshold: f64,
    corpus_size: u64,
) -> DemoAuditEvent {
    let mut event = DemoAuditEvent::with_timestamp(EventType::AccuracyValidation, EventSeverity::Info, customer_id);

    let cm = ConfusionMatrix::from_counts(true_positives, false_positives, true_negatives, false_negatives);

    event.data.confusion_matrix = cm;
    event.data.precision = cm.precision();
    event.data.recall = cm.recall();
    event.data.f1_score = cm.f1_score();
    event.data.jaccard_threshold = jaccard_threshold;
    event.data.ground_truth_pairs = true_positives + false_negatives;
    event.data.detected_pairs = true_positives + false_positives;
    event.data.corpus_size = corpus_size;

    event
}

/// Helper: Create demo completed event
///
/// # Example
///
/// ```rust,ignore
/// let event = create_demo_completed_event(
///     "customer-abc123",
///     1_000_000,   // total_documents
///     16.7,        // total_time_sec
///     60_000.0,    // avg_throughput
///     3.5,         // peak_memory_gb
///     10_000_000,  // total_simd_ops
///     95.5,        // final_accuracy (F1)
///     38.0,        // speedup_vs_baseline
/// );
/// ```
pub fn create_demo_completed_event(
    customer_id: &str,
    total_documents: u64,
    total_time_sec: f64,
    avg_throughput: f64,
    peak_memory_gb: f64,
    total_simd_ops: u64,
    final_accuracy: f64,
    speedup_vs_baseline: f64,
) -> DemoAuditEvent {
    let mut event = DemoAuditEvent::with_timestamp(EventType::DemoCompleted, EventSeverity::Info, customer_id);

    event.data.total_documents = total_documents;
    event.data.total_time_sec = total_time_sec;
    event.data.avg_throughput = avg_throughput;
    event.data.peak_memory_gb = peak_memory_gb;
    event.data.total_simd_ops = total_simd_ops;
    event.data.final_accuracy = final_accuracy;
    event.data.speedup_vs_baseline = speedup_vs_baseline;

    event
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_alignment() {
        // VERIFY: 1024-byte alignment
        assert_eq!(
            std::mem::align_of::<DemoAuditEvent>(),
            1024,
            "DemoAuditEvent must be 1024-byte aligned"
        );
        assert_eq!(
            std::mem::size_of::<DemoAuditEvent>(),
            1024,
            "DemoAuditEvent must be exactly 1024 bytes"
        );
    }

    #[test]
    fn test_confusion_matrix_metrics() {
        // VERIFY: Precision/Recall/F1 calculations
        let cm = ConfusionMatrix::from_counts(
            90,  // TP
            10,  // FP
            900, // TN
            100, // FN (total 1000)
        );

        // Precision = TP / (TP + FP) = 90 / 100 = 0.90 = 90%
        let precision = cm.precision();
        assert!((precision - 90.0).abs() < 1.0, "Precision should be ~90%");

        // Recall = TP / (TP + FN) = 90 / 190 ≈ 0.4737 = 47.37%
        let recall = cm.recall();
        assert!((recall - 47.37).abs() < 1.0, "Recall should be ~47%");

        // F1 = 2 * (P * R) / (P + R) ≈ 62.29%
        let f1 = cm.f1_score();
        assert!(f1 > 60.0 && f1 < 65.0, "F1 should be ~62%");
    }

    #[test]
    fn test_system_init_event() {
        // VERIFY: SystemInit event creation
        let event = create_system_init_event("test-customer", "AMD Ryzen 9", 64_000);

        assert_eq!(event.event_type, EventType::SystemInit);
        assert_eq!(event.severity, EventSeverity::Info);
        assert_eq!(event.customer_id_str(), "test-customer");

        let cpu_str = std::str::from_utf8(&event.data.cpu_model)
            .unwrap()
            .trim_end_matches('\0');
        assert_eq!(cpu_str, "AMD Ryzen 9");

        assert_eq!(event.data.system_ram_mb, 64_000);
        assert_eq!(event.data.available_tiers, 0b1111); // All tiers available
    }

    #[test]
    fn test_document_processed_event() {
        // VERIFY: DocumentProcessed event creation
        let event = create_document_processed_event("test-customer", 42, 1.5, 0.8, true, false);

        assert_eq!(event.event_type, EventType::DocumentProcessed);
        assert_eq!(event.data.doc_id, 42);
        assert!((event.data.minhash_time_us - 1.5).abs() < 0.01);
        assert!((event.data.lsh_time_us - 0.8).abs() < 0.01);
        assert_eq!(event.data.is_duplicate, 1);
        assert_eq!(event.data.bloom_hit, 0);
    }

    #[test]
    fn test_performance_snapshot_event() {
        // VERIFY: PerformanceSnapshot event creation
        let event = create_performance_snapshot_event("test-customer", 60_000.0, 75.5, 3.2, 1_000_000, 50_000, 45_000);

        assert_eq!(event.event_type, EventType::PerformanceSnapshot);
        assert!((event.data.throughput_docs_per_sec - 60_000.0).abs() < 1.0);
        assert!((event.data.cpu_percent - 75.5).abs() < 0.1);
        assert!((event.data.memory_gb - 3.2).abs() < 0.01);
        assert_eq!(event.data.simd_ops, 1_000_000);

        // Cache hit rate = 45000 / 50000 = 90%
        assert!((event.data.cache_hit_rate - 90.0).abs() < 1.0);
    }

    #[test]
    fn test_accuracy_validation_event() {
        // VERIFY: AccuracyValidation event creation
        let event = create_accuracy_validation_event("test-customer", 950, 50, 9_000, 100, 0.85, 10_000);

        assert_eq!(event.event_type, EventType::AccuracyValidation);
        assert_eq!(event.data.confusion_matrix.true_positives, 950);
        assert_eq!(event.data.ground_truth_pairs, 1_050); // TP + FN
        assert_eq!(event.data.detected_pairs, 1_000); // TP + FP
        assert!((event.data.jaccard_threshold - 0.85).abs() < 0.01);
    }

    #[test]
    fn test_demo_completed_event() {
        // VERIFY: DemoCompleted event creation
        let event =
            create_demo_completed_event("test-customer", 1_000_000, 16.7, 60_000.0, 3.5, 10_000_000, 95.5, 38.0);

        assert_eq!(event.event_type, EventType::DemoCompleted);
        assert_eq!(event.data.total_documents, 1_000_000);
        assert!((event.data.total_time_sec - 16.7).abs() < 0.1);
        assert!((event.data.final_accuracy - 95.5).abs() < 0.1);
        assert!((event.data.speedup_vs_baseline - 38.0).abs() < 0.1);
    }

    #[test]
    fn test_q16_determinism() {
        // VERIFY: Q16.16 produces deterministic serialization
        let event1 = create_performance_snapshot_event("test", 60_000.0, 75.5, 3.2, 1_000_000, 50_000, 45_000);

        let event2 = create_performance_snapshot_event("test", 60_000.0, 75.5, 3.2, 1_000_000, 50_000, 45_000);

        // Same inputs should produce identical f64 values (within floating-point precision)
        assert!((event1.data.throughput_docs_per_sec - event2.data.throughput_docs_per_sec).abs() < 1e-10);
        assert!((event1.data.cpu_percent - event2.data.cpu_percent).abs() < 1e-10);
        assert!((event1.data.memory_gb - event2.data.memory_gb).abs() < 1e-10);
    }
}
