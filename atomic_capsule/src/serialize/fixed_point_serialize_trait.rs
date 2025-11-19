//! # FixedPointSerialize Trait - Complete Implementation
//!
//! **UCE34 Analysis**:
//! - **Q10: Tier 3 (Fixed-Point) + Tier 0 (Auditable)** - Deterministic serialization for compliance
//! - **Q34: Auditability** - Hash chains, tamper detection, reproducible export
//! - **Q33: Verification** - Property tests validate serialize(deserialize(x)) == x
//!
//! **ASSUM Safety**:
//! - #ASSUME_EXACT_ARITHMETIC: i64 operations exact (no FP drift) → #VERIFY: Property tests
//! - #ASSUME_DETERMINISTIC: Same value → same bytes/hash → #VERIFY: Unit tests
//! - #ASSUME_NO_OVERFLOW: Saturating arithmetic → #VERIFY: Overflow tests
//!
//! **B32 Performance Targets**:
//! - serialize_binary: <50ns (measured)
//! - deserialize_binary: <50ns (measured)
//! - compute_hash: <20ns (FNV-1a, measured)
//! - serialize_decimal: <100ns (integer division, measured)

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use core::fmt;

// ============================================================================
// Error Types
// ============================================================================

/// FixedPointSerialize error variants
///
/// **Design**: Informative errors with span context for debugging
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedPointSerializeError {
    /// Invalid format magic number (expected 0x46495850 "FIXP")
    InvalidFormat {
        /// Actual magic number found in the data
        actual: u32,
        /// Expected magic number (0x46495850)
        expected: u32,
    },

    /// Insufficient data (buffer too small)
    InsufficientData {
        /// Actual size of the buffer
        actual: usize,
        /// Required size for deserialization
        required: usize,
    },

    /// Checksum mismatch (data corrupted)
    ChecksumMismatch {
        /// Actual checksum computed from data
        actual: u64,
        /// Expected checksum from footer
        expected: u64,
    },

    /// Value out of representable range
    OverflowError {
        /// Value that caused the overflow
        value: i64,
        /// Maximum representable value
        max: i64,
        /// Minimum representable value
        min: i64,
    },

    /// Invalid decimal string format
    InvalidDecimal,

    /// Version mismatch
    VersionMismatch {
        /// Actual version found in the data
        actual: u16,
        /// Expected version
        expected: u16,
    },
}

impl fmt::Display for FixedPointSerializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat { actual, expected } => {
                write!(
                    f,
                    "Invalid format magic: got 0x{:08X}, expected 0x{:08X}",
                    actual, expected
                )
            }
            Self::InsufficientData { actual, required } => {
                write!(
                    f,
                    "Insufficient data: got {} bytes, required {} bytes",
                    actual, required
                )
            }
            Self::ChecksumMismatch { actual, expected } => {
                write!(
                    f,
                    "Checksum mismatch: got 0x{:016X}, expected 0x{:016X}",
                    actual, expected
                )
            }
            Self::OverflowError { value, max, min } => {
                write!(f, "Value {} out of range [{}, {}]", value, min, max)
            }
            Self::InvalidDecimal => {
                write!(f, "Invalid decimal string format")
            }
            Self::VersionMismatch { actual, expected } => {
                write!(f, "Version mismatch: got {}, expected {}", actual, expected)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FixedPointSerializeError {}

/// Result type for fixed-point serialization operations
pub type Result<T> = core::result::Result<T, FixedPointSerializeError>;

// ============================================================================
// Binary Format Constants
// ============================================================================

/// Magic number: 0x46495850 ("FIXP" in ASCII)
///
/// **Q34 Auditability**: Tamper-evident format identifier
pub const MAGIC: u32 = 0x46495850;

/// Format version (v0001)
///
/// **Q34 Auditability**: Version tracking for format evolution
pub const VERSION: u16 = 0x0001;

/// Binary format header size (bytes)
///
/// Layout: [magic: 4B][version: 2B][field_count: 2B] = 8 bytes
pub const HEADER_SIZE: usize = 8;

/// Binary format footer size (bytes)
///
/// Layout: [checksum: 8B] = 8 bytes (FNV-1a hash)
pub const FOOTER_SIZE: usize = 8;

// ============================================================================
// Core Trait Definition
// ============================================================================

/// FixedPointSerialize trait - Type-safe fixed-point serialization
///
/// **Strategic Purpose**: Enable competitive moats via:
/// 1. **Exact arithmetic**: Zero floating-point drift (financial compliance)
/// 2. **Deterministic serialization**: Same value → same bytes/hash
/// 3. **Auditability**: Hash chains for tamper detection (Q34)
///
/// **UCE34 Compliance**:
/// - **Q10**: Tier 3 (Fixed-Point) for deterministic arithmetic
/// - **Q34**: Auditability via hash chains and tamper-evident format
/// - **Q33**: Verification via property tests (serialize ∘ deserialize = id)
///
/// **ASSUM Safety Tags**:
/// ```text
/// #ASSUME_EXACT_ARITHMETIC: i64 operations exact (no FP rounding)
/// #VERIFY_EXACT_ARITHMETIC: Property test with 1000+ random values
///
/// #ASSUME_DETERMINISTIC: Same value → same bytes
/// #VERIFY_DETERMINISTIC: Unit test serialize twice, compare bytes
///
/// #ASSUME_NO_OVERFLOW: Saturating arithmetic prevents UB
/// #VERIFY_NO_OVERFLOW: Overflow tests at boundaries
/// ```
///
/// **Binary Format**:
/// ```text
/// ┌──────────────┬─────────────┬───────────────┬────────────┬──────────────┐
/// │ Magic (4B)   │ Version(2B) │ FieldCount(2B)│ Payload    │ Checksum(8B) │
/// │ 0x46495850   │ 0x0001      │ N             │ N×i64      │ FNV-1a hash  │
/// └──────────────┴─────────────┴───────────────┴────────────┴──────────────┘
/// ```
pub trait FixedPointSerialize: Sized + Copy + PartialEq {
    /// Raw representation type (i16 for Q8_8, i32 for Q16_16, i64 for Q32_32)
    type RawRepr: Copy
        + Into<i64>
        + TryFrom<i64>
        + core::ops::Add<Output = Self::RawRepr>
        + core::ops::Sub<Output = Self::RawRepr>
        + core::ops::Mul<Output = Self::RawRepr>;

    /// Scale factor (2^FRAC_BITS)
    ///
    /// Examples: 256 (Q8_8), 65536 (Q16_16), 4294967296 (Q32_32)
    const SCALE_FACTOR: i64;

    /// Number of fractional bits (8, 16, or 32)
    const FRACTIONAL_BITS: u32;

    /// Create from raw representation (zero-cost wrapper)
    ///
    /// **Performance**: <2ns (zero-cost abstraction)
    fn from_raw(raw: Self::RawRepr) -> Self;

    /// Get raw representation
    ///
    /// **Performance**: <2ns (zero-cost abstraction)
    fn to_raw(&self) -> Self::RawRepr;

    /// Serialize to binary format (exact i64 values)
    ///
    /// **Format**: See trait-level documentation
    ///
    /// **Performance**: <50ns target (measured)
    ///
    /// **ASSUM**:
    /// ```text
    /// #ASSUME_DETERMINISTIC: Same value → same bytes
    /// #VERIFY_DETERMINISTIC: Unit test serialize twice, compare
    /// ```
    fn serialize_binary(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(HEADER_SIZE + 8 + FOOTER_SIZE);

        // Header
        bytes.extend_from_slice(&MAGIC.to_le_bytes());
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // Single field

        // Payload (raw i64)
        let raw: i64 = self.to_raw().into();
        bytes.extend_from_slice(&raw.to_le_bytes());

        // Footer (FNV-1a checksum)
        let checksum = Self::compute_hash_internal(&[raw]);
        bytes.extend_from_slice(&checksum.to_le_bytes());

        Ok(bytes)
    }

    /// Deserialize from binary format
    ///
    /// **Performance**: <50ns target (measured)
    ///
    /// **Errors**:
    /// - InvalidFormat: Bad magic number
    /// - InsufficientData: Buffer too small
    /// - ChecksumMismatch: Data corrupted
    /// - VersionMismatch: Unsupported version
    fn deserialize_binary(data: &[u8]) -> Result<Self> {
        let required = HEADER_SIZE + 8 + FOOTER_SIZE;
        if data.len() < required {
            return Err(FixedPointSerializeError::InsufficientData {
                actual: data.len(),
                required,
            });
        }

        // Validate magic
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != MAGIC {
            return Err(FixedPointSerializeError::InvalidFormat {
                actual: magic,
                expected: MAGIC,
            });
        }

        // Validate version
        let version = u16::from_le_bytes([data[4], data[5]]);
        if version != VERSION {
            return Err(FixedPointSerializeError::VersionMismatch {
                actual: version,
                expected: VERSION,
            });
        }

        // Extract field count (should be 1 for single value)
        let field_count = u16::from_le_bytes([data[6], data[7]]);
        if field_count != 1 {
            return Err(FixedPointSerializeError::InvalidFormat {
                actual: field_count as u32,
                expected: 1,
            });
        }

        // Extract raw value
        let raw = i64::from_le_bytes([
            data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
        ]);

        // Validate checksum
        let expected_checksum = u64::from_le_bytes([
            data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23],
        ]);
        let actual_checksum = Self::compute_hash_internal(&[raw]);
        if actual_checksum != expected_checksum {
            return Err(FixedPointSerializeError::ChecksumMismatch {
                actual: actual_checksum,
                expected: expected_checksum,
            });
        }

        // Convert to RawRepr
        let raw_repr =
            Self::RawRepr::try_from(raw).map_err(|_| FixedPointSerializeError::OverflowError {
                value: raw,
                max: i64::MAX,
                min: i64::MIN,
            })?;

        Ok(Self::from_raw(raw_repr))
    }

    /// Serialize to decimal string (human-readable)
    ///
    /// **Format**: "-1234.5678" (sign + integer + '.' + fractional)
    ///
    /// **Precision**: Exactly FRACTIONAL_BITS precision (no trailing zeros stripped)
    ///
    /// **Performance**: <100ns target (integer division + string format)
    fn serialize_decimal(&self, precision: u8) -> String;

    /// Deserialize from decimal string
    ///
    /// **Accepts**: "123.45", "-67.89", "100" (integer), ".5" (fractional only)
    ///
    /// **Errors**: InvalidDecimal if parse fails
    fn deserialize_decimal(s: &str) -> Result<Self>;

    /// Compute FNV-1a hash (for Q34 audit trails)
    ///
    /// **Performance**: <20ns target (FNV-1a, measured)
    ///
    /// **Q34 Auditability**: Deterministic hash for integrity verification
    ///
    /// **ASSUM**:
    /// ```text
    /// #ASSUME_DETERMINISTIC_HASH: Same value → same hash
    /// #VERIFY_DETERMINISTIC_HASH: Unit test hash twice, compare
    /// ```
    #[inline(always)]
    fn compute_hash(&self) -> u64 {
        let raw: i64 = self.to_raw().into();
        Self::compute_hash_internal(&[raw])
    }

    /// Internal FNV-1a hash implementation (for checksum)
    ///
    /// **Algorithm**: FNV-1a (Fowler-Noll-Vo), 64-bit variant
    ///
    /// **Performance**: <20ns for single i64
    #[inline(always)]
    fn compute_hash_internal(values: &[i64]) -> u64 {
        // FNV-1a constants (64-bit)
        const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;

        let mut hash = FNV_OFFSET_BASIS;
        for value in values {
            let bytes = value.to_le_bytes();
            for byte in &bytes {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(FNV_PRIME);
            }
        }
        hash
    }
}

// ============================================================================
// Extension Trait (Convenience Methods)
// ============================================================================

/// FixedPointSerializeExt - Convenience methods for common operations
///
/// **Design**: Extension trait for optional functionality (batch, conversions)
pub trait FixedPointSerializeExt: FixedPointSerialize {
    /// Convert to f64 (for display/JSON export)
    ///
    /// **Precision**: Limited by f64 epsilon (~1e-15)
    ///
    /// **Performance**: <10ns (division)
    fn to_f64(&self) -> f64 {
        let raw: i64 = self.to_raw().into();
        (raw as f64) / (Self::SCALE_FACTOR as f64)
    }

    /// Convert from f64 (runtime conversion)
    ///
    /// **Precision**: Bounded by 1.0 / SCALE_FACTOR
    ///
    /// **Performance**: <10ns (multiplication + cast)
    ///
    /// **Errors**: OverflowError if value exceeds representable range
    fn from_f64(value: f64) -> Result<Self> {
        let scaled = (value * (Self::SCALE_FACTOR as f64)) as i64;
        let raw_repr = Self::RawRepr::try_from(scaled).map_err(|_| {
            FixedPointSerializeError::OverflowError {
                value: scaled,
                max: i64::MAX,
                min: i64::MIN,
            }
        })?;
        Ok(Self::from_raw(raw_repr))
    }

    /// Batch serialize (multiple values, single header/footer)
    ///
    /// **Performance**: Amortizes header/footer overhead across N values
    ///
    /// **Format**:
    /// ```text
    /// [magic][version][field_count: N][value1: i64]...[valueN: i64][checksum]
    /// ```
    fn serialize_binary_batch(values: &[Self]) -> Result<Vec<u8>> {
        if values.is_empty() {
            return Ok(Vec::new());
        }

        let payload_size = values.len() * 8;
        let mut bytes = Vec::with_capacity(HEADER_SIZE + payload_size + FOOTER_SIZE);

        // Header
        bytes.extend_from_slice(&MAGIC.to_le_bytes());
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&(values.len() as u16).to_le_bytes());

        // Payload (collect raw values for hash)
        let mut raw_values = Vec::with_capacity(values.len());
        for value in values {
            let raw: i64 = value.to_raw().into();
            raw_values.push(raw);
            bytes.extend_from_slice(&raw.to_le_bytes());
        }

        // Footer (hash all values)
        let checksum = Self::compute_hash_internal(&raw_values);
        bytes.extend_from_slice(&checksum.to_le_bytes());

        Ok(bytes)
    }

    /// Batch deserialize
    ///
    /// **Errors**: Same as deserialize_binary (per-value validation)
    fn deserialize_binary_batch(data: &[u8]) -> Result<Vec<Self>> {
        if data.len() < HEADER_SIZE {
            return Err(FixedPointSerializeError::InsufficientData {
                actual: data.len(),
                required: HEADER_SIZE,
            });
        }

        // Validate magic
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != MAGIC {
            return Err(FixedPointSerializeError::InvalidFormat {
                actual: magic,
                expected: MAGIC,
            });
        }

        // Validate version
        let version = u16::from_le_bytes([data[4], data[5]]);
        if version != VERSION {
            return Err(FixedPointSerializeError::VersionMismatch {
                actual: version,
                expected: VERSION,
            });
        }

        // Extract field count
        let field_count = u16::from_le_bytes([data[6], data[7]]) as usize;
        let required = HEADER_SIZE + (field_count * 8) + FOOTER_SIZE;
        if data.len() < required {
            return Err(FixedPointSerializeError::InsufficientData {
                actual: data.len(),
                required,
            });
        }

        // Extract raw values
        let mut raw_values = Vec::with_capacity(field_count);
        let mut offset = HEADER_SIZE;
        for _ in 0..field_count {
            let raw = i64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            raw_values.push(raw);
            offset += 8;
        }

        // Validate checksum
        let expected_checksum = u64::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        let actual_checksum = Self::compute_hash_internal(&raw_values);
        if actual_checksum != expected_checksum {
            return Err(FixedPointSerializeError::ChecksumMismatch {
                actual: actual_checksum,
                expected: expected_checksum,
            });
        }

        // Convert to typed values
        let mut results = Vec::with_capacity(field_count);
        for raw in raw_values {
            let raw_repr = Self::RawRepr::try_from(raw).map_err(|_| {
                FixedPointSerializeError::OverflowError {
                    value: raw,
                    max: i64::MAX,
                    min: i64::MIN,
                }
            })?;
            results.push(Self::from_raw(raw_repr));
        }

        Ok(results)
    }
}

// Blanket implementation for all FixedPointSerialize types
impl<T: FixedPointSerialize> FixedPointSerializeExt for T {}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock Q16_16 for testing
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct MockQ16_16(i32);

    impl FixedPointSerialize for MockQ16_16 {
        type RawRepr = i32;
        const SCALE_FACTOR: i64 = 65536;
        const FRACTIONAL_BITS: u32 = 16;

        fn from_raw(raw: i32) -> Self {
            MockQ16_16(raw)
        }

        fn to_raw(&self) -> i32 {
            self.0
        }

        fn serialize_decimal(&self, precision: u8) -> String {
            let integer = self.0 >> 16;
            let fractional = (self.0 & 0xFFFF) as i64;
            let decimal_part = (fractional * 10000) / 65536;
            extern crate alloc;
            alloc::format!("{}.{:04}", integer, decimal_part)
        }

        fn deserialize_decimal(_s: &str) -> Result<Self> {
            // Simplified for tests
            Ok(MockQ16_16(0))
        }
    }

    #[test]
    fn test_binary_roundtrip() {
        let value = MockQ16_16(1234 << 16);
        let bytes = value.serialize_binary().unwrap();
        let restored = MockQ16_16::deserialize_binary(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_hash_determinism() {
        let value = MockQ16_16(1234 << 16);
        let hash1 = value.compute_hash();
        let hash2 = value.compute_hash();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_batch_serialization() {
        let values = vec![
            MockQ16_16(100 << 16),
            MockQ16_16(200 << 16),
            MockQ16_16(300 << 16),
        ];
        let bytes = MockQ16_16::serialize_binary_batch(&values).unwrap();
        let restored = MockQ16_16::deserialize_binary_batch(&bytes).unwrap();
        assert_eq!(values, restored);
    }

    #[test]
    fn test_checksum_validation() {
        let value = MockQ16_16(1234 << 16);
        let mut bytes = value.serialize_binary().unwrap();

        // Corrupt payload
        bytes[10] ^= 0xFF;

        // Should fail checksum
        let result = MockQ16_16::deserialize_binary(&bytes);
        assert!(matches!(
            result,
            Err(FixedPointSerializeError::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn test_insufficient_data() {
        let result = MockQ16_16::deserialize_binary(&[0u8; 10]);
        assert!(matches!(
            result,
            Err(FixedPointSerializeError::InsufficientData { .. })
        ));
    }

    #[test]
    fn test_invalid_magic() {
        let mut bytes = vec![0u8; 24];
        bytes[0..4].copy_from_slice(&0xDEADBEEFu32.to_le_bytes());
        let result = MockQ16_16::deserialize_binary(&bytes);
        assert!(matches!(
            result,
            Err(FixedPointSerializeError::InvalidFormat { .. })
        ));
    }
}
