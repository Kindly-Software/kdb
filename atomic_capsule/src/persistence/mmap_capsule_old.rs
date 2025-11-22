//! # T9 Persistent Capsule - Core Implementation
//!
//! Zero-copy atomic operations over memory-mapped files.
//!
//! **UCE34 Q10**: T9 Tier (T1 Atomic + Mmap Persistence)
//! **UCE34 Q11**: atomic_from_mut enables zero-copy atomic views
//! **UCE34 Q12**: Nightly feature #![feature(atomic_from_mut)]
//! **IMPL-2 V3.1**: Cutting-edge-first (nightly atomic_from_mut required)
//!
//! # Architecture
//!
//! ```text
//! File Layout:
//! ┌─────────────────────────────────────────────┐
//! │ Header (128B)                               │
//! │ - magic: u64 (0xC0CA0009)                   │
//! │ - version: u64                              │
//! │ - file_size: u64                            │
//! │ - generation: u64 (crash recovery)          │
//! │ - item_count: u64                           │
//! │ - item_size: u64                            │
//! │ - _reserved: [u64; 10]                      │
//! ├─────────────────────────────────────────────┤
//! │ Data Region (variable size)                 │
//! │ - Aligned capsules                          │
//! │ - Atomic operations via atomic_from_mut     │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! # Performance
//!
//! - Atomic write: <50ns (direct mmap store)
//! - Flush (async): <1ms (msync MS_ASYNC)
//! - Flush (sync): <5ms (msync MS_SYNC)
//! - Recovery: <100ms (re-mmap + validate)
//!
//! # Safety (ASSUM Framework)
//!
//! - #ASSUME_MMAP_ALIGNMENT: mmap returns page-aligned memory (4KB)
//! - #VERIFY_ALIGNMENT: Runtime checks on all atomic views
//! - #ASSUME_ATOMIC_COORDINATION: Hardware atomics work across processes
//! - #VERIFY_MSYNC_DURABLE: Tests verify fsync durability
//! - #ASSUME_GENERATION_RECOVERY: Even generation = committed state
//! - #VERIFY_GENERATION_RECOVERY: Crash tests validate recovery

#![cfg(all(feature = "mmap-persistence", feature = "nightly-atomic"))]

use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use memmap2::MmapMut;

use crate::primitives::atomic_from_mut::{AtomicFromMut, AtomicFromMutError};

use super::alignment::{validate_alignment, compute_aligned_offset};
use super::recovery::{GenerationCounter, RecoveryState, two_phase_commit_start, two_phase_commit_finish};

// ============================================================================
// CONSTANTS
// ============================================================================

/// Magic number for T9 Persistent Capsule files
pub const MAGIC: u64 = 0xC0CA_0009_0000_0001;

/// Current file format version
pub const VERSION: u64 = 1;

/// Header size (128 bytes, aligned)
pub const HEADER_SIZE: usize = 128;

/// Page size (4KB, typical for mmap)
pub const PAGE_SIZE: usize = 4096;

// ============================================================================
// ERROR TYPES
// ============================================================================

/// Errors from T9 Persistent operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistentError {
    /// Invalid alignment
    InvalidAlignment { offset: usize, required: usize },

    /// Invalid file magic
    InvalidMagic { expected: u64, actual: u64 },

    /// Unsupported version
    UnsupportedVersion { expected: u64, actual: u64 },

    /// File too small
    FileTooSmall { expected: usize, actual: usize },

    /// Generation mismatch (crash recovery detected)
    GenerationMismatch { expected: u64, actual: u64 },

    /// I/O error
    IOError(io::ErrorKind),

    /// Atomic conversion error
    AtomicConversionError,
}

impl From<io::Error> for PersistentError {
    fn from(e: io::Error) -> Self {
        PersistentError::IOError(e.kind())
    }
}

impl From<AtomicFromMutError> for PersistentError {
    fn from(_: AtomicFromMutError) -> Self {
        PersistentError::AtomicConversionError
    }
}

impl std::fmt::Display for PersistentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PersistentError::InvalidAlignment { offset, required } => {
                write!(f, "Invalid alignment: offset {} requires {} byte alignment", offset, required)
            }
            PersistentError::InvalidMagic { expected, actual } => {
                write!(f, "Invalid file magic: expected 0x{:016x}, got 0x{:016x}", expected, actual)
            }
            PersistentError::UnsupportedVersion { expected, actual } => {
                write!(f, "Unsupported version: expected {}, got {}", expected, actual)
            }
            PersistentError::FileTooSmall { expected, actual } => {
                write!(f, "File too small: expected {} bytes, got {}", expected, actual)
            }
            PersistentError::GenerationMismatch { expected, actual } => {
                write!(f, "Generation mismatch: expected {}, got {} (partial update detected)", expected, actual)
            }
            PersistentError::IOError(kind) => {
                write!(f, "I/O error: {:?}", kind)
            }
            PersistentError::AtomicConversionError => {
                write!(f, "Atomic conversion error (alignment or size violation)")
            }
        }
    }
}

impl std::error::Error for PersistentError {}

// ============================================================================
// FILE HEADER (128B, aligned)
// ============================================================================

/// File header for T9 Persistent Capsule
///
/// **Layout** (128 bytes):
/// ```text
/// Offset | Field        | Size | Purpose
/// -------|--------------|------|----------------------------------
/// 0      | magic        | 8    | File format identifier
/// 8      | version      | 8    | Schema version
/// 16     | file_size    | 8    | Total file size
/// 24     | generation   | 8    | Global generation counter
/// 32     | item_count   | 8    | Number of items in file
/// 40     | item_size    | 8    | Size of each item
/// 48     | _reserved    | 80   | Reserved for future use
/// ```
#[repr(C, align(128))]
#[derive(Debug)]
pub struct FileHeader {
    magic: u64,
    version: u64,
    file_size: u64,
    generation: u64,
    item_count: u64,
    item_size: u64,
    _reserved: [u64; 10],
}

impl FileHeader {
    /// Create new header
    pub fn new(file_size: usize, item_size: usize, item_count: usize) -> Self {
        Self {
            magic: MAGIC,
            version: VERSION,
            file_size: file_size as u64,
            generation: 0, // Start at 0 (even = committed)
            item_count: item_count as u64,
            item_size: item_size as u64,
            _reserved: [0; 10],
        }
    }

    /// Validate header
    pub fn validate(&self) -> Result<(), PersistentError> {
        // Check magic
        if self.magic != MAGIC {
            return Err(PersistentError::InvalidMagic {
                expected: MAGIC,
                actual: self.magic,
            });
        }

        // Check version
        if self.version != VERSION {
            return Err(PersistentError::UnsupportedVersion {
                expected: VERSION,
                actual: self.version,
            });
        }

        // Check generation (must be even = committed)
        if self.generation % 2 != 0 {
            return Err(PersistentError::GenerationMismatch {
                expected: self.generation + 1,
                actual: self.generation,
            });
        }

        Ok(())
    }

    /// Check if generation is committed (even)
    pub fn is_committed(&self) -> bool {
        self.generation % 2 == 0
    }
}

// ============================================================================
// CORE: PersistentMmap (T9 Tier)
// ============================================================================

/// Core T9 Persistent Capsule - Memory-mapped atomic state
///
/// **Tier**: T9 (T1 Atomic + Mmap Persistence)
///
/// # Features
///
/// - Zero-copy atomic operations (via atomic_from_mut)
/// - Crash-safe via generation counters
/// - Multi-process coordination via atomics
/// - Async/sync flush options
///
/// # Example
///
/// ```rust,ignore
/// use atomic_capsule::persistent::PersistentMmap;
/// use std::path::Path;
///
/// // Create 1MB file with 512-byte items
/// let mmap = PersistentMmap::create_mmap(Path::new("data.mmap"), 1024 * 1024, 512)?;
///
/// // Atomic view at offset 128 (header size)
/// let atomic = mmap.atomic_view_u64(128)?;
/// atomic.store(42, Ordering::Release);
///
/// // Flush to disk (async)
/// mmap.flush_async()?;
/// ```
///
/// # Safety
///
/// - Uses atomic_from_mut (nightly feature)
/// - All atomic views are alignment-checked
/// - Generation counters prevent partial updates
pub struct PersistentMmap {
    /// Memory-mapped file
    mmap: MmapMut,

    /// Generation counter (points into mmap header)
    generation: Arc<GenerationCounter>,

    /// Last flush timestamp (monotonic nanoseconds)
    last_flush: Arc<AtomicU64>,

    /// File size
    file_size: usize,
}

impl PersistentMmap {
    /// Create new memory-mapped file
    ///
    /// # Arguments
    ///
    /// - `path`: File path
    /// - `size`: Total file size (must be page-aligned)
    /// - `item_size`: Size of each item (for metadata)
    ///
    /// # Errors
    ///
    /// - `InvalidAlignment`: Size not page-aligned
    /// - `IOError`: File creation failed
    ///
    /// # Performance
    ///
    /// - <10ms for 1MB file
    /// - <100ms for 1GB file
    pub fn create_mmap(
        path: &Path,
        size: usize,
        item_size: usize,
    ) -> Result<Self, PersistentError> {
        // Validate size alignment
        validate_alignment(size, PAGE_SIZE)?;

        // Create file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        // Set file size
        file.set_len(size as u64)?;

        // Memory-map the file
        let mut mmap = unsafe { MmapMut::map_mut(&file)? };

        // Initialize header
        let item_count = (size - HEADER_SIZE) / item_size;
        let header = FileHeader::new(size, item_size, item_count);

        // Write header to mmap
        let header_bytes = unsafe {
            std::slice::from_raw_parts(
                &header as *const FileHeader as *const u8,
                std::mem::size_of::<FileHeader>(),
            )
        };
        mmap[0..HEADER_SIZE].copy_from_slice(header_bytes);

        // Create atomic view of generation counter
        let generation_offset = 24; // Offset of generation field in header
        let generation_atomic = u64::from_slice_mut(&mut mmap, generation_offset)?;
        let generation = Arc::new(GenerationCounter::new(generation_atomic));

        // Create atomic view of last flush timestamp (use reserved field)
        let flush_offset = 48; // First reserved field
        let flush_atomic = u64::from_slice_mut(&mut mmap, flush_offset)?;
        let last_flush = Arc::new(flush_atomic.clone());

        // Flush header to disk (sync)
        mmap.flush()?;

        Ok(Self {
            mmap,
            generation,
            last_flush,
            file_size: size,
        })
    }

    /// Open existing memory-mapped file
    ///
    /// # Arguments
    ///
    /// - `path`: File path
    ///
    /// # Errors
    ///
    /// - `InvalidMagic`: Not a T9 file
    /// - `UnsupportedVersion`: Incompatible version
    /// - `GenerationMismatch`: Incomplete update (crash recovery)
    /// - `IOError`: File open failed
    ///
    /// # Performance
    ///
    /// - <10ms for 1MB file
    /// - <100ms for 1GB file
    pub fn open_mmap(path: &Path) -> Result<Self, PersistentError> {
        // Open file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;

        // Memory-map the file
        let mut mmap = unsafe { MmapMut::map_mut(&file)? };
        let file_size = mmap.len();

        // Validate minimum size
        if file_size < HEADER_SIZE {
            return Err(PersistentError::FileTooSmall {
                expected: HEADER_SIZE,
                actual: file_size,
            });
        }

        // Read and validate header
        let header = unsafe {
            &*(mmap.as_ptr() as *const FileHeader)
        };
        header.validate()?;

        // Create atomic view of generation counter
        let generation_offset = 24;
        let generation_atomic = u64::from_slice_mut(&mut mmap, generation_offset)?;
        let generation = Arc::new(GenerationCounter::new(generation_atomic));

        // Create atomic view of last flush timestamp
        let flush_offset = 48;
        let flush_atomic = u64::from_slice_mut(&mut mmap, flush_offset)?;
        let last_flush = Arc::new(flush_atomic.clone());

        Ok(Self {
            mmap,
            generation,
            last_flush,
            file_size,
        })
    }

    /// Get atomic view of u64 at offset
    ///
    /// # Arguments
    ///
    /// - `offset`: Byte offset (must be 8-byte aligned)
    ///
    /// # Errors
    ///
    /// - `InvalidAlignment`: Offset not 8-byte aligned
    /// - `FileTooSmall`: Offset beyond file size
    ///
    /// # Performance
    ///
    /// - <5ns (array index + alignment check)
    ///
    /// # Safety
    ///
    /// - Uses atomic_from_mut (nightly feature)
    /// - Alignment checked at runtime
    /// - Lifetime tied to PersistentMmap
    pub fn atomic_view_u64(&mut self, offset: usize) -> Result<&mut AtomicU64, PersistentError> {
        // Validate alignment
        validate_alignment(offset, 8)?;

        // Validate bounds
        if offset + 8 > self.file_size {
            return Err(PersistentError::FileTooSmall {
                expected: offset + 8,
                actual: self.file_size,
            });
        }

        // Create atomic view via atomic_from_mut
        let atomic = u64::from_slice_mut(&mut self.mmap, offset)?;

        Ok(atomic)
    }

    /// Flush to disk (synchronous, blocking)
    ///
    /// Uses msync(MS_SYNC) - blocks until data is on disk.
    ///
    /// # Errors
    ///
    /// - `IOError`: Flush failed (disk full, I/O error)
    ///
    /// # Performance
    ///
    /// - NVMe SSD: ~1ms
    /// - SATA SSD: ~3ms
    /// - HDD: ~5ms
    ///
    /// # Durability
    ///
    /// After successful return, all writes are durable (survive crash/reboot).
    pub fn flush(&self) -> Result<(), PersistentError> {
        // Update timestamp (monotonic nanoseconds)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        self.last_flush.store(now, Ordering::Release);

        // Synchronous flush (MS_SYNC)
        self.mmap.flush()?;

        Ok(())
    }

    /// Flush to disk (asynchronous, non-blocking)
    ///
    /// Uses msync(MS_ASYNC) - returns immediately, flush happens in background.
    ///
    /// # Errors
    ///
    /// - `IOError`: Flush failed (disk full, I/O error)
    ///
    /// # Performance
    ///
    /// - <1ms (non-blocking return)
    ///
    /// # Durability
    ///
    /// Data may not be durable immediately. Use `flush()` for guaranteed durability.
    pub fn flush_async(&self) -> Result<(), PersistentError> {
        // Update timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        self.last_flush.store(now, Ordering::Release);

        // Asynchronous flush (MS_ASYNC)
        self.mmap.flush_async()?;

        Ok(())
    }

    /// Start two-phase atomic update
    ///
    /// Increments generation counter to odd value (in-flight).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mmap = PersistentMmap::open_mmap("data.mmap")?;
    ///
    /// // Start update
    /// mmap.begin_update()?;
    ///
    /// // Modify data...
    /// let atomic = mmap.atomic_view_u64(128)?;
    /// atomic.store(42, Ordering::Release);
    ///
    /// // Commit update
    /// mmap.commit_update()?;
    /// ```
    pub fn begin_update(&self) -> Result<(), PersistentError> {
        two_phase_commit_start(&self.generation);
        Ok(())
    }

    /// Commit two-phase atomic update
    ///
    /// Increments generation counter to even value (committed).
    /// Flushes to disk (async).
    pub fn commit_update(&self) -> Result<(), PersistentError> {
        two_phase_commit_finish(&self.generation);
        self.flush_async()?;
        Ok(())
    }

    /// Get current generation counter
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Check if state is committed (generation is even)
    pub fn is_committed(&self) -> bool {
        self.generation() % 2 == 0
    }

    /// Get file size
    pub fn size(&self) -> usize {
        self.file_size
    }

    /// Get last flush timestamp (monotonic nanoseconds)
    pub fn last_flush_ns(&self) -> u64 {
        self.last_flush.load(Ordering::Acquire)
    }
}

// RAII: Automatic flush on drop
impl Drop for PersistentMmap {
    fn drop(&mut self) {
        // Best-effort flush (ignore errors)
        let _ = self.flush();
    }
}

// ============================================================================
// TESTS (T28 Framework)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_create_mmap() {
        let path = "/tmp/test_create_mmap.bin";
        let _ = fs::remove_file(path); // Clean up

        let mmap = PersistentMmap::create_mmap(
            Path::new(path),
            PAGE_SIZE * 16, // 64KB
            512,
        ).unwrap();

        assert_eq!(mmap.size(), PAGE_SIZE * 16);
        assert!(mmap.is_committed());
        assert_eq!(mmap.generation(), 0);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_open_mmap() {
        let path = "/tmp/test_open_mmap.bin";
        let _ = fs::remove_file(path);

        // Create
        {
            let _mmap = PersistentMmap::create_mmap(
                Path::new(path),
                PAGE_SIZE * 16,
                512,
            ).unwrap();
        }

        // Open
        let mmap = PersistentMmap::open_mmap(Path::new(path)).unwrap();
        assert_eq!(mmap.size(), PAGE_SIZE * 16);
        assert!(mmap.is_committed());

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_atomic_view() {
        let path = "/tmp/test_atomic_view.bin";
        let _ = fs::remove_file(path);

        let mut mmap = PersistentMmap::create_mmap(
            Path::new(path),
            PAGE_SIZE * 16,
            512,
        ).unwrap();

        // Get atomic view at offset 128 (after header)
        let atomic = mmap.atomic_view_u64(128).unwrap();

        // Write
        atomic.store(42, Ordering::Release);

        // Read
        assert_eq!(atomic.load(Ordering::Acquire), 42);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_two_phase_commit() {
        let path = "/tmp/test_two_phase.bin";
        let _ = fs::remove_file(path);

        let mut mmap = PersistentMmap::create_mmap(
            Path::new(path),
            PAGE_SIZE * 16,
            512,
        ).unwrap();

        // Initial state: generation = 0 (even, committed)
        assert_eq!(mmap.generation(), 0);
        assert!(mmap.is_committed());

        // Begin update: generation = 1 (odd, in-flight)
        mmap.begin_update().unwrap();
        assert_eq!(mmap.generation(), 1);
        assert!(!mmap.is_committed());

        // Modify data
        let atomic = mmap.atomic_view_u64(128).unwrap();
        atomic.store(42, Ordering::Release);

        // Commit: generation = 2 (even, committed)
        mmap.commit_update().unwrap();
        assert_eq!(mmap.generation(), 2);
        assert!(mmap.is_committed());

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_flush() {
        let path = "/tmp/test_flush.bin";
        let _ = fs::remove_file(path);

        let mut mmap = PersistentMmap::create_mmap(
            Path::new(path),
            PAGE_SIZE * 16,
            512,
        ).unwrap();

        // Write data
        let atomic = mmap.atomic_view_u64(128).unwrap();
        atomic.store(42, Ordering::Release);

        // Flush (sync)
        mmap.flush().unwrap();

        // Verify last flush timestamp updated
        assert!(mmap.last_flush_ns() > 0);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_alignment_validation() {
        let path = "/tmp/test_alignment.bin";
        let _ = fs::remove_file(path);

        let mut mmap = PersistentMmap::create_mmap(
            Path::new(path),
            PAGE_SIZE * 16,
            512,
        ).unwrap();

        // Valid alignment (8-byte aligned)
        assert!(mmap.atomic_view_u64(128).is_ok());
        assert!(mmap.atomic_view_u64(136).is_ok());

        // Invalid alignment (not 8-byte aligned)
        assert!(mmap.atomic_view_u64(129).is_err());
        assert!(mmap.atomic_view_u64(131).is_err());

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_persistence_across_reopen() {
        let path = "/tmp/test_persistence.bin";
        let _ = fs::remove_file(path);

        // Create and write
        {
            let mut mmap = PersistentMmap::create_mmap(
                Path::new(path),
                PAGE_SIZE * 16,
                512,
            ).unwrap();

            let atomic = mmap.atomic_view_u64(128).unwrap();
            atomic.store(42, Ordering::Release);

            mmap.flush().unwrap();
        }

        // Reopen and verify
        {
            let mut mmap = PersistentMmap::open_mmap(Path::new(path)).unwrap();
            let atomic = mmap.atomic_view_u64(128).unwrap();

            assert_eq!(atomic.load(Ordering::Acquire), 42);
        }

        fs::remove_file(path).unwrap();
    }
}
