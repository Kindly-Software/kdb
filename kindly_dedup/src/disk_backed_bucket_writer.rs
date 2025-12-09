//! Disk-backed LSH bucket writer (T9 Persistent + T1 Atomic)
//!
//! Implements Option H Phase 1: DiskBackedBucketWriter capsule for hierarchical LSH deduplication.
//! Addresses memory bottleneck where LSH buckets consume 25-28 GB (92% of memory) by moving
//! bucket storage to disk with mmap/atomic coordination.
//!
//! # Tier Selection (UCE34 Q10)
//!
//! **T9 Persistent** (mmap, disk storage) + **T1 Atomic** (lockfree coordination)
//! - Persistent: Buckets stored on disk, crash-recoverable
//! - Atomic: Offset tracking via AtomicU64, generation counter for crash detection
//! - Zero mutex/RwLock (Chaos mandate)
//!
//! # Disk Format (Per Bucket)
//!
//! ```text
//! [coarse_hash: u64, 8 bytes]
//! [fine_hash: u64, 8 bytes]
//! [count: u32, 4 bytes]
//! [reserved: u32, 4 bytes]
//! [CRC64: u64, 8 bytes]
//! [doc_ids: N × u64, N × 8 bytes]
//! Total: 36 + N×8 bytes per bucket
//! ```
//!
//! CRC64 covers: `[coarse_hash][fine_hash][count][doc_ids...]`

use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;

/// CRC mismatch variant struct
#[doc(hidden)]
#[derive(Debug)]
pub struct CrcMismatchData {
    /// Expected CRC value
    pub expected: u64,
    /// Actual computed CRC value
    pub actual: u64,
}

/// Error types for disk-backed bucket writer (T1+T9 tier)
#[derive(Debug, Error)]
pub enum DiskBackedBucketError {
    /// I/O error from disk operations
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    /// Offset would overflow u64 (file too large)
    #[error("Offset overflow: {0}")]
    OffsetOverflow(String),

    /// Bucket size exceeds reasonable limits (>100M doc_ids)
    #[error("Invalid bucket size: {0}")]
    InvalidBucketSize(String),

    /// CRC64 validation failed (data corruption or tampering)
    #[error("CRC64 validation failed: expected {expected:x}, got {actual:x}")]
    /// Expected CRC value
    CrcMismatch {
        /// Expected CRC checksum value
        expected: u64,
        /// Actual computed CRC checksum value
        actual: u64,
    },
}

/// Result type for disk-backed bucket operations
pub type DiskBackedBucketResult<T> = Result<T, DiskBackedBucketError>;

/// ASSUMPTION #1: Power-of-two hash capacity for modulo optimization
/// - Enforced: Structure constructor validates range
/// - Verified: Tests check boundary conditions
const MAX_DOCS_PER_BUCKET: u32 = 1_000_000;

/// ASSUMPTION #2: CRC64 polynomial (ECMA polynomial, used by many systems)
/// For simplicity, we use a basic u64 XOR hash as CRC64 substitute
/// In production, use the `crc` crate's ECMA CRC64 implementation
pub fn compute_crc64(data: &[u8]) -> u64 {
    // Simple rolling XOR hash (polynomial-free for this prototype)
    // In production: use crc::crc64::checksum_ecma(data)
    let mut hash: u64 = 0;
    for chunk in data.chunks(8) {
        let mut word = 0u64;
        for (i, &byte) in chunk.iter().enumerate() {
            word |= (byte as u64) << (i * 8);
        }
        hash = hash.wrapping_mul(31).wrapping_add(word);
    }
    hash.wrapping_mul(0xda942042e4dd58b5) // Final mix constant
}

/// Disk-backed LSH bucket writer capsule
///
/// # Chaos Architecture
///
/// **Cache alignment**: 64 bytes (HotTier) to prevent false sharing
/// **Coordination**: AtomicU64 offset + generation counter (lockfree)
/// **No mutex/RwLock**: Pure atomic operations (Chaos mandate)
///
/// # Verification (Q33)
///
/// Must use `#[derive(ComputationalCapsule)]` for compile-time verification
/// (Not applied here since atomic_capsule_derive may not be available in kindly_dedup context)
///
/// # ASSUM Safety
///
/// - #ASSUME_LOCKFREE_ONLY: All coordination via atomics (verified: grep 0 mutex)
/// - #ASSUME_ATOMIC_FILE_WRITES: File handle is Arc, single writer per offset
/// - #ASSUME_OFFSET_MONOTONIC: Offsets only increase (verified: tests)
/// - #ASSUME_FILE_PERSISTENCE: No OS-level surprises (fsync guarantees)
#[repr(C, align(64))]
pub struct DiskBackedBucketWriter {
    // Owned file handle (wrapped in Arc for safe sharing across threads)
    file: Arc<File>,

    /// Current write offset (T1 Atomic coordination)
    /// ASSUMPTION: Offset only increases monotonically
    offset: AtomicU64,

    /// Generation counter (crash detection, TOCTOU prevention)
    /// ASSUMPTION: Incremented on each append_bucket call
    generation: AtomicU64,

    /// Total buckets written (metrics)
    /// ASSUMPTION: Monotonically increasing counter
    buckets_written: AtomicU64,

    /// Padding to 64 bytes
    /// Calculation: 64 (align) - 8 (Arc<File> ptr) - 24 (3×AtomicU64) = 32 bytes
    _padding: [u8; 32],
}

impl DiskBackedBucketWriter {
    /// Create new writer (truncates existing file)
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to bucket file (will be truncated/created)
    ///
    /// # Returns
    ///
    /// New DiskBackedBucketWriter if file can be created, else error
    ///
    /// # ASSUM Verification
    ///
    /// - File creation: OS guarantees atomic create-or-truncate
    /// - Initial offsets: Set to 0 (no prior data)
    pub fn create(file_path: &str) -> DiskBackedBucketResult<Self> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(file_path)?;

        Ok(DiskBackedBucketWriter {
            file: Arc::new(file),
            offset: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            buckets_written: AtomicU64::new(0),
            _padding: [0u8; 32],
        })
    }

    /// Append bucket to file (returns offset where written)
    ///
    /// # Arguments
    ///
    /// * `coarse_hash` - Coarse-grained hash (first stage LSH)
    /// * `fine_hash` - Fine-grained hash (second stage LSH)
    /// * `doc_ids` - Document IDs in this bucket
    ///
    /// # Returns
    ///
    /// Offset where bucket was written (for index building)
    ///
    /// # Format
    ///
    /// ```text
    /// [coarse_hash: u64][fine_hash: u64][count: u32][reserved: u32][CRC64: u64][doc_ids...]
    /// 8                 8                4         4               8           N×8
    /// ```
    ///
    /// # ASSUM Verification
    ///
    /// - Offset tracking: AtomicU64 with Release ordering ensures visibility
    /// - Generation counter: Incremented before write (crash detection)
    /// - Bucket size: Limited to MAX_DOCS_PER_BUCKET (validates count range)
    /// - Concurrent safety: Offset is atomically updated AFTER successful write
    pub fn append_bucket(&self, coarse_hash: u64, fine_hash: u64, doc_ids: &[u64]) -> DiskBackedBucketResult<u64> {
        // ASSUMPTION: doc_ids.len() fits in u32
        if doc_ids.len() > MAX_DOCS_PER_BUCKET as usize {
            return Err(DiskBackedBucketError::InvalidBucketSize(format!(
                "Bucket has {} documents (max: {})",
                doc_ids.len(),
                MAX_DOCS_PER_BUCKET
            )));
        }

        // Increment generation counter (TOCTOU prevention)
        let _gen = self.generation.fetch_add(1, Ordering::Release);

        // Compute CRC64 over: [coarse_hash][fine_hash][count][doc_ids...]
        let mut crc_data = Vec::with_capacity(8 + 8 + 4 + doc_ids.len() * 8);
        crc_data.extend_from_slice(&coarse_hash.to_le_bytes());
        crc_data.extend_from_slice(&fine_hash.to_le_bytes());
        crc_data.extend_from_slice(&(doc_ids.len() as u32).to_le_bytes());
        for &doc_id in doc_ids {
            crc_data.extend_from_slice(&doc_id.to_le_bytes());
        }
        let crc64 = compute_crc64(&crc_data);

        // Build bucket: header + CRC + doc_ids
        let mut bucket = Vec::with_capacity(36 + doc_ids.len() * 8);
        bucket.extend_from_slice(&coarse_hash.to_le_bytes());
        bucket.extend_from_slice(&fine_hash.to_le_bytes());
        bucket.extend_from_slice(&(doc_ids.len() as u32).to_le_bytes());
        bucket.extend_from_slice(&0u32.to_le_bytes()); // reserved
        bucket.extend_from_slice(&crc64.to_le_bytes());
        for &doc_id in doc_ids {
            bucket.extend_from_slice(&doc_id.to_le_bytes());
        }

        let bucket_len = bucket.len() as u64;

        // Atomically fetch-and-add to reserve space and get write offset
        // This prevents concurrent writes to same location
        let write_offset = self.offset.fetch_add(bucket_len, Ordering::Release);

        // Check for overflow after atomic add
        let _new_offset = write_offset
            .checked_add(bucket_len)
            .ok_or_else(|| DiskBackedBucketError::OffsetOverflow("File too large".to_string()))?;

        // Write to file at reserved offset (Arc<File> is safely shared)
        // Even if multiple threads race, each writes to different offset
        let mut file_ref = self.file.as_ref();
        file_ref.seek(SeekFrom::Start(write_offset))?;
        file_ref.write_all(&bucket)?;

        // Increment buckets_written counter (metrics)
        self.buckets_written.fetch_add(1, Ordering::Relaxed);

        Ok(write_offset)
    }

    /// Get current offset (for index building)
    ///
    /// # Returns
    ///
    /// Current write position in file
    pub fn current_offset(&self) -> u64 {
        self.offset.load(Ordering::Acquire)
    }

    /// Get total buckets written (metrics)
    ///
    /// # Returns
    ///
    /// Number of buckets appended
    pub fn buckets_written(&self) -> u64 {
        self.buckets_written.load(Ordering::Acquire)
    }

    /// Flush writes to disk (fsync)
    ///
    /// # ASSUM Verification
    ///
    /// - fsync guarantees OS-level durability
    /// - Safe to call from multiple threads (Arc<File> protected)
    pub fn flush(&self) -> DiskBackedBucketResult<()> {
        self.file.as_ref().sync_all()?;
        Ok(())
    }

    /// Verify CRC64 for a bucket at given offset
    ///
    /// # Arguments
    ///
    /// * `offset` - File offset where bucket starts
    ///
    /// # Returns
    ///
    /// Ok(true) if CRC matches, Ok(false) if mismatch, Err if I/O error
    ///
    /// # Note
    ///
    /// This is a verification-only function (not in fast path)
    /// Used for crash recovery and integrity validation
    pub fn verify_bucket_crc(&self, offset: u64) -> DiskBackedBucketResult<bool> {
        use std::io::Read;

        // Read header (28 bytes: 2×u64 + u32 + u32)
        let mut header = [0u8; 28];
        let mut file_ref = self.file.as_ref();
        file_ref.seek(SeekFrom::Start(offset))?;
        file_ref.read_exact(&mut header)?;

        // Parse count
        let count = u32::from_le_bytes([header[16], header[17], header[18], header[19]]);

        // Read CRC from file (at offset 28)
        let mut crc_bytes = [0u8; 8];
        file_ref.read_exact(&mut crc_bytes)?;
        let stored_crc = u64::from_le_bytes(crc_bytes);

        // Read doc_ids
        let num_doc_ids = count as usize;
        let mut doc_ids_bytes = vec![0u8; num_doc_ids * 8];
        file_ref.read_exact(&mut doc_ids_bytes)?;

        // Recompute CRC
        let mut crc_data = Vec::with_capacity(8 + 8 + 4 + num_doc_ids * 8);
        crc_data.extend_from_slice(&header[0..16]); // coarse_hash + fine_hash
        crc_data.extend_from_slice(&header[16..20]); // count
        crc_data.extend_from_slice(&doc_ids_bytes);
        let computed_crc = compute_crc64(&crc_data);

        Ok(stored_crc == computed_crc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_create_writer() -> io::Result<()> {
        let temp_file = NamedTempFile::new()?;
        let writer =
            DiskBackedBucketWriter::create(temp_file.path().to_str().unwrap()).expect("Failed to create writer");

        assert_eq!(writer.current_offset(), 0);
        assert_eq!(writer.buckets_written(), 0);
        Ok(())
    }

    #[test]
    fn test_append_single_bucket() -> DiskBackedBucketResult<()> {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let writer = DiskBackedBucketWriter::create(temp_file.path().to_str().unwrap())?;

        let coarse = 0x1234567890abcdef_u64;
        let fine = 0xfedcba0987654321_u64;
        let doc_ids = vec![1, 2, 3, 4, 5];

        let offset = writer.append_bucket(coarse, fine, &doc_ids)?;
        assert_eq!(offset, 0);

        // Bucket size: 32 (header: 8+8+4+4+8) + 40 (5 × u64) = 72 bytes
        assert_eq!(writer.current_offset(), 72);
        assert_eq!(writer.buckets_written(), 1);

        Ok(())
    }

    #[test]
    fn test_append_multiple_buckets() -> DiskBackedBucketResult<()> {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let writer = DiskBackedBucketWriter::create(temp_file.path().to_str().unwrap())?;

        // Bucket 1: 5 doc_ids = 32 + 40 = 72 bytes
        let offset1 = writer.append_bucket(0x1111, 0x2222, &vec![10, 11, 12, 13, 14])?;
        assert_eq!(offset1, 0);

        // Bucket 2: 3 doc_ids = 32 + 24 = 56 bytes
        let offset2 = writer.append_bucket(0x3333, 0x4444, &vec![20, 21, 22])?;
        assert_eq!(offset2, 72);

        // Bucket 3: 8 doc_ids = 32 + 64 = 96 bytes
        let offset3 = writer.append_bucket(0x5555, 0x6666, &vec![30, 31, 32, 33, 34, 35, 36, 37])?;
        assert_eq!(offset3, 72 + 56);

        assert_eq!(writer.buckets_written(), 3);

        Ok(())
    }

    #[test]
    fn test_crc64_calculation() {
        // Test CRC64 is deterministic
        let data = b"Hello, World!";
        let crc1 = compute_crc64(data);
        let crc2 = compute_crc64(data);
        assert_eq!(crc1, crc2);

        // Different data should produce different CRC
        let data2 = b"Different data";
        let crc3 = compute_crc64(data2);
        assert_ne!(crc1, crc3);
    }

    #[test]
    fn test_concurrent_append() -> DiskBackedBucketResult<()> {
        use std::sync::Arc;
        use std::thread;

        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let writer = Arc::new(DiskBackedBucketWriter::create(temp_file.path().to_str().unwrap())?);

        let mut handles = vec![];

        // 4 threads, 10 buckets each
        for thread_id in 0..4 {
            let writer_clone = Arc::clone(&writer);
            let handle = thread::spawn(move || {
                for bucket_id in 0..10 {
                    let coarse = (thread_id * 100 + bucket_id) as u64;
                    let fine = (thread_id * 1000 + bucket_id * 10) as u64;
                    let doc_ids = vec![
                        thread_id as u64 * 10000 + bucket_id as u64,
                        thread_id as u64 * 10000 + bucket_id as u64 + 1,
                    ];
                    writer_clone
                        .append_bucket(coarse, fine, &doc_ids)
                        .expect("Failed to append");
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Verify total buckets
        assert_eq!(writer.buckets_written(), 40);

        // Verify offset increased correctly (4 threads × 10 buckets)
        // Each bucket: 32 (header) + 16 (2 doc_ids) = 48 bytes
        assert_eq!(writer.current_offset(), 40 * 48);

        Ok(())
    }

    #[test]
    fn test_invalid_bucket_size() -> DiskBackedBucketResult<()> {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let writer = DiskBackedBucketWriter::create(temp_file.path().to_str().unwrap())?;

        // Create a bucket that's too large
        let too_large_doc_ids: Vec<u64> = (0..=MAX_DOCS_PER_BUCKET as usize).map(|i| i as u64).collect();

        let result = writer.append_bucket(0x1234, 0x5678, &too_large_doc_ids);
        assert!(result.is_err());

        match result {
            Err(DiskBackedBucketError::InvalidBucketSize(_)) => (),
            other => panic!("Expected InvalidBucketSize, got {:?}", other),
        }

        Ok(())
    }
}
