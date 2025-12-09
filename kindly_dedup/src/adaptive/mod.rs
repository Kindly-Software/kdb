//! Adaptive Pipeline Module - T6 Mixed Tier
//!
//! Provides runtime CPU/GPU mode selection based on performance metrics,
//! memory budget enforcement, and coordinated pipeline orchestration.
//!
//! # Architecture
//!
//! ```text
//! AdaptivePipelineCapsule (T6 Mixed Orchestrator)
//! +-- CrossoverDetectorCapsule (T1+T3) - EMA + hysteresis
//! +-- MemoryBudgetCapsule (T0) - O(1) enforcement
//! +-- WorkStealingCapsule (T4) - Transition coordination
//! ```
//!
//! # Week 5 Components (Complete Architecture)
//!
//! - **CrossoverDetectorCapsule**: T1+T3 EMA-based CPU/GPU crossover detection
//! - **MemoryBudgetCapsule**: T0 O(1) memory budget enforcement with presets
//! - **WorkStealingCapsule**: T4 Batch transition coordination
//! - **AdaptivePipelineCapsule**: T6 Mixed orchestrator (coordinates all sub-capsules)
//!
//! # Phase 2 Components (Legacy)
//!
//! - PipelineSelectorCapsule: Auto-selects between DedupPipeline and StreamingDedupPipeline
//! - RamDetectorCapsule: Detects available system RAM for tier selection
//!
//! # Framework Compliance
//!
//! - **UCE34 Q10**: T6 Mixed (compound: T0+T1+T3+T4)
//! - **Chaos**: 100% lockfree state management
//! - **ASSUM**: All assumptions documented
//! - **B32**: <500ns crossover decision
//!
//! # Example
//!
//! ```rust,ignore
//! use kindly_dedup::adaptive::{
//!     CrossoverDetectorCapsule, ExecutionMode,
//!     MemoryBudgetCapsule, memory_presets,
//!     AdaptivePipelineCapsule,
//! };
//!
//! // Memory-constrained environment
//! let budget = MemoryBudgetCapsule::new_gb(4);
//!
//! // Full adaptive pipeline
//! let pipeline = AdaptivePipelineCapsule::with_defaults();
//! assert_eq!(pipeline.current_mode(), ExecutionMode::CpuStreaming);
//!
//! // Crossover detection
//! let detector = CrossoverDetectorCapsule::new();
//! if let Some(new_mode) = detector.update_and_check(75_000, false) {
//!     match new_mode {
//!         ExecutionMode::GpuLsh => println!("Switching to GPU"),
//!         ExecutionMode::CpuStreaming => println!("Staying on CPU"),
//!     }
//! }
//! ```

// Crossover detector (T1+T3: Atomic + Fixed-Point EMA)
pub mod crossover_detector;

// Legacy selector (Phase 2: T0+T1 Auditable + Atomic)
pub mod selector;

// Work stealing coordinator (T4: Batch tier - transition phase coordination)
pub mod work_stealing;

// Memory budget enforcement (T0: Auditable tier - O(1) budget tracking)
pub mod memory_budget;

// Adaptive pipeline orchestrator (T6: Mixed tier - coordinates all sub-capsules)
pub mod pipeline_capsule;

// Re-exports: Crossover detector (primary)
pub use crossover_detector::{
    CrossoverDetectorCapsule,
    CrossoverSnapshot,
    ExecutionMode,
    STABILITY_THRESHOLD,
    ALPHA_Q16,
    CROSSOVER_THRESHOLD,
    HYSTERESIS_BAND,
};

// Re-exports: Legacy selector
pub use selector::{PipelineSelection, PipelineSelectorCapsule, RamDetectorCapsule};

// Re-exports: Work stealing coordinator
pub use work_stealing::{
    WorkStealingCapsule,
    WorkStealingSnapshot,
    TransitionPhase,
    WorkTarget,
    TransitionError,
};

// Re-exports: Memory budget (T0 Auditable tier)
pub use memory_budget::{
    MemoryBudgetCapsule,
    MemoryBudgetSnapshot,
    MemoryError,
    presets as memory_presets,
};

// Re-exports: Adaptive pipeline orchestrator (T6 Mixed tier)
pub use pipeline_capsule::{
    AdaptivePipelineCapsule,
    AdaptivePipelineConfig,
    AdaptivePipelineStats,
};

/// Adaptive pipeline feature flag
pub const ADAPTIVE_ENABLED: bool = true;

/// Check if adaptive pipeline is available
#[inline]
pub const fn is_adaptive_enabled() -> bool {
    ADAPTIVE_ENABLED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        let detector = CrossoverDetectorCapsule::new();
        assert_eq!(detector.get_recommendation(), ExecutionMode::CpuStreaming);
    }

    #[test]
    fn test_constants_exported() {
        assert_eq!(STABILITY_THRESHOLD, 10);
        assert!(ALPHA_Q16 > 0);
    }

    #[test]
    fn test_legacy_selector_exports() {
        // Verify legacy selector types are accessible
        let _selection = PipelineSelection::Fast;
        let _streaming = PipelineSelection::Streaming;
    }

    #[test]
    fn test_adaptive_enabled() {
        assert!(is_adaptive_enabled());
        assert!(ADAPTIVE_ENABLED);
    }

    #[test]
    fn test_work_stealing_exports() {
        let ws = WorkStealingCapsule::new();
        assert_eq!(ws.phase(), TransitionPhase::Steady);
    }

    #[test]
    fn test_memory_budget_exports() {
        let budget = MemoryBudgetCapsule::new_gb(1);
        assert!(budget.can_allocate(1000));
    }

    #[test]
    fn test_adaptive_pipeline_exports() {
        let pipeline = AdaptivePipelineCapsule::with_defaults();
        assert_eq!(pipeline.current_mode(), ExecutionMode::CpuStreaming);
    }
}
