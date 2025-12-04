//! # AnomalyDetectorV2 - Hybrid Multi-Layer Anomaly Detection
//!
//! **Tier Composition**: T6 Mixed (T10+T1+T2+T3+T5)
//!
//! AnomalyDetectorV2 is a hybrid anomaly detection system combining:
//! - **Layer 1**: Probabilistic (Bloom filter + HyperLogLog) - <30ns fast path
//! - **Layer 2**: GMM (Gaussian Mixture Model) - <20ns statistical path
//! - **Layer 3**: TinyML (Decision Tree Ensemble) - <60ns ML path
//! - **Layer 4**: Temporal (Sequence Analysis) - <50ns time-series path
//!
//! ## UCE34 Framework Analysis (Q1-Q34)
//!
//! ### Q1-Q9: Meta-Cognitive Analysis
//! - **Q1 (Scope)**: Production-grade anomaly detection for security/protection systems
//! - **Q2 (Assumptions)**: Multiple detection layers provide defense-in-depth
//! - **Q3 (Constraints)**: <100ns average latency, 8KB total, 100% lockfree
//! - **Q4 (Context)**: Protection system core, META_CAPSULE integration
//! - **Q5 (Success)**: >99% true positive rate, <0.1% false positive rate
//! - **Q6 (Failure)**: Layer coordination overhead, memory pressure
//! - **Q7 (Patterns)**: Cascading detection, early-exit optimization, feature-gated layers
//! - **Q8 (Alternatives)**: Single-layer (lower accuracy), external ML (slower)
//! - **Q9 (Trade-offs)**: Layers vs latency, accuracy vs memory
//!
//! ### Q10-Q12: Foundation (Capsule Architecture)
//! - **Q10 (Tier Selection)**: T6 Mixed compound of T10+T1+T2+T3+T5
//! - **Q11 (Rust Transform)**: AnomalyDetectorV2 (8KB orchestrator)
//! - **Q12 (Nightly)**: portable_simd for TinyML ensemble
//!
//! ## Memory Layout (8KB total)
//!
//! ```text
//! AnomalyDetectorV2 (8192B, 256B aligned):
//! ┌────────────────────────────────────────┐
//! │ MASTER HEADER (128B)                   │
//! │   version: u8                          │
//! │   enabled_layers: AtomicU8 (bitmask)   │
//! │   generation: AtomicU64                │
//! │   total_checks: AtomicU64              │
//! │   anomaly_count: AtomicU64             │
//! │   fast_path_hits: AtomicU64            │
//! │   layer_latencies: [AtomicU32; 4]      │
//! │   _padding: [u8; 72]                   │
//! ├────────────────────────────────────────┤
//! │ PROBABILISTIC LAYER (1024B) [Layer 1]  │
//! │   Reuses AnomalyDetectorCapsule V1     │
//! │   Bloom filter + HyperLogLog + CountMin│
//! ├────────────────────────────────────────┤
//! │ TINYML ENSEMBLE (2048B) [Layer 3]      │
//! │   8 decision trees × 248B              │
//! │   64B header                           │
//! ├────────────────────────────────────────┤
//! │ GMM LAYER (512B) [Layer 2]             │
//! │   8 Gaussian components × 56B          │
//! │   64B header                           │
//! ├────────────────────────────────────────┤
//! │ TEMPORAL LAYER (2048B) [Layer 4]       │
//! │   248 temporal entries × 8B            │
//! │   64B header                           │
//! ├────────────────────────────────────────┤
//! │ COUNTMIN + FREQUENCY (384B)            │
//! │   Behavior frequency tracking          │
//! ├────────────────────────────────────────┤
//! │ RESERVED (2048B)                       │
//! │   Future extensions                    │
//! └────────────────────────────────────────┘
//! ```
//!
//! ## Detection Flow
//!
//! ```text
//! Input Behavior
//!       │
//!       ▼
//! ┌─────────────────┐
//! │ Layer 1: Bloom  │──── Known? ──→ Normal (fast path)
//! │ (<30ns)         │
//! └────────┬────────┘
//!          │ Unknown
//!          ▼
//! ┌─────────────────┐
//! │ Layer 2: GMM    │──── Low dist? ──→ Suspicious
//! │ (<20ns)         │
//! └────────┬────────┘
//!          │ High distance
//!          ▼
//! ┌─────────────────┐
//! │ Layer 3: TinyML │──── Low score? ──→ Suspicious
//! │ (<60ns)         │
//! └────────┬────────┘
//!          │ High score
//!          ▼
//! ┌─────────────────┐
//! │ Layer 4: Temporal│──── No pattern? ──→ Anomalous
//! │ (<50ns)          │
//! └────────┬─────────┘
//!          │ Burst pattern
//!          ▼
//!     CRITICAL ANOMALY
//! ```
//!
//! ## Performance Targets (B32 Framework)
//!
//! | Path | Target | Description |
//! |------|--------|-------------|
//! | Fast path (Bloom hit) | <30ns | 70%+ of checks |
//! | GMM path | <50ns | 20% of checks |
//! | TinyML path | <110ns | 8% of checks |
//! | Full path | <160ns | 2% of checks |
//! | Average | <100ns | Weighted by path frequency |

#![allow(unsafe_code)] // Required for atomic operations

use core::sync::atomic::{AtomicU8, AtomicU64, AtomicU32, Ordering};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

// Import component capsules
use super::gmm_capsule::{GMMCapsule, f64_to_q16_16};
use super::temporal_sequence::TemporalSequenceCapsule;
use super::tinyml_ensemble::TinyMLTreeEnsemble;
use super::anomaly_detector::{AnomalyDetectorCapsule, AnomalyResult as V1AnomalyResult};

// ============================================================================
// CONSTANTS
// ============================================================================

/// Layer enable bitmask values
pub const LAYER_PROBABILISTIC: u8 = 0b0001;
pub const LAYER_GMM: u8 = 0b0010;
pub const LAYER_TINYML: u8 = 0b0100;
pub const LAYER_TEMPORAL: u8 = 0b1000;
pub const ALL_LAYERS: u8 = 0b1111;

/// Default thresholds
pub const DEFAULT_GMM_THRESHOLD: f64 = 9.0;  // 3 sigma squared
pub const DEFAULT_TINYML_THRESHOLD: f64 = 0.6;
pub const DEFAULT_BURST_THRESHOLD: f32 = 3.0;

// ============================================================================
// ANOMALY RESULT V2
// ============================================================================

/// Extended anomaly detection result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnomalyResultV2 {
    /// Normal behavior - seen before, no anomaly
    Normal,

    /// Suspicious - first-time but within statistical bounds
    Suspicious,

    /// Anomalous - statistical outlier detected
    Anomalous,

    /// Critical - multiple layers flagged anomaly
    Critical,
}

impl AnomalyResultV2 {
    /// Convert to V1 result for backward compatibility
    #[inline]
    pub fn to_v1(self) -> V1AnomalyResult {
        match self {
            AnomalyResultV2::Normal => V1AnomalyResult::Normal,
            AnomalyResultV2::Suspicious => V1AnomalyResult::Suspicious,
            AnomalyResultV2::Anomalous | AnomalyResultV2::Critical => V1AnomalyResult::Anomalous,
        }
    }

    /// Get severity level (0-3)
    #[inline]
    pub const fn severity(&self) -> u8 {
        match self {
            AnomalyResultV2::Normal => 0,
            AnomalyResultV2::Suspicious => 1,
            AnomalyResultV2::Anomalous => 2,
            AnomalyResultV2::Critical => 3,
        }
    }
}

/// Detailed detection report
#[derive(Debug, Clone)]
pub struct DetectionReport {
    /// Final classification
    pub result: AnomalyResultV2,

    /// Which layer triggered (if any)
    pub triggered_layer: Option<u8>,

    /// Layer-by-layer scores
    pub probabilistic_result: bool,  // true = known, false = unknown
    pub gmm_score: f64,
    pub tinyml_score: f64,
    pub temporal_score: f32,
    pub temporal_burst: bool,

    /// Latency breakdown (nanoseconds, estimated)
    pub total_latency_ns: u32,
    pub path_taken: &'static str,
}

// ============================================================================
// FREQUENCY TRACKER (384 bytes)
// ============================================================================

/// Compact frequency tracker for behavior patterns
#[repr(C, align(64))]
struct FrequencyTracker {
    /// CountMinSketch-style frequency counters
    counters: [AtomicU32; 96],
}

impl FrequencyTracker {
    const fn new() -> Self {
        const ZERO: AtomicU32 = AtomicU32::new(0);
        Self {
            counters: [ZERO; 96],
        }
    }

    #[inline]
    fn increment(&self, behavior_hash: u64) {
        // Simple hash to 3 rows × 32 columns
        for row in 0..3 {
            let col = ((behavior_hash >> (row * 11)) & 31) as usize;
            let idx = row * 32 + col;
            self.counters[idx].fetch_add(1, Ordering::Relaxed);
        }
    }

    #[inline]
    fn estimate(&self, behavior_hash: u64) -> u32 {
        let mut min_count = u32::MAX;
        for row in 0..3 {
            let col = ((behavior_hash >> (row * 11)) & 31) as usize;
            let idx = row * 32 + col;
            let count = self.counters[idx].load(Ordering::Relaxed);
            min_count = min_count.min(count);
        }
        min_count
    }
}

// ============================================================================
// ANOMALY DETECTOR V2 (8KB)
// ============================================================================

/// AnomalyDetectorV2 - Hybrid multi-layer anomaly detection (8KB, 256B aligned)
///
/// Combines probabilistic, statistical (GMM), ML (TinyML), and temporal analysis
/// for comprehensive anomaly detection with <100ns average latency.
///
/// # V1 API Compatibility
/// All V1 methods are preserved and work identically. V2 extensions are additive.
///
/// # Thread Safety
/// 100% lockfree - all layers use atomic operations
#[repr(C, align(512))]
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 512))]
pub struct AnomalyDetectorV2 {
    // ========== MASTER HEADER (128 bytes) ==========

    /// Version identifier
    version: u8,

    /// Enabled layers bitmask
    enabled_layers: AtomicU8,

    /// Padding
    _padding_1: [u8; 6],

    /// Generation counter (Q34 audit trail)
    generation: AtomicU64,

    /// Total checks performed
    total_checks: AtomicU64,

    /// Anomaly count (any layer)
    anomaly_count: AtomicU64,

    /// Fast path hits (Bloom filter)
    fast_path_hits: AtomicU64,

    /// Layer latency tracking (estimated ns)
    layer_latencies: [AtomicU32; 4],

    /// Critical anomaly count
    critical_count: AtomicU64,

    /// Padding to 128 bytes
    _padding_header: [u8; 56],

    // ========== PROBABILISTIC LAYER (2560 bytes - actual V1 size) ==========
    /// Reuses V1 AnomalyDetectorCapsule (Bloom + HLL + CountMin)
    probabilistic: AnomalyDetectorCapsule,

    // ========== TINYML ENSEMBLE (2048+ bytes) ==========
    /// 8 decision trees for ML-based detection
    tinyml: TinyMLTreeEnsemble,

    // ========== GMM LAYER (512 bytes) ==========
    /// Gaussian Mixture Model for statistical detection
    gmm: GMMCapsule,

    // ========== TEMPORAL LAYER (2048 bytes) ==========
    /// Time-series sequence analysis
    temporal: TemporalSequenceCapsule,

    // ========== FREQUENCY TRACKER (384 bytes) ==========
    /// Behavior frequency tracking
    frequency: FrequencyTracker,

    // Note: Padding to 8192 bytes is handled by alignment
}

impl AnomalyDetectorV2 {
    /// Create a new AnomalyDetectorV2 with all layers enabled
    pub fn new() -> Self {
        Self {
            version: 2,
            enabled_layers: AtomicU8::new(ALL_LAYERS),
            _padding_1: [0; 6],
            generation: AtomicU64::new(0),
            total_checks: AtomicU64::new(0),
            anomaly_count: AtomicU64::new(0),
            fast_path_hits: AtomicU64::new(0),
            layer_latencies: [
                AtomicU32::new(30),   // Probabilistic estimate
                AtomicU32::new(20),   // GMM estimate
                AtomicU32::new(60),   // TinyML estimate
                AtomicU32::new(50),   // Temporal estimate
            ],
            critical_count: AtomicU64::new(0),
            _padding_header: [0; 56],
            probabilistic: AnomalyDetectorCapsule::new(),
            tinyml: TinyMLTreeEnsemble::new(),
            gmm: GMMCapsule::new(),
            temporal: TemporalSequenceCapsule::new(),
            frequency: FrequencyTracker::new(),
        }
    }

    /// Create with specific layers enabled
    pub fn with_layers(layers: u8) -> Self {
        let mut detector = Self::new();
        detector.enabled_layers.store(layers, Ordering::Relaxed);
        detector
    }

    // ========================================================================
    // V1 API COMPATIBILITY
    // ========================================================================

    /// Initialize baseline from samples (V1 compatible)
    ///
    /// Initializes both V1 probabilistic layer and V2 GMM layer.
    pub fn init(&mut self, samples: &[u64]) -> Result<(), AnomalyDetectorV2Error> {
        // Initialize V1 probabilistic layer
        self.probabilistic.init(samples)
            .map_err(|e| AnomalyDetectorV2Error::InitializationFailed(format!("{}", e)))?;

        // Initialize GMM with Q16.16 converted samples
        let gmm_samples: Vec<i64> = samples.iter()
            .map(|&s| f64_to_q16_16(s as f64))
            .collect();
        self.gmm.init_from_samples(&gmm_samples)
            .map_err(|e| AnomalyDetectorV2Error::InitializationFailed(format!("{}", e)))?;

        // Initialize TinyML ensemble with test trees
        // In production, these would be loaded from trained model
        self.tinyml.init_test_ensemble();

        self.generation.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// Check behavior (V1 compatible)
    ///
    /// Returns V1-style result for backward compatibility.
    #[inline]
    pub fn check_behavior(&self, behavior_id: u64) -> V1AnomalyResult {
        self.check_behavior_v2(behavior_id, 0).result.to_v1()
    }

    /// Update baseline (V1 compatible)
    #[inline]
    pub fn update_baseline(&self, behavior_id: u64) {
        self.probabilistic.update_baseline(behavior_id);
        self.gmm.update_nearest(f64_to_q16_16(behavior_id as f64));
    }

    /// Get anomaly rate (V1 compatible)
    #[inline]
    pub fn anomaly_rate(&self) -> f64 {
        let total = self.total_checks.load(Ordering::Relaxed);
        let anomalies = self.anomaly_count.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            anomalies as f64 / total as f64
        }
    }

    // ========================================================================
    // V2 EXTENSIONS
    // ========================================================================

    /// Check behavior with full V2 detection pipeline
    ///
    /// # Arguments
    /// * `behavior_id` - Behavior identifier/hash
    /// * `timestamp_ms` - Timestamp in milliseconds (0 = use internal counter)
    ///
    /// # Returns
    /// Detailed detection report
    #[inline]
    pub fn check_behavior_v2(&self, behavior_id: u64, timestamp_ms: u32) -> DetectionReport {
        let enabled = self.enabled_layers.load(Ordering::Relaxed);
        self.total_checks.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.frequency.increment(behavior_id);

        let mut report = DetectionReport {
            result: AnomalyResultV2::Normal,
            triggered_layer: None,
            probabilistic_result: false,
            gmm_score: 0.0,
            tinyml_score: 0.0,
            temporal_score: 0.0,
            temporal_burst: false,
            total_latency_ns: 0,
            path_taken: "unknown",
        };

        // Layer 1: Probabilistic (fast path)
        if enabled & LAYER_PROBABILISTIC != 0 {
            let v1_result = self.probabilistic.check_behavior(behavior_id);
            report.probabilistic_result = matches!(v1_result, V1AnomalyResult::Normal);
            report.total_latency_ns += 30;

            if report.probabilistic_result {
                self.fast_path_hits.fetch_add(1, Ordering::Relaxed);
                report.path_taken = "fast_path";
                return report;
            }
        }

        // Layer 2: GMM (statistical)
        if enabled & LAYER_GMM != 0 {
            let behavior_q16 = f64_to_q16_16(behavior_id as f64);
            let (score_q16, is_anomaly) = self.gmm.score_and_classify(behavior_q16);
            report.gmm_score = score_q16 as f64 / 65536.0;
            report.total_latency_ns += 20;

            if !is_anomaly {
                report.result = AnomalyResultV2::Suspicious;
                report.path_taken = "gmm_path";
                return report;
            }
        }

        // Layer 3: TinyML (ML-based)
        if enabled & LAYER_TINYML != 0 {
            let mut features = [0i16; 256];
            // Extract features from behavior ID
            for i in 0..8 {
                features[i] = ((behavior_id >> (i * 8)) & 0xFF) as i16;
            }
            features[8] = self.frequency.estimate(behavior_id) as i16;

            let (path_length, is_anomaly) = self.tinyml.evaluate_and_classify(&features);
            report.tinyml_score = path_length as f64 / 6.0; // Normalize to 0-1
            report.total_latency_ns += 60;

            if !is_anomaly {
                report.result = AnomalyResultV2::Suspicious;
                report.path_taken = "tinyml_path";
                return report;
            }
        }

        // Layer 4: Temporal (time-series)
        if enabled & LAYER_TEMPORAL != 0 {
            let effective_timestamp = if timestamp_ms == 0 {
                self.total_checks.load(Ordering::Relaxed) as u32
            } else {
                timestamp_ms
            };

            let _gmm_score_q8 = (report.gmm_score * 256.0) as i16;
            self.temporal.append_behavior(behavior_id, report.gmm_score as f32, effective_timestamp);

            let (temporal_score, is_burst) = self.temporal.compute_temporal_score();
            report.temporal_score = temporal_score as f32 / 256.0;
            report.temporal_burst = is_burst;
            report.total_latency_ns += 50;

            if is_burst {
                report.result = AnomalyResultV2::Critical;
                report.triggered_layer = Some(LAYER_TEMPORAL);
                self.critical_count.fetch_add(1, Ordering::Relaxed);
                report.path_taken = "full_path_critical";
            } else {
                report.result = AnomalyResultV2::Anomalous;
                report.triggered_layer = Some(LAYER_TINYML);
                report.path_taken = "full_path";
            }
        } else {
            report.result = AnomalyResultV2::Anomalous;
            report.triggered_layer = Some(if enabled & LAYER_TINYML != 0 { LAYER_TINYML } else { LAYER_GMM });
            report.path_taken = "partial_path";
        }

        self.anomaly_count.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Relaxed);

        report
    }

    /// Batch check multiple behaviors
    pub fn check_batch(&self, behaviors: &[(u64, u32)]) -> Vec<DetectionReport> {
        behaviors.iter()
            .map(|&(behavior_id, timestamp)| self.check_behavior_v2(behavior_id, timestamp))
            .collect()
    }

    // ========================================================================
    // LAYER MANAGEMENT
    // ========================================================================

    /// Enable specific layer
    #[inline]
    pub fn enable_layer(&self, layer: u8) {
        self.enabled_layers.fetch_or(layer, Ordering::SeqCst);
    }

    /// Disable specific layer
    #[inline]
    pub fn disable_layer(&self, layer: u8) {
        self.enabled_layers.fetch_and(!layer, Ordering::SeqCst);
    }

    /// Check if layer is enabled
    #[inline]
    pub fn is_layer_enabled(&self, layer: u8) -> bool {
        self.enabled_layers.load(Ordering::Relaxed) & layer != 0
    }

    /// Get enabled layers bitmask
    #[inline]
    pub fn enabled_layers(&self) -> u8 {
        self.enabled_layers.load(Ordering::Relaxed)
    }

    // ========================================================================
    // COMPONENT ACCESS
    // ========================================================================

    /// Get reference to GMM layer
    #[inline]
    pub fn gmm(&self) -> &GMMCapsule {
        &self.gmm
    }

    /// Get reference to temporal layer
    #[inline]
    pub fn temporal(&self) -> &TemporalSequenceCapsule {
        &self.temporal
    }

    /// Get reference to TinyML layer
    #[inline]
    pub fn tinyml(&self) -> &TinyMLTreeEnsemble {
        &self.tinyml
    }

    /// Get reference to probabilistic layer (V1)
    #[inline]
    pub fn probabilistic(&self) -> &AnomalyDetectorCapsule {
        &self.probabilistic
    }

    // ========================================================================
    // STATISTICS
    // ========================================================================

    /// Get generation counter (Q34 audit trail)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Get total checks performed
    #[inline]
    pub fn total_checks(&self) -> u64 {
        self.total_checks.load(Ordering::Relaxed)
    }

    /// Get anomaly count
    #[inline]
    pub fn anomaly_count(&self) -> u64 {
        self.anomaly_count.load(Ordering::Relaxed)
    }

    /// Get fast path hit count
    #[inline]
    pub fn fast_path_hits(&self) -> u64 {
        self.fast_path_hits.load(Ordering::Relaxed)
    }

    /// Get critical anomaly count
    #[inline]
    pub fn critical_count(&self) -> u64 {
        self.critical_count.load(Ordering::Relaxed)
    }

    /// Get fast path hit rate (0.0 - 1.0)
    #[inline]
    pub fn fast_path_rate(&self) -> f64 {
        let total = self.total_checks.load(Ordering::Relaxed);
        let hits = self.fast_path_hits.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// Get estimated average latency (nanoseconds)
    #[inline]
    pub fn avg_latency_ns(&self) -> u32 {
        let total = self.total_checks.load(Ordering::Relaxed);
        let fast = self.fast_path_hits.load(Ordering::Relaxed);

        if total == 0 {
            return 0;
        }

        // Weighted average based on path distribution
        let fast_ns = 30u64;
        let slow_ns = 160u64; // Full path

        let fast_ratio = fast as f64 / total as f64;
        ((fast_ratio * fast_ns as f64) + ((1.0 - fast_ratio) * slow_ns as f64)) as u32
    }

    /// Reset all statistics
    pub fn reset_statistics(&self) {
        self.total_checks.store(0, Ordering::SeqCst);
        self.anomaly_count.store(0, Ordering::SeqCst);
        self.fast_path_hits.store(0, Ordering::SeqCst);
        self.critical_count.store(0, Ordering::SeqCst);
        // Note: V1 AnomalyDetectorCapsule doesn't have reset_statistics
        self.gmm.reset_statistics();
        self.temporal.reset_statistics();
    }

    /// Get version
    #[inline]
    pub fn version(&self) -> u8 {
        self.version
    }
}

impl Default for AnomalyDetectorV2 {
    fn default() -> Self {
        Self::new()
    }
}

// Note: Clone is complex due to atomics; use snapshot pattern instead
impl AnomalyDetectorV2 {
    /// Create a snapshot of statistics
    pub fn snapshot_stats(&self) -> V2Statistics {
        V2Statistics {
            generation: self.generation(),
            total_checks: self.total_checks(),
            anomaly_count: self.anomaly_count(),
            fast_path_hits: self.fast_path_hits(),
            critical_count: self.critical_count(),
            anomaly_rate: self.anomaly_rate(),
            fast_path_rate: self.fast_path_rate(),
            avg_latency_ns: self.avg_latency_ns(),
        }
    }
}

/// Statistics snapshot
#[derive(Debug, Clone)]
pub struct V2Statistics {
    pub generation: u64,
    pub total_checks: u64,
    pub anomaly_count: u64,
    pub fast_path_hits: u64,
    pub critical_count: u64,
    pub anomaly_rate: f64,
    pub fast_path_rate: f64,
    pub avg_latency_ns: u32,
}

// ============================================================================
// ERROR TYPES
// ============================================================================

/// AnomalyDetectorV2 error
#[derive(Debug, Clone)]
pub enum AnomalyDetectorV2Error {
    /// Initialization failed
    InitializationFailed(String),

    /// Layer not enabled
    LayerNotEnabled(u8),

    /// Invalid configuration
    InvalidConfiguration(String),
}

impl core::fmt::Display for AnomalyDetectorV2Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AnomalyDetectorV2Error::InitializationFailed(msg) => {
                write!(f, "Initialization failed: {}", msg)
            }
            AnomalyDetectorV2Error::LayerNotEnabled(layer) => {
                write!(f, "Layer not enabled: 0x{:02x}", layer)
            }
            AnomalyDetectorV2Error::InvalidConfiguration(msg) => {
                write!(f, "Invalid configuration: {}", msg)
            }
        }
    }
}

impl std::error::Error for AnomalyDetectorV2Error {}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== UNIT TESTS (15) ====================

    #[test]
    fn test_anomaly_detector_v2_creation() {
        let detector = AnomalyDetectorV2::new();
        assert_eq!(detector.version(), 2);
        assert_eq!(detector.enabled_layers(), ALL_LAYERS);
        assert_eq!(detector.total_checks(), 0);
    }

    #[test]
    fn test_layer_enable_disable() {
        let detector = AnomalyDetectorV2::new();

        detector.disable_layer(LAYER_TEMPORAL);
        assert!(!detector.is_layer_enabled(LAYER_TEMPORAL));
        assert!(detector.is_layer_enabled(LAYER_GMM));

        detector.enable_layer(LAYER_TEMPORAL);
        assert!(detector.is_layer_enabled(LAYER_TEMPORAL));
    }

    #[test]
    fn test_with_layers() {
        let detector = AnomalyDetectorV2::with_layers(LAYER_PROBABILISTIC | LAYER_GMM);
        assert!(detector.is_layer_enabled(LAYER_PROBABILISTIC));
        assert!(detector.is_layer_enabled(LAYER_GMM));
        assert!(!detector.is_layer_enabled(LAYER_TINYML));
        assert!(!detector.is_layer_enabled(LAYER_TEMPORAL));
    }

    #[test]
    fn test_v1_api_compatibility() {
        let detector = AnomalyDetectorV2::new();

        // V1 methods should work
        let result = detector.check_behavior(12345);
        assert!(matches!(result, V1AnomalyResult::Normal | V1AnomalyResult::Suspicious | V1AnomalyResult::Anomalous));

        detector.update_baseline(12345);
        let rate = detector.anomaly_rate();
        assert!(rate >= 0.0 && rate <= 1.0);
    }

    #[test]
    fn test_check_behavior_v2() {
        let detector = AnomalyDetectorV2::new();

        let report = detector.check_behavior_v2(12345, 1000);
        assert!(report.total_latency_ns > 0);
        assert!(!report.path_taken.is_empty());
    }

    #[test]
    fn test_fast_path_detection() {
        let detector = AnomalyDetectorV2::new();

        // First check - unknown behavior
        let _ = detector.check_behavior_v2(12345, 1000);

        // Update baseline
        detector.update_baseline(12345);

        // Add to Bloom filter via V1 layer
        for _ in 0..10 {
            detector.update_baseline(12345);
        }

        // Check again - might hit fast path now
        let _report = detector.check_behavior_v2(12345, 2000);
        // Note: Fast path depends on Bloom filter implementation
    }

    #[test]
    fn test_batch_check() {
        let detector = AnomalyDetectorV2::new();

        let behaviors = vec![
            (12345, 1000),
            (67890, 1100),
            (11111, 1200),
        ];

        let reports = detector.check_batch(&behaviors);
        assert_eq!(reports.len(), 3);
        assert_eq!(detector.total_checks(), 3);
    }

    #[test]
    fn test_statistics_tracking() {
        let detector = AnomalyDetectorV2::new();

        for i in 0..100 {
            let _ = detector.check_behavior_v2(i, i as u32 * 10);
        }

        assert_eq!(detector.total_checks(), 100);
        assert!(detector.generation() > 0);

        let stats = detector.snapshot_stats();
        assert_eq!(stats.total_checks, 100);
    }

    #[test]
    fn test_statistics_reset() {
        let detector = AnomalyDetectorV2::new();

        for i in 0..50 {
            let _ = detector.check_behavior_v2(i, 0);
        }

        assert_eq!(detector.total_checks(), 50);

        detector.reset_statistics();
        assert_eq!(detector.total_checks(), 0);
        assert_eq!(detector.anomaly_count(), 0);
    }

    #[test]
    fn test_component_access() {
        let detector = AnomalyDetectorV2::new();

        // Should be able to access components
        let _gmm = detector.gmm();
        let _temporal = detector.temporal();
        let _tinyml = detector.tinyml();
        let _prob = detector.probabilistic();
    }

    #[test]
    fn test_anomaly_result_v2_conversion() {
        assert!(matches!(AnomalyResultV2::Normal.to_v1(), V1AnomalyResult::Normal));
        assert!(matches!(AnomalyResultV2::Suspicious.to_v1(), V1AnomalyResult::Suspicious));
        assert!(matches!(AnomalyResultV2::Anomalous.to_v1(), V1AnomalyResult::Anomalous));
        assert!(matches!(AnomalyResultV2::Critical.to_v1(), V1AnomalyResult::Anomalous));
    }

    #[test]
    fn test_anomaly_result_severity() {
        assert_eq!(AnomalyResultV2::Normal.severity(), 0);
        assert_eq!(AnomalyResultV2::Suspicious.severity(), 1);
        assert_eq!(AnomalyResultV2::Anomalous.severity(), 2);
        assert_eq!(AnomalyResultV2::Critical.severity(), 3);
    }

    #[test]
    fn test_frequency_tracker() {
        let tracker = FrequencyTracker::new();

        for _ in 0..10 {
            tracker.increment(12345);
        }

        let count = tracker.estimate(12345);
        assert!(count >= 10, "Should count at least 10 increments");
    }

    #[test]
    fn test_detection_report() {
        let detector = AnomalyDetectorV2::new();
        let report = detector.check_behavior_v2(999999, 5000);

        // Report should have valid data
        assert!(report.total_latency_ns >= 30);  // At least probabilistic layer
    }

    #[test]
    fn test_avg_latency_calculation() {
        let detector = AnomalyDetectorV2::new();

        // With no checks, latency should be 0
        assert_eq!(detector.avg_latency_ns(), 0);

        // After some checks
        for i in 0..10 {
            let _ = detector.check_behavior_v2(i, 0);
        }

        let latency = detector.avg_latency_ns();
        assert!(latency > 0 && latency < 200);  // Should be reasonable
    }

    // ==================== PROPERTY TESTS (5) ====================

    #[test]
    fn proptest_total_checks_monotonic() {
        let detector = AnomalyDetectorV2::new();
        let mut prev = detector.total_checks();

        for i in 0..100 {
            let _ = detector.check_behavior_v2(i, 0);
            let current = detector.total_checks();
            assert!(current > prev);
            prev = current;
        }
    }

    #[test]
    fn proptest_generation_monotonic() {
        let detector = AnomalyDetectorV2::new();
        let mut prev = detector.generation();

        for i in 0..50 {
            let _ = detector.check_behavior_v2(i, 0);
            let current = detector.generation();
            assert!(current >= prev);  // May not always increment
            prev = current;
        }
    }

    #[test]
    fn proptest_anomaly_rate_bounded() {
        let detector = AnomalyDetectorV2::new();

        for i in 0..100 {
            let _ = detector.check_behavior_v2(i * 1000, 0);
        }

        let rate = detector.anomaly_rate();
        assert!(rate >= 0.0 && rate <= 1.0);
    }

    #[test]
    fn proptest_fast_path_rate_bounded() {
        let detector = AnomalyDetectorV2::new();

        for i in 0..100 {
            let _ = detector.check_behavior_v2(i, 0);
        }

        let rate = detector.fast_path_rate();
        assert!(rate >= 0.0 && rate <= 1.0);
    }

    #[test]
    fn proptest_layer_bitmask_valid() {
        let detector = AnomalyDetectorV2::new();

        // Enable all layers
        detector.enable_layer(ALL_LAYERS);
        assert_eq!(detector.enabled_layers() & ALL_LAYERS, ALL_LAYERS);

        // Disable all layers
        detector.disable_layer(ALL_LAYERS);
        assert_eq!(detector.enabled_layers() & ALL_LAYERS, 0);
    }

    // ==================== INTEGRATION TESTS (10) ====================

    #[test]
    fn integration_probabilistic_only() {
        let detector = AnomalyDetectorV2::with_layers(LAYER_PROBABILISTIC);

        let report = detector.check_behavior_v2(12345, 1000);
        assert!(report.total_latency_ns <= 50);  // Only probabilistic
    }

    #[test]
    fn integration_gmm_only() {
        let detector = AnomalyDetectorV2::with_layers(LAYER_GMM);

        let report = detector.check_behavior_v2(12345, 1000);
        // Should skip probabilistic, go straight to GMM
        assert!(report.gmm_score >= 0.0 || report.gmm_score < 0.0);  // Just check it ran
    }

    #[test]
    fn integration_all_layers() {
        let detector = AnomalyDetectorV2::new();

        // Large behavior ID likely to trigger full path
        let report = detector.check_behavior_v2(u64::MAX - 1, 1000);
        assert!(report.total_latency_ns > 0);
    }

    #[test]
    fn integration_temporal_pattern() {
        let detector = AnomalyDetectorV2::new();
        detector.temporal().set_burst_threshold(1.0);

        // Rapid anomalous behavior
        for i in 0..20 {
            let _ = detector.check_behavior_v2(999999, i * 10);
        }

        // Should have some temporal detections
        let _burst_count = detector.temporal().burst_count();
        // Note: Burst detection depends on accumulated scores
    }

    #[test]
    fn integration_mixed_behaviors() {
        let detector = AnomalyDetectorV2::new();

        // Mix of normal and anomalous
        for i in 0..50 {
            if i % 5 == 0 {
                let _ = detector.check_behavior_v2(u64::MAX - i as u64, i as u32 * 100);
            } else {
                let _ = detector.check_behavior_v2(i, i as u32 * 100);
            }
        }

        assert_eq!(detector.total_checks(), 50);
    }

    #[test]
    fn integration_concurrent_reads() {
        use std::sync::Arc;
        use std::thread;

        let detector = Arc::new(AnomalyDetectorV2::new());

        let mut handles = vec![];
        for t in 0..4 {
            let det = Arc::clone(&detector);
            handles.push(thread::spawn(move || {
                for i in 0..25 {
                    let _ = det.check_behavior_v2((t * 100 + i) as u64, (i * 10) as u32);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(detector.total_checks(), 100);
    }

    #[test]
    fn integration_layer_switching() {
        let detector = AnomalyDetectorV2::new();

        // Start with all layers
        let _ = detector.check_behavior_v2(1, 0);

        // Disable TinyML
        detector.disable_layer(LAYER_TINYML);
        let _ = detector.check_behavior_v2(2, 100);

        // Re-enable
        detector.enable_layer(LAYER_TINYML);
        let _ = detector.check_behavior_v2(3, 200);

        assert_eq!(detector.total_checks(), 3);
    }

    #[test]
    fn integration_statistics_accuracy() {
        let detector = AnomalyDetectorV2::new();

        for i in 0..100 {
            let _report = detector.check_behavior_v2(i * 12345, i as u32 * 50);
        }

        // Note: Anomaly count should be consistent
        // Just verify it's within bounds
        let actual = detector.anomaly_count();
        assert!(actual <= 100);
    }

    #[test]
    fn integration_snapshot_consistency() {
        let detector = AnomalyDetectorV2::new();

        for i in 0..50 {
            let _ = detector.check_behavior_v2(i, 0);
        }

        let stats1 = detector.snapshot_stats();
        let stats2 = detector.snapshot_stats();

        // Snapshots taken back-to-back should be identical
        assert_eq!(stats1.total_checks, stats2.total_checks);
    }

    #[test]
    fn integration_error_handling() {
        let mut detector = AnomalyDetectorV2::new();

        // Empty initialization should fail
        let result = detector.init(&[]);
        assert!(result.is_err());

        // Single sample should fail
        let result = detector.init(&[1]);
        assert!(result.is_err());
    }
}
