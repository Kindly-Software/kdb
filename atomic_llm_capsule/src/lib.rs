//! # Atomic LLM Capsule - LLM Quantization Primitives
//!
//! **Zero-copy, cache-aligned LLM inference primitives using atomic capsule architecture.**

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs, rust_2018_idioms)]

// Q32 Nightly Enhancement - Cutting-edge features
#![cfg_attr(feature = "portable_simd", feature(portable_simd))]
#![feature(generic_const_exprs)]
#![feature(atomic_from_mut)]
#![allow(incomplete_features)]

#[cfg(feature = "std")]
extern crate std;

// Public API modules
pub mod error;
pub mod traits;
pub mod primitives;
pub mod integration;

// Storage backend (feature-gated)
// TODO: Implement storage module (UCE-D7: disabled to fix compilation)
// #[cfg(feature = "std")]
// pub mod storage;

// Re-export foundation crate
pub use atomic_capsule;

// Re-export core traits
pub use traits::{
    QuantizedCapsule, StaticQuantizedCapsule, AdaptiveQuantizedCapsule,
    // TODO: Re-enable when ssd_backed module is implemented
    // SsdBackedCapsule, PrefetchHint, EvictionPolicy, AccessPattern,
};

// Re-export storage types
// TODO: Re-enable when storage module is implemented
// #[cfg(feature = "std")]
// pub use storage::{MmapBackend, MmapError};

// Re-export error types
pub use error::{QuantError, QuantResult};

// Re-export primitives for benchmarks and examples
pub use primitives::{MicroBlockQuantCapsule, AdaptiveQuantCapsule};
// TODO: Re-enable when kv_ssd module is implemented
// pub use primitives::StreamingKVCapsule;

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_exists() {
        assert!(!VERSION.is_empty());
    }
}
