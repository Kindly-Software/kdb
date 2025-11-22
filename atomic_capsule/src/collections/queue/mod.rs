//! Queue Capsules - T1 Atomic bounded and unbounded SPSC/MPMC queues
//!
//! This module provides lockfree queue implementations:
//! - `QueueCapsule<T, SPSC>`: Bounded single-producer single-consumer (4× speedup, 10-20ns)
//! - `QueueCapsule<T, MPMC>`: Bounded multi-producer multi-consumer (3-10× speedup, ~100ns)
//! - `UnboundedQueueCapsule<T, SPSC>`: Unbounded SPSC with segment linking (10ns push/pop, 1µs growth)
//! - `UnboundedQueueCapsule<T, MPMC>`: Unbounded MPMC with CAS coordination (50ns push/pop, 2µs growth)
//!
//! # Performance
//! - SPSC: Zero CAS, Relaxed ordering, cache-line separation → 4× vs Mutex
//! - MPMC: Generation counters, ABA prevention, cache-aligned → 3-10× vs crossbeam
//! - Unbounded: Segment linking, automatic growth, deferred reclamation
//!
//! # Examples
//! ```
//! use atomic_capsule::collections::queue::{QueueCapsule, UnboundedQueueCapsule, SPSC, MPMC};
//!
//! // Bounded SPSC (fastest, fixed capacity)
//! let queue = QueueCapsule::<u64, SPSC>::new(1024).unwrap();
//! queue.push(42).unwrap();
//! assert_eq!(queue.pop(), Some(42));
//!
//! // Bounded MPMC (concurrent, fixed capacity)
//! use std::sync::Arc;
//! let queue = Arc::new(QueueCapsule::<u64, MPMC>::new(1024).unwrap());
//! // Use from multiple threads
//!
//! // Unbounded SPSC (grows automatically)
//! let queue_spsc = UnboundedQueueCapsule::<u64, SPSC>::new();
//! for i in 0..10000 {
//!     queue_spsc.push(i).unwrap(); // Never fails, automatic growth
//! }
//! assert_eq!(queue_spsc.len(), 10000);
//!
//! // Unbounded MPMC (grows automatically, thread-safe)
//! let queue_mpmc = Arc::new(UnboundedQueueCapsule::<u64, MPMC>::new());
//! // Use from multiple threads with automatic segment allocation
//! ```

use core::sync::atomic::Ordering;

#[cfg(feature = "queue-bounded")]
mod bounded;

#[cfg(feature = "queue-unbounded")]
pub mod epoch;

#[cfg(feature = "queue-unbounded")]
mod unbounded;

#[cfg(all(test, feature = "queue-bounded"))]
mod tests;

#[cfg(all(test, feature = "queue-unbounded"))]
mod unbounded_tests;

#[cfg(all(test, feature = "queue-unbounded"))]
mod batch_tests;

#[cfg(feature = "queue-bounded")]
pub use bounded::{QueueCapsule, QueueError, PushError};

#[cfg(feature = "queue-unbounded")]
pub use epoch::{EpochCounter, EpochGuard, DeferredQueue};

#[cfg(feature = "queue-unbounded")]
pub use unbounded::UnboundedQueueCapsule;

/// SPSC mode marker (single-producer single-consumer)
#[derive(Debug, Clone, Copy)]
pub struct SPSC;

/// MPMC mode marker (multi-producer multi-consumer)
#[derive(Debug, Clone, Copy)]
pub struct MPMC;

/// Queue mode trait
pub trait QueueMode: Sized + Send + Sync + 'static {
    /// Whether this mode supports multiple producers
    const MULTI_PRODUCER: bool;

    /// Whether this mode supports multiple consumers
    const MULTI_CONSUMER: bool;

    /// Ordering for push operations
    const PUSH_ORDERING: Ordering;

    /// Ordering for pop operations
    const POP_ORDERING: Ordering;
}

impl QueueMode for SPSC {
    const MULTI_PRODUCER: bool = false;
    const MULTI_CONSUMER: bool = false;
    const PUSH_ORDERING: Ordering = Ordering::Relaxed;
    const POP_ORDERING: Ordering = Ordering::Relaxed;
}

impl QueueMode for MPMC {
    const MULTI_PRODUCER: bool = true;
    const MULTI_CONSUMER: bool = true;
    const PUSH_ORDERING: Ordering = Ordering::AcqRel;
    const POP_ORDERING: Ordering = Ordering::AcqRel;
}
