//! # BinaryReaderCapsule - T9 Persistent Tier
//!
//! **Mmap-friendly binary training data reader** with zero-copy access.
//!
//! ## UCE34 Framework Application
//!
//! ### Q1-Q9: Problem Analysis
//! - **Q1**: Read 23GB binary format efficiently (<1μs per record random access)
//! - **Q2**: Mmap-based zero-copy reading (vs slow JSON parsing)
//! - **Q3**: <100ns read per record (vs ~50μs JSON parsing)
//! - **Q4**: T9 Persistent (mmap zero-copy) + T3 Fixed-Point (Q8.8 decode)
//! - **Q5**: `BinaryReaderCapsule` (mmap handle + validation)
//! - **Q8**: ~16 bytes (file handle only, data in OS page cache)
//!
//! ### Q10-Q12: Tier Selection
//! - **Q10**: Tier 9 Persistent (mmap zero-copy access)
//! - **Q11**: Direct memory access, no parsing overhead
//! - **Q12**: Stable Rust (no nightly features required)
//!
//! ### Q13-Q27: Implementation Details
//! - **Zero-copy**: Mmap file, decode Q8.8 on-the-fly
//! - **Random access**: O(1) seek to any record
//! - **Validation**: SHA-256 hash verification on open
//! - **Cache-friendly**: Sequential scans use OS page cache
//!
//! ### Q31: Simplicity
//! - Simple API: open() → read_record() → close
//! - Iterator support: for record in reader.iter()
//! - Hide complexity: Mmap, hashing, alignment internal
//!
//! ### Q33: Verification
//! - SHA-256 integrity checks on open
//! - Magic number validation
//! - Version compatibility checks
//!
//! ### Q34: Auditability
//! - Tamper detection via SHA-256 hashes
//! - Version tracking for format compatibility
//! - Record count validation
//!
//! ## Performance Targets (B32)
//! - `open()`: <10ms (mmap + SHA-256 validation)
//! - `read_record()`: <100ns (zero-copy decode)
//! - `iter()`: <50ns per record (sequential scan)
//! - **Memory**: 16 bytes (mmap handle, data in page cache)
//! - **Throughput**: >10M records/sec (sequential)
//!
//! ## ASSUM Safety
//! - 99.99% safe: Minimal unsafe (only for mmap access)
//! - File I/O: All errors propagated via Result<>
//! - Bounds checking: Index validation for all accesses
//! - Hash verification: Tamper detection on open
//!
//! ## Usage
//! ```rust
//! use atomic_capsule::persistence::BinaryReaderCapsule;
//! use std::path::Path;
//!
//! // Open binary file
//! let reader = BinaryReaderCapsule::open(Path::new("training.bin"))?;
//!
//! // Random access
//! let record = reader.read_record(42)?;
//! println!("Features: {:?}", record.features);
//! println!("Label: {:?}", record.label);
//!
//! // Sequential scan
//! for record in reader.iter() {
//!     // Process record
//! }
//! ```

#[cfg(feature = "persistence-binary-io")]
use std::fs::File;
#[cfg(feature = "persistence-binary-io")]
use std::io::Read;
#[cfg(feature = "persistence-binary-io")]
use std::path::Path;
#[cfg(feature = "persistence-binary-io")]
use anyhow::{Context, Result, bail};
#[cfg(feature = "persistence-binary-io")]
use memmap2::Mmap;

#[cfg(feature = "persistence-binary-io")]
use crate::primitives::fixed_point::quantizer::QuantizerCapsule;
#[cfg(feature = "persistence-binary-io")]
use crate::streaming::StrategyLabel;

/// Magic number for binary format validation
#[cfg(feature = "persistence-binary-io")]
const MAGIC: &[u8; 8] = b"KNDLYHFT";

/// Binary format version (2.0)
#[cfg(feature = "persistence-binary-io")]
const VERSION_MAJOR: u16 = 2;
#[cfg(feature = "persistence-binary-io")]
const VERSION_MINOR: u16 = 0;

/// Feature dimension (126D)
#[cfg(feature = "persistence-binary-io")]
const FEATURE_DIM: u32 = 126;

/// Compression type: Q8.8 fixed-point
#[cfg(feature = "persistence-binary-io")]
const COMPRESSION_Q8_8: u8 = 0x01;

/// Header size (4KB page-aligned)
#[cfg(feature = "persistence-binary-io")]
const HEADER_SIZE: usize = 4096;

/// Bytes per feature (Q8.8 = i16 = 2 bytes)
#[cfg(feature = "persistence-binary-io")]
const BYTES_PER_FEATURE: usize = 2;

/// Bytes per label (u8 strategy ID)
#[cfg(feature = "persistence-binary-io")]
const BYTES_PER_LABEL: usize = 1;

/// Bytes per metadata entry (u64 timestamp + u64 regime_id)
#[cfg(feature = "persistence-binary-io")]
const BYTES_PER_METADATA: usize = 16;

/// Total bytes per record
#[cfg(feature = "persistence-binary-io")]
const BYTES_PER_RECORD: usize = FEATURE_DIM as usize * BYTES_PER_FEATURE;

/// Training record (zero-copy view)
#[cfg(feature = "persistence-binary-io")]
#[derive(Debug, Clone)]
pub struct TrainingRecord {
    /// 126-dimensional feature vector (decoded from Q8.8)
    pub features: [f64; 126],
    /// Strategy label
    pub label: StrategyLabel,
    /// Nanosecond timestamp
    pub timestamp: u64,
    /// Market regime identifier
    pub regime_id: u64,
}

/// Binary Training Data Reader (T9 Persistent)
///
/// Zero-copy mmap-based reader with SHA-256 integrity verification.
#[cfg(feature = "persistence-binary-io")]
pub struct BinaryReaderCapsule {
    mmap: Mmap,
    record_count: u64,
    feature_offset: usize,
    label_offset: usize,
    metadata_offset: usize,
}

#[cfg(feature = "persistence-binary-io")]
impl BinaryReaderCapsule {
    /// Open binary training file with validation
    ///
    /// # Arguments
    /// - `path`: Binary file path
    ///
    /// # Returns
    /// - Reader with mmap handle and validated header
    ///
    /// # Performance
    /// - Mmap: <1ms
    /// - SHA-256 validation: <10ms (hardware acceleration)
    /// - Total: <10ms
    ///
    /// # ASSUM Safety
    /// - #ASSUME_FILE_OPEN: File I/O errors propagated via Result
    /// - #ASSUME_MMAP: Mmap succeeds (file exists and readable)
    /// - #ASSUME_HASH_VALID: SHA-256 verification detects tampering
    ///
    /// #VERIFY: Integration test validates tamper detection
    pub fn open(path: &Path) -> Result<Self> {
        use sha2::{Sha256, Digest};

        // Open file
        let file = File::open(path)
            .context("Failed to open binary training file")?;

        // Mmap file (zero-copy)
        let mmap = unsafe {
            // SAFETY: File is opened read-only, no concurrent writes
            // #ASSUME_MMAP_SAFE: OS ensures memory safety for read-only mmap
            memmap2::MmapOptions::new()
                .map(&file)
                .context("Failed to mmap binary file")?
        };

        // Validate minimum size
        if mmap.len() < HEADER_SIZE {
            bail!("File too small: {} bytes (expected ≥ {})", mmap.len(), HEADER_SIZE);
        }

        // Parse header
        let header = &mmap[0..HEADER_SIZE];

        // Validate magic
        if &header[0..8] != MAGIC {
            bail!("Invalid magic number: expected {:?}, got {:?}",
                MAGIC, &header[0..8]);
        }

        // Validate version
        let major = u16::from_le_bytes([header[8], header[9]]);
        let minor = u16::from_le_bytes([header[10], header[11]]);

        if major != VERSION_MAJOR {
            bail!("Unsupported version: {}.{} (expected {}.{})",
                major, minor, VERSION_MAJOR, VERSION_MINOR);
        }

        // Parse record count
        let record_count = u64::from_le_bytes([
            header[12], header[13], header[14], header[15],
            header[16], header[17], header[18], header[19],
        ]);

        // Validate feature dimension
        let feature_dim = u32::from_le_bytes([
            header[20], header[21], header[22], header[23],
        ]);

        if feature_dim != FEATURE_DIM {
            bail!("Invalid feature dimension: {} (expected {})",
                feature_dim, FEATURE_DIM);
        }

        // Validate compression type
        if header[24] != COMPRESSION_Q8_8 {
            bail!("Unsupported compression: 0x{:02x} (expected Q8.8 = 0x01)",
                header[24]);
        }

        // Extract hashes from header
        let feature_hash_expected = &header[28..60];
        let label_hash_expected = &header[60..92];
        let metadata_hash_expected = &header[92..124];
        let header_hash_expected = &header[124..156];

        // Validate header hash
        let mut header_hasher = Sha256::new();
        header_hasher.update(&header[0..124]);
        let header_hash_actual = header_hasher.finalize();

        if &header_hash_actual[..] != header_hash_expected {
            bail!("Header hash mismatch: file may be corrupted or tampered");
        }

        // Calculate section offsets
        let feature_offset = HEADER_SIZE;
        let label_offset = feature_offset + (record_count as usize * BYTES_PER_RECORD);
        let metadata_offset = label_offset + (record_count as usize * BYTES_PER_LABEL);

        // Validate file size
        let expected_size = metadata_offset + (record_count as usize * BYTES_PER_METADATA);
        if mmap.len() < expected_size {
            bail!("File truncated: {} bytes (expected ≥ {})", mmap.len(), expected_size);
        }

        // Validate feature data hash
        let feature_data = &mmap[feature_offset..label_offset];
        let mut feature_hasher = Sha256::new();
        feature_hasher.update(feature_data);
        let feature_hash_actual = feature_hasher.finalize();

        if &feature_hash_actual[..] != feature_hash_expected {
            bail!("Feature data hash mismatch: file may be corrupted");
        }

        // Validate label data hash
        let label_data = &mmap[label_offset..metadata_offset];
        let mut label_hasher = Sha256::new();
        label_hasher.update(label_data);
        let label_hash_actual = label_hasher.finalize();

        if &label_hash_actual[..] != label_hash_expected {
            bail!("Label data hash mismatch: file may be corrupted");
        }

        // Validate metadata hash
        let metadata_data = &mmap[metadata_offset..expected_size];
        let mut metadata_hasher = Sha256::new();
        metadata_hasher.update(metadata_data);
        let metadata_hash_actual = metadata_hasher.finalize();

        if &metadata_hash_actual[..] != metadata_hash_expected {
            bail!("Metadata hash mismatch: file may be corrupted");
        }

        Ok(Self {
            mmap,
            record_count,
            feature_offset,
            label_offset,
            metadata_offset,
        })
    }

    /// Read single record by index
    ///
    /// # Arguments
    /// - `index`: Record index (0 to record_count-1)
    ///
    /// # Returns
    /// - Decoded training record
    ///
    /// # Performance
    /// - Zero-copy: <100ns (decode Q8.8 on-the-fly)
    /// - Page cache: OS manages memory efficiently
    ///
    /// # ASSUM Safety
    /// - #ASSUME_BOUNDS: Index validated before access
    /// - #ASSUME_DECODE: Q8.8 decode preserves ±0.004 precision
    ///
    /// #VERIFY: Bounds test ensures panic-free operation
    pub fn read_record(&self, index: u64) -> Result<TrainingRecord> {
        // Validate index
        if index >= self.record_count {
            bail!("Index {} out of bounds (total records: {})",
                index, self.record_count);
        }

        let idx = index as usize;

        // Read feature data (Q8.8 encoded)
        let feature_start = self.feature_offset + idx * BYTES_PER_RECORD;
        let feature_end = feature_start + BYTES_PER_RECORD;
        let feature_bytes = &self.mmap[feature_start..feature_end];

        // Decode features from Q8.8
        let mut encoded = [0i16; 126];
        for i in 0..126 {
            let offset = i * 2;
            encoded[i] = i16::from_le_bytes([
                feature_bytes[offset],
                feature_bytes[offset + 1],
            ]);
        }

        let features_vec = QuantizerCapsule::decode_batch(&encoded);
        let features: [f64; 126] = features_vec.try_into()
            .map_err(|_| anyhow::anyhow!("Feature vector size mismatch"))?;

        // Read label
        let label_byte = self.mmap[self.label_offset + idx];
        let label = match label_byte {
            0 => StrategyLabel::Trend,
            1 => StrategyLabel::MeanReversion,
            2 => StrategyLabel::Breakout,
            3 => StrategyLabel::Range,
            _ => bail!("Invalid label: {} at index {}", label_byte, index),
        };

        // Read metadata
        let metadata_start = self.metadata_offset + idx * BYTES_PER_METADATA;
        let metadata_bytes = &self.mmap[metadata_start..metadata_start + BYTES_PER_METADATA];

        let timestamp = u64::from_le_bytes([
            metadata_bytes[0], metadata_bytes[1], metadata_bytes[2], metadata_bytes[3],
            metadata_bytes[4], metadata_bytes[5], metadata_bytes[6], metadata_bytes[7],
        ]);

        let regime_id = u64::from_le_bytes([
            metadata_bytes[8], metadata_bytes[9], metadata_bytes[10], metadata_bytes[11],
            metadata_bytes[12], metadata_bytes[13], metadata_bytes[14], metadata_bytes[15],
        ]);

        Ok(TrainingRecord {
            features,
            label,
            timestamp,
            regime_id,
        })
    }

    /// Get total record count
    #[inline]
    pub fn record_count(&self) -> u64 {
        self.record_count
    }

    /// Create iterator over all records
    pub fn iter(&self) -> BinaryReaderIterator {
        BinaryReaderIterator {
            reader: self,
            index: 0,
        }
    }
}

/// Iterator over binary training records
#[cfg(feature = "persistence-binary-io")]
pub struct BinaryReaderIterator<'a> {
    reader: &'a BinaryReaderCapsule,
    index: u64,
}

#[cfg(feature = "persistence-binary-io")]
impl<'a> Iterator for BinaryReaderIterator<'a> {
    type Item = Result<TrainingRecord>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.reader.record_count {
            return None;
        }

        let record = self.reader.read_record(self.index);
        self.index += 1;

        Some(record)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.reader.record_count - self.index) as usize;
        (remaining, Some(remaining))
    }
}

#[cfg(feature = "persistence-binary-io")]
impl<'a> ExactSizeIterator for BinaryReaderIterator<'a> {}

#[cfg(all(test, feature = "persistence-binary-io"))]
mod tests {
    use super::*;
    use crate::persistence::binary_writer::BinaryWriterCapsule;
    use tempfile::NamedTempFile;

    fn create_test_file(records: usize) -> (NamedTempFile, Vec<[f64; 126]>) {
        let temp = NamedTempFile::new().unwrap();
        let mut writer = BinaryWriterCapsule::new(temp.path()).unwrap();

        let mut original_features = Vec::new();

        for i in 0..records {
            let features: [f64; 126] = core::array::from_fn(|j| {
                ((i + j) as f64 / 1000.0) * 2.0 - 1.0
            });

            let label = match i % 4 {
                0 => StrategyLabel::Trend,
                1 => StrategyLabel::MeanReversion,
                2 => StrategyLabel::Breakout,
                _ => StrategyLabel::Range,
            };

            writer.write_record(&features, label, i as u64 * 1_000_000, i as u64).unwrap();
            original_features.push(features);
        }

        writer.finalize().unwrap();

        (temp, original_features)
    }

    #[test]
    fn test_open_and_read() {
        let (temp, _) = create_test_file(10);
        let reader = BinaryReaderCapsule::open(temp.path()).unwrap();

        assert_eq!(reader.record_count(), 10);
    }

    #[test]
    fn test_read_single_record() {
        let (temp, original) = create_test_file(10);
        let reader = BinaryReaderCapsule::open(temp.path()).unwrap();

        let record = reader.read_record(0).unwrap();

        // Validate round-trip (within Q8.8 precision)
        for i in 0..126 {
            let error = (record.features[i] - original[0][i]).abs();
            assert!(error <= 0.004, "Feature {} error {} exceeds tolerance", i, error);
        }

        assert_eq!(record.label as u8, StrategyLabel::Trend as u8);
        assert_eq!(record.timestamp, 0);
        assert_eq!(record.regime_id, 0);
    }

    #[test]
    fn test_read_all_labels() {
        let (temp, _) = create_test_file(4);
        let reader = BinaryReaderCapsule::open(temp.path()).unwrap();

        assert_eq!(reader.read_record(0).unwrap().label as u8, StrategyLabel::Trend as u8);
        assert_eq!(reader.read_record(1).unwrap().label as u8, StrategyLabel::MeanReversion as u8);
        assert_eq!(reader.read_record(2).unwrap().label as u8, StrategyLabel::Breakout as u8);
        assert_eq!(reader.read_record(3).unwrap().label as u8, StrategyLabel::Range as u8);
    }

    #[test]
    fn test_read_bounds_check() {
        let (temp, _) = create_test_file(10);
        let reader = BinaryReaderCapsule::open(temp.path()).unwrap();

        // Valid indices
        assert!(reader.read_record(0).is_ok());
        assert!(reader.read_record(9).is_ok());

        // Invalid indices
        assert!(reader.read_record(10).is_err());
        assert!(reader.read_record(100).is_err());
    }

    #[test]
    fn test_iterator() {
        let (temp, _) = create_test_file(10);
        let reader = BinaryReaderCapsule::open(temp.path()).unwrap();

        let records: Vec<_> = reader.iter().collect();
        assert_eq!(records.len(), 10);

        for (i, record) in records.into_iter().enumerate() {
            let record = record.unwrap();
            assert_eq!(record.timestamp, i as u64 * 1_000_000);
            assert_eq!(record.regime_id, i as u64);
        }
    }

    #[test]
    fn test_iterator_exact_size() {
        let (temp, _) = create_test_file(10);
        let reader = BinaryReaderCapsule::open(temp.path()).unwrap();

        let iter = reader.iter();
        assert_eq!(iter.len(), 10);

        let iter = iter.skip(3);
        assert_eq!(iter.len(), 7);
    }

    #[test]
    fn test_large_dataset_roundtrip() {
        let (temp, original) = create_test_file(1000);
        let reader = BinaryReaderCapsule::open(temp.path()).unwrap();

        assert_eq!(reader.record_count(), 1000);

        // Spot check random indices
        for idx in [0, 100, 500, 999] {
            let record = reader.read_record(idx).unwrap();

            for i in 0..126 {
                let error = (record.features[i] - original[idx as usize][i]).abs();
                assert!(error <= 0.004,
                    "Record {} feature {} error {} exceeds tolerance",
                    idx, i, error);
            }
        }
    }
}
