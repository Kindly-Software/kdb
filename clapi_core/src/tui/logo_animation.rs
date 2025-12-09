//! Logo Animation Capsule - Byzantine Purple ↔ Gold Ping-Pong
//!
//! # UCE34 Framework
//! - Q1-Q9: 60 FPS logo color animation (Byzantine Purple ↔ Gold)
//! - Q10: Tier 1 (Atomic) - Lockfree frame counter for animation state
//! - Q11: Rust atomic patterns for concurrent read/write (animator thread + render thread)
//! - Q12: Nightly N/A (stable atomics sufficient)
//! - Q13-Q28: Animation smoothness, color interpolation correctness
//! - Q31: Simplicity - Single frame counter, linear RGB interpolation
//! - Q33: Validation - #[derive(ComputationalCapsule)] compile-time verification
//! - Q34: Auditability N/A (no state modification beyond frame counter)
//!
//! # ASSUM Framework
//! - #ASSUME: AtomicU32 stores current frame (0-59, wraps around)
//! - #VERIFY: Frame wrapping logic prevents overflow
//! - #ASSUME: Relaxed ordering sufficient (no critical inter-thread synchronization)
//! - #VERIFY: Render thread can tolerate 1-frame stale reads (<16ms latency)
//!
//! # Performance
//! - Frame reads: <5ns (single atomic load, Relaxed ordering)
//! - Frame updates: <10ns (single atomic store, Relaxed ordering)
//! - Animation latency: <16ms (60 FPS target)
//! - Cache alignment: 64B (single cache line, zero false sharing)
//!
//! # Animation Specification
//! - **Total frames**: 60 (1 second cycle at 60 FPS)
//! - **Phase 1 (0-29)**: Byzantine Purple → Gold (30 frames)
//! - **Phase 2 (30-59)**: Gold → Byzantine Purple (30 frames)
//! - **Interpolation**: Linear RGB blending
//! - **Colors**: Byzantine Purple (#663399) ↔ Gold (#FFD700)

#![warn(clippy::missing_capsule_verification)]

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Logo Animation Capsule (T1 Atomic, 64B aligned)
///
/// # Memory Layout
/// ```text
/// Offset | Field              | Size | Alignment
/// -------|-------------------|------|----------
/// 0      | frame             | 4    | 4
/// 4      | _padding1         | 4    | -
/// 8      | shutdown          | 1    | 1
/// 9-63   | _padding2         | 55   | - (pad to 64B)
/// ```
///
/// # Chaos Principles
/// - Cache-aligned (64B) - Single cache line access
/// - Atomic fields - Lockfree frame updates
/// - Zero dependencies - No external animation libraries
/// - Compile-time verified - #[derive(ComputationalCapsule)]
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct LogoAnimationCapsule {
    /// Current animation frame (0-59, wraps around)
    frame: AtomicU32,
    _padding1: [u8; 4],

    /// Shutdown signal (atomic flag for graceful termination)
    shutdown: AtomicBool,

    /// Padding to 64B
    _padding2: [u8; 55],
}

impl LogoAnimationCapsule {
    /// Create new logo animation capsule
    ///
    /// # Performance
    /// - <10ns initialization (2 atomic stores)
    /// - Zero allocation
    ///
    /// # Example
    /// ```
    /// use clapi_core::tui::LogoAnimationCapsule;
    /// let anim = LogoAnimationCapsule::new();
    /// let (r, g, b) = anim.current_color();
    /// assert_eq!((r, g, b), (102, 51, 153)); // Byzantine Purple at frame 0
    /// ```
    pub const fn new() -> Self {
        Self {
            frame: AtomicU32::new(0),
            _padding1: [0u8; 4],
            shutdown: AtomicBool::new(false),
            _padding2: [0u8; 55],
        }
    }

    /// Get current frame (0-59)
    ///
    /// # Performance
    /// - <5ns (single atomic load, Relaxed ordering)
    ///
    /// # ASSUM
    /// - #ASSUME: Relaxed ordering sufficient (no critical synchronization)
    /// - #VERIFY: Render thread tolerates 1-frame stale reads (<16ms)
    #[inline(always)]
    pub fn current_frame(&self) -> u32 {
        self.frame.load(Ordering::Relaxed)
    }

    /// Update to next frame (wraps at 60)
    ///
    /// # Performance
    /// - <10ns (atomic fetch_add + modulo)
    ///
    /// # ASSUM
    /// - #ASSUME: Wrapping prevents overflow
    /// - #VERIFY: Modulo 60 ensures frame ∈ [0, 59]
    #[inline(always)]
    pub fn update_frame(&self) {
        let next_frame = (self.frame.fetch_add(1, Ordering::Relaxed) + 1) % 60;
        self.frame.store(next_frame, Ordering::Relaxed);
    }

    /// Get current RGB color (interpolated between Byzantine Purple and Gold)
    ///
    /// # Returns
    /// (r, g, b) tuple in range [0, 255]
    ///
    /// # Performance
    /// - <50ns (atomic load + linear interpolation)
    ///
    /// # Animation Phases
    /// - Frames 0-29: Byzantine Purple (#663399) → Gold (#FFD700)
    /// - Frames 30-59: Gold (#FFD700) → Byzantine Purple (#663399)
    #[inline]
    pub fn current_color(&self) -> (u8, u8, u8) {
        let frame = self.current_frame();
        interpolate_color(frame)
    }

    /// Signal shutdown to animation thread
    ///
    /// # Performance
    /// - <5ns (single atomic store, Release ordering)
    ///
    /// # ASSUM
    /// - #ASSUME: Release ordering ensures visibility to animator thread
    /// - #VERIFY: Animator checks shutdown flag on each iteration
    #[inline(always)]
    pub fn signal_shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
    }

    /// Check if shutdown signaled
    ///
    /// # Performance
    /// - <5ns (single atomic load, Acquire ordering)
    ///
    /// # ASSUM
    /// - #ASSUME: Acquire ordering ensures visibility from shutdown signal
    #[inline(always)]
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    /// Reset animation to frame 0
    #[inline(always)]
    pub fn reset(&self) {
        self.frame.store(0, Ordering::Relaxed);
    }
}

/// Interpolate color between Byzantine Purple and Gold
///
/// # Arguments
/// - `frame`: Current animation frame (0-59)
///
/// # Returns
/// (r, g, b) tuple in range [0, 255]
///
/// # Algorithm
/// - Frames 0-29: Byzantine Purple → Gold (linear interpolation)
/// - Frames 30-59: Gold → Byzantine Purple (linear interpolation)
///
/// # Performance
/// - <30ns (integer arithmetic only, no floating point)
///
/// # Colors
/// - Byzantine Purple: #663399 (102, 51, 153)
/// - Gold: #FFD700 (255, 215, 0)
#[inline]
fn interpolate_color(frame: u32) -> (u8, u8, u8) {
    const PURPLE_R: u32 = 102;
    const PURPLE_G: u32 = 51;
    const PURPLE_B: u32 = 153;

    const GOLD_R: u32 = 255;
    const GOLD_G: u32 = 215;
    const GOLD_B: u32 = 0;

    if frame < 30 {
        // Phase 1: Purple → Gold (frames 0-29)
        let t = frame; // 0-29
        let r = PURPLE_R + (GOLD_R - PURPLE_R) * t / 29;
        let g = PURPLE_G + (GOLD_G - PURPLE_G) * t / 29;
        let b = PURPLE_B.saturating_sub((PURPLE_B - GOLD_B) * t / 29);
        (r as u8, g as u8, b as u8)
    } else {
        // Phase 2: Gold → Purple (frames 30-59)
        let t = frame - 30; // 0-29
        let r = GOLD_R.saturating_sub((GOLD_R - PURPLE_R) * t / 29);
        let g = GOLD_G.saturating_sub((GOLD_G - PURPLE_G) * t / 29);
        let b = GOLD_B + (PURPLE_B - GOLD_B) * t / 29;
        (r as u8, g as u8, b as u8)
    }
}

/// Spawn background animation thread
///
/// # Arguments
/// - `capsule`: Arc-wrapped LogoAnimationCapsule
///
/// # Returns
/// JoinHandle for graceful shutdown
///
/// # Performance
/// - Thread overhead: <1ms (one-time cost)
/// - Frame update latency: <10ns per iteration
/// - Target FPS: 60 (16.6ms per frame)
///
/// # Shutdown
/// Call `capsule.signal_shutdown()` to gracefully terminate thread.
/// Join handle to wait for completion: `handle.join()`.
///
/// # Example
/// ```no_run
/// use clapi_core::tui::{LogoAnimationCapsule, spawn_logo_animator};
/// use std::sync::Arc;
///
/// let capsule = Arc::new(LogoAnimationCapsule::new());
/// let handle = spawn_logo_animator(capsule.clone());
///
/// // ... render loop reads capsule.current_color() ...
///
/// // Shutdown
/// capsule.signal_shutdown();
/// handle.join().unwrap();
/// ```
pub fn spawn_logo_animator(capsule: Arc<LogoAnimationCapsule>) -> JoinHandle<()> {
    thread::spawn(move || {
        // Animation loop: 60 FPS (16.6ms per frame)
        const FRAME_DURATION: Duration = Duration::from_millis(16); // 16ms ≈ 60 FPS

        loop {
            // Check shutdown signal
            if capsule.is_shutdown() {
                break;
            }

            // Update frame (lockfree, <10ns)
            capsule.update_frame();

            // Sleep until next frame
            thread::sleep(FRAME_DURATION);
        }
    })
}

impl Default for LogoAnimationCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_alignment() {
        // Verify capsule properties
        assert_eq!(std::mem::size_of::<LogoAnimationCapsule>(), 64);
        assert_eq!(std::mem::align_of::<LogoAnimationCapsule>(), 64);
    }

    #[test]
    fn test_frame_wrapping() {
        let capsule = LogoAnimationCapsule::new();
        assert_eq!(capsule.current_frame(), 0);

        // Update 60 times (full cycle)
        for _ in 0..60 {
            capsule.update_frame();
        }

        // Should wrap back to 0
        assert_eq!(capsule.current_frame(), 0);
    }

    #[test]
    fn test_color_interpolation_purple_to_gold() {
        // Frame 0: Byzantine Purple (#663399)
        let (r, g, b) = interpolate_color(0);
        assert_eq!((r, g, b), (102, 51, 153));

        // Frame 29: Gold (#FFD700)
        let (r, g, b) = interpolate_color(29);
        assert_eq!((r, g, b), (255, 215, 0));
    }

    #[test]
    fn test_color_interpolation_gold_to_purple() {
        // Frame 30: Gold (#FFD700)
        let (r, g, b) = interpolate_color(30);
        assert_eq!((r, g, b), (255, 215, 0));

        // Frame 59: Byzantine Purple (#663399)
        let (r, g, b) = interpolate_color(59);
        assert_eq!((r, g, b), (102, 51, 153));
    }

    #[test]
    fn test_color_interpolation_midpoint() {
        // Frame 15: Midpoint Purple→Gold
        let (r15, g15, b15) = interpolate_color(15);

        // Should be roughly halfway between purple and gold
        // Purple: (102, 51, 153), Gold: (255, 215, 0)
        // Midpoint: (178, 133, 76)
        assert!((r15 as i32 - 178).abs() <= 10); // Allow ±10 tolerance
        assert!((g15 as i32 - 133).abs() <= 10);
        assert!((b15 as i32 - 76).abs() <= 10);

        // Frame 45: Midpoint Gold→Purple
        let (r45, g45, b45) = interpolate_color(45);

        // Should be roughly halfway between gold and purple
        assert!((r45 as i32 - 178).abs() <= 10);
        assert!((g45 as i32 - 133).abs() <= 10);
        assert!((b45 as i32 - 76).abs() <= 10);
    }

    #[test]
    fn test_shutdown_signal() {
        let capsule = LogoAnimationCapsule::new();
        assert!(!capsule.is_shutdown());

        capsule.signal_shutdown();
        assert!(capsule.is_shutdown());
    }

    #[test]
    fn test_reset() {
        let capsule = LogoAnimationCapsule::new();

        // Advance to frame 30
        for _ in 0..30 {
            capsule.update_frame();
        }
        assert_eq!(capsule.current_frame(), 30);

        // Reset
        capsule.reset();
        assert_eq!(capsule.current_frame(), 0);
    }

    #[test]
    fn test_animation_thread() {
        use std::time::Duration;

        let capsule = Arc::new(LogoAnimationCapsule::new());
        let handle = spawn_logo_animator(capsule.clone());

        // Let it run for ~100ms (should advance ~6 frames at 60 FPS)
        thread::sleep(Duration::from_millis(100));

        // Verify frame advanced
        let frame = capsule.current_frame();
        assert!(frame >= 4 && frame <= 8, "Expected 4-8 frames, got {}", frame);

        // Shutdown
        capsule.signal_shutdown();
        handle.join().unwrap();
    }

    #[test]
    fn test_concurrent_read_write() {
        use std::sync::Arc;

        let capsule = Arc::new(LogoAnimationCapsule::new());
        let reader = capsule.clone();

        // Spawn animator thread
        let animator_handle = spawn_logo_animator(capsule.clone());

        // Spawn reader thread
        let reader_handle = thread::spawn(move || {
            for _ in 0..100 {
                let _color = reader.current_color();
                thread::sleep(Duration::from_millis(5));
            }
        });

        // Let both run for 500ms
        thread::sleep(Duration::from_millis(500));

        // Shutdown
        capsule.signal_shutdown();
        animator_handle.join().unwrap();
        reader_handle.join().unwrap();

        // No panics = success (lockfree concurrent access)
    }
}
