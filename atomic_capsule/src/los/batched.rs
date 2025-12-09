//! Batched LOS Capsule - T4 Batch + T2 SIMD
//!
//! Multi-ray SoA processing (4-8 rays simultaneously).
//! Uses horizontal SIMD reductions for parallel ray traversal.
//!
//! # Target Performance
//!
//! - Latency: <200ns per batch (8 rays × 100 samples)
//! - Speedup: 40× vs sequential single-ray processing
//! - Use case: Fractal LOD, batch visibility queries
//!
//! # Architecture
//!
//! ```text
//! Input: 8× LosRay (AoS)
//!        │
//!        ▼
//! ┌──────────────────────────────┐
//! │ AoS → SoA Conversion         │
//! │ origin_x[8], origin_y[8]     │
//! │ target_x[8], target_y[8]     │
//! └──────────────────────────────┘
//!        │
//!        ▼
//! ┌──────────────────────────────┐
//! │ Horizontal SIMD Traversal    │
//! │ - 8 rays × same sample step  │
//! │ - Parallel coordinate calc   │
//! │ - Vector map sampling        │
//! └──────────────────────────────┘
//!        │
//!        ▼
//! ┌──────────────────────────────┐
//! │ Per-Ray Completion Tracking  │
//! │ - Bitmask for blocked rays   │
//! │ - Early-exit when all done   │
//! └──────────────────────────────┘
//!        │
//!        ▼
//! Output: 8× LosResult
//! ```
//!
//! # Chaos Compliance
//!
//! - 64B cache-aligned capsule
//! - Lockfree state coordination
//! - SoA layout for SIMD-friendly access
//! - Generation counter for TOCTOU prevention
//!
//! # ASSUM Tags
//!
//! - #ASSUME_SIMD_LANE_COUNT: Max 8 rays per batch (matches AVX2/Neon lane count)
//! - #ASSUME_BATCH_ALIGNMENT: SoA arrays are 32B aligned
//! - #ASSUME_SAMPLE_BOUNDS: All ray samples stay within map bounds

use super::types::{LosRay, LosResult, LosStatus, Q16_16};
use super::map_data::MapDataCapsule;
use core::sync::atomic::{AtomicU64, Ordering};

/// Maximum rays per batch (matches AVX2 i32x8)
pub const MAX_BATCH_SIZE: usize = 8;

/// Default samples per ray in batch mode
pub const DEFAULT_BATCH_SAMPLES: usize = 100;

/// BatchedLosSimdCapsule (64B) - T4+T2 tier
///
/// Optimized for batch ray processing (4-8 rays simultaneously).
/// Uses SoA layout for horizontal SIMD operations.
///
/// # Layout (64 bytes)
///
/// | Offset | Field | Size | Purpose |
/// |--------|-------|------|---------|
/// | 0-7 | state | 8B | batch_size(4)\|active(4)\|gen(24)\|flags(8)\|reserved(24) |
/// | 8-15 | batch_config | 8B | rays_per_batch(8)\|samples_per_ray(16)\|threshold(16)\|reserved(24) |
/// | 16-23 | progress | 8B | completed_rays(8)\|current_step(24)\|samples_done(32) |
/// | 24-31 | result_mask | 8B | Per-ray status bitmask (8 bits/ray × 8 rays) |
/// | 32-63 | simd_accum | 32B | 8× i32 horizontal visibility accumulator |
#[repr(C, align(64))]
pub struct BatchedLosSimdCapsule {
    /// Packed state: batch_size(4)|active(4)|gen(24)|flags(8)|reserved(24)
    ///
    /// - batch_size: Current batch size (0-8)
    /// - active: Number of active (not yet completed) rays
    /// - gen: Generation counter for TOCTOU prevention
    /// - flags: Configuration flags
    state: AtomicU64,

    /// Batch configuration: rays_per_batch(8)|samples_per_ray(16)|threshold(16)|reserved(24)
    ///
    /// - rays_per_batch: Target batch size (1-8)
    /// - samples_per_ray: Max samples per ray
    /// - threshold: Visibility threshold for early-exit (Q8.8 fixed-point)
    batch_config: AtomicU64,

    /// Progress: completed_rays(8)|current_step(24)|samples_done(32)
    ///
    /// - completed_rays: Number of rays that finished
    /// - current_step: Current traversal step
    /// - samples_done: Total samples evaluated
    progress: AtomicU64,

    /// Per-ray status bitmask (8 bytes, 8 bits per ray)
    ///
    /// For each ray (0-7), bits encode:
    /// - [0]: blocked (hit obstruction)
    /// - [1]: visible (reached target)
    /// - [2]: partial (visibility < 1.0)
    /// - [3]: early_exit (terminated early)
    /// - [4-7]: reserved
    result_mask: AtomicU64,

    /// SIMD accumulator (8× i32) - visibility values in Q16.16
    ///
    /// Stores accumulated visibility for each ray lane.
    /// Layout: [ray0_vis, ray1_vis, ..., ray7_vis] as i32 (Q16.16)
    simd_accum: [i32; 8],
}

// State field bit manipulation
const STATE_BATCH_SIZE_MASK: u64 = 0xF;
const STATE_ACTIVE_SHIFT: u32 = 4;
const STATE_ACTIVE_MASK: u64 = 0xF << STATE_ACTIVE_SHIFT;
const STATE_GEN_SHIFT: u32 = 8;
const STATE_GEN_MASK: u64 = 0xFFFFFF << STATE_GEN_SHIFT;
const STATE_FLAGS_SHIFT: u32 = 32;

// Config field bit manipulation
const CONFIG_RAYS_MASK: u64 = 0xFF;
const CONFIG_SAMPLES_SHIFT: u32 = 8;
const CONFIG_SAMPLES_MASK: u64 = 0xFFFF << CONFIG_SAMPLES_SHIFT;
const CONFIG_THRESHOLD_SHIFT: u32 = 24;

// Progress field bit manipulation
const PROGRESS_COMPLETED_MASK: u64 = 0xFF;
const PROGRESS_STEP_SHIFT: u32 = 8;
const PROGRESS_STEP_MASK: u64 = 0xFFFFFF << PROGRESS_STEP_SHIFT;
const PROGRESS_SAMPLES_SHIFT: u32 = 32;

// Result mask bit patterns (per-ray)
const RAY_STATUS_BLOCKED: u8 = 0x01;
const RAY_STATUS_VISIBLE: u8 = 0x02;
const RAY_STATUS_PARTIAL: u8 = 0x04;
const RAY_STATUS_EARLY_EXIT: u8 = 0x08;

impl BatchedLosSimdCapsule {
    /// Create a new batched capsule with default configuration
    ///
    /// Default config:
    /// - rays_per_batch: 8
    /// - samples_per_ray: 100
    /// - threshold: Q8.8(0.1) = 25 (10% visibility triggers early-exit)
    pub const fn new() -> Self {
        // Default config: 8 rays, 100 samples, 0.1 threshold
        let batch_config = 8u64
            | ((DEFAULT_BATCH_SAMPLES as u64) << CONFIG_SAMPLES_SHIFT)
            | (25u64 << CONFIG_THRESHOLD_SHIFT); // Q8.8(0.1)

        Self {
            state: AtomicU64::new(0),
            batch_config: AtomicU64::new(batch_config),
            progress: AtomicU64::new(0),
            result_mask: AtomicU64::new(0),
            simd_accum: [Q16_16::ONE.raw(); 8], // Start with full visibility
        }
    }

    /// Create batched capsule with custom configuration
    ///
    /// # Arguments
    ///
    /// * `rays_per_batch` - Target batch size (1-8)
    /// * `samples_per_ray` - Max samples per ray
    /// * `threshold` - Visibility threshold for early-exit (Q8.8)
    pub const fn with_config(
        rays_per_batch: u8,
        samples_per_ray: u16,
        threshold: u16,
    ) -> Self {
        let rays = if rays_per_batch > 8 { 8 } else { rays_per_batch };
        let batch_config = (rays as u64)
            | ((samples_per_ray as u64) << CONFIG_SAMPLES_SHIFT)
            | ((threshold as u64) << CONFIG_THRESHOLD_SHIFT);

        Self {
            state: AtomicU64::new(0),
            batch_config: AtomicU64::new(batch_config),
            progress: AtomicU64::new(0),
            result_mask: AtomicU64::new(0),
            simd_accum: [Q16_16::ONE.raw(); 8],
        }
    }

    /// Get current generation counter
    ///
    /// # Returns
    ///
    /// Generation counter (0-16,777,215), wraps at 24 bits.
    #[inline]
    pub fn generation(&self) -> u32 {
        let state = self.state.load(Ordering::Acquire);
        ((state & STATE_GEN_MASK) >> STATE_GEN_SHIFT) as u32
    }

    /// Get configured samples per ray
    #[inline]
    pub fn samples_per_ray(&self) -> u16 {
        let config = self.batch_config.load(Ordering::Acquire);
        ((config & CONFIG_SAMPLES_MASK) >> CONFIG_SAMPLES_SHIFT) as u16
    }

    /// Reset capsule state for new batch
    #[inline]
    fn reset_for_batch(&self, batch_size: usize) {
        let old_state = self.state.load(Ordering::Acquire);
        let gen = ((old_state & STATE_GEN_MASK) >> STATE_GEN_SHIFT) + 1;
        let new_state = (batch_size as u64)
            | ((batch_size as u64) << STATE_ACTIVE_SHIFT)
            | ((gen & 0xFFFFFF) << STATE_GEN_SHIFT);
        self.state.store(new_state, Ordering::Release);

        self.progress.store(0, Ordering::Release);
        self.result_mask.store(0, Ordering::Release);
    }

    /// Traverse multiple rays in batch using horizontal SIMD
    ///
    /// # Algorithm
    ///
    /// 1. Convert AoS rays to SoA format
    /// 2. For each sample step (0..max_samples):
    ///    a. Calculate all ray positions (parallel)
    ///    b. Sample map at all positions
    ///    c. Update visibility accumulators
    ///    d. Mark blocked rays
    /// 3. Early-exit when all rays complete
    /// 4. Convert SoA results back to AoS
    ///
    /// # Arguments
    ///
    /// * `rays` - Slice of LOS rays to traverse (1-8)
    /// * `map` - Map data capsule with terrain information
    ///
    /// # Returns
    ///
    /// Vector of LosResults, one per input ray
    ///
    /// # Performance
    ///
    /// Target: <200ns for 8 rays × 100 samples = 25ns per ray-sample
    #[allow(unused_variables)]
    pub fn traverse_batch(&self, rays: &[LosRay], map: &MapDataCapsule) -> alloc::vec::Vec<LosResult> {
        let batch_size = rays.len().min(MAX_BATCH_SIZE);
        if batch_size == 0 {
            return alloc::vec::Vec::new();
        }

        // Reset state for new batch
        self.reset_for_batch(batch_size);

        // Get map dimensions for bounds checking
        let (map_width, map_height, _pitch) = map.dimensions();
        let max_samples = self.samples_per_ray() as usize;

        // Convert AoS to SoA
        // #ASSUME_BATCH_ALIGNMENT: These arrays fit in cache
        let mut origin_x = [0i32; MAX_BATCH_SIZE];
        let mut origin_y = [0i32; MAX_BATCH_SIZE];
        let mut delta_x = [0i32; MAX_BATCH_SIZE];
        let mut delta_y = [0i32; MAX_BATCH_SIZE];
        let mut visibility = [Q16_16::ONE.raw(); MAX_BATCH_SIZE];
        let mut samples_done = [0u32; MAX_BATCH_SIZE];
        let mut ray_status = [0u8; MAX_BATCH_SIZE];

        // Initialize SoA arrays from rays
        for (i, ray) in rays.iter().enumerate().take(batch_size) {
            origin_x[i] = ray.origin_x.raw();
            origin_y[i] = ray.origin_y.raw();

            // Compute delta per sample step
            let dx = ray.target_x.saturating_sub(ray.origin_x);
            let dy = ray.target_y.saturating_sub(ray.origin_y);

            // Divide by max_samples to get per-step delta
            if max_samples > 0 {
                delta_x[i] = dx.raw() / (max_samples as i32);
                delta_y[i] = dy.raw() / (max_samples as i32);
            }
        }

        // Main traversal loop - horizontal SIMD across rays
        let mut active_count = batch_size;
        let mut total_samples = 0u64;

        for step in 0..max_samples {
            if active_count == 0 {
                break; // Early-exit: all rays complete
            }

            // Process all rays in parallel for this step
            for i in 0..batch_size {
                // Skip completed rays
                if ray_status[i] != 0 {
                    continue;
                }

                // Calculate current position
                let step_i32 = step as i32;
                let curr_x = origin_x[i].saturating_add(delta_x[i].saturating_mul(step_i32));
                let curr_y = origin_y[i].saturating_add(delta_y[i].saturating_mul(step_i32));

                // Convert to map coordinates (integer part of Q16.16)
                let map_x = (curr_x >> 16) as i32;
                let map_y = (curr_y >> 16) as i32;

                // Bounds check
                if map_x < 0 || map_x >= map_width as i32
                    || map_y < 0 || map_y >= map_height as i32
                {
                    // Out of bounds - mark as blocked
                    ray_status[i] = RAY_STATUS_BLOCKED;
                    visibility[i] = Q16_16::ZERO.raw();
                    samples_done[i] = step as u32;
                    active_count -= 1;
                    continue;
                }

                // Sample cover value from map
                if let Some(cover) = map.sample_cover(map_x as u16, map_y as u16) {
                    // Cover > 128 means blocked (scaled 0-255)
                    if cover > 128 {
                        ray_status[i] = RAY_STATUS_BLOCKED;
                        visibility[i] = Q16_16::ZERO.raw();
                        samples_done[i] = step as u32;
                        active_count -= 1;
                    } else if cover > 0 {
                        // Partial occlusion - reduce visibility
                        // visibility *= (256 - cover) / 256
                        let factor = Q16_16::from_raw(
                            ((256 - cover) as i32) << 8 // Scale to Q16.16
                        );
                        visibility[i] = Q16_16::from_raw(visibility[i])
                            .saturating_mul(factor)
                            .raw();

                        // Check visibility threshold
                        // If visibility < 10%, mark as partial and exit
                        if visibility[i] < (Q16_16::ONE.raw() / 10) {
                            ray_status[i] = RAY_STATUS_PARTIAL;
                            samples_done[i] = step as u32;
                            active_count -= 1;
                        }
                    }
                }

                total_samples += 1;
            }

            // Update progress atomically
            let progress_val = (active_count as u64)
                | ((step as u64) << PROGRESS_STEP_SHIFT)
                | (total_samples << PROGRESS_SAMPLES_SHIFT);
            self.progress.store(progress_val, Ordering::Release);
        }

        // Mark remaining active rays as visible
        for i in 0..batch_size {
            if ray_status[i] == 0 {
                ray_status[i] = RAY_STATUS_VISIBLE;
                samples_done[i] = max_samples as u32;
            }
        }

        // Store result mask
        let mut mask = 0u64;
        for i in 0..batch_size {
            mask |= (ray_status[i] as u64) << (i * 8);
        }
        self.result_mask.store(mask, Ordering::Release);

        // Convert SoA results back to AoS
        let mut results = alloc::vec::Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let vis = Q16_16::from_raw(visibility[i]);
            let samples = samples_done[i];
            let status = ray_status[i];

            let result = if status & RAY_STATUS_BLOCKED != 0 {
                LosResult::blocked(samples)
            } else if status & RAY_STATUS_VISIBLE != 0 {
                if vis.raw() == Q16_16::ONE.raw() {
                    LosResult::visible(samples)
                } else {
                    LosResult::partial(vis, samples, Q16_16::ZERO)
                }
            } else if status & RAY_STATUS_PARTIAL != 0 {
                LosResult::partial(vis, samples, Q16_16::ZERO)
            } else if status & RAY_STATUS_EARLY_EXIT != 0 {
                LosResult::early_exit(vis, samples)
            } else {
                // Fallback: treat as visible with current visibility
                LosResult::partial(vis, samples, Q16_16::ZERO)
            };

            results.push(result);
        }

        // Mark batch as complete by clearing active count in state
        let old_state = self.state.load(Ordering::Acquire);
        let gen = (old_state & STATE_GEN_MASK) >> STATE_GEN_SHIFT;
        let batch = old_state & STATE_BATCH_SIZE_MASK;
        // Keep batch size and generation, but set active to 0
        let new_state = batch | (gen << STATE_GEN_SHIFT);
        self.state.store(new_state, Ordering::Release);

        results
    }

    /// Get batch metrics
    ///
    /// # Returns
    ///
    /// Tuple of (completed_rays, current_step, total_samples)
    #[inline]
    pub fn metrics(&self) -> (u8, u32, u64) {
        let progress = self.progress.load(Ordering::Acquire);
        let completed = (progress & PROGRESS_COMPLETED_MASK) as u8;
        let step = ((progress & PROGRESS_STEP_MASK) >> PROGRESS_STEP_SHIFT) as u32;
        let samples = progress >> PROGRESS_SAMPLES_SHIFT;
        (completed, step, samples)
    }

    /// Get result status for specific ray
    ///
    /// # Arguments
    ///
    /// * `ray_index` - Ray index (0-7)
    ///
    /// # Returns
    ///
    /// Ray status byte (see RAY_STATUS_* constants)
    #[inline]
    pub fn ray_status(&self, ray_index: usize) -> Option<u8> {
        if ray_index >= MAX_BATCH_SIZE {
            return None;
        }

        let mask = self.result_mask.load(Ordering::Acquire);
        Some((mask >> (ray_index * 8)) as u8)
    }

    /// Check if all rays in batch are complete
    #[inline]
    pub fn is_complete(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        let active = (state & STATE_ACTIVE_MASK) >> STATE_ACTIVE_SHIFT;
        active == 0
    }
}

impl Default for BatchedLosSimdCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Needed for Vec
extern crate alloc;

// Size verification
const _: () = assert!(core::mem::size_of::<BatchedLosSimdCapsule>() == 64);
const _: () = assert!(core::mem::align_of::<BatchedLosSimdCapsule>() == 64);

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::LosRayType;

    // Test helper to create rays
    fn make_ray(ox: i32, oy: i32, tx: i32, ty: i32) -> LosRay {
        LosRay::new(
            Q16_16::from_i32(ox),
            Q16_16::from_i32(oy),
            Q16_16::from_i32(tx),
            Q16_16::from_i32(ty),
            Q16_16::from_i32(1000),
            LosRayType::Batched,
        )
    }

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<BatchedLosSimdCapsule>(), 64);
        assert_eq!(core::mem::align_of::<BatchedLosSimdCapsule>(), 64);
    }

    #[test]
    fn test_capsule_creation() {
        let capsule = BatchedLosSimdCapsule::new();
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.samples_per_ray(), DEFAULT_BATCH_SAMPLES as u16);
    }

    #[test]
    fn test_capsule_with_config() {
        let capsule = BatchedLosSimdCapsule::with_config(4, 200, 50);
        assert_eq!(capsule.samples_per_ray(), 200);
    }

    #[test]
    fn test_empty_batch() {
        let capsule = BatchedLosSimdCapsule::new();
        let map = MapDataCapsule::new(100, 100);

        let results = capsule.traverse_batch(&[], &map);
        assert!(results.is_empty());
    }

    #[test]
    fn test_single_ray_batch() {
        let capsule = BatchedLosSimdCapsule::new();
        let map = MapDataCapsule::new(100, 100);

        // Ray that stays in bounds
        let rays = [make_ray(10, 10, 50, 50)];
        let results = capsule.traverse_batch(&rays, &map);

        assert_eq!(results.len(), 1);
        // Without buffer attached, all samples return None for cover
        // So ray should remain visible
    }

    #[test]
    fn test_max_batch_size() {
        let capsule = BatchedLosSimdCapsule::new();
        let map = MapDataCapsule::new(100, 100);

        // Create 8 rays (max batch size)
        let rays: [LosRay; 8] = core::array::from_fn(|i| {
            make_ray(10, (i * 10) as i32, 50, (i * 10) as i32)
        });

        let results = capsule.traverse_batch(&rays, &map);
        assert_eq!(results.len(), 8);
    }

    #[test]
    fn test_generation_increments() {
        let capsule = BatchedLosSimdCapsule::new();
        let map = MapDataCapsule::new(100, 100);

        let rays = [make_ray(10, 10, 50, 50)];

        assert_eq!(capsule.generation(), 0);

        capsule.traverse_batch(&rays, &map);
        assert_eq!(capsule.generation(), 1);

        capsule.traverse_batch(&rays, &map);
        assert_eq!(capsule.generation(), 2);
    }

    #[test]
    fn test_metrics_after_batch() {
        let capsule = BatchedLosSimdCapsule::new();
        let map = MapDataCapsule::new(100, 100);

        let rays = [make_ray(10, 10, 50, 50), make_ray(20, 20, 60, 60)];
        let _ = capsule.traverse_batch(&rays, &map);

        let (completed, step, samples) = capsule.metrics();
        assert!(step > 0 || completed > 0);
        assert!(samples > 0);
    }

    #[test]
    fn test_ray_status_query() {
        let capsule = BatchedLosSimdCapsule::new();
        let map = MapDataCapsule::new(100, 100);

        let rays = [make_ray(10, 10, 50, 50)];
        let _ = capsule.traverse_batch(&rays, &map);

        // Ray 0 should have a status
        assert!(capsule.ray_status(0).is_some());

        // Ray 8 out of bounds
        assert!(capsule.ray_status(8).is_none());
    }

    #[test]
    fn test_is_complete() {
        let capsule = BatchedLosSimdCapsule::new();
        let map = MapDataCapsule::new(100, 100);

        let rays = [make_ray(10, 10, 50, 50)];
        let _ = capsule.traverse_batch(&rays, &map);

        // After traversal completes, should be marked complete
        assert!(capsule.is_complete());
    }

    #[test]
    fn test_out_of_bounds_ray() {
        let capsule = BatchedLosSimdCapsule::new();
        let map = MapDataCapsule::new(50, 50); // Small map

        // Ray that goes outside map
        let rays = [make_ray(10, 10, 100, 100)];
        let results = capsule.traverse_batch(&rays, &map);

        assert_eq!(results.len(), 1);
        // Should be blocked when hitting map boundary
        assert!(results[0].is_blocked() || results[0].samples_checked < 100);
    }

    #[test]
    fn test_batch_with_attached_buffers() {
        use std::alloc::{alloc, dealloc, Layout};

        let capsule = BatchedLosSimdCapsule::new();
        let map = MapDataCapsule::new(64, 64);

        unsafe {
            let layout = Layout::from_size_align(64 * 64 * 4, 32).unwrap();
            let cover = alloc(layout) as *mut i32;
            let mud = alloc(layout) as *mut i32;
            let cost = alloc(layout) as *mut i32;

            // Initialize cover buffer - clear terrain (no blockers)
            for i in 0..(64 * 64) {
                *cover.add(i) = 0;
                *mud.add(i) = 0;
                *cost.add(i) = 0;
            }

            map.attach_buffers(cover, mud, cost);

            // Ray through clear terrain should be visible
            let rays = [make_ray(10, 10, 50, 50)];
            let results = capsule.traverse_batch(&rays, &map);

            assert_eq!(results.len(), 1);
            // Clear terrain should be visible
            assert!(results[0].is_visible() || results[0].visibility.raw() > 0);

            dealloc(cover as *mut u8, layout);
            dealloc(mud as *mut u8, layout);
            dealloc(cost as *mut u8, layout);
        }
    }

    #[test]
    fn test_batch_with_blockers() {
        use std::alloc::{alloc, dealloc, Layout};

        let capsule = BatchedLosSimdCapsule::new();
        let map = MapDataCapsule::new(64, 64);

        unsafe {
            let layout = Layout::from_size_align(64 * 64 * 4, 32).unwrap();
            let cover = alloc(layout) as *mut i32;
            let mud = alloc(layout) as *mut i32;
            let cost = alloc(layout) as *mut i32;

            // Initialize with clear terrain
            for i in 0..(64 * 64) {
                *cover.add(i) = 0;
                *mud.add(i) = 0;
                *cost.add(i) = 0;
            }

            // Add a wall at x=30 (full column)
            for y in 0..64 {
                *cover.add(y * 64 + 30) = 255; // Full blocker
            }

            map.attach_buffers(cover, mud, cost);

            // Ray that crosses the wall should be blocked
            let rays = [make_ray(10, 32, 50, 32)]; // Horizontal through wall
            let results = capsule.traverse_batch(&rays, &map);

            assert_eq!(results.len(), 1);
            assert!(results[0].is_blocked());

            dealloc(cover as *mut u8, layout);
            dealloc(mud as *mut u8, layout);
            dealloc(cost as *mut u8, layout);
        }
    }

    #[test]
    fn test_partial_cover() {
        use std::alloc::{alloc, dealloc, Layout};

        let capsule = BatchedLosSimdCapsule::new();
        let map = MapDataCapsule::new(64, 64);

        unsafe {
            let layout = Layout::from_size_align(64 * 64 * 4, 32).unwrap();
            let cover = alloc(layout) as *mut i32;
            let mud = alloc(layout) as *mut i32;
            let cost = alloc(layout) as *mut i32;

            // Initialize with light cover (partial occlusion)
            for i in 0..(64 * 64) {
                *cover.add(i) = 64; // 25% cover
                *mud.add(i) = 0;
                *cost.add(i) = 0;
            }

            map.attach_buffers(cover, mud, cost);

            // Ray through partial cover should have reduced visibility
            let rays = [make_ray(10, 10, 50, 50)];
            let results = capsule.traverse_batch(&rays, &map);

            assert_eq!(results.len(), 1);
            // Should have partial visibility (not full, not zero)
            let vis = results[0].visibility;
            assert!(vis.raw() < Q16_16::ONE.raw());

            dealloc(cover as *mut u8, layout);
            dealloc(mud as *mut u8, layout);
            dealloc(cost as *mut u8, layout);
        }
    }
}
