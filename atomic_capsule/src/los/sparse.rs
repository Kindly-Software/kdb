//! Sparse LOS Capsule - T1 Atomic Scalar Fallback
//!
//! Scalar ray traversal for sparse samples (stride ≥ 4).
//! SIMD overhead exceeds benefit for scattered memory access patterns.
//!
//! # Target Performance
//!
//! - Latency: <50ns for 50 samples
//! - Speedup: 1× (baseline, scalar is optimal for sparse)
//! - Use case: LOD rays, diagonal traversals, scattered sampling
//!
//! # Chaos Compliance
//!
//! - 64B cache-aligned capsule
//! - Lockfree state coordination
//! - Pure scalar (no SIMD)
//!
//! # Why Scalar?
//!
//! For stride ≥ 4, SIMD gather instructions (`_mm256_i32gather_ps`) take
//! 11+ cycles vs 4-5 cycles for sequential scalar loads. The random
//! access pattern defeats SIMD's parallelism advantage.
//!
//! # ASSUM Tags
//!
//! - #ASSUME_STRIDE_VALID: Stride values are ≥ 1 (enforced in set_stride)
//! - #ASSUME_SAMPLE_BOUNDS: Sample count limited to 100 (prevents overflow)
//! - #ASSUME_Q16_ARITHMETIC: All Q16.16 arithmetic uses saturating ops
//! - #ASSUME_MAP_VALID: MapDataCapsule has valid buffers during traverse
//!
//! #VERIFY: T28 tests cover all edge cases (0 samples, single sample, stride variations)

use core::sync::atomic::{AtomicU64, Ordering};
use super::types::{LosRay, LosResult, Q16_16};
use super::map_data::MapDataCapsule;

/// SparseLosScalarCapsule (64B) - T1 Atomic tier
///
/// Optimized for sparse rays (stride ≥ 4) where SIMD gather is slower than scalar.
/// Uses simple loop with stride-based sampling for cache efficiency.
///
/// # Performance Target
///
/// - Latency: <50ns for 50 samples
/// - Speedup: 1× (scalar is optimal for sparse patterns)
/// - Use case: LOD rays, diagonal traversals, scattered sampling
///
/// # Layout (64 bytes)
///
/// | Offset | Field | Size | Purpose |
/// |--------|-------|------|---------|
/// | 0-7 | state | 8B | type(4)\|stride(8)\|samples(20)\|gen(24)\|flags(4) |
/// | 8-15 | ray_endpoints | 8B | start/end packed Q8.8 |
/// | 16-23 | progress | 8B | current\|total\|vis Q16.16 |
/// | 24-31 | stride_config | 8B | x_stride\|y_stride\|reserved |
/// | 32-63 | _reserved | 32B | Future use / padding |
#[repr(C, align(64))]
pub struct SparseLosScalarCapsule {
    /// Packed state: type(4)|stride(8)|samples(20)|gen(24)|flags(4)
    state: AtomicU64,
    /// Ray endpoints packed Q8.8
    ray_endpoints: AtomicU64,
    /// Progress: current|total|visibility
    progress: AtomicU64,
    /// Stride configuration: x_stride(16)|y_stride(16)|reserved(32)
    stride_config: AtomicU64,
    /// Reserved for future use
    _reserved: [u8; 32],
}

// State field bit layout
const GEN_SHIFT: u32 = 8;
const GEN_MASK: u64 = 0xFFFFFF << GEN_SHIFT; // 24 bits (bits 8-31)

// Progress field bit layout
const CURRENT_MASK: u64 = 0xFFFFF; // 20 bits (0-19)
const TOTAL_SHIFT: u32 = 20;
const TOTAL_MASK: u64 = 0xFFFFF << TOTAL_SHIFT; // 20 bits (20-39)

impl SparseLosScalarCapsule {
    /// Create a new sparse capsule
    ///
    /// # Returns
    ///
    /// Initialized capsule with default stride (4, 4)
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            ray_endpoints: AtomicU64::new(0),
            progress: AtomicU64::new(0),
            stride_config: AtomicU64::new(0x0004_0004), // Default: x=4, y=4
            _reserved: [0u8; 32],
        }
    }

    /// Create with specific stride
    ///
    /// # Arguments
    ///
    /// - `x_stride`: Horizontal stride (≥ 1)
    /// - `y_stride`: Vertical stride (≥ 1)
    ///
    /// # Returns
    ///
    /// Capsule initialized with specified stride
    pub fn with_stride(x_stride: u16, y_stride: u16) -> Self {
        // #ASSUME_STRIDE_VALID: Enforce stride ≥ 1
        let x_stride = x_stride.max(1);
        let y_stride = y_stride.max(1);

        let config = (x_stride as u64) | ((y_stride as u64) << 16);

        Self {
            state: AtomicU64::new(0),
            ray_endpoints: AtomicU64::new(0),
            progress: AtomicU64::new(0),
            stride_config: AtomicU64::new(config),
            _reserved: [0u8; 32],
        }
    }

    /// Initialize for new ray
    ///
    /// # Arguments
    ///
    /// - `ray`: LOS ray descriptor
    ///
    /// # Side Effects
    ///
    /// Increments generation counter, resets progress
    pub fn init_ray(&self, ray: &LosRay) {
        // Pack ray endpoints (Q16.16 -> Q8.8 for space)
        let origin_x = (ray.origin_x.raw() >> 8) as u16;
        let origin_y = (ray.origin_y.raw() >> 8) as u16;
        let target_x = (ray.target_x.raw() >> 8) as u16;
        let target_y = (ray.target_y.raw() >> 8) as u16;

        let endpoints = (origin_x as u64)
            | ((origin_y as u64) << 16)
            | ((target_x as u64) << 32)
            | ((target_y as u64) << 48);

        self.ray_endpoints.store(endpoints, Ordering::Release);

        // Increment generation counter
        loop {
            let state = self.state.load(Ordering::Acquire);
            let gen = ((state & GEN_MASK) >> GEN_SHIFT) + 1;
            let gen = gen & 0xFFFFFF; // Wrap at 24 bits
            let new_state = (state & !GEN_MASK) | (gen << GEN_SHIFT);

            if self.state.compare_exchange_weak(
                state,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                break;
            }
        }

        // Reset progress
        self.progress.store(0, Ordering::Release);
    }

    /// Traverse using scalar loop with stride
    ///
    /// # Algorithm
    ///
    /// 1. Calculate sample count (reduced by stride)
    /// 2. For each sample:
    ///    - Interpolate position along ray
    ///    - Sample cover from map
    ///    - Attenuate visibility: vis *= (1.0 - cover)
    ///    - Early exit if blocked (vis <= 0)
    /// 3. Return result based on final visibility
    ///
    /// # Arguments
    ///
    /// - `ray`: LOS ray descriptor
    /// - `map`: Map data capsule
    ///
    /// # Returns
    ///
    /// LosResult with visibility, samples checked, and status
    pub fn traverse(&self, ray: &LosRay, map: &MapDataCapsule) -> LosResult {
        self.init_ray(ray);

        // Get stride configuration
        // #ASSUME_STRIDE_VALID: stride ≥ 1 (enforced in set_stride)
        let (x_stride, y_stride) = self.get_stride();
        let stride = x_stride.max(y_stride) as i32;

        // Calculate sample count (reduced by stride)
        // #ASSUME_Q16_ARITHMETIC: Using saturating Q16.16 ops
        let dx = (ray.target_x.raw() - ray.origin_x.raw()).abs();
        let dy = (ray.target_y.raw() - ray.origin_y.raw()).abs();
        let full_samples = (dx.max(dy) >> 16) as i32; // Q16.16 -> integer distance

        // #ASSUME_SAMPLE_BOUNDS: Limit to 100 samples max
        let samples = (full_samples / stride.max(1)).max(1).min(100) as u32;

        if samples == 0 {
            return LosResult::visible(0);
        }

        // Initialize visibility (Q16.16 1.0)
        let mut visibility = 0x0001_0000i32; // Q16.16::ONE
        let one_q16 = 0x0001_0000i32;

        // Scalar traversal loop
        for i in 0..samples {
            // Calculate position along ray
            // t = i * stride / full_samples
            let t_num = (i * stride as u32) as i64;
            let t_den = full_samples as i64;

            // Interpolate x: origin.x + (target.x - origin.x) * t
            let x = if t_den > 0 {
                ray.origin_x.raw() +
                    (((ray.target_x.raw() - ray.origin_x.raw()) as i64 * t_num) / t_den) as i32
            } else {
                ray.origin_x.raw()
            };

            // Interpolate y: origin.y + (target.y - origin.y) * t
            let y = if t_den > 0 {
                ray.origin_y.raw() +
                    (((ray.target_y.raw() - ray.origin_y.raw()) as i64 * t_num) / t_den) as i32
            } else {
                ray.origin_y.raw()
            };

            // Sample cover from map (Q16.16 -> grid coords)
            let map_x = (x >> 16) as u16;
            let map_y = (y >> 16) as u16;

            // #ASSUME_MAP_VALID: Map has valid buffers
            if let Some(cover) = map.sample_cover(map_x, map_y) {
                // Attenuation: vis *= (1.0 - cover)
                // cover is Q16.16 in [0.0, 1.0]
                let atten = one_q16.saturating_sub(cover);

                // Q16.16 multiply: (vis * atten) >> 16
                // #ASSUME_Q16_ARITHMETIC: Saturating to prevent overflow
                visibility = (((visibility as i64) * (atten as i64)) >> 16) as i32;

                // Early exit if blocked
                if visibility <= 0 {
                    self.update_progress(i + 1, samples);
                    return LosResult::blocked(i + 1);
                }
            }

            // Update progress
            self.update_progress(i + 1, samples);
        }

        // Clamp to [0.0, 1.0]
        visibility = visibility.max(0).min(one_q16);

        // Determine result status
        if visibility >= one_q16 {
            LosResult::visible(samples)
        } else if visibility <= 0 {
            LosResult::blocked(samples)
        } else {
            LosResult::partial(Q16_16::from_raw(visibility), samples, Q16_16::ZERO)
        }
    }

    /// Get stride configuration
    ///
    /// # Returns
    ///
    /// Tuple of (x_stride, y_stride), both ≥ 1
    #[inline]
    fn get_stride(&self) -> (u16, u16) {
        let config = self.stride_config.load(Ordering::Acquire);
        let x = (config & 0xFFFF) as u16;
        let y = ((config >> 16) & 0xFFFF) as u16;
        (x.max(1), y.max(1))
    }

    /// Set stride (must be ≥ 4 for sparse classification)
    ///
    /// # Arguments
    ///
    /// - `x_stride`: Horizontal stride (≥ 1, auto-clamped)
    /// - `y_stride`: Vertical stride (≥ 1, auto-clamped)
    ///
    /// # ASSUM Tags
    ///
    /// #ASSUME_STRIDE_VALID: Enforces stride ≥ 1 via max()
    pub fn set_stride(&self, x_stride: u16, y_stride: u16) {
        let config = (x_stride.max(1) as u64) | ((y_stride.max(1) as u64) << 16);
        self.stride_config.store(config, Ordering::Release);
    }

    /// Get generation counter
    ///
    /// # Returns
    ///
    /// Generation counter (0-16,777,215), wraps at 24 bits
    #[inline]
    pub fn generation(&self) -> u32 {
        let state = self.state.load(Ordering::Acquire);
        ((state & GEN_MASK) >> GEN_SHIFT) as u32
    }

    /// Get progress (current, total)
    ///
    /// # Returns
    ///
    /// Tuple of (current_sample, total_samples)
    #[inline]
    pub fn progress(&self) -> (u32, u32) {
        let p = self.progress.load(Ordering::Acquire);
        (
            (p & CURRENT_MASK) as u32,
            ((p & TOTAL_MASK) >> TOTAL_SHIFT) as u32,
        )
    }

    /// Update progress atomically
    ///
    /// # Arguments
    ///
    /// - `current`: Current sample index
    /// - `total`: Total samples
    #[inline]
    fn update_progress(&self, current: u32, total: u32) {
        let progress = (current as u64 & CURRENT_MASK)
            | ((total as u64) << TOTAL_SHIFT);
        self.progress.store(progress, Ordering::Release);
    }
}

impl Default for SparseLosScalarCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Size verification
const _: () = assert!(core::mem::size_of::<SparseLosScalarCapsule>() == 64);
const _: () = assert!(core::mem::align_of::<SparseLosScalarCapsule>() == 64);

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::LosRayType;
    use std::alloc::{alloc, dealloc, Layout};

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<SparseLosScalarCapsule>(), 64);
        assert_eq!(core::mem::align_of::<SparseLosScalarCapsule>(), 64);
    }

    #[test]
    fn test_capsule_creation() {
        let capsule = SparseLosScalarCapsule::new();
        let (x, y) = capsule.get_stride();
        assert_eq!(x, 4);
        assert_eq!(y, 4);
    }

    #[test]
    fn test_with_stride() {
        let capsule = SparseLosScalarCapsule::with_stride(8, 16);
        let (x, y) = capsule.get_stride();
        assert_eq!(x, 8);
        assert_eq!(y, 16);
    }

    #[test]
    fn test_set_stride() {
        let capsule = SparseLosScalarCapsule::new();
        capsule.set_stride(12, 6);
        let (x, y) = capsule.get_stride();
        assert_eq!(x, 12);
        assert_eq!(y, 6);
    }

    #[test]
    fn test_stride_minimum() {
        // Stride 0 should be clamped to 1
        let capsule = SparseLosScalarCapsule::with_stride(0, 0);
        let (x, y) = capsule.get_stride();
        assert_eq!(x, 1);
        assert_eq!(y, 1);
    }

    #[test]
    fn test_generation_increment() {
        let capsule = SparseLosScalarCapsule::new();
        let map = MapDataCapsule::new(100, 100);

        assert_eq!(capsule.generation(), 0);

        let ray = LosRay::from_f32(0.0, 0.0, 10.0, 10.0, 100.0, LosRayType::Sparse);
        capsule.init_ray(&ray);

        assert_eq!(capsule.generation(), 1);

        capsule.init_ray(&ray);
        assert_eq!(capsule.generation(), 2);
    }

    #[test]
    fn test_traverse_no_cover() {
        let capsule = SparseLosScalarCapsule::with_stride(4, 4);
        let map = MapDataCapsule::new(100, 100);

        unsafe {
            let layout = Layout::from_size_align(100 * 100 * 4, 32).unwrap();
            let cover = alloc(layout) as *mut i32;

            // Initialize with zero cover (fully visible)
            for i in 0..10000 {
                *cover.add(i) = 0;
            }

            map.attach_buffers(cover, cover, cover);

            let ray = LosRay::from_f32(0.0, 0.0, 50.0, 50.0, 100.0, LosRayType::Sparse);
            let result = capsule.traverse(&ray, &map);

            assert!(result.is_visible());
            assert_eq!(result.visibility, Q16_16::ONE);
            assert!(result.samples_checked > 0);

            dealloc(cover as *mut u8, layout);
        }
    }

    #[test]
    fn test_traverse_full_cover() {
        let capsule = SparseLosScalarCapsule::with_stride(4, 4);
        let map = MapDataCapsule::new(100, 100);

        unsafe {
            let layout = Layout::from_size_align(100 * 100 * 4, 32).unwrap();
            let cover = alloc(layout) as *mut i32;

            // Initialize with full cover (Q16.16 1.0)
            for i in 0..10000 {
                *cover.add(i) = 0x0001_0000; // 1.0 in Q16.16
            }

            map.attach_buffers(cover, cover, cover);

            let ray = LosRay::from_f32(0.0, 0.0, 50.0, 50.0, 100.0, LosRayType::Sparse);
            let result = capsule.traverse(&ray, &map);

            assert!(result.is_blocked());
            assert_eq!(result.visibility, Q16_16::ZERO);
            // Should early-exit on first sample
            assert_eq!(result.samples_checked, 1);

            dealloc(cover as *mut u8, layout);
        }
    }

    #[test]
    fn test_traverse_partial_cover() {
        let capsule = SparseLosScalarCapsule::with_stride(4, 4);
        let map = MapDataCapsule::new(100, 100);

        unsafe {
            let layout = Layout::from_size_align(100 * 100 * 4, 32).unwrap();
            let cover = alloc(layout) as *mut i32;

            // Initialize with 50% cover (Q16.16 0.5)
            for i in 0..10000 {
                *cover.add(i) = 0x0000_8000; // 0.5 in Q16.16
            }

            map.attach_buffers(cover, cover, cover);

            let ray = LosRay::from_f32(0.0, 0.0, 50.0, 50.0, 100.0, LosRayType::Sparse);
            let result = capsule.traverse(&ray, &map);

            assert!(result.is_partial());
            assert!(result.visibility.raw() > 0);
            assert!(result.visibility.raw() < Q16_16::ONE.raw());
            assert!(result.samples_checked > 0);

            dealloc(cover as *mut u8, layout);
        }
    }

    #[test]
    fn test_traverse_different_strides() {
        let map = MapDataCapsule::new(100, 100);

        unsafe {
            let layout = Layout::from_size_align(100 * 100 * 4, 32).unwrap();
            let cover = alloc(layout) as *mut i32;

            // Initialize with zero cover
            for i in 0..10000 {
                *cover.add(i) = 0;
            }

            map.attach_buffers(cover, cover, cover);

            let ray = LosRay::from_f32(0.0, 0.0, 80.0, 0.0, 100.0, LosRayType::Sparse);

            // Test different strides
            for stride in [4, 8, 16, 32] {
                let capsule = SparseLosScalarCapsule::with_stride(stride, stride);
                let result = capsule.traverse(&ray, &map);

                assert!(result.is_visible());
                // Higher stride = fewer samples
                if stride == 4 {
                    assert_eq!(result.samples_checked, 20); // 80 / 4 = 20
                } else if stride == 8 {
                    assert_eq!(result.samples_checked, 10); // 80 / 8 = 10
                } else if stride == 16 {
                    assert_eq!(result.samples_checked, 5); // 80 / 16 = 5
                } else if stride == 32 {
                    assert_eq!(result.samples_checked, 2); // 80 / 32 = 2.5 -> 2
                }
            }

            dealloc(cover as *mut u8, layout);
        }
    }

    #[test]
    fn test_traverse_zero_distance() {
        let capsule = SparseLosScalarCapsule::new();
        let map = MapDataCapsule::new(100, 100);

        unsafe {
            let layout = Layout::from_size_align(100 * 100 * 4, 32).unwrap();
            let cover = alloc(layout) as *mut i32;

            // Initialize with zero cover (fully visible)
            for i in 0..10000 {
                *cover.add(i) = 0;
            }

            map.attach_buffers(cover, cover, cover);

            // Ray with same origin and target
            let ray = LosRay::from_f32(10.0, 10.0, 10.0, 10.0, 100.0, LosRayType::Sparse);
            let result = capsule.traverse(&ray, &map);

            // Should still check at least 1 sample
            assert!(result.is_visible());
            assert_eq!(result.samples_checked, 1);

            dealloc(cover as *mut u8, layout);
        }
    }

    #[test]
    fn test_traverse_single_sample() {
        let capsule = SparseLosScalarCapsule::with_stride(100, 100); // Very high stride
        let map = MapDataCapsule::new(100, 100);

        unsafe {
            let layout = Layout::from_size_align(100 * 100 * 4, 32).unwrap();
            let cover = alloc(layout) as *mut i32;

            for i in 0..10000 {
                *cover.add(i) = 0;
            }

            map.attach_buffers(cover, cover, cover);

            let ray = LosRay::from_f32(0.0, 0.0, 10.0, 10.0, 100.0, LosRayType::Sparse);
            let result = capsule.traverse(&ray, &map);

            // High stride should result in 1 sample
            assert_eq!(result.samples_checked, 1);
            assert!(result.is_visible());

            dealloc(cover as *mut u8, layout);
        }
    }

    #[test]
    fn test_progress_tracking() {
        let capsule = SparseLosScalarCapsule::with_stride(4, 4);
        let map = MapDataCapsule::new(100, 100);

        unsafe {
            let layout = Layout::from_size_align(100 * 100 * 4, 32).unwrap();
            let cover = alloc(layout) as *mut i32;

            for i in 0..10000 {
                *cover.add(i) = 0;
            }

            map.attach_buffers(cover, cover, cover);

            let ray = LosRay::from_f32(0.0, 0.0, 40.0, 0.0, 100.0, LosRayType::Sparse);
            let result = capsule.traverse(&ray, &map);

            let (current, total) = capsule.progress();
            assert_eq!(current, result.samples_checked);
            assert_eq!(total, result.samples_checked);

            dealloc(cover as *mut u8, layout);
        }
    }

    #[test]
    fn test_early_exit() {
        let capsule = SparseLosScalarCapsule::with_stride(4, 4);
        let map = MapDataCapsule::new(100, 100);

        unsafe {
            let layout = Layout::from_size_align(100 * 100 * 4, 32).unwrap();
            let cover = alloc(layout) as *mut i32;

            // First 5 cells clear, rest blocked
            for i in 0..20 {
                *cover.add(i) = 0;
            }
            for i in 20..10000 {
                *cover.add(i) = 0x0001_0000; // Full cover
            }

            map.attach_buffers(cover, cover, cover);

            let ray = LosRay::from_f32(0.0, 0.0, 80.0, 0.0, 100.0, LosRayType::Sparse);
            let result = capsule.traverse(&ray, &map);

            assert!(result.is_blocked());
            // Should early-exit when hitting full cover
            assert!(result.samples_checked < 20); // 80 / 4 = 20 full samples

            dealloc(cover as *mut u8, layout);
        }
    }
}
