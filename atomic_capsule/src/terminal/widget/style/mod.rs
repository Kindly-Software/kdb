//! Widget Style System
//!
//! Computational capsule-based style system for terminal widgets with Q8.8 fixed-point precision.

pub mod cache;
pub mod computed;

pub use cache::{CacheStats, StyleCacheCapsule};
pub use computed::{ComputedStyleCapsule, PseudoState, flags};
