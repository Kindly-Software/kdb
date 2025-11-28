//! EncoderSubCapsules - T4 Batch tier handle for AV1 encoder sub-capsules
//!
//! UCE34 Compliance: Q10 T4 Batch tier (holds batch of capsules), Q33 lockfree
//! COCA Compliance: Generation counter, cache-aligned, no mutex

use atomic_capsule::encoder::{
    DctTransformCapsule, EntropyCoderCapsule, EncoderStateCapsule, FrameBufferCapsule,
    ObuBitstreamWriterCapsule, QuantizationCapsule, ReferenceFrameCapsule,
    TileCoordinatorCapsule,
};
use std::sync::atomic::{AtomicU64, Ordering};

/// EncoderSubCapsules - Opaque handle holding references to all encoder sub-capsules
///
/// # Architecture
/// - 256B cache-aligned container
/// - Holds Box references to 8 atomic_capsule encoder capsules
/// - Generation counter for COCA compliance (ABA prevention)
/// - Zero mutex, 100% lockfree access patterns
///
/// # Tier: T4 Batch
/// - Orchestrates batch of capsules as a single unit
/// - Enables atomic snapshot of full encoder state
/// - Provides coordinated access to sub-capsules
///
/// # Layout
/// ```text
/// EncoderSubCapsules (256 bytes, cache-aligned)
/// ├─ generation: AtomicU64 (8B)          // COCA generation counter
/// ├─ state: Box<EncoderStateCapsule> (8B) // Encoder configuration + state
/// ├─ frame_buffer: Box<FrameBufferCapsule> (8B) // Frame storage + YUV
/// ├─ quantizer: Box<QuantizationCapsule> (8B) // Quantization tables
/// ├─ dct: Box<DctTransformCapsule> (8B)   // DCT/IDCT transforms
/// ├─ entropy: Box<EntropyCoderCapsule> (8B) // Entropy coding
/// ├─ tile_coord: Box<TileCoordinatorCapsule> (8B) // Tile parallelism
/// ├─ bitstream: Box<ObuBitstreamWriterCapsule> (8B) // OBU output
/// ├─ ref_frames: Box<ReferenceFrameCapsule> (8B) // Reference frame management
/// └─ _padding: [u8; 184]                  // Align to 256B
/// ```
///
/// # Safety
/// - Generation counter prevents ABA races
/// - All sub-capsules are COCA-compliant (100% lockfree)
/// - Cache-aligned to prevent false sharing
///
/// # Examples
/// ```rust,no_run
/// use kindly_av1::encoder::EncoderSubCapsules;
///
/// // Create with all sub-capsules initialized
/// let mut subs = EncoderSubCapsules::new(
///     width, height, fps, bitrate, quality
/// );
///
/// // Access individual capsules
/// let state = subs.state();
/// let frame_buffer = subs.frame_buffer_mut();
/// ```
#[repr(C, align(256))]
pub struct EncoderSubCapsules {
    /// Generation counter for COCA compliance (ABA prevention)
    generation: AtomicU64,

    /// Encoder state capsule (configuration, frame counters, etc.)
    state: Box<EncoderStateCapsule>,

    /// Frame buffer capsule (YUV storage, dimensions)
    frame_buffer: Box<FrameBufferCapsule>,

    /// Quantization capsule (Q-tables, delta-Q)
    quantizer: Box<QuantizationCapsule>,

    /// DCT transform capsule (forward/inverse DCT)
    dct: Box<DctTransformCapsule>,

    /// Entropy coder capsule (CABAC/range coding)
    entropy: Box<EntropyCoderCapsule>,

    /// Tile coordinator capsule (parallel tile encoding)
    tile_coord: Box<TileCoordinatorCapsule>,

    /// OBU bitstream writer capsule (output formatting)
    bitstream: Box<ObuBitstreamWriterCapsule>,

    /// Reference frame capsule (reference frame management)
    ref_frames: Box<ReferenceFrameCapsule>,

    /// Padding to 256 bytes (cache line alignment)
    /// 256 - 8 (generation) - 9*8 (Box pointers) = 184 bytes
    _padding: [u8; 184],
}

impl EncoderSubCapsules {
    /// Create new EncoderSubCapsules with default initialized sub-capsules
    ///
    /// # Returns
    /// Initialized EncoderSubCapsules with all sub-capsules using defaults
    ///
    /// # Examples
    /// ```rust,no_run
    /// use kindly_av1::encoder::EncoderSubCapsules;
    /// let subs = EncoderSubCapsules::new();
    /// ```
    pub fn new() -> Self {
        use atomic_capsule::encoder::SpeedPreset;
        use atomic_capsule::encoder::QualityMode;
        use atomic_capsule::encoder::frame_buffer::FrameType;

        Self {
            generation: AtomicU64::new(0),
            // EncoderStateCapsule::new(width, height, speed, quality)
            state: Box::new(EncoderStateCapsule::new(
                1920,
                1080,
                SpeedPreset::Medium,
                QualityMode::ConstantQuality,
            )),
            // FrameBufferCapsule::new(width, height, frame_type)
            frame_buffer: Box::new(FrameBufferCapsule::new(1920, 1080, FrameType::Key)),
            // QuantizationCapsule::new(quantizer_index)
            quantizer: Box::new(QuantizationCapsule::new(28)),
            dct: Box::new(DctTransformCapsule::new()),
            entropy: Box::new(EntropyCoderCapsule::new()),
            // TileCoordinatorCapsule::new(num_cols, num_rows)
            tile_coord: Box::new(TileCoordinatorCapsule::new(1, 1)),
            bitstream: Box::new(ObuBitstreamWriterCapsule::new()),
            ref_frames: Box::new(ReferenceFrameCapsule::new()),
            _padding: [0u8; 184],
        }
    }

    /// Get current generation (for COCA compliance)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Increment generation counter (called on state changes)
    #[inline]
    pub fn increment_generation(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Get immutable reference to encoder state capsule
    #[inline]
    pub fn state(&self) -> &EncoderStateCapsule {
        &self.state
    }

    /// Get mutable reference to encoder state capsule
    #[inline]
    pub fn state_mut(&mut self) -> &mut EncoderStateCapsule {
        &mut self.state
    }

    /// Get immutable reference to frame buffer capsule
    #[inline]
    pub fn frame_buffer(&self) -> &FrameBufferCapsule {
        &self.frame_buffer
    }

    /// Get mutable reference to frame buffer capsule
    #[inline]
    pub fn frame_buffer_mut(&mut self) -> &mut FrameBufferCapsule {
        &mut self.frame_buffer
    }

    /// Get immutable reference to quantization capsule
    #[inline]
    pub fn quantizer(&self) -> &QuantizationCapsule {
        &self.quantizer
    }

    /// Get mutable reference to quantization capsule
    #[inline]
    pub fn quantizer_mut(&mut self) -> &mut QuantizationCapsule {
        &mut self.quantizer
    }

    /// Get immutable reference to DCT transform capsule
    #[inline]
    pub fn dct(&self) -> &DctTransformCapsule {
        &self.dct
    }

    /// Get mutable reference to DCT transform capsule
    #[inline]
    pub fn dct_mut(&mut self) -> &mut DctTransformCapsule {
        &mut self.dct
    }

    /// Get immutable reference to entropy coder capsule
    #[inline]
    pub fn entropy(&self) -> &EntropyCoderCapsule {
        &self.entropy
    }

    /// Get mutable reference to entropy coder capsule
    #[inline]
    pub fn entropy_mut(&mut self) -> &mut EntropyCoderCapsule {
        &mut self.entropy
    }

    /// Get immutable reference to tile coordinator capsule
    #[inline]
    pub fn tile_coord(&self) -> &TileCoordinatorCapsule {
        &self.tile_coord
    }

    /// Get mutable reference to tile coordinator capsule
    #[inline]
    pub fn tile_coord_mut(&mut self) -> &mut TileCoordinatorCapsule {
        &mut self.tile_coord
    }

    /// Get immutable reference to OBU bitstream writer capsule
    #[inline]
    pub fn bitstream(&self) -> &ObuBitstreamWriterCapsule {
        &self.bitstream
    }

    /// Get mutable reference to OBU bitstream writer capsule
    #[inline]
    pub fn bitstream_mut(&mut self) -> &mut ObuBitstreamWriterCapsule {
        &mut self.bitstream
    }

    /// Get immutable reference to reference frame capsule
    #[inline]
    pub fn ref_frames(&self) -> &ReferenceFrameCapsule {
        &self.ref_frames
    }

    /// Get mutable reference to reference frame capsule
    #[inline]
    pub fn ref_frames_mut(&mut self) -> &mut ReferenceFrameCapsule {
        &mut self.ref_frames
    }
}

// Verify size at compile-time
const _: () = {
    assert!(
        core::mem::size_of::<EncoderSubCapsules>() == 256,
        "EncoderSubCapsules must be exactly 256 bytes"
    );
    assert!(
        core::mem::align_of::<EncoderSubCapsules>() == 256,
        "EncoderSubCapsules must be 256-byte aligned"
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_and_alignment() {
        assert_eq!(
            core::mem::size_of::<EncoderSubCapsules>(),
            256,
            "EncoderSubCapsules must be 256 bytes"
        );
        assert_eq!(
            core::mem::align_of::<EncoderSubCapsules>(),
            256,
            "EncoderSubCapsules must be 256-byte aligned"
        );
    }

    #[test]
    fn test_new() {
        let subs = EncoderSubCapsules::new();
        assert_eq!(subs.generation(), 0, "Initial generation should be 0");
    }

    #[test]
    fn test_generation_counter() {
        let subs = EncoderSubCapsules::new();
        assert_eq!(subs.generation(), 0);

        subs.increment_generation();
        assert_eq!(subs.generation(), 1);

        subs.increment_generation();
        assert_eq!(subs.generation(), 2);
    }

    #[test]
    fn test_accessor_methods() {
        let mut subs = EncoderSubCapsules::new();

        // Test immutable access
        let _state = subs.state();
        let _fb = subs.frame_buffer();
        let _q = subs.quantizer();
        let _dct = subs.dct();
        let _ent = subs.entropy();
        let _tile = subs.tile_coord();
        let _bits = subs.bitstream();
        let _refs = subs.ref_frames();

        // Test mutable access
        let _state_mut = subs.state_mut();
        let _fb_mut = subs.frame_buffer_mut();
        let _q_mut = subs.quantizer_mut();
        let _dct_mut = subs.dct_mut();
        let _ent_mut = subs.entropy_mut();
        let _tile_mut = subs.tile_coord_mut();
        let _bits_mut = subs.bitstream_mut();
        let _refs_mut = subs.ref_frames_mut();
    }
}
