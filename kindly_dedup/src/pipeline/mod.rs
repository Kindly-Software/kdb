//! # Phase 3 Pipeline Integration
//!
//! **Status**: Phase 3.1 - Universal Pipeline Wrapper Capsule
//! **Tier**: T6 Mixed (Wrapper Capsule + Arc<DedupMetacapsule>)
//!
//! ## Architecture
//!
//! ```text
//! UniversalDedupPipelineCapsule (T6 Mixed Wrapper, 128 bytes)
//! ├── metacapsule: Arc<DedupMetacapsule>     (Orchestrator reference)
//! ├── config: DedupConfig                     (Read-only configuration)
//! ├── state: AtomicU64                        (Wrapper state machine)
//! ├── error_ptr: AtomicPtr<String>            (Optional error message)
//! └── _padding: [u8; 48]                      (Cache alignment)
//! ```
//!
//! ## Wrapper Pattern
//!
//! **Key Design** (per user requirement): "make sure the wrapper is also a capsule"
//! - Wrapper IS a ComputationalCapsule (#[derive(ComputationalCapsule)])
//! - Holds Arc<DedupMetacapsule> (orchestrator reference, like RatatuiProgressAdapter)
//! - 128-byte cache-aligned (orchestrator wrapper pattern)
//! - 100% lockfree coordination (atomic state machine)
//!
//! ## Backward Compatibility
//!
//! Old API preserved (feature-gated):
//! ```rust
//! let pipeline = UniversalDedupPipelineCapsule::new(
//!     "corpus.jsonl",
//!     100_000,
//!     0.85,
//!     0,
//!     100_000,
//! )?;
//! pipeline.process_corpus()?;
//! let clusters = pipeline.find_duplicates(0.85)?;
//! ```
//!
//! ## Modules
//!
//! - `universal_capsule`: Wrapper capsule implementation (~500 lines)
//! - `stage_wiring`: 3-stage lockfree coordination (~300 lines)

pub mod universal_capsule;
pub mod stage_wiring;

pub use universal_capsule::{
    UniversalDedupPipelineCapsule, WrapperError, WrapperResult, WrapperState, DedupConfig,
};

pub use stage_wiring::{
    stage1_streaming_loop, stage2_worker_loop, stage3_wait_for_completion,
    execute_3_stage_pipeline, spawn_stage2_workers, StageError,
};

// Re-export legacy pipeline types for backward compatibility
// These are imported from the legacy_pipeline module when phase3-metacapsule is enabled
// This allows existing code to continue using `crate::pipeline::DocId` etc.
pub use crate::legacy_pipeline::{DocId, JaccardThreshold, PipelineError, DedupPipeline};
