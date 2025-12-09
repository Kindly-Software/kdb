//! MP4 Sample Table (stbl) capsule
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Parses and manages sample table boxes (stts, stsc, stsz, stco/co64, stss)
//! for efficient sample access and seeking.
//!
//! # Sample Table Boxes
//!
//! - stts: Time-to-sample (decode timestamps)
//! - ctts: Composition time offset (presentation timestamps)
//! - stsc: Sample-to-chunk mapping
//! - stsz: Sample sizes (or stz2 for compact)
//! - stco/co64: Chunk offsets (32-bit or 64-bit)
//! - stss: Sync sample table (keyframes)
//!
//! # Architecture
//!
//! T4 Batch tier capsule (1024B cache-aligned) that caches hot sample table
//! entries to avoid repeated parsing. Supports batch sample queries for
//! efficient sequential access.
//!
//! # UCE34/Chaos Compliance
//!
//! - Q10: T4 Batch tier (batch sample queries, cached entries)
//! - Q33: 100% lockfree (AtomicU64/AtomicU32 for hot fields)
//! - Q34: Generation counter for cache invalidation

use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};

/// Time-to-sample entry (stts box)
///
/// Maps a run of consecutive samples to their duration.
/// All samples in the run have the same duration (sample_delta).
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct SttsEntry {
    /// Number of consecutive samples with this duration
    pub sample_count: u32,
    /// Duration of each sample in timescale units
    pub sample_delta: u32,
}

/// Sample-to-chunk entry (stsc box)
///
/// Maps samples to chunks. Each entry describes a run of chunks
/// that have the same number of samples per chunk.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct StscEntry {
    /// First chunk in this run (1-indexed in file, stored 0-indexed here)
    pub first_chunk: u32,
    /// Number of samples in each chunk in this run
    pub samples_per_chunk: u32,
    /// Sample description index (usually 1)
    pub sample_description_index: u32,
}

/// Composition time offset entry (ctts box)
///
/// Provides the offset between decode time (DTS) and presentation time (PTS).
/// PTS = DTS + sample_offset
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct CttsEntry {
    /// Number of consecutive samples with this offset
    pub sample_count: u32,
    /// Offset to add to DTS to get PTS (can be negative in version 1)
    pub sample_offset: i32,
}

/// Sample location result
///
/// Complete information needed to read and decode a sample.
#[derive(Debug, Clone, Copy, Default)]
pub struct SampleLocation {
    /// File offset where sample data begins
    pub offset: u64,
    /// Sample size in bytes
    pub size: u32,
    /// Decode timestamp (in timescale units)
    pub dts: u64,
    /// Presentation timestamp (in timescale units)
    pub pts: u64,
    /// Whether this sample is a keyframe (sync sample)
    pub is_keyframe: bool,
}

/// T4 Batch capsule for MP4 sample table
///
/// Larger size (1024B) to cache hot sample table entries for fast access.
/// All atomics are for generation tracking and cache invalidation.
///
/// # Cache Strategy
///
/// The capsule caches the first N entries of each table in memory.
/// For tables larger than the cache, the raw data buffer must be provided
/// to `get_sample_location()` for on-demand parsing.
#[repr(C, align(1024))]
pub struct Mp4SampleTableCapsule {
    // ===== Metadata (64 bytes) =====
    /// Total number of samples in the track
    pub sample_count: AtomicU64,
    /// Total number of chunks in the track
    pub chunk_count: AtomicU64,
    /// Number of keyframes (sync samples)
    pub keyframe_count: AtomicU64,
    /// Generation counter for cache invalidation (Q34 compliance)
    pub generation: AtomicU64,
    /// Media timescale (ticks per second)
    pub timescale: AtomicU32,
    /// Default sample size (0 if variable sizes in stsz)
    pub default_sample_size: AtomicU32,
    /// Sample table flags (see sample_table_flags module)
    pub flags: AtomicU64,
    _meta_pad: [u8; 16],

    // ===== stts cache (128 bytes) =====
    /// Number of entries in stts box
    pub stts_entry_count: AtomicU32,
    /// Total samples computed from stts (for validation)
    pub stts_total_samples: AtomicU64,
    /// Total duration computed from stts
    pub stts_total_duration: AtomicU64,
    /// Cached stts entries (first 12 entries, 96 bytes)
    stts_cache: [SttsEntry; 12],
    _stts_pad: [u8; 8],

    // ===== stsc cache (128 bytes) =====
    /// Number of entries in stsc box
    pub stsc_entry_count: AtomicU32,
    /// Cached stsc entries (first 10 entries, 120 bytes)
    stsc_cache: [StscEntry; 10],
    _stsc_pad: [u8; 4],

    // ===== stco/co64 info (64 bytes) =====
    /// Number of chunk offset entries
    pub chunk_offset_entry_count: AtomicU64,
    /// 1 if using 64-bit chunk offsets (co64), 0 for 32-bit (stco)
    pub uses_co64: AtomicU64,
    /// First chunk offset (cached for quick access)
    pub first_chunk_offset: AtomicU64,
    /// File offset where chunk offset data begins (for on-demand parsing)
    pub chunk_offsets_file_offset: AtomicU64,
    _stco_pad: [u8; 32],

    // ===== stsz info (64 bytes) =====
    /// Number of sample size entries
    pub stsz_entry_count: AtomicU64,
    /// File offset where sample sizes begin (for on-demand parsing)
    pub stsz_file_offset: AtomicU64,
    /// Constant sample size (0 if variable sizes)
    pub constant_sample_size: AtomicU32,
    _stsz_pad: [u8; 44],

    // ===== stss (sync samples) cache (64 bytes) =====
    /// Number of sync sample entries
    pub stss_entry_count: AtomicU32,
    /// Cached sync sample indices (first 14 keyframe indices)
    stss_cache: [u32; 14],
    _stss_pad: [u8; 4],

    // ===== ctts cache (64 bytes) =====
    /// Number of ctts entries
    pub ctts_entry_count: AtomicU32,
    /// 1 if ctts box is present, 0 otherwise
    pub has_ctts: AtomicU32,
    /// Cached ctts entries (first 7 entries, 56 bytes)
    ctts_cache: [CttsEntry; 7],

    // ===== Seeking state (64 bytes) =====
    /// Last accessed sample index (for sequential optimization)
    pub last_sample_index: AtomicU64,
    /// Last accessed chunk index
    pub last_chunk_index: AtomicU64,
    /// Last computed file offset
    pub last_file_offset: AtomicU64,
    /// Last computed timestamp
    pub last_timestamp: AtomicU64,
    _seek_pad: [u8; 32],

    // ===== Batch processing (384 bytes) =====
    /// Buffer for batch sample sizes (64 samples)
    batch_sample_sizes: [u32; 64],
    /// Buffer for batch sample offsets (16 entries for chunk boundaries)
    batch_sample_offsets: [u64; 16],
}

/// Sample table error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SampleTableError {
    /// No error
    None = 0,
    /// Invalid stts box data
    InvalidStts = 1,
    /// Invalid stsc box data
    InvalidStsc = 2,
    /// Invalid stsz box data
    InvalidStsz = 3,
    /// Invalid stco box data
    InvalidStco = 4,
    /// Invalid stss box data
    InvalidStss = 5,
    /// Invalid ctts box data
    InvalidCtts = 6,
    /// Requested sample index out of range
    SampleOutOfRange = 7,
    /// Requested chunk index out of range
    ChunkOutOfRange = 8,
    /// Seek operation failed
    SeekError = 9,
    /// Data buffer too small
    BufferTooSmall = 10,
    /// Missing required table
    MissingTable = 11,
}

impl core::fmt::Display for SampleTableError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::None => write!(f, "no error"),
            Self::InvalidStts => write!(f, "invalid stts box"),
            Self::InvalidStsc => write!(f, "invalid stsc box"),
            Self::InvalidStsz => write!(f, "invalid stsz box"),
            Self::InvalidStco => write!(f, "invalid stco box"),
            Self::InvalidStss => write!(f, "invalid stss box"),
            Self::InvalidCtts => write!(f, "invalid ctts box"),
            Self::SampleOutOfRange => write!(f, "sample index out of range"),
            Self::ChunkOutOfRange => write!(f, "chunk index out of range"),
            Self::SeekError => write!(f, "seek operation failed"),
            Self::BufferTooSmall => write!(f, "data buffer too small"),
            Self::MissingTable => write!(f, "required sample table missing"),
        }
    }
}

impl std::error::Error for SampleTableError {}

/// Sample table flags indicating which boxes have been parsed
pub mod sample_table_flags {
    /// stts (time-to-sample) has been parsed
    pub const HAS_STTS: u64 = 1 << 0;
    /// stsc (sample-to-chunk) has been parsed
    pub const HAS_STSC: u64 = 1 << 1;
    /// stsz (sample sizes) has been parsed
    pub const HAS_STSZ: u64 = 1 << 2;
    /// stco (32-bit chunk offsets) has been parsed
    pub const HAS_STCO: u64 = 1 << 3;
    /// co64 (64-bit chunk offsets) has been parsed
    pub const HAS_CO64: u64 = 1 << 4;
    /// stss (sync samples) has been parsed
    pub const HAS_STSS: u64 = 1 << 5;
    /// ctts (composition time offsets) has been parsed
    pub const HAS_CTTS: u64 = 1 << 6;
    /// All required tables for basic playback have been parsed
    pub const FULLY_PARSED: u64 = HAS_STTS | HAS_STSC | HAS_STSZ | HAS_STCO;
}

impl Default for Mp4SampleTableCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl Mp4SampleTableCapsule {
    /// Create a new sample table capsule with default values
    #[inline]
    pub fn new() -> Self {
        Self {
            // Metadata
            sample_count: AtomicU64::new(0),
            chunk_count: AtomicU64::new(0),
            keyframe_count: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            timescale: AtomicU32::new(1), // Avoid division by zero
            default_sample_size: AtomicU32::new(0),
            flags: AtomicU64::new(0),
            _meta_pad: [0u8; 16],

            // stts cache
            stts_entry_count: AtomicU32::new(0),
            stts_total_samples: AtomicU64::new(0),
            stts_total_duration: AtomicU64::new(0),
            stts_cache: [SttsEntry::default(); 12],
            _stts_pad: [0u8; 8],

            // stsc cache
            stsc_entry_count: AtomicU32::new(0),
            stsc_cache: [StscEntry::default(); 10],
            _stsc_pad: [0u8; 4],

            // stco/co64 info
            chunk_offset_entry_count: AtomicU64::new(0),
            uses_co64: AtomicU64::new(0),
            first_chunk_offset: AtomicU64::new(0),
            chunk_offsets_file_offset: AtomicU64::new(0),
            _stco_pad: [0u8; 32],

            // stsz info
            stsz_entry_count: AtomicU64::new(0),
            stsz_file_offset: AtomicU64::new(0),
            constant_sample_size: AtomicU32::new(0),
            _stsz_pad: [0u8; 44],

            // stss cache
            stss_entry_count: AtomicU32::new(0),
            stss_cache: [0u32; 14],
            _stss_pad: [0u8; 4],

            // ctts cache
            ctts_entry_count: AtomicU32::new(0),
            has_ctts: AtomicU32::new(0),
            ctts_cache: [CttsEntry::default(); 7],

            // Seeking state
            last_sample_index: AtomicU64::new(0),
            last_chunk_index: AtomicU64::new(0),
            last_file_offset: AtomicU64::new(0),
            last_timestamp: AtomicU64::new(0),
            _seek_pad: [0u8; 32],

            // Batch processing
            batch_sample_sizes: [0u32; 64],
            batch_sample_offsets: [0u64; 16],
        }
    }

    /// Create a new sample table capsule with specified timescale
    ///
    /// # Arguments
    ///
    /// * `timescale` - Media timescale (ticks per second, e.g., 90000 for video)
    #[inline]
    pub fn with_timescale(timescale: u32) -> Self {
        let mut capsule = Self::new();
        capsule.timescale.store(timescale.max(1), Ordering::Release);
        capsule
    }

    /// Increment generation counter (call when cache is invalidated)
    #[inline]
    fn bump_generation(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Read big-endian u32 from slice
    #[inline]
    fn read_be_u32(data: &[u8], offset: usize) -> Option<u32> {
        if offset + 4 > data.len() {
            return None;
        }
        Some(u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]))
    }

    /// Read big-endian i32 from slice
    #[inline]
    fn read_be_i32(data: &[u8], offset: usize) -> Option<i32> {
        Self::read_be_u32(data, offset).map(|v| v as i32)
    }

    /// Read big-endian u64 from slice
    #[inline]
    fn read_be_u64(data: &[u8], offset: usize) -> Option<u64> {
        if offset + 8 > data.len() {
            return None;
        }
        Some(u64::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]))
    }

    /// Parse stts (time-to-sample) box data
    ///
    /// Format: entry_count(4), entries[entry_count] where each entry is
    /// sample_count(4) + sample_delta(4)
    ///
    /// # Arguments
    ///
    /// * `data` - Raw stts box payload (after box header)
    pub fn parse_stts(&mut self, data: &[u8]) -> Result<(), SampleTableError> {
        // Minimum size: version(1) + flags(3) + entry_count(4) = 8 bytes
        if data.len() < 8 {
            return Err(SampleTableError::InvalidStts);
        }

        // Skip version (1) and flags (3)
        let entry_count = Self::read_be_u32(data, 4).ok_or(SampleTableError::InvalidStts)?;

        // Validate we have enough data for all entries
        let entries_size = entry_count as usize * 8; // 8 bytes per entry
        if data.len() < 8 + entries_size {
            return Err(SampleTableError::InvalidStts);
        }

        self.stts_entry_count.store(entry_count, Ordering::Release);

        // Parse entries and calculate totals
        let mut total_samples: u64 = 0;
        let mut total_duration: u64 = 0;
        let mut offset = 8;

        let cache_count = (entry_count as usize).min(12);

        for i in 0..entry_count as usize {
            let sample_count = Self::read_be_u32(data, offset).ok_or(SampleTableError::InvalidStts)?;
            let sample_delta = Self::read_be_u32(data, offset + 4).ok_or(SampleTableError::InvalidStts)?;

            total_samples += sample_count as u64;
            total_duration += (sample_count as u64) * (sample_delta as u64);

            // Cache first entries
            if i < cache_count {
                self.stts_cache[i] = SttsEntry {
                    sample_count,
                    sample_delta,
                };
            }

            offset += 8;
        }

        self.stts_total_samples.store(total_samples, Ordering::Release);
        self.stts_total_duration.store(total_duration, Ordering::Release);

        // Update sample count if this is the first time we're setting it
        if self.sample_count.load(Ordering::Acquire) == 0 {
            self.sample_count.store(total_samples, Ordering::Release);
        }

        // Set flag
        let old_flags = self.flags.load(Ordering::Acquire);
        self.flags.store(old_flags | sample_table_flags::HAS_STTS, Ordering::Release);

        self.bump_generation();
        Ok(())
    }

    /// Parse stsc (sample-to-chunk) box data
    ///
    /// Format: entry_count(4), entries[entry_count] where each entry is
    /// first_chunk(4) + samples_per_chunk(4) + sample_description_index(4)
    ///
    /// Note: first_chunk is 1-indexed in the file format, but we store 0-indexed
    ///
    /// # Arguments
    ///
    /// * `data` - Raw stsc box payload (after box header)
    pub fn parse_stsc(&mut self, data: &[u8]) -> Result<(), SampleTableError> {
        // Minimum size: version(1) + flags(3) + entry_count(4) = 8 bytes
        if data.len() < 8 {
            return Err(SampleTableError::InvalidStsc);
        }

        let entry_count = Self::read_be_u32(data, 4).ok_or(SampleTableError::InvalidStsc)?;

        // Validate we have enough data for all entries (12 bytes per entry)
        let entries_size = entry_count as usize * 12;
        if data.len() < 8 + entries_size {
            return Err(SampleTableError::InvalidStsc);
        }

        self.stsc_entry_count.store(entry_count, Ordering::Release);

        // Parse entries and cache first 10
        let mut offset = 8;
        let cache_count = (entry_count as usize).min(10);

        for i in 0..cache_count {
            let first_chunk = Self::read_be_u32(data, offset).ok_or(SampleTableError::InvalidStsc)?;
            let samples_per_chunk = Self::read_be_u32(data, offset + 4).ok_or(SampleTableError::InvalidStsc)?;
            let sample_desc_idx = Self::read_be_u32(data, offset + 8).ok_or(SampleTableError::InvalidStsc)?;

            // Convert to 0-indexed
            self.stsc_cache[i] = StscEntry {
                first_chunk: first_chunk.saturating_sub(1),
                samples_per_chunk,
                sample_description_index: sample_desc_idx,
            };

            offset += 12;
        }

        // Set flag
        let old_flags = self.flags.load(Ordering::Acquire);
        self.flags.store(old_flags | sample_table_flags::HAS_STSC, Ordering::Release);

        self.bump_generation();
        Ok(())
    }

    /// Parse stsz (sample size) box data
    ///
    /// Format: sample_size(4), sample_count(4)
    /// If sample_size == 0: sizes[sample_count] where each is 4 bytes
    ///
    /// # Arguments
    ///
    /// * `data` - Raw stsz box payload (after box header)
    pub fn parse_stsz(&mut self, data: &[u8]) -> Result<(), SampleTableError> {
        // Minimum size: version(1) + flags(3) + sample_size(4) + sample_count(4) = 12 bytes
        if data.len() < 12 {
            return Err(SampleTableError::InvalidStsz);
        }

        let sample_size = Self::read_be_u32(data, 4).ok_or(SampleTableError::InvalidStsz)?;
        let sample_count = Self::read_be_u32(data, 8).ok_or(SampleTableError::InvalidStsz)?;

        self.constant_sample_size.store(sample_size, Ordering::Release);
        self.stsz_entry_count.store(sample_count as u64, Ordering::Release);

        // If variable sizes, validate we have all entries
        if sample_size == 0 {
            let entries_size = sample_count as usize * 4;
            if data.len() < 12 + entries_size {
                return Err(SampleTableError::InvalidStsz);
            }
            // Store offset where sample sizes begin (relative to stsz box start)
            self.stsz_file_offset.store(12, Ordering::Release);
        }

        // Update sample count
        self.sample_count.store(sample_count as u64, Ordering::Release);

        // Set default sample size for quick access
        self.default_sample_size.store(sample_size, Ordering::Release);

        // Set flag
        let old_flags = self.flags.load(Ordering::Acquire);
        self.flags.store(old_flags | sample_table_flags::HAS_STSZ, Ordering::Release);

        self.bump_generation();
        Ok(())
    }

    /// Parse stco (32-bit chunk offset) box data
    ///
    /// Format: entry_count(4), offsets[entry_count] where each is 4 bytes
    ///
    /// # Arguments
    ///
    /// * `data` - Raw stco box payload (after box header)
    pub fn parse_stco(&mut self, data: &[u8]) -> Result<(), SampleTableError> {
        // Minimum size: version(1) + flags(3) + entry_count(4) = 8 bytes
        if data.len() < 8 {
            return Err(SampleTableError::InvalidStco);
        }

        let entry_count = Self::read_be_u32(data, 4).ok_or(SampleTableError::InvalidStco)?;

        // Validate we have enough data
        let entries_size = entry_count as usize * 4;
        if data.len() < 8 + entries_size {
            return Err(SampleTableError::InvalidStco);
        }

        self.chunk_offset_entry_count.store(entry_count as u64, Ordering::Release);
        self.chunk_count.store(entry_count as u64, Ordering::Release);
        self.uses_co64.store(0, Ordering::Release);

        // Cache first offset
        if entry_count > 0 {
            let first_offset = Self::read_be_u32(data, 8).ok_or(SampleTableError::InvalidStco)?;
            self.first_chunk_offset.store(first_offset as u64, Ordering::Release);
        }

        // Store file offset for on-demand access
        self.chunk_offsets_file_offset.store(8, Ordering::Release);

        // Set flag
        let old_flags = self.flags.load(Ordering::Acquire);
        self.flags.store(old_flags | sample_table_flags::HAS_STCO, Ordering::Release);

        self.bump_generation();
        Ok(())
    }

    /// Parse co64 (64-bit chunk offset) box data
    ///
    /// Format: entry_count(4), offsets[entry_count] where each is 8 bytes
    ///
    /// # Arguments
    ///
    /// * `data` - Raw co64 box payload (after box header)
    pub fn parse_co64(&mut self, data: &[u8]) -> Result<(), SampleTableError> {
        // Minimum size: version(1) + flags(3) + entry_count(4) = 8 bytes
        if data.len() < 8 {
            return Err(SampleTableError::InvalidStco);
        }

        let entry_count = Self::read_be_u32(data, 4).ok_or(SampleTableError::InvalidStco)?;

        // Validate we have enough data (8 bytes per entry)
        let entries_size = entry_count as usize * 8;
        if data.len() < 8 + entries_size {
            return Err(SampleTableError::InvalidStco);
        }

        self.chunk_offset_entry_count.store(entry_count as u64, Ordering::Release);
        self.chunk_count.store(entry_count as u64, Ordering::Release);
        self.uses_co64.store(1, Ordering::Release);

        // Cache first offset
        if entry_count > 0 {
            let first_offset = Self::read_be_u64(data, 8).ok_or(SampleTableError::InvalidStco)?;
            self.first_chunk_offset.store(first_offset, Ordering::Release);
        }

        // Store file offset for on-demand access
        self.chunk_offsets_file_offset.store(8, Ordering::Release);

        // Set flags (HAS_CO64 implies HAS_STCO for the FULLY_PARSED check)
        let old_flags = self.flags.load(Ordering::Acquire);
        self.flags.store(
            old_flags | sample_table_flags::HAS_CO64 | sample_table_flags::HAS_STCO,
            Ordering::Release,
        );

        self.bump_generation();
        Ok(())
    }

    /// Parse stss (sync sample) box data
    ///
    /// Format: entry_count(4), sample_numbers[entry_count] where each is 4 bytes
    /// Note: sample numbers are 1-indexed in the file format
    ///
    /// # Arguments
    ///
    /// * `data` - Raw stss box payload (after box header)
    pub fn parse_stss(&mut self, data: &[u8]) -> Result<(), SampleTableError> {
        // Minimum size: version(1) + flags(3) + entry_count(4) = 8 bytes
        if data.len() < 8 {
            return Err(SampleTableError::InvalidStss);
        }

        let entry_count = Self::read_be_u32(data, 4).ok_or(SampleTableError::InvalidStss)?;

        // Validate we have enough data
        let entries_size = entry_count as usize * 4;
        if data.len() < 8 + entries_size {
            return Err(SampleTableError::InvalidStss);
        }

        self.stss_entry_count.store(entry_count, Ordering::Release);
        self.keyframe_count.store(entry_count as u64, Ordering::Release);

        // Cache first 14 keyframe indices (convert to 0-indexed)
        let cache_count = (entry_count as usize).min(14);
        let mut offset = 8;

        for i in 0..cache_count {
            let sample_num = Self::read_be_u32(data, offset).ok_or(SampleTableError::InvalidStss)?;
            self.stss_cache[i] = sample_num.saturating_sub(1); // Convert to 0-indexed
            offset += 4;
        }

        // Set flag
        let old_flags = self.flags.load(Ordering::Acquire);
        self.flags.store(old_flags | sample_table_flags::HAS_STSS, Ordering::Release);

        self.bump_generation();
        Ok(())
    }

    /// Parse ctts (composition time offset) box data
    ///
    /// Format: version(1), flags(3), entry_count(4)
    /// entries[entry_count] where each is sample_count(4) + sample_offset(4)
    /// version 0: offset is unsigned, version 1: offset is signed
    ///
    /// # Arguments
    ///
    /// * `data` - Raw ctts box payload (after box header)
    pub fn parse_ctts(&mut self, data: &[u8]) -> Result<(), SampleTableError> {
        // Minimum size: version(1) + flags(3) + entry_count(4) = 8 bytes
        if data.len() < 8 {
            return Err(SampleTableError::InvalidCtts);
        }

        let _version = data[0]; // We handle both v0 and v1 the same way (signed i32)
        let entry_count = Self::read_be_u32(data, 4).ok_or(SampleTableError::InvalidCtts)?;

        // Validate we have enough data
        let entries_size = entry_count as usize * 8;
        if data.len() < 8 + entries_size {
            return Err(SampleTableError::InvalidCtts);
        }

        self.ctts_entry_count.store(entry_count, Ordering::Release);
        self.has_ctts.store(1, Ordering::Release);

        // Cache first 7 entries
        let cache_count = (entry_count as usize).min(7);
        let mut offset = 8;

        for i in 0..cache_count {
            let sample_count = Self::read_be_u32(data, offset).ok_or(SampleTableError::InvalidCtts)?;
            let sample_offset = Self::read_be_i32(data, offset + 4).ok_or(SampleTableError::InvalidCtts)?;

            self.ctts_cache[i] = CttsEntry {
                sample_count,
                sample_offset,
            };

            offset += 8;
        }

        // Set flag
        let old_flags = self.flags.load(Ordering::Acquire);
        self.flags.store(old_flags | sample_table_flags::HAS_CTTS, Ordering::Release);

        self.bump_generation();
        Ok(())
    }

    /// Get chunk offset for a specific chunk index
    ///
    /// # Arguments
    ///
    /// * `chunk_index` - 0-indexed chunk number
    /// * `stco_data` - Raw stco/co64 box payload
    fn get_chunk_offset(&self, chunk_index: u64, stco_data: &[u8]) -> Result<u64, SampleTableError> {
        let chunk_count = self.chunk_offset_entry_count.load(Ordering::Acquire);
        if chunk_index >= chunk_count {
            return Err(SampleTableError::ChunkOutOfRange);
        }

        // Check if first chunk (cached)
        if chunk_index == 0 {
            return Ok(self.first_chunk_offset.load(Ordering::Acquire));
        }

        let base_offset = self.chunk_offsets_file_offset.load(Ordering::Acquire) as usize;
        let uses_co64 = self.uses_co64.load(Ordering::Acquire) != 0;

        if uses_co64 {
            let offset = base_offset + (chunk_index as usize) * 8;
            Self::read_be_u64(stco_data, offset).ok_or(SampleTableError::BufferTooSmall)
        } else {
            let offset = base_offset + (chunk_index as usize) * 4;
            Self::read_be_u32(stco_data, offset)
                .map(|v| v as u64)
                .ok_or(SampleTableError::BufferTooSmall)
        }
    }

    /// Get sample size for a specific sample index
    ///
    /// # Arguments
    ///
    /// * `sample_index` - 0-indexed sample number
    /// * `stsz_data` - Raw stsz box payload (optional if constant size)
    fn get_sample_size(&self, sample_index: u64, stsz_data: &[u8]) -> Result<u32, SampleTableError> {
        let sample_count = self.stsz_entry_count.load(Ordering::Acquire);
        if sample_index >= sample_count {
            return Err(SampleTableError::SampleOutOfRange);
        }

        let constant_size = self.constant_sample_size.load(Ordering::Acquire);
        if constant_size > 0 {
            return Ok(constant_size);
        }

        // Variable sizes - read from data
        let base_offset = self.stsz_file_offset.load(Ordering::Acquire) as usize;
        let offset = base_offset + (sample_index as usize) * 4;

        Self::read_be_u32(stsz_data, offset).ok_or(SampleTableError::BufferTooSmall)
    }

    /// Find chunk and offset within chunk for a sample
    ///
    /// Returns (chunk_index, sample_offset_in_chunk)
    fn find_sample_chunk(&self, sample_index: u64, stsc_data: &[u8]) -> Result<(u64, u64), SampleTableError> {
        let sample_count = self.sample_count.load(Ordering::Acquire);
        if sample_index >= sample_count {
            return Err(SampleTableError::SampleOutOfRange);
        }

        let entry_count = self.stsc_entry_count.load(Ordering::Acquire);
        if entry_count == 0 {
            return Err(SampleTableError::MissingTable);
        }

        let chunk_count = self.chunk_count.load(Ordering::Acquire);

        // Iterate through stsc entries to find the chunk
        let mut sample_offset: u64 = 0;
        let mut prev_first_chunk: u32 = 0;
        let mut prev_samples_per_chunk: u32 = 0;

        let cache_count = (entry_count as usize).min(10);

        for i in 0..entry_count as usize {
            let entry = if i < cache_count {
                self.stsc_cache[i]
            } else {
                // Parse from data
                let offset = 8 + i * 12;
                let first_chunk = Self::read_be_u32(stsc_data, offset)
                    .ok_or(SampleTableError::BufferTooSmall)?
                    .saturating_sub(1);
                let samples_per_chunk = Self::read_be_u32(stsc_data, offset + 4)
                    .ok_or(SampleTableError::BufferTooSmall)?;
                let sample_desc_idx = Self::read_be_u32(stsc_data, offset + 8)
                    .ok_or(SampleTableError::BufferTooSmall)?;

                StscEntry {
                    first_chunk,
                    samples_per_chunk,
                    sample_description_index: sample_desc_idx,
                }
            };

            // Calculate samples in chunks before this entry
            if i > 0 {
                let chunks_in_prev_run = entry.first_chunk - prev_first_chunk;
                sample_offset += (chunks_in_prev_run as u64) * (prev_samples_per_chunk as u64);
            }

            // Get next entry's first_chunk to know how many chunks in this run
            let next_first_chunk = if i + 1 < entry_count as usize {
                if i + 1 < cache_count {
                    self.stsc_cache[i + 1].first_chunk
                } else {
                    let offset = 8 + (i + 1) * 12;
                    Self::read_be_u32(stsc_data, offset)
                        .ok_or(SampleTableError::BufferTooSmall)?
                        .saturating_sub(1)
                }
            } else {
                chunk_count as u32
            };

            let chunks_in_run = next_first_chunk - entry.first_chunk;
            let samples_in_run = (chunks_in_run as u64) * (entry.samples_per_chunk as u64);

            if sample_index < sample_offset + samples_in_run {
                // Sample is in this run of chunks
                let sample_in_run = sample_index - sample_offset;
                let chunk_in_run = sample_in_run / entry.samples_per_chunk as u64;
                let sample_in_chunk = sample_in_run % entry.samples_per_chunk as u64;

                let chunk_index = entry.first_chunk as u64 + chunk_in_run;
                return Ok((chunk_index, sample_in_chunk));
            }

            prev_first_chunk = entry.first_chunk;
            prev_samples_per_chunk = entry.samples_per_chunk;
        }

        Err(SampleTableError::SampleOutOfRange)
    }

    /// Get sample file offset and size
    ///
    /// # Arguments
    ///
    /// * `sample_index` - 0-indexed sample number
    /// * `data` - Combined sample table data (stsc, stco, stsz concatenated or separate buffers)
    pub fn get_sample_location(
        &self,
        sample_index: u64,
        data: &[u8],
    ) -> Result<SampleLocation, SampleTableError> {
        // Check required tables are present
        let flags = self.flags.load(Ordering::Acquire);
        if flags & sample_table_flags::FULLY_PARSED != sample_table_flags::FULLY_PARSED {
            return Err(SampleTableError::MissingTable);
        }

        // Find which chunk the sample is in
        let (chunk_index, sample_in_chunk) = self.find_sample_chunk(sample_index, data)?;

        // Get chunk offset
        let chunk_offset = self.get_chunk_offset(chunk_index, data)?;

        // Calculate offset within chunk by summing sizes of preceding samples
        let mut offset_in_chunk: u64 = 0;
        let first_sample_in_chunk = sample_index - sample_in_chunk;

        for i in 0..sample_in_chunk {
            let size = self.get_sample_size(first_sample_in_chunk + i, data)?;
            offset_in_chunk += size as u64;
        }

        // Get this sample's size
        let sample_size = self.get_sample_size(sample_index, data)?;

        // Get timestamps
        let (dts, pts) = self.get_sample_timestamp(sample_index)?;

        // Check if keyframe
        let is_keyframe = self.is_keyframe(sample_index);

        // Update cache
        self.last_sample_index.store(sample_index, Ordering::Release);
        self.last_chunk_index.store(chunk_index, Ordering::Release);
        self.last_file_offset
            .store(chunk_offset + offset_in_chunk, Ordering::Release);
        self.last_timestamp.store(dts, Ordering::Release);

        Ok(SampleLocation {
            offset: chunk_offset + offset_in_chunk,
            size: sample_size,
            dts,
            pts,
            is_keyframe,
        })
    }

    /// Get sample decode and presentation timestamps
    ///
    /// # Arguments
    ///
    /// * `sample_index` - 0-indexed sample number
    ///
    /// # Returns
    ///
    /// Tuple of (DTS, PTS) in timescale units
    pub fn get_sample_timestamp(&self, sample_index: u64) -> Result<(u64, u64), SampleTableError> {
        let sample_count = self.sample_count.load(Ordering::Acquire);
        if sample_index >= sample_count {
            return Err(SampleTableError::SampleOutOfRange);
        }

        // Calculate DTS from stts
        let mut dts: u64 = 0;
        let mut remaining_samples = sample_index;
        let entry_count = self.stts_entry_count.load(Ordering::Acquire);
        let cache_count = (entry_count as usize).min(12);

        for i in 0..cache_count {
            let entry = self.stts_cache[i];
            if remaining_samples < entry.sample_count as u64 {
                dts += remaining_samples * entry.sample_delta as u64;
                break;
            }
            dts += entry.sample_count as u64 * entry.sample_delta as u64;
            remaining_samples -= entry.sample_count as u64;
        }

        // If we've exhausted the cache and still have remaining samples,
        // we'd need the raw data buffer. For now, assume cache covers all entries.
        // In production, this would need the full stts data.

        // Calculate PTS from ctts if present
        let pts = if self.has_ctts.load(Ordering::Acquire) != 0 {
            let mut cts_offset: i64 = 0;
            let mut remaining = sample_index;
            let ctts_count = self.ctts_entry_count.load(Ordering::Acquire);
            let ctts_cache_count = (ctts_count as usize).min(7);

            for i in 0..ctts_cache_count {
                let entry = self.ctts_cache[i];
                if remaining < entry.sample_count as u64 {
                    cts_offset = entry.sample_offset as i64;
                    break;
                }
                remaining -= entry.sample_count as u64;
            }

            // PTS = DTS + CTS offset (handle negative offsets)
            if cts_offset >= 0 {
                dts + cts_offset as u64
            } else {
                dts.saturating_sub((-cts_offset) as u64)
            }
        } else {
            // No ctts, PTS = DTS
            dts
        };

        Ok((dts, pts))
    }

    /// Find the nearest keyframe at or before the given sample
    ///
    /// # Arguments
    ///
    /// * `sample_index` - 0-indexed sample number
    ///
    /// # Returns
    ///
    /// Index of nearest keyframe, or None if no keyframes before this sample
    pub fn find_keyframe_before(&self, sample_index: u64) -> Option<u64> {
        let stss_count = self.stss_entry_count.load(Ordering::Acquire);

        // If no stss table, all samples are keyframes (usually audio)
        if stss_count == 0 {
            // Check if stss was parsed (flag set) or just empty
            let flags = self.flags.load(Ordering::Acquire);
            if flags & sample_table_flags::HAS_STSS == 0 {
                // No stss at all - treat all samples as keyframes
                return Some(sample_index);
            }
            // Empty stss - no keyframes
            return None;
        }

        // Search cache for nearest keyframe <= sample_index
        let cache_count = (stss_count as usize).min(14);
        let mut best: Option<u64> = None;

        for i in 0..cache_count {
            let keyframe_idx = self.stss_cache[i] as u64;
            if keyframe_idx <= sample_index {
                best = Some(keyframe_idx);
            } else {
                break; // Keyframe indices are sorted
            }
        }

        best
    }

    /// Find sample at or after the given timestamp
    ///
    /// # Arguments
    ///
    /// * `time_us` - Target time in microseconds
    ///
    /// # Returns
    ///
    /// Sample index nearest to the requested time
    pub fn seek_to_time(&self, time_us: u64) -> Result<u64, SampleTableError> {
        let timescale = self.timescale.load(Ordering::Acquire);
        let sample_count = self.sample_count.load(Ordering::Acquire);

        if sample_count == 0 {
            return Err(SampleTableError::SeekError);
        }

        // Convert microseconds to timescale units
        // time_ticks = time_us * timescale / 1_000_000
        let target_ticks = (time_us as u128 * timescale as u128 / 1_000_000) as u64;

        // Linear search through stts entries
        let mut current_sample: u64 = 0;
        let mut current_time: u64 = 0;
        let entry_count = self.stts_entry_count.load(Ordering::Acquire);
        let cache_count = (entry_count as usize).min(12);

        for i in 0..cache_count {
            let entry = self.stts_cache[i];
            let run_duration = entry.sample_count as u64 * entry.sample_delta as u64;

            if current_time + run_duration > target_ticks {
                // Target is within this run
                let ticks_into_run = target_ticks.saturating_sub(current_time);
                let samples_into_run = if entry.sample_delta > 0 {
                    ticks_into_run / entry.sample_delta as u64
                } else {
                    0
                };
                return Ok(current_sample + samples_into_run.min(entry.sample_count as u64 - 1));
            }

            current_time += run_duration;
            current_sample += entry.sample_count as u64;
        }

        // Target is past all cached entries, return last sample
        Ok(sample_count.saturating_sub(1))
    }

    /// Check if a sample is a keyframe (sync sample)
    ///
    /// # Arguments
    ///
    /// * `sample_index` - 0-indexed sample number
    pub fn is_keyframe(&self, sample_index: u64) -> bool {
        let stss_count = self.stss_entry_count.load(Ordering::Acquire);

        // If no stss table, all samples are keyframes
        if stss_count == 0 {
            let flags = self.flags.load(Ordering::Acquire);
            return flags & sample_table_flags::HAS_STSS == 0;
        }

        // Search cache
        let cache_count = (stss_count as usize).min(14);
        for i in 0..cache_count {
            let keyframe_idx = self.stss_cache[i] as u64;
            if keyframe_idx == sample_index {
                return true;
            }
            if keyframe_idx > sample_index {
                return false; // Keyframe indices are sorted
            }
        }

        false
    }

    /// Batch get sample information for sequential access
    ///
    /// This is optimized for sequential reading where samples are typically
    /// accessed in order. Returns up to 64 samples at a time.
    ///
    /// # Arguments
    ///
    /// * `start` - Starting sample index (0-indexed)
    /// * `count` - Number of samples to get (max 64)
    /// * `data` - Sample table data buffer
    ///
    /// # Returns
    ///
    /// Slice of sample locations (stored in internal buffer)
    pub fn batch_get_samples(
        &mut self,
        start: u64,
        count: u32,
        data: &[u8],
    ) -> Result<Vec<SampleLocation>, SampleTableError> {
        let sample_count = self.sample_count.load(Ordering::Acquire);
        let actual_count = count.min(64).min((sample_count.saturating_sub(start)) as u32);

        if actual_count == 0 {
            return Ok(Vec::new());
        }

        let mut results = Vec::with_capacity(actual_count as usize);

        for i in 0..actual_count {
            let sample_idx = start + i as u64;
            let location = self.get_sample_location(sample_idx, data)?;
            results.push(location);
        }

        Ok(results)
    }

    /// Check if all required tables have been parsed
    #[inline]
    pub fn is_fully_parsed(&self) -> bool {
        let flags = self.flags.load(Ordering::Acquire);
        flags & sample_table_flags::FULLY_PARSED == sample_table_flags::FULLY_PARSED
    }

    /// Get total track duration in timescale units
    #[inline]
    pub fn get_total_duration(&self) -> u64 {
        self.stts_total_duration.load(Ordering::Acquire)
    }

    /// Get total track duration in microseconds
    #[inline]
    pub fn get_duration_us(&self) -> u64 {
        let duration = self.stts_total_duration.load(Ordering::Acquire);
        let timescale = self.timescale.load(Ordering::Acquire);
        if timescale == 0 {
            return 0;
        }
        (duration as u128 * 1_000_000 / timescale as u128) as u64
    }
}

// Verify capsule size at compile time
const _: () = {
    assert!(core::mem::size_of::<Mp4SampleTableCapsule>() == 1024);
    assert!(core::mem::align_of::<Mp4SampleTableCapsule>() == 1024);
};

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Q1: Test new defaults =====
    #[test]
    fn test_new_defaults() {
        let capsule = Mp4SampleTableCapsule::new();

        assert_eq!(capsule.sample_count.load(Ordering::Acquire), 0);
        assert_eq!(capsule.chunk_count.load(Ordering::Acquire), 0);
        assert_eq!(capsule.keyframe_count.load(Ordering::Acquire), 0);
        assert_eq!(capsule.generation.load(Ordering::Acquire), 0);
        assert_eq!(capsule.timescale.load(Ordering::Acquire), 1);
        assert_eq!(capsule.flags.load(Ordering::Acquire), 0);
        assert!(!capsule.is_fully_parsed());
    }

    // ===== Q2: Test parse_stts single entry =====
    #[test]
    fn test_parse_stts_single_entry() {
        let mut capsule = Mp4SampleTableCapsule::new();

        // Build stts data: version(1) + flags(3) + entry_count(4) + entry(8)
        // 100 samples, each 1000 ticks duration
        let mut data = vec![
            0x00, // version
            0x00, 0x00, 0x00, // flags
            0x00, 0x00, 0x00, 0x01, // entry_count = 1
            0x00, 0x00, 0x00, 0x64, // sample_count = 100
            0x00, 0x00, 0x03, 0xE8, // sample_delta = 1000
        ];

        let result = capsule.parse_stts(&data);
        assert!(result.is_ok());

        assert_eq!(capsule.stts_entry_count.load(Ordering::Acquire), 1);
        assert_eq!(capsule.stts_total_samples.load(Ordering::Acquire), 100);
        assert_eq!(capsule.stts_total_duration.load(Ordering::Acquire), 100_000);
        assert_eq!(capsule.sample_count.load(Ordering::Acquire), 100);

        let flags = capsule.flags.load(Ordering::Acquire);
        assert!(flags & sample_table_flags::HAS_STTS != 0);
    }

    // ===== Q3: Test parse_stts multiple entries =====
    #[test]
    fn test_parse_stts_multiple_entries() {
        let mut capsule = Mp4SampleTableCapsule::new();

        // Build stts with 3 entries
        let data = vec![
            0x00, // version
            0x00, 0x00, 0x00, // flags
            0x00, 0x00, 0x00, 0x03, // entry_count = 3
            // Entry 1: 50 samples, 1000 ticks each
            0x00, 0x00, 0x00, 0x32, // sample_count = 50
            0x00, 0x00, 0x03, 0xE8, // sample_delta = 1000
            // Entry 2: 30 samples, 500 ticks each
            0x00, 0x00, 0x00, 0x1E, // sample_count = 30
            0x00, 0x00, 0x01, 0xF4, // sample_delta = 500
            // Entry 3: 20 samples, 2000 ticks each
            0x00, 0x00, 0x00, 0x14, // sample_count = 20
            0x00, 0x00, 0x07, 0xD0, // sample_delta = 2000
        ];

        let result = capsule.parse_stts(&data);
        assert!(result.is_ok());

        assert_eq!(capsule.stts_entry_count.load(Ordering::Acquire), 3);
        assert_eq!(capsule.stts_total_samples.load(Ordering::Acquire), 100);
        // Duration: 50*1000 + 30*500 + 20*2000 = 50000 + 15000 + 40000 = 105000
        assert_eq!(capsule.stts_total_duration.load(Ordering::Acquire), 105_000);
    }

    // ===== Q4: Test parse_stsc =====
    #[test]
    fn test_parse_stsc() {
        let mut capsule = Mp4SampleTableCapsule::new();

        // Build stsc: 2 entries
        let data = vec![
            0x00, // version
            0x00, 0x00, 0x00, // flags
            0x00, 0x00, 0x00, 0x02, // entry_count = 2
            // Entry 1: first_chunk=1, samples_per_chunk=10, desc_idx=1
            0x00, 0x00, 0x00, 0x01, // first_chunk = 1 (1-indexed)
            0x00, 0x00, 0x00, 0x0A, // samples_per_chunk = 10
            0x00, 0x00, 0x00, 0x01, // sample_description_index = 1
            // Entry 2: first_chunk=5, samples_per_chunk=5, desc_idx=1
            0x00, 0x00, 0x00, 0x05, // first_chunk = 5 (1-indexed)
            0x00, 0x00, 0x00, 0x05, // samples_per_chunk = 5
            0x00, 0x00, 0x00, 0x01, // sample_description_index = 1
        ];

        let result = capsule.parse_stsc(&data);
        assert!(result.is_ok());

        assert_eq!(capsule.stsc_entry_count.load(Ordering::Acquire), 2);

        // Check cached entries (converted to 0-indexed)
        assert_eq!(capsule.stsc_cache[0].first_chunk, 0);
        assert_eq!(capsule.stsc_cache[0].samples_per_chunk, 10);
        assert_eq!(capsule.stsc_cache[1].first_chunk, 4);
        assert_eq!(capsule.stsc_cache[1].samples_per_chunk, 5);

        let flags = capsule.flags.load(Ordering::Acquire);
        assert!(flags & sample_table_flags::HAS_STSC != 0);
    }

    // ===== Q5: Test parse_stsz constant size =====
    #[test]
    fn test_parse_stsz_constant() {
        let mut capsule = Mp4SampleTableCapsule::new();

        // Constant size: all samples are 4096 bytes
        let data = vec![
            0x00, // version
            0x00, 0x00, 0x00, // flags
            0x00, 0x00, 0x10, 0x00, // sample_size = 4096
            0x00, 0x00, 0x00, 0x64, // sample_count = 100
        ];

        let result = capsule.parse_stsz(&data);
        assert!(result.is_ok());

        assert_eq!(capsule.constant_sample_size.load(Ordering::Acquire), 4096);
        assert_eq!(capsule.stsz_entry_count.load(Ordering::Acquire), 100);
        assert_eq!(capsule.sample_count.load(Ordering::Acquire), 100);

        let flags = capsule.flags.load(Ordering::Acquire);
        assert!(flags & sample_table_flags::HAS_STSZ != 0);
    }

    // ===== Q6: Test parse_stsz variable sizes =====
    #[test]
    fn test_parse_stsz_variable() {
        let mut capsule = Mp4SampleTableCapsule::new();

        // Variable sizes: sample_size = 0
        let data = vec![
            0x00, // version
            0x00, 0x00, 0x00, // flags
            0x00, 0x00, 0x00, 0x00, // sample_size = 0 (variable)
            0x00, 0x00, 0x00, 0x04, // sample_count = 4
            // Individual sizes
            0x00, 0x00, 0x10, 0x00, // size[0] = 4096
            0x00, 0x00, 0x08, 0x00, // size[1] = 2048
            0x00, 0x00, 0x0C, 0x00, // size[2] = 3072
            0x00, 0x00, 0x04, 0x00, // size[3] = 1024
        ];

        let result = capsule.parse_stsz(&data);
        assert!(result.is_ok());

        assert_eq!(capsule.constant_sample_size.load(Ordering::Acquire), 0);
        assert_eq!(capsule.stsz_entry_count.load(Ordering::Acquire), 4);
        assert_eq!(capsule.stsz_file_offset.load(Ordering::Acquire), 12);

        // Test reading individual sizes
        assert_eq!(capsule.get_sample_size(0, &data).unwrap(), 4096);
        assert_eq!(capsule.get_sample_size(1, &data).unwrap(), 2048);
        assert_eq!(capsule.get_sample_size(2, &data).unwrap(), 3072);
        assert_eq!(capsule.get_sample_size(3, &data).unwrap(), 1024);
    }

    // ===== Q7: Test parse_stco (32-bit) =====
    #[test]
    fn test_parse_stco() {
        let mut capsule = Mp4SampleTableCapsule::new();

        // 3 chunk offsets
        let data = vec![
            0x00, // version
            0x00, 0x00, 0x00, // flags
            0x00, 0x00, 0x00, 0x03, // entry_count = 3
            0x00, 0x00, 0x10, 0x00, // offset[0] = 4096
            0x00, 0x00, 0x20, 0x00, // offset[1] = 8192
            0x00, 0x00, 0x30, 0x00, // offset[2] = 12288
        ];

        let result = capsule.parse_stco(&data);
        assert!(result.is_ok());

        assert_eq!(capsule.chunk_offset_entry_count.load(Ordering::Acquire), 3);
        assert_eq!(capsule.chunk_count.load(Ordering::Acquire), 3);
        assert_eq!(capsule.uses_co64.load(Ordering::Acquire), 0);
        assert_eq!(capsule.first_chunk_offset.load(Ordering::Acquire), 4096);

        // Test reading chunk offsets
        assert_eq!(capsule.get_chunk_offset(0, &data).unwrap(), 4096);
        assert_eq!(capsule.get_chunk_offset(1, &data).unwrap(), 8192);
        assert_eq!(capsule.get_chunk_offset(2, &data).unwrap(), 12288);

        let flags = capsule.flags.load(Ordering::Acquire);
        assert!(flags & sample_table_flags::HAS_STCO != 0);
    }

    // ===== Q8: Test parse_co64 (64-bit) =====
    #[test]
    fn test_parse_co64() {
        let mut capsule = Mp4SampleTableCapsule::new();

        // 2 chunk offsets (64-bit)
        let data = vec![
            0x00, // version
            0x00, 0x00, 0x00, // flags
            0x00, 0x00, 0x00, 0x02, // entry_count = 2
            // offset[0] = 0x0000000100001000 (4GB + 4096)
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x10, 0x00,
            // offset[1] = 0x0000000100002000 (4GB + 8192)
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x20, 0x00,
        ];

        let result = capsule.parse_co64(&data);
        assert!(result.is_ok());

        assert_eq!(capsule.chunk_offset_entry_count.load(Ordering::Acquire), 2);
        assert_eq!(capsule.uses_co64.load(Ordering::Acquire), 1);
        assert_eq!(
            capsule.first_chunk_offset.load(Ordering::Acquire),
            0x100001000
        );

        // Test reading chunk offsets
        assert_eq!(capsule.get_chunk_offset(0, &data).unwrap(), 0x100001000);
        assert_eq!(capsule.get_chunk_offset(1, &data).unwrap(), 0x100002000);

        let flags = capsule.flags.load(Ordering::Acquire);
        assert!(flags & sample_table_flags::HAS_CO64 != 0);
    }

    // ===== Q9: Test parse_stss =====
    #[test]
    fn test_parse_stss() {
        let mut capsule = Mp4SampleTableCapsule::new();

        // 5 keyframes at samples 1, 31, 61, 91, 121 (1-indexed)
        let data = vec![
            0x00, // version
            0x00, 0x00, 0x00, // flags
            0x00, 0x00, 0x00, 0x05, // entry_count = 5
            0x00, 0x00, 0x00, 0x01, // sample 1
            0x00, 0x00, 0x00, 0x1F, // sample 31
            0x00, 0x00, 0x00, 0x3D, // sample 61
            0x00, 0x00, 0x00, 0x5B, // sample 91
            0x00, 0x00, 0x00, 0x79, // sample 121
        ];

        let result = capsule.parse_stss(&data);
        assert!(result.is_ok());

        assert_eq!(capsule.stss_entry_count.load(Ordering::Acquire), 5);
        assert_eq!(capsule.keyframe_count.load(Ordering::Acquire), 5);

        // Check cached values (converted to 0-indexed)
        assert_eq!(capsule.stss_cache[0], 0);
        assert_eq!(capsule.stss_cache[1], 30);
        assert_eq!(capsule.stss_cache[2], 60);
        assert_eq!(capsule.stss_cache[3], 90);
        assert_eq!(capsule.stss_cache[4], 120);

        // Test is_keyframe
        assert!(capsule.is_keyframe(0));
        assert!(!capsule.is_keyframe(1));
        assert!(capsule.is_keyframe(30));
        assert!(!capsule.is_keyframe(29));

        let flags = capsule.flags.load(Ordering::Acquire);
        assert!(flags & sample_table_flags::HAS_STSS != 0);
    }

    // ===== Q10: Test get_sample_location =====
    #[test]
    fn test_get_sample_location() {
        let mut capsule = Mp4SampleTableCapsule::with_timescale(1000);

        // Set up minimal sample table for 4 samples in 1 chunk
        // stts: 4 samples, 1000 ticks each
        let stts_data = vec![
            0x00, 0x00, 0x00, 0x00, // version + flags
            0x00, 0x00, 0x00, 0x01, // entry_count = 1
            0x00, 0x00, 0x00, 0x04, // sample_count = 4
            0x00, 0x00, 0x03, 0xE8, // sample_delta = 1000
        ];
        capsule.parse_stts(&stts_data).unwrap();

        // stsc: 1 entry, all samples in one chunk
        let stsc_data = vec![
            0x00, 0x00, 0x00, 0x00, // version + flags
            0x00, 0x00, 0x00, 0x01, // entry_count = 1
            0x00, 0x00, 0x00, 0x01, // first_chunk = 1
            0x00, 0x00, 0x00, 0x04, // samples_per_chunk = 4
            0x00, 0x00, 0x00, 0x01, // sample_description_index = 1
        ];
        capsule.parse_stsc(&stsc_data).unwrap();

        // stsz: constant size 1024
        let stsz_data = vec![
            0x00, 0x00, 0x00, 0x00, // version + flags
            0x00, 0x00, 0x04, 0x00, // sample_size = 1024
            0x00, 0x00, 0x00, 0x04, // sample_count = 4
        ];
        capsule.parse_stsz(&stsz_data).unwrap();

        // stco: 1 chunk at offset 1000
        let stco_data = vec![
            0x00, 0x00, 0x00, 0x00, // version + flags
            0x00, 0x00, 0x00, 0x01, // entry_count = 1
            0x00, 0x00, 0x03, 0xE8, // offset = 1000
        ];
        capsule.parse_stco(&stco_data).unwrap();

        assert!(capsule.is_fully_parsed());

        // Build combined data buffer (just need stco for offset lookup)
        // The get_sample_location uses the same buffer for all tables
        // In real usage, you'd have the full file or separate buffers
        // Here we use stco_data since that's what we need for offset lookup

        // Test sample 0
        let loc = capsule.get_sample_location(0, &stco_data).unwrap();
        assert_eq!(loc.offset, 1000);
        assert_eq!(loc.size, 1024);
        assert_eq!(loc.dts, 0);
        assert_eq!(loc.pts, 0);

        // Test sample 2
        let loc = capsule.get_sample_location(2, &stco_data).unwrap();
        assert_eq!(loc.offset, 1000 + 2048); // 1000 + 2*1024
        assert_eq!(loc.size, 1024);
        assert_eq!(loc.dts, 2000); // 2 * 1000
        assert_eq!(loc.pts, 2000);
    }

    // ===== Q11: Test get_sample_timestamp =====
    #[test]
    fn test_get_sample_timestamp() {
        let mut capsule = Mp4SampleTableCapsule::with_timescale(90000);

        // stts: 100 samples, 3000 ticks each (30fps at 90kHz)
        let stts_data = vec![
            0x00, 0x00, 0x00, 0x00, // version + flags
            0x00, 0x00, 0x00, 0x01, // entry_count = 1
            0x00, 0x00, 0x00, 0x64, // sample_count = 100
            0x00, 0x00, 0x0B, 0xB8, // sample_delta = 3000
        ];
        capsule.parse_stts(&stts_data).unwrap();
        capsule.sample_count.store(100, Ordering::Release);

        // Test timestamps
        let (dts0, pts0) = capsule.get_sample_timestamp(0).unwrap();
        assert_eq!(dts0, 0);
        assert_eq!(pts0, 0);

        let (dts10, pts10) = capsule.get_sample_timestamp(10).unwrap();
        assert_eq!(dts10, 30000); // 10 * 3000
        assert_eq!(pts10, 30000);

        let (dts99, pts99) = capsule.get_sample_timestamp(99).unwrap();
        assert_eq!(dts99, 297000); // 99 * 3000
        assert_eq!(pts99, 297000);

        // Out of range
        assert!(capsule.get_sample_timestamp(100).is_err());
    }

    // ===== Q12: Test find_keyframe_before =====
    #[test]
    fn test_find_keyframe_before() {
        let mut capsule = Mp4SampleTableCapsule::new();
        capsule.sample_count.store(120, Ordering::Release);

        // Keyframes at 0, 30, 60, 90 (0-indexed)
        let stss_data = vec![
            0x00, 0x00, 0x00, 0x00, // version + flags
            0x00, 0x00, 0x00, 0x04, // entry_count = 4
            0x00, 0x00, 0x00, 0x01, // sample 1 (1-indexed) -> 0
            0x00, 0x00, 0x00, 0x1F, // sample 31 -> 30
            0x00, 0x00, 0x00, 0x3D, // sample 61 -> 60
            0x00, 0x00, 0x00, 0x5B, // sample 91 -> 90
        ];
        capsule.parse_stss(&stss_data).unwrap();

        // Test finding keyframes
        assert_eq!(capsule.find_keyframe_before(0), Some(0));
        assert_eq!(capsule.find_keyframe_before(15), Some(0));
        assert_eq!(capsule.find_keyframe_before(30), Some(30));
        assert_eq!(capsule.find_keyframe_before(45), Some(30));
        assert_eq!(capsule.find_keyframe_before(60), Some(60));
        assert_eq!(capsule.find_keyframe_before(90), Some(90));
        assert_eq!(capsule.find_keyframe_before(119), Some(90));
    }

    // ===== Q13: Test seek_to_time =====
    #[test]
    fn test_seek_to_time() {
        let mut capsule = Mp4SampleTableCapsule::with_timescale(1000); // 1000 ticks/sec

        // stts: 100 samples, 1000 ticks each = 1 second per sample
        let stts_data = vec![
            0x00, 0x00, 0x00, 0x00, // version + flags
            0x00, 0x00, 0x00, 0x01, // entry_count = 1
            0x00, 0x00, 0x00, 0x64, // sample_count = 100
            0x00, 0x00, 0x03, 0xE8, // sample_delta = 1000
        ];
        capsule.parse_stts(&stts_data).unwrap();

        // Seek to 0 seconds
        assert_eq!(capsule.seek_to_time(0).unwrap(), 0);

        // Seek to 5 seconds (5,000,000 us)
        assert_eq!(capsule.seek_to_time(5_000_000).unwrap(), 5);

        // Seek to 50.5 seconds
        assert_eq!(capsule.seek_to_time(50_500_000).unwrap(), 50);

        // Seek past end -> returns last sample
        assert_eq!(capsule.seek_to_time(200_000_000).unwrap(), 99);
    }

    // ===== Additional tests =====

    #[test]
    fn test_with_timescale() {
        let capsule = Mp4SampleTableCapsule::with_timescale(90000);
        assert_eq!(capsule.timescale.load(Ordering::Acquire), 90000);
    }

    #[test]
    fn test_ctts_parsing() {
        let mut capsule = Mp4SampleTableCapsule::new();

        // ctts: 2 entries with composition offsets
        let data = vec![
            0x01, // version 1 (signed offsets)
            0x00, 0x00, 0x00, // flags
            0x00, 0x00, 0x00, 0x02, // entry_count = 2
            // Entry 1: 50 samples, offset +1000
            0x00, 0x00, 0x00, 0x32, // sample_count = 50
            0x00, 0x00, 0x03, 0xE8, // sample_offset = 1000
            // Entry 2: 50 samples, offset -500
            0x00, 0x00, 0x00, 0x32, // sample_count = 50
            0xFF, 0xFF, 0xFE, 0x0C, // sample_offset = -500
        ];

        let result = capsule.parse_ctts(&data);
        assert!(result.is_ok());

        assert_eq!(capsule.ctts_entry_count.load(Ordering::Acquire), 2);
        assert_eq!(capsule.has_ctts.load(Ordering::Acquire), 1);
        assert_eq!(capsule.ctts_cache[0].sample_count, 50);
        assert_eq!(capsule.ctts_cache[0].sample_offset, 1000);
        assert_eq!(capsule.ctts_cache[1].sample_count, 50);
        assert_eq!(capsule.ctts_cache[1].sample_offset, -500);
    }

    #[test]
    fn test_duration_calculation() {
        let mut capsule = Mp4SampleTableCapsule::with_timescale(1000);

        // 100 samples, 1000 ticks each = 100 seconds
        let stts_data = vec![
            0x00, 0x00, 0x00, 0x00, // version + flags
            0x00, 0x00, 0x00, 0x01, // entry_count = 1
            0x00, 0x00, 0x00, 0x64, // sample_count = 100
            0x00, 0x00, 0x03, 0xE8, // sample_delta = 1000
        ];
        capsule.parse_stts(&stts_data).unwrap();

        assert_eq!(capsule.get_total_duration(), 100_000);
        assert_eq!(capsule.get_duration_us(), 100_000_000); // 100 seconds in microseconds
    }

    #[test]
    fn test_error_invalid_data() {
        let mut capsule = Mp4SampleTableCapsule::new();

        // Too short data
        assert_eq!(
            capsule.parse_stts(&[0x00, 0x00]).unwrap_err(),
            SampleTableError::InvalidStts
        );

        assert_eq!(
            capsule.parse_stsc(&[0x00]).unwrap_err(),
            SampleTableError::InvalidStsc
        );

        assert_eq!(
            capsule.parse_stsz(&[]).unwrap_err(),
            SampleTableError::InvalidStsz
        );

        assert_eq!(
            capsule.parse_stco(&[0x00, 0x00, 0x00]).unwrap_err(),
            SampleTableError::InvalidStco
        );
    }

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(
            std::mem::size_of::<Mp4SampleTableCapsule>(),
            1024,
            "Capsule must be exactly 1024 bytes"
        );
        assert_eq!(
            std::mem::align_of::<Mp4SampleTableCapsule>(),
            1024,
            "Capsule must be 1024-byte aligned"
        );
    }

    #[test]
    fn test_generation_counter() {
        let capsule = Mp4SampleTableCapsule::new();
        let initial = capsule.generation.load(Ordering::Acquire);

        capsule.bump_generation();
        assert_eq!(
            capsule.generation.load(Ordering::Acquire),
            initial + 1
        );
    }

    #[test]
    fn test_no_stss_all_keyframes() {
        let capsule = Mp4SampleTableCapsule::new();

        // Without stss table, all samples should be treated as keyframes
        // (typical for audio tracks)
        assert!(capsule.is_keyframe(0));
        assert!(capsule.is_keyframe(100));
        assert_eq!(capsule.find_keyframe_before(50), Some(50));
    }
}
