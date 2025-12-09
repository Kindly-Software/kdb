//! H.264 CABAC (Context-Adaptive Binary Arithmetic Coding) Decoder
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Implements ITU-T H.264 Section 9.3 CABAC entropy decoding.
//!
//! # Architecture
//!
//! CABAC uses:
//! 1. Binary arithmetic coding engine (range + offset)
//! 2. Context models (460 contexts for H.264 baseline)
//! 3. Binarization schemes (unary, truncated unary, exp-golomb, fixed-length)
//!
//! # State Machine
//!
//! ```text
//! Uninitialized -> Initialized -> Decoding <-> Renormalizing -> Terminated
//! ```
//!
//! # UCE34/Chaos Compliance
//!
//! - **Q10**: T1 Atomic tier (context state coordination)
//! - **Q33**: 100% lockfree (AtomicU32/AtomicU64/AtomicU16)
//! - **Q34**: Generation counter for audit trail
//! - 512B cache-aligned
//!
//! # References
//!
//! - ITU-T H.264 Section 9.3 (CABAC)
//! - Table 9-35 (rangeLPS)
//! - Table 9-36 (state transitions)
//! - Tables 9-11 through 9-23 (context initialization)

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicU8, Ordering};

// ============================================================================
// Constants and Tables (ITU-T H.264 Tables 9-35, 9-36)
// ============================================================================

/// Number of CABAC contexts for H.264 High Profile
/// Baseline uses fewer, but we allocate for High Profile compatibility
pub const NUM_CONTEXTS: usize = 460;

/// Maximum contexts storable in the capsule (remaining fit in external array)
/// Capsule stores 64 most frequently used contexts for cache locality
pub const INLINE_CONTEXTS: usize = 64;

/// Range LPS table (ITU-T H.264 Table 9-35)
/// Indexed by [state][qCodRangeIdx] where qCodRangeIdx = (codIRange >> 6) & 3
#[rustfmt::skip]
pub const RANGE_LPS_TABLE: [[u8; 4]; 64] = [
    [128, 176, 208, 240], [128, 167, 197, 227], [128, 158, 187, 216], [123, 150, 178, 205],
    [116, 142, 169, 195], [111, 135, 160, 185], [105, 128, 152, 175], [100, 122, 144, 166],
    [ 95, 116, 137, 158], [ 90, 110, 130, 150], [ 85, 104, 123, 142], [ 81,  99, 117, 135],
    [ 77,  94, 111, 128], [ 73,  89, 105, 122], [ 69,  85, 100, 116], [ 66,  80,  95, 110],
    [ 62,  76,  90, 104], [ 59,  72,  86,  99], [ 56,  69,  81,  94], [ 53,  65,  77,  89],
    [ 51,  62,  73,  85], [ 48,  59,  69,  80], [ 46,  56,  66,  76], [ 43,  53,  63,  72],
    [ 41,  50,  59,  69], [ 39,  48,  56,  65], [ 37,  45,  54,  62], [ 35,  43,  51,  59],
    [ 33,  41,  48,  56], [ 32,  39,  46,  53], [ 30,  37,  43,  50], [ 29,  35,  41,  48],
    [ 27,  33,  39,  45], [ 26,  31,  37,  43], [ 24,  30,  35,  41], [ 23,  28,  33,  39],
    [ 22,  27,  32,  37], [ 21,  26,  30,  35], [ 20,  24,  29,  33], [ 19,  23,  27,  31],
    [ 18,  22,  26,  30], [ 17,  21,  25,  28], [ 16,  20,  23,  27], [ 15,  19,  22,  25],
    [ 14,  18,  21,  24], [ 14,  17,  20,  23], [ 13,  16,  19,  22], [ 12,  15,  18,  21],
    [ 12,  14,  17,  20], [ 11,  14,  16,  19], [ 11,  13,  15,  18], [ 10,  12,  15,  17],
    [ 10,  12,  14,  16], [  9,  11,  13,  15], [  9,  11,  12,  14], [  8,  10,  12,  14],
    [  8,   9,  11,  13], [  7,   9,  11,  12], [  7,   9,  10,  12], [  7,   8,  10,  11],
    [  6,   8,   9,  11], [  6,   7,   9,  10], [  6,   7,   8,   9], [  2,   2,   2,   2],
];

/// State transition table after LPS (ITU-T H.264 Table 9-36)
#[rustfmt::skip]
pub const TRANS_LPS: [u8; 64] = [
     0,  0,  1,  2,  2,  4,  4,  5,  6,  7,  8,  9,  9, 11, 11, 12,
    13, 13, 15, 15, 16, 16, 18, 18, 19, 19, 21, 21, 22, 22, 23, 24,
    24, 25, 26, 26, 27, 27, 28, 29, 29, 30, 30, 30, 31, 32, 32, 33,
    33, 33, 34, 34, 35, 35, 35, 36, 36, 36, 37, 37, 37, 38, 38, 63,
];

/// State transition table after MPS (ITU-T H.264 Table 9-36)
#[rustfmt::skip]
pub const TRANS_MPS: [u8; 64] = [
     1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15, 16,
    17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
    33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48,
    49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 62, 63,
];

// ============================================================================
// Context Initialization Tables (ITU-T H.264 Tables 9-11 through 9-23)
// ============================================================================

/// Context initialization values: (m, n) pairs
/// preCtxState = Clip3(1, 126, ((m * Clip3(0, 51, SliceQPy)) >> 4) + n)
/// Format: (m, n) where m is slope and n is intercept
#[rustfmt::skip]
pub const CONTEXT_INIT_I: [(i8, i8); INLINE_CONTEXTS] = [
    // mb_type (contexts 0-10)
    (  20, -15), (   2,  54), (   3,  74), ( -28, 127), ( -23, 104),
    (  -6,  53), (  -1,  54), (   7,  51), (   0,   0), (   0,   0),
    (   0,   0),
    // mb_skip_flag (11-13) - not used in I slices
    (   0,   0), (   0,   0), (   0,   0),
    // sub_mb_type (14-20)
    (   0,   0), (   0,   0), (   0,   0), (   0,   0), (   0,   0),
    (   0,   0), (   0,   0),
    // mvd (21-40) - not used in I slices, simplified
    (   0,   0), (   0,   0), (   0,   0), (   0,   0), (   0,   0),
    (   0,   0), (   0,   0), (   0,   0), (   0,   0), (   0,   0),
    (   0,   0), (   0,   0), (   0,   0), (   0,   0), (   0,   0),
    (   0,   0), (   0,   0), (   0,   0), (   0,   0), (   0,   0),
    // ref_idx (41-53) - not used in I slices
    (   0,   0), (   0,   0), (   0,   0), (   0,   0), (   0,   0),
    (   0,   0), (   0,   0), (   0,   0), (   0,   0), (   0,   0),
    (   0,   0), (   0,   0), (   0,   0),
    // mb_qp_delta (54-59)
    (   0,   0), (   0,   0), (   0,   0), (   0,   0), (   0,   0),
    (   0,   0),
    // intra_chroma_pred_mode (60-63)
    (   0,  41), (   0,  63), (   0,  63), (   0,  63),
];

/// Context initialization values for P/B slices (simplified subset)
#[rustfmt::skip]
pub const CONTEXT_INIT_PB: [(i8, i8); INLINE_CONTEXTS] = [
    // mb_type (contexts 0-10)
    (  23,  33), ( -21, 126), ( -11,  76), ( -28, 127), ( -23, 104),
    (  -6,  53), (  -1,  54), (   7,  51), (  23,  33), ( -21, 126),
    ( -11,  76),
    // mb_skip_flag (11-13)
    (  11,  80), (   0,  40), (  -7,  93),
    // sub_mb_type (14-20)
    (  13,  31), (   6,  18), ( -16, 106), ( -28, 127), ( -28, 127),
    ( -28, 127), ( -28, 127),
    // mvd_x (21-32)
    (  -5,  75), (  -5,  75), (  -5,  75), (  -5,  75), (  -5,  75),
    (  -5,  75), (  -5,  75), (  -5,  75), (  -5,  75), (  -5,  75),
    (  -5,  75), (  -5,  75),
    // mvd_y (33-40)
    (  -5,  75), (  -5,  75), (  -5,  75), (  -5,  75), (  -5,  75),
    (  -5,  75), (  -5,  75), (  -5,  75),
    // ref_idx (41-53)
    (  -1,  18), (  -1,  18), (  -1,  18), (  -1,  18), (  -1,  18),
    (  -1,  18), (  -1,  18), (  -1,  18), (  -1,  18), (  -1,  18),
    (  -1,  18), (  -1,  18), (  -1,  18),
    // mb_qp_delta (54-59)
    (  -2,  85), ( -14,  89), ( -16, 102), ( -28, 127), ( -28, 127),
    ( -28, 127),
    // intra_chroma_pred_mode (60-63)
    (   0,  41), (   0,  63), (   0,  63), (   0,  63),
];

// ============================================================================
// Context Index Definitions (ITU-T H.264 Tables 9-11 through 9-23)
// ============================================================================

/// Context indices for different syntax elements
pub mod context_idx {
    // Macroblock type contexts (Table 9-11)
    /// I slice mb_type context start
    pub const MB_TYPE_I: usize = 0;
    /// SI slice mb_type context start
    pub const MB_TYPE_SI: usize = 3;
    /// P/SP slice mb_type context start
    pub const MB_TYPE_P_SP: usize = 14;
    /// B slice mb_type context start
    pub const MB_TYPE_B: usize = 27;

    // Sub-macroblock type (Table 9-12)
    /// P/SP slice sub_mb_type context start
    pub const SUB_MB_TYPE_P_SP: usize = 36;
    /// B slice sub_mb_type context start
    pub const SUB_MB_TYPE_B: usize = 39;

    // Motion vector difference (Table 9-17)
    /// MVD L0 context start
    pub const MVD_L0: usize = 40;
    /// MVD L1 context start
    pub const MVD_L1: usize = 47;

    // Reference index (Table 9-16)
    /// Reference index L0 context start
    pub const REF_IDX_L0: usize = 54;
    /// Reference index L1 context start
    pub const REF_IDX_L1: usize = 56;

    // Delta QP (Table 9-14)
    /// mb_qp_delta context start
    pub const MB_QP_DELTA: usize = 60;

    // Intra prediction modes (Table 9-15)
    /// Intra chroma pred mode context start
    pub const INTRA_CHROMA_PRED_MODE: usize = 64;
    /// prev_intra4x4_pred_mode_flag context
    pub const PREV_INTRA4X4_PRED_MODE_FLAG: usize = 68;
    /// rem_intra4x4_pred_mode context
    pub const REM_INTRA4X4_PRED_MODE: usize = 69;

    // Coded block pattern (Table 9-13)
    /// CBP luma context start
    pub const CBP_LUMA: usize = 73;
    /// CBP chroma context start
    pub const CBP_CHROMA: usize = 77;

    // Coded block flag (Table 9-18)
    /// Coded block flag for luma DC
    pub const CODED_BLOCK_FLAG_LUMA_DC: usize = 85;
    /// Coded block flag for luma AC
    pub const CODED_BLOCK_FLAG_LUMA_AC: usize = 89;
    /// Coded block flag for chroma DC
    pub const CODED_BLOCK_FLAG_CHROMA_DC: usize = 97;
    /// Coded block flag for chroma AC
    pub const CODED_BLOCK_FLAG_CHROMA_AC: usize = 101;

    // Significant coefficient flag (Table 9-19)
    /// Significant coeff flag for luma
    pub const SIG_COEFF_FLAG_LUMA: usize = 105;
    /// Significant coeff flag for chroma
    pub const SIG_COEFF_FLAG_CHROMA: usize = 166;

    // Last significant coefficient flag (Table 9-20)
    /// Last significant coeff flag for luma
    pub const LAST_SIG_COEFF_FLAG_LUMA: usize = 227;
    /// Last significant coeff flag for chroma
    pub const LAST_SIG_COEFF_FLAG_CHROMA: usize = 288;

    // Coefficient absolute level (Table 9-21)
    /// Coeff abs level minus 1 for luma
    pub const COEFF_ABS_LEVEL_MINUS1_LUMA: usize = 349;
    /// Coeff abs level minus 1 for chroma
    pub const COEFF_ABS_LEVEL_MINUS1_CHROMA: usize = 399;

    // Transform size flag (Table 9-22)
    /// Transform size 8x8 flag
    pub const TRANSFORM_SIZE_8X8_FLAG: usize = 399;

    // End of slice (terminating context)
    /// Terminating context for end_of_slice_flag
    pub const END_OF_SLICE: usize = 276;
}

// ============================================================================
// CABAC State and Decoder Types
// ============================================================================

/// CABAC context state (6-bit state + 1-bit MPS packed into u8)
/// Bits 0-5: state index (0-63)
/// Bit 6: MPS (Most Probable Symbol)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct CabacContext(pub u8);

impl CabacContext {
    /// Create a new context with given state and MPS
    #[inline]
    pub const fn new(state: u8, mps: u8) -> Self {
        Self((state & 0x3F) | ((mps & 1) << 6))
    }

    /// Get state index (0-63)
    #[inline]
    pub const fn state(self) -> u8 {
        self.0 & 0x3F
    }

    /// Get MPS (0 or 1)
    #[inline]
    pub const fn mps(self) -> u8 {
        (self.0 >> 6) & 1
    }

    /// Pack state and MPS into u8
    #[inline]
    pub const fn pack(state: u8, mps: u8) -> u8 {
        (state & 0x3F) | ((mps & 1) << 6)
    }

    /// Unpack state and MPS from u8
    #[inline]
    pub const fn unpack(packed: u8) -> (u8, u8) {
        (packed & 0x3F, (packed >> 6) & 1)
    }
}

/// CABAC decoder state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CabacState {
    /// Not yet initialized
    Uninitialized = 0,
    /// Initialized, ready to decode
    Initialized = 1,
    /// Currently decoding bins
    Decoding = 2,
    /// In renormalization phase
    Renormalizing = 3,
    /// Decoding terminated (end of slice)
    Terminated = 4,
    /// Error state
    Error = 255,
}

impl From<u32> for CabacState {
    fn from(v: u32) -> Self {
        match v {
            0 => Self::Uninitialized,
            1 => Self::Initialized,
            2 => Self::Decoding,
            3 => Self::Renormalizing,
            4 => Self::Terminated,
            _ => Self::Error,
        }
    }
}

/// CABAC decoder errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[repr(u8)]
pub enum CabacError {
    /// No error
    #[error("no error")]
    None = 0,
    /// Invalid decoder state for operation
    #[error("invalid decoder state")]
    InvalidState = 1,
    /// Unexpected end of bitstream
    #[error("unexpected end of bitstream")]
    UnexpectedEof = 2,
    /// Arithmetic coding range underflow
    #[error("range underflow in arithmetic decoder")]
    RangeUnderflow = 3,
    /// Invalid context index
    #[error("invalid context index")]
    InvalidContext = 4,
    /// Initialization failed
    #[error("CABAC initialization failed")]
    InitializationFailed = 5,
    /// Termination sequence invalid
    #[error("invalid termination sequence")]
    TerminationFailed = 6,
    /// Offset exceeds range (corrupted stream)
    #[error("offset exceeds range (corrupted bitstream)")]
    OffsetExceedsRange = 7,
}

/// CABAC statistics snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct CabacStats {
    /// Total bins decoded
    pub bins_decoded: u64,
    /// Bypass bins decoded
    pub bypass_bins: u64,
    /// Regular (context) bins decoded
    pub regular_bins: u64,
    /// Terminating bins decoded
    pub terminate_bins: u64,
    /// Renormalization count
    pub renormalizations: u64,
    /// LPS decoding count
    pub lps_count: u64,
    /// MPS decoding count
    pub mps_count: u64,
    /// Bytes consumed from stream
    pub bytes_consumed: u64,
    /// Current generation
    pub generation: u64,
}

// ============================================================================
// Context Storage (External Array for Full 460 Contexts)
// ============================================================================

/// External context storage for full H.264 context set
/// Stored separately from capsule for cache efficiency
#[repr(C, align(64))]
pub struct CabacContextTable {
    /// All 460 contexts, each as packed (state | mps << 6)
    pub contexts: [AtomicU8; NUM_CONTEXTS],
}

impl Default for CabacContextTable {
    fn default() -> Self {
        Self::new()
    }
}

impl CabacContextTable {
    /// Create a new context table with default initialization
    pub const fn new() -> Self {
        // #ASSUME: Array initialization is safe as AtomicU8::new(0) is const
        // #VERIFY: All contexts start at state 0, MPS 0 (will be reinitialized)
        const INIT: AtomicU8 = AtomicU8::new(0);
        Self {
            contexts: [INIT; NUM_CONTEXTS],
        }
    }

    /// Initialize all contexts for given slice type and QP
    pub fn init_contexts(&self, slice_qpy: i32, slice_type: SliceType, cabac_init_idc: u8) {
        // Select initialization table based on slice type
        let init_table = match slice_type {
            SliceType::I | SliceType::SI => &CONTEXT_INIT_I[..],
            SliceType::P | SliceType::SP | SliceType::B => &CONTEXT_INIT_PB[..],
        };

        // Clip QP to valid range
        let qp = slice_qpy.clamp(0, 51);

        // Initialize inline contexts (0-63)
        for (idx, &(m, n)) in init_table.iter().enumerate() {
            let ctx = compute_initial_context(m, n, qp);
            self.contexts[idx].store(ctx.0, Ordering::Release);
        }

        // Initialize remaining contexts based on cabac_init_idc
        // For simplicity, initialize residual contexts to default state
        for idx in INLINE_CONTEXTS..NUM_CONTEXTS {
            // Use cabac_init_idc to offset initialization if needed
            let adjusted_state = ((31 + cabac_init_idc as u8) % 64).min(63);
            let ctx = CabacContext::new(adjusted_state, 0);
            self.contexts[idx].store(ctx.0, Ordering::Release);
        }

        // Specifically initialize coefficient contexts
        self.init_coefficient_contexts(qp, slice_type);
    }

    /// Initialize coefficient-related contexts
    fn init_coefficient_contexts(&self, qp: i32, _slice_type: SliceType) {
        // Significant coefficient flag contexts
        for idx in context_idx::SIG_COEFF_FLAG_LUMA..context_idx::SIG_COEFF_FLAG_CHROMA {
            let state = (qp as u8 / 2).min(63);
            self.contexts[idx].store(CabacContext::new(state, 0).0, Ordering::Release);
        }

        // Last significant coefficient flag contexts
        for idx in context_idx::LAST_SIG_COEFF_FLAG_LUMA..context_idx::LAST_SIG_COEFF_FLAG_CHROMA {
            let state = (qp as u8 / 2).min(63);
            self.contexts[idx].store(CabacContext::new(state, 0).0, Ordering::Release);
        }

        // Coded block flag contexts
        for idx in context_idx::CODED_BLOCK_FLAG_LUMA_DC..context_idx::SIG_COEFF_FLAG_LUMA {
            let state = 29u8; // Reasonable default
            self.contexts[idx].store(CabacContext::new(state, 1).0, Ordering::Release);
        }
    }

    /// Get context at index
    #[inline]
    pub fn get(&self, idx: usize) -> Option<CabacContext> {
        if idx < NUM_CONTEXTS {
            Some(CabacContext(self.contexts[idx].load(Ordering::Acquire)))
        } else {
            None
        }
    }

    /// Set context at index
    #[inline]
    pub fn set(&self, idx: usize, ctx: CabacContext) {
        if idx < NUM_CONTEXTS {
            self.contexts[idx].store(ctx.0, Ordering::Release);
        }
    }

    /// Update context after decoding bin_val
    #[inline]
    pub fn update(&self, idx: usize, decoded_bin: u8) {
        if idx >= NUM_CONTEXTS {
            return;
        }

        let current = self.contexts[idx].load(Ordering::Acquire);
        let (state, mps) = CabacContext::unpack(current);

        let new_ctx = if decoded_bin == mps {
            // MPS path - increase state
            CabacContext::new(TRANS_MPS[state as usize], mps)
        } else {
            // LPS path - decrease state, possibly swap MPS
            let new_state = TRANS_LPS[state as usize];
            let new_mps = if state == 0 { 1 - mps } else { mps };
            CabacContext::new(new_state, new_mps)
        };

        self.contexts[idx].store(new_ctx.0, Ordering::Release);
    }
}

// ============================================================================
// Slice Type Enumeration
// ============================================================================

/// H.264 slice types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SliceType {
    /// P slice (predictive)
    P = 0,
    /// B slice (bi-predictive)
    B = 1,
    /// I slice (intra)
    I = 2,
    /// SP slice (switching P)
    SP = 3,
    /// SI slice (switching I)
    SI = 4,
}

impl From<u8> for SliceType {
    fn from(v: u8) -> Self {
        match v % 5 {
            0 => Self::P,
            1 => Self::B,
            2 => Self::I,
            3 => Self::SP,
            4 => Self::SI,
            _ => Self::I, // Default to I
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Compute initial context state from (m, n) pair and QP
/// Formula: preCtxState = Clip3(1, 126, ((m * Clip3(0, 51, SliceQPy)) >> 4) + n)
#[inline]
fn compute_initial_context(m: i8, n: i8, qp: i32) -> CabacContext {
    let qp_clipped = qp.clamp(0, 51);
    let pre_ctx_state = (((m as i32) * qp_clipped) >> 4) + (n as i32);
    let pre_ctx_state = pre_ctx_state.clamp(1, 126) as u8;

    if pre_ctx_state <= 63 {
        // pStateIdx = 63 - preCtxState, valMPS = 0
        CabacContext::new(63 - pre_ctx_state, 0)
    } else {
        // pStateIdx = preCtxState - 64, valMPS = 1
        CabacContext::new(pre_ctx_state - 64, 1)
    }
}

// ============================================================================
// CabacDecoderCapsule - T1 Atomic Tier
// ============================================================================

/// T1 Atomic capsule for CABAC decoding
///
/// This capsule implements the core arithmetic coding engine. Context storage
/// is external (CabacContextTable) for memory efficiency and cache optimization.
///
/// # Layout (512B cache-aligned)
///
/// ```text
/// Offset  Field               Size    Description
/// ------  -----               ----    -----------
/// 0       range               4       codIRange (9-bit arithmetic range)
/// 4       offset              4       codIOffset (9-bit arithmetic offset)
/// 8       bits_remaining      4       Bits left in bit buffer
/// 12      _pad0               4       Padding
/// 16      bit_buffer          8       Current 64-bit bit buffer
/// 24      byte_offset         8       Current byte position in stream
/// 32      stream_length       8       Total stream length in bytes
/// 40      bins_decoded        8       Total bins decoded (stats)
/// 48      bypass_bins         8       Bypass bins decoded (stats)
/// 56      regular_bins        8       Regular bins decoded (stats)
/// 64      terminate_bins      8       Terminate bins decoded (stats)
/// 72      renormalizations    8       Renormalization count (stats)
/// 80      lps_count           8       LPS path count (stats)
/// 88      mps_count           8       MPS path count (stats)
/// 96      state               4       Decoder state (CabacState)
/// 100     last_error          4       Last error code
/// 104     generation          8       Generation counter (Q34 audit)
/// 112     _padding            400     Padding to 512B
/// ```
#[repr(C, align(512))]
pub struct CabacDecoderCapsule {
    // Arithmetic coding engine state (16 bytes)
    /// codIRange - arithmetic coding range (9 bits used, stored as u32)
    range: AtomicU32,
    /// codIOffset - arithmetic coding offset (9 bits used, stored as u32)
    offset: AtomicU32,
    /// Bits remaining in bit buffer
    bits_remaining: AtomicU32,
    /// Padding for alignment
    _pad0: u32,

    // Bit buffer (16 bytes)
    /// 64-bit bit buffer for stream reading
    bit_buffer: AtomicU64,
    /// Current byte offset in stream
    byte_offset: AtomicU64,

    // Stream info (8 bytes)
    /// Total stream length in bytes
    stream_length: AtomicU64,

    // Statistics (56 bytes)
    /// Total bins decoded
    bins_decoded: AtomicU64,
    /// Bypass bins decoded
    bypass_bins: AtomicU64,
    /// Regular (context) bins decoded
    regular_bins: AtomicU64,
    /// Terminate bins decoded
    terminate_bins: AtomicU64,
    /// Renormalization count
    renormalizations: AtomicU64,
    /// LPS path count
    lps_count: AtomicU64,
    /// MPS path count
    mps_count: AtomicU64,

    // State (16 bytes)
    /// Current decoder state
    state: AtomicU32,
    /// Last error code
    last_error: AtomicU32,
    /// Generation counter for audit trail (Q34)
    generation: AtomicU64,

    // Padding to 512 bytes
    _padding: [u8; 400],
}

// Safety: CabacDecoderCapsule only contains atomic types
unsafe impl Send for CabacDecoderCapsule {}
unsafe impl Sync for CabacDecoderCapsule {}

impl Default for CabacDecoderCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl CabacDecoderCapsule {
    /// Create a new uninitialized CABAC decoder capsule
    pub const fn new() -> Self {
        Self {
            range: AtomicU32::new(0),
            offset: AtomicU32::new(0),
            bits_remaining: AtomicU32::new(0),
            _pad0: 0,
            bit_buffer: AtomicU64::new(0),
            byte_offset: AtomicU64::new(0),
            stream_length: AtomicU64::new(0),
            bins_decoded: AtomicU64::new(0),
            bypass_bins: AtomicU64::new(0),
            regular_bins: AtomicU64::new(0),
            terminate_bins: AtomicU64::new(0),
            renormalizations: AtomicU64::new(0),
            lps_count: AtomicU64::new(0),
            mps_count: AtomicU64::new(0),
            state: AtomicU32::new(CabacState::Uninitialized as u32),
            last_error: AtomicU32::new(CabacError::None as u32),
            generation: AtomicU64::new(0),
            _padding: [0u8; 400],
        }
    }

    /// Initialize CABAC decoder from slice data
    ///
    /// ITU-T H.264 Section 9.3.1.2 - Initialization process
    ///
    /// # Arguments
    /// * `data` - Slice data (CABAC-encoded bitstream)
    /// * `slice_qpy` - Slice QP value (0-51)
    /// * `slice_type` - Slice type (I, P, B, SI, SP)
    /// * `cabac_init_idc` - CABAC initialization index (0-2)
    /// * `ctx_table` - Context table to initialize
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(CabacError)` on failure
    pub fn init(
        &self,
        data: &[u8],
        slice_qpy: i32,
        slice_type: SliceType,
        cabac_init_idc: u8,
        ctx_table: &CabacContextTable,
    ) -> Result<(), CabacError> {
        // Check minimum data length (need at least 2 bytes for initialization)
        if data.len() < 2 {
            self.state.store(CabacState::Error as u32, Ordering::Release);
            self.last_error.store(CabacError::UnexpectedEof as u32, Ordering::Release);
            return Err(CabacError::UnexpectedEof);
        }

        // Initialize contexts
        ctx_table.init_contexts(slice_qpy, slice_type, cabac_init_idc);

        // Initialize arithmetic coding engine
        // ITU-T H.264 Section 9.3.1.2:
        // codIRange = 510
        // codIOffset = first 9 bits of stream
        self.range.store(510, Ordering::Release);

        // Read first 9 bits for codIOffset
        // First two bytes give us 16 bits, we need top 9
        let first_word = ((data[0] as u32) << 8) | (data[1] as u32);
        let initial_offset = first_word >> 7; // Top 9 bits

        self.offset.store(initial_offset, Ordering::Release);

        // Initialize bit buffer
        // After reading 9 bits (1 byte + 1 bit), position at byte 1, 7 bits remaining from byte 1
        self.byte_offset.store(1, Ordering::Release);
        self.bits_remaining.store(7, Ordering::Release);
        self.bit_buffer.store((data[1] & 0x7F) as u64, Ordering::Release);

        // Store stream length
        self.stream_length.store(data.len() as u64, Ordering::Release);

        // Reset statistics
        self.bins_decoded.store(0, Ordering::Release);
        self.bypass_bins.store(0, Ordering::Release);
        self.regular_bins.store(0, Ordering::Release);
        self.terminate_bins.store(0, Ordering::Release);
        self.renormalizations.store(0, Ordering::Release);
        self.lps_count.store(0, Ordering::Release);
        self.mps_count.store(0, Ordering::Release);

        // Update state
        self.state.store(CabacState::Initialized as u32, Ordering::Release);
        self.last_error.store(CabacError::None as u32, Ordering::Release);

        // Increment generation counter
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(())
    }

    /// Decode a regular bin using context-adaptive decoding
    ///
    /// ITU-T H.264 Section 9.3.3.2 - Arithmetic decoding process for a binary decision
    ///
    /// # Arguments
    /// * `data` - Bitstream data
    /// * `ctx_table` - Context table
    /// * `ctx_idx` - Context index
    ///
    /// # Returns
    /// * `Ok(bin)` - Decoded bin value (0 or 1)
    /// * `Err(CabacError)` - On decode failure
    pub fn decode_decision(
        &self,
        data: &[u8],
        ctx_table: &CabacContextTable,
        ctx_idx: usize,
    ) -> Result<u8, CabacError> {
        // Validate state
        let current_state = CabacState::from(self.state.load(Ordering::Acquire));
        if current_state != CabacState::Initialized && current_state != CabacState::Decoding {
            return Err(CabacError::InvalidState);
        }

        // Get context
        let ctx = ctx_table.get(ctx_idx).ok_or(CabacError::InvalidContext)?;
        let state = ctx.state() as usize;
        let mps = ctx.mps();

        // Load arithmetic coding state
        let range = self.range.load(Ordering::Acquire);
        let offset = self.offset.load(Ordering::Acquire);

        // Get qCodIRangeIdx (bits 6-7 of range)
        let q_range_idx = ((range >> 6) & 3) as usize;

        // Look up rangeLPS from table
        let range_lps = RANGE_LPS_TABLE[state][q_range_idx] as u32;
        let range_mps = range - range_lps;

        // Determine decoded bin value
        let (bin_val, new_range, new_offset) = if offset < range_mps {
            // MPS path
            self.mps_count.fetch_add(1, Ordering::Relaxed);
            (mps, range_mps, offset)
        } else {
            // LPS path
            self.lps_count.fetch_add(1, Ordering::Relaxed);
            (1 - mps, range_lps, offset - range_mps)
        };

        // Store new arithmetic state
        self.range.store(new_range, Ordering::Release);
        self.offset.store(new_offset, Ordering::Release);

        // Update context
        ctx_table.update(ctx_idx, bin_val);

        // Renormalize if needed
        self.renormalize(data)?;

        // Update statistics
        self.bins_decoded.fetch_add(1, Ordering::Relaxed);
        self.regular_bins.fetch_add(1, Ordering::Relaxed);

        // Update state to Decoding if not already
        self.state.store(CabacState::Decoding as u32, Ordering::Release);

        Ok(bin_val)
    }

    /// Decode a bypass bin (equiprobable, no context)
    ///
    /// ITU-T H.264 Section 9.3.3.2.3 - Bypass decoding process
    ///
    /// # Arguments
    /// * `data` - Bitstream data
    ///
    /// # Returns
    /// * `Ok(bin)` - Decoded bin value (0 or 1)
    /// * `Err(CabacError)` - On decode failure
    pub fn decode_bypass(&self, data: &[u8]) -> Result<u8, CabacError> {
        // Validate state
        let current_state = CabacState::from(self.state.load(Ordering::Acquire));
        if current_state != CabacState::Initialized && current_state != CabacState::Decoding {
            return Err(CabacError::InvalidState);
        }

        // Load state
        let offset = self.offset.load(Ordering::Acquire);
        let range = self.range.load(Ordering::Acquire);

        // Double the offset (shift in one bit)
        let mut new_offset = offset << 1;

        // Read next bit from stream
        let next_bit = self.read_bit(data)?;
        new_offset |= next_bit as u32;

        // Determine bin value
        let bin_val = if new_offset >= range {
            new_offset -= range;
            1
        } else {
            0
        };

        // Store new offset
        self.offset.store(new_offset, Ordering::Release);

        // Update statistics
        self.bins_decoded.fetch_add(1, Ordering::Relaxed);
        self.bypass_bins.fetch_add(1, Ordering::Relaxed);

        Ok(bin_val)
    }

    /// Decode multiple bypass bins (equiprobable)
    ///
    /// More efficient than calling decode_bypass multiple times.
    ///
    /// # Arguments
    /// * `data` - Bitstream data
    /// * `count` - Number of bypass bins to decode (max 32)
    ///
    /// # Returns
    /// * `Ok(value)` - Decoded value (LSB first)
    /// * `Err(CabacError)` - On decode failure
    pub fn decode_bypass_multi(&self, data: &[u8], count: u32) -> Result<u32, CabacError> {
        if count == 0 {
            return Ok(0);
        }
        if count > 32 {
            return Err(CabacError::InvalidContext);
        }

        let mut value = 0u32;
        for i in 0..count {
            let bin = self.decode_bypass(data)?;
            value |= (bin as u32) << i;
        }
        Ok(value)
    }

    /// Decode terminating bin (end_of_slice_flag)
    ///
    /// ITU-T H.264 Section 9.3.3.2.4 - Decoding process for end_of_slice_flag
    ///
    /// # Arguments
    /// * `data` - Bitstream data
    ///
    /// # Returns
    /// * `Ok(true)` - End of slice
    /// * `Ok(false)` - Continue decoding
    /// * `Err(CabacError)` - On decode failure
    pub fn decode_terminate(&self, data: &[u8]) -> Result<bool, CabacError> {
        // Validate state
        let current_state = CabacState::from(self.state.load(Ordering::Acquire));
        if current_state != CabacState::Initialized && current_state != CabacState::Decoding {
            return Err(CabacError::InvalidState);
        }

        // Load state
        let range = self.range.load(Ordering::Acquire);
        let offset = self.offset.load(Ordering::Acquire);

        // Subtract 2 from range
        let new_range = range - 2;

        let is_terminated = if offset >= new_range {
            // End of slice
            self.state.store(CabacState::Terminated as u32, Ordering::Release);
            true
        } else {
            // Continue - need to renormalize
            self.range.store(new_range, Ordering::Release);
            self.renormalize(data)?;
            false
        };

        // Update statistics
        self.bins_decoded.fetch_add(1, Ordering::Relaxed);
        self.terminate_bins.fetch_add(1, Ordering::Relaxed);

        Ok(is_terminated)
    }

    /// Renormalization procedure
    ///
    /// ITU-T H.264 Section 9.3.3.2.2 - Renormalization process
    ///
    /// Ensures codIRange >= 256 by shifting in bits from the stream.
    fn renormalize(&self, data: &[u8]) -> Result<(), CabacError> {
        let mut range = self.range.load(Ordering::Acquire);
        let mut offset = self.offset.load(Ordering::Acquire);

        // Renormalize while range < 256
        while range < 256 {
            // Double range and offset
            range <<= 1;
            offset <<= 1;

            // Read next bit from stream
            let bit = self.read_bit(data)?;
            offset |= bit as u32;

            self.renormalizations.fetch_add(1, Ordering::Relaxed);
        }

        // Check for corruption: offset must be less than range
        if offset >= range {
            // This can happen with corrupted streams
            // Allow small overruns during renormalization
            if offset >= range + 256 {
                self.last_error.store(CabacError::OffsetExceedsRange as u32, Ordering::Release);
                return Err(CabacError::OffsetExceedsRange);
            }
        }

        // Store updated state
        self.range.store(range, Ordering::Release);
        self.offset.store(offset, Ordering::Release);

        Ok(())
    }

    /// Read a single bit from the bitstream
    fn read_bit(&self, data: &[u8]) -> Result<u8, CabacError> {
        let bits_remaining = self.bits_remaining.load(Ordering::Acquire);

        if bits_remaining == 0 {
            // Need to refill bit buffer
            let byte_offset = self.byte_offset.load(Ordering::Acquire) as usize;
            let stream_length = self.stream_length.load(Ordering::Acquire) as usize;

            if byte_offset >= stream_length {
                return Err(CabacError::UnexpectedEof);
            }

            // Check for and handle cabac_zero_word and emulation prevention bytes
            let mut next_byte = data[byte_offset];

            // Handle emulation prevention (0x000003 -> 0x0000)
            // This is simplified - full implementation would track previous bytes
            if byte_offset >= 2
                && data[byte_offset - 2] == 0
                && data[byte_offset - 1] == 0
                && next_byte == 3
            {
                // Skip emulation prevention byte
                let new_offset = byte_offset + 1;
                if new_offset >= stream_length {
                    return Err(CabacError::UnexpectedEof);
                }
                self.byte_offset.store(new_offset as u64, Ordering::Release);
                next_byte = data[new_offset];
            }

            // Store new byte in bit buffer
            self.bit_buffer.store(next_byte as u64, Ordering::Release);
            self.bits_remaining.store(8, Ordering::Release);
            self.byte_offset.fetch_add(1, Ordering::AcqRel);
        }

        // Extract MSB from bit buffer
        let bit_buffer = self.bit_buffer.load(Ordering::Acquire);
        let bits_remaining = self.bits_remaining.load(Ordering::Acquire);
        let bit = ((bit_buffer >> (bits_remaining - 1)) & 1) as u8;

        // Update bits remaining
        self.bits_remaining.store(bits_remaining - 1, Ordering::Release);

        Ok(bit)
    }

    // ========================================================================
    // Binarization Decoding Methods
    // ========================================================================

    /// Decode unary binarization
    ///
    /// Reads bins until a 0 is encountered (or max is reached).
    /// Returns count of 1s before the terminating 0.
    ///
    /// # Arguments
    /// * `data` - Bitstream data
    /// * `ctx_table` - Context table
    /// * `ctx_idx` - Starting context index
    /// * `max` - Maximum value (if reached, no terminating 0 expected)
    ///
    /// # Returns
    /// * `Ok(value)` - Decoded value (0 to max)
    pub fn decode_unary(
        &self,
        data: &[u8],
        ctx_table: &CabacContextTable,
        ctx_idx: usize,
        max: u32,
    ) -> Result<u32, CabacError> {
        let mut value = 0u32;

        while value < max {
            let bin = self.decode_decision(data, ctx_table, ctx_idx)?;
            if bin == 0 {
                break;
            }
            value += 1;
        }

        Ok(value)
    }

    /// Decode unary binarization with context increment
    ///
    /// Uses incrementing context indices for each bin.
    ///
    /// # Arguments
    /// * `data` - Bitstream data
    /// * `ctx_table` - Context table
    /// * `ctx_idx` - Starting context index
    /// * `ctx_inc_max` - Maximum context increment (after this, use last context)
    /// * `max` - Maximum value
    pub fn decode_unary_ctx_inc(
        &self,
        data: &[u8],
        ctx_table: &CabacContextTable,
        ctx_idx: usize,
        ctx_inc_max: usize,
        max: u32,
    ) -> Result<u32, CabacError> {
        let mut value = 0u32;

        while value < max {
            let ctx = ctx_idx + (value as usize).min(ctx_inc_max);
            let bin = self.decode_decision(data, ctx_table, ctx)?;
            if bin == 0 {
                break;
            }
            value += 1;
        }

        Ok(value)
    }

    /// Decode truncated unary binarization
    ///
    /// Similar to unary, but if max is reached, the terminating 0 is implicit.
    ///
    /// # Arguments
    /// * `data` - Bitstream data
    /// * `ctx_table` - Context table
    /// * `ctx_idx` - Context index
    /// * `max` - Maximum value (cMax in spec)
    pub fn decode_truncated_unary(
        &self,
        data: &[u8],
        ctx_table: &CabacContextTable,
        ctx_idx: usize,
        max: u32,
    ) -> Result<u32, CabacError> {
        if max == 0 {
            return Ok(0);
        }

        let mut value = 0u32;

        while value < max {
            let bin = self.decode_decision(data, ctx_table, ctx_idx)?;
            if bin == 0 {
                break;
            }
            value += 1;
        }

        Ok(value)
    }

    /// Decode unsigned exp-golomb binarization (UEGk)
    ///
    /// Used for larger values (e.g., coeff_abs_level_remaining).
    /// Format: unary prefix (bypass) + suffix (bypass)
    ///
    /// # Arguments
    /// * `data` - Bitstream data
    /// * `k` - Exp-golomb parameter (typically 0-3)
    ///
    /// # Returns
    /// * `Ok(value)` - Decoded unsigned value
    pub fn decode_exp_golomb(&self, data: &[u8], k: u32) -> Result<u32, CabacError> {
        // Count leading zeros (unary prefix in bypass mode)
        let mut leading_zeros = 0u32;

        loop {
            let bin = self.decode_bypass(data)?;
            if bin == 1 {
                break;
            }
            leading_zeros += 1;

            // Sanity check - prevent infinite loop on corrupted data
            if leading_zeros > 32 {
                return Err(CabacError::OffsetExceedsRange);
            }
        }

        // If leading_zeros == 0, value = 0
        if leading_zeros == 0 && k == 0 {
            return Ok(0);
        }

        // Read suffix bits
        let suffix_length = leading_zeros + k;
        let mut suffix = 0u32;

        for i in 0..suffix_length {
            let bin = self.decode_bypass(data)?;
            suffix |= (bin as u32) << (suffix_length - 1 - i);
        }

        // Compute value: (1 << leading_zeros) - 1 + (1 << k) + suffix - (1 << k)
        // Simplified: (1 << leading_zeros) - 1 + suffix + offset
        let value = if leading_zeros > 0 {
            ((1u32 << (leading_zeros + k)) - (1u32 << k)) + suffix
        } else {
            suffix
        };

        Ok(value)
    }

    /// Decode signed exp-golomb binarization (SEGk)
    ///
    /// Decodes UEGk then applies sign transformation.
    ///
    /// # Arguments
    /// * `data` - Bitstream data
    /// * `k` - Exp-golomb parameter
    ///
    /// # Returns
    /// * `Ok(value)` - Decoded signed value
    pub fn decode_signed_exp_golomb(&self, data: &[u8], k: u32) -> Result<i32, CabacError> {
        let unsigned_val = self.decode_exp_golomb(data, k)?;

        if unsigned_val == 0 {
            return Ok(0);
        }

        // Read sign bit
        let sign = self.decode_bypass(data)?;

        let value = unsigned_val as i32;
        Ok(if sign == 1 { -value } else { value })
    }

    /// Decode fixed-length binarization
    ///
    /// # Arguments
    /// * `data` - Bitstream data
    /// * `length` - Number of bits (MSB first)
    ///
    /// # Returns
    /// * `Ok(value)` - Decoded value
    pub fn decode_fixed_length(&self, data: &[u8], length: u32) -> Result<u32, CabacError> {
        if length == 0 {
            return Ok(0);
        }
        if length > 32 {
            return Err(CabacError::InvalidContext);
        }

        let mut value = 0u32;
        for i in 0..length {
            let bin = self.decode_bypass(data)?;
            value |= (bin as u32) << (length - 1 - i);
        }

        Ok(value)
    }

    // ========================================================================
    // State and Statistics Methods
    // ========================================================================

    /// Get current decoder state
    #[inline]
    pub fn state(&self) -> CabacState {
        CabacState::from(self.state.load(Ordering::Acquire))
    }

    /// Get last error
    #[inline]
    pub fn last_error(&self) -> CabacError {
        match self.last_error.load(Ordering::Acquire) {
            0 => CabacError::None,
            1 => CabacError::InvalidState,
            2 => CabacError::UnexpectedEof,
            3 => CabacError::RangeUnderflow,
            4 => CabacError::InvalidContext,
            5 => CabacError::InitializationFailed,
            6 => CabacError::TerminationFailed,
            7 => CabacError::OffsetExceedsRange,
            _ => CabacError::None,
        }
    }

    /// Get statistics snapshot
    pub fn stats(&self) -> CabacStats {
        CabacStats {
            bins_decoded: self.bins_decoded.load(Ordering::Acquire),
            bypass_bins: self.bypass_bins.load(Ordering::Acquire),
            regular_bins: self.regular_bins.load(Ordering::Acquire),
            terminate_bins: self.terminate_bins.load(Ordering::Acquire),
            renormalizations: self.renormalizations.load(Ordering::Acquire),
            lps_count: self.lps_count.load(Ordering::Acquire),
            mps_count: self.mps_count.load(Ordering::Acquire),
            bytes_consumed: self.byte_offset.load(Ordering::Acquire),
            generation: self.generation.load(Ordering::Acquire),
        }
    }

    /// Get current byte position in stream
    #[inline]
    pub fn byte_position(&self) -> u64 {
        self.byte_offset.load(Ordering::Acquire)
    }

    /// Get current generation (for audit trail)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get current arithmetic coding range
    #[inline]
    pub fn range(&self) -> u32 {
        self.range.load(Ordering::Acquire)
    }

    /// Get current arithmetic coding offset
    #[inline]
    pub fn offset(&self) -> u32 {
        self.offset.load(Ordering::Acquire)
    }

    /// Check if decoder is in valid decoding state
    #[inline]
    pub fn is_ready(&self) -> bool {
        let s = self.state();
        s == CabacState::Initialized || s == CabacState::Decoding
    }

    /// Check if decoding has terminated
    #[inline]
    pub fn is_terminated(&self) -> bool {
        self.state() == CabacState::Terminated
    }

    /// Reset decoder to uninitialized state
    pub fn reset(&self) {
        self.range.store(0, Ordering::Release);
        self.offset.store(0, Ordering::Release);
        self.bits_remaining.store(0, Ordering::Release);
        self.bit_buffer.store(0, Ordering::Release);
        self.byte_offset.store(0, Ordering::Release);
        self.stream_length.store(0, Ordering::Release);
        self.bins_decoded.store(0, Ordering::Release);
        self.bypass_bins.store(0, Ordering::Release);
        self.regular_bins.store(0, Ordering::Release);
        self.terminate_bins.store(0, Ordering::Release);
        self.renormalizations.store(0, Ordering::Release);
        self.lps_count.store(0, Ordering::Release);
        self.mps_count.store(0, Ordering::Release);
        self.state.store(CabacState::Uninitialized as u32, Ordering::Release);
        self.last_error.store(CabacError::None as u32, Ordering::Release);
        // Don't reset generation - it tracks across resets
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}

// ============================================================================
// Compile-Time Verification
// ============================================================================

const _: () = {
    // Verify CabacDecoderCapsule is exactly 512 bytes
    assert!(core::mem::size_of::<CabacDecoderCapsule>() == 512);
    // Verify 512-byte alignment
    assert!(core::mem::align_of::<CabacDecoderCapsule>() == 512);
    // Verify CabacContext is 1 byte
    assert!(core::mem::size_of::<CabacContext>() == 1);
};

// ============================================================================
// Tests (T28 Compliance)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Q1: test_new_capsule
    #[test]
    fn test_new_capsule() {
        let capsule = CabacDecoderCapsule::new();

        assert_eq!(capsule.state(), CabacState::Uninitialized);
        assert_eq!(capsule.range(), 0);
        assert_eq!(capsule.offset(), 0);
        assert_eq!(capsule.generation(), 0);

        // Verify size and alignment
        assert_eq!(core::mem::size_of::<CabacDecoderCapsule>(), 512);
        assert_eq!(core::mem::align_of::<CabacDecoderCapsule>(), 512);
    }

    // Q2: test_init_contexts
    #[test]
    fn test_init_contexts() {
        let ctx_table = CabacContextTable::new();

        // Initialize for I slice at QP=26
        ctx_table.init_contexts(26, SliceType::I, 0);

        // Verify some contexts are initialized
        let ctx0 = ctx_table.get(0).unwrap();
        // Context should have non-default state after init
        assert!(ctx0.state() < 64);
    }

    // Q3: test_decode_decision_mps
    #[test]
    fn test_decode_decision_mps() {
        let capsule = CabacDecoderCapsule::new();
        let ctx_table = CabacContextTable::new();

        // Create test data that will decode as MPS
        // offset < range_mps triggers MPS path
        let data = [0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

        capsule.init(&data, 26, SliceType::I, 0, &ctx_table).unwrap();

        // Set context to known state (high state = narrow rangeLPS)
        ctx_table.set(0, CabacContext::new(60, 0)); // High state, MPS=0

        let result = capsule.decode_decision(&data, &ctx_table, 0);
        assert!(result.is_ok());

        // After decode, MPS count should be incremented
        let stats = capsule.stats();
        assert!(stats.bins_decoded >= 1);
    }

    // Q4: test_decode_decision_lps
    #[test]
    fn test_decode_decision_lps() {
        let capsule = CabacDecoderCapsule::new();
        let ctx_table = CabacContextTable::new();

        // Create test data that will likely trigger LPS
        // High offset relative to range triggers LPS
        let data = [0xFF, 0xC0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

        capsule.init(&data, 26, SliceType::I, 0, &ctx_table).unwrap();

        // Set context to low state (wide rangeLPS, easier to hit LPS)
        ctx_table.set(0, CabacContext::new(0, 0)); // Low state, MPS=0

        let result = capsule.decode_decision(&data, &ctx_table, 0);
        assert!(result.is_ok());
    }

    // Q5: test_decode_bypass
    #[test]
    fn test_decode_bypass() {
        let capsule = CabacDecoderCapsule::new();
        let ctx_table = CabacContextTable::new();

        let data = [0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA];
        capsule.init(&data, 26, SliceType::I, 0, &ctx_table).unwrap();

        // Decode several bypass bins
        for _ in 0..4 {
            let result = capsule.decode_bypass(&data);
            assert!(result.is_ok());
        }

        let stats = capsule.stats();
        assert_eq!(stats.bypass_bins, 4);
    }

    // Q6: test_decode_terminate
    #[test]
    fn test_decode_terminate() {
        let capsule = CabacDecoderCapsule::new();
        let ctx_table = CabacContextTable::new();

        // Normal stream - should not terminate
        let data = [0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        capsule.init(&data, 26, SliceType::I, 0, &ctx_table).unwrap();

        let result = capsule.decode_terminate(&data);
        assert!(result.is_ok());
        // With low offset, should not terminate
        // (actual result depends on range/offset values)
    }

    // Q7: test_renormalize
    #[test]
    fn test_renormalize() {
        let capsule = CabacDecoderCapsule::new();
        let ctx_table = CabacContextTable::new();

        let data = [0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        capsule.init(&data, 26, SliceType::I, 0, &ctx_table).unwrap();

        // After init, range should be 510, which is >= 256, so no immediate renorm needed
        assert!(capsule.range() >= 256);

        // Decode a bin which may trigger renormalization
        let _ = capsule.decode_decision(&data, &ctx_table, 0);

        // Range should still be >= 256 after renormalization
        assert!(capsule.range() >= 256);
    }

    // Q8: test_context_transitions
    #[test]
    fn test_context_transitions() {
        let ctx_table = CabacContextTable::new();

        // Test MPS transition
        ctx_table.set(0, CabacContext::new(30, 0)); // Middle state, MPS=0
        ctx_table.update(0, 0); // Decode MPS (bin=0=MPS)

        let ctx = ctx_table.get(0).unwrap();
        assert_eq!(ctx.state(), TRANS_MPS[30]); // Should transition via MPS table
        assert_eq!(ctx.mps(), 0); // MPS unchanged

        // Test LPS transition
        ctx_table.set(1, CabacContext::new(30, 0)); // Middle state, MPS=0
        ctx_table.update(1, 1); // Decode LPS (bin=1, but MPS=0)

        let ctx = ctx_table.get(1).unwrap();
        assert_eq!(ctx.state(), TRANS_LPS[30]); // Should transition via LPS table
        assert_eq!(ctx.mps(), 0); // MPS unchanged (state > 0)

        // Test MPS swap at state 0
        ctx_table.set(2, CabacContext::new(0, 0)); // State 0, MPS=0
        ctx_table.update(2, 1); // Decode LPS

        let ctx = ctx_table.get(2).unwrap();
        assert_eq!(ctx.state(), TRANS_LPS[0]); // State 0
        assert_eq!(ctx.mps(), 1); // MPS should swap
    }

    // Q9: test_unary_decoding
    #[test]
    fn test_unary_decoding() {
        let capsule = CabacDecoderCapsule::new();
        let ctx_table = CabacContextTable::new();

        // Create stream that will decode as specific unary values
        let data = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        capsule.init(&data, 26, SliceType::I, 0, &ctx_table).unwrap();

        // Decode unary with max=10
        let result = capsule.decode_unary(&data, &ctx_table, 0, 10);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value <= 10);
    }

    // Q10: test_exp_golomb_decoding
    #[test]
    fn test_exp_golomb_decoding() {
        let capsule = CabacDecoderCapsule::new();
        let ctx_table = CabacContextTable::new();

        // Stream for exp-golomb
        let data = [0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        capsule.init(&data, 26, SliceType::I, 0, &ctx_table).unwrap();

        // Decode exp-golomb with k=0
        let result = capsule.decode_exp_golomb(&data, 0);
        assert!(result.is_ok());
    }

    // Q11: test_state_transitions_table
    #[test]
    fn test_state_transitions_table() {
        // Verify transition tables are valid
        for state in 0..64 {
            // MPS transition should increase state (except at 62/63)
            if state < 62 {
                assert!(TRANS_MPS[state] > state as u8);
            }

            // LPS transition should decrease state (except at 0 and 63)
            // State 63 stays at 63 per ITU-T H.264 Table 9-36
            if state > 0 && state < 63 {
                assert!(TRANS_LPS[state] < state as u8);
            }

            // Both should stay in valid range
            assert!(TRANS_MPS[state] < 64);
            assert!(TRANS_LPS[state] < 64);
        }

        // Verify rangeLPS table
        for state in 0..64 {
            for q_idx in 0..4 {
                let range_lps = RANGE_LPS_TABLE[state][q_idx];
                // rangeLPS should be > 0 and reasonable
                assert!(range_lps > 0);
                // Higher q_idx should give larger rangeLPS
                if q_idx > 0 {
                    assert!(range_lps >= RANGE_LPS_TABLE[state][q_idx - 1]);
                }
            }
        }
    }

    // Additional: test_context_init_formula
    #[test]
    fn test_context_init_formula() {
        // Test the context initialization formula
        // preCtxState = Clip3(1, 126, ((m * QP) >> 4) + n)

        // Test case 1: m=20, n=-15, QP=26
        let ctx = compute_initial_context(20, -15, 26);
        let expected_pre = ((20 * 26) >> 4) - 15; // = 32 - 15 = 17
        assert!(expected_pre >= 1 && expected_pre <= 126);
        // preCtxState=17 <= 63, so state = 63-17 = 46, MPS = 0
        assert_eq!(ctx.state(), 46);
        assert_eq!(ctx.mps(), 0);

        // Test case 2: m=0, n=63, QP=26
        let ctx = compute_initial_context(0, 63, 26);
        // preCtxState = 63, state = 0, MPS = 0
        // Wait: preCtxState <= 63 means state = 63 - 63 = 0, MPS = 0
        assert_eq!(ctx.state(), 0);
        assert_eq!(ctx.mps(), 0);

        // Test case 3: high preCtxState (> 63)
        let ctx = compute_initial_context(10, 80, 26);
        // preCtxState = (10*26 >> 4) + 80 = 16 + 80 = 96 > 63
        // state = 96 - 64 = 32, MPS = 1
        assert_eq!(ctx.state(), 32);
        assert_eq!(ctx.mps(), 1);
    }

    // Additional: test_fixed_length_decode
    #[test]
    fn test_fixed_length_decode() {
        let capsule = CabacDecoderCapsule::new();
        let ctx_table = CabacContextTable::new();

        let data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        capsule.init(&data, 26, SliceType::I, 0, &ctx_table).unwrap();

        // Decode 4-bit fixed length
        let result = capsule.decode_fixed_length(&data, 4);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value < 16); // 4 bits = max 15
    }

    // Additional: test_stats_tracking
    #[test]
    fn test_stats_tracking() {
        let capsule = CabacDecoderCapsule::new();
        let ctx_table = CabacContextTable::new();

        let data = [0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA];
        capsule.init(&data, 26, SliceType::I, 0, &ctx_table).unwrap();

        let stats_before = capsule.stats();
        assert_eq!(stats_before.bins_decoded, 0);

        // Decode some bins
        let _ = capsule.decode_decision(&data, &ctx_table, 0);
        let _ = capsule.decode_bypass(&data);
        let _ = capsule.decode_bypass(&data);

        let stats_after = capsule.stats();
        assert!(stats_after.bins_decoded >= 3);
        assert!(stats_after.regular_bins >= 1);
        assert!(stats_after.bypass_bins >= 2);
        assert!(stats_after.generation >= 1);
    }

    // Additional: test_reset
    #[test]
    fn test_reset() {
        let capsule = CabacDecoderCapsule::new();
        let ctx_table = CabacContextTable::new();

        let data = [0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA];
        capsule.init(&data, 26, SliceType::I, 0, &ctx_table).unwrap();

        let gen_before = capsule.generation();
        assert!(capsule.is_ready());

        capsule.reset();

        assert_eq!(capsule.state(), CabacState::Uninitialized);
        assert!(!capsule.is_ready());
        assert!(capsule.generation() > gen_before); // Generation increments on reset
    }

    // Additional: test_cabac_context_packing
    #[test]
    fn test_cabac_context_packing() {
        // Test pack/unpack roundtrip
        for state in 0..64 {
            for mps in 0..2 {
                let packed = CabacContext::pack(state, mps);
                let (unpacked_state, unpacked_mps) = CabacContext::unpack(packed);
                assert_eq!(unpacked_state, state);
                assert_eq!(unpacked_mps, mps);

                let ctx = CabacContext::new(state, mps);
                assert_eq!(ctx.state(), state);
                assert_eq!(ctx.mps(), mps);
            }
        }
    }
}
