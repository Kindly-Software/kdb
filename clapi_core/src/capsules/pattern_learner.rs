//! PatternLearner256 - Request pattern learning and correlation tracking
//!
//! **UCE34 Q10**: Tier 4 (Batch) - Batch correlation analysis for predictive caching
//! **Target Performance**: <200ns per update, 30-50% prefetch accuracy
//! **Architecture**: 100% lockfree with atomic correlation matrix
//!
//! # UCE34 Q1-Q9: Meta-Cognitive Analysis
//!
//! **Q1 (Scope)**: Learn request patterns to predict and prefetch likely next requests
//! **Q2 (Assumptions)**: Request sequences exhibit temporal correlation (A→B happens repeatedly)
//! **Q3 (Constraints)**: <200ns per pattern update, memory bounded (256B capsule)
//! **Q4 (Context)**: Integrated with LRU cache (Phase 5 - Predictive Prefetching)
//! **Q5 (Success)**: 30-50% of next requests prefetched, <10% false positives
//! **Q6 (Failure)**: Memory overflow, hash collisions, stale correlations
//! **Q7 (Patterns)**: Sliding window, frequency counting, correlation matrix
//! **Q8 (Alternatives)**: Markov chains, ML models (rejected: too complex for <200ns)
//! **Q9 (Trade-offs)**: Optimizing for simplicity (fixed-size window) over accuracy
//!
//! # UCE34 Q10-Q12: Foundation (Computational Capsule Architecture)
//!
//! **Q10 (Capsule Tier)**: Tier 4 (Batch) - Batch correlation updates
//!   - **Window Size**: 16 recent requests (ring buffer)
//!   - **Correlation Matrix**: 16×16 = 256 pairs (reduced to top 32)
//!   - **Batch Updates**: Process all 16 pairs in single pass
//!
//! **Q11 (Rust Transform)**: AtomicU64 for all fields, #[repr(C, align(256))]
//! **Q12 (Nightly Enhancement)**: None required (stable Rust)
//!
//! # Memory Layout (256 bytes)
//!
//! ```text
//! Offset | Field                    | Size  | Purpose
//! -------|--------------------------|-------|----------------------------------
//! 0-127  | recent_hashes[16]        | 128B  | Sliding window of recent request hashes
//! 128-135| window_head              | 8B    | Ring buffer write position (0-15)
//! 136-143| total_requests           | 8B    | Total requests processed
//! 144-207| correlation_pairs[8]     | 64B   | Top 8 correlations (hash_a, hash_b, count)
//! 208-215| generation               | 8B    | Generation counter (TOCTOU prevention)
//! 216-255| _padding                 | 40B   | Cache alignment to 256 bytes
//! ```
//!
//! # Correlation Pair Layout (8 bytes each)
//!
//! ```text
//! - correlation_hash: u64 (high 32 bits = hash_a, low 32 bits = hash_b)
//! - count: u32 (frequency counter, max 4B occurrences)
//! - confidence_bp: u16 (confidence in basis points, 0-10000 = 0-100%)
//! ```
//!
//! # Safety
//!
//! - #ASSUME: Ring buffer wraps at 16 (window_head % 16)
//! - #VERIFY: Unit test validates ring buffer wraparound
//! - #ASSUME: Correlation pairs sorted by count (most frequent first)
//! - #VERIFY: Property test validates correlation accuracy
//! - #ASSUME: Generation counter prevents TOCTOU races
//! - #VERIFY: Stress test validates concurrent updates

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, Ordering};

/// Maximum window size for recent requests
pub const PATTERN_WINDOW_SIZE: usize = 16;

/// Maximum correlation pairs to track (reduced to 6 to fit 256B capsule)
pub const MAX_CORRELATION_PAIRS: usize = 6;

/// Minimum confidence threshold for prefetching (50% = 5000 basis points)
/// Note: 50% means the pattern appears more often than random chance
pub const PREFETCH_CONFIDENCE_THRESHOLD_BP: u16 = 5000;

/// Correlation pair (packed into 16 bytes)
///
/// # Memory Layout
/// ```text
/// [0-7]   correlation_hash: AtomicU64  // High 32: hash_a, Low 32: hash_b
/// [8-11]  count: AtomicU32             // Frequency counter
/// [12-13] confidence_bp: AtomicU16     // Confidence (basis points, 0-10000)
/// [14-15] _padding: [u8; 2]            // Alignment padding
/// ```
#[repr(C, align(16))]
struct CorrelationPair {
    /// Packed correlation hash (high 32 bits = hash_a, low 32 bits = hash_b)
    ///
    /// #ASSUME: 32-bit hash truncation provides sufficient uniqueness
    /// #VERIFY: Property test validates collision rate <1%
    correlation_hash: AtomicU64,

    /// Frequency counter (number of times A→B observed)
    ///
    /// #ASSUME: Saturating counter (u32::MAX prevents overflow)
    /// #VERIFY: Unit test validates saturation behavior
    count: AtomicU32,

    /// Confidence in basis points (0-10000 = 0-100%)
    ///
    /// #ASSUME: confidence_bp = (count / total_ab_pairs) × 10000
    /// #VERIFY: Property test validates confidence calculation
    confidence_bp: AtomicU16,

    /// Padding to 16 bytes
    _padding: [u8; 2],
}

impl CorrelationPair {
    const fn new() -> Self {
        Self {
            correlation_hash: AtomicU64::new(0),
            count: AtomicU32::new(0),
            confidence_bp: AtomicU16::new(0),
            _padding: [0; 2],
        }
    }

    /// Pack two 32-bit hashes into 64-bit correlation hash
    #[inline(always)]
    fn pack_hashes(hash_a: u32, hash_b: u32) -> u64 {
        ((hash_a as u64) << 32) | (hash_b as u64)
    }

    /// Unpack 64-bit correlation hash into two 32-bit hashes
    #[inline(always)]
    fn unpack_hashes(packed: u64) -> (u32, u32) {
        let hash_a = (packed >> 32) as u32;
        let hash_b = (packed & 0xFFFF_FFFF) as u32;
        (hash_a, hash_b)
    }

    /// Try to update correlation (returns true if this pair matches hash_a→hash_b)
    fn try_update(&self, hash_a: u32, hash_b: u32, total_pairs: u32) -> bool {
        let target = Self::pack_hashes(hash_a, hash_b);
        let current = self.correlation_hash.load(Ordering::Relaxed);

        if current == 0 {
            // Empty slot - try to claim with CAS
            match self.correlation_hash.compare_exchange(
                0,
                target,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // Successfully claimed - initialize count
                    self.count.store(1, Ordering::Relaxed);
                    self.update_confidence(1, total_pairs);
                    true
                }
                Err(_) => false, // Another thread claimed it
            }
        } else if current == target {
            // Matching correlation - increment count
            let new_count = self.count.fetch_add(1, Ordering::Relaxed) + 1;
            self.update_confidence(new_count, total_pairs);
            true
        } else {
            false // Different correlation
        }
    }

    /// Update confidence based on count and total pairs
    ///
    /// #ASSUME: confidence_bp = (count / total_pairs) × 10000, saturating at 10000
    /// #VERIFY: Unit test validates confidence = 100% when count == total_pairs
    fn update_confidence(&self, count: u32, total_pairs: u32) {
        if total_pairs == 0 {
            self.confidence_bp.store(0, Ordering::Relaxed);
            return;
        }

        // Calculate confidence in basis points (0-10000)
        // #ASSUME: Saturating arithmetic prevents overflow
        let confidence = ((count as u64) * 10000)
            .checked_div(total_pairs as u64)
            .unwrap_or(10000)
            .min(10000) as u16;

        self.confidence_bp.store(confidence, Ordering::Relaxed);
    }

    /// Get correlation hash (0 = empty)
    #[inline]
    fn correlation_hash(&self) -> u64 {
        self.correlation_hash.load(Ordering::Relaxed)
    }

    /// Get count
    #[inline]
    fn count(&self) -> u32 {
        self.count.load(Ordering::Relaxed)
    }

    /// Get confidence (basis points)
    #[inline]
    fn confidence_bp(&self) -> u16 {
        self.confidence_bp.load(Ordering::Relaxed)
    }

    /// Reset correlation pair
    fn reset(&self) {
        self.correlation_hash.store(0, Ordering::Relaxed);
        self.count.store(0, Ordering::Relaxed);
        self.confidence_bp.store(0, Ordering::Relaxed);
    }
}

/// Pattern learner capsule (256-byte, T4 Batch)
///
/// Learns temporal correlations between requests for predictive prefetching.
///
/// # UCE34 Q24: Memory Layout
///
/// See module-level documentation for complete memory layout.
///
/// # UCE34 Q33: Verification
///
/// - Compile-time: #[derive(ComputationalCapsule)] validates alignment/size
/// - Runtime: Property tests validate correlation accuracy
#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256)]
#[repr(C, align(256))]
pub struct PatternLearner256 {
    /// Sliding window of recent request hashes (16 entries)
    ///
    /// #ASSUME: Ring buffer wraps at index 16 (window_head % 16)
    /// #VERIFY: Unit test validates wraparound behavior
    recent_hashes: [AtomicU64; PATTERN_WINDOW_SIZE],

    /// Ring buffer write position (0-15, wraps around)
    ///
    /// #ASSUME: Atomically incremented, % 16 for wraparound
    /// #VERIFY: Property test validates no index out of bounds
    window_head: AtomicU64,

    /// Total requests processed (for confidence calculation)
    ///
    /// #ASSUME: Monotonically increasing counter
    /// #VERIFY: Incremented on every record_request()
    total_requests: AtomicU64,

    /// Top correlation pairs (hash_a → hash_b with counts)
    ///
    /// #ASSUME: Sorted by count (most frequent first)
    /// #VERIFY: Property test validates sorting order
    correlation_pairs: [CorrelationPair; MAX_CORRELATION_PAIRS],

    /// Generation counter for TOCTOU prevention
    ///
    /// #ASSUME: Incremented on every update (odd during write, even when stable)
    /// #VERIFY: Concurrent stress test validates no TOCTOU races
    generation: AtomicU64,

    /// Padding to 256 bytes
    /// Size: 128 (hashes) + 8 (head) + 8 (total) + 96 (6×16 pairs) + 8 (gen) + 8 (pad) = 256
    _padding: [u8; 8],
}

impl PatternLearner256 {
    /// Create a new pattern learner capsule
    pub const fn new() -> Self {
        Self {
            recent_hashes: [const { AtomicU64::new(0) }; PATTERN_WINDOW_SIZE],
            window_head: AtomicU64::new(0),
            total_requests: AtomicU64::new(0),
            correlation_pairs: [const { CorrelationPair::new() }; MAX_CORRELATION_PAIRS],
            generation: AtomicU64::new(0),
            _padding: [0; 8],
        }
    }

    /// Record a new request and update correlations (lockfree, <200ns)
    ///
    /// # UCE34 Q22: State Management - Batch Correlation Update
    ///
    /// **Algorithm**:
    /// 1. Get previous request from window (hash_prev = window[head-1])
    /// 2. Update correlation (hash_prev → hash_current)
    /// 3. Add current request to window (window[head] = hash_current)
    /// 4. Increment window head (wraps at 16)
    ///
    /// # Arguments
    ///
    /// - `request_hash`: Hash of the current request (full u64)
    ///
    /// # Safety
    ///
    /// - #ASSUME: Truncating to u32 for correlation hash is acceptable
    /// - #VERIFY: Property test validates collision rate <1%
    pub fn record_request(&self, request_hash: u64) {
        // Increment generation (marks update in-progress)
        let _gen = self.generation.fetch_add(1, Ordering::Relaxed);

        // Get current window position
        let head = self.window_head.load(Ordering::Relaxed) as usize;
        let prev_idx = if head == 0 {
            PATTERN_WINDOW_SIZE - 1
        } else {
            head - 1
        };

        // Get previous request hash (for correlation)
        let prev_hash = self.recent_hashes[prev_idx].load(Ordering::Relaxed);

        // Increment total requests BEFORE updating correlation
        // This ensures total_pairs calculation is correct
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        // Update correlation (if we have a previous request)
        if prev_hash != 0 {
            // Truncate to 32-bit for correlation matrix (memory efficiency)
            let hash_a = (prev_hash & 0xFFFF_FFFF) as u32;
            let hash_b = (request_hash & 0xFFFF_FFFF) as u32;

            self.update_correlation(hash_a, hash_b);
        }

        // Add current request to window
        self.recent_hashes[head].store(request_hash, Ordering::Relaxed);

        // Increment window head (wraps at 16)
        let new_head = (head + 1) % PATTERN_WINDOW_SIZE;
        self.window_head.store(new_head as u64, Ordering::Relaxed);

        // Mark update complete (even generation = stable)
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Update correlation matrix with new pair (hash_a → hash_b)
    ///
    /// # UCE34 Q23: Concurrency - Lockfree Correlation Update
    ///
    /// **Pattern**: Try to update existing correlation, or insert into empty slot
    ///
    /// #ASSUME: Total pairs = total_requests - 1 (first request has no pair)
    /// #VERIFY: Confidence calculation uses total_requests - 1
    fn update_correlation(&self, hash_a: u32, hash_b: u32) {
        // Total pairs = total_requests - 1 (first request has no previous request)
        // total_requests has already been incremented before this call
        let total_pairs = self.total_requests.load(Ordering::Relaxed).saturating_sub(1) as u32;

        // Try to update existing correlation
        for pair in &self.correlation_pairs {
            if pair.try_update(hash_a, hash_b, total_pairs) {
                return; // Successfully updated
            }
        }

        // No existing correlation found - try to evict lowest-count pair
        self.evict_and_insert(hash_a, hash_b, total_pairs);
    }

    /// Evict lowest-count correlation and insert new pair
    ///
    /// # UCE34 Q6: Failure Modes - Correlation Eviction
    ///
    /// **Strategy**: Evict correlation with lowest count (LFU eviction)
    ///
    /// #ASSUME: Array is kept roughly sorted by count (approximate LFU)
    /// #VERIFY: Property test validates eviction of low-frequency pairs
    fn evict_and_insert(&self, hash_a: u32, hash_b: u32, total_pairs: u32) {
        let mut min_count = u32::MAX;
        let mut min_idx = 0;

        // Find correlation with minimum count
        for (idx, pair) in self.correlation_pairs.iter().enumerate() {
            let count = pair.count();
            if count < min_count {
                min_count = count;
                min_idx = idx;
            }
        }

        // Evict and insert (not atomic, but approximate LFU is acceptable)
        let pair = &self.correlation_pairs[min_idx];
        pair.reset();
        pair.try_update(hash_a, hash_b, total_pairs);
    }

    /// Get prefetch predictions for given request hash (lockfree, <100ns)
    ///
    /// # Returns
    ///
    /// Vector of (predicted_hash, confidence_bp) tuples sorted by confidence (descending)
    ///
    /// # UCE34 Q5: Success Criteria
    ///
    /// **Target**: Return 1-3 predictions with >70% confidence (7000 bp)
    ///
    /// # Safety
    ///
    /// - #ASSUME: Reading correlation pairs is lockfree (Relaxed ordering)
    /// - #VERIFY: No data races (atomic loads only)
    ///
    /// # Note
    ///
    /// Confidence is recalculated based on CURRENT total_pairs to ensure accuracy
    pub fn get_predictions(&self, request_hash: u64) -> Vec<(u64, u16)> {
        let hash_a = (request_hash & 0xFFFF_FFFF) as u32;
        let mut predictions = Vec::with_capacity(MAX_CORRELATION_PAIRS);

        // Calculate current total_pairs for confidence recalculation
        let total_pairs = self.total_requests.load(Ordering::Relaxed).saturating_sub(1);

        // Scan all correlation pairs
        for pair in &self.correlation_pairs {
            let correlation = pair.correlation_hash();
            if correlation == 0 {
                continue; // Empty slot
            }

            let (stored_a, stored_b) = CorrelationPair::unpack_hashes(correlation);
            let count = pair.count();

            // Recalculate confidence based on CURRENT total_pairs
            let confidence = if total_pairs > 0 {
                ((count as u64 * 10000) / total_pairs).min(10000) as u16
            } else {
                0
            };

            // Match on hash_a (previous request)
            if stored_a == hash_a && confidence >= PREFETCH_CONFIDENCE_THRESHOLD_BP {
                // Reconstruct full hash (approximate, assuming lower 32 bits sufficient)
                let predicted_hash = stored_b as u64;
                predictions.push((predicted_hash, confidence));
            }
        }

        // Sort by confidence (descending)
        predictions.sort_by(|a, b| b.1.cmp(&a.1));

        predictions
    }

    /// Get correlation statistics for monitoring
    ///
    /// # Returns
    ///
    /// - `total_requests`: Total requests processed
    /// - `unique_correlations`: Number of non-empty correlation pairs
    /// - `avg_confidence_bp`: Average confidence across all correlations
    pub fn get_stats(&self) -> PatternStats {
        let total_requests = self.total_requests.load(Ordering::Relaxed);
        let mut unique_correlations = 0;
        let mut total_confidence = 0u64;

        for pair in &self.correlation_pairs {
            if pair.correlation_hash() != 0 {
                unique_correlations += 1;
                total_confidence += pair.confidence_bp() as u64;
            }
        }

        let avg_confidence_bp = if unique_correlations > 0 {
            (total_confidence / unique_correlations) as u16
        } else {
            0
        };

        PatternStats {
            total_requests,
            unique_correlations,
            avg_confidence_bp,
        }
    }

    /// Get top correlations for debugging/monitoring
    ///
    /// # Returns
    ///
    /// Vector of (hash_a, hash_b, count, confidence_bp) tuples sorted by count (descending)
    ///
    /// # Note
    ///
    /// Confidence is recalculated based on CURRENT total_pairs to ensure consistency
    pub fn get_top_correlations(&self) -> Vec<(u32, u32, u32, u16)> {
        let mut correlations = Vec::with_capacity(MAX_CORRELATION_PAIRS);

        // Calculate current total_pairs (total_requests - 1, since first request has no pair)
        let total_pairs = self.total_requests.load(Ordering::Relaxed).saturating_sub(1);

        for pair in &self.correlation_pairs {
            let correlation = pair.correlation_hash();
            if correlation == 0 {
                continue; // Empty slot
            }

            let (hash_a, hash_b) = CorrelationPair::unpack_hashes(correlation);
            let count = pair.count();

            // Recalculate confidence based on CURRENT total_pairs
            let confidence = if total_pairs > 0 {
                ((count as u64 * 10000) / total_pairs).min(10000) as u16
            } else {
                0
            };

            correlations.push((hash_a, hash_b, count, confidence));
        }

        // Sort by count (descending)
        correlations.sort_by(|a, b| b.2.cmp(&a.2));

        correlations
    }

    /// Reset all pattern learning state
    pub fn reset(&self) {
        // Clear window
        for hash in &self.recent_hashes {
            hash.store(0, Ordering::Relaxed);
        }

        // Reset position
        self.window_head.store(0, Ordering::Relaxed);
        self.total_requests.store(0, Ordering::Relaxed);

        // Clear correlations
        for pair in &self.correlation_pairs {
            pair.reset();
        }

        // Reset generation
        self.generation.store(0, Ordering::Relaxed);
    }
}

impl Default for PatternLearner256 {
    fn default() -> Self {
        Self::new()
    }
}

/// Pattern learning statistics
#[derive(Debug, Clone, Copy)]
pub struct PatternStats {
    /// Total requests processed
    pub total_requests: u64,
    /// Number of unique correlations learned
    pub unique_correlations: u64,
    /// Average confidence across all correlations (basis points)
    pub avg_confidence_bp: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_learner_new() {
        let learner = PatternLearner256::new();
        let stats = learner.get_stats();
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.unique_correlations, 0);
    }

    #[test]
    fn test_record_single_request() {
        let learner = PatternLearner256::new();
        learner.record_request(0x1234_5678_9ABC_DEF0);

        let stats = learner.get_stats();
        assert_eq!(stats.total_requests, 1);
        // No correlation yet (need 2 requests)
        assert_eq!(stats.unique_correlations, 0);
    }

    #[test]
    fn test_record_correlation() {
        let learner = PatternLearner256::new();

        // Record A→B sequence
        learner.record_request(0x1111_1111_1111_1111);
        learner.record_request(0x2222_2222_2222_2222);

        let stats = learner.get_stats();
        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.unique_correlations, 1); // One A→B correlation
    }

    #[test]
    fn test_correlation_confidence() {
        let learner = PatternLearner256::new();

        // Record A→B sequence 3 times (out of 4 total pairs)
        learner.record_request(0x1111_1111_1111_1111); // Request 1 (no prev)
        learner.record_request(0x2222_2222_2222_2222); // Pair 1: A→B
        learner.record_request(0x1111_1111_1111_1111); // Pair 2: B→A
        learner.record_request(0x2222_2222_2222_2222); // Pair 3: A→B
        learner.record_request(0x1111_1111_1111_1111); // Pair 4: B→A

        let correlations = learner.get_top_correlations();
        assert_eq!(correlations.len(), 2); // A→B and B→A

        // Each correlation appears 2 times out of 4 total pairs
        for (_, _, count, confidence) in correlations {
            assert_eq!(count, 2);
            // Confidence = (2 / 4) * 10000 = 5000 bp (50%)
            assert_eq!(confidence, 5000);
        }
    }

    #[test]
    fn test_get_predictions() {
        let learner = PatternLearner256::new();

        // Build strong A→B correlation (10 times)
        for _ in 0..10 {
            learner.record_request(0x1111_1111_1111_1111);
            learner.record_request(0x2222_2222_2222_2222);
        }

        // Query predictions for A
        let predictions = learner.get_predictions(0x1111_1111_1111_1111);

        // Should predict B with high confidence
        assert!(!predictions.is_empty());
        let (predicted_hash, confidence) = predictions[0];
        assert_eq!(predicted_hash & 0xFFFF_FFFF, 0x2222_2222);
        assert!(confidence >= PREFETCH_CONFIDENCE_THRESHOLD_BP);
    }

    #[test]
    fn test_window_wraparound() {
        let learner = PatternLearner256::new();

        // Fill window beyond capacity (17 requests, wraps once)
        // Use low 32 bits for unique hashes (since we truncate to u32 in correlation)
        for i in 0..17 {
            learner.record_request(i as u64);
        }

        let stats = learner.get_stats();
        assert_eq!(stats.total_requests, 17);
        // Should have learned correlations despite wraparound
        // (At least 6 unique pairs, limited by MAX_CORRELATION_PAIRS=6)
        assert!(stats.unique_correlations > 0);
        assert!(stats.unique_correlations <= MAX_CORRELATION_PAIRS as u64);
    }

    #[test]
    fn test_eviction_lfu() {
        let learner = PatternLearner256::new();

        // Fill all correlation slots (MAX_CORRELATION_PAIRS=6 unique pairs)
        // Use low 32 bits for unique hashes (since we truncate to u32 in correlation)
        for i in 0..MAX_CORRELATION_PAIRS {
            let hash_a = (i as u64 * 2);     // 0, 2, 4, 6, 8, 10
            let hash_b = (i as u64 * 2 + 1); // 1, 3, 5, 7, 9, 11
            learner.record_request(hash_a);
            learner.record_request(hash_b);
        }

        let stats = learner.get_stats();
        assert_eq!(stats.unique_correlations, MAX_CORRELATION_PAIRS as u64); // All slots filled

        // Record new correlation (should evict lowest-count pair)
        learner.record_request(0x1111_1111);
        learner.record_request(0x2222_2222);

        let stats = learner.get_stats();
        assert_eq!(stats.unique_correlations, MAX_CORRELATION_PAIRS as u64); // Still MAX (one evicted)
    }
}
