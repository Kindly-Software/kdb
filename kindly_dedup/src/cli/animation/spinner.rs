//! Rotating emoji spinner animation
//!
//! ## UCE34 Framework
//! - Q10: Tier T1 Atomic (frame counter, <5ns per render)
//! - Q11: Rust transform: AtomicU64 for lockfree frame counting
//! - Q28: Simplicity: 3-frame rotating animation
//! - Q33: Verification: Simple atomic operations verified by Rust compiler

use std::sync::atomic::{AtomicU64, Ordering};

/// Rotating spinner animation with 3 frames
///
/// Cycles through rotating arrows at maximum speed:
/// - Frame 0: 🔄 (rotating arrows)
/// - Frame 1: 🔃 (rotating arrows reversed)
/// - Frame 2: 🔁 (counterclockwise arrows)
///
/// Useful for indicating loading/processing without blocking UI.
///
/// ## Performance
/// - `render()`: <5ns (atomic load + modulo + array access)
/// - <1ns per frame advance (atomic fetch_add)
///
/// ## Example
/// ```ignore
/// let spinner = SpinnerAnimation::new();
/// print!("Processing {} ", spinner.render());
/// ```
#[derive(Debug)]
pub struct SpinnerAnimation {
    frame_counter: AtomicU64,
}

impl SpinnerAnimation {
    /// Create new spinner with frame counter at 0
    #[inline]
    pub fn new() -> Self {
        Self {
            frame_counter: AtomicU64::new(0),
        }
    }

    /// Get current spinner frame emoji
    ///
    /// Auto-advances frame counter on each call.
    /// Returns rotating emoji: 🔄 → 🔃 → 🔁 → 🔄 ...
    ///
    /// ## Performance
    /// <5ns (atomic fetch_add + const array lookup)
    ///
    /// ## Example
    /// ```ignore
    /// let spinner = SpinnerAnimation::new();
    /// println!("{}", spinner.render());  // 🔄
    /// println!("{}", spinner.render());  // 🔃
    /// println!("{}", spinner.render());  // 🔁
    /// println!("{}", spinner.render());  // 🔄 (wraps)
    /// ```
    #[inline]
    pub fn render(&self) -> &'static str {
        let frame = self.frame_counter.fetch_add(1, Ordering::Relaxed) % 3;
        match frame {
            0 => "🔄",
            1 => "🔃",
            2 => "🔁",
            _ => "🔄", // Unreachable (modulo 3)
        }
    }

    /// Get current frame number (0-2) without advancing
    ///
    /// ## Performance
    /// <1ns (atomic load)
    #[inline]
    pub fn current_frame(&self) -> u64 {
        self.frame_counter.load(Ordering::Relaxed) % 3
    }

    /// Get total frames rendered (diagnostic)
    ///
    /// ## Performance
    /// <1ns (atomic load)
    #[inline]
    pub fn frame_count(&self) -> u64 {
        self.frame_counter.load(Ordering::Relaxed)
    }

    /// Reset spinner to frame 0
    ///
    /// ## Performance
    /// <1ns (atomic store)
    #[inline]
    pub fn reset(&self) {
        self.frame_counter.store(0, Ordering::Relaxed);
    }

    /// Advance to next frame without returning emoji
    ///
    /// Useful when you want separate control over advancement and rendering.
    ///
    /// ## Performance
    /// <1ns (atomic fetch_add)
    #[inline]
    pub fn advance(&self) {
        let _ = self.frame_counter.fetch_add(1, Ordering::Relaxed);
    }

    /// Set frame counter to specific value
    ///
    /// Useful for testing or synchronization.
    ///
    /// ## Performance
    /// <1ns (atomic store)
    #[inline]
    pub fn set_frame(&self, frame: u64) {
        self.frame_counter.store(frame, Ordering::Relaxed);
    }
}

impl Default for SpinnerAnimation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_creation() {
        let spinner = SpinnerAnimation::new();
        assert_eq!(spinner.frame_count(), 0);
    }

    #[test]
    fn test_spinner_frames() {
        let spinner = SpinnerAnimation::new();

        // Test sequence: 🔄 → 🔃 → 🔁 → 🔄 ...
        assert_eq!(spinner.render(), "🔄");
        assert_eq!(spinner.render(), "🔃");
        assert_eq!(spinner.render(), "🔁");
        assert_eq!(spinner.render(), "🔄"); // Wraps
    }

    #[test]
    fn test_current_frame() {
        let spinner = SpinnerAnimation::new();

        assert_eq!(spinner.current_frame(), 0);
        spinner.advance();
        assert_eq!(spinner.current_frame(), 1);
        spinner.advance();
        assert_eq!(spinner.current_frame(), 2);
        spinner.advance();
        assert_eq!(spinner.current_frame(), 0); // Wraps
    }

    #[test]
    fn test_frame_count() {
        let spinner = SpinnerAnimation::new();

        assert_eq!(spinner.frame_count(), 0);
        spinner.render();
        assert_eq!(spinner.frame_count(), 1);
        spinner.render();
        assert_eq!(spinner.frame_count(), 2);
    }

    #[test]
    fn test_reset() {
        let spinner = SpinnerAnimation::new();

        spinner.render();
        spinner.render();
        assert!(spinner.frame_count() >= 2);

        spinner.reset();
        assert_eq!(spinner.frame_count(), 0);
        assert_eq!(spinner.current_frame(), 0);
    }

    #[test]
    fn test_set_frame() {
        let spinner = SpinnerAnimation::new();

        spinner.set_frame(0);
        assert_eq!(spinner.current_frame(), 0);

        spinner.set_frame(1);
        assert_eq!(spinner.current_frame(), 1);

        spinner.set_frame(100);
        assert_eq!(spinner.current_frame(), 1); // 100 % 3 = 1
    }

    #[test]
    fn test_advance() {
        let spinner = SpinnerAnimation::new();

        assert_eq!(spinner.current_frame(), 0);
        spinner.advance();
        assert_eq!(spinner.current_frame(), 1);
        spinner.advance();
        assert_eq!(spinner.current_frame(), 2);
    }

    #[test]
    fn test_default() {
        let spinner = SpinnerAnimation::default();
        assert_eq!(spinner.frame_count(), 0);
    }

    #[test]
    fn test_render_consistency() {
        let spinner = SpinnerAnimation::new();

        // Test that render() always returns valid emoji
        for _ in 0..100 {
            let emoji = spinner.render();
            assert!(emoji == "🔄" || emoji == "🔃" || emoji == "🔁");
        }
    }
}
