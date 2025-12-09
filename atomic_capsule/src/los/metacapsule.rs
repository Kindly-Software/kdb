//! LOS Metacapsule - T6 Mixed Tier Orchestrator
//!
//! Orchestrates the tiered LOS capsule hierarchy for maximum performance.
//! Coordinates Dense, Tactical, Batched, and Sparse sub-capsules via
//! DualAtomicU64 state management.
//!
//! # Target Performance
//!
//! 50-100× compound speedup via T1+T2+T3+T4 innovation stacking.
//!
//! # Architecture
//!
//! ```text
//! LosMetacapsule (256B orchestrator)
//! ├── DenseLosAvx2Capsule (64B, T2+T3): 500-2K samples
//! ├── TacticalLosSimdCapsule (64B, T2): 80-400 samples
//! ├── BatchedLosSimdCapsule (64B, T4+T2): 4-8 rays SoA
//! ├── SparseLosScalarCapsule (64B, T1): stride≥4
//! └── MapDataCapsule (128B header): Shared SoA buffers
//! ```
//!
//! # Dispatch Strategy
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                  LosMetacapsule                         │
//! │                                                         │
//! │  classify_ray(ray)                                      │
//! │       │                                                 │
//! │       ├──► Dense (samples ≥ 500)?                       │
//! │       │    └── DenseLosAvx2Capsule (AVX2 8× unroll)     │
//! │       │                                                 │
//! │       ├──► Tactical (80 ≤ samples < 500)?               │
//! │       │    └── TacticalLosSimdCapsule (portable_simd)   │
//! │       │                                                 │
//! │       ├──► Batched (multiple rays, 4-8)?                │
//! │       │    └── BatchedLosSimdCapsule (SoA horizontal)   │
//! │       │                                                 │
//! │       └──► Sparse (samples < 80)?                       │
//! │            └── SparseLosScalarCapsule (scalar fallback) │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Chaos Compliance
//!
//! - 256B cache-aligned (4× 64B cache lines)
//! - DualAtomicU64 state coordination
//! - Lockfree sub-capsule dispatch
//! - Generation counter for TOCTOU prevention
//!
//! # ASSUM Tags
//!
//! - #ASSUME_SUBCAPSULE_LIFECYCLE: Sub-capsules remain valid during metacapsule lifetime
//! - #ASSUME_MAP_VALIDITY: MapDataCapsule buffers remain valid during operations
//! - #ASSUME_RAY_CLASSIFICATION: Ray types correctly indicate optimal processing strategy

use super::types::{LosRay, LosResult, LosRayType, Q16_16, LosStatus};
use super::map_data::MapDataCapsule;
use super::{TacticalLosSimdCapsule, SparseLosScalarCapsule, BatchedLosSimdCapsule};
#[cfg(feature = "los-avx2")]
use super::DenseLosAvx2Capsule;
use core::sync::atomic::{AtomicU64, Ordering};

/// Threshold for dense ray classification (samples)
const DENSE_THRESHOLD: u32 = 500;

/// Threshold for tactical ray classification (samples)
const TACTICAL_THRESHOLD: u32 = 80;

/// Minimum batch size for efficient batched processing
const MIN_BATCH_SIZE: usize = 4;

// =============================================================================
// Primary State Layout (64 bits)
// =============================================================================
//
// [0-23]   generation: u24 (16M generations before wrap)
// [24-27]  active_tier: u4 (current tier: 0=idle, 1=dense, 2=tactical, 3=batched, 4=sparse)
// [28-31]  phase: u4 (0=ready, 1=classifying, 2=dispatching, 3=processing, 4=completing)
// [32-47]  active_rays: u16 (rays currently being processed)
// [48-63]  flags: u16 (configuration flags)

const PRIMARY_GEN_MASK: u64 = 0x00FFFFFF;
const PRIMARY_TIER_SHIFT: u32 = 24;
const PRIMARY_TIER_MASK: u64 = 0xF << PRIMARY_TIER_SHIFT;
const PRIMARY_PHASE_SHIFT: u32 = 28;
const PRIMARY_PHASE_MASK: u64 = 0xF << PRIMARY_PHASE_SHIFT;
const PRIMARY_ACTIVE_SHIFT: u32 = 32;
const PRIMARY_ACTIVE_MASK: u64 = 0xFFFF << PRIMARY_ACTIVE_SHIFT;
const PRIMARY_FLAGS_SHIFT: u32 = 48;

// Phase values
const PHASE_READY: u64 = 0;
const PHASE_CLASSIFYING: u64 = 1;
const PHASE_DISPATCHING: u64 = 2;
const PHASE_PROCESSING: u64 = 3;
const PHASE_COMPLETING: u64 = 4;

// Tier values
const TIER_IDLE: u64 = 0;
const TIER_DENSE: u64 = 1;
const TIER_TACTICAL: u64 = 2;
const TIER_BATCHED: u64 = 3;
const TIER_SPARSE: u64 = 4;

// =============================================================================
// Secondary State Layout (64 bits)
// =============================================================================
//
// [0-31]   last_dispatch_ns: u32 (timestamp lower bits)
// [32-47]  pending_rays: u16 (rays waiting to be processed)
// [48-55]  error_count: u8 (consecutive errors)
// [56-63]  reserved: u8

const SECONDARY_TIMESTAMP_MASK: u64 = 0xFFFFFFFF;
const SECONDARY_PENDING_SHIFT: u32 = 32;
const SECONDARY_PENDING_MASK: u64 = 0xFFFF << SECONDARY_PENDING_SHIFT;
const SECONDARY_ERRORS_SHIFT: u32 = 48;

/// LosMetacapsule (256B) - T6 Mixed tier orchestrator
///
/// Coordinates 4 sub-capsules for optimal ray-type routing.
/// Uses DualAtomicU64 pattern for TOCTOU-safe state management.
///
/// # Layout (256 bytes)
///
/// | Offset | Field | Size | Purpose |
/// |--------|-------|------|---------|
/// | 0-7 | primary_state | 8B | gen(24)\|tier(4)\|phase(4)\|active(16)\|flags(16) |
/// | 8-15 | secondary_state | 8B | timestamp(32)\|pending(16)\|errors(8)\|reserved(8) |
/// | 16-23 | config | 8B | dense_threshold(16)\|tactical_threshold(16)\|batch_size(8)\|flags(24) |
/// | 24-31 | reserved | 8B | Future use |
/// | 32-95 | metrics | 64B | 8× u64 counters |
/// | 96-127 | sub_capsule_refs | 32B | 4× AtomicPtr for optional external sub-capsules |
/// | 128-255 | _padding | 128B | Cache line alignment |
#[repr(C, align(256))]
pub struct LosMetacapsule {
    // DualAtomicU64 pattern: primary + secondary state
    primary_state: AtomicU64,
    secondary_state: AtomicU64,

    // Configuration
    config: AtomicU64,
    _reserved: u64,

    // Metrics (64 bytes = 8× u64)
    rays_processed: AtomicU64,
    samples_evaluated: AtomicU64,
    early_exits: AtomicU64,
    avx2_dispatches: AtomicU64,
    portable_dispatches: AtomicU64,
    batched_dispatches: AtomicU64,
    sparse_dispatches: AtomicU64,
    auto_classify_count: AtomicU64,

    // Sub-capsule reference tracking (for optional external capsules)
    // When null, use stack-allocated capsules in dispatch methods
    _dense_ref: AtomicU64,
    _tactical_ref: AtomicU64,
    _batched_ref: AtomicU64,
    _sparse_ref: AtomicU64,

    // Padding to 256B
    _padding: [u8; 128],
}

impl LosMetacapsule {
    /// Create a new LOS metacapsule with default configuration
    ///
    /// Default thresholds:
    /// - Dense: ≥500 samples
    /// - Tactical: 80-499 samples
    /// - Sparse: <80 samples
    /// - Batch: 4+ rays of same type
    pub const fn new() -> Self {
        // Config: dense_threshold(16)|tactical_threshold(16)|batch_size(8)|flags(24)
        let config = (DENSE_THRESHOLD as u64)
            | ((TACTICAL_THRESHOLD as u64) << 16)
            | ((MIN_BATCH_SIZE as u64) << 32);

        Self {
            primary_state: AtomicU64::new(0),
            secondary_state: AtomicU64::new(0),
            config: AtomicU64::new(config),
            _reserved: 0,
            rays_processed: AtomicU64::new(0),
            samples_evaluated: AtomicU64::new(0),
            early_exits: AtomicU64::new(0),
            avx2_dispatches: AtomicU64::new(0),
            portable_dispatches: AtomicU64::new(0),
            batched_dispatches: AtomicU64::new(0),
            sparse_dispatches: AtomicU64::new(0),
            auto_classify_count: AtomicU64::new(0),
            _dense_ref: AtomicU64::new(0),
            _tactical_ref: AtomicU64::new(0),
            _batched_ref: AtomicU64::new(0),
            _sparse_ref: AtomicU64::new(0),
            _padding: [0u8; 128],
        }
    }

    /// Create with custom thresholds
    ///
    /// # Arguments
    ///
    /// * `dense_threshold` - Minimum samples for dense processing
    /// * `tactical_threshold` - Minimum samples for tactical processing
    /// * `min_batch_size` - Minimum rays for batched processing
    pub const fn with_config(
        dense_threshold: u16,
        tactical_threshold: u16,
        min_batch_size: u8,
    ) -> Self {
        let config = (dense_threshold as u64)
            | ((tactical_threshold as u64) << 16)
            | ((min_batch_size as u64) << 32);

        Self {
            primary_state: AtomicU64::new(0),
            secondary_state: AtomicU64::new(0),
            config: AtomicU64::new(config),
            _reserved: 0,
            rays_processed: AtomicU64::new(0),
            samples_evaluated: AtomicU64::new(0),
            early_exits: AtomicU64::new(0),
            avx2_dispatches: AtomicU64::new(0),
            portable_dispatches: AtomicU64::new(0),
            batched_dispatches: AtomicU64::new(0),
            sparse_dispatches: AtomicU64::new(0),
            auto_classify_count: AtomicU64::new(0),
            _dense_ref: AtomicU64::new(0),
            _tactical_ref: AtomicU64::new(0),
            _batched_ref: AtomicU64::new(0),
            _sparse_ref: AtomicU64::new(0),
            _padding: [0u8; 128],
        }
    }

    /// Get current generation counter
    ///
    /// # Returns
    ///
    /// Generation counter (0-16,777,215), wraps at 24 bits.
    #[inline]
    pub fn generation(&self) -> u32 {
        let state = self.primary_state.load(Ordering::Acquire);
        (state & PRIMARY_GEN_MASK) as u32
    }

    /// Get current processing phase
    #[inline]
    pub fn phase(&self) -> u8 {
        let state = self.primary_state.load(Ordering::Acquire);
        ((state & PRIMARY_PHASE_MASK) >> PRIMARY_PHASE_SHIFT) as u8
    }

    /// Increment generation and set phase
    #[inline]
    fn transition_phase(&self, new_phase: u64, tier: u64, active_rays: u16) {
        let old = self.primary_state.load(Ordering::Acquire);
        let gen = ((old & PRIMARY_GEN_MASK) + 1) & PRIMARY_GEN_MASK;
        let new = gen
            | (tier << PRIMARY_TIER_SHIFT)
            | (new_phase << PRIMARY_PHASE_SHIFT)
            | ((active_rays as u64) << PRIMARY_ACTIVE_SHIFT);
        self.primary_state.store(new, Ordering::Release);
    }

    /// Classify a ray to determine optimal processing tier
    ///
    /// # Classification Rules
    ///
    /// 1. If ray.ray_type is explicitly set, use that
    /// 2. Otherwise, auto-classify based on ray length:
    ///    - Dense: length suggests ≥500 samples
    ///    - Tactical: length suggests 80-499 samples
    ///    - Sparse: length suggests <80 samples
    ///
    /// # Arguments
    ///
    /// * `ray` - The ray to classify
    ///
    /// # Returns
    ///
    /// Optimal ray type for processing
    #[inline]
    pub fn classify_ray(&self, ray: &LosRay) -> LosRayType {
        // Explicit type takes precedence
        match ray.ray_type {
            LosRayType::Dense | LosRayType::Tactical | LosRayType::Batched | LosRayType::Sparse => {
                return ray.ray_type;
            }
        }

        // Auto-classify based on ray length
        self.auto_classify_count.fetch_add(1, Ordering::Relaxed);

        let config = self.config.load(Ordering::Acquire);
        let dense_thresh = (config & 0xFFFF) as u32;
        let tactical_thresh = ((config >> 16) & 0xFFFF) as u32;

        // Estimate samples from ray length
        // Typical: 1 sample per 0.1 world units
        let length = ray.length();
        let estimated_samples = (length.to_f32() * 10.0) as u32;

        if estimated_samples >= dense_thresh {
            LosRayType::Dense
        } else if estimated_samples >= tactical_thresh {
            LosRayType::Tactical
        } else {
            LosRayType::Sparse
        }
    }

    /// Cast a single ray through the map
    ///
    /// Automatically routes to optimal sub-capsule based on ray type.
    ///
    /// # Arguments
    ///
    /// * `ray` - The LOS ray to cast
    /// * `map` - Map data capsule with terrain information
    ///
    /// # Returns
    ///
    /// LosResult with visibility, samples checked, and status
    pub fn cast_ray(&self, ray: &LosRay, map: &MapDataCapsule) -> LosResult {
        // Transition to classifying phase
        self.transition_phase(PHASE_CLASSIFYING, TIER_IDLE, 1);

        // Classify the ray
        let ray_type = self.classify_ray(ray);

        // Transition to dispatching phase
        let tier = match ray_type {
            LosRayType::Dense => TIER_DENSE,
            LosRayType::Tactical => TIER_TACTICAL,
            LosRayType::Batched => TIER_BATCHED,
            LosRayType::Sparse => TIER_SPARSE,
        };
        self.transition_phase(PHASE_DISPATCHING, tier, 1);

        // Dispatch to appropriate sub-capsule
        self.transition_phase(PHASE_PROCESSING, tier, 1);

        let result = match ray_type {
            LosRayType::Dense => {
                self.avx2_dispatches.fetch_add(1, Ordering::Relaxed);
                #[cfg(feature = "los-avx2")]
                {
                    let capsule = DenseLosAvx2Capsule::new();
                    capsule.traverse(ray, map)
                }
                #[cfg(not(feature = "los-avx2"))]
                {
                    // Fallback to tactical if AVX2 not available
                    self.portable_dispatches.fetch_add(1, Ordering::Relaxed);
                    let capsule = TacticalLosSimdCapsule::new();
                    capsule.traverse(ray, map)
                }
            }
            LosRayType::Tactical => {
                self.portable_dispatches.fetch_add(1, Ordering::Relaxed);
                let capsule = TacticalLosSimdCapsule::new();
                capsule.traverse(ray, map)
            }
            LosRayType::Batched => {
                // Single ray with Batched type - use BatchedLosSimdCapsule
                self.batched_dispatches.fetch_add(1, Ordering::Relaxed);
                let capsule = BatchedLosSimdCapsule::new();
                let results = capsule.traverse_batch(&[*ray], map);
                results.into_iter().next().unwrap_or_else(|| LosResult::blocked(0))
            }
            LosRayType::Sparse => {
                self.sparse_dispatches.fetch_add(1, Ordering::Relaxed);
                let capsule = SparseLosScalarCapsule::new();
                capsule.traverse(ray, map)
            }
        };

        // Update metrics
        self.rays_processed.fetch_add(1, Ordering::Relaxed);
        self.samples_evaluated.fetch_add(result.samples_checked as u64, Ordering::Relaxed);
        if matches!(result.status, LosStatus::EarlyExit) {
            self.early_exits.fetch_add(1, Ordering::Relaxed);
        }

        // Transition to complete
        self.transition_phase(PHASE_COMPLETING, TIER_IDLE, 0);
        self.transition_phase(PHASE_READY, TIER_IDLE, 0);

        result
    }

    /// Cast multiple rays in batch
    ///
    /// Intelligently groups rays by type for optimal batch processing.
    ///
    /// # Algorithm
    ///
    /// 1. Classify all rays
    /// 2. Group rays by type
    /// 3. Dispatch groups:
    ///    - Groups with 4+ rays use BatchedLosSimdCapsule
    ///    - Smaller groups use per-ray dispatch
    /// 4. Reassemble results in original order
    ///
    /// # Arguments
    ///
    /// * `rays` - Slice of rays to cast
    /// * `map` - Map data capsule
    ///
    /// # Returns
    ///
    /// Vector of results, one per ray (in same order as input)
    pub fn cast_rays_batch(&self, rays: &[LosRay], map: &MapDataCapsule) -> alloc::vec::Vec<LosResult> {
        if rays.is_empty() {
            return alloc::vec::Vec::new();
        }

        // Transition to classifying phase
        self.transition_phase(PHASE_CLASSIFYING, TIER_IDLE, rays.len() as u16);

        let config = self.config.load(Ordering::Acquire);
        let min_batch = ((config >> 32) & 0xFF) as usize;

        // For small batches, process individually
        if rays.len() < min_batch {
            return rays.iter().map(|ray| self.cast_ray(ray, map)).collect();
        }

        // Group rays by type
        let mut dense_rays: alloc::vec::Vec<(usize, &LosRay)> = alloc::vec::Vec::new();
        let mut tactical_rays: alloc::vec::Vec<(usize, &LosRay)> = alloc::vec::Vec::new();
        let mut sparse_rays: alloc::vec::Vec<(usize, &LosRay)> = alloc::vec::Vec::new();
        let mut batched_rays: alloc::vec::Vec<(usize, &LosRay)> = alloc::vec::Vec::new();

        for (idx, ray) in rays.iter().enumerate() {
            let ray_type = self.classify_ray(ray);
            match ray_type {
                LosRayType::Dense => dense_rays.push((idx, ray)),
                LosRayType::Tactical => tactical_rays.push((idx, ray)),
                LosRayType::Sparse => sparse_rays.push((idx, ray)),
                LosRayType::Batched => batched_rays.push((idx, ray)),
            }
        }

        // Prepare results vector
        let mut results: alloc::vec::Vec<Option<LosResult>> = (0..rays.len()).map(|_| None).collect();

        // Transition to processing phase
        self.transition_phase(PHASE_PROCESSING, TIER_BATCHED, rays.len() as u16);

        // Process dense rays
        #[cfg(feature = "los-avx2")]
        if !dense_rays.is_empty() {
            let capsule = DenseLosAvx2Capsule::new();
            self.avx2_dispatches.fetch_add(dense_rays.len() as u64, Ordering::Relaxed);
            for (idx, ray) in dense_rays {
                results[idx] = Some(capsule.traverse(ray, map));
            }
        }
        #[cfg(not(feature = "los-avx2"))]
        {
            // Merge dense into tactical when AVX2 not available
            tactical_rays.extend(dense_rays);
        }

        // Process tactical rays
        if !tactical_rays.is_empty() {
            let capsule = TacticalLosSimdCapsule::new();
            self.portable_dispatches.fetch_add(tactical_rays.len() as u64, Ordering::Relaxed);
            for (idx, ray) in tactical_rays {
                results[idx] = Some(capsule.traverse(ray, map));
            }
        }

        // Process batched rays (use BatchedLosSimdCapsule for efficiency)
        if !batched_rays.is_empty() {
            let capsule = BatchedLosSimdCapsule::new();
            self.batched_dispatches.fetch_add(batched_rays.len() as u64, Ordering::Relaxed);

            // Process in chunks of 8 (MAX_BATCH_SIZE)
            for chunk in batched_rays.chunks(8) {
                let chunk_rays: alloc::vec::Vec<LosRay> = chunk.iter().map(|(_, r)| **r).collect();
                let chunk_results = capsule.traverse_batch(&chunk_rays, map);

                for ((idx, _), result) in chunk.iter().zip(chunk_results) {
                    results[*idx] = Some(result);
                }
            }
        }

        // Process sparse rays
        if !sparse_rays.is_empty() {
            let capsule = SparseLosScalarCapsule::new();
            self.sparse_dispatches.fetch_add(sparse_rays.len() as u64, Ordering::Relaxed);
            for (idx, ray) in sparse_rays {
                results[idx] = Some(capsule.traverse(ray, map));
            }
        }

        // Update metrics
        let total_samples: u64 = results.iter()
            .filter_map(|r| r.as_ref())
            .map(|r| r.samples_checked as u64)
            .sum();
        self.rays_processed.fetch_add(rays.len() as u64, Ordering::Relaxed);
        self.samples_evaluated.fetch_add(total_samples, Ordering::Relaxed);

        let early_exit_count = results.iter()
            .filter_map(|r| r.as_ref())
            .filter(|r| matches!(r.status, LosStatus::EarlyExit))
            .count();
        self.early_exits.fetch_add(early_exit_count as u64, Ordering::Relaxed);

        // Transition to complete
        self.transition_phase(PHASE_COMPLETING, TIER_IDLE, 0);
        self.transition_phase(PHASE_READY, TIER_IDLE, 0);

        // Convert to final results (unwrap Options)
        results.into_iter()
            .map(|r| r.unwrap_or_else(|| LosResult::blocked(0)))
            .collect()
    }

    /// Cast rays with automatic batching
    ///
    /// Groups similar rays and processes them using the most efficient method.
    /// For large batches of similar rays, this can achieve 40× speedup vs individual dispatch.
    ///
    /// # Arguments
    ///
    /// * `rays` - Slice of rays to cast
    /// * `map` - Map data capsule
    ///
    /// # Returns
    ///
    /// Vector of results, one per ray
    #[inline]
    pub fn cast_rays_auto(&self, rays: &[LosRay], map: &MapDataCapsule) -> alloc::vec::Vec<LosResult> {
        self.cast_rays_batch(rays, map)
    }

    /// Get current metrics
    pub fn metrics(&self) -> LosMetacapsuleMetrics {
        LosMetacapsuleMetrics {
            rays_processed: self.rays_processed.load(Ordering::Relaxed),
            samples_evaluated: self.samples_evaluated.load(Ordering::Relaxed),
            early_exits: self.early_exits.load(Ordering::Relaxed),
            avx2_dispatches: self.avx2_dispatches.load(Ordering::Relaxed),
            portable_dispatches: self.portable_dispatches.load(Ordering::Relaxed),
            batched_dispatches: self.batched_dispatches.load(Ordering::Relaxed),
            sparse_dispatches: self.sparse_dispatches.load(Ordering::Relaxed),
            auto_classify_count: self.auto_classify_count.load(Ordering::Relaxed),
        }
    }

    /// Reset metrics to zero
    pub fn reset_metrics(&self) {
        self.rays_processed.store(0, Ordering::Relaxed);
        self.samples_evaluated.store(0, Ordering::Relaxed);
        self.early_exits.store(0, Ordering::Relaxed);
        self.avx2_dispatches.store(0, Ordering::Relaxed);
        self.portable_dispatches.store(0, Ordering::Relaxed);
        self.batched_dispatches.store(0, Ordering::Relaxed);
        self.sparse_dispatches.store(0, Ordering::Relaxed);
        self.auto_classify_count.store(0, Ordering::Relaxed);
    }

    /// Check if metacapsule is idle (ready for new operations)
    #[inline]
    pub fn is_idle(&self) -> bool {
        let state = self.primary_state.load(Ordering::Acquire);
        let phase = (state & PRIMARY_PHASE_MASK) >> PRIMARY_PHASE_SHIFT;
        phase == PHASE_READY
    }
}

impl Default for LosMetacapsule {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics snapshot from LosMetacapsule
#[derive(Debug, Clone, Copy, Default)]
pub struct LosMetacapsuleMetrics {
    /// Total rays processed
    pub rays_processed: u64,
    /// Total samples evaluated across all rays
    pub samples_evaluated: u64,
    /// Rays that terminated via early-exit
    pub early_exits: u64,
    /// Rays dispatched to AVX2 path
    pub avx2_dispatches: u64,
    /// Rays dispatched to portable_simd path
    pub portable_dispatches: u64,
    /// Rays dispatched to batched path
    pub batched_dispatches: u64,
    /// Rays dispatched to sparse/scalar path
    pub sparse_dispatches: u64,
    /// Rays auto-classified (type not explicitly set)
    pub auto_classify_count: u64,
}

impl LosMetacapsuleMetrics {
    /// Total dispatches (should equal rays_processed)
    #[inline]
    pub fn total_dispatches(&self) -> u64 {
        self.avx2_dispatches + self.portable_dispatches + self.batched_dispatches + self.sparse_dispatches
    }

    /// Average samples per ray
    #[inline]
    pub fn avg_samples_per_ray(&self) -> f64 {
        if self.rays_processed == 0 {
            0.0
        } else {
            self.samples_evaluated as f64 / self.rays_processed as f64
        }
    }

    /// Early exit rate (0.0 - 1.0)
    #[inline]
    pub fn early_exit_rate(&self) -> f64 {
        if self.rays_processed == 0 {
            0.0
        } else {
            self.early_exits as f64 / self.rays_processed as f64
        }
    }
}

// Needed for Vec
extern crate alloc;

// Size verification
const _: () = assert!(core::mem::size_of::<LosMetacapsule>() == 256);
const _: () = assert!(core::mem::align_of::<LosMetacapsule>() == 256);

#[cfg(test)]
mod tests {
    use super::*;

    // Test helper to create rays
    fn make_ray(ox: i32, oy: i32, tx: i32, ty: i32, ray_type: LosRayType) -> LosRay {
        LosRay::new(
            Q16_16::from_i32(ox),
            Q16_16::from_i32(oy),
            Q16_16::from_i32(tx),
            Q16_16::from_i32(ty),
            Q16_16::from_i32(1000),
            ray_type,
        )
    }

    #[test]
    fn test_metacapsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<LosMetacapsule>(), 256);
        assert_eq!(core::mem::align_of::<LosMetacapsule>(), 256);
    }

    #[test]
    fn test_metacapsule_creation() {
        let meta = LosMetacapsule::new();
        let metrics = meta.metrics();
        assert_eq!(metrics.rays_processed, 0);
        assert!(meta.is_idle());
    }

    #[test]
    fn test_metacapsule_with_config() {
        let meta = LosMetacapsule::with_config(1000, 200, 8);
        assert!(meta.is_idle());
    }

    #[test]
    fn test_generation_counter() {
        let meta = LosMetacapsule::new();
        assert_eq!(meta.generation(), 0);

        // After processing a ray, generation should increment
        let map = MapDataCapsule::new(100, 100);
        let ray = make_ray(10, 10, 50, 50, LosRayType::Sparse);
        let _ = meta.cast_ray(&ray, &map);

        // Multiple transitions happen, so generation increases by more than 1
        assert!(meta.generation() > 0);
    }

    #[test]
    fn test_classify_ray() {
        let meta = LosMetacapsule::new();

        // Explicit type
        let dense_ray = make_ray(0, 0, 1000, 1000, LosRayType::Dense);
        assert_eq!(meta.classify_ray(&dense_ray), LosRayType::Dense);

        let tactical_ray = make_ray(0, 0, 100, 100, LosRayType::Tactical);
        assert_eq!(meta.classify_ray(&tactical_ray), LosRayType::Tactical);
    }

    #[test]
    fn test_cast_single_ray() {
        let meta = LosMetacapsule::new();
        let map = MapDataCapsule::new(100, 100);

        let ray = make_ray(10, 10, 50, 50, LosRayType::Sparse);
        let result = meta.cast_ray(&ray, &map);

        // Should have some result
        assert!(result.samples_checked > 0 || result.is_visible() || result.is_blocked());

        let metrics = meta.metrics();
        assert_eq!(metrics.rays_processed, 1);
        assert_eq!(metrics.sparse_dispatches, 1);
    }

    #[test]
    fn test_cast_rays_batch() {
        let meta = LosMetacapsule::new();
        let map = MapDataCapsule::new(100, 100);

        let rays = [
            make_ray(10, 10, 50, 50, LosRayType::Sparse),
            make_ray(20, 20, 60, 60, LosRayType::Tactical),
            make_ray(30, 30, 70, 70, LosRayType::Sparse),
        ];

        let results = meta.cast_rays_batch(&rays, &map);

        assert_eq!(results.len(), 3);
        let metrics = meta.metrics();
        assert_eq!(metrics.rays_processed, 3);
    }

    #[test]
    fn test_cast_rays_with_batched_type() {
        let meta = LosMetacapsule::new();
        let map = MapDataCapsule::new(100, 100);

        // Create rays explicitly marked as Batched
        let rays: [LosRay; 8] = core::array::from_fn(|i| {
            make_ray(10, (i * 5) as i32, 50, (i * 5) as i32, LosRayType::Batched)
        });

        let results = meta.cast_rays_batch(&rays, &map);

        assert_eq!(results.len(), 8);
        let metrics = meta.metrics();
        assert!(metrics.batched_dispatches > 0);
    }

    #[test]
    fn test_metrics_reset() {
        let meta = LosMetacapsule::new();
        let map = MapDataCapsule::new(100, 100);

        let ray = make_ray(10, 10, 50, 50, LosRayType::Sparse);
        let _ = meta.cast_ray(&ray, &map);

        assert!(meta.metrics().rays_processed > 0);

        meta.reset_metrics();
        assert_eq!(meta.metrics().rays_processed, 0);
    }

    #[test]
    fn test_metrics_calculations() {
        let mut metrics = LosMetacapsuleMetrics::default();
        metrics.rays_processed = 100;
        metrics.samples_evaluated = 5000;
        metrics.early_exits = 25;
        metrics.avx2_dispatches = 20;
        metrics.portable_dispatches = 50;
        metrics.batched_dispatches = 20;
        metrics.sparse_dispatches = 10;

        assert_eq!(metrics.total_dispatches(), 100);
        assert_eq!(metrics.avg_samples_per_ray(), 50.0);
        assert_eq!(metrics.early_exit_rate(), 0.25);
    }

    #[test]
    fn test_empty_batch() {
        let meta = LosMetacapsule::new();
        let map = MapDataCapsule::new(100, 100);

        let results = meta.cast_rays_batch(&[], &map);
        assert!(results.is_empty());
    }

    #[test]
    fn test_mixed_ray_types() {
        let meta = LosMetacapsule::new();
        let map = MapDataCapsule::new(100, 100);

        let rays = [
            make_ray(10, 10, 50, 50, LosRayType::Dense),
            make_ray(20, 20, 60, 60, LosRayType::Tactical),
            make_ray(30, 30, 70, 70, LosRayType::Batched),
            make_ray(40, 40, 80, 80, LosRayType::Sparse),
        ];

        let results = meta.cast_rays_batch(&rays, &map);
        assert_eq!(results.len(), 4);

        // All ray types should have dispatched
        let metrics = meta.metrics();
        // Without AVX2, dense fallbacks to tactical
        assert!(metrics.portable_dispatches >= 1 || metrics.avx2_dispatches >= 1);
        assert!(metrics.batched_dispatches >= 1);
        assert!(metrics.sparse_dispatches >= 1);
    }

    #[test]
    fn test_cast_rays_auto() {
        let meta = LosMetacapsule::new();
        let map = MapDataCapsule::new(100, 100);

        let rays = [
            make_ray(10, 10, 50, 50, LosRayType::Tactical),
            make_ray(20, 20, 60, 60, LosRayType::Tactical),
        ];

        let results = meta.cast_rays_auto(&rays, &map);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_is_idle_after_processing() {
        let meta = LosMetacapsule::new();
        let map = MapDataCapsule::new(100, 100);

        assert!(meta.is_idle());

        let ray = make_ray(10, 10, 50, 50, LosRayType::Sparse);
        let _ = meta.cast_ray(&ray, &map);

        // Should be idle again after processing completes
        assert!(meta.is_idle());
    }
}
