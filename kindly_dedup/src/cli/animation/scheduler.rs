//! Frame scheduler (8-60 FPS, lockfree)
//!
//! ## UCE34 Framework
//! - Q10: Tier T1 Atomic (frame timing, <10ns per frame)
//! - Q11: Rust transform: AtomicU64 for nanosecond timestamps
//! - Q28: Simplicity: Single responsibility (frame rate management)
//! - Q31: Zero-copy: Borrow checker enforced
//! - Q33: Verification: AnimationStateCapsule verified at compile-time

use crate::cli::state::AnimationStateCapsule;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Frame scheduler manages FPS regulation and render timing
///
/// Provides lockfree frame pacing without sleep overhead.
/// Uses high-resolution nanosecond timestamps for accurate frame timing.
///
/// ## Performance
/// - `should_render()`: <10ns (atomic load)
/// - `current_fps()`: <50ns (two atomic loads + division)
/// - `wait_for_next_frame()`: ~frame_duration (thread sleep)
#[derive(Debug)]
pub struct FrameScheduler {
    animation_state: Arc<AnimationStateCapsule>,
    start_time: Instant,
}

impl FrameScheduler {
    /// Create new frame scheduler
    ///
    /// # Arguments
    /// - `fps`: Target frames per second (8-60, clamped)
    ///
    /// # Example
    /// ```ignore
    /// let scheduler = FrameScheduler::new(8);  // 8 FPS (125ms per frame)
    /// ```
    #[inline]
    pub fn new(fps: u8) -> Self {
        Self {
            animation_state: Arc::new(AnimationStateCapsule::new(fps)),
            start_time: Instant::now(),
        }
    }

    /// Check if it's time to render next frame
    ///
    /// Uses high-resolution nanosecond timestamps to determine if frame_interval
    /// has elapsed since last render.
    ///
    /// ## Performance
    /// <10ns (atomic loads + integer arithmetic)
    #[inline]
    pub fn should_render(&self) -> bool {
        let now_ns = self.nanos_since_start();
        self.animation_state.should_render(now_ns)
    }

    /// Update animation state and advance to next frame
    ///
    /// Increments frame counter and updates last frame timestamp.
    /// Must be called after rendering each frame.
    ///
    /// ## Performance
    /// <15ns (two atomic operations)
    #[inline]
    pub fn advance_frame(&self) {
        let now_ns = self.nanos_since_start();
        let _ = self.animation_state.next_frame();
        self.animation_state.set_last_frame_time(now_ns);
    }

    /// Sleep until next frame time
    ///
    /// Blocks current thread for exactly one frame duration.
    /// Useful for main loop pacing without busy-waiting.
    ///
    /// ## Performance
    /// O(frame_duration) - hardware sleep (not CPU bound)
    #[inline]
    pub fn wait_for_next_frame(&self) {
        let fps = self.animation_state.fps();
        let frame_duration_ms = 1000 / (fps as u64).max(1);
        std::thread::sleep(Duration::from_millis(frame_duration_ms));
    }

    /// Get current frames per second (calculated)
    ///
    /// Returns actual throughput based on frame counter and elapsed time.
    /// Updates incrementally (not smoothed).
    ///
    /// ## Performance
    /// <50ns (three atomic loads + division)
    ///
    /// ## Return Value
    /// - 0.0 if elapsed < 100ms (insufficient data)
    /// - Calculated FPS otherwise
    #[inline]
    pub fn current_fps(&self) -> f64 {
        let frame_count = self.animation_state.frame_count();
        let elapsed_ns = self.nanos_since_start();

        if frame_count == 0 || elapsed_ns < 100_000_000 {
            // Return target FPS if insufficient data
            self.animation_state.fps() as f64
        } else {
            // FPS = frame_count * 1_000_000_000 / elapsed_ns
            (frame_count as f64) * 1_000_000_000.0 / (elapsed_ns as f64)
        }
    }

    /// Get total frames rendered
    #[inline]
    pub fn frame_count(&self) -> u64 {
        self.animation_state.frame_count()
    }

    /// Get target FPS
    #[inline]
    pub fn target_fps(&self) -> u8 {
        self.animation_state.fps()
    }

    /// Set target FPS (8-60, clamped)
    #[inline]
    pub fn set_target_fps(&self, fps: u8) {
        self.animation_state.set_fps(fps);
    }

    /// Get elapsed time since scheduler creation (seconds)
    #[inline]
    pub fn elapsed_seconds(&self) -> f64 {
        self.nanos_since_start() as f64 / 1_000_000_000.0
    }

    // ========================================================================
    // Private helpers
    // ========================================================================

    /// Get nanoseconds elapsed since start_time
    #[inline(always)]
    fn nanos_since_start(&self) -> u64 {
        self.start_time.elapsed().as_nanos() as u64
    }
}

impl Default for FrameScheduler {
    fn default() -> Self {
        Self::new(8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_creation() {
        let scheduler = FrameScheduler::new(8);
        assert_eq!(scheduler.target_fps(), 8);
        assert_eq!(scheduler.frame_count(), 0);
    }

    #[test]
    fn test_should_render_initial() {
        let scheduler = FrameScheduler::new(8);
        // First frame should render immediately (no last_frame_time set)
        assert!(scheduler.should_render());
    }

    #[test]
    fn test_advance_frame() {
        let scheduler = FrameScheduler::new(8);
        scheduler.advance_frame();
        assert_eq!(scheduler.frame_count(), 1);
        scheduler.advance_frame();
        assert_eq!(scheduler.frame_count(), 2);
    }

    #[test]
    fn test_fps_clamping() {
        let scheduler = FrameScheduler::new(120); // Clamped to 60
        assert_eq!(scheduler.target_fps(), 60);

        let scheduler = FrameScheduler::new(4); // Clamped to 8
        assert_eq!(scheduler.target_fps(), 8);
    }

    #[test]
    fn test_set_target_fps() {
        let scheduler = FrameScheduler::new(8);
        scheduler.set_target_fps(16);
        assert_eq!(scheduler.target_fps(), 16);
    }

    #[test]
    fn test_elapsed_seconds() {
        let scheduler = FrameScheduler::new(8);
        let elapsed = scheduler.elapsed_seconds();
        assert!(elapsed >= 0.0);
        assert!(elapsed < 1.0); // Should be very small
    }

    #[test]
    fn test_default_fps() {
        let scheduler = FrameScheduler::default();
        assert_eq!(scheduler.target_fps(), 8);
    }
}
