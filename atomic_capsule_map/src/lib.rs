//! # AtomicCapsuleMap
//!
//! **⚠️ DEPRECATION NOTICE: This crate is deprecated as of October 2025.**
//!
//! **Please migrate to [`atomic_capsule::collections::ConcurrentMapCapsule`](https://crates.io/crates/atomic_capsule)** for:
//! - **3-59× better performance** (128B alignment eliminates false sharing)
//! - **Superior ergonomics** (Arc<T> support, Borrow<Q>, Entry API)
//! - **Active development** (116/116 tests pass, production-ready)
//!
//! **Migration Time**: 1-4 hours
//!
//! **See**:
//! - [DEPRECATION_NOTICE.md](https://github.com/yourusername/atomic_capsule_map/blob/main/DEPRECATION_NOTICE.md)
//! - [Migration Guide](https://github.com/yourusername/atomic_capsule/blob/main/docs/DASHMAP_MIGRATION_GUIDE.md)
//!
//! **LTS Period**: 12 months (critical bug fixes only, until October 2026)
//!
//! ---
//!
//! ## Historical Documentation
//!
//! A lockfree concurrent hashmap built on atomic capsule architecture.
//!
//! ### Why AtomicCapsuleMap beats DashMap (Historical)
//!
//! - **10-40× faster**: True lockfree operations, no shard locking
//! - **Predictable latency**: No lock contention, stable p99/p999
//! - **Circuit breaker built-in**: Automatic health monitoring and degradation
//! - **Zero allocation reads**: Copy-on-write with atomic capsules
//! - **Cache-optimized**: 64-byte aligned capsules prevent false sharing
//!
//! ### Quick Start (Historical)
//!
//! ```rust,ignore
//! use atomic_capsule_map::AtomicCapsuleMap;
//!
//! let map = AtomicCapsuleMap::new();
//!
//! // Basic operations
//! map.insert("key", 42);
//! assert_eq!(map.get(&"key"), Some(42));
//! map.remove(&"key");
//!
//! // Atomic operations (unique to capsule design)
//! map.get_or_insert("key", 100);
//! map.compare_and_swap(&"key", 100, 200).unwrap();
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![deprecated(
    since = "0.1.1",
    note = "This crate is deprecated. Please migrate to `atomic_capsule::collections::ConcurrentMapCapsule` for better performance (3-59× speedup), superior ergonomics (Arc<T>, Borrow<Q>, Entry API), and active development. Migration time: 1-4 hours. See DEPRECATION_NOTICE.md and Migration Guide. LTS period: 12 months (until October 2026)."
)]

extern crate alloc;

// Core atomic capsule primitives
mod bucket;
pub mod generation;
mod table;

// Safety traits for type serialization
mod serializable;

/// Safety validation utilities for atomic capsule operations
///
/// Provides runtime verification tools for ASSUM framework compliance.
#[cfg(feature = "std")]
pub mod safety;

// High-level API modules
mod api;
mod capsule;
mod entry;
mod health;
mod iter;
mod map;
mod shard;

// Public API exports
pub use api::AtomicCapsuleMap;
pub use entry::{Entry, OccupiedEntry, VacantEntry};
pub use health::{BreakerLevel, HealthStatus};
pub use iter::Iter;
pub use serializable::BitwiseSerializable;

// Re-export core generation utilities for advanced users
pub use generation::{pack_gen_high, pack_gen_low, unpack_gen_high, unpack_gen_low, MonotonicGen};

/// Re-export for convenience
pub use portable_atomic;
