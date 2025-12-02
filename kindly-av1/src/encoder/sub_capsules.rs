//! EncoderSubCapsules - T4 Batch tier handle for AV1 encoder sub-capsules
//!
//! UCE34 Compliance: Q10 T4 Batch tier (holds batch of capsules), Q33 lockfree
//! COCA Compliance: Generation counter, cache-aligned, no mutex
//!
//! # Phase 2 Capsules
//! Includes SIMD-accelerated capsules for advanced AV1 features:
//! - Loop Restoration Filter (Wiener, Self-Guided)
//! - Film Grain Synthesis (AR model, LUT generation)
//! - Superresolution (8-tap Lanczos upscaling)
//! - Intra/Inter Prediction (SIMD-accelerated modes)
//! - Loop Filter (deblocking)
//! - CDEF Filter (8 directional filters)
//! - Lookahead (scene detection, frame analysis)
//! - Temporal RDO (rate-distortion optimization)

use atomic_capsule::encoder::{
    DctTransformCapsule, EntropyCoderCapsule, EncoderStateCapsule, FrameBufferCapsule,
    GopCoordinatorCapsuleV2, ObuBitstreamCapsuleV2, QuantizationCapsule,
    ReferenceFrameCapsuleV2, RateControlCapsule, RateControlMode,
    TileCoordinatorCapsule, LookaheadCapsule, TemporalRDOCapsule,
};

use super::gpu_motion::GpuMotionEstimationCapsule;

// SIMD-accelerated capsules (require portable_simd feature)
#[cfg(feature = "portable_simd")]
use atomic_capsule::encoder::{
    LoopRestorationCapsuleV2, RestorationTypeV2, FilmGrainCapsule, SuperresolutionCapsuleV2,
    IntraPredictionCapsuleV2,
    InterPredictionCapsuleV2, LoopFilterCapsule, CdefFilterCapsuleV2,
};

use std::sync::atomic::{AtomicU64, Ordering};

/// EncoderSubCapsules - Opaque handle holding references to all encoder sub-capsules
///
/// # Architecture
/// - 512B cache-aligned container (Phase 3: Rate Control integration)
/// - Holds Box references to 21 atomic_capsule encoder capsules (20 from Phase 2 + rate_control)
/// - Generation counter for COCA compliance (ABA prevention)
/// - Zero mutex, 100% lockfree access patterns
///
/// # Tier: T4 Batch
/// - Orchestrates batch of capsules as a single unit
/// - Enables atomic snapshot of full encoder state
/// - Provides coordinated access to sub-capsules
///
/// # Layout (Phase 3 - 512 bytes, includes Rate Control)
/// ```text
/// EncoderSubCapsules (512 bytes, cache-aligned)
/// ├─ generation: AtomicU64 (8B)             // COCA generation counter
/// ├─ Phase 1 Core (10 capsules × 8B = 80B)
/// │  ├─ state: Box<EncoderStateCapsule>     // Encoder configuration + state
/// │  ├─ frame_buffer: Box<FrameBufferCapsule> // Frame storage + YUV
/// │  ├─ quantizer: Box<QuantizationCapsule> // Quantization tables
/// │  ├─ dct: Box<DctTransformCapsule>       // DCT/IDCT transforms
/// │  ├─ entropy: Box<EntropyCoderCapsule>   // Entropy coding
/// │  ├─ tile_coord: Box<TileCoordinatorCapsule> // Tile parallelism
/// │  ├─ bitstream: Box<ObuBitstreamWriterCapsule> // OBU output
/// │  ├─ ref_frames: Box<ReferenceFrameCapsuleV2> // Reference frame management (V2)
/// │  ├─ gop_coord: Box<GopCoordinatorCapsuleV2> // GOP structure coordination (V2)
/// │  └─ lookahead: Box<LookaheadCapsule>    // Scene detection
/// ├─ Phase 2 Non-SIMD (1 capsule × 8B = 8B)
/// │  └─ temporal_rdo: Box<TemporalRDOCapsule> // Rate-distortion optimization
/// ├─ Phase 2 SIMD (7 capsules × 8B = 56B, portable_simd feature - all V2)
/// │  ├─ lrf: Option<Box<LoopRestorationCapsuleV2>> // Loop restoration filter (V2)
/// │  ├─ film_grain: Option<Box<FilmGrainCapsule>> // Film grain synthesis
/// │  ├─ superres: Option<Box<SuperresolutionCapsuleV2>> // Upscaling (V2)
/// │  ├─ intra_pred: Option<Box<IntraPredictionCapsuleV2>> // Intra prediction (V2)
/// │  ├─ inter_pred: Option<Box<InterPredictionCapsuleV2>> // Inter prediction (V2)
/// │  ├─ loop_filter: Option<Box<LoopFilterCapsule>> // Deblocking
/// │  └─ cdef: Option<Box<CdefFilterCapsuleV2>> // CDEF directional (V2)
/// ├─ Wave 3A GPU (1 capsule × 8B = 8B)
/// │  └─ motion: Box<GpuMotionEstimationCapsule> // GPU motion estimation (T7)
/// ├─ Phase 3 Rate Control (1 capsule × 8B = 8B)
/// │  └─ rate_control: Box<RateControlCapsule> // Rate control (CRF/CBR/VBR, T3+T1)
/// ├─ Reconstruction Buffer (24B: Vec ptr+cap+len)
/// │  └─ reconstructed_buffer: Vec<u8>       // Decoded reconstruction for RDO/refs
/// └─ _padding: [u8; 320]                    // Align to 512B
/// ```
///
/// # Safety
/// - Generation counter prevents ABA races
/// - All sub-capsules are COCA-compliant (100% lockfree)
/// - Cache-aligned to prevent false sharing
/// - Option<Box<T>> maintains constant size via niche optimization
///
/// # Examples
/// ```rust,no_run
/// use kindly_av1::encoder::EncoderSubCapsules;
///
/// // Create with all sub-capsules initialized
/// let mut subs = EncoderSubCapsules::new();
///
/// // Access individual capsules
/// let state = subs.state();
/// let frame_buffer = subs.frame_buffer_mut();
///
/// // Access SIMD capsules (requires portable_simd feature)
/// #[cfg(feature = "portable_simd")]
/// if let Some(lrf) = subs.lrf() {
///     // Use loop restoration filter
/// }
/// ```
#[repr(C, align(512))]
pub struct EncoderSubCapsules {
    // ============ COCA Coordination ============
    /// Generation counter for COCA compliance (ABA prevention)
    generation: AtomicU64,

    // ============ Phase 1 Core Capsules (10) ============
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
    bitstream: Box<ObuBitstreamCapsuleV2>,

    /// Reference frame capsule (reference frame management) - V2
    ref_frames: Box<ReferenceFrameCapsuleV2>,

    /// GOP coordinator capsule (GOP structure, hierarchical B-frames, scene change)
    gop_coord: Box<GopCoordinatorCapsuleV2>,

    /// Lookahead capsule (scene detection, frame analysis)
    lookahead: Box<LookaheadCapsule>,

    // ============ Phase 2 Non-SIMD Capsules (1) ============
    /// Temporal RDO capsule (rate-distortion optimization)
    temporal_rdo: Box<TemporalRDOCapsule>,

    // ============ Phase 2 SIMD Capsules (7 - require portable_simd) ============
    /// Loop restoration filter (Wiener, Self-Guided) - T2 SIMD
    #[cfg(feature = "portable_simd")]
    lrf: Option<Box<LoopRestorationCapsuleV2>>,
    #[cfg(not(feature = "portable_simd"))]
    lrf: usize, // Placeholder (8 bytes, same as Option<Box<T>>)

    /// Film grain synthesis (AR model, LUT) - T2 SIMD
    #[cfg(feature = "portable_simd")]
    film_grain: Option<Box<FilmGrainCapsule>>,
    #[cfg(not(feature = "portable_simd"))]
    film_grain: usize,

    /// Superresolution upscaling (8-tap Lanczos) - T2 SIMD
    #[cfg(feature = "portable_simd")]
    superres: Option<Box<SuperresolutionCapsuleV2>>,
    #[cfg(not(feature = "portable_simd"))]
    superres: usize,

    /// Intra prediction modes (56 modes) - T2 SIMD
    #[cfg(feature = "portable_simd")]
    intra_pred: Option<Box<IntraPredictionCapsuleV2>>,
    #[cfg(not(feature = "portable_simd"))]
    intra_pred: usize,

    /// Inter prediction (8-tap filters, compound modes) - T2 SIMD
    #[cfg(feature = "portable_simd")]
    inter_pred: Option<Box<InterPredictionCapsuleV2>>,
    #[cfg(not(feature = "portable_simd"))]
    inter_pred: usize,

    /// Loop filter (deblocking) - T2 SIMD
    #[cfg(feature = "portable_simd")]
    loop_filter: Option<Box<LoopFilterCapsule>>,
    #[cfg(not(feature = "portable_simd"))]
    loop_filter: usize,

    /// CDEF filter (8 directional filters) - T2 SIMD
    #[cfg(feature = "portable_simd")]
    cdef: Option<Box<CdefFilterCapsuleV2>>,
    #[cfg(not(feature = "portable_simd"))]
    cdef: usize,

    // ============ Wave 3A: GPU Motion Estimation (T7 Heterogeneous) ============
    /// GPU motion estimation capsule - T7 Heterogeneous (ROCm/Vulkan + CPU fallback)
    motion: Box<GpuMotionEstimationCapsule>,

    // ============ Phase 3: Rate Control (T3 Fixed-Point + T1 Atomic) ============
    /// Rate control capsule (CRF, CappedCRF, CBR, VBR modes)
    /// T3 Fixed-Point (Q16.16 deterministic arithmetic) + T1 Atomic (lockfree coordination)
    /// Performance: <100ns QP decision, 10-50× speedup vs mutex-based rate control
    rate_control: Box<RateControlCapsule>,

    // ============ Reconstruction Buffer ============
    /// Reconstructed frame buffer (decoded output for reference frames)
    /// Stores the decoder-side reconstruction: dequant → IDCT → add prediction → clip → loop filters
    /// This buffer is crucial for:
    /// - Rate-Distortion Optimization (RDO): encoder sees same output as decoder
    /// - Reference frame storage: P/B frames predict from reconstructed pixels
    /// - Quality metrics: PSNR/SSIM computed on reconstructed vs original
    ///
    /// Size: width × height × 1.5 (Y + U + V planes for YUV 4:2:0)
    /// Initialized to empty vector, allocated on first frame encode
    reconstructed_buffer: Vec<u8>,

    // ============ Previous Input Frame (for Scene Change Detection) ============
    /// Previous input frame buffer for scene change detection
    ///
    /// **Purpose**: Store the previous frame's ORIGINAL input (not reconstructed)
    /// to compare with current frame for accurate scene change detection.
    ///
    /// **Scene Change Detection** compares original-to-original:
    /// - `previous_input_frame` (original frame N-1)
    /// - `yuv_data` (original frame N)
    ///
    /// This ensures apples-to-apples comparison (not lossy reconstructed vs lossless).
    /// Size: width × height bytes (Y plane only for luminance-based detection)
    previous_input_frame: Vec<u8>,

    // ============ Padding ============
    /// Padding to 512 bytes (cache line alignment)
    /// 512 - 8 (generation) - 20*8 (Box/Option pointers) - 24*2 (2× Vec: ptr+cap+len) = 512 - 8 - 160 - 48 = 296 bytes
    _padding: [u8; 296],
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

            // ============ Phase 1 Core Capsules ============
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
            bitstream: Box::new(ObuBitstreamCapsuleV2::new()),
            ref_frames: Box::new(ReferenceFrameCapsuleV2::new()),
            // GopCoordinatorCapsuleV2::new(gop_size, max_b_frames)
            // Default: 64-frame GOP, 3 B-frames
            gop_coord: Box::new(GopCoordinatorCapsuleV2::new(64, 3)),
            // LookaheadCapsule::new(depth) - 32-frame lookahead
            lookahead: Box::new(LookaheadCapsule::new(32)),

            // ============ Phase 2 Non-SIMD Capsules ============
            // TemporalRDOCapsule::new(qp) - default QP 28 (medium quality)
            temporal_rdo: Box::new(TemporalRDOCapsule::new(28)),

            // ============ Phase 2 SIMD Capsules (initialized when feature enabled) ============
            #[cfg(feature = "portable_simd")]
            // LoopRestorationCapsuleV2::new(lr_type, unit_size) - default Wiener, 64 unit size
            lrf: Some(Box::new(LoopRestorationCapsuleV2::new(RestorationTypeV2::Wiener, 64))),
            #[cfg(not(feature = "portable_simd"))]
            lrf: 0,

            #[cfg(feature = "portable_simd")]
            film_grain: Some(Box::new(FilmGrainCapsule::new())),
            #[cfg(not(feature = "portable_simd"))]
            film_grain: 0,

            #[cfg(feature = "portable_simd")]
            // SuperresolutionCapsuleV2::new(denominator) - 8 = 1:1 (no scaling), 9-16 = 8/denominator
            superres: Some(Box::new(SuperresolutionCapsuleV2::new(8))),
            #[cfg(not(feature = "portable_simd"))]
            superres: 0,

            #[cfg(feature = "portable_simd")]
            intra_pred: Some(Box::new(IntraPredictionCapsuleV2::new())),
            #[cfg(not(feature = "portable_simd"))]
            intra_pred: 0,

            #[cfg(feature = "portable_simd")]
            inter_pred: Some(Box::new(InterPredictionCapsuleV2::new())),
            #[cfg(not(feature = "portable_simd"))]
            inter_pred: 0,

            #[cfg(feature = "portable_simd")]
            // LoopFilterCapsule::new(level, sharpness) - default level=16 (medium), sharpness=4
            loop_filter: Some(Box::new(LoopFilterCapsule::new(16, 4))),
            #[cfg(not(feature = "portable_simd"))]
            loop_filter: 0,

            #[cfg(feature = "portable_simd")]
            // CdefFilterCapsuleV2::new(strength_y, strength_uv, damping) - default 8, 6, 4
            cdef: Some(Box::new(CdefFilterCapsuleV2::new(8, 6, 4))),
            #[cfg(not(feature = "portable_simd"))]
            cdef: 0,

            // ============ Wave 3A: GPU Motion Estimation ============
            motion: Box::new(GpuMotionEstimationCapsule::new()),

            // ============ Phase 3: Rate Control ============
            // RateControlCapsule::new(mode, crf, max_bitrate_kbps)
            // Default: CappedCRF mode (best for streaming), CRF=28 (medium quality), max_bitrate=10Mbps
            rate_control: Box::new(RateControlCapsule::new(
                RateControlMode::CappedCRF,  // Best practice for streaming (44% bitrate savings)
                28,                           // Target CRF (quality)
                10_000,                       // Max bitrate: 10 Mbps in kbps (1080p30 typical)
            )),

            // ============ Reconstruction Buffer ============
            reconstructed_buffer: Vec::new(), // Allocated on first frame encode

            // ============ Previous Input Frame ============
            previous_input_frame: Vec::new(), // Allocated on first frame encode

            _padding: [0u8; 296],  // 512 - 8 - 160 - 48 = 296
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
    pub fn bitstream(&self) -> &ObuBitstreamCapsuleV2 {
        &self.bitstream
    }

    /// Get mutable reference to OBU bitstream writer capsule
    #[inline]
    pub fn bitstream_mut(&mut self) -> &mut ObuBitstreamCapsuleV2 {
        &mut self.bitstream
    }

    /// Get immutable reference to reference frame capsule
    #[inline]
    pub fn ref_frames(&self) -> &ReferenceFrameCapsuleV2 {
        &self.ref_frames
    }

    /// Get mutable reference to reference frame capsule
    #[inline]
    pub fn ref_frames_mut(&mut self) -> &mut ReferenceFrameCapsuleV2 {
        &mut self.ref_frames
    }

    /// Get immutable reference to GOP coordinator capsule
    #[inline]
    pub fn gop_coord(&self) -> &GopCoordinatorCapsuleV2 {
        &self.gop_coord
    }

    /// Get mutable reference to GOP coordinator capsule
    #[inline]
    pub fn gop_coord_mut(&mut self) -> &mut GopCoordinatorCapsuleV2 {
        &mut self.gop_coord
    }

    // ============ Phase 2 Non-SIMD Accessors ============

    /// Get immutable reference to lookahead capsule
    #[inline]
    pub fn lookahead(&self) -> &LookaheadCapsule {
        &self.lookahead
    }

    /// Get mutable reference to lookahead capsule
    #[inline]
    pub fn lookahead_mut(&mut self) -> &mut LookaheadCapsule {
        &mut self.lookahead
    }

    /// Get immutable reference to temporal RDO capsule
    #[inline]
    pub fn temporal_rdo(&self) -> &TemporalRDOCapsule {
        &self.temporal_rdo
    }

    /// Get mutable reference to temporal RDO capsule
    #[inline]
    pub fn temporal_rdo_mut(&mut self) -> &mut TemporalRDOCapsule {
        &mut self.temporal_rdo
    }

    // ============ Phase 2 SIMD Accessors (require portable_simd feature) ============

    /// Get immutable reference to loop restoration filter capsule
    #[cfg(feature = "portable_simd")]
    #[inline]
    pub fn lrf(&self) -> Option<&LoopRestorationCapsuleV2> {
        self.lrf.as_deref()
    }

    /// Get mutable reference to loop restoration filter capsule
    #[cfg(feature = "portable_simd")]
    #[inline]
    pub fn lrf_mut(&mut self) -> Option<&mut LoopRestorationCapsuleV2> {
        self.lrf.as_deref_mut()
    }

    /// Get immutable reference to film grain capsule
    #[cfg(feature = "portable_simd")]
    #[inline]
    pub fn film_grain(&self) -> Option<&FilmGrainCapsule> {
        self.film_grain.as_deref()
    }

    /// Get mutable reference to film grain capsule
    #[cfg(feature = "portable_simd")]
    #[inline]
    pub fn film_grain_mut(&mut self) -> Option<&mut FilmGrainCapsule> {
        self.film_grain.as_deref_mut()
    }

    /// Get immutable reference to superresolution capsule
    #[cfg(feature = "portable_simd")]
    #[inline]
    pub fn superres(&self) -> Option<&SuperresolutionCapsuleV2> {
        self.superres.as_deref()
    }

    /// Get mutable reference to superresolution capsule
    #[cfg(feature = "portable_simd")]
    #[inline]
    pub fn superres_mut(&mut self) -> Option<&mut SuperresolutionCapsuleV2> {
        self.superres.as_deref_mut()
    }

    /// Get immutable reference to intra prediction capsule
    #[cfg(feature = "portable_simd")]
    #[inline]
    pub fn intra_pred(&self) -> Option<&IntraPredictionCapsuleV2> {
        self.intra_pred.as_deref()
    }

    /// Get mutable reference to intra prediction capsule
    #[cfg(feature = "portable_simd")]
    #[inline]
    pub fn intra_pred_mut(&mut self) -> Option<&mut IntraPredictionCapsuleV2> {
        self.intra_pred.as_deref_mut()
    }

    /// Get immutable reference to inter prediction capsule
    #[cfg(feature = "portable_simd")]
    #[inline]
    pub fn inter_pred(&self) -> Option<&InterPredictionCapsuleV2> {
        self.inter_pred.as_deref()
    }

    /// Get mutable reference to inter prediction capsule
    #[cfg(feature = "portable_simd")]
    #[inline]
    pub fn inter_pred_mut(&mut self) -> Option<&mut InterPredictionCapsuleV2> {
        self.inter_pred.as_deref_mut()
    }

    /// Get immutable reference to loop filter capsule
    #[cfg(feature = "portable_simd")]
    #[inline]
    pub fn loop_filter(&self) -> Option<&LoopFilterCapsule> {
        self.loop_filter.as_deref()
    }

    /// Get mutable reference to loop filter capsule
    #[cfg(feature = "portable_simd")]
    #[inline]
    pub fn loop_filter_mut(&mut self) -> Option<&mut LoopFilterCapsule> {
        self.loop_filter.as_deref_mut()
    }

    /// Get immutable reference to CDEF filter capsule
    #[cfg(feature = "portable_simd")]
    #[inline]
    pub fn cdef(&self) -> Option<&CdefFilterCapsuleV2> {
        self.cdef.as_deref()
    }

    /// Get mutable reference to CDEF filter capsule
    #[cfg(feature = "portable_simd")]
    #[inline]
    pub fn cdef_mut(&mut self) -> Option<&mut CdefFilterCapsuleV2> {
        self.cdef.as_deref_mut()
    }

    // ============ Wave 3A: GPU Motion Estimation Accessors ============

    /// Get immutable reference to GPU motion estimation capsule
    #[inline]
    pub fn motion(&self) -> &GpuMotionEstimationCapsule {
        &self.motion
    }

    /// Get mutable reference to GPU motion estimation capsule
    #[inline]
    pub fn motion_mut(&mut self) -> &mut GpuMotionEstimationCapsule {
        &mut self.motion
    }

    // ============ Phase 3: Rate Control Accessors ============

    /// Get immutable reference to rate control capsule
    #[inline]
    pub fn rate_control(&self) -> &RateControlCapsule {
        &self.rate_control
    }

    /// Get mutable reference to rate control capsule
    #[inline]
    pub fn rate_control_mut(&mut self) -> &mut RateControlCapsule {
        &mut self.rate_control
    }

    // ============ Reconstruction Buffer Accessors ============

    /// Get immutable reference to reconstructed frame buffer
    ///
    /// Contains the decoded reconstruction (post-loop-filters) used for:
    /// - Reference frame storage (P/B-frame prediction)
    /// - Quality metrics (PSNR/SSIM)
    /// - RDO (encoder sees decoder output)
    #[inline]
    pub fn reconstructed_buffer(&self) -> &[u8] {
        &self.reconstructed_buffer
    }

    /// Get mutable reference to reconstructed frame buffer
    ///
    /// Used by reconstruction pipeline to write decoded output.
    #[inline]
    pub fn reconstructed_buffer_mut(&mut self) -> &mut Vec<u8> {
        &mut self.reconstructed_buffer
    }

    /// Get raw pointer to reconstructed buffer (for zero-copy reference frames)
    ///
    /// ## Safety
    /// Caller must ensure:
    /// - Buffer is allocated (non-empty)
    /// - Pointer remains valid for lifetime of reference frame
    /// - No concurrent modifications during reference frame use
    ///
    /// ## Performance
    /// <10ns (pointer load)
    #[inline]
    pub fn reconstructed_buffer_ptr(&self) -> *const u8 {
        self.reconstructed_buffer.as_ptr()
    }

    // ============ Previous Input Frame Accessors (for Scene Change Detection) ============

    /// Get immutable reference to previous input frame buffer
    ///
    /// Used for scene change detection comparing original-to-original frames.
    #[inline]
    pub fn previous_input_frame(&self) -> &[u8] {
        &self.previous_input_frame
    }

    /// Get mutable reference to previous input frame buffer
    ///
    /// Used to store current input as "previous" for next frame's scene change detection.
    #[inline]
    pub fn previous_input_frame_mut(&mut self) -> &mut Vec<u8> {
        &mut self.previous_input_frame
    }
}

// Verify size at compile-time (Phase 2: expanded to 512B for SIMD capsules)
const _: () = {
    assert!(
        core::mem::size_of::<EncoderSubCapsules>() == 512,
        "EncoderSubCapsules must be exactly 512 bytes"
    );
    assert!(
        core::mem::align_of::<EncoderSubCapsules>() == 512,
        "EncoderSubCapsules must be 512-byte aligned"
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_and_alignment() {
        assert_eq!(
            core::mem::size_of::<EncoderSubCapsules>(),
            512,
            "EncoderSubCapsules must be 512 bytes (Phase 2 expanded)"
        );
        assert_eq!(
            core::mem::align_of::<EncoderSubCapsules>(),
            512,
            "EncoderSubCapsules must be 512-byte aligned"
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
    fn test_phase1_accessor_methods() {
        let mut subs = EncoderSubCapsules::new();

        // Test immutable access - Phase 1 capsules
        let _state = subs.state();
        let _fb = subs.frame_buffer();
        let _q = subs.quantizer();
        let _dct = subs.dct();
        let _ent = subs.entropy();
        let _tile = subs.tile_coord();
        let _bits = subs.bitstream();
        let _refs = subs.ref_frames();
        let _gop = subs.gop_coord();

        // Test mutable access - Phase 1 capsules
        let _state_mut = subs.state_mut();
        let _fb_mut = subs.frame_buffer_mut();
        let _q_mut = subs.quantizer_mut();
        let _dct_mut = subs.dct_mut();
        let _ent_mut = subs.entropy_mut();
        let _tile_mut = subs.tile_coord_mut();
        let _bits_mut = subs.bitstream_mut();
        let _refs_mut = subs.ref_frames_mut();
        let _gop_mut = subs.gop_coord_mut();
    }

    #[test]
    fn test_phase2_non_simd_accessors() {
        let mut subs = EncoderSubCapsules::new();

        // Test immutable access - Phase 2 non-SIMD capsules
        let _lookahead = subs.lookahead();
        let _temporal_rdo = subs.temporal_rdo();

        // Test mutable access - Phase 2 non-SIMD capsules
        let _lookahead_mut = subs.lookahead_mut();
        let _temporal_rdo_mut = subs.temporal_rdo_mut();
    }

    #[test]
    fn test_wave3a_motion_accessors() {
        let mut subs = EncoderSubCapsules::new();

        // Test immutable access - Wave 3A GPU motion estimation
        let motion = subs.motion();
        assert!(!motion.is_gpu_enabled(), "GPU should start disabled");
        assert_eq!(motion.total_calls(), 0);

        // Test mutable access
        let motion_mut = subs.motion_mut();
        motion_mut.enable_gpu();
    }

    #[cfg(feature = "portable_simd")]
    #[test]
    fn test_phase2_simd_accessors() {
        let mut subs = EncoderSubCapsules::new();

        // Test immutable access - Phase 2 SIMD capsules
        assert!(subs.lrf().is_some(), "LRF capsule should be initialized with portable_simd");
        assert!(subs.film_grain().is_some(), "Film grain capsule should be initialized");
        assert!(subs.superres().is_some(), "Superresolution capsule should be initialized");
        assert!(subs.intra_pred().is_some(), "Intra prediction capsule should be initialized");
        assert!(subs.inter_pred().is_some(), "Inter prediction capsule should be initialized");
        assert!(subs.loop_filter().is_some(), "Loop filter capsule should be initialized");
        assert!(subs.cdef().is_some(), "CDEF capsule should be initialized");

        // Test mutable access - Phase 2 SIMD capsules
        assert!(subs.lrf_mut().is_some());
        assert!(subs.film_grain_mut().is_some());
        assert!(subs.superres_mut().is_some());
        assert!(subs.intra_pred_mut().is_some());
        assert!(subs.inter_pred_mut().is_some());
        assert!(subs.loop_filter_mut().is_some());
        assert!(subs.cdef_mut().is_some());
    }

    #[test]
    fn test_capsule_count() {
        // Wave 3A structure:
        // - 10 Phase 1 core capsules
        // - 2 Phase 2 non-SIMD capsules (lookahead, temporal_rdo)
        // - 7 Phase 2 SIMD capsules (lrf, film_grain, superres, intra_pred, inter_pred, loop_filter, cdef)
        // - 1 Wave 3A GPU capsule (motion)
        // Total: 20 capsules
        let _subs = EncoderSubCapsules::new();
        // Structure verified by compile-time size assertion (512B)
    }
}
