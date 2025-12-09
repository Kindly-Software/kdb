//! AV1 Tile Group OBU Parser Capsule
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Implements AOM AV1 Specification Section 5.11 (tile_group_obu) for parsing
//! tile group headers and extracting tile data for parallel decoding.
//!
//! # Architecture
//!
//! T4 Batch tier capsule (512B cache-aligned) for parallel tile group processing.
//! Supports 1-4096 tiles (64x64 max grid) with lockfree offset tracking.
//!
//! ```text
//! Av1TileGroupCapsule (T4 Batch, 512B aligned)
//! +-------------------------------------------------------------------------+
//! |  state: AtomicU64           - decoder state | tile_start_and_end_present|
//! |  generation: AtomicU64      - Q34 audit trail generation counter        |
//! |  tile_cols: AtomicU32       - number of tile columns                    |
//! |  tile_rows: AtomicU32       - number of tile rows                       |
//! |  tile_cols_log2: AtomicU32  - log2(tile_cols)                           |
//! |  tile_rows_log2: AtomicU32  - log2(tile_rows)                           |
//! |  context_update_tile_id: AtomicU32 - tile to update context from        |
//! |  uniform_tile_spacing: AtomicU32   - uniform spacing flag               |
//! |  tile_width_sb: AtomicU32   - tile width in superblocks                 |
//! |  tile_height_sb: AtomicU32  - tile height in superblocks                |
//! |  tg_start: AtomicU32        - first tile in this tile group             |
//! |  tg_end: AtomicU32          - last tile in this tile group              |
//! |  tile_offsets: [AtomicU64; 32] - packed (offset:40, size:24) for tiles  |
//! |  tiles_decoded: AtomicU64   - count of tiles decoded                    |
//! |  bytes_processed: AtomicU64 - total bytes processed                     |
//! |  _padding: [u8; N]          - pad to 512B                               |
//! +-------------------------------------------------------------------------+
//! ```
//!
//! # AV1 Tile Structure
//!
//! AV1 frames can contain multiple tile groups, each with:
//! - Optional tg_start/tg_end signaling (tile_start_and_end_present_flag)
//! - Variable-length tile size coding (tile_size_bytes_minus_1 + 1)
//! - Superblock-aligned tile boundaries
//!
//! # UCE34/Chaos Compliance
//!
//! - **Q10**: T4 Batch tier (parallel tile processing, 10-100x speedup)
//! - **Q33**: 100% lockfree (AtomicU64/AtomicU32 only, no mutex/RwLock)
//! - **Q34**: Generation counter for audit trail integrity
//! - 512B cache-aligned to prevent false sharing

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// ============================================================================
// Constants
// ============================================================================

/// Maximum tile columns (AV1 spec: 2^6 = 64)
pub const AV1_MAX_TILE_COLS: u32 = 64;
/// Maximum tile rows (AV1 spec: 2^6 = 64)
pub const AV1_MAX_TILE_ROWS: u32 = 64;
/// Maximum total tiles (64x64 = 4096)
pub const AV1_MAX_TILES: u32 = 4096;
/// Maximum log2 of tile columns
pub const AV1_MAX_TILE_COLS_LOG2: u32 = 6;
/// Maximum log2 of tile rows
pub const AV1_MAX_TILE_ROWS_LOG2: u32 = 6;
/// Maximum tile size bytes field value (1-4)
pub const AV1_MAX_TILE_SIZE_BYTES: u32 = 4;
/// Inline tile offset storage capacity
pub const AV1_INLINE_TILE_OFFSETS: usize = 32;
/// Superblock size for 64x64 (most common)
pub const AV1_SB_SIZE_64: u32 = 64;
/// Superblock size for 128x128
pub const AV1_SB_SIZE_128: u32 = 128;
/// Minimum tile width in pixels
pub const AV1_MIN_TILE_WIDTH: u32 = 1;
/// Maximum tile width in superblocks (4096 >> 6 = 64 for 64x64 SB)
pub const AV1_MAX_TILE_WIDTH_SB: u32 = 64;
/// Maximum tile area in superblocks
pub const AV1_MAX_TILE_AREA_SB: u32 = 4096;

// ============================================================================
// State Flags
// ============================================================================

/// State flags for tile group decoder
pub mod state_flags {
    /// Tile info has been parsed from frame header
    pub const TILE_INFO_PARSED: u64 = 1 << 0;
    /// Tile group OBU header parsed
    pub const TG_HEADER_PARSED: u64 = 1 << 1;
    /// All tiles in group have been parsed
    pub const TG_TILES_PARSED: u64 = 1 << 2;
    /// Tile start and end present in OBU
    pub const TG_START_END_PRESENT: u64 = 1 << 3;
    /// Large scale tile mode (for 4K+ content)
    pub const LARGE_SCALE_TILE: u64 = 1 << 4;
    /// Context update tile has been decoded
    pub const CONTEXT_UPDATE_DONE: u64 = 1 << 5;
    /// Error occurred during parsing
    pub const ERROR_STATE: u64 = 1 << 6;
    /// Ready for parallel decode
    pub const READY_FOR_DECODE: u64 = TILE_INFO_PARSED | TG_HEADER_PARSED | TG_TILES_PARSED;
}

// ============================================================================
// Error Types
// ============================================================================

/// AV1 tile group parsing errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Av1TileGroupError {
    /// No error
    None = 0,
    /// Invalid tile column count
    InvalidTileCols = 1,
    /// Invalid tile row count
    InvalidTileRows = 2,
    /// Invalid tile group range (tg_start > tg_end)
    InvalidTileGroupRange = 3,
    /// Tile index out of bounds
    TileIndexOutOfBounds = 4,
    /// Buffer too small for tile data
    BufferTooSmall = 5,
    /// Invalid tile size encoding
    InvalidTileSize = 6,
    /// Tile info not yet parsed
    TileInfoNotParsed = 7,
    /// Invalid superblock size
    InvalidSuperblockSize = 8,
    /// Invalid frame dimensions
    InvalidFrameDimensions = 9,
    /// Tile group already parsed
    AlreadyParsed = 10,
    /// Bitstream read error
    BitstreamError = 11,
    /// Context update tile ID out of range
    InvalidContextUpdateTile = 12,
    /// Tile offset storage full
    TileOffsetStorageFull = 13,
    /// Invalid uniform tile spacing
    InvalidUniformSpacing = 14,
    /// Tile size mismatch
    TileSizeMismatch = 15,
}

impl core::fmt::Display for Av1TileGroupError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::None => write!(f, "no error"),
            Self::InvalidTileCols => write!(f, "invalid tile column count"),
            Self::InvalidTileRows => write!(f, "invalid tile row count"),
            Self::InvalidTileGroupRange => write!(f, "invalid tile group range (tg_start > tg_end)"),
            Self::TileIndexOutOfBounds => write!(f, "tile index out of bounds"),
            Self::BufferTooSmall => write!(f, "buffer too small for tile data"),
            Self::InvalidTileSize => write!(f, "invalid tile size encoding"),
            Self::TileInfoNotParsed => write!(f, "tile info not yet parsed"),
            Self::InvalidSuperblockSize => write!(f, "invalid superblock size"),
            Self::InvalidFrameDimensions => write!(f, "invalid frame dimensions"),
            Self::AlreadyParsed => write!(f, "tile group already parsed"),
            Self::BitstreamError => write!(f, "bitstream read error"),
            Self::InvalidContextUpdateTile => write!(f, "invalid context update tile ID"),
            Self::TileOffsetStorageFull => write!(f, "tile offset storage full"),
            Self::InvalidUniformSpacing => write!(f, "invalid uniform tile spacing"),
            Self::TileSizeMismatch => write!(f, "tile size mismatch"),
        }
    }
}

impl std::error::Error for Av1TileGroupError {}

// ============================================================================
// Statistics
// ============================================================================

/// Tile group parsing statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct Av1TileGroupStats {
    /// Total tile groups parsed
    pub tile_groups_parsed: u64,
    /// Total tiles parsed
    pub tiles_parsed: u64,
    /// Total bytes processed
    pub bytes_processed: u64,
    /// Current tile columns
    pub tile_cols: u32,
    /// Current tile rows
    pub tile_rows: u32,
    /// Current tg_start
    pub tg_start: u32,
    /// Current tg_end
    pub tg_end: u32,
    /// Average tile size in bytes
    pub avg_tile_size: u32,
    /// Generation counter
    pub generation: u64,
}

// ============================================================================
// Tile Coordinate Helper
// ============================================================================

/// Tile coordinates within the frame
#[derive(Debug, Clone, Copy, Default)]
pub struct Av1TileCoords {
    /// Tile column index
    pub col: u32,
    /// Tile row index
    pub row: u32,
    /// Pixel X offset
    pub x: u32,
    /// Pixel Y offset
    pub y: u32,
    /// Tile width in pixels
    pub width: u32,
    /// Tile height in pixels
    pub height: u32,
    /// Tile width in superblocks
    pub width_sb: u32,
    /// Tile height in superblocks
    pub height_sb: u32,
}

// ============================================================================
// Av1TileGroupCapsule - T4 Batch Tier
// ============================================================================

/// T4 Batch capsule for AV1 tile group parsing and coordination
///
/// This capsule manages parsing of tile_group_obu (Section 5.11) and provides
/// tile offset information for parallel decoding.
///
/// # Memory Layout (512B cache-aligned)
///
/// ```text
/// Offset  Field                      Size    Description
/// ------  -----                      ----    -----------
/// 0       state                      8       Decoder state flags
/// 8       generation                 8       Q34 audit generation counter
/// 16      tile_cols                  4       Number of tile columns
/// 20      tile_rows                  4       Number of tile rows
/// 24      tile_cols_log2             4       log2(tile_cols)
/// 28      tile_rows_log2             4       log2(tile_rows)
/// 32      context_update_tile_id     4       Context update tile ID
/// 36      uniform_tile_spacing       4       Uniform spacing flag
/// 40      tile_width_sb              4       Tile width in superblocks
/// 44      tile_height_sb             4       Tile height in superblocks
/// 48      tg_start                   4       First tile in group
/// 52      tg_end                     4       Last tile in group
/// 56      tile_size_bytes            4       Tile size field width (1-4)
/// 60      sb_size                    4       Superblock size (64 or 128)
/// 64      frame_width                4       Frame width in pixels
/// 68      frame_height               4       Frame height in pixels
/// 72      mi_cols                    4       Frame width in 4x4 units
/// 76      mi_rows                    4       Frame height in 4x4 units
/// 80      tile_offsets[0..31]        256     Packed tile offsets (32 tiles)
/// 336     tiles_decoded              8       Tiles decoded count
/// 344     bytes_processed            8       Total bytes processed
/// 352     tile_groups_parsed         8       Tile groups parsed count
/// 360     error_code                 4       Last error code
/// 364     reserved                   4       Reserved for alignment
/// 368     _padding                   144     Padding to 512B
/// ```
#[repr(C, align(128))]
pub struct Av1TileGroupCapsule {
    // State and generation (16 bytes)
    /// Decoder state flags
    state: AtomicU64,
    /// Q34 audit trail generation counter
    generation: AtomicU64,

    // Tile grid configuration (16 bytes)
    /// Number of tile columns (1-64)
    tile_cols: AtomicU32,
    /// Number of tile rows (1-64)
    tile_rows: AtomicU32,
    /// log2(tile_cols)
    tile_cols_log2: AtomicU32,
    /// log2(tile_rows)
    tile_rows_log2: AtomicU32,

    // Tile configuration (16 bytes)
    /// Context update tile ID
    context_update_tile_id: AtomicU32,
    /// Uniform tile spacing flag
    uniform_tile_spacing: AtomicU32,
    /// Tile width in superblocks (uniform mode)
    tile_width_sb: AtomicU32,
    /// Tile height in superblocks (uniform mode)
    tile_height_sb: AtomicU32,

    // Tile group range (16 bytes)
    /// First tile index in this tile group
    tg_start: AtomicU32,
    /// Last tile index in this tile group (inclusive)
    tg_end: AtomicU32,
    /// Tile size field width minus 1 (0-3 -> 1-4 bytes)
    tile_size_bytes: AtomicU32,
    /// Superblock size (64 or 128)
    sb_size: AtomicU32,

    // Frame dimensions (16 bytes)
    /// Frame width in pixels
    frame_width: AtomicU32,
    /// Frame height in pixels
    frame_height: AtomicU32,
    /// Frame width in 4x4 MI units
    mi_cols: AtomicU32,
    /// Frame height in 4x4 MI units
    mi_rows: AtomicU32,

    // Tile offset storage (256 bytes) - packed (offset:40, size:24)
    /// Tile offsets: upper 40 bits = byte offset, lower 24 bits = size
    tile_offsets: [AtomicU64; AV1_INLINE_TILE_OFFSETS],

    // Statistics (24 bytes)
    /// Count of tiles successfully decoded
    tiles_decoded: AtomicU64,
    /// Total bytes processed
    bytes_processed: AtomicU64,
    /// Tile groups parsed
    tile_groups_parsed: AtomicU64,

    // Error tracking (8 bytes)
    /// Last error code
    error_code: AtomicU32,
    /// Reserved for alignment
    _reserved: AtomicU32,

    // Padding to 512 bytes
    _padding: [u8; 144],
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<Av1TileGroupCapsule>() == 512);
const _: () = assert!(core::mem::align_of::<Av1TileGroupCapsule>() == 128);

// Safety: Av1TileGroupCapsule only contains atomic types
unsafe impl Send for Av1TileGroupCapsule {}
unsafe impl Sync for Av1TileGroupCapsule {}

impl Default for Av1TileGroupCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl Av1TileGroupCapsule {
    /// Create a new tile group capsule
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            tile_cols: AtomicU32::new(1),
            tile_rows: AtomicU32::new(1),
            tile_cols_log2: AtomicU32::new(0),
            tile_rows_log2: AtomicU32::new(0),
            context_update_tile_id: AtomicU32::new(0),
            uniform_tile_spacing: AtomicU32::new(1),
            tile_width_sb: AtomicU32::new(0),
            tile_height_sb: AtomicU32::new(0),
            tg_start: AtomicU32::new(0),
            tg_end: AtomicU32::new(0),
            tile_size_bytes: AtomicU32::new(4),
            sb_size: AtomicU32::new(AV1_SB_SIZE_64),
            frame_width: AtomicU32::new(0),
            frame_height: AtomicU32::new(0),
            mi_cols: AtomicU32::new(0),
            mi_rows: AtomicU32::new(0),
            tile_offsets: [
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
            ],
            tiles_decoded: AtomicU64::new(0),
            bytes_processed: AtomicU64::new(0),
            tile_groups_parsed: AtomicU64::new(0),
            error_code: AtomicU32::new(0),
            _reserved: AtomicU32::new(0),
            _padding: [0u8; 144],
        }
    }

    // ========================================================================
    // Generation Counter (Q34 Audit)
    // ========================================================================

    /// Get current generation for Q34 audit trail
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Increment generation counter
    #[inline]
    fn bump_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    // ========================================================================
    // State Management
    // ========================================================================

    /// Get current state flags
    #[inline]
    pub fn state(&self) -> u64 {
        self.state.load(Ordering::Acquire)
    }

    /// Check if a specific state flag is set
    #[inline]
    pub fn has_state(&self, flag: u64) -> bool {
        (self.state() & flag) == flag
    }

    /// Set state flag
    #[inline]
    fn set_state_flag(&self, flag: u64) {
        self.state.fetch_or(flag, Ordering::AcqRel);
    }

    /// Clear state flag
    #[inline]
    fn clear_state_flag(&self, flag: u64) {
        self.state.fetch_and(!flag, Ordering::AcqRel);
    }

    /// Check if ready for parallel decode
    #[inline]
    pub fn is_ready_for_decode(&self) -> bool {
        self.has_state(state_flags::READY_FOR_DECODE)
    }

    /// Check if in error state
    #[inline]
    pub fn is_error(&self) -> bool {
        self.has_state(state_flags::ERROR_STATE)
    }

    /// Set error state
    fn set_error(&self, error: Av1TileGroupError) {
        self.error_code.store(error as u32, Ordering::Release);
        self.set_state_flag(state_flags::ERROR_STATE);
    }

    /// Get last error code
    #[inline]
    pub fn last_error(&self) -> Av1TileGroupError {
        let code = self.error_code.load(Ordering::Acquire);
        match code {
            0 => Av1TileGroupError::None,
            1 => Av1TileGroupError::InvalidTileCols,
            2 => Av1TileGroupError::InvalidTileRows,
            3 => Av1TileGroupError::InvalidTileGroupRange,
            4 => Av1TileGroupError::TileIndexOutOfBounds,
            5 => Av1TileGroupError::BufferTooSmall,
            6 => Av1TileGroupError::InvalidTileSize,
            7 => Av1TileGroupError::TileInfoNotParsed,
            8 => Av1TileGroupError::InvalidSuperblockSize,
            9 => Av1TileGroupError::InvalidFrameDimensions,
            10 => Av1TileGroupError::AlreadyParsed,
            11 => Av1TileGroupError::BitstreamError,
            12 => Av1TileGroupError::InvalidContextUpdateTile,
            13 => Av1TileGroupError::TileOffsetStorageFull,
            14 => Av1TileGroupError::InvalidUniformSpacing,
            15 => Av1TileGroupError::TileSizeMismatch,
            _ => Av1TileGroupError::None,
        }
    }

    // ========================================================================
    // Frame Configuration
    // ========================================================================

    /// Set frame dimensions
    ///
    /// Must be called before parse_tile_info with frame dimensions from
    /// the sequence header or frame header.
    pub fn set_frame_dimensions(
        &self,
        width: u32,
        height: u32,
        sb_size: u32,
    ) -> Result<(), Av1TileGroupError> {
        if width == 0 || height == 0 {
            return Err(Av1TileGroupError::InvalidFrameDimensions);
        }
        if sb_size != AV1_SB_SIZE_64 && sb_size != AV1_SB_SIZE_128 {
            return Err(Av1TileGroupError::InvalidSuperblockSize);
        }

        self.frame_width.store(width, Ordering::Release);
        self.frame_height.store(height, Ordering::Release);
        self.sb_size.store(sb_size, Ordering::Release);

        // Calculate MI dimensions (4x4 blocks)
        let mi_cols = (width + 3) >> 2;
        let mi_rows = (height + 3) >> 2;
        self.mi_cols.store(mi_cols, Ordering::Release);
        self.mi_rows.store(mi_rows, Ordering::Release);

        self.bump_generation();
        Ok(())
    }

    /// Get frame width in pixels
    #[inline]
    pub fn frame_width(&self) -> u32 {
        self.frame_width.load(Ordering::Acquire)
    }

    /// Get frame height in pixels
    #[inline]
    pub fn frame_height(&self) -> u32 {
        self.frame_height.load(Ordering::Acquire)
    }

    /// Get superblock size
    #[inline]
    pub fn sb_size(&self) -> u32 {
        self.sb_size.load(Ordering::Acquire)
    }

    // ========================================================================
    // Tile Info Parsing (Section 5.9.15)
    // ========================================================================

    /// Parse tile_info from frame header
    ///
    /// This corresponds to AV1 spec Section 5.9.15 tile_info().
    /// Must be called before parse_tile_group_obu().
    ///
    /// # Arguments
    /// * `frame_width` - Frame width in pixels
    /// * `frame_height` - Frame height in pixels
    /// * `sb_size` - Superblock size (64 or 128)
    ///
    /// # Errors
    /// Returns error if dimensions are invalid or tile configuration is out of spec.
    pub fn parse_tile_info(
        &self,
        frame_width: u32,
        frame_height: u32,
        sb_size: u32,
    ) -> Result<(), Av1TileGroupError> {
        // Validate inputs
        if frame_width == 0 || frame_height == 0 {
            self.set_error(Av1TileGroupError::InvalidFrameDimensions);
            return Err(Av1TileGroupError::InvalidFrameDimensions);
        }
        if sb_size != AV1_SB_SIZE_64 && sb_size != AV1_SB_SIZE_128 {
            self.set_error(Av1TileGroupError::InvalidSuperblockSize);
            return Err(Av1TileGroupError::InvalidSuperblockSize);
        }

        // Store frame dimensions
        self.set_frame_dimensions(frame_width, frame_height, sb_size)?;

        // Calculate frame dimensions in superblocks
        let sb_cols = (frame_width + sb_size - 1) / sb_size;
        let sb_rows = (frame_height + sb_size - 1) / sb_size;

        // Default to single tile (uniform spacing)
        self.uniform_tile_spacing.store(1, Ordering::Release);
        self.tile_cols.store(1, Ordering::Release);
        self.tile_rows.store(1, Ordering::Release);
        self.tile_cols_log2.store(0, Ordering::Release);
        self.tile_rows_log2.store(0, Ordering::Release);
        self.tile_width_sb.store(sb_cols, Ordering::Release);
        self.tile_height_sb.store(sb_rows, Ordering::Release);

        // Default context update tile is 0
        self.context_update_tile_id.store(0, Ordering::Release);

        self.set_state_flag(state_flags::TILE_INFO_PARSED);
        self.bump_generation();

        Ok(())
    }

    /// Configure tile grid with explicit parameters
    ///
    /// Used when parsing actual tile_info syntax from bitstream.
    pub fn configure_tile_grid(
        &self,
        tile_cols: u32,
        tile_rows: u32,
        tile_cols_log2: u32,
        tile_rows_log2: u32,
        uniform_spacing: bool,
    ) -> Result<(), Av1TileGroupError> {
        if tile_cols == 0 || tile_cols > AV1_MAX_TILE_COLS {
            self.set_error(Av1TileGroupError::InvalidTileCols);
            return Err(Av1TileGroupError::InvalidTileCols);
        }
        if tile_rows == 0 || tile_rows > AV1_MAX_TILE_ROWS {
            self.set_error(Av1TileGroupError::InvalidTileRows);
            return Err(Av1TileGroupError::InvalidTileRows);
        }
        if tile_cols_log2 > AV1_MAX_TILE_COLS_LOG2 || tile_rows_log2 > AV1_MAX_TILE_ROWS_LOG2 {
            self.set_error(Av1TileGroupError::InvalidTileCols);
            return Err(Av1TileGroupError::InvalidTileCols);
        }

        self.tile_cols.store(tile_cols, Ordering::Release);
        self.tile_rows.store(tile_rows, Ordering::Release);
        self.tile_cols_log2.store(tile_cols_log2, Ordering::Release);
        self.tile_rows_log2.store(tile_rows_log2, Ordering::Release);
        self.uniform_tile_spacing.store(uniform_spacing as u32, Ordering::Release);

        // Calculate tile dimensions for uniform spacing
        if uniform_spacing {
            let sb_cols = self.sb_cols();
            let sb_rows = self.sb_rows();

            let tile_width_sb = (sb_cols + tile_cols - 1) / tile_cols;
            let tile_height_sb = (sb_rows + tile_rows - 1) / tile_rows;

            self.tile_width_sb.store(tile_width_sb, Ordering::Release);
            self.tile_height_sb.store(tile_height_sb, Ordering::Release);
        }

        self.set_state_flag(state_flags::TILE_INFO_PARSED);
        self.bump_generation();

        Ok(())
    }

    /// Set context update tile ID
    pub fn set_context_update_tile_id(&self, tile_id: u32) -> Result<(), Av1TileGroupError> {
        let num_tiles = self.num_tiles();
        if tile_id >= num_tiles {
            self.set_error(Av1TileGroupError::InvalidContextUpdateTile);
            return Err(Av1TileGroupError::InvalidContextUpdateTile);
        }
        self.context_update_tile_id.store(tile_id, Ordering::Release);
        Ok(())
    }

    // ========================================================================
    // Tile Group OBU Parsing (Section 5.11)
    // ========================================================================

    /// Parse tile_group_obu header and extract tile data offsets
    ///
    /// This corresponds to AV1 spec Section 5.11 tile_group_obu().
    ///
    /// # Arguments
    /// * `data` - Raw OBU payload data (after OBU header)
    /// * `num_tiles` - Total number of tiles in frame (from tile_info)
    ///
    /// # Returns
    /// Number of tiles in this tile group on success.
    ///
    /// # Errors
    /// Returns error if bitstream is malformed or tile info wasn't parsed.
    pub fn parse_tile_group_obu(
        &self,
        data: &[u8],
        num_tiles: u32,
    ) -> Result<u32, Av1TileGroupError> {
        // Ensure tile info was parsed
        if !self.has_state(state_flags::TILE_INFO_PARSED) {
            self.set_error(Av1TileGroupError::TileInfoNotParsed);
            return Err(Av1TileGroupError::TileInfoNotParsed);
        }

        if data.is_empty() {
            self.set_error(Av1TileGroupError::BufferTooSmall);
            return Err(Av1TileGroupError::BufferTooSmall);
        }

        let mut offset: usize = 0;
        let mut bit_offset: u32 = 0;

        // Determine if tg_start and tg_end are present
        // tile_start_and_end_present_flag is signaled in uncompressed header
        // For simplicity, we check if num_tiles > 1
        let tile_start_and_end_present = num_tiles > 1;

        let (tg_start, tg_end) = if tile_start_and_end_present {
            // Read tg_start and tg_end
            let tile_bits = self.tile_log2(num_tiles);

            if tile_bits > 0 {
                let tg_start = self.read_bits(data, &mut offset, &mut bit_offset, tile_bits)?;
                let tg_end = self.read_bits(data, &mut offset, &mut bit_offset, tile_bits)?;

                if tg_start > tg_end || tg_end >= num_tiles {
                    self.set_error(Av1TileGroupError::InvalidTileGroupRange);
                    return Err(Av1TileGroupError::InvalidTileGroupRange);
                }

                self.set_state_flag(state_flags::TG_START_END_PRESENT);
                (tg_start, tg_end)
            } else {
                (0, 0)
            }
        } else {
            (0, num_tiles.saturating_sub(1))
        };

        self.tg_start.store(tg_start, Ordering::Release);
        self.tg_end.store(tg_end, Ordering::Release);

        // Byte-align after header
        if bit_offset > 0 {
            offset += 1;
            bit_offset = 0;
        }

        // Parse tile sizes and calculate offsets
        let tiles_in_group = tg_end - tg_start + 1;
        let tile_size_bytes = self.tile_size_bytes.load(Ordering::Acquire);

        // Parse all tile sizes (except last tile)
        let mut current_offset = offset as u64;

        for tile_idx in tg_start..tg_end {
            // Read tile size (variable length: 1-4 bytes, little-endian)
            let tile_size = self.read_tile_size(data, &mut offset, tile_size_bytes)?;

            // Store offset and size
            let inline_idx = (tile_idx - tg_start) as usize;
            if inline_idx < AV1_INLINE_TILE_OFFSETS {
                self.store_tile_offset(inline_idx, current_offset, tile_size as u32);
            }

            current_offset = offset as u64 + tile_size;
            offset += tile_size as usize;

            // Validate offset doesn't exceed data
            if offset > data.len() {
                self.set_error(Av1TileGroupError::BufferTooSmall);
                return Err(Av1TileGroupError::BufferTooSmall);
            }
        }

        // Last tile size is implicit (remaining data)
        let last_tile_idx = tg_end;
        let last_tile_size = data.len().saturating_sub(offset);

        let inline_idx = (last_tile_idx - tg_start) as usize;
        if inline_idx < AV1_INLINE_TILE_OFFSETS {
            self.store_tile_offset(inline_idx, offset as u64, last_tile_size as u32);
        }

        // Update statistics
        self.bytes_processed.fetch_add(data.len() as u64, Ordering::AcqRel);
        self.tile_groups_parsed.fetch_add(1, Ordering::AcqRel);

        self.set_state_flag(state_flags::TG_HEADER_PARSED);
        self.set_state_flag(state_flags::TG_TILES_PARSED);
        self.bump_generation();

        Ok(tiles_in_group)
    }

    /// Read bits from data (MSB first)
    fn read_bits(
        &self,
        data: &[u8],
        offset: &mut usize,
        bit_offset: &mut u32,
        num_bits: u32,
    ) -> Result<u32, Av1TileGroupError> {
        if num_bits == 0 || num_bits > 32 {
            return Ok(0);
        }

        let mut result: u32 = 0;
        let mut bits_remaining = num_bits;

        while bits_remaining > 0 {
            if *offset >= data.len() {
                self.set_error(Av1TileGroupError::BitstreamError);
                return Err(Av1TileGroupError::BitstreamError);
            }

            let byte = data[*offset];
            let bits_in_byte = 8 - *bit_offset;
            let bits_to_read = bits_remaining.min(bits_in_byte);

            let shift = bits_in_byte - bits_to_read;
            let mask = ((1u32 << bits_to_read) - 1) as u8;
            let value = ((byte >> shift) & mask) as u32;

            result = (result << bits_to_read) | value;
            bits_remaining -= bits_to_read;

            *bit_offset += bits_to_read;
            if *bit_offset >= 8 {
                *bit_offset = 0;
                *offset += 1;
            }
        }

        Ok(result)
    }

    /// Read tile size (little-endian, variable bytes)
    fn read_tile_size(
        &self,
        data: &[u8],
        offset: &mut usize,
        size_bytes: u32,
    ) -> Result<u64, Av1TileGroupError> {
        let size_bytes = size_bytes.min(4) as usize;

        if *offset + size_bytes > data.len() {
            self.set_error(Av1TileGroupError::BufferTooSmall);
            return Err(Av1TileGroupError::BufferTooSmall);
        }

        let mut size: u64 = 0;
        for i in 0..size_bytes {
            size |= (data[*offset + i] as u64) << (i * 8);
        }

        // tile_size_minus_1 encoding
        size += 1;

        *offset += size_bytes;
        Ok(size)
    }

    /// Store tile offset and size (packed format)
    #[inline]
    fn store_tile_offset(&self, idx: usize, offset: u64, size: u32) {
        if idx < AV1_INLINE_TILE_OFFSETS {
            // Pack: upper 40 bits = offset, lower 24 bits = size
            let packed = ((offset & 0xFF_FFFF_FFFF) << 24) | ((size as u64) & 0xFFFFFF);
            self.tile_offsets[idx].store(packed, Ordering::Release);
        }
    }

    /// Calculate log2 of tile count (ceil)
    #[inline]
    const fn tile_log2(&self, num_tiles: u32) -> u32 {
        if num_tiles <= 1 {
            0
        } else {
            32 - (num_tiles - 1).leading_zeros()
        }
    }

    // ========================================================================
    // Tile Access Methods
    // ========================================================================

    /// Get tile offset and size for a specific tile index
    ///
    /// # Arguments
    /// * `tile_idx` - Tile index within current tile group (relative to tg_start)
    ///
    /// # Returns
    /// Tuple of (byte_offset, size_in_bytes)
    pub fn get_tile_offset(&self, tile_idx: u32) -> (u64, u32) {
        let tg_start = self.tg_start.load(Ordering::Acquire);
        let tg_end = self.tg_end.load(Ordering::Acquire);

        // Convert absolute tile_idx to relative index in this tile group
        if tile_idx < tg_start || tile_idx > tg_end {
            return (0, 0);
        }

        let relative_idx = (tile_idx - tg_start) as usize;
        if relative_idx >= AV1_INLINE_TILE_OFFSETS {
            return (0, 0);
        }

        let packed = self.tile_offsets[relative_idx].load(Ordering::Acquire);
        let offset = (packed >> 24) & 0xFF_FFFF_FFFF;
        let size = (packed & 0xFFFFFF) as u32;

        (offset, size)
    }

    /// Get tile coordinates for a specific tile index
    ///
    /// # Arguments
    /// * `tile_idx` - Absolute tile index (0 to num_tiles-1)
    ///
    /// # Returns
    /// Tile coordinates including column, row, pixel position, and dimensions
    pub fn get_tile_coords(&self, tile_idx: u32) -> (u32, u32) {
        let tile_cols = self.tile_cols.load(Ordering::Acquire);

        if tile_cols == 0 {
            return (0, 0);
        }

        let col = tile_idx % tile_cols;
        let row = tile_idx / tile_cols;

        (col, row)
    }

    /// Get detailed tile coordinates and dimensions
    pub fn get_tile_info(&self, tile_idx: u32) -> Av1TileCoords {
        let tile_cols = self.tile_cols.load(Ordering::Acquire);
        let tile_rows = self.tile_rows.load(Ordering::Acquire);
        let sb_size = self.sb_size.load(Ordering::Acquire);
        let frame_width = self.frame_width.load(Ordering::Acquire);
        let frame_height = self.frame_height.load(Ordering::Acquire);

        if tile_cols == 0 || tile_rows == 0 {
            return Av1TileCoords::default();
        }

        let col = tile_idx % tile_cols;
        let row = tile_idx / tile_cols;

        let uniform_spacing = self.uniform_tile_spacing.load(Ordering::Acquire) != 0;

        let (tile_width_sb, tile_height_sb, x, y, width, height) = if uniform_spacing {
            let tile_width_sb = self.tile_width_sb.load(Ordering::Acquire);
            let tile_height_sb = self.tile_height_sb.load(Ordering::Acquire);

            let x = col * tile_width_sb * sb_size;
            let y = row * tile_height_sb * sb_size;

            // Clamp to frame boundaries
            let width = (frame_width.saturating_sub(x)).min(tile_width_sb * sb_size);
            let height = (frame_height.saturating_sub(y)).min(tile_height_sb * sb_size);

            (tile_width_sb, tile_height_sb, x, y, width, height)
        } else {
            // Non-uniform spacing requires per-tile size arrays (not implemented inline)
            let tile_width_sb = self.tile_width_sb.load(Ordering::Acquire);
            let tile_height_sb = self.tile_height_sb.load(Ordering::Acquire);
            let x = col * tile_width_sb * sb_size;
            let y = row * tile_height_sb * sb_size;
            let width = (frame_width.saturating_sub(x)).min(tile_width_sb * sb_size);
            let height = (frame_height.saturating_sub(y)).min(tile_height_sb * sb_size);
            (tile_width_sb, tile_height_sb, x, y, width, height)
        };

        Av1TileCoords {
            col,
            row,
            x,
            y,
            width,
            height,
            width_sb: tile_width_sb,
            height_sb: tile_height_sb,
        }
    }

    // ========================================================================
    // Tile Grid Queries
    // ========================================================================

    /// Get number of tile columns
    #[inline]
    pub fn tile_cols(&self) -> u32 {
        self.tile_cols.load(Ordering::Acquire)
    }

    /// Get number of tile rows
    #[inline]
    pub fn tile_rows(&self) -> u32 {
        self.tile_rows.load(Ordering::Acquire)
    }

    /// Get log2 of tile columns
    #[inline]
    pub fn tile_cols_log2(&self) -> u32 {
        self.tile_cols_log2.load(Ordering::Acquire)
    }

    /// Get log2 of tile rows
    #[inline]
    pub fn tile_rows_log2(&self) -> u32 {
        self.tile_rows_log2.load(Ordering::Acquire)
    }

    /// Get total number of tiles
    #[inline]
    pub fn num_tiles(&self) -> u32 {
        self.tile_cols.load(Ordering::Acquire) * self.tile_rows.load(Ordering::Acquire)
    }

    /// Get frame width in superblocks
    #[inline]
    pub fn sb_cols(&self) -> u32 {
        let frame_width = self.frame_width.load(Ordering::Acquire);
        let sb_size = self.sb_size.load(Ordering::Acquire);
        if sb_size == 0 {
            0
        } else {
            (frame_width + sb_size - 1) / sb_size
        }
    }

    /// Get frame height in superblocks
    #[inline]
    pub fn sb_rows(&self) -> u32 {
        let frame_height = self.frame_height.load(Ordering::Acquire);
        let sb_size = self.sb_size.load(Ordering::Acquire);
        if sb_size == 0 {
            0
        } else {
            (frame_height + sb_size - 1) / sb_size
        }
    }

    /// Get tile group start index
    #[inline]
    pub fn tg_start(&self) -> u32 {
        self.tg_start.load(Ordering::Acquire)
    }

    /// Get tile group end index
    #[inline]
    pub fn tg_end(&self) -> u32 {
        self.tg_end.load(Ordering::Acquire)
    }

    /// Get number of tiles in current tile group
    #[inline]
    pub fn tiles_in_group(&self) -> u32 {
        let start = self.tg_start.load(Ordering::Acquire);
        let end = self.tg_end.load(Ordering::Acquire);
        end.saturating_sub(start) + 1
    }

    /// Get context update tile ID
    #[inline]
    pub fn context_update_tile_id(&self) -> u32 {
        self.context_update_tile_id.load(Ordering::Acquire)
    }

    /// Check if uniform tile spacing is used
    #[inline]
    pub fn is_uniform_tile_spacing(&self) -> bool {
        self.uniform_tile_spacing.load(Ordering::Acquire) != 0
    }

    /// Get tile width in superblocks (uniform mode)
    #[inline]
    pub fn tile_width_sb(&self) -> u32 {
        self.tile_width_sb.load(Ordering::Acquire)
    }

    /// Get tile height in superblocks (uniform mode)
    #[inline]
    pub fn tile_height_sb(&self) -> u32 {
        self.tile_height_sb.load(Ordering::Acquire)
    }

    // ========================================================================
    // Tile Decoding Coordination
    // ========================================================================

    /// Mark a tile as decoded
    pub fn mark_tile_decoded(&self, _tile_idx: u32) {
        self.tiles_decoded.fetch_add(1, Ordering::AcqRel);
    }

    /// Get count of decoded tiles
    #[inline]
    pub fn tiles_decoded(&self) -> u64 {
        self.tiles_decoded.load(Ordering::Acquire)
    }

    /// Check if all tiles in group have been decoded
    #[inline]
    pub fn is_group_complete(&self) -> bool {
        let decoded = self.tiles_decoded.load(Ordering::Acquire) as u32;
        decoded >= self.tiles_in_group()
    }

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Get parsing statistics
    pub fn stats(&self) -> Av1TileGroupStats {
        let tile_groups = self.tile_groups_parsed.load(Ordering::Acquire);
        let tiles = self.tiles_decoded.load(Ordering::Acquire);
        let bytes = self.bytes_processed.load(Ordering::Acquire);

        Av1TileGroupStats {
            tile_groups_parsed: tile_groups,
            tiles_parsed: tiles,
            bytes_processed: bytes,
            tile_cols: self.tile_cols.load(Ordering::Acquire),
            tile_rows: self.tile_rows.load(Ordering::Acquire),
            tg_start: self.tg_start.load(Ordering::Acquire),
            tg_end: self.tg_end.load(Ordering::Acquire),
            avg_tile_size: if tiles > 0 {
                (bytes / tiles) as u32
            } else {
                0
            },
            generation: self.generation.load(Ordering::Acquire),
        }
    }

    // ========================================================================
    // Reset
    // ========================================================================

    /// Reset capsule to initial state
    pub fn reset(&self) {
        self.state.store(0, Ordering::Release);
        self.tile_cols.store(1, Ordering::Release);
        self.tile_rows.store(1, Ordering::Release);
        self.tile_cols_log2.store(0, Ordering::Release);
        self.tile_rows_log2.store(0, Ordering::Release);
        self.context_update_tile_id.store(0, Ordering::Release);
        self.uniform_tile_spacing.store(1, Ordering::Release);
        self.tile_width_sb.store(0, Ordering::Release);
        self.tile_height_sb.store(0, Ordering::Release);
        self.tg_start.store(0, Ordering::Release);
        self.tg_end.store(0, Ordering::Release);
        self.tile_size_bytes.store(4, Ordering::Release);
        self.frame_width.store(0, Ordering::Release);
        self.frame_height.store(0, Ordering::Release);
        self.mi_cols.store(0, Ordering::Release);
        self.mi_rows.store(0, Ordering::Release);
        self.tiles_decoded.store(0, Ordering::Release);
        self.bytes_processed.store(0, Ordering::Release);
        self.tile_groups_parsed.store(0, Ordering::Release);
        self.error_code.store(0, Ordering::Release);

        // Clear tile offsets
        for offset in &self.tile_offsets {
            offset.store(0, Ordering::Release);
        }

        self.bump_generation();
    }

    /// Reset for new tile group (keeps tile info)
    pub fn reset_tile_group(&self) {
        self.clear_state_flag(state_flags::TG_HEADER_PARSED);
        self.clear_state_flag(state_flags::TG_TILES_PARSED);
        self.clear_state_flag(state_flags::TG_START_END_PRESENT);
        self.clear_state_flag(state_flags::ERROR_STATE);

        self.tg_start.store(0, Ordering::Release);
        self.tg_end.store(0, Ordering::Release);
        self.tiles_decoded.store(0, Ordering::Release);
        self.error_code.store(0, Ordering::Release);

        // Clear tile offsets
        for offset in &self.tile_offsets {
            offset.store(0, Ordering::Release);
        }

        self.bump_generation();
    }

    /// Set tile size bytes (from frame header)
    pub fn set_tile_size_bytes(&self, size_bytes: u32) {
        let size = size_bytes.clamp(1, 4);
        self.tile_size_bytes.store(size, Ordering::Release);
    }
}

// ============================================================================
// Tests (T28 5-Tier: Unit/Property/Integration/Production/Determinism)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: Unit Tests
    // ========================================================================

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<Av1TileGroupCapsule>(), 512);
        assert_eq!(core::mem::align_of::<Av1TileGroupCapsule>(), 128);
    }

    #[test]
    fn test_new_capsule_defaults() {
        let capsule = Av1TileGroupCapsule::new();

        assert_eq!(capsule.tile_cols(), 1);
        assert_eq!(capsule.tile_rows(), 1);
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.num_tiles(), 1);
        assert!(!capsule.is_error());
        assert!(!capsule.is_ready_for_decode());
    }

    #[test]
    fn test_state_flags() {
        let capsule = Av1TileGroupCapsule::new();

        assert!(!capsule.has_state(state_flags::TILE_INFO_PARSED));
        capsule.set_state_flag(state_flags::TILE_INFO_PARSED);
        assert!(capsule.has_state(state_flags::TILE_INFO_PARSED));

        capsule.clear_state_flag(state_flags::TILE_INFO_PARSED);
        assert!(!capsule.has_state(state_flags::TILE_INFO_PARSED));
    }

    #[test]
    fn test_frame_dimensions() {
        let capsule = Av1TileGroupCapsule::new();

        assert!(capsule.set_frame_dimensions(1920, 1080, AV1_SB_SIZE_64).is_ok());
        assert_eq!(capsule.frame_width(), 1920);
        assert_eq!(capsule.frame_height(), 1080);
        assert_eq!(capsule.sb_size(), AV1_SB_SIZE_64);
        assert!(capsule.generation() > 0);
    }

    #[test]
    fn test_frame_dimensions_invalid() {
        let capsule = Av1TileGroupCapsule::new();

        assert!(capsule.set_frame_dimensions(0, 1080, AV1_SB_SIZE_64).is_err());
        assert!(capsule.set_frame_dimensions(1920, 0, AV1_SB_SIZE_64).is_err());
        assert!(capsule.set_frame_dimensions(1920, 1080, 32).is_err());
    }

    #[test]
    fn test_tile_log2() {
        let capsule = Av1TileGroupCapsule::new();

        assert_eq!(capsule.tile_log2(1), 0);
        assert_eq!(capsule.tile_log2(2), 1);
        assert_eq!(capsule.tile_log2(3), 2);
        assert_eq!(capsule.tile_log2(4), 2);
        assert_eq!(capsule.tile_log2(5), 3);
        assert_eq!(capsule.tile_log2(64), 6);
    }

    #[test]
    fn test_error_display() {
        assert_eq!(format!("{}", Av1TileGroupError::None), "no error");
        assert_eq!(
            format!("{}", Av1TileGroupError::InvalidTileCols),
            "invalid tile column count"
        );
        assert_eq!(
            format!("{}", Av1TileGroupError::BufferTooSmall),
            "buffer too small for tile data"
        );
    }

    // ========================================================================
    // Q8-Q14: Property Tests (Tile Info Parsing)
    // ========================================================================

    #[test]
    fn test_parse_tile_info_basic() {
        let capsule = Av1TileGroupCapsule::new();

        let result = capsule.parse_tile_info(1920, 1080, AV1_SB_SIZE_64);
        assert!(result.is_ok());
        assert!(capsule.has_state(state_flags::TILE_INFO_PARSED));
        assert_eq!(capsule.tile_cols(), 1);
        assert_eq!(capsule.tile_rows(), 1);
        assert!(capsule.is_uniform_tile_spacing());
    }

    #[test]
    fn test_parse_tile_info_invalid_dimensions() {
        let capsule = Av1TileGroupCapsule::new();

        assert_eq!(
            capsule.parse_tile_info(0, 1080, AV1_SB_SIZE_64),
            Err(Av1TileGroupError::InvalidFrameDimensions)
        );
        assert!(capsule.is_error());
    }

    #[test]
    fn test_configure_tile_grid() {
        let capsule = Av1TileGroupCapsule::new();
        capsule.set_frame_dimensions(1920, 1080, AV1_SB_SIZE_64).unwrap();

        let result = capsule.configure_tile_grid(4, 4, 2, 2, true);
        assert!(result.is_ok());
        assert_eq!(capsule.tile_cols(), 4);
        assert_eq!(capsule.tile_rows(), 4);
        assert_eq!(capsule.tile_cols_log2(), 2);
        assert_eq!(capsule.tile_rows_log2(), 2);
        assert_eq!(capsule.num_tiles(), 16);
    }

    #[test]
    fn test_configure_tile_grid_invalid() {
        let capsule = Av1TileGroupCapsule::new();

        // Zero columns
        assert_eq!(
            capsule.configure_tile_grid(0, 4, 2, 2, true),
            Err(Av1TileGroupError::InvalidTileCols)
        );

        // Too many columns
        assert_eq!(
            capsule.configure_tile_grid(65, 4, 2, 2, true),
            Err(Av1TileGroupError::InvalidTileCols)
        );

        // Zero rows
        assert_eq!(
            capsule.configure_tile_grid(4, 0, 2, 2, true),
            Err(Av1TileGroupError::InvalidTileRows)
        );
    }

    #[test]
    fn test_context_update_tile_id() {
        let capsule = Av1TileGroupCapsule::new();
        capsule.set_frame_dimensions(1920, 1080, AV1_SB_SIZE_64).unwrap();
        capsule.configure_tile_grid(4, 4, 2, 2, true).unwrap();

        // Valid tile ID
        assert!(capsule.set_context_update_tile_id(5).is_ok());
        assert_eq!(capsule.context_update_tile_id(), 5);

        // Invalid tile ID
        assert_eq!(
            capsule.set_context_update_tile_id(100),
            Err(Av1TileGroupError::InvalidContextUpdateTile)
        );
    }

    #[test]
    fn test_sb_dimensions() {
        let capsule = Av1TileGroupCapsule::new();
        capsule.set_frame_dimensions(1920, 1080, AV1_SB_SIZE_64).unwrap();

        // 1920 / 64 = 30, 1080 / 64 = 17 (rounded up)
        assert_eq!(capsule.sb_cols(), 30);
        assert_eq!(capsule.sb_rows(), 17);

        // With 128x128 superblocks
        capsule.set_frame_dimensions(1920, 1080, AV1_SB_SIZE_128).unwrap();
        assert_eq!(capsule.sb_cols(), 15);
        assert_eq!(capsule.sb_rows(), 9);
    }

    // ========================================================================
    // Q15-Q21: Integration Tests (Tile Group Parsing)
    // ========================================================================

    #[test]
    fn test_parse_tile_group_obu_requires_tile_info() {
        let capsule = Av1TileGroupCapsule::new();
        let data = [0u8; 32];

        assert_eq!(
            capsule.parse_tile_group_obu(&data, 4),
            Err(Av1TileGroupError::TileInfoNotParsed)
        );
    }

    #[test]
    fn test_parse_tile_group_obu_single_tile() {
        let capsule = Av1TileGroupCapsule::new();
        capsule.parse_tile_info(640, 480, AV1_SB_SIZE_64).unwrap();

        // Single tile frame - tile data only, no header
        let tile_data = vec![0xAA; 100];

        let result = capsule.parse_tile_group_obu(&tile_data, 1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
        assert!(capsule.is_ready_for_decode());
    }

    #[test]
    fn test_parse_tile_group_obu_multiple_tiles() {
        let capsule = Av1TileGroupCapsule::new();
        capsule.parse_tile_info(1920, 1080, AV1_SB_SIZE_64).unwrap();
        capsule.configure_tile_grid(2, 2, 1, 1, true).unwrap();
        capsule.set_tile_size_bytes(2);

        // Construct multi-tile data:
        // 2 bits for tg_start (0), 2 bits for tg_end (3) -> byte aligned
        // Tile sizes (2 bytes each, little-endian, size-1 encoding)
        // Tile data
        let mut data = Vec::new();

        // Header byte: tg_start=0 (2 bits), tg_end=3 (2 bits), padding
        data.push(0b00_11_0000);

        // Tile 0 size (99 bytes encoded as 98, little-endian)
        data.push(98);
        data.push(0);
        // Tile 0 data
        data.extend(vec![0xAA; 99]);

        // Tile 1 size (50 bytes encoded as 49)
        data.push(49);
        data.push(0);
        // Tile 1 data
        data.extend(vec![0xBB; 50]);

        // Tile 2 size (75 bytes encoded as 74)
        data.push(74);
        data.push(0);
        // Tile 2 data
        data.extend(vec![0xCC; 75]);

        // Tile 3 (last tile, size implicit)
        data.extend(vec![0xDD; 60]);

        let result = capsule.parse_tile_group_obu(&data, 4);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 4);
        assert_eq!(capsule.tg_start(), 0);
        assert_eq!(capsule.tg_end(), 3);
        assert_eq!(capsule.tiles_in_group(), 4);
    }

    #[test]
    fn test_get_tile_offset() {
        let capsule = Av1TileGroupCapsule::new();
        capsule.parse_tile_info(640, 480, AV1_SB_SIZE_64).unwrap();

        // Store some test offsets
        capsule.store_tile_offset(0, 100, 200);
        capsule.store_tile_offset(1, 300, 150);

        // Also set tg_start/tg_end
        capsule.tg_start.store(0, Ordering::Release);
        capsule.tg_end.store(1, Ordering::Release);

        let (offset0, size0) = capsule.get_tile_offset(0);
        assert_eq!(offset0, 100);
        assert_eq!(size0, 200);

        let (offset1, size1) = capsule.get_tile_offset(1);
        assert_eq!(offset1, 300);
        assert_eq!(size1, 150);

        // Out of range
        let (offset_oor, size_oor) = capsule.get_tile_offset(100);
        assert_eq!(offset_oor, 0);
        assert_eq!(size_oor, 0);
    }

    #[test]
    fn test_get_tile_coords() {
        let capsule = Av1TileGroupCapsule::new();
        capsule.set_frame_dimensions(1920, 1080, AV1_SB_SIZE_64).unwrap();
        capsule.configure_tile_grid(4, 4, 2, 2, true).unwrap();

        // Tile 0: col=0, row=0
        assert_eq!(capsule.get_tile_coords(0), (0, 0));

        // Tile 1: col=1, row=0
        assert_eq!(capsule.get_tile_coords(1), (1, 0));

        // Tile 4: col=0, row=1
        assert_eq!(capsule.get_tile_coords(4), (0, 1));

        // Tile 5: col=1, row=1
        assert_eq!(capsule.get_tile_coords(5), (1, 1));

        // Tile 15: col=3, row=3
        assert_eq!(capsule.get_tile_coords(15), (3, 3));
    }

    #[test]
    fn test_get_tile_info_uniform() {
        let capsule = Av1TileGroupCapsule::new();
        capsule.set_frame_dimensions(1920, 1080, AV1_SB_SIZE_64).unwrap();
        capsule.configure_tile_grid(2, 2, 1, 1, true).unwrap();

        let info = capsule.get_tile_info(0);
        assert_eq!(info.col, 0);
        assert_eq!(info.row, 0);
        assert_eq!(info.x, 0);
        assert_eq!(info.y, 0);
        assert!(info.width > 0);
        assert!(info.height > 0);
    }

    // ========================================================================
    // Q22-Q28: Production Tests (Stress & Performance)
    // ========================================================================

    #[test]
    fn test_large_tile_grid() {
        let capsule = Av1TileGroupCapsule::new();
        capsule.set_frame_dimensions(7680, 4320, AV1_SB_SIZE_128).unwrap();

        // Maximum tile grid (64x64)
        let result = capsule.configure_tile_grid(64, 64, 6, 6, true);
        assert!(result.is_ok());
        assert_eq!(capsule.num_tiles(), 4096);
    }

    #[test]
    fn test_statistics_tracking() {
        let capsule = Av1TileGroupCapsule::new();
        capsule.parse_tile_info(640, 480, AV1_SB_SIZE_64).unwrap();

        let tile_data = vec![0xAA; 100];
        capsule.parse_tile_group_obu(&tile_data, 1).unwrap();

        let stats = capsule.stats();
        assert_eq!(stats.tile_groups_parsed, 1);
        assert_eq!(stats.bytes_processed, 100);
        assert_eq!(stats.tile_cols, 1);
        assert_eq!(stats.tile_rows, 1);
    }

    #[test]
    fn test_tile_decoded_tracking() {
        let capsule = Av1TileGroupCapsule::new();
        capsule.parse_tile_info(640, 480, AV1_SB_SIZE_64).unwrap();
        capsule.configure_tile_grid(2, 2, 1, 1, true).unwrap();
        capsule.tg_start.store(0, Ordering::Release);
        capsule.tg_end.store(3, Ordering::Release);

        assert_eq!(capsule.tiles_decoded(), 0);
        assert!(!capsule.is_group_complete());

        capsule.mark_tile_decoded(0);
        capsule.mark_tile_decoded(1);
        assert_eq!(capsule.tiles_decoded(), 2);
        assert!(!capsule.is_group_complete());

        capsule.mark_tile_decoded(2);
        capsule.mark_tile_decoded(3);
        assert_eq!(capsule.tiles_decoded(), 4);
        assert!(capsule.is_group_complete());
    }

    #[test]
    fn test_reset() {
        let capsule = Av1TileGroupCapsule::new();
        capsule.set_frame_dimensions(1920, 1080, AV1_SB_SIZE_64).unwrap();
        capsule.configure_tile_grid(4, 4, 2, 2, true).unwrap();

        let gen_before = capsule.generation();
        capsule.reset();

        assert_eq!(capsule.tile_cols(), 1);
        assert_eq!(capsule.tile_rows(), 1);
        assert_eq!(capsule.frame_width(), 0);
        assert!(!capsule.has_state(state_flags::TILE_INFO_PARSED));
        assert!(capsule.generation() > gen_before);
    }

    #[test]
    fn test_reset_tile_group() {
        let capsule = Av1TileGroupCapsule::new();
        capsule.parse_tile_info(1920, 1080, AV1_SB_SIZE_64).unwrap();
        capsule.configure_tile_grid(4, 4, 2, 2, true).unwrap();

        capsule.set_state_flag(state_flags::TG_HEADER_PARSED);
        capsule.tg_start.store(5, Ordering::Release);
        capsule.tiles_decoded.store(10, Ordering::Release);

        capsule.reset_tile_group();

        // Tile info should be preserved
        assert!(capsule.has_state(state_flags::TILE_INFO_PARSED));
        assert_eq!(capsule.tile_cols(), 4);

        // Tile group state should be reset
        assert!(!capsule.has_state(state_flags::TG_HEADER_PARSED));
        assert_eq!(capsule.tg_start(), 0);
        assert_eq!(capsule.tiles_decoded(), 0);
    }

    #[test]
    fn test_boundary_tile_sizes() {
        let capsule = Av1TileGroupCapsule::new();
        capsule.parse_tile_info(1920, 1080, AV1_SB_SIZE_64).unwrap();

        // Test with different tile_size_bytes values
        capsule.set_tile_size_bytes(1);
        assert_eq!(capsule.tile_size_bytes.load(Ordering::Acquire), 1);

        capsule.set_tile_size_bytes(4);
        assert_eq!(capsule.tile_size_bytes.load(Ordering::Acquire), 4);

        // Clamped to valid range
        capsule.set_tile_size_bytes(0);
        assert_eq!(capsule.tile_size_bytes.load(Ordering::Acquire), 1);

        capsule.set_tile_size_bytes(10);
        assert_eq!(capsule.tile_size_bytes.load(Ordering::Acquire), 4);
    }

    // ========================================================================
    // Q29-Q35: Determinism Tests
    // ========================================================================

    #[test]
    fn test_deterministic_initialization() {
        let capsule1 = Av1TileGroupCapsule::new();
        let capsule2 = Av1TileGroupCapsule::new();

        assert_eq!(capsule1.tile_cols(), capsule2.tile_cols());
        assert_eq!(capsule1.tile_rows(), capsule2.tile_rows());
        assert_eq!(capsule1.generation(), capsule2.generation());
        assert_eq!(capsule1.state(), capsule2.state());
    }

    #[test]
    fn test_deterministic_tile_info_parsing() {
        let capsule1 = Av1TileGroupCapsule::new();
        let capsule2 = Av1TileGroupCapsule::new();

        capsule1.parse_tile_info(1920, 1080, AV1_SB_SIZE_64).unwrap();
        capsule2.parse_tile_info(1920, 1080, AV1_SB_SIZE_64).unwrap();

        assert_eq!(capsule1.tile_cols(), capsule2.tile_cols());
        assert_eq!(capsule1.tile_rows(), capsule2.tile_rows());
        assert_eq!(capsule1.sb_cols(), capsule2.sb_cols());
        assert_eq!(capsule1.sb_rows(), capsule2.sb_rows());
    }

    #[test]
    fn test_deterministic_grid_configuration() {
        let capsule1 = Av1TileGroupCapsule::new();
        let capsule2 = Av1TileGroupCapsule::new();

        capsule1.set_frame_dimensions(1920, 1080, AV1_SB_SIZE_64).unwrap();
        capsule2.set_frame_dimensions(1920, 1080, AV1_SB_SIZE_64).unwrap();

        capsule1.configure_tile_grid(4, 4, 2, 2, true).unwrap();
        capsule2.configure_tile_grid(4, 4, 2, 2, true).unwrap();

        for i in 0..16 {
            assert_eq!(capsule1.get_tile_coords(i), capsule2.get_tile_coords(i));
        }
    }

    #[test]
    fn test_offset_packing_roundtrip() {
        let capsule = Av1TileGroupCapsule::new();

        // Test various offset/size combinations
        let test_cases = [
            (0u64, 0u32),
            (100u64, 200u32),
            (0xFF_FFFF_FFFFu64, 0xFFFFFFu32), // Max values
            (12345678u64, 9999u32),
        ];

        for (idx, (offset, size)) in test_cases.iter().enumerate() {
            capsule.store_tile_offset(idx, *offset, *size);

            // Need to set tg_start/tg_end for get_tile_offset to work
            capsule.tg_start.store(0, Ordering::Release);
            capsule.tg_end.store(test_cases.len() as u32 - 1, Ordering::Release);

            let (read_offset, read_size) = capsule.get_tile_offset(idx as u32);
            assert_eq!(read_offset, *offset & 0xFF_FFFF_FFFF);
            assert_eq!(read_size, *size & 0xFFFFFF);
        }
    }

    #[test]
    fn test_generation_monotonic() {
        let capsule = Av1TileGroupCapsule::new();

        let mut last_gen = capsule.generation();

        // Each operation should increase generation
        capsule.set_frame_dimensions(1920, 1080, AV1_SB_SIZE_64).unwrap();
        assert!(capsule.generation() > last_gen);
        last_gen = capsule.generation();

        capsule.configure_tile_grid(4, 4, 2, 2, true).unwrap();
        assert!(capsule.generation() > last_gen);
        last_gen = capsule.generation();

        capsule.reset();
        assert!(capsule.generation() > last_gen);
    }

    #[test]
    fn test_tile_coords_default() {
        let coords = Av1TileCoords::default();
        assert_eq!(coords.col, 0);
        assert_eq!(coords.row, 0);
        assert_eq!(coords.x, 0);
        assert_eq!(coords.y, 0);
        assert_eq!(coords.width, 0);
        assert_eq!(coords.height, 0);
    }

    #[test]
    fn test_stats_default() {
        let stats = Av1TileGroupStats::default();
        assert_eq!(stats.tile_groups_parsed, 0);
        assert_eq!(stats.tiles_parsed, 0);
        assert_eq!(stats.bytes_processed, 0);
        assert_eq!(stats.avg_tile_size, 0);
    }

    #[test]
    fn test_concurrent_reads() {
        // Simulate concurrent read access (single-threaded simulation)
        let capsule = Av1TileGroupCapsule::new();
        capsule.set_frame_dimensions(1920, 1080, AV1_SB_SIZE_64).unwrap();
        capsule.configure_tile_grid(4, 4, 2, 2, true).unwrap();

        // Rapid concurrent reads should be consistent
        for _ in 0..100 {
            let cols = capsule.tile_cols();
            let rows = capsule.tile_rows();
            let num = capsule.num_tiles();

            assert_eq!(cols, 4);
            assert_eq!(rows, 4);
            assert_eq!(num, 16);
        }
    }

    #[test]
    fn test_4k_resolution() {
        let capsule = Av1TileGroupCapsule::new();

        // 4K resolution
        capsule.set_frame_dimensions(3840, 2160, AV1_SB_SIZE_128).unwrap();

        assert_eq!(capsule.sb_cols(), 30);  // 3840 / 128 = 30
        assert_eq!(capsule.sb_rows(), 17);  // ceil(2160 / 128) = 17
    }

    #[test]
    fn test_8k_resolution() {
        let capsule = Av1TileGroupCapsule::new();

        // 8K resolution
        capsule.set_frame_dimensions(7680, 4320, AV1_SB_SIZE_128).unwrap();

        assert_eq!(capsule.sb_cols(), 60);  // 7680 / 128 = 60
        assert_eq!(capsule.sb_rows(), 34);  // ceil(4320 / 128) = 34
    }
}
