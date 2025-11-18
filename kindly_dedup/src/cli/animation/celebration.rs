//! Success celebration animation (sparkles + hearts)
//!
//! ## UCE34 Framework
//! - Q10: Tier T1 Atomic (state flags + counters, <5ns per operation)
//! - Q11: Rust transform: AtomicU8 + AtomicBool for lockfree state
//! - Q28: Simplicity: Single celebration effect with 5-frame animation
//! - Q33: Verification: Atomic operations verified by Rust compiler

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// Success celebration animation (250ms duration, 5 frames @ 20 FPS)
///
/// Displays a short sparkle+heart effect to celebrate completion:
/// - Frame 0: ✨ ✨ ✨ (sparkles)
/// - Frame 1: ✨ 💛 ✨ (gold heart)
/// - Frame 2: 💛 ✨ 💛 (hearts)
/// - Frame 3: ✨ 💛 ✨ (gold heart)
/// - Frame 4: ✨ ✨ ✨ (sparkles)
/// - Then stops
///
/// ## Performance
/// - `render()`: <20ns (atomic loads + match)
/// - `start()`: <10ns (atomic stores)
/// - Memory: 2 bytes (AtomicU8 + AtomicBool)
///
/// ## Example
/// ```ignore
/// let celebration = CelebrationAnimation::new();
/// celebration.start();
/// println!("{}", celebration.render());  // ✨ ✨ ✨
/// println!("{}", celebration.render());  // ✨ 💛 ✨
/// // ... etc
/// ```
#[derive(Debug)]
pub struct CelebrationAnimation {
    frame_counter: AtomicU8,
    is_active: AtomicBool,
}

impl CelebrationAnimation {
    /// Create new celebration animation (inactive)
    #[inline]
    pub fn new() -> Self {
        Self {
            frame_counter: AtomicU8::new(0),
            is_active: AtomicBool::new(false),
        }
    }

    /// Start celebration effect
    ///
    /// Resets frame counter and marks as active.
    /// Celebration will play through 5 frames then automatically stop.
    ///
    /// ## Performance
    /// <10ns (two atomic stores)
    #[inline]
    pub fn start(&self) {
        self.frame_counter.store(0, Ordering::Release);
        self.is_active.store(true, Ordering::Release);
    }

    /// Render current frame of celebration
    ///
    /// Returns celebration animation string if active, empty string otherwise.
    /// Auto-advances frame and marks inactive after final frame.
    ///
    /// ## Performance
    /// <20ns (atomic loads + match + possible store)
    pub fn render(&self) -> String {
        if !self.is_active.load(Ordering::Acquire) {
            return String::new();
        }

        let frame = self.frame_counter.load(Ordering::Relaxed);

        let output = match frame {
            0 => "✨ ✨ ✨".to_string(), // Sparkles
            1 => "✨ 💛 ✨".to_string(), // Gold heart in center
            2 => "💛 ✨ 💛".to_string(), // Hearts on sides
            3 => "✨ 💛 ✨".to_string(), // Gold heart in center
            4 => "✨ ✨ ✨".to_string(), // Sparkles (final)
            _ => {
                self.is_active.store(false, Ordering::Release);
                return String::new();
            }
        };

        // Advance frame
        if frame < 4 {
            self.frame_counter.fetch_add(1, Ordering::Relaxed);
        } else {
            // Final frame - deactivate
            self.is_active.store(false, Ordering::Release);
        }

        output
    }

    /// Check if celebration is currently active
    ///
    /// ## Performance
    /// <1ns (atomic load)
    #[inline]
    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::Acquire)
    }

    /// Get current frame number (0-4)
    ///
    /// ## Performance
    /// <1ns (atomic load)
    #[inline]
    pub fn current_frame(&self) -> u8 {
        self.frame_counter.load(Ordering::Relaxed)
    }

    /// Stop celebration immediately
    ///
    /// ## Performance
    /// <1ns (atomic store)
    #[inline]
    pub fn stop(&self) {
        self.is_active.store(false, Ordering::Release);
    }

    /// Reset to frame 0 (without starting)
    ///
    /// ## Performance
    /// <1ns (atomic store)
    #[inline]
    pub fn reset(&self) {
        self.frame_counter.store(0, Ordering::Relaxed);
    }

    /// Render as single-line variant (no spaces)
    ///
    /// Useful for inline display.
    pub fn render_compact(&self) -> String {
        if !self.is_active.load(Ordering::Acquire) {
            return String::new();
        }

        let frame = self.frame_counter.load(Ordering::Relaxed);

        let output = match frame {
            0 => "✨✨✨".to_string(),
            1 => "✨💛✨".to_string(),
            2 => "💛✨💛".to_string(),
            3 => "✨💛✨".to_string(),
            4 => "✨✨✨".to_string(),
            _ => {
                self.is_active.store(false, Ordering::Release);
                return String::new();
            }
        };

        // Advance frame
        if frame < 4 {
            self.frame_counter.fetch_add(1, Ordering::Relaxed);
        } else {
            self.is_active.store(false, Ordering::Release);
        }

        output
    }

    /// Render as success message with emoji
    pub fn render_with_text(&self, text: &str) -> String {
        let animation = self.render();
        if animation.is_empty() {
            String::new()
        } else {
            format!("{} {} {}", animation, text, animation)
        }
    }
}

impl Default for CelebrationAnimation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creation() {
        let celebration = CelebrationAnimation::new();
        assert!(!celebration.is_active());
        assert_eq!(celebration.current_frame(), 0);
    }

    #[test]
    fn test_start() {
        let celebration = CelebrationAnimation::new();
        celebration.start();
        assert!(celebration.is_active());
        assert_eq!(celebration.current_frame(), 0);
    }

    #[test]
    fn test_render_sequence() {
        let celebration = CelebrationAnimation::new();
        celebration.start();

        // Verify frame sequence
        let frame_0 = celebration.render();
        assert_eq!(frame_0, "✨ ✨ ✨");
        assert!(celebration.is_active());

        let frame_1 = celebration.render();
        assert_eq!(frame_1, "✨ 💛 ✨");
        assert!(celebration.is_active());

        let frame_2 = celebration.render();
        assert_eq!(frame_2, "💛 ✨ 💛");
        assert!(celebration.is_active());

        let frame_3 = celebration.render();
        assert_eq!(frame_3, "✨ 💛 ✨");
        assert!(celebration.is_active());

        let frame_4 = celebration.render();
        assert_eq!(frame_4, "✨ ✨ ✨");
        assert!(!celebration.is_active()); // Should be inactive now
    }

    #[test]
    fn test_inactive_render() {
        let celebration = CelebrationAnimation::new();
        let result = celebration.render();
        assert_eq!(result, "");
    }

    #[test]
    fn test_stop() {
        let celebration = CelebrationAnimation::new();
        celebration.start();
        assert!(celebration.is_active());

        celebration.stop();
        assert!(!celebration.is_active());
    }

    #[test]
    fn test_reset() {
        let celebration = CelebrationAnimation::new();
        celebration.start();
        celebration.render();
        celebration.render();
        assert_eq!(celebration.current_frame(), 2);

        celebration.reset();
        assert_eq!(celebration.current_frame(), 0);
        assert!(!celebration.is_active()); // reset doesn't activate
    }

    #[test]
    fn test_compact_render() {
        let celebration = CelebrationAnimation::new();
        celebration.start();

        let frame_0 = celebration.render_compact();
        assert_eq!(frame_0, "✨✨✨");

        let frame_1 = celebration.render_compact();
        assert_eq!(frame_1, "✨💛✨");
    }

    #[test]
    fn test_multiple_starts() {
        let celebration = CelebrationAnimation::new();

        // First celebration
        celebration.start();
        celebration.render();
        celebration.render();
        celebration.render();
        celebration.render();
        celebration.render();
        assert!(!celebration.is_active());

        // Second celebration (restart)
        celebration.start();
        assert!(celebration.is_active());
        assert_eq!(celebration.current_frame(), 0);
    }

    #[test]
    fn test_render_with_text() {
        let celebration = CelebrationAnimation::new();
        celebration.start();

        let output = celebration.render_with_text("Success!");
        assert!(output.contains("Success!"));
        assert!(output.contains("✨"));
    }

    #[test]
    fn test_default() {
        let celebration = CelebrationAnimation::default();
        assert!(!celebration.is_active());
    }
}
