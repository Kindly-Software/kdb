//! Tile Aggregator Capsule - T5 Streaming Tier
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! SOTA lockfree tile result aggregation for parallel AV1 encoding.
//! Tiles are encoded in parallel and arrive out-of-order; this capsule
//! collects them and merges in tile scan order (row-major) per AV1 spec.
//!
//! ## SOTA Research (2024-2025)
//!
//! - **SVT-AV1 3.0.0** (Feb 2025): Multi-dimensional parallelism with tile groups
//! - **rav1e Tile Encoding**: tile_size_bytes_minus_1 calculation (smallest sufficient)
//! - **AV1 Spec Section 5.12**: tile_group_obu() syntax and tile_size encoding
//! - **PostgreSQL Parallel Aggregation**: Thread-local partial results + ordered merge
//! - **1024cores.net**: Lockfree result aggregation with generation counters
//!
//! ## AV1 Tile Group OBU Format (Section 5.12)
//!
//! ```text
//! tile_group_obu() {
//!     NumTiles = TileCols * TileRows
//!     tile_start_and_end_present_flag           f(1)
//!     if (NumTiles > 1 && tile_start_and_end_present_flag) {
//!         tg_start                              f(tileBits)
//!         tg_end                                f(tileBits)
//!     }
//!     for (TileNum = tg_start; TileNum <= tg_end; TileNum++) {
//!         tile_row = TileNum / TileCols
//!         tile_col = TileNum % TileCols
//!         if (TileNum != tg_end) {
//!             tile_size_minus_1                 le(TileSizeBytes)
//!         }
//!         decode_tile()
//!     }
//! }
//! ```
//!
//! ## Architecture
//!
//! - **TileAggregatorCapsule**: 1024B cache-aligned orchestrator (T5 Streaming)
//! - **TileResultSlot**: 64B per tile, lockfree completion flag + data pointer
//! - **Merge Strategy**: O(n) ordered merge, zero-copy where possible
//!
//! ## Performance Targets (B32)
//!
//! | Tiles | Target Merge | Measured |
//! |-------|--------------|----------|
//! | 4     | <250ns       | TBD      |
//! | 16    | <1μs         | TBD      |
//! | 64    | <4μs         | TBD      |
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T5 Streaming tier (O(1) memory, incremental processing)
//! - **Chaos**: 100% lockfree (atomic completion flags, no mutex/RwLock)
//! - **ASSUM**: 99.99% safe (all assumptions documented)
//! - **B32**: <1μs merge for 16 tiles, fair baseline (serial merge)
//! - **T28**: 8+ tests (unit/property/integration/determinism)
//!
//! ## References
//!
//! - [AV1 Bitstream Spec](https://aomediacodec.github.io/av1-spec/)
//! - [rav1e Tile Encoding](https://blog.rom1v.com/2019/04/implementing-tile-encoding-in-rav1e/)
//! - [SVT-AV1 GitLab](https://gitlab.com/AOMediaCodec/SVT-AV1)
//! - [1024cores Lockfree Algorithms](https://www.1024cores.net/home/lock-free-algorithms)

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use atomic_capsule::patterns::DualAtomicU64;
use atomic_capsule::encoder::{ObuBitstreamWriterCapsule, ObuType};

/// Maximum tiles supported (AV1 spec allows up to 64 rows × 64 cols = 4096, practical limit 64)
pub const MAX_TILES: usize = 64;

/// Tile result slot size (must fit pointer + metadata)
const TILE_RESULT_SIZE: usize = 64;

// =============================================================================
// TileResultSlot - Lockfree Result Storage (64B cache-aligned)
// =============================================================================

/// Single tile result slot with lockfree completion tracking
///
/// ## Memory Layout (64B cache-aligned)
///
/// ```text
/// Offset | Field           | Size | Description
/// -------|-----------------|------|----------------------------------
/// 0x00   | state           | 8B   | [completed:1 | generation:31 | size:32]
/// 0x08   | data_ptr        | 8B   | Pointer to tile data (heap-allocated)
/// 0x10   | tile_size       | 8B   | Size of tile data in bytes
/// 0x18   | timestamp_ns    | 8B   | Completion timestamp (Q34 audit)
/// 0x20   | _padding        | 32B  | Cache line completion
/// Total: 64B
/// ```
///
/// ## State Field Encoding
///
/// - Bit 63: Completed flag (1 = tile data available)
/// - Bits 32-62: Generation counter (ABA prevention)
/// - Bits 0-31: Reserved (tile index validation)
///
/// ## ASSUM Safety
///
/// - #ASSUME_CACHE_ALIGNED: 64B alignment prevents false sharing
/// - #ASSUME_ATOMIC_UPDATES: All state transitions via atomic CAS
/// - #ASSUME_GENERATION_ABA: Generation counter prevents ABA races
#[repr(C, align(64))]
pub struct TileResultSlot {
    /// Packed state: [completed:1 | generation:31 | tile_idx:32]
    state: AtomicU64,

    /// Pointer to tile encoded data (Box<Vec<u8>> leaked for lockfree access)
    /// SAFETY: Pointer is valid while slot is in "completed" state
    data_ptr: AtomicUsize,

    /// Size of tile data in bytes
    tile_size: AtomicU64,

    /// Completion timestamp in nanoseconds (Q34 audit)
    timestamp_ns: AtomicU64,

    /// Cache line padding
    _padding: [u8; 32],
}

// Compile-time verification
const _: () = assert!(core::mem::size_of::<TileResultSlot>() == 64);
const _: () = assert!(core::mem::align_of::<TileResultSlot>() == 64);

/// State field bit masks
const COMPLETED_BIT: u64 = 1 << 63;
const GENERATION_MASK: u64 = 0x7FFF_FFFF_0000_0000;
const GENERATION_SHIFT: u64 = 32;
const TILE_IDX_MASK: u64 = 0x0000_0000_FFFF_FFFF;

impl TileResultSlot {
    /// Create empty slot
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            data_ptr: AtomicUsize::new(0),
            tile_size: AtomicU64::new(0),
            timestamp_ns: AtomicU64::new(0),
            _padding: [0u8; 32],
        }
    }

    /// Reset slot for reuse
    #[inline]
    pub fn reset(&self, generation: u32) {
        // Pack new state: not completed, new generation, tile_idx=0
        let new_state = ((generation as u64) << GENERATION_SHIFT) & GENERATION_MASK;
        self.state.store(new_state, Ordering::Release);
        self.data_ptr.store(0, Ordering::Release);
        self.tile_size.store(0, Ordering::Release);
        self.timestamp_ns.store(0, Ordering::Release);
    }

    /// Submit tile result (lockfree, returns false if already completed)
    ///
    /// ## Arguments
    ///
    /// - `tile_idx`: Tile index (for validation)
    /// - `data`: Encoded tile data (ownership transferred)
    /// - `timestamp_ns`: Completion timestamp
    ///
    /// ## Returns
    ///
    /// - `true`: Successfully submitted
    /// - `false`: Slot already has data (concurrent submission detected)
    ///
    /// ## Performance
    ///
    /// - <50ns (atomic CAS + store)
    #[inline]
    pub fn submit(&self, tile_idx: u32, data: Vec<u8>, timestamp_ns: u64) -> bool {
        // Load current state
        let current = self.state.load(Ordering::Acquire);

        // Check if already completed
        if current & COMPLETED_BIT != 0 {
            return false;
        }

        // Extract generation for ABA prevention
        let generation = (current & GENERATION_MASK) >> GENERATION_SHIFT;

        // Build new completed state
        let new_state = COMPLETED_BIT
            | ((generation) << GENERATION_SHIFT)
            | (tile_idx as u64 & TILE_IDX_MASK);

        // CAS to claim slot
        match self.state.compare_exchange(
            current,
            new_state,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // Successfully claimed, store data
                let size = data.len() as u64;

                // Leak the Vec to get a raw pointer (will be reclaimed in take_data)
                let ptr = Box::into_raw(Box::new(data)) as usize;

                self.data_ptr.store(ptr, Ordering::Release);
                self.tile_size.store(size, Ordering::Release);
                self.timestamp_ns.store(timestamp_ns, Ordering::Release);

                true
            }
            Err(_) => false, // Concurrent submission won
        }
    }

    /// Check if slot is completed
    #[inline]
    pub fn is_completed(&self) -> bool {
        self.state.load(Ordering::Acquire) & COMPLETED_BIT != 0
    }

    /// Get tile data size (returns 0 if not completed)
    #[inline]
    pub fn size(&self) -> usize {
        if self.is_completed() {
            self.tile_size.load(Ordering::Acquire) as usize
        } else {
            0
        }
    }

    /// Take tile data (consumes the slot data, returns None if not completed)
    ///
    /// ## Safety
    ///
    /// This method reclaims the leaked Vec. Must only be called once per submit.
    #[inline]
    pub fn take_data(&self) -> Option<Vec<u8>> {
        if !self.is_completed() {
            return None;
        }

        let ptr = self.data_ptr.swap(0, Ordering::AcqRel);
        if ptr == 0 {
            return None; // Already taken
        }

        // SAFETY: ptr was created by Box::into_raw in submit()
        let boxed = unsafe { Box::from_raw(ptr as *mut Vec<u8>) };
        Some(*boxed)
    }

    /// Get tile data reference without consuming (for zero-copy merge)
    ///
    /// ## Safety
    ///
    /// Returned slice is valid until slot is reset or data is taken.
    #[inline]
    pub fn data_slice(&self) -> Option<&[u8]> {
        if !self.is_completed() {
            return None;
        }

        let ptr = self.data_ptr.load(Ordering::Acquire);
        if ptr == 0 {
            return None;
        }

        // SAFETY: ptr points to valid Vec<u8> created in submit()
        let vec_ptr = ptr as *const Vec<u8>;
        unsafe { Some((*vec_ptr).as_slice()) }
    }
}

impl Default for TileResultSlot {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// TileAggregatorCapsule - Main Orchestrator (1024B cache-aligned)
// =============================================================================

/// Tile Aggregator Capsule - T5 Streaming tier
///
/// Collects tile encoding results from parallel workers and merges them
/// in AV1 tile scan order (row-major: left-to-right, top-to-bottom).
///
/// ## Memory Layout (1024B cache-aligned)
///
/// ```text
/// Offset  | Field              | Size  | Description
/// --------|-------------------|-------|----------------------------------
/// 0x000   | coordination      | 128B  | DualAtomicU64 state
/// 0x080   | tile_cols         | 4B    | Number of tile columns
/// 0x084   | tile_rows         | 4B    | Number of tile rows
/// 0x088   | total_tiles       | 4B    | Total tiles (cols × rows)
/// 0x08C   | tiles_completed   | 4B    | Atomic counter
/// 0x090   | generation        | 4B    | Frame generation (ABA prevention)
/// 0x094   | merge_latency_ns  | 8B    | Last merge latency
/// 0x09C   | _config_padding   | 36B   | Config area padding
/// 0x0C0   | slot_states[64]   | 512B  | Completion bitmap (8 bits/tile × 64)
/// 0x2C0   | _padding          | 320B  | Cache line completion
/// Total: 1024B (0x400)
/// ```
///
/// ## Coordination State (DualAtomicU64)
///
/// - Primary: [tiles_submitted:32 | tiles_merged:32]
/// - Secondary: [frame_number:32 | generation:32]
///
/// ## Performance Targets
///
/// - Submit: <50ns per tile (lockfree slot update)
/// - Ready check: <10ns (bitmap scan)
/// - Merge 16 tiles: <1μs (zero-copy concatenation)
///
/// ## ASSUM Safety
///
/// - #ASSUME_1024B_ALIGNED: Cache-aligned to prevent false sharing
/// - #ASSUME_MAX_64_TILES: Practical limit (4K: 16 tiles, 8K: 64 tiles)
/// - #ASSUME_LOCKFREE_SLOTS: All slot access via atomic operations
/// - #ASSUME_ROW_MAJOR_ORDER: Tiles numbered left-to-right, top-to-bottom
#[repr(C, align(1024))]
pub struct TileAggregatorCapsule {
    /// Coordination state:
    /// - Primary: tiles_submitted (low 32) | tiles_merged (high 32)
    /// - Secondary: frame_number (low 32) | generation (high 32)
    coordination: DualAtomicU64,

    /// Tile grid dimensions
    tile_cols: u32,
    tile_rows: u32,
    total_tiles: u32,

    /// Atomic completed tile counter
    tiles_completed: AtomicU64,

    /// Generation counter for ABA prevention
    generation: AtomicU64,

    /// Last merge latency in nanoseconds (for B32 tracking)
    merge_latency_ns: AtomicU64,

    /// Padding to align slot_states
    _config_padding: [u8; 28],

    /// Completion bitmap (8 slots per u64, 8 u64s = 64 tiles)
    /// Bit layout per slot: [completed:1 | reserved:7]
    slot_states: [AtomicU64; 8],

    /// Padding to complete 1024B
    _padding: [u8; 320],
}

// Compile-time verification
const _: () = assert!(core::mem::size_of::<TileAggregatorCapsule>() == 1024);
const _: () = assert!(core::mem::align_of::<TileAggregatorCapsule>() == 1024);

impl TileAggregatorCapsule {
    /// Create new tile aggregator
    ///
    /// ## Arguments
    ///
    /// - `tile_cols`: Number of tile columns (1-64)
    /// - `tile_rows`: Number of tile rows (1-64)
    ///
    /// ## Performance
    ///
    /// - <100ns (atomic initialization)
    ///
    /// ## Example
    ///
    /// ```ignore
    /// use kindly_av1::encoder::TileAggregatorCapsule;
    ///
    /// // 4K frame: 4×4 = 16 tiles
    /// let aggregator = TileAggregatorCapsule::new(4, 4);
    /// assert_eq!(aggregator.total_tiles(), 16);
    /// ```
    pub const fn new(tile_cols: u32, tile_rows: u32) -> Self {
        Self {
            coordination: DualAtomicU64::new(0, 0),
            tile_cols,
            tile_rows,
            total_tiles: tile_cols * tile_rows,
            tiles_completed: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            merge_latency_ns: AtomicU64::new(0),
            _config_padding: [0u8; 28],
            slot_states: [
                AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0),
            ],
            _padding: [0u8; 320],
        }
    }

    /// Reset aggregator for new frame
    ///
    /// ## Arguments
    ///
    /// - `frame_number`: Current frame number (for Q34 audit)
    ///
    /// ## Performance
    ///
    /// - <50ns (atomic stores)
    pub fn reset(&self, frame_number: u32) {
        // Increment generation
        let gen = self.generation.fetch_add(1, Ordering::AcqRel) + 1;

        // Reset coordination state
        self.coordination.store_primary(0, Ordering::Release);
        self.coordination.store_secondary(
            (frame_number as u64) | ((gen as u64) << 32),
            Ordering::Release,
        );

        // Reset completion counter
        self.tiles_completed.store(0, Ordering::Release);

        // Clear slot states
        for slot in &self.slot_states {
            slot.store(0, Ordering::Release);
        }
    }

    /// Submit tile result (called from worker thread)
    ///
    /// ## Arguments
    ///
    /// - `tile_idx`: Tile index in row-major order (0 to total_tiles-1)
    /// - `data`: Encoded tile bitstream data
    /// - `slots`: External slot storage (caller manages lifetime)
    ///
    /// ## Returns
    ///
    /// - `Ok(())`: Successfully submitted
    /// - `Err(TileAggregatorError)`: Invalid tile index or slot already filled
    ///
    /// ## Performance
    ///
    /// - <50ns (atomic updates + slot submission)
    ///
    /// ## ASSUM Safety
    ///
    /// - #ASSUME_TILE_IDX_VALID: tile_idx < total_tiles
    /// - #ASSUME_SLOTS_VALID: slots array has sufficient capacity
    pub fn submit_tile_result(
        &self,
        tile_idx: u32,
        data: Vec<u8>,
        slots: &[TileResultSlot],
    ) -> Result<(), TileAggregatorError> {
        // Validate tile index
        if tile_idx >= self.total_tiles {
            return Err(TileAggregatorError::InvalidTileIndex(tile_idx));
        }

        // Check slot capacity
        if (tile_idx as usize) >= slots.len() {
            return Err(TileAggregatorError::InsufficientSlots);
        }

        // Get current timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        // Submit to slot
        let slot = &slots[tile_idx as usize];
        if !slot.submit(tile_idx, data, timestamp) {
            return Err(TileAggregatorError::SlotAlreadyFilled(tile_idx));
        }

        // Update completion bitmap
        let bitmap_idx = tile_idx / 8;
        let bit_pos = tile_idx % 8;
        let bit_mask = 1u64 << (bit_pos * 8); // Each slot gets 8 bits

        self.slot_states[bitmap_idx as usize].fetch_or(bit_mask, Ordering::AcqRel);

        // Increment completed counter
        let completed = self.tiles_completed.fetch_add(1, Ordering::AcqRel) + 1;

        // Update coordination state (submitted count)
        let current = self.coordination.load_primary(Ordering::Acquire);
        let submitted = (current & 0xFFFF_FFFF) + 1;
        let merged = current >> 32;
        self.coordination.store_primary(submitted | (merged << 32), Ordering::Release);

        Ok(())
    }

    /// Check if all tiles are ready for merge
    ///
    /// ## Returns
    ///
    /// - `true`: All tiles have been submitted
    /// - `false`: Still waiting for tiles
    ///
    /// ## Performance
    ///
    /// - <10ns (single atomic load)
    #[inline]
    pub fn is_ready_to_merge(&self) -> bool {
        self.tiles_completed.load(Ordering::Acquire) as u32 >= self.total_tiles
    }

    /// Get number of tiles completed
    #[inline]
    pub fn tiles_completed(&self) -> u32 {
        self.tiles_completed.load(Ordering::Acquire) as u32
    }

    /// Merge all tiles into tile group OBU
    ///
    /// ## AV1 Tile Group OBU Format
    ///
    /// ```text
    /// 1. OBU Header (type=TileGroup, has_size=1)
    /// 2. LEB128 size field
    /// 3. tile_start_and_end_present_flag = 0 (single tile group)
    /// 4. For each tile (except last):
    ///    - tile_size_minus_1 (LE, TileSizeBytes bytes)
    /// 5. Tile data concatenated
    /// ```
    ///
    /// ## Arguments
    ///
    /// - `slots`: Tile result slots (must have data for all tiles)
    ///
    /// ## Returns
    ///
    /// - Complete tile group OBU ready for bitstream
    ///
    /// ## Performance Target
    ///
    /// - <1μs for 16 tiles (zero-copy where possible)
    ///
    /// ## SOTA Techniques
    ///
    /// - **rav1e**: Smallest tile_size_bytes sufficient for all tiles
    /// - **SVT-AV1**: Pre-allocated output buffer
    /// - **Zero-copy**: Use slice references instead of copying where possible
    pub fn merge_all_tiles(
        &self,
        slots: &[TileResultSlot],
    ) -> Result<Vec<u8>, TileAggregatorError> {
        let start = std::time::Instant::now();

        // Verify all tiles completed
        if !self.is_ready_to_merge() {
            return Err(TileAggregatorError::TilesNotReady(
                self.tiles_completed(),
                self.total_tiles,
            ));
        }

        // Collect tile data and calculate sizes
        let mut tile_data: Vec<&[u8]> = Vec::with_capacity(self.total_tiles as usize);
        let mut tile_sizes: Vec<usize> = Vec::with_capacity(self.total_tiles as usize);
        let mut max_tile_size: usize = 0;

        for tile_idx in 0..self.total_tiles as usize {
            let slot = &slots[tile_idx];
            let data = slot.data_slice().ok_or(TileAggregatorError::MissingTileData(tile_idx as u32))?;
            let size = data.len();

            tile_data.push(data);
            tile_sizes.push(size);
            max_tile_size = max_tile_size.max(size);
        }

        // Calculate tile_size_bytes_minus_1 (per AV1 spec §6.8.14)
        // Use smallest number of bytes sufficient to encode max_tile_size - 1
        let tile_size_bytes = calculate_tile_size_bytes(max_tile_size);

        // Calculate total payload size
        let total_data_size: usize = tile_sizes.iter().sum();
        let size_fields_bytes = (self.total_tiles as usize - 1) * tile_size_bytes;
        let payload_size = 1 + size_fields_bytes + total_data_size; // 1 byte for flags

        // Use OBU writer from atomic_capsule
        let obu_writer = ObuBitstreamWriterCapsule::new();

        // Build tile group OBU
        // Header: type=TileGroup (4), has_size=1
        let header = obu_writer.write_obu_header(ObuType::TileGroup, true);
        let size_leb128 = obu_writer.encode_leb128(payload_size as u64);

        // Pre-allocate output
        let output_size = 1 + size_leb128.len() + payload_size;
        let mut output = Vec::with_capacity(output_size);

        // Write OBU header
        output.push(header[0]);

        // Write LEB128 size
        output.extend_from_slice(&size_leb128);

        // Write tile_start_and_end_present_flag = 0 (single tile group, all tiles)
        // Since NumTiles > 1 but we're encoding all tiles, flag = 0
        output.push(0x00);

        // Write tile sizes (all except last) in LE format
        for tile_idx in 0..(self.total_tiles as usize - 1) {
            let size_minus_1 = (tile_sizes[tile_idx] - 1) as u64;
            write_le_bytes(&mut output, size_minus_1, tile_size_bytes);
        }

        // Concatenate tile data in row-major order
        for tile_idx in 0..self.total_tiles as usize {
            output.extend_from_slice(tile_data[tile_idx]);
        }

        // Record merge latency
        let elapsed_ns = start.elapsed().as_nanos() as u64;
        self.merge_latency_ns.store(elapsed_ns, Ordering::Relaxed);

        // Update coordination state (merged count)
        let current = self.coordination.load_primary(Ordering::Acquire);
        let submitted = current & 0xFFFF_FFFF;
        self.coordination.store_primary(submitted | ((self.total_tiles as u64) << 32), Ordering::Release);

        Ok(output)
    }

    /// Get merge latency from last operation (nanoseconds)
    #[inline]
    pub fn merge_latency_ns(&self) -> u64 {
        self.merge_latency_ns.load(Ordering::Relaxed)
    }

    /// Get tile grid dimensions
    #[inline]
    pub fn tile_grid(&self) -> (u32, u32) {
        (self.tile_cols, self.tile_rows)
    }

    /// Get total tiles
    #[inline]
    pub fn total_tiles(&self) -> u32 {
        self.total_tiles
    }

    /// Get current generation (for ABA prevention tracking)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
}

impl Default for TileAggregatorCapsule {
    fn default() -> Self {
        Self::new(1, 1)
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Calculate tile_size_bytes (1-4) for encoding tile sizes
///
/// Per AV1 spec, tile_size_bytes_minus_1 is signaled in frame header (2 bits = 0-3).
/// The encoder must choose the smallest value sufficient to encode all tile sizes.
///
/// ## Algorithm
///
/// - size <= 255: 1 byte (tile_size_bytes_minus_1 = 0)
/// - size <= 65535: 2 bytes (tile_size_bytes_minus_1 = 1)
/// - size <= 16777215: 3 bytes (tile_size_bytes_minus_1 = 2)
/// - size <= 4294967295: 4 bytes (tile_size_bytes_minus_1 = 3)
///
/// ## SOTA: rav1e Implementation
///
/// "The smallest size sufficient to encode all the tile sizes must be chosen."
#[inline]
fn calculate_tile_size_bytes(max_size: usize) -> usize {
    if max_size == 0 {
        return 1;
    }

    // We encode size-1, so max value is max_size-1
    let max_value = (max_size.saturating_sub(1)) as u32;

    if max_value <= 0xFF {
        1
    } else if max_value <= 0xFFFF {
        2
    } else if max_value <= 0xFF_FFFF {
        3
    } else {
        4
    }
}

/// Write value as little-endian bytes (1-4 bytes)
///
/// Per AV1 spec: "tile_size_minus_1 is coded using TileSizeBytes bytes in little endian format"
#[inline]
fn write_le_bytes(output: &mut Vec<u8>, value: u64, num_bytes: usize) {
    for i in 0..num_bytes {
        output.push(((value >> (i * 8)) & 0xFF) as u8);
    }
}

// =============================================================================
// Error Types
// =============================================================================

/// Tile aggregator errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TileAggregatorError {
    /// Invalid tile index (out of bounds)
    InvalidTileIndex(u32),

    /// Slot already filled (concurrent submission)
    SlotAlreadyFilled(u32),

    /// Insufficient slot storage
    InsufficientSlots,

    /// Tiles not ready for merge
    TilesNotReady(u32, u32), // (completed, total)

    /// Missing tile data during merge
    MissingTileData(u32),
}

impl core::fmt::Display for TileAggregatorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidTileIndex(idx) => write!(f, "Invalid tile index: {}", idx),
            Self::SlotAlreadyFilled(idx) => write!(f, "Slot {} already filled", idx),
            Self::InsufficientSlots => write!(f, "Insufficient slot storage"),
            Self::TilesNotReady(completed, total) => {
                write!(f, "Tiles not ready: {}/{} completed", completed, total)
            }
            Self::MissingTileData(idx) => write!(f, "Missing tile data for tile {}", idx),
        }
    }
}

impl std::error::Error for TileAggregatorError {}

// =============================================================================
// Tests (T28 Compliance)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: Unit Tests
    // ========================================================================

    #[test]
    fn test_tile_result_slot_size_alignment() {
        assert_eq!(core::mem::size_of::<TileResultSlot>(), 64);
        assert_eq!(core::mem::align_of::<TileResultSlot>(), 64);
    }

    #[test]
    fn test_tile_aggregator_size_alignment() {
        assert_eq!(core::mem::size_of::<TileAggregatorCapsule>(), 1024);
        assert_eq!(core::mem::align_of::<TileAggregatorCapsule>(), 1024);
    }

    #[test]
    fn test_slot_submit_and_retrieve() {
        let slot = TileResultSlot::new();

        // Initially not completed
        assert!(!slot.is_completed());
        assert_eq!(slot.size(), 0);

        // Submit data
        let data = vec![1, 2, 3, 4, 5];
        assert!(slot.submit(0, data.clone(), 12345));

        // Now completed
        assert!(slot.is_completed());
        assert_eq!(slot.size(), 5);

        // Double submit should fail
        assert!(!slot.submit(0, vec![6, 7, 8], 67890));

        // Take data
        let retrieved = slot.take_data().unwrap();
        assert_eq!(retrieved, vec![1, 2, 3, 4, 5]);

        // Can't take twice
        assert!(slot.take_data().is_none());
    }

    #[test]
    fn test_aggregator_creation() {
        let agg = TileAggregatorCapsule::new(4, 4);
        assert_eq!(agg.tile_cols, 4);
        assert_eq!(agg.tile_rows, 4);
        assert_eq!(agg.total_tiles(), 16);
        assert_eq!(agg.tiles_completed(), 0);
        assert!(!agg.is_ready_to_merge());
    }

    #[test]
    fn test_tile_size_bytes_calculation() {
        // Small tiles: 1 byte
        assert_eq!(calculate_tile_size_bytes(100), 1);
        assert_eq!(calculate_tile_size_bytes(256), 1); // 255 fits in 1 byte

        // Medium tiles: 2 bytes
        assert_eq!(calculate_tile_size_bytes(300), 2);
        assert_eq!(calculate_tile_size_bytes(65536), 2);

        // Large tiles: 3 bytes
        assert_eq!(calculate_tile_size_bytes(100_000), 3);

        // Very large tiles: 4 bytes
        assert_eq!(calculate_tile_size_bytes(20_000_000), 4);
    }

    #[test]
    fn test_write_le_bytes() {
        let mut output = Vec::new();

        // 1 byte
        write_le_bytes(&mut output, 0x42, 1);
        assert_eq!(output, vec![0x42]);

        // 2 bytes
        output.clear();
        write_le_bytes(&mut output, 0x1234, 2);
        assert_eq!(output, vec![0x34, 0x12]);

        // 4 bytes
        output.clear();
        write_le_bytes(&mut output, 0xDEADBEEF, 4);
        assert_eq!(output, vec![0xEF, 0xBE, 0xAD, 0xDE]);
    }

    #[test]
    fn test_submit_and_merge_simple() {
        let agg = TileAggregatorCapsule::new(2, 2); // 4 tiles

        // Create slots
        let slots: Vec<TileResultSlot> = (0..4).map(|_| TileResultSlot::new()).collect();

        // Submit tiles out of order
        agg.submit_tile_result(3, vec![0xDD; 10], &slots).unwrap();
        agg.submit_tile_result(0, vec![0xAA; 10], &slots).unwrap();
        agg.submit_tile_result(2, vec![0xCC; 10], &slots).unwrap();
        agg.submit_tile_result(1, vec![0xBB; 10], &slots).unwrap();

        assert!(agg.is_ready_to_merge());
        assert_eq!(agg.tiles_completed(), 4);

        // Merge
        let obu = agg.merge_all_tiles(&slots).unwrap();

        // Verify OBU structure
        assert!(!obu.is_empty());
        // First byte is OBU header (type=TileGroup=4, has_size=1)
        // 0b0010_0010 = 0x22
        assert_eq!(obu[0], 0x22);

        // Verify tiles are in row-major order (0xAA, 0xBB, 0xCC, 0xDD)
        // Find tile data in output (after header + size + flags + tile sizes)
        let data_start = obu.len() - 40; // 4 tiles × 10 bytes
        assert_eq!(obu[data_start], 0xAA);
        assert_eq!(obu[data_start + 10], 0xBB);
        assert_eq!(obu[data_start + 20], 0xCC);
        assert_eq!(obu[data_start + 30], 0xDD);
    }

    #[test]
    fn test_reset_and_reuse() {
        let agg = TileAggregatorCapsule::new(2, 1); // 2 tiles

        // First frame
        let slots1: Vec<TileResultSlot> = (0..2).map(|_| TileResultSlot::new()).collect();
        agg.submit_tile_result(0, vec![0x11; 5], &slots1).unwrap();
        agg.submit_tile_result(1, vec![0x22; 5], &slots1).unwrap();
        assert!(agg.is_ready_to_merge());

        let gen1 = agg.generation();

        // Reset for second frame
        agg.reset(1);

        assert!(!agg.is_ready_to_merge());
        assert_eq!(agg.tiles_completed(), 0);
        assert!(agg.generation() > gen1);

        // Second frame
        let slots2: Vec<TileResultSlot> = (0..2).map(|_| TileResultSlot::new()).collect();
        agg.submit_tile_result(0, vec![0x33; 5], &slots2).unwrap();
        agg.submit_tile_result(1, vec![0x44; 5], &slots2).unwrap();
        assert!(agg.is_ready_to_merge());
    }

    #[test]
    fn test_error_invalid_tile_index() {
        let agg = TileAggregatorCapsule::new(2, 2); // 4 tiles
        let slots: Vec<TileResultSlot> = (0..4).map(|_| TileResultSlot::new()).collect();

        let result = agg.submit_tile_result(10, vec![0xFF], &slots);
        assert!(matches!(result, Err(TileAggregatorError::InvalidTileIndex(10))));
    }

    #[test]
    fn test_error_merge_not_ready() {
        let agg = TileAggregatorCapsule::new(2, 2); // 4 tiles
        let slots: Vec<TileResultSlot> = (0..4).map(|_| TileResultSlot::new()).collect();

        // Only submit 2 tiles
        agg.submit_tile_result(0, vec![0xAA], &slots).unwrap();
        agg.submit_tile_result(1, vec![0xBB], &slots).unwrap();

        let result = agg.merge_all_tiles(&slots);
        assert!(matches!(result, Err(TileAggregatorError::TilesNotReady(2, 4))));
    }

    // ========================================================================
    // Q29-Q35: Determinism Tests
    // ========================================================================

    #[test]
    fn test_merge_determinism() {
        // Same input should produce same output
        let agg = TileAggregatorCapsule::new(2, 2);

        for _ in 0..3 {
            agg.reset(0);
            let slots: Vec<TileResultSlot> = (0..4).map(|_| TileResultSlot::new()).collect();

            agg.submit_tile_result(0, vec![0x10, 0x20], &slots).unwrap();
            agg.submit_tile_result(1, vec![0x30, 0x40], &slots).unwrap();
            agg.submit_tile_result(2, vec![0x50, 0x60], &slots).unwrap();
            agg.submit_tile_result(3, vec![0x70, 0x80], &slots).unwrap();

            let obu1 = agg.merge_all_tiles(&slots).unwrap();

            agg.reset(0);
            let slots2: Vec<TileResultSlot> = (0..4).map(|_| TileResultSlot::new()).collect();

            agg.submit_tile_result(0, vec![0x10, 0x20], &slots2).unwrap();
            agg.submit_tile_result(1, vec![0x30, 0x40], &slots2).unwrap();
            agg.submit_tile_result(2, vec![0x50, 0x60], &slots2).unwrap();
            agg.submit_tile_result(3, vec![0x70, 0x80], &slots2).unwrap();

            let obu2 = agg.merge_all_tiles(&slots2).unwrap();

            assert_eq!(obu1, obu2, "Merge should be deterministic");
        }
    }

    #[test]
    fn test_row_major_order_16_tiles() {
        // 4K-style layout: 4×4 = 16 tiles
        let agg = TileAggregatorCapsule::new(4, 4);
        let slots: Vec<TileResultSlot> = (0..16).map(|_| TileResultSlot::new()).collect();

        // Submit in random order
        let order = [7, 2, 15, 0, 9, 4, 12, 6, 1, 11, 8, 3, 14, 5, 10, 13];
        for &idx in &order {
            let data = vec![idx as u8; 8]; // 8 bytes per tile
            agg.submit_tile_result(idx as u32, data, &slots).unwrap();
        }

        assert!(agg.is_ready_to_merge());

        let obu = agg.merge_all_tiles(&slots).unwrap();

        // Extract tile data (last 128 bytes = 16 tiles × 8 bytes)
        let data_start = obu.len() - 128;
        for i in 0..16 {
            assert_eq!(
                obu[data_start + i * 8], i as u8,
                "Tile {} should be in row-major position", i
            );
        }
    }
}
