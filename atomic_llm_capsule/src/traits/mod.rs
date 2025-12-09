//! # Trait Hierarchy for LLM Quantization Capsules
//!
//! **UCE33 Q33 (Atomic Capsule)**: Extends ComputationalCapsule for quantization primitives.
//!
//! ## Trait Hierarchy
//!
//! ```text
//! ComputationalCapsule (from atomic_capsule)
//!   └─ QuantizedCapsule (this crate)
//!       ├─ StaticQuantizedCapsule (fixed parameters)
//!       ├─ AdaptiveQuantizedCapsule (dynamic parameters)
//!       └─ SsdBackedCapsule (SSD-backed storage)
//!           ├─ PrefetchHint (sequential access optimization)
//!           └─ EvictionPolicy (lockfree RAM→SSD transfers)
//! ```
//!
//! ## IMPL-2 V3.0 Justification
//!
//! Each trait justified by 3+ implementations:
//!
//! 1. **QuantizedCapsule**: Base quantization operations
//!    - 1-bit: Binary quantization
//!    - 2-bit: 4-level quantization
//!    - 4-bit: 16-level quantization
//!    - 8-bit: 256-level quantization
//!    - 16-bit: High-precision quantization
//!
//! 2. **StaticQuantizedCapsule**: Fixed quantization parameters
//!    - INT8 quantization (static scale/zero-point)
//!    - INT4 quantization (static scale/zero-point)
//!    - Binary quantization (static threshold)
//!
//! 3. **AdaptiveQuantizedCapsule**: Dynamic quantization
//!    - Per-channel quantization (dynamic scale per channel)
//!    - Per-group quantization (dynamic scale per group)
//!    - Outlier-aware quantization (adaptive threshold)
//!
//! 4. **SsdBackedCapsule**: SSD-backed storage with lockfree eviction
//!    - INT8 SSD-backed weights
//!    - INT4 SSD-backed weights
//!    - Adaptive SSD-backed weights
//!
//! 5. **PrefetchHint**: Sequential access optimization
//!    - Layer-by-layer inference
//!    - Streaming batch processing
//!    - Model sharding across devices
//!
//! 6. **EvictionPolicy**: Lockfree eviction strategies
//!    - LRU (least-recently-used)
//!    - LFU (least-frequently-used)
//!    - Size-aware (largest first)
//!
//! ## UCE33 Q31 (Rust Transform)
//!
//! - **Const generics**: Bit width, group size compile-time parameters
//! - **Associated types**: Scale/zero-point types for static dispatch
//! - **Zero-cost abstractions**: All traits compile to optimal code

pub mod quantized;
pub mod adaptive;
// TODO: Implement ssd_backed module (UCE-D7: disabled to fix compilation)
// pub mod ssd_backed;

pub use quantized::{QuantizedCapsule, StaticQuantizedCapsule};
pub use adaptive::AdaptiveQuantizedCapsule;
// TODO: Re-enable when ssd_backed is implemented
// pub use ssd_backed::{SsdBackedCapsule, PrefetchHint, EvictionPolicy, AccessPattern, PAGE_SIZE, GENERATION_MASK};
