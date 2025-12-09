//! State Machine for kindly_dedup GUI
//!
//! Defines the finite state machine (FSM) for application state transitions.
//! Uses AtomicU64 for lockfree state management (T1 Atomic tier).
//!
//! # State Diagram
//!
//! ```text
//! ┌─────────┐  FileSelected  ┌──────────┐  StartClicked  ┌────────────┐
//! │  Idle   │───────────────►│  Ready   │───────────────►│ Processing │
//! └─────────┘                └──────────┘                └────────────┘
//!      ▲                          │                            │
//!      │         Reset            │                            │ Complete
//!      └──────────────────────────┴────────────────────────────┘
//!                                                              │
//!                                                              ▼
//!                                                        ┌──────────┐
//!                                                        │ Complete │
//!                                                        └──────────┘
//! ```

use core::sync::atomic::{AtomicU64, Ordering};

/// Application state machine phases
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AppState {
    /// Initial state - no file selected
    Idle = 0,
    /// File selected, ready to start
    Ready = 1,
    /// Processing in progress
    Processing = 2,
    /// Processing complete, showing results
    Complete = 3,
    /// Error occurred
    Error = 4,
}

impl AppState {
    /// Convert from raw u8
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Idle),
            1 => Some(Self::Ready),
            2 => Some(Self::Processing),
            3 => Some(Self::Complete),
            4 => Some(Self::Error),
            _ => None,
        }
    }

    /// Check if transitions to target state is valid
    pub fn can_transition_to(self, target: Self) -> bool {
        match (self, target) {
            // From Idle: can go to Ready (file selected) or Error
            (Self::Idle, Self::Ready) => true,
            (Self::Idle, Self::Error) => true,

            // From Ready: can start processing, reset to idle, or error
            (Self::Ready, Self::Processing) => true,
            (Self::Ready, Self::Idle) => true,
            (Self::Ready, Self::Error) => true,

            // From Processing: can complete, error, or cancel (back to ready)
            (Self::Processing, Self::Complete) => true,
            (Self::Processing, Self::Error) => true,
            (Self::Processing, Self::Ready) => true, // Cancel

            // From Complete: can reset to idle or ready (new file)
            (Self::Complete, Self::Idle) => true,
            (Self::Complete, Self::Ready) => true,

            // From Error: can reset to idle
            (Self::Error, Self::Idle) => true,

            // All other transitions invalid
            _ => false,
        }
    }
}

/// Processing phase within Processing state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProcessingPhase {
    /// Initializing pipeline
    Initializing = 0,
    /// Adding documents to pipeline
    Adding = 1,
    /// Finding duplicates
    Finding = 2,
    /// Writing results
    Writing = 3,
    /// Finalizing
    Finalizing = 4,
}

impl ProcessingPhase {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Initializing),
            1 => Some(Self::Adding),
            2 => Some(Self::Finding),
            3 => Some(Self::Writing),
            4 => Some(Self::Finalizing),
            _ => None,
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Initializing => "Initializing pipeline...",
            Self::Adding => "Adding documents...",
            Self::Finding => "Finding duplicates...",
            Self::Writing => "Writing results...",
            Self::Finalizing => "Finalizing...",
        }
    }
}

/// Atomic state capsule for lockfree state management
///
/// # Memory Layout (64 bits)
///
/// ```text
/// Bits 0-7:   AppState (u8)
/// Bits 8-15:  ProcessingPhase (u8)
/// Bits 16-31: Generation counter (u16)
/// Bits 32-63: Reserved for future use
/// ```
#[repr(C, align(64))]
pub struct AppStateCapsule {
    /// Packed state: [state:8][phase:8][generation:16][reserved:32]
    state: AtomicU64,
    /// Padding to fill cache line
    _pad: [u8; 56],
}

impl AppStateCapsule {
    // Bit positions
    const STATE_SHIFT: u32 = 0;
    const STATE_MASK: u64 = 0xFF;
    const PHASE_SHIFT: u32 = 8;
    const PHASE_MASK: u64 = 0xFF00;
    const GENERATION_SHIFT: u32 = 16;
    const GENERATION_MASK: u64 = 0xFFFF_0000;

    /// Create new state capsule in Idle state
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            _pad: [0; 56],
        }
    }

    /// Get current app state
    pub fn state(&self) -> AppState {
        let raw = self.state.load(Ordering::Acquire);
        let state_byte = (raw & Self::STATE_MASK) as u8;
        AppState::from_u8(state_byte).unwrap_or(AppState::Idle)
    }

    /// Get current processing phase (only valid in Processing state)
    pub fn phase(&self) -> ProcessingPhase {
        let raw = self.state.load(Ordering::Acquire);
        let phase_byte = ((raw & Self::PHASE_MASK) >> Self::PHASE_SHIFT) as u8;
        ProcessingPhase::from_u8(phase_byte).unwrap_or(ProcessingPhase::Initializing)
    }

    /// Get generation counter (for TOCTOU safety)
    pub fn generation(&self) -> u16 {
        let raw = self.state.load(Ordering::Acquire);
        ((raw & Self::GENERATION_MASK) >> Self::GENERATION_SHIFT) as u16
    }

    /// Attempt to transition to new state
    ///
    /// Returns true if transition succeeded, false if invalid or raced.
    pub fn transition(&self, target: AppState) -> bool {
        loop {
            let current = self.state.load(Ordering::Acquire);
            let current_state = AppState::from_u8((current & Self::STATE_MASK) as u8)
                .unwrap_or(AppState::Idle);

            if !current_state.can_transition_to(target) {
                return false;
            }

            // Increment generation, update state
            let current_gen = ((current & Self::GENERATION_MASK) >> Self::GENERATION_SHIFT) as u16;
            let new_gen = current_gen.wrapping_add(1);

            let new_value = (target as u64)
                | ((ProcessingPhase::Initializing as u64) << Self::PHASE_SHIFT)
                | ((new_gen as u64) << Self::GENERATION_SHIFT);

            match self.state.compare_exchange_weak(
                current,
                new_value,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(_) => continue, // Retry on spurious failure
            }
        }
    }

    /// Update processing phase (only valid in Processing state)
    pub fn set_phase(&self, phase: ProcessingPhase) -> bool {
        loop {
            let current = self.state.load(Ordering::Acquire);
            let current_state = AppState::from_u8((current & Self::STATE_MASK) as u8)
                .unwrap_or(AppState::Idle);

            if current_state != AppState::Processing {
                return false;
            }

            let new_value = (current & !Self::PHASE_MASK)
                | ((phase as u64) << Self::PHASE_SHIFT);

            match self.state.compare_exchange_weak(
                current,
                new_value,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(_) => continue,
            }
        }
    }

    /// Reset to Idle state
    pub fn reset(&self) {
        let current = self.state.load(Ordering::Acquire);
        let current_gen = ((current & Self::GENERATION_MASK) >> Self::GENERATION_SHIFT) as u16;
        let new_gen = current_gen.wrapping_add(1);

        let new_value = (AppState::Idle as u64)
            | ((new_gen as u64) << Self::GENERATION_SHIFT);

        self.state.store(new_value, Ordering::Release);
    }
}

impl Default for AppStateCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_transitions() {
        assert!(AppState::Idle.can_transition_to(AppState::Ready));
        assert!(AppState::Ready.can_transition_to(AppState::Processing));
        assert!(AppState::Processing.can_transition_to(AppState::Complete));
        assert!(AppState::Complete.can_transition_to(AppState::Idle));

        // Invalid transitions
        assert!(!AppState::Idle.can_transition_to(AppState::Processing));
        assert!(!AppState::Idle.can_transition_to(AppState::Complete));
    }

    #[test]
    fn test_state_capsule_creation() {
        let capsule = AppStateCapsule::new();
        assert_eq!(capsule.state(), AppState::Idle);
        assert_eq!(capsule.generation(), 0);
    }

    #[test]
    fn test_state_transitions() {
        let capsule = AppStateCapsule::new();

        // Idle -> Ready
        assert!(capsule.transition(AppState::Ready));
        assert_eq!(capsule.state(), AppState::Ready);
        assert_eq!(capsule.generation(), 1);

        // Ready -> Processing
        assert!(capsule.transition(AppState::Processing));
        assert_eq!(capsule.state(), AppState::Processing);
        assert_eq!(capsule.generation(), 2);

        // Processing -> Complete
        assert!(capsule.transition(AppState::Complete));
        assert_eq!(capsule.state(), AppState::Complete);

        // Complete -> Idle (reset)
        assert!(capsule.transition(AppState::Idle));
        assert_eq!(capsule.state(), AppState::Idle);
    }

    #[test]
    fn test_invalid_transition() {
        let capsule = AppStateCapsule::new();

        // Can't go directly from Idle to Processing
        assert!(!capsule.transition(AppState::Processing));
        assert_eq!(capsule.state(), AppState::Idle);
    }

    #[test]
    fn test_processing_phases() {
        let capsule = AppStateCapsule::new();

        // Move to processing
        capsule.transition(AppState::Ready);
        capsule.transition(AppState::Processing);

        // Update phases
        assert!(capsule.set_phase(ProcessingPhase::Adding));
        assert_eq!(capsule.phase(), ProcessingPhase::Adding);

        assert!(capsule.set_phase(ProcessingPhase::Finding));
        assert_eq!(capsule.phase(), ProcessingPhase::Finding);
    }

    #[test]
    fn test_phase_only_in_processing() {
        let capsule = AppStateCapsule::new();

        // Can't set phase in Idle
        assert!(!capsule.set_phase(ProcessingPhase::Adding));
    }

    #[test]
    fn test_reset() {
        let capsule = AppStateCapsule::new();
        capsule.transition(AppState::Ready);
        capsule.transition(AppState::Processing);

        let gen_before = capsule.generation();
        capsule.reset();

        assert_eq!(capsule.state(), AppState::Idle);
        assert_eq!(capsule.generation(), gen_before + 1);
    }

    #[test]
    fn test_capsule_size() {
        assert_eq!(core::mem::size_of::<AppStateCapsule>(), 64);
        assert_eq!(core::mem::align_of::<AppStateCapsule>(), 64);
    }

    #[test]
    fn test_processing_phase_descriptions() {
        assert_eq!(ProcessingPhase::Initializing.description(), "Initializing pipeline...");
        assert_eq!(ProcessingPhase::Adding.description(), "Adding documents...");
        assert_eq!(ProcessingPhase::Finding.description(), "Finding duplicates...");
    }
}
