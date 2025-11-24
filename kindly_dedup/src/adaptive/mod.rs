//! # Phase 2: Adaptive Pipeline Selection (T0+T1 Tier)
//!
//! **UCE34 Framework**: Q1-Q34 systematic discovery applied
//! **Computational Capsule**: T0 (Auditable) + T1 (Atomic) tier
//!
//! ## Overview
//!
//! This module provides automatic selection between:
//! 1. **DedupPipeline** (legacy): O(N) memory, 136K docs/sec (fast but RAM-limited)
//! 2. **StreamingDedupPipeline**: O(1) 273 MB constant memory, 30-100K docs/sec target (safe, scalable)
//!
//! Users don't need to understand memory complexity - the system chooses optimally.

pub mod selector;

pub use selector::{PipelineSelection, PipelineSelectorCapsule, RamDetectorCapsule};
