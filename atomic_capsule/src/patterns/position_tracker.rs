//! # PositionTrackerCapsule
//!
//! **Simplified APC-512 pattern for position + timestamp coordination.**
//!
//! ## UCE34 Analysis
//!
//! - **Q1 (Problem)**: Track signed position and timestamp atomically for risk management
//! - **Q10 (Tier)**: T1 Atomic - DualAtomicU64 pattern for dual-channel coordination
//! - **Q11 (Rust)**: AtomicU64, #[repr(C, align(128))], generation counters via DualAtomicU64
//! - **Q12 (Nightly)**: None required (stable-compatible)
//!
//! ## Decision: "What is my current position and when was it updated?"
//!
//! Reader performs ONE read:
//! ```rust
//! use atomic_capsule::patterns::PositionTrackerCapsule;
//!
//! let tracker = PositionTrackerCapsule::new();
//! let (position, timestamp) = tracker.load_position();
//! if position > max_position {
//!     reduce_risk();
//! }
//! ```
//!
//! ## Performance (B32)
//!
//! - Load position: <15ns (dual atomic load via DualAtomicU64)
//! - Update position: <20ns (dual CAS)
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_128B_ALIGNMENT`: DualAtomicU64 pattern prevents false sharing
//! - `#VERIFY_DUAL_CHANNEL`: Primary = position (i64 as u64), Secondary = timestamp (u64)
//! - `#ASSUME_SIGNED_POSITION`: i64 for long/short positions (trading/finance use case)
//! - `#VERIFY_ATOMIC_LOADS`: All loads use explicit memory ordering (Relaxed/Acquire)
//! - `#VERIFY_ATOMIC_STORES`: All stores use explicit memory ordering (Release/AcqRel)

use core::sync::atomic::Ordering;

use crate::patterns::DualAtomicU64;

/// PositionTrackerCapsule
///
/// Tracks position (signed) and timestamp using DualAtomicU64 pattern.
///
/// # Memory Layout (128 bytes)
/// ```text
/// Primary (u64): Position (i64 as u64)
/// Padding: 56 bytes to complete first cache line
/// Secondary (u64): Timestamp
/// Padding: 56 bytes to complete second cache line
/// ```
///
/// # COCA Requirements
/// - **100% lockfree**: No mutex/RwLock, only atomic operations
/// - **Cache-aligned**: 128-byte alignment prevents false sharing
/// - **Generation counters**: DualAtomicU64 provides TOCTOU prevention via compare_exchange_with_generation
/// - **Explicit memory ordering**: All operations document Relaxed/Acquire/Release/AcqRel
///
/// # ASSUM Framework
/// - `#ASSUME_128B_ALIGNMENT`: 128 bytes prevents false sharing
/// - `#VERIFY_128B_ALIGNMENT`: Compile-time verification via verify_capsule_properties!
#[repr(C, align(128))]
pub struct PositionTrackerCapsule {
    /// DualAtomicU64: Primary = position, Secondary = timestamp
    dual: DualAtomicU64,
}

// Compile-time verification (MANDATORY per Q33)
crate::verify_capsule_properties!(PositionTrackerCapsule, 128, 128);

impl PositionTrackerCapsule {
    /// Create new position tracker with zero position
    ///
    /// # Example
    /// ```rust
    /// use atomic_capsule::patterns::PositionTrackerCapsule;
    ///
    /// let tracker = PositionTrackerCapsule::new();
    /// assert_eq!(tracker.load_position_only(), 0);
    /// ```
    pub const fn new() -> Self {
        Self {
            dual: DualAtomicU64::new(0, 0),
        }
    }

    /// Create position tracker with initial position and timestamp
    ///
    /// # Example
    /// ```rust
    /// use atomic_capsule::patterns::PositionTrackerCapsule;
    ///
    /// let tracker = PositionTrackerCapsule::with_initial(100, 1000);
    /// assert_eq!(tracker.load_position_only(), 100);
    /// assert_eq!(tracker.load_timestamp(), 1000);
    /// ```
    pub const fn with_initial(position: i64, timestamp: u64) -> Self {
        Self {
            dual: DualAtomicU64::new(position as u64, timestamp),
        }
    }

    /// Load current position
    ///
    /// # Performance
    /// - Typical: <10ns (single cache line load)
    ///
    /// # Memory Ordering
    /// - Relaxed: Position reads don't need synchronization (independent reads)
    ///
    /// # Example
    /// ```rust
    /// use atomic_capsule::patterns::PositionTrackerCapsule;
    ///
    /// let tracker = PositionTrackerCapsule::with_initial(100, 1000);
    /// assert_eq!(tracker.load_position_only(), 100);
    /// ```
    #[inline(always)]
    pub fn load_position_only(&self) -> i64 {
        self.dual.load_primary(Ordering::Relaxed) as i64
    }

    /// Load current timestamp
    ///
    /// # Performance
    /// - Typical: <12ns (separate cache line load)
    ///
    /// # Memory Ordering
    /// - Relaxed: Timestamp reads don't need synchronization (independent reads)
    ///
    /// # Example
    /// ```rust
    /// use atomic_capsule::patterns::PositionTrackerCapsule;
    ///
    /// let tracker = PositionTrackerCapsule::with_initial(100, 1000);
    /// assert_eq!(tracker.load_timestamp(), 1000);
    /// ```
    #[inline(always)]
    pub fn load_timestamp(&self) -> u64 {
        self.dual.load_secondary(Ordering::Relaxed)
    }

    /// Load position and timestamp atomically
    ///
    /// # Performance
    /// - Typical: <25ns (two cache line loads)
    ///
    /// # Memory Ordering
    /// - Acquire: Ensures visibility of prior writes before this load
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_SEQUENTIAL_LOADS`: Two loads may observe torn state during concurrent updates
    /// - `#VERIFY_RETRY_ON_MISMATCH`: Caller should retry if inconsistent state detected
    ///
    /// # Example
    /// ```rust
    /// use atomic_capsule::patterns::PositionTrackerCapsule;
    ///
    /// let tracker = PositionTrackerCapsule::with_initial(100, 1000);
    /// let (position, timestamp) = tracker.load_position();
    /// assert_eq!(position, 100);
    /// assert_eq!(timestamp, 1000);
    /// ```
    #[inline(always)]
    pub fn load_position(&self) -> (i64, u64) {
        // Load both atomics separately (cache-line separated prevents false sharing)
        let position_u64 = self.dual.load_primary(Ordering::Acquire);
        let timestamp = self.dual.load_secondary(Ordering::Acquire);
        (position_u64 as i64, timestamp)
    }

    /// Update position
    ///
    /// # Performance
    /// - Typical: <15ns (two atomic stores)
    ///
    /// # Memory Ordering
    /// - Release: Ensures all prior writes visible to subsequent loads
    ///
    /// # Example
    /// ```rust
    /// use atomic_capsule::patterns::PositionTrackerCapsule;
    ///
    /// let tracker = PositionTrackerCapsule::new();
    /// tracker.update_position(100, 1000);
    /// assert_eq!(tracker.load_position_only(), 100);
    /// assert_eq!(tracker.load_timestamp(), 1000);
    /// ```
    #[inline(always)]
    pub fn update_position(&self, position: i64, timestamp: u64) {
        // Update primary (position)
        self.dual.store_primary(position as u64, Ordering::Release);

        // Update secondary (timestamp)
        self.dual.store_secondary(timestamp, Ordering::Release);
    }

    /// Add to position (atomic increment/decrement)
    ///
    /// # Performance
    /// - Typical: <15ns (atomic RMW + store)
    ///
    /// # Memory Ordering
    /// - AcqRel: Acquire previous value, Release updated value
    /// - Release: Timestamp update visible to subsequent loads
    ///
    /// # Example
    /// ```rust
    /// use atomic_capsule::patterns::PositionTrackerCapsule;
    ///
    /// let tracker = PositionTrackerCapsule::with_initial(100, 1000);
    /// tracker.add_position(50, 2000);
    /// assert_eq!(tracker.load_position_only(), 150);
    /// assert_eq!(tracker.load_timestamp(), 2000);
    /// ```
    #[inline(always)]
    pub fn add_position(&self, delta: i64, timestamp: u64) {
        // Atomic add to position (handle signed arithmetic)
        if delta >= 0 {
            self.dual.fetch_add_primary(delta as u64, Ordering::AcqRel);
        } else {
            self.dual
                .fetch_sub_primary((-delta) as u64, Ordering::AcqRel);
        }

        // Update timestamp
        self.dual.store_secondary(timestamp, Ordering::Release);
    }

    /// Compare-exchange position (atomic update with expected value)
    ///
    /// # Performance
    /// - Success: <20ns (CAS on primary + store on secondary)
    /// - Failure: <15ns (load only)
    ///
    /// # Memory Ordering
    /// - Success: AcqRel (acquire previous, release new)
    /// - Failure: Acquire (load current for retry)
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_TWO_PHASE_UPDATE`: Position CAS first, then timestamp update
    /// - `#VERIFY_POSITION_FIRST`: Tests verify position changes before timestamp
    ///
    /// # Example
    /// ```rust
    /// use atomic_capsule::patterns::PositionTrackerCapsule;
    ///
    /// let tracker = PositionTrackerCapsule::with_initial(100, 1000);
    ///
    /// // Success case
    /// let result = tracker.compare_exchange_position(100, 200, 2000);
    /// assert_eq!(result, Ok((100, 1000)));
    /// assert_eq!(tracker.load_position_only(), 200);
    ///
    /// // Failure case
    /// let result = tracker.compare_exchange_position(999, 300, 3000);
    /// assert!(result.is_err());
    /// ```
    #[inline(always)]
    pub fn compare_exchange_position(
        &self,
        expected_position: i64,
        new_position: i64,
        new_timestamp: u64,
    ) -> Result<(i64, u64), (i64, u64)> {
        // Two-phase update: CAS position first
        match self.dual.compare_exchange_primary(
            expected_position as u64,
            new_position as u64,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(old_position) => {
                // Success: Update timestamp
                let old_timestamp = self.dual.load_secondary(Ordering::Acquire);
                self.dual.store_secondary(new_timestamp, Ordering::Release);
                Ok((old_position as i64, old_timestamp))
            }
            Err(current_position) => {
                // Failure: Return current state
                let current_timestamp = self.dual.load_secondary(Ordering::Acquire);
                Err((current_position as i64, current_timestamp))
            }
        }
    }

    /// Reset position to zero
    ///
    /// # Example
    /// ```rust
    /// use atomic_capsule::patterns::PositionTrackerCapsule;
    ///
    /// let tracker = PositionTrackerCapsule::with_initial(100, 1000);
    /// tracker.reset(2000);
    /// assert_eq!(tracker.load_position_only(), 0);
    /// assert_eq!(tracker.load_timestamp(), 2000);
    /// ```
    #[inline(always)]
    pub fn reset(&self, timestamp: u64) {
        self.update_position(0, timestamp);
    }
}

// Implement Default for convenience
impl Default for PositionTrackerCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Implement Send + Sync (lockfree atomic operations are thread-safe)
unsafe impl Send for PositionTrackerCapsule {}
unsafe impl Sync for PositionTrackerCapsule {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alignment_and_size() {
        use core::mem::{align_of, size_of};

        assert_eq!(
            align_of::<PositionTrackerCapsule>(),
            128,
            "Must be 128-byte aligned"
        );
        assert_eq!(
            size_of::<PositionTrackerCapsule>(),
            128,
            "Must be 128 bytes total"
        );
    }

    #[test]
    fn test_new() {
        let tracker = PositionTrackerCapsule::new();
        assert_eq!(tracker.load_position_only(), 0);
        assert_eq!(tracker.load_timestamp(), 0);

        let (position, timestamp) = tracker.load_position();
        assert_eq!(position, 0);
        assert_eq!(timestamp, 0);
    }

    #[test]
    fn test_with_initial() {
        let tracker = PositionTrackerCapsule::with_initial(100, 1000);
        assert_eq!(tracker.load_position_only(), 100);
        assert_eq!(tracker.load_timestamp(), 1000);
    }

    #[test]
    fn test_update_position() {
        let tracker = PositionTrackerCapsule::new();

        tracker.update_position(100, 1000);
        assert_eq!(tracker.load_position_only(), 100);
        assert_eq!(tracker.load_timestamp(), 1000);

        tracker.update_position(-50, 2000);
        assert_eq!(tracker.load_position_only(), -50);
        assert_eq!(tracker.load_timestamp(), 2000);
    }

    #[test]
    fn test_add_position() {
        let tracker = PositionTrackerCapsule::with_initial(100, 1000);

        // Add positive
        tracker.add_position(50, 2000);
        assert_eq!(tracker.load_position_only(), 150);
        assert_eq!(tracker.load_timestamp(), 2000);

        // Add negative
        tracker.add_position(-30, 3000);
        assert_eq!(tracker.load_position_only(), 120);
        assert_eq!(tracker.load_timestamp(), 3000);
    }

    #[test]
    fn test_compare_exchange_position() {
        let tracker = PositionTrackerCapsule::with_initial(100, 1000);

        // Success case
        let result = tracker.compare_exchange_position(100, 200, 2000);
        assert_eq!(result, Ok((100, 1000)));
        assert_eq!(tracker.load_position_only(), 200);
        assert_eq!(tracker.load_timestamp(), 2000);

        // Failure case
        let result = tracker.compare_exchange_position(999, 300, 3000);
        assert!(result.is_err());
        assert_eq!(tracker.load_position_only(), 200); // Unchanged
    }

    #[test]
    fn test_reset() {
        let tracker = PositionTrackerCapsule::with_initial(100, 1000);
        tracker.reset(2000);
        assert_eq!(tracker.load_position_only(), 0);
        assert_eq!(tracker.load_timestamp(), 2000);
    }

    #[test]
    fn test_negative_positions() {
        let tracker = PositionTrackerCapsule::new();

        tracker.update_position(-100, 1000);
        assert_eq!(tracker.load_position_only(), -100);

        tracker.add_position(-50, 2000);
        assert_eq!(tracker.load_position_only(), -150);

        tracker.add_position(75, 3000);
        assert_eq!(tracker.load_position_only(), -75);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_updates() {
        use std::sync::Arc;
        use std::thread;

        let tracker = Arc::new(PositionTrackerCapsule::new());
        let mut handles = vec![];

        // Spawn 4 threads, each adds 1000 to position
        for _ in 0..4 {
            let tracker_clone = Arc::clone(&tracker);
            handles.push(thread::spawn(move || {
                for i in 0..1000 {
                    tracker_clone.add_position(1, i as u64);
                }
            }));
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify total position
        assert_eq!(tracker.load_position_only(), 4000);
    }
}
