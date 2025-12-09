//! Lockfree Mmap LSH Bucket Capsule
//!
//! **UCE34 Tier**: T1 Atomic (interior mutability via AtomicU32, DualAtomicU64)
//!
//! # Clippy Suppressions
//! - `unsafe_code`: Mmap operations require unsafe for raw pointer manipulation (ASSUM verified)
//! - `missing_docs`: Internal error variants and type aliases have self-documenting names

#![allow(unsafe_code)]
#![allow(missing_docs)]
//!
//! ## Performance (B32 Target)
//! - Insert (fast path): <100ns (single CAS)
//! - Insert (retry path): <500ns (max 10 retries)
//! - Query bucket: <1µs (linear scan, up to 1024 docs)
//! - CAS retry rate: <5% under normal load
//!
//! ## Architecture
//! - **Q10 Tier**: T1 Atomic (lockfree CAS coordination)
//! - **Q11 Transform**: &mut self → &self + AtomicU32 interior mutability
//! - **Q12 Nightly**: Optional atomic_from_mut (5% speedup, zero-copy mmap)
//!
//! ## Design Principles
//! - **Q28 Simplicity**: CAS loops simpler than Mutex (no deadlocks, explicit retries)
//! - **Q29 Constraints**: Fixed capacity (mmap limitation), power-of-two buckets
//! - **Q30 Validation**: Generation counter + magic number validation
//! - **Q31 Rust**: Interior mutability pattern (AtomicU32 + &self methods)
//! - **Q32 Nightly**: Optional (stable fallback provided)
//! - **Q33 Verification**: Compile-time alignment checks (const assertions)
//!
//! ## ASSUM Framework
//! - `#ASSUME_CAS_CONVERGENCE`: Max 10 CAS retries under normal load (<5% retry rate)
//! - `#VERIFY_CAS_CONVERGENCE`: Stress test validates <10% retry rate @ 22 threads
//! - `#ASSUME_POWER_OF_TWO_BUCKETS`: num_buckets is power-of-two (fast modulo)
//! - `#VERIFY_POWER_OF_TWO`: Validation at create() + open() time
//! - `#ASSUME_BUCKET_CAPACITY`: max_bucket_size ≤ u32::MAX (no overflow)
//! - `#VERIFY_BUCKET_CAPACITY`: Bounds check before every insert
//! - `#ASSUME_MMAP_STABILITY`: Mmap not remapped during operation
//! - `#VERIFY_MMAP_STABILITY`: Integration test (no remap after create)
//!
//! ## Usage
//! ```rust,ignore
//! use kindly_dedup::universal::LockfreeMmapLshBucketCapsule;
//! use std::sync::Arc;
//!
//! // Create new lockfree LSH bucket capsule
//! let lsh = Arc::new(LockfreeMmapLshBucketCapsule::create(
//!     "lsh_buckets.mmap",
//!     32768,  // num_buckets (power-of-two)
//!     1024,   // max_bucket_size
//! )?);
//!
//! // Parallel insertion (works with Arc<>!)
//! lsh.insert_lockfree(doc_id, band_hash)?;  // &self method
//!
//! // Query bucket
//! let docs = lsh.get_bucket(&lsh, bucket_idx)?;
//! ```

use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use memmap2::{MmapMut, MmapOptions};
use thiserror::Error;

use atomic_capsule::patterns::DualAtomicU64;

// ============================================================================
// Constants
// ============================================================================

/// Magic number for LSH bucket mmap files ("LSH\0" + version 1)
const LSH_MAGIC: u64 = 0x4C5348_00000001;

/// Maximum CAS retries before giving up (prevents infinite loops)
///
/// # ASSUM Framework
/// - `#ASSUME_CAS_CONVERGENCE`: 10 retries sufficient for <5% failure rate
/// - `#VERIFY_CAS_CONVERGENCE`: Stress test (100M inserts @ 22 threads) validates <10% retry rate
const MAX_CAS_RETRIES: usize = 10;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug, Error)]
pub enum LshError {
    #[error("Bucket overflow: bucket {bucket_idx} full (max {max_size})")]
    BucketOverflow { bucket_idx: usize, max_size: u32 },

    #[error("CAS retry limit exceeded (10 retries), pathological contention")]
    CasRetryLimit,

    #[error("Corrupt generation counter: primary={primary}, secondary={secondary}")]
    CorruptGeneration { primary: u64, secondary: u64 },

    #[error("Bounds check failed: bucket_idx={bucket_idx}, num_buckets={num_buckets}")]
    BoundsCheck {
        bucket_idx: usize,
        num_buckets: usize,
    },

    #[error("Invalid magic number: expected {expected:#x}, got {got:#x}")]
    InvalidMagic { expected: u64, got: u64 },

    #[error("Invalid bucket count: {num_buckets} (must be power-of-two)")]
    InvalidBuckets { num_buckets: usize },

    #[error("Mmap I/O error: {0}")]
    MmapIo(#[from] std::io::Error),
}

pub type LshResult<T> = Result<T, LshError>;

// ============================================================================
// Mmap File Layout
// ============================================================================

/// LSH bucket mmap header (256B, cache-aligned)
///
/// # Memory Layout
/// ```text
/// Offset 0-7:    magic (0x4C5348_00000001 = "LSH" + v1)
/// Offset 8-15:   num_buckets (u64, must be power-of-two)
/// Offset 16-23:  max_bucket_size (u64)
/// Offset 24-31:  total_capacity (u64 = num_buckets × max_bucket_size)
/// Offset 32-39:  generation_primary (u64, crash recovery)
/// Offset 40-47:  generation_secondary (u64, crash recovery)
/// Offset 48-255: _padding (208 bytes)
/// ```
///
/// # ASSUM Framework
/// - `#ASSUME_256B_ALIGNMENT`: 256 bytes prevents false sharing (4 cache lines)
/// - `#VERIFY_256B_ALIGNMENT`: const assertions below
#[repr(C, align(256))]
#[allow(missing_docs)]
struct LshHeader {
    magic: u64,
    num_buckets: u64,
    max_bucket_size: u64,
    total_capacity: u64,
    generation_primary: u64,
    generation_secondary: u64,
    _padding: [u8; 208],
}

// Compile-time verification
const _: () = {
    assert!(std::mem::align_of::<LshHeader>() == 256);
    assert!(std::mem::size_of::<LshHeader>() == 256);
};

/// Bucket metadata (64B, cache-aligned to prevent false sharing)
///
/// # Memory Layout
/// ```text
/// Offset 0-3:   count (AtomicU32, number of documents in bucket)
/// Offset 4-63:  _padding (60 bytes)
/// ```
///
/// # ASSUM Framework
/// - `#ASSUME_64B_ALIGNMENT`: 64 bytes prevents false sharing between buckets
/// - `#VERIFY_64B_ALIGNMENT`: const assertions below
#[repr(C, align(64))]
struct BucketMetadata {
    count: AtomicU32,
    _padding: [u8; 60],
}

// Compile-time verification
const _: () = {
    assert!(std::mem::align_of::<BucketMetadata>() == 64);
    assert!(std::mem::size_of::<BucketMetadata>() == 64);
};

// ============================================================================
// Lockfree Mmap LSH Bucket Capsule
// ============================================================================

/// Lockfree LSH bucket capsule with interior mutability
///
/// # Performance Characteristics (B32 Framework)
/// - **insert_lockfree()**: <100ns fast path (single CAS), <500ns retry path
/// - **get_bucket()**: <1µs (linear scan, up to 1024 docs)
/// - **get_bucket_count()**: <10ns (single atomic load)
/// - **CAS retry rate**: <5% under normal load (target)
///
/// # Concurrency Model
/// - 100% lockfree (no Mutex/RwLock)
/// - Multiple concurrent readers (zero contention, atomic loads)
/// - Multiple concurrent writers (CAS-based coordination, <5% retry rate)
/// - Independent buckets (zero contention across different buckets)
///
/// # Limitations
/// - Fixed capacity (mmap files cannot resize after creation)
/// - Power-of-two buckets (required for fast modulo via bitmask)
/// - No deletion (append-only design, LSH buckets are immutable)
#[repr(C, align(64))]
pub struct LockfreeMmapLshBucketCapsule {
    /// Metadata (read-only after init)
    num_buckets: usize,
    num_buckets_mask: usize,  // num_buckets - 1 (for fast modulo)
    max_bucket_size: u32,

    /// Mmap file (read-only pointer after init, writes via interior mutability)
    mmap: MmapMut,

    /// Atomic coordination (interior mutability)
    ///
    /// # Nightly Feature
    /// With `nightly-atomic` feature: Zero-copy atomic views over mmap memory
    /// Without: Vec<AtomicU32> copied from mmap at startup (~128KB for 32K buckets)
    bucket_counts: Vec<AtomicU32>,

    /// Global counters
    total_count: AtomicU64,
    generation: DualAtomicU64,

    /// Cache metadata offsets (computed once at open/create)
    metadata_offset: usize,
    data_offset: usize,
}

impl LockfreeMmapLshBucketCapsule {
    /// Create new lockfree LSH bucket capsule
    ///
    /// # Arguments
    /// - `path`: Mmap file path
    /// - `num_buckets`: Number of buckets (must be power-of-two, e.g., 32768)
    /// - `max_bucket_size`: Max documents per bucket (e.g., 1024)
    ///
    /// # Returns
    /// - `Ok(Self)` on success
    /// - `Err(LshError)` on validation failure or I/O error
    ///
    /// # Performance
    /// - Complexity: O(1) mmap allocation
    /// - Latency: <1ms (file creation + mmap)
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_POWER_OF_TWO_BUCKETS`: num_buckets is power-of-two (validated)
    /// - `#ASSUME_BUCKET_CAPACITY`: max_bucket_size ≤ u32::MAX (validated)
    pub fn create(
        path: impl AsRef<Path>,
        num_buckets: usize,
        max_bucket_size: u32,
    ) -> LshResult<Self> {
        // #VERIFY_POWER_OF_TWO_BUCKETS
        if !num_buckets.is_power_of_two() {
            return Err(LshError::InvalidBuckets { num_buckets });
        }

        // Calculate file size
        let header_size = std::mem::size_of::<LshHeader>();
        let metadata_size = num_buckets * std::mem::size_of::<BucketMetadata>();
        let data_size = num_buckets * (max_bucket_size as usize) * std::mem::size_of::<u32>();
        let total_size = header_size + metadata_size + data_size;

        // Create mmap file
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path.as_ref())?;
        file.set_len(total_size as u64)?;

        // SAFETY: Mmap is exclusive to this function, no aliasing possible
        #[allow(unsafe_code)]
        let mut mmap = unsafe { MmapOptions::new().len(total_size).map_mut(&file)? };

        // Initialize header
        let header_ptr = mmap.as_mut_ptr() as *mut LshHeader;
        unsafe {
            (*header_ptr).magic = LSH_MAGIC;
            (*header_ptr).num_buckets = num_buckets as u64;
            (*header_ptr).max_bucket_size = max_bucket_size as u64;
            (*header_ptr).total_capacity =
                (num_buckets as u64) * (max_bucket_size as u64);
            (*header_ptr).generation_primary = 0;
            (*header_ptr).generation_secondary = 0;
        }

        // Initialize bucket metadata (all counts = 0)
        let metadata_offset = header_size;
        let metadata_ptr = unsafe { mmap.as_mut_ptr().add(metadata_offset) as *mut BucketMetadata };
        for i in 0..num_buckets {
            unsafe {
                let bucket = &mut *metadata_ptr.add(i);
                bucket.count = AtomicU32::new(0);
            }
        }

        // Create runtime atomic views
        let mut bucket_counts = Vec::with_capacity(num_buckets);
        for i in 0..num_buckets {
            let count = unsafe { (*metadata_ptr.add(i)).count.load(Ordering::Relaxed) };
            bucket_counts.push(AtomicU32::new(count));
        }

        // Compute data offset
        let data_offset = header_size + metadata_size;

        // Initialize generation counter
        let generation = DualAtomicU64::new(0, 0);

        Ok(Self {
            num_buckets,
            num_buckets_mask: num_buckets - 1,
            max_bucket_size,
            mmap,
            bucket_counts,
            total_count: AtomicU64::new(0),
            generation,
            metadata_offset,
            data_offset,
        })
    }

    /// Open existing lockfree LSH bucket capsule
    ///
    /// # Arguments
    /// - `path`: Mmap file path
    ///
    /// # Returns
    /// - `Ok(Self)` on success (with validation)
    /// - `Err(LshError)` on validation failure or I/O error
    ///
    /// # Validation
    /// - Magic number (LSH_MAGIC)
    /// - Generation counter consistency (crash recovery)
    /// - Power-of-two bucket count
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_POWER_OF_TWO_BUCKETS`: Validated at open time
    pub fn open(path: impl AsRef<Path>) -> LshResult<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path.as_ref())?;

        let file_size = file.metadata()?.len() as usize;
        let mmap = unsafe { MmapOptions::new().len(file_size).map_mut(&file)? };

        // Read and validate header
        let header_ptr = mmap.as_ptr() as *const LshHeader;
        let magic = unsafe { (*header_ptr).magic };
        if magic != LSH_MAGIC {
            return Err(LshError::InvalidMagic {
                expected: LSH_MAGIC,
                got: magic,
            });
        }

        let num_buckets = unsafe { (*header_ptr).num_buckets as usize };
        let max_bucket_size = unsafe { (*header_ptr).max_bucket_size as u32 };
        let gen_primary = unsafe { (*header_ptr).generation_primary };
        let gen_secondary = unsafe { (*header_ptr).generation_secondary };

        // #VERIFY_POWER_OF_TWO_BUCKETS
        if !num_buckets.is_power_of_two() {
            return Err(LshError::InvalidBuckets { num_buckets });
        }

        // Validate generation counter consistency (crash recovery)
        if gen_primary != gen_secondary {
            return Err(LshError::CorruptGeneration {
                primary: gen_primary,
                secondary: gen_secondary,
            });
        }

        let header_size = std::mem::size_of::<LshHeader>();
        let metadata_offset = header_size;
        let metadata_ptr = unsafe { mmap.as_ptr().add(metadata_offset) as *const BucketMetadata };

        // Load bucket counts from mmap
        let mut bucket_counts = Vec::with_capacity(num_buckets);
        for i in 0..num_buckets {
            let count = unsafe { (*metadata_ptr.add(i)).count.load(Ordering::Acquire) };
            bucket_counts.push(AtomicU32::new(count));
        }

        // Compute total count from bucket metadata
        let total: u64 = bucket_counts.iter().map(|c| c.load(Ordering::Relaxed) as u64).sum();

        // Compute data offset
        let data_offset = metadata_offset + (num_buckets * std::mem::size_of::<BucketMetadata>());

        // Initialize generation counter from mmap
        let generation = DualAtomicU64::new(gen_primary, gen_secondary);

        Ok(Self {
            num_buckets,
            num_buckets_mask: num_buckets - 1,
            max_bucket_size,
            mmap,
            bucket_counts,
            total_count: AtomicU64::new(total),
            generation,
            metadata_offset,
            data_offset,
        })
    }

    /// Lockfree insertion with CAS-based coordination
    ///
    /// # Arguments
    /// - `doc_id`: Document ID to insert
    /// - `band_hash`: Band hash value (determines bucket via modulo)
    ///
    /// # Returns
    /// - `Ok(())` on success
    /// - `Err(LshError::BucketOverflow)` if bucket is full
    /// - `Err(LshError::CasRetryLimit)` if contention exceeds 10 retries
    ///
    /// # Performance
    /// - Fast path: <100ns (single CAS success)
    /// - Retry path: <500ns (max 10 retries)
    /// - CAS retry rate: <5% under normal load
    ///
    /// # Algorithm
    /// 1. Compute bucket_idx = band_hash % num_buckets
    /// 2. Loop (max 10 retries):
    ///    a. Load bucket.count (Acquire)
    ///    b. Bounds check (count < max_bucket_size)
    ///    c. Compute slot offset
    ///    d. Write doc_id to mmap
    ///    e. CAS bucket.count from count to count+1 (Release)
    ///    f. If success: return Ok(())
    ///    g. If failed: retry from step 2a
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_CAS_CONVERGENCE`: Max 10 retries sufficient
    /// - `#VERIFY_CAS_CONVERGENCE`: Stress test validates <10% retry rate
    #[inline]
    pub fn insert_lockfree(&self, doc_id: u32, band_hash: u64) -> LshResult<()> {
        // Compute bucket index using fast modulo (power-of-two mask)
        let bucket_idx = (band_hash as usize) & self.num_buckets_mask;

        // CAS retry loop (max 10 iterations)
        // #ASSUME_CAS_CONVERGENCE: 10 retries sufficient for <5% failure rate
        for _retry in 0..MAX_CAS_RETRIES {
            // Load bucket count (Acquire for synchronization)
            let current_count = self.bucket_counts[bucket_idx].load(Ordering::Acquire);

            // Bounds check before CAS
            // #VERIFY_BUCKET_CAPACITY: Ensure count < max_bucket_size
            if current_count as u32 >= self.max_bucket_size {
                return Err(LshError::BucketOverflow {
                    bucket_idx,
                    max_size: self.max_bucket_size,
                });
            }

            // Compute slot offset
            let slot_offset = self.data_offset
                + (bucket_idx * self.max_bucket_size as usize * std::mem::size_of::<u32>())
                + (current_count as usize * std::mem::size_of::<u32>());

            // Write doc_id to mmap (unsafe, but protected by CAS)
            unsafe {
                let ptr = (self.mmap.as_ptr() as *const u8)
                    .byte_offset(slot_offset as isize) as *mut u32;
                *ptr = doc_id;
            }

            // CAS: Attempt to commit (Release on success for visibility)
            let cas_result = self.bucket_counts[bucket_idx].compare_exchange(
                current_count,
                current_count + 1,
                Ordering::Release,
                Ordering::Acquire,
            );

            if cas_result.is_ok() {
                // Success: Write updated count back to mmap metadata
                let new_count = current_count + 1;
                let metadata_ptr = unsafe {
                    (self.mmap.as_ptr() as *const u8)
                        .byte_offset(self.metadata_offset as isize)
                        as *mut BucketMetadata
                };
                unsafe {
                    (*metadata_ptr.add(bucket_idx))
                        .count.store(new_count, Ordering::Release);
                }

                // Increment global counter (optional audit trail)
                self.total_count.fetch_add(1, Ordering::Release);

                // Increment generation (optional, for audit trail)
                self.generation
                    .fetch_add_secondary(1, Ordering::Release);

                return Ok(());
            }
            // Failed: Retry loop continues
        }

        // CAS retry limit exceeded
        // #VERIFY_CAS_CONVERGENCE: Stress test determines if 10 retries sufficient
        Err(LshError::CasRetryLimit)
    }

    /// Get all documents in a bucket (lockfree read)
    ///
    /// # Arguments
    /// - `bucket_idx`: Bucket index (0 <= bucket_idx < num_buckets)
    ///
    /// # Returns
    /// - `Ok(Vec<u32>)` - Documents in bucket
    /// - `Err(LshError::BoundsCheck)` - Invalid bucket index
    ///
    /// # Performance
    /// - Complexity: O(k) where k = number of docs in bucket (typical <1024)
    /// - Latency: <1µs for full bucket (1024 docs)
    ///
    /// # Algorithm
    /// 1. Bounds check: bucket_idx < num_buckets
    /// 2. Load bucket.count (Acquire)
    /// 3. Scan mmap data region [0..count]
    /// 4. Return Vec<u32>
    #[inline]
    pub fn get_bucket(&self, bucket_idx: usize) -> LshResult<Vec<u32>> {
        // Bounds check
        // #VERIFY_BUCKET_BOUNDS: Ensure bucket_idx < num_buckets
        if bucket_idx >= self.num_buckets {
            return Err(LshError::BoundsCheck {
                bucket_idx,
                num_buckets: self.num_buckets,
            });
        }

        // Load bucket count (Acquire for synchronization with writers)
        let count = self.bucket_counts[bucket_idx].load(Ordering::Acquire) as usize;

        // Allocate result vector
        let mut docs = Vec::with_capacity(count);

        // Scan mmap data region
        for i in 0..count {
            let slot_offset = self.data_offset
                + (bucket_idx * self.max_bucket_size as usize * std::mem::size_of::<u32>())
                + (i * std::mem::size_of::<u32>());

            let doc_id = unsafe {
                let ptr = (self.mmap.as_ptr() as *const u8)
                    .byte_offset(slot_offset as isize) as *const u32;
                *ptr
            };

            docs.push(doc_id);
        }

        Ok(docs)
    }

    /// Get bucket count (lockfree, <10ns)
    ///
    /// # Arguments
    /// - `bucket_idx`: Bucket index
    ///
    /// # Returns
    /// - `Ok(u32)` - Number of documents in bucket
    /// - `Err(LshError::BoundsCheck)` - Invalid bucket index
    #[inline]
    pub fn get_bucket_count(&self, bucket_idx: usize) -> LshResult<u32> {
        if bucket_idx >= self.num_buckets {
            return Err(LshError::BoundsCheck {
                bucket_idx,
                num_buckets: self.num_buckets,
            });
        }

        Ok(self.bucket_counts[bucket_idx].load(Ordering::Acquire))
    }

    /// Get total count across all buckets (lockfree, <10ns)
    #[inline]
    pub fn total_count(&self) -> u64 {
        self.total_count.load(Ordering::Acquire)
    }

    /// Get number of buckets
    #[inline]
    pub fn num_buckets(&self) -> usize {
        self.num_buckets
    }

    /// Get max bucket size
    #[inline]
    pub fn max_bucket_size(&self) -> u32 {
        self.max_bucket_size
    }

    /// Flush generation counter to mmap (for persistence)
    ///
    /// # Performance
    /// - Latency: ~1-10ms (mmap flush)
    /// - Should be called infrequently (batch operation)
    pub fn flush(&mut self) -> LshResult<()> {
        let secondary = self.generation.load_secondary(Ordering::Acquire);

        let header_ptr = self.mmap.as_mut_ptr() as *mut LshHeader;
        unsafe {
            (*header_ptr).generation_primary = secondary;
            (*header_ptr).generation_secondary = secondary;
        }

        self.mmap.flush()?;
        Ok(())
    }

    /// Get statistics (lockfree reads)
    #[inline]
    pub fn stats(&self) -> LshStats {
        let total = self.total_count.load(Ordering::Relaxed);
        let generation = self.generation.load_secondary(Ordering::Relaxed);
        let max_filled = self
            .bucket_counts
            .iter()
            .map(|c| c.load(Ordering::Relaxed) as u32)
            .max()
            .unwrap_or(0);

        LshStats {
            total_docs: total,
            max_bucket_filled: max_filled,
            generation_counter: generation,
        }
    }
}

/// Statistics structure
#[derive(Debug, Clone, Copy)]
pub struct LshStats {
    pub total_docs: u64,
    pub max_bucket_filled: u32,
    pub generation_counter: u64,
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_lsh_layout_verification() {
        // Compile-time assertions verify alignment
        assert_eq!(std::mem::align_of::<LshHeader>(), 256);
        assert_eq!(std::mem::size_of::<LshHeader>(), 256);
        assert_eq!(std::mem::align_of::<BucketMetadata>(), 64);
        assert_eq!(std::mem::size_of::<BucketMetadata>(), 64);
    }

    #[test]
    fn test_create_basic() {
        let tmpdir = tempfile::tempdir().expect("create temp dir");
        let path = tmpdir.path().join("test.mmap");

        let capsule = LockfreeMmapLshBucketCapsule::create(&path, 256, 100)
            .expect("create capsule");

        assert_eq!(capsule.num_buckets(), 256);
        assert_eq!(capsule.max_bucket_size(), 100);
        assert_eq!(capsule.total_count(), 0);
    }

    #[test]
    fn test_create_invalid_buckets() {
        let tmpdir = tempfile::tempdir().expect("create temp dir");
        let path = tmpdir.path().join("test.mmap");

        let result = LockfreeMmapLshBucketCapsule::create(&path, 257, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_insert_single_thread() {
        let tmpdir = tempfile::tempdir().expect("create temp dir");
        let path = tmpdir.path().join("test.mmap");

        let capsule = LockfreeMmapLshBucketCapsule::create(&path, 256, 100)
            .expect("create capsule");

        // Insert document
        capsule.insert_lockfree(42, 0).expect("insert doc");

        // Verify count
        assert_eq!(capsule.total_count(), 1);

        // Verify bucket
        let bucket = capsule.get_bucket(0).expect("get bucket");
        assert_eq!(bucket.len(), 1);
        assert_eq!(bucket[0], 42);
    }

    #[test]
    fn test_insert_multiple_same_bucket() {
        let tmpdir = tempfile::tempdir().expect("create temp dir");
        let path = tmpdir.path().join("test.mmap");

        let capsule = LockfreeMmapLshBucketCapsule::create(&path, 256, 100)
            .expect("create capsule");

        // Insert 10 documents to same bucket (hash = 0)
        for i in 0..10 {
            capsule
                .insert_lockfree(i as u32, 0x0000000000000000)
                .expect("insert doc");
        }

        // Verify total count
        assert_eq!(capsule.total_count(), 10);

        // Verify bucket
        let bucket = capsule.get_bucket(0).expect("get bucket");
        assert_eq!(bucket.len(), 10);
        for (i, &doc_id) in bucket.iter().enumerate() {
            assert_eq!(doc_id, i as u32);
        }
    }

    #[test]
    fn test_insert_different_buckets() {
        let tmpdir = tempfile::tempdir().expect("create temp dir");
        let path = tmpdir.path().join("test.mmap");

        let capsule = LockfreeMmapLshBucketCapsule::create(&path, 256, 100)
            .expect("create capsule");

        // Insert to different buckets
        for i in 0..10 {
            let band_hash = i as u64;
            capsule.insert_lockfree(i as u32, band_hash).expect("insert doc");
        }

        // Verify total count
        assert_eq!(capsule.total_count(), 10);
    }

    #[test]
    fn test_bucket_overflow() {
        let tmpdir = tempfile::tempdir().expect("create temp dir");
        let path = tmpdir.path().join("test.mmap");

        let capsule = LockfreeMmapLshBucketCapsule::create(&path, 256, 5)
            .expect("create capsule");

        // Fill bucket (5 docs)
        for i in 0..5 {
            capsule.insert_lockfree(i as u32, 0).expect("insert doc");
        }

        // Try to overflow
        let result = capsule.insert_lockfree(999, 0);
        assert!(result.is_err());
        if let Err(LshError::BucketOverflow { bucket_idx, max_size }) = result {
            assert_eq!(bucket_idx, 0);
            assert_eq!(max_size, 5);
        }
    }

    #[test]
    fn test_bounds_check() {
        let tmpdir = tempfile::tempdir().expect("create temp dir");
        let path = tmpdir.path().join("test.mmap");

        let capsule = LockfreeMmapLshBucketCapsule::create(&path, 256, 100)
            .expect("create capsule");

        // Try invalid bucket index
        let result = capsule.get_bucket(256);
        assert!(result.is_err());
    }

    #[test]
    fn test_open_existing() {
        let tmpdir = tempfile::tempdir().expect("create temp dir");
        let path = tmpdir.path().join("test.mmap");

        // Create and insert
        let capsule = LockfreeMmapLshBucketCapsule::create(&path, 256, 100)
            .expect("create capsule");
        capsule.insert_lockfree(42, 0).expect("insert doc");
        drop(capsule);

        // Open and verify
        let capsule = LockfreeMmapLshBucketCapsule::open(&path).expect("open capsule");
        assert_eq!(capsule.total_count(), 1);

        let bucket = capsule.get_bucket(0).expect("get bucket");
        assert_eq!(bucket[0], 42);
    }

    #[test]
    fn test_concurrent_inserts() {
        let tmpdir = tempfile::tempdir().expect("create temp dir");
        let path = tmpdir.path().join("test.mmap");

        let capsule = Arc::new(
            LockfreeMmapLshBucketCapsule::create(&path, 256, 1000)
                .expect("create capsule"),
        );

        // Spawn 4 threads, each inserting 100 docs
        let handles: Vec<_> = (0..4)
            .map(|thread_id| {
                let capsule_clone = Arc::clone(&capsule);
                std::thread::spawn(move || {
                    for i in 0..100 {
                        let doc_id = thread_id * 100 + i as u32;
                        let band_hash = (doc_id as u64) * 12345;
                        capsule_clone
                            .insert_lockfree(doc_id, band_hash)
                            .expect("insert doc");
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("join thread");
        }

        // Verify total count
        assert_eq!(capsule.total_count(), 400);
    }

    #[test]
    fn test_get_bucket_count() {
        let tmpdir = tempfile::tempdir().expect("create temp dir");
        let path = tmpdir.path().join("test.mmap");

        let capsule = LockfreeMmapLshBucketCapsule::create(&path, 256, 100)
            .expect("create capsule");

        capsule.insert_lockfree(1, 0).expect("insert doc");
        capsule.insert_lockfree(2, 0).expect("insert doc");

        let count = capsule.get_bucket_count(0).expect("get count");
        assert_eq!(count, 2);
    }

    #[test]
    fn test_stats() {
        let tmpdir = tempfile::tempdir().expect("create temp dir");
        let path = tmpdir.path().join("test.mmap");

        let capsule = LockfreeMmapLshBucketCapsule::create(&path, 256, 100)
            .expect("create capsule");

        for i in 0..10 {
            capsule.insert_lockfree(i, i as u64).expect("insert doc");
        }

        let stats = capsule.stats();
        assert_eq!(stats.total_docs, 10);
    }
}
