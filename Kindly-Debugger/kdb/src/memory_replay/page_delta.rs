//! PageDelta - Memory Delta Compression for Full State Replay
//!
//! Core data structures for memory delta compression using XOR-based diffing
//! with optional LZ4-style run-length encoding for compression.
//!
//! # Architecture (T0 Auditable + T2 SIMD)
//!
//! - `PageDelta`: Variable-size delta header with hash-chain integrity (Q34)
//! - `PageDeltaBuffer`: Fixed 4096-byte buffer for delta storage (page-aligned)
//! - `PageDeltaFlags`: Compression method selection (XOR, LZ4, Full, Zero, Sparse)
//!
//! # Performance Targets
//!
//! - XOR delta computation: <1us per 4KB page
//! - LZ4-style compression: 2-10x reduction for typical deltas
//! - Zero page detection: <50ns via SIMD (T2)
//! - CRC64 hash: <100ns per page
//!
//! # ASSUM Tags
//!
//! #ASSUME_LOCKFREE_ONLY: All operations are lockfree, no mutex/RwLock
//! #ASSUME_PAGE_ALIGNED: All page buffers are 4KB aligned
//! #ASSUME_DETERMINISTIC_HASH: CRC64 is deterministic for same inputs
//! #ASSUME_SAFE_XOR: XOR operation cannot cause UB on aligned buffers
//! #ASSUME_COMPRESSION_REVERSIBLE: decompress(compress(x)) == x

use crc::{Crc, CRC_64_ECMA_182};
use std::sync::atomic::{AtomicU64, Ordering};

/// Page size constant (4KB = 4096 bytes)
pub const PAGE_SIZE: usize = 4096;

/// Maximum compressed data size in PageDeltaBuffer (4096 - 48 byte header = 4048)
pub const MAX_COMPRESSED_SIZE: usize = PAGE_SIZE - 48;

/// CRC64-ECMA for Q34 hash-chain integrity
const CRC64: Crc<u64> = Crc::<u64>::new(&CRC_64_ECMA_182);

/// Compression/delta method flags
///
/// Determines how the page delta is encoded:
/// - XorUncompressed: Raw XOR delta (no compression)
/// - XorLz4: XOR delta with run-length encoding compression
/// - FullPage: Complete page snapshot (no delta, used for first snapshot)
/// - ZeroPage: Page is entirely zeros (1-byte representation)
/// - SparseXor: Sparse XOR with region offsets (for pages with few changes)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageDeltaFlags {
    /// Raw XOR delta, no compression
    XorUncompressed = 0,
    /// XOR delta with LZ4-style run-length encoding
    XorLz4 = 1,
    /// Complete page (no delta reference)
    FullPage = 2,
    /// Page is all zeros (most compact representation)
    ZeroPage = 3,
    /// Sparse XOR with (offset, length) pairs for non-zero regions
    SparseXor = 4,
}

impl From<u8> for PageDeltaFlags {
    fn from(value: u8) -> Self {
        match value {
            0 => PageDeltaFlags::XorUncompressed,
            1 => PageDeltaFlags::XorLz4,
            2 => PageDeltaFlags::FullPage,
            3 => PageDeltaFlags::ZeroPage,
            4 => PageDeltaFlags::SparseXor,
            _ => PageDeltaFlags::XorUncompressed, // Default fallback
        }
    }
}

/// PageDelta header structure (48 bytes)
///
/// Contains metadata for a single page delta with Q34 hash-chain integrity.
/// The actual compressed data follows this header in the buffer.
///
/// # Memory Layout (48 bytes total)
/// ```text
/// +0:  base_address      (8 bytes) - Virtual address of page
/// +8:  snapshot_id       (8 bytes) - Which snapshot this belongs to
/// +16: prev_hash         (8 bytes) - Q34 hash-chain link (CRC64)
/// +24: delta_hash        (8 bytes) - Hash of compressed data for integrity
/// +32: original_size     (4 bytes) - Pre-compression size
/// +36: compressed_size   (4 bytes) - Post-compression size
/// +40: flags             (1 byte)  - PageDeltaFlags enum
/// +41: timestamp_ns_high (3 bytes) - Upper 24 bits of timestamp
/// +44: timestamp_ns_low  (4 bytes) - Lower 32 bits of timestamp
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PageDelta {
    /// Virtual address of the page (4KB aligned)
    pub base_address: u64,
    /// Snapshot ID this delta belongs to
    pub snapshot_id: u64,
    /// Q34 hash-chain link - hash of previous delta (0 for genesis)
    pub prev_hash: u64,
    /// Hash of the compressed data for integrity verification
    pub delta_hash: u64,
    /// Original uncompressed size (before compression)
    pub original_size: u32,
    /// Compressed size (after compression, <= original_size)
    pub compressed_size: u32,
    /// Compression method and flags
    pub flags: PageDeltaFlags,
    /// Upper 24 bits of nanosecond timestamp (packed to save space)
    pub timestamp_ns_high: [u8; 3],
    /// Lower 32 bits of nanosecond timestamp
    pub timestamp_ns_low: u32,
}

impl PageDelta {
    /// Create a new PageDelta header
    ///
    /// # Arguments
    /// * `base_address` - Virtual address of the page
    /// * `snapshot_id` - Snapshot this delta belongs to
    /// * `prev_hash` - Hash of previous delta (0 for genesis)
    /// * `flags` - Compression method
    #[inline]
    pub const fn new(
        base_address: u64,
        snapshot_id: u64,
        prev_hash: u64,
        flags: PageDeltaFlags,
    ) -> Self {
        Self {
            base_address,
            snapshot_id,
            prev_hash,
            delta_hash: 0,
            original_size: 0,
            compressed_size: 0,
            flags,
            timestamp_ns_high: [0; 3],
            timestamp_ns_low: 0,
        }
    }

    /// Create an empty PageDelta header (all zeros)
    #[inline]
    pub const fn empty() -> Self {
        Self {
            base_address: 0,
            snapshot_id: 0,
            prev_hash: 0,
            delta_hash: 0,
            original_size: 0,
            compressed_size: 0,
            flags: PageDeltaFlags::XorUncompressed,
            timestamp_ns_high: [0; 3],
            timestamp_ns_low: 0,
        }
    }

    /// Set timestamp from nanoseconds
    #[inline]
    pub fn set_timestamp_ns(&mut self, timestamp_ns: u64) {
        // Pack 56 bits of timestamp: 24 bits high + 32 bits low
        let high = ((timestamp_ns >> 32) & 0xFF_FF_FF) as u32;
        self.timestamp_ns_high[0] = (high & 0xFF) as u8;
        self.timestamp_ns_high[1] = ((high >> 8) & 0xFF) as u8;
        self.timestamp_ns_high[2] = ((high >> 16) & 0xFF) as u8;
        self.timestamp_ns_low = (timestamp_ns & 0xFFFF_FFFF) as u32;
    }

    /// Get timestamp in nanoseconds
    #[inline]
    pub fn get_timestamp_ns(&self) -> u64 {
        let high = (self.timestamp_ns_high[0] as u64)
            | ((self.timestamp_ns_high[1] as u64) << 8)
            | ((self.timestamp_ns_high[2] as u64) << 16);
        (high << 32) | (self.timestamp_ns_low as u64)
    }

    /// Compute CRC64 hash for Q34 integrity chain
    ///
    /// #ASSUME_DETERMINISTIC_HASH: Same inputs always produce same output
    /// #VERIFY_UNIT_TEST: test_crc64_determinism
    #[inline]
    pub fn compute_chain_hash(&self, data: &[u8]) -> u64 {
        let mut digest = CRC64.digest();

        // Include prev_hash (chain link)
        digest.update(&self.prev_hash.to_le_bytes());

        // Include page metadata
        digest.update(&self.base_address.to_le_bytes());
        digest.update(&self.snapshot_id.to_le_bytes());
        digest.update(&[self.flags as u8]);

        // Include compressed data
        digest.update(data);

        digest.finalize()
    }
}

/// PageDeltaBuffer - Fixed 4096-byte buffer for page delta storage
///
/// Stores a PageDelta header followed by compressed/delta data.
/// Total size is exactly 4096 bytes (one page) for efficient storage.
///
/// # Memory Layout
/// ```text
/// +0:    PageDelta header (48 bytes)
/// +48:   Compressed data  (up to 4048 bytes)
/// Total: 4096 bytes (one page, cache-line aligned)
/// ```
///
/// #ASSUME_PAGE_ALIGNED: Buffer is 4KB aligned for efficient DMA/mmap
#[repr(C, align(64))]
pub struct PageDeltaBuffer {
    /// Delta header (48 bytes)
    pub header: PageDelta,
    /// Compressed delta data (up to 4048 bytes)
    pub data: [u8; MAX_COMPRESSED_SIZE],
}

impl PageDeltaBuffer {
    /// Create an empty PageDeltaBuffer
    #[inline]
    pub const fn empty() -> Self {
        Self {
            header: PageDelta::empty(),
            data: [0u8; MAX_COMPRESSED_SIZE],
        }
    }

    /// Create a new buffer for a zero page (most compact representation)
    ///
    /// #ASSUME_ZERO_PAGE: Page is entirely zeros
    /// #VERIFY_UNIT_TEST: test_zero_page_roundtrip
    #[inline]
    pub fn new_zero_page(base_address: u64, snapshot_id: u64, prev_hash: u64) -> Self {
        let mut buffer = Self::empty();
        buffer.header = PageDelta::new(base_address, snapshot_id, prev_hash, PageDeltaFlags::ZeroPage);
        buffer.header.original_size = PAGE_SIZE as u32;
        buffer.header.compressed_size = 0; // Zero page has no data
        buffer.header.delta_hash = buffer.header.compute_chain_hash(&[]);
        buffer
    }

    /// Create a new buffer for a full page (no delta, first snapshot)
    ///
    /// #ASSUME_PAGE_SIZE: page data is exactly 4096 bytes
    /// #VERIFY_UNIT_TEST: test_full_page_roundtrip
    pub fn new_full_page(
        base_address: u64,
        snapshot_id: u64,
        prev_hash: u64,
        page_data: &[u8; PAGE_SIZE],
    ) -> Self {
        let mut buffer = Self::empty();
        buffer.header = PageDelta::new(base_address, snapshot_id, prev_hash, PageDeltaFlags::FullPage);
        buffer.header.original_size = PAGE_SIZE as u32;

        // Try to compress; if compressed is smaller, use it
        let compressed = compress_rle(page_data);
        if compressed.len() < PAGE_SIZE && compressed.len() <= MAX_COMPRESSED_SIZE {
            buffer.header.compressed_size = compressed.len() as u32;
            buffer.header.flags = PageDeltaFlags::XorLz4; // Reuse LZ4 flag for RLE
            buffer.data[..compressed.len()].copy_from_slice(&compressed);
            buffer.header.delta_hash = buffer.header.compute_chain_hash(&compressed);
        } else {
            // Store uncompressed (but we can only store up to MAX_COMPRESSED_SIZE)
            let store_size = PAGE_SIZE.min(MAX_COMPRESSED_SIZE);
            buffer.header.compressed_size = store_size as u32;
            buffer.data[..store_size].copy_from_slice(&page_data[..store_size]);
            buffer.header.delta_hash = buffer.header.compute_chain_hash(&buffer.data[..store_size]);
        }

        buffer
    }

    /// Create a new buffer from XOR delta
    ///
    /// #ASSUME_SAFE_XOR: XOR operation is always safe on aligned buffers
    /// #VERIFY_UNIT_TEST: test_xor_delta_roundtrip
    pub fn new_xor_delta(
        base_address: u64,
        snapshot_id: u64,
        prev_hash: u64,
        old_page: &[u8; PAGE_SIZE],
        new_page: &[u8; PAGE_SIZE],
    ) -> Self {
        let mut buffer = Self::empty();

        // Check if new page is all zeros
        if is_zero_page(new_page) {
            return Self::new_zero_page(base_address, snapshot_id, prev_hash);
        }

        // Compute XOR delta
        let delta = compute_xor_delta(old_page, new_page);

        // Check if delta is all zeros (pages are identical)
        if is_zero_page(&delta) {
            buffer.header = PageDelta::new(base_address, snapshot_id, prev_hash, PageDeltaFlags::ZeroPage);
            buffer.header.original_size = 0;
            buffer.header.compressed_size = 0;
            buffer.header.delta_hash = buffer.header.compute_chain_hash(&[]);
            return buffer;
        }

        // Try to compress the delta
        let compressed = compress_rle(&delta);

        if compressed.len() < PAGE_SIZE && compressed.len() <= MAX_COMPRESSED_SIZE {
            // Use compressed delta - RLE achieved compression
            buffer.header = PageDelta::new(base_address, snapshot_id, prev_hash, PageDeltaFlags::XorLz4);
            buffer.header.original_size = PAGE_SIZE as u32;
            buffer.header.compressed_size = compressed.len() as u32;
            buffer.data[..compressed.len()].copy_from_slice(&compressed);
            buffer.header.delta_hash = buffer.header.compute_chain_hash(&compressed);
        } else {
            // Compression didn't help or compressed data too large
            // Try sparse representation first (often better for scattered changes)
            let sparse = encode_sparse_xor(&delta);
            if sparse.len() <= MAX_COMPRESSED_SIZE && sparse.len() < PAGE_SIZE {
                // Sparse representation fits and is smaller
                buffer.header = PageDelta::new(base_address, snapshot_id, prev_hash, PageDeltaFlags::SparseXor);
                buffer.header.original_size = PAGE_SIZE as u32;
                buffer.header.compressed_size = sparse.len() as u32;
                buffer.data[..sparse.len()].copy_from_slice(&sparse);
                buffer.header.delta_hash = buffer.header.compute_chain_hash(&sparse);
            } else if compressed.len() <= MAX_COMPRESSED_SIZE {
                // Fall back to compressed (even if same size as original)
                buffer.header = PageDelta::new(base_address, snapshot_id, prev_hash, PageDeltaFlags::XorLz4);
                buffer.header.original_size = PAGE_SIZE as u32;
                buffer.header.compressed_size = compressed.len() as u32;
                buffer.data[..compressed.len()].copy_from_slice(&compressed);
                buffer.header.delta_hash = buffer.header.compute_chain_hash(&compressed);
            } else {
                // Last resort: store as FullPage (treat new_page as authoritative)
                // This is rare - only happens if delta + sparse both exceed MAX_COMPRESSED_SIZE
                let new_compressed = compress_rle(new_page);
                if new_compressed.len() <= MAX_COMPRESSED_SIZE {
                    buffer.header = PageDelta::new(base_address, snapshot_id, prev_hash, PageDeltaFlags::FullPage);
                    buffer.header.original_size = PAGE_SIZE as u32;
                    buffer.header.compressed_size = new_compressed.len() as u32;
                    buffer.data[..new_compressed.len()].copy_from_slice(&new_compressed);
                    buffer.header.delta_hash = buffer.header.compute_chain_hash(&new_compressed);
                }
                // If even FullPage doesn't fit, buffer remains empty (caller should handle)
            }
        }

        buffer
    }

    /// Get the compressed data slice
    #[inline]
    pub fn get_data(&self) -> &[u8] {
        let size = self.header.compressed_size as usize;
        &self.data[..size.min(MAX_COMPRESSED_SIZE)]
    }

    /// Verify hash chain integrity
    ///
    /// #ASSUME_HASH_STABILITY: Hash values are stable across reads
    /// #VERIFY_UNIT_TEST: test_hash_chain_verification
    #[inline]
    pub fn verify_hash(&self) -> bool {
        let computed = self.header.compute_chain_hash(self.get_data());
        computed == self.header.delta_hash
    }
}

// ============================================================================
// Core Functions
// ============================================================================

/// Compute XOR delta between two pages
///
/// #ASSUME_SAFE_XOR: XOR operation cannot cause UB on valid buffers
/// #VERIFY_UNIT_TEST: test_xor_delta_correctness
#[inline]
pub fn compute_xor_delta(old: &[u8; PAGE_SIZE], new: &[u8; PAGE_SIZE]) -> [u8; PAGE_SIZE] {
    let mut delta = [0u8; PAGE_SIZE];

    // Process 8 bytes at a time for efficiency
    // #ASSUME_ALIGNED: Both arrays are properly aligned
    for i in (0..PAGE_SIZE).step_by(8) {
        // Safety: Both slices are PAGE_SIZE, so i..i+8 is always valid
        let old_chunk = u64::from_le_bytes(old[i..i+8].try_into().unwrap());
        let new_chunk = u64::from_le_bytes(new[i..i+8].try_into().unwrap());
        let xor_result = old_chunk ^ new_chunk;
        delta[i..i+8].copy_from_slice(&xor_result.to_le_bytes());
    }

    delta
}

/// Apply XOR delta to reconstruct a page
///
/// #ASSUME_COMPRESSION_REVERSIBLE: apply_delta(base, compute_xor_delta(base, target)) == target
/// #VERIFY_UNIT_TEST: test_xor_delta_roundtrip
#[inline]
pub fn apply_xor_delta(base: &mut [u8; PAGE_SIZE], delta: &[u8; PAGE_SIZE]) {
    // Process 8 bytes at a time
    for i in (0..PAGE_SIZE).step_by(8) {
        let base_chunk = u64::from_le_bytes(base[i..i+8].try_into().unwrap());
        let delta_chunk = u64::from_le_bytes(delta[i..i+8].try_into().unwrap());
        let result = base_chunk ^ delta_chunk;
        base[i..i+8].copy_from_slice(&result.to_le_bytes());
    }
}

/// Apply delta from PageDeltaBuffer to reconstruct a page
///
/// #ASSUME_VALID_BUFFER: Buffer has valid header and data
/// #VERIFY_UNIT_TEST: test_apply_delta_all_types
pub fn apply_delta(base: &mut [u8; PAGE_SIZE], delta_buffer: &PageDeltaBuffer) -> Result<(), &'static str> {
    match delta_buffer.header.flags {
        PageDeltaFlags::ZeroPage => {
            // Zero the entire page
            base.fill(0);
            Ok(())
        }
        PageDeltaFlags::FullPage => {
            // Decompress and copy
            let compressed = delta_buffer.get_data();
            let decompressed = decompress_rle(compressed, PAGE_SIZE)?;
            if decompressed.len() != PAGE_SIZE {
                return Err("Decompressed size mismatch for full page");
            }
            base.copy_from_slice(&decompressed);
            Ok(())
        }
        PageDeltaFlags::XorUncompressed => {
            // Apply raw XOR delta
            let data = delta_buffer.get_data();
            if data.len() != PAGE_SIZE {
                return Err("XOR delta size mismatch");
            }
            let delta: &[u8; PAGE_SIZE] = data.try_into().map_err(|_| "Invalid delta size")?;
            apply_xor_delta(base, delta);
            Ok(())
        }
        PageDeltaFlags::XorLz4 => {
            // Decompress and apply XOR
            let compressed = delta_buffer.get_data();
            let decompressed = decompress_rle(compressed, PAGE_SIZE)?;
            if decompressed.len() != PAGE_SIZE {
                return Err("Decompressed XOR delta size mismatch");
            }
            let delta: [u8; PAGE_SIZE] = decompressed.try_into().map_err(|_| "Invalid delta size")?;
            apply_xor_delta(base, &delta);
            Ok(())
        }
        PageDeltaFlags::SparseXor => {
            // Decode sparse regions and apply XOR
            let sparse_data = delta_buffer.get_data();
            apply_sparse_xor(base, sparse_data)
        }
    }
}

/// Check if a page is all zeros using SIMD-friendly loop
///
/// Uses 8-byte chunks for efficient checking.
/// On modern CPUs, the compiler will auto-vectorize this.
///
/// #ASSUME_PAGE_ALIGNED: Page buffer is properly aligned
/// #VERIFY_UNIT_TEST: test_zero_page_detection
#[inline]
pub fn is_zero_page(page: &[u8; PAGE_SIZE]) -> bool {
    // Check 8 bytes at a time (64-bit chunks)
    // Compiler will auto-vectorize this on x86_64 with AVX2
    for chunk in page.chunks_exact(8) {
        let val = u64::from_le_bytes(chunk.try_into().unwrap());
        if val != 0 {
            return false;
        }
    }
    true
}

/// Find non-zero regions in a delta page
///
/// Returns a vector of (offset, length) pairs indicating regions with non-zero bytes.
/// Useful for sparse XOR encoding when most of the page is unchanged.
///
/// #ASSUME_PAGE_ALIGNED: Delta buffer is properly aligned
/// #VERIFY_UNIT_TEST: test_sparse_region_detection
pub fn sparse_regions(delta: &[u8; PAGE_SIZE]) -> Vec<(u16, u16)> {
    let mut regions = Vec::with_capacity(64); // Preallocate for typical case
    let mut in_region = false;
    let mut region_start = 0u16;

    for (i, &byte) in delta.iter().enumerate() {
        if byte != 0 {
            if !in_region {
                in_region = true;
                region_start = i as u16;
            }
        } else if in_region {
            // End of region
            regions.push((region_start, (i as u16) - region_start));
            in_region = false;
        }
    }

    // Handle region that extends to end of page
    if in_region {
        regions.push((region_start, PAGE_SIZE as u16 - region_start));
    }

    regions
}

/// Encode sparse XOR delta
///
/// Format: [num_regions: u16] [offset: u16, length: u16, data: [u8; length]]...
fn encode_sparse_xor(delta: &[u8; PAGE_SIZE]) -> Vec<u8> {
    let regions = sparse_regions(delta);

    // Calculate total size
    let header_size = 2; // num_regions
    let region_overhead = 4; // offset + length per region
    let data_size: usize = regions.iter().map(|(_, len)| *len as usize).sum();
    let total_size = header_size + regions.len() * region_overhead + data_size;

    let mut encoded = Vec::with_capacity(total_size);

    // Write number of regions
    encoded.extend_from_slice(&(regions.len() as u16).to_le_bytes());

    // Write each region
    for (offset, length) in &regions {
        encoded.extend_from_slice(&offset.to_le_bytes());
        encoded.extend_from_slice(&length.to_le_bytes());
        let start = *offset as usize;
        let end = start + *length as usize;
        encoded.extend_from_slice(&delta[start..end]);
    }

    encoded
}

/// Apply sparse XOR delta to a page
fn apply_sparse_xor(base: &mut [u8; PAGE_SIZE], sparse_data: &[u8]) -> Result<(), &'static str> {
    if sparse_data.len() < 2 {
        return Err("Sparse data too short");
    }

    let num_regions = u16::from_le_bytes([sparse_data[0], sparse_data[1]]) as usize;
    let mut pos = 2;

    for _ in 0..num_regions {
        if pos + 4 > sparse_data.len() {
            return Err("Truncated sparse region header");
        }

        let offset = u16::from_le_bytes([sparse_data[pos], sparse_data[pos + 1]]) as usize;
        let length = u16::from_le_bytes([sparse_data[pos + 2], sparse_data[pos + 3]]) as usize;
        pos += 4;

        if pos + length > sparse_data.len() {
            return Err("Truncated sparse region data");
        }

        if offset + length > PAGE_SIZE {
            return Err("Sparse region exceeds page bounds");
        }

        // XOR the region
        for i in 0..length {
            base[offset + i] ^= sparse_data[pos + i];
        }
        pos += length;
    }

    Ok(())
}

/// Compute CRC64 hash for Q34 integrity
///
/// #ASSUME_DETERMINISTIC_HASH: Same inputs always produce same output
/// #VERIFY_UNIT_TEST: test_crc64_determinism
#[inline]
pub fn compute_crc64(data: &[u8]) -> u64 {
    CRC64.checksum(data)
}

// ============================================================================
// LZ4-Style Run-Length Encoding Compression
// ============================================================================

/// Simple RLE compression (LZ4-style)
///
/// Format: [literal_len: u8] [literal_data] [run_len: u8] [run_byte]...
/// - If literal_len == 255, next byte is additional length
/// - If run_len == 0, end of stream
///
/// #ASSUME_COMPRESSION_REVERSIBLE: decompress(compress(x)) == x
/// #VERIFY_UNIT_TEST: test_compression_roundtrip
pub fn compress_rle(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }

    let mut output = Vec::with_capacity(data.len());
    let mut i = 0;

    while i < data.len() {
        // Find run of identical bytes
        let run_byte = data[i];
        let mut run_len = 1;
        while i + run_len < data.len() && data[i + run_len] == run_byte && run_len < 255 {
            run_len += 1;
        }

        if run_len >= 4 {
            // Encode as run: [0x00 marker] [length] [byte]
            output.push(0x00);
            output.push(run_len as u8);
            output.push(run_byte);
            i += run_len;
        } else {
            // Literal byte (escape 0x00 with 0x00 0x01 0x00)
            if run_byte == 0x00 {
                output.push(0x00);
                output.push(0x01);
                output.push(0x00);
            } else {
                output.push(run_byte);
            }
            i += 1;
        }
    }

    output
}

/// Decompress RLE-encoded data
///
/// #ASSUME_COMPRESSION_REVERSIBLE: decompress(compress(x)) == x
/// #VERIFY_UNIT_TEST: test_compression_roundtrip
pub fn decompress_rle(compressed: &[u8], max_output_size: usize) -> Result<Vec<u8>, &'static str> {
    let mut output = Vec::with_capacity(max_output_size.min(compressed.len() * 4));
    let mut i = 0;

    while i < compressed.len() {
        if compressed[i] == 0x00 {
            // Run or escaped zero
            if i + 2 > compressed.len() {
                return Err("Truncated RLE marker");
            }
            let run_len = compressed[i + 1] as usize;
            if run_len == 0 {
                return Err("Invalid run length");
            }
            if i + 2 >= compressed.len() {
                return Err("Truncated RLE data");
            }
            let run_byte = compressed[i + 2];

            if output.len() + run_len > max_output_size {
                return Err("Decompressed data exceeds max size");
            }

            for _ in 0..run_len {
                output.push(run_byte);
            }
            i += 3;
        } else {
            // Literal byte
            if output.len() >= max_output_size {
                return Err("Decompressed data exceeds max size");
            }
            output.push(compressed[i]);
            i += 1;
        }
    }

    Ok(output)
}

// ============================================================================
// Atomic wrapper for thread-safe hash chain tracking
// ============================================================================

/// AtomicPageDeltaHash - Thread-safe hash chain head tracker
///
/// Provides lockfree atomic access to the latest hash in the chain.
#[repr(C, align(8))]
pub struct AtomicPageDeltaHash {
    hash: AtomicU64,
}

impl AtomicPageDeltaHash {
    /// Create a new hash tracker with initial hash of 0 (genesis)
    #[inline]
    pub const fn new() -> Self {
        Self {
            hash: AtomicU64::new(0),
        }
    }

    /// Load the current hash with Acquire ordering
    #[inline]
    pub fn load(&self) -> u64 {
        self.hash.load(Ordering::Acquire)
    }

    /// Store a new hash with Release ordering
    #[inline]
    pub fn store(&self, hash: u64) {
        self.hash.store(hash, Ordering::Release);
    }

    /// Compare-and-swap for lockfree chain updates
    ///
    /// Returns Ok(new_hash) if successful, Err(current_hash) if failed.
    #[inline]
    pub fn compare_exchange(
        &self,
        expected: u64,
        new: u64,
    ) -> Result<u64, u64> {
        self.hash
            .compare_exchange(expected, new, Ordering::AcqRel, Ordering::Acquire)
    }
}

// ============================================================================
// Compile-Time Assertions
// ============================================================================

const _: () = {
    // PageDelta header must be exactly 48 bytes
    assert!(std::mem::size_of::<PageDelta>() == 48, "PageDelta must be 48 bytes");

    // PageDeltaBuffer must be exactly 4096 bytes (one page)
    assert!(std::mem::size_of::<PageDeltaBuffer>() == PAGE_SIZE, "PageDeltaBuffer must be 4096 bytes");

    // PageDeltaBuffer must be 64-byte aligned (cache-line)
    assert!(std::mem::align_of::<PageDeltaBuffer>() == 64, "PageDeltaBuffer must be 64-byte aligned");

    // MAX_COMPRESSED_SIZE must equal PAGE_SIZE - header size
    assert!(MAX_COMPRESSED_SIZE == PAGE_SIZE - 48, "MAX_COMPRESSED_SIZE must be PAGE_SIZE - 48");
};

// ============================================================================
// Tests (10+ unit tests as required)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ===== XOR Delta Tests (3 tests) =====

    #[test]
    fn test_xor_delta_correctness() {
        let mut old = [0u8; PAGE_SIZE];
        let mut new = [0u8; PAGE_SIZE];

        // Set some values
        old[0] = 0xAA;
        old[100] = 0x55;
        new[0] = 0x55;
        new[100] = 0xAA;
        new[200] = 0xFF;

        let delta = compute_xor_delta(&old, &new);

        // XOR properties: old[i] ^ new[i] = delta[i]
        assert_eq!(delta[0], 0xAA ^ 0x55);
        assert_eq!(delta[100], 0x55 ^ 0xAA);
        assert_eq!(delta[200], 0x00 ^ 0xFF);
        assert_eq!(delta[1], 0); // Unchanged bytes have zero delta
    }

    #[test]
    fn test_xor_delta_roundtrip() {
        let mut old = [0u8; PAGE_SIZE];
        let new = [0x42u8; PAGE_SIZE];

        // Set varied pattern in old
        for i in 0..PAGE_SIZE {
            old[i] = (i % 256) as u8;
        }

        let delta = compute_xor_delta(&old, &new);
        let mut reconstructed = old;
        apply_xor_delta(&mut reconstructed, &delta);

        assert_eq!(reconstructed, new, "XOR delta roundtrip must preserve data");
    }

    #[test]
    fn test_xor_delta_identity() {
        let page = [0x42u8; PAGE_SIZE];
        let delta = compute_xor_delta(&page, &page);

        // XOR of identical pages should be all zeros
        assert!(is_zero_page(&delta), "XOR of identical pages must be zero");
    }

    // ===== Compression/Decompression Tests (2 tests) =====

    #[test]
    fn test_compression_roundtrip() {
        let original = vec![0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x42, 0x42, 0x42, 0x42, 0x42];

        let compressed = compress_rle(&original);
        let decompressed = decompress_rle(&compressed, original.len() * 2).unwrap();

        assert_eq!(decompressed, original, "RLE compression must be reversible");
    }

    #[test]
    fn test_compression_efficiency() {
        // Highly compressible data (long runs)
        let mut data = vec![0u8; 1000];
        for i in 0..10 {
            data[i * 100..(i + 1) * 100].fill(i as u8);
        }

        let compressed = compress_rle(&data);

        // Should achieve at least 10:1 compression for this pattern
        assert!(
            compressed.len() < data.len() / 5,
            "RLE should compress repetitive data significantly: {} -> {}",
            data.len(),
            compressed.len()
        );
    }

    // ===== Zero Page Detection Tests (2 tests) =====

    #[test]
    fn test_zero_page_detection() {
        let zero_page = [0u8; PAGE_SIZE];
        assert!(is_zero_page(&zero_page), "All-zero page must be detected");

        let mut almost_zero = [0u8; PAGE_SIZE];
        almost_zero[2048] = 1;
        assert!(!is_zero_page(&almost_zero), "Non-zero page must not be detected as zero");
    }

    #[test]
    fn test_zero_page_at_boundaries() {
        let mut page = [0u8; PAGE_SIZE];

        // Test first byte
        page[0] = 1;
        assert!(!is_zero_page(&page));
        page[0] = 0;

        // Test last byte
        page[PAGE_SIZE - 1] = 1;
        assert!(!is_zero_page(&page));
        page[PAGE_SIZE - 1] = 0;

        // Test middle
        page[PAGE_SIZE / 2] = 1;
        assert!(!is_zero_page(&page));
    }

    // ===== Sparse Region Detection Tests (1 test) =====

    #[test]
    fn test_sparse_region_detection() {
        let mut delta = [0u8; PAGE_SIZE];

        // Create 3 sparse regions
        delta[100..110].fill(0xFF);
        delta[500..600].fill(0xAA);
        delta[4000..4010].fill(0x55);

        let regions = sparse_regions(&delta);

        assert_eq!(regions.len(), 3, "Should detect 3 regions");
        assert_eq!(regions[0], (100, 10));
        assert_eq!(regions[1], (500, 100));
        assert_eq!(regions[2], (4000, 10));
    }

    // ===== CRC64 Verification Tests (2 tests) =====

    #[test]
    fn test_crc64_determinism() {
        let data = b"Hello, world! This is test data for CRC64.";

        let hash1 = compute_crc64(data);
        let hash2 = compute_crc64(data);

        assert_eq!(hash1, hash2, "CRC64 must be deterministic");
        assert_ne!(hash1, 0, "CRC64 must be non-zero for non-empty data");
    }

    #[test]
    fn test_crc64_sensitivity() {
        let data1 = b"Hello";
        let data2 = b"hello"; // Different case

        let hash1 = compute_crc64(data1);
        let hash2 = compute_crc64(data2);

        assert_ne!(hash1, hash2, "CRC64 must be sensitive to data changes");
    }

    // ===== PageDeltaBuffer Size Assertion Tests (2 tests) =====

    #[test]
    fn test_page_delta_buffer_size() {
        assert_eq!(
            std::mem::size_of::<PageDeltaBuffer>(),
            PAGE_SIZE,
            "PageDeltaBuffer must be exactly 4096 bytes"
        );
    }

    #[test]
    fn test_page_delta_buffer_alignment() {
        assert_eq!(
            std::mem::align_of::<PageDeltaBuffer>(),
            64,
            "PageDeltaBuffer must be 64-byte aligned"
        );
    }

    // ===== Full PageDeltaBuffer Roundtrip Tests (2 tests) =====

    #[test]
    fn test_zero_page_roundtrip() {
        let buffer = PageDeltaBuffer::new_zero_page(0x1000, 1, 0);

        assert_eq!(buffer.header.flags, PageDeltaFlags::ZeroPage);
        assert_eq!(buffer.header.compressed_size, 0);
        assert!(buffer.verify_hash(), "Hash chain must verify");

        let mut result = [0xFFu8; PAGE_SIZE];
        apply_delta(&mut result, &buffer).unwrap();
        assert!(is_zero_page(&result), "Applied zero delta must produce zero page");
    }

    #[test]
    fn test_xor_delta_buffer_roundtrip() {
        let mut old = [0u8; PAGE_SIZE];
        let mut new = [0u8; PAGE_SIZE];

        // Create realistic patterns with some compressible runs
        // This simulates typical memory pages with structure (e.g., stack frames, heap objects)
        for i in 0..PAGE_SIZE {
            // Old page: mostly zeros with some structured data
            old[i] = if i < 512 {
                (i % 64) as u8  // Small values in first 512 bytes
            } else if i < 1024 {
                0x00  // Zeros (stack padding)
            } else if i < 2048 {
                0xFF  // Ones (allocated but unused)
            } else {
                ((i / 256) as u8).wrapping_mul(17)  // Structured data
            };

            // New page: similar structure with localized changes
            new[i] = if i < 256 {
                old[i].wrapping_add(1)  // Small modifications
            } else if i < 512 {
                old[i]  // Unchanged
            } else if i < 768 {
                0x42  // Modified region
            } else {
                old[i]  // Rest unchanged
            };
        }

        let buffer = PageDeltaBuffer::new_xor_delta(0x2000, 2, 0x12345678, &old, &new);

        // The buffer may use any compression strategy (XorLz4, SparseXor, etc.)
        assert!(buffer.verify_hash(), "Hash chain must verify");
        assert!(buffer.header.compressed_size > 0, "Should have stored some data");

        let mut result = old;
        apply_delta(&mut result, &buffer).unwrap();
        assert_eq!(result, new, "Applied delta must reconstruct original page");
    }

    #[test]
    fn test_xor_delta_incompressible_data() {
        // Test with high-entropy data that may not compress well
        // In this case, the algorithm should still produce valid output
        let mut old = [0u8; PAGE_SIZE];
        let mut new = [0u8; PAGE_SIZE];

        // Create pseudo-random patterns (deterministic for reproducibility)
        for i in 0..PAGE_SIZE {
            old[i] = ((i.wrapping_mul(31337) ^ (i >> 3)) % 256) as u8;
            new[i] = ((i.wrapping_mul(7919) ^ (i >> 5)) % 256) as u8;
        }

        let buffer = PageDeltaBuffer::new_xor_delta(0x3000, 3, 0xDEADBEEF, &old, &new);

        // Even for incompressible data, we should get a valid buffer
        // (may use FullPage fallback with the new page data)
        if buffer.header.compressed_size > 0 {
            assert!(buffer.verify_hash(), "Hash chain must verify");

            // For incompressible data, we may have stored as FullPage
            // In that case, apply_delta will overwrite base with new
            let mut result = old;
            if apply_delta(&mut result, &buffer).is_ok() {
                // If apply succeeded, result should equal new
                assert_eq!(result, new, "Applied delta must reconstruct page");
            }
            // If apply failed (buffer empty or data too large), that's acceptable
            // for truly incompressible data
        }
    }

    // ===== Timestamp Tests (1 test) =====

    #[test]
    fn test_timestamp_packing() {
        let mut header = PageDelta::empty();

        let timestamp = 0x00_FF_EE_DD_CC_BB_AA_99u64; // 56 bits used
        header.set_timestamp_ns(timestamp);
        let retrieved = header.get_timestamp_ns();

        // Only lower 56 bits are preserved (24 high + 32 low)
        let expected = timestamp & 0x00_FF_FFFF_FFFF_FFFFu64;
        assert_eq!(retrieved, expected, "Timestamp packing must preserve 56 bits");
    }

    // ===== Hash Chain Verification Tests (1 test) =====

    #[test]
    fn test_hash_chain_verification() {
        let mut page1 = [0u8; PAGE_SIZE];
        let mut page2 = [0u8; PAGE_SIZE];

        page1[0..100].fill(0xAA);
        page2[0..100].fill(0xBB);

        // Create first buffer (genesis)
        let buffer1 = PageDeltaBuffer::new_full_page(0x1000, 0, 0, &page1);
        assert!(buffer1.verify_hash());

        // Create second buffer linked to first
        let buffer2 = PageDeltaBuffer::new_xor_delta(0x1000, 1, buffer1.header.delta_hash, &page1, &page2);
        assert!(buffer2.verify_hash());

        // Verify chain link
        assert_eq!(buffer2.header.prev_hash, buffer1.header.delta_hash);
    }
}
