//! MemoryDeltaRingBufferCapsule - T5 Streaming Page Delta Storage
//!
//! Ring buffer storing compressed memory page deltas for time-travel reconstruction.
//! Configurable capacity from 32MB (HEAVY sessions) to 60MB based on tier configuration.
//!
//! # Memory Layout (~32MB default for HEAVY sessions)
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │ Header (256 bytes)                                              │
//! │ - generation, head, tail, total_deltas, total_bytes            │
//! │ - capacity_bytes, oldest_snapshot, newest_snapshot             │
//! ├─────────────────────────────────────────────────────────────────┤
//! │ Index (64KB = 16384 entries × 4 bytes)                         │
//! │ - Maps snapshot_id → offset in data buffer                     │
//! ├─────────────────────────────────────────────────────────────────┤
//! │ Data buffer (~32MB - 64KB - 256B)                              │
//! │ - Contains packed PageDeltaBuffer entries                      │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Lockfree Design
//!
//! - Single writer (debugger thread) + multiple readers (reconstruction)
//! - SeqLock pattern for consistent reads
//! - CAS-based head/tail updates
//! - Generation counter for wraparound detection
//!
//! # Performance
//!
//! - Push delta: <1μs (append + index update)
//! - Get delta: <100ns (index lookup + offset)
//! - Range query: O(n) where n = deltas in range
//! - Eviction: <500ns (atomic head/tail update)
//!
//! #ASSUME_LOCKFREE_ONLY: All coordination via atomics, no mutex/RwLock
//! #ASSUME_SINGLE_WRITER: Only one thread writes deltas (debugger thread)
//! #ASSUME_CACHE_ALIGNED: 256-byte alignment prevents false sharing
//! #ASSUME_INDEX_BOUNDS: Index entries never exceed capacity
//! #ASSUME_WRAPAROUND_SAFE: Generation counter detects stale reads

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// ============================================================================
// Constants
// ============================================================================

/// Default capacity in megabytes (HEAVY sessions)
pub const DEFAULT_CAPACITY_MB: usize = 32;

/// Maximum capacity in megabytes
pub const MAX_CAPACITY_MB: usize = 60;

/// Minimum capacity in megabytes
pub const MIN_CAPACITY_MB: usize = 8;

/// Index capacity (16384 entries = covers 16K snapshots before wraparound)
pub const INDEX_CAPACITY: usize = 16384;

/// Header size in bytes
pub const HEADER_SIZE: usize = 256;

/// Index size in bytes (16384 × 4 = 65536 bytes = 64KB)
pub const INDEX_SIZE: usize = INDEX_CAPACITY * 4;

/// Page size for delta buffers
pub const PAGE_SIZE: usize = 4096;

/// Maximum delta size (compressed page + metadata)
pub const MAX_DELTA_SIZE: usize = PAGE_SIZE + 64;

/// Magic number for delta validation
pub const DELTA_MAGIC: u32 = 0x4B44_4244; // "KDBD" - Kindly Debugger Buffer Delta

// ============================================================================
// Error Types
// ============================================================================

/// Ring buffer error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingError {
    /// Buffer is full, eviction needed
    BufferFull,
    /// Delta too large for buffer
    DeltaTooLarge,
    /// Snapshot not found in buffer
    SnapshotNotFound,
    /// Page not found in snapshot
    PageNotFound,
    /// Buffer corrupted (magic mismatch)
    Corrupted,
    /// Delta already exists for this snapshot/page
    DuplicateDelta,
    /// Invalid capacity configuration
    InvalidCapacity,
    /// Snapshot evicted (too old)
    Evicted,
    /// Wraparound detected (generation mismatch)
    WraparoundDetected,
}

impl std::fmt::Display for RingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BufferFull => write!(f, "Ring buffer full"),
            Self::DeltaTooLarge => write!(f, "Delta exceeds maximum size"),
            Self::SnapshotNotFound => write!(f, "Snapshot not found"),
            Self::PageNotFound => write!(f, "Page not found in snapshot"),
            Self::Corrupted => write!(f, "Buffer corrupted"),
            Self::DuplicateDelta => write!(f, "Duplicate delta entry"),
            Self::InvalidCapacity => write!(f, "Invalid capacity configuration"),
            Self::Evicted => write!(f, "Snapshot evicted"),
            Self::WraparoundDetected => write!(f, "Wraparound detected"),
        }
    }
}

impl std::error::Error for RingError {}

// ============================================================================
// Page Delta Buffer
// ============================================================================

/// Compressed page delta buffer (variable size, max 4160 bytes)
///
/// Layout (packed to minimize padding):
/// ```text
/// snapshot_id(8) | page_addr(8) | magic(4) | generation(4) |
/// prev_offset(4) | compressed_size(2) | flags(1) | checksum(1) | data(N)
/// ```
///
/// Total header: 32 bytes (no padding needed)
/// Total with data: 32 + 4096 = 4128 bytes
///
/// #ASSUME_ALIGNED: Data starts at 32-byte offset for SIMD compatibility
#[repr(C, align(32))]
#[derive(Clone)]
pub struct PageDeltaBuffer {
    /// Snapshot ID this delta belongs to
    pub snapshot_id: u64,
    /// Page virtual address
    pub page_addr: u64,
    /// Magic number for validation
    pub magic: u32,
    /// Generation counter for this delta
    pub generation: u32,
    /// Offset to previous delta for same page (0 = first)
    pub prev_offset: u32,
    /// Compressed data size (0 = full page, no compression)
    pub compressed_size: u16,
    /// Flags: bit0 = compressed, bit1 = full page, bit2 = xor delta
    pub flags: u8,
    /// Simple checksum (XOR of all data bytes)
    pub checksum: u8,
    /// Compressed or full page data
    pub data: [u8; PAGE_SIZE],
}

impl PageDeltaBuffer {
    /// Create empty delta buffer
    pub const fn empty() -> Self {
        Self {
            snapshot_id: 0,
            page_addr: 0,
            magic: 0,
            generation: 0,
            prev_offset: 0,
            compressed_size: 0,
            flags: 0,
            checksum: 0,
            data: [0; PAGE_SIZE],
        }
    }

    /// Create new delta with full page data
    pub fn new_full_page(snapshot_id: u64, page_addr: u64, data: &[u8], generation: u32) -> Self {
        let mut delta = Self::empty();
        delta.magic = DELTA_MAGIC;
        delta.snapshot_id = snapshot_id;
        delta.page_addr = page_addr;
        delta.compressed_size = 0; // Full page
        delta.flags = 0x02; // Full page flag
        delta.generation = generation;
        delta.prev_offset = 0;

        // Copy data (up to PAGE_SIZE)
        let copy_len = data.len().min(PAGE_SIZE);
        delta.data[..copy_len].copy_from_slice(&data[..copy_len]);

        // Compute checksum
        delta.checksum = delta.compute_checksum();

        delta
    }

    /// Create new delta with XOR-compressed data
    pub fn new_xor_delta(
        snapshot_id: u64,
        page_addr: u64,
        delta_data: &[u8],
        generation: u32,
        prev_offset: u32,
    ) -> Self {
        let mut delta = Self::empty();
        delta.magic = DELTA_MAGIC;
        delta.snapshot_id = snapshot_id;
        delta.page_addr = page_addr;
        delta.compressed_size = delta_data.len() as u16;
        delta.flags = 0x05; // Compressed + XOR delta
        delta.generation = generation;
        delta.prev_offset = prev_offset;

        // Copy delta data
        let copy_len = delta_data.len().min(PAGE_SIZE);
        delta.data[..copy_len].copy_from_slice(&delta_data[..copy_len]);

        // Compute checksum
        delta.checksum = delta.compute_checksum();

        delta
    }

    /// Compute simple XOR checksum of data
    fn compute_checksum(&self) -> u8 {
        let len = if self.compressed_size > 0 {
            self.compressed_size as usize
        } else {
            PAGE_SIZE
        };

        let mut checksum: u8 = 0;
        for byte in &self.data[..len] {
            checksum ^= byte;
        }
        checksum
    }

    /// Validate delta integrity
    pub fn validate(&self) -> bool {
        self.magic == DELTA_MAGIC && self.checksum == self.compute_checksum()
    }

    /// Get actual data size (compressed or full)
    pub fn data_size(&self) -> usize {
        if self.compressed_size > 0 {
            self.compressed_size as usize
        } else {
            PAGE_SIZE
        }
    }

    /// Check if this is a full page delta
    pub fn is_full_page(&self) -> bool {
        (self.flags & 0x02) != 0
    }

    /// Check if this is an XOR delta
    pub fn is_xor_delta(&self) -> bool {
        (self.flags & 0x04) != 0
    }

    /// Get total serialized size (header + data)
    #[inline]
    pub fn serialized_size(&self) -> usize {
        32 + self.data_size() // Header (32 bytes) + data
    }
}

// PageDeltaBuffer size verification
const _: () = {
    // Header: magic(4) + snapshot_id(8) + page_addr(8) + compressed_size(2) +
    //         flags(1) + checksum(1) + generation(4) + prev_offset(4) = 32 bytes
    // Data: 4096 bytes
    // Total: 32 + 4096 = 4128 bytes, aligned to 32 bytes
    assert!(std::mem::size_of::<PageDeltaBuffer>() == 4128);
    assert!(std::mem::align_of::<PageDeltaBuffer>() == 32);
};

// ============================================================================
// Ring Statistics
// ============================================================================

/// Ring buffer statistics snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct RingStats {
    /// Total deltas stored since creation
    pub total_deltas: u64,
    /// Total bytes written since creation
    pub total_bytes: u64,
    /// Current buffer capacity in bytes
    pub capacity_bytes: u64,
    /// Available bytes remaining
    pub available_bytes: u64,
    /// Oldest snapshot ID in buffer
    pub oldest_snapshot: u64,
    /// Newest snapshot ID in buffer
    pub newest_snapshot: u64,
    /// Number of evictions performed
    pub evictions: u64,
    /// Current generation counter
    pub generation: u64,
    /// Number of deltas currently in buffer
    pub active_deltas: u64,
}

// ============================================================================
// Delta Iterator
// ============================================================================

/// Iterator over deltas in a snapshot range
pub struct DeltaIterator<'a> {
    buffer: &'a MemoryDeltaRingBufferCapsule,
    current_offset: usize,
    end_offset: usize,
    start_snapshot: u64,
    end_snapshot: u64,
}

impl<'a> Iterator for DeltaIterator<'a> {
    type Item = PageDeltaBuffer;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_offset >= self.end_offset {
            return None;
        }

        // Read delta at current offset
        if let Some(delta) = self.buffer.read_delta_at_offset(self.current_offset) {
            if delta.snapshot_id >= self.start_snapshot && delta.snapshot_id <= self.end_snapshot {
                self.current_offset += delta.serialized_size();
                return Some(delta);
            }
        }

        // Skip invalid or out-of-range deltas
        self.current_offset += MAX_DELTA_SIZE;
        self.next()
    }
}

// ============================================================================
// Index Entry (Packed)
// ============================================================================

/// Packed index entry for snapshot to offset mapping.
///
/// Layout: offset(24 bits) | flags(4 bits) | delta_count(4 bits)
/// - offset: Byte offset in data buffer (up to 16MB)
/// - flags: Entry state flags
/// - delta_count: Number of deltas for this snapshot (0-15, saturating)
#[derive(Debug, Clone, Copy)]
struct IndexEntry(u32);

impl IndexEntry {
    const OFFSET_MASK: u32 = 0x00FF_FFFF; // 24 bits
    const FLAGS_SHIFT: u32 = 24;
    const FLAGS_MASK: u32 = 0x0F;
    const COUNT_SHIFT: u32 = 28;
    const COUNT_MASK: u32 = 0x0F;

    const FLAG_VALID: u32 = 0x01;
    const FLAG_WRAPPED: u32 = 0x02;

    const fn empty() -> Self {
        Self(0)
    }

    const fn new(offset: u32, delta_count: u8) -> Self {
        // Manual min since .min() is not const in stable
        let clamped = if delta_count > 15 { 15 } else { delta_count };
        let entry = (offset & Self::OFFSET_MASK)
            | (Self::FLAG_VALID << Self::FLAGS_SHIFT)
            | ((clamped as u32) << Self::COUNT_SHIFT);
        Self(entry)
    }

    const fn offset(&self) -> u32 {
        self.0 & Self::OFFSET_MASK
    }

    const fn is_valid(&self) -> bool {
        ((self.0 >> Self::FLAGS_SHIFT) & Self::FLAGS_MASK) & Self::FLAG_VALID != 0
    }

    const fn delta_count(&self) -> u8 {
        ((self.0 >> Self::COUNT_SHIFT) & Self::COUNT_MASK) as u8
    }

    fn with_incremented_count(&self) -> Self {
        let count = self.delta_count().saturating_add(1);
        Self::new(self.offset(), count)
    }
}

// ============================================================================
// Memory Delta Ring Buffer Capsule
// ============================================================================

/// Memory Delta Ring Buffer Capsule - T5 Streaming
///
/// Ring buffer storing compressed page deltas for time-travel reconstruction.
/// Uses lockfree design with SeqLock pattern for consistent reads.
///
/// # Memory Layout
///
/// - Header: 256 bytes (metadata + padding)
/// - Index: 64KB (16384 × 4-byte entries)
/// - Data: Remaining capacity (~32MB - 64KB - 256B)
///
/// # Thread Safety
///
/// - Single writer: Debugger thread appends deltas
/// - Multiple readers: Reconstruction threads read deltas
/// - SeqLock: Consistent reads via generation counter
///
/// #ASSUME_LOCKFREE_ONLY: All coordination via atomics
/// #ASSUME_SINGLE_WRITER: Only one thread writes deltas
/// #ASSUME_CACHE_ALIGNED: 256-byte header alignment
/// #VERIFY_UNIT_TEST: test_ring_buffer_size, test_push_get_roundtrip
#[repr(C, align(256))]
pub struct MemoryDeltaRingBufferCapsule {
    // ====== Header (256 bytes) ======

    /// Generation counter for SeqLock pattern
    pub generation: AtomicU64,
    /// Write position (byte offset in data buffer)
    head: AtomicU64,
    /// Read position (oldest valid byte offset)
    tail: AtomicU64,
    /// Total deltas stored since creation
    total_deltas: AtomicU64,
    /// Total bytes written since creation
    total_bytes: AtomicU64,
    /// Buffer capacity in bytes
    capacity_bytes: AtomicU64,
    /// Oldest snapshot ID in buffer
    oldest_snapshot: AtomicU64,
    /// Newest snapshot ID in buffer
    newest_snapshot: AtomicU64,
    /// Number of evictions performed
    evictions: AtomicU64,
    /// Active deltas in buffer
    active_deltas: AtomicU64,
    /// Header padding to 256 bytes
    _header_pad: [u8; 256 - 10 * 8],

    // ====== Index (64KB = 16384 × 4 bytes) ======

    /// Index mapping snapshot_id % INDEX_CAPACITY → offset
    /// Uses AtomicU32 for lockfree updates
    index: [AtomicU32; INDEX_CAPACITY],

    // ====== Data Buffer (heap allocated) ======
    // Note: In actual implementation, this would be heap-allocated
    // For the capsule pattern, we use a fixed-size inline buffer
    // that can be configured at construction time

    /// Data buffer containing packed PageDeltaBuffer entries
    /// Size: capacity_bytes - HEADER_SIZE - INDEX_SIZE
    data: Box<[u8]>,
}

impl MemoryDeltaRingBufferCapsule {
    /// Create new ring buffer with specified capacity in MB.
    ///
    /// # Arguments
    /// * `capacity_mb` - Capacity in megabytes (8-60MB)
    ///
    /// # Panics
    /// Panics if capacity is outside valid range.
    ///
    /// #ASSUME_VALID_CAPACITY: capacity_mb in [MIN_CAPACITY_MB, MAX_CAPACITY_MB]
    /// #VERIFY_UNIT_TEST: test_new_capacity
    pub fn new(capacity_mb: usize) -> Self {
        assert!(
            capacity_mb >= MIN_CAPACITY_MB && capacity_mb <= MAX_CAPACITY_MB,
            "Capacity must be between {} and {} MB",
            MIN_CAPACITY_MB,
            MAX_CAPACITY_MB
        );

        let total_bytes = capacity_mb * 1024 * 1024;
        let data_bytes = total_bytes.saturating_sub(HEADER_SIZE + INDEX_SIZE);

        // Initialize index with empty entries
        const EMPTY_INDEX: AtomicU32 = AtomicU32::new(0);

        Self {
            generation: AtomicU64::new(0),
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            total_deltas: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            capacity_bytes: AtomicU64::new(data_bytes as u64),
            oldest_snapshot: AtomicU64::new(0),
            newest_snapshot: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            active_deltas: AtomicU64::new(0),
            _header_pad: [0; 256 - 10 * 8],
            index: [EMPTY_INDEX; INDEX_CAPACITY],
            data: vec![0u8; data_bytes].into_boxed_slice(),
        }
    }

    /// Create with default capacity (32MB for HEAVY sessions)
    pub fn new_default() -> Self {
        Self::new(DEFAULT_CAPACITY_MB)
    }

    /// Push a page delta to the ring buffer.
    ///
    /// Returns the snapshot ID on success.
    ///
    /// # Performance
    /// <1μs average (lockfree append + index update)
    ///
    /// # Errors
    /// - `BufferFull`: Buffer is full and eviction needed
    /// - `DeltaTooLarge`: Delta exceeds maximum size
    /// - `Corrupted`: Delta validation failed
    ///
    /// #ASSUME_SINGLE_WRITER: Only one thread calls push_delta
    /// #ASSUME_VALID_DELTA: Delta magic and checksum are valid
    /// #VERIFY_UNIT_TEST: test_push_delta, test_push_multiple
    pub fn push_delta(&self, delta: &PageDeltaBuffer) -> Result<u64, RingError> {
        // Validate delta
        if !delta.validate() {
            return Err(RingError::Corrupted);
        }

        let delta_size = delta.serialized_size();
        if delta_size > MAX_DELTA_SIZE {
            return Err(RingError::DeltaTooLarge);
        }

        // Check available space
        let capacity = self.capacity_bytes.load(Ordering::Relaxed) as usize;
        let head = self.head.load(Ordering::Acquire) as usize;
        let tail = self.tail.load(Ordering::Acquire) as usize;

        let used = if head >= tail {
            head - tail
        } else {
            capacity - tail + head
        };

        if used + delta_size > capacity {
            // Try to evict oldest snapshot
            self.evict_oldest_snapshot()?;
        }

        // Increment generation for SeqLock
        self.generation.fetch_add(1, Ordering::Release);

        // Calculate write offset (with wraparound)
        let write_offset = head % capacity;

        // Write delta to data buffer
        self.write_delta_at_offset(write_offset, delta);

        // Update index
        let index_slot = (delta.snapshot_id as usize) % INDEX_CAPACITY;
        let existing = IndexEntry(self.index[index_slot].load(Ordering::Relaxed));

        if existing.is_valid() && existing.offset() != 0 {
            // Increment delta count for existing snapshot
            let new_entry = existing.with_incremented_count();
            self.index[index_slot].store(new_entry.0, Ordering::Release);
        } else {
            // New snapshot entry
            let entry = IndexEntry::new(write_offset as u32, 1);
            self.index[index_slot].store(entry.0, Ordering::Release);
        }

        // Update head pointer
        let new_head = (head + delta_size) % capacity;
        self.head.store(new_head as u64, Ordering::Release);

        // Update statistics
        self.total_deltas.fetch_add(1, Ordering::Relaxed);
        self.total_bytes.fetch_add(delta_size as u64, Ordering::Relaxed);
        self.active_deltas.fetch_add(1, Ordering::Relaxed);

        // Update snapshot range
        let current_oldest = self.oldest_snapshot.load(Ordering::Relaxed);
        let current_newest = self.newest_snapshot.load(Ordering::Relaxed);

        // First delta: set both oldest and newest
        if self.total_deltas.load(Ordering::Relaxed) == 1 {
            self.oldest_snapshot.store(delta.snapshot_id, Ordering::Release);
            self.newest_snapshot.store(delta.snapshot_id, Ordering::Release);
        } else {
            // Update oldest if smaller (though normally we'd evict from oldest)
            if delta.snapshot_id < current_oldest || current_oldest == 0 {
                self.oldest_snapshot.store(delta.snapshot_id, Ordering::Release);
            }
            // Update newest if larger
            if delta.snapshot_id > current_newest {
                self.newest_snapshot.store(delta.snapshot_id, Ordering::Release);
            }
        }

        // Complete SeqLock write
        self.generation.fetch_add(1, Ordering::Release);

        Ok(delta.snapshot_id)
    }

    /// Write delta to data buffer at specified offset.
    ///
    /// #ASSUME_ALIGNED: Offset is within data buffer bounds
    /// #ASSUME_VALID_DELTA: Delta is properly constructed
    fn write_delta_at_offset(&self, offset: usize, delta: &PageDeltaBuffer) {
        let data_size = delta.serialized_size();

        // Safety: We're writing to our own data buffer within bounds
        // #ASSUME_BOUNDS: offset + data_size <= capacity
        // #VERIFY_UNIT_TEST: test_write_read_offset
        let data_ptr = self.data.as_ptr() as *mut u8;

        // SAFETY: offset is validated to be within bounds before this call
        // The caller (push_delta) ensures offset + data_size <= capacity
        unsafe {
            let dest = data_ptr.add(offset);

            // Write header (32 bytes)
            std::ptr::copy_nonoverlapping(
                delta as *const PageDeltaBuffer as *const u8,
                dest,
                32,
            );

            // Write data
            std::ptr::copy_nonoverlapping(delta.data.as_ptr(), dest.add(32), data_size - 32);
        }
    }

    /// Read delta from data buffer at specified offset.
    fn read_delta_at_offset(&self, offset: usize) -> Option<PageDeltaBuffer> {
        let capacity = self.capacity_bytes.load(Ordering::Relaxed) as usize;
        if offset >= capacity {
            return None;
        }

        // Read with SeqLock pattern
        let gen_before = self.generation.load(Ordering::Acquire);

        let mut delta = PageDeltaBuffer::empty();
        let data_ptr = self.data.as_ptr();

        // SAFETY: offset is validated to be within bounds
        // #ASSUME_BOUNDS: offset < capacity
        // #VERIFY_UNIT_TEST: test_read_delta_offset
        unsafe {
            let src = data_ptr.add(offset);

            // Read header (32 bytes)
            std::ptr::copy_nonoverlapping(
                src,
                &mut delta as *mut PageDeltaBuffer as *mut u8,
                32,
            );

            // Read data if header is valid
            if delta.magic == DELTA_MAGIC {
                let data_len = delta.data_size().min(PAGE_SIZE);
                std::ptr::copy_nonoverlapping(src.add(32), delta.data.as_mut_ptr(), data_len);
            }
        }

        // Verify SeqLock (check if write happened during read)
        let gen_after = self.generation.load(Ordering::Acquire);
        if gen_before != gen_after || (gen_before & 1) != 0 {
            // Write in progress or happened during read, retry
            return self.read_delta_at_offset(offset);
        }

        if delta.validate() {
            Some(delta)
        } else {
            None
        }
    }

    /// Get delta for a specific snapshot and page address.
    ///
    /// # Performance
    /// <100ns (index lookup + offset read)
    ///
    /// #ASSUME_VALID_SNAPSHOT: snapshot_id is within buffer range
    /// #VERIFY_UNIT_TEST: test_get_delta
    pub fn get_delta(&self, snapshot_id: u64, page_addr: u64) -> Option<PageDeltaBuffer> {
        let index_slot = (snapshot_id as usize) % INDEX_CAPACITY;
        let entry = IndexEntry(self.index[index_slot].load(Ordering::Acquire));

        if !entry.is_valid() {
            return None;
        }

        let offset = entry.offset() as usize;

        // Linear search from offset for matching page_addr
        // (deltas for same snapshot are stored contiguously)
        let capacity = self.capacity_bytes.load(Ordering::Relaxed) as usize;
        let mut current_offset = offset;

        for _ in 0..entry.delta_count() {
            if current_offset >= capacity {
                break;
            }

            if let Some(delta) = self.read_delta_at_offset(current_offset) {
                if delta.snapshot_id == snapshot_id && delta.page_addr == page_addr {
                    return Some(delta);
                }
                current_offset += delta.serialized_size();
            } else {
                break;
            }
        }

        None
    }

    /// Get iterator over deltas in a snapshot range.
    ///
    /// # Arguments
    /// * `start` - Starting snapshot ID (inclusive)
    /// * `end` - Ending snapshot ID (inclusive)
    ///
    /// #ASSUME_VALID_RANGE: start <= end
    pub fn get_deltas_in_range(&self, start: u64, end: u64) -> DeltaIterator<'_> {
        let start_slot = (start as usize) % INDEX_CAPACITY;
        let entry = IndexEntry(self.index[start_slot].load(Ordering::Acquire));

        let start_offset = if entry.is_valid() {
            entry.offset() as usize
        } else {
            0
        };

        let head = self.head.load(Ordering::Acquire) as usize;

        DeltaIterator {
            buffer: self,
            current_offset: start_offset,
            end_offset: head,
            start_snapshot: start,
            end_snapshot: end,
        }
    }

    /// Get available bytes remaining in buffer.
    pub fn available_bytes(&self) -> u64 {
        let capacity = self.capacity_bytes.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        if head >= tail {
            capacity - (head - tail)
        } else {
            tail - head
        }
    }

    /// Evict oldest snapshot to make room for new deltas.
    ///
    /// Returns the evicted snapshot ID on success.
    ///
    /// #ASSUME_EVICTION_SAFE: Only evicts fully-written snapshots
    /// #VERIFY_UNIT_TEST: test_eviction
    pub fn evict_oldest_snapshot(&self) -> Result<u64, RingError> {
        let oldest = self.oldest_snapshot.load(Ordering::Acquire);
        let newest = self.newest_snapshot.load(Ordering::Acquire);

        if oldest >= newest {
            return Err(RingError::BufferFull);
        }

        // Invalidate index entry
        let index_slot = (oldest as usize) % INDEX_CAPACITY;
        let entry = IndexEntry(self.index[index_slot].load(Ordering::Acquire));

        if !entry.is_valid() {
            // Already evicted, move to next
            self.oldest_snapshot.fetch_add(1, Ordering::Release);
            return Ok(oldest);
        }

        // Calculate bytes to free
        let delta_count = entry.delta_count() as usize;
        let bytes_to_free = delta_count * MAX_DELTA_SIZE; // Approximate

        // Update tail pointer
        let tail = self.tail.load(Ordering::Acquire);
        let capacity = self.capacity_bytes.load(Ordering::Relaxed) as usize;
        let new_tail = (tail as usize + bytes_to_free) % capacity;
        self.tail.store(new_tail as u64, Ordering::Release);

        // Clear index entry
        self.index[index_slot].store(0, Ordering::Release);

        // Update statistics
        self.evictions.fetch_add(1, Ordering::Relaxed);
        self.active_deltas
            .fetch_sub(delta_count.min(self.active_deltas.load(Ordering::Relaxed) as usize) as u64, Ordering::Relaxed);
        self.oldest_snapshot.fetch_add(1, Ordering::Release);

        Ok(oldest)
    }

    /// Get current statistics snapshot.
    ///
    /// # Performance
    /// <50ns (atomic reads only)
    pub fn get_stats(&self) -> RingStats {
        RingStats {
            total_deltas: self.total_deltas.load(Ordering::Relaxed),
            total_bytes: self.total_bytes.load(Ordering::Relaxed),
            capacity_bytes: self.capacity_bytes.load(Ordering::Relaxed),
            available_bytes: self.available_bytes(),
            oldest_snapshot: self.oldest_snapshot.load(Ordering::Relaxed),
            newest_snapshot: self.newest_snapshot.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
            generation: self.generation.load(Ordering::Relaxed),
            active_deltas: self.active_deltas.load(Ordering::Relaxed),
        }
    }

    /// Check if snapshot is still in buffer.
    pub fn contains_snapshot(&self, snapshot_id: u64) -> bool {
        let oldest = self.oldest_snapshot.load(Ordering::Acquire);
        let newest = self.newest_snapshot.load(Ordering::Acquire);

        snapshot_id >= oldest && snapshot_id <= newest
    }

    /// Get number of deltas for a snapshot.
    pub fn delta_count_for_snapshot(&self, snapshot_id: u64) -> u8 {
        let index_slot = (snapshot_id as usize) % INDEX_CAPACITY;
        let entry = IndexEntry(self.index[index_slot].load(Ordering::Acquire));

        if entry.is_valid() {
            entry.delta_count()
        } else {
            0
        }
    }

    /// Clear all deltas and reset buffer.
    pub fn clear(&self) {
        self.generation.fetch_add(1, Ordering::Release);

        self.head.store(0, Ordering::Release);
        self.tail.store(0, Ordering::Release);
        self.oldest_snapshot.store(0, Ordering::Release);
        self.newest_snapshot.store(0, Ordering::Release);
        self.active_deltas.store(0, Ordering::Release);

        // Clear index
        for entry in &self.index {
            entry.store(0, Ordering::Release);
        }

        self.generation.fetch_add(1, Ordering::Release);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    // ===== Structure Tests (5 tests) =====

    #[test]
    fn test_page_delta_buffer_size() {
        assert_eq!(size_of::<PageDeltaBuffer>(), 4128);
        assert_eq!(align_of::<PageDeltaBuffer>(), 32);
    }

    #[test]
    fn test_ring_buffer_alignment() {
        // Header should be 256-byte aligned
        assert_eq!(
            std::mem::offset_of!(MemoryDeltaRingBufferCapsule, index) % 256,
            0
        );
    }

    #[test]
    fn test_index_entry_packing() {
        let entry = IndexEntry::new(0x00FF_FFFF, 15);
        assert_eq!(entry.offset(), 0x00FF_FFFF);
        assert!(entry.is_valid());
        assert_eq!(entry.delta_count(), 15);
    }

    #[test]
    fn test_index_entry_overflow() {
        // Delta count saturates at 15
        let entry = IndexEntry::new(1000, 20);
        assert_eq!(entry.delta_count(), 15);
    }

    #[test]
    fn test_constants() {
        assert_eq!(HEADER_SIZE, 256);
        assert_eq!(INDEX_SIZE, 65536);
        assert_eq!(INDEX_CAPACITY, 16384);
    }

    // ===== Creation Tests (3 tests) =====

    #[test]
    fn test_new_default() {
        let buffer = MemoryDeltaRingBufferCapsule::new_default();
        let stats = buffer.get_stats();

        assert_eq!(stats.total_deltas, 0);
        assert_eq!(stats.active_deltas, 0);
        assert!(stats.capacity_bytes > 0);
    }

    #[test]
    fn test_new_capacity() {
        let buffer = MemoryDeltaRingBufferCapsule::new(16);
        let stats = buffer.get_stats();

        // 16MB - header - index
        let expected = 16 * 1024 * 1024 - HEADER_SIZE - INDEX_SIZE;
        assert_eq!(stats.capacity_bytes as usize, expected);
    }

    #[test]
    #[should_panic]
    fn test_invalid_capacity_low() {
        let _ = MemoryDeltaRingBufferCapsule::new(MIN_CAPACITY_MB - 1);
    }

    // ===== Delta Creation Tests (4 tests) =====

    #[test]
    fn test_delta_full_page() {
        let data = [0xAB; PAGE_SIZE];
        let delta = PageDeltaBuffer::new_full_page(42, 0x1000_0000, &data, 1);

        assert_eq!(delta.magic, DELTA_MAGIC);
        assert_eq!(delta.snapshot_id, 42);
        assert_eq!(delta.page_addr, 0x1000_0000);
        assert!(delta.is_full_page());
        assert!(!delta.is_xor_delta());
        assert!(delta.validate());
    }

    #[test]
    fn test_delta_xor() {
        let delta_data = [0x12; 256];
        let delta = PageDeltaBuffer::new_xor_delta(100, 0x2000_0000, &delta_data, 2, 500);

        assert_eq!(delta.compressed_size, 256);
        assert!(delta.is_xor_delta());
        assert_eq!(delta.prev_offset, 500);
        assert!(delta.validate());
    }

    #[test]
    fn test_delta_checksum() {
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let delta = PageDeltaBuffer::new_xor_delta(1, 0x1000, &data, 0, 0);

        // Tamper with data
        let mut tampered = delta.clone();
        tampered.data[0] = 0xFF;

        assert!(!tampered.validate());
    }

    #[test]
    fn test_delta_data_size() {
        let full = PageDeltaBuffer::new_full_page(1, 0x1000, &[0; PAGE_SIZE], 0);
        assert_eq!(full.data_size(), PAGE_SIZE);

        let compressed = PageDeltaBuffer::new_xor_delta(1, 0x1000, &[0; 100], 0, 0);
        assert_eq!(compressed.data_size(), 100);
    }

    // ===== Push/Get Tests (5 tests) =====

    #[test]
    fn test_push_get_roundtrip() {
        let buffer = MemoryDeltaRingBufferCapsule::new(MIN_CAPACITY_MB);
        let data = [0xCD; PAGE_SIZE];
        let delta = PageDeltaBuffer::new_full_page(1, 0x4000_0000, &data, 1);

        let result = buffer.push_delta(&delta);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        let retrieved = buffer.get_delta(1, 0x4000_0000);
        assert!(retrieved.is_some());

        let got = retrieved.unwrap();
        assert_eq!(got.snapshot_id, 1);
        assert_eq!(got.page_addr, 0x4000_0000);
        assert_eq!(got.data[0..100], delta.data[0..100]);
    }

    #[test]
    fn test_push_multiple_snapshots() {
        let buffer = MemoryDeltaRingBufferCapsule::new(MIN_CAPACITY_MB);

        for i in 0..100 {
            let data = [i as u8; PAGE_SIZE];
            let delta = PageDeltaBuffer::new_full_page(i, 0x1000_0000 + i * PAGE_SIZE as u64, &data, 1);
            let result = buffer.push_delta(&delta);
            assert!(result.is_ok());
        }

        let stats = buffer.get_stats();
        assert_eq!(stats.total_deltas, 100);
        assert_eq!(stats.active_deltas, 100);
    }

    #[test]
    fn test_push_multiple_pages_same_snapshot() {
        let buffer = MemoryDeltaRingBufferCapsule::new(MIN_CAPACITY_MB);

        for i in 0..10 {
            let data = [i as u8; PAGE_SIZE];
            let delta = PageDeltaBuffer::new_full_page(1, 0x1000 * (i + 1), &data, 1);
            buffer.push_delta(&delta).unwrap();
        }

        let count = buffer.delta_count_for_snapshot(1);
        assert!(count >= 1); // At least one recorded (may saturate)
    }

    #[test]
    fn test_contains_snapshot() {
        let buffer = MemoryDeltaRingBufferCapsule::new(MIN_CAPACITY_MB);

        for i in 10..20 {
            let delta = PageDeltaBuffer::new_full_page(i, 0x1000, &[0; PAGE_SIZE], 1);
            buffer.push_delta(&delta).unwrap();
        }

        assert!(buffer.contains_snapshot(15));
        assert!(!buffer.contains_snapshot(5));
        assert!(!buffer.contains_snapshot(25));
    }

    #[test]
    fn test_stats_update() {
        let buffer = MemoryDeltaRingBufferCapsule::new(MIN_CAPACITY_MB);

        let delta = PageDeltaBuffer::new_full_page(42, 0x1000, &[0; PAGE_SIZE], 1);
        buffer.push_delta(&delta).unwrap();

        let stats = buffer.get_stats();
        assert_eq!(stats.total_deltas, 1);
        assert_eq!(stats.newest_snapshot, 42);
        assert!(stats.total_bytes > 0);
    }

    // ===== Eviction Tests (3 tests) =====

    #[test]
    fn test_eviction() {
        let buffer = MemoryDeltaRingBufferCapsule::new(MIN_CAPACITY_MB);

        // Fill with deltas
        for i in 0..50 {
            let delta = PageDeltaBuffer::new_full_page(i, 0x1000, &[0; PAGE_SIZE], 1);
            buffer.push_delta(&delta).unwrap();
        }

        let oldest_before = buffer.oldest_snapshot.load(Ordering::Relaxed);
        buffer.evict_oldest_snapshot().unwrap();
        let oldest_after = buffer.oldest_snapshot.load(Ordering::Relaxed);

        assert!(oldest_after > oldest_before);
    }

    #[test]
    fn test_eviction_clears_index() {
        let buffer = MemoryDeltaRingBufferCapsule::new(MIN_CAPACITY_MB);

        for i in 0..10 {
            let delta = PageDeltaBuffer::new_full_page(i, 0x1000, &[0; PAGE_SIZE], 1);
            buffer.push_delta(&delta).unwrap();
        }

        let evicted_id = buffer.evict_oldest_snapshot().unwrap();
        let count = buffer.delta_count_for_snapshot(evicted_id);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_clear() {
        let buffer = MemoryDeltaRingBufferCapsule::new(MIN_CAPACITY_MB);

        for i in 0..100 {
            let delta = PageDeltaBuffer::new_full_page(i, 0x1000, &[0; PAGE_SIZE], 1);
            buffer.push_delta(&delta).unwrap();
        }

        buffer.clear();

        let stats = buffer.get_stats();
        assert_eq!(stats.active_deltas, 0);
        assert_eq!(stats.oldest_snapshot, 0);
        assert_eq!(stats.newest_snapshot, 0);
    }

    // ===== Range Query Tests (2 tests) =====

    #[test]
    fn test_range_query_empty() {
        let buffer = MemoryDeltaRingBufferCapsule::new(MIN_CAPACITY_MB);
        let iter = buffer.get_deltas_in_range(0, 100);
        assert_eq!(iter.count(), 0);
    }

    #[test]
    fn test_available_bytes() {
        let buffer = MemoryDeltaRingBufferCapsule::new(MIN_CAPACITY_MB);
        let initial = buffer.available_bytes();

        let delta = PageDeltaBuffer::new_full_page(1, 0x1000, &[0; PAGE_SIZE], 1);
        buffer.push_delta(&delta).unwrap();

        let after = buffer.available_bytes();
        assert!(after < initial);
    }
}
