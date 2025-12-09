//! Interactive state capsule for keyboard-driven CLI control
//!
//! # Tier: T1 Atomic
//!
//! 64-byte cache-aligned lockfree capsule for real-time interactive control.
//! Supports pause/resume, GPU toggling, cancellation, and CRF adjustment.
//!
//! # Layout (64 bytes)
//! - state: AtomicU64 (8 bytes) - packed control flags + generation
//! - _padding: [u8; 56] - cache alignment to 64 bytes
//!
//! # State Bit Layout (AtomicU64)
//! - Bit 0: paused flag
//! - Bit 1: cancelled flag
//! - Bit 2: gpu_enabled flag
//! - Bit 3: menu_open flag
//! - Bit 4: wizard_active flag
//! - Bits 5-7: wizard_step (3 bits, 0-7)
//! - Bits 8-15: crf_adjustment (i8, -10 to +10)
//! - Bits 16-63: generation counter (48 bits)
//!
//! # Performance
//! - Read operations: <5ns (single atomic load)
//! - Mutation operations: <10ns (CAS loop)
//! - Snapshot: <5ns (single atomic load + unpacking)
//!
//! # Chaos Compliance
//! - 100% lockfree (AtomicU64 only)
//! - 64B cache-aligned
//! - Generation counter increments on every mutation
//! - Acquire/Release memory ordering

use std::sync::atomic::{AtomicU64, Ordering};

// Bit masks for state packing
const PAUSED_BIT: u64 = 1 << 0;
const CANCELLED_BIT: u64 = 1 << 1;
const GPU_ENABLED_BIT: u64 = 1 << 2;
const MENU_OPEN_BIT: u64 = 1 << 3;
const WIZARD_ACTIVE_BIT: u64 = 1 << 4;
const WIZARD_STEP_MASK: u64 = 0x7 << 5; // Bits 5-7 (3 bits for 0-7)
const WIZARD_STEP_SHIFT: u32 = 5;
const CRF_MASK: u64 = 0xFF << 8; // Bits 8-15
const CRF_SHIFT: u32 = 8;
const GENERATION_MASK: u64 = 0xFFFF_FFFF_FFFF << 16; // Bits 16-63
const GENERATION_SHIFT: u32 = 16;

// CRF bounds
const CRF_MIN: i8 = -10;
const CRF_MAX: i8 = 10;

/// Interactive state snapshot (atomic point-in-time view)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteractiveSnapshot {
    pub paused: bool,
    pub cancelled: bool,
    pub gpu_enabled: bool,
    pub menu_open: bool,
    pub wizard_active: bool,
    pub wizard_step: u8,
    pub crf_adjustment: i8,
    pub generation: u64,
}

/// Interactive state capsule for keyboard control (64B, T1 Atomic)
///
/// Provides lockfree atomic control for:
/// - Pause/resume encoding
/// - GPU enable/disable
/// - Cancellation requests
/// - CRF quality adjustment (-10 to +10)
/// - Menu overlay (open/close)
/// - Wizard flow (8-step navigation)
///
/// # Examples
///
/// ```
/// use kindly_av1::progress::InteractiveStateCapsule;
///
/// let state = InteractiveStateCapsule::new();
///
/// // Toggle pause
/// state.toggle_pause();
/// assert!(state.is_paused());
///
/// // Adjust CRF quality
/// state.adjust_crf(5); // Increase CRF by 5
/// assert_eq!(state.crf_adjustment(), 5);
///
/// // Menu overlay
/// state.open_menu();
/// assert!(state.is_menu_open());
/// state.close_menu();
///
/// // Wizard flow
/// state.start_wizard();
/// assert_eq!(state.wizard_step(), 0);
/// state.wizard_next();
/// assert_eq!(state.wizard_step(), 1);
/// state.finish_wizard();
///
/// // Atomic snapshot
/// let snapshot = state.snapshot();
/// assert_eq!(snapshot.paused, true);
/// assert_eq!(snapshot.crf_adjustment, 5);
/// assert_eq!(snapshot.menu_open, false);
/// assert_eq!(snapshot.wizard_active, false);
/// ```
#[repr(C, align(64))]
pub struct InteractiveStateCapsule {
    /// Packed state: flags (bits 0-7) | crf_adjustment (bits 8-15) | generation (bits 16-63)
    state: AtomicU64,
    /// Padding to 64 bytes for cache alignment
    _padding: [u8; 56],
}

impl InteractiveStateCapsule {
    /// Create new interactive state with defaults
    ///
    /// Defaults:
    /// - paused: false
    /// - cancelled: false
    /// - gpu_enabled: true
    /// - crf_adjustment: 0
    /// - generation: 0
    pub fn new() -> Self {
        // Initial state: gpu_enabled=true, all else false/zero
        let initial = GPU_ENABLED_BIT;

        Self {
            state: AtomicU64::new(initial),
            _padding: [0; 56],
        }
    }

    /// Check if encoding is paused
    ///
    /// # Performance
    /// <5ns (single atomic load)
    #[inline]
    pub fn is_paused(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        (state & PAUSED_BIT) != 0
    }

    /// Check if cancellation requested
    ///
    /// # Performance
    /// <5ns (single atomic load)
    #[inline]
    pub fn is_cancelled(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        (state & CANCELLED_BIT) != 0
    }

    /// Check if GPU acceleration enabled
    ///
    /// # Performance
    /// <5ns (single atomic load)
    #[inline]
    pub fn is_gpu_enabled(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        (state & GPU_ENABLED_BIT) != 0
    }

    /// Get current CRF adjustment
    ///
    /// # Performance
    /// <5ns (single atomic load + bit extraction)
    #[inline]
    pub fn crf_adjustment(&self) -> i8 {
        let state = self.state.load(Ordering::Acquire);
        let crf_bits = ((state & CRF_MASK) >> CRF_SHIFT) as u8;
        crf_bits as i8
    }

    /// Get current generation counter
    ///
    /// # Performance
    /// <5ns (single atomic load + bit extraction)
    #[inline]
    pub fn generation(&self) -> u64 {
        let state = self.state.load(Ordering::Acquire);
        (state & GENERATION_MASK) >> GENERATION_SHIFT
    }

    /// Toggle pause state (pause ↔ resume)
    ///
    /// # Performance
    /// <10ns (CAS loop, typically 1 iteration)
    pub fn toggle_pause(&self) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let paused = (current & PAUSED_BIT) != 0;
            let new_state = if paused {
                current & !PAUSED_BIT // Clear pause bit
            } else {
                current | PAUSED_BIT // Set pause bit
            };

            // Increment generation
            let new_state = self.increment_generation(new_state);

            match self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    /// Request cancellation (sets cancelled flag)
    ///
    /// # Performance
    /// <10ns (CAS loop, typically 1 iteration)
    pub fn request_cancel(&self) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let new_state = current | CANCELLED_BIT;
            let new_state = self.increment_generation(new_state);

            match self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    /// Toggle GPU acceleration (enabled ↔ disabled)
    ///
    /// # Performance
    /// <10ns (CAS loop, typically 1 iteration)
    pub fn toggle_gpu(&self) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let gpu_enabled = (current & GPU_ENABLED_BIT) != 0;
            let new_state = if gpu_enabled {
                current & !GPU_ENABLED_BIT // Disable GPU
            } else {
                current | GPU_ENABLED_BIT // Enable GPU
            };

            // Increment generation
            let new_state = self.increment_generation(new_state);

            match self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    /// Adjust CRF value by delta (clamped to [-10, +10])
    ///
    /// # Arguments
    /// - `delta`: Change to apply (positive or negative)
    ///
    /// # Performance
    /// <10ns (CAS loop, typically 1 iteration)
    pub fn adjust_crf(&self, delta: i8) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let current_crf = ((current & CRF_MASK) >> CRF_SHIFT) as i8;
            let new_crf = (current_crf + delta).clamp(CRF_MIN, CRF_MAX);

            // Clear old CRF and set new CRF
            let mut new_state = current & !CRF_MASK;
            new_state |= ((new_crf as u8 as u64) << CRF_SHIFT) & CRF_MASK;

            // Increment generation
            let new_state = self.increment_generation(new_state);

            match self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    /// Check if menu overlay is open
    ///
    /// # Performance
    /// <5ns (single atomic load)
    #[inline]
    pub fn is_menu_open(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        (state & MENU_OPEN_BIT) != 0
    }

    /// Check if wizard is active
    ///
    /// # Performance
    /// <5ns (single atomic load)
    #[inline]
    pub fn is_wizard_active(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        (state & WIZARD_ACTIVE_BIT) != 0
    }

    /// Get current wizard step (0-7)
    ///
    /// # Performance
    /// <5ns (single atomic load + bit extraction)
    #[inline]
    pub fn wizard_step(&self) -> u8 {
        let state = self.state.load(Ordering::Acquire);
        ((state & WIZARD_STEP_MASK) >> WIZARD_STEP_SHIFT) as u8
    }

    /// Open menu overlay
    ///
    /// # Performance
    /// <10ns (CAS loop, typically 1 iteration)
    pub fn open_menu(&self) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let new_state = current | MENU_OPEN_BIT;
            let new_state = self.increment_generation(new_state);

            match self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    /// Close menu overlay
    ///
    /// # Performance
    /// <10ns (CAS loop, typically 1 iteration)
    pub fn close_menu(&self) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let new_state = current & !MENU_OPEN_BIT;
            let new_state = self.increment_generation(new_state);

            match self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    /// Start wizard flow (sets wizard_active=true, wizard_step=0)
    ///
    /// # Performance
    /// <10ns (CAS loop, typically 1 iteration)
    pub fn start_wizard(&self) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            // Set wizard_active=true, wizard_step=0
            let mut new_state = current | WIZARD_ACTIVE_BIT;
            new_state &= !WIZARD_STEP_MASK; // Clear wizard step bits (sets to 0)
            let new_state = self.increment_generation(new_state);

            match self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    /// Advance wizard to next step (wraps at step 7 → 0)
    ///
    /// # Performance
    /// <10ns (CAS loop, typically 1 iteration)
    pub fn wizard_next(&self) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let current_step = ((current & WIZARD_STEP_MASK) >> WIZARD_STEP_SHIFT) as u8;
            let next_step = (current_step + 1) & 0x7; // Wrap at 7

            // Clear old step and set new step
            let mut new_state = current & !WIZARD_STEP_MASK;
            new_state |= ((next_step as u64) << WIZARD_STEP_SHIFT) & WIZARD_STEP_MASK;
            let new_state = self.increment_generation(new_state);

            match self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    /// Go to previous wizard step (wraps at step 0 → 7)
    ///
    /// # Performance
    /// <10ns (CAS loop, typically 1 iteration)
    pub fn wizard_prev(&self) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let current_step = ((current & WIZARD_STEP_MASK) >> WIZARD_STEP_SHIFT) as u8;
            let prev_step = current_step.wrapping_sub(1) & 0x7; // Wrap at 0

            // Clear old step and set new step
            let mut new_state = current & !WIZARD_STEP_MASK;
            new_state |= ((prev_step as u64) << WIZARD_STEP_SHIFT) & WIZARD_STEP_MASK;
            let new_state = self.increment_generation(new_state);

            match self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    /// Finish wizard flow (clears wizard_active flag)
    ///
    /// # Performance
    /// <10ns (CAS loop, typically 1 iteration)
    pub fn finish_wizard(&self) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let new_state = current & !WIZARD_ACTIVE_BIT;
            let new_state = self.increment_generation(new_state);

            match self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    /// Get atomic snapshot of all state
    ///
    /// # Performance
    /// <5ns (single atomic load + unpacking)
    #[inline]
    pub fn snapshot(&self) -> InteractiveSnapshot {
        let state = self.state.load(Ordering::Acquire);

        InteractiveSnapshot {
            paused: (state & PAUSED_BIT) != 0,
            cancelled: (state & CANCELLED_BIT) != 0,
            gpu_enabled: (state & GPU_ENABLED_BIT) != 0,
            menu_open: (state & MENU_OPEN_BIT) != 0,
            wizard_active: (state & WIZARD_ACTIVE_BIT) != 0,
            wizard_step: ((state & WIZARD_STEP_MASK) >> WIZARD_STEP_SHIFT) as u8,
            crf_adjustment: ((state & CRF_MASK) >> CRF_SHIFT) as i8,
            generation: (state & GENERATION_MASK) >> GENERATION_SHIFT,
        }
    }

    /// Increment generation counter in state value
    ///
    /// # ASSUME
    /// #ASSUME: Generation counter will not overflow (48 bits = 281 trillion)
    /// #VERIFY: For CLI usage at <1000 updates/sec, overflow after 8.9 million years
    #[inline]
    fn increment_generation(&self, state: u64) -> u64 {
        let generation = (state & GENERATION_MASK) >> GENERATION_SHIFT;
        let new_generation = generation.wrapping_add(1) & 0xFFFF_FFFF_FFFF; // 48-bit wraparound
        (state & !GENERATION_MASK) | (new_generation << GENERATION_SHIFT)
    }
}

impl Default for InteractiveStateCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_defaults() {
        let state = InteractiveStateCapsule::new();

        assert_eq!(state.is_paused(), false);
        assert_eq!(state.is_cancelled(), false);
        assert_eq!(state.is_gpu_enabled(), true);
        assert_eq!(state.crf_adjustment(), 0);
        assert_eq!(state.generation(), 0);
    }

    #[test]
    fn test_toggle_pause() {
        let state = InteractiveStateCapsule::new();

        // Initially not paused
        assert_eq!(state.is_paused(), false);

        // Toggle to paused
        state.toggle_pause();
        assert_eq!(state.is_paused(), true);
        assert_eq!(state.generation(), 1);

        // Toggle back to unpaused
        state.toggle_pause();
        assert_eq!(state.is_paused(), false);
        assert_eq!(state.generation(), 2);
    }

    #[test]
    fn test_toggle_gpu() {
        let state = InteractiveStateCapsule::new();

        // Initially GPU enabled
        assert_eq!(state.is_gpu_enabled(), true);

        // Toggle to disabled
        state.toggle_gpu();
        assert_eq!(state.is_gpu_enabled(), false);
        assert_eq!(state.generation(), 1);

        // Toggle back to enabled
        state.toggle_gpu();
        assert_eq!(state.is_gpu_enabled(), true);
        assert_eq!(state.generation(), 2);
    }

    #[test]
    fn test_request_cancel() {
        let state = InteractiveStateCapsule::new();

        // Initially not cancelled
        assert_eq!(state.is_cancelled(), false);

        // Request cancellation
        state.request_cancel();
        assert_eq!(state.is_cancelled(), true);
        assert_eq!(state.generation(), 1);

        // Second cancel request is idempotent (still increments generation)
        state.request_cancel();
        assert_eq!(state.is_cancelled(), true);
        assert_eq!(state.generation(), 2);
    }

    #[test]
    fn test_crf_adjustment_bounds() {
        let state = InteractiveStateCapsule::new();

        // Adjust upward
        state.adjust_crf(5);
        assert_eq!(state.crf_adjustment(), 5);
        assert_eq!(state.generation(), 1);

        // Adjust beyond max (should clamp)
        state.adjust_crf(10);
        assert_eq!(state.crf_adjustment(), 10); // Clamped to CRF_MAX
        assert_eq!(state.generation(), 2);

        // Adjust downward
        state.adjust_crf(-15);
        assert_eq!(state.crf_adjustment(), -5); // 10 - 15 = -5
        assert_eq!(state.generation(), 3);

        // Adjust below min (should clamp)
        state.adjust_crf(-20);
        assert_eq!(state.crf_adjustment(), -10); // Clamped to CRF_MIN
        assert_eq!(state.generation(), 4);

        // Adjust back to zero
        state.adjust_crf(10);
        assert_eq!(state.crf_adjustment(), 0); // -10 + 10 = 0
        assert_eq!(state.generation(), 5);
    }

    #[test]
    fn test_snapshot_atomicity() {
        let state = InteractiveStateCapsule::new();

        // Modify state
        state.toggle_pause();
        state.adjust_crf(7);
        state.toggle_gpu();

        // Get snapshot
        let snapshot = state.snapshot();

        assert_eq!(snapshot.paused, true);
        assert_eq!(snapshot.cancelled, false);
        assert_eq!(snapshot.gpu_enabled, false);
        assert_eq!(snapshot.crf_adjustment, 7);
        assert_eq!(snapshot.generation, 3); // 3 mutations
    }

    #[test]
    fn test_generation_increments() {
        let state = InteractiveStateCapsule::new();

        assert_eq!(state.generation(), 0);

        state.toggle_pause();
        assert_eq!(state.generation(), 1);

        state.toggle_gpu();
        assert_eq!(state.generation(), 2);

        state.adjust_crf(3);
        assert_eq!(state.generation(), 3);

        state.request_cancel();
        assert_eq!(state.generation(), 4);
    }

    #[test]
    fn test_size_and_alignment() {
        use std::mem::{align_of, size_of};

        assert_eq!(size_of::<InteractiveStateCapsule>(), 64);
        assert_eq!(align_of::<InteractiveStateCapsule>(), 64);
    }

    #[test]
    fn test_multiple_adjustments() {
        let state = InteractiveStateCapsule::new();

        // Series of adjustments
        state.adjust_crf(3);
        state.adjust_crf(2);
        state.adjust_crf(-1);
        assert_eq!(state.crf_adjustment(), 4);
        assert_eq!(state.generation(), 3);
    }

    #[test]
    fn test_mixed_operations() {
        let state = InteractiveStateCapsule::new();

        // Complex sequence
        state.toggle_pause(); // gen=1
        state.adjust_crf(5); // gen=2
        state.toggle_gpu(); // gen=3
        state.toggle_pause(); // gen=4 (unpause)
        state.request_cancel(); // gen=5

        let snapshot = state.snapshot();
        assert_eq!(snapshot.paused, false);
        assert_eq!(snapshot.cancelled, true);
        assert_eq!(snapshot.gpu_enabled, false);
        assert_eq!(snapshot.crf_adjustment, 5);
        assert_eq!(snapshot.generation, 5);
    }

    #[test]
    fn test_menu_open_close() {
        let state = InteractiveStateCapsule::new();

        // Initially menu closed
        assert_eq!(state.is_menu_open(), false);

        // Open menu
        state.open_menu();
        assert_eq!(state.is_menu_open(), true);
        assert_eq!(state.generation(), 1);

        // Close menu
        state.close_menu();
        assert_eq!(state.is_menu_open(), false);
        assert_eq!(state.generation(), 2);
    }

    #[test]
    fn test_wizard_lifecycle() {
        let state = InteractiveStateCapsule::new();

        // Initially wizard inactive
        assert_eq!(state.is_wizard_active(), false);
        assert_eq!(state.wizard_step(), 0);

        // Start wizard
        state.start_wizard();
        assert_eq!(state.is_wizard_active(), true);
        assert_eq!(state.wizard_step(), 0);
        assert_eq!(state.generation(), 1);

        // Finish wizard
        state.finish_wizard();
        assert_eq!(state.is_wizard_active(), false);
        assert_eq!(state.generation(), 2);
    }

    #[test]
    fn test_wizard_next_prev() {
        let state = InteractiveStateCapsule::new();
        state.start_wizard();

        // Step 0 → 1
        state.wizard_next();
        assert_eq!(state.wizard_step(), 1);
        assert_eq!(state.generation(), 2);

        // Step 1 → 2
        state.wizard_next();
        assert_eq!(state.wizard_step(), 2);
        assert_eq!(state.generation(), 3);

        // Step 2 → 1 (previous)
        state.wizard_prev();
        assert_eq!(state.wizard_step(), 1);
        assert_eq!(state.generation(), 4);

        // Step 1 → 0 (previous)
        state.wizard_prev();
        assert_eq!(state.wizard_step(), 0);
        assert_eq!(state.generation(), 5);
    }

    #[test]
    fn test_wizard_step_wraparound() {
        let state = InteractiveStateCapsule::new();
        state.start_wizard();

        // Advance to step 7 (max)
        for _ in 0..7 {
            state.wizard_next();
        }
        assert_eq!(state.wizard_step(), 7);

        // Next wraps to 0
        state.wizard_next();
        assert_eq!(state.wizard_step(), 0);

        // Previous wraps to 7
        state.wizard_prev();
        assert_eq!(state.wizard_step(), 7);
    }

    #[test]
    fn test_wizard_step_all_values() {
        let state = InteractiveStateCapsule::new();
        state.start_wizard();

        // Test all wizard steps 0-7
        for expected_step in 0..8 {
            assert_eq!(state.wizard_step(), expected_step);
            state.wizard_next();
        }

        // Should wrap back to 0
        assert_eq!(state.wizard_step(), 0);
    }

    #[test]
    fn test_snapshot_menu_wizard() {
        let state = InteractiveStateCapsule::new();

        // Open menu and start wizard at step 3
        state.open_menu();
        state.start_wizard();
        state.wizard_next();
        state.wizard_next();
        state.wizard_next();

        let snapshot = state.snapshot();
        assert_eq!(snapshot.menu_open, true);
        assert_eq!(snapshot.wizard_active, true);
        assert_eq!(snapshot.wizard_step, 3);
        assert_eq!(snapshot.generation, 5); // open_menu + start_wizard + 3 next
    }

    #[test]
    fn test_menu_wizard_isolation() {
        let state = InteractiveStateCapsule::new();

        // Menu and wizard should not interfere with other flags
        state.toggle_pause();
        state.adjust_crf(5);
        state.open_menu();
        state.start_wizard();

        let snapshot = state.snapshot();
        assert_eq!(snapshot.paused, true);
        assert_eq!(snapshot.crf_adjustment, 5);
        assert_eq!(snapshot.menu_open, true);
        assert_eq!(snapshot.wizard_active, true);
        assert_eq!(snapshot.wizard_step, 0);
    }

    #[test]
    fn test_generation_increments_menu_wizard() {
        let state = InteractiveStateCapsule::new();

        assert_eq!(state.generation(), 0);

        state.open_menu();
        assert_eq!(state.generation(), 1);

        state.close_menu();
        assert_eq!(state.generation(), 2);

        state.start_wizard();
        assert_eq!(state.generation(), 3);

        state.wizard_next();
        assert_eq!(state.generation(), 4);

        state.wizard_prev();
        assert_eq!(state.generation(), 5);

        state.finish_wizard();
        assert_eq!(state.generation(), 6);
    }

    #[test]
    fn test_menu_idempotent() {
        let state = InteractiveStateCapsule::new();

        // Opening menu twice still increments generation each time
        state.open_menu();
        assert_eq!(state.is_menu_open(), true);
        assert_eq!(state.generation(), 1);

        state.open_menu();
        assert_eq!(state.is_menu_open(), true);
        assert_eq!(state.generation(), 2);
    }
}
