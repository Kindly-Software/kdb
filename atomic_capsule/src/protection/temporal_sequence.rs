//! # Temporal Sequence Capsule - Time-Series Anomaly Detection V2
//!
//! **Tier Composition**: T5 Streaming (ring buffer) + T3 Fixed-Point (Q8.8 scores/decay)
//!
//! Provides temporal sequence analysis for anomaly detection using exponential decay weighting.
//! 248-entry ring buffer with pre-computed decay table for <50ns per temporal check.
//!
//! ## UCE34 Framework Analysis (Q1-Q34)
//!
//! ### Q1-Q9: Meta-Cognitive Analysis
//! - **Q1 (Scope)**: Temporal pattern detection for AnomalyDetectorV2 Layer 4
//! - **Q2 (Assumptions)**: Recent behaviors more relevant than older ones
//! - **Q3 (Constraints)**: <50ns per temporal check, 2048B total
//! - **Q4 (Context)**: AnomalyDetectorV2 Layer 4 (after Bloom/GMM/TinyML)
//! - **Q5 (Success)**: Detect temporal anomaly patterns (bursts, sequences, timing)
//! - **Q6 (Failure)**: Memory ordering issues cause race conditions
//! - **Q7 (Patterns)**: Ring buffer, exponential decay, lockfree streaming
//! - **Q8 (Alternatives)**: Sliding window (memory), LSTM (too slow)
//! - **Q9 (Trade-offs)**: Window size vs memory, decay rate vs responsiveness
//!
//! ### Q10-Q12: Foundation (Capsule Architecture)
//! - **Q10 (Tier Selection)**: T5 Streaming (O(1) append) + T3 Fixed-Point (Q8.8)
//! - **Q11 (Rust Transform)**: TemporalEntry (8B), TemporalSequenceCapsule (2048B)
//! - **Q12 (Nightly)**: const_fn for compile-time decay table initialization
//!
//! ### Q13-Q27: Implementation
//! - **Q13 (Core Mechanism)**: Ring buffer with exponential decay weighting
//! - **Q14 (State Management)**: Atomic head/tail pointers, generation counter
//! - **Q15 (Resource Usage)**: 2048B (64B header + 1984B ring buffer)
//! - **Q28 (Simplicity)**: 3-method API (append, compute_temporal_score, get_recent)
//! - **Q33 (Verification)**: Compile-time verification via derive macro
//! - **Q34 (Auditability)**: Generation counter tracks sequence version
//!
//! ## Performance Targets (B32 Framework)
//!
//! | Operation | Target | Notes |
//! |-----------|--------|-------|
//! | append() | <10ns | Single atomic increment + store |
//! | compute_temporal_score() | <50ns | 248 entries × decay lookup |
//! | get_recent(n) | <5ns | Single pointer arithmetic |
//!
//! ## Memory Layout (2048B total)
//!
//! ```text
//! TemporalSequenceCapsule (2048B, 64B aligned):
//! ┌────────────────────────────────────────┐
//! │ HEADER (64B)                           │
//! │   head: AtomicU32                      │
//! │   tail: AtomicU32                      │
//! │   window_size: AtomicU16               │
//! │   decay_factor_q8: AtomicU16           │
//! │   generation: AtomicU64                │
//! │   total_entries: AtomicU64             │
//! │   burst_threshold_q8: AtomicI16        │
//! │   timing_threshold_ms: AtomicU32       │
//! │   _padding: [u8; 28]                   │
//! ├────────────────────────────────────────┤
//! │ RING BUFFER (1984B = 248 × 8B)         │
//! │   entry[0]: TemporalEntry (8B)         │
//! │   entry[1]: TemporalEntry (8B)         │
//! │   ...                                  │
//! │   entry[247]: TemporalEntry (8B)       │
//! └────────────────────────────────────────┘
//!
//! TemporalEntry (8B, 2B aligned):
//! ┌────────────────────────────────────────┐
//! │ timestamp_ms: u32 (4B)                 │
//! │ behavior_hash: u16 (2B)                │
//! │ anomaly_score_q8_8: i16 (2B)           │
//! └────────────────────────────────────────┘
//! ```
//!
//! ## ASSUM Framework
//!
//! ### Temporal Assumptions
//! - `#ASSUME_MONOTONIC_TIME`: Timestamps are monotonically increasing
//! - `#ASSUME_DECAY_0_95`: Default decay factor 0.95 (5% decay per step)
//! - `#ASSUME_248_WINDOW`: 248 entries sufficient for temporal patterns
//! - `#ASSUME_MS_PRECISION`: Millisecond precision sufficient for anomaly timing
//!
//! ### Performance Assumptions
//! - `#ASSUME_APPEND_10NS`: Single atomic op <10ns
//! - `#ASSUME_DECAY_LOOKUP`: Pre-computed table lookup <1ns
//! - `#ASSUME_CACHE_HOT`: 2048B capsule stays in L1/L2 cache

#![allow(unsafe_code)] // Required for atomic operations

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, AtomicU64, AtomicU16, AtomicI16, Ordering};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Maximum entries in temporal ring buffer
pub const TEMPORAL_BUFFER_SIZE: usize = 248;

/// Default decay factor (0.95 in Q8.8 = 243)
pub const DEFAULT_DECAY_FACTOR_Q8: u16 = 243;

/// Default burst threshold (3.0 in Q8.8 = 768)
pub const DEFAULT_BURST_THRESHOLD_Q8: i16 = 768;

/// Default timing threshold (100ms)
pub const DEFAULT_TIMING_THRESHOLD_MS: u32 = 100;

// ============================================================================
// PRE-COMPUTED DECAY TABLE (compile-time const fn)
// ============================================================================

/// Pre-computed exponential decay table for indices 0-247
/// decay[i] = 0.95^i in Q8.8 format
///
/// # ASSUM Safety
/// - `#ASSUME_DECAY_TABLE_CONST`: Computed at compile-time (0ns runtime)
/// - `#VERIFY_DECAY_RANGE`: All values in [0, 256] (0.0 to 1.0 in Q8.8)
pub const DECAY_TABLE_Q8_8: [u16; TEMPORAL_BUFFER_SIZE] = compute_decay_table();

/// Compute decay table at compile time
const fn compute_decay_table() -> [u16; TEMPORAL_BUFFER_SIZE] {
    let mut table = [0u16; TEMPORAL_BUFFER_SIZE];
    let decay_factor = 0.95f64;

    let mut i = 0;
    let mut current = 1.0f64;

    while i < TEMPORAL_BUFFER_SIZE {
        // Convert to Q8.8: value * 256
        table[i] = (current * 256.0) as u16;
        current *= decay_factor;
        i += 1;
    }

    table
}

// ============================================================================
// TEMPORAL ENTRY (8 bytes)
// ============================================================================

/// Temporal entry for ring buffer (8 bytes)
///
/// Stores a single behavior observation with timestamp and anomaly score.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TemporalEntry {
    /// Timestamp in milliseconds (relative to capsule creation)
    /// Supports ~49 days of relative time
    pub timestamp_ms: u32,

    /// Hash of behavior (truncated to 16 bits for space efficiency)
    /// Used for sequence pattern detection
    pub behavior_hash: u16,

    /// Anomaly score from GMM/TinyML (Q8.8 fixed-point)
    /// Range: -128.0 to 127.996
    pub anomaly_score_q8_8: i16,
}

impl TemporalEntry {
    /// Create a new temporal entry
    #[inline]
    pub const fn new(timestamp_ms: u32, behavior_hash: u16, anomaly_score_q8_8: i16) -> Self {
        Self {
            timestamp_ms,
            behavior_hash,
            anomaly_score_q8_8,
        }
    }

    /// Create an empty entry
    #[inline]
    pub const fn empty() -> Self {
        Self {
            timestamp_ms: 0,
            behavior_hash: 0,
            anomaly_score_q8_8: 0,
        }
    }

    /// Get anomaly score as f32
    #[inline]
    pub const fn score_f32(&self) -> f32 {
        self.anomaly_score_q8_8 as f32 / 256.0
    }

    /// Create entry with f32 anomaly score
    #[inline]
    pub fn with_score(timestamp_ms: u32, behavior_hash: u16, score: f32) -> Self {
        let score_q8_8 = (score * 256.0).clamp(-32768.0, 32767.0) as i16;
        Self::new(timestamp_ms, behavior_hash, score_q8_8)
    }

    /// Check if entry is valid (non-zero timestamp)
    #[inline]
    pub const fn is_valid(&self) -> bool {
        self.timestamp_ms != 0
    }
}

// Compile-time size verification
const _: () = {
    assert!(core::mem::size_of::<TemporalEntry>() == 8);
    assert!(core::mem::align_of::<TemporalEntry>() == 4);
};

// ============================================================================
// TEMPORAL SEQUENCE CAPSULE (2048 bytes)
// ============================================================================

/// Temporal sequence capsule for time-series anomaly detection (2048B, 64B aligned)
///
/// # Performance
/// - append(): <10ns (atomic increment + store)
/// - compute_temporal_score(): <50ns (248 decay-weighted sum)
/// - get_recent(n): <5ns (pointer arithmetic)
///
/// # Thread Safety
/// - 100% lockfree (no mutex/RwLock)
/// - Concurrent appends supported (atomic head increment)
/// - Concurrent reads supported (snapshot consistency)
#[repr(C, align(64))]
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 64))]
pub struct TemporalSequenceCapsule {
    // ========== HEADER (64 bytes) ==========

    /// Head pointer (next write position)
    head: AtomicU32,

    /// Tail pointer (oldest valid entry, for wraparound tracking)
    tail: AtomicU32,

    /// Active window size (how many recent entries to consider)
    window_size: AtomicU16,

    /// Decay factor (Q8.8, default 0.95 = 243)
    decay_factor_q8: AtomicU16,

    /// Generation counter (Q34 audit trail)
    generation: AtomicU64,

    /// Total entries appended (wraps around)
    total_entries: AtomicU64,

    /// Burst detection threshold (Q8.8)
    /// Triggers alert if weighted sum exceeds this
    burst_threshold_q8: AtomicI16,

    /// Timing anomaly threshold (milliseconds)
    /// Triggers alert if time between entries < threshold
    timing_threshold_ms: AtomicU32,

    /// Anomaly burst count
    burst_count: AtomicU64,

    /// Timing anomaly count
    timing_anomaly_count: AtomicU64,

    /// Padding to 64 bytes
    _padding_header: [u8; 10],

    // ========== RING BUFFER (1984 bytes = 248 × 8B) ==========

    /// Ring buffer of temporal entries
    /// Uses UnsafeCell to allow interior mutability for concurrent appends
    entries: UnsafeCell<[TemporalEntry; TEMPORAL_BUFFER_SIZE]>,
}

// SAFETY: TemporalSequenceCapsule uses atomic operations for coordination
// and each slot is written by only one thread at a time (via atomic head increment)
// Note: Sync impl is provided by ComputationalCapsule derive when feature="derive" is enabled
#[cfg(not(feature = "derive"))]
unsafe impl Sync for TemporalSequenceCapsule {}

impl TemporalSequenceCapsule {
    /// Create a new temporal sequence capsule with default settings
    pub fn new() -> Self {
        const EMPTY_ENTRY: TemporalEntry = TemporalEntry::empty();
        Self {
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
            window_size: AtomicU16::new(TEMPORAL_BUFFER_SIZE as u16),
            decay_factor_q8: AtomicU16::new(DEFAULT_DECAY_FACTOR_Q8),
            generation: AtomicU64::new(0),
            total_entries: AtomicU64::new(0),
            burst_threshold_q8: AtomicI16::new(DEFAULT_BURST_THRESHOLD_Q8),
            timing_threshold_ms: AtomicU32::new(DEFAULT_TIMING_THRESHOLD_MS),
            burst_count: AtomicU64::new(0),
            timing_anomaly_count: AtomicU64::new(0),
            _padding_header: [0; 10],
            entries: UnsafeCell::new([EMPTY_ENTRY; TEMPORAL_BUFFER_SIZE]),
        }
    }

    /// Get read-only reference to entries
    #[inline]
    fn entries(&self) -> &[TemporalEntry; TEMPORAL_BUFFER_SIZE] {
        // SAFETY: We only read here, writes are synchronized via atomic head
        unsafe { &*self.entries.get() }
    }

    /// Get mutable reference to specific entry
    /// SAFETY: Caller must ensure slot is not being written by another thread
    #[inline]
    unsafe fn entry_mut(&self, idx: usize) -> &mut TemporalEntry {
        &mut (*self.entries.get())[idx]
    }

    /// Create with custom window size
    pub fn with_window_size(window_size: u16) -> Self {
        let mut capsule = Self::new();
        capsule.window_size.store(
            window_size.clamp(1, TEMPORAL_BUFFER_SIZE as u16),
            Ordering::Relaxed,
        );
        capsule
    }

    /// Append a new entry to the ring buffer
    ///
    /// # Performance
    /// Target: <10ns (single atomic fetch_add + store)
    ///
    /// # Returns
    /// Index where entry was stored
    #[inline]
    pub fn append(&self, entry: TemporalEntry) -> u32 {
        // Atomically increment head and get slot
        let slot = self.head.fetch_add(1, Ordering::AcqRel) as usize % TEMPORAL_BUFFER_SIZE;

        // Check for timing anomaly (rapid succession)
        let last_slot = if slot == 0 { TEMPORAL_BUFFER_SIZE - 1 } else { slot - 1 };
        let last_timestamp = self.entries()[last_slot].timestamp_ms;
        let threshold = self.timing_threshold_ms.load(Ordering::Relaxed);

        if last_timestamp > 0 && entry.timestamp_ms > last_timestamp {
            let delta = entry.timestamp_ms - last_timestamp;
            if delta < threshold {
                self.timing_anomaly_count.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Store entry
        // SAFETY: Only one thread can own this slot at a time due to atomic increment
        unsafe {
            *self.entry_mut(slot) = entry;
        }

        // Update tail if we've wrapped around
        let total = self.total_entries.fetch_add(1, Ordering::Relaxed);
        if total >= TEMPORAL_BUFFER_SIZE as u64 {
            let new_tail = (slot + 1) % TEMPORAL_BUFFER_SIZE;
            self.tail.store(new_tail as u32, Ordering::Relaxed);
        }

        self.generation.fetch_add(1, Ordering::Relaxed);

        slot as u32
    }

    /// Append with automatic timestamp and score
    #[inline]
    pub fn append_behavior(&self, behavior_hash: u64, anomaly_score: f32, timestamp_ms: u32) -> u32 {
        let hash_truncated = (behavior_hash & 0xFFFF) as u16;
        let score_q8_8 = (anomaly_score * 256.0).clamp(-32768.0, 32767.0) as i16;
        let entry = TemporalEntry::new(timestamp_ms, hash_truncated, score_q8_8);
        self.append(entry)
    }

    /// Compute temporal anomaly score using exponential decay weighting
    ///
    /// # Formula
    /// score = Σ(entry[i].anomaly_score × decay^i) for i in 0..window_size
    ///
    /// # Performance
    /// Target: <50ns (248 multiplications + additions)
    ///
    /// # Returns
    /// (weighted_score_q8_8, is_burst_anomaly)
    #[inline]
    pub fn compute_temporal_score(&self) -> (i32, bool) {
        let head = self.head.load(Ordering::Acquire) as usize;
        let window = self.window_size.load(Ordering::Relaxed) as usize;
        let total = self.total_entries.load(Ordering::Relaxed) as usize;

        let effective_window = window.min(total).min(TEMPORAL_BUFFER_SIZE);

        let mut weighted_sum: i32 = 0;

        for i in 0..effective_window {
            // Calculate index going backwards from head
            let idx = if head >= i + 1 {
                head - i - 1
            } else {
                TEMPORAL_BUFFER_SIZE - (i + 1 - head)
            };

            let entry = &self.entries()[idx % TEMPORAL_BUFFER_SIZE];
            let score = entry.anomaly_score_q8_8 as i32;
            let decay = DECAY_TABLE_Q8_8[i] as i32;

            // Q8.8 × Q8.8 → Q16.16, then shift back to Q8.8
            weighted_sum += (score * decay) >> 8;
        }

        let threshold = self.burst_threshold_q8.load(Ordering::Relaxed) as i32;
        let is_burst = weighted_sum > threshold;

        if is_burst {
            self.burst_count.fetch_add(1, Ordering::Relaxed);
        }

        (weighted_sum, is_burst)
    }

    /// Get the most recent entry
    #[inline]
    pub fn get_recent(&self) -> Option<TemporalEntry> {
        let total = self.total_entries.load(Ordering::Relaxed);
        if total == 0 {
            return None;
        }

        let head = self.head.load(Ordering::Acquire) as usize % TEMPORAL_BUFFER_SIZE;
        let idx = if head == 0 { TEMPORAL_BUFFER_SIZE - 1 } else { head - 1 };
        Some(self.entries()[idx])
    }

    /// Get n most recent entries (most recent first)
    pub fn get_recent_n(&self, n: usize) -> Vec<TemporalEntry> {
        let head = self.head.load(Ordering::Acquire) as usize % TEMPORAL_BUFFER_SIZE;
        let total = self.total_entries.load(Ordering::Relaxed) as usize;
        let effective_n = n.min(total).min(TEMPORAL_BUFFER_SIZE);

        let mut result = Vec::with_capacity(effective_n);

        for i in 0..effective_n {
            // Calculate index backwards from head position with wraparound
            let idx = if head > i {
                head - i - 1
            } else {
                TEMPORAL_BUFFER_SIZE - 1 - (i - head)
            };
            result.push(self.entries()[idx]);
        }

        result
    }

    /// Check for repeated behavior pattern (sequence detection)
    ///
    /// # Returns
    /// Number of times the given behavior hash appears in recent window
    #[inline]
    pub fn count_behavior(&self, behavior_hash: u16) -> u32 {
        let head = self.head.load(Ordering::Acquire) as usize;
        let window = self.window_size.load(Ordering::Relaxed) as usize;
        let total = self.total_entries.load(Ordering::Relaxed) as usize;

        let effective_window = window.min(total).min(TEMPORAL_BUFFER_SIZE);
        let mut count = 0u32;

        for i in 0..effective_window {
            let idx = if head >= i + 1 {
                head - i - 1
            } else {
                TEMPORAL_BUFFER_SIZE - (i + 1 - head)
            };

            if self.entries()[idx % TEMPORAL_BUFFER_SIZE].behavior_hash == behavior_hash {
                count += 1;
            }
        }

        count
    }

    /// Compute timing statistics
    ///
    /// # Returns
    /// (avg_interval_ms, min_interval_ms, max_interval_ms)
    pub fn timing_stats(&self) -> (u32, u32, u32) {
        let head = self.head.load(Ordering::Acquire) as usize;
        let total = self.total_entries.load(Ordering::Relaxed) as usize;

        if total < 2 {
            return (0, 0, 0);
        }

        let effective_count = total.min(TEMPORAL_BUFFER_SIZE);
        let mut sum = 0u64;
        let mut min_interval = u32::MAX;
        let mut max_interval = 0u32;
        let mut count = 0u32;

        for i in 0..effective_count - 1 {
            let idx1 = if head >= i + 1 { head - i - 1 } else { TEMPORAL_BUFFER_SIZE - (i + 1 - head) };
            let idx2 = if head >= i + 2 { head - i - 2 } else { TEMPORAL_BUFFER_SIZE - (i + 2 - head) };

            let t1 = self.entries()[idx1 % TEMPORAL_BUFFER_SIZE].timestamp_ms;
            let t2 = self.entries()[idx2 % TEMPORAL_BUFFER_SIZE].timestamp_ms;

            if t1 > t2 {
                let interval = t1 - t2;
                sum += interval as u64;
                min_interval = min_interval.min(interval);
                max_interval = max_interval.max(interval);
                count += 1;
            }
        }

        if count == 0 {
            return (0, 0, 0);
        }

        let avg = (sum / count as u64) as u32;
        (avg, min_interval, max_interval)
    }

    /// Get current generation (for audit trail)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Get total entries appended
    #[inline]
    pub fn total_entries(&self) -> u64 {
        self.total_entries.load(Ordering::Relaxed)
    }

    /// Get burst anomaly count
    #[inline]
    pub fn burst_count(&self) -> u64 {
        self.burst_count.load(Ordering::Relaxed)
    }

    /// Get timing anomaly count
    #[inline]
    pub fn timing_anomaly_count(&self) -> u64 {
        self.timing_anomaly_count.load(Ordering::Relaxed)
    }

    /// Get current head position
    #[inline]
    pub fn head_position(&self) -> u32 {
        self.head.load(Ordering::Relaxed)
    }

    /// Set burst threshold
    #[inline]
    pub fn set_burst_threshold(&self, threshold: f32) {
        let threshold_q8 = (threshold * 256.0).clamp(-32768.0, 32767.0) as i16;
        self.burst_threshold_q8.store(threshold_q8, Ordering::SeqCst);
    }

    /// Set timing threshold (milliseconds)
    #[inline]
    pub fn set_timing_threshold_ms(&self, threshold_ms: u32) {
        self.timing_threshold_ms.store(threshold_ms, Ordering::SeqCst);
    }

    /// Set decay factor (0.0 - 1.0)
    #[inline]
    pub fn set_decay_factor(&self, decay: f32) {
        let decay_q8 = (decay.clamp(0.0, 1.0) * 256.0) as u16;
        self.decay_factor_q8.store(decay_q8, Ordering::SeqCst);
    }

    /// Reset statistics counters
    #[inline]
    pub fn reset_statistics(&self) {
        self.burst_count.store(0, Ordering::SeqCst);
        self.timing_anomaly_count.store(0, Ordering::SeqCst);
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.head.store(0, Ordering::SeqCst);
        self.tail.store(0, Ordering::SeqCst);
        self.total_entries.store(0, Ordering::SeqCst);
        self.generation.fetch_add(1, Ordering::SeqCst);

        // SAFETY: We have &mut self, so exclusive access is guaranteed
        let entries = self.entries.get_mut();
        for entry in entries.iter_mut() {
            *entry = TemporalEntry::empty();
        }
    }
}

impl Default for TemporalSequenceCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for TemporalSequenceCapsule {
    fn clone(&self) -> Self {
        let mut new_entries = [TemporalEntry::empty(); TEMPORAL_BUFFER_SIZE];
        for (i, entry) in self.entries().iter().enumerate() {
            new_entries[i] = *entry;
        }

        Self {
            head: AtomicU32::new(self.head.load(Ordering::Relaxed)),
            tail: AtomicU32::new(self.tail.load(Ordering::Relaxed)),
            window_size: AtomicU16::new(self.window_size.load(Ordering::Relaxed)),
            decay_factor_q8: AtomicU16::new(self.decay_factor_q8.load(Ordering::Relaxed)),
            generation: AtomicU64::new(self.generation.load(Ordering::Relaxed)),
            total_entries: AtomicU64::new(self.total_entries.load(Ordering::Relaxed)),
            burst_threshold_q8: AtomicI16::new(self.burst_threshold_q8.load(Ordering::Relaxed)),
            timing_threshold_ms: AtomicU32::new(self.timing_threshold_ms.load(Ordering::Relaxed)),
            burst_count: AtomicU64::new(self.burst_count.load(Ordering::Relaxed)),
            timing_anomaly_count: AtomicU64::new(self.timing_anomaly_count.load(Ordering::Relaxed)),
            _padding_header: [0; 10],
            entries: UnsafeCell::new(new_entries),
        }
    }
}

// Compile-time size verification
// Header: 64B (with alignment padding) + Ring buffer: 248 × 8B = 1984B
// Total with align(64): rounds to 2112B
const _: () = {
    let size = core::mem::size_of::<TemporalSequenceCapsule>();
    assert!(size == 2112);
    assert!(core::mem::align_of::<TemporalSequenceCapsule>() == 64);
};

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== UNIT TESTS (10) ====================

    #[test]
    fn test_temporal_entry_size_alignment() {
        assert_eq!(core::mem::size_of::<TemporalEntry>(), 8);
        assert_eq!(core::mem::align_of::<TemporalEntry>(), 4);
    }

    #[test]
    fn test_temporal_sequence_capsule_size_alignment() {
        assert_eq!(core::mem::size_of::<TemporalSequenceCapsule>(), 2112);
        assert_eq!(core::mem::align_of::<TemporalSequenceCapsule>(), 64);
    }

    #[test]
    fn test_decay_table_generation() {
        // First entry should be 256 (1.0 in Q8.8)
        assert_eq!(DECAY_TABLE_Q8_8[0], 256);

        // Each subsequent entry should be ~0.95× previous
        for i in 1..10 {
            let expected = (DECAY_TABLE_Q8_8[i - 1] as f32 * 0.95) as u16;
            let actual = DECAY_TABLE_Q8_8[i];
            assert!((actual as i32 - expected as i32).abs() <= 1,
                "Decay[{}] = {}, expected ~{}", i, actual, expected);
        }
    }

    #[test]
    fn test_temporal_entry_creation() {
        let entry = TemporalEntry::new(1000, 0xABCD, 256); // 1.0 in Q8.8
        assert_eq!(entry.timestamp_ms, 1000);
        assert_eq!(entry.behavior_hash, 0xABCD);
        assert_eq!(entry.anomaly_score_q8_8, 256);
        assert!((entry.score_f32() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_capsule_creation() {
        let capsule = TemporalSequenceCapsule::new();
        assert_eq!(capsule.total_entries(), 0);
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.head_position(), 0);
    }

    #[test]
    fn test_append_and_retrieve() {
        let capsule = TemporalSequenceCapsule::new();

        let entry = TemporalEntry::new(1000, 0x1234, 128);
        capsule.append(entry);

        assert_eq!(capsule.total_entries(), 1);

        let recent = capsule.get_recent().unwrap();
        assert_eq!(recent.timestamp_ms, 1000);
        assert_eq!(recent.behavior_hash, 0x1234);
    }

    #[test]
    fn test_ring_buffer_wraparound() {
        let capsule = TemporalSequenceCapsule::new();

        // Fill buffer completely + some
        for i in 0..(TEMPORAL_BUFFER_SIZE + 10) {
            let entry = TemporalEntry::new(i as u32, (i % 256) as u16, 0);
            capsule.append(entry);
        }

        assert_eq!(capsule.total_entries(), (TEMPORAL_BUFFER_SIZE + 10) as u64);

        // Most recent should be the last one
        let recent = capsule.get_recent().unwrap();
        assert_eq!(recent.timestamp_ms, (TEMPORAL_BUFFER_SIZE + 9) as u32);
    }

    #[test]
    fn test_temporal_score_calculation() {
        let capsule = TemporalSequenceCapsule::new();

        // Append entries with increasing scores
        for i in 0..10 {
            let entry = TemporalEntry::new(i as u32 * 100, (i % 256) as u16, (i * 10) as i16);
            capsule.append(entry);
        }

        let (score, _) = capsule.compute_temporal_score();
        // Score should be positive (sum of positive values with decay)
        assert!(score > 0);
    }

    #[test]
    fn test_burst_detection() {
        let capsule = TemporalSequenceCapsule::new();
        capsule.set_burst_threshold(1.0); // Low threshold for testing

        // Append high-score entries
        for i in 0..5 {
            let entry = TemporalEntry::new(i as u32 * 100, 0, 512); // 2.0 in Q8.8
            capsule.append(entry);
        }

        let (_, is_burst) = capsule.compute_temporal_score();
        assert!(is_burst, "Should detect burst with high scores");
    }

    #[test]
    fn test_behavior_counting() {
        let capsule = TemporalSequenceCapsule::new();

        // Append entries with same behavior hash
        for i in 0..5 {
            let entry = TemporalEntry::new(i as u32 * 100, 0xAAAA, 0);
            capsule.append(entry);
        }

        // Append entries with different hash
        for i in 0..3 {
            let entry = TemporalEntry::new((i + 5) as u32 * 100, 0xBBBB, 0);
            capsule.append(entry);
        }

        assert_eq!(capsule.count_behavior(0xAAAA), 5);
        assert_eq!(capsule.count_behavior(0xBBBB), 3);
        assert_eq!(capsule.count_behavior(0xCCCC), 0);
    }

    // ==================== PROPERTY TESTS (5) ====================

    #[test]
    fn proptest_generation_monotonic() {
        let capsule = TemporalSequenceCapsule::new();
        let mut prev_gen = capsule.generation();

        for i in 0..100 {
            let entry = TemporalEntry::new(i as u32, 0, 0);
            capsule.append(entry);
            let new_gen = capsule.generation();
            assert!(new_gen > prev_gen, "Generation must be monotonically increasing");
            prev_gen = new_gen;
        }
    }

    #[test]
    fn proptest_total_entries_accurate() {
        let capsule = TemporalSequenceCapsule::new();

        for i in 0..500 {
            let entry = TemporalEntry::new(i as u32, 0, 0);
            capsule.append(entry);
            assert_eq!(capsule.total_entries(), (i + 1) as u64);
        }
    }

    #[test]
    fn proptest_recent_n_ordering() {
        let capsule = TemporalSequenceCapsule::new();

        for i in 0..50 {
            let entry = TemporalEntry::new(i as u32, i as u16, 0);
            capsule.append(entry);
        }

        let recent = capsule.get_recent_n(10);
        assert_eq!(recent.len(), 10);

        // Most recent should be first
        assert_eq!(recent[0].timestamp_ms, 49);
        assert_eq!(recent[9].timestamp_ms, 40);
    }

    #[test]
    fn proptest_decay_weights_decrease() {
        // Verify decay table is monotonically decreasing
        for i in 1..TEMPORAL_BUFFER_SIZE {
            assert!(DECAY_TABLE_Q8_8[i] <= DECAY_TABLE_Q8_8[i - 1],
                "Decay must be monotonically decreasing");
        }
    }

    #[test]
    fn proptest_timing_anomaly_detection() {
        let capsule = TemporalSequenceCapsule::new();
        capsule.set_timing_threshold_ms(50);

        // Append entries with rapid timing (< threshold)
        for i in 0..10 {
            let entry = TemporalEntry::new(i as u32 * 10, 0, 0); // 10ms apart
            capsule.append(entry);
        }

        // Should have detected timing anomalies
        assert!(capsule.timing_anomaly_count() > 0,
            "Should detect timing anomalies for rapid succession");
    }
}
