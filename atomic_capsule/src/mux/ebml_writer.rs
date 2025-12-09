//! # EbmlWriterCapsule (T1 Atomic)
//!
//! Extensible Binary Meta Language (EBML) element serialization for MKV/WebM container formats.
//!
//! **Tier**: T1 Atomic
//! **Size**: 4352 bytes (4096 buffer + 256 metadata), 256-byte aligned
//! **Purpose**: Lockfree EBML element assembly for video container muxing
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! # EBML Specification
//!
//! EBML is a binary XML-like format used by Matroska (MKV) and WebM containers.
//! Key concepts:
//! - **VINT**: Variable-size integers (1-8 bytes, MSB indicates length)
//! - **Element ID**: 1-4 byte identifier with VINT encoding
//! - **Data Size**: 1-8 byte length with VINT encoding (or unknown size: 0x01FFFFFFFFFFFFFF)
//! - **Master Elements**: Containers holding child elements
//!
//! # Layout (4352 bytes, 256-byte aligned)
//!
//! ```text
//! +------+------+------+------+------+------+------+------+
//! | buffer[4096] - Element assembly buffer                |
//! +------+------+------+------+------+------+------+------+
//! | write_pos (AtomicU32)                                 |
//! | stack_depth (AtomicU8)                                |
//! | generation (AtomicU64)                                |
//! | element_stack[8] - Nesting hierarchy (id, size_pos)   |
//! | _padding (to reach 256 metadata bytes)                |
//! +------+------+------+------+------+------+------+------+
//! ```
//!
//! # Performance Targets
//!
//! | Operation | Latency | Notes |
//! |-----------|---------|-------|
//! | encode_vint | <20ns | 1-8 byte variable integer encoding |
//! | write_element_id | <30ns | 1-4 byte element ID |
//! | write_master_start | <50ns | Push to stack, reserve size |
//! | write_master_end | <80ns | Pop stack, patch size |
//! | write_binary | <100ns | ID + size + data copy |
//!
//! # ASSUM Framework
//!
//! - `#ASSUME_LOCKFREE_ONLY`: All coordination via atomics (no mutex/RwLock)
//! - `#ASSUME_BUFFER_SUFFICIENT`: 4096-byte buffer sufficient for typical elements
//! - `#ASSUME_STACK_DEPTH_8`: 8-level nesting sufficient for MKV/WebM structure
//! - `#ASSUME_VINT_SPEC`: VINT encoding per EBML RFC 8794
//! - `#ASSUME_GENERATION_MONOTONIC`: Generation counter prevents ABA issues
//!
//! # Usage
//!
//! ```rust,ignore
//! use atomic_capsule::mux::EbmlWriterCapsule;
//!
//! let writer = EbmlWriterCapsule::new();
//!
//! // Write EBML header
//! writer.write_master_start(EBML_ID)?;
//! writer.write_unsigned(EBML_VERSION_ID, 1)?;
//! writer.write_unsigned(EBML_READ_VERSION_ID, 1)?;
//! writer.write_unsigned(EBML_MAX_ID_LENGTH_ID, 4)?;
//! writer.write_unsigned(EBML_MAX_SIZE_LENGTH_ID, 8)?;
//! writer.write_string(DOC_TYPE_ID, "matroska")?;
//! writer.write_master_end()?;
//!
//! let data = writer.get_data();
//! ```

use core::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};

// ============================================================================
// MKV/WebM Element IDs (Matroska specification)
// ============================================================================

/// EBML Header (0x1A45DFA3)
pub const EBML_ID: u32 = 0x1A45_DFA3;
/// EBML Version (0x4286)
pub const EBML_VERSION_ID: u32 = 0x4286;
/// EBML Read Version (0x42F7)
pub const EBML_READ_VERSION_ID: u32 = 0x42F7;
/// EBML Max ID Length (0x42F2)
pub const EBML_MAX_ID_LENGTH_ID: u32 = 0x42F2;
/// EBML Max Size Length (0x42F3)
pub const EBML_MAX_SIZE_LENGTH_ID: u32 = 0x42F3;
/// Doc Type (0x4282)
pub const DOC_TYPE_ID: u32 = 0x4282;
/// Doc Type Version (0x4287)
pub const DOC_TYPE_VERSION_ID: u32 = 0x4287;
/// Doc Type Read Version (0x4285)
pub const DOC_TYPE_READ_VERSION_ID: u32 = 0x4285;

/// Segment (0x18538067)
pub const SEGMENT_ID: u32 = 0x1853_8067;
/// SeekHead (0x114D9B74)
pub const SEEK_HEAD_ID: u32 = 0x114D_9B74;
/// Seek (0x4DBB)
pub const SEEK_ID: u32 = 0x4DBB;
/// SeekID (0x53AB)
pub const SEEK_ID_ID: u32 = 0x53AB;
/// SeekPosition (0x53AC)
pub const SEEK_POSITION_ID: u32 = 0x53AC;

/// Info (0x1549A966)
pub const INFO_ID: u32 = 0x1549_A966;
/// TimestampScale (0x2AD7B1)
pub const TIMESTAMP_SCALE_ID: u32 = 0x2AD7_B1;
/// Duration (0x4489)
pub const DURATION_ID: u32 = 0x4489;
/// MuxingApp (0x4D80)
pub const MUXING_APP_ID: u32 = 0x4D80;
/// WritingApp (0x5741)
pub const WRITING_APP_ID: u32 = 0x5741;
/// DateUTC (0x4461)
pub const DATE_UTC_ID: u32 = 0x4461;
/// SegmentUID (0x73A4)
pub const SEGMENT_UID_ID: u32 = 0x73A4;

/// Tracks (0x1654AE6B)
pub const TRACKS_ID: u32 = 0x1654_AE6B;
/// TrackEntry (0xAE)
pub const TRACK_ENTRY_ID: u32 = 0xAE;
/// TrackNumber (0xD7)
pub const TRACK_NUMBER_ID: u32 = 0xD7;
/// TrackUID (0x73C5)
pub const TRACK_UID_ID: u32 = 0x73C5;
/// TrackType (0x83)
pub const TRACK_TYPE_ID: u32 = 0x83;
/// FlagEnabled (0xB9)
pub const FLAG_ENABLED_ID: u32 = 0xB9;
/// FlagDefault (0x88)
pub const FLAG_DEFAULT_ID: u32 = 0x88;
/// FlagLacing (0x9C)
pub const FLAG_LACING_ID: u32 = 0x9C;
/// CodecID (0x86)
pub const CODEC_ID_ID: u32 = 0x86;
/// CodecPrivate (0x63A2)
pub const CODEC_PRIVATE_ID: u32 = 0x63A2;
/// Language (0x22B59C)
pub const LANGUAGE_ID: u32 = 0x22B5_9C;
/// Name (0x536E)
pub const NAME_ID: u32 = 0x536E;

/// Video (0xE0)
pub const VIDEO_ID: u32 = 0xE0;
/// PixelWidth (0xB0)
pub const PIXEL_WIDTH_ID: u32 = 0xB0;
/// PixelHeight (0xBA)
pub const PIXEL_HEIGHT_ID: u32 = 0xBA;
/// DisplayWidth (0x54B0)
pub const DISPLAY_WIDTH_ID: u32 = 0x54B0;
/// DisplayHeight (0x54BA)
pub const DISPLAY_HEIGHT_ID: u32 = 0x54BA;
/// FlagInterlaced (0x9A)
pub const FLAG_INTERLACED_ID: u32 = 0x9A;
/// ColourSpace (0x2EB524)
pub const COLOUR_SPACE_ID: u32 = 0x2EB5_24;

/// Audio (0xE1)
pub const AUDIO_ID: u32 = 0xE1;
/// SamplingFrequency (0xB5)
pub const SAMPLING_FREQUENCY_ID: u32 = 0xB5;
/// Channels (0x9F)
pub const CHANNELS_ID: u32 = 0x9F;
/// BitDepth (0x6264)
pub const BIT_DEPTH_ID: u32 = 0x6264;

/// Chapters (0x1043A770)
pub const CHAPTERS_ID: u32 = 0x1043_A770;
/// EditionEntry (0x45B9)
pub const EDITION_ENTRY_ID: u32 = 0x45B9;
/// ChapterAtom (0xB6)
pub const CHAPTER_ATOM_ID: u32 = 0xB6;
/// ChapterUID (0x73C4)
pub const CHAPTER_UID_ID: u32 = 0x73C4;
/// ChapterTimeStart (0x91)
pub const CHAPTER_TIME_START_ID: u32 = 0x91;
/// ChapterTimeEnd (0x92)
pub const CHAPTER_TIME_END_ID: u32 = 0x92;
/// ChapterDisplay (0x80)
pub const CHAPTER_DISPLAY_ID: u32 = 0x80;
/// ChapString (0x85)
pub const CHAP_STRING_ID: u32 = 0x85;
/// ChapLanguage (0x437C)
pub const CHAP_LANGUAGE_ID: u32 = 0x437C;

/// Cluster (0x1F43B675)
pub const CLUSTER_ID: u32 = 0x1F43_B675;
/// Timestamp (0xE7)
pub const TIMESTAMP_ID: u32 = 0xE7;
/// SimpleBlock (0xA3)
pub const SIMPLE_BLOCK_ID: u32 = 0xA3;
/// BlockGroup (0xA0)
pub const BLOCK_GROUP_ID: u32 = 0xA0;
/// Block (0xA1)
pub const BLOCK_ID: u32 = 0xA1;
/// BlockDuration (0x9B)
pub const BLOCK_DURATION_ID: u32 = 0x9B;
/// ReferenceBlock (0xFB)
pub const REFERENCE_BLOCK_ID: u32 = 0xFB;

/// Cues (0x1C53BB6B)
pub const CUES_ID: u32 = 0x1C53_BB6B;
/// CuePoint (0xBB)
pub const CUE_POINT_ID: u32 = 0xBB;
/// CueTime (0xB3)
pub const CUE_TIME_ID: u32 = 0xB3;
/// CueTrackPositions (0xB7)
pub const CUE_TRACK_POSITIONS_ID: u32 = 0xB7;
/// CueTrack (0xF7)
pub const CUE_TRACK_ID: u32 = 0xF7;
/// CueClusterPosition (0xF1)
pub const CUE_CLUSTER_POSITION_ID: u32 = 0xF1;

/// Tags (0x1254C367)
pub const TAGS_ID: u32 = 0x1254_C367;
/// Tag (0x7373)
pub const TAG_ID: u32 = 0x7373;
/// Targets (0x63C0)
pub const TARGETS_ID: u32 = 0x63C0;
/// TagTrackUID (0x63C5)
pub const TAG_TRACK_UID_ID: u32 = 0x63C5;
/// SimpleTag (0x67C8)
pub const SIMPLE_TAG_ID: u32 = 0x67C8;
/// TagName (0x45A3)
pub const TAG_NAME_ID: u32 = 0x45A3;
/// TagString (0x4487)
pub const TAG_STRING_ID: u32 = 0x4487;

/// Void (0xEC) - Padding element
pub const VOID_ID: u32 = 0xEC;
/// CRC-32 (0xBF)
pub const CRC32_ID: u32 = 0xBF;

/// Unknown size marker (streaming mode)
pub const UNKNOWN_SIZE_8: u64 = 0x01FF_FFFF_FFFF_FFFF;

// ============================================================================
// Error Types
// ============================================================================

/// EBML Writer errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EbmlError {
    /// Buffer overflow - not enough space
    BufferOverflow,
    /// Stack overflow - too many nested elements
    StackOverflow,
    /// Stack underflow - no element to close
    StackUnderflow,
    /// Invalid element ID (must be 1-4 bytes)
    InvalidElementId,
    /// Invalid VINT value (too large)
    InvalidVint,
    /// Invalid UTF-8 string
    InvalidUtf8,
    /// Size exceeds maximum (8-byte VINT limit)
    SizeOverflow,
    /// Concurrent modification detected (generation mismatch)
    ConcurrentModification,
}

impl core::fmt::Display for EbmlError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EbmlError::BufferOverflow => write!(f, "Buffer overflow"),
            EbmlError::StackOverflow => write!(f, "Stack overflow (max 8 levels)"),
            EbmlError::StackUnderflow => write!(f, "Stack underflow (no element to close)"),
            EbmlError::InvalidElementId => write!(f, "Invalid element ID"),
            EbmlError::InvalidVint => write!(f, "Invalid VINT value"),
            EbmlError::InvalidUtf8 => write!(f, "Invalid UTF-8 string"),
            EbmlError::SizeOverflow => write!(f, "Size overflow"),
            EbmlError::ConcurrentModification => write!(f, "Concurrent modification"),
        }
    }
}

// ============================================================================
// Element Stack Entry
// ============================================================================

/// Stack entry for tracking master element hierarchy
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct ElementStackEntry {
    /// Element ID (for validation)
    pub element_id: u32,
    /// Position in buffer where size bytes begin
    pub size_position: u32,
    /// Start position of element content
    pub content_start: u32,
    /// Reserved for future use (alignment)
    pub _reserved: u32,
}

impl ElementStackEntry {
    /// Create a new stack entry
    #[inline]
    pub const fn new(element_id: u32, size_position: u32, content_start: u32) -> Self {
        ElementStackEntry {
            element_id,
            size_position,
            content_start,
            _reserved: 0,
        }
    }

    /// Check if entry is empty
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.element_id == 0 && self.size_position == 0
    }
}

// ============================================================================
// EBML Writer Capsule
// ============================================================================

/// EBML element serialization capsule for MKV/WebM.
///
/// **Tier**: T1 Atomic
/// **Size**: 4352 bytes (4096 buffer + 256 metadata), 256-byte aligned
///
/// # ASSUM Safety Tags
///
/// - `#ASSUME_LOCKFREE_ONLY`: All coordination via atomics (verified)
/// - `#ASSUME_BUFFER_4096`: 4096-byte buffer for element assembly (sufficient for typical elements)
/// - `#ASSUME_STACK_8`: 8-level stack for nested master elements
/// - `#ASSUME_VINT_RFC8794`: VINT encoding per EBML specification
/// - `#ASSUME_GENERATION_ABA`: Generation counter prevents ABA race conditions
///
/// # Safety Proof
///
/// - Alignment: `#[repr(C, align(256))]` enforces 256-byte alignment
/// - Buffer bounds: All writes check `write_pos + len <= BUFFER_SIZE`
/// - Stack bounds: Stack depth checked before push/pop
/// - Generation: Atomic increment on each modification
#[repr(C, align(256))]
pub struct EbmlWriterCapsule {
    /// Element assembly buffer (4096 bytes)
    buffer: [u8; 4096],

    /// Current write position in buffer
    write_pos: AtomicU32,

    /// Current stack depth (0-7)
    stack_depth: AtomicU8,

    /// Generation counter for ABA prevention
    generation: AtomicU64,

    /// Element nesting stack (8 entries × 16 bytes = 128 bytes)
    element_stack: [ElementStackEntry; 8],

    /// Padding to reach 256 bytes metadata
    /// 256 - 4 (write_pos) - 1 (stack_depth) - 8 (generation) - 128 (stack) - 3 (align) = 112 bytes
    _padding: [u8; 112],
}

/// Compile-time size verification
const _: () = {
    // Buffer: 4096 bytes
    // Metadata: 256 bytes (write_pos=4, stack_depth=1, generation=8, stack=128, padding=112, alignment=3)
    // Total: 4352 bytes aligned to 256
    const EXPECTED_SIZE: usize = 4352;
    const fn check_size() {
        if core::mem::size_of::<EbmlWriterCapsule>() != EXPECTED_SIZE {
            panic!("EbmlWriterCapsule size mismatch");
        }
    }
    check_size();
};

impl EbmlWriterCapsule {
    /// Buffer size constant
    pub const BUFFER_SIZE: usize = 4096;

    /// Maximum stack depth
    pub const MAX_STACK_DEPTH: usize = 8;

    /// Create a new EBML writer with empty buffer.
    ///
    /// # Returns
    ///
    /// New writer with zero write position and empty stack.
    ///
    /// # Performance
    ///
    /// <10ns (const initialization)
    #[inline]
    pub const fn new() -> Self {
        EbmlWriterCapsule {
            buffer: [0u8; 4096],
            write_pos: AtomicU32::new(0),
            stack_depth: AtomicU8::new(0),
            generation: AtomicU64::new(0),
            element_stack: [
                ElementStackEntry::new(0, 0, 0),
                ElementStackEntry::new(0, 0, 0),
                ElementStackEntry::new(0, 0, 0),
                ElementStackEntry::new(0, 0, 0),
                ElementStackEntry::new(0, 0, 0),
                ElementStackEntry::new(0, 0, 0),
                ElementStackEntry::new(0, 0, 0),
                ElementStackEntry::new(0, 0, 0),
            ],
            _padding: [0u8; 112],
        }
    }

    /// Reset the writer to empty state.
    ///
    /// # Performance
    ///
    /// <20ns (atomic stores only, no buffer clear)
    #[inline]
    pub fn reset(&self) {
        self.write_pos.store(0, Ordering::Release);
        self.stack_depth.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Get current write position.
    #[inline]
    pub fn position(&self) -> u32 {
        self.write_pos.load(Ordering::Acquire)
    }

    /// Get current stack depth.
    #[inline]
    pub fn depth(&self) -> u8 {
        self.stack_depth.load(Ordering::Acquire)
    }

    /// Get generation counter.
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get written data as slice.
    ///
    /// # Safety
    ///
    /// This provides a snapshot of the buffer up to current write_pos.
    /// The caller should ensure no concurrent writes occur.
    #[inline]
    pub fn get_data(&self) -> &[u8] {
        let pos = self.write_pos.load(Ordering::Acquire) as usize;
        // #ASSUME_POS_IN_BOUNDS: write_pos is always <= BUFFER_SIZE
        // #VERIFY: All write operations check bounds before incrementing
        &self.buffer[..pos]
    }

    /// Get mutable buffer for external patching (use with caution).
    ///
    /// # Safety
    ///
    /// This returns the raw buffer. Caller must ensure:
    /// - No concurrent writes occur
    /// - Written data stays within bounds
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_SINGLE_WRITER`: Only one writer modifies buffer at a time
    /// - `#VERIFY: Caller is responsible for synchronization`
    #[inline]
    pub fn get_buffer_mut(&mut self) -> &mut [u8; 4096] {
        &mut self.buffer
    }

    // ========================================================================
    // VINT (Variable-size Integer) Encoding
    // ========================================================================

    /// Calculate the minimum number of bytes needed to encode a value as VINT.
    ///
    /// EBML VINT encoding:
    /// - 1 byte: values 0 - 126 (0x7E), marker bit at 0x80
    /// - 2 bytes: values 0 - 16382 (0x3FFE), marker bits at 0x4000
    /// - 3 bytes: values 0 - 2097150 (0x1FFFFE), marker bits at 0x200000
    /// - 4 bytes: values 0 - 268435454 (0x0FFFFFFE), marker bits at 0x10000000
    /// - etc. up to 8 bytes
    ///
    /// # Parameters
    ///
    /// - `value`: The value to encode
    ///
    /// # Returns
    ///
    /// Number of bytes needed (1-8)
    #[inline]
    pub const fn vint_size(value: u64) -> usize {
        if value <= 0x7E {
            1
        } else if value <= 0x3FFE {
            2
        } else if value <= 0x1F_FFFE {
            3
        } else if value <= 0x0FFF_FFFE {
            4
        } else if value <= 0x07_FFFF_FFFE {
            5
        } else if value <= 0x03FF_FFFF_FFFE {
            6
        } else if value <= 0x01_FFFF_FFFF_FFFE {
            7
        } else {
            8
        }
    }

    /// Encode a value as VINT with minimum bytes.
    ///
    /// # Parameters
    ///
    /// - `value`: The value to encode
    /// - `dest`: Destination buffer (must have at least 8 bytes)
    ///
    /// # Returns
    ///
    /// Number of bytes written
    ///
    /// # Performance
    ///
    /// <20ns
    #[inline]
    pub fn encode_vint(value: u64, dest: &mut [u8]) -> usize {
        let size = Self::vint_size(value);
        Self::encode_vint_fixed(value, dest, size)
    }

    /// Encode a value as VINT with a specific number of bytes.
    ///
    /// # Parameters
    ///
    /// - `value`: The value to encode
    /// - `dest`: Destination buffer
    /// - `size`: Number of bytes to use (1-8)
    ///
    /// # Returns
    ///
    /// Number of bytes written
    #[inline]
    pub fn encode_vint_fixed(value: u64, dest: &mut [u8], size: usize) -> usize {
        // #ASSUME_SIZE_VALID: size is 1-8
        // #VERIFY: Caller provides valid size from vint_size()
        // EBML VINT marker: 1-byte=0x80, 2-byte=0x4000, 3-byte=0x200000, etc.
        // Formula: marker = 1 << (7 * size) places the marker bit correctly
        let marker = 1u64 << (7 * size);
        let val_with_marker = value | marker;

        // Write big-endian bytes
        for i in 0..size {
            let shift = (size - 1 - i) * 8;
            dest[i] = ((val_with_marker >> shift) & 0xFF) as u8;
        }
        size
    }

    /// Encode unknown size (streaming mode) for Segment/Cluster.
    ///
    /// Unknown size is represented as all 1s except marker bit:
    /// - 1 byte: 0xFF (not used - reserved for element IDs)
    /// - 8 bytes: 0x01FFFFFFFFFFFFFF (standard unknown size)
    ///
    /// # Parameters
    ///
    /// - `dest`: Destination buffer (must have at least 8 bytes)
    ///
    /// # Returns
    ///
    /// Number of bytes written (8)
    #[inline]
    pub fn encode_unknown_size(dest: &mut [u8]) -> usize {
        // 8-byte unknown size: 0x01FFFFFFFFFFFFFF
        dest[0] = 0x01;
        dest[1] = 0xFF;
        dest[2] = 0xFF;
        dest[3] = 0xFF;
        dest[4] = 0xFF;
        dest[5] = 0xFF;
        dest[6] = 0xFF;
        dest[7] = 0xFF;
        8
    }

    // ========================================================================
    // Element ID Encoding
    // ========================================================================

    /// Calculate bytes needed for element ID.
    ///
    /// EBML element IDs use VINT encoding with leading 1 bit:
    /// - 1 byte: 0x80-0xFE (IDs 0x80-0xFE)
    /// - 2 bytes: 0x4000-0x7FFE
    /// - 3 bytes: 0x200000-0x3FFFFE
    /// - 4 bytes: 0x10000000-0x1FFFFFFE
    ///
    /// # Parameters
    ///
    /// - `id`: Element ID
    ///
    /// # Returns
    ///
    /// Number of bytes needed (1-4)
    #[inline]
    pub const fn element_id_size(id: u32) -> usize {
        if id <= 0xFE && id >= 0x80 {
            1
        } else if id <= 0x7FFE && id >= 0x4000 {
            2
        } else if id <= 0x3F_FFFE && id >= 0x20_0000 {
            3
        } else {
            4
        }
    }

    /// Write element ID to buffer.
    ///
    /// # Parameters
    ///
    /// - `id`: Element ID (1-4 bytes when encoded)
    ///
    /// # Returns
    ///
    /// Number of bytes written, or error
    ///
    /// # Performance
    ///
    /// <30ns
    pub fn write_element_id(&mut self, id: u32) -> Result<usize, EbmlError> {
        let size = Self::element_id_size(id);
        let pos = self.write_pos.load(Ordering::Acquire) as usize;

        // Check buffer space
        if pos + size > Self::BUFFER_SIZE {
            return Err(EbmlError::BufferOverflow);
        }

        // Write ID bytes (big-endian)
        for i in 0..size {
            let shift = (size - 1 - i) * 8;
            self.buffer[pos + i] = ((id >> shift) & 0xFF) as u8;
        }

        self.write_pos.store((pos + size) as u32, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        Ok(size)
    }

    // ========================================================================
    // Master Element Handling
    // ========================================================================

    /// Start a master (container) element.
    ///
    /// This writes the element ID and reserves space for the size,
    /// pushing the element onto the stack for later completion.
    ///
    /// # Parameters
    ///
    /// - `id`: Master element ID
    ///
    /// # Returns
    ///
    /// Position where content starts, or error
    ///
    /// # Performance
    ///
    /// <50ns
    pub fn write_master_start(&mut self, id: u32) -> Result<u32, EbmlError> {
        let depth = self.stack_depth.load(Ordering::Acquire) as usize;
        if depth >= Self::MAX_STACK_DEPTH {
            return Err(EbmlError::StackOverflow);
        }

        let pos = self.write_pos.load(Ordering::Acquire) as usize;
        let id_size = Self::element_id_size(id);

        // Reserve 8 bytes for size (maximum VINT size for patching later)
        let total_header = id_size + 8;
        if pos + total_header > Self::BUFFER_SIZE {
            return Err(EbmlError::BufferOverflow);
        }

        // Write element ID
        for i in 0..id_size {
            let shift = (id_size - 1 - i) * 8;
            self.buffer[pos + i] = ((id >> shift) & 0xFF) as u8;
        }

        let size_pos = pos + id_size;
        let content_start = size_pos + 8;

        // Initialize size to zero (will be patched in write_master_end)
        for i in 0..8 {
            self.buffer[size_pos + i] = 0;
        }

        // Push to stack
        self.element_stack[depth] =
            ElementStackEntry::new(id, size_pos as u32, content_start as u32);

        self.write_pos
            .store(content_start as u32, Ordering::Release);
        self.stack_depth.store((depth + 1) as u8, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(content_start as u32)
    }

    /// Start a master element with unknown size (streaming mode).
    ///
    /// Used for Segment and Cluster elements in streaming scenarios.
    ///
    /// # Parameters
    ///
    /// - `id`: Master element ID
    ///
    /// # Returns
    ///
    /// Position where content starts, or error
    pub fn write_master_start_unknown_size(&mut self, id: u32) -> Result<u32, EbmlError> {
        let depth = self.stack_depth.load(Ordering::Acquire) as usize;
        if depth >= Self::MAX_STACK_DEPTH {
            return Err(EbmlError::StackOverflow);
        }

        let pos = self.write_pos.load(Ordering::Acquire) as usize;
        let id_size = Self::element_id_size(id);
        let total_header = id_size + 8; // Unknown size is always 8 bytes

        if pos + total_header > Self::BUFFER_SIZE {
            return Err(EbmlError::BufferOverflow);
        }

        // Write element ID
        for i in 0..id_size {
            let shift = (id_size - 1 - i) * 8;
            self.buffer[pos + i] = ((id >> shift) & 0xFF) as u8;
        }

        let size_pos = pos + id_size;
        let content_start = size_pos + 8;

        // Write unknown size marker
        Self::encode_unknown_size(&mut self.buffer[size_pos..]);

        // Push to stack with special marker (size_position = 0xFFFFFFFF means unknown)
        self.element_stack[depth] = ElementStackEntry::new(
            id,
            0xFFFF_FFFF, // Special marker for unknown size
            content_start as u32,
        );

        self.write_pos
            .store(content_start as u32, Ordering::Release);
        self.stack_depth.store((depth + 1) as u8, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(content_start as u32)
    }

    /// End a master (container) element.
    ///
    /// This pops the element from the stack and patches the size field.
    ///
    /// # Returns
    ///
    /// The element ID that was closed, or error
    ///
    /// # Performance
    ///
    /// <80ns
    pub fn write_master_end(&mut self) -> Result<u32, EbmlError> {
        let depth = self.stack_depth.load(Ordering::Acquire) as usize;
        if depth == 0 {
            return Err(EbmlError::StackUnderflow);
        }

        let entry = self.element_stack[depth - 1];
        let size_pos = entry.size_position as usize;
        let content_start = entry.content_start as usize;
        let current_pos = self.write_pos.load(Ordering::Acquire) as usize;

        // Skip patching for unknown size elements
        if size_pos != 0xFFFF_FFFF {
            // Calculate content size
            let content_size = (current_pos - content_start) as u64;

            // Encode size as 8-byte VINT (always use 8 bytes for reserved space)
            Self::encode_vint_fixed(content_size, &mut self.buffer[size_pos..], 8);
        }

        // Pop from stack
        self.element_stack[depth - 1] = ElementStackEntry::new(0, 0, 0);
        self.stack_depth.store((depth - 1) as u8, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(entry.element_id)
    }

    // ========================================================================
    // Primitive Element Writers
    // ========================================================================

    /// Write a binary element.
    ///
    /// # Parameters
    ///
    /// - `id`: Element ID
    /// - `data`: Binary data to write
    ///
    /// # Returns
    ///
    /// Total bytes written (ID + size + data), or error
    ///
    /// # Performance
    ///
    /// <100ns + data copy time
    pub fn write_binary(&mut self, id: u32, data: &[u8]) -> Result<usize, EbmlError> {
        let pos = self.write_pos.load(Ordering::Acquire) as usize;
        let id_size = Self::element_id_size(id);
        let size_size = Self::vint_size(data.len() as u64);
        let total = id_size + size_size + data.len();

        if pos + total > Self::BUFFER_SIZE {
            return Err(EbmlError::BufferOverflow);
        }

        // Write element ID
        let mut offset = pos;
        for i in 0..id_size {
            let shift = (id_size - 1 - i) * 8;
            self.buffer[offset + i] = ((id >> shift) & 0xFF) as u8;
        }
        offset += id_size;

        // Write size
        let size_written = Self::encode_vint(data.len() as u64, &mut self.buffer[offset..]);
        offset += size_written;

        // Copy data
        self.buffer[offset..offset + data.len()].copy_from_slice(data);

        self.write_pos
            .store((pos + total) as u32, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        Ok(total)
    }

    /// Write a UTF-8 string element.
    ///
    /// # Parameters
    ///
    /// - `id`: Element ID
    /// - `s`: UTF-8 string
    ///
    /// # Returns
    ///
    /// Total bytes written, or error
    pub fn write_string(&mut self, id: u32, s: &str) -> Result<usize, EbmlError> {
        self.write_binary(id, s.as_bytes())
    }

    /// Write an unsigned integer element (1-8 bytes, minimum encoding).
    ///
    /// # Parameters
    ///
    /// - `id`: Element ID
    /// - `value`: Unsigned integer value
    ///
    /// # Returns
    ///
    /// Total bytes written, or error
    ///
    /// # Performance
    ///
    /// <50ns
    pub fn write_unsigned(&mut self, id: u32, value: u64) -> Result<usize, EbmlError> {
        // Calculate minimum bytes needed for value
        let value_size = if value == 0 {
            1
        } else {
            ((64 - value.leading_zeros() + 7) / 8) as usize
        };

        let pos = self.write_pos.load(Ordering::Acquire) as usize;
        let id_size = Self::element_id_size(id);
        let size_size = Self::vint_size(value_size as u64);
        let total = id_size + size_size + value_size;

        if pos + total > Self::BUFFER_SIZE {
            return Err(EbmlError::BufferOverflow);
        }

        // Write element ID
        let mut offset = pos;
        for i in 0..id_size {
            let shift = (id_size - 1 - i) * 8;
            self.buffer[offset + i] = ((id >> shift) & 0xFF) as u8;
        }
        offset += id_size;

        // Write size
        let size_written = Self::encode_vint(value_size as u64, &mut self.buffer[offset..]);
        offset += size_written;

        // Write value (big-endian)
        for i in 0..value_size {
            let shift = (value_size - 1 - i) * 8;
            self.buffer[offset + i] = ((value >> shift) & 0xFF) as u8;
        }

        self.write_pos
            .store((pos + total) as u32, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        Ok(total)
    }

    /// Write a signed integer element (1-8 bytes, minimum encoding).
    ///
    /// # Parameters
    ///
    /// - `id`: Element ID
    /// - `value`: Signed integer value
    ///
    /// # Returns
    ///
    /// Total bytes written, or error
    pub fn write_signed(&mut self, id: u32, value: i64) -> Result<usize, EbmlError> {
        // Calculate minimum bytes needed for signed value
        let value_size = if value == 0 {
            1
        } else if value > 0 {
            // Positive: need sign bit = 0
            ((65 - value.leading_zeros() + 7) / 8) as usize
        } else {
            // Negative: need sign bit = 1
            ((65 - (!value).leading_zeros() + 7) / 8) as usize
        };

        let pos = self.write_pos.load(Ordering::Acquire) as usize;
        let id_size = Self::element_id_size(id);
        let size_size = Self::vint_size(value_size as u64);
        let total = id_size + size_size + value_size;

        if pos + total > Self::BUFFER_SIZE {
            return Err(EbmlError::BufferOverflow);
        }

        // Write element ID
        let mut offset = pos;
        for i in 0..id_size {
            let shift = (id_size - 1 - i) * 8;
            self.buffer[offset + i] = ((id >> shift) & 0xFF) as u8;
        }
        offset += id_size;

        // Write size
        let size_written = Self::encode_vint(value_size as u64, &mut self.buffer[offset..]);
        offset += size_written;

        // Write value (big-endian, sign-extended)
        let bytes = value.to_be_bytes();
        let start = 8 - value_size;
        self.buffer[offset..offset + value_size].copy_from_slice(&bytes[start..]);

        self.write_pos
            .store((pos + total) as u32, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        Ok(total)
    }

    /// Write a 32-bit float element.
    ///
    /// # Parameters
    ///
    /// - `id`: Element ID
    /// - `value`: 32-bit float value
    ///
    /// # Returns
    ///
    /// Total bytes written, or error
    pub fn write_float32(&mut self, id: u32, value: f32) -> Result<usize, EbmlError> {
        let bytes = value.to_be_bytes();
        self.write_binary_fixed(id, &bytes, 4)
    }

    /// Write a 64-bit float element.
    ///
    /// # Parameters
    ///
    /// - `id`: Element ID
    /// - `value`: 64-bit float value
    ///
    /// # Returns
    ///
    /// Total bytes written, or error
    pub fn write_float64(&mut self, id: u32, value: f64) -> Result<usize, EbmlError> {
        let bytes = value.to_be_bytes();
        self.write_binary_fixed(id, &bytes, 8)
    }

    /// Write a date element (nanoseconds since 2001-01-01 00:00:00 UTC).
    ///
    /// EBML dates are signed 64-bit integers representing nanoseconds
    /// since the millennium (2001-01-01 00:00:00.000000000 UTC).
    ///
    /// # Parameters
    ///
    /// - `id`: Element ID
    /// - `ns_since_2001`: Nanoseconds since 2001-01-01 UTC
    ///
    /// # Returns
    ///
    /// Total bytes written, or error
    pub fn write_date(&mut self, id: u32, ns_since_2001: i64) -> Result<usize, EbmlError> {
        // Date is always 8 bytes
        let bytes = ns_since_2001.to_be_bytes();
        self.write_binary_fixed(id, &bytes, 8)
    }

    /// Write a binary element with fixed size (no size optimization).
    fn write_binary_fixed(
        &mut self,
        id: u32,
        data: &[u8],
        size: usize,
    ) -> Result<usize, EbmlError> {
        let pos = self.write_pos.load(Ordering::Acquire) as usize;
        let id_size = Self::element_id_size(id);
        let size_size = Self::vint_size(size as u64);
        let total = id_size + size_size + size;

        if pos + total > Self::BUFFER_SIZE {
            return Err(EbmlError::BufferOverflow);
        }

        // Write element ID
        let mut offset = pos;
        for i in 0..id_size {
            let shift = (id_size - 1 - i) * 8;
            self.buffer[offset + i] = ((id >> shift) & 0xFF) as u8;
        }
        offset += id_size;

        // Write size
        let size_written = Self::encode_vint(size as u64, &mut self.buffer[offset..]);
        offset += size_written;

        // Copy data (pad with zeros if needed)
        let copy_len = data.len().min(size);
        self.buffer[offset..offset + copy_len].copy_from_slice(&data[..copy_len]);
        for i in copy_len..size {
            self.buffer[offset + i] = 0;
        }

        self.write_pos
            .store((pos + total) as u32, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        Ok(total)
    }

    /// Write a Void (padding) element.
    ///
    /// # Parameters
    ///
    /// - `size`: Size of void content (total element size = ID + size VINT + content)
    ///
    /// # Returns
    ///
    /// Total bytes written, or error
    pub fn write_void(&mut self, size: usize) -> Result<usize, EbmlError> {
        let pos = self.write_pos.load(Ordering::Acquire) as usize;
        let id_size = 1; // VOID_ID = 0xEC (1 byte)
        let size_size = Self::vint_size(size as u64);
        let total = id_size + size_size + size;

        if pos + total > Self::BUFFER_SIZE {
            return Err(EbmlError::BufferOverflow);
        }

        // Write element ID (0xEC)
        self.buffer[pos] = VOID_ID as u8;

        // Write size
        let size_written = Self::encode_vint(size as u64, &mut self.buffer[pos + 1..]);

        // Fill with zeros
        let content_start = pos + id_size + size_written;
        for i in 0..size {
            self.buffer[content_start + i] = 0;
        }

        self.write_pos
            .store((pos + total) as u32, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        Ok(total)
    }

    // ========================================================================
    // High-Level Helpers
    // ========================================================================

    /// Write complete EBML header for Matroska.
    ///
    /// # Parameters
    ///
    /// - `doc_type`: Document type ("matroska" or "webm")
    /// - `doc_type_version`: Document type version (usually 4)
    /// - `doc_type_read_version`: Minimum version to read (usually 2)
    ///
    /// # Returns
    ///
    /// Total bytes written, or error
    pub fn write_ebml_header(
        &mut self,
        doc_type: &str,
        doc_type_version: u64,
        doc_type_read_version: u64,
    ) -> Result<usize, EbmlError> {
        let start_pos = self.position();

        self.write_master_start(EBML_ID)?;
        self.write_unsigned(EBML_VERSION_ID, 1)?;
        self.write_unsigned(EBML_READ_VERSION_ID, 1)?;
        self.write_unsigned(EBML_MAX_ID_LENGTH_ID, 4)?;
        self.write_unsigned(EBML_MAX_SIZE_LENGTH_ID, 8)?;
        self.write_string(DOC_TYPE_ID, doc_type)?;
        self.write_unsigned(DOC_TYPE_VERSION_ID, doc_type_version)?;
        self.write_unsigned(DOC_TYPE_READ_VERSION_ID, doc_type_read_version)?;
        self.write_master_end()?;

        Ok((self.position() - start_pos) as usize)
    }

    /// Write a SimpleBlock element.
    ///
    /// # Parameters
    ///
    /// - `track_number`: Track number (VINT encoded in block)
    /// - `timestamp`: Relative timestamp (signed 16-bit)
    /// - `keyframe`: Is this a keyframe?
    /// - `data`: Frame data
    ///
    /// # Returns
    ///
    /// Total bytes written, or error
    pub fn write_simple_block(
        &mut self,
        track_number: u64,
        timestamp: i16,
        keyframe: bool,
        data: &[u8],
    ) -> Result<usize, EbmlError> {
        let pos = self.write_pos.load(Ordering::Acquire) as usize;
        let id_size = 1; // SIMPLE_BLOCK_ID = 0xA3 (1 byte)
        let track_vint_size = Self::vint_size(track_number);
        let header_size = track_vint_size + 2 + 1; // track + timestamp(2) + flags(1)
        let content_size = header_size + data.len();
        let size_vint_size = Self::vint_size(content_size as u64);
        let total = id_size + size_vint_size + content_size;

        if pos + total > Self::BUFFER_SIZE {
            return Err(EbmlError::BufferOverflow);
        }

        let mut offset = pos;

        // Write element ID (0xA3)
        self.buffer[offset] = SIMPLE_BLOCK_ID as u8;
        offset += 1;

        // Write size
        let size_written = Self::encode_vint(content_size as u64, &mut self.buffer[offset..]);
        offset += size_written;

        // Write track number (VINT)
        let track_written = Self::encode_vint(track_number, &mut self.buffer[offset..]);
        offset += track_written;

        // Write timestamp (signed 16-bit, big-endian)
        let ts_bytes = timestamp.to_be_bytes();
        self.buffer[offset] = ts_bytes[0];
        self.buffer[offset + 1] = ts_bytes[1];
        offset += 2;

        // Write flags
        let flags: u8 = if keyframe { 0x80 } else { 0x00 };
        self.buffer[offset] = flags;
        offset += 1;

        // Write data
        self.buffer[offset..offset + data.len()].copy_from_slice(data);

        self.write_pos
            .store((pos + total) as u32, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        Ok(total)
    }

    /// Reserve space for later patching (returns position).
    ///
    /// # Parameters
    ///
    /// - `size`: Number of bytes to reserve
    ///
    /// # Returns
    ///
    /// Starting position of reserved space, or error
    pub fn reserve(&mut self, size: usize) -> Result<u32, EbmlError> {
        let pos = self.write_pos.load(Ordering::Acquire) as usize;
        if pos + size > Self::BUFFER_SIZE {
            return Err(EbmlError::BufferOverflow);
        }

        // Zero-fill reserved space
        for i in 0..size {
            self.buffer[pos + i] = 0;
        }

        self.write_pos.store((pos + size) as u32, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        Ok(pos as u32)
    }

    /// Patch bytes at a specific position.
    ///
    /// # Parameters
    ///
    /// - `position`: Position to patch
    /// - `data`: Data to write
    ///
    /// # Returns
    ///
    /// Number of bytes patched, or error
    ///
    /// # Safety
    ///
    /// Caller must ensure position + data.len() <= write_pos
    pub fn patch(&mut self, position: u32, data: &[u8]) -> Result<usize, EbmlError> {
        let pos = position as usize;
        let current_pos = self.write_pos.load(Ordering::Acquire) as usize;

        if pos + data.len() > current_pos {
            return Err(EbmlError::BufferOverflow);
        }

        self.buffer[pos..pos + data.len()].copy_from_slice(data);
        self.generation.fetch_add(1, Ordering::AcqRel);
        Ok(data.len())
    }

    /// Patch a 64-bit unsigned value at a specific position (big-endian).
    pub fn patch_u64(&mut self, position: u32, value: u64) -> Result<(), EbmlError> {
        let bytes = value.to_be_bytes();
        self.patch(position, &bytes)?;
        Ok(())
    }
}

impl Default for EbmlWriterCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for EbmlWriterCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EbmlWriterCapsule")
            .field("write_pos", &self.write_pos.load(Ordering::Relaxed))
            .field("stack_depth", &self.stack_depth.load(Ordering::Relaxed))
            .field("generation", &self.generation.load(Ordering::Relaxed))
            .finish()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: Unit Tests for Basic Operations
    // ========================================================================

    #[test]
    fn test_capsule_creation() {
        let writer = EbmlWriterCapsule::new();
        assert_eq!(writer.position(), 0);
        assert_eq!(writer.depth(), 0);
        assert_eq!(writer.generation(), 0);
    }

    #[test]
    fn test_capsule_reset() {
        let mut writer = EbmlWriterCapsule::new();
        let gen_before = writer.generation();
        writer.write_unsigned(0x4286, 1).unwrap();
        assert!(writer.position() > 0);

        writer.reset();
        assert_eq!(writer.position(), 0);
        assert_eq!(writer.depth(), 0);
        // Generation should have incremented (write + reset operations)
        assert!(writer.generation() > gen_before);
    }

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<EbmlWriterCapsule>(), 4352);
        assert_eq!(core::mem::align_of::<EbmlWriterCapsule>(), 256);
    }

    #[test]
    fn test_vint_size_1_byte() {
        assert_eq!(EbmlWriterCapsule::vint_size(0), 1);
        assert_eq!(EbmlWriterCapsule::vint_size(1), 1);
        assert_eq!(EbmlWriterCapsule::vint_size(126), 1);
    }

    #[test]
    fn test_vint_size_2_bytes() {
        assert_eq!(EbmlWriterCapsule::vint_size(127), 2);
        assert_eq!(EbmlWriterCapsule::vint_size(16382), 2);
    }

    #[test]
    fn test_vint_size_3_bytes() {
        assert_eq!(EbmlWriterCapsule::vint_size(16383), 3);
        assert_eq!(EbmlWriterCapsule::vint_size(0x1F_FFFE), 3);
    }

    #[test]
    fn test_vint_size_4_bytes() {
        assert_eq!(EbmlWriterCapsule::vint_size(0x1F_FFFF), 4);
        assert_eq!(EbmlWriterCapsule::vint_size(0x0FFF_FFFE), 4);
    }

    #[test]
    fn test_vint_encoding_1_byte() {
        let mut buf = [0u8; 8];
        let size = EbmlWriterCapsule::encode_vint(0, &mut buf);
        assert_eq!(size, 1);
        assert_eq!(buf[0], 0x80);

        let size = EbmlWriterCapsule::encode_vint(1, &mut buf);
        assert_eq!(size, 1);
        assert_eq!(buf[0], 0x81);

        let size = EbmlWriterCapsule::encode_vint(126, &mut buf);
        assert_eq!(size, 1);
        assert_eq!(buf[0], 0xFE);
    }

    #[test]
    fn test_vint_encoding_2_bytes() {
        let mut buf = [0u8; 8];
        let size = EbmlWriterCapsule::encode_vint(127, &mut buf);
        assert_eq!(size, 2);
        assert_eq!(buf[0], 0x40);
        assert_eq!(buf[1], 0x7F);
    }

    #[test]
    fn test_unknown_size_encoding() {
        let mut buf = [0u8; 8];
        let size = EbmlWriterCapsule::encode_unknown_size(&mut buf);
        assert_eq!(size, 8);
        assert_eq!(buf[0], 0x01);
        for i in 1..8 {
            assert_eq!(buf[i], 0xFF);
        }
    }

    #[test]
    fn test_element_id_size() {
        // 1-byte IDs: 0x80-0xFE
        assert_eq!(EbmlWriterCapsule::element_id_size(0x80), 1);
        assert_eq!(EbmlWriterCapsule::element_id_size(0xEC), 1); // Void
        assert_eq!(EbmlWriterCapsule::element_id_size(0xA3), 1); // SimpleBlock

        // 2-byte IDs: 0x4000-0x7FFE
        assert_eq!(EbmlWriterCapsule::element_id_size(0x4286), 2); // EBML Version
        assert_eq!(EbmlWriterCapsule::element_id_size(0x4DBB), 2); // Seek

        // 3-byte IDs: 0x200000-0x3FFFFE
        assert_eq!(EbmlWriterCapsule::element_id_size(0x2AD7B1), 3); // TimestampScale

        // 4-byte IDs
        assert_eq!(EbmlWriterCapsule::element_id_size(0x1A45_DFA3), 4); // EBML
        assert_eq!(EbmlWriterCapsule::element_id_size(0x1853_8067), 4); // Segment
    }

    #[test]
    fn test_write_element_id() {
        let mut writer = EbmlWriterCapsule::new();

        // 1-byte ID
        let size = writer.write_element_id(0xEC).unwrap();
        assert_eq!(size, 1);
        assert_eq!(writer.get_data()[0], 0xEC);

        // 4-byte ID
        writer.reset();
        let size = writer.write_element_id(EBML_ID).unwrap();
        assert_eq!(size, 4);
        assert_eq!(writer.get_data()[0], 0x1A);
        assert_eq!(writer.get_data()[1], 0x45);
        assert_eq!(writer.get_data()[2], 0xDF);
        assert_eq!(writer.get_data()[3], 0xA3);
    }

    #[test]
    fn test_write_unsigned_small() {
        let mut writer = EbmlWriterCapsule::new();
        let size = writer.write_unsigned(EBML_VERSION_ID, 1).unwrap();
        // ID (2) + size VINT (1) + value (1) = 4
        assert_eq!(size, 4);
        let data = writer.get_data();
        assert_eq!(data[0], 0x42); // ID high byte
        assert_eq!(data[1], 0x86); // ID low byte
        assert_eq!(data[2], 0x81); // Size = 1 (VINT)
        assert_eq!(data[3], 0x01); // Value = 1
    }

    #[test]
    fn test_write_unsigned_large() {
        let mut writer = EbmlWriterCapsule::new();
        let size = writer
            .write_unsigned(TIMESTAMP_SCALE_ID, 1_000_000)
            .unwrap();
        // ID (3) + size VINT (1) + value (3) = 7
        assert_eq!(size, 7);
    }

    #[test]
    fn test_write_signed_positive() {
        let mut writer = EbmlWriterCapsule::new();
        writer.write_signed(REFERENCE_BLOCK_ID, 100).unwrap();
        let data = writer.get_data();
        assert_eq!(data[0], 0xFB); // ID
    }

    #[test]
    fn test_write_signed_negative() {
        let mut writer = EbmlWriterCapsule::new();
        writer.write_signed(REFERENCE_BLOCK_ID, -100).unwrap();
        let data = writer.get_data();
        assert_eq!(data[0], 0xFB); // ID
    }

    #[test]
    fn test_write_string() {
        let mut writer = EbmlWriterCapsule::new();
        writer.write_string(DOC_TYPE_ID, "matroska").unwrap();
        let data = writer.get_data();
        assert_eq!(data[0], 0x42); // ID high byte
        assert_eq!(data[1], 0x82); // ID low byte
        assert_eq!(data[2], 0x88); // Size = 8 (VINT)
        assert_eq!(&data[3..11], b"matroska");
    }

    #[test]
    fn test_write_binary() {
        let mut writer = EbmlWriterCapsule::new();
        let test_data = [0x00, 0x01, 0x02, 0x03];
        writer.write_binary(CODEC_PRIVATE_ID, &test_data).unwrap();
        let data = writer.get_data();
        // ID (2) + size (1) + data (4)
        assert_eq!(data.len(), 7);
    }

    #[test]
    fn test_write_float32() {
        let mut writer = EbmlWriterCapsule::new();
        writer
            .write_float32(SAMPLING_FREQUENCY_ID, 48000.0)
            .unwrap();
        let data = writer.get_data();
        assert_eq!(data[0], 0xB5); // ID
    }

    #[test]
    fn test_write_float64() {
        let mut writer = EbmlWriterCapsule::new();
        writer.write_float64(DURATION_ID, 120.5).unwrap();
        let data = writer.get_data();
        // ID (2) + size (1) + float64 (8) = 11
        assert_eq!(data.len(), 11);
    }

    #[test]
    fn test_write_date() {
        let mut writer = EbmlWriterCapsule::new();
        // Some date after 2001-01-01
        let ns: i64 = 631_152_000_000_000_000; // ~20 years in nanoseconds
        writer.write_date(DATE_UTC_ID, ns).unwrap();
        let data = writer.get_data();
        // ID (2) + size (1) + date (8) = 11
        assert_eq!(data.len(), 11);
    }

    #[test]
    fn test_write_void() {
        let mut writer = EbmlWriterCapsule::new();
        let size = writer.write_void(10).unwrap();
        // ID (1) + size VINT (1) + void content (10) = 12
        assert_eq!(size, 12);
        let data = writer.get_data();
        assert_eq!(data[0], 0xEC); // Void ID
                                   // Content should be zeros
        for i in 2..12 {
            assert_eq!(data[i], 0);
        }
    }

    // ========================================================================
    // Q8-Q14: Property Tests for VINT Encoding
    // ========================================================================

    #[test]
    fn test_vint_roundtrip_all_sizes() {
        let test_values: [u64; 8] = [
            0,                     // 1 byte
            127,                   // 2 bytes
            16383,                 // 3 bytes
            0x1F_FFFF,             // 4 bytes
            0x07_FFFF_FFFF,        // 5 bytes
            0x03FF_FFFF_FFFF,      // 6 bytes
            0x01_FFFF_FFFF_FFFF,   // 7 bytes
            0x00FF_FFFF_FFFF_FFFF, // 8 bytes
        ];

        for &val in &test_values {
            let mut buf = [0u8; 8];
            let size = EbmlWriterCapsule::encode_vint(val, &mut buf);
            assert!(
                size >= 1 && size <= 8,
                "Size {} out of range for value {}",
                size,
                val
            );

            // Verify marker bit is set in first byte
            // EBML marker: 1-byte=0x80, 2-byte=0x40, 3-byte=0x20, etc.
            // For size N, marker bit is at position (8-N), i.e., mask = 1 << (8-size)
            let marker_bit = 1u8 << (8 - size);
            assert!(buf[0] & marker_bit != 0, "Marker bit not set for size {}", size);
        }
    }

    #[test]
    fn test_vint_fixed_encoding() {
        // Test that fixed-size encoding works correctly
        let mut buf = [0u8; 8];

        // Encode 0 in 8 bytes
        EbmlWriterCapsule::encode_vint_fixed(0, &mut buf, 8);
        assert_eq!(buf[0], 0x01); // 8-byte marker
        for i in 1..8 {
            assert_eq!(buf[i], 0x00);
        }

        // Encode 1 in 4 bytes
        EbmlWriterCapsule::encode_vint_fixed(1, &mut buf, 4);
        assert_eq!(buf[0], 0x10); // 4-byte marker
        assert_eq!(buf[1], 0x00);
        assert_eq!(buf[2], 0x00);
        assert_eq!(buf[3], 0x01);
    }

    #[test]
    fn test_vint_boundary_values() {
        // Test values at size boundaries
        let boundaries: [(u64, usize); 7] = [
            (0x7E, 1),
            (0x7F, 2),
            (0x3FFE, 2),
            (0x3FFF, 3),
            (0x1F_FFFE, 3),
            (0x1F_FFFF, 4),
            (0x0FFF_FFFE, 4),
        ];

        for (val, expected_size) in boundaries {
            let actual_size = EbmlWriterCapsule::vint_size(val);
            assert_eq!(
                actual_size, expected_size,
                "Value {} expected size {}, got {}",
                val, expected_size, actual_size
            );
        }
    }

    #[test]
    fn test_element_id_all_sizes() {
        // 1-byte IDs
        for id in [0x80, 0xA3, 0xEC, 0xFE] {
            assert_eq!(EbmlWriterCapsule::element_id_size(id), 1);
        }

        // 2-byte IDs
        for id in [0x4000, 0x4286, 0x7FFE] {
            assert_eq!(EbmlWriterCapsule::element_id_size(id), 2);
        }

        // 3-byte IDs
        for id in [0x20_0000, 0x2AD7_B1, 0x3F_FFFE] {
            assert_eq!(EbmlWriterCapsule::element_id_size(id), 3);
        }

        // 4-byte IDs
        for id in [EBML_ID, SEGMENT_ID, TRACKS_ID, CLUSTER_ID, CUES_ID] {
            assert_eq!(EbmlWriterCapsule::element_id_size(id), 4);
        }
    }

    #[test]
    fn test_unsigned_value_sizes() {
        let mut writer = EbmlWriterCapsule::new();

        // 1-byte value
        let size1 = writer.write_unsigned(0x4286, 0).unwrap();
        writer.reset();

        // 2-byte value
        let size2 = writer.write_unsigned(0x4286, 256).unwrap();
        writer.reset();

        // 8-byte value
        let size3 = writer.write_unsigned(0x4286, u64::MAX).unwrap();

        // Each should be larger due to value size
        assert!(size1 < size2);
        assert!(size2 < size3);
    }

    #[test]
    fn test_signed_encoding_symmetry() {
        let mut writer = EbmlWriterCapsule::new();

        // Positive and negative of same magnitude should have same size
        let size_pos = writer.write_signed(0xFB, 100).unwrap();
        writer.reset();
        let size_neg = writer.write_signed(0xFB, -100).unwrap();

        // Should be similar (within 1 byte due to sign extension)
        assert!((size_pos as i32 - size_neg as i32).abs() <= 1);
    }

    // ========================================================================
    // Q15-Q21: Integration Tests for Element Hierarchy
    // ========================================================================

    #[test]
    fn test_master_element_basic() {
        let mut writer = EbmlWriterCapsule::new();

        let content_start = writer.write_master_start(EBML_ID).unwrap();
        assert!(content_start > 0);
        assert_eq!(writer.depth(), 1);

        writer.write_unsigned(EBML_VERSION_ID, 1).unwrap();

        let id = writer.write_master_end().unwrap();
        assert_eq!(id, EBML_ID);
        assert_eq!(writer.depth(), 0);
    }

    #[test]
    fn test_nested_master_elements() {
        let mut writer = EbmlWriterCapsule::new();

        // Level 1
        writer.write_master_start(TRACKS_ID).unwrap();
        assert_eq!(writer.depth(), 1);

        // Level 2
        writer.write_master_start(TRACK_ENTRY_ID).unwrap();
        assert_eq!(writer.depth(), 2);

        // Level 3
        writer.write_master_start(VIDEO_ID).unwrap();
        assert_eq!(writer.depth(), 3);

        // Write some content
        writer.write_unsigned(PIXEL_WIDTH_ID, 1920).unwrap();
        writer.write_unsigned(PIXEL_HEIGHT_ID, 1080).unwrap();

        // Close all
        writer.write_master_end().unwrap(); // Video
        assert_eq!(writer.depth(), 2);

        writer.write_master_end().unwrap(); // TrackEntry
        assert_eq!(writer.depth(), 1);

        writer.write_master_end().unwrap(); // Tracks
        assert_eq!(writer.depth(), 0);
    }

    #[test]
    fn test_master_element_unknown_size() {
        let mut writer = EbmlWriterCapsule::new();

        let content_start = writer.write_master_start_unknown_size(SEGMENT_ID).unwrap();
        assert!(content_start > 0);
        assert_eq!(writer.depth(), 1);

        // Write content
        writer.write_master_start_unknown_size(CLUSTER_ID).unwrap();
        writer.write_unsigned(TIMESTAMP_ID, 0).unwrap();
        writer.write_master_end().unwrap(); // Cluster (unknown size)

        writer.write_master_end().unwrap(); // Segment (unknown size)
        assert_eq!(writer.depth(), 0);
    }

    #[test]
    fn test_ebml_header_complete() {
        let mut writer = EbmlWriterCapsule::new();
        let size = writer.write_ebml_header("matroska", 4, 2).unwrap();

        assert!(size > 0);
        assert_eq!(writer.depth(), 0);

        let data = writer.get_data();
        // Should start with EBML ID
        assert_eq!(data[0], 0x1A);
        assert_eq!(data[1], 0x45);
        assert_eq!(data[2], 0xDF);
        assert_eq!(data[3], 0xA3);
    }

    #[test]
    fn test_webm_header() {
        let mut writer = EbmlWriterCapsule::new();
        writer.write_ebml_header("webm", 4, 2).unwrap();

        let data = writer.get_data();
        // Verify "webm" is in the data
        let webm_found = data.windows(4).any(|w| w == b"webm");
        assert!(webm_found);
    }

    #[test]
    fn test_simple_block() {
        let mut writer = EbmlWriterCapsule::new();
        let frame_data = [0x00, 0x00, 0x00, 0x01]; // NAL unit

        let size = writer.write_simple_block(1, 0, true, &frame_data).unwrap();
        assert!(size > 0);

        let data = writer.get_data();
        // Should start with SimpleBlock ID
        assert_eq!(data[0], 0xA3);
    }

    #[test]
    fn test_simple_block_non_keyframe() {
        let mut writer = EbmlWriterCapsule::new();
        let frame_data = [0x00, 0x00, 0x00, 0x01];

        writer
            .write_simple_block(1, 100, false, &frame_data)
            .unwrap();

        let data = writer.get_data();
        assert_eq!(data[0], 0xA3);
    }

    #[test]
    fn test_track_structure() {
        let mut writer = EbmlWriterCapsule::new();

        writer.write_master_start(TRACKS_ID).unwrap();

        // Video track
        writer.write_master_start(TRACK_ENTRY_ID).unwrap();
        writer.write_unsigned(TRACK_NUMBER_ID, 1).unwrap();
        writer.write_unsigned(TRACK_UID_ID, 1234567890).unwrap();
        writer.write_unsigned(TRACK_TYPE_ID, 1).unwrap(); // Video
        writer.write_string(CODEC_ID_ID, "V_VP9").unwrap();

        writer.write_master_start(VIDEO_ID).unwrap();
        writer.write_unsigned(PIXEL_WIDTH_ID, 1920).unwrap();
        writer.write_unsigned(PIXEL_HEIGHT_ID, 1080).unwrap();
        writer.write_master_end().unwrap(); // Video

        writer.write_master_end().unwrap(); // TrackEntry

        // Audio track
        writer.write_master_start(TRACK_ENTRY_ID).unwrap();
        writer.write_unsigned(TRACK_NUMBER_ID, 2).unwrap();
        writer.write_unsigned(TRACK_UID_ID, 1234567891).unwrap();
        writer.write_unsigned(TRACK_TYPE_ID, 2).unwrap(); // Audio
        writer.write_string(CODEC_ID_ID, "A_OPUS").unwrap();

        writer.write_master_start(AUDIO_ID).unwrap();
        writer
            .write_float64(SAMPLING_FREQUENCY_ID, 48000.0)
            .unwrap();
        writer.write_unsigned(CHANNELS_ID, 2).unwrap();
        writer.write_master_end().unwrap(); // Audio

        writer.write_master_end().unwrap(); // TrackEntry

        writer.write_master_end().unwrap(); // Tracks

        assert_eq!(writer.depth(), 0);
    }

    #[test]
    fn test_cue_point_structure() {
        let mut writer = EbmlWriterCapsule::new();

        writer.write_master_start(CUES_ID).unwrap();

        writer.write_master_start(CUE_POINT_ID).unwrap();
        writer.write_unsigned(CUE_TIME_ID, 0).unwrap();

        writer.write_master_start(CUE_TRACK_POSITIONS_ID).unwrap();
        writer.write_unsigned(CUE_TRACK_ID, 1).unwrap();
        writer
            .write_unsigned(CUE_CLUSTER_POSITION_ID, 1234)
            .unwrap();
        writer.write_master_end().unwrap(); // CueTrackPositions

        writer.write_master_end().unwrap(); // CuePoint

        writer.write_master_end().unwrap(); // Cues

        assert_eq!(writer.depth(), 0);
    }

    #[test]
    fn test_chapter_structure() {
        let mut writer = EbmlWriterCapsule::new();

        writer.write_master_start(CHAPTERS_ID).unwrap();
        writer.write_master_start(EDITION_ENTRY_ID).unwrap();

        writer.write_master_start(CHAPTER_ATOM_ID).unwrap();
        writer.write_unsigned(CHAPTER_UID_ID, 1).unwrap();
        writer.write_unsigned(CHAPTER_TIME_START_ID, 0).unwrap();
        writer
            .write_unsigned(CHAPTER_TIME_END_ID, 60_000_000_000)
            .unwrap();

        writer.write_master_start(CHAPTER_DISPLAY_ID).unwrap();
        writer.write_string(CHAP_STRING_ID, "Chapter 1").unwrap();
        writer.write_string(CHAP_LANGUAGE_ID, "eng").unwrap();
        writer.write_master_end().unwrap(); // ChapterDisplay

        writer.write_master_end().unwrap(); // ChapterAtom
        writer.write_master_end().unwrap(); // EditionEntry
        writer.write_master_end().unwrap(); // Chapters

        assert_eq!(writer.depth(), 0);
    }

    #[test]
    fn test_tag_structure() {
        let mut writer = EbmlWriterCapsule::new();

        writer.write_master_start(TAGS_ID).unwrap();
        writer.write_master_start(TAG_ID).unwrap();

        writer.write_master_start(TARGETS_ID).unwrap();
        writer.write_unsigned(TAG_TRACK_UID_ID, 1234567890).unwrap();
        writer.write_master_end().unwrap(); // Targets

        writer.write_master_start(SIMPLE_TAG_ID).unwrap();
        writer.write_string(TAG_NAME_ID, "TITLE").unwrap();
        writer.write_string(TAG_STRING_ID, "Test Video").unwrap();
        writer.write_master_end().unwrap(); // SimpleTag

        writer.write_master_end().unwrap(); // Tag
        writer.write_master_end().unwrap(); // Tags

        assert_eq!(writer.depth(), 0);
    }

    // ========================================================================
    // Error Handling Tests
    // ========================================================================

    #[test]
    fn test_buffer_overflow() {
        let mut writer = EbmlWriterCapsule::new();

        // Try to write more than buffer size
        let large_data = [0u8; 5000];
        let result = writer.write_binary(0xA3, &large_data);
        assert_eq!(result, Err(EbmlError::BufferOverflow));
    }

    #[test]
    fn test_stack_overflow() {
        let mut writer = EbmlWriterCapsule::new();

        // Push 8 elements (max depth)
        for _ in 0..8 {
            writer.write_master_start(TRACK_ENTRY_ID).unwrap();
        }

        // 9th should fail
        let result = writer.write_master_start(TRACK_ENTRY_ID);
        assert_eq!(result, Err(EbmlError::StackOverflow));
    }

    #[test]
    fn test_stack_underflow() {
        let mut writer = EbmlWriterCapsule::new();

        // Try to close without opening
        let result = writer.write_master_end();
        assert_eq!(result, Err(EbmlError::StackUnderflow));
    }

    #[test]
    fn test_error_display() {
        let errors = [
            EbmlError::BufferOverflow,
            EbmlError::StackOverflow,
            EbmlError::StackUnderflow,
            EbmlError::InvalidElementId,
            EbmlError::InvalidVint,
            EbmlError::InvalidUtf8,
            EbmlError::SizeOverflow,
            EbmlError::ConcurrentModification,
        ];

        for err in errors {
            let msg = format!("{}", err);
            assert!(!msg.is_empty());
        }
    }

    // ========================================================================
    // Reserve and Patch Tests
    // ========================================================================

    #[test]
    fn test_reserve_and_patch() {
        let mut writer = EbmlWriterCapsule::new();

        // Write some data
        writer.write_unsigned(EBML_VERSION_ID, 1).unwrap();

        // Reserve space
        let reserved_pos = writer.reserve(8).unwrap();

        // Write more data
        writer.write_unsigned(EBML_READ_VERSION_ID, 1).unwrap();

        // Patch the reserved space
        let patch_data = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        writer.patch(reserved_pos, &patch_data).unwrap();

        // Verify patch
        let data = writer.get_data();
        let patch_start = reserved_pos as usize;
        assert_eq!(&data[patch_start..patch_start + 8], &patch_data);
    }

    #[test]
    fn test_patch_u64() {
        let mut writer = EbmlWriterCapsule::new();

        let pos = writer.reserve(8).unwrap();
        writer.patch_u64(pos, 0x0102030405060708).unwrap();

        let data = writer.get_data();
        assert_eq!(data[0], 0x01);
        assert_eq!(data[7], 0x08);
    }

    #[test]
    fn test_patch_out_of_bounds() {
        let mut writer = EbmlWriterCapsule::new();

        writer.write_unsigned(0x4286, 1).unwrap();

        // Try to patch beyond write position
        let result = writer.patch(100, &[0x00; 8]);
        assert_eq!(result, Err(EbmlError::BufferOverflow));
    }

    // ========================================================================
    // Generation Counter Tests
    // ========================================================================

    #[test]
    fn test_generation_increments() {
        let mut writer = EbmlWriterCapsule::new();

        assert_eq!(writer.generation(), 0);

        writer.write_unsigned(0x4286, 1).unwrap();
        assert_eq!(writer.generation(), 1);

        writer.write_string(DOC_TYPE_ID, "test").unwrap();
        assert_eq!(writer.generation(), 2);

        writer.reset();
        assert_eq!(writer.generation(), 3);
    }

    // ========================================================================
    // Codec-Specific Tests
    // ========================================================================

    #[test]
    fn test_vp9_codec_private() {
        let mut writer = EbmlWriterCapsule::new();

        // VP9 codec private data (minimal)
        let vp9_private = [0x01, 0x00, 0x00, 0x00];
        writer.write_binary(CODEC_PRIVATE_ID, &vp9_private).unwrap();

        assert!(writer.position() > 0);
    }

    #[test]
    fn test_opus_codec_private() {
        let mut writer = EbmlWriterCapsule::new();

        // Opus identification header
        let opus_head = b"OpusHead\x01\x02";
        writer.write_binary(CODEC_PRIVATE_ID, opus_head).unwrap();

        let data = writer.get_data();
        // Verify "OpusHead" is in the output
        let opus_found = data.windows(8).any(|w| w == b"OpusHead");
        assert!(opus_found);
    }

    #[test]
    fn test_av1_codec_string() {
        let mut writer = EbmlWriterCapsule::new();
        writer.write_string(CODEC_ID_ID, "V_AV1").unwrap();

        let data = writer.get_data();
        let av1_found = data.windows(5).any(|w| w == b"V_AV1");
        assert!(av1_found);
    }

    // ========================================================================
    // Full File Structure Tests
    // ========================================================================

    #[test]
    fn test_minimal_mkv_structure() {
        let mut writer = EbmlWriterCapsule::new();

        // EBML Header
        writer.write_ebml_header("matroska", 4, 2).unwrap();

        // Segment (unknown size for streaming)
        writer.write_master_start_unknown_size(SEGMENT_ID).unwrap();

        // Info
        writer.write_master_start(INFO_ID).unwrap();
        writer
            .write_unsigned(TIMESTAMP_SCALE_ID, 1_000_000)
            .unwrap();
        writer
            .write_string(MUXING_APP_ID, "atomic_capsule")
            .unwrap();
        writer.write_string(WRITING_APP_ID, "test").unwrap();
        writer.write_master_end().unwrap(); // Info

        // Tracks
        writer.write_master_start(TRACKS_ID).unwrap();
        writer.write_master_start(TRACK_ENTRY_ID).unwrap();
        writer.write_unsigned(TRACK_NUMBER_ID, 1).unwrap();
        writer.write_unsigned(TRACK_UID_ID, 1).unwrap();
        writer.write_unsigned(TRACK_TYPE_ID, 1).unwrap();
        writer.write_string(CODEC_ID_ID, "V_VP9").unwrap();
        writer.write_master_end().unwrap(); // TrackEntry
        writer.write_master_end().unwrap(); // Tracks

        writer.write_master_end().unwrap(); // Segment

        assert_eq!(writer.depth(), 0);
        assert!(writer.position() > 0);
    }

    #[test]
    fn test_webm_minimal_structure() {
        let mut writer = EbmlWriterCapsule::new();

        writer.write_ebml_header("webm", 4, 2).unwrap();

        writer.write_master_start_unknown_size(SEGMENT_ID).unwrap();

        writer.write_master_start(INFO_ID).unwrap();
        writer
            .write_unsigned(TIMESTAMP_SCALE_ID, 1_000_000)
            .unwrap();
        writer.write_master_end().unwrap();

        writer.write_master_start(TRACKS_ID).unwrap();
        writer.write_master_start(TRACK_ENTRY_ID).unwrap();
        writer.write_unsigned(TRACK_NUMBER_ID, 1).unwrap();
        writer.write_unsigned(TRACK_UID_ID, 1).unwrap();
        writer.write_unsigned(TRACK_TYPE_ID, 1).unwrap();
        writer.write_string(CODEC_ID_ID, "V_VP9").unwrap();
        writer.write_master_end().unwrap();
        writer.write_master_end().unwrap();

        writer.write_master_end().unwrap();

        // Verify webm is in output
        let data = writer.get_data();
        let webm_found = data.windows(4).any(|w| w == b"webm");
        assert!(webm_found);
    }

    // ========================================================================
    // Additional Tests to reach 40+
    // ========================================================================

    #[test]
    fn test_default_trait() {
        let writer = EbmlWriterCapsule::default();
        assert_eq!(writer.position(), 0);
    }

    #[test]
    fn test_debug_trait() {
        let writer = EbmlWriterCapsule::new();
        let debug_str = format!("{:?}", writer);
        assert!(debug_str.contains("EbmlWriterCapsule"));
        assert!(debug_str.contains("write_pos"));
    }

    #[test]
    fn test_stack_entry_is_empty() {
        let empty = ElementStackEntry::new(0, 0, 0);
        assert!(empty.is_empty());

        let non_empty = ElementStackEntry::new(EBML_ID, 10, 20);
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_get_buffer_mut() {
        let mut writer = EbmlWriterCapsule::new();
        let buf = writer.get_buffer_mut();
        buf[0] = 0xFF;
        assert_eq!(writer.get_data().get(0), None); // Position is still 0
    }

    #[test]
    fn test_multiple_void_elements() {
        let mut writer = EbmlWriterCapsule::new();

        writer.write_void(10).unwrap();
        writer.write_void(20).unwrap();
        writer.write_void(30).unwrap();

        // Each void has ID(1) + size VINT + content
        assert!(writer.position() > 60);
    }

    #[test]
    fn test_seek_head_structure() {
        let mut writer = EbmlWriterCapsule::new();

        writer.write_master_start(SEEK_HEAD_ID).unwrap();

        // Seek entry for Info
        writer.write_master_start(SEEK_ID).unwrap();
        let info_id_bytes = INFO_ID.to_be_bytes();
        writer
            .write_binary(SEEK_ID_ID, &info_id_bytes[1..])
            .unwrap(); // 3-byte ID
        writer.write_unsigned(SEEK_POSITION_ID, 100).unwrap();
        writer.write_master_end().unwrap();

        // Seek entry for Tracks
        writer.write_master_start(SEEK_ID).unwrap();
        let tracks_id_bytes = TRACKS_ID.to_be_bytes();
        writer.write_binary(SEEK_ID_ID, &tracks_id_bytes).unwrap();
        writer.write_unsigned(SEEK_POSITION_ID, 200).unwrap();
        writer.write_master_end().unwrap();

        writer.write_master_end().unwrap(); // SeekHead

        assert_eq!(writer.depth(), 0);
    }

    #[test]
    fn test_segment_uid() {
        let mut writer = EbmlWriterCapsule::new();

        // SegmentUID is 16 bytes (128-bit UUID)
        let segment_uid: [u8; 16] = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10,
        ];
        writer.write_binary(SEGMENT_UID_ID, &segment_uid).unwrap();

        assert!(writer.position() > 16);
    }

    #[test]
    fn test_cluster_with_blocks() {
        let mut writer = EbmlWriterCapsule::new();

        writer.write_master_start(CLUSTER_ID).unwrap();
        writer.write_unsigned(TIMESTAMP_ID, 0).unwrap();

        // SimpleBlocks
        writer
            .write_simple_block(1, 0, true, &[0x00, 0x01])
            .unwrap();
        writer
            .write_simple_block(1, 33, false, &[0x00, 0x02])
            .unwrap();
        writer
            .write_simple_block(1, 66, false, &[0x00, 0x03])
            .unwrap();

        writer.write_master_end().unwrap();

        assert_eq!(writer.depth(), 0);
    }

    #[test]
    fn test_block_group() {
        let mut writer = EbmlWriterCapsule::new();

        writer.write_master_start(BLOCK_GROUP_ID).unwrap();
        writer
            .write_binary(BLOCK_ID, &[0x81, 0x00, 0x00, 0x00, 0x01])
            .unwrap();
        writer.write_unsigned(BLOCK_DURATION_ID, 33).unwrap();
        writer.write_signed(REFERENCE_BLOCK_ID, -33).unwrap();
        writer.write_master_end().unwrap();

        assert_eq!(writer.depth(), 0);
    }

    #[test]
    fn test_display_dimensions() {
        let mut writer = EbmlWriterCapsule::new();

        writer.write_master_start(VIDEO_ID).unwrap();
        writer.write_unsigned(PIXEL_WIDTH_ID, 3840).unwrap();
        writer.write_unsigned(PIXEL_HEIGHT_ID, 2160).unwrap();
        writer.write_unsigned(DISPLAY_WIDTH_ID, 1920).unwrap();
        writer.write_unsigned(DISPLAY_HEIGHT_ID, 1080).unwrap();
        writer.write_master_end().unwrap();

        assert_eq!(writer.depth(), 0);
    }

    #[test]
    fn test_interlaced_flag() {
        let mut writer = EbmlWriterCapsule::new();

        writer.write_master_start(VIDEO_ID).unwrap();
        writer.write_unsigned(FLAG_INTERLACED_ID, 2).unwrap(); // Progressive
        writer.write_master_end().unwrap();

        assert_eq!(writer.depth(), 0);
    }

    #[test]
    fn test_audio_bit_depth() {
        let mut writer = EbmlWriterCapsule::new();

        writer.write_master_start(AUDIO_ID).unwrap();
        writer
            .write_float64(SAMPLING_FREQUENCY_ID, 44100.0)
            .unwrap();
        writer.write_unsigned(CHANNELS_ID, 2).unwrap();
        writer.write_unsigned(BIT_DEPTH_ID, 16).unwrap();
        writer.write_master_end().unwrap();

        assert_eq!(writer.depth(), 0);
    }

    #[test]
    fn test_crc32_element() {
        let mut writer = EbmlWriterCapsule::new();

        // CRC-32 is 4 bytes
        let crc = 0xDEADBEEFu32.to_be_bytes();
        writer.write_binary(CRC32_ID, &crc).unwrap();

        let data = writer.get_data();
        assert_eq!(data[0], 0xBF); // CRC32 ID
    }

    #[test]
    fn test_language_code() {
        let mut writer = EbmlWriterCapsule::new();

        writer.write_string(LANGUAGE_ID, "eng").unwrap();

        let data = writer.get_data();
        let eng_found = data.windows(3).any(|w| w == b"eng");
        assert!(eng_found);
    }

    #[test]
    fn test_track_name() {
        let mut writer = EbmlWriterCapsule::new();

        writer.write_string(NAME_ID, "Main Video Track").unwrap();

        let data = writer.get_data();
        let name_found = data.windows(5).any(|w| w == b"Main ");
        assert!(name_found);
    }

    #[test]
    fn test_zero_length_binary() {
        let mut writer = EbmlWriterCapsule::new();

        // Empty binary element
        let result = writer.write_binary(CODEC_PRIVATE_ID, &[]);
        assert!(result.is_ok());

        let data = writer.get_data();
        // Should have ID + size (0)
        assert!(data.len() >= 3);
    }

    #[test]
    fn test_max_nesting_depth() {
        let mut writer = EbmlWriterCapsule::new();

        // Push to max depth
        for i in 0..8 {
            writer.write_master_start(TRACK_ENTRY_ID).unwrap();
            assert_eq!(writer.depth(), (i + 1) as u8);
        }

        // Pop all
        for i in (0..8).rev() {
            writer.write_master_end().unwrap();
            assert_eq!(writer.depth(), i as u8);
        }
    }
}
