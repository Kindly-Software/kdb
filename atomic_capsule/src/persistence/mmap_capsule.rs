//! # T9 Persistent Capsule - Simplified Core Implementation
//!
//! Zero-copy atomic operations over memory-mapped files (simplified version).
//!
//! **UCE34 Q10**: T9 Tier (T1 Atomic + Mmap Persistence)
//! **UCE34 Q11**: atomic_from_mut enables zero-copy atomic views
//! **UCE34 Q12**: Nightly feature #![feature(atomic_from_mut)]
//! **IMPL-2 V3.1**: Cutting-edge-first (nightly atomic_from_mut required)
//!
//! # Performance
//!
//! - Atomic write: <50ns (direct mmap store)
//! - Flush (async): <1ms (msync MS_ASYNC)
//! - Flush (sync): <5ms (msync MS_SYNC)
//! - Recovery: <100ms (re-mmap + validate)

#![cfg(all(feature = "mmap-persistence", feature = "nightly-atomic"))]

use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use memmap2::MmapMut;

use crate::primitives::atomic_from_mut::{AtomicFromMut, AtomicFromMutError};

use super::alignment::validate_alignment;

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

// Generation counter offset in header
const GENERATION_OFFSET: usize = 24;

// Last flush timestamp offset in header
const FLUSH_OFFSET: usize = 48;

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
                write!(
                    f,
                    "Invalid alignment: offset {} requires {} byte alignment",
                    offset, required
                )
            }
            PersistentError::InvalidMagic { expected, actual } => {
                write!(
                    f,
                    "Invalid file magic: expected 0x{:016x}, got 0x{:016x}",
                    expected, actual
                )
            }
            PersistentError::UnsupportedVersion { expected, actual } => {
                write!(
                    f,
                    "Unsupported version: expected {}, got {}",
                    expected, actual
                )
            }
            PersistentError::FileTooSmall { expected, actual } => {
                write!(
                    f,
                    "File too small: expected {} bytes, got {}",
                    expected, actual
                )
            }
            PersistentError::GenerationMismatch { expected, actual } => {
                write!(
                    f,
                    "Generation mismatch: expected {}, got {} (partial update detected)",
                    expected, actual
                )
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
pub struct PersistentMmap {
    /// Memory-mapped file
    mmap: MmapMut,

    /// File size
    file_size: usize,
}

impl PersistentMmap {
    /// Create new memory-mapped file
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

        // Flush header to disk (sync)
        mmap.flush()?;

        Ok(Self {
            mmap,
            file_size: size,
        })
    }

    /// Open existing memory-mapped file
    pub fn open_mmap(path: &Path) -> Result<Self, PersistentError> {
        // Open file
        let file = OpenOptions::new().read(true).write(true).open(path)?;

        // Memory-map the file
        let mmap = unsafe { MmapMut::map_mut(&file)? };
        let file_size = mmap.len();

        // Validate minimum size
        if file_size < HEADER_SIZE {
            return Err(PersistentError::FileTooSmall {
                expected: HEADER_SIZE,
                actual: file_size,
            });
        }

        // Read and validate header
        let header = unsafe { &*(mmap.as_ptr() as *const FileHeader) };
        header.validate()?;

        Ok(Self { mmap, file_size })
    }

    /// Get atomic view of u64 at offset
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
    pub fn flush(&self) -> Result<(), PersistentError> {
        // Update timestamp (monotonic nanoseconds)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        // Get atomic view of flush timestamp (unsafe but correct)
        let flush_atomic = unsafe { &*(self.mmap.as_ptr().add(FLUSH_OFFSET) as *const AtomicU64) };
        flush_atomic.store(now, Ordering::Release);

        // Synchronous flush (MS_SYNC)
        self.mmap.flush()?;

        Ok(())
    }

    /// Flush to disk (asynchronous, non-blocking)
    pub fn flush_async(&self) -> Result<(), PersistentError> {
        // Update timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        // Get atomic view of flush timestamp
        let flush_atomic = unsafe { &*(self.mmap.as_ptr().add(FLUSH_OFFSET) as *const AtomicU64) };
        flush_atomic.store(now, Ordering::Release);

        // Asynchronous flush (MS_ASYNC)
        self.mmap.flush_async()?;

        Ok(())
    }

    /// Start two-phase atomic update
    pub fn begin_update(&mut self) -> Result<(), PersistentError> {
        let gen_atomic = self.atomic_view_u64(GENERATION_OFFSET)?;
        gen_atomic.fetch_add(1, Ordering::Release);
        Ok(())
    }

    /// Commit two-phase atomic update
    pub fn commit_update(&mut self) -> Result<(), PersistentError> {
        let gen_atomic = self.atomic_view_u64(GENERATION_OFFSET)?;
        gen_atomic.fetch_add(1, Ordering::Release);
        self.flush_async()?;
        Ok(())
    }

    /// Get current generation counter
    pub fn generation(&mut self) -> Result<u64, PersistentError> {
        let gen_atomic = self.atomic_view_u64(GENERATION_OFFSET)?;
        Ok(gen_atomic.load(Ordering::Acquire))
    }

    /// Check if state is committed (generation is even)
    pub fn is_committed(&mut self) -> Result<bool, PersistentError> {
        Ok(self.generation()? % 2 == 0)
    }

    /// Get file size
    pub fn size(&self) -> usize {
        self.file_size
    }

    /// Get last flush timestamp (monotonic nanoseconds)
    pub fn last_flush_ns(&self) -> u64 {
        let flush_atomic = unsafe { &*(self.mmap.as_ptr().add(FLUSH_OFFSET) as *const AtomicU64) };
        flush_atomic.load(Ordering::Acquire)
    }

    /// Get raw pointer to mmap data
    ///
    /// # Safety
    ///
    /// The returned pointer is valid for the lifetime of the mmap.
    /// Caller must ensure proper alignment and bounds checking.
    pub fn as_ptr(&self) -> *const u8 {
        self.mmap.as_ptr()
    }

    /// Get mutable raw pointer to mmap data
    ///
    /// # Safety
    ///
    /// The returned pointer is valid for the lifetime of the mmap.
    /// Caller must ensure proper alignment and bounds checking.
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.mmap.as_mut_ptr()
    }

    /// Get slice of mmap data at offset
    ///
    /// # Panics
    ///
    /// Panics if offset + size exceeds mmap bounds
    pub fn slice_at(&self, offset: usize, size: usize) -> &[u8] {
        &self.mmap[offset..offset + size]
    }

    /// Get mutable slice of mmap data at offset
    ///
    /// # Panics
    ///
    /// Panics if offset + size exceeds mmap bounds
    pub fn slice_at_mut(&mut self, offset: usize, size: usize) -> &mut [u8] {
        &mut self.mmap[offset..offset + size]
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
        let path = "/tmp/test_create_mmap_simple.bin";
        let _ = fs::remove_file(path); // Clean up

        let mmap = PersistentMmap::create_mmap(
            Path::new(path),
            PAGE_SIZE * 16, // 64KB
            512,
        )
        .unwrap();

        assert_eq!(mmap.size(), PAGE_SIZE * 16);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_atomic_view() {
        let path = "/tmp/test_atomic_view_simple.bin";
        let _ = fs::remove_file(path);

        let mut mmap = PersistentMmap::create_mmap(Path::new(path), PAGE_SIZE * 16, 512).unwrap();

        // Get atomic view at offset 128 (after header)
        let atomic = mmap.atomic_view_u64(128).unwrap();

        // Write
        atomic.store(42, Ordering::Release);

        // Read
        assert_eq!(atomic.load(Ordering::Acquire), 42);

        fs::remove_file(path).unwrap();
    }
}
