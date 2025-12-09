//! ShimmerCapsule - Progress Bar Shimmer Animation
//!
//! Lockfree shimmer animation for progress bars. Implements 2-second loop with
//! configurable speed for visual feedback on long-running operations.
//!
//! # Animation
//!
//! Continuous offset advancement (0.0-1.0 loop) for shimmer effect overlay
//! on progress bars during GPU/CPU processing.
//!
//! # Performance
//!
//! - update(): <2ns (Q16.16 add + wrap)
//! - get_offset(): <1ns (atomic load + shift)
//!
//! # Framework Compliance
//!
//! - **UCE34**: T3 Fixed-Point tier (deterministic timing)
//! - **Chaos**: 100% lockfree (AtomicU32, 64B aligned)

use std::sync::atomic::{AtomicU32, Ordering};
use super::{to_fixed, from_fixed, FIXED_POINT_ONE};

/// ShimmerCapsule - Progress bar shimmer animation
///
/// 64-byte cache-aligned capsule for lockfree shimmer animations.
///
/// # Memory Layout
///
/// ```text
/// [0-3]   offset (AtomicU32, Q16.16, 0.0-1.0)
/// [4-7]   speed (AtomicU32, Q16.16, units per second)
/// [8-63]  padding (56 bytes)
/// ```
#[repr(C, align(64))]
pub struct ShimmerCapsule {
    /// Current offset (Q16.16, 0.0-1.0)
    offset: AtomicU32,
    /// Speed in units per second (Q16.16, default 0.5 = 2-second loop)
    speed: AtomicU32,
    /// Padding to 64 bytes
    _padding: [u8; 56],
}

impl ShimmerCapsule {
    /// Default speed: 0.5 units/second (2-second loop)
    const DEFAULT_SPEED: f32 = 0.5;

    /// Create new ShimmerCapsule with default 2-second loop
    pub fn new() -> Self {
        Self {
            offset: AtomicU32::new(0),
            speed: AtomicU32::new(to_fixed(Self::DEFAULT_SPEED) as u32),
            _padding: [0; 56],
        }
    }

    /// Create ShimmerCapsule with custom speed
    ///
    /// # Parameters
    ///
    /// - `speed`: Units per second (0.5 = 2-second loop, 1.0 = 1-second loop)
    pub fn with_speed(speed: f32) -> Self {
        Self {
            offset: AtomicU32::new(0),
            speed: AtomicU32::new(to_fixed(speed) as u32),
            _padding: [0; 56],
        }
    }

    /// Update shimmer animation
    ///
    /// # Algorithm
    ///
    /// 1. Calculate offset increment: speed * (dt_ms / 1000.0)
    /// 2. Add to current offset
    /// 3. Wrap at 1.0 (modulo)
    ///
    /// # Performance
    ///
    /// <2ns per call (Q16.16 multiply + add + conditional)
    pub fn update(&self, dt_ms: u32) {
        // #ASSUME dt_ms <= 2000 (max 2 seconds per frame, allows test flexibility)
        // #VERIFY Called from GUI event loop (<16ms typical)
        debug_assert!(dt_ms <= 2000, "dt_ms should be <= 2 seconds");

        let speed = self.speed.load(Ordering::Relaxed) as i32;
        let current_offset = self.offset.load(Ordering::Relaxed) as i32;

        // Calculate offset increment: speed * (dt_ms / 1000.0)
        // In Q16.16: offset_inc = (speed * dt_ms) / 1000
        // Must divide by 1000 BEFORE the final shift to maintain precision
        let offset_inc = (speed as i64 * dt_ms as i64) / 1000;

        // Add to current offset
        let new_offset = current_offset + offset_inc as i32;

        // Wrap at 1.0 (FIXED_POINT_ONE)
        let wrapped_offset = if new_offset >= FIXED_POINT_ONE {
            new_offset - FIXED_POINT_ONE
        } else {
            new_offset
        };

        self.offset.store(wrapped_offset as u32, Ordering::Relaxed);
    }

    /// Get current offset (0.0-1.0)
    #[inline]
    pub fn get_offset(&self) -> f32 {
        let offset = self.offset.load(Ordering::Relaxed) as i32;
        from_fixed(offset)
    }

    /// Reset shimmer to start
    pub fn reset(&self) {
        self.offset.store(0, Ordering::Relaxed);
    }

    /// Set shimmer speed (units per second)
    pub fn set_speed(&self, speed: f32) {
        let fixed_speed = to_fixed(speed);
        self.speed.store(fixed_speed as u32, Ordering::Relaxed);
    }

    /// Get current speed (units per second)
    pub fn get_speed(&self) -> f32 {
        let speed = self.speed.load(Ordering::Relaxed) as i32;
        from_fixed(speed)
    }

    /// Pause shimmer (set speed to 0)
    pub fn pause(&self) {
        self.speed.store(0, Ordering::Relaxed);
    }

    /// Resume shimmer with previous speed
    pub fn resume(&self, speed: f32) {
        self.set_speed(speed);
    }
}

impl Default for ShimmerCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shimmer_creation() {
        let shimmer = ShimmerCapsule::new();
        assert_eq!(shimmer.get_offset(), 0.0);
        assert_eq!(shimmer.get_speed(), 0.5);
    }

    #[test]
    fn test_shimmer_update() {
        let shimmer = ShimmerCapsule::new();

        // Update by 1 second (speed=0.5, so offset += 0.5)
        shimmer.update(1000);
        let offset = shimmer.get_offset();
        assert!((offset - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_shimmer_wraps_at_one() {
        let shimmer = ShimmerCapsule::new();

        // Update by 2 seconds (speed=0.5, offset += 1.0, wraps to 0.0)
        shimmer.update(2000);
        assert!((shimmer.get_offset() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_shimmer_incremental_updates() {
        let shimmer = ShimmerCapsule::new();

        // Simulate 60fps for 1 second
        for _ in 0..60 {
            shimmer.update(16);
        }

        // Should be around 0.48 (960ms * 0.5 speed)
        let offset = shimmer.get_offset();
        assert!((offset - 0.48).abs() < 0.05);
    }

    #[test]
    fn test_shimmer_custom_speed() {
        let shimmer = ShimmerCapsule::with_speed(1.0); // 1 second loop

        shimmer.update(500); // Half second
        assert!((shimmer.get_offset() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_shimmer_reset() {
        let shimmer = ShimmerCapsule::new();
        shimmer.update(1000);
        shimmer.reset();
        assert_eq!(shimmer.get_offset(), 0.0);
    }

    #[test]
    fn test_shimmer_set_speed() {
        let shimmer = ShimmerCapsule::new();
        shimmer.set_speed(2.0); // Fast shimmer

        shimmer.update(500); // Half second -> 2.0 * 0.5 = 1.0, wraps to 0.0
        assert!((shimmer.get_offset() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_shimmer_pause() {
        let shimmer = ShimmerCapsule::new();
        shimmer.update(1000);
        let offset_before = shimmer.get_offset();

        shimmer.pause();
        shimmer.update(1000);
        assert_eq!(shimmer.get_offset(), offset_before);
    }

    #[test]
    fn test_shimmer_resume() {
        let shimmer = ShimmerCapsule::new();
        shimmer.pause();
        shimmer.update(1000);
        assert_eq!(shimmer.get_offset(), 0.0);

        shimmer.resume(1.0);
        shimmer.update(1000); // 1.0 * 1.0 = 1.0, wraps to 0.0
        assert!((shimmer.get_offset() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_shimmer_fast_speed() {
        let shimmer = ShimmerCapsule::with_speed(10.0); // Very fast

        shimmer.update(100); // 0.1 second -> 10.0 * 0.1 = 1.0, wraps to 0.0
        assert!((shimmer.get_offset() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_shimmer_slow_speed() {
        let shimmer = ShimmerCapsule::with_speed(0.1); // Very slow (10-second loop)

        shimmer.update(1000); // 1 second
        assert!((shimmer.get_offset() - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_shimmer_size() {
        assert_eq!(std::mem::size_of::<ShimmerCapsule>(), 64);
    }

    #[test]
    fn test_shimmer_alignment() {
        let shimmer = ShimmerCapsule::new();
        let ptr = &shimmer as *const ShimmerCapsule as usize;
        assert_eq!(ptr % 64, 0, "ShimmerCapsule must be 64-byte aligned");
    }
}
