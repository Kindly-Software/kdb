//! Checkpoint file format for kindly-av1
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Binary format with header, frame index, and trailer for crash-safe
//! encoding state persistence with two-phase commit.
//!
//! ## File Layout
//!
//! ```text
//! +------------------+  0
//! | CheckpointHeader |  128 bytes
//! +------------------+  128
//! | FrameIndexEntry  |  32 bytes each
//! | FrameIndexEntry  |
//! | ...              |
//! +------------------+  128 + (32 * frame_count)
//! | CheckpointTrailer|  32 bytes
//! +------------------+  EOF
//! ```
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T9 Persistent tier
//! - **Chaos**: Fixed-size structures, cache-aligned
//! - **ASSUM**: All unsafe documented with #ASSUME/#VERIFY

use std::time::{SystemTime, UNIX_EPOCH};

/// Magic bytes: "KDLYCKPT"
pub const CHECKPOINT_MAGIC: [u8; 8] = *b"KDLYCKPT";

/// Checkpoint file version
pub const CHECKPOINT_VERSION: u32 = 1;

/// Header size in bytes
pub const HEADER_SIZE: usize = 128;

/// Frame index entry size in bytes
pub const FRAME_ENTRY_SIZE: usize = 32;

/// Trailer size in bytes
pub const TRAILER_SIZE: usize = 32;

/// Checkpoint header (128 bytes)
///
/// Contains metadata about the encoding session and validation hashes.
///
/// ## Memory Layout
/// - 8 bytes: Magic "KDLYCKPT"
/// - 4 bytes: Version
/// - 4 bytes: Reserved
/// - 32 bytes: Input file hash (Blake3, first 1MB)
/// - 8 bytes: Total frames
/// - 8 bytes: Completed frames
/// - 32 bytes: Encoder config hash
/// - 8 bytes: Output file size
/// - 8 bytes: Timestamp (Unix epoch seconds)
/// - 16 bytes: Padding
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct CheckpointHeader {
    /// Magic bytes "KDLYCKPT"
    pub magic: [u8; 8],
    /// Format version
    pub version: u32,
    /// Reserved for future use
    pub _reserved: u32,
    /// Blake3 hash of input file (first 1MB)
    pub input_hash: [u8; 32],
    /// Total frames in video
    pub total_frames: u64,
    /// Last completed frame
    pub completed_frames: u64,
    /// Encoder config hash (for validation)
    pub config_hash: [u8; 32],
    /// Output file size at checkpoint
    pub output_size: u64,
    /// Timestamp of checkpoint (Unix epoch seconds)
    pub timestamp: u64,
    /// Padding to 128 bytes
    pub _padding: [u8; 16],
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<CheckpointHeader>() == 128);
const _: () = assert!(core::mem::align_of::<CheckpointHeader>() == 8);

impl CheckpointHeader {
    /// Create new header with metadata
    ///
    /// # Arguments
    /// * `input_hash` - Blake3 hash of input file (first 1MB)
    /// * `total_frames` - Total number of frames to encode
    /// * `config_hash` - Hash of encoder configuration
    pub fn new(input_hash: [u8; 32], total_frames: u64, config_hash: [u8; 32]) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            magic: CHECKPOINT_MAGIC,
            version: CHECKPOINT_VERSION,
            _reserved: 0,
            input_hash,
            total_frames,
            completed_frames: 0,
            config_hash,
            output_size: 0,
            timestamp,
            _padding: [0u8; 16],
        }
    }

    /// Validate magic bytes and version
    #[inline]
    pub fn validate(&self) -> bool {
        self.magic == CHECKPOINT_MAGIC && self.version == CHECKPOINT_VERSION
    }

    /// Convert to bytes for writing
    ///
    /// # Safety
    /// #ASSUME: Self is repr(C) with no padding holes that contain
    /// uninitialized memory. All fields are initialized.
    /// #VERIFY: Compile-time size assertion ensures layout is correct.
    #[inline]
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut bytes = [0u8; HEADER_SIZE];

        // Manual serialization for safety (no unsafe transmute)
        bytes[0..8].copy_from_slice(&self.magic);
        bytes[8..12].copy_from_slice(&self.version.to_le_bytes());
        bytes[12..16].copy_from_slice(&self._reserved.to_le_bytes());
        bytes[16..48].copy_from_slice(&self.input_hash);
        bytes[48..56].copy_from_slice(&self.total_frames.to_le_bytes());
        bytes[56..64].copy_from_slice(&self.completed_frames.to_le_bytes());
        bytes[64..96].copy_from_slice(&self.config_hash);
        bytes[96..104].copy_from_slice(&self.output_size.to_le_bytes());
        bytes[104..112].copy_from_slice(&self.timestamp.to_le_bytes());
        bytes[112..128].copy_from_slice(&self._padding);

        bytes
    }

    /// Parse from bytes
    ///
    /// Returns `None` if magic/version validation fails.
    #[inline]
    pub fn from_bytes(bytes: &[u8; HEADER_SIZE]) -> Option<Self> {
        let mut magic = [0u8; 8];
        magic.copy_from_slice(&bytes[0..8]);

        let version = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);

        // Early validation
        if magic != CHECKPOINT_MAGIC || version != CHECKPOINT_VERSION {
            return None;
        }

        let _reserved = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);

        let mut input_hash = [0u8; 32];
        input_hash.copy_from_slice(&bytes[16..48]);

        let total_frames = u64::from_le_bytes([
            bytes[48], bytes[49], bytes[50], bytes[51],
            bytes[52], bytes[53], bytes[54], bytes[55],
        ]);

        let completed_frames = u64::from_le_bytes([
            bytes[56], bytes[57], bytes[58], bytes[59],
            bytes[60], bytes[61], bytes[62], bytes[63],
        ]);

        let mut config_hash = [0u8; 32];
        config_hash.copy_from_slice(&bytes[64..96]);

        let output_size = u64::from_le_bytes([
            bytes[96], bytes[97], bytes[98], bytes[99],
            bytes[100], bytes[101], bytes[102], bytes[103],
        ]);

        let timestamp = u64::from_le_bytes([
            bytes[104], bytes[105], bytes[106], bytes[107],
            bytes[108], bytes[109], bytes[110], bytes[111],
        ]);

        let mut _padding = [0u8; 16];
        _padding.copy_from_slice(&bytes[112..128]);

        Some(Self {
            magic,
            version,
            _reserved,
            input_hash,
            total_frames,
            completed_frames,
            config_hash,
            output_size,
            timestamp,
            _padding,
        })
    }

    /// Update progress fields
    #[inline]
    pub fn update_progress(&mut self, completed_frames: u64, output_size: u64) {
        self.completed_frames = completed_frames;
        self.output_size = output_size;
        self.timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
    }
}

impl Default for CheckpointHeader {
    fn default() -> Self {
        Self {
            magic: CHECKPOINT_MAGIC,
            version: CHECKPOINT_VERSION,
            _reserved: 0,
            input_hash: [0u8; 32],
            total_frames: 0,
            completed_frames: 0,
            config_hash: [0u8; 32],
            output_size: 0,
            timestamp: 0,
            _padding: [0u8; 16],
        }
    }
}

impl core::fmt::Debug for CheckpointHeader {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CheckpointHeader")
            .field("magic", &core::str::from_utf8(&self.magic).unwrap_or("invalid"))
            .field("version", &self.version)
            .field("total_frames", &self.total_frames)
            .field("completed_frames", &self.completed_frames)
            .field("output_size", &self.output_size)
            .field("timestamp", &self.timestamp)
            .finish()
    }
}

/// Frame index entry (32 bytes)
///
/// Records the location and metadata of each encoded frame in the output.
/// Used for resuming encoding and validating partial output.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameIndexEntry {
    /// Frame number (0-indexed)
    pub frame_num: u64,
    /// Byte offset in output file where frame data begins
    pub output_offset: u64,
    /// Size of encoded frame data in bytes
    pub encoded_size: u64,
    /// PSNR quality metric (Q16.16 fixed-point, 0 if not calculated)
    pub psnr_q16: u64,
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<FrameIndexEntry>() == 32);

impl FrameIndexEntry {
    /// Create new frame index entry
    #[inline]
    pub const fn new(frame_num: u64, output_offset: u64, encoded_size: u64) -> Self {
        Self {
            frame_num,
            output_offset,
            encoded_size,
            psnr_q16: 0,
        }
    }

    /// Create with PSNR quality metric
    #[inline]
    pub const fn with_psnr(mut self, psnr: f64) -> Self {
        // Convert f64 to Q16.16 fixed-point
        // PSNR typically ranges 20-60 dB
        self.psnr_q16 = (psnr * 65536.0) as u64;
        self
    }

    /// Get PSNR as f64 (0.0 if not set)
    #[inline]
    pub fn psnr(&self) -> f64 {
        self.psnr_q16 as f64 / 65536.0
    }

    /// Convert to bytes for writing
    #[inline]
    pub fn to_bytes(&self) -> [u8; FRAME_ENTRY_SIZE] {
        let mut bytes = [0u8; FRAME_ENTRY_SIZE];
        bytes[0..8].copy_from_slice(&self.frame_num.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.output_offset.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.encoded_size.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.psnr_q16.to_le_bytes());
        bytes
    }

    /// Parse from bytes
    #[inline]
    pub fn from_bytes(bytes: &[u8; FRAME_ENTRY_SIZE]) -> Self {
        Self {
            frame_num: u64::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3],
                bytes[4], bytes[5], bytes[6], bytes[7],
            ]),
            output_offset: u64::from_le_bytes([
                bytes[8], bytes[9], bytes[10], bytes[11],
                bytes[12], bytes[13], bytes[14], bytes[15],
            ]),
            encoded_size: u64::from_le_bytes([
                bytes[16], bytes[17], bytes[18], bytes[19],
                bytes[20], bytes[21], bytes[22], bytes[23],
            ]),
            psnr_q16: u64::from_le_bytes([
                bytes[24], bytes[25], bytes[26], bytes[27],
                bytes[28], bytes[29], bytes[30], bytes[31],
            ]),
        }
    }
}

/// Checkpoint trailer (32 bytes)
///
/// Contains CRC for integrity verification and generation counter for
/// two-phase commit protocol.
///
/// ## Two-Phase Commit Protocol
///
/// 1. Begin checkpoint: generation becomes ODD (inflight transaction)
/// 2. Write checkpoint data (header + frame index)
/// 3. Commit checkpoint: generation becomes EVEN (committed)
/// 4. On crash recovery: if ODD, rollback to last EVEN state
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug)]
pub struct CheckpointTrailer {
    /// CRC32 of header + all frame entries
    pub crc32: u32,
    /// Reserved for future use
    pub _reserved: u32,
    /// Generation counter (odd = inflight, even = committed)
    pub generation: u64,
    /// Commit flag (1 = committed, 0 = incomplete)
    pub committed: u8,
    /// Padding to 32 bytes
    pub _padding: [u8; 15],
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<CheckpointTrailer>() == 32);
const _: () = assert!(core::mem::align_of::<CheckpointTrailer>() == 8);

impl CheckpointTrailer {
    /// Create new trailer (uncommitted, generation 0)
    #[inline]
    pub const fn new() -> Self {
        Self {
            crc32: 0,
            _reserved: 0,
            generation: 0,
            committed: 0,
            _padding: [0u8; 15],
        }
    }

    /// Create committed trailer with CRC
    #[inline]
    pub const fn committed(crc32: u32, generation: u64) -> Self {
        Self {
            crc32,
            _reserved: 0,
            generation,
            committed: 1,
            _padding: [0u8; 15],
        }
    }

    /// Check if checkpoint is valid (generation is EVEN and committed)
    #[inline]
    pub const fn is_valid(&self) -> bool {
        self.generation % 2 == 0 && self.committed == 1
    }

    /// Check if checkpoint is in-flight (generation is ODD)
    #[inline]
    pub const fn is_inflight(&self) -> bool {
        self.generation % 2 == 1
    }

    /// Convert to bytes for writing
    #[inline]
    pub fn to_bytes(&self) -> [u8; TRAILER_SIZE] {
        let mut bytes = [0u8; TRAILER_SIZE];
        bytes[0..4].copy_from_slice(&self.crc32.to_le_bytes());
        bytes[4..8].copy_from_slice(&self._reserved.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.generation.to_le_bytes());
        bytes[16] = self.committed;
        bytes[17..32].copy_from_slice(&self._padding);
        bytes
    }

    /// Parse from bytes
    #[inline]
    pub fn from_bytes(bytes: &[u8; TRAILER_SIZE]) -> Self {
        let mut _padding = [0u8; 15];
        _padding.copy_from_slice(&bytes[17..32]);

        Self {
            crc32: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            _reserved: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            generation: u64::from_le_bytes([
                bytes[8], bytes[9], bytes[10], bytes[11],
                bytes[12], bytes[13], bytes[14], bytes[15],
            ]),
            committed: bytes[16],
            _padding,
        }
    }
}

impl Default for CheckpointTrailer {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate CRC32 checksum for checkpoint data
///
/// Computes CRC32 over header and all frame index entries using IEEE 802.3
/// CRC-32 polynomial (0xEDB88320).
///
/// # Framework Compliance
/// - **UCE34**: Q10 T0 Auditable tier (deterministic, no external deps)
/// - **Chaos**: Pure function, no state, cache-friendly lookup table
/// - **Zero Dependencies**: Inline implementation eliminates crc32fast dependency
///
/// # Implementation
///
/// Uses const-evaluated 256-entry lookup table for single-pass CRC calculation.
/// Table is computed at compile time, resulting in zero runtime overhead for
/// initialization.
#[inline]
pub fn calculate_crc32(header: &CheckpointHeader, entries: &[FrameIndexEntry]) -> u32 {
    /// IEEE 802.3 CRC-32 lookup table (const-evaluated at compile time)
    ///
    /// This table is computed once at compile time using const evaluation,
    /// eliminating runtime initialization overhead.
    const CRC32_TABLE: [u32; 256] = {
        let mut table = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut crc = i as u32;
            let mut j = 0;
            while j < 8 {
                crc = if crc & 1 != 0 {
                    0xEDB88320 ^ (crc >> 1)
                } else {
                    crc >> 1
                };
                j += 1;
            }
            table[i] = crc;
            i += 1;
        }
        table
    };

    let mut crc = 0xFFFFFFFF_u32;

    // Hash header
    for &byte in &header.to_bytes() {
        crc = CRC32_TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
    }

    // Hash all entries
    for entry in entries {
        for &byte in &entry.to_bytes() {
            crc = CRC32_TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
        }
    }

    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_size_and_alignment() {
        assert_eq!(core::mem::size_of::<CheckpointHeader>(), 128);
        assert_eq!(core::mem::align_of::<CheckpointHeader>(), 8);
    }

    #[test]
    fn test_frame_entry_size() {
        assert_eq!(core::mem::size_of::<FrameIndexEntry>(), 32);
    }

    #[test]
    fn test_trailer_size_and_alignment() {
        assert_eq!(core::mem::size_of::<CheckpointTrailer>(), 32);
        assert_eq!(core::mem::align_of::<CheckpointTrailer>(), 8);
    }

    #[test]
    fn test_header_magic_validation() {
        let header = CheckpointHeader::new([0u8; 32], 1000, [0u8; 32]);
        assert!(header.validate());

        let mut invalid = header;
        invalid.magic = *b"BADMAGIC";
        assert!(!invalid.validate());
    }

    #[test]
    fn test_header_roundtrip() {
        let input_hash = [0xABu8; 32];
        let config_hash = [0xCDu8; 32];
        let mut original = CheckpointHeader::new(input_hash, 1000, config_hash);
        original.update_progress(500, 1024 * 1024);

        let bytes = original.to_bytes();
        let parsed = CheckpointHeader::from_bytes(&bytes).expect("parse failed");

        assert_eq!(parsed.magic, CHECKPOINT_MAGIC);
        assert_eq!(parsed.version, CHECKPOINT_VERSION);
        assert_eq!(parsed.input_hash, input_hash);
        assert_eq!(parsed.total_frames, 1000);
        assert_eq!(parsed.completed_frames, 500);
        assert_eq!(parsed.config_hash, config_hash);
        assert_eq!(parsed.output_size, 1024 * 1024);
    }

    #[test]
    fn test_frame_entry_roundtrip() {
        let entry = FrameIndexEntry::new(42, 1024, 512).with_psnr(45.5);

        let bytes = entry.to_bytes();
        let parsed = FrameIndexEntry::from_bytes(&bytes);

        assert_eq!(parsed.frame_num, 42);
        assert_eq!(parsed.output_offset, 1024);
        assert_eq!(parsed.encoded_size, 512);
        assert!((parsed.psnr() - 45.5).abs() < 0.001);
    }

    #[test]
    fn test_trailer_two_phase_commit() {
        // Initial state: generation 0 (even), uncommitted
        let trailer = CheckpointTrailer::new();
        assert!(!trailer.is_valid()); // uncommitted
        assert!(!trailer.is_inflight()); // even generation

        // Inflight state: generation 1 (odd)
        let inflight = CheckpointTrailer {
            generation: 1,
            ..Default::default()
        };
        assert!(inflight.is_inflight());
        assert!(!inflight.is_valid());

        // Committed state: generation 2 (even), committed
        let committed = CheckpointTrailer::committed(0x12345678, 2);
        assert!(committed.is_valid());
        assert!(!committed.is_inflight());
    }

    #[test]
    fn test_trailer_roundtrip() {
        let original = CheckpointTrailer::committed(0xDEADBEEF, 4);

        let bytes = original.to_bytes();
        let parsed = CheckpointTrailer::from_bytes(&bytes);

        assert_eq!(parsed.crc32, 0xDEADBEEF);
        assert_eq!(parsed.generation, 4);
        assert_eq!(parsed.committed, 1);
        assert!(parsed.is_valid());
    }

    #[test]
    fn test_crc32_calculation() {
        let header = CheckpointHeader::new([0xABu8; 32], 100, [0xCDu8; 32]);
        let entries = vec![
            FrameIndexEntry::new(0, 0, 1000),
            FrameIndexEntry::new(1, 1000, 1200),
            FrameIndexEntry::new(2, 2200, 800),
        ];

        let crc1 = calculate_crc32(&header, &entries);
        let crc2 = calculate_crc32(&header, &entries);

        // Same input produces same CRC
        assert_eq!(crc1, crc2);

        // Different input produces different CRC
        let different_entries = vec![FrameIndexEntry::new(0, 0, 999)];
        let crc3 = calculate_crc32(&header, &different_entries);
        assert_ne!(crc1, crc3);
    }

    #[test]
    fn test_psnr_fixed_point_precision() {
        // Test various PSNR values for Q16.16 precision
        for psnr in [20.0, 30.5, 40.123, 50.0, 60.999] {
            let entry = FrameIndexEntry::new(0, 0, 0).with_psnr(psnr);
            let recovered = entry.psnr();
            assert!((recovered - psnr).abs() < 0.001,
                "PSNR {psnr} roundtrip failed: got {recovered}");
        }
    }
}
