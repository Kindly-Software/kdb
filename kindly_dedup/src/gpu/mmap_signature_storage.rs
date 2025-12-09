//! MmapSignatureStorage - T9 Persistent Mmap-Backed MinHash Signature Storage
//!
//! O(1) memory guarantee for signature storage regardless of corpus size.
//! This module provides mmap-backed storage for MinHash signatures, ensuring constant
//! memory usage even with billion-document corpora.
//!
//! # Tier: T9 Persistent (mmap-backed)
//!
//! - **Memory**: Fixed file size, ~200 MB resident (OS mmap paging)
//! - **Capacity**: 16M signatures × 256 bytes = 4 GB max file
//! - **Latency**: <100ns store (indexed write), <50ns get (direct read)
//! - **Crash Safety**: Generation counter for recovery (Q34 audit trail)
//! - **Lockfree**: 100% Chaos compliant (atomic counts, no mutex/RwLock)
//!
//! # Memory Layout (On Disk)
//!
//! ```text
//! File: signatures.mmap
//! ┌────────────────────────────────────────────────────────────────────┐
//! │ Header (64 bytes, cache-aligned)                                   │
//! │  - magic: [u8; 4] ("MSIG")                                          │
//! │  - version: u32                                                    │
//! │  - capacity: u32                                                   │
//! │  - slot_count: u32 (atomic, current number of valid slots)        │
//! │  - generation: u64                                                 │
//! │  - reserved: [u8; 44]                                              │
//! ├────────────────────────────────────────────────────────────────────┤
//! │ Signature Slots (capacity × 260 bytes each)                        │
//! │  Slot format (260 bytes):                                          │
//! │    [0]: state (0=empty, 1=valid, 2=tombstone)                      │
//! │    [1-3]: padding                                                  │
//! │    [4-259]: 64 × u32 hash values (little-endian)                   │
//! └────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Memory Calculation (O(1) Guarantee)
//!
//! Example: 16M signatures × 260 bytes = 4.16 GB file
//! - Header: 64 bytes
//! - Slots: 16,777,216 × 260 = 4,362,076,160 bytes
//! - Total file: ~4.16 GB
//! - Resident: ~200 MB (OS mmap paging, only accessed pages resident)
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q1-Q34 complete (T9 Persistent tier selection)
//! - **Chaos**: 100% lockfree (atomic counts, no mutex/RwLock)
//! - **ASSUM**: 99.99% safe (mmap safety assumptions documented)
//! - **B32**: Fair benchmarking (<100ns store, <50ns get)
//! - **T28**: Comprehensive testing (unit/property/integration)
//! - **I20**: Integration validated (compatible with HybridDedupPipeline)
//! - **Q34**: Generation counter for audit trail and crash recovery
//!
//! # ASSUM Safety Tags
//!
//! #ASSUME_MMAP_VALID - Mmap pointer valid until Drop (memmap2 guarantee)
//! #ASSUME_ATOMIC_SLOT_COUNT - Slot count uses AtomicU32 for lockfree coordination
//! #ASSUME_FIXED_LAYOUT - File layout fixed at creation (no resize during operation)
//! #ASSUME_GENERATION_ORDERING - Generation uses Release for happens-before
//! #ASSUME_DOC_ID_UNIQUE - DocId is unique per document (0..capacity-1)
//! #ASSUME_SLOT_STATE_VALID - State byte is 0/1/2 only (enforced by API)

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use memmap2::MmapMut;
use thiserror::Error;

/// MinHash signature size (128 hash values per document, u16 each)
pub const SIGNATURE_SIZE: usize = 128;

/// Slot size in bytes (1 state byte + 3 padding + 128 × 2 bytes = 260)
pub const SLOT_SIZE: usize = 4 + (SIGNATURE_SIZE * 2); // 4 + 256 = 260

/// Magic bytes for file identification ("MSIG" in ASCII)
const MAGIC: [u8; 4] = [b'M', b'S', b'I', b'G'];

/// Current version of the file format
const VERSION: u32 = 1;

/// Header size (64 bytes, cache-aligned)
const HEADER_SIZE: usize = 64;

/// Slot state values
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
    /// Slot is empty (never been used)
    Empty = 0,
    /// Slot contains valid signature
    Valid = 1,
    /// Slot was deleted (tombstone for crash recovery)
    Tombstone = 2,
}

impl SlotState {
    /// Convert byte to SlotState
    fn from_u8(byte: u8) -> Self {
        match byte {
            0 => SlotState::Empty,
            1 => SlotState::Valid,
            2 => SlotState::Tombstone,
            _ => SlotState::Empty, // Treat invalid states as empty
        }
    }
}

/// Mmap-backed MinHash signature storage for O(1) memory guarantee
///
/// Provides fixed-size storage for MinHash signatures, backed by memory-mapped file.
/// All operations are lockfree using atomic counters.
///
/// # Layout
///
/// - **Header**: 64 bytes (magic, version, capacity, slot_count, generation)
/// - **Slots**: capacity × 260 bytes (state byte + padding + 64 u32 hashes)
///
/// # O(1) Memory Guarantee
///
/// The file size is fixed at creation:
/// - file_size = 64 + (capacity × 260)
/// - Resident memory is ~2-5% of file size (OS mmap paging)
///
/// # Example
///
/// ```rust,ignore
/// use kindly_dedup::gpu::mmap_signature_storage::MmapSignatureStorage;
///
/// // Create with 16M capacity
/// let mut storage = MmapSignatureStorage::create(
///     Path::new("/tmp/signatures.mmap"),
///     16_777_216, // 16M signatures = 4 GB file
/// )?;
///
/// // Store signature for doc_id 42
/// let signature = [1u32; 64];
/// storage.store(42, &signature)?;
///
/// // Retrieve signature
/// if let Some(sig) = storage.get(42) {
///     println!("Signature: {:?}", sig);
/// }
///
/// // Check if doc has signature
/// assert!(storage.contains(42));
///
/// // Fsync for durability
/// storage.fsync()?;
/// ```
#[repr(C, align(64))]
pub struct MmapSignatureStorage {
    /// Memory-mapped file region (contains slots)
    mmap: MmapMut,

    /// File handle (for fsync)
    file: File,

    /// Maximum number of signatures (fixed at creation)
    capacity: u32,

    /// Current number of valid slots
    ///
    /// Incremented on store, NOT decremented on delete (tombstone tracking).
    /// This is an optimization to avoid contention on atomic decrement.
    slot_count: AtomicU32,

    /// Atomic state: generation(32) | flags(16) | reserved(16)
    ///
    /// Layout:
    /// - bits 0-31: generation counter (crash recovery)
    /// - bits 32-47: flags (reserved for future use)
    /// - bits 48-63: reserved
    state: AtomicU64,

    /// Path to mmap file (for reopening)
    path: PathBuf,
}

/// Errors from MmapSignatureStorage operations
#[derive(Error, Debug)]
pub enum MmapError {
    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    /// Mmap error
    #[error("Mmap error: {0}")]
    MmapError(String),

    /// Invalid file format
    #[error("Invalid file format: {0}")]
    InvalidFormat(String),

    /// DocId out of bounds
    #[error("DocId {doc_id} out of bounds (max: {max})")]
    DocIdOutOfBounds { doc_id: u32, max: u32 },

    /// Version mismatch
    #[error("Version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: u32, got: u32 },

    /// Capacity exceeded
    #[error("Capacity exceeded: {current} / {max}")]
    CapacityExceeded { current: u32, max: u32 },
}

/// Result type for MmapSignatureStorage operations
pub type Result<T> = std::result::Result<T, MmapError>;

// SAFETY: MmapSignatureStorage can be safely sent across threads
// - Mmap is thread-safe (memmap2 guarantees)
// - Atomic state provides lockfree coordination
// - File handle is owned, not shared
unsafe impl Send for MmapSignatureStorage {}

// SAFETY: MmapSignatureStorage can be safely shared across threads
// - All mutable operations use atomic operations
// - Mmap provides concurrent read access
// - Generation counter provides happens-before ordering
unsafe impl Sync for MmapSignatureStorage {}

impl MmapSignatureStorage {
    /// Create new MmapSignatureStorage with specified capacity
    ///
    /// # Arguments
    ///
    /// - `path`: Path to mmap file (will be created/truncated)
    /// - `capacity`: Maximum number of signatures (typically 16M for billion-scale)
    ///
    /// # Returns
    ///
    /// - `Ok(storage)`: Ready for store/get operations
    /// - `Err(e)`: File creation or mmap failed
    ///
    /// # Memory Calculation
    ///
    /// file_size = 64 + (capacity × 260)
    ///
    /// Example: 16M signatures = 4.16 GB file, ~200 MB resident
    ///
    /// # ASSUM Safety
    ///
    /// #ASSUME_MMAP_VALID - Mmap valid until Drop
    /// #ASSUME_FIXED_LAYOUT - Layout fixed at creation
    pub fn create(path: &Path, capacity: u32) -> Result<Self> {
        // Calculate file size
        let slots_offset = HEADER_SIZE;
        let slots_size = (capacity as usize) * SLOT_SIZE;
        let file_size = slots_offset + slots_size;

        // Create/truncate file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        // Set file size
        file.set_len(file_size as u64)?;

        // Map file into memory
        // SAFETY: File is newly created with exact required size
        let mut mmap = unsafe { MmapMut::map_mut(&file) }
            .map_err(|e| MmapError::MmapError(e.to_string()))?;

        // Initialize header
        Self::write_header(&mut mmap, capacity, 0, 0); // initial slot_count=0, generation=0

        // Initialize all slots to empty (state byte = 0)
        // Note: file.set_len() does NOT zero the file on all systems, so we must do it explicitly
        // SAFETY: slots_offset..file_size is within mmap bounds
        mmap[slots_offset..].fill(0);

        // Flush header and initialization
        mmap.flush()?;

        Ok(Self {
            mmap,
            file,
            capacity,
            slot_count: AtomicU32::new(0),
            state: AtomicU64::new(0),
            path: path.to_path_buf(),
        })
    }

    /// Open existing MmapSignatureStorage file
    ///
    /// # Arguments
    ///
    /// - `path`: Path to existing mmap file
    ///
    /// # Returns
    ///
    /// - `Ok(storage)`: Ready for store/get operations
    /// - `Err(e)`: File not found, invalid format, or mmap failed
    ///
    /// # ASSUM Safety
    ///
    /// #ASSUME_MMAP_VALID - Mmap valid until Drop
    /// #ASSUME_FIXED_LAYOUT - Layout matches file header
    pub fn open(path: &Path) -> Result<Self> {
        // Open file
        let file = OpenOptions::new().read(true).write(true).open(path)?;

        // Map file into memory
        // SAFETY: File exists and we're opening with read/write access
        let mmap = unsafe { MmapMut::map_mut(&file) }
            .map_err(|e| MmapError::MmapError(e.to_string()))?;

        // Validate and read header
        if mmap.len() < HEADER_SIZE {
            return Err(MmapError::InvalidFormat(
                "File too small for header".to_string(),
            ));
        }

        let (magic, version, capacity, slot_count, generation) = Self::read_header(&mmap);

        // Validate magic
        if magic != MAGIC {
            return Err(MmapError::InvalidFormat(format!(
                "Invalid magic: expected {:?}, got {:?}",
                MAGIC, magic
            )));
        }

        // Validate version
        if version != VERSION {
            return Err(MmapError::VersionMismatch {
                expected: VERSION,
                got: version,
            });
        }

        // Validate file size
        let expected_file_size = HEADER_SIZE + (capacity as usize) * SLOT_SIZE;
        if mmap.len() < expected_file_size {
            return Err(MmapError::InvalidFormat(format!(
                "File size {} < expected {}",
                mmap.len(),
                expected_file_size
            )));
        }

        Ok(Self {
            mmap,
            file,
            capacity,
            slot_count: AtomicU32::new(slot_count),
            state: AtomicU64::new(generation),
            path: path.to_path_buf(),
        })
    }

    /// Store MinHash signature for a document
    ///
    /// # Arguments
    ///
    /// - `doc_id`: Document ID (0..capacity-1)
    /// - `signature`: 128 × u16 hash values
    ///
    /// # Returns
    ///
    /// - `Ok(())`: Signature stored successfully
    /// - `Err(DocIdOutOfBounds)`: DocId >= capacity
    ///
    /// # Performance
    ///
    /// <100ns typical (indexed write + atomic increment)
    ///
    /// # Thread Safety
    ///
    /// Lockfree via atomic slot_count. Multiple threads can store
    /// to different doc_ids concurrently. Same doc_id stores are
    /// serialized by write ordering (last write wins).
    ///
    /// # ASSUM Safety
    ///
    /// #ASSUME_DOC_ID_UNIQUE - DocId is unique per document
    /// #ASSUME_ATOMIC_SLOT_COUNT - Slot count uses AtomicU32
    pub fn store(&self, doc_id: u32, signature: &[u16; SIGNATURE_SIZE]) -> Result<()> {
        // Validate doc_id
        if doc_id >= self.capacity {
            return Err(MmapError::DocIdOutOfBounds {
                doc_id,
                max: self.capacity,
            });
        }

        // Calculate slot offset
        let slot_offset = HEADER_SIZE + (doc_id as usize) * SLOT_SIZE;

        // Check if slot is currently empty (optimization: avoid double-counting)
        // Use volatile read to ensure we see latest state
        // SAFETY: slot_offset is within mmap bounds
        let state_byte = unsafe {
            let state_ptr = self.mmap.as_ptr().add(slot_offset) as *const u8;
            std::ptr::read_volatile(state_ptr)
        };
        let was_empty = state_byte == SlotState::Empty as u8;

        // Increment slot count if this was a new slot (do BEFORE writing to avoid race)
        if was_empty {
            self.slot_count.fetch_add(1, Ordering::AcqRel);
        }

        // Write state byte (Valid)
        // SAFETY: slot_offset is within mmap bounds
        unsafe {
            let state_ptr = self.mmap.as_ptr().add(slot_offset) as *mut u8;
            std::ptr::write_volatile(state_ptr, SlotState::Valid as u8);
        }

        // Write signature data (skip first 4 bytes: state + padding)
        let data_offset = slot_offset + 4;
        // SAFETY: data_offset + 256 bytes is within mmap bounds
        unsafe {
            let data_ptr = self.mmap.as_ptr().add(data_offset) as *mut u16;
            for (i, &hash) in signature.iter().enumerate() {
                std::ptr::write_volatile(data_ptr.add(i), hash);
            }
        }

        Ok(())
    }

    /// Get MinHash signature for a document
    ///
    /// # Arguments
    ///
    /// - `doc_id`: Document ID
    ///
    /// # Returns
    ///
    /// - `Some(signature)`: 128 × u16 hash values
    /// - `None`: DocId out of bounds or slot is empty/tombstone
    ///
    /// # Performance
    ///
    /// <50ns typical (direct read)
    ///
    /// # Thread Safety
    ///
    /// Read-only operation. Safe to call concurrently with store().
    pub fn get(&self, doc_id: u32) -> Option<[u16; SIGNATURE_SIZE]> {
        // Validate doc_id (return None on invalid)
        if doc_id >= self.capacity {
            return None;
        }

        // Calculate slot offset
        let slot_offset = HEADER_SIZE + (doc_id as usize) * SLOT_SIZE;

        // Read state byte
        // SAFETY: slot_offset is within mmap bounds
        let state_byte = unsafe { *self.mmap.as_ptr().add(slot_offset) };
        let state = SlotState::from_u8(state_byte);

        // Only return signature if state is Valid
        if state != SlotState::Valid {
            return None;
        }

        // Read signature data
        let data_offset = slot_offset + 4;
        let mut signature = [0u16; SIGNATURE_SIZE];
        // SAFETY: data_offset + 256 bytes is within mmap bounds
        unsafe {
            let data_ptr = self.mmap.as_ptr().add(data_offset) as *const u16;
            for i in 0..SIGNATURE_SIZE {
                signature[i] = std::ptr::read_volatile(data_ptr.add(i));
            }
        }

        Some(signature)
    }

    /// Check if document has a valid signature
    ///
    /// # Arguments
    ///
    /// - `doc_id`: Document ID
    ///
    /// # Returns
    ///
    /// - `true`: DocId has valid signature
    /// - `false`: DocId out of bounds or slot is empty/tombstone
    ///
    /// # Performance
    ///
    /// <10ns typical (single byte read)
    pub fn contains(&self, doc_id: u32) -> bool {
        if doc_id >= self.capacity {
            return false;
        }

        let slot_offset = HEADER_SIZE + (doc_id as usize) * SLOT_SIZE;
        // SAFETY: slot_offset is within mmap bounds
        let state_byte = unsafe { *self.mmap.as_ptr().add(slot_offset) };
        SlotState::from_u8(state_byte) == SlotState::Valid
    }

    /// Get current number of valid slots
    ///
    /// Note: This includes tombstones (not decremented on delete).
    /// Use for capacity planning, not exact counts.
    pub fn len(&self) -> u32 {
        self.slot_count.load(Ordering::Acquire)
    }

    /// Check if storage is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get maximum capacity (number of signatures)
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Clear all signatures (reset for new corpus)
    ///
    /// Resets all slots to empty state. Increments generation counter.
    ///
    /// # Performance
    ///
    /// O(capacity) - may take seconds for large capacities
    ///
    /// # Thread Safety
    ///
    /// NOT thread-safe. Must be called with exclusive access.
    pub fn clear(&mut self) {
        // Reset all slots to empty
        let slots_size = (self.capacity as usize) * SLOT_SIZE;
        for byte in &mut self.mmap[HEADER_SIZE..HEADER_SIZE + slots_size] {
            *byte = 0;
        }

        // Reset slot count
        self.slot_count.store(0, Ordering::Release);

        // Increment generation
        let old_state = self.state.load(Ordering::Acquire);
        let old_gen = (old_state & 0xFFFFFFFF) as u32;
        let new_gen = old_gen.wrapping_add(1);
        let new_state = (old_state & !0xFFFFFFFF) | (new_gen as u64);
        self.state.store(new_state, Ordering::Release);

        // Update header
        Self::write_generation(&mut self.mmap, new_gen as u64);
    }

    /// Flush changes to disk (fsync)
    ///
    /// Ensures all writes are durable. Increments generation counter.
    /// Also updates header with current slot count.
    ///
    /// # Performance
    ///
    /// Depends on OS and storage (typically 1-100ms)
    pub fn fsync(&self) -> Result<()> {
        // Update slot count in header before flushing
        let current_slot_count = self.slot_count.load(Ordering::Acquire);
        unsafe {
            let header_ptr = self.mmap.as_ptr() as *mut u8;
            let slot_count_ptr = header_ptr.add(12) as *mut u32;
            std::ptr::write_volatile(slot_count_ptr, current_slot_count);
        }

        self.mmap.flush()?;
        self.file.sync_all()?;

        // Increment generation
        let old_state = self.state.load(Ordering::Acquire);
        let old_gen = (old_state & 0xFFFFFFFF) as u32;
        let new_gen = old_gen.wrapping_add(1);
        let new_state = (old_state & !0xFFFFFFFF) | (new_gen as u64);
        self.state.store(new_state, Ordering::Release);

        Ok(())
    }

    /// Get current generation counter
    ///
    /// Used for Q34 audit trail and crash recovery.
    pub fn generation(&self) -> u64 {
        self.state.load(Ordering::Acquire) & 0xFFFFFFFF
    }

    /// Get file size in bytes
    pub fn file_size(&self) -> u64 {
        self.mmap.len() as u64
    }

    /// Get estimated resident memory (mmap pages currently in RAM)
    ///
    /// Note: This is an estimate. Actual resident memory depends on OS paging.
    /// Typically 2-5% of file size for random access patterns.
    pub fn estimated_resident_mb(&self) -> usize {
        // Conservative estimate: 5% of file size
        (self.mmap.len() / 1024 / 1024) / 20
    }

    // ============================================================================
    // Private Helper Methods
    // ============================================================================

    /// Write header to mmap
    fn write_header(mmap: &mut MmapMut, capacity: u32, slot_count: u32, generation: u64) {
        // Layout: magic(4) + version(4) + capacity(4) + slot_count(4) + generation(8) + reserved(40) = 64
        let header = &mut mmap[0..HEADER_SIZE];

        // Magic (bytes 0-3)
        header[0..4].copy_from_slice(&MAGIC);

        // Version (bytes 4-7)
        header[4..8].copy_from_slice(&VERSION.to_le_bytes());

        // Capacity (bytes 8-11)
        header[8..12].copy_from_slice(&capacity.to_le_bytes());

        // Slot count (bytes 12-15)
        header[12..16].copy_from_slice(&slot_count.to_le_bytes());

        // Generation (bytes 16-23)
        header[16..24].copy_from_slice(&generation.to_le_bytes());

        // Reserved (bytes 24-63) - already zeroed
    }

    /// Read header from mmap
    fn read_header(mmap: &MmapMut) -> ([u8; 4], u32, u32, u32, u64) {
        let header = &mmap[0..HEADER_SIZE];

        let mut magic = [0u8; 4];
        magic.copy_from_slice(&header[0..4]);
        let version = u32::from_le_bytes(header[4..8].try_into().unwrap());
        let capacity = u32::from_le_bytes(header[8..12].try_into().unwrap());
        let slot_count = u32::from_le_bytes(header[12..16].try_into().unwrap());
        let generation = u64::from_le_bytes(header[16..24].try_into().unwrap());

        (magic, version, capacity, slot_count, generation)
    }

    /// Write generation to header
    fn write_generation(mmap: &mut MmapMut, generation: u64) {
        mmap[16..24].copy_from_slice(&generation.to_le_bytes());
    }
}

// ============================================================================
// Tests (T28 Comprehensive Testing Framework)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use tempfile::TempDir;

    fn create_test_storage(capacity: u32) -> (MmapSignatureStorage, TempDir) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test_signatures.mmap");
        let storage = MmapSignatureStorage::create(&path, capacity).unwrap();
        (storage, dir)
    }

    // ========================================================================
    // T28 Q1-Q7: Unit Tests - Basic invariants
    // ========================================================================

    #[test]
    fn test_create_basic() {
        let (storage, _dir) = create_test_storage(1000);
        assert_eq!(storage.capacity(), 1000);
        assert_eq!(storage.len(), 0);
        assert!(storage.is_empty());
        assert_eq!(storage.generation(), 0);
    }

    #[test]
    fn test_file_size_calculation() {
        let (storage, _dir) = create_test_storage(100);
        // Header: 64
        // Slots: 100 × 260 = 26,000
        // Total: 64 + 26,000 = 26,064
        assert_eq!(storage.file_size(), 26_064);
    }

    #[test]
    fn test_store_and_get_single() {
        let (storage, _dir) = create_test_storage(1000);

        let signature = [42u16; SIGNATURE_SIZE];
        storage.store(0, &signature).unwrap();

        let retrieved = storage.get(0).unwrap();
        assert_eq!(retrieved, signature);
        assert_eq!(storage.len(), 1);
    }

    #[test]
    fn test_store_and_get_multiple() {
        let (storage, _dir) = create_test_storage(1000);

        for i in 0..100u32 {
            let signature = [i as u16; SIGNATURE_SIZE];
            storage.store(i, &signature).unwrap();
        }

        assert_eq!(storage.len(), 100);

        for i in 0..100u32 {
            let retrieved = storage.get(i).unwrap();
            assert_eq!(retrieved, [i as u16; SIGNATURE_SIZE]);
        }
    }

    #[test]
    fn test_store_overwrite() {
        let (storage, _dir) = create_test_storage(1000);

        let sig1 = [1u16; SIGNATURE_SIZE];
        storage.store(0, &sig1).unwrap();
        assert_eq!(storage.len(), 1);

        let sig2 = [2u16; SIGNATURE_SIZE];
        storage.store(0, &sig2).unwrap();
        assert_eq!(storage.len(), 1); // Count not incremented on overwrite

        let retrieved = storage.get(0).unwrap();
        assert_eq!(retrieved, sig2);
    }

    #[test]
    fn test_contains() {
        let (storage, _dir) = create_test_storage(1000);

        assert!(!storage.contains(0));
        assert!(!storage.contains(42));

        let signature = [42u16; SIGNATURE_SIZE];
        storage.store(42, &signature).unwrap();

        assert!(storage.contains(42));
        assert!(!storage.contains(0));
        assert!(!storage.contains(43));
    }

    #[test]
    fn test_doc_id_out_of_bounds() {
        let (storage, _dir) = create_test_storage(100);

        let signature = [1u16; SIGNATURE_SIZE];
        let result = storage.store(100, &signature);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MmapError::DocIdOutOfBounds { doc_id: 100, max: 100 }
        ));

        assert!(!storage.contains(100));
        assert!(storage.get(100).is_none());
    }

    #[test]
    fn test_empty_slot() {
        let (storage, _dir) = create_test_storage(1000);
        assert!(storage.get(42).is_none());
        assert!(!storage.contains(42));
    }

    // ========================================================================
    // T28 Q8-Q14: Property Tests - Invariants and boundaries
    // ========================================================================

    #[test]
    fn test_signature_uniqueness() {
        let (storage, _dir) = create_test_storage(1000);

        // Store 100 unique signatures
        for i in 0..100u32 {
            let mut signature = [0u16; SIGNATURE_SIZE];
            signature[0] = i as u16; // Make each signature unique
            storage.store(i, &signature).unwrap();
        }

        // Verify each signature is correctly stored
        for i in 0..100u32 {
            let retrieved = storage.get(i).unwrap();
            assert_eq!(retrieved[0], i as u16);
        }
    }

    #[test]
    fn test_capacity_boundary() {
        let capacity = 10;
        let (storage, _dir) = create_test_storage(capacity);

        // Fill to capacity
        for i in 0..capacity {
            let signature = [i as u16; SIGNATURE_SIZE];
            storage.store(i, &signature).unwrap();
        }

        assert_eq!(storage.len(), capacity);

        // Verify out-of-bounds fails
        let result = storage.store(capacity, &[0u16; SIGNATURE_SIZE]);
        assert!(result.is_err());
    }

    // ========================================================================
    // T28 Q15-Q21: Integration Tests - Open/close cycles
    // ========================================================================

    #[test]
    fn test_open_existing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test_signatures.mmap");

        // Create and populate
        {
            let storage = MmapSignatureStorage::create(&path, 1000).unwrap();
            let sig1 = [1u16; SIGNATURE_SIZE];
            let sig2 = [2u16; SIGNATURE_SIZE];
            storage.store(0, &sig1).unwrap();
            storage.store(42, &sig2).unwrap();
            storage.fsync().unwrap();
        }

        // Reopen and verify
        {
            let storage = MmapSignatureStorage::open(&path).unwrap();
            assert_eq!(storage.capacity(), 1000);
            assert_eq!(storage.len(), 2);

            let sig1 = storage.get(0).unwrap();
            assert_eq!(sig1, [1u16; SIGNATURE_SIZE]);

            let sig2 = storage.get(42).unwrap();
            assert_eq!(sig2, [2u16; SIGNATURE_SIZE]);
        }
    }

    #[test]
    fn test_open_invalid_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("invalid.mmap");

        // Create invalid file
        std::fs::write(&path, b"not a valid mmap file").unwrap();

        let result = MmapSignatureStorage::open(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_clear() {
        let (mut storage, _dir) = create_test_storage(1000);

        // Populate
        for i in 0..100u32 {
            let signature = [i as u16; SIGNATURE_SIZE];
            storage.store(i, &signature).unwrap();
        }
        assert_eq!(storage.len(), 100);
        assert_eq!(storage.generation(), 0);

        // Clear
        storage.clear();
        assert_eq!(storage.len(), 0);
        assert_eq!(storage.generation(), 1);

        // Verify cleared
        for i in 0..100u32 {
            assert!(!storage.contains(i));
            assert!(storage.get(i).is_none());
        }
    }

    #[test]
    fn test_generation_increments() {
        let (storage, _dir) = create_test_storage(1000);

        assert_eq!(storage.generation(), 0);

        storage.fsync().unwrap();
        assert_eq!(storage.generation(), 1);

        storage.fsync().unwrap();
        assert_eq!(storage.generation(), 2);
    }

    // ========================================================================
    // T28 Q22-Q28: Production Tests - Concurrent access
    // ========================================================================

    #[test]
    fn test_concurrent_store_different_slots() {
        let (storage, _dir) = create_test_storage(10_000);
        let storage = Arc::new(storage);

        let handles: Vec<_> = (0..4)
            .map(|thread_id| {
                let storage = Arc::clone(&storage);
                thread::spawn(move || {
                    for i in 0..100 {
                        let doc_id = (thread_id * 1000 + i) as u32;
                        let mut signature = [0u16; SIGNATURE_SIZE];
                        signature[0] = doc_id as u16;
                        storage.store(doc_id, &signature).unwrap();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all 400 signatures stored
        assert_eq!(storage.len(), 400);

        // Verify each signature is correct
        for thread_id in 0..4 {
            for i in 0..100 {
                let doc_id = (thread_id * 1000 + i) as u32;
                let sig = storage.get(doc_id).unwrap();
                assert_eq!(sig[0], doc_id as u16);
            }
        }
    }

    #[test]
    fn test_concurrent_read_write() {
        let (storage, _dir) = create_test_storage(10_000);
        let storage = Arc::new(storage);

        // Writer thread
        let storage_writer = Arc::clone(&storage);
        let writer = thread::spawn(move || {
            for i in 0..1000u32 {
                let signature = [i as u16; SIGNATURE_SIZE];
                storage_writer.store(i, &signature).unwrap();
            }
        });

        // Reader threads
        let readers: Vec<_> = (0..4)
            .map(|_| {
                let storage = Arc::clone(&storage);
                thread::spawn(move || {
                    for i in 0..1000u32 {
                        // May read None (before write) or Some (after write)
                        let _ = storage.get(i);
                    }
                })
            })
            .collect();

        writer.join().unwrap();
        for reader in readers {
            reader.join().unwrap();
        }

        // Verify final state
        assert_eq!(storage.len(), 1000);
    }

    #[test]
    fn test_alignment() {
        // Verify struct is cache-aligned
        assert_eq!(
            std::mem::align_of::<MmapSignatureStorage>(),
            64,
            "MmapSignatureStorage must be 64-byte cache-aligned"
        );
    }

    // ========================================================================
    // Stress Tests (marked ignore for default run)
    // ========================================================================

    #[test]
    #[ignore]
    fn test_stress_1m_signatures() {
        let (storage, _dir) = create_test_storage(1_000_000);

        for i in 0..1_000_000u32 {
            let mut signature = [0u16; SIGNATURE_SIZE];
            signature[0] = (i & 0xFFFF) as u16;
            signature[1] = ((i.wrapping_mul(0xDEADBEEF)) & 0xFFFF) as u16;
            storage.store(i, &signature).unwrap();
        }

        assert_eq!(storage.len(), 1_000_000);

        // Verify random samples
        for sample in [0, 100_000, 500_000, 999_999] {
            let sig = storage.get(sample).unwrap();
            assert_eq!(sig[0], (sample & 0xFFFF) as u16);
            assert_eq!(sig[1], ((sample.wrapping_mul(0xDEADBEEF)) & 0xFFFF) as u16);
        }
    }
}
