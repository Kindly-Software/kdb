//! # T5 Streaming Tier
//!
//! **O(1) incremental computation primitives for streaming data.**
//!
//! This module provides 15 T5 Streaming capsules:
//! - **Window management**: `StreamingWindowCapsule<T>` (sliding/tumbling windows)
//! - **Aggregation**: `StreamingAggregationCapsule` (sum/avg/min/max/variance), `StreamingStatsCapsule`
//! - **Stream operators**: `StreamingJoinCapsule`, `StreamingFilterCapsule`, `StreamingMapCapsule`
//! - **Reduction**: `StreamingReduceCapsule`, `StreamingGroupByCapsule`
//! - **Deduplication**: `StreamingDedupCapsule<T>` (Bloom filter + exact hash)
//! - **Advanced**: `StrategyLabelerCapsule`, `AsyncLogCapsule`, `BTreeStatsCapsule`
//!
//! ## UCE34 Framework Application
//!
//! - **Q10**: Tier 5 Streaming (O(1) rolling window updates, incremental operations)
//! - **Q28 (Simplicity)**: Simple append() API, hide complexity
//! - **Q29 (Constraints)**: Fixed memory footprint, bounded history
//! - **Q30 (Validation)**: B32 benchmarks (95% CI, fair baselines)
//! - **Q31 (Rust Transform)**: Lockfree atomic coordination + O(1) ring buffers
//! - **Q33 (Verification)**: Compile-time capsule verification
//!
//! ## Design Principles
//!
//! All streaming capsules follow atomic capsule principles:
//! - O(1) update complexity (no iteration over history)
//! - Fixed memory footprint (ring buffers, not Vec)
//! - Lockfree coordination via atomics
//! - Cache-aligned structures (64B/128B/256B)
//!
//! ## ASSUM Framework
//!
//! Safety assumptions documented per capsule:
//! - `#ASSUME_RING_WRAP`: Modulo wraps cleanly for power-of-2 sizes
//! - `#ASSUME_RELAXED_METRICS`: Approximate metrics acceptable
//! - `#ASSUME_CACHE_ALIGNED`: 64/128/256-byte alignment prevents false sharing
//! - `#ASSUME_CAS_CONVERGENCE`: CAS succeeds within 10 attempts under normal load
//! - `#ASSUME_NUMERIC_STABILITY`: Welford's algorithm error <1e-12

// T5 Window Management (NEW - Nov 2025)
#[cfg(feature = "streaming-window")]
pub mod window;

// Nightly Phase 2: Const Generics Window (NEW - Nov 2025)
#[cfg(feature = "nightly-const-streaming")]
pub mod window_const;

// T5 Aggregation (NEW - Nov 2025)
#[cfg(feature = "streaming-aggregation")]
pub mod aggregation;

// T5 Stream Operators (NEW - Phase 2: Filter, Map, Reduce complete)
#[cfg(feature = "streaming-filter")]
pub mod filter;

#[cfg(feature = "streaming-map")]
pub mod map;

// T5 Reduction (NEW - Phase 2: Reduce complete)
#[cfg(feature = "streaming-reduce")]
pub mod reduce;

// #[cfg(feature = "streaming-join")]
// pub mod join;

// #[cfg(feature = "streaming-group-by")]
// pub mod group_by;

// #[cfg(feature = "streaming-dedup")]
// pub mod dedup;

// T5 Strategy Labeling (moved from kindly_hft)
#[cfg(feature = "streaming-strategy-labeler")]
pub mod strategy_labeler;

// Re-export for convenience
#[cfg(feature = "streaming-window")]
pub use window::{StreamingWindowCapsule, WindowType, WindowEntry, WINDOW_CAPACITY, DEFAULT_WINDOW_SIZE};

#[cfg(feature = "nightly-const-streaming")]
pub use window_const::{StreamingWindowConst, validate_window_ms, validate_sample_rate, calculate_window_size};

#[cfg(feature = "streaming-aggregation")]
pub use aggregation::{StreamingAggregationCapsule, AggregationSnapshot};

#[cfg(feature = "streaming-filter")]
pub use filter::StreamingFilterCapsule;

#[cfg(feature = "streaming-map")]
pub use map::StreamingMapCapsule;

#[cfg(feature = "streaming-reduce")]
pub use reduce::StreamingReduceCapsule;

// #[cfg(feature = "streaming-join")]
// pub use join::{StreamingJoinCapsule, JoinType};

// #[cfg(feature = "streaming-group-by")]
// pub use group_by::StreamingGroupByCapsule;

// #[cfg(feature = "streaming-dedup")]
// pub use dedup::StreamingDedupCapsule;

#[cfg(feature = "streaming-strategy-labeler")]
pub use strategy_labeler::{StrategyLabel, StrategyLabelerCapsule, StrategyStats};
