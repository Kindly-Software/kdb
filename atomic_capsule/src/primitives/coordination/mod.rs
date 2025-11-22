//! # Coordination Primitives - Lockfree Coordination Patterns
//!
//! **Lockfree coordination primitives** for multi-threaded algorithms.
//!
//! This module provides atomic coordination patterns for:
//! - **Phase coordination**: Multi-phase workflow state machines
//! - **Hash bucketing**: Collision chaining for hash tables
//! - **Parallel partitioning**: Work distribution for parallel algorithms
//!
//! ## Design Principles
//!
//! All coordination primitives follow atomic design principles:
//! - 100% lockfree (no mutex/RwLock)
//! - Cache-aligned structures (128B or 256B)
//! - Generation counters for TOCTOU prevention
//! - Memory ordering documentation (Acquire/Release/Relaxed)
//!
//! ## Performance Characteristics
//!
//! - **PhaseCoordinatorCapsule**: <20ns transitions, <10ns queries
//! - **LockfreeHashBucketCapsule**: <50ns insert, <10ns probe
//! - **ParallelPartitionCapsule**: <20ns thread-local, <15ns shared operations
//!
//! ## Example
//!
//! ```rust
//! use atomic_capsule::primitives::coordination::{
//!     PhaseCoordinatorCapsule,
//!     LockfreeHashBucketCapsule,
//!     ParallelPartitionCapsule,
//! };
//!
//! // Phase coordination
//! let coord = PhaseCoordinatorCapsule::new();
//! coord.start_phase(1).unwrap();
//! coord.finish_phase(1).unwrap();
//!
//! // Hash bucketing
//! let bucket = LockfreeHashBucketCapsule::new();
//! bucket.insert(42, 100).unwrap();
//! assert_eq!(bucket.probe(42), Some(100));
//!
//! // Parallel partitioning
//! let partition = ParallelPartitionCapsule::new();
//! partition.push_result().unwrap();
//! partition.increment_processed(1);
//! ```

pub mod phase_coordinator;
pub mod hash_bucket;
pub mod parallel_partition;

// Re-export main types
pub use phase_coordinator::{PhaseCoordinatorCapsule, PhaseError, PhaseStats, PhaseStatus};
pub use hash_bucket::{LockfreeHashBucketCapsule, InsertError, BucketStats};
pub use parallel_partition::{ParallelPartitionCapsule, PartitionError, PartitionStats, PartitionStatus};

// Comprehensive T28 tests (36+ tests, 100% coverage)
#[cfg(test)]
mod tests;
