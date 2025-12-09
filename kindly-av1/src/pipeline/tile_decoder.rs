//! Tile Decoder Capsule - Parallel Tile/Slice Decoding Coordination
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Coordinates parallel decoding of video tiles/slices for modern codecs (VP9, AV1, H.264).
//! Spatial partitioning into independent tiles enables parallel decoding with load balancing.
//!
//! # Architecture
//!
//! T4 Batch tier capsule (512B cache-aligned) for parallel tile processing coordination.
//! Implements lockfree work distribution with optional row-based dependencies.
//!
//! ```text
//! TileDecoderCapsule (T4 Batch, 512B aligned)
//! +-------------------------------------------------------------------------+
//! |  state: AtomicU64           - decoder_state | frame_id (packed)         |
//! |  generation: AtomicU64      - Q34 audit trail generation counter        |
//! |  grid_config: AtomicU64     - cols | rows | tile_width | tile_height    |
//! |  current_frame: AtomicU64   - current frame being decoded               |
//! |  total_tiles: AtomicU32     - total tiles in current frame              |
//! |  worker_count: AtomicU32    - number of worker threads                  |
//! |  tile_states_low: AtomicU64 - tile states bits 0-31 (2 bits each)       |
//! |  tile_states_high: AtomicU64- tile states bits 32-63 (2 bits each)      |
//! |  tiles_queued: AtomicU32    - tiles waiting to start                    |
//! |  tiles_decoding: AtomicU32  - tiles currently being decoded             |
//! |  tiles_complete: AtomicU32  - tiles finished successfully               |
//! |  tiles_error: AtomicU32     - tiles that failed                         |
//! |  queue_head: AtomicU32      - work queue head (producer)                |
//! |  queue_tail: AtomicU32      - work queue tail (consumer)                |
//! |  row_complete_mask: AtomicU64 - bit per row for dependency tracking     |
//! |  frames_decoded: AtomicU64  - total frames decoded                      |
//! |  total_tiles_decoded: AtomicU64 - total tiles decoded                   |
//! |  decode_time_ns: AtomicU64  - total decode time                         |
//! |  max_parallel: AtomicU32    - max parallel tiles observed               |
//! |  _padding: [u8; N]          - pad to 512B                               |
//! +-------------------------------------------------------------------------+
//! ```
//!
//! # Tile Grid Configuration
//!
//! Video codecs use different tile grid configurations:
//! - VP9: Up to 64 columns, configurable rows
//! - AV1: Up to 64x64 grid (though typically smaller)
//! - H.264: Slices (similar concept)
//!
//! # Dependency Models
//!
//! - **Independent tiles**: All tiles can decode in parallel (VP9/AV1 default)
//! - **Row-dependent tiles**: Tile depends on previous row completion
//! - **Wavefront parallel**: Diagonal dependency pattern
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

/// Maximum number of tile columns (VP9/AV1 spec)
pub const MAX_TILE_COLS: u8 = 64;
/// Maximum number of tile rows (VP9/AV1 spec)
pub const MAX_TILE_ROWS: u8 = 64;
/// Maximum total inline tiles (tracked with 2-bit states)
pub const MAX_INLINE_TILES: u16 = 64;
/// Work queue capacity for tile scheduling
pub const WORK_QUEUE_CAPACITY: usize = 128;
/// Default worker count (auto-detect uses this as minimum)
pub const DEFAULT_WORKERS: u8 = 4;

// ============================================================================
// Tile State Enumeration
// ============================================================================

/// State of a single tile during decoding
///
/// Uses 2-bit encoding for compact inline storage (32 tiles per AtomicU64)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum TileState {
    /// Tile waiting to start decoding
    #[default]
    Pending = 0,
    /// Tile queued in work queue
    Queued = 1,
    /// Tile currently being decoded by a worker
    Decoding = 2,
    /// Tile decoded successfully
    Complete = 3,
}

impl TileState {
    /// Create from 2-bit value
    #[inline]
    pub const fn from_bits(bits: u8) -> Self {
        match bits & 0x03 {
            0 => Self::Pending,
            1 => Self::Queued,
            2 => Self::Decoding,
            3 => Self::Complete,
            _ => Self::Pending, // Unreachable
        }
    }

    /// Convert to 2-bit value
    #[inline]
    pub const fn to_bits(self) -> u8 {
        self as u8
    }

    /// Check if tile is finished (complete or error treated as complete for flow)
    #[inline]
    pub const fn is_finished(self) -> bool {
        matches!(self, Self::Complete)
    }
}

// ============================================================================
// Decoder State Enumeration
// ============================================================================

/// Overall decoder state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum DecoderState {
    /// Decoder idle, not processing
    #[default]
    Idle = 0,
    /// Decoder configured with grid
    Configured = 1,
    /// Frame decode in progress
    Decoding = 2,
    /// Frame decode complete
    Complete = 3,
    /// Frame decode failed
    Error = 4,
}

impl DecoderState {
    /// Create from packed value
    #[inline]
    pub const fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::Idle,
            1 => Self::Configured,
            2 => Self::Decoding,
            3 => Self::Complete,
            4 => Self::Error,
            _ => Self::Idle,
        }
    }
}

// ============================================================================
// Tile Grid Configuration
// ============================================================================

/// Tile grid configuration
#[derive(Debug, Clone, Copy, Default)]
pub struct TileGrid {
    /// Number of tile columns (1-64)
    pub cols: u8,
    /// Number of tile rows (1-64)
    pub rows: u8,
    /// Pixels per tile column
    pub tile_width: u32,
    /// Pixels per tile row
    pub tile_height: u32,
}

impl TileGrid {
    /// Create a new tile grid configuration
    #[inline]
    pub const fn new(cols: u8, rows: u8, tile_width: u32, tile_height: u32) -> Self {
        Self {
            cols,
            rows,
            tile_width,
            tile_height,
        }
    }

    /// Total number of tiles in the grid
    #[inline]
    pub const fn total_tiles(&self) -> u16 {
        (self.cols as u16) * (self.rows as u16)
    }

    /// Check if configuration is valid
    #[inline]
    pub const fn is_valid(&self) -> bool {
        self.cols >= 1
            && self.cols <= MAX_TILE_COLS
            && self.rows >= 1
            && self.rows <= MAX_TILE_ROWS
            && self.tile_width > 0
            && self.tile_height > 0
    }

    /// Pack into u64 for atomic storage
    /// Layout: [cols:8][rows:8][tile_width:24][tile_height:24]
    #[inline]
    pub const fn pack(&self) -> u64 {
        ((self.cols as u64) << 56)
            | ((self.rows as u64) << 48)
            | (((self.tile_width & 0xFFFFFF) as u64) << 24)
            | ((self.tile_height & 0xFFFFFF) as u64)
    }

    /// Unpack from u64
    #[inline]
    pub const fn unpack(packed: u64) -> Self {
        Self {
            cols: ((packed >> 56) & 0xFF) as u8,
            rows: ((packed >> 48) & 0xFF) as u8,
            tile_width: ((packed >> 24) & 0xFFFFFF) as u32,
            tile_height: (packed & 0xFFFFFF) as u32,
        }
    }
}

// ============================================================================
// Tile Information
// ============================================================================

/// Information about a single tile
#[derive(Debug, Clone, Copy, Default)]
pub struct TileInfo {
    /// Tile index (row * cols + col)
    pub id: u16,
    /// Tile column
    pub col: u8,
    /// Tile row
    pub row: u8,
    /// Pixel X offset
    pub x: u32,
    /// Pixel Y offset
    pub y: u32,
    /// Tile width in pixels
    pub width: u32,
    /// Tile height in pixels
    pub height: u32,
    /// Offset in bitstream
    pub data_offset: usize,
    /// Size in bytes
    pub data_size: usize,
}

impl TileInfo {
    /// Create tile info from grid and index
    #[inline]
    pub fn from_grid(grid: &TileGrid, id: u16) -> Self {
        let col = (id % grid.cols as u16) as u8;
        let row = (id / grid.cols as u16) as u8;
        Self {
            id,
            col,
            row,
            x: col as u32 * grid.tile_width,
            y: row as u32 * grid.tile_height,
            width: grid.tile_width,
            height: grid.tile_height,
            data_offset: 0,
            data_size: 0,
        }
    }
}

// ============================================================================
// Tile Work Item
// ============================================================================

/// Work item for tile scheduling
#[derive(Debug, Clone, Copy, Default)]
pub struct TileWork {
    /// Tile ID
    pub tile_id: u16,
    /// Priority (higher = decode first)
    pub priority: u8,
    /// Number of tiles that must complete first
    pub dependencies: u8,
}

impl TileWork {
    /// Pack into u32 for queue storage
    #[inline]
    pub const fn pack(&self) -> u32 {
        ((self.tile_id as u32) << 16) | ((self.priority as u32) << 8) | (self.dependencies as u32)
    }

    /// Unpack from u32
    #[inline]
    pub const fn unpack(packed: u32) -> Self {
        Self {
            tile_id: ((packed >> 16) & 0xFFFF) as u16,
            priority: ((packed >> 8) & 0xFF) as u8,
            dependencies: (packed & 0xFF) as u8,
        }
    }
}

// ============================================================================
// Statistics
// ============================================================================

/// Tile decoder statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct TileDecoderStats {
    /// Total frames decoded
    pub frames_decoded: u64,
    /// Total tiles decoded
    pub tiles_decoded: u64,
    /// Average tiles per frame
    pub avg_tiles_per_frame: f32,
    /// Average decode time in microseconds
    pub avg_decode_time_us: f64,
    /// Maximum parallel tiles observed
    pub max_parallel_tiles: u32,
    /// Error rate (errors / total)
    pub error_rate: f32,
}

// ============================================================================
// Error Types
// ============================================================================

/// Tile decoder errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TileDecoderError {
    /// No error
    None = 0,
    /// Invalid tile grid configuration
    InvalidGrid = 1,
    /// Tile ID out of range
    InvalidTileId = 2,
    /// Decoder not configured
    NotConfigured = 3,
    /// Frame not started
    NoActiveFrame = 4,
    /// Frame already complete
    FrameComplete = 5,
    /// Tile already queued/decoding/complete
    TileNotPending = 6,
    /// Dependencies not satisfied
    DependenciesNotMet = 7,
    /// Work queue full
    QueueFull = 8,
    /// Tile decode failed
    DecodeFailed = 9,
    /// Invalid worker count
    InvalidWorkerCount = 10,
    /// Timeout waiting for tile
    Timeout = 11,
    /// Invalid state transition
    InvalidState = 12,
}

impl core::fmt::Display for TileDecoderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::None => write!(f, "no error"),
            Self::InvalidGrid => write!(f, "invalid tile grid configuration"),
            Self::InvalidTileId => write!(f, "tile ID out of range"),
            Self::NotConfigured => write!(f, "decoder not configured"),
            Self::NoActiveFrame => write!(f, "no active frame"),
            Self::FrameComplete => write!(f, "frame already complete"),
            Self::TileNotPending => write!(f, "tile not in pending state"),
            Self::DependenciesNotMet => write!(f, "tile dependencies not satisfied"),
            Self::QueueFull => write!(f, "work queue full"),
            Self::DecodeFailed => write!(f, "tile decode failed"),
            Self::InvalidWorkerCount => write!(f, "invalid worker count"),
            Self::Timeout => write!(f, "timeout waiting for tile"),
            Self::InvalidState => write!(f, "invalid state transition"),
        }
    }
}

impl std::error::Error for TileDecoderError {}

// ============================================================================
// Tile Decoder Capsule
// ============================================================================

/// T4 Batch tier capsule for parallel tile decoding coordination
///
/// # Memory Layout
///
/// 512B cache-aligned structure with lockfree atomics for all coordination.
/// Supports up to 64 inline tiles with 2-bit state tracking.
///
/// # Usage
///
/// ```rust,ignore
/// let mut decoder = TileDecoderCapsule::new();
///
/// // Configure grid
/// let grid = TileGrid::new(4, 4, 256, 256);
/// decoder.configure(&grid)?;
/// decoder.set_worker_count(8);
///
/// // Start frame
/// decoder.begin_frame(0)?;
///
/// // Add tiles
/// for id in 0..16 {
///     let tile = TileInfo::from_grid(&grid, id);
///     decoder.add_tile(&tile)?;
/// }
///
/// // Parallel decode
/// decoder.decode_all()?;
/// ```
#[repr(C, align(512))]
pub struct TileDecoderCapsule {
    // Configuration (16 bytes)
    /// Packed state: [decoder_state:8][reserved:24][frame_id:32]
    state: AtomicU64,
    /// Q34 audit trail generation counter
    generation: AtomicU64,

    // Grid configuration (8 bytes)
    /// Packed: [cols:8][rows:8][tile_width:24][tile_height:24]
    grid_config: AtomicU64,

    // Frame state (16 bytes)
    /// Current frame ID being decoded
    current_frame: AtomicU64,
    /// Total tiles in current frame
    total_tiles: AtomicU32,
    /// Number of worker threads
    worker_count: AtomicU32,

    // Tile state tracking (16 bytes)
    /// Tile states bits 0-31 (2 bits per tile)
    tile_states_low: AtomicU64,
    /// Tile states bits 32-63 (2 bits per tile)
    tile_states_high: AtomicU64,

    // Progress counters (16 bytes)
    /// Tiles waiting to start
    tiles_queued: AtomicU32,
    /// Tiles currently being decoded
    tiles_decoding: AtomicU32,
    /// Tiles finished successfully
    tiles_complete: AtomicU32,
    /// Tiles that failed
    tiles_error: AtomicU32,

    // Work queue (8 bytes)
    /// Work queue head index (producer)
    queue_head: AtomicU32,
    /// Work queue tail index (consumer)
    queue_tail: AtomicU32,

    // Dependency tracking (8 bytes)
    /// Bit per row indicating completion
    row_complete_mask: AtomicU64,

    // Statistics (32 bytes)
    /// Total frames decoded
    frames_decoded: AtomicU64,
    /// Total tiles decoded
    total_tiles_decoded: AtomicU64,
    /// Total decode time in nanoseconds
    decode_time_ns: AtomicU64,
    /// Maximum parallel tiles observed
    max_parallel: AtomicU32,
    /// Reserved for alignment
    _stats_reserved: AtomicU32,

    // Padding to 512B
    _padding: [u8; 512 - 128],
}

// Verify size at compile time
const _: () = assert!(core::mem::size_of::<TileDecoderCapsule>() == 512);
const _: () = assert!(core::mem::align_of::<TileDecoderCapsule>() == 512);

impl Default for TileDecoderCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl TileDecoderCapsule {
    /// Create a new tile decoder capsule
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            grid_config: AtomicU64::new(0),
            current_frame: AtomicU64::new(0),
            total_tiles: AtomicU32::new(0),
            worker_count: AtomicU32::new(DEFAULT_WORKERS as u32),
            tile_states_low: AtomicU64::new(0),
            tile_states_high: AtomicU64::new(0),
            tiles_queued: AtomicU32::new(0),
            tiles_decoding: AtomicU32::new(0),
            tiles_complete: AtomicU32::new(0),
            tiles_error: AtomicU32::new(0),
            queue_head: AtomicU32::new(0),
            queue_tail: AtomicU32::new(0),
            row_complete_mask: AtomicU64::new(0),
            frames_decoded: AtomicU64::new(0),
            total_tiles_decoded: AtomicU64::new(0),
            decode_time_ns: AtomicU64::new(0),
            max_parallel: AtomicU32::new(0),
            _stats_reserved: AtomicU32::new(0),
            _padding: [0u8; 512 - 128],
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

    /// Increment generation counter (internal use)
    #[inline]
    fn bump_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    // ========================================================================
    // State Management
    // ========================================================================

    /// Get current decoder state
    #[inline]
    pub fn decoder_state(&self) -> DecoderState {
        let packed = self.state.load(Ordering::Acquire);
        DecoderState::from_u8((packed >> 56) as u8)
    }

    /// Set decoder state (internal)
    #[inline]
    fn set_decoder_state(&self, state: DecoderState) {
        let old = self.state.load(Ordering::Acquire);
        let new = (old & 0x00FFFFFFFFFFFFFF) | ((state as u64) << 56);
        self.state.store(new, Ordering::Release);
    }

    // ========================================================================
    // Configuration
    // ========================================================================

    /// Configure the tile grid
    ///
    /// Must be called before starting frame decode.
    pub fn configure(&mut self, grid: &TileGrid) -> Result<(), TileDecoderError> {
        if !grid.is_valid() {
            return Err(TileDecoderError::InvalidGrid);
        }
        if grid.total_tiles() > MAX_INLINE_TILES {
            return Err(TileDecoderError::InvalidGrid);
        }

        self.grid_config.store(grid.pack(), Ordering::Release);
        self.set_decoder_state(DecoderState::Configured);
        self.bump_generation();

        Ok(())
    }

    /// Get current grid configuration
    #[inline]
    pub fn grid(&self) -> TileGrid {
        TileGrid::unpack(self.grid_config.load(Ordering::Acquire))
    }

    /// Set number of worker threads
    pub fn set_worker_count(&mut self, workers: u8) -> Result<(), TileDecoderError> {
        if workers == 0 {
            return Err(TileDecoderError::InvalidWorkerCount);
        }
        self.worker_count.store(workers as u32, Ordering::Release);
        Ok(())
    }

    /// Get worker count
    #[inline]
    pub fn worker_count(&self) -> u8 {
        self.worker_count.load(Ordering::Acquire) as u8
    }

    // ========================================================================
    // Frame Management
    // ========================================================================

    /// Begin decoding a new frame
    ///
    /// Resets all tile states and prepares for new frame.
    pub fn begin_frame(&mut self, frame_id: u64) -> Result<(), TileDecoderError> {
        if self.decoder_state() == DecoderState::Idle {
            return Err(TileDecoderError::NotConfigured);
        }

        // Reset tile states
        self.tile_states_low.store(0, Ordering::Release);
        self.tile_states_high.store(0, Ordering::Release);

        // Reset counters
        self.tiles_queued.store(0, Ordering::Release);
        self.tiles_decoding.store(0, Ordering::Release);
        self.tiles_complete.store(0, Ordering::Release);
        self.tiles_error.store(0, Ordering::Release);

        // Reset queue
        self.queue_head.store(0, Ordering::Release);
        self.queue_tail.store(0, Ordering::Release);

        // Reset row completion
        self.row_complete_mask.store(0, Ordering::Release);

        // Set frame ID and state
        self.current_frame.store(frame_id, Ordering::Release);
        self.total_tiles.store(0, Ordering::Release);
        self.set_decoder_state(DecoderState::Decoding);
        self.bump_generation();

        Ok(())
    }

    /// Get current frame ID
    #[inline]
    pub fn current_frame(&self) -> u64 {
        self.current_frame.load(Ordering::Acquire)
    }

    // ========================================================================
    // Tile State Management
    // ========================================================================

    /// Get state of a specific tile
    pub fn tile_state(&self, tile_id: u16) -> TileState {
        if tile_id >= MAX_INLINE_TILES {
            return TileState::Pending;
        }

        let (storage, shift) = if tile_id < 32 {
            (&self.tile_states_low, (tile_id as u32) * 2)
        } else {
            (&self.tile_states_high, ((tile_id - 32) as u32) * 2)
        };

        let packed = storage.load(Ordering::Acquire);
        TileState::from_bits(((packed >> shift) & 0x03) as u8)
    }

    /// Set state of a specific tile (lockfree CAS loop)
    fn set_tile_state(&self, tile_id: u16, state: TileState) -> bool {
        if tile_id >= MAX_INLINE_TILES {
            return false;
        }

        let (storage, shift) = if tile_id < 32 {
            (&self.tile_states_low, (tile_id as u32) * 2)
        } else {
            (&self.tile_states_high, ((tile_id - 32) as u32) * 2)
        };

        let mask = 0x03u64 << shift;
        let new_bits = (state.to_bits() as u64) << shift;

        loop {
            let current = storage.load(Ordering::Acquire);
            let new_value = (current & !mask) | new_bits;
            match storage.compare_exchange_weak(
                current,
                new_value,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(_) => continue,
            }
        }
    }

    /// Try to transition tile state atomically
    fn try_transition_tile(
        &self,
        tile_id: u16,
        expected: TileState,
        new: TileState,
    ) -> Result<(), TileDecoderError> {
        if tile_id >= MAX_INLINE_TILES {
            return Err(TileDecoderError::InvalidTileId);
        }

        let (storage, shift) = if tile_id < 32 {
            (&self.tile_states_low, (tile_id as u32) * 2)
        } else {
            (&self.tile_states_high, ((tile_id - 32) as u32) * 2)
        };

        let mask = 0x03u64 << shift;
        let expected_bits = (expected.to_bits() as u64) << shift;
        let new_bits = (new.to_bits() as u64) << shift;

        loop {
            let current = storage.load(Ordering::Acquire);
            if (current & mask) != expected_bits {
                return Err(TileDecoderError::TileNotPending);
            }
            let new_value = (current & !mask) | new_bits;
            match storage.compare_exchange_weak(
                current,
                new_value,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(()),
                Err(_) => continue,
            }
        }
    }

    // ========================================================================
    // Tile Registration
    // ========================================================================

    /// Add a tile to the current frame
    pub fn add_tile(&mut self, tile: &TileInfo) -> Result<(), TileDecoderError> {
        if self.decoder_state() != DecoderState::Decoding {
            return Err(TileDecoderError::NoActiveFrame);
        }
        if tile.id >= MAX_INLINE_TILES {
            return Err(TileDecoderError::InvalidTileId);
        }

        // Increment total tiles
        let total = self.total_tiles.fetch_add(1, Ordering::AcqRel) + 1;
        if total > MAX_INLINE_TILES as u32 {
            self.total_tiles.fetch_sub(1, Ordering::AcqRel);
            return Err(TileDecoderError::InvalidTileId);
        }

        // Tile starts in Pending state (already 0)
        self.set_tile_state(tile.id, TileState::Pending);
        self.bump_generation();

        Ok(())
    }

    /// Set tile data (placeholder for data offset/size registration)
    pub fn set_tile_data(
        &mut self,
        tile_id: u16,
        _data: &[u8],
    ) -> Result<(), TileDecoderError> {
        if tile_id >= MAX_INLINE_TILES {
            return Err(TileDecoderError::InvalidTileId);
        }
        // Data management is external - this just validates tile ID
        Ok(())
    }

    // ========================================================================
    // Work Distribution
    // ========================================================================

    /// Queue a tile for decoding
    fn queue_tile(&self, tile_id: u16) -> Result<(), TileDecoderError> {
        // Transition tile to Queued state
        self.try_transition_tile(tile_id, TileState::Pending, TileState::Queued)?;

        // Increment queued counter
        self.tiles_queued.fetch_add(1, Ordering::AcqRel);

        // Add to work queue (simple ring buffer)
        let head = self.queue_head.fetch_add(1, Ordering::AcqRel);
        if head >= WORK_QUEUE_CAPACITY as u32 {
            // Queue overflow - should not happen with proper sizing
            self.queue_head.fetch_sub(1, Ordering::AcqRel);
            return Err(TileDecoderError::QueueFull);
        }

        Ok(())
    }

    /// Get next tile to decode (for workers)
    ///
    /// Returns None if no tiles are available.
    pub fn next_tile(&self) -> Option<u16> {
        // Dequeue from work queue
        loop {
            let tail = self.queue_tail.load(Ordering::Acquire);
            let head = self.queue_head.load(Ordering::Acquire);

            if tail >= head {
                return None; // Queue empty
            }

            // Try to claim this slot
            match self.queue_tail.compare_exchange_weak(
                tail,
                tail + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Successfully claimed slot, find tile at this position
                    // In a real impl, we'd have a queue array. Here we scan states.
                    let grid = self.grid();
                    let total = grid.total_tiles();
                    for id in 0..total {
                        if self.tile_state(id) == TileState::Queued {
                            // Try to transition to Decoding
                            if self
                                .try_transition_tile(id, TileState::Queued, TileState::Decoding)
                                .is_ok()
                            {
                                // Update counters
                                self.tiles_queued.fetch_sub(1, Ordering::AcqRel);
                                let decoding =
                                    self.tiles_decoding.fetch_add(1, Ordering::AcqRel) + 1;

                                // Track max parallel
                                let mut max = self.max_parallel.load(Ordering::Acquire);
                                while decoding as u32 > max {
                                    match self.max_parallel.compare_exchange_weak(
                                        max,
                                        decoding as u32,
                                        Ordering::AcqRel,
                                        Ordering::Acquire,
                                    ) {
                                        Ok(_) => break,
                                        Err(v) => max = v,
                                    }
                                }

                                return Some(id);
                            }
                        }
                    }
                    return None;
                }
                Err(_) => continue,
            }
        }
    }

    /// Mark tile as complete
    pub fn complete_tile(&self, tile_id: u16) -> Result<(), TileDecoderError> {
        // Transition to Complete
        self.try_transition_tile(tile_id, TileState::Decoding, TileState::Complete)?;

        // Update counters
        self.tiles_decoding.fetch_sub(1, Ordering::AcqRel);
        self.tiles_complete.fetch_add(1, Ordering::AcqRel);
        self.total_tiles_decoded.fetch_add(1, Ordering::AcqRel);

        // Update row completion mask
        let grid = self.grid();
        let row = tile_id / (grid.cols as u16);

        // Check if entire row is complete
        let row_start = row * (grid.cols as u16);
        let row_end = row_start + (grid.cols as u16);
        let mut row_complete = true;
        for id in row_start..row_end {
            if self.tile_state(id) != TileState::Complete {
                row_complete = false;
                break;
            }
        }

        if row_complete {
            let mask = 1u64 << row;
            self.row_complete_mask.fetch_or(mask, Ordering::AcqRel);
        }

        // Check if frame is complete
        let total = self.total_tiles.load(Ordering::Acquire);
        let complete = self.tiles_complete.load(Ordering::Acquire);
        if complete >= total && total > 0 {
            self.set_decoder_state(DecoderState::Complete);
            self.frames_decoded.fetch_add(1, Ordering::AcqRel);
        }

        self.bump_generation();
        Ok(())
    }

    /// Mark tile as failed
    pub fn fail_tile(&self, tile_id: u16, _error: TileDecoderError) {
        // Force transition to Complete (we use Complete for both success and error tracking)
        // Error count is tracked separately
        if self.set_tile_state(tile_id, TileState::Complete) {
            let was_decoding = self.tile_state(tile_id) == TileState::Decoding;
            if was_decoding {
                self.tiles_decoding.fetch_sub(1, Ordering::AcqRel);
            }
            self.tiles_error.fetch_add(1, Ordering::AcqRel);
            self.bump_generation();
        }
    }

    // ========================================================================
    // Parallel Execution
    // ========================================================================

    /// Check if tile can start (dependencies satisfied)
    pub fn can_start_tile(&self, tile_id: u16, row_dependent: bool) -> bool {
        let grid = self.grid();
        let row = tile_id / (grid.cols as u16);

        if !row_dependent || row == 0 {
            return true; // Top row or independent tiles can always start
        }

        // Check if previous row is complete
        let mask = self.row_complete_mask.load(Ordering::Acquire);
        (mask & (1u64 << (row - 1))) != 0
    }

    /// Decode all tiles (main entry point for parallel decode)
    ///
    /// Queues all tiles for parallel processing. Actual decode is done by workers.
    pub fn decode_all(&mut self) -> Result<(), TileDecoderError> {
        if self.decoder_state() != DecoderState::Decoding {
            return Err(TileDecoderError::NoActiveFrame);
        }

        let grid = self.grid();
        let total = grid.total_tiles();

        // Queue all tiles (independent mode - no dependencies)
        for id in 0..total {
            if self.tile_state(id) == TileState::Pending {
                self.queue_tile(id)?;
            }
        }

        Ok(())
    }

    /// Decode all tiles with row dependencies
    pub fn decode_all_row_dependent(&mut self) -> Result<(), TileDecoderError> {
        if self.decoder_state() != DecoderState::Decoding {
            return Err(TileDecoderError::NoActiveFrame);
        }

        let grid = self.grid();
        let cols = grid.cols as u16;

        // Queue first row immediately (no dependencies)
        for col in 0..cols {
            if self.tile_state(col) == TileState::Pending {
                self.queue_tile(col)?;
            }
        }

        Ok(())
    }

    /// Queue next row when previous completes
    pub fn queue_next_row(&self, completed_row: u8) -> Result<u32, TileDecoderError> {
        let grid = self.grid();
        let next_row = completed_row + 1;

        if next_row >= grid.rows {
            return Ok(0); // No more rows
        }

        let row_start = (next_row as u16) * (grid.cols as u16);
        let row_end = row_start + (grid.cols as u16);
        let mut queued = 0;

        for id in row_start..row_end {
            if self.tile_state(id) == TileState::Pending && self.can_start_tile(id, true) {
                if self.queue_tile(id).is_ok() {
                    queued += 1;
                }
            }
        }

        Ok(queued)
    }

    // ========================================================================
    // Synchronization
    // ========================================================================

    /// Wait for a specific tile to complete (busy-wait)
    pub fn wait_for_tile(&self, tile_id: u16) -> Result<(), TileDecoderError> {
        if tile_id >= MAX_INLINE_TILES {
            return Err(TileDecoderError::InvalidTileId);
        }

        // Busy-wait with yield
        let mut spins = 0;
        while self.tile_state(tile_id) != TileState::Complete {
            spins += 1;
            if spins > 1_000_000 {
                return Err(TileDecoderError::Timeout);
            }
            core::hint::spin_loop();
        }

        Ok(())
    }

    /// Wait for entire row to complete
    pub fn wait_for_row(&self, row: u8) -> Result<(), TileDecoderError> {
        let mut spins = 0;
        loop {
            let mask = self.row_complete_mask.load(Ordering::Acquire);
            if (mask & (1u64 << row)) != 0 {
                return Ok(());
            }
            spins += 1;
            if spins > 1_000_000 {
                return Err(TileDecoderError::Timeout);
            }
            core::hint::spin_loop();
        }
    }

    /// Wait for all tiles to complete
    pub fn wait_for_all(&self) -> Result<(), TileDecoderError> {
        let mut spins = 0;
        while !self.is_frame_complete() {
            spins += 1;
            if spins > 10_000_000 {
                return Err(TileDecoderError::Timeout);
            }
            core::hint::spin_loop();
        }
        Ok(())
    }

    /// Check if frame decode is complete
    #[inline]
    pub fn is_frame_complete(&self) -> bool {
        let state = self.decoder_state();
        if state == DecoderState::Complete {
            return true;
        }
        let total = self.total_tiles.load(Ordering::Acquire);
        let complete = self.tiles_complete.load(Ordering::Acquire);
        let errors = self.tiles_error.load(Ordering::Acquire);
        total > 0 && (complete + errors) >= total
    }

    // ========================================================================
    // Progress Queries
    // ========================================================================

    /// Get count of completed tiles
    #[inline]
    pub fn completed_count(&self) -> u32 {
        self.tiles_complete.load(Ordering::Acquire)
    }

    /// Get count of pending tiles
    #[inline]
    pub fn pending_count(&self) -> u32 {
        let total = self.total_tiles.load(Ordering::Acquire);
        let queued = self.tiles_queued.load(Ordering::Acquire);
        let decoding = self.tiles_decoding.load(Ordering::Acquire);
        let complete = self.tiles_complete.load(Ordering::Acquire);
        let errors = self.tiles_error.load(Ordering::Acquire);
        total.saturating_sub(queued + decoding + complete + errors)
    }

    /// Get count of error tiles
    #[inline]
    pub fn error_count(&self) -> u32 {
        self.tiles_error.load(Ordering::Acquire)
    }

    /// Get count of tiles currently decoding
    #[inline]
    pub fn decoding_count(&self) -> u32 {
        self.tiles_decoding.load(Ordering::Acquire)
    }

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Get decoder statistics
    pub fn stats(&self) -> TileDecoderStats {
        let frames = self.frames_decoded.load(Ordering::Acquire);
        let tiles = self.total_tiles_decoded.load(Ordering::Acquire);
        let time_ns = self.decode_time_ns.load(Ordering::Acquire);
        let max_parallel = self.max_parallel.load(Ordering::Acquire);
        let errors = self.tiles_error.load(Ordering::Acquire);

        TileDecoderStats {
            frames_decoded: frames,
            tiles_decoded: tiles,
            avg_tiles_per_frame: if frames > 0 {
                tiles as f32 / frames as f32
            } else {
                0.0
            },
            avg_decode_time_us: if tiles > 0 {
                (time_ns as f64 / tiles as f64) / 1000.0
            } else {
                0.0
            },
            max_parallel_tiles: max_parallel,
            error_rate: if tiles > 0 {
                errors as f32 / tiles as f32
            } else {
                0.0
            },
        }
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.frames_decoded.store(0, Ordering::Release);
        self.total_tiles_decoded.store(0, Ordering::Release);
        self.decode_time_ns.store(0, Ordering::Release);
        self.max_parallel.store(0, Ordering::Release);
    }

    /// Record decode time for a tile
    pub fn record_decode_time(&self, time_ns: u64) {
        self.decode_time_ns.fetch_add(time_ns, Ordering::AcqRel);
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
        assert_eq!(core::mem::size_of::<TileDecoderCapsule>(), 512);
        assert_eq!(core::mem::align_of::<TileDecoderCapsule>(), 512);
    }

    #[test]
    fn test_tile_state_bits() {
        assert_eq!(TileState::Pending.to_bits(), 0);
        assert_eq!(TileState::Queued.to_bits(), 1);
        assert_eq!(TileState::Decoding.to_bits(), 2);
        assert_eq!(TileState::Complete.to_bits(), 3);

        assert_eq!(TileState::from_bits(0), TileState::Pending);
        assert_eq!(TileState::from_bits(1), TileState::Queued);
        assert_eq!(TileState::from_bits(2), TileState::Decoding);
        assert_eq!(TileState::from_bits(3), TileState::Complete);
    }

    #[test]
    fn test_decoder_state_enum() {
        assert_eq!(DecoderState::from_u8(0), DecoderState::Idle);
        assert_eq!(DecoderState::from_u8(1), DecoderState::Configured);
        assert_eq!(DecoderState::from_u8(2), DecoderState::Decoding);
        assert_eq!(DecoderState::from_u8(3), DecoderState::Complete);
        assert_eq!(DecoderState::from_u8(4), DecoderState::Error);
        assert_eq!(DecoderState::from_u8(255), DecoderState::Idle);
    }

    #[test]
    fn test_tile_grid_pack_unpack() {
        let grid = TileGrid::new(4, 4, 256, 256);
        let packed = grid.pack();
        let unpacked = TileGrid::unpack(packed);

        assert_eq!(unpacked.cols, 4);
        assert_eq!(unpacked.rows, 4);
        assert_eq!(unpacked.tile_width, 256);
        assert_eq!(unpacked.tile_height, 256);
    }

    #[test]
    fn test_tile_grid_validation() {
        let valid = TileGrid::new(4, 4, 256, 256);
        assert!(valid.is_valid());

        let invalid_cols = TileGrid::new(0, 4, 256, 256);
        assert!(!invalid_cols.is_valid());

        let invalid_rows = TileGrid::new(4, 0, 256, 256);
        assert!(!invalid_rows.is_valid());

        let invalid_width = TileGrid::new(4, 4, 0, 256);
        assert!(!invalid_width.is_valid());

        let too_many_cols = TileGrid::new(65, 4, 256, 256);
        assert!(!too_many_cols.is_valid());
    }

    #[test]
    fn test_tile_info_from_grid() {
        let grid = TileGrid::new(4, 4, 256, 256);
        let tile = TileInfo::from_grid(&grid, 5);

        assert_eq!(tile.id, 5);
        assert_eq!(tile.col, 1);
        assert_eq!(tile.row, 1);
        assert_eq!(tile.x, 256);
        assert_eq!(tile.y, 256);
    }

    #[test]
    fn test_tile_work_pack_unpack() {
        let work = TileWork {
            tile_id: 42,
            priority: 100,
            dependencies: 3,
        };
        let packed = work.pack();
        let unpacked = TileWork::unpack(packed);

        assert_eq!(unpacked.tile_id, 42);
        assert_eq!(unpacked.priority, 100);
        assert_eq!(unpacked.dependencies, 3);
    }

    #[test]
    fn test_new_capsule_defaults() {
        let capsule = TileDecoderCapsule::new();
        assert_eq!(capsule.decoder_state(), DecoderState::Idle);
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.worker_count(), DEFAULT_WORKERS);
    }

    // ========================================================================
    // Q8-Q14: Property Tests (State Transitions)
    // ========================================================================

    #[test]
    fn test_configure_valid_grid() {
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(4, 4, 256, 256);

        assert!(capsule.configure(&grid).is_ok());
        assert_eq!(capsule.decoder_state(), DecoderState::Configured);
        assert!(capsule.generation() > 0);
    }

    #[test]
    fn test_configure_invalid_grid() {
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(0, 4, 256, 256);

        assert_eq!(capsule.configure(&grid), Err(TileDecoderError::InvalidGrid));
        assert_eq!(capsule.decoder_state(), DecoderState::Idle);
    }

    #[test]
    fn test_begin_frame_requires_config() {
        let mut capsule = TileDecoderCapsule::new();
        assert_eq!(
            capsule.begin_frame(0),
            Err(TileDecoderError::NotConfigured)
        );
    }

    #[test]
    fn test_begin_frame_after_config() {
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(4, 4, 256, 256);
        capsule.configure(&grid).unwrap();

        assert!(capsule.begin_frame(0).is_ok());
        assert_eq!(capsule.decoder_state(), DecoderState::Decoding);
        assert_eq!(capsule.current_frame(), 0);
    }

    #[test]
    fn test_tile_state_transitions() {
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(4, 4, 256, 256);
        capsule.configure(&grid).unwrap();
        capsule.begin_frame(0).unwrap();

        let tile = TileInfo::from_grid(&grid, 0);
        capsule.add_tile(&tile).unwrap();

        assert_eq!(capsule.tile_state(0), TileState::Pending);

        // Transition to queued
        capsule.queue_tile(0).unwrap();
        assert_eq!(capsule.tile_state(0), TileState::Queued);
    }

    #[test]
    fn test_tile_state_bit_packing() {
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(8, 8, 128, 128);
        capsule.configure(&grid).unwrap();
        capsule.begin_frame(0).unwrap();

        // Add multiple tiles and verify state isolation
        for id in 0..64 {
            let tile = TileInfo::from_grid(&grid, id);
            capsule.add_tile(&tile).unwrap();
        }

        // All should be pending
        for id in 0..64 {
            assert_eq!(capsule.tile_state(id), TileState::Pending);
        }

        // Queue specific tiles
        capsule.queue_tile(0).unwrap();
        capsule.queue_tile(31).unwrap();
        capsule.queue_tile(32).unwrap();
        capsule.queue_tile(63).unwrap();

        assert_eq!(capsule.tile_state(0), TileState::Queued);
        assert_eq!(capsule.tile_state(31), TileState::Queued);
        assert_eq!(capsule.tile_state(32), TileState::Queued);
        assert_eq!(capsule.tile_state(63), TileState::Queued);

        // Other tiles still pending
        assert_eq!(capsule.tile_state(1), TileState::Pending);
        assert_eq!(capsule.tile_state(30), TileState::Pending);
        assert_eq!(capsule.tile_state(33), TileState::Pending);
        assert_eq!(capsule.tile_state(62), TileState::Pending);
    }

    #[test]
    fn test_worker_count_validation() {
        let mut capsule = TileDecoderCapsule::new();
        assert!(capsule.set_worker_count(8).is_ok());
        assert_eq!(capsule.worker_count(), 8);

        assert_eq!(
            capsule.set_worker_count(0),
            Err(TileDecoderError::InvalidWorkerCount)
        );
    }

    #[test]
    fn test_generation_counter_increments() {
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(4, 4, 256, 256);

        let gen0 = capsule.generation();
        capsule.configure(&grid).unwrap();
        let gen1 = capsule.generation();
        assert!(gen1 > gen0);

        capsule.begin_frame(0).unwrap();
        let gen2 = capsule.generation();
        assert!(gen2 > gen1);
    }

    // ========================================================================
    // Q15-Q21: Integration Tests
    // ========================================================================

    #[test]
    fn test_full_frame_decode_flow() {
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(2, 2, 256, 256);
        capsule.configure(&grid).unwrap();
        capsule.begin_frame(0).unwrap();

        // Add all tiles
        for id in 0..4 {
            let tile = TileInfo::from_grid(&grid, id);
            capsule.add_tile(&tile).unwrap();
        }

        // Queue all tiles
        capsule.decode_all().unwrap();

        // Simulate worker processing
        while let Some(tile_id) = capsule.next_tile() {
            capsule.complete_tile(tile_id).unwrap();
        }

        assert!(capsule.is_frame_complete());
        assert_eq!(capsule.completed_count(), 4);
        assert_eq!(capsule.error_count(), 0);
    }

    #[test]
    fn test_row_dependent_decode() {
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(4, 4, 256, 256);
        capsule.configure(&grid).unwrap();
        capsule.begin_frame(0).unwrap();

        // Add all tiles
        for id in 0..16 {
            let tile = TileInfo::from_grid(&grid, id);
            capsule.add_tile(&tile).unwrap();
        }

        // Queue first row
        capsule.decode_all_row_dependent().unwrap();

        // Complete first row
        for id in 0..4 {
            while capsule.tile_state(id) == TileState::Queued {
                if capsule.next_tile() == Some(id) {
                    capsule.complete_tile(id).unwrap();
                }
            }
        }

        // Verify first row complete
        assert!(capsule.can_start_tile(4, true)); // Row 1 can now start
    }

    #[test]
    fn test_row_completion_mask() {
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(4, 2, 256, 256);
        capsule.configure(&grid).unwrap();
        capsule.begin_frame(0).unwrap();

        // Add all tiles to the frame
        for id in 0..8 {
            let tile = TileInfo::from_grid(&grid, id);
            capsule.add_tile(&tile).unwrap();
        }

        // Queue and process first row through proper state machine
        capsule.decode_all().unwrap();

        // Complete first row (tiles 0-3)
        for _ in 0..4 {
            if let Some(tile_id) = capsule.next_tile() {
                if tile_id < 4 {
                    capsule.complete_tile(tile_id).unwrap();
                }
            }
        }

        // Row 0 should be marked complete
        let mask = capsule.row_complete_mask.load(Ordering::Acquire);
        assert_eq!(mask & 0x01, 1);
    }

    #[test]
    fn test_multi_frame_decode() {
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(2, 2, 256, 256);
        capsule.configure(&grid).unwrap();

        for frame_id in 0..3 {
            capsule.begin_frame(frame_id).unwrap();

            for id in 0..4 {
                let tile = TileInfo::from_grid(&grid, id);
                capsule.add_tile(&tile).unwrap();
            }

            capsule.decode_all().unwrap();

            while let Some(tile_id) = capsule.next_tile() {
                capsule.complete_tile(tile_id).unwrap();
            }

            assert!(capsule.is_frame_complete());
        }

        let stats = capsule.stats();
        assert_eq!(stats.frames_decoded, 3);
        assert_eq!(stats.tiles_decoded, 12);
    }

    #[test]
    fn test_error_handling() {
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(2, 2, 256, 256);
        capsule.configure(&grid).unwrap();
        capsule.begin_frame(0).unwrap();

        for id in 0..4 {
            let tile = TileInfo::from_grid(&grid, id);
            capsule.add_tile(&tile).unwrap();
        }

        capsule.decode_all().unwrap();

        // Complete 2, fail 2
        if let Some(tile_id) = capsule.next_tile() {
            capsule.complete_tile(tile_id).unwrap();
        }
        if let Some(tile_id) = capsule.next_tile() {
            capsule.fail_tile(tile_id, TileDecoderError::DecodeFailed);
        }
        if let Some(tile_id) = capsule.next_tile() {
            capsule.complete_tile(tile_id).unwrap();
        }
        if let Some(tile_id) = capsule.next_tile() {
            capsule.fail_tile(tile_id, TileDecoderError::DecodeFailed);
        }

        // Frame should still complete (errors count as processed)
        assert!(capsule.is_frame_complete());
        assert_eq!(capsule.completed_count(), 2);
        assert_eq!(capsule.error_count(), 2);
    }

    // ========================================================================
    // Q22-Q28: Production Tests (Stress & Performance)
    // ========================================================================

    #[test]
    fn test_max_tile_grid() {
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(8, 8, 128, 128); // 64 tiles (max inline)
        capsule.configure(&grid).unwrap();
        capsule.begin_frame(0).unwrap();

        for id in 0..64 {
            let tile = TileInfo::from_grid(&grid, id);
            capsule.add_tile(&tile).unwrap();
        }

        capsule.decode_all().unwrap();

        let mut decoded = 0;
        while let Some(tile_id) = capsule.next_tile() {
            capsule.complete_tile(tile_id).unwrap();
            decoded += 1;
        }

        assert_eq!(decoded, 64);
        assert!(capsule.is_frame_complete());
    }

    #[test]
    fn test_progress_counters_consistency() {
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(4, 4, 256, 256);
        capsule.configure(&grid).unwrap();
        capsule.begin_frame(0).unwrap();

        for id in 0..16 {
            let tile = TileInfo::from_grid(&grid, id);
            capsule.add_tile(&tile).unwrap();
        }

        capsule.decode_all().unwrap();

        // During decode, counters should be consistent
        let total = 16u32;
        let mut iterations = 0;
        while !capsule.is_frame_complete() && iterations < 100 {
            if let Some(tile_id) = capsule.next_tile() {
                let queued = capsule.tiles_queued.load(Ordering::Acquire);
                let decoding = capsule.decoding_count();
                let complete = capsule.completed_count();
                let pending = capsule.pending_count();

                // All counters should sum to total or less (during transitions)
                assert!(queued + decoding + complete + pending <= total + 1);

                capsule.complete_tile(tile_id).unwrap();
            }
            iterations += 1;
        }

        assert!(capsule.is_frame_complete());
    }

    #[test]
    fn test_statistics_accuracy() {
        let mut capsule = TileDecoderCapsule::new();
        capsule.reset_stats();

        let grid = TileGrid::new(4, 4, 256, 256);
        capsule.configure(&grid).unwrap();

        for frame in 0..5 {
            capsule.begin_frame(frame).unwrap();

            for id in 0..16 {
                let tile = TileInfo::from_grid(&grid, id);
                capsule.add_tile(&tile).unwrap();
            }

            capsule.decode_all().unwrap();

            while let Some(tile_id) = capsule.next_tile() {
                capsule.record_decode_time(1000); // 1us per tile
                capsule.complete_tile(tile_id).unwrap();
            }
        }

        let stats = capsule.stats();
        assert_eq!(stats.frames_decoded, 5);
        assert_eq!(stats.tiles_decoded, 80);
        assert!((stats.avg_tiles_per_frame - 16.0).abs() < 0.1);
        assert_eq!(stats.error_rate, 0.0);
    }

    #[test]
    fn test_boundary_tile_states() {
        // Test tiles at storage boundaries (0, 31, 32, 63)
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(8, 8, 128, 128);
        capsule.configure(&grid).unwrap();
        capsule.begin_frame(0).unwrap();

        let boundary_tiles = [0, 31, 32, 63];
        for &id in &boundary_tiles {
            let tile = TileInfo::from_grid(&grid, id);
            capsule.add_tile(&tile).unwrap();
        }

        // Verify independent state management at boundaries
        for &id in &boundary_tiles {
            assert_eq!(capsule.tile_state(id), TileState::Pending);
            capsule.set_tile_state(id, TileState::Queued);
        }

        for &id in &boundary_tiles {
            assert_eq!(capsule.tile_state(id), TileState::Queued);
        }
    }

    #[test]
    fn test_concurrent_state_updates() {
        // Single-threaded simulation of concurrent updates
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(4, 4, 256, 256);
        capsule.configure(&grid).unwrap();
        capsule.begin_frame(0).unwrap();

        for id in 0..16 {
            let tile = TileInfo::from_grid(&grid, id);
            capsule.add_tile(&tile).unwrap();
        }

        // Rapid state transitions
        for id in 0..16 {
            capsule.set_tile_state(id, TileState::Queued);
        }
        for id in 0..16 {
            capsule.set_tile_state(id, TileState::Decoding);
        }
        for id in 0..16 {
            capsule.set_tile_state(id, TileState::Complete);
        }

        for id in 0..16 {
            assert_eq!(capsule.tile_state(id), TileState::Complete);
        }
    }

    // ========================================================================
    // Q29-Q35: Determinism Tests
    // ========================================================================

    #[test]
    fn test_deterministic_initialization() {
        let capsule1 = TileDecoderCapsule::new();
        let capsule2 = TileDecoderCapsule::new();

        assert_eq!(capsule1.decoder_state(), capsule2.decoder_state());
        assert_eq!(capsule1.generation(), capsule2.generation());
        assert_eq!(capsule1.worker_count(), capsule2.worker_count());
    }

    #[test]
    fn test_deterministic_frame_reset() {
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(4, 4, 256, 256);
        capsule.configure(&grid).unwrap();

        // First frame
        capsule.begin_frame(0).unwrap();
        for id in 0..16 {
            let tile = TileInfo::from_grid(&grid, id);
            capsule.add_tile(&tile).unwrap();
            capsule.set_tile_state(id, TileState::Complete);
        }

        // Second frame should reset cleanly
        capsule.begin_frame(1).unwrap();
        for id in 0..16 {
            assert_eq!(capsule.tile_state(id), TileState::Pending);
        }
        assert_eq!(capsule.completed_count(), 0);
        assert_eq!(capsule.error_count(), 0);
    }

    #[test]
    fn test_grid_pack_unpack_roundtrip() {
        // Test various grid configurations for deterministic packing
        let configs = [
            (1, 1, 1920, 1080),
            (4, 4, 256, 256),
            (8, 8, 128, 128),
            (64, 1, 30, 1080),
            (1, 64, 1920, 17),
        ];

        for (cols, rows, width, height) in configs {
            let grid = TileGrid::new(cols, rows, width, height);
            let packed = grid.pack();
            let unpacked = TileGrid::unpack(packed);

            assert_eq!(grid.cols, unpacked.cols);
            assert_eq!(grid.rows, unpacked.rows);
            assert_eq!(grid.tile_width, unpacked.tile_width);
            assert_eq!(grid.tile_height, unpacked.tile_height);
        }
    }

    #[test]
    fn test_state_isolation() {
        // Verify tile state changes don't affect other tiles
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(8, 8, 128, 128);
        capsule.configure(&grid).unwrap();
        capsule.begin_frame(0).unwrap();

        for id in 0..64 {
            let tile = TileInfo::from_grid(&grid, id);
            capsule.add_tile(&tile).unwrap();
        }

        // Modify every other tile
        for id in (0..64).step_by(2) {
            capsule.set_tile_state(id, TileState::Complete);
        }

        // Verify isolation
        for id in 0..64 {
            if id % 2 == 0 {
                assert_eq!(capsule.tile_state(id), TileState::Complete);
            } else {
                assert_eq!(capsule.tile_state(id), TileState::Pending);
            }
        }
    }

    #[test]
    fn test_error_display() {
        let errors = [
            (TileDecoderError::None, "no error"),
            (TileDecoderError::InvalidGrid, "invalid tile grid configuration"),
            (TileDecoderError::InvalidTileId, "tile ID out of range"),
            (TileDecoderError::NotConfigured, "decoder not configured"),
            (TileDecoderError::NoActiveFrame, "no active frame"),
            (TileDecoderError::FrameComplete, "frame already complete"),
            (TileDecoderError::TileNotPending, "tile not in pending state"),
            (TileDecoderError::DependenciesNotMet, "tile dependencies not satisfied"),
            (TileDecoderError::QueueFull, "work queue full"),
            (TileDecoderError::DecodeFailed, "tile decode failed"),
            (TileDecoderError::InvalidWorkerCount, "invalid worker count"),
            (TileDecoderError::Timeout, "timeout waiting for tile"),
            (TileDecoderError::InvalidState, "invalid state transition"),
        ];

        for (error, expected) in errors {
            assert_eq!(format!("{}", error), expected);
        }
    }

    #[test]
    fn test_tile_info_calculation() {
        let grid = TileGrid::new(4, 4, 256, 256);

        // Test corner tiles
        let top_left = TileInfo::from_grid(&grid, 0);
        assert_eq!(top_left.col, 0);
        assert_eq!(top_left.row, 0);
        assert_eq!(top_left.x, 0);
        assert_eq!(top_left.y, 0);

        let top_right = TileInfo::from_grid(&grid, 3);
        assert_eq!(top_right.col, 3);
        assert_eq!(top_right.row, 0);
        assert_eq!(top_right.x, 768);
        assert_eq!(top_right.y, 0);

        let bottom_left = TileInfo::from_grid(&grid, 12);
        assert_eq!(bottom_left.col, 0);
        assert_eq!(bottom_left.row, 3);
        assert_eq!(bottom_left.x, 0);
        assert_eq!(bottom_left.y, 768);

        let bottom_right = TileInfo::from_grid(&grid, 15);
        assert_eq!(bottom_right.col, 3);
        assert_eq!(bottom_right.row, 3);
        assert_eq!(bottom_right.x, 768);
        assert_eq!(bottom_right.y, 768);
    }

    #[test]
    fn test_max_parallel_tracking() {
        let mut capsule = TileDecoderCapsule::new();
        let grid = TileGrid::new(4, 4, 256, 256);
        capsule.configure(&grid).unwrap();
        capsule.begin_frame(0).unwrap();

        for id in 0..16 {
            let tile = TileInfo::from_grid(&grid, id);
            capsule.add_tile(&tile).unwrap();
        }

        capsule.decode_all().unwrap();

        // Get multiple tiles before completing any
        let tile1 = capsule.next_tile();
        let tile2 = capsule.next_tile();
        let tile3 = capsule.next_tile();

        // At this point, 3 tiles should be decoding
        assert!(capsule.decoding_count() >= 1);

        // Complete them
        if let Some(id) = tile1 {
            capsule.complete_tile(id).unwrap();
        }
        if let Some(id) = tile2 {
            capsule.complete_tile(id).unwrap();
        }
        if let Some(id) = tile3 {
            capsule.complete_tile(id).unwrap();
        }

        // Max parallel should be at least 1
        assert!(capsule.max_parallel.load(Ordering::Acquire) >= 1);
    }
}
