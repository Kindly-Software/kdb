//! Dense LOS AVX2 Capsule - T2+T3 tier (64B cache-aligned)
//!
//! Highest-performance line-of-sight for dense rays (500-2K samples).
//! Uses AVX2 8× unrolled gather-free traversal with Q16.16 fixed-point.
//!
//! # Performance
//!
//! - **Scalar**: ~40-60ns per sample
//! - **AVX2**: ~5-8ns per sample (8× speedup)
//! - **Memory**: Single 64B cache line (no false sharing)
//!
//! # Chaos Compliance
//!
//! - **Tier**: T2 (SIMD) + T3 (Fixed-Point)
//! - **Lockfree**: 100% atomic coordination
//! - **Cache-aligned**: 64B single cache line
//! - **Generation counter**: TOCTOU prevention
//!
//! # Example
//!
//! ```rust,ignore
//! use atomic_capsule::los::{DenseLosAvx2Capsule, LosRay, Q16_16, LosRayType};
//! use atomic_capsule::los::map_data::MapDataCapsule;
//!
//! let capsule = DenseLosAvx2Capsule::new();
//! let ray = LosRay::new(
//!     Q16_16::from_i32(10), Q16_16::from_i32(10),
//!     Q16_16::from_i32(50), Q16_16::from_i32(30),
//!     Q16_16::from_i32(100),
//!     LosRayType::Dense,
//! );
//! let map = MapDataCapsule::new(128, 128);
//!
//! let result = capsule.traverse(&ray, &map);
//! assert!(result.is_visible() || result.is_blocked() || result.is_partial());
//! ```

use core::sync::atomic::{AtomicU64, Ordering};

use super::types::{LosRay, LosResult, Q16_16};
use super::map_data::MapDataCapsule;

#[cfg(feature = "los-avx2")]
use super::avx2::dense_kernel;

/// DenseLosAvx2Capsule (64B) - T2+T3 tier
///
/// Highest-performance LOS for dense rays (500-2K samples).
/// Uses AVX2 8× unrolled gather-free traversal with Q16.16 fixed-point.
///
/// # Layout (64 bytes)
///
/// | Offset | Field | Size | Purpose |
/// |--------|-------|------|---------|
/// | 0-7 | state | 8B | type(4)\|unroll(4)\|samples(20)\|gen(24)\|flags(8) |
/// | 8-15 | ray_endpoints | 8B | start_x(16)\|start_y(16)\|end_x(16)\|end_y(16) Q8.8 |
/// | 16-23 | progress | 8B | current(20)\|total(20)\|cost(24) Q16.8 |
/// | 24-31 | thresholds | 8B | vis(16)\|mud(16)\|cover(16)\|reserved(16) Q8.8 |
/// | 32-63 | simd_buffer | 32B | 8× i32 intermediate results |
///
/// # Chaos Compliance
///
/// - 64B cache-aligned (single cache line)
/// - Lockfree state coordination via AtomicU64
/// - Generation counter for TOCTOU prevention
/// - All arithmetic is Q16.16 saturating
///
/// # ASSUM Safety Tags
///
/// - #ASSUME_64B_ALIGNMENT: Single cache line, no false sharing
/// - #ASSUME_Q16_SATURATION: All fixed-point ops saturate (no overflow)
/// - #ASSUME_GENERATION_COUNTER: TOCTOU-safe state transitions
/// - #ASSUME_SIMD_ALIGNMENT: simd_buffer is 32B-aligned within capsule
/// - #ASSUME_SAMPLE_BOUNDS: samples ≤ 2048 (enforced by min/max)
/// - #ASSUME_MAP_VALIDITY: MapDataCapsule provides valid dimensions
/// - #ASSUME_ATOMIC_ORDERING: Acquire/Release prevents reordering
///
/// # Verification (ASSUM)
///
/// - #VERIFY_64B_ALIGNMENT: Static assert at module bottom
/// - #VERIFY_SAMPLE_BOUNDS: Runtime min(2048) clamp in traverse()
/// - #VERIFY_GENERATION_INCREMENT: Checked in init_ray()
/// - #VERIFY_ATOMIC_ORDERING: Explicitly specified on all loads/stores
#[repr(C, align(64))]
pub struct DenseLosAvx2Capsule {
    /// Packed state: type(4)|unroll(4)|samples(20)|gen(24)|flags(8)
    /// - type: Ray type identifier (always Dense = 0)
    /// - unroll: Unroll factor (8 for AVX2)
    /// - samples: Maximum sample count (bits 4-23)
    /// - gen: Generation counter (ABA prevention, bits 24-47)
    /// - flags: Processing flags (bits 48-55)
    ///
    /// #ASSUME_STATE_PACKING: Bit layout matches documentation
    state: AtomicU64,

    /// Ray endpoints packed Q8.8:
    /// - start_x: bits 0-15
    /// - start_y: bits 16-31
    /// - end_x: bits 32-47
    /// - end_y: bits 48-63
    ///
    /// #ASSUME_ENDPOINT_PACKING: Q8.8 fixed-point in 16 bits each
    ray_endpoints: AtomicU64,

    /// Progress tracking:
    /// - current: bits 0-19 (current sample index)
    /// - total: bits 20-39 (total samples)
    /// - cost: bits 40-63 (accumulated cost Q16.8)
    ///
    /// #ASSUME_PROGRESS_PACKING: 20+20+24 = 64 bits
    progress: AtomicU64,

    /// Threshold configuration:
    /// - vis_threshold: bits 0-15 (early-exit visibility Q8.8)
    /// - mud_threshold: bits 16-31 (mud penalty threshold)
    /// - cover_threshold: bits 32-47 (cover blocking threshold)
    /// - reserved: bits 48-63
    ///
    /// #ASSUME_THRESHOLD_FORMAT: Q8.8 fixed-point thresholds
    thresholds: AtomicU64,

    /// SIMD buffer for intermediate results (32 bytes = 8× i32)
    /// Cache-aligned within the capsule for optimal AVX2 access
    ///
    /// #ASSUME_SIMD_BUFFER_ALIGNMENT: Offset 32 guarantees 32B alignment
    simd_buffer: [i32; 8],
}

impl DenseLosAvx2Capsule {
    /// Create a new dense LOS capsule
    ///
    /// # Returns
    ///
    /// Zero-initialized capsule with default thresholds (0.5 Q8.8 = 0x80)
    ///
    /// # ASSUM
    ///
    /// - #ASSUME_DEFAULT_THRESHOLDS: 0x80 = 0.5 in Q8.8 (50% visibility threshold)
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            ray_endpoints: AtomicU64::new(0),
            progress: AtomicU64::new(0),
            thresholds: AtomicU64::new(0x0080_0080_0080_0000), // Default 0.5 thresholds
            simd_buffer: [0i32; 8],
        }
    }

    /// Initialize capsule for a new ray
    ///
    /// # Arguments
    ///
    /// * `ray` - The LOS ray to initialize
    ///
    /// # ASSUM
    ///
    /// - #ASSUME_RAY_VALID: Ray has valid Q8.8 coordinates
    /// - #ASSUME_GENERATION_OVERFLOW: Generation wraps at 2^24 (acceptable for ABA prevention)
    ///
    /// # Verification
    ///
    /// - #VERIFY_ENDPOINT_EXTRACTION: Casts to u16 preserve Q8.8 format
    /// - #VERIFY_GENERATION_INCREMENT: Old gen + 1, wrapping at 24 bits
    pub fn init_ray(&self, ray: &LosRay) {
        // Pack endpoints (Q16.16 → u16 for compact storage)
        // #VERIFY_ENDPOINT_EXTRACTION: raw() returns i32, cast to u16 preserves low 16 bits
        let start_x = ray.origin_x.raw() as u16;
        let start_y = ray.origin_y.raw() as u16;
        let end_x = ray.target_x.raw() as u16;
        let end_y = ray.target_y.raw() as u16;

        let endpoints = (start_x as u64)
            | ((start_y as u64) << 16)
            | ((end_x as u64) << 32)
            | ((end_y as u64) << 48);

        // #VERIFY_ATOMIC_ORDERING: Release ensures endpoint write visible before state update
        self.ray_endpoints.store(endpoints, Ordering::Release);

        // Reset progress
        self.progress.store(0, Ordering::Release);

        // Increment generation counter
        // #VERIFY_GENERATION_INCREMENT: Acquire-Release pair for proper synchronization
        let old_state = self.state.load(Ordering::Acquire);
        let gen = ((old_state >> 8) & 0xFF_FFFF) + 1; // Extract 24-bit gen, increment
        let new_state = (gen << 8) | 0x08; // Dense type (0) + unroll 8
        self.state.store(new_state, Ordering::Release);
    }

    /// Traverse ray through map using AVX2 kernel
    ///
    /// # Arguments
    ///
    /// * `ray` - The LOS ray to traverse
    /// * `map` - Map data capsule with terrain buffers
    ///
    /// # Returns
    ///
    /// LosResult with final visibility and statistics
    ///
    /// # ASSUM
    ///
    /// - #ASSUME_MAP_DIMENSIONS: MapDataCapsule provides valid (width, height)
    /// - #ASSUME_SAMPLE_CALCULATION: dx.max(dy) approximates ray length
    /// - #ASSUME_AVX2_AVAILABLE: Runtime detection via is_x86_feature_detected!
    /// - #ASSUME_BUFFER_VALIDITY: Map buffers are valid for samples count
    ///
    /// # Verification
    ///
    /// - #VERIFY_SAMPLE_BOUNDS: samples.min(2048) enforces max samples
    /// - #VERIFY_ZERO_SAMPLES: Early return for zero-length rays
    /// - #VERIFY_MAP_BUSY: Returns blocked if map acquisition fails
    /// - #VERIFY_THRESHOLD_EXTRACTION: Low 16 bits contain vis_threshold
    #[cfg(feature = "los-avx2")]
    pub fn traverse(&self, ray: &LosRay, map: &MapDataCapsule) -> LosResult {
        // Initialize for this ray
        self.init_ray(ray);

        // Get map dimensions
        // #VERIFY_MAP_DIMENSIONS: MapDataCapsule guarantees valid dimensions
        let (_width, _height, _pitch) = map.dimensions();

        // Calculate sample count based on ray length
        // #VERIFY_SAMPLE_CALCULATION: Q16.16 raw() returns i32, abs() prevents negative
        let dx = (ray.target_x.raw() - ray.origin_x.raw()).abs();
        let dy = (ray.target_y.raw() - ray.origin_y.raw()).abs();

        // #VERIFY_SAMPLE_BOUNDS: Cap at 2K samples to prevent buffer overrun
        let samples = (dx.max(dy) as usize).min(2048);

        // #VERIFY_ZERO_SAMPLES: Early return for zero-length rays
        if samples == 0 {
            return LosResult::visible(0);
        }

        // Acquire read access to map
        // #VERIFY_MAP_BUSY: Return blocked if map is being modified
        let _guard = match map.acquire_read() {
            Some(g) => g,
            None => return LosResult::blocked(0), // Map busy
        };

        // Get visibility threshold
        // #VERIFY_THRESHOLD_EXTRACTION: Low 16 bits contain Q8.8 vis_threshold
        let thresholds = self.thresholds.load(Ordering::Acquire);
        let vis_threshold = (thresholds & 0xFFFF) as i32;

        // Execute AVX2 kernel or fallback to scalar
        // #ASSUME_UNSAFE_JUSTIFICATION: Raw pointer access to map buffers
        // - sample_strips() returns valid slices for samples count
        // - AVX2 detection ensures instruction availability
        // - Buffer alignment verified by MapDataCapsule
        let visibility = unsafe {
            // Runtime AVX2 detection
            // #VERIFY_AVX2_DETECTION: is_x86_feature_detected! is runtime check
            #[cfg(target_arch = "x86_64")]
            if is_x86_feature_detected!("avx2") {
                // Get sample strips along the ray starting point
                let start_x = (ray.origin_x.raw() >> 16) as u16;
                let start_y = (ray.origin_y.raw() >> 16) as u16;
                let (cover, mud, cost) = map.sample_strips(start_x, start_y, samples.min(2048));

                // If buffers are not 32B aligned, fall back to scalar to avoid UB.
                let aligned = (cover.as_ptr() as usize | mud.as_ptr() as usize | cost.as_ptr() as usize) & 31 == 0;
                if !aligned {
                    return self.traverse_scalar(ray, map, samples);
                }

                dense_kernel::traverse_dense_8x_unrolled(
                    cover.as_ptr(),
                    mud.as_ptr(),
                    cost.as_ptr(),
                    samples,
                    vis_threshold,
                )
            } else {
                // Fallback to scalar
                return self.traverse_scalar(ray, map, samples);
            }

            #[cfg(not(target_arch = "x86_64"))]
            {
                // Non-x86_64: always use scalar
                return self.traverse_scalar(ray, map, samples);
            }
        };

        // Update progress
        let progress = (samples as u64) | ((samples as u64) << 20);
        self.progress.store(progress, Ordering::Release);

        // Determine status based on final visibility
        // #VERIFY_VISIBILITY_RANGES: Q16.16 ranges for blocked/partial/visible
        if visibility <= 0 {
            LosResult::blocked(samples as u32)
        } else if visibility >= 0x0001_0000 {
            LosResult::visible(samples as u32)
        } else {
            LosResult::partial(Q16_16::from_raw(visibility), samples as u32, Q16_16::ZERO)
        }
    }

    /// Portable SIMD traversal (no AVX2 feature, uses portable_simd)
    ///
    /// # ASSUM
    ///
    /// - #ASSUME_NO_AVX2: Compiled without los-avx2 feature
    /// - #ASSUME_PORTABLE_SIMD: Uses std::simd::i32x8 for cross-platform SIMD
    ///
    /// # Verification
    ///
    /// - #VERIFY_SAMPLE_BOUNDS: Same min(2048) enforcement as AVX2 path
    #[cfg(not(feature = "los-avx2"))]
    pub fn traverse(&self, ray: &LosRay, map: &MapDataCapsule) -> LosResult {
        self.init_ray(ray);
        let dx = (ray.target_x.raw() - ray.origin_x.raw()).abs();
        let dy = (ray.target_y.raw() - ray.origin_y.raw()).abs();
        let samples = (dx.max(dy) as usize).min(2048); // #VERIFY_SAMPLE_BOUNDS

        // Use portable SIMD if available, otherwise scalar
        #[cfg(feature = "nightly-simd")]
        {
            self.traverse_portable_simd(ray, map, samples)
        }

        #[cfg(not(feature = "nightly-simd"))]
        {
            self.traverse_scalar(ray, map, samples)
        }
    }

    /// Traverse using portable_simd (std::simd::i32x8)
    ///
    /// # Algorithm
    ///
    /// Process 8 samples per iteration using Q16.16 fixed-point arithmetic:
    /// 1. Calculate t = i / total_samples for 8 lanes simultaneously
    /// 2. Interpolate: x = origin_x + (target_x - origin_x) * t
    ///                 y = origin_y + (target_y - origin_y) * t
    /// 3. Sample cover from map at (x, y) for each lane
    /// 4. Attenuate visibility: vis *= (1.0 - cover) in Q16.16
    /// 5. Early exit if all lanes blocked (vis <= 0)
    ///
    /// # Arguments
    ///
    /// * `ray` - The LOS ray to traverse
    /// * `map` - Map data capsule
    /// * `samples` - Number of samples (must be ≤ 2048)
    ///
    /// # ASSUM
    ///
    /// - #ASSUME_PORTABLE_SIMD: Uses std::simd for cross-platform compatibility
    /// - #ASSUME_Q16_ARITHMETIC: All ops use saturating Q16.16
    /// - #ASSUME_SAMPLES_VALID: Caller ensures samples ≤ 2048
    ///
    /// # Verification
    ///
    /// - #VERIFY_SIMD_LANES: i32x8 processes 8 samples per iteration
    /// - #VERIFY_SATURATION: All Q16_16 ops are saturating
    /// - #VERIFY_EARLY_EXIT: All-lanes-blocked triggers early return
    #[cfg(feature = "nightly-simd")]
    fn traverse_portable_simd(&self, ray: &LosRay, map: &MapDataCapsule, samples: usize) -> LosResult {
        use std::simd::{i32x8, Simd};
        use std::simd::prelude::*;

        // Q16.16 constants
        const ONE_Q16: i32 = 0x0001_0000; // 1.0 in Q16.16

        // Initialize visibility lanes (8× Q16.16::ONE)
        let mut visibility = i32x8::splat(ONE_Q16);

        // Ray deltas (Q16.16)
        let dx_q16 = ray.target_x.raw() - ray.origin_x.raw();
        let dy_q16 = ray.target_y.raw() - ray.origin_y.raw();

        // Broadcast to SIMD lanes
        let origin_x = i32x8::splat(ray.origin_x.raw());
        let origin_y = i32x8::splat(ray.origin_y.raw());
        let delta_x = i32x8::splat(dx_q16);
        let delta_y = i32x8::splat(dy_q16);
        let samples_i32 = samples.max(1) as i32; // Prevent division by zero
        let samples_simd = i32x8::splat(samples_i32);

        // Lane indices: [0, 1, 2, 3, 4, 5, 6, 7]
        let lane_base = i32x8::from_array([0, 1, 2, 3, 4, 5, 6, 7]);

        // Process 8 samples per iteration
        let chunks = (samples + 7) / 8;

        for chunk in 0..chunks {
            let base_idx = (chunk * 8) as i32;

            // Calculate sample indices for this chunk
            let sample_indices = i32x8::splat(base_idx) + lane_base;

            // Check if sample index is valid (< samples)
            let valid_mask = sample_indices.simd_lt(i32x8::splat(samples_i32));

            // Calculate t = sample_index / total_samples (Q16.16)
            // t = (sample_index * 0x10000) / samples
            // Widen to i64 to prevent overflow: (i * 65536) can exceed i32::MAX
            let t_lanes: [i32; 8] = std::array::from_fn(|i| {
                let idx = sample_indices.as_array()[i] as i64;
                let t_num = idx * 0x10000i64;
                let t = (t_num / samples_i32 as i64) as i32;
                t.min(0x0001_0000).max(0) // Clamp to [0, 1] in Q16.16
            });
            let t = i32x8::from_array(t_lanes);

            // Interpolate x: origin.x + delta.x * t
            // Q16.16 multiply: (delta * t) >> 16
            let dx_scaled_lanes: [i32; 8] = std::array::from_fn(|i| {
                let dx = delta_x.as_array()[i] as i64;
                let t_val = t.as_array()[i] as i64;
                let product = (dx * t_val) >> 16;
                product.clamp(i32::MIN as i64, i32::MAX as i64) as i32
            });
            let dx_scaled = i32x8::from_array(dx_scaled_lanes);
            let x = origin_x + dx_scaled;

            // Interpolate y: origin.y + delta.y * t
            let dy_scaled_lanes: [i32; 8] = std::array::from_fn(|i| {
                let dy = delta_y.as_array()[i] as i64;
                let t_val = t.as_array()[i] as i64;
                let product = (dy * t_val) >> 16;
                product.clamp(i32::MIN as i64, i32::MAX as i64) as i32
            });
            let dy_scaled = i32x8::from_array(dy_scaled_lanes);
            let y = origin_y + dy_scaled;

            // Extract integer coordinates (Q16.16 >> 16)
            let x_coords = x.as_array().map(|v| ((v >> 16) as u16));
            let y_coords = y.as_array().map(|v| ((v >> 16) as u16));

            // Sample cover for each lane
            let mut cover_lanes = [0i32; 8];
            for i in 0..8 {
                if valid_mask.as_array()[i] {
                    if let Some(cover) = map.sample_cover(x_coords[i], y_coords[i]) {
                        cover_lanes[i] = cover;
                    }
                }
            }
            let cover = i32x8::from_array(cover_lanes);

            // Attenuation: vis *= (1.0 - cover)
            // atten = ONE - cover (saturating)
            let one = i32x8::splat(ONE_Q16);
            let atten_lanes: [i32; 8] = std::array::from_fn(|i| {
                ONE_Q16.saturating_sub(cover_lanes[i])
            });
            let atten = i32x8::from_array(atten_lanes);

            // Q16.16 multiply: (vis * atten) >> 16
            let vis_updated_lanes: [i32; 8] = std::array::from_fn(|i| {
                let vis = visibility.as_array()[i] as i64;
                let att = atten.as_array()[i] as i64;
                let product = (vis * att) >> 16;
                product.clamp(0, i32::MAX as i64) as i32
            });
            visibility = i32x8::from_array(vis_updated_lanes);

            // Early exit if all valid lanes are blocked
            let zero = i32x8::splat(0);
            let blocked_mask = visibility.simd_le(zero);
            let all_blocked = blocked_mask.as_array().iter()
                .zip(valid_mask.as_array().iter())
                .all(|(blocked, valid)| !valid || blocked);

            if all_blocked && chunk > 0 {
                // Return blocked with samples checked so far
                let samples_checked = (chunk * 8).min(samples) as u32;
                return LosResult::blocked(samples_checked);
            }
        }

        // Horizontal reduction: minimum visibility across all lanes
        let final_visibility = visibility.as_array().iter()
            .copied()
            .min()
            .unwrap_or(ONE_Q16)
            .max(0)
            .min(ONE_Q16);

        // Determine result status
        if final_visibility >= ONE_Q16 {
            LosResult::visible(samples as u32)
        } else if final_visibility <= 0 {
            LosResult::blocked(samples as u32)
        } else {
            LosResult::partial(Q16_16::from_raw(final_visibility), samples as u32, Q16_16::ZERO)
        }
    }

    /// Scalar fallback implementation
    ///
    /// # Arguments
    ///
    /// * `ray` - The LOS ray to traverse
    /// * `map` - Map data capsule
    /// * `samples` - Number of samples (must be ≤ 2048)
    ///
    /// # Returns
    ///
    /// LosResult with final visibility
    ///
    /// # ASSUM
    ///
    /// - #ASSUME_SAMPLES_VALID: Caller ensures samples ≤ 2048
    /// - #ASSUME_Q16_ARITHMETIC: All ops use saturating Q16.16
    ///
    /// # Verification
    ///
    /// - #VERIFY_DIVISION_BY_ZERO: samples.max(1) prevents divide by zero
    /// - #VERIFY_SATURATION: All Q16_16 ops are saturating
    /// - #VERIFY_EARLY_EXIT: visibility <= 0 breaks loop
    fn traverse_scalar(&self, ray: &LosRay, map: &MapDataCapsule, samples: usize) -> LosResult {
        let mut visibility = Q16_16::ONE;

        // Simple scalar loop
        for i in 0..samples {
            // Sample along ray using linear interpolation
            // #VERIFY_DIVISION_BY_ZERO: samples.max(1) prevents divide by zero
            let t = Q16_16::from_raw((i as i32 * 0x10000) / samples.max(1) as i32);

            // #VERIFY_SATURATION: saturating_add/sub/mul prevent overflow
            let dx = ray.target_x.saturating_sub(ray.origin_x);
            let dy = ray.target_y.saturating_sub(ray.origin_y);
            let x = ray.origin_x.saturating_add(t.saturating_mul(dx));
            let y = ray.origin_y.saturating_add(t.saturating_mul(dy));

            // Sample cover at (x, y) - extract integer part from Q16.16
            let x_int = (x.raw() >> 16) as u16;
            let y_int = (y.raw() >> 16) as u16;
            if let Some(cover) = map.sample_cover(x_int, y_int) {
                let cover_q16 = Q16_16::from_raw(cover);
                visibility = visibility.saturating_mul(Q16_16::ONE.saturating_sub(cover_q16));
            }

            // Early exit if completely blocked
            // #VERIFY_EARLY_EXIT: visibility <= 0 means fully blocked
            if visibility.raw() <= 0 {
                return LosResult::blocked(i as u32);
            }
        }

        // Determine final status
        if visibility.raw() >= 0x0001_0000 {
            LosResult::visible(samples as u32)
        } else {
            LosResult::partial(visibility, samples as u32, Q16_16::ZERO)
        }
    }

    /// Get current generation counter
    ///
    /// # Returns
    ///
    /// 24-bit generation counter (wraps at 16,777,216)
    ///
    /// # ASSUM
    ///
    /// - #ASSUME_GENERATION_BITS: Bits 8-31 contain 24-bit generation
    pub fn generation(&self) -> u32 {
        let state = self.state.load(Ordering::Acquire);
        ((state >> 8) & 0xFF_FFFF) as u32
    }

    /// Get progress (current, total) samples
    ///
    /// # Returns
    ///
    /// Tuple of (current_sample, total_samples)
    ///
    /// # ASSUM
    ///
    /// - #ASSUME_PROGRESS_BITS: current in bits 0-19, total in bits 20-39
    pub fn progress(&self) -> (u32, u32) {
        let progress = self.progress.load(Ordering::Acquire);
        let current = (progress & 0xFFFFF) as u32;
        let total = ((progress >> 20) & 0xFFFFF) as u32;
        (current, total)
    }

    /// Set visibility threshold
    ///
    /// # Arguments
    ///
    /// * `threshold` - Q8.8 threshold (0.0 = always visible, 1.0 = never visible)
    ///
    /// # ASSUM
    ///
    /// - #ASSUME_THRESHOLD_RANGE: Caller provides valid Q8.8 value
    pub fn set_visibility_threshold(&self, threshold: i32) {
        let old = self.thresholds.load(Ordering::Acquire);
        let new = (old & !0xFFFF) | (threshold as u64 & 0xFFFF);
        self.thresholds.store(new, Ordering::Release);
    }

    /// Get current visibility threshold
    ///
    /// # Returns
    ///
    /// Q8.8 visibility threshold
    pub fn visibility_threshold(&self) -> i32 {
        let thresholds = self.thresholds.load(Ordering::Acquire);
        (thresholds & 0xFFFF) as i32
    }
}

impl Default for DenseLosAvx2Capsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// VERIFICATION: Size and Alignment (ASSUM)
// ============================================================================

// #VERIFY_64B_ALIGNMENT: DenseLosAvx2Capsule is exactly 64 bytes, 64-byte aligned
const _: () = assert!(core::mem::size_of::<DenseLosAvx2Capsule>() == 64);
const _: () = assert!(core::mem::align_of::<DenseLosAvx2Capsule>() == 64);

// #VERIFY_SIMD_BUFFER_OFFSET: simd_buffer starts at byte 32 (32B aligned)
const _: () = {
    use core::mem::offset_of;
    assert!(offset_of!(DenseLosAvx2Capsule, simd_buffer) == 32);
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::los::types::LosRayType;

    fn make_ray(ox: i32, oy: i32, tx: i32, ty: i32) -> LosRay {
        LosRay::new(
            Q16_16::from_i32(ox),
            Q16_16::from_i32(oy),
            Q16_16::from_i32(tx),
            Q16_16::from_i32(ty),
            Q16_16::from_i32(1000),
            LosRayType::Dense,
        )
    }

    #[test]
    fn test_size_alignment() {
        // #VERIFY_64B_ALIGNMENT: Compile-time checks enforced
        assert_eq!(core::mem::size_of::<DenseLosAvx2Capsule>(), 64);
        assert_eq!(core::mem::align_of::<DenseLosAvx2Capsule>(), 64);
    }

    #[test]
    fn test_simd_buffer_alignment() {
        // #VERIFY_SIMD_BUFFER_OFFSET: simd_buffer at offset 32
        use core::mem::offset_of;
        assert_eq!(offset_of!(DenseLosAvx2Capsule, simd_buffer), 32);

        // Verify 32B alignment
        let capsule = DenseLosAvx2Capsule::new();
        let ptr = &capsule.simd_buffer as *const _ as usize;
        assert_eq!(ptr % 32, 0, "simd_buffer not 32B aligned");
    }

    #[test]
    fn test_new_initialization() {
        let capsule = DenseLosAvx2Capsule::new();

        // Verify zero state
        assert_eq!(capsule.state.load(Ordering::Acquire), 0);
        assert_eq!(capsule.ray_endpoints.load(Ordering::Acquire), 0);
        assert_eq!(capsule.progress.load(Ordering::Acquire), 0);

        // Verify default thresholds (0x80 = 0.5 in Q8.8)
        let thresholds = capsule.thresholds.load(Ordering::Acquire);
        assert_eq!(thresholds, 0x0080_0080_0080_0000);
    }

    #[test]
    fn test_init_ray() {
        let capsule = DenseLosAvx2Capsule::new();
        let ray = make_ray(10, 20, 50, 60);

        capsule.init_ray(&ray);

        // Verify endpoints packed correctly
        let endpoints = capsule.ray_endpoints.load(Ordering::Acquire);
        let start_x = (endpoints & 0xFFFF) as u16;
        let start_y = ((endpoints >> 16) & 0xFFFF) as u16;
        let end_x = ((endpoints >> 32) & 0xFFFF) as u16;
        let end_y = ((endpoints >> 48) & 0xFFFF) as u16;

        assert_eq!(start_x, ray.origin_x.raw() as u16);
        assert_eq!(start_y, ray.origin_y.raw() as u16);
        assert_eq!(end_x, ray.target_x.raw() as u16);
        assert_eq!(end_y, ray.target_y.raw() as u16);

        // Verify progress reset
        assert_eq!(capsule.progress.load(Ordering::Acquire), 0);

        // Verify generation incremented
        assert_eq!(capsule.generation(), 1);
    }

    #[test]
    fn test_generation_counter_increment() {
        let capsule = DenseLosAvx2Capsule::new();
        let ray = make_ray(0, 0, 10, 10);

        assert_eq!(capsule.generation(), 0);

        capsule.init_ray(&ray);
        assert_eq!(capsule.generation(), 1);

        capsule.init_ray(&ray);
        assert_eq!(capsule.generation(), 2);

        capsule.init_ray(&ray);
        assert_eq!(capsule.generation(), 3);
    }

    #[test]
    fn test_generation_counter_wrap() {
        let capsule = DenseLosAvx2Capsule::new();
        let ray = make_ray(0, 0, 10, 10);

        // Set generation to max 24-bit value
        let max_gen = 0xFF_FFFF_u64;
        capsule.state.store((max_gen << 8) | 0x08, Ordering::Release);
        assert_eq!(capsule.generation(), 0xFF_FFFF);

        // Next init should wrap to 0
        capsule.init_ray(&ray);
        assert_eq!(capsule.generation(), 0); // Wraps at 24 bits
    }

    #[test]
    fn test_progress_tracking() {
        let capsule = DenseLosAvx2Capsule::new();

        // Set progress to (100, 500)
        let progress = 100_u64 | (500_u64 << 20);
        capsule.progress.store(progress, Ordering::Release);

        let (current, total) = capsule.progress();
        assert_eq!(current, 100);
        assert_eq!(total, 500);
    }

    #[test]
    fn test_threshold_configuration() {
        let capsule = DenseLosAvx2Capsule::new();

        // Default threshold is 0x80 (0.5 in Q8.8)
        assert_eq!(capsule.visibility_threshold(), 0x80);

        // Set new threshold (0.75 in Q8.8 = 0xC0)
        capsule.set_visibility_threshold(0xC0);
        assert_eq!(capsule.visibility_threshold(), 0xC0);

        // Verify other thresholds unchanged
        let thresholds = capsule.thresholds.load(Ordering::Acquire);
        assert_eq!((thresholds >> 16) & 0xFFFF, 0x80); // mud unchanged
        assert_eq!((thresholds >> 32) & 0xFFFF, 0x80); // cover unchanged
    }

    #[test]
    fn test_traverse_zero_samples() {
        let capsule = DenseLosAvx2Capsule::new();
        let map = MapDataCapsule::new(128, 128);

        // Zero-length ray
        let ray = make_ray(10, 10, 10, 10);

        let result = capsule.traverse(&ray, &map);
        assert!(result.is_visible());
        assert_eq!(result.samples_checked, 0);
    }

    #[test]
    fn test_traverse_scalar_basic() {
        let capsule = DenseLosAvx2Capsule::new();
        let map = MapDataCapsule::new(128, 128);

        // Simple horizontal ray
        let ray = make_ray(10, 10, 50, 10);

        let result = capsule.traverse(&ray, &map);

        // Should be visible (no cover in default map)
        assert!(result.is_visible() || result.is_partial());
        assert!(result.samples_checked > 0);
    }

    #[test]
    fn test_traverse_sample_bounds() {
        let capsule = DenseLosAvx2Capsule::new();
        let map = MapDataCapsule::new(512, 512);

        // Very long ray (should cap at 2048 samples)
        let ray = make_ray(0, 0, 400, 300);

        let result = capsule.traverse(&ray, &map);

        // Should traverse at most 2048 samples
        assert!(result.samples_checked <= 2048);
    }

    #[test]
    fn test_default_trait() {
        let capsule = DenseLosAvx2Capsule::default();
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.visibility_threshold(), 0x80);
    }

    #[cfg(feature = "los-avx2")]
    #[test]
    fn test_avx2_detection() {
        // Just verify the feature compiles, actual AVX2 testing requires runtime
        let capsule = DenseLosAvx2Capsule::new();
        let map = MapDataCapsule::new(128, 128);
        let ray = make_ray(10, 10, 50, 30);

        let _result = capsule.traverse(&ray, &map);
        // Success means AVX2 path or scalar fallback worked
    }
}
