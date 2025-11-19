//! Binary format implementations for CapsuleSerialize
//!
//! This module provides the concrete implementations of CapsuleSerialize
//! for basic types (primitives, arrays, tuples).
//!
//! Also re-exports binary format primitives from binary_format module (when enabled).

// Re-export binary format primitives (when feature is enabled)
#[cfg(all(feature = "std", feature = "crc32fast"))]
pub use super::binary_format::*;
