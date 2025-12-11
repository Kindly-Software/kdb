//! # EntangledGeneration - License-Bound Generation Counter
//!
//! **[TRADE SECRET] - Generation counter incorporating license rotation**
//!
//! ## UCE34 Framework Compliance
//!
//! **Q10 Tier**: T1 Atomic + T3 Fixed-Point
//! **Q33 Lockfree**: 100% lockfree, no Mutex/RwLock
//! **Q34 Audit**: Generation counter provides replay attack detection
//!
//! ## Core Innovation
//!
//! The generation counter incorporates the license rotation schedule.
//! This means:
//! 1. Each generation increment is entangled with license transform
//! 2. Generation values are predictable only with correct license
//! 3. Replay attacks are detectable (generation monotonically increases)
//! 4. License rotation changes generation sequence
//!
//! ## Memory Layout (64 bytes, cache-line aligned)
//!
//! ```text
//! Offset 0-7:    generation (AtomicU64) - current generation value
//! Offset 8-15:   rotation_factor (u64) - derived from license
//! Offset 16-23:  rotation_epoch (AtomicU64) - rotation epoch counter
//! Offset 24-31:  last_rotated (AtomicU64) - timestamp of last rotation
//! Offset 32-63:  padding (32 bytes)
//! ```
//!
//! ## Performance (B32 Targets)
//! - Increment: <10ns (atomic + XOR)
//! - Rotate: <50ns (recompute rotation factor)
//! - Verify: <5ns (range check)

use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

/// Rotation schedule configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotationSchedule {
    /// Rotation interval in seconds (0 = no rotation)
    pub interval_seconds: u64,
    /// Maximum rotations before requiring re-licensing
    pub max_rotations: u64,
    /// Initial rotation factor (from license)
    pub initial_factor: u64,
}

impl RotationSchedule {
    /// No rotation (infinite validity within license period)
    pub const NONE: Self = Self {
        interval_seconds: 0,
        max_rotations: 0,
        initial_factor: 0,
    };

    /// Daily rotation (86400 seconds)
    pub const DAILY: Self = Self {
        interval_seconds: 86400,
        max_rotations: 365,
        initial_factor: 0,
    };

    /// Weekly rotation
    pub const WEEKLY: Self = Self {
        interval_seconds: 604800,
        max_rotations: 52,
        initial_factor: 0,
    };

    /// Monthly rotation (30 days)
    pub const MONTHLY: Self = Self {
        interval_seconds: 2592000,
        max_rotations: 12,
        initial_factor: 0,
    };

    /// Create custom rotation schedule
    pub const fn new(interval_seconds: u64, max_rotations: u64, initial_factor: u64) -> Self {
        Self {
            interval_seconds,
            max_rotations,
            initial_factor,
        }
    }

    /// Create schedule with initial factor from license transform
    pub const fn with_factor(mut self, factor: u64) -> Self {
        self.initial_factor = factor;
        self
    }

    /// Check if rotation is required
    pub fn needs_rotation(&self, last_rotated: u64, now: u64) -> bool {
        if self.interval_seconds == 0 {
            return false;
        }
        now.saturating_sub(last_rotated) >= self.interval_seconds
    }

    /// Compute rotation factor for given epoch
    ///
    /// Each epoch has a unique factor derived from initial + epoch number
    pub fn factor_for_epoch(&self, epoch: u64) -> u64 {
        if epoch == 0 {
            return self.initial_factor;
        }

        // Mix initial factor with epoch
        let mut factor = self.initial_factor;
        factor = factor.wrapping_add(epoch.wrapping_mul(0x9e3779b97f4a7c15));
        factor = factor.rotate_left((epoch & 63) as u32);
        factor ^= epoch.wrapping_mul(0x517cc1b727220a95);

        factor
    }
}

impl Default for RotationSchedule {
    fn default() -> Self {
        Self::NONE
    }
}

/// EntangledGeneration - Generation counter bound to license rotation
///
/// ## Memory Layout (64 bytes, cache-line aligned)
///
/// - Offset 0-7: `generation` (AtomicU64) - current generation
/// - Offset 8-15: `rotation_factor` (u64) - current rotation factor
/// - Offset 16-23: `rotation_epoch` (AtomicU64) - rotation epoch counter
/// - Offset 24-31: `last_rotated` (AtomicU64) - last rotation timestamp
/// - Offset 32-63: `_padding` ([u8; 32])
///
/// ## Security Properties
///
/// - Generation values include rotation factor
/// - Rotation factor changes on schedule
/// - Replay attacks detectable via monotonic generation
/// - License rotation enforces periodic re-validation
///
/// ## Performance (B32 Targets)
/// - Increment: <10ns
/// - Rotate: <50ns
/// - Verify: <5ns
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 64, size = 64))]
#[repr(C, align(64))]
pub struct EntangledGeneration {
    /// Current generation value (entangled with rotation factor)
    generation: AtomicU64,

    /// Current rotation factor (derived from license + epoch)
    rotation_factor: u64,

    /// Rotation epoch counter (increments on rotation)
    rotation_epoch: AtomicU64,

    /// Last rotation timestamp (Unix seconds)
    last_rotated: AtomicU64,

    /// Padding to 64 bytes
    _padding: [u8; 32],
}

// Compile-time verification
#[cfg(not(feature = "derive"))]
crate::verify_capsule_properties!(EntangledGeneration, 64, 64);

// Send + Sync safety
#[cfg(not(feature = "derive"))]
unsafe impl Send for EntangledGeneration {}
#[cfg(not(feature = "derive"))]
unsafe impl Sync for EntangledGeneration {}

impl EntangledGeneration {
    /// Create new entangled generation counter
    ///
    /// ## Arguments
    /// - `initial_factor`: License transform (SHA256(signature)[0..8])
    ///
    /// ## Returns
    /// Initialized generation counter
    pub const fn new(initial_factor: u64) -> Self {
        Self {
            generation: AtomicU64::new(initial_factor), // Initial gen IS the factor
            rotation_factor: initial_factor,
            rotation_epoch: AtomicU64::new(0),
            last_rotated: AtomicU64::new(0),
            _padding: [0u8; 32],
        }
    }

    /// Create with explicit initial values
    pub const fn with_values(initial_factor: u64, initial_gen: u64, epoch: u64) -> Self {
        Self {
            generation: AtomicU64::new(initial_gen),
            rotation_factor: initial_factor,
            rotation_epoch: AtomicU64::new(epoch),
            last_rotated: AtomicU64::new(0),
            _padding: [0u8; 32],
        }
    }

    /// Increment generation (entangled with rotation factor)
    ///
    /// ## Returns
    /// New generation value
    ///
    /// ## Performance
    /// <10ns (atomic fetch_add + XOR)
    ///
    /// ## ASSUM Framework
    /// - `#ASSUME_MONOTONIC`: Generation only increases (wrapping at u64::MAX)
    /// - `#VERIFY_MONOTONIC`: Tests validate monotonic property
    #[inline(always)]
    pub fn increment(&self) -> u64 {
        // Increment base generation
        let base = self.generation.fetch_add(1, Ordering::AcqRel);

        // Return entangled value (base XOR factor)
        base.wrapping_add(1) ^ self.rotation_factor
    }

    /// Get current generation (entangled)
    ///
    /// ## Performance
    /// <5ns (atomic load + XOR)
    #[inline(always)]
    pub fn current(&self) -> u64 {
        let base = self.generation.load(Ordering::Acquire);
        base ^ self.rotation_factor
    }

    /// Get raw generation (not entangled, for internal use)
    #[inline(always)]
    pub fn raw(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get current rotation epoch
    #[inline(always)]
    pub fn epoch(&self) -> u64 {
        self.rotation_epoch.load(Ordering::Acquire)
    }

    /// Get rotation factor
    #[inline(always)]
    pub fn rotation_factor(&self) -> u64 {
        self.rotation_factor
    }

    /// Get last rotation timestamp
    #[inline(always)]
    pub fn last_rotated(&self) -> u64 {
        self.last_rotated.load(Ordering::Acquire)
    }

    /// Verify generation is in expected range
    ///
    /// ## Arguments
    /// - `expected_min`: Minimum expected generation
    /// - `expected_max`: Maximum expected generation (inclusive)
    ///
    /// ## Returns
    /// - `true` if generation is in range
    /// - `false` if potential replay or tampering detected
    ///
    /// ## Performance
    /// <5ns (2 comparisons)
    #[inline(always)]
    pub fn verify_range(&self, expected_min: u64, expected_max: u64) -> bool {
        let gen = self.raw();
        gen >= expected_min && gen <= expected_max
    }

    /// Check if rotation is needed and perform it
    ///
    /// ## Arguments
    /// - `schedule`: Rotation schedule
    /// - `now`: Current timestamp (Unix seconds)
    ///
    /// ## Returns
    /// - `Some(new_epoch)` if rotation performed
    /// - `None` if no rotation needed
    ///
    /// ## Performance
    /// <50ns when rotation occurs (factor recomputation)
    pub fn maybe_rotate(&mut self, schedule: &RotationSchedule, now: u64) -> Option<u64> {
        let last = self.last_rotated.load(Ordering::Acquire);

        if !schedule.needs_rotation(last, now) {
            return None;
        }

        // Check max rotations
        let current_epoch = self.rotation_epoch.load(Ordering::Acquire);
        if current_epoch >= schedule.max_rotations && schedule.max_rotations > 0 {
            return None; // Max rotations reached
        }

        // Perform rotation
        let new_epoch = current_epoch + 1;
        self.rotation_factor = schedule.factor_for_epoch(new_epoch);
        self.rotation_epoch.store(new_epoch, Ordering::Release);
        self.last_rotated.store(now, Ordering::Release);

        Some(new_epoch)
    }

    /// Force rotation (for testing or manual rotation)
    ///
    /// ## Arguments
    /// - `schedule`: Rotation schedule
    /// - `now`: Current timestamp
    ///
    /// ## Returns
    /// New epoch number
    pub fn force_rotate(&mut self, schedule: &RotationSchedule, now: u64) -> u64 {
        let new_epoch = self.rotation_epoch.load(Ordering::Acquire) + 1;
        self.rotation_factor = schedule.factor_for_epoch(new_epoch);
        self.rotation_epoch.store(new_epoch, Ordering::Release);
        self.last_rotated.store(now, Ordering::Release);

        new_epoch
    }

    /// Compare and exchange generation (for CAS operations)
    ///
    /// ## Arguments
    /// - `expected`: Expected raw generation value
    /// - `new`: New raw generation value
    ///
    /// ## Returns
    /// - `Ok(old)` if CAS succeeded
    /// - `Err(actual)` if CAS failed
    pub fn compare_exchange(
        &self,
        expected: u64,
        new: u64,
    ) -> Result<u64, u64> {
        self.generation.compare_exchange(
            expected,
            new,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
    }
}

impl Default for EntangledGeneration {
    fn default() -> Self {
        Self::new(0)
    }
}

// ============================================================================
// T28 COMPREHENSIVE TESTING
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// T28: Unit Test - Rotation schedule constants
    #[test]
    fn test_rotation_schedules() {
        assert_eq!(RotationSchedule::NONE.interval_seconds, 0);
        assert_eq!(RotationSchedule::DAILY.interval_seconds, 86400);
        assert_eq!(RotationSchedule::WEEKLY.interval_seconds, 604800);
        assert_eq!(RotationSchedule::MONTHLY.interval_seconds, 2592000);
    }

    /// T28: Unit Test - Schedule needs_rotation
    #[test]
    fn test_needs_rotation() {
        let daily = RotationSchedule::DAILY;

        // Just created - no rotation needed
        assert!(!daily.needs_rotation(0, 1000));

        // Almost a day - no rotation
        assert!(!daily.needs_rotation(0, 86399));

        // Exactly a day - rotation needed
        assert!(daily.needs_rotation(0, 86400));

        // More than a day - rotation needed
        assert!(daily.needs_rotation(0, 100000));

        // No rotation schedule - never needs rotation
        assert!(!RotationSchedule::NONE.needs_rotation(0, u64::MAX));
    }

    /// T28: Unit Test - Factor for epoch
    #[test]
    fn test_factor_for_epoch() {
        let schedule = RotationSchedule::new(86400, 365, 0xDEADBEEF);

        // Epoch 0 = initial factor
        assert_eq!(schedule.factor_for_epoch(0), 0xDEADBEEF);

        // Different epochs = different factors
        let f1 = schedule.factor_for_epoch(1);
        let f2 = schedule.factor_for_epoch(2);
        let f3 = schedule.factor_for_epoch(3);

        assert_ne!(f1, 0xDEADBEEF);
        assert_ne!(f1, f2);
        assert_ne!(f2, f3);
    }

    /// T28: Unit Test - Generation creation
    #[test]
    fn test_entangled_generation_creation() {
        let gen = EntangledGeneration::new(0xCAFEBABE);

        assert_eq!(gen.rotation_factor(), 0xCAFEBABE);
        assert_eq!(gen.epoch(), 0);
        assert_eq!(gen.raw(), 0xCAFEBABE); // Initial gen IS factor
    }

    /// T28: Property Test - Increment monotonicity
    #[test]
    fn test_increment_monotonic() {
        let gen = EntangledGeneration::new(0x1234);

        let mut prev_raw = gen.raw();
        for _ in 0..100 {
            gen.increment();
            let new_raw = gen.raw();
            assert!(new_raw > prev_raw);
            prev_raw = new_raw;
        }
    }

    /// T28: Property Test - Entanglement changes output
    #[test]
    fn test_entanglement_effect() {
        // Two counters with different factors
        let gen1 = EntangledGeneration::new(0xAAAA);
        let gen2 = EntangledGeneration::new(0xBBBB);

        // Same number of increments
        for _ in 0..10 {
            gen1.increment();
            gen2.increment();
        }

        // Same raw values (started at different points, but incremented same)
        // Different entangled values (different factors)
        assert_ne!(gen1.current(), gen2.current());
    }

    /// T28: Property Test - Range verification
    #[test]
    fn test_verify_range() {
        let gen = EntangledGeneration::with_values(0, 100, 0);

        assert!(gen.verify_range(50, 150));
        assert!(gen.verify_range(100, 100));
        assert!(!gen.verify_range(101, 200));
        assert!(!gen.verify_range(50, 99));
    }

    /// T28: Integration Test - Rotation
    #[test]
    fn test_maybe_rotate() {
        let mut gen = EntangledGeneration::new(0xFEED);
        let schedule = RotationSchedule::new(100, 10, 0xFEED);

        // Too early - no rotation
        assert!(gen.maybe_rotate(&schedule, 50).is_none());
        assert_eq!(gen.epoch(), 0);

        // Time for rotation
        let new_epoch = gen.maybe_rotate(&schedule, 100).unwrap();
        assert_eq!(new_epoch, 1);
        assert_eq!(gen.epoch(), 1);
        assert_ne!(gen.rotation_factor(), 0xFEED); // Factor changed
    }

    /// T28: Integration Test - Max rotations
    #[test]
    fn test_max_rotations() {
        let mut gen = EntangledGeneration::with_values(0xABCD, 100, 9);
        let schedule = RotationSchedule::new(100, 10, 0xABCD);

        // One more rotation allowed
        assert!(gen.maybe_rotate(&schedule, 1000).is_some());
        assert_eq!(gen.epoch(), 10);

        // Max reached - no more rotations
        assert!(gen.maybe_rotate(&schedule, 2000).is_none());
        assert_eq!(gen.epoch(), 10);
    }

    /// T28: Integration Test - Force rotate
    #[test]
    fn test_force_rotate() {
        let mut gen = EntangledGeneration::new(0x1111);
        let schedule = RotationSchedule::new(86400, 100, 0x1111);

        let epoch = gen.force_rotate(&schedule, 12345);
        assert_eq!(epoch, 1);
        assert_eq!(gen.last_rotated(), 12345);
        assert_ne!(gen.rotation_factor(), 0x1111);
    }

    /// T28: Production Test - Compare exchange
    #[test]
    fn test_compare_exchange() {
        let gen = EntangledGeneration::with_values(0, 100, 0);

        // Successful CAS
        assert_eq!(gen.compare_exchange(100, 101), Ok(100));
        assert_eq!(gen.raw(), 101);

        // Failed CAS (wrong expected)
        assert_eq!(gen.compare_exchange(100, 102), Err(101));
        assert_eq!(gen.raw(), 101);
    }

    /// T28: Production Test - Concurrent increments
    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_increments() {
        use std::sync::Arc;
        use std::thread;

        let gen = Arc::new(EntangledGeneration::new(0));

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let gen = Arc::clone(&gen);
                thread::spawn(move || {
                    for _ in 0..1000 {
                        gen.increment();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // 4000 increments from initial value
        assert_eq!(gen.raw(), 4000);
    }

    /// T28: Production Test - Memory layout
    #[test]
    fn test_memory_layout() {
        use core::mem::{size_of, align_of};

        assert_eq!(size_of::<EntangledGeneration>(), 64);
        assert_eq!(align_of::<EntangledGeneration>(), 64);
    }
}
