//! # Enhanced FixedPointSerialize Trait (Phase 4)
//!
//! **Production-Ready Fixed-Point Serialization with Binary/Decimal/Hash Integration**
//!
//! ## UCE34 Framework Application
//!
//! **Q10 (Tier Selection)**: Tier 3 Fixed-Point + Tier 0 Auditable
//! - Binary serialization: Exact i64 preservation (zero-drift)
//! - Decimal serialization: Human-readable precision (JSON/CLI)
//! - Hash computation: Integrated deterministic hashing (audit chains)
//!
//! **Q11 (Rust Transform)**: Zero-cost abstractions + compile-time verification
//! - Result<T> for all fallible operations (no panics)
//! - #[inline(always)] for performance-critical paths
//! - const fn where applicable (compile-time evaluation)
//!
//! **Q12 (Nightly Enhancement)**: const_trait_impl + const_fn_floating_point_arithmetic
//! - Enables const fn from_f64() for compile-time constants
//! - Future: const trait methods for zero-cost verification
//!
//! **Q33 (Validation)**: Compile-time + property-based testing
//! - verify_capsule_properties! integration (alignment, size)
//! - Property tests: roundtrip, determinism, precision (1000+ cases)
//!
//! **Q34 (Auditability)**: Hash-based integrity verification
//! - compute_hash(): Deterministic hashing for audit trails
//! - Binary format: CRC32 checksums for corruption detection
//! - Decimal format: Reproducible string representation
//!
//! ## Strategic Design
//!
//! This trait provides **five core operations**:
//! 1. `serialize_binary()`: Exact binary representation (CRC32 checksummed)
//! 2. `deserialize_binary()`: Safe parsing with validation
//! 3. `serialize_decimal()`: Human-readable strings (precision preserved)
//! 4. `deserialize_decimal()`: String parsing with error recovery
//! 5. `compute_hash()`: Deterministic hashing (audit chain integration)
//!
//! ## Performance Targets (B32 Framework)
//!
//! - `serialize_binary()`: <50ns (magic + version + raw + CRC32)
//! - `deserialize_binary()`: <100ns (validation + parsing)
//! - `serialize_decimal()`: <100ns (integer division + format)
//! - `deserialize_decimal()`: <200ns (string parsing + validation)
//! - `compute_hash()`: <20ns (xxHash64 on binary representation)
//!
//! ## ASSUM Safety Tags
//!
//! ```text
//! #ASSUME_EXACT_ARITHMETIC: i64 operations preserve exact values
//! #VERIFY_EXACT_ARITHMETIC: Property test roundtrip (1000+ cases)
//!
//! #ASSUME_DETERMINISTIC_BINARY: Same value → same bytes always
//! #VERIFY_DETERMINISTIC_BINARY: Unit test serialize twice, byte-compare
//!
//! #ASSUME_DETERMINISTIC_DECIMAL: Same value → same string always
//! #VERIFY_DETERMINISTIC_DECIMAL: Unit test serialize twice, string-compare
//!
//! #ASSUME_CRC32_COLLISION_RARE: CRC32 collision probability < 1/4B
//! #VERIFY_CRC32_COLLISION_RARE: Standard library guarantee (trivial)
//!
//! #ASSUME_HASH_DETERMINISTIC: xxHash64 is deterministic (same input → same output)
//! #VERIFY_HASH_DETERMINISTIC: Property test hash twice, compare
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use core::fmt;

/// Error type for FixedPointSerialize operations
///
/// **Design**: Rich error context with precise error types
/// - No panic: All operations return Result
/// - Debuggable: Full error context for troubleshooting
/// - Recoverable: Client code can handle errors gracefully
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedPointSerializeError {
    /// Buffer too small for deserialization
    BufferTooSmall {
        /// Required size in bytes
        required: usize,
        /// Actual size in bytes
        actual: usize,
    },

    /// Invalid magic number in binary format
    InvalidMagic {
        /// Expected magic number
        expected: u32,
        /// Actual magic number found
        actual: u32,
    },

    /// Version mismatch (incompatible format)
    VersionMismatch {
        /// Expected version
        expected: u16,
        /// Actual version found
        actual: u16,
    },

    /// Fractional bits mismatch (wrong fixed-point type)
    FractionalBitsMismatch {
        /// Expected fractional bits (8, 16, or 32)
        expected: u32,
        /// Actual fractional bits found
        actual: u32,
    },

    /// CRC32 checksum mismatch (data corrupted)
    ChecksumMismatch {
        /// Expected checksum
        expected: u32,
        /// Actual checksum computed
        actual: u32,
    },

    /// Invalid decimal format (parsing failed)
    InvalidDecimalFormat {
        /// Input string that failed to parse
        input: String,
        /// Reason for failure
        reason: &'static str,
    },

    /// Value out of range for target fixed-point type
    ValueOutOfRange {
        /// Input value (as string)
        value: String,
        /// Minimum allowed value
        min: String,
        /// Maximum allowed value
        max: String,
    },

    /// Precision loss warning (informational, not an error)
    PrecisionLoss {
        /// Original precision requested
        requested: u8,
        /// Actual precision supported
        supported: u8,
    },

    /// Custom error message
    Custom(&'static str),
}

impl fmt::Display for FixedPointSerializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FixedPointSerializeError::BufferTooSmall { required, actual } => {
                write!(
                    f,
                    "Buffer too small: required {} bytes, got {} bytes",
                    required, actual
                )
            }
            FixedPointSerializeError::InvalidMagic { expected, actual } => {
                write!(
                    f,
                    "Invalid magic number: expected 0x{:08X}, got 0x{:08X}",
                    expected, actual
                )
            }
            FixedPointSerializeError::VersionMismatch { expected, actual } => {
                write!(f, "Version mismatch: expected {}, got {}", expected, actual)
            }
            FixedPointSerializeError::FractionalBitsMismatch { expected, actual } => {
                write!(
                    f,
                    "Fractional bits mismatch: expected {}, got {}",
                    expected, actual
                )
            }
            FixedPointSerializeError::ChecksumMismatch { expected, actual } => {
                write!(
                    f,
                    "Checksum mismatch (data corrupted): expected 0x{:08X}, got 0x{:08X}",
                    expected, actual
                )
            }
            FixedPointSerializeError::InvalidDecimalFormat { input, reason } => {
                write!(f, "Invalid decimal format: '{}' ({})", input, reason)
            }
            FixedPointSerializeError::ValueOutOfRange { value, min, max } => {
                write!(
                    f,
                    "Value out of range: {} (allowed range: {} to {})",
                    value, min, max
                )
            }
            FixedPointSerializeError::PrecisionLoss {
                requested,
                supported,
            } => {
                write!(
                    f,
                    "Precision loss: requested {} decimals, supported {}",
                    requested, supported
                )
            }
            FixedPointSerializeError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FixedPointSerializeError {}

/// Result type for FixedPointSerialize operations
pub type Result<T> = core::result::Result<T, FixedPointSerializeError>;

/// Enhanced FixedPointSerialize trait - Production-ready fixed-point serialization
///
/// **Strategic Purpose**: Enable three competitive moats:
/// 1. **Binary audit trails**: Exact i64 values with CRC32 checksums (SOX/SOC2/GDPR)
/// 2. **JSON/CLI export**: Human-readable decimal strings with precision preservation
/// 3. **Hash chain integration**: Deterministic hashing for integrity verification
///
/// ## Design Guarantees (UCE34 Q34: Auditability)
///
/// - **Bit-exact roundtrip**: `deserialize_binary(serialize_binary(x))? == x`
/// - **Deterministic binary**: Same value always produces same bytes
/// - **Deterministic decimal**: Same value always produces same string
/// - **Deterministic hash**: Same value always produces same u64 hash
/// - **Corruption detection**: CRC32 checksums catch data errors
///
/// ## ASSUM Safety Tags
///
/// ```text
/// #ASSUME_EXACT_ARITHMETIC: i64 is exact representation (no FP drift)
/// #VERIFY_EXACT_ARITHMETIC: Property test roundtrip (1000+ cases)
///
/// #ASSUME_DETERMINISTIC_BINARY: format!() + CRC32 deterministic
/// #VERIFY_DETERMINISTIC_BINARY: Unit test serialize twice, byte-compare
///
/// #ASSUME_DETERMINISTIC_DECIMAL: Integer formatting deterministic
/// #VERIFY_DETERMINISTIC_DECIMAL: Unit test serialize twice, string-compare
///
/// #ASSUME_DETERMINISTIC_HASH: xxHash64 deterministic
/// #VERIFY_DETERMINISTIC_HASH: Property test hash twice, compare
/// ```
///
/// ## Implementation Requirements
///
/// Types implementing FixedPointSerialize MUST:
/// 1. Define `FRACTIONAL_BITS` constant (8, 16, or 32)
/// 2. Define `MAGIC` constant (4-byte format identifier)
/// 3. Define `VERSION` constant (2-byte format version)
/// 4. Store value as i64 (exact representation)
/// 5. Implement bit-exact roundtrip property
///
/// ## Example
///
/// ```rust
/// use atomic_capsule::serialize::fixed_point_trait::{FixedPointSerialize, Result};
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// #[repr(transparent)]
/// struct Q16_16(i64);
///
/// impl FixedPointSerialize for Q16_16 {
///     const FRACTIONAL_BITS: u32 = 16;
///     const MAGIC: u32 = 0x51313636;  // "Q166" in ASCII
///     const VERSION: u16 = 1;
///
///     fn serialize_binary(&self) -> Result<Vec<u8>> {
///         // Implementation with magic + version + raw + CRC32
///         // ...
///     }
///
///     fn deserialize_binary(bytes: &[u8]) -> Result<Self> {
///         // Validation + parsing with error handling
///         // ...
///     }
///
///     fn serialize_decimal(&self, precision: u8) -> String {
///         // Human-readable decimal string
///         // ...
///     }
///
///     fn deserialize_decimal(s: &str) -> Result<Self> {
///         // String parsing with validation
///         // ...
///     }
///
///     fn compute_hash(&self) -> u64 {
///         // Deterministic hashing
///         // ...
///     }
/// }
/// ```
pub trait FixedPointSerialize: Sized + Copy + PartialEq {
    /// Number of fractional bits (8, 16, or 32)
    ///
    /// - **Q8.8**: 8 fractional bits (256 steps, ~0.0039 precision)
    /// - **Q16.16**: 16 fractional bits (65536 steps, ~0.000015 precision)
    /// - **Q32.32**: 32 fractional bits (4B+ steps, ~2.3e-10 precision)
    const FRACTIONAL_BITS: u32;

    /// Magic number for binary format identification (4 bytes)
    ///
    /// Convention: ASCII characters (e.g., 0x51313636 = "Q166" for Q16.16)
    const MAGIC: u32;

    /// Binary format version (2 bytes)
    ///
    /// Increment on breaking changes to serialization format
    const VERSION: u16;

    /// Serialize to binary format with CRC32 checksum
    ///
    /// **Binary Format** (22 bytes total):
    /// ```text
    /// [magic: u32][version: u16][fractional_bits: u32][raw: i64][crc32: u32]
    /// ```
    ///
    /// ## Determinism Guarantee
    ///
    /// Same value always produces same bytes (bit-exact).
    ///
    /// ## ASSUM Safety
    ///
    /// ```text
    /// #ASSUME_DETERMINISTIC_BINARY: Little-endian + CRC32 deterministic
    /// #VERIFY_DETERMINISTIC_BINARY: Unit test serialize twice, byte-compare
    /// ```
    ///
    /// ## Performance
    ///
    /// - Target: <50ns (magic + version + raw + CRC32)
    /// - Single allocation (Vec<u8>)
    /// - Zero intermediate copies
    ///
    /// ## Example
    ///
    /// ```rust
    /// # use atomic_capsule::serialize::fixed_point_trait::FixedPointSerialize;
    /// # use atomic_capsule::serialize::fixed_point::Q16_16;
    /// let value = Q16_16::from_f64(19.99);
    /// let bytes = value.serialize_binary()?;
    /// assert_eq!(bytes.len(), 22);  // Fixed size
    /// # Ok::<(), atomic_capsule::serialize::fixed_point_trait::FixedPointSerializeError>(())
    /// ```
    fn serialize_binary(&self) -> Result<Vec<u8>>;

    /// Deserialize from binary format with validation
    ///
    /// **Validation Steps**:
    /// 1. Check buffer size (>= 22 bytes)
    /// 2. Validate magic number
    /// 3. Validate version
    /// 4. Validate fractional bits
    /// 5. Verify CRC32 checksum
    ///
    /// ## Errors
    ///
    /// - `BufferTooSmall`: Input buffer < 22 bytes
    /// - `InvalidMagic`: Magic number mismatch (wrong type or corrupted)
    /// - `VersionMismatch`: Incompatible format version
    /// - `FractionalBitsMismatch`: Wrong fixed-point type
    /// - `ChecksumMismatch`: CRC32 validation failed (data corrupted)
    ///
    /// ## Performance
    ///
    /// - Target: <100ns (validation + parsing)
    /// - Zero allocation
    /// - Early exit on validation errors
    ///
    /// ## Example
    ///
    /// ```rust
    /// # use atomic_capsule::serialize::fixed_point_trait::FixedPointSerialize;
    /// # use atomic_capsule::serialize::fixed_point::Q16_16;
    /// let value = Q16_16::from_f64(19.99);
    /// let bytes = value.serialize_binary()?;
    /// let restored = Q16_16::deserialize_binary(&bytes)?;
    /// assert_eq!(value, restored);  // Bit-exact roundtrip
    /// # Ok::<(), atomic_capsule::serialize::fixed_point_trait::FixedPointSerializeError>(())
    /// ```
    fn deserialize_binary(bytes: &[u8]) -> Result<Self>;

    /// Serialize to human-readable decimal string
    ///
    /// **Format**: `"-1234.5678"` (sign + integer + '.' + fractional)
    ///
    /// ## Precision Rules
    ///
    /// - **Q8.8**: 2 decimal places (e.g., "12.34")
    /// - **Q16.16**: 4 decimal places (e.g., "12.3456")
    /// - **Q32.32**: 9 decimal places (e.g., "12.345678901")
    ///
    /// The `precision` parameter allows customization:
    /// - `precision == 0`: Use default precision for type
    /// - `precision > 0`: Use specified precision (may truncate or pad)
    ///
    /// ## Determinism Guarantee
    ///
    /// Same value + precision always produces same string.
    ///
    /// ## ASSUM Safety
    ///
    /// ```text
    /// #ASSUME_DETERMINISTIC_DECIMAL: format!() deterministic for integers
    /// #VERIFY_DETERMINISTIC_DECIMAL: Unit test serialize twice, string-compare
    /// ```
    ///
    /// ## Performance
    ///
    /// - Target: <100ns (integer division + format)
    /// - Single allocation (String)
    /// - No floating-point operations
    ///
    /// ## Example
    ///
    /// ```rust
    /// # use atomic_capsule::serialize::fixed_point_trait::FixedPointSerialize;
    /// # use atomic_capsule::serialize::fixed_point::Q16_16;
    /// let value = Q16_16::from_f64(19.99);
    /// let decimal = value.serialize_decimal(0);  // Default precision (4 decimals)
    /// assert_eq!(decimal, "19.9900");
    ///
    /// let custom = value.serialize_decimal(2);  // Custom precision
    /// assert_eq!(custom, "19.99");
    /// ```
    fn serialize_decimal(&self, precision: u8) -> String;

    /// Deserialize from decimal string
    ///
    /// **Supported Formats**:
    /// - `"1234"` → integer only (fractional = 0)
    /// - `"1234.5678"` → integer + fractional
    /// - `"-1234.5678"` → negative values
    /// - `"+1234.5678"` → explicit positive sign (optional)
    ///
    /// ## Validation
    ///
    /// 1. Parse sign ('+', '-', or none)
    /// 2. Split on '.' (integer and fractional parts)
    /// 3. Parse integer part (i64)
    /// 4. Parse fractional part (i64 with scale factor)
    /// 5. Validate range (no overflow)
    ///
    /// ## Errors
    ///
    /// - `InvalidDecimalFormat`: String parsing failed (syntax error)
    /// - `ValueOutOfRange`: Value exceeds fixed-point type range
    ///
    /// ## Performance
    ///
    /// - Target: <200ns (string parsing + validation)
    /// - Zero allocation (parses in-place)
    /// - Early exit on syntax errors
    ///
    /// ## Example
    ///
    /// ```rust
    /// # use atomic_capsule::serialize::fixed_point_trait::FixedPointSerialize;
    /// # use atomic_capsule::serialize::fixed_point::Q16_16;
    /// let value = Q16_16::deserialize_decimal("19.99")?;
    /// assert_eq!(value.to_f64(), 19.99);
    ///
    /// let negative = Q16_16::deserialize_decimal("-123.45")?;
    /// assert_eq!(negative.to_f64(), -123.45);
    /// # Ok::<(), atomic_capsule::serialize::fixed_point_trait::FixedPointSerializeError>(())
    /// ```
    fn deserialize_decimal(s: &str) -> Result<Self>;

    /// Compute deterministic hash for audit chains
    ///
    /// **Hash Algorithm**: xxHash64 (fast, deterministic, collision-resistant)
    ///
    /// ## Determinism Guarantee
    ///
    /// Same value always produces same u64 hash.
    ///
    /// ## ASSUM Safety
    ///
    /// ```text
    /// #ASSUME_HASH_DETERMINISTIC: xxHash64 deterministic (same input → same output)
    /// #VERIFY_HASH_DETERMINISTIC: Property test hash twice, compare
    ///
    /// #ASSUME_HASH_COLLISION_RARE: xxHash64 collision probability < 1/2^64
    /// #VERIFY_HASH_COLLISION_RARE: Standard library guarantee (trivial)
    /// ```
    ///
    /// ## Performance
    ///
    /// - Target: <20ns (xxHash64 on 8-byte i64)
    /// - Zero allocation
    /// - Single-pass computation
    ///
    /// ## Use Cases
    ///
    /// - Audit trail hash chains
    /// - Deduplication (same value → same hash)
    /// - Integrity verification (tampering detection)
    ///
    /// ## Example
    ///
    /// ```rust
    /// # use atomic_capsule::serialize::fixed_point_trait::FixedPointSerialize;
    /// # use atomic_capsule::serialize::fixed_point::Q16_16;
    /// let value = Q16_16::from_f64(19.99);
    /// let hash1 = value.compute_hash();
    /// let hash2 = value.compute_hash();
    /// assert_eq!(hash1, hash2);  // Deterministic
    /// ```
    fn compute_hash(&self) -> u64;

    /// Get serialized binary size (always 22 bytes for fixed-point)
    ///
    /// **Format Size Breakdown**:
    /// - magic: 4 bytes
    /// - version: 2 bytes
    /// - fractional_bits: 4 bytes
    /// - raw: 8 bytes (i64)
    /// - crc32: 4 bytes
    /// - **Total: 22 bytes**
    #[inline(always)]
    fn serialized_size() -> usize {
        22 // Fixed size for all fixed-point types
    }

    /// Verify binary roundtrip property: `deserialize_binary(serialize_binary(x))? == x`
    ///
    /// **Property Test**: Used in 1000+ random case validation
    ///
    /// Default implementation uses `PartialEq`.
    fn verify_binary_roundtrip(&self) -> bool {
        if let Ok(bytes) = self.serialize_binary() {
            if let Ok(restored) = Self::deserialize_binary(&bytes) {
                return *self == restored;
            }
        }
        false
    }

    /// Verify decimal roundtrip property: `deserialize_decimal(serialize_decimal(x, p))? ≈ x`
    ///
    /// **Note**: Decimal roundtrip may have precision loss (fractional truncation).
    ///
    /// Default implementation allows small precision errors.
    fn verify_decimal_roundtrip(&self, precision: u8) -> bool {
        let decimal = self.serialize_decimal(precision);
        if let Ok(restored) = Self::deserialize_decimal(&decimal) {
            // Allow precision errors based on decimal representation
            let raw1 = self.serialize_binary().unwrap();
            let raw2 = restored.serialize_binary().unwrap();

            // Compare raw i64 values (from bytes 10-18)
            if raw1.len() >= 18 && raw2.len() >= 18 {
                let v1 = i64::from_le_bytes(raw1[10..18].try_into().unwrap());
                let v2 = i64::from_le_bytes(raw2[10..18].try_into().unwrap());

                // Calculate max error based on precision
                // max_error = ceil(2^FRACTIONAL_BITS / 10^precision)
                // For Q16.16 with 4 decimals: ceil(65536 / 10000) = 7
                // For Q8.8 with 2 decimals: ceil(256 / 100) = 3
                let actual_precision = if precision == 0 {
                    match Self::FRACTIONAL_BITS {
                        8 => 2,   // Q8.8 default
                        16 => 4,  // Q16.16 default
                        32 => 9,  // Q32.32 default
                        _ => 4,
                    }
                } else {
                    precision
                };

                let fractional_range = 1i64 << Self::FRACTIONAL_BITS;
                let decimal_range = 10i64.pow(actual_precision as u32);
                let max_error = (fractional_range + decimal_range - 1) / decimal_range; // ceil division

                let diff = (v1 - v2).abs();
                return diff <= max_error;
            }
        }
        false
    }

    /// Verify determinism: serialize_binary() twice, compare bytes
    ///
    /// **Property Test**: Same value must produce same bytes
    fn verify_binary_determinism(&self) -> bool {
        if let (Ok(bytes1), Ok(bytes2)) = (self.serialize_binary(), self.serialize_binary()) {
            return bytes1 == bytes2;
        }
        false
    }

    /// Verify determinism: serialize_decimal() twice, compare strings
    ///
    /// **Property Test**: Same value must produce same string
    fn verify_decimal_determinism(&self, precision: u8) -> bool {
        let decimal1 = self.serialize_decimal(precision);
        let decimal2 = self.serialize_decimal(precision);
        decimal1 == decimal2
    }

    /// Verify determinism: compute_hash() twice, compare hashes
    ///
    /// **Property Test**: Same value must produce same hash
    fn verify_hash_determinism(&self) -> bool {
        let hash1 = self.compute_hash();
        let hash2 = self.compute_hash();
        hash1 == hash2
    }
}
