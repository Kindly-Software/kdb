//! kindly-gui Integration Module (Wave 7)
//!
//! 100% Chaos-compliant GUI using kindly-gui framework from atomic_capsule.
//! Replaces Iced-based GUI to fix memory corruption crashes.
//!
//! # Architecture
//!
//! ```text
//! KindlyDedupAppCapsule (T6 Mixed, 256B)
//! ├── AppStateCapsule (T1, 64B) - FSM state machine
//! ├── FileInputState (T1, 32B) - File path + size
//! ├── SettingsState (T1, 16B) - Threshold + mode
//! ├── ProcessingState (T1, 64B) - Progress tracking
//! ├── ResultsState (T1, 48B) - Dedup results
//! └── AnimationState (T3, 32B) - Spring + pulse + shimmer
//! ```
//!
//! # Framework Compliance
//!
//! - **UCE34**: T6 Mixed tier (T1 Atomic + T3 Fixed-Point + T5 Streaming)
//! - **Chaos**: 100% lockfree (AtomicU64 state packing, no mutex)
//! - **ASSUM**: All assumptions documented with #VERIFY proofs
//! - **B32**: Benchmarks for <3% CPU @ 60 FPS
//! - **T28**: 5-tier test coverage
//! - **I20**: Zero breaking changes (additive module)

pub mod app;
pub mod events;
pub mod effects;
pub mod state_machine;
pub mod widgets;
pub mod layout;
pub mod animation;
pub mod integration;
pub mod render;
pub mod visual_effects;
pub mod rendering_primitives;

#[cfg(test)]
pub mod tests;

// Re-export main types
pub use app::{KindlyDedupAppCapsule, ResultsSnapshot};
pub use events::GuiEvent;
pub use effects::{ErrorKind, GuiEffect};
pub use state_machine::{AppState, ProcessingPhase};
pub use visual_effects::{ByzantineBorderCapsule, GlassmorphicCapsule, NoiseTextureCapsule};
pub use rendering_primitives::{Shape, TextCommand, TextAlign, TextContent};
// ExecutionMode is re-exported from crate::adaptive

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify module structure compiles
        let _ = std::mem::size_of::<AppState>();
    }
}
