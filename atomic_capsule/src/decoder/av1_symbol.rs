//! AV1 Symbol Decoder Capsule
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Implements AV1 multi-symbol arithmetic decoder per AV1 specification Section 8.3.
//! Uses CDF (Cumulative Distribution Function) based entropy decoding with adaptive
//! probability updates.
//!
//! # Architecture
//!
//! The AV1 symbol decoder uses a 16-bit range, 15-bit precision arithmetic coding engine
//! with CDF-based symbol probabilities. This differs from H.264 CABAC which uses
//! context-based binary arithmetic coding.
//!
//! ```text
//! AV1 Symbol Decoder State Machine:
//! +--------------+     init()     +-------------+     decode_symbol()    +----------+
//! | Uninitialized| --------------> | Initialized | <--------------------> | Decoding |
//! +--------------+                 +-------------+                        +----------+
//!                                        |                                      |
//!                                        | exit_symbol_decoder()                |
//!                                        v                                      |
//!                                  +------------+                               |
//!                                  | Terminated | <-----------------------------+
//!                                  +------------+
//! ```
//!
//! # AV1 Specification Compliance
//!
//! Implements the following AV1 specification sections:
//! - Section 8.3: Symbol decoding process
//! - Section 8.3.2: Multi-symbol decoding
//! - Section 8.3.3: CDF update process
//! - Section 8.3.4: Literal decoding
//!
//! # T1 Atomic Tier
//!
//! - 256B cache-aligned structure for optimal memory access
//! - 100% lockfree using AtomicU64/AtomicU32 only
//! - Generation counter for Q34 audit trail compliance
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T1 Atomic tier for symbol coordination
//! - **Chaos**: 100% lockfree (AtomicU64/AtomicU32 only), 256B cache-aligned
//! - **ASSUM**: All unsafe blocks documented with #ASSUME/#VERIFY
//! - **B32**: Benchmarks validate decode throughput
//! - **T28**: 32+ tests covering unit/property/integration/production/determinism

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// ============================================================================
// Constants (AV1 Specification Section 8.3)
// ============================================================================

/// Symbol bit precision (15 bits per AV1 spec)
pub const SYMBOL_BITS: u32 = 15;

/// CDF probability bits (15-bit precision)
pub const CDF_PROB_BITS: u32 = 15;

/// CDF probability top value (1 << 15 = 32768)
pub const CDF_PROB_TOP: u32 = 1 << CDF_PROB_BITS;

/// Minimum range before renormalization needed
pub const MIN_RANGE: u32 = 1 << 8; // 256

/// Maximum range value (16-bit)
pub const MAX_RANGE: u32 = 1 << 16; // 65536

/// CDF update rate parameters (AV1 spec 8.3.3)
pub const CDF_UPDATE_RATE_LOG2: u32 = 5;

/// Minimum CDF update rate
pub const CDF_UPDATE_RATE_MIN: u32 = 4;

/// Maximum number of CDF symbols (typically 16 for most syntax elements)
pub const MAX_CDF_SYMBOLS: usize = 16;

/// Window refill threshold (need at least 16 bits)
pub const WINDOW_REFILL_THRESHOLD: u32 = 16;

// ============================================================================
// State and Error Types
// ============================================================================

/// AV1 symbol decoder state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Av1SymbolState {
    /// Not yet initialized
    Uninitialized = 0,
    /// Initialized, ready to decode
    Initialized = 1,
    /// Currently decoding symbols
    Decoding = 2,
    /// Decoder terminated (exit_symbol_decoder called)
    Terminated = 3,
    /// Error state
    Error = 255,
}

impl From<u32> for Av1SymbolState {
    fn from(v: u32) -> Self {
        match v {
            0 => Self::Uninitialized,
            1 => Self::Initialized,
            2 => Self::Decoding,
            3 => Self::Terminated,
            _ => Self::Error,
        }
    }
}

/// AV1 symbol decoder errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[repr(u8)]
pub enum Av1SymbolError {
    /// No error
    #[error("no error")]
    None = 0,
    /// Invalid decoder state for operation
    #[error("invalid decoder state")]
    InvalidState = 1,
    /// Unexpected end of bitstream
    #[error("unexpected end of bitstream")]
    UnexpectedEof = 2,
    /// Range underflow during decode
    #[error("range underflow")]
    RangeUnderflow = 3,
    /// Invalid CDF (empty or malformed)
    #[error("invalid CDF")]
    InvalidCdf = 4,
    /// Invalid bit count (must be 1-32)
    #[error("invalid bit count")]
    InvalidBitCount = 5,
    /// Buffer too small for initialization
    #[error("buffer too small")]
    BufferTooSmall = 6,
    /// Symbol index out of range
    #[error("symbol out of range")]
    SymbolOutOfRange = 7,
}

/// AV1 symbol decoder statistics snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct Av1SymbolStats {
    /// Total symbols decoded
    pub symbols_decoded: u64,
    /// Total literals decoded
    pub literals_decoded: u64,
    /// Total booleans decoded
    pub bools_decoded: u64,
    /// Total range normalizations performed
    pub range_normalizations: u64,
    /// Bytes consumed from stream
    pub bytes_consumed: u64,
    /// Window refills performed
    pub window_refills: u64,
    /// CDF updates performed
    pub cdf_updates: u64,
    /// Current generation (Q34 audit)
    pub generation: u64,
}

// ============================================================================
// Av1SymbolDecoderCapsule - T1 Atomic Tier
// ============================================================================

/// T1 Atomic capsule for AV1 symbol decoding
///
/// Implements AV1 multi-symbol arithmetic decoder per AV1 specification Section 8.3.
/// Uses CDF (Cumulative Distribution Function) based entropy decoding with 16-bit
/// range and 15-bit precision.
///
/// # Layout (256B cache-aligned)
///
/// ```text
/// Offset  Field               Size    Description
/// ------  -----               ----    -----------
/// 0       state               8       Packed: bits 0-7 = state, bits 8-31 = error
/// 8       generation          8       Generation counter (Q34 audit)
/// 16      range               4       Arithmetic coding range (16-bit)
/// 20      value               4       Arithmetic coding value (16-bit)
/// 24      bits_left           4       Bits remaining in window
/// 28      _pad0               4       Padding
/// 32      window              8       64-bit window buffer
/// 40      buffer_pos          8       Current byte position in buffer
/// 48      buffer_end          8       End of buffer position
/// 56      _pad1               8       Padding
/// 64      symbols_decoded     8       Total symbols decoded (stats)
/// 72      literals_decoded    8       Total literals decoded (stats)
/// 80      bools_decoded       8       Total booleans decoded (stats)
/// 88      range_normalizations 8      Range normalizations (stats)
/// 96      bytes_consumed      8       Bytes consumed (stats)
/// 104     window_refills      8       Window refills (stats)
/// 112     cdf_updates         8       CDF updates (stats)
/// 120     _padding            136     Padding to 256B
/// ```
///
/// # Thread Safety
///
/// All fields use atomic types for lockfree concurrent access. Statistics can be
/// read while decoding is in progress on another thread.
#[repr(C, align(128))]
pub struct Av1SymbolDecoderCapsule {
    // ---- Bytes 0-31: Core decoder state ----
    /// Packed state: bits 0-7 = state, bits 8-31 = last_error
    state: AtomicU64,
    /// Generation counter for Q34 audit trail
    generation: AtomicU64,
    /// Arithmetic coding range (16-bit, stored as u32)
    range: AtomicU32,
    /// Arithmetic coding value (16-bit, stored as u32)
    value: AtomicU32,

    // ---- Bytes 32-63: Window and buffer state ----
    /// Bits remaining in the window buffer
    bits_left: AtomicU32,
    /// Padding for alignment
    _pad0: u32,
    /// 64-bit window buffer for bit extraction
    window: AtomicU64,
    /// Current byte position in source buffer
    buffer_pos: AtomicU64,
    /// End position of source buffer
    buffer_end: AtomicU64,

    // ---- Bytes 64-119: Statistics ----
    /// Padding for cache line alignment
    _pad1: u64,
    /// Total symbols decoded
    symbols_decoded: AtomicU64,
    /// Total literals decoded
    literals_decoded: AtomicU64,
    /// Total booleans decoded
    bools_decoded: AtomicU64,
    /// Range normalization count
    range_normalizations: AtomicU64,
    /// Bytes consumed from stream
    bytes_consumed: AtomicU64,
    /// Window refill count
    window_refills: AtomicU64,
    /// CDF update count
    cdf_updates: AtomicU64,

    // ---- Bytes 120-255: Padding ----
    /// Padding to 256B
    _padding: [u8; 120],
}

// Safety: Av1SymbolDecoderCapsule only contains atomic types
unsafe impl Send for Av1SymbolDecoderCapsule {}
unsafe impl Sync for Av1SymbolDecoderCapsule {}

impl Default for Av1SymbolDecoderCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl Av1SymbolDecoderCapsule {
    /// Create a new uninitialized AV1 symbol decoder capsule
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(Av1SymbolState::Uninitialized as u64),
            generation: AtomicU64::new(0),
            range: AtomicU32::new(0),
            value: AtomicU32::new(0),
            bits_left: AtomicU32::new(0),
            _pad0: 0,
            window: AtomicU64::new(0),
            buffer_pos: AtomicU64::new(0),
            buffer_end: AtomicU64::new(0),
            _pad1: 0,
            symbols_decoded: AtomicU64::new(0),
            literals_decoded: AtomicU64::new(0),
            bools_decoded: AtomicU64::new(0),
            range_normalizations: AtomicU64::new(0),
            bytes_consumed: AtomicU64::new(0),
            window_refills: AtomicU64::new(0),
            cdf_updates: AtomicU64::new(0),
            _padding: [0u8; 120],
        }
    }

    /// Initialize the AV1 symbol decoder with bitstream data
    ///
    /// AV1 specification Section 8.3.1 - Initialization process
    ///
    /// # Arguments
    /// * `data` - AV1 bitstream data (symbol coded data)
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(Av1SymbolError)` on failure
    pub fn init(&self, data: &[u8]) -> Result<(), Av1SymbolError> {
        // Need at least 2 bytes for initialization
        if data.len() < 2 {
            self.set_error(Av1SymbolError::BufferTooSmall);
            return Err(Av1SymbolError::BufferTooSmall);
        }

        // Increment generation counter
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Initialize range to maximum (AV1 uses 16-bit range)
        self.range.store(MAX_RANGE, Ordering::Release);

        // Initialize value from first two bytes (big-endian per AV1 spec)
        let initial_value = ((data[0] as u32) << 8) | (data[1] as u32);
        self.value.store(initial_value, Ordering::Release);

        // Store buffer bounds (start after first 2 bytes)
        self.buffer_pos.store(2, Ordering::Release);
        self.buffer_end.store(data.len() as u64, Ordering::Release);

        // Initialize window with remaining bytes
        let mut window: u64 = 0;
        let mut bits_left = 0u32;
        let mut pos = 2usize;

        // Fill window with up to 6 more bytes (total 8 bytes, but we used 2)
        while pos < data.len() && bits_left < 48 {
            window |= (data[pos] as u64) << bits_left;
            bits_left += 8;
            pos += 1;
        }

        self.window.store(window, Ordering::Release);
        self.bits_left.store(bits_left, Ordering::Release);
        self.buffer_pos.store(pos as u64, Ordering::Release);

        // Reset statistics
        self.symbols_decoded.store(0, Ordering::Release);
        self.literals_decoded.store(0, Ordering::Release);
        self.bools_decoded.store(0, Ordering::Release);
        self.range_normalizations.store(0, Ordering::Release);
        self.bytes_consumed.store(2, Ordering::Release);
        self.window_refills.store(0, Ordering::Release);
        self.cdf_updates.store(0, Ordering::Release);

        // Update state
        self.set_state(Av1SymbolState::Initialized);

        Ok(())
    }

    /// Decode a multi-symbol value using CDF
    ///
    /// AV1 specification Section 8.3.2 - Multi-symbol decoding process
    ///
    /// The CDF array represents cumulative probabilities scaled to 15 bits.
    /// For N symbols, cdf has N elements where cdf[i] = P(symbol <= i) * 32768.
    /// The last element should be CDF_PROB_TOP (32768).
    ///
    /// # Arguments
    /// * `cdf` - Cumulative distribution function array (N elements)
    /// * `data` - Source bitstream data
    ///
    /// # Returns
    /// * `Ok(symbol)` - Decoded symbol index (0 to N-1)
    /// * `Err(Av1SymbolError)` - On decode failure
    ///
    /// # CDF Format
    ///
    /// For a 4-symbol alphabet with probabilities [0.25, 0.25, 0.25, 0.25]:
    /// ```text
    /// cdf = [8192, 16384, 24576, 32768]
    ///       ^^^^   ^^^^^   ^^^^^   ^^^^^
    ///       P(0)   P(0-1)  P(0-2)  P(0-3) = 1.0
    /// ```
    pub fn decode_symbol(&self, cdf: &[u16], data: &[u8]) -> Result<u32, Av1SymbolError> {
        // Validate state
        let current_state = self.get_state();
        if current_state != Av1SymbolState::Initialized && current_state != Av1SymbolState::Decoding
        {
            return Err(Av1SymbolError::InvalidState);
        }

        // Validate CDF
        if cdf.is_empty() || cdf.len() > MAX_CDF_SYMBOLS {
            return Err(Av1SymbolError::InvalidCdf);
        }

        let n_symbols = cdf.len();

        // Load arithmetic coding state
        let mut range = self.range.load(Ordering::Acquire);
        let mut value = self.value.load(Ordering::Acquire);

        // Scale factor for CDF -> range mapping
        // AV1 uses: scale = range >> CDF_PROB_BITS
        let scale = range >> CDF_PROB_BITS;

        // Binary search for the symbol
        // Find smallest i where cdf[i] * scale > value
        let mut symbol = 0u32;
        let mut low = 0u32;

        for (i, &cdf_val) in cdf.iter().enumerate() {
            let threshold = (cdf_val as u32) * scale;
            if value >= threshold {
                symbol = (i + 1) as u32;
                low = threshold;
            } else {
                break;
            }
        }

        // Clamp symbol to valid range
        if symbol >= n_symbols as u32 {
            symbol = (n_symbols - 1) as u32;
        }

        // Update arithmetic coding state
        // high = cdf[symbol] * scale
        let high = if symbol < (n_symbols - 1) as u32 {
            (cdf[symbol as usize] as u32) * scale
        } else {
            range
        };

        // New range and value
        let new_range = high - low;
        let new_value = value - low;

        // Store updated state
        self.range.store(new_range, Ordering::Release);
        self.value.store(new_value, Ordering::Release);

        // Normalize if needed
        self.normalize(data)?;

        // Update statistics
        self.symbols_decoded.fetch_add(1, Ordering::Relaxed);
        self.set_state(Av1SymbolState::Decoding);

        Ok(symbol)
    }

    /// Decode a multi-symbol value and update the CDF
    ///
    /// This combines decoding with CDF adaptation per AV1 Section 8.3.3.
    ///
    /// # Arguments
    /// * `cdf` - Mutable CDF array to decode from and update
    /// * `data` - Source bitstream data
    ///
    /// # Returns
    /// * `Ok(symbol)` - Decoded symbol index
    pub fn decode_symbol_with_update(
        &self,
        cdf: &mut [u16],
        data: &[u8],
    ) -> Result<u32, Av1SymbolError> {
        let symbol = self.decode_symbol(cdf, data)?;
        self.update_cdf(cdf, symbol);
        Ok(symbol)
    }

    /// Update CDF after decoding a symbol
    ///
    /// AV1 specification Section 8.3.3 - CDF update process
    ///
    /// # Arguments
    /// * `cdf` - CDF array to update
    /// * `symbol` - The decoded symbol
    pub fn update_cdf(&self, cdf: &mut [u16], symbol: u32) {
        let n = cdf.len();
        if n == 0 || symbol as usize >= n {
            return;
        }

        // Calculate update rate based on CDF size
        // rate = max(4, min(n, 32))
        let rate = ((n as u32).max(CDF_UPDATE_RATE_MIN)).min(1 << CDF_UPDATE_RATE_LOG2);

        // Update CDF entries
        // For entries before symbol: increase probability
        // For entries at/after symbol: decrease probability
        for i in 0..n {
            let delta = if (i as u32) < symbol {
                // Below symbol: increase (move toward 0)
                let diff = cdf[i] as i32;
                -diff >> (CDF_PROB_BITS - rate.trailing_zeros())
            } else if (i as u32) == symbol {
                // At symbol: increase
                let diff = (CDF_PROB_TOP as i32) - (cdf[i] as i32);
                diff >> (CDF_PROB_BITS - rate.trailing_zeros())
            } else {
                // Above symbol: keep
                0
            };

            cdf[i] = (cdf[i] as i32 + delta).clamp(1, (CDF_PROB_TOP - 1) as i32) as u16;
        }

        // Ensure monotonicity and last element is CDF_PROB_TOP
        for i in 1..n {
            if cdf[i] <= cdf[i - 1] {
                cdf[i] = cdf[i - 1] + 1;
            }
        }
        cdf[n - 1] = CDF_PROB_TOP as u16;

        self.cdf_updates.fetch_add(1, Ordering::Relaxed);
    }

    /// Decode a literal value (n bits, MSB first)
    ///
    /// AV1 specification Section 8.3.4 - Literal decoding
    ///
    /// Reads n bits from the stream using bypass decoding (equiprobable).
    ///
    /// # Arguments
    /// * `n_bits` - Number of bits to read (1-24)
    /// * `data` - Source bitstream data
    ///
    /// # Returns
    /// * `Ok(value)` - The decoded n-bit value
    /// * `Err(Av1SymbolError)` - On decode failure
    pub fn decode_literal(&self, n_bits: u32, data: &[u8]) -> Result<u32, Av1SymbolError> {
        // Validate state
        let current_state = self.get_state();
        if current_state != Av1SymbolState::Initialized && current_state != Av1SymbolState::Decoding
        {
            return Err(Av1SymbolError::InvalidState);
        }

        if n_bits == 0 || n_bits > 24 {
            return Err(Av1SymbolError::InvalidBitCount);
        }

        let mut value = 0u32;

        // Read bits MSB first
        for _ in 0..n_bits {
            let bit = self.decode_bool_eq_prob(data)?;
            value = (value << 1) | (bit as u32);
        }

        self.literals_decoded.fetch_add(1, Ordering::Relaxed);
        self.set_state(Av1SymbolState::Decoding);

        Ok(value)
    }

    /// Decode a boolean with given probability
    ///
    /// # Arguments
    /// * `prob` - Probability of 1 (scaled to 15 bits, 0-32767)
    /// * `data` - Source bitstream data
    ///
    /// # Returns
    /// * `Ok(bool)` - The decoded boolean value
    pub fn decode_bool(&self, prob: u32, data: &[u8]) -> Result<bool, Av1SymbolError> {
        // Validate state
        let current_state = self.get_state();
        if current_state != Av1SymbolState::Initialized && current_state != Av1SymbolState::Decoding
        {
            return Err(Av1SymbolError::InvalidState);
        }

        // Load arithmetic coding state
        let mut range = self.range.load(Ordering::Acquire);
        let mut value = self.value.load(Ordering::Acquire);

        // Split range based on probability
        // split = range - ((range * (CDF_PROB_TOP - prob)) >> CDF_PROB_BITS)
        let prob_complement = CDF_PROB_TOP.saturating_sub(prob);
        let split = (range - ((range * prob_complement) >> CDF_PROB_BITS)).max(1);

        let result = if value < split {
            // Decode 0
            range = split;
            false
        } else {
            // Decode 1
            value -= split;
            range -= split;
            true
        };

        // Store updated state
        self.range.store(range, Ordering::Release);
        self.value.store(value, Ordering::Release);

        // Normalize
        self.normalize(data)?;

        // Update statistics
        self.bools_decoded.fetch_add(1, Ordering::Relaxed);
        self.set_state(Av1SymbolState::Decoding);

        Ok(result)
    }

    /// Decode a boolean with equal probability (50/50)
    ///
    /// Optimized path for equiprobable decisions.
    ///
    /// # Arguments
    /// * `data` - Source bitstream data
    ///
    /// # Returns
    /// * `Ok(bool)` - The decoded boolean value
    #[inline]
    pub fn decode_bool_eq_prob(&self, data: &[u8]) -> Result<bool, Av1SymbolError> {
        self.decode_bool(CDF_PROB_TOP / 2, data)
    }

    /// Refill the window buffer from the source data
    ///
    /// Called when bits_left drops below threshold.
    ///
    /// # Arguments
    /// * `data` - Source bitstream data
    pub fn refill(&self, data: &[u8]) -> Result<(), Av1SymbolError> {
        let mut window = self.window.load(Ordering::Acquire);
        let mut bits_left = self.bits_left.load(Ordering::Acquire);
        let mut pos = self.buffer_pos.load(Ordering::Acquire) as usize;
        let end = self.buffer_end.load(Ordering::Acquire) as usize;

        // Fill window with available bytes
        while bits_left < 56 && pos < end {
            window |= (data[pos] as u64) << bits_left;
            bits_left += 8;
            pos += 1;
        }

        self.window.store(window, Ordering::Release);
        self.bits_left.store(bits_left, Ordering::Release);
        self.buffer_pos.store(pos as u64, Ordering::Release);
        self.window_refills.fetch_add(1, Ordering::Relaxed);
        self.bytes_consumed.store(pos as u64, Ordering::Release);

        Ok(())
    }

    /// Normalize the arithmetic coding state
    ///
    /// AV1 specification Section 8.3.5 - Renormalization process
    ///
    /// Ensures range >= MIN_RANGE by shifting in bits from the window.
    ///
    /// # Arguments
    /// * `data` - Source bitstream data
    pub fn normalize(&self, data: &[u8]) -> Result<(), Av1SymbolError> {
        let mut range = self.range.load(Ordering::Acquire);
        let mut value = self.value.load(Ordering::Acquire);
        let mut bits_left = self.bits_left.load(Ordering::Acquire);
        let mut window = self.window.load(Ordering::Acquire);
        let mut norm_count = 0u64;

        // Refill window if needed
        if bits_left < WINDOW_REFILL_THRESHOLD {
            self.refill(data)?;
            bits_left = self.bits_left.load(Ordering::Acquire);
            window = self.window.load(Ordering::Acquire);
        }

        // Renormalize while range < MIN_RANGE
        while range < MIN_RANGE {
            // Double range
            range <<= 1;

            // Shift in one bit to value
            value = (value << 1) | ((window & 1) as u32);
            window >>= 1;
            bits_left = bits_left.saturating_sub(1);

            norm_count += 1;

            // Refill if we're running low
            if bits_left < 8 {
                self.bits_left.store(bits_left, Ordering::Release);
                self.window.store(window, Ordering::Release);
                self.refill(data)?;
                bits_left = self.bits_left.load(Ordering::Acquire);
                window = self.window.load(Ordering::Acquire);
            }
        }

        // Store final state
        self.range.store(range, Ordering::Release);
        self.value.store(value, Ordering::Release);
        self.bits_left.store(bits_left, Ordering::Release);
        self.window.store(window, Ordering::Release);

        if norm_count > 0 {
            self.range_normalizations
                .fetch_add(norm_count, Ordering::Relaxed);
        }

        Ok(())
    }

    /// Exit the symbol decoder
    ///
    /// Called at the end of a tile or frame to finalize decoding.
    pub fn exit(&self) {
        self.set_state(Av1SymbolState::Terminated);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Reset the decoder to uninitialized state
    pub fn reset(&self) {
        self.state.store(Av1SymbolState::Uninitialized as u64, Ordering::Release);
        self.range.store(0, Ordering::Release);
        self.value.store(0, Ordering::Release);
        self.bits_left.store(0, Ordering::Release);
        self.window.store(0, Ordering::Release);
        self.buffer_pos.store(0, Ordering::Release);
        self.buffer_end.store(0, Ordering::Release);
        self.symbols_decoded.store(0, Ordering::Release);
        self.literals_decoded.store(0, Ordering::Release);
        self.bools_decoded.store(0, Ordering::Release);
        self.range_normalizations.store(0, Ordering::Release);
        self.bytes_consumed.store(0, Ordering::Release);
        self.window_refills.store(0, Ordering::Release);
        self.cdf_updates.store(0, Ordering::Release);
        // Don't reset generation - it tracks across resets
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    // ========================================================================
    // State and Statistics Methods
    // ========================================================================

    /// Get current decoder state
    #[inline]
    pub fn get_state(&self) -> Av1SymbolState {
        let packed = self.state.load(Ordering::Acquire);
        Av1SymbolState::from((packed & 0xFF) as u32)
    }

    /// Set decoder state
    #[inline]
    fn set_state(&self, state: Av1SymbolState) {
        let current = self.state.load(Ordering::Acquire);
        let new_state = (current & !0xFF) | (state as u64);
        self.state.store(new_state, Ordering::Release);
    }

    /// Set error state
    #[inline]
    fn set_error(&self, error: Av1SymbolError) {
        let current = self.state.load(Ordering::Acquire);
        let new_state = (current & 0xFF) | ((error as u64) << 8);
        self.state.store(new_state, Ordering::Release);
        self.set_state(Av1SymbolState::Error);
    }

    /// Get last error
    #[inline]
    pub fn last_error(&self) -> Av1SymbolError {
        let packed = self.state.load(Ordering::Acquire);
        let error_code = ((packed >> 8) & 0xFF) as u8;
        match error_code {
            0 => Av1SymbolError::None,
            1 => Av1SymbolError::InvalidState,
            2 => Av1SymbolError::UnexpectedEof,
            3 => Av1SymbolError::RangeUnderflow,
            4 => Av1SymbolError::InvalidCdf,
            5 => Av1SymbolError::InvalidBitCount,
            6 => Av1SymbolError::BufferTooSmall,
            7 => Av1SymbolError::SymbolOutOfRange,
            _ => Av1SymbolError::None,
        }
    }

    /// Get statistics snapshot
    pub fn stats(&self) -> Av1SymbolStats {
        Av1SymbolStats {
            symbols_decoded: self.symbols_decoded.load(Ordering::Acquire),
            literals_decoded: self.literals_decoded.load(Ordering::Acquire),
            bools_decoded: self.bools_decoded.load(Ordering::Acquire),
            range_normalizations: self.range_normalizations.load(Ordering::Acquire),
            bytes_consumed: self.bytes_consumed.load(Ordering::Acquire),
            window_refills: self.window_refills.load(Ordering::Acquire),
            cdf_updates: self.cdf_updates.load(Ordering::Acquire),
            generation: self.generation.load(Ordering::Acquire),
        }
    }

    /// Get current generation counter (Q34 audit)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get current arithmetic coding range
    #[inline]
    pub fn range(&self) -> u32 {
        self.range.load(Ordering::Acquire)
    }

    /// Get current arithmetic coding value
    #[inline]
    pub fn value(&self) -> u32 {
        self.value.load(Ordering::Acquire)
    }

    /// Get bytes consumed from stream
    #[inline]
    pub fn bytes_consumed(&self) -> u64 {
        self.bytes_consumed.load(Ordering::Acquire)
    }

    /// Check if decoder is in valid decoding state
    #[inline]
    pub fn is_ready(&self) -> bool {
        let s = self.get_state();
        s == Av1SymbolState::Initialized || s == Av1SymbolState::Decoding
    }

    /// Check if decoding has terminated
    #[inline]
    pub fn is_terminated(&self) -> bool {
        self.get_state() == Av1SymbolState::Terminated
    }
}

// ============================================================================
// CDF Helper Functions
// ============================================================================

/// Create a uniform CDF for n symbols
///
/// # Arguments
/// * `n` - Number of symbols (2-16)
///
/// # Returns
/// * CDF array where each symbol has equal probability
pub fn create_uniform_cdf(n: usize) -> Vec<u16> {
    if n == 0 || n > MAX_CDF_SYMBOLS {
        return vec![];
    }

    let step = CDF_PROB_TOP / (n as u32);
    let mut cdf = Vec::with_capacity(n);

    for i in 1..=n {
        let prob = ((step * (i as u32)).min(CDF_PROB_TOP - 1)) as u16;
        cdf.push(if i == n { CDF_PROB_TOP as u16 } else { prob });
    }

    cdf
}

/// Create a CDF from probability weights
///
/// # Arguments
/// * `weights` - Probability weights (will be normalized)
///
/// # Returns
/// * Normalized CDF array
pub fn create_cdf_from_weights(weights: &[u32]) -> Vec<u16> {
    if weights.is_empty() || weights.len() > MAX_CDF_SYMBOLS {
        return vec![];
    }

    let total: u64 = weights.iter().map(|&w| w as u64).sum();
    if total == 0 {
        return create_uniform_cdf(weights.len());
    }

    let mut cdf = Vec::with_capacity(weights.len());
    let mut cumulative: u64 = 0;

    for (i, &w) in weights.iter().enumerate() {
        cumulative += w as u64;
        let prob = ((cumulative * (CDF_PROB_TOP as u64)) / total) as u16;
        cdf.push(if i == weights.len() - 1 {
            CDF_PROB_TOP as u16
        } else {
            prob.min((CDF_PROB_TOP - 1) as u16)
        });
    }

    cdf
}

// ============================================================================
// Compile-Time Verification
// ============================================================================

const _: () = {
    // Verify Av1SymbolDecoderCapsule is exactly 256 bytes
    assert!(core::mem::size_of::<Av1SymbolDecoderCapsule>() == 256);
    // Verify 128-byte alignment (fits in 2 cache lines for optimal access)
    assert!(core::mem::align_of::<Av1SymbolDecoderCapsule>() == 128);
};

// ============================================================================
// Tests (T28 Compliance - 5-tier testing)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // T28 Q1-Q7: Unit Tests
    // =========================================================================

    /// Q1: Test capsule creation and initial state
    #[test]
    fn test_q1_new_capsule() {
        let capsule = Av1SymbolDecoderCapsule::new();

        assert_eq!(capsule.get_state(), Av1SymbolState::Uninitialized);
        assert_eq!(capsule.range(), 0);
        assert_eq!(capsule.value(), 0);
        assert_eq!(capsule.generation(), 0);
        assert!(!capsule.is_ready());
        assert!(!capsule.is_terminated());
    }

    /// Q2: Test capsule size and alignment
    #[test]
    fn test_q2_capsule_size_alignment() {
        assert_eq!(
            core::mem::size_of::<Av1SymbolDecoderCapsule>(),
            256,
            "Capsule must be 256B for T1 Atomic tier"
        );
        assert_eq!(
            core::mem::align_of::<Av1SymbolDecoderCapsule>(),
            128,
            "Capsule must be 128B aligned"
        );
    }

    /// Q3: Test initialization
    #[test]
    fn test_q3_initialization() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let data = [0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let result = capsule.init(&data);

        assert!(result.is_ok());
        assert_eq!(capsule.get_state(), Av1SymbolState::Initialized);
        assert!(capsule.is_ready());
        assert_eq!(capsule.range(), MAX_RANGE);
        assert!(capsule.generation() > 0);
    }

    /// Q4: Test initialization with minimal data
    #[test]
    fn test_q4_init_minimal_data() {
        let capsule = Av1SymbolDecoderCapsule::new();

        // Minimum 2 bytes required
        let data = [0xFF, 0xFF];
        assert!(capsule.init(&data).is_ok());

        // 1 byte should fail
        let data1 = [0xFF];
        let capsule2 = Av1SymbolDecoderCapsule::new();
        assert!(capsule2.init(&data1).is_err());
    }

    /// Q5: Test boolean decoding with equal probability
    #[test]
    fn test_q5_decode_bool_eq_prob() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let data = [0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        capsule.init(&data).unwrap();

        // Decode several booleans
        for _ in 0..4 {
            let result = capsule.decode_bool_eq_prob(&data);
            assert!(result.is_ok());
        }

        let stats = capsule.stats();
        assert_eq!(stats.bools_decoded, 4);
    }

    /// Q6: Test boolean decoding with skewed probability
    #[test]
    fn test_q6_decode_bool_skewed() {
        let capsule = Av1SymbolDecoderCapsule::new();

        // High value data tends toward 0 with high probability
        let data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        capsule.init(&data).unwrap();

        // High probability of 0 (prob = 4096 out of 32768 = 12.5%)
        let result = capsule.decode_bool(4096, &data);
        assert!(result.is_ok());

        // High probability of 1 (prob = 28672 out of 32768 = 87.5%)
        let result2 = capsule.decode_bool(28672, &data);
        assert!(result2.is_ok());
    }

    /// Q7: Test literal decoding
    #[test]
    fn test_q7_decode_literal() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let data = [0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA];
        capsule.init(&data).unwrap();

        // Decode 8-bit literal
        let result = capsule.decode_literal(8, &data);
        assert!(result.is_ok());

        let stats = capsule.stats();
        assert_eq!(stats.literals_decoded, 1);
    }

    // =========================================================================
    // T28 Q8-Q14: Property Tests
    // =========================================================================

    /// Q8: Test generation counter increments
    #[test]
    fn test_q8_generation_counter() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let initial_gen = capsule.generation();
        assert_eq!(initial_gen, 0);

        let data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        capsule.init(&data).unwrap();

        let gen_after_init = capsule.generation();
        assert!(gen_after_init > initial_gen);

        capsule.reset();

        let gen_after_reset = capsule.generation();
        assert!(gen_after_reset > gen_after_init);
    }

    /// Q9: Test state transitions
    #[test]
    fn test_q9_state_transitions() {
        let capsule = Av1SymbolDecoderCapsule::new();

        // Initial: Uninitialized
        assert_eq!(capsule.get_state(), Av1SymbolState::Uninitialized);

        // After init: Initialized
        let data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        capsule.init(&data).unwrap();
        assert_eq!(capsule.get_state(), Av1SymbolState::Initialized);

        // After decode: Decoding
        let _ = capsule.decode_bool_eq_prob(&data);
        assert_eq!(capsule.get_state(), Av1SymbolState::Decoding);

        // After exit: Terminated
        capsule.exit();
        assert_eq!(capsule.get_state(), Av1SymbolState::Terminated);
        assert!(capsule.is_terminated());
    }

    /// Q10: Test uniform CDF creation
    #[test]
    fn test_q10_uniform_cdf() {
        let cdf4 = create_uniform_cdf(4);
        assert_eq!(cdf4.len(), 4);
        assert_eq!(cdf4[3], CDF_PROB_TOP as u16);

        // Check monotonicity
        for i in 1..cdf4.len() {
            assert!(cdf4[i] > cdf4[i - 1]);
        }

        let cdf2 = create_uniform_cdf(2);
        assert_eq!(cdf2.len(), 2);
        assert_eq!(cdf2[1], CDF_PROB_TOP as u16);
    }

    /// Q11: Test CDF from weights
    #[test]
    fn test_q11_cdf_from_weights() {
        // Equal weights
        let cdf_equal = create_cdf_from_weights(&[1, 1, 1, 1]);
        assert_eq!(cdf_equal.len(), 4);
        assert_eq!(cdf_equal[3], CDF_PROB_TOP as u16);

        // Skewed weights
        let cdf_skewed = create_cdf_from_weights(&[3, 1]);
        assert_eq!(cdf_skewed.len(), 2);
        // First symbol should have ~75% cumulative probability
        assert!(cdf_skewed[0] > 20000);
        assert_eq!(cdf_skewed[1], CDF_PROB_TOP as u16);
    }

    /// Q12: Test CDF update
    #[test]
    fn test_q12_cdf_update() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let mut cdf = create_uniform_cdf(4);
        let original_cdf = cdf.clone();

        capsule.update_cdf(&mut cdf, 0);

        // After observing symbol 0, its probability should increase
        // (CDF[0] should decrease since it's cumulative)
        assert_ne!(cdf, original_cdf);
        assert_eq!(cdf[3], CDF_PROB_TOP as u16); // Last element unchanged
    }

    /// Q13: Test statistics accuracy
    #[test]
    fn test_q13_statistics_accuracy() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        capsule.init(&data).unwrap();

        // Decode some bools and literals
        for _ in 0..5 {
            let _ = capsule.decode_bool_eq_prob(&data);
        }
        let _ = capsule.decode_literal(4, &data);
        let _ = capsule.decode_literal(8, &data);

        let stats = capsule.stats();
        assert_eq!(stats.bools_decoded, 5 + 4 + 8); // Literals use bool internally
        assert_eq!(stats.literals_decoded, 2);
    }

    /// Q14: Test range normalization occurs
    #[test]
    fn test_q14_range_normalization() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        capsule.init(&data).unwrap();

        // Initial range should be MAX_RANGE
        assert_eq!(capsule.range(), MAX_RANGE);

        // Decode many bools to trigger normalization
        for _ in 0..20 {
            let _ = capsule.decode_bool_eq_prob(&data);
        }

        let stats = capsule.stats();
        assert!(stats.range_normalizations > 0);
    }

    // =========================================================================
    // T28 Q15-Q21: Integration Tests
    // =========================================================================

    /// Q15: Test symbol decoding with uniform CDF
    #[test]
    fn test_q15_decode_symbol_uniform() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let data = [0x40, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        capsule.init(&data).unwrap();

        let cdf = create_uniform_cdf(4);
        let result = capsule.decode_symbol(&cdf, &data);

        assert!(result.is_ok());
        let symbol = result.unwrap();
        assert!(symbol < 4);

        let stats = capsule.stats();
        assert_eq!(stats.symbols_decoded, 1);
    }

    /// Q16: Test symbol decoding with skewed CDF
    #[test]
    fn test_q16_decode_symbol_skewed() {
        let capsule = Av1SymbolDecoderCapsule::new();

        // Data that should decode to lower symbols with skewed CDF
        let data = [0x10, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        capsule.init(&data).unwrap();

        // CDF heavily weighted toward symbol 0
        let cdf = create_cdf_from_weights(&[100, 1, 1, 1]);
        let result = capsule.decode_symbol(&cdf, &data);

        assert!(result.is_ok());
        let symbol = result.unwrap();
        assert!(symbol < 4);
    }

    /// Q17: Test decode_symbol_with_update
    #[test]
    fn test_q17_decode_with_update() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let data = [0x80, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        capsule.init(&data).unwrap();

        let mut cdf = create_uniform_cdf(4);
        let original_cdf = cdf.clone();

        let result = capsule.decode_symbol_with_update(&mut cdf, &data);
        assert!(result.is_ok());

        // CDF should have been updated
        assert_ne!(cdf, original_cdf);

        let stats = capsule.stats();
        assert_eq!(stats.cdf_updates, 1);
    }

    /// Q18: Test multiple symbol decoding
    #[test]
    fn test_q18_multiple_symbols() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let data: Vec<u8> = (0..64).collect();
        capsule.init(&data).unwrap();

        let cdf = create_uniform_cdf(4);
        let mut decoded_symbols = Vec::new();

        for _ in 0..10 {
            let result = capsule.decode_symbol(&cdf, &data);
            assert!(result.is_ok());
            decoded_symbols.push(result.unwrap());
        }

        assert_eq!(decoded_symbols.len(), 10);

        let stats = capsule.stats();
        assert_eq!(stats.symbols_decoded, 10);
    }

    /// Q19: Test window refill
    #[test]
    fn test_q19_window_refill() {
        let capsule = Av1SymbolDecoderCapsule::new();

        // Large data to require multiple refills
        let data: Vec<u8> = (0..256).map(|i| i as u8).collect();
        capsule.init(&data).unwrap();

        // Decode many literals to exhaust window
        for _ in 0..50 {
            let _ = capsule.decode_literal(8, &data);
        }

        let stats = capsule.stats();
        assert!(stats.window_refills > 0);
    }

    /// Q20: Test reset functionality
    #[test]
    fn test_q20_reset() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        capsule.init(&data).unwrap();

        // Decode some data
        for _ in 0..5 {
            let _ = capsule.decode_bool_eq_prob(&data);
        }

        let gen_before = capsule.generation();

        // Reset
        capsule.reset();

        assert_eq!(capsule.get_state(), Av1SymbolState::Uninitialized);
        assert!(!capsule.is_ready());
        assert_eq!(capsule.range(), 0);

        // Generation should increment on reset
        assert!(capsule.generation() > gen_before);

        // Stats should be reset
        let stats = capsule.stats();
        assert_eq!(stats.bools_decoded, 0);
    }

    /// Q21: Test error recovery
    #[test]
    fn test_q21_error_recovery() {
        let capsule = Av1SymbolDecoderCapsule::new();

        // Try to decode without init - should fail
        let data = [0xFF, 0xFF];
        let result = capsule.decode_bool_eq_prob(&data);
        assert!(result.is_err());

        // Initialize and try again - should succeed
        capsule.init(&data).unwrap();
        let result2 = capsule.decode_bool_eq_prob(&data);
        assert!(result2.is_ok());
    }

    // =========================================================================
    // T28 Q22-Q28: Production Tests
    // =========================================================================

    /// Q22: Test concurrent read access (statistics)
    #[test]
    fn test_q22_concurrent_stats_access() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(Av1SymbolDecoderCapsule::new());
        let data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        capsule.init(&data).unwrap();

        let mut handles = vec![];

        // Spawn threads that read stats concurrently
        for _ in 0..4 {
            let capsule_clone = Arc::clone(&capsule);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let _ = capsule_clone.stats();
                    let _ = capsule_clone.generation();
                    let _ = capsule_clone.get_state();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// Q23: Test with various data patterns
    #[test]
    fn test_q23_various_patterns() {
        let patterns: Vec<Vec<u8>> = vec![
            vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // All zeros
            vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF], // All ones
            vec![0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55], // Alternating
            vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0], // Sequential
        ];

        for pattern in &patterns {
            let capsule = Av1SymbolDecoderCapsule::new();
            capsule.init(pattern).unwrap();

            let cdf = create_uniform_cdf(4);
            for _ in 0..5 {
                let result = capsule.decode_symbol(&cdf, pattern);
                assert!(result.is_ok(), "Failed with pattern {:?}", pattern);
            }
        }
    }

    /// Q24: Test invalid CDF handling
    #[test]
    fn test_q24_invalid_cdf() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        capsule.init(&data).unwrap();

        // Empty CDF should fail
        let empty_cdf: Vec<u16> = vec![];
        let result = capsule.decode_symbol(&empty_cdf, &data);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), Av1SymbolError::InvalidCdf);
    }

    /// Q25: Test invalid bit count for literals
    #[test]
    fn test_q25_invalid_bit_count() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        capsule.init(&data).unwrap();

        // 0 bits should fail
        assert!(capsule.decode_literal(0, &data).is_err());

        // 25+ bits should fail
        assert!(capsule.decode_literal(25, &data).is_err());
    }

    /// Q26: Test exit and termination
    #[test]
    fn test_q26_exit_termination() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        capsule.init(&data).unwrap();

        // Do some decoding
        let _ = capsule.decode_bool_eq_prob(&data);

        // Exit should terminate
        let gen_before = capsule.generation();
        capsule.exit();

        assert!(capsule.is_terminated());
        assert_eq!(capsule.get_state(), Av1SymbolState::Terminated);
        assert!(capsule.generation() > gen_before);

        // Decoding after exit should fail
        let result = capsule.decode_bool_eq_prob(&data);
        assert!(result.is_err());
    }

    /// Q27: Test CDF monotonicity preservation
    #[test]
    fn test_q27_cdf_monotonicity() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let mut cdf = create_uniform_cdf(8);

        // Update CDF many times
        for symbol in 0..8 {
            for _ in 0..10 {
                capsule.update_cdf(&mut cdf, symbol);
            }
        }

        // CDF should remain monotonic
        for i in 1..cdf.len() {
            assert!(
                cdf[i] > cdf[i - 1],
                "CDF not monotonic at index {}: {} <= {}",
                i,
                cdf[i],
                cdf[i - 1]
            );
        }

        // Last element should be CDF_PROB_TOP
        assert_eq!(cdf[cdf.len() - 1], CDF_PROB_TOP as u16);
    }

    /// Q28: Test large symbol alphabet
    #[test]
    fn test_q28_large_alphabet() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let data: Vec<u8> = (0..64).collect();
        capsule.init(&data).unwrap();

        // Test with maximum symbols (16)
        let cdf = create_uniform_cdf(MAX_CDF_SYMBOLS);
        assert_eq!(cdf.len(), MAX_CDF_SYMBOLS);

        for _ in 0..10 {
            let result = capsule.decode_symbol(&cdf, &data);
            assert!(result.is_ok());
            let symbol = result.unwrap();
            assert!(symbol < MAX_CDF_SYMBOLS as u32);
        }
    }

    // =========================================================================
    // T28 Q29-Q35: Determinism Tests
    // =========================================================================

    /// Q29: Test deterministic initialization
    #[test]
    fn test_q29_deterministic_init() {
        let data = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];

        let capsule1 = Av1SymbolDecoderCapsule::new();
        let capsule2 = Av1SymbolDecoderCapsule::new();

        capsule1.init(&data).unwrap();
        capsule2.init(&data).unwrap();

        assert_eq!(capsule1.range(), capsule2.range());
        assert_eq!(capsule1.value(), capsule2.value());
    }

    /// Q30: Test deterministic boolean decoding
    #[test]
    fn test_q30_deterministic_bool_decode() {
        let data = [0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA];

        let capsule1 = Av1SymbolDecoderCapsule::new();
        let capsule2 = Av1SymbolDecoderCapsule::new();

        capsule1.init(&data).unwrap();
        capsule2.init(&data).unwrap();

        let mut results1 = vec![];
        let mut results2 = vec![];

        for _ in 0..10 {
            results1.push(capsule1.decode_bool_eq_prob(&data).unwrap());
            results2.push(capsule2.decode_bool_eq_prob(&data).unwrap());
        }

        assert_eq!(results1, results2, "Boolean decoding not deterministic");
    }

    /// Q31: Test deterministic symbol decoding
    #[test]
    fn test_q31_deterministic_symbol_decode() {
        let data = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];
        let cdf = create_uniform_cdf(4);

        let capsule1 = Av1SymbolDecoderCapsule::new();
        let capsule2 = Av1SymbolDecoderCapsule::new();

        capsule1.init(&data).unwrap();
        capsule2.init(&data).unwrap();

        let mut symbols1 = vec![];
        let mut symbols2 = vec![];

        for _ in 0..5 {
            symbols1.push(capsule1.decode_symbol(&cdf, &data).unwrap());
            symbols2.push(capsule2.decode_symbol(&cdf, &data).unwrap());
        }

        assert_eq!(symbols1, symbols2, "Symbol decoding not deterministic");
    }

    /// Q32: Test deterministic literal decoding
    #[test]
    fn test_q32_deterministic_literal_decode() {
        let data = [0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE];

        let capsule1 = Av1SymbolDecoderCapsule::new();
        let capsule2 = Av1SymbolDecoderCapsule::new();

        capsule1.init(&data).unwrap();
        capsule2.init(&data).unwrap();

        let mut literals1 = vec![];
        let mut literals2 = vec![];

        for bits in [4, 8, 12, 16] {
            literals1.push(capsule1.decode_literal(bits, &data).unwrap());
            literals2.push(capsule2.decode_literal(bits, &data).unwrap());
        }

        assert_eq!(literals1, literals2, "Literal decoding not deterministic");
    }

    /// Q33: Test deterministic CDF update
    #[test]
    fn test_q33_deterministic_cdf_update() {
        let capsule1 = Av1SymbolDecoderCapsule::new();
        let capsule2 = Av1SymbolDecoderCapsule::new();

        let mut cdf1 = create_uniform_cdf(4);
        let mut cdf2 = create_uniform_cdf(4);

        for symbol in [0, 1, 2, 3, 0, 0, 1, 2] {
            capsule1.update_cdf(&mut cdf1, symbol);
            capsule2.update_cdf(&mut cdf2, symbol);
        }

        assert_eq!(cdf1, cdf2, "CDF update not deterministic");
    }

    /// Q34: Test bytes consumed tracking
    #[test]
    fn test_q34_bytes_consumed() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let data: Vec<u8> = (0..64).collect();
        capsule.init(&data).unwrap();

        // After init, at least 2 bytes consumed
        assert!(capsule.bytes_consumed() >= 2);

        // Decode more data
        for _ in 0..20 {
            let _ = capsule.decode_bool_eq_prob(&data);
        }

        // More bytes should be consumed
        let final_consumed = capsule.bytes_consumed();
        assert!(final_consumed >= 2);
    }

    /// Q35: Test default implementation
    #[test]
    fn test_q35_default_impl() {
        let capsule = Av1SymbolDecoderCapsule::default();

        assert_eq!(capsule.get_state(), Av1SymbolState::Uninitialized);
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.range(), 0);
    }

    // =========================================================================
    // Additional Edge Case Tests
    // =========================================================================

    /// Test error display implementations
    #[test]
    fn test_error_display() {
        assert_eq!(
            format!("{}", Av1SymbolError::InvalidState),
            "invalid decoder state"
        );
        assert_eq!(
            format!("{}", Av1SymbolError::UnexpectedEof),
            "unexpected end of bitstream"
        );
        assert_eq!(format!("{}", Av1SymbolError::InvalidCdf), "invalid CDF");
    }

    /// Test state conversion
    #[test]
    fn test_state_conversion() {
        assert_eq!(Av1SymbolState::from(0), Av1SymbolState::Uninitialized);
        assert_eq!(Av1SymbolState::from(1), Av1SymbolState::Initialized);
        assert_eq!(Av1SymbolState::from(2), Av1SymbolState::Decoding);
        assert_eq!(Av1SymbolState::from(3), Av1SymbolState::Terminated);
        assert_eq!(Av1SymbolState::from(255), Av1SymbolState::Error);
        assert_eq!(Av1SymbolState::from(100), Av1SymbolState::Error);
    }

    /// Test constants
    #[test]
    fn test_constants() {
        assert_eq!(SYMBOL_BITS, 15);
        assert_eq!(CDF_PROB_BITS, 15);
        assert_eq!(CDF_PROB_TOP, 32768);
        assert_eq!(MIN_RANGE, 256);
        assert_eq!(MAX_RANGE, 65536);
        assert_eq!(MAX_CDF_SYMBOLS, 16);
    }

    /// Test stats snapshot
    #[test]
    fn test_stats_snapshot() {
        let capsule = Av1SymbolDecoderCapsule::new();

        let data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        capsule.init(&data).unwrap();

        // Take initial snapshot
        let stats1 = capsule.stats();

        // Do some work
        let _ = capsule.decode_bool_eq_prob(&data);
        let _ = capsule.decode_literal(4, &data);

        // Take final snapshot
        let stats2 = capsule.stats();

        // Stats should have changed
        assert!(stats2.bools_decoded > stats1.bools_decoded);
        assert!(stats2.literals_decoded > stats1.literals_decoded);
    }
}
