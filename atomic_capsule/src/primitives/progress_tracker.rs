//! # ProgressTrackerCapsule - Tier 1 Atomic Progress Tracking
//!
//! **Lockfree progress tracking** with <10ns atomic operations for real-time monitoring.
//!
//! ## UCE34 Framework (Tier 1: Atomic)
//!
//! ### Q1-Q9: Problem Analysis
//! - **Q1**: Lockfree progress tracking (current progress, total items)
//! - **Q2**: Traditional approach requires Mutex<(u64, u64)>, 100-500ns overhead
//! - **Q3**: <5ns atomic reads, <10ns atomic increments
//! - **Q4**: Two AtomicU64 fields (current, total)
//! - **Q5**: `ProgressTrackerCapsule` (64-byte aligned)
//! - **Q8**: 64 bytes (single cache line)
//!
//! ### Q10-Q12: Tier Selection
//! - **Q10**: Tier 1 Atomic (pure atomic fields, <100ns operations)
//! - **Q11**: All fields AtomicU64
//! - **Q12**: None (stable Rust)
//!
//! ### Q13-Q27: Implementation Details
//! - **Memory ordering**: Relaxed for increments (advisory, no synchronization needed)
//! - **Memory ordering**: Relaxed for reads (approximate progress acceptable)
//! - **Overflow handling**: Saturating arithmetic (no panic)
//! - **No locks**: 100% lockfree, no panics
//!
//! ### Q33: Verification
//! - Automatic verification via #[derive(ComputationalCapsule)]
//! - Compile-time alignment and size checks
//!
//! ### Q34: Testing & Benchmarking
//! - T28: Unit tests, property tests, stress tests
//! - B32: Benchmarks vs Mutex<(u64, u64)> (honest baselines)
//!
//! ## Performance Targets
//!
//! - `increment()`: <10ns (Relaxed, saturating)
//! - `increment_by(n)`: <10ns (Relaxed, saturating)
//! - `progress()`: <5ns (Relaxed read × 2)
//! - `completed()`: <3ns (Relaxed read)
//! - `total()`: <3ns (Relaxed read)
//! - `is_complete()`: <5ns (Relaxed read × 2)
//!
//! ## Example
//!
//! ```rust
//! use atomic_capsule::primitives::ProgressTrackerCapsule;
//!
//! let tracker = ProgressTrackerCapsule::new(100);
//!
//! // Increment progress (lockfree, <10ns)
//! for _ in 0..50 {
//!     tracker.increment();
//! }
//!
//! // Check progress (lockfree, <5ns)
//! assert_eq!(tracker.progress(), 0.5);
//! assert_eq!(tracker.completed(), 50);
//! assert_eq!(tracker.total(), 100);
//! assert!(!tracker.is_complete());
//!
//! // Complete remaining
//! tracker.increment_by(50);
//! assert!(tracker.is_complete());
//! ```
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_RELAXED_SUFFICIENT`: Relaxed ordering sufficient for advisory progress tracking
//! - `#VERIFY_RELAXED_SUFFICIENT`: No happens-before required, progress is approximate
//! - `#ASSUME_SATURATING_SAFE`: Saturating add prevents overflow panics
//! - `#VERIFY_SATURATING_SAFE`: Property tests verify u64::MAX handling

use crate::alignment::AlignmentTier;
use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

/// Tier 1 Atomic Progress Tracker Capsule (64 bytes, single cache line).
///
/// ## Architecture
///
/// - **Alignment**: 64 bytes (single cache line)
/// - **Size**: 64 bytes
/// - **Tier**: T1 (Atomic)
/// - **Performance**: <10ns increments, <5ns reads
///
/// ## UCE34 Q10: Why Tier 1 (Atomic)?
///
/// - Pure atomic fields (2 × AtomicU64)
/// - No dependencies between fields
/// - <100ns operations
/// - Single cache line (64B)
///
/// ## Memory Layout
///
/// ```text
/// Offset 0-7:   completed (AtomicU64) - current progress counter
/// Offset 8-15:  total (AtomicU64) - total items to process
/// Offset 16-63: _padding (48 bytes) - complete cache line alignment
/// ```
///
/// ## ASSUM Framework
///
/// - `#ASSUME_RELAXED_SUFFICIENT`: Relaxed ordering for advisory progress (no synchronization needed)
/// - `#VERIFY_RELAXED_SUFFICIENT`: Progress tracking is approximate, no happens-before required
/// - `#ASSUME_SATURATING_SAFE`: Saturating arithmetic prevents overflow panics
/// - `#VERIFY_SATURATING_SAFE`: Property tests verify u64::MAX handling
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 64, size = 64))]
#[repr(C, align(64))]
pub struct ProgressTrackerCapsule {
    /// Current progress counter (number of completed items).
    ///
    /// Offset 0-7 (first 8 bytes of cache line)
    completed: AtomicU64,

    /// Total number of items to process.
    ///
    /// Offset 8-15 (second 8 bytes of cache line)
    total: AtomicU64,

    /// Padding to complete 64-byte cache line alignment.
    ///
    /// Offset 16-63 (remaining 48 bytes of cache line)
    _padding: [u8; 48],
}

impl AlignmentTier for ProgressTrackerCapsule {
    const TIER: &'static str = "hot";
    const ALIGNMENT: usize = 64;
}

// Compile-time verification of layout (Q33: Mandatory verification)
#[cfg(not(feature = "derive"))]
crate::verify_capsule_properties!(ProgressTrackerCapsule, 64, 64);

impl ProgressTrackerCapsule {
    /// Create new ProgressTrackerCapsule with specified total items.
    ///
    /// # Arguments
    ///
    /// * `total` - Total number of items to process
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::primitives::ProgressTrackerCapsule;
    ///
    /// let tracker = ProgressTrackerCapsule::new(100);
    /// assert_eq!(tracker.total(), 100);
    /// assert_eq!(tracker.completed(), 0);
    /// assert_eq!(tracker.progress(), 0.0);
    /// ```
    #[inline]
    pub const fn new(total: u64) -> Self {
        Self {
            completed: AtomicU64::new(0),
            total: AtomicU64::new(total),
            _padding: [0u8; 48],
        }
    }

    /// Increment progress by 1 (saturating at u64::MAX).
    ///
    /// # Performance
    ///
    /// - Typical: <10ns (single atomic fetch_add)
    /// - Under contention: <15ns (cache line bouncing)
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_RELAXED_SUFFICIENT`: No synchronization needed, progress is advisory
    /// - `#VERIFY_RELAXED_SUFFICIENT`: Property tests verify correctness
    /// - `#ASSUME_SATURATING_SAFE`: saturating_add prevents overflow panics
    /// - `#VERIFY_SATURATING_SAFE`: Unit tests verify u64::MAX handling
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::primitives::ProgressTrackerCapsule;
    ///
    /// let tracker = ProgressTrackerCapsule::new(10);
    /// tracker.increment();
    /// tracker.increment();
    /// assert_eq!(tracker.completed(), 2);
    /// ```
    #[inline(always)]
    pub fn increment(&self) {
        // #ASSUME_RELAXED_SUFFICIENT: Progress tracking is advisory, no synchronization needed
        // #VERIFY_RELAXED_SUFFICIENT: No happens-before required for progress monitoring
        let prev = self.completed.fetch_add(1, Ordering::Relaxed);

        // #ASSUME_SATURATING_SAFE: Detect overflow and saturate at u64::MAX
        // #VERIFY_SATURATING_SAFE: Property tests verify overflow handling
        if prev == u64::MAX {
            // Saturate at u64::MAX (undo the wrap-around)
            self.completed.store(u64::MAX, Ordering::Relaxed);
        }
    }

    /// Increment progress by n (saturating at u64::MAX).
    ///
    /// # Performance
    ///
    /// - Typical: <10ns (single atomic fetch_add)
    /// - Under contention: <15ns (cache line bouncing)
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_RELAXED_SUFFICIENT`: No synchronization needed, progress is advisory
    /// - `#VERIFY_RELAXED_SUFFICIENT`: Property tests verify correctness
    /// - `#ASSUME_SATURATING_SAFE`: Saturating add prevents overflow panics
    /// - `#VERIFY_SATURATING_SAFE`: Unit tests verify u64::MAX handling
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::primitives::ProgressTrackerCapsule;
    ///
    /// let tracker = ProgressTrackerCapsule::new(100);
    /// tracker.increment_by(25);
    /// assert_eq!(tracker.completed(), 25);
    /// tracker.increment_by(75);
    /// assert_eq!(tracker.completed(), 100);
    /// ```
    #[inline(always)]
    pub fn increment_by(&self, n: u64) {
        if n == 0 {
            return;
        }

        // #ASSUME_RELAXED_SUFFICIENT: Progress tracking is advisory, no synchronization needed
        // #VERIFY_RELAXED_SUFFICIENT: No happens-before required for progress monitoring
        let prev = self.completed.fetch_add(n, Ordering::Relaxed);

        // #ASSUME_SATURATING_SAFE: Detect overflow and saturate at u64::MAX
        // #VERIFY_SATURATING_SAFE: Property tests verify overflow handling
        if prev > u64::MAX - n {
            // Overflow occurred, saturate at u64::MAX
            self.completed.store(u64::MAX, Ordering::Relaxed);
        }
    }

    /// Get current progress as a fraction (0.0 to 1.0).
    ///
    /// Returns 0.0 if total is 0.
    /// Returns 1.0 if completed >= total.
    ///
    /// # Performance
    ///
    /// - Typical: <5ns (two atomic loads)
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_RELAXED_SUFFICIENT`: Approximate progress acceptable
    /// - `#VERIFY_RELAXED_SUFFICIENT`: No strict consistency required
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::primitives::ProgressTrackerCapsule;
    ///
    /// let tracker = ProgressTrackerCapsule::new(100);
    /// tracker.increment_by(50);
    /// assert_eq!(tracker.progress(), 0.5);
    /// tracker.increment_by(50);
    /// assert_eq!(tracker.progress(), 1.0);
    /// ```
    #[inline]
    pub fn progress(&self) -> f64 {
        // #ASSUME_RELAXED_SUFFICIENT: Approximate progress acceptable for monitoring
        // #VERIFY_RELAXED_SUFFICIENT: Relaxed ordering sufficient for advisory progress
        let completed = self.completed.load(Ordering::Relaxed);
        let total = self.total.load(Ordering::Relaxed);

        if total == 0 {
            0.0
        } else if completed >= total {
            1.0
        } else {
            completed as f64 / total as f64
        }
    }

    /// Get number of completed items.
    ///
    /// # Performance
    ///
    /// - Typical: <3ns (single atomic load)
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::primitives::ProgressTrackerCapsule;
    ///
    /// let tracker = ProgressTrackerCapsule::new(100);
    /// tracker.increment_by(42);
    /// assert_eq!(tracker.completed(), 42);
    /// ```
    #[inline]
    pub fn completed(&self) -> u64 {
        // #ASSUME_RELAXED_SUFFICIENT: Advisory read, no synchronization needed
        // #VERIFY_RELAXED_SUFFICIENT: Approximate value acceptable
        self.completed.load(Ordering::Relaxed)
    }

    /// Get total number of items.
    ///
    /// # Performance
    ///
    /// - Typical: <3ns (single atomic load)
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::primitives::ProgressTrackerCapsule;
    ///
    /// let tracker = ProgressTrackerCapsule::new(100);
    /// assert_eq!(tracker.total(), 100);
    /// ```
    #[inline]
    pub fn total(&self) -> u64 {
        // #ASSUME_RELAXED_SUFFICIENT: Advisory read, no synchronization needed
        // #VERIFY_RELAXED_SUFFICIENT: Approximate value acceptable
        self.total.load(Ordering::Relaxed)
    }

    /// Check if progress is complete (completed >= total).
    ///
    /// # Performance
    ///
    /// - Typical: <5ns (two atomic loads)
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::primitives::ProgressTrackerCapsule;
    ///
    /// let tracker = ProgressTrackerCapsule::new(10);
    /// assert!(!tracker.is_complete());
    /// tracker.increment_by(10);
    /// assert!(tracker.is_complete());
    /// ```
    #[inline]
    pub fn is_complete(&self) -> bool {
        // #ASSUME_RELAXED_SUFFICIENT: Advisory check, no synchronization needed
        // #VERIFY_RELAXED_SUFFICIENT: Approximate comparison acceptable
        let completed = self.completed.load(Ordering::Relaxed);
        let total = self.total.load(Ordering::Relaxed);
        completed >= total
    }

    /// Reset progress to zero (keep same total).
    ///
    /// # Performance
    ///
    /// - Typical: <10ns (single atomic store)
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::primitives::ProgressTrackerCapsule;
    ///
    /// let tracker = ProgressTrackerCapsule::new(100);
    /// tracker.increment_by(50);
    /// assert_eq!(tracker.completed(), 50);
    /// tracker.reset();
    /// assert_eq!(tracker.completed(), 0);
    /// assert_eq!(tracker.total(), 100);
    /// ```
    #[inline]
    pub fn reset(&self) {
        // #ASSUME_RELAXED_SUFFICIENT: Reset is advisory, no synchronization needed
        // #VERIFY_RELAXED_SUFFICIENT: Relaxed ordering sufficient for reset
        self.completed.store(0, Ordering::Relaxed);
    }
}

// ============================================================================
// Unit Tests (T28 Framework: Q1-Q7)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let tracker = ProgressTrackerCapsule::new(100);
        assert_eq!(tracker.total(), 100);
        assert_eq!(tracker.completed(), 0);
        assert_eq!(tracker.progress(), 0.0);
        assert!(!tracker.is_complete());
    }

    #[test]
    fn test_increment() {
        let tracker = ProgressTrackerCapsule::new(10);
        tracker.increment();
        assert_eq!(tracker.completed(), 1);
        tracker.increment();
        assert_eq!(tracker.completed(), 2);
    }

    #[test]
    fn test_increment_by() {
        let tracker = ProgressTrackerCapsule::new(100);
        tracker.increment_by(25);
        assert_eq!(tracker.completed(), 25);
        tracker.increment_by(75);
        assert_eq!(tracker.completed(), 100);
    }

    #[test]
    fn test_progress() {
        let tracker = ProgressTrackerCapsule::new(100);
        assert_eq!(tracker.progress(), 0.0);

        tracker.increment_by(25);
        assert_eq!(tracker.progress(), 0.25);

        tracker.increment_by(25);
        assert_eq!(tracker.progress(), 0.5);

        tracker.increment_by(50);
        assert_eq!(tracker.progress(), 1.0);

        // Over 100% should clamp to 1.0
        tracker.increment_by(10);
        assert_eq!(tracker.progress(), 1.0);
    }

    #[test]
    fn test_is_complete() {
        let tracker = ProgressTrackerCapsule::new(10);
        assert!(!tracker.is_complete());

        tracker.increment_by(9);
        assert!(!tracker.is_complete());

        tracker.increment();
        assert!(tracker.is_complete());

        tracker.increment();
        assert!(tracker.is_complete());
    }

    #[test]
    fn test_reset() {
        let tracker = ProgressTrackerCapsule::new(100);
        tracker.increment_by(50);
        assert_eq!(tracker.completed(), 50);

        tracker.reset();
        assert_eq!(tracker.completed(), 0);
        assert_eq!(tracker.total(), 100);
        assert_eq!(tracker.progress(), 0.0);
    }

    #[test]
    fn test_zero_total() {
        let tracker = ProgressTrackerCapsule::new(0);
        assert_eq!(tracker.progress(), 0.0);
        assert!(tracker.is_complete());
    }

    #[test]
    fn test_saturating_add() {
        let tracker = ProgressTrackerCapsule::new(100);
        // Set to near max
        tracker.increment_by(u64::MAX - 10);
        assert_eq!(tracker.completed(), u64::MAX - 10);

        // Increment beyond max (should saturate)
        tracker.increment_by(20);
        assert_eq!(tracker.completed(), u64::MAX);

        // Further increments should keep it at max
        tracker.increment();
        assert_eq!(tracker.completed(), u64::MAX);
    }

    #[test]
    fn test_alignment() {
        let tracker = ProgressTrackerCapsule::new(100);
        let ptr = &tracker as *const ProgressTrackerCapsule as usize;
        assert_eq!(
            ptr % 64,
            0,
            "ProgressTrackerCapsule must be 64-byte aligned"
        );
    }

    #[test]
    fn test_size() {
        assert_eq!(
            core::mem::size_of::<ProgressTrackerCapsule>(),
            64,
            "ProgressTrackerCapsule must be exactly 64 bytes"
        );
    }

    // T28 Q2-Q7: Property Tests (concurrent correctness)
    #[cfg(all(test, not(miri)))]
    mod property_tests {
        use super::*;
        use std::sync::Arc;
        use std::thread;

        #[test]
        fn test_concurrent_increments() {
            const THREADS: usize = 8;
            const INCREMENTS_PER_THREAD: u64 = 1000;
            const EXPECTED_TOTAL: u64 = THREADS as u64 * INCREMENTS_PER_THREAD;

            let tracker = Arc::new(ProgressTrackerCapsule::new(EXPECTED_TOTAL));
            let mut handles: Vec<std::thread::JoinHandle<()>> = vec![];

            for _ in 0..THREADS {
                let tracker_clone = Arc::clone(&tracker);
                handles.push(thread::spawn(move || {
                    for _ in 0..INCREMENTS_PER_THREAD {
                        tracker_clone.increment();
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }

            assert_eq!(tracker.completed(), EXPECTED_TOTAL);
            assert!(tracker.is_complete());
        }

        #[test]
        fn test_concurrent_increment_by() {
            const THREADS: usize = 4;
            const BATCH_SIZE: u64 = 250;
            const EXPECTED_TOTAL: u64 = THREADS as u64 * BATCH_SIZE;

            let tracker = Arc::new(ProgressTrackerCapsule::new(EXPECTED_TOTAL));
            let mut handles: Vec<std::thread::JoinHandle<()>> = vec![];

            for _ in 0..THREADS {
                let tracker_clone = Arc::clone(&tracker);
                handles.push(thread::spawn(move || {
                    tracker_clone.increment_by(BATCH_SIZE);
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }

            assert_eq!(tracker.completed(), EXPECTED_TOTAL);
            assert!(tracker.is_complete());
        }
    }
}
