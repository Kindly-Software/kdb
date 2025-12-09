//! PulseCapsule - Periodic Glow Animation
//!
//! Lockfree periodic animation for glow effects. Implements 6-second cycle matching
//! gui_v2 design: 2 seconds purple dwell, 4 seconds fade to gold and back.
//!
//! # Animation Cycle
//!
//! - Phase 0.0-0.33: Purple dwell (2 seconds)
//! - Phase 0.33-0.67: Fade purple → gold (2 seconds)
//! - Phase 0.67-1.0: Fade gold → purple (2 seconds)
//!
//! # Performance
//!
//! - update(): <3ns (Q16.16 add + wrap)
//! - get_phase(): <1ns (atomic load + shift)
//!
//! # Framework Compliance
//!
//! - **UCE34**: T3 Fixed-Point tier (deterministic timing)
//! - **Chaos**: 100% lockfree (AtomicU32, 64B aligned)

use std::sync::atomic::{AtomicU32, Ordering};
use super::{to_fixed, from_fixed, FIXED_POINT_ONE};

/// PulseCapsule - Periodic glow animation
///
/// 64-byte cache-aligned capsule for lockfree periodic animations.
///
/// # Memory Layout
///
/// ```text
/// [0-3]   phase (AtomicU32, Q16.16, 0.0-1.0)
/// [4-7]   period_ms (AtomicU32, milliseconds)
/// [8-63]  padding (56 bytes)
/// ```
#[repr(C, align(64))]
pub struct PulseCapsule {
    /// Current phase (Q16.16, 0.0-1.0)
    phase: AtomicU32,
    /// Period in milliseconds (6000ms = 6 seconds)
    period_ms: AtomicU32,
    /// Padding to 64 bytes
    _padding: [u8; 56],
}

impl PulseCapsule {
    /// Default period: 6 seconds (matches gui_v2 glow cycle)
    const DEFAULT_PERIOD_MS: u32 = 6000;

    /// Purple dwell phase threshold (0.33 = 2 seconds)
    const PURPLE_DWELL_PHASE: f32 = 0.33;

    /// Gold midpoint phase (0.5 = 3 seconds)
    const GOLD_PHASE: f32 = 0.5;

    /// Create new PulseCapsule with default 6-second period
    pub fn new() -> Self {
        Self {
            phase: AtomicU32::new(0),
            period_ms: AtomicU32::new(Self::DEFAULT_PERIOD_MS),
            _padding: [0; 56],
        }
    }

    /// Create PulseCapsule with custom period
    pub fn with_period(period_ms: u32) -> Self {
        Self {
            phase: AtomicU32::new(0),
            period_ms: AtomicU32::new(period_ms),
            _padding: [0; 56],
        }
    }

    /// Update pulse animation
    ///
    /// # Algorithm
    ///
    /// 1. Calculate phase increment: dt_ms / period_ms
    /// 2. Add to current phase
    /// 3. Wrap at 1.0 (modulo)
    ///
    /// # Performance
    ///
    /// <3ns per call (Q16.16 add + conditional)
    pub fn update(&self, dt_ms: u32) {
        // #ASSUME dt_ms <= period_ms (frame time less than or equal to full cycle)
        // #VERIFY Called from GUI event loop (<16ms typical, period=6000ms)
        debug_assert!(dt_ms <= self.period_ms.load(Ordering::Relaxed));

        let period = self.period_ms.load(Ordering::Relaxed);
        let current_phase = self.phase.load(Ordering::Relaxed) as i32;

        // Calculate phase increment (dt_ms / period_ms in Q16.16)
        // phase_inc = (dt_ms << 16) / period
        let phase_inc = ((dt_ms as i64) << 16) / (period as i64);

        // Add to current phase
        let new_phase = current_phase + phase_inc as i32;

        // Wrap at 1.0 (FIXED_POINT_ONE)
        let wrapped_phase = if new_phase >= FIXED_POINT_ONE {
            new_phase - FIXED_POINT_ONE
        } else {
            new_phase
        };

        self.phase.store(wrapped_phase as u32, Ordering::Relaxed);
    }

    /// Get current phase (0.0-1.0)
    #[inline]
    pub fn get_phase(&self) -> f32 {
        let phase = self.phase.load(Ordering::Relaxed) as i32;
        from_fixed(phase)
    }

    /// Get glow intensity (0.0-1.0) based on phase
    ///
    /// # Cycle
    ///
    /// - Phase 0.0-0.33: Intensity 1.0 (purple dwell)
    /// - Phase 0.33-0.67: Fade 1.0 → 0.0 (to gold)
    /// - Phase 0.67-1.0: Fade 0.0 → 1.0 (back to purple)
    pub fn get_intensity(&self) -> f32 {
        let phase = self.get_phase();

        if phase < Self::PURPLE_DWELL_PHASE {
            // Purple dwell (0.0-0.33)
            1.0
        } else if phase < 2.0 * Self::PURPLE_DWELL_PHASE {
            // Fade to gold (0.33-0.67)
            let fade_phase = (phase - Self::PURPLE_DWELL_PHASE) / Self::PURPLE_DWELL_PHASE;
            1.0 - fade_phase
        } else {
            // Fade back to purple (0.67-1.0)
            let fade_phase = (phase - 2.0 * Self::PURPLE_DWELL_PHASE) / (1.0 - 2.0 * Self::PURPLE_DWELL_PHASE);
            fade_phase
        }
    }

    /// Reset pulse to beginning of cycle
    pub fn reset(&self) {
        self.phase.store(0, Ordering::Relaxed);
    }

    /// Set period in milliseconds
    pub fn set_period(&self, period_ms: u32) {
        self.period_ms.store(period_ms, Ordering::Relaxed);
    }
}

impl Default for PulseCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pulse_creation() {
        let pulse = PulseCapsule::new();
        assert_eq!(pulse.get_phase(), 0.0);
    }

    #[test]
    fn test_pulse_update() {
        let pulse = PulseCapsule::new();

        // Update by 1 second (1000ms / 6000ms = 0.1667)
        pulse.update(1000);
        let phase = pulse.get_phase();
        assert!((phase - 0.1667).abs() < 0.01);
    }

    #[test]
    fn test_pulse_wraps_at_one() {
        let pulse = PulseCapsule::new();

        // Update by full period
        pulse.update(6000);
        assert!((pulse.get_phase() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_pulse_incremental_updates() {
        let pulse = PulseCapsule::new();

        // Simulate 60fps for 1 second
        for _ in 0..60 {
            pulse.update(16);
        }

        // Should be around 0.16 (960ms / 6000ms)
        let phase = pulse.get_phase();
        assert!((phase - 0.16).abs() < 0.02);
    }

    #[test]
    fn test_pulse_custom_period() {
        let pulse = PulseCapsule::with_period(1000); // 1 second

        pulse.update(500); // Half period
        assert!((pulse.get_phase() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_pulse_reset() {
        let pulse = PulseCapsule::new();
        pulse.update(1000);
        pulse.reset();
        assert_eq!(pulse.get_phase(), 0.0);
    }

    #[test]
    fn test_pulse_intensity_purple_dwell() {
        let pulse = PulseCapsule::new();
        // Phase 0.0-0.33 should be full intensity
        assert_eq!(pulse.get_intensity(), 1.0);

        pulse.update(1000); // Phase ~0.167
        assert_eq!(pulse.get_intensity(), 1.0);
    }

    #[test]
    fn test_pulse_intensity_fade_to_gold() {
        let pulse = PulseCapsule::new();
        pulse.update(3000); // Phase 0.5 (midpoint)

        let intensity = pulse.get_intensity();
        // Should be somewhere between 0.0 and 1.0
        assert!(intensity >= 0.0 && intensity <= 1.0);
    }

    #[test]
    fn test_pulse_intensity_fade_to_purple() {
        let pulse = PulseCapsule::new();
        pulse.update(5000); // Phase ~0.833

        let intensity = pulse.get_intensity();
        // Should be fading back up
        assert!(intensity > 0.0);
    }

    #[test]
    fn test_pulse_set_period() {
        let pulse = PulseCapsule::new();
        pulse.set_period(2000);

        pulse.update(1000); // Half of new period
        assert!((pulse.get_phase() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_pulse_size() {
        assert_eq!(std::mem::size_of::<PulseCapsule>(), 64);
    }

    #[test]
    fn test_pulse_alignment() {
        let pulse = PulseCapsule::new();
        let ptr = &pulse as *const PulseCapsule as usize;
        assert_eq!(ptr % 64, 0, "PulseCapsule must be 64-byte aligned");
    }
}
