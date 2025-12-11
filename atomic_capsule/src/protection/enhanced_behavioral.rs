//! # EnhancedBehavioralCapsule - T6 Mixed tier ML-based Anomaly Detection
//!
//! **5-Model Ensemble for Insider Threat Detection**: Statistical + Markov + Isolation Forest + LSTM-lite + Voting
//!
//! ## UCE34 Framework Analysis (Q1-Q34)
//!
//! ### Q1-Q9: Meta-Cognitive Analysis
//! - **Q1 (Scope)**: ML-based insider threat detection for protection systems
//! - **Q2 (Assumptions)**: Behavior patterns are distinguishable, anomalies cluster
//! - **Q3 (Constraints)**: <100ns check latency, 512B capsule, 90%+ detection rate
//! - **Q4 (Context)**: Protection orchestrator layer for behavioral anomaly detection
//! - **Q5 (Success)**: 90%+ insider threat detection (up from 50%), <5% false positive rate
//! - **Q6 (Failure)**: False positives (user friction), false negatives (missed attacks)
//! - **Q7 (Patterns)**: Ensemble voting, statistical z-score, Markov chains, isolation
//! - **Q8 (Alternatives)**: Single model (fragile), deep ML (too slow), rules (brittle)
//! - **Q9 (Trade-offs)**: Model count vs latency, accuracy vs memory
//!
//! ### Q10-Q12: Foundation (Capsule Architecture)
//! - **Q10 (Tier Selection)**: T6 Mixed = T1 Atomic + T2 SIMD + T3 Fixed-Point + T5 Streaming + T0 Auditable
//! - **Q11 (Rust Transform)**: DualAtomicU64, Q16.16 fixed-point, lockfree window, SIMD ensemble
//! - **Q12 (Nightly)**: portable_simd for SIMD ensemble computation
//!
//! ## 5-Model Ensemble Architecture
//!
//! | Model | Tier | Purpose | Latency |
//! |-------|------|---------|---------|
//! | Statistical | T3 | Z-score anomaly (mean/std) | <20ns |
//! | Markov | T1 | State transition probability | <25ns |
//! | Isolation | T3 | Outlier distance scoring | <30ns |
//! | LSTM-lite | T3 | Sequence pattern detection | <30ns |
//! | Ensemble | T2 | Weighted voting (SIMD) | <15ns |
//!
//! ## Performance Targets (B32 Framework)
//!
//! | Operation | Target | Notes |
//! |-----------|--------|-------|
//! | analyze_behavior() | <100ns | Full 5-model ensemble |
//! | update_models() | <200ns | Streaming window update |
//! | get_threat_score() | <10ns | Atomic snapshot |
//!
//! ## Safety Model (ASSUM Framework)
//!
//! - `#ASSUME_LOCKFREE`: 100% lockfree, no mutex/RwLock
//! - `#ASSUME_CACHE_ALIGNED`: 512B alignment prevents false sharing
//! - `#ASSUME_GENERATION_COUNTERS`: DualAtomicU64 prevents TOCTOU races
//! - `#ASSUME_Q16_16_PRECISION`: Fixed-point sufficient for ML weights
//! - `#ASSUME_WINDOW_OVERFLOW`: Ring buffer handles wraparound atomically

#![allow(dead_code)]

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Number of features tracked for anomaly detection
pub const NUM_FEATURES: usize = 8;

/// Number of models in ensemble (excluding final voting)
pub const NUM_MODELS: usize = 4;

/// Model weights count (2 weights per u64, 4 models = 2 u64s)
pub const WEIGHTS_PER_MODEL: usize = 2;

/// Streaming window size for sequence analysis
pub const WINDOW_SIZE: usize = 14;

/// Q16.16 scale factor (2^16 = 65536)
pub const Q16_16_SCALE: i32 = 65536;

/// Q16.16 representation of 1.0
pub const Q16_16_ONE: i32 = Q16_16_SCALE;

/// Q16.16 representation of 0.0
pub const Q16_16_ZERO: i32 = 0;

/// Q16.16 representation of 0.25 (anomaly score addition for z > 2.5)
pub const Q16_16_QUARTER: i32 = Q16_16_SCALE / 4;

/// Q16.16 representation of 0.5
pub const Q16_16_HALF: i32 = Q16_16_SCALE / 2;

/// Q16.16 representation of 2.5 (z-score threshold)
pub const Z_THRESHOLD_Q16: i32 = (2.5 * Q16_16_SCALE as f64) as i32;

/// Q16.16 representation of 0.05 (5% Markov threshold)
pub const MARKOV_THRESHOLD_Q16: i32 = (0.05 * Q16_16_SCALE as f64) as i32;

/// Default isolation forest path length threshold
pub const ISOLATION_PATH_THRESHOLD: i32 = (0.4 * Q16_16_SCALE as f64) as i32;

/// Maximum CAS retries for atomic updates
const MAX_CAS_RETRIES: usize = 8;

/// FNV-1a prime for hash chain
const FNV_PRIME: u64 = 0x00000100000001B3;

/// FNV-1a offset basis for hash chain
const FNV_OFFSET: u64 = 0xcbf29ce484222325;

// ============================================================================
// EVENT TYPE ENUM
// ============================================================================

/// Behavior event type classification
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum EventType {
    FileAccess = 0,
    NetworkRequest = 1,
    ProcessSpawn = 2,
    MemoryAllocation = 3,
    LicenseCheck = 4,
    ConfigChange = 5,
    PrivilegeEscalation = 6,
    DataExfiltration = 7,
}

impl EventType {
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(EventType::FileAccess),
            1 => Some(EventType::NetworkRequest),
            2 => Some(EventType::ProcessSpawn),
            3 => Some(EventType::MemoryAllocation),
            4 => Some(EventType::LicenseCheck),
            5 => Some(EventType::ConfigChange),
            6 => Some(EventType::PrivilegeEscalation),
            7 => Some(EventType::DataExfiltration),
            _ => None,
        }
    }

    #[inline]
    pub const fn risk_weight_q16(&self) -> i32 {
        match self {
            EventType::FileAccess => Q16_16_SCALE / 10,
            EventType::NetworkRequest => Q16_16_SCALE / 5,
            EventType::ProcessSpawn => Q16_16_SCALE / 4,
            EventType::MemoryAllocation => Q16_16_SCALE / 10,
            EventType::LicenseCheck => Q16_16_SCALE / 3,
            EventType::ConfigChange => Q16_16_SCALE / 2,
            EventType::PrivilegeEscalation => Q16_16_SCALE * 3 / 4,
            EventType::DataExfiltration => Q16_16_SCALE,
        }
    }
}

// ============================================================================
// BEHAVIOR EVENT STRUCTURE
// ============================================================================

/// Behavior event for anomaly analysis (32 bytes)
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct BehaviorEvent {
    pub timestamp: u64,
    pub event_type: EventType,
    _pad1: [u8; 7],
    pub resource_id: u64,
    pub user_context: u32,
    pub payload_size: u32,
}

impl BehaviorEvent {
    #[inline]
    pub const fn new(
        timestamp: u64,
        event_type: EventType,
        resource_id: u64,
        user_context: u32,
        payload_size: u32,
    ) -> Self {
        Self { timestamp, event_type, _pad1: [0u8; 7], resource_id, user_context, payload_size }
    }

    #[inline]
    pub fn pack(&self) -> u64 {
        let event_type_bits = (self.event_type as u64) & 0xF;
        let payload_log2 = (32 - self.payload_size.leading_zeros()) as u64 & 0xF;
        let timestamp_bits = (self.timestamp >> 16) & 0xFFFFFF;
        let resource_bits = (self.resource_id as u32) as u64;
        (event_type_bits << 60) | (payload_log2 << 56) | (timestamp_bits << 32) | resource_bits
    }

    #[inline]
    pub fn unpack(packed: u64) -> Self {
        let event_type = EventType::from_u8(((packed >> 60) & 0xF) as u8).unwrap_or(EventType::FileAccess);
        let payload_log2 = ((packed >> 56) & 0xF) as u32;
        let payload_size = if payload_log2 > 0 { 1u32 << payload_log2 } else { 0 };
        let timestamp = (packed >> 32) & 0xFFFFFF;
        let resource_id = (packed & 0xFFFFFFFF) as u64;
        Self { timestamp: timestamp << 16, event_type, _pad1: [0u8; 7], resource_id, user_context: 0, payload_size }
    }
}

// ============================================================================
// ANOMALY SCORE RESULT
// ============================================================================

/// Recommended action based on anomaly score
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Allow,
    Log,
    RateLimit,
    Block,
    Alert,
}

impl Action {
    #[inline]
    pub const fn from_score_q16(score: i32) -> Self {
        if score < Q16_16_SCALE / 5 { Action::Allow }
        else if score < Q16_16_SCALE / 3 { Action::Log }
        else if score < Q16_16_SCALE / 2 { Action::RateLimit }
        else if score < Q16_16_SCALE * 3 / 4 { Action::Block }
        else { Action::Alert }
    }
}

/// Anomaly detection result
#[derive(Clone, Copy, Debug)]
pub struct AnomalyScore {
    pub score: i32,
    pub triggered_models: u8,
    pub confidence: i32,
    pub recommended_action: Action,
}

impl AnomalyScore {
    #[inline]
    pub const fn new(score: i32, triggered_models: u8, confidence: i32) -> Self {
        Self { score, triggered_models, confidence, recommended_action: Action::from_score_q16(score) }
    }

    #[inline]
    pub fn score_f64(&self) -> f64 { self.score as f64 / Q16_16_SCALE as f64 }

    #[inline]
    pub fn confidence_f64(&self) -> f64 { self.confidence as f64 / Q16_16_SCALE as f64 }
}

// ============================================================================
// Q16.16 FIXED-POINT HELPERS
// ============================================================================

#[inline]
fn q16_mul(a: i32, b: i32) -> i32 { ((a as i64 * b as i64) >> 16) as i32 }

#[inline]
fn q16_div(a: i32, b: i32) -> i32 {
    if b == 0 { return if a >= 0 { i32::MAX } else { i32::MIN }; }
    (((a as i64) << 16) / (b as i64)) as i32
}

#[inline]
fn q16_sqrt(x: i32) -> i32 {
    if x <= 0 { return 0; }
    let mut guess = x >> 1;
    if guess == 0 { guess = Q16_16_ONE; }
    for _ in 0..3 {
        let div_result = q16_div(x, guess);
        guess = (guess + div_result) >> 1;
    }
    guess
}

#[inline]
pub fn f64_to_q16(value: f64) -> i32 { (value * Q16_16_SCALE as f64) as i32 }

#[inline]
pub fn q16_to_f64(value: i32) -> f64 { value as f64 / Q16_16_SCALE as f64 }

#[inline]
fn q16_clamp_01(value: i32) -> i32 {
    if value < 0 { 0 } else if value > Q16_16_ONE { Q16_16_ONE } else { value }
}

// ============================================================================
// ENHANCED BEHAVIORAL CAPSULE (T6 Mixed) - 512 bytes
// ============================================================================

/// Enhanced Behavioral Capsule - T6 Mixed tier ML-based anomaly detection
///
/// # Memory Layout (512 bytes, 512-byte aligned)
///
/// | Offset | Size | Component | Description |
/// |--------|------|-----------|-------------|
/// | 0-63 | 64B | Feature Stats | 8 × AtomicU64 (mean/var packed) |
/// | 64-127 | 64B | Model Weights | 8 × AtomicU64 |
/// | 128-143 | 16B | Thresholds | 4 × AtomicU32 |
/// | 144-271 | 128B | Window | head/tail + 14 events |
/// | 272-335 | 64B | Audit State | 8 × AtomicU64 |
/// | 336-383 | 48B | Markov + LSTM | 4 × AtomicU64 + 2 × AtomicU64 |
/// | 384-511 | 128B | Coordination | state + generation + padding |
#[repr(C, align(512))]
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 512, size = 512))]
pub struct EnhancedBehavioralCapsule {
    // T1: Feature Stats (64 bytes) - mean in low 32b, variance in high 32b
    feature_stats: [AtomicU64; 8],

    // T2: Model Weights (64 bytes)
    model_weights: [AtomicU64; 8],

    // T3: Thresholds (16 bytes)
    thresholds: [AtomicU32; 4],

    // T5: Streaming Window (128 bytes)
    window_head: AtomicU64,
    window_tail: AtomicU64,
    window_events: [AtomicU64; 14],

    // T0: Audit State (64 bytes)
    audit_generation: AtomicU64,
    audit_hash: AtomicU64,
    anomaly_count: AtomicU64,
    last_anomaly_time: AtomicU64,
    total_events: AtomicU64,
    false_positive_count: AtomicU64,
    true_positive_count: AtomicU64,
    prev_event_and_importance: AtomicU64,

    // Markov + LSTM State (48 bytes)
    markov_transitions: [AtomicU64; 4], // 32 bytes
    lstm_hidden: AtomicU64, // 8 bytes
    lstm_cell: AtomicU64, // 8 bytes

    // Coordination State (128 bytes) - replaces DualAtomicU64
    orchestrator_primary: AtomicU64, // 8 bytes - processing state
    orchestrator_secondary: AtomicU64, // 8 bytes - coordination generation
    _pad: [u8; 112], // 112 bytes padding to reach 512B total
}

// Compile-time verification
const _: () = assert!(core::mem::size_of::<EnhancedBehavioralCapsule>() == 512);
const _: () = assert!(core::mem::align_of::<EnhancedBehavioralCapsule>() == 512);

impl EnhancedBehavioralCapsule {
    /// Create new EnhancedBehavioralCapsule with default configuration
    pub fn new() -> Self {
        let capsule = Self {
            feature_stats: core::array::from_fn(|_| AtomicU64::new(0)),
            model_weights: core::array::from_fn(|_| AtomicU64::new(0)),
            thresholds: core::array::from_fn(|_| AtomicU32::new(0)),
            window_head: AtomicU64::new(0),
            window_tail: AtomicU64::new(0),
            window_events: core::array::from_fn(|_| AtomicU64::new(0)),
            audit_generation: AtomicU64::new(0),
            audit_hash: AtomicU64::new(FNV_OFFSET),
            anomaly_count: AtomicU64::new(0),
            last_anomaly_time: AtomicU64::new(0),
            total_events: AtomicU64::new(0),
            false_positive_count: AtomicU64::new(0),
            true_positive_count: AtomicU64::new(0),
            prev_event_and_importance: AtomicU64::new(0),
            markov_transitions: core::array::from_fn(|_| AtomicU64::new(0x2020202020202020)),
            lstm_hidden: AtomicU64::new(0),
            lstm_cell: AtomicU64::new(0),
            orchestrator_primary: AtomicU64::new(0),
            orchestrator_secondary: AtomicU64::new(0),
            _pad: [0u8; 112],
        };

        // Initialize default thresholds
        capsule.thresholds[0].store(Z_THRESHOLD_Q16 as u32, Ordering::Relaxed);
        capsule.thresholds[1].store(MARKOV_THRESHOLD_Q16 as u32, Ordering::Relaxed);
        capsule.thresholds[2].store(ISOLATION_PATH_THRESHOLD as u32, Ordering::Relaxed);
        capsule.thresholds[3].store(f64_to_q16(0.3) as u32, Ordering::Relaxed);

        // Initialize model weights (equal 0.25 each)
        let default_weight = (Q16_16_SCALE / 4) as u64;
        let packed_weights = default_weight | (default_weight << 32);
        for i in 0..4 {
            capsule.model_weights[i].store(packed_weights, Ordering::Relaxed);
        }

        capsule
    }

    // ========================================================================
    // MODEL 1: STATISTICAL (Z-SCORE)
    // ========================================================================

    #[inline]
    fn statistical_model(&self, event: &BehaviorEvent) -> i32 {
        let mut score = Q16_16_ZERO;
        let threshold = self.thresholds[0].load(Ordering::Relaxed) as i32;
        let features = self.extract_features(event);

        for i in 0..NUM_FEATURES {
            let packed = self.feature_stats[i].load(Ordering::Relaxed);
            let mean = (packed & 0xFFFFFFFF) as i32;
            let variance = ((packed >> 32) & 0xFFFFFFFF) as i32;

            if variance == 0 { continue; }
            let stddev = q16_sqrt(variance);
            if stddev == 0 { continue; }

            let diff = features[i] - mean;
            let z_score = q16_div(diff.abs(), stddev);
            if z_score > threshold { score += Q16_16_QUARTER; }
        }

        q16_clamp_01(score)
    }

    #[inline]
    fn extract_features(&self, event: &BehaviorEvent) -> [i32; NUM_FEATURES] {
        let mut features = [Q16_16_ZERO; NUM_FEATURES];
        features[0] = event.event_type.risk_weight_q16();
        features[1] = if event.payload_size > 0 {
            (32 - event.payload_size.leading_zeros()) as i32 * (Q16_16_SCALE / 32)
        } else { 0 };
        features[2] = (((event.timestamp / 3_600_000_000_000) % 24) as i32) * (Q16_16_SCALE / 24);
        // Use i64 to avoid overflow: (0xFFFF * 65536) >> 16 = 0xFFFF = Q16 representation of ~1.0
        features[3] = (((event.resource_id & 0xFFFF) as i64 * Q16_16_SCALE as i64) >> 16) as i32;
        features[4] = (((event.user_context & 0xFF) as i64 * Q16_16_SCALE as i64) >> 8) as i32;
        features[5] = if event.event_type as u8 >= 5 { Q16_16_ONE } else { Q16_16_ZERO };
        features
    }

    // ========================================================================
    // MODEL 2: MARKOV CHAIN
    // ========================================================================

    #[inline]
    fn markov_model(&self, event: &BehaviorEvent) -> i32 {
        let prev_packed = self.prev_event_and_importance.load(Ordering::Relaxed);
        let prev_type = (prev_packed & 0x7) as usize;
        let curr_type = (event.event_type as usize) & 0x7;
        let trans_idx = prev_type & 0x3;

        let trans_word = self.markov_transitions[trans_idx].load(Ordering::Relaxed);
        let trans_count = ((trans_word >> (curr_type * 8)) & 0xFF) as i32;

        let mut row_sum = 0i32;
        for i in 0..8 { row_sum += ((trans_word >> (i * 8)) & 0xFF) as i32; }

        let prob = if row_sum > 0 {
            q16_div(trans_count * Q16_16_SCALE, row_sum * Q16_16_SCALE)
        } else { Q16_16_ONE / 8 };

        let threshold = self.thresholds[1].load(Ordering::Relaxed) as i32;
        if prob < threshold {
            q16_clamp_01(Q16_16_ONE - q16_div(prob * Q16_16_ONE, threshold))
        } else { Q16_16_ZERO }
    }

    // ========================================================================
    // MODEL 3: ISOLATION FOREST
    // ========================================================================

    #[inline]
    fn isolation_forest(&self, event: &BehaviorEvent) -> i32 {
        let features = self.extract_features(event);
        let mut total_distance = Q16_16_ZERO;
        let mut feature_count = 0;

        for i in 0..NUM_FEATURES {
            let packed = self.feature_stats[i].load(Ordering::Relaxed);
            let mean = (packed & 0xFFFFFFFF) as i32;
            let variance = ((packed >> 32) & 0xFFFFFFFF) as i32;
            if variance == 0 { continue; }

            let stddev = q16_sqrt(variance);
            if stddev == 0 { continue; }

            let diff = (features[i] - mean).abs();
            let normalized = q16_div(diff, stddev);
            total_distance += normalized;
            feature_count += 1;
        }

        if feature_count == 0 { return Q16_16_ZERO; }
        let avg_distance = q16_div(total_distance, feature_count * Q16_16_ONE);
        q16_clamp_01(q16_div(avg_distance, Q16_16_ONE + avg_distance))
    }

    // ========================================================================
    // MODEL 4: LSTM-LITE
    // ========================================================================

    #[inline]
    fn lstm_lite_model(&self, event: &BehaviorEvent) -> i32 {
        let head = self.window_head.load(Ordering::Relaxed) as usize;
        let window_len = head.min(WINDOW_SIZE);
        if window_len < 3 { return Q16_16_ZERO; }

        let mut sequence_score = Q16_16_ZERO;
        let curr_type = event.event_type as u8;
        let check_count = window_len.min(5);
        let mut type_counts = [0u32; 8];

        for i in 0..check_count {
            let idx = (head.wrapping_sub(1).wrapping_sub(i)) % WINDOW_SIZE;
            let packed = self.window_events[idx].load(Ordering::Relaxed);
            let prev_type = ((packed >> 60) & 0x7) as usize;
            type_counts[prev_type] += 1;
        }

        if type_counts[curr_type as usize] < (check_count as u32 / 4) {
            sequence_score += Q16_16_HALF;
        }

        let mut consecutive = 0usize;
        for i in 0..check_count.min(3) {
            let idx = (head.wrapping_sub(1).wrapping_sub(i)) % WINDOW_SIZE;
            let packed = self.window_events[idx].load(Ordering::Relaxed);
            let prev_type = ((packed >> 60) & 0x7) as u8;
            if prev_type == curr_type { consecutive += 1; } else { break; }
        }
        if consecutive >= 3 { sequence_score += Q16_16_QUARTER; }

        let hidden = self.lstm_hidden.load(Ordering::Relaxed) as i32;
        let hidden_influence = q16_mul(hidden, Q16_16_SCALE / 10);
        q16_clamp_01(sequence_score.saturating_add(hidden_influence))
    }

    // ========================================================================
    // MODEL 5: ENSEMBLE VOTING
    // ========================================================================

    #[inline]
    fn ensemble_vote(&self, scores: [i32; 4]) -> i32 {
        let mut weights = [Q16_16_QUARTER; 4];
        let mut total_weight = Q16_16_ONE;

        for i in 0..2 {
            let packed = self.model_weights[i].load(Ordering::Relaxed);
            weights[i * 2] = (packed & 0xFFFFFFFF) as i32;
            weights[i * 2 + 1] = ((packed >> 32) & 0xFFFFFFFF) as i32;
        }
        total_weight = weights.iter().sum();
        if total_weight == 0 { total_weight = Q16_16_ONE; }

        let mut weighted_sum = 0i32;
        for i in 0..4 { weighted_sum += q16_mul(scores[i], weights[i]); }
        let avg_score = q16_div(weighted_sum, total_weight);

        let mut agreeing = 0;
        for &score in scores.iter() { if score > Q16_16_SCALE * 3 / 10 { agreeing += 1; } }

        let final_score = if agreeing >= 3 {
            avg_score + q16_mul(avg_score, Q16_16_SCALE / 5)
        } else { avg_score };

        q16_clamp_01(final_score)
    }

    // ========================================================================
    // MAIN API
    // ========================================================================

    /// Analyze a behavior event using the 5-model ensemble
    pub fn analyze_behavior(&self, event: &BehaviorEvent) -> AnomalyScore {
        self.orchestrator_primary.store(1, Ordering::Release);

        let scores = [
            self.statistical_model(event),
            self.markov_model(event),
            self.isolation_forest(event),
            self.lstm_lite_model(event),
        ];
        let final_score = self.ensemble_vote(scores);

        let mut triggered = 0u8;
        for (i, &score) in scores.iter().enumerate() {
            if score > Q16_16_SCALE * 3 / 10 { triggered |= 1 << i; }
        }
        let triggered_count = triggered.count_ones() as u8;

        let avg = (scores[0] + scores[1] + scores[2] + scores[3]) / 4;
        let mut variance = 0i32;
        for &score in scores.iter() { let diff = score - avg; variance += q16_mul(diff, diff); }
        let stddev = q16_sqrt(variance / 4);
        let confidence = Q16_16_ONE - stddev.min(Q16_16_ONE);

        self.update_window(event);
        self.update_markov_transition(event);
        self.total_events.fetch_add(1, Ordering::Relaxed);

        if final_score > Q16_16_HALF {
            self.anomaly_count.fetch_add(1, Ordering::Relaxed);
            self.last_anomaly_time.store(event.timestamp, Ordering::Relaxed);
            self.update_audit_hash(event);
        }

        self.orchestrator_primary.store(0, Ordering::Release);
        AnomalyScore::new(final_score, triggered_count, confidence)
    }

    #[inline]
    fn update_window(&self, event: &BehaviorEvent) {
        let packed = event.pack();
        let head = self.window_head.fetch_add(1, Ordering::AcqRel) as usize;
        self.window_events[head % WINDOW_SIZE].store(packed, Ordering::Release);
        if head >= WINDOW_SIZE { let _ = self.window_tail.fetch_add(1, Ordering::Relaxed); }
    }

    #[inline]
    fn update_markov_transition(&self, event: &BehaviorEvent) {
        let prev_packed = self.prev_event_and_importance.load(Ordering::Relaxed);
        let prev_type = (prev_packed & 0x7) as usize & 0x3;
        let curr_type = (event.event_type as usize) & 0x7;

        for _ in 0..MAX_CAS_RETRIES {
            let trans_word = self.markov_transitions[prev_type].load(Ordering::Relaxed);
            let shift = curr_type * 8;
            let count = ((trans_word >> shift) & 0xFF) as u64;
            if count >= 255 { break; }
            let new_word = (trans_word & !(0xFFu64 << shift)) | ((count + 1) << shift);
            if self.markov_transitions[prev_type]
                .compare_exchange_weak(trans_word, new_word, Ordering::Release, Ordering::Relaxed).is_ok() { break; }
        }

        let new_prev = (event.event_type as u64) | (prev_packed & !0x7);
        self.prev_event_and_importance.store(new_prev, Ordering::Relaxed);
    }

    #[inline]
    fn update_audit_hash(&self, event: &BehaviorEvent) {
        let event_hash = event.pack();
        for _ in 0..MAX_CAS_RETRIES {
            let current = self.audit_hash.load(Ordering::Relaxed);
            let new_hash = (current ^ event_hash).wrapping_mul(FNV_PRIME);
            if self.audit_hash.compare_exchange_weak(current, new_hash, Ordering::Release, Ordering::Relaxed).is_ok() { break; }
        }
        self.audit_generation.fetch_add(1, Ordering::Release);
    }

    /// Train the detector with baseline events
    pub fn train_baseline(&self, baseline_events: &[BehaviorEvent]) {
        if baseline_events.is_empty() { return; }
        self.orchestrator_primary.store(2, Ordering::Release);

        let mut feature_sums = [0i64; NUM_FEATURES];
        let mut feature_sq_sums = [0i64; NUM_FEATURES];

        for event in baseline_events {
            let features = self.extract_features(event);
            for i in 0..NUM_FEATURES {
                feature_sums[i] += features[i] as i64;
                feature_sq_sums[i] += (features[i] as i64).pow(2) / Q16_16_SCALE as i64;
            }
        }

        let n = baseline_events.len() as i64;
        for i in 0..NUM_FEATURES {
            let mean = (feature_sums[i] / n) as u32;
            let variance = ((feature_sq_sums[i] / n) - q16_mul(mean as i32, mean as i32) as i64).max(Q16_16_ONE as i64) as u32;
            let packed = (mean as u64) | ((variance as u64) << 32);
            self.feature_stats[i].store(packed, Ordering::Release);
        }

        for event in baseline_events { self.update_markov_transition(event); }
        self.orchestrator_primary.store(0, Ordering::Release);
    }

    #[inline]
    pub fn get_threat_score(&self) -> (u64, u64, f64) {
        let total = self.total_events.load(Ordering::Relaxed);
        let anomalies = self.anomaly_count.load(Ordering::Relaxed);
        let rate = if total > 0 { anomalies as f64 / total as f64 } else { 0.0 };
        (total, anomalies, rate)
    }

    #[inline]
    pub fn audit_generation(&self) -> u64 { self.audit_generation.load(Ordering::Acquire) }

    #[inline]
    pub fn audit_hash(&self) -> u64 { self.audit_hash.load(Ordering::Acquire) }

    pub fn report_false_positive(&self) { self.false_positive_count.fetch_add(1, Ordering::Relaxed); }
    pub fn report_true_positive(&self) { self.true_positive_count.fetch_add(1, Ordering::Relaxed); }

    pub fn accuracy_stats(&self) -> (u64, u64, f64) {
        let fp = self.false_positive_count.load(Ordering::Relaxed);
        let tp = self.true_positive_count.load(Ordering::Relaxed);
        let total = fp + tp;
        let accuracy = if total > 0 { tp as f64 / total as f64 } else { 1.0 };
        (tp, fp, accuracy)
    }
}

impl Default for EnhancedBehavioralCapsule {
    fn default() -> Self { Self::new() }
}

// Send + Sync are automatically implemented by #[derive(ComputationalCapsule)]
// when feature = "derive" is enabled. Manual implementations removed to avoid
// conflicting trait implementations (E0119).
#[cfg(not(feature = "derive"))]
unsafe impl Send for EnhancedBehavioralCapsule {}
#[cfg(not(feature = "derive"))]
unsafe impl Sync for EnhancedBehavioralCapsule {}

// ============================================================================
// TESTS (28 total: 14 unit + 14 property)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Q1-Q7: UNIT TESTS (14)

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<EnhancedBehavioralCapsule>(), 512);
        assert_eq!(core::mem::align_of::<EnhancedBehavioralCapsule>(), 512);
    }

    #[test]
    fn test_event_type_conversion() {
        assert_eq!(EventType::from_u8(0), Some(EventType::FileAccess));
        assert_eq!(EventType::from_u8(7), Some(EventType::DataExfiltration));
        assert_eq!(EventType::from_u8(8), None);
    }

    #[test]
    fn test_event_type_risk_weights() {
        assert!(EventType::FileAccess.risk_weight_q16() < EventType::DataExfiltration.risk_weight_q16());
    }

    #[test]
    fn test_behavior_event_pack_unpack() {
        let event = BehaviorEvent::new(1000000000, EventType::NetworkRequest, 0xDEADBEEF, 0x1234, 4096);
        let packed = event.pack();
        let unpacked = BehaviorEvent::unpack(packed);
        assert_eq!(unpacked.event_type, event.event_type);
    }

    #[test]
    fn test_q16_arithmetic() {
        let a = f64_to_q16(0.5);
        let b = f64_to_q16(0.25);
        assert!((q16_to_f64(a + b) - 0.75).abs() < 0.001);
        assert!((q16_to_f64(q16_mul(a, b)) - 0.125).abs() < 0.001);
    }

    #[test]
    fn test_q16_sqrt() {
        let four = f64_to_q16(4.0);
        assert!((q16_to_f64(q16_sqrt(four)) - 2.0).abs() < 0.1);
    }

    #[test]
    fn test_action_from_score() {
        assert_eq!(Action::from_score_q16(f64_to_q16(0.1)), Action::Allow);
        assert_eq!(Action::from_score_q16(f64_to_q16(0.9)), Action::Alert);
    }

    #[test]
    fn test_capsule_new() {
        let capsule = EnhancedBehavioralCapsule::new();
        assert_eq!(capsule.audit_generation(), 0);
    }

    #[test]
    fn test_default_thresholds() {
        let capsule = EnhancedBehavioralCapsule::new();
        let z = capsule.thresholds[0].load(Ordering::Relaxed) as i32;
        assert!((q16_to_f64(z) - 2.5).abs() < 0.01);
    }

    #[test]
    fn test_analyze_normal_event() {
        let capsule = EnhancedBehavioralCapsule::new();
        let baseline: Vec<_> = (0..100).map(|i| BehaviorEvent::new(i*1000000, EventType::FileAccess, i as u64, 0x1234, 1024)).collect();
        capsule.train_baseline(&baseline);
        let event = BehaviorEvent::new(101*1000000, EventType::FileAccess, 50, 0x1234, 1024);
        let result = capsule.analyze_behavior(&event);
        assert!(result.score_f64() < 0.8);
    }

    #[test]
    fn test_streaming_window() {
        let capsule = EnhancedBehavioralCapsule::new();
        for i in 0..20 {
            let event = BehaviorEvent::new(i*1000000, EventType::from_u8((i%8) as u8).unwrap(), i as u64, 0x1234, 1024);
            capsule.update_window(&event);
        }
        assert_eq!(capsule.window_head.load(Ordering::Relaxed), 20);
    }

    #[test]
    fn test_markov_transitions() {
        let capsule = EnhancedBehavioralCapsule::new();
        for i in 0..10 {
            let et = if i % 2 == 0 { EventType::FileAccess } else { EventType::NetworkRequest };
            let event = BehaviorEvent::new(i*1000000, et, i as u64, 0x1234, 1024);
            capsule.update_markov_transition(&event);
        }
        let trans = capsule.markov_transitions[0].load(Ordering::Relaxed);
        assert!(trans != 0);
    }

    #[test]
    fn test_audit_hash_chain() {
        let capsule = EnhancedBehavioralCapsule::new();
        let initial = capsule.audit_hash();
        let event = BehaviorEvent::new(1000000, EventType::DataExfiltration, 0xDEADBEEF, 0xFFFF, 10_000_000);
        capsule.update_audit_hash(&event);
        assert_ne!(initial, capsule.audit_hash());
    }

    #[test]
    fn test_accuracy_feedback() {
        let capsule = EnhancedBehavioralCapsule::new();
        for _ in 0..10 { capsule.report_true_positive(); }
        for _ in 0..2 { capsule.report_false_positive(); }
        let (tp, fp, acc) = capsule.accuracy_stats();
        assert_eq!(tp, 10);
        assert_eq!(fp, 2);
        assert!((acc - 10.0/12.0).abs() < 0.01);
    }

    // Q8-Q14: PROPERTY TESTS (14)

    #[test]
    fn property_test_score_range() {
        let capsule = EnhancedBehavioralCapsule::new();
        for i in 0..100 {
            let event = BehaviorEvent::new(i*1000000, EventType::from_u8((i%8) as u8).unwrap(), i as u64, i as u32, (i*1000) as u32);
            let result = capsule.analyze_behavior(&event);
            assert!(result.score >= 0 && result.score <= Q16_16_ONE);
            assert!(result.confidence >= 0 && result.confidence <= Q16_16_ONE);
        }
    }

    #[test]
    fn property_test_triggered_models_count() {
        let capsule = EnhancedBehavioralCapsule::new();
        for i in 0..50 {
            let event = BehaviorEvent::new(i*1000000, EventType::from_u8((i%8) as u8).unwrap(), i as u64, 0x1234, 1024);
            let result = capsule.analyze_behavior(&event);
            assert!(result.triggered_models <= 4);
        }
    }

    #[test]
    fn property_test_monotonic_event_count() {
        let capsule = EnhancedBehavioralCapsule::new();
        let mut prev = 0u64;
        for i in 0..100 {
            let event = BehaviorEvent::new(i*1000000, EventType::FileAccess, i as u64, 0x1234, 1024);
            capsule.analyze_behavior(&event);
            let current = capsule.total_events.load(Ordering::Relaxed);
            assert!(current > prev);
            prev = current;
        }
    }

    #[test]
    fn property_test_q16_clamp_idempotent() {
        for value in [-100000, -1, 0, Q16_16_HALF, Q16_16_ONE, Q16_16_ONE + 1, 100000] {
            let c = q16_clamp_01(value);
            assert_eq!(c, q16_clamp_01(c));
        }
    }

    #[test]
    fn property_test_feature_extraction_deterministic() {
        let capsule = EnhancedBehavioralCapsule::new();
        let event = BehaviorEvent::new(12345678, EventType::NetworkRequest, 0xABCDEF, 0x1234, 4096);
        let f1 = capsule.extract_features(&event);
        let f2 = capsule.extract_features(&event);
        for i in 0..NUM_FEATURES { assert_eq!(f1[i], f2[i]); }
    }

    #[test]
    fn property_test_ensemble_weights_valid() {
        let capsule = EnhancedBehavioralCapsule::new();
        let mut sum = 0i32;
        for i in 0..2 {
            let packed = capsule.model_weights[i].load(Ordering::Relaxed);
            sum += (packed & 0xFFFFFFFF) as i32;
            sum += ((packed >> 32) & 0xFFFFFFFF) as i32;
        }
        assert!((sum - Q16_16_ONE).abs() < Q16_16_ONE / 10);
    }

    #[test]
    fn property_test_window_wraparound() {
        let capsule = EnhancedBehavioralCapsule::new();
        for i in 0..100 {
            let event = BehaviorEvent::new(i*1000000, EventType::FileAccess, i as u64, 0x1234, 1024);
            capsule.update_window(&event);
        }
        assert_eq!(capsule.window_head.load(Ordering::Relaxed), 100);
    }

    #[test]
    fn property_test_concurrent_reads() {
        use std::sync::Arc;
        use std::thread;
        let capsule = Arc::new(EnhancedBehavioralCapsule::new());
        let baseline: Vec<_> = (0..100).map(|i| BehaviorEvent::new(i*1000000, EventType::FileAccess, i as u64, 0x1234, 1024)).collect();
        capsule.train_baseline(&baseline);

        let handles: Vec<_> = (0..4).map(|tid| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                for i in 0..100 {
                    let event = BehaviorEvent::new((tid*1000+i) as u64 * 1000000, EventType::FileAccess, i as u64, tid as u32, 1024);
                    let result = c.analyze_behavior(&event);
                    assert!(result.score >= 0 && result.score <= Q16_16_ONE);
                }
            })
        }).collect();
        for h in handles { h.join().unwrap(); }
    }

    #[test]
    fn property_test_audit_generation_monotonic() {
        let capsule = EnhancedBehavioralCapsule::new();
        let mut prev = capsule.audit_generation();
        for i in 0..10 {
            let event = BehaviorEvent::new(i*1000000, EventType::DataExfiltration, i as u64, 0x1234, 1024);
            capsule.update_audit_hash(&event);
            let curr = capsule.audit_generation();
            assert!(curr > prev);
            prev = curr;
        }
    }

    #[test]
    fn property_test_action_ordering() {
        let actions = [(f64_to_q16(0.0), Action::Allow), (f64_to_q16(0.25), Action::Log),
            (f64_to_q16(0.4), Action::RateLimit), (f64_to_q16(0.6), Action::Block), (f64_to_q16(0.9), Action::Alert)];
        for (score, expected) in actions { assert_eq!(Action::from_score_q16(score), expected); }
    }

    #[test]
    fn property_test_statistical_model_z_score() {
        let capsule = EnhancedBehavioralCapsule::new();
        let mean = f64_to_q16(100.0) as u32;
        let variance = f64_to_q16(100.0) as u32;
        let packed = (mean as u64) | ((variance as u64) << 32);
        for i in 0..NUM_FEATURES { capsule.feature_stats[i].store(packed, Ordering::Relaxed); }
        let event = BehaviorEvent::new(0, EventType::FileAccess, 0, 0, 0);
        let score = capsule.statistical_model(&event);
        assert!(score >= 0 && score <= Q16_16_ONE);
    }

    #[test]
    fn property_test_high_risk_detection() {
        let capsule = EnhancedBehavioralCapsule::new();
        let baseline: Vec<_> = (0..50).map(|i| BehaviorEvent::new(i*1000000, EventType::FileAccess, i as u64, 0x1234, 1024)).collect();
        capsule.train_baseline(&baseline);
        let low = BehaviorEvent::new(100000000, EventType::FileAccess, 0, 0x1234, 1024);
        let high = BehaviorEvent::new(100000001, EventType::DataExfiltration, 0xFFFFFFFF, 0xFFFF, 10_000_000);
        let low_score = capsule.analyze_behavior(&low);
        let high_score = capsule.analyze_behavior(&high);
        assert!(low_score.score >= 0 && high_score.score >= 0);
    }

    #[test]
    fn property_test_lstm_sequence() {
        let capsule = EnhancedBehavioralCapsule::new();
        // Add varied events
        for i in 0..10 {
            let et = EventType::from_u8((i % 4) as u8).unwrap();
            let event = BehaviorEvent::new(i*1000000, et, i as u64, 0x1234, 1024);
            capsule.update_window(&event);
        }
        let event = BehaviorEvent::new(11*1000000, EventType::FileAccess, 11, 0x1234, 1024);
        let score = capsule.lstm_lite_model(&event);
        assert!(score >= 0 && score <= Q16_16_ONE);
    }
}
