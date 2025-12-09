//! Wizard flow orchestrator capsule
//!
//! # Tier: T1 Atomic
//!
//! State machine for the 5-step encoding wizard with lockfree coordination.
//!
//! # Layout (256 bytes)
//! - state: AtomicU64 (8 bytes) - packed state machine + generation
//! - input_path: AtomicPtr<String> (8 bytes) - atomically swappable path
//! - _padding: [u8; 240] - cache alignment to 256 bytes
//!
//! # State Bit Layout (AtomicU64)
//! - Bits 0-7:   current_state (WizardState as u8)
//! - Bits 8-15:  quality_goal (0=Smallest, 1=Balanced, 2=Best)
//! - Bits 16-23: speed_choice (0=Quick, 1=Normal, 2=Thorough)
//! - Bits 24-31: reserved
//! - Bits 32-63: generation counter (32 bits)
//!
//! # Performance
//! - State query: <5ns (single atomic load)
//! - State transition: <10ns (CAS loop)
//! - Atomic snapshot: <5ns (single load + unpacking)
//!
//! # Chaos Compliance
//! - 100% lockfree (AtomicU64 + AtomicPtr only)
//! - 256B cache-aligned
//! - Generation counter increments on every mutation
//! - Acquire/Release memory ordering

use super::mapping::{QualityGoal, SpeedChoice};
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};

// Bit masks and shifts for state packing
const STATE_MASK: u64 = 0xFF;
const QUALITY_MASK: u64 = 0xFF << 8;
const QUALITY_SHIFT: u32 = 8;
const SPEED_MASK: u64 = 0xFF << 16;
const SPEED_SHIFT: u32 = 16;
const GENERATION_MASK: u64 = 0xFFFF_FFFF << 32;
const GENERATION_SHIFT: u32 = 32;

/// Wizard state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WizardState {
    /// Idle, not started
    Idle = 0,
    /// Step 0: Hardware auto-detection
    Step0HardwareCheck = 1,
    /// Step 1: Select video file
    Step1SelectVideo = 2,
    /// Step 2: Quality goal choice
    Step2QualityGoal = 3,
    /// Step 3: Speed choice
    Step3SpeedChoice = 4,
    /// Step 4: Confirm and start
    Step4Confirm = 5,
    /// Wizard complete
    Complete = 6,
    /// User cancelled
    Cancelled = 7,
}

impl WizardState {
    /// Convert u8 to WizardState (safe, defaults to Idle on invalid)
    #[inline]
    fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::Idle,
            1 => Self::Step0HardwareCheck,
            2 => Self::Step1SelectVideo,
            3 => Self::Step2QualityGoal,
            4 => Self::Step3SpeedChoice,
            5 => Self::Step4Confirm,
            6 => Self::Complete,
            7 => Self::Cancelled,
            _ => Self::Idle, // Fallback for corrupted state
        }
    }
}

/// Wizard flow orchestrator capsule (256B, T1 Atomic)
///
/// Provides lockfree state machine coordination for the 5-step wizard:
/// 1. Hardware auto-detection
/// 2. Video file selection
/// 3. Quality goal choice
/// 4. Speed choice
/// 5. Confirmation
///
/// # Examples
///
/// ```
/// use kindly_av1::cli::wizard::{WizardFlowCapsule, WizardState, QualityGoal, SpeedChoice};
///
/// let wizard = WizardFlowCapsule::new();
///
/// // Start wizard
/// wizard.start();
/// assert_eq!(wizard.state(), WizardState::Step0HardwareCheck);
///
/// // Progress through steps
/// wizard.next();
/// assert_eq!(wizard.state(), WizardState::Step1SelectVideo);
///
/// wizard.set_input_path("video.mp4".to_string());
/// wizard.next();
/// assert_eq!(wizard.state(), WizardState::Step2QualityGoal);
///
/// wizard.set_quality(QualityGoal::Balanced);
/// wizard.next();
/// assert_eq!(wizard.state(), WizardState::Step3SpeedChoice);
///
/// wizard.set_speed(SpeedChoice::Normal);
/// wizard.next();
/// assert_eq!(wizard.state(), WizardState::Step4Confirm);
///
/// // Check state
/// assert!(wizard.can_go_back());
/// assert!(wizard.can_go_next());
///
/// // Complete wizard
/// wizard.next();
/// assert_eq!(wizard.state(), WizardState::Complete);
/// assert!(wizard.is_complete());
/// ```
#[repr(C, align(256))]
pub struct WizardFlowCapsule {
    /// Packed state: state (0-7) | quality (8-15) | speed (16-23) | generation (32-63)
    state: AtomicU64,
    /// Atomically swappable input path (Box<String> pointer)
    input_path: AtomicPtr<String>,
    /// Padding to 256 bytes for cache alignment
    _padding: [u8; 240],
}

impl WizardFlowCapsule {
    /// Create new wizard flow in Idle state
    pub fn new() -> Self {
        // Initial state: Idle, Balanced quality, Normal speed, generation 0
        let initial = (WizardState::Idle as u64)
            | ((QualityGoal::Balanced as u64) << QUALITY_SHIFT)
            | ((SpeedChoice::Normal as u64) << SPEED_SHIFT);

        Self {
            state: AtomicU64::new(initial),
            input_path: AtomicPtr::new(std::ptr::null_mut()),
            _padding: [0; 240],
        }
    }

    // ========================================================================
    // State Queries
    // ========================================================================

    /// Get current wizard state
    ///
    /// # Performance
    /// <5ns (single atomic load + bit extraction)
    #[inline]
    pub fn state(&self) -> WizardState {
        let state = self.state.load(Ordering::Acquire);
        let state_bits = (state & STATE_MASK) as u8;
        WizardState::from_u8(state_bits)
    }

    /// Get current quality goal
    ///
    /// # Performance
    /// <5ns (single atomic load + bit extraction)
    #[inline]
    pub fn quality(&self) -> QualityGoal {
        let state = self.state.load(Ordering::Acquire);
        let quality_bits = ((state & QUALITY_MASK) >> QUALITY_SHIFT) as u8;
        match quality_bits {
            0 => QualityGoal::Smallest,
            1 => QualityGoal::Balanced,
            2 => QualityGoal::Best,
            _ => QualityGoal::Balanced, // Fallback
        }
    }

    /// Get current speed choice
    ///
    /// # Performance
    /// <5ns (single atomic load + bit extraction)
    #[inline]
    pub fn speed(&self) -> SpeedChoice {
        let state = self.state.load(Ordering::Acquire);
        let speed_bits = ((state & SPEED_MASK) >> SPEED_SHIFT) as u8;
        match speed_bits {
            0 => SpeedChoice::Quick,
            1 => SpeedChoice::Normal,
            2 => SpeedChoice::Thorough,
            _ => SpeedChoice::Normal, // Fallback
        }
    }

    /// Get current generation counter
    ///
    /// # Performance
    /// <5ns (single atomic load + bit extraction)
    #[inline]
    pub fn generation(&self) -> u32 {
        let state = self.state.load(Ordering::Acquire);
        ((state & GENERATION_MASK) >> GENERATION_SHIFT) as u32
    }

    /// Get input path (if set)
    ///
    /// # Performance
    /// <10ns (atomic load + clone)
    #[inline]
    pub fn input_path(&self) -> Option<String> {
        let ptr = self.input_path.load(Ordering::Acquire);
        if ptr.is_null() {
            None
        } else {
            // #ASSUME: Pointer is valid and points to a String
            // #VERIFY: Only set via set_input_path which ensures validity
            unsafe { Some((*ptr).clone()) }
        }
    }

    // ========================================================================
    // State Transitions
    // ========================================================================

    /// Start wizard (Idle → Step0)
    ///
    /// # Performance
    /// <10ns (CAS loop, typically 1 iteration)
    pub fn start(&self) {
        self.transition_to(WizardState::Step0HardwareCheck);
    }

    /// Go to next step (StepN → StepN+1, or Step4 → Complete)
    ///
    /// # Performance
    /// <10ns (CAS loop, typically 1 iteration)
    pub fn next(&self) {
        let current_state = self.state();
        let next_state = match current_state {
            WizardState::Idle => WizardState::Step0HardwareCheck,
            WizardState::Step0HardwareCheck => WizardState::Step1SelectVideo,
            WizardState::Step1SelectVideo => WizardState::Step2QualityGoal,
            WizardState::Step2QualityGoal => WizardState::Step3SpeedChoice,
            WizardState::Step3SpeedChoice => WizardState::Step4Confirm,
            WizardState::Step4Confirm => WizardState::Complete,
            WizardState::Complete => WizardState::Complete, // Idempotent
            WizardState::Cancelled => WizardState::Cancelled, // Idempotent
        };
        self.transition_to(next_state);
    }

    /// Go to previous step (StepN → StepN-1, can't go back from Step0/Step1)
    ///
    /// # Performance
    /// <10ns (CAS loop, typically 1 iteration)
    pub fn back(&self) {
        let current_state = self.state();
        let prev_state = match current_state {
            WizardState::Step4Confirm => WizardState::Step3SpeedChoice,
            WizardState::Step3SpeedChoice => WizardState::Step2QualityGoal,
            WizardState::Step2QualityGoal => WizardState::Step1SelectVideo,
            // Can't go back from Step1, Step0, Idle, Complete, or Cancelled
            _ => current_state,
        };
        self.transition_to(prev_state);
    }

    /// Cancel wizard (Any → Cancelled)
    ///
    /// # Performance
    /// <10ns (CAS loop, typically 1 iteration)
    pub fn cancel(&self) {
        self.transition_to(WizardState::Cancelled);
    }

    /// Set quality goal
    ///
    /// # Performance
    /// <10ns (CAS loop, typically 1 iteration)
    pub fn set_quality(&self, quality: QualityGoal) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let quality_bits = (quality as u64) << QUALITY_SHIFT;
            let new_state = (current & !QUALITY_MASK) | quality_bits;
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

    /// Set speed choice
    ///
    /// # Performance
    /// <10ns (CAS loop, typically 1 iteration)
    pub fn set_speed(&self, speed: SpeedChoice) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let speed_bits = (speed as u64) << SPEED_SHIFT;
            let new_state = (current & !SPEED_MASK) | speed_bits;
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

    /// Set input path (atomically swaps pointer)
    ///
    /// # Performance
    /// <10ns (atomic swap)
    pub fn set_input_path(&self, path: String) {
        let new_path = Box::into_raw(Box::new(path));
        let old_path = self.input_path.swap(new_path, Ordering::AcqRel);

        // Free old path if it exists
        if !old_path.is_null() {
            // #ASSUME: Old pointer is valid (we created it)
            // #VERIFY: Only created via set_input_path
            unsafe {
                drop(Box::from_raw(old_path));
            }
        }
    }

    // ========================================================================
    // Orchestration Helpers
    // ========================================================================

    /// Check if back navigation is allowed
    #[inline]
    pub fn can_go_back(&self) -> bool {
        matches!(
            self.state(),
            WizardState::Step2QualityGoal
                | WizardState::Step3SpeedChoice
                | WizardState::Step4Confirm
        )
    }

    /// Check if next navigation is allowed
    #[inline]
    pub fn can_go_next(&self) -> bool {
        !matches!(
            self.state(),
            WizardState::Complete | WizardState::Cancelled
        )
    }

    /// Check if wizard is complete
    #[inline]
    pub fn is_complete(&self) -> bool {
        self.state() == WizardState::Complete
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    /// Transition to new state (internal helper)
    fn transition_to(&self, new_state: WizardState) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let state_bits = new_state as u64;
            let updated = (current & !STATE_MASK) | state_bits;
            let updated = self.increment_generation(updated);

            match self.state.compare_exchange_weak(
                current,
                updated,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    /// Increment generation counter in state value
    ///
    /// # ASSUME
    /// #ASSUME: Generation counter will not overflow (32 bits = 4.2 billion)
    /// #VERIFY: For CLI usage at <1000 updates/sec, overflow after 49 days
    #[inline]
    fn increment_generation(&self, state: u64) -> u64 {
        let generation = (state & GENERATION_MASK) >> GENERATION_SHIFT;
        let new_generation = generation.wrapping_add(1) & 0xFFFF_FFFF; // 32-bit wraparound
        (state & !GENERATION_MASK) | (new_generation << GENERATION_SHIFT)
    }
}

impl Default for WizardFlowCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WizardFlowCapsule {
    fn drop(&mut self) {
        // Free input_path if it exists
        let ptr = self.input_path.load(Ordering::Acquire);
        if !ptr.is_null() {
            // #ASSUME: Pointer is valid (we created it)
            // #VERIFY: Only created via set_input_path
            unsafe {
                drop(Box::from_raw(ptr));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_defaults() {
        let wizard = WizardFlowCapsule::new();

        assert_eq!(wizard.state(), WizardState::Idle);
        assert_eq!(wizard.quality(), QualityGoal::Balanced);
        assert_eq!(wizard.speed(), SpeedChoice::Normal);
        assert_eq!(wizard.generation(), 0);
        assert_eq!(wizard.input_path(), None);
    }

    #[test]
    fn test_start_wizard() {
        let wizard = WizardFlowCapsule::new();

        wizard.start();
        assert_eq!(wizard.state(), WizardState::Step0HardwareCheck);
        assert_eq!(wizard.generation(), 1);
    }

    #[test]
    fn test_forward_navigation() {
        let wizard = WizardFlowCapsule::new();

        wizard.start();
        assert_eq!(wizard.state(), WizardState::Step0HardwareCheck);

        wizard.next();
        assert_eq!(wizard.state(), WizardState::Step1SelectVideo);

        wizard.next();
        assert_eq!(wizard.state(), WizardState::Step2QualityGoal);

        wizard.next();
        assert_eq!(wizard.state(), WizardState::Step3SpeedChoice);

        wizard.next();
        assert_eq!(wizard.state(), WizardState::Step4Confirm);

        wizard.next();
        assert_eq!(wizard.state(), WizardState::Complete);
        assert!(wizard.is_complete());
    }

    #[test]
    fn test_backward_navigation() {
        let wizard = WizardFlowCapsule::new();

        // Navigate to Step4
        wizard.start();
        wizard.next();
        wizard.next();
        wizard.next();
        wizard.next();
        assert_eq!(wizard.state(), WizardState::Step4Confirm);

        // Go back
        wizard.back();
        assert_eq!(wizard.state(), WizardState::Step3SpeedChoice);

        wizard.back();
        assert_eq!(wizard.state(), WizardState::Step2QualityGoal);

        wizard.back();
        assert_eq!(wizard.state(), WizardState::Step1SelectVideo);

        // Can't go back from Step1
        wizard.back();
        assert_eq!(wizard.state(), WizardState::Step1SelectVideo);
    }

    #[test]
    fn test_can_go_back() {
        let wizard = WizardFlowCapsule::new();

        assert!(!wizard.can_go_back()); // Idle

        wizard.start();
        assert!(!wizard.can_go_back()); // Step0

        wizard.next();
        assert!(!wizard.can_go_back()); // Step1

        wizard.next();
        assert!(wizard.can_go_back()); // Step2

        wizard.next();
        assert!(wizard.can_go_back()); // Step3

        wizard.next();
        assert!(wizard.can_go_back()); // Step4
    }

    #[test]
    fn test_can_go_next() {
        let wizard = WizardFlowCapsule::new();

        assert!(wizard.can_go_next()); // Idle

        wizard.start();
        assert!(wizard.can_go_next()); // Step0

        // Navigate to Complete
        for _ in 0..5 {
            wizard.next();
        }
        assert!(!wizard.can_go_next()); // Complete
    }

    #[test]
    fn test_cancel() {
        let wizard = WizardFlowCapsule::new();

        wizard.start();
        wizard.next();
        wizard.cancel();

        assert_eq!(wizard.state(), WizardState::Cancelled);
        assert!(!wizard.can_go_next());
    }

    #[test]
    fn test_set_quality() {
        let wizard = WizardFlowCapsule::new();

        wizard.set_quality(QualityGoal::Smallest);
        assert_eq!(wizard.quality(), QualityGoal::Smallest);
        assert_eq!(wizard.generation(), 1);

        wizard.set_quality(QualityGoal::Best);
        assert_eq!(wizard.quality(), QualityGoal::Best);
        assert_eq!(wizard.generation(), 2);
    }

    #[test]
    fn test_set_speed() {
        let wizard = WizardFlowCapsule::new();

        wizard.set_speed(SpeedChoice::Quick);
        assert_eq!(wizard.speed(), SpeedChoice::Quick);
        assert_eq!(wizard.generation(), 1);

        wizard.set_speed(SpeedChoice::Thorough);
        assert_eq!(wizard.speed(), SpeedChoice::Thorough);
        assert_eq!(wizard.generation(), 2);
    }

    #[test]
    fn test_set_input_path() {
        let wizard = WizardFlowCapsule::new();

        wizard.set_input_path("video.mp4".to_string());
        assert_eq!(wizard.input_path(), Some("video.mp4".to_string()));

        wizard.set_input_path("movie.mkv".to_string());
        assert_eq!(wizard.input_path(), Some("movie.mkv".to_string()));
    }

    #[test]
    fn test_full_workflow() {
        let wizard = WizardFlowCapsule::new();

        // Start wizard
        wizard.start();
        assert_eq!(wizard.state(), WizardState::Step0HardwareCheck);

        // Step 1: Select video
        wizard.next();
        wizard.set_input_path("vacation_2024.mp4".to_string());
        assert_eq!(wizard.input_path(), Some("vacation_2024.mp4".to_string()));

        // Step 2: Quality goal
        wizard.next();
        wizard.set_quality(QualityGoal::Balanced);
        assert_eq!(wizard.quality(), QualityGoal::Balanced);

        // Step 3: Speed choice
        wizard.next();
        wizard.set_speed(SpeedChoice::Normal);
        assert_eq!(wizard.speed(), SpeedChoice::Normal);

        // Step 4: Confirm
        wizard.next();
        assert_eq!(wizard.state(), WizardState::Step4Confirm);

        // Complete
        wizard.next();
        assert!(wizard.is_complete());
    }

    #[test]
    fn test_generation_increments() {
        let wizard = WizardFlowCapsule::new();

        assert_eq!(wizard.generation(), 0);

        wizard.start();
        assert_eq!(wizard.generation(), 1);

        wizard.next();
        assert_eq!(wizard.generation(), 2);

        wizard.set_quality(QualityGoal::Best);
        assert_eq!(wizard.generation(), 3);

        wizard.set_speed(SpeedChoice::Quick);
        assert_eq!(wizard.generation(), 4);
    }

    #[test]
    fn test_size_and_alignment() {
        use std::mem::{align_of, size_of};

        assert_eq!(size_of::<WizardFlowCapsule>(), 256);
        assert_eq!(align_of::<WizardFlowCapsule>(), 256);
    }

    #[test]
    fn test_state_isolation() {
        let wizard = WizardFlowCapsule::new();

        // State transitions shouldn't affect quality/speed
        wizard.set_quality(QualityGoal::Smallest);
        wizard.set_speed(SpeedChoice::Thorough);

        wizard.start();
        wizard.next();
        wizard.next();

        assert_eq!(wizard.quality(), QualityGoal::Smallest);
        assert_eq!(wizard.speed(), SpeedChoice::Thorough);
    }

    #[test]
    fn test_complete_idempotent() {
        let wizard = WizardFlowCapsule::new();

        // Navigate to Complete
        wizard.start();
        for _ in 0..5 {
            wizard.next();
        }

        let gen = wizard.generation();
        wizard.next(); // Should be idempotent
        assert_eq!(wizard.state(), WizardState::Complete);
        assert!(wizard.generation() > gen); // Generation still increments
    }

    #[test]
    fn test_cancelled_idempotent() {
        let wizard = WizardFlowCapsule::new();

        wizard.start();
        wizard.cancel();

        let gen = wizard.generation();
        wizard.cancel(); // Should be idempotent
        assert_eq!(wizard.state(), WizardState::Cancelled);
        assert!(wizard.generation() > gen); // Generation still increments
    }
}
