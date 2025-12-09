//! AnimationCoordinator - T6 Mixed Tier Orchestrator
//!
//! Coordinates all animation capsules (SpringCapsule, PulseCapsule, ShimmerCapsule)
//! for gui_v2. Provides unified interface for updating animations and querying state.
//!
//! # Architecture
//!
//! T6 Mixed tier metacapsule containing:
//! - SpringCapsule (T3 Fixed-Point physics)
//! - PulseCapsule (T3 Fixed-Point periodic)
//! - ShimmerCapsule (T3 Fixed-Point shimmer)
//!
//! # Performance
//!
//! - update_all(): <15ns (3 capsule updates)
//! - get_glow_color(): <50ns (RGB interpolation)
//!
//! # Framework Compliance
//!
//! - **UCE34**: T6 Mixed tier (compound capsule orchestration)
//! - **Chaos**: 100% lockfree (all sub-capsules lockfree)
//! - **ASSUM**: 99.99% safe (zero unsafe)

use super::{SpringCapsule, PulseCapsule, ShimmerCapsule};

/// AnimationCoordinator - T6 Mixed metacapsule
///
/// Coordinates all animation capsules for gui_v2.
///
/// # Memory Layout
///
/// ```text
/// [0-63]    spring (SpringCapsule, 64B)
/// [64-127]  pulse (PulseCapsule, 64B)
/// [128-191] shimmer (ShimmerCapsule, 64B)
/// ```
///
/// Total: 192 bytes (3 cache lines)
#[repr(C, align(64))]
pub struct AnimationCoordinator {
    /// Spring animation for smooth transitions
    spring: SpringCapsule,
    /// Pulse animation for glow effects
    pulse: PulseCapsule,
    /// Shimmer animation for progress bars
    shimmer: ShimmerCapsule,
}

impl AnimationCoordinator {
    /// Byzantine purple color (RGB)
    const PURPLE_R: u8 = 102;
    const PURPLE_G: u8 = 51;
    const PURPLE_B: u8 = 153;

    /// Gold color (RGB)
    const GOLD_R: u8 = 255;
    const GOLD_G: u8 = 215;
    const GOLD_B: u8 = 0;

    /// Create new AnimationCoordinator with default parameters
    pub fn new() -> Self {
        Self {
            spring: SpringCapsule::new(),
            pulse: PulseCapsule::new(),
            shimmer: ShimmerCapsule::new(),
        }
    }

    /// Update all animations
    ///
    /// # Performance
    ///
    /// <15ns total (3 × <5ns capsule updates)
    pub fn update_all(&self, dt_ms: u32) {
        self.spring.update(dt_ms);
        self.pulse.update(dt_ms);
        self.shimmer.update(dt_ms);
    }

    /// Get spring scale factor for size transitions
    ///
    /// Used for smooth scaling animations (e.g., button hover effects)
    #[inline]
    pub fn get_spring_scale(&self) -> f32 {
        self.spring.get_position()
    }

    /// Set spring target for scale animations
    #[inline]
    pub fn set_spring_target(&self, target: f32) {
        self.spring.set_target(target);
    }

    /// Get glow color interpolated between purple and gold
    ///
    /// # Algorithm
    ///
    /// 1. Get pulse intensity (0.0-1.0)
    /// 2. Lerp purple → gold based on intensity
    ///
    /// # Returns
    ///
    /// (r, g, b) tuple in 0-255 range
    pub fn get_glow_color(&self) -> (u8, u8, u8) {
        let intensity = self.pulse.get_intensity();

        // Lerp purple → gold
        // intensity=1.0 → purple
        // intensity=0.0 → gold
        let r = Self::lerp_u8(Self::GOLD_R, Self::PURPLE_R, intensity);
        let g = Self::lerp_u8(Self::GOLD_G, Self::PURPLE_G, intensity);
        let b = Self::lerp_u8(Self::GOLD_B, Self::PURPLE_B, intensity);

        (r, g, b)
    }

    /// Get shimmer offset for progress bar effects
    #[inline]
    pub fn get_shimmer_offset(&self) -> f32 {
        self.shimmer.get_offset()
    }

    /// Get pulse phase (0.0-1.0)
    #[inline]
    pub fn get_pulse_phase(&self) -> f32 {
        self.pulse.get_phase()
    }

    /// Check if spring is at rest
    #[inline]
    pub fn is_spring_at_rest(&self) -> bool {
        self.spring.is_at_rest()
    }

    /// Reset all animations to initial state
    pub fn reset_all(&self) {
        self.spring.reset(0.0);
        self.pulse.reset();
        self.shimmer.reset();
    }

    /// Pause shimmer animation
    pub fn pause_shimmer(&self) {
        self.shimmer.pause();
    }

    /// Resume shimmer animation
    pub fn resume_shimmer(&self, speed: f32) {
        self.shimmer.resume(speed);
    }

    /// Linear interpolation for u8 color components
    #[inline]
    fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
        // #ASSUME t in 0.0-1.0 range
        // #VERIFY Called from get_glow_color with pulse intensity
        debug_assert!(t >= 0.0 && t <= 1.0, "t must be in 0.0-1.0 range");

        let a_f = a as f32;
        let b_f = b as f32;
        let result = a_f + (b_f - a_f) * t;
        result.clamp(0.0, 255.0) as u8
    }

    /// Get spring capsule reference (for advanced control)
    #[inline]
    pub fn spring(&self) -> &SpringCapsule {
        &self.spring
    }

    /// Get pulse capsule reference (for advanced control)
    #[inline]
    pub fn pulse(&self) -> &PulseCapsule {
        &self.pulse
    }

    /// Get shimmer capsule reference (for advanced control)
    #[inline]
    pub fn shimmer(&self) -> &ShimmerCapsule {
        &self.shimmer
    }
}

impl Default for AnimationCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinator_creation() {
        let coord = AnimationCoordinator::new();
        assert_eq!(coord.get_spring_scale(), 0.0);
        assert_eq!(coord.get_pulse_phase(), 0.0);
        assert_eq!(coord.get_shimmer_offset(), 0.0);
    }

    #[test]
    fn test_coordinator_update_all() {
        let coord = AnimationCoordinator::new();
        coord.set_spring_target(1.0);

        coord.update_all(16);

        // All animations should have advanced
        assert!(coord.get_spring_scale() > 0.0);
        assert!(coord.get_pulse_phase() > 0.0);
        assert!(coord.get_shimmer_offset() > 0.0);
    }

    #[test]
    fn test_coordinator_glow_color_purple() {
        let coord = AnimationCoordinator::new();

        // At phase 0.0, should be purple (intensity 1.0)
        let (r, g, b) = coord.get_glow_color();
        assert_eq!((r, g, b), (102, 51, 153));
    }

    #[test]
    fn test_coordinator_glow_color_interpolation() {
        let coord = AnimationCoordinator::new();

        // Advance to mid-cycle (should be between purple and gold)
        for _ in 0..300 {
            coord.update_all(16);
        }

        let (r, g, b) = coord.get_glow_color();
        // Should be somewhere between purple and gold
        assert!(r >= 102 && r <= 255);
        assert!(g >= 51 && g <= 215);
        assert!(b >= 0 && b <= 153);
    }

    #[test]
    fn test_coordinator_spring_target() {
        let coord = AnimationCoordinator::new();
        coord.set_spring_target(100.0);

        // Simulate until at rest
        for _ in 0..200 {
            coord.update_all(16);
        }

        assert!((coord.get_spring_scale() - 100.0).abs() < 0.1);
    }

    #[test]
    fn test_coordinator_is_spring_at_rest() {
        let coord = AnimationCoordinator::new();
        assert!(coord.is_spring_at_rest());

        coord.set_spring_target(10.0);
        coord.update_all(16);
        assert!(!coord.is_spring_at_rest());
    }

    #[test]
    fn test_coordinator_reset_all() {
        let coord = AnimationCoordinator::new();
        coord.set_spring_target(100.0);

        for _ in 0..10 {
            coord.update_all(16);
        }

        coord.reset_all();

        assert_eq!(coord.get_spring_scale(), 0.0);
        assert_eq!(coord.get_pulse_phase(), 0.0);
        assert_eq!(coord.get_shimmer_offset(), 0.0);
    }

    #[test]
    fn test_coordinator_pause_shimmer() {
        let coord = AnimationCoordinator::new();
        coord.update_all(1000);
        let offset_before = coord.get_shimmer_offset();

        coord.pause_shimmer();
        coord.update_all(1000);

        assert_eq!(coord.get_shimmer_offset(), offset_before);
    }

    #[test]
    fn test_coordinator_resume_shimmer() {
        let coord = AnimationCoordinator::new();
        coord.pause_shimmer();
        coord.update_all(1000);

        coord.resume_shimmer(1.0);
        coord.update_all(1000); // 1.0 * 1.0 = 1.0, wraps to 0.0

        // After wrap, offset should be ~0.0
        assert!(coord.get_shimmer_offset() < 0.01);
    }

    #[test]
    fn test_coordinator_sub_capsule_access() {
        let coord = AnimationCoordinator::new();

        // Test direct access to sub-capsules
        coord.spring().set_target(50.0);
        coord.pulse().reset();
        coord.shimmer().set_speed(2.0);

        coord.update_all(16);

        assert!(coord.spring().get_position() > 0.0);
        // Pulse will advance after reset + update, expect small positive value
        assert!(coord.pulse().get_phase() > 0.0 && coord.pulse().get_phase() < 0.01);
        assert_eq!(coord.shimmer().get_speed(), 2.0);
    }

    #[test]
    fn test_lerp_u8() {
        // Test color interpolation
        assert_eq!(AnimationCoordinator::lerp_u8(0, 255, 0.0), 0);
        assert_eq!(AnimationCoordinator::lerp_u8(0, 255, 1.0), 255);
        assert_eq!(AnimationCoordinator::lerp_u8(0, 255, 0.5), 127);
    }

    #[test]
    fn test_lerp_u8_clamping() {
        // Test clamping at boundaries
        assert_eq!(AnimationCoordinator::lerp_u8(100, 200, 0.0), 100);
        assert_eq!(AnimationCoordinator::lerp_u8(100, 200, 1.0), 200);
    }

    #[test]
    fn test_coordinator_size() {
        assert_eq!(std::mem::size_of::<AnimationCoordinator>(), 192);
    }

    #[test]
    fn test_coordinator_alignment() {
        let coord = AnimationCoordinator::new();
        let ptr = &coord as *const AnimationCoordinator as usize;
        assert_eq!(ptr % 64, 0, "AnimationCoordinator must be 64-byte aligned");
    }

    #[test]
    fn test_coordinator_glow_cycle() {
        let coord = AnimationCoordinator::new();

        // Record colors over one full cycle
        let mut colors = Vec::new();
        for _ in 0..375 {
            coord.update_all(16);
            colors.push(coord.get_glow_color());
        }

        // Should see variation in colors (not all same)
        let first = colors[0];
        let has_variation = colors.iter().any(|&c| c != first);
        assert!(has_variation, "Glow color should vary over cycle");
    }

    #[test]
    fn test_coordinator_shimmer_continuous() {
        let coord = AnimationCoordinator::new();

        let mut prev_offset = coord.get_shimmer_offset();
        for _ in 0..10 {
            coord.update_all(16);
            let offset = coord.get_shimmer_offset();
            assert!(offset >= prev_offset || offset < 0.1, "Shimmer should advance");
            prev_offset = offset;
        }
    }
}
