//! # Image Processing Computational Capsules
//!
//! **T2+T3 SIMD + Fixed-Point image processing primitives for high-performance resampling.**
//!
//! This module provides image processing capsules optimized for forensic AI detection
//! and high-quality image resizing with the following design principles:
//!
//! ## Architecture
//!
//! - **Tier**: T2 SIMD + T3 Fixed-Point (compound 8-120× speedup)
//! - **Separable 2D**: O(2N) instead of O(N²) convolution
//! - **Compile-time LUT**: Lanczos3 kernel weights as const arrays
//! - **Cache-friendly**: 64×64 tile processing (12KB fits L1 cache)
//! - **True SIMD**: No `to_array()` in inner loops (breaks vectorization)
//!
//! ## UCE34 Framework Compliance
//!
//! - **Q10**: T2+T3 tier selection (SIMD vectorization + fixed-point determinism)
//! - **Q11**: 100% Rust, zero external dependencies
//! - **Q12**: Nightly features (portable_simd, const_fn_floating_point)
//! - **Q33**: Lockfree coordination (generation counters, atomic state)
//! - **Q34**: Audit trail ready (deterministic output, reproducible)
//!
//! ## Performance Targets (B32)
//!
//! | Operation | Current | Target | Improvement |
//! |-----------|---------|--------|-------------|
//! | 1024→224 resize | 3.9-61.5ms | <500µs | 8-120× |
//! | Horizontal pass | - | <200µs | - |
//! | Vertical pass | - | <200µs | - |
//!
//! ## Module Contents
//!
//! - [`Lanczos3KernelCapsule`]: SIMD-accelerated Lanczos3 separable convolution
//! - [`TileProcessorCapsule`]: Cache-friendly tile-based parallel processing
//!
//! ## ASSUM Safety Tags
//!
//! - `#ASSUME_LOCKFREE_COORDINATION`: All state via AtomicU64, no mutex/RwLock
//! - `#ASSUME_CACHE_ALIGNED`: 64-byte alignment prevents false sharing
//! - `#ASSUME_SIMD_ALIGNMENT`: Input slices aligned for f32x8 vectorization
//! - `#ASSUME_SEPARABLE_KERNEL`: Lanczos3 kernel is separable (h(x,y) = h(x)·h(y))
//! - `#ASSUME_LUT_BOUNDS`: Kernel LUT index clamped to [0, LANCZOS3_LUT_SIZE-1]

// Feature gate: Requires portable_simd for SIMD operations
#[cfg(feature = "portable_simd")]
pub mod lanczos3;

// Re-export main types
#[cfg(feature = "portable_simd")]
pub use lanczos3::{Lanczos3KernelCapsule, ResizeError, ResizeResult};

/// Image processing constants
pub mod constants {
    /// Lanczos3 kernel radius (3 lobes = 7-tap kernel)
    pub const LANCZOS3_RADIUS: usize = 3;

    /// Lanczos3 kernel taps (2 * radius + 1)
    pub const LANCZOS3_TAPS: usize = 7;

    /// LUT size for Lanczos3 kernel (256 entries covers [0, 3] with high precision)
    pub const LANCZOS3_LUT_SIZE: usize = 256;

    /// Fixed-point scale for kernel weights (Q16.16)
    pub const KERNEL_SCALE: i32 = 65536;

    /// Tile size for cache-friendly processing (64×64 = 12KB fits L1)
    pub const TILE_SIZE: usize = 64;

    /// Minimum supported image width/height
    pub const MIN_DIMENSION: usize = 8;

    /// Maximum supported image width/height
    pub const MAX_DIMENSION: usize = 16384;
}

#[cfg(test)]
mod tests {
    use super::constants::*;

    #[test]
    fn test_constants() {
        assert_eq!(LANCZOS3_RADIUS, 3);
        assert_eq!(LANCZOS3_TAPS, 7);
        assert_eq!(LANCZOS3_LUT_SIZE, 256);
        assert_eq!(KERNEL_SCALE, 65536);
        assert_eq!(TILE_SIZE, 64);

        // Verify tile fits L1 cache (64×64×3 RGB = 12,288 bytes < 32KB L1)
        let tile_bytes = TILE_SIZE * TILE_SIZE * 3;
        assert!(tile_bytes < 32768, "Tile must fit in L1 cache");
    }
}
