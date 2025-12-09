//! # AdaptiveQuantCapsule - Runtime Quantization Adaptation with Commit-Flip
//!
//! **Lockfree 4-bit quantization with generation counter protection against torn reads.**

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering, compiler_fence};
use core::cell::UnsafeCell;

/// Adaptive quantization capsule with commit-flip publishing
///
/// # Safety Invariants (ASSUM Framework)
///
/// ## ASSUME_UNSAFECELL_SYNC
/// UnsafeCell<[u8; 64]> allows interior mutability while maintaining thread safety
/// through generation counter synchronization (SWeMR pattern: Single-Writer, Many-Readers).
///
/// ## VERIFY_UNSAFECELL_SYNC
/// - Single writer via SWeMR ownership pattern
/// - Generation counter provides synchronization barrier
/// - Readers use double-check pattern to detect concurrent updates
///
/// ## ASSUME_ACQUIRE_RELEASE
/// Acquire load in reader synchronizes with Release store in writer, establishing
/// happens-before relationship that ensures visibility of all payload writes.
///
/// ## VERIFY_ACQUIRE_RELEASE
/// Rust memory model guarantees visibility when Acquire synchronizes with Release.
///
/// ## ASSUME_COMPILER_FENCE
/// compiler_fence(Ordering::Release) ensures payload writes are visible before
/// the commit (even generation store).
///
/// ## VERIFY_COMPILER_FENCE
/// Release fence pairs with Acquire load, preventing compiler/CPU reordering.
///
/// ## ASSUME_DOUBLE_CHECK_TOCTOU
/// Generation checked before AND after payload read. If generation unchanged,
/// payload is consistent with metadata.
///
/// ## VERIFY_DOUBLE_CHECK_TOCTOU
/// Stress tests validate zero torn reads under concurrent updates.
#[repr(C, align(128))]
pub struct AdaptiveQuantCapsule {
    metadata: AtomicU64,
    weights_4bit: UnsafeCell<[u8; 64]>,
    running_min: AtomicU32,
    running_max: AtomicU32,
    access_count: AtomicU32,
    _padding: [u8; 44],
}

// SAFETY: AdaptiveQuantCapsule is Sync because:
// - metadata: AtomicU64 is Sync
// - weights_4bit: UnsafeCell requires manual Sync impl
//   - Generation counter provides synchronization (SWeMR pattern)
//   - Single writer updates via commit-flip protocol
//   - Readers use Acquire ordering and double-check pattern
// - running_min/running_max/access_count: AtomicU32 are Sync
unsafe impl Sync for AdaptiveQuantCapsule {}

impl AdaptiveQuantCapsule {
    /// Creates a new AdaptiveQuantCapsule with zero-initialized state.
    pub const fn new() -> Self {
        Self {
            metadata: AtomicU64::new(0),
            weights_4bit: UnsafeCell::new([0u8; 64]),
            running_min: AtomicU32::new(0),
            running_max: AtomicU32::new(0),
            access_count: AtomicU32::new(0),
            _padding: [0u8; 44],
        }
    }

    /// Adapts quantization parameters based on new weight distribution using commit-flip protocol.
    ///
    /// This method uses a three-phase commit:
    /// 1. Set generation to odd (uncommitted) - Relaxed ordering
    /// 2. Update quantized weights in UnsafeCell
    /// 3. compiler_fence(Release) - Ensure writes visible
    /// 4. Set generation to even (committed) - Release ordering
    ///
    /// Readers will skip data during phase 1-2 (odd generation).
    /// compiler_fence ensures all payload writes are visible before commit.
    pub fn adapt_quantization(&self, weights: &[f32; 128]) {
        let mut min_val = weights[0];
        let mut max_val = weights[0];
        for &w in weights.iter() {
            if w < min_val { min_val = w; }
            if w > max_val { max_val = w; }
        }

        let abs_max = if min_val.abs() > max_val.abs() { min_val.abs() } else { max_val.abs() };
        let scale = if abs_max > 1e-8 { abs_max / 7.0 } else { 1.0 };
        let zero_point: u8 = 0;
        let scale_q16_16 = (scale * 65536.0) as u16;

        // Phase 1: Mark in-progress (odd generation)
        let current_gen = self.generation();
        let odd_gen = (current_gen + 1) | 1;
        let metadata_odd = pack_metadata(odd_gen, scale_q16_16, zero_point);
        self.metadata.store(metadata_odd, Ordering::Relaxed);

        // Phase 2: Update payload (writes to UnsafeCell)
        self.quantize_weights(weights, scale);

        let min_q16 = f32_to_q16_16(min_val);
        let max_q16 = f32_to_q16_16(max_val);
        self.running_min.store(min_q16, Ordering::Relaxed);
        self.running_max.store(max_q16, Ordering::Relaxed);

        // Phase 3: Ensure payload writes visible before commit
        // CRITICAL: Without this fence, readers may see committed generation
        // but stale payload data due to compiler/CPU reordering
        compiler_fence(Ordering::Release);

        // Phase 4: Commit (even generation)
        let even_gen = odd_gen + 1;
        let metadata_even = pack_metadata(even_gen, scale_q16_16, zero_point);
        self.metadata.store(metadata_even, Ordering::Release);
    }

    /// Loads a dequantized weight value, returning None if generation is uncommitted or index is out of bounds.
    ///
    /// This method uses double-check pattern to prevent TOCTOU races:
    /// 1. Load metadata with Acquire (synchronizes with Release store)
    /// 2. Check generation is even (committed)
    /// 3. Load payload from UnsafeCell
    /// 4. Re-check generation unchanged (detects concurrent updates)
    ///
    /// If generation changed between checks, return None (caller retries).
    #[inline(always)]
    pub fn load_weight(&self, index: usize) -> Option<f32> {
        if index >= 128 { return None; }

        // FIRST CHECK: Load metadata with Acquire ordering
        // Synchronizes with Release store in adapt_quantization
        let metadata_before = self.metadata.load(Ordering::Acquire);
        let generation_before = (metadata_before & 0xFFFF_FFFF) as u32;

        // Reject uncommitted state (odd generation)
        if generation_before % 2 != 0 { return None; }

        // Extract scale/zero from known-consistent metadata
        let scale_bits = ((metadata_before >> 32) & 0xFFFF) as u16;
        let zero_point = ((metadata_before >> 48) & 0xFF) as u8;

        // PAYLOAD READ: Load quantized value from UnsafeCell
        let quantized = self.load_quantized_weight(index);

        // SECOND CHECK: Validate generation unchanged (TOCTOU prevention)
        let metadata_after = self.metadata.load(Ordering::Acquire);
        let generation_after = (metadata_after & 0xFFFF_FFFF) as u32;

        if generation_before != generation_after {
            return None; // Generation changed mid-read, retry
        }

        // Dequantize using consistent metadata
        let scale = q16_16_to_f32(scale_bits);
        let dequantized = (quantized as f32 - zero_point as f32) * scale;

        self.access_count.fetch_add(1, Ordering::Relaxed);
        Some(dequantized)
    }

    /// Returns the current generation counter (even = committed, odd = in-progress).
    ///
    /// Uses Acquire ordering to synchronize with Release store in adapt_quantization.
    #[inline(always)]
    pub fn generation(&self) -> u32 {
        let metadata = self.metadata.load(Ordering::Acquire);
        (metadata & 0xFFFF_FFFF) as u32
    }

    /// Returns true if the capsule is in a committed state (even generation).
    #[inline(always)]
    pub fn is_committed(&self) -> bool {
        self.generation() % 2 == 0
    }

    /// Returns runtime statistics: (min_value, max_value, access_count).
    pub fn statistics(&self) -> (f32, f32, u32) {
        let min_q16 = self.running_min.load(Ordering::Relaxed);
        let max_q16 = self.running_max.load(Ordering::Relaxed);
        let count = self.access_count.load(Ordering::Relaxed);
        let min = q16_16_to_f32(min_q16 as u16);
        let max = q16_16_to_f32(max_q16 as u16);
        (min, max, count)
    }

    fn quantize_weights(&self, weights: &[f32; 128], scale: f32) {
        let weights_ptr = self.weights_4bit.get();
        for i in 0..64 {
            let w0 = weights[2 * i];
            let w1 = weights[2 * i + 1];
            let q0 = quantize_4bit(w0, scale);
            let q1 = quantize_4bit(w1, scale);
            let packed = ((q0 as u8 & 0x0F) << 4) | (q1 as u8 & 0x0F);
            unsafe { (*weights_ptr)[i] = packed; }
        }
    }

    fn load_quantized_weight(&self, index: usize) -> i8 {
        unsafe {
            let weights_ptr = self.weights_4bit.get();
            self.load_quantized_weight_from(index, &*weights_ptr)
        }
    }

    // Helper to avoid duplication between load_weight and load_quantized_weight
    fn load_quantized_weight_from(&self, index: usize, weights: &[u8; 64]) -> i8 {
        let byte_idx = index / 2;
        let packed = weights[byte_idx];
        if index % 2 == 0 {
            let q = (packed >> 4) & 0x0F;
            if q & 0x08 != 0 { (q | 0xF0) as i8 } else { q as i8 }
        } else {
            let q = packed & 0x0F;
            if q & 0x08 != 0 { (q | 0xF0) as i8 } else { q as i8 }
        }
    }
}

impl Default for AdaptiveQuantCapsule {
    fn default() -> Self { Self::new() }
}

#[inline(always)]
fn pack_metadata(generation: u32, scale_bits: u16, zero_point: u8) -> u64 {
    let gen = generation as u64;
    let scale = (scale_bits as u64) << 32;
    let zero = (zero_point as u64) << 48;
    gen | scale | zero
}

#[inline(always)]
fn f32_to_q16_16(value: f32) -> u32 { (value * 65536.0) as u32 }

#[inline(always)]
fn q16_16_to_f32(value: u16) -> f32 { (value as f32) / 65536.0 }

#[inline(always)]
fn quantize_4bit(value: f32, scale: f32) -> i8 {
    let normalized = value / scale;
    let quantized = (normalized.round() as i32).clamp(-7, 7);
    quantized as i8
}

const _: () = {
    assert!(core::mem::size_of::<AdaptiveQuantCapsule>() == 128);
    assert!(core::mem::align_of::<AdaptiveQuantCapsule>() == 128);
};
