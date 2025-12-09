//! Bounds Checker Capsule - Memory Safety Validation
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! # Architecture
//!
//! - **Tier**: T1 Atomic (128 bytes, cache-aligned)
//! - **Size**: 128 bytes (single cache line, false-sharing free)
//! - **Purpose**: Memory bounds validation and buffer overflow prevention
//!
//! # Design Philosophy
//!
//! All buffer accesses are validated BEFORE they occur to prevent:
//! - Buffer overreads (information disclosure)
//! - Buffer overwrites (arbitrary code execution)
//! - Out-of-bounds motion vectors (video corruption)
//! - Null pointer dereferences (crashes)
//! - Alignment violations (performance/correctness)
//!
//! # Performance
//!
//! - **Inline check**: ~1-2ns (optimized to branch prediction)
//! - **Violation recording**: ~5ns (atomic increment)
//! - **Statistics snapshot**: ~10ns (6 atomic loads)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_ALIGNMENT`: 128B cache alignment enforced by repr(C, align(128))
//! - `#ASSUME_ATOMIC_ORDERING`: Acquire/Release for generation, Relaxed for counters
//! - `#ASSUME_NO_OVERFLOW`: Violation counters limited to u32 (4B+ checks before overflow)
//! - `#ASSUME_INLINE_OPTIMIZATION`: All check methods are #[inline(always)] for zero-cost
//!
//! # UCE34 Compliance
//!
//! - Q10: T1 Atomic tier (minimal overhead, lockfree)
//! - Q33: #[repr(C, align(128))] verification
//! - Q34: Generation counter for audit trail
//!
//! # References
//!
//! - CWE-119: Improper Restriction of Operations within the Bounds of a Memory Buffer
//! - CWE-787: Out-of-bounds Write
//! - CWE-125: Out-of-bounds Read

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// ============================================================================
// Bounds Check Types
// ============================================================================

/// Types of bounds checks performed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BoundsCheckType {
    // Read operations (0x00-0x0F)
    /// Single byte read
    ReadByte = 0x00,
    /// Slice read with length
    ReadSlice = 0x01,
    /// Bitstream read with bit count
    ReadBitstream = 0x02,

    // Write operations (0x10-0x1F)
    /// Single byte write
    WriteByte = 0x10,
    /// Slice write with length
    WriteSlice = 0x11,
    /// Frame buffer write
    WriteFrame = 0x12,

    // Index operations (0x20-0x2F)
    /// Array index access
    ArrayIndex = 0x20,
    /// Pointer arithmetic offset
    PointerOffset = 0x21,

    // Video-specific (0x30-0x3F)
    /// Motion vector pointing outside reference frame
    MotionVector = 0x30,
    /// Tile index in grid
    TileIndex = 0x31,
    /// Reference frame buffer index
    RefFrameIndex = 0x32,
}

impl BoundsCheckType {
    /// Check if this is a read operation
    #[inline]
    pub const fn is_read(self) -> bool {
        (self as u8) < 0x10
    }

    /// Check if this is a write operation
    #[inline]
    pub const fn is_write(self) -> bool {
        let v = self as u8;
        v >= 0x10 && v < 0x20
    }

    /// Check if this is an index operation
    #[inline]
    pub const fn is_index(self) -> bool {
        let v = self as u8;
        v >= 0x20 && v < 0x30
    }

    /// Check if this is video-specific
    #[inline]
    pub const fn is_video(self) -> bool {
        (self as u8) >= 0x30
    }

    /// Get human-readable name
    pub const fn name(self) -> &'static str {
        match self {
            BoundsCheckType::ReadByte => "ReadByte",
            BoundsCheckType::ReadSlice => "ReadSlice",
            BoundsCheckType::ReadBitstream => "ReadBitstream",
            BoundsCheckType::WriteByte => "WriteByte",
            BoundsCheckType::WriteSlice => "WriteSlice",
            BoundsCheckType::WriteFrame => "WriteFrame",
            BoundsCheckType::ArrayIndex => "ArrayIndex",
            BoundsCheckType::PointerOffset => "PointerOffset",
            BoundsCheckType::MotionVector => "MotionVector",
            BoundsCheckType::TileIndex => "TileIndex",
            BoundsCheckType::RefFrameIndex => "RefFrameIndex",
        }
    }
}

impl core::fmt::Display for BoundsCheckType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// Bounds Violation Types
// ============================================================================

/// Types of bounds violations detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundsViolation {
    /// Attempted read beyond buffer end
    BufferOverread {
        offset: usize,
        length: usize,
        buffer_size: usize,
    },

    /// Attempted write beyond buffer end
    BufferOverwrite {
        offset: usize,
        length: usize,
        buffer_size: usize,
    },

    /// Negative offset in pointer arithmetic
    NegativeOffset { offset: i64 },

    /// Array index exceeds maximum
    IndexOutOfBounds { index: usize, max: usize },

    /// Motion vector points outside reference frame
    MVOutOfFrame {
        mv_x: i32,
        mv_y: i32,
        frame_width: u32,
        frame_height: u32,
    },

    /// Null pointer access attempt
    NullPointer,

    /// Address not properly aligned
    AlignmentViolation { address: usize, required: usize },

    /// Not enough bits remaining in bitstream
    BitstreamUnderflow {
        bits_needed: u32,
        bits_remaining: u32,
    },

    /// Tile index exceeds grid dimensions
    TileOutOfBounds {
        tile_col: u32,
        tile_row: u32,
        max_cols: u32,
        max_rows: u32,
    },

    /// Reference frame index invalid
    InvalidRefFrame { ref_idx: usize, max_refs: usize },
}

impl BoundsViolation {
    /// Get violation code for atomic storage (fits in u8)
    pub const fn code(&self) -> u8 {
        match self {
            BoundsViolation::BufferOverread { .. } => 1,
            BoundsViolation::BufferOverwrite { .. } => 2,
            BoundsViolation::NegativeOffset { .. } => 3,
            BoundsViolation::IndexOutOfBounds { .. } => 4,
            BoundsViolation::MVOutOfFrame { .. } => 5,
            BoundsViolation::NullPointer => 6,
            BoundsViolation::AlignmentViolation { .. } => 7,
            BoundsViolation::BitstreamUnderflow { .. } => 8,
            BoundsViolation::TileOutOfBounds { .. } => 9,
            BoundsViolation::InvalidRefFrame { .. } => 10,
        }
    }

    /// Get human-readable description
    pub fn message(&self) -> String {
        match self {
            BoundsViolation::BufferOverread {
                offset,
                length,
                buffer_size,
            } => {
                format!(
                    "Buffer overread: offset {} + length {} = {} exceeds buffer size {}",
                    offset,
                    length,
                    offset.saturating_add(*length),
                    buffer_size
                )
            }
            BoundsViolation::BufferOverwrite {
                offset,
                length,
                buffer_size,
            } => {
                format!(
                    "Buffer overwrite: offset {} + length {} = {} exceeds buffer size {}",
                    offset,
                    length,
                    offset.saturating_add(*length),
                    buffer_size
                )
            }
            BoundsViolation::NegativeOffset { offset } => {
                format!("Negative pointer offset: {}", offset)
            }
            BoundsViolation::IndexOutOfBounds { index, max } => {
                format!("Index {} out of bounds (max {})", index, max)
            }
            BoundsViolation::MVOutOfFrame {
                mv_x,
                mv_y,
                frame_width,
                frame_height,
            } => {
                format!(
                    "Motion vector ({}, {}) outside frame {}x{}",
                    mv_x, mv_y, frame_width, frame_height
                )
            }
            BoundsViolation::NullPointer => "Null pointer access".to_string(),
            BoundsViolation::AlignmentViolation { address, required } => {
                format!(
                    "Address 0x{:x} not aligned to {} bytes",
                    address, required
                )
            }
            BoundsViolation::BitstreamUnderflow {
                bits_needed,
                bits_remaining,
            } => {
                format!(
                    "Bitstream underflow: need {} bits, only {} remaining",
                    bits_needed, bits_remaining
                )
            }
            BoundsViolation::TileOutOfBounds {
                tile_col,
                tile_row,
                max_cols,
                max_rows,
            } => {
                format!(
                    "Tile ({}, {}) out of bounds (grid {}x{})",
                    tile_col, tile_row, max_cols, max_rows
                )
            }
            BoundsViolation::InvalidRefFrame { ref_idx, max_refs } => {
                format!(
                    "Reference frame index {} invalid (max {})",
                    ref_idx, max_refs
                )
            }
        }
    }

    /// Check if this is a critical security violation
    pub const fn is_critical(&self) -> bool {
        matches!(
            self,
            BoundsViolation::BufferOverwrite { .. }
                | BoundsViolation::NullPointer
                | BoundsViolation::NegativeOffset { .. }
        )
    }
}

impl core::fmt::Display for BoundsViolation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for BoundsViolation {}

// ============================================================================
// State Flags (packed into AtomicU64)
// ============================================================================

/// Bounds checker state flags
pub mod bounds_flags {
    /// Strict mode enabled (panic on violation)
    pub const STRICT_MODE: u64 = 1 << 0;
    /// Frame bounds have been set
    pub const FRAME_BOUNDS_SET: u64 = 1 << 1;
    /// Bitstream bounds have been set
    pub const BITSTREAM_BOUNDS_SET: u64 = 1 << 2;
    /// At least one violation has occurred
    pub const HAS_VIOLATIONS: u64 = 1 << 3;
    /// At least one critical violation
    pub const HAS_CRITICAL: u64 = 1 << 4;
    /// Bounds checking enabled (can be disabled for perf)
    pub const CHECKING_ENABLED: u64 = 1 << 5;

    // Frame dimensions packed in upper bits
    /// Shift for frame width (bits 16-31)
    pub const FRAME_WIDTH_SHIFT: u64 = 16;
    /// Shift for frame height (bits 32-47)
    pub const FRAME_HEIGHT_SHIFT: u64 = 32;
    /// Mask for 16-bit dimension
    pub const DIMENSION_MASK: u64 = 0xFFFF;
}

// ============================================================================
// BoundsCheckerCapsule
// ============================================================================

/// T1 Atomic capsule for memory bounds validation
///
/// 128B cache-aligned, lockfree, O(1) bounds checking
///
/// # Layout (128 bytes)
///
/// ```text
/// [0..8)      | state: AtomicU64             | Flags + frame dimensions (packed)
/// [8..16)     | generation: AtomicU64        | Q34 audit generation counter
/// [16..24)    | total_checks: AtomicU64      | Total bounds checks performed
/// [24..32)    | violations: AtomicU64        | Total violations detected
/// [32..36)    | last_violation_type: AtomicU32 | Last violation code
/// [36..40)    | last_violation_offset: AtomicU32 | Last violation offset (truncated)
/// [40..44)    | read_violations: AtomicU32   | Read violation count
/// [44..48)    | write_violations: AtomicU32  | Write violation count
/// [48..52)    | index_violations: AtomicU32  | Index violation count
/// [52..56)    | mv_violations: AtomicU32     | Motion vector violation count
/// [56..60)    | critical_violations: AtomicU32 | Critical violation count
/// [60..64)    | null_violations: AtomicU32   | Null pointer violation count
/// [64..128)   | _padding: [u8; 64]           | Cache alignment padding
/// ```
#[repr(C, align(128))]
pub struct BoundsCheckerCapsule {
    /// State flags + frame dimensions (packed)
    /// Bits 0-15: flags
    /// Bits 16-31: frame width (max 65535)
    /// Bits 32-47: frame height (max 65535)
    state: AtomicU64,

    /// Generation counter for Q34 audit trails
    generation: AtomicU64,

    /// Total bounds checks performed
    total_checks: AtomicU64,

    /// Total violations detected
    violations: AtomicU64,

    /// Last violation type (code)
    last_violation_type: AtomicU32,

    /// Last violation offset (truncated to u32)
    last_violation_offset: AtomicU32,

    /// Read violation count
    read_violations: AtomicU32,

    /// Write violation count
    write_violations: AtomicU32,

    /// Index violation count
    index_violations: AtomicU32,

    /// Motion vector violation count
    mv_violations: AtomicU32,

    /// Critical violation count (security)
    critical_violations: AtomicU32,

    /// Null pointer violation count
    null_violations: AtomicU32,

    /// Padding to 128B cache line
    _padding: [u8; 64],
}

// Compile-time size and alignment verification
const _: () = {
    assert!(core::mem::size_of::<BoundsCheckerCapsule>() == 128);
    assert!(core::mem::align_of::<BoundsCheckerCapsule>() == 128);
};

impl BoundsCheckerCapsule {
    /// Create a new bounds checker capsule with checking enabled
    pub fn new() -> Self {
        Self {
            state: AtomicU64::new(bounds_flags::CHECKING_ENABLED),
            generation: AtomicU64::new(0),
            total_checks: AtomicU64::new(0),
            violations: AtomicU64::new(0),
            last_violation_type: AtomicU32::new(0),
            last_violation_offset: AtomicU32::new(0),
            read_violations: AtomicU32::new(0),
            write_violations: AtomicU32::new(0),
            index_violations: AtomicU32::new(0),
            mv_violations: AtomicU32::new(0),
            critical_violations: AtomicU32::new(0),
            null_violations: AtomicU32::new(0),
            _padding: [0u8; 64],
        }
    }

    /// Create with strict mode enabled (panics on violation)
    pub fn strict() -> Self {
        Self {
            state: AtomicU64::new(bounds_flags::CHECKING_ENABLED | bounds_flags::STRICT_MODE),
            generation: AtomicU64::new(0),
            total_checks: AtomicU64::new(0),
            violations: AtomicU64::new(0),
            last_violation_type: AtomicU32::new(0),
            last_violation_offset: AtomicU32::new(0),
            read_violations: AtomicU32::new(0),
            write_violations: AtomicU32::new(0),
            index_violations: AtomicU32::new(0),
            mv_violations: AtomicU32::new(0),
            critical_violations: AtomicU32::new(0),
            null_violations: AtomicU32::new(0),
            _padding: [0u8; 64],
        }
    }

    // ========================================================================
    // Configuration
    // ========================================================================

    /// Enable or disable strict mode (panic on violation)
    pub fn set_strict_mode(&self, strict: bool) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        if strict {
            self.set_flag(bounds_flags::STRICT_MODE);
        } else {
            self.clear_flag(bounds_flags::STRICT_MODE);
        }
    }

    /// Check if strict mode is enabled
    #[inline]
    pub fn is_strict(&self) -> bool {
        self.has_flag(bounds_flags::STRICT_MODE)
    }

    /// Enable or disable bounds checking (for performance-critical paths)
    pub fn set_checking_enabled(&self, enabled: bool) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        if enabled {
            self.set_flag(bounds_flags::CHECKING_ENABLED);
        } else {
            self.clear_flag(bounds_flags::CHECKING_ENABLED);
        }
    }

    /// Check if bounds checking is enabled
    #[inline]
    pub fn is_checking_enabled(&self) -> bool {
        self.has_flag(bounds_flags::CHECKING_ENABLED)
    }

    /// Set frame bounds for motion vector validation
    pub fn set_frame_bounds(&self, width: u32, height: u32) {
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Clamp to 16-bit range
        let w = (width.min(65535) as u64) << bounds_flags::FRAME_WIDTH_SHIFT;
        let h = (height.min(65535) as u64) << bounds_flags::FRAME_HEIGHT_SHIFT;

        // Load current state, preserve flags, update dimensions
        let current = self.state.load(Ordering::Acquire);
        let flags = current & 0xFFFF;
        let new_state = flags | w | h | bounds_flags::FRAME_BOUNDS_SET;
        self.state.store(new_state, Ordering::Release);
    }

    /// Get frame width
    #[inline]
    pub fn frame_width(&self) -> u32 {
        let state = self.state.load(Ordering::Acquire);
        ((state >> bounds_flags::FRAME_WIDTH_SHIFT) & bounds_flags::DIMENSION_MASK) as u32
    }

    /// Get frame height
    #[inline]
    pub fn frame_height(&self) -> u32 {
        let state = self.state.load(Ordering::Acquire);
        ((state >> bounds_flags::FRAME_HEIGHT_SHIFT) & bounds_flags::DIMENSION_MASK) as u32
    }

    // ========================================================================
    // Inline Bounds Checks (designed for zero-cost when inlined)
    // ========================================================================

    /// Check if a read operation is within bounds
    ///
    /// # Arguments
    /// * `offset` - Starting offset in buffer
    /// * `length` - Number of bytes to read
    /// * `buffer_size` - Total buffer size
    ///
    /// # Returns
    /// * `Ok(())` if bounds check passes
    /// * `Err(BoundsViolation::BufferOverread)` if out of bounds
    #[inline(always)]
    pub fn check_read(
        &self,
        offset: usize,
        length: usize,
        buffer_size: usize,
    ) -> Result<(), BoundsViolation> {
        // Early exit if checking disabled
        if !self.is_checking_enabled() {
            return Ok(());
        }

        self.total_checks.fetch_add(1, Ordering::Relaxed);

        // Check for overflow in offset + length
        let end = offset.checked_add(length);
        if end.is_none() || end.unwrap() > buffer_size {
            let violation = BoundsViolation::BufferOverread {
                offset,
                length,
                buffer_size,
            };
            self.record_violation_internal(&violation, BoundsCheckType::ReadSlice);
            return Err(violation);
        }

        Ok(())
    }

    /// Check if a write operation is within bounds
    ///
    /// # Arguments
    /// * `offset` - Starting offset in buffer
    /// * `length` - Number of bytes to write
    /// * `buffer_size` - Total buffer size
    ///
    /// # Returns
    /// * `Ok(())` if bounds check passes
    /// * `Err(BoundsViolation::BufferOverwrite)` if out of bounds
    #[inline(always)]
    pub fn check_write(
        &self,
        offset: usize,
        length: usize,
        buffer_size: usize,
    ) -> Result<(), BoundsViolation> {
        if !self.is_checking_enabled() {
            return Ok(());
        }

        self.total_checks.fetch_add(1, Ordering::Relaxed);

        let end = offset.checked_add(length);
        if end.is_none() || end.unwrap() > buffer_size {
            let violation = BoundsViolation::BufferOverwrite {
                offset,
                length,
                buffer_size,
            };
            self.record_violation_internal(&violation, BoundsCheckType::WriteSlice);
            return Err(violation);
        }

        Ok(())
    }

    /// Check if an index is within bounds
    ///
    /// # Arguments
    /// * `index` - Array index to check
    /// * `max` - Maximum valid index (exclusive)
    ///
    /// # Returns
    /// * `Ok(())` if index < max
    /// * `Err(BoundsViolation::IndexOutOfBounds)` if index >= max
    #[inline(always)]
    pub fn check_index(&self, index: usize, max: usize) -> Result<(), BoundsViolation> {
        if !self.is_checking_enabled() {
            return Ok(());
        }

        self.total_checks.fetch_add(1, Ordering::Relaxed);

        if index >= max {
            let violation = BoundsViolation::IndexOutOfBounds { index, max };
            self.record_violation_internal(&violation, BoundsCheckType::ArrayIndex);
            return Err(violation);
        }

        Ok(())
    }

    /// Check if a slice range is within bounds
    ///
    /// # Type Parameters
    /// * `T` - Element type (for documentation only, not used in check)
    ///
    /// # Arguments
    /// * `slice` - The slice to check against
    /// * `start` - Starting index
    /// * `end` - Ending index (exclusive)
    ///
    /// # Returns
    /// * `Ok(())` if start <= end && end <= slice.len()
    /// * `Err(BoundsViolation::IndexOutOfBounds)` otherwise
    #[inline(always)]
    pub fn check_slice_bounds<T>(
        &self,
        slice: &[T],
        start: usize,
        end: usize,
    ) -> Result<(), BoundsViolation> {
        if !self.is_checking_enabled() {
            return Ok(());
        }

        self.total_checks.fetch_add(1, Ordering::Relaxed);

        if start > end || end > slice.len() {
            let violation = BoundsViolation::IndexOutOfBounds {
                index: end,
                max: slice.len(),
            };
            self.record_violation_internal(&violation, BoundsCheckType::ArrayIndex);
            return Err(violation);
        }

        Ok(())
    }

    /// Check if a pointer offset is valid (non-negative)
    ///
    /// # Arguments
    /// * `offset` - Signed offset value
    ///
    /// # Returns
    /// * `Ok(())` if offset >= 0
    /// * `Err(BoundsViolation::NegativeOffset)` if offset < 0
    #[inline(always)]
    pub fn check_offset(&self, offset: i64) -> Result<(), BoundsViolation> {
        if !self.is_checking_enabled() {
            return Ok(());
        }

        self.total_checks.fetch_add(1, Ordering::Relaxed);

        if offset < 0 {
            let violation = BoundsViolation::NegativeOffset { offset };
            self.record_violation_internal(&violation, BoundsCheckType::PointerOffset);
            return Err(violation);
        }

        Ok(())
    }

    /// Check if a pointer is non-null
    ///
    /// # Arguments
    /// * `ptr` - Raw pointer to check
    ///
    /// # Returns
    /// * `Ok(())` if pointer is non-null
    /// * `Err(BoundsViolation::NullPointer)` if pointer is null
    #[inline(always)]
    pub fn check_non_null<T>(&self, ptr: *const T) -> Result<(), BoundsViolation> {
        if !self.is_checking_enabled() {
            return Ok(());
        }

        self.total_checks.fetch_add(1, Ordering::Relaxed);

        if ptr.is_null() {
            let violation = BoundsViolation::NullPointer;
            self.record_violation_internal(&violation, BoundsCheckType::PointerOffset);
            return Err(violation);
        }

        Ok(())
    }

    /// Check pointer alignment
    ///
    /// # Arguments
    /// * `ptr` - Raw pointer to check
    /// * `alignment` - Required alignment in bytes (must be power of 2)
    ///
    /// # Returns
    /// * `Ok(())` if properly aligned
    /// * `Err(BoundsViolation::AlignmentViolation)` if misaligned
    #[inline(always)]
    pub fn check_alignment<T>(&self, ptr: *const T, alignment: usize) -> Result<(), BoundsViolation> {
        if !self.is_checking_enabled() {
            return Ok(());
        }

        self.total_checks.fetch_add(1, Ordering::Relaxed);

        let address = ptr as usize;
        if address & (alignment - 1) != 0 {
            let violation = BoundsViolation::AlignmentViolation {
                address,
                required: alignment,
            };
            self.record_violation_internal(&violation, BoundsCheckType::PointerOffset);
            return Err(violation);
        }

        Ok(())
    }

    // ========================================================================
    // Video-Specific Bounds Checks
    // ========================================================================

    /// Check if a motion vector stays within the reference frame
    ///
    /// # Arguments
    /// * `base_x` - Block's X position in frame
    /// * `base_y` - Block's Y position in frame
    /// * `mv_x` - Motion vector X component (in pixels or sub-pixels)
    /// * `mv_y` - Motion vector Y component
    /// * `block_width` - Width of the block being predicted
    /// * `block_height` - Height of the block being predicted
    /// * `frame_width` - Reference frame width
    /// * `frame_height` - Reference frame height
    ///
    /// # Returns
    /// * `Ok(())` if MV keeps block within frame
    /// * `Err(BoundsViolation::MVOutOfFrame)` if MV goes outside
    #[inline(always)]
    pub fn check_mv_bounds(
        &self,
        base_x: i32,
        base_y: i32,
        mv_x: i32,
        mv_y: i32,
        block_width: u32,
        block_height: u32,
        frame_width: u32,
        frame_height: u32,
    ) -> Result<(), BoundsViolation> {
        if !self.is_checking_enabled() {
            return Ok(());
        }

        self.total_checks.fetch_add(1, Ordering::Relaxed);

        // Calculate predicted block position
        let pred_x = base_x.saturating_add(mv_x);
        let pred_y = base_y.saturating_add(mv_y);

        // Check left/top bounds
        if pred_x < 0 || pred_y < 0 {
            let violation = BoundsViolation::MVOutOfFrame {
                mv_x,
                mv_y,
                frame_width,
                frame_height,
            };
            self.record_violation_internal(&violation, BoundsCheckType::MotionVector);
            return Err(violation);
        }

        // Check right/bottom bounds
        let pred_right = pred_x as u32 + block_width;
        let pred_bottom = pred_y as u32 + block_height;

        if pred_right > frame_width || pred_bottom > frame_height {
            let violation = BoundsViolation::MVOutOfFrame {
                mv_x,
                mv_y,
                frame_width,
                frame_height,
            };
            self.record_violation_internal(&violation, BoundsCheckType::MotionVector);
            return Err(violation);
        }

        Ok(())
    }

    /// Check if a reference frame index is valid
    ///
    /// # Arguments
    /// * `ref_idx` - Reference frame index
    /// * `max_refs` - Maximum number of reference frames
    ///
    /// # Returns
    /// * `Ok(())` if ref_idx < max_refs
    /// * `Err(BoundsViolation::InvalidRefFrame)` if invalid
    #[inline(always)]
    pub fn check_ref_frame(&self, ref_idx: usize, max_refs: usize) -> Result<(), BoundsViolation> {
        if !self.is_checking_enabled() {
            return Ok(());
        }

        self.total_checks.fetch_add(1, Ordering::Relaxed);

        if ref_idx >= max_refs {
            let violation = BoundsViolation::InvalidRefFrame { ref_idx, max_refs };
            self.record_violation_internal(&violation, BoundsCheckType::RefFrameIndex);
            return Err(violation);
        }

        Ok(())
    }

    /// Check if enough bits are available in bitstream
    ///
    /// # Arguments
    /// * `bits_needed` - Number of bits to read
    /// * `bits_remaining` - Bits remaining in buffer
    ///
    /// # Returns
    /// * `Ok(())` if bits_needed <= bits_remaining
    /// * `Err(BoundsViolation::BitstreamUnderflow)` if insufficient
    #[inline(always)]
    pub fn check_bits_available(
        &self,
        bits_needed: u32,
        bits_remaining: u32,
    ) -> Result<(), BoundsViolation> {
        if !self.is_checking_enabled() {
            return Ok(());
        }

        self.total_checks.fetch_add(1, Ordering::Relaxed);

        if bits_needed > bits_remaining {
            let violation = BoundsViolation::BitstreamUnderflow {
                bits_needed,
                bits_remaining,
            };
            self.record_violation_internal(&violation, BoundsCheckType::ReadBitstream);
            return Err(violation);
        }

        Ok(())
    }

    /// Check if tile coordinates are within grid
    ///
    /// # Arguments
    /// * `tile_col` - Tile column index
    /// * `tile_row` - Tile row index
    /// * `max_cols` - Maximum columns in grid
    /// * `max_rows` - Maximum rows in grid
    ///
    /// # Returns
    /// * `Ok(())` if within bounds
    /// * `Err(BoundsViolation::TileOutOfBounds)` if outside grid
    #[inline(always)]
    pub fn check_tile_bounds(
        &self,
        tile_col: u32,
        tile_row: u32,
        max_cols: u32,
        max_rows: u32,
    ) -> Result<(), BoundsViolation> {
        if !self.is_checking_enabled() {
            return Ok(());
        }

        self.total_checks.fetch_add(1, Ordering::Relaxed);

        if tile_col >= max_cols || tile_row >= max_rows {
            let violation = BoundsViolation::TileOutOfBounds {
                tile_col,
                tile_row,
                max_cols,
                max_rows,
            };
            self.record_violation_internal(&violation, BoundsCheckType::TileIndex);
            return Err(violation);
        }

        Ok(())
    }

    // ========================================================================
    // Violation Tracking
    // ========================================================================

    /// Record a violation (public API for custom checks)
    pub fn record_violation(&self, violation: &BoundsViolation, check_type: BoundsCheckType) {
        self.record_violation_internal(violation, check_type);
    }

    /// Internal violation recording
    fn record_violation_internal(&self, violation: &BoundsViolation, check_type: BoundsCheckType) {
        // Increment generation for audit trail
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Update violation counters
        self.violations.fetch_add(1, Ordering::Relaxed);
        self.set_flag(bounds_flags::HAS_VIOLATIONS);

        // Track by category
        if check_type.is_read() {
            self.read_violations.fetch_add(1, Ordering::Relaxed);
        } else if check_type.is_write() {
            self.write_violations.fetch_add(1, Ordering::Relaxed);
        } else if check_type.is_index() {
            self.index_violations.fetch_add(1, Ordering::Relaxed);
        } else if check_type.is_video() {
            self.mv_violations.fetch_add(1, Ordering::Relaxed);
        }

        // Track critical and null pointer violations
        if violation.is_critical() {
            self.critical_violations.fetch_add(1, Ordering::Relaxed);
            self.set_flag(bounds_flags::HAS_CRITICAL);
        }

        if matches!(violation, BoundsViolation::NullPointer) {
            self.null_violations.fetch_add(1, Ordering::Relaxed);
        }

        // Store last violation info
        self.last_violation_type
            .store(violation.code() as u32, Ordering::Release);

        // Store offset if applicable (truncated to u32)
        let offset = match violation {
            BoundsViolation::BufferOverread { offset, .. } => *offset as u32,
            BoundsViolation::BufferOverwrite { offset, .. } => *offset as u32,
            BoundsViolation::IndexOutOfBounds { index, .. } => *index as u32,
            _ => 0,
        };
        self.last_violation_offset.store(offset, Ordering::Release);

        // Handle strict mode
        if self.is_strict() {
            panic!("BoundsChecker: {}", violation);
        }
    }

    // ========================================================================
    // Statistics and Queries
    // ========================================================================

    /// Get total number of bounds checks performed
    pub fn total_checks(&self) -> u64 {
        self.total_checks.load(Ordering::Acquire)
    }

    /// Get total number of violations
    pub fn violation_count(&self) -> u64 {
        self.violations.load(Ordering::Acquire)
    }

    /// Check if any violations have occurred
    pub fn has_violations(&self) -> bool {
        self.has_flag(bounds_flags::HAS_VIOLATIONS)
    }

    /// Check if any critical violations have occurred
    pub fn has_critical_violations(&self) -> bool {
        self.has_flag(bounds_flags::HAS_CRITICAL)
    }

    /// Get last violation type code (0 = none)
    pub fn last_violation_code(&self) -> u8 {
        self.last_violation_type.load(Ordering::Acquire) as u8
    }

    /// Get read violation count
    pub fn read_violations(&self) -> u32 {
        self.read_violations.load(Ordering::Acquire)
    }

    /// Get write violation count
    pub fn write_violations(&self) -> u32 {
        self.write_violations.load(Ordering::Acquire)
    }

    /// Get index violation count
    pub fn index_violations(&self) -> u32 {
        self.index_violations.load(Ordering::Acquire)
    }

    /// Get motion vector violation count
    pub fn mv_violations(&self) -> u32 {
        self.mv_violations.load(Ordering::Acquire)
    }

    /// Get critical violation count
    pub fn critical_violations(&self) -> u32 {
        self.critical_violations.load(Ordering::Acquire)
    }

    /// Get null pointer violation count
    pub fn null_violations(&self) -> u32 {
        self.null_violations.load(Ordering::Acquire)
    }

    /// Get generation counter (Q34 audit)
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get complete statistics snapshot
    pub fn stats(&self) -> BoundsCheckerStats {
        let total = self.total_checks.load(Ordering::Acquire);
        let violations = self.violations.load(Ordering::Acquire);

        BoundsCheckerStats {
            total_checks: total,
            violations,
            violation_rate: if total > 0 {
                (violations as f64) / (total as f64)
            } else {
                0.0
            },
            read_violations: self.read_violations.load(Ordering::Acquire),
            write_violations: self.write_violations.load(Ordering::Acquire),
            index_violations: self.index_violations.load(Ordering::Acquire),
            mv_violations: self.mv_violations.load(Ordering::Acquire),
            critical_violations: self.critical_violations.load(Ordering::Acquire),
            null_violations: self.null_violations.load(Ordering::Acquire),
            checking_enabled: self.is_checking_enabled(),
            strict_mode: self.is_strict(),
            frame_width: self.frame_width(),
            frame_height: self.frame_height(),
        }
    }

    /// Reset all statistics (but not configuration)
    pub fn reset_stats(&self) {
        // Increment generation for audit trail
        self.generation.fetch_add(1, Ordering::AcqRel);

        self.total_checks.store(0, Ordering::Release);
        self.violations.store(0, Ordering::Release);
        self.last_violation_type.store(0, Ordering::Release);
        self.last_violation_offset.store(0, Ordering::Release);
        self.read_violations.store(0, Ordering::Release);
        self.write_violations.store(0, Ordering::Release);
        self.index_violations.store(0, Ordering::Release);
        self.mv_violations.store(0, Ordering::Release);
        self.critical_violations.store(0, Ordering::Release);
        self.null_violations.store(0, Ordering::Release);

        // Clear violation flags but keep config flags
        self.clear_flag(bounds_flags::HAS_VIOLATIONS);
        self.clear_flag(bounds_flags::HAS_CRITICAL);
    }

    /// Get raw state flags (for debugging/testing)
    pub fn raw_state(&self) -> u64 {
        self.state.load(Ordering::Acquire)
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    #[inline]
    fn set_flag(&self, flag: u64) {
        self.state.fetch_or(flag, Ordering::AcqRel);
    }

    #[inline]
    fn clear_flag(&self, flag: u64) {
        self.state.fetch_and(!flag, Ordering::AcqRel);
    }

    #[inline]
    fn has_flag(&self, flag: u64) -> bool {
        self.state.load(Ordering::Acquire) & flag != 0
    }
}

impl Default for BoundsCheckerCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Ensure Send + Sync (required for concurrent use)
unsafe impl Send for BoundsCheckerCapsule {}
unsafe impl Sync for BoundsCheckerCapsule {}

// ============================================================================
// Statistics Snapshot
// ============================================================================

/// Snapshot of bounds checker statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct BoundsCheckerStats {
    /// Total bounds checks performed
    pub total_checks: u64,
    /// Total violations detected
    pub violations: u64,
    /// Violation rate (violations / checks)
    pub violation_rate: f64,
    /// Read violation count
    pub read_violations: u32,
    /// Write violation count
    pub write_violations: u32,
    /// Index violation count
    pub index_violations: u32,
    /// Motion vector violation count
    pub mv_violations: u32,
    /// Critical violation count
    pub critical_violations: u32,
    /// Null pointer violation count
    pub null_violations: u32,
    /// Whether checking is enabled
    pub checking_enabled: bool,
    /// Whether strict mode is enabled
    pub strict_mode: bool,
    /// Frame width (if set)
    pub frame_width: u32,
    /// Frame height (if set)
    pub frame_height: u32,
}

// ============================================================================
// Feature-Gated Macros for Zero-Cost Bounds Checking
// ============================================================================

/// Bounds check macro that compiles to no-op when bounds-checking feature is disabled
///
/// # Usage
/// ```ignore
/// bounds_check_read!(checker, offset, length, buffer_size)?;
/// ```
#[macro_export]
#[cfg(feature = "bounds-checking")]
macro_rules! bounds_check_read {
    ($checker:expr, $offset:expr, $len:expr, $buf_size:expr) => {
        $checker.check_read($offset, $len, $buf_size)?
    };
}

#[macro_export]
#[cfg(not(feature = "bounds-checking"))]
macro_rules! bounds_check_read {
    ($checker:expr, $offset:expr, $len:expr, $buf_size:expr) => {
        ()
    };
}

/// Bounds check macro for writes
#[macro_export]
#[cfg(feature = "bounds-checking")]
macro_rules! bounds_check_write {
    ($checker:expr, $offset:expr, $len:expr, $buf_size:expr) => {
        $checker.check_write($offset, $len, $buf_size)?
    };
}

#[macro_export]
#[cfg(not(feature = "bounds-checking"))]
macro_rules! bounds_check_write {
    ($checker:expr, $offset:expr, $len:expr, $buf_size:expr) => {
        ()
    };
}

/// Bounds check macro for array indices
#[macro_export]
#[cfg(feature = "bounds-checking")]
macro_rules! bounds_check_index {
    ($checker:expr, $index:expr, $max:expr) => {
        $checker.check_index($index, $max)?
    };
}

#[macro_export]
#[cfg(not(feature = "bounds-checking"))]
macro_rules! bounds_check_index {
    ($checker:expr, $index:expr, $max:expr) => {
        ()
    };
}

// ============================================================================
// Tests (T28 Compliant: Q1-Q7 Unit, Q8-Q14 Property, Q15-Q21 Integration,
//        Q22-Q28 Production, Q29-Q35 Determinism)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    // ========================================================================
    // Q1-Q7: Unit Tests
    // ========================================================================

    // Q1: test_capsule_creation
    #[test]
    fn test_capsule_creation() {
        let checker = BoundsCheckerCapsule::new();

        assert_eq!(checker.generation(), 0);
        assert_eq!(checker.total_checks(), 0);
        assert_eq!(checker.violation_count(), 0);
        assert!(checker.is_checking_enabled());
        assert!(!checker.is_strict());
        assert!(!checker.has_violations());
    }

    // Q2: test_capsule_size_and_alignment
    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<BoundsCheckerCapsule>(), 128);
        assert_eq!(core::mem::align_of::<BoundsCheckerCapsule>(), 128);
    }

    // Q3: test_strict_mode_creation
    #[test]
    fn test_strict_mode_creation() {
        let checker = BoundsCheckerCapsule::strict();

        assert!(checker.is_strict());
        assert!(checker.is_checking_enabled());
    }

    // Q4: test_valid_read_check
    #[test]
    fn test_valid_read_check() {
        let checker = BoundsCheckerCapsule::new();

        // Valid reads
        assert!(checker.check_read(0, 10, 100).is_ok());
        assert!(checker.check_read(90, 10, 100).is_ok());
        assert!(checker.check_read(0, 100, 100).is_ok());
        assert!(checker.check_read(50, 0, 100).is_ok()); // Zero-length read

        assert_eq!(checker.total_checks(), 4);
        assert_eq!(checker.violation_count(), 0);
    }

    // Q5: test_invalid_read_check
    #[test]
    fn test_invalid_read_check() {
        let checker = BoundsCheckerCapsule::new();

        // Invalid reads
        let result = checker.check_read(90, 20, 100);
        assert!(result.is_err());

        if let Err(BoundsViolation::BufferOverread {
            offset,
            length,
            buffer_size,
        }) = result
        {
            assert_eq!(offset, 90);
            assert_eq!(length, 20);
            assert_eq!(buffer_size, 100);
        } else {
            panic!("Expected BufferOverread");
        }

        assert_eq!(checker.read_violations(), 1);
        assert!(checker.has_violations());
    }

    // Q6: test_valid_write_check
    #[test]
    fn test_valid_write_check() {
        let checker = BoundsCheckerCapsule::new();

        assert!(checker.check_write(0, 10, 100).is_ok());
        assert!(checker.check_write(90, 10, 100).is_ok());

        assert_eq!(checker.total_checks(), 2);
        assert_eq!(checker.write_violations(), 0);
    }

    // Q7: test_invalid_write_check
    #[test]
    fn test_invalid_write_check() {
        let checker = BoundsCheckerCapsule::new();

        let result = checker.check_write(95, 10, 100);
        assert!(result.is_err());

        if let Err(BoundsViolation::BufferOverwrite { .. }) = result {
            // Expected
        } else {
            panic!("Expected BufferOverwrite");
        }

        assert_eq!(checker.write_violations(), 1);
        assert!(checker.has_violations());

        // Write violations are critical
        assert!(checker.has_critical_violations());
    }

    // ========================================================================
    // Q8-Q14: Property Tests (Edge Cases, Boundary Conditions)
    // ========================================================================

    // Q8: test_index_bounds
    #[test]
    fn test_index_bounds() {
        let checker = BoundsCheckerCapsule::new();

        // Valid indices
        assert!(checker.check_index(0, 10).is_ok());
        assert!(checker.check_index(9, 10).is_ok());

        // Invalid indices
        assert!(checker.check_index(10, 10).is_err());
        assert!(checker.check_index(100, 10).is_err());

        assert_eq!(checker.index_violations(), 2);
    }

    // Q9: test_slice_bounds
    #[test]
    fn test_slice_bounds() {
        let checker = BoundsCheckerCapsule::new();
        let data = [0u8; 100];

        // Valid ranges
        assert!(checker.check_slice_bounds(&data, 0, 100).is_ok());
        assert!(checker.check_slice_bounds(&data, 0, 50).is_ok());
        assert!(checker.check_slice_bounds(&data, 50, 100).is_ok());
        assert!(checker.check_slice_bounds(&data, 50, 50).is_ok()); // Empty range

        // Invalid ranges
        assert!(checker.check_slice_bounds(&data, 0, 101).is_err());
        assert!(checker.check_slice_bounds(&data, 50, 40).is_err()); // start > end
    }

    // Q10: test_offset_checks
    #[test]
    fn test_offset_checks() {
        let checker = BoundsCheckerCapsule::new();

        // Valid offsets
        assert!(checker.check_offset(0).is_ok());
        assert!(checker.check_offset(100).is_ok());
        assert!(checker.check_offset(i64::MAX).is_ok());

        // Invalid offsets
        assert!(checker.check_offset(-1).is_err());
        assert!(checker.check_offset(-100).is_err());
        assert!(checker.check_offset(i64::MIN).is_err());

        // Check violation type
        if let Err(BoundsViolation::NegativeOffset { offset }) = checker.check_offset(-42) {
            assert_eq!(offset, -42);
        }
    }

    // Q11: test_null_pointer_check
    #[test]
    fn test_null_pointer_check() {
        let checker = BoundsCheckerCapsule::new();

        // Non-null pointer
        let value: u32 = 42;
        assert!(checker.check_non_null(&value as *const u32).is_ok());

        // Null pointer
        let null_ptr: *const u32 = std::ptr::null();
        assert!(checker.check_non_null(null_ptr).is_err());

        assert_eq!(checker.null_violations(), 1);
    }

    // Q12: test_alignment_check
    #[test]
    fn test_alignment_check() {
        let checker = BoundsCheckerCapsule::new();

        // Create aligned and unaligned pointers
        let aligned_16: [u8; 32] = [0; 32];
        let ptr = aligned_16.as_ptr();

        // Check various alignments
        assert!(checker.check_alignment(ptr, 1).is_ok());

        // Test with explicit aligned pointer
        let aligned_value: u64 = 0;
        let aligned_ptr = &aligned_value as *const u64;
        assert!(checker.check_alignment(aligned_ptr, 8).is_ok());
    }

    // Q13: test_motion_vector_bounds
    #[test]
    fn test_motion_vector_bounds() {
        let checker = BoundsCheckerCapsule::new();

        // Valid MV (block stays within frame)
        assert!(checker
            .check_mv_bounds(0, 0, 10, 10, 16, 16, 1920, 1080)
            .is_ok());
        assert!(checker
            .check_mv_bounds(100, 100, 0, 0, 16, 16, 1920, 1080)
            .is_ok());

        // Invalid MV (negative direction)
        assert!(checker
            .check_mv_bounds(0, 0, -1, 0, 16, 16, 1920, 1080)
            .is_err());
        assert!(checker
            .check_mv_bounds(0, 0, 0, -1, 16, 16, 1920, 1080)
            .is_err());

        // Invalid MV (exceeds frame)
        assert!(checker
            .check_mv_bounds(1900, 1060, 20, 20, 16, 16, 1920, 1080)
            .is_err());

        assert_eq!(checker.mv_violations(), 3);
    }

    // Q14: test_generation_counter
    #[test]
    fn test_generation_counter() {
        let checker = BoundsCheckerCapsule::new();
        assert_eq!(checker.generation(), 0);

        // Generation increments on violations
        let _ = checker.check_read(100, 10, 50);
        assert_eq!(checker.generation(), 1);

        // Generation increments on config changes
        checker.set_strict_mode(true);
        assert_eq!(checker.generation(), 2);

        checker.set_frame_bounds(1920, 1080);
        assert_eq!(checker.generation(), 3);

        // Reset stats increments generation
        checker.reset_stats();
        assert_eq!(checker.generation(), 4);
    }

    // ========================================================================
    // Q15-Q21: Integration Tests (Full Workflow)
    // ========================================================================

    // Q15: test_full_decode_workflow
    #[test]
    fn test_full_decode_workflow() {
        let checker = BoundsCheckerCapsule::new();
        checker.set_frame_bounds(1920, 1080);

        // Simulate frame decode workflow
        let bitstream = [0u8; 1000];
        let frame_buffer = [0u8; 1920 * 1080];

        // 1. Bitstream parsing
        assert!(checker.check_bits_available(32, 8000).is_ok());

        // 2. Coefficient reading
        assert!(checker.check_read(0, 100, bitstream.len()).is_ok());

        // 3. Reference frame access
        assert!(checker.check_ref_frame(0, 8).is_ok());
        assert!(checker.check_ref_frame(7, 8).is_ok());

        // 4. Motion compensation
        assert!(checker
            .check_mv_bounds(100, 100, 5, 5, 16, 16, 1920, 1080)
            .is_ok());

        // 5. Frame buffer write
        assert!(checker
            .check_write(0, 1920 * 16, frame_buffer.len())
            .is_ok());

        // 6. Tile access
        assert!(checker.check_tile_bounds(0, 0, 4, 4).is_ok());
        assert!(checker.check_tile_bounds(3, 3, 4, 4).is_ok());

        let stats = checker.stats();
        assert_eq!(stats.violations, 0);
        assert!(stats.total_checks >= 8);
    }

    // Q16: test_bitstream_underflow
    #[test]
    fn test_bitstream_underflow() {
        let checker = BoundsCheckerCapsule::new();

        // Valid bitstream reads
        assert!(checker.check_bits_available(1, 100).is_ok());
        assert!(checker.check_bits_available(100, 100).is_ok());

        // Underflow
        let result = checker.check_bits_available(101, 100);
        assert!(result.is_err());

        if let Err(BoundsViolation::BitstreamUnderflow {
            bits_needed,
            bits_remaining,
        }) = result
        {
            assert_eq!(bits_needed, 101);
            assert_eq!(bits_remaining, 100);
        }
    }

    // Q17: test_tile_bounds
    #[test]
    fn test_tile_bounds() {
        let checker = BoundsCheckerCapsule::new();

        // 4x4 tile grid
        assert!(checker.check_tile_bounds(0, 0, 4, 4).is_ok());
        assert!(checker.check_tile_bounds(3, 3, 4, 4).is_ok());

        // Out of bounds
        assert!(checker.check_tile_bounds(4, 0, 4, 4).is_err());
        assert!(checker.check_tile_bounds(0, 4, 4, 4).is_err());

        if let Err(BoundsViolation::TileOutOfBounds {
            tile_col,
            tile_row,
            max_cols,
            max_rows,
        }) = checker.check_tile_bounds(5, 6, 4, 4)
        {
            assert_eq!(tile_col, 5);
            assert_eq!(tile_row, 6);
            assert_eq!(max_cols, 4);
            assert_eq!(max_rows, 4);
        }
    }

    // Q18: test_reference_frame_check
    #[test]
    fn test_reference_frame_check() {
        let checker = BoundsCheckerCapsule::new();

        // AV1 allows up to 8 reference frames
        for i in 0..8 {
            assert!(checker.check_ref_frame(i, 8).is_ok());
        }

        // Invalid references
        assert!(checker.check_ref_frame(8, 8).is_err());
        assert!(checker.check_ref_frame(100, 8).is_err());

        if let Err(BoundsViolation::InvalidRefFrame { ref_idx, max_refs }) =
            checker.check_ref_frame(10, 8)
        {
            assert_eq!(ref_idx, 10);
            assert_eq!(max_refs, 8);
        }
    }

    // Q19: test_checking_disabled
    #[test]
    fn test_checking_disabled() {
        let checker = BoundsCheckerCapsule::new();
        checker.set_checking_enabled(false);

        // All checks should pass (no-op)
        assert!(checker.check_read(1000, 1000, 10).is_ok());
        assert!(checker.check_write(1000, 1000, 10).is_ok());
        assert!(checker.check_index(1000, 10).is_ok());

        // No checks should be counted
        assert_eq!(checker.total_checks(), 0);
        assert_eq!(checker.violation_count(), 0);
    }

    // Q20: test_frame_bounds_storage
    #[test]
    fn test_frame_bounds_storage() {
        let checker = BoundsCheckerCapsule::new();

        checker.set_frame_bounds(1920, 1080);
        assert_eq!(checker.frame_width(), 1920);
        assert_eq!(checker.frame_height(), 1080);

        checker.set_frame_bounds(3840, 2160);
        assert_eq!(checker.frame_width(), 3840);
        assert_eq!(checker.frame_height(), 2160);

        // Max 16-bit values
        checker.set_frame_bounds(65535, 65535);
        assert_eq!(checker.frame_width(), 65535);
        assert_eq!(checker.frame_height(), 65535);

        // Clamped to 16-bit
        checker.set_frame_bounds(100000, 100000);
        assert_eq!(checker.frame_width(), 65535);
        assert_eq!(checker.frame_height(), 65535);
    }

    // Q21: test_stats_snapshot
    #[test]
    fn test_stats_snapshot() {
        let checker = BoundsCheckerCapsule::new();
        checker.set_frame_bounds(1920, 1080);
        checker.set_strict_mode(false);

        // Generate some stats
        for _ in 0..100 {
            let _ = checker.check_read(0, 10, 100);
        }
        let _ = checker.check_read(90, 20, 100); // Violation

        let stats = checker.stats();

        assert_eq!(stats.total_checks, 101);
        assert_eq!(stats.violations, 1);
        assert_eq!(stats.read_violations, 1);
        assert!((stats.violation_rate - 0.0099).abs() < 0.001);
        assert!(stats.checking_enabled);
        assert!(!stats.strict_mode);
        assert_eq!(stats.frame_width, 1920);
        assert_eq!(stats.frame_height, 1080);
    }

    // ========================================================================
    // Q22-Q28: Production Tests (Real Scenarios)
    // ========================================================================

    // Q22: test_av1_decode_simulation
    #[test]
    fn test_av1_decode_simulation() {
        let checker = BoundsCheckerCapsule::new();
        checker.set_frame_bounds(1920, 1080);

        // Simulate decoding 100 blocks
        for block_y in 0..10 {
            for block_x in 0..10 {
                let base_x = block_x * 64;
                let base_y = block_y * 64;

                // Motion vector check
                let mv_x = (block_x as i32 - 5) * 4; // Random-ish MV
                let mv_y = (block_y as i32 - 5) * 4;

                // Only check MVs that could be valid
                if base_x as i32 + mv_x >= 0 && base_y as i32 + mv_y >= 0 {
                    let _ = checker.check_mv_bounds(
                        base_x as i32,
                        base_y as i32,
                        mv_x,
                        mv_y,
                        64,
                        64,
                        1920,
                        1080,
                    );
                }

                // Reference frame check
                let _ = checker.check_ref_frame(block_x % 8, 8);
            }
        }

        let stats = checker.stats();
        assert!(stats.total_checks > 0);
    }

    // Q23: test_concurrent_access
    #[test]
    fn test_concurrent_access() {
        let checker = Arc::new(BoundsCheckerCapsule::new());
        let mut handles = vec![];

        // Spawn 4 threads doing bounds checks
        for _ in 0..4 {
            let c = Arc::clone(&checker);
            handles.push(thread::spawn(move || {
                for i in 0..1000 {
                    let _ = c.check_read(i % 100, 10, 100);
                    let _ = c.check_index(i % 50, 50);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // Should have 8000 total checks (4 threads * 1000 iterations * 2 checks)
        assert_eq!(checker.total_checks(), 8000);
    }

    // Q24: test_violation_error_messages
    #[test]
    fn test_violation_error_messages() {
        let violations = vec![
            BoundsViolation::BufferOverread {
                offset: 90,
                length: 20,
                buffer_size: 100,
            },
            BoundsViolation::BufferOverwrite {
                offset: 90,
                length: 20,
                buffer_size: 100,
            },
            BoundsViolation::NegativeOffset { offset: -42 },
            BoundsViolation::IndexOutOfBounds { index: 10, max: 5 },
            BoundsViolation::MVOutOfFrame {
                mv_x: 100,
                mv_y: 200,
                frame_width: 1920,
                frame_height: 1080,
            },
            BoundsViolation::NullPointer,
            BoundsViolation::AlignmentViolation {
                address: 0x1001,
                required: 8,
            },
            BoundsViolation::BitstreamUnderflow {
                bits_needed: 100,
                bits_remaining: 50,
            },
            BoundsViolation::TileOutOfBounds {
                tile_col: 5,
                tile_row: 6,
                max_cols: 4,
                max_rows: 4,
            },
            BoundsViolation::InvalidRefFrame {
                ref_idx: 10,
                max_refs: 8,
            },
        ];

        for v in violations {
            let msg = v.to_string();
            assert!(!msg.is_empty());
            println!("{}: {}", v.code(), msg);
        }
    }

    // Q25: test_check_type_classification
    #[test]
    fn test_check_type_classification() {
        // Read operations
        assert!(BoundsCheckType::ReadByte.is_read());
        assert!(BoundsCheckType::ReadSlice.is_read());
        assert!(BoundsCheckType::ReadBitstream.is_read());
        assert!(!BoundsCheckType::ReadByte.is_write());

        // Write operations
        assert!(BoundsCheckType::WriteByte.is_write());
        assert!(BoundsCheckType::WriteSlice.is_write());
        assert!(BoundsCheckType::WriteFrame.is_write());
        assert!(!BoundsCheckType::WriteByte.is_read());

        // Index operations
        assert!(BoundsCheckType::ArrayIndex.is_index());
        assert!(BoundsCheckType::PointerOffset.is_index());

        // Video operations
        assert!(BoundsCheckType::MotionVector.is_video());
        assert!(BoundsCheckType::TileIndex.is_video());
        assert!(BoundsCheckType::RefFrameIndex.is_video());
    }

    // Q26: test_reset_stats
    #[test]
    fn test_reset_stats() {
        let checker = BoundsCheckerCapsule::new();

        // Generate some violations
        let _ = checker.check_read(100, 10, 50);
        let _ = checker.check_write(100, 10, 50);
        let _ = checker.check_index(100, 50);

        assert!(checker.violation_count() > 0);
        let gen_before = checker.generation();

        checker.reset_stats();

        assert_eq!(checker.total_checks(), 0);
        assert_eq!(checker.violation_count(), 0);
        assert_eq!(checker.read_violations(), 0);
        assert_eq!(checker.write_violations(), 0);
        assert_eq!(checker.index_violations(), 0);
        assert!(!checker.has_violations());
        assert!(!checker.has_critical_violations());

        // Generation should increment
        assert!(checker.generation() > gen_before);
    }

    // Q27: test_overflow_protection
    #[test]
    fn test_overflow_protection() {
        let checker = BoundsCheckerCapsule::new();

        // Test arithmetic overflow in bounds check
        let result = checker.check_read(usize::MAX, 1, 100);
        assert!(result.is_err());

        let result = checker.check_read(usize::MAX - 5, 10, 100);
        assert!(result.is_err());
    }

    // Q28: test_last_violation_tracking
    #[test]
    fn test_last_violation_tracking() {
        let checker = BoundsCheckerCapsule::new();

        // Initial state
        assert_eq!(checker.last_violation_code(), 0);

        // Buffer overread
        let _ = checker.check_read(100, 10, 50);
        assert_eq!(
            checker.last_violation_code(),
            BoundsViolation::BufferOverread {
                offset: 0,
                length: 0,
                buffer_size: 0
            }
            .code()
        );

        // Index out of bounds (overwrites last)
        let _ = checker.check_index(100, 50);
        assert_eq!(
            checker.last_violation_code(),
            BoundsViolation::IndexOutOfBounds { index: 0, max: 0 }.code()
        );
    }

    // ========================================================================
    // Q29-Q35: Determinism Tests (Reproducibility, Consistency)
    // ========================================================================

    // Q29: test_deterministic_violation_counts
    #[test]
    fn test_deterministic_violation_counts() {
        // Run the same sequence twice, should get same results
        for _ in 0..2 {
            let checker = BoundsCheckerCapsule::new();

            let _ = checker.check_read(50, 60, 100);
            let _ = checker.check_write(50, 60, 100);
            let _ = checker.check_index(10, 5);
            let _ = checker.check_mv_bounds(0, 0, -10, 0, 16, 16, 1920, 1080);

            assert_eq!(checker.violation_count(), 4);
            assert_eq!(checker.read_violations(), 1);
            assert_eq!(checker.write_violations(), 1);
            assert_eq!(checker.index_violations(), 1);
            assert_eq!(checker.mv_violations(), 1);
        }
    }

    // Q30: test_violation_code_stability
    #[test]
    fn test_violation_code_stability() {
        // Violation codes should be stable for serialization/audit
        assert_eq!(
            BoundsViolation::BufferOverread {
                offset: 0,
                length: 0,
                buffer_size: 0
            }
            .code(),
            1
        );
        assert_eq!(
            BoundsViolation::BufferOverwrite {
                offset: 0,
                length: 0,
                buffer_size: 0
            }
            .code(),
            2
        );
        assert_eq!(BoundsViolation::NegativeOffset { offset: 0 }.code(), 3);
        assert_eq!(
            BoundsViolation::IndexOutOfBounds { index: 0, max: 0 }.code(),
            4
        );
        assert_eq!(
            BoundsViolation::MVOutOfFrame {
                mv_x: 0,
                mv_y: 0,
                frame_width: 0,
                frame_height: 0
            }
            .code(),
            5
        );
        assert_eq!(BoundsViolation::NullPointer.code(), 6);
        assert_eq!(
            BoundsViolation::AlignmentViolation {
                address: 0,
                required: 0
            }
            .code(),
            7
        );
        assert_eq!(
            BoundsViolation::BitstreamUnderflow {
                bits_needed: 0,
                bits_remaining: 0
            }
            .code(),
            8
        );
        assert_eq!(
            BoundsViolation::TileOutOfBounds {
                tile_col: 0,
                tile_row: 0,
                max_cols: 0,
                max_rows: 0
            }
            .code(),
            9
        );
        assert_eq!(
            BoundsViolation::InvalidRefFrame {
                ref_idx: 0,
                max_refs: 0
            }
            .code(),
            10
        );
    }

    // Q31: test_critical_violation_classification
    #[test]
    fn test_critical_violation_classification() {
        // Critical violations (security-relevant)
        assert!(BoundsViolation::BufferOverwrite {
            offset: 0,
            length: 0,
            buffer_size: 0
        }
        .is_critical());
        assert!(BoundsViolation::NullPointer.is_critical());
        assert!(BoundsViolation::NegativeOffset { offset: 0 }.is_critical());

        // Non-critical violations
        assert!(!BoundsViolation::BufferOverread {
            offset: 0,
            length: 0,
            buffer_size: 0
        }
        .is_critical());
        assert!(!BoundsViolation::IndexOutOfBounds { index: 0, max: 0 }.is_critical());
        assert!(!BoundsViolation::MVOutOfFrame {
            mv_x: 0,
            mv_y: 0,
            frame_width: 0,
            frame_height: 0
        }
        .is_critical());
    }

    // Q32: test_check_type_values
    #[test]
    fn test_check_type_values() {
        // Check type enum values should be stable
        assert_eq!(BoundsCheckType::ReadByte as u8, 0x00);
        assert_eq!(BoundsCheckType::ReadSlice as u8, 0x01);
        assert_eq!(BoundsCheckType::ReadBitstream as u8, 0x02);
        assert_eq!(BoundsCheckType::WriteByte as u8, 0x10);
        assert_eq!(BoundsCheckType::WriteSlice as u8, 0x11);
        assert_eq!(BoundsCheckType::WriteFrame as u8, 0x12);
        assert_eq!(BoundsCheckType::ArrayIndex as u8, 0x20);
        assert_eq!(BoundsCheckType::PointerOffset as u8, 0x21);
        assert_eq!(BoundsCheckType::MotionVector as u8, 0x30);
        assert_eq!(BoundsCheckType::TileIndex as u8, 0x31);
        assert_eq!(BoundsCheckType::RefFrameIndex as u8, 0x32);
    }

    // Q33: test_check_type_display
    #[test]
    fn test_check_type_display() {
        assert_eq!(format!("{}", BoundsCheckType::ReadByte), "ReadByte");
        assert_eq!(format!("{}", BoundsCheckType::WriteSlice), "WriteSlice");
        assert_eq!(format!("{}", BoundsCheckType::MotionVector), "MotionVector");
    }

    // Q34: test_generation_monotonic
    #[test]
    fn test_generation_monotonic() {
        let checker = BoundsCheckerCapsule::new();
        let mut last_gen = checker.generation();

        // Perform various operations that increment generation
        for _ in 0..10 {
            let _ = checker.check_read(100, 10, 50); // Violation increments
            let current = checker.generation();
            assert!(current > last_gen);
            last_gen = current;
        }

        checker.set_strict_mode(false);
        assert!(checker.generation() > last_gen);
        last_gen = checker.generation();

        checker.reset_stats();
        assert!(checker.generation() > last_gen);
    }

    // Q35: test_send_sync_bounds
    #[test]
    fn test_send_sync_bounds() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<BoundsCheckerCapsule>();
        assert_sync::<BoundsCheckerCapsule>();
    }

    // ========================================================================
    // Additional Edge Case Tests
    // ========================================================================

    #[test]
    fn test_zero_length_operations() {
        let checker = BoundsCheckerCapsule::new();

        // Zero-length operations should always succeed
        assert!(checker.check_read(0, 0, 0).is_ok());
        assert!(checker.check_read(100, 0, 100).is_ok());
        assert!(checker.check_write(0, 0, 0).is_ok());
        assert!(checker.check_bits_available(0, 0).is_ok());
    }

    #[test]
    fn test_max_frame_dimensions() {
        let checker = BoundsCheckerCapsule::new();

        // 8K resolution
        checker.set_frame_bounds(7680, 4320);
        assert_eq!(checker.frame_width(), 7680);
        assert_eq!(checker.frame_height(), 4320);

        // MV check with 8K frame
        assert!(checker
            .check_mv_bounds(0, 0, 100, 100, 64, 64, 7680, 4320)
            .is_ok());
    }

    #[test]
    fn test_default_trait() {
        let checker = BoundsCheckerCapsule::default();
        assert!(checker.is_checking_enabled());
        assert!(!checker.is_strict());
    }
}
