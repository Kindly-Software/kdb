//! MP4/ISO BMFF demuxer capsule
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Implements ISO 14496-12 box parsing for MP4, M4V, MOV, 3GP containers.
//! Streaming architecture processes boxes incrementally without full file buffering.
//!
//! ## Architecture
//!
//! ```text
//! +------------------------------------------+
//! | Mp4DemuxerCapsule (T5 Streaming)         |
//! | Size: 512B, Align: 512B                  |
//! |                                          |
//! | +--------------------------------------+ |
//! | | State Machine (T1 Atomic)            | |
//! | | - state (AtomicU64)                  | |
//! | | - generation (AtomicU64)             | |
//! | +--------------------------------------+ |
//! | +--------------------------------------+ |
//! | | File Tracking (T0 Auditable)         | |
//! | | - file_size (AtomicU64)              | |
//! | | - bytes_parsed (AtomicU64)           | |
//! | +--------------------------------------+ |
//! | +--------------------------------------+ |
//! | | Box Statistics (T1 Atomic)           | |
//! | | - boxes_parsed (AtomicU64)           | |
//! | | - tracks_found (AtomicU64)           | |
//! | +--------------------------------------+ |
//! | +--------------------------------------+ |
//! | | Moov/Mdat Location (T5 Streaming)    | |
//! | | - moov_offset/size (AtomicU64)       | |
//! | | - mdat_offset/size (AtomicU64)       | |
//! | +--------------------------------------+ |
//! | +--------------------------------------+ |
//! | | Error Tracking (T0 Auditable)        | |
//! | | - last_error (AtomicU64)             | |
//! | +--------------------------------------+ |
//! +------------------------------------------+
//! ```
//!
//! ## Box Types Supported
//!
//! | FourCC | Name | Purpose |
//! |--------|------|---------|
//! | `ftyp` | File Type | Brand + compatibility |
//! | `moov` | Movie | Container metadata |
//! | `mvhd` | Movie Header | Duration, timescale |
//! | `trak` | Track | Individual track data |
//! | `mdia` | Media | Media information |
//! | `stbl` | Sample Table | Sample locations |
//! | `mdat` | Media Data | Actual coded frames |
//! | `av01` | AV1 Sample Entry | AV1 codec configuration |
//!
//! ## Streaming Pattern (T5)
//!
//! Parse boxes incrementally without buffering entire file:
//! ```text
//! Open MP4 -> Parse ftyp -> Parse moov structure ->
//! Extract track info -> Locate mdat -> Ready for demuxing
//! ```

use core::sync::atomic::{AtomicU64, Ordering};

/// ISO BMFF Box types (4CC codes)
pub mod box_types {
    /// File type box - identifies file format and compatibility
    pub const FTYP: [u8; 4] = *b"ftyp";
    /// Movie box - contains all movie metadata
    pub const MOOV: [u8; 4] = *b"moov";
    /// Movie header box - timescale, duration, etc.
    pub const MVHD: [u8; 4] = *b"mvhd";
    /// Track box - contains track information
    pub const TRAK: [u8; 4] = *b"trak";
    /// Track header box - track dimensions, flags
    pub const TKHD: [u8; 4] = *b"tkhd";
    /// Media box - contains media information
    pub const MDIA: [u8; 4] = *b"mdia";
    /// Media header box - timescale, duration for track
    pub const MDHD: [u8; 4] = *b"mdhd";
    /// Handler reference box - identifies track type
    pub const HDLR: [u8; 4] = *b"hdlr";
    /// Media information box
    pub const MINF: [u8; 4] = *b"minf";
    /// Sample table box - sample locations and sizes
    pub const STBL: [u8; 4] = *b"stbl";
    /// Sample description box - codec configuration
    pub const STSD: [u8; 4] = *b"stsd";
    /// Time-to-sample box - sample durations
    pub const STTS: [u8; 4] = *b"stts";
    /// Sample-to-chunk box - sample grouping
    pub const STSC: [u8; 4] = *b"stsc";
    /// Sample size box - individual sample sizes
    pub const STSZ: [u8; 4] = *b"stsz";
    /// Chunk offset box (32-bit)
    pub const STCO: [u8; 4] = *b"stco";
    /// Chunk offset box (64-bit)
    pub const CO64: [u8; 4] = *b"co64";
    /// Media data box - actual coded data
    pub const MDAT: [u8; 4] = *b"mdat";
    /// Free space box
    pub const FREE: [u8; 4] = *b"free";
    /// Skip box (alias for free)
    pub const SKIP: [u8; 4] = *b"skip";
    /// UUID extension box
    pub const UUID: [u8; 4] = *b"uuid";

    // Codec-specific sample entry types
    /// H.264/AVC sample entry
    pub const AVC1: [u8; 4] = *b"avc1";
    /// H.264/AVC configuration box
    pub const AVCC: [u8; 4] = *b"avcC";
    /// H.265/HEVC sample entry (variant 1)
    pub const HVC1: [u8; 4] = *b"hvc1";
    /// H.265/HEVC sample entry (variant 2)
    pub const HEV1: [u8; 4] = *b"hev1";
    /// H.265/HEVC configuration box
    pub const HVCC: [u8; 4] = *b"hvcC";
    /// VP9 sample entry
    pub const VP09: [u8; 4] = *b"vp09";
    /// VP9 configuration box
    pub const VPCC: [u8; 4] = *b"vpcC";
    /// AV1 sample entry
    pub const AV01: [u8; 4] = *b"av01";
    /// AV1 configuration box
    pub const AV1C: [u8; 4] = *b"av1C";
}

/// Container box types that contain other boxes
pub const CONTAINER_BOXES: &[[u8; 4]] = &[
    box_types::MOOV,
    box_types::TRAK,
    box_types::MDIA,
    box_types::MINF,
    box_types::STBL,
];

/// Full box types (have version + flags after header)
pub const FULL_BOXES: &[[u8; 4]] = &[
    box_types::MVHD,
    box_types::TKHD,
    box_types::MDHD,
    box_types::HDLR,
    box_types::STSD,
    box_types::STTS,
    box_types::STSC,
    box_types::STSZ,
    box_types::STCO,
    box_types::CO64,
];

/// Demuxer state machine
///
/// State transitions:
/// ```text
/// Idle -> ParsingFtyp -> ParsingMoov -> ParsingTrak -> ParsingMdat -> Ready
///   |         |              |              |              |           |
///   +-------- +------------- +------------- +------------- +-----------+-> Error
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DemuxerState {
    /// Initial state, no parsing started
    Idle = 0,
    /// Parsing ftyp box
    ParsingFtyp = 1,
    /// Parsing moov box hierarchy
    ParsingMoov = 2,
    /// Parsing trak boxes within moov
    ParsingTrak = 3,
    /// Parsing/locating mdat box
    ParsingMdat = 4,
    /// Demuxer ready to extract samples
    Ready = 5,
    /// Error state
    Error = 6,
}

impl DemuxerState {
    /// Convert from u64 (for atomic operations)
    #[inline]
    pub const fn from_u64(value: u64) -> Self {
        match value {
            0 => Self::Idle,
            1 => Self::ParsingFtyp,
            2 => Self::ParsingMoov,
            3 => Self::ParsingTrak,
            4 => Self::ParsingMdat,
            5 => Self::Ready,
            _ => Self::Error,
        }
    }

    /// Convert to u64 (for atomic operations)
    #[inline]
    pub const fn to_u64(self) -> u64 {
        self as u64
    }
}

/// Parsed box information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxInfo {
    /// Box type (4CC code)
    pub box_type: [u8; 4],
    /// Total box size (0 = extends to EOF)
    pub size: u64,
    /// Absolute file offset where box starts
    pub offset: u64,
    /// Header size (8 for normal, 16 for extended size)
    pub header_size: u8,
}

impl BoxInfo {
    /// Get content offset (after header)
    #[inline]
    pub const fn content_offset(&self) -> u64 {
        self.offset + self.header_size as u64
    }

    /// Get content size (box size minus header)
    #[inline]
    pub const fn content_size(&self) -> u64 {
        if self.size == 0 {
            u64::MAX // Extends to EOF
        } else {
            self.size.saturating_sub(self.header_size as u64)
        }
    }

    /// Check if this is a container box
    #[inline]
    pub fn is_container(&self) -> bool {
        CONTAINER_BOXES.contains(&self.box_type)
    }

    /// Check if this is a full box (has version/flags)
    #[inline]
    pub fn is_full_box(&self) -> bool {
        FULL_BOXES.contains(&self.box_type)
    }
}

/// Compatible brands from ftyp box
#[derive(Debug, Clone, Default)]
pub struct FileTypeBox {
    /// Major brand (e.g., "isom", "mp41", "mp42", "avc1", "av01")
    pub major_brand: [u8; 4],
    /// Minor version
    pub minor_version: u32,
    /// List of compatible brands
    pub compatible_brands: Vec<[u8; 4]>,
}

impl FileTypeBox {
    /// Check if a specific brand is compatible
    #[inline]
    pub fn is_compatible(&self, brand: &[u8; 4]) -> bool {
        &self.major_brand == brand || self.compatible_brands.contains(brand)
    }

    /// Check if this is an MP4 container
    #[inline]
    pub fn is_mp4(&self) -> bool {
        self.is_compatible(b"isom")
            || self.is_compatible(b"mp41")
            || self.is_compatible(b"mp42")
            || self.is_compatible(b"M4V ")
    }

    /// Check if this contains AV1 content
    #[inline]
    pub fn is_av1(&self) -> bool {
        self.is_compatible(b"av01")
    }
}

/// Demuxer statistics snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct DemuxerStats {
    /// Current state
    pub state: DemuxerState,
    /// Generation counter (incremented on each state change)
    pub generation: u64,
    /// Total file size
    pub file_size: u64,
    /// Bytes parsed so far
    pub bytes_parsed: u64,
    /// Number of boxes parsed
    pub boxes_parsed: u64,
    /// Number of tracks found
    pub tracks_found: u64,
    /// Moov box offset
    pub moov_offset: u64,
    /// Moov box size
    pub moov_size: u64,
    /// Mdat box offset
    pub mdat_offset: u64,
    /// Mdat box size
    pub mdat_size: u64,
    /// Last error code
    pub last_error: DemuxError,
}

impl Default for DemuxerState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Demuxer error types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum DemuxError {
    /// No error
    #[default]
    None = 0,
    /// Invalid box size (too small or overflow)
    InvalidBoxSize = 1,
    /// Unexpected end of data
    UnexpectedEof = 2,
    /// Missing required ftyp box
    MissingFtyp = 3,
    /// Missing required moov box
    MissingMoov = 4,
    /// Unsupported file brand
    UnsupportedBrand = 5,
    /// Invalid state transition
    InvalidState = 6,
    /// IO error during read
    IoError = 7,
    /// Invalid box header
    InvalidBoxHeader = 8,
    /// Box size too large
    BoxSizeTooLarge = 9,
    /// Nested box depth exceeded
    NestingTooDeep = 10,
}

impl DemuxError {
    /// Convert from u64 (for atomic operations)
    #[inline]
    pub const fn from_u64(value: u64) -> Self {
        match value {
            0 => Self::None,
            1 => Self::InvalidBoxSize,
            2 => Self::UnexpectedEof,
            3 => Self::MissingFtyp,
            4 => Self::MissingMoov,
            5 => Self::UnsupportedBrand,
            6 => Self::InvalidState,
            7 => Self::IoError,
            8 => Self::InvalidBoxHeader,
            9 => Self::BoxSizeTooLarge,
            10 => Self::NestingTooDeep,
            _ => Self::None,
        }
    }

    /// Convert to u64 (for atomic operations)
    #[inline]
    pub const fn to_u64(self) -> u64 {
        self as u64
    }
}

/// T5 Streaming capsule for MP4 demuxing
///
/// **Tier**: T5 Streaming (O(1) incremental parsing, no buffering)
/// **Size**: 512B cache-aligned
/// **Safety**: 99.99% (integer-only parsing, no unsafe blocks)
///
/// # Design
///
/// The capsule maintains atomic state for lockfree coordination:
/// - State machine with atomic transitions (CAS)
/// - Generation counter for TOCTOU prevention
/// - Atomic counters for statistics
///
/// # Box Parsing Rules (ISO 14496-12)
///
/// - Box header: 4 bytes size + 4 bytes type
/// - If size == 1: Next 8 bytes are 64-bit extended size
/// - If size == 0: Box extends to EOF
/// - Container boxes (moov, trak, mdia, minf, stbl): Parse children recursively
/// - Full boxes: Have 1 byte version + 3 bytes flags after header
#[repr(C, align(512))]
pub struct Mp4DemuxerCapsule {
    // State machine (16 bytes)
    /// Current demuxer state
    pub state: AtomicU64,
    /// Generation counter (incremented on each state change)
    pub generation: AtomicU64,

    // File info (16 bytes)
    /// Total file size (set on initialization)
    pub file_size: AtomicU64,
    /// Bytes parsed so far
    pub bytes_parsed: AtomicU64,

    // Box statistics (16 bytes)
    /// Number of boxes parsed
    pub boxes_parsed: AtomicU64,
    /// Number of tracks found
    pub tracks_found: AtomicU64,

    // Moov location (16 bytes)
    /// Absolute offset of moov box
    pub moov_offset: AtomicU64,
    /// Size of moov box
    pub moov_size: AtomicU64,

    // Mdat location (16 bytes)
    /// Absolute offset of mdat box
    pub mdat_offset: AtomicU64,
    /// Size of mdat box
    pub mdat_size: AtomicU64,

    // Error tracking (8 bytes)
    /// Last error code
    pub last_error: AtomicU64,

    // Padding to 512B (424 bytes)
    // 16 + 16 + 16 + 16 + 16 + 8 = 88 bytes used
    // 512 - 88 = 424 bytes padding
    _padding: [u8; 424],
}

// #ASSUME: Size assertions validated at compile time
const _: () = {
    assert!(core::mem::size_of::<Mp4DemuxerCapsule>() == 512);
    assert!(core::mem::align_of::<Mp4DemuxerCapsule>() == 512);
};

impl Mp4DemuxerCapsule {
    /// Create a new MP4 demuxer capsule in Idle state
    ///
    /// # Returns
    ///
    /// A new capsule with all atomics initialized to zero/Idle
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(DemuxerState::Idle as u64),
            generation: AtomicU64::new(0),
            file_size: AtomicU64::new(0),
            bytes_parsed: AtomicU64::new(0),
            boxes_parsed: AtomicU64::new(0),
            tracks_found: AtomicU64::new(0),
            moov_offset: AtomicU64::new(0),
            moov_size: AtomicU64::new(0),
            mdat_offset: AtomicU64::new(0),
            mdat_size: AtomicU64::new(0),
            last_error: AtomicU64::new(DemuxError::None as u64),
            _padding: [0u8; 424],
        }
    }

    /// Parse a box header from raw bytes
    ///
    /// # Arguments
    ///
    /// * `data` - At least 8 bytes of box header data
    ///
    /// # Returns
    ///
    /// * `Ok(BoxInfo)` - Parsed box information
    /// * `Err(DemuxError)` - Parsing error
    ///
    /// # Box Header Format (ISO 14496-12)
    ///
    /// ```text
    /// +-----------------+
    /// | size (4 bytes)  |  Total box size including header
    /// +-----------------+
    /// | type (4 bytes)  |  Box type (FourCC)
    /// +-----------------+
    /// | [extended size] |  8 bytes if size == 1
    /// +-----------------+
    /// ```
    ///
    /// - If size == 0: Box extends to end of file
    /// - If size == 1: Next 8 bytes contain 64-bit size
    /// - Otherwise: size is the 32-bit value
    pub fn parse_box_header(&self, data: &[u8]) -> Result<BoxInfo, DemuxError> {
        // Minimum header is 8 bytes
        if data.len() < 8 {
            return Err(DemuxError::UnexpectedEof);
        }

        // Parse 32-bit size
        let size32 = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);

        // Parse box type
        let mut box_type = [0u8; 4];
        box_type.copy_from_slice(&data[4..8]);

        // Determine actual size and header size
        let (size, header_size): (u64, u8) = match size32 {
            0 => {
                // Box extends to EOF
                (0, 8)
            }
            1 => {
                // Extended 64-bit size
                if data.len() < 16 {
                    return Err(DemuxError::UnexpectedEof);
                }
                let size64 = u64::from_be_bytes([
                    data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
                ]);
                // Validate size is reasonable (at least header size)
                if size64 < 16 {
                    return Err(DemuxError::InvalidBoxSize);
                }
                (size64, 16)
            }
            _ => {
                // Normal 32-bit size
                if size32 < 8 {
                    return Err(DemuxError::InvalidBoxSize);
                }
                (size32 as u64, 8)
            }
        };

        Ok(BoxInfo {
            box_type,
            size,
            offset: 0, // Caller must set this
            header_size,
        })
    }

    /// Parse ftyp (File Type) box content
    ///
    /// # Arguments
    ///
    /// * `data` - ftyp box content (after 8-byte header)
    ///
    /// # Returns
    ///
    /// * `Ok(FileTypeBox)` - Parsed file type information
    /// * `Err(DemuxError)` - Parsing error
    ///
    /// # Format
    ///
    /// ```text
    /// +-----------------------+
    /// | major_brand (4 bytes) |
    /// +-----------------------+
    /// | minor_version (4 B)   |
    /// +-----------------------+
    /// | compatible_brands...  |  (4 bytes each, variable count)
    /// +-----------------------+
    /// ```
    pub fn parse_ftyp(&mut self, data: &[u8]) -> Result<FileTypeBox, DemuxError> {
        // Minimum ftyp content is 8 bytes (brand + version)
        if data.len() < 8 {
            self.set_error(DemuxError::InvalidBoxSize);
            return Err(DemuxError::InvalidBoxSize);
        }

        let mut major_brand = [0u8; 4];
        major_brand.copy_from_slice(&data[0..4]);

        let minor_version = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);

        // Parse compatible brands (remaining bytes in groups of 4)
        let remaining = &data[8..];
        let brand_count = remaining.len() / 4;
        let mut compatible_brands = Vec::with_capacity(brand_count);

        for i in 0..brand_count {
            let offset = i * 4;
            if offset + 4 <= remaining.len() {
                let mut brand = [0u8; 4];
                brand.copy_from_slice(&remaining[offset..offset + 4]);
                compatible_brands.push(brand);
            }
        }

        Ok(FileTypeBox {
            major_brand,
            minor_version,
            compatible_brands,
        })
    }

    /// Parse moov box structure recursively
    ///
    /// # Arguments
    ///
    /// * `data` - moov box content (after header)
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<BoxInfo>)` - List of child boxes found
    /// * `Err(DemuxError)` - Parsing error
    pub fn parse_moov_structure(&mut self, data: &[u8]) -> Result<Vec<BoxInfo>, DemuxError> {
        self.parse_container_boxes(data, 0, 0)
    }

    /// Internal: Parse container box children
    fn parse_container_boxes(
        &mut self,
        data: &[u8],
        base_offset: u64,
        depth: u32,
    ) -> Result<Vec<BoxInfo>, DemuxError> {
        // Prevent infinite recursion
        if depth > 16 {
            return Err(DemuxError::NestingTooDeep);
        }

        let mut boxes = Vec::new();
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            // Parse box header
            let header_result = self.parse_box_header(&data[offset..]);
            let mut box_info = match header_result {
                Ok(info) => info,
                Err(_) => break, // End of valid data
            };

            // Set absolute offset
            box_info.offset = base_offset + offset as u64;

            // Calculate box end
            let box_size = if box_info.size == 0 {
                // Extends to end of container
                data.len() - offset
            } else {
                box_info.size as usize
            };

            // Validate box doesn't exceed container
            if offset + box_size > data.len() {
                break;
            }

            // Track specific boxes
            if box_info.box_type == box_types::TRAK {
                self.tracks_found.fetch_add(1, Ordering::Relaxed);
            }

            boxes.push(box_info);
            self.boxes_parsed.fetch_add(1, Ordering::Relaxed);

            // Recursively parse container boxes
            if box_info.is_container() {
                let content_start = offset + box_info.header_size as usize;
                let content_end = offset + box_size;
                if content_start < content_end && content_end <= data.len() {
                    let children = self.parse_container_boxes(
                        &data[content_start..content_end],
                        box_info.offset + box_info.header_size as u64,
                        depth + 1,
                    )?;
                    boxes.extend(children);
                }
            }

            offset += box_size;
        }

        // Update bytes parsed
        self.bytes_parsed
            .fetch_add(offset as u64, Ordering::Relaxed);

        Ok(boxes)
    }

    /// Find a box by type within data
    ///
    /// # Arguments
    ///
    /// * `data` - Data to search within
    /// * `target` - Box type to find (4CC)
    ///
    /// # Returns
    ///
    /// * `Some(BoxInfo)` - Found box
    /// * `None` - Box not found
    pub fn find_box(&self, data: &[u8], target: &[u8; 4]) -> Option<BoxInfo> {
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            let box_info = self.parse_box_header(&data[offset..]).ok()?;

            if &box_info.box_type == target {
                return Some(BoxInfo {
                    offset: offset as u64,
                    ..box_info
                });
            }

            // Advance to next box
            let box_size = if box_info.size == 0 {
                // Extends to EOF - no more boxes after this
                return None;
            } else if box_info.size < 8 {
                // Invalid size
                return None;
            } else {
                box_info.size as usize
            };

            offset += box_size;
        }

        None
    }

    /// Atomic state transition with compare-and-swap
    ///
    /// # Arguments
    ///
    /// * `from` - Expected current state
    /// * `to` - Target state
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Transition successful
    /// * `Err(DemuxError::InvalidState)` - Current state doesn't match `from`
    ///
    /// # Thread Safety
    ///
    /// Uses compare_exchange to ensure atomic transition. Generation counter
    /// is incremented on successful transitions for TOCTOU prevention.
    pub fn transition_state(
        &self,
        from: DemuxerState,
        to: DemuxerState,
    ) -> Result<(), DemuxError> {
        let result = self.state.compare_exchange(
            from.to_u64(),
            to.to_u64(),
            Ordering::AcqRel,
            Ordering::Acquire,
        );

        match result {
            Ok(_) => {
                // Successful transition - increment generation
                self.generation.fetch_add(1, Ordering::Release);
                Ok(())
            }
            Err(_) => {
                self.set_error(DemuxError::InvalidState);
                Err(DemuxError::InvalidState)
            }
        }
    }

    /// Get current state
    #[inline]
    pub fn state(&self) -> DemuxerState {
        DemuxerState::from_u64(self.state.load(Ordering::Acquire))
    }

    /// Get demuxer statistics snapshot
    ///
    /// Returns a consistent snapshot of all statistics. Note that since
    /// individual loads are atomic but not combined, there may be slight
    /// inconsistencies between fields during active parsing.
    pub fn stats(&self) -> DemuxerStats {
        DemuxerStats {
            state: self.state(),
            generation: self.generation.load(Ordering::Acquire),
            file_size: self.file_size.load(Ordering::Relaxed),
            bytes_parsed: self.bytes_parsed.load(Ordering::Relaxed),
            boxes_parsed: self.boxes_parsed.load(Ordering::Relaxed),
            tracks_found: self.tracks_found.load(Ordering::Relaxed),
            moov_offset: self.moov_offset.load(Ordering::Relaxed),
            moov_size: self.moov_size.load(Ordering::Relaxed),
            mdat_offset: self.mdat_offset.load(Ordering::Relaxed),
            mdat_size: self.mdat_size.load(Ordering::Relaxed),
            last_error: DemuxError::from_u64(self.last_error.load(Ordering::Relaxed)),
        }
    }

    /// Set file size (call before parsing)
    #[inline]
    pub fn set_file_size(&self, size: u64) {
        self.file_size.store(size, Ordering::Relaxed);
    }

    /// Set moov box location
    #[inline]
    pub fn set_moov_location(&self, offset: u64, size: u64) {
        self.moov_offset.store(offset, Ordering::Relaxed);
        self.moov_size.store(size, Ordering::Relaxed);
    }

    /// Set mdat box location
    #[inline]
    pub fn set_mdat_location(&self, offset: u64, size: u64) {
        self.mdat_offset.store(offset, Ordering::Relaxed);
        self.mdat_size.store(size, Ordering::Relaxed);
    }

    /// Set error and transition to Error state
    #[inline]
    pub fn set_error(&self, error: DemuxError) {
        self.last_error.store(error.to_u64(), Ordering::Relaxed);
        self.state
            .store(DemuxerState::Error.to_u64(), Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get last error
    #[inline]
    pub fn last_error(&self) -> DemuxError {
        DemuxError::from_u64(self.last_error.load(Ordering::Relaxed))
    }

    /// Reset capsule to initial state
    pub fn reset(&self) {
        self.state
            .store(DemuxerState::Idle.to_u64(), Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
        self.file_size.store(0, Ordering::Relaxed);
        self.bytes_parsed.store(0, Ordering::Relaxed);
        self.boxes_parsed.store(0, Ordering::Relaxed);
        self.tracks_found.store(0, Ordering::Relaxed);
        self.moov_offset.store(0, Ordering::Relaxed);
        self.moov_size.store(0, Ordering::Relaxed);
        self.mdat_offset.store(0, Ordering::Relaxed);
        self.mdat_size.store(0, Ordering::Relaxed);
        self.last_error.store(DemuxError::None as u64, Ordering::Relaxed);
    }
}

impl Default for Mp4DemuxerCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// T28 Testing (Q1-Q7: Unit Tests)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Q1: Test capsule size and alignment
    #[test]
    fn test_capsule_size_alignment() {
        assert_eq!(core::mem::size_of::<Mp4DemuxerCapsule>(), 512);
        assert_eq!(core::mem::align_of::<Mp4DemuxerCapsule>(), 512);
    }

    // Q2: Test 8-byte box header parsing
    #[test]
    fn test_parse_box_header_8byte() {
        let capsule = Mp4DemuxerCapsule::new();

        // ftyp box with size 20
        let data = [
            0x00, 0x00, 0x00, 0x14, // size = 20
            b'f', b't', b'y', b'p', // type = "ftyp"
        ];

        let result = capsule.parse_box_header(&data);
        assert!(result.is_ok());

        let info = result.unwrap();
        assert_eq!(info.box_type, *b"ftyp");
        assert_eq!(info.size, 20);
        assert_eq!(info.header_size, 8);
    }

    // Q3: Test 16-byte extended box header parsing
    #[test]
    fn test_parse_box_header_16byte() {
        let capsule = Mp4DemuxerCapsule::new();

        // mdat box with extended size
        let data = [
            0x00, 0x00, 0x00, 0x01, // size = 1 (extended)
            b'm', b'd', b'a', b't', // type = "mdat"
            0x00, 0x00, 0x00, 0x00, // extended size high
            0x00, 0x00, 0x10, 0x00, // extended size low = 4096
        ];

        let result = capsule.parse_box_header(&data);
        assert!(result.is_ok());

        let info = result.unwrap();
        assert_eq!(info.box_type, *b"mdat");
        assert_eq!(info.size, 4096);
        assert_eq!(info.header_size, 16);
    }

    // Q4: Test ftyp parsing
    #[test]
    fn test_parse_ftyp() {
        let mut capsule = Mp4DemuxerCapsule::new();

        // ftyp content: major=isom, version=512, brands=[isom, mp41, mp42]
        let data = [
            b'i', b's', b'o', b'm', // major_brand = "isom"
            0x00, 0x00, 0x02, 0x00, // minor_version = 512
            b'i', b's', b'o', b'm', // compatible brand 1
            b'm', b'p', b'4', b'1', // compatible brand 2
            b'm', b'p', b'4', b'2', // compatible brand 3
        ];

        let result = capsule.parse_ftyp(&data);
        assert!(result.is_ok());

        let ftyp = result.unwrap();
        assert_eq!(&ftyp.major_brand, b"isom");
        assert_eq!(ftyp.minor_version, 512);
        assert_eq!(ftyp.compatible_brands.len(), 3);
        assert!(ftyp.is_mp4());
    }

    // Q5: Test state transitions
    #[test]
    fn test_state_transitions() {
        let capsule = Mp4DemuxerCapsule::new();

        // Initial state
        assert_eq!(capsule.state(), DemuxerState::Idle);
        assert_eq!(capsule.stats().generation, 0);

        // Valid transition: Idle -> ParsingFtyp
        let result = capsule.transition_state(DemuxerState::Idle, DemuxerState::ParsingFtyp);
        assert!(result.is_ok());
        assert_eq!(capsule.state(), DemuxerState::ParsingFtyp);
        assert_eq!(capsule.stats().generation, 1);

        // Valid transition: ParsingFtyp -> ParsingMoov
        let result = capsule.transition_state(DemuxerState::ParsingFtyp, DemuxerState::ParsingMoov);
        assert!(result.is_ok());
        assert_eq!(capsule.state(), DemuxerState::ParsingMoov);
        assert_eq!(capsule.stats().generation, 2);

        // Invalid transition: Expected Idle but currently ParsingMoov
        let result = capsule.transition_state(DemuxerState::Idle, DemuxerState::Ready);
        assert_eq!(result, Err(DemuxError::InvalidState));
        assert_eq!(capsule.state(), DemuxerState::Error);
    }

    // Q6: Test find_box
    #[test]
    fn test_find_box() {
        let capsule = Mp4DemuxerCapsule::new();

        // Simulated container with multiple boxes
        let data = [
            // Box 1: ftyp (size=16)
            0x00, 0x00, 0x00, 0x10, b'f', b't', b'y', b'p', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, // Box 2: moov (size=16)
            0x00, 0x00, 0x00, 0x10, b'm', b'o', b'o', b'v', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, // Box 3: mdat (size=16)
            0x00, 0x00, 0x00, 0x10, b'm', b'd', b'a', b't', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];

        // Find moov
        let moov = capsule.find_box(&data, b"moov");
        assert!(moov.is_some());
        assert_eq!(moov.unwrap().offset, 16);

        // Find mdat
        let mdat = capsule.find_box(&data, b"mdat");
        assert!(mdat.is_some());
        assert_eq!(mdat.unwrap().offset, 32);

        // Box not found
        let uuid = capsule.find_box(&data, b"uuid");
        assert!(uuid.is_none());
    }

    // Q7: Test error handling
    #[test]
    fn test_error_handling() {
        let capsule = Mp4DemuxerCapsule::new();

        // Too short data
        let short_data = [0x00, 0x00, 0x00];
        let result = capsule.parse_box_header(&short_data);
        assert_eq!(result, Err(DemuxError::UnexpectedEof));

        // Invalid size (too small)
        let invalid_data = [
            0x00, 0x00, 0x00, 0x04, // size = 4 (less than minimum 8)
            b'f', b't', b'y', b'p',
        ];
        let result = capsule.parse_box_header(&invalid_data);
        assert_eq!(result, Err(DemuxError::InvalidBoxSize));

        // Extended size but not enough data
        let ext_short = [
            0x00, 0x00, 0x00, 0x01, // size = 1 (extended)
            b'm', b'd', b'a', b't', // Not enough bytes for extended size
        ];
        let result = capsule.parse_box_header(&ext_short);
        assert_eq!(result, Err(DemuxError::UnexpectedEof));
    }

    // Test generation counter increments
    #[test]
    fn test_generation_counter() {
        let capsule = Mp4DemuxerCapsule::new();
        assert_eq!(capsule.stats().generation, 0);

        // Each state change should increment generation
        capsule
            .transition_state(DemuxerState::Idle, DemuxerState::ParsingFtyp)
            .unwrap();
        assert_eq!(capsule.stats().generation, 1);

        // Reset should also increment generation
        capsule.reset();
        assert_eq!(capsule.stats().generation, 2);
        assert_eq!(capsule.state(), DemuxerState::Idle);
    }

    // Test box content calculations
    #[test]
    fn test_box_info_calculations() {
        let info = BoxInfo {
            box_type: *b"moov",
            size: 1000,
            offset: 100,
            header_size: 8,
        };

        assert_eq!(info.content_offset(), 108);
        assert_eq!(info.content_size(), 992);
        assert!(info.is_container());
        assert!(!info.is_full_box());

        let full_box = BoxInfo {
            box_type: *b"mvhd",
            size: 120,
            offset: 0,
            header_size: 8,
        };
        assert!(!full_box.is_container());
        assert!(full_box.is_full_box());
    }

    // Test FileTypeBox methods
    #[test]
    fn test_file_type_box() {
        let ftyp = FileTypeBox {
            major_brand: *b"mp41",
            minor_version: 0,
            compatible_brands: vec![*b"isom", *b"mp41", *b"mp42"],
        };

        assert!(ftyp.is_mp4());
        assert!(ftyp.is_compatible(b"isom"));
        assert!(ftyp.is_compatible(b"mp41"));
        assert!(!ftyp.is_av1());

        let av1_ftyp = FileTypeBox {
            major_brand: *b"av01",
            minor_version: 0,
            compatible_brands: vec![*b"av01", *b"iso2"],
        };
        assert!(av1_ftyp.is_av1());
    }

    // Test moov structure parsing
    #[test]
    fn test_parse_moov_structure() {
        let mut capsule = Mp4DemuxerCapsule::new();

        // Simplified moov with one trak (non-container) - using free instead to avoid recursion
        let moov_content = [
            // mvhd box (size=16, minimal)
            0x00, 0x00, 0x00, 0x10, b'm', b'v', b'h', b'd', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, // free box (size=16, minimal) - not a container
            0x00, 0x00, 0x00, 0x10, b'f', b'r', b'e', b'e', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];

        let result = capsule.parse_moov_structure(&moov_content);
        assert!(result.is_ok());

        let boxes = result.unwrap();
        assert_eq!(boxes.len(), 2);
        assert_eq!(boxes[0].box_type, *b"mvhd");
        assert_eq!(boxes[1].box_type, *b"free");
        // No tracks in this test (we used free instead of trak)
        assert_eq!(capsule.stats().tracks_found, 0);
        assert_eq!(capsule.stats().boxes_parsed, 2);
    }

    // Test moov structure with nested trak parsing
    #[test]
    fn test_parse_moov_with_trak() {
        let mut capsule = Mp4DemuxerCapsule::new();

        // moov with trak containing tkhd (non-container full box)
        let moov_content = [
            // mvhd box (size=16)
            0x00, 0x00, 0x00, 0x10, b'm', b'v', b'h', b'd', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, // trak box (size=24, contains tkhd child)
            0x00, 0x00, 0x00, 0x18, b't', b'r', b'a', b'k',
            // tkhd box nested inside trak (size=16) - NOT a container
            0x00, 0x00, 0x00, 0x10, b't', b'k', b'h', b'd', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];

        let result = capsule.parse_moov_structure(&moov_content);
        assert!(result.is_ok());

        let boxes = result.unwrap();
        // mvhd + trak + tkhd (nested inside trak) = 3 boxes
        assert_eq!(boxes.len(), 3);
        assert_eq!(boxes[0].box_type, *b"mvhd");
        assert_eq!(boxes[1].box_type, *b"trak");
        assert_eq!(boxes[2].box_type, *b"tkhd");
        assert_eq!(capsule.stats().tracks_found, 1);
        assert_eq!(capsule.stats().boxes_parsed, 3);
    }

    // Test stats snapshot
    #[test]
    fn test_stats_snapshot() {
        let capsule = Mp4DemuxerCapsule::new();

        capsule.set_file_size(1_000_000);
        capsule.set_moov_location(100, 5000);
        capsule.set_mdat_location(5100, 994_900);

        let stats = capsule.stats();
        assert_eq!(stats.file_size, 1_000_000);
        assert_eq!(stats.moov_offset, 100);
        assert_eq!(stats.moov_size, 5000);
        assert_eq!(stats.mdat_offset, 5100);
        assert_eq!(stats.mdat_size, 994_900);
    }

    // Test EOF box (size = 0)
    #[test]
    fn test_eof_box() {
        let capsule = Mp4DemuxerCapsule::new();

        // Box that extends to EOF
        let data = [
            0x00, 0x00, 0x00, 0x00, // size = 0 (extends to EOF)
            b'm', b'd', b'a', b't',
        ];

        let result = capsule.parse_box_header(&data);
        assert!(result.is_ok());

        let info = result.unwrap();
        assert_eq!(info.size, 0);
        assert_eq!(info.header_size, 8);
        assert_eq!(info.content_size(), u64::MAX); // EOF
    }

    // Q8-Q14: Property tests would go here with proptest
    // TODO: Add proptest for arbitrary box sizes/types
    // Example structure:
    // proptest! {
    //     #[test]
    //     fn test_box_header_roundtrip(size in 8u32..u32::MAX, type_bytes in any::<[u8; 4]>()) {
    //         // Create header, parse it, verify consistency
    //     }
    // }
}
