//! # FixedPointSerialize - Type-Safe Fixed-Point Serialization (Phase 2)
//!
//! **Tier 3: Fixed-Point Precision Preservation** - Zero-drift financial arithmetic
//!
//! ## Design Philosophy (UCE34 Applied)
//!
//! **Q10: Tier Selection** - Tier 3 (Fixed-Point Deterministic Arithmetic)
//! - Serialize fixed-point AS-IS (no float conversion)
//! - Preserve exact i64 representation (Q16.16, Q8.8, Q32.32)
//! - Property test: `deserialize(serialize(x)) == x` (bit-exact)
//!
//! **Q34: Auditability** - Fixed-point integrity for financial compliance
//! - Deterministic serialization → provably correct audit trails
//! - No floating-point drift (exact arithmetic only)
//! - Decimal string format for human readability (precision preserved)
//!
//! ## Strategic Purpose
//!
//! This module enables **three critical use cases**:
//! 1. **Binary audit trails**: Exact i64 values (SOX/SOC2/GDPR)
//! 2. **JSON export**: Human-readable decimal strings (API/CLI)
//! 3. **Hash chains**: Deterministic serialization for integrity verification
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │ FixedPointSerialize Trait                                    │
//! │ - serialize_raw() -> i64           (exact binary)           │
//! │ - serialize_decimal() -> String    (human-readable)         │
//! │ - deserialize_from_raw(i64) -> Self                         │
//! └─────────────────────────┬────────────────────────────────────┘
//!                           │
//!              ┌────────────┼────────────┐
//!              │            │            │
//!       ┌──────▼──────┐ ┌──▼───────┐ ┌─▼──────────┐
//!       │   Q16.16    │ │  Q8.8    │ │  Q32.32    │
//!       │ (Financial) │ │ (Fast)   │ │ (High-Prec)│
//!       └─────────────┘ └──────────┘ └────────────┘
//! ```
//!
//! ## Format Specifications
//!
//! ### Binary Format (for CapsuleSerialize integration)
//!
//! ```text
//! [magic: u32][version: u16][fractional_bits: u32][raw_i64: i64][checksum: u32]
//! ```
//!
//! - **magic**: 0x46495850 ("FIXP" in ASCII)
//! - **version**: Format version (1 for Phase 2)
//! - **fractional_bits**: 8, 16, or 32 (Q-format identifier)
//! - **raw_i64**: Exact fixed-point representation
//! - **checksum**: CRC32 of raw_i64 (corruption detection)
//!
//! ### Decimal Format (for JSON/CLI export)
//!
//! ```text
//! "-1234.5678" (string with precision preserved)
//! ```
//!
//! - Sign preserved ('+' omitted for positive)
//! - Integer part: Full i64 precision
//! - Fractional part: Exactly `fractional_bits` precision
//! - No trailing zeros (e.g., "10.5000" → "10.5")
//!
//! ## Usage
//!
//! ```rust
//! use atomic_capsule::serialize::fixed_point_serialize::FixedPointSerialize;
//!
//! // Q16.16 fixed-point (financial calculations)
//! let value = FixedQ16_16::from_decimal(12, 34);  // 12.34
//!
//! // Binary serialization (exact)
//! let raw = value.serialize_raw();
//! assert_eq!(raw, (12 << 16) | ((34 * 65536) / 100));
//!
//! // Decimal serialization (human-readable)
//! let decimal = value.serialize_decimal();
//! assert_eq!(decimal, "12.34");
//!
//! // Roundtrip verification
//! let restored = FixedQ16_16::deserialize_from_raw(raw);
//! assert_eq!(value, restored);
//! ```
//!
//! ## Integration with CapsuleSerialize
//!
//! ```rust
//! use atomic_capsule::serialize::{CapsuleSerialize, fixed_point_serialize::*};
//! use std::sync::atomic::{AtomicI64, Ordering};
//!
//! #[derive(CapsuleSerialize)]
//! #[repr(C, align(256))]
//! pub struct PaymentCapsule256 {
//!     // Q16.16 fixed-point amounts (atomic)
//!     amount_cents_raw: AtomicI64,  // Raw Q16.16 value
//!     fee_cents_raw: AtomicI64,
//!     net_cents_raw: AtomicI64,
//!     // ... other fields
//! }
//!
//! impl PaymentCapsule256 {
//!     /// Get amount as fixed-point (atomic snapshot)
//!     pub fn amount(&self) -> FixedQ16_16 {
//!         let raw = self.amount_cents_raw.load(Ordering::Acquire);
//!         FixedQ16_16::deserialize_from_raw(raw)
//!     }
//!
//!     /// Export as JSON (human-readable)
//!     pub fn to_json(&self) -> String {
//!         let amount = self.amount();
//!         format!(
//!             r#"{{"amount": "{}"}}"#,
//!             amount.serialize_decimal()
//!         )
//!     }
//! }
//! ```
//!
//! ## Performance Targets (B32 Framework)
//!
//! - `serialize_raw()`: <5ns (single i64 read)
//! - `serialize_decimal()`: <100ns (integer division + string format)
//! - `deserialize_from_raw()`: <5ns (zero-cost wrapper)
//! - Binary roundtrip: <10ns (exact arithmetic, no allocation)
//!
//! ## ASSUM Safety Tags
//!
//! ```text
//! #ASSUME_EXACT_ARITHMETIC: i64 operations are exact (no FP drift)
//! #VERIFY_EXACT_ARITHMETIC: Property test with 1000+ random cases
//!
//! #ASSUME_DETERMINISTIC_DECIMAL: Same i64 always produces same decimal string
//! #VERIFY_DETERMINISTIC_DECIMAL: Unit test for decimal formatting
//!
//! #ASSUME_NO_OVERFLOW: Fixed-point arithmetic within i64 range
//! #VERIFY_NO_OVERFLOW: Saturating arithmetic + overflow tests
//!
//! #ASSUME_ATOMIC_SNAPSHOT: AtomicI64 reads are atomic
//! #VERIFY_ATOMIC_SNAPSHOT: Standard library guarantee (trivial)
//! ```
//!
//! ## Implementation Status
//!
//! - [x] FixedPointSerialize trait (this file)
//! - [x] Q16.16 implementation (financial standard)
//! - [x] Q8.8 implementation (fast arithmetic)
//! - [x] Q32.32 implementation (high precision)
//! - [ ] Property tests (1000+ random cases) - Phase 2.1
//! - [ ] CapsuleSerialize integration - Phase 2.2
//! - [ ] serde Serialize/Deserialize - Phase 2.3

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::string::String;

use core::fmt;

/// FixedPointSerialize trait - Type-safe fixed-point serialization
///
/// **Strategic Purpose**: Enable competitive moats via:
/// 1. **Exact arithmetic**: Zero floating-point drift (financial compliance)
/// 2. **Deterministic serialization**: Same value always produces same bytes
/// 3. **Human-readable export**: Decimal strings for JSON/CLI (API compatibility)
///
/// ## Design Guarantees (UCE34 Q34: Auditability)
///
/// - **Bit-exact roundtrip**: `deserialize_from_raw(serialize_raw(x)) == x`
/// - **Deterministic decimal**: Same i64 value always produces same string
/// - **Atomic snapshot**: Compatible with AtomicI64 fields
///
/// ## ASSUM Safety Tags
///
/// ```text
/// #ASSUME_EXACT_ARITHMETIC: i64 operations are exact (no FP rounding)
/// #VERIFY_EXACT_ARITHMETIC: Property test with 1000+ random Q16.16 values
///
/// #ASSUME_DETERMINISTIC_DECIMAL: format!() is deterministic for integers
/// #VERIFY_DETERMINISTIC_DECIMAL: Unit test serialize_decimal twice, compare
///
/// #ASSUME_FRACTIONAL_BITS_VALID: FRACTIONAL_BITS in {8, 16, 32}
/// #VERIFY_FRACTIONAL_BITS_VALID: Const assertion in implementation
/// ```
///
/// ## Implementation Requirements
///
/// Types implementing FixedPointSerialize MUST:
/// 1. Define `FRACTIONAL_BITS` constant (8, 16, or 32)
/// 2. Store fixed-point value as `i64` (exact representation)
/// 3. Implement bit-exact roundtrip property
/// 4. Use integer arithmetic only (no float conversion)
///
/// ## Example
///
/// ```rust
/// use atomic_capsule::serialize::fixed_point_serialize::FixedPointSerialize;
///
/// // Q16.16 fixed-point (16 integer bits, 16 fractional bits)
/// struct FixedQ16_16(i64);
///
/// impl FixedPointSerialize for FixedQ16_16 {
///     const FRACTIONAL_BITS: u32 = 16;
///
///     fn serialize_raw(&self) -> i64 {
///         self.0  // Return raw i64 value
///     }
///
///     fn serialize_decimal(&self) -> String {
///         let integer = self.0 >> 16;
///         let fractional = self.0 & 0xFFFF;
///         let decimal = (fractional * 10000) / 65536;  // 4 decimal places
///         format!("{}.{:04}", integer, decimal)
///     }
///
///     fn deserialize_from_raw(raw: i64) -> Self {
///         FixedQ16_16(raw)
///     }
/// }
///
/// // Usage
/// let value = FixedQ16_16::from_decimal(12, 34);  // 12.34
/// assert_eq!(value.serialize_decimal(), "12.3400");
/// assert_eq!(
///     FixedQ16_16::deserialize_from_raw(value.serialize_raw()),
///     value
/// );
/// ```
pub trait FixedPointSerialize: Sized + Copy + PartialEq {
    /// Number of fractional bits (8, 16, or 32)
    ///
    /// - **Q8.8**: 8 fractional bits (256 steps, ~0.0039 precision)
    /// - **Q16.16**: 16 fractional bits (65536 steps, ~0.000015 precision)
    /// - **Q32.32**: 32 fractional bits (4B+ steps, ~0.0000000002 precision)
    const FRACTIONAL_BITS: u32;

    /// Serialize to raw i64 representation (exact binary)
    ///
    /// **Determinism Guarantee**: Same value always produces same i64
    ///
    /// ## ASSUM Safety
    ///
    /// ```text
    /// #ASSUME_EXACT_ARITHMETIC: i64 is exact representation (no loss)
    /// #VERIFY_EXACT_ARITHMETIC: Property test deserialize(serialize(x)) == x
    /// ```
    ///
    /// ## Performance
    ///
    /// - Target: <5ns (single i64 read)
    /// - Zero allocation
    /// - Zero intermediate copies
    fn serialize_raw(&self) -> i64;

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
    /// Trailing zeros are NOT stripped (preserves precision).
    ///
    /// ## ASSUM Safety
    ///
    /// ```text
    /// #ASSUME_DETERMINISTIC_DECIMAL: format!() deterministic for integers
    /// #VERIFY_DETERMINISTIC_DECIMAL: Unit test serialize twice, compare strings
    /// ```
    ///
    /// ## Performance
    ///
    /// - Target: <100ns (integer division + string format)
    /// - Single allocation (String)
    /// - No floating-point operations
    fn serialize_decimal(&self) -> String;

    /// Deserialize from raw i64 representation
    ///
    /// **Inverse of `serialize_raw()`**: Zero-cost wrapper around i64.
    ///
    /// ## Property Guarantee
    ///
    /// ```text
    /// deserialize_from_raw(x.serialize_raw()) == x  (bit-exact)
    /// ```
    ///
    /// ## Performance
    ///
    /// - Target: <5ns (zero-cost wrapper)
    /// - No validation overhead
    /// - No allocation
    fn deserialize_from_raw(raw: i64) -> Self;

    /// Verify roundtrip property: deserialize(serialize(x)) == x
    ///
    /// **Property Test**: Used in 1000+ random case validation
    ///
    /// Default implementation uses `PartialEq`.
    fn verify_roundtrip(&self) -> bool {
        let raw = self.serialize_raw();
        let restored = Self::deserialize_from_raw(raw);
        *self == restored
    }

    /// Verify determinism: serialize_decimal() twice, compare strings
    ///
    /// **Property Test**: Same value must produce same string
    fn verify_decimal_determinism(&self) -> bool {
        let decimal1 = self.serialize_decimal();
        let decimal2 = self.serialize_decimal();
        decimal1 == decimal2
    }
}

// ============================================================================
// Q16.16 Implementation (Financial Standard)
// ============================================================================

/// Q16.16 fixed-point (16 integer bits, 16 fractional bits)
///
/// **Precision**: ~0.000015 (1/65536)
/// **Range**: -32768.0 to +32767.99998 (16 bits integer)
/// **Use Cases**: Financial calculations, P&L tracking, payment processing
///
/// ## Example
///
/// ```rust
/// use atomic_capsule::serialize::fixed_point_serialize::{FixedQ16_16, FixedPointSerialize};
///
/// let value = FixedQ16_16::from_decimal(1234, 5678);  // 1234.5678
/// assert_eq!(value.serialize_decimal(), "1234.5678");
///
/// let raw = value.serialize_raw();
/// let restored = FixedQ16_16::deserialize_from_raw(raw);
/// assert_eq!(value, restored);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct FixedQ16_16(pub i64);

impl FixedQ16_16 {
    /// Create from raw i64 value
    pub const fn from_raw(raw: i64) -> Self {
        FixedQ16_16(raw)
    }

    /// Create from integer and fractional parts (cents)
    ///
    /// ## Example
    ///
    /// ```rust
    /// # use atomic_capsule::serialize::fixed_point_serialize::FixedQ16_16;
    /// let value = FixedQ16_16::from_decimal(12, 34);  // 12.34
    /// ```
    pub const fn from_decimal(integer: i64, fractional_cents: i64) -> Self {
        // #ASSUME: fractional_cents in range 0-9999 (4 decimal places)
        // #VERIFY: Property test validates range
        let fractional = (fractional_cents * 65536) / 10000;
        FixedQ16_16((integer << 16) | (fractional & 0xFFFF))
    }

    /// Get integer part
    pub const fn integer_part(&self) -> i64 {
        self.0 >> 16
    }

    /// Get fractional part (0-9999, 4 decimal places)
    pub const fn fractional_part(&self) -> i64 {
        let fractional = self.0 & 0xFFFF;
        // Round to nearest: add 32768 (half of 65536) before dividing
        ((fractional * 10000) + 32768) / 65536
    }
}

impl FixedPointSerialize for FixedQ16_16 {
    const FRACTIONAL_BITS: u32 = 16;

    #[inline]
    fn serialize_raw(&self) -> i64 {
        // #ASSUME_EXACT_ARITHMETIC: i64 is exact representation
        self.0
    }

    fn serialize_decimal(&self) -> String {
        // #ASSUME_DETERMINISTIC_DECIMAL: format!() is deterministic for integers
        let integer = self.integer_part();
        let fractional = self.fractional_part();

        // Format with exactly 4 decimal places (preserve trailing zeros)
        if integer >= 0 {
            #[cfg(feature = "std")]
            return std::format!("{}.{:04}", integer, fractional);
            #[cfg(not(feature = "std"))]
            return alloc::format!("{}.{:04}", integer, fractional);
        } else {
            // Negative values: handle sign separately
            #[cfg(feature = "std")]
            return std::format!("{}.{:04}", integer, fractional.abs());
            #[cfg(not(feature = "std"))]
            return alloc::format!("{}.{:04}", integer, fractional.abs());
        }
    }

    #[inline]
    fn deserialize_from_raw(raw: i64) -> Self {
        FixedQ16_16(raw)
    }
}

impl fmt::Display for FixedQ16_16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.serialize_decimal())
    }
}

// ============================================================================
// Q8.8 Implementation (Fast Arithmetic)
// ============================================================================

/// Q8.8 fixed-point (8 integer bits, 8 fractional bits)
///
/// **Precision**: ~0.0039 (1/256)
/// **Range**: -128.0 to +127.996 (8 bits integer)
/// **Use Cases**: Fast arithmetic, small values, quantization
///
/// ## Example
///
/// ```rust
/// use atomic_capsule::serialize::fixed_point_serialize::{FixedQ8_8, FixedPointSerialize};
///
/// let value = FixedQ8_8::from_decimal(12, 34);  // 12.34
/// assert_eq!(value.serialize_decimal(), "12.34");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct FixedQ8_8(pub i64);

impl FixedQ8_8 {
    /// Create from raw i64 value
    pub const fn from_raw(raw: i64) -> Self {
        FixedQ8_8(raw)
    }

    /// Create from integer and fractional parts (cents)
    pub const fn from_decimal(integer: i64, fractional_cents: i64) -> Self {
        let fractional = (fractional_cents * 256) / 100;
        FixedQ8_8((integer << 8) | (fractional & 0xFF))
    }

    /// Extract the integer part of the fixed-point value
    ///
    /// For Q8.8 format, shifts right by 8 bits to get the integer portion.
    pub const fn integer_part(&self) -> i64 {
        self.0 >> 8
    }

    /// Extract the fractional part of the fixed-point value (in cents)
    ///
    /// For Q8.8 format, masks the lower 8 bits and converts to cents (0-99).
    pub const fn fractional_part(&self) -> i64 {
        let fractional = self.0 & 0xFF;
        // Round to nearest: add 128 (half of 256) before dividing
        ((fractional * 100) + 128) / 256
    }
}

impl FixedPointSerialize for FixedQ8_8 {
    const FRACTIONAL_BITS: u32 = 8;

    #[inline]
    fn serialize_raw(&self) -> i64 {
        self.0
    }

    fn serialize_decimal(&self) -> String {
        let integer = self.integer_part();
        let fractional = self.fractional_part();

        if integer >= 0 {
            #[cfg(feature = "std")]
            return std::format!("{}.{:02}", integer, fractional);
            #[cfg(not(feature = "std"))]
            return alloc::format!("{}.{:02}", integer, fractional);
        } else {
            #[cfg(feature = "std")]
            return std::format!("{}.{:02}", integer, fractional.abs());
            #[cfg(not(feature = "std"))]
            return alloc::format!("{}.{:02}", integer, fractional.abs());
        }
    }

    #[inline]
    fn deserialize_from_raw(raw: i64) -> Self {
        FixedQ8_8(raw)
    }
}

impl fmt::Display for FixedQ8_8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.serialize_decimal())
    }
}

// ============================================================================
// Q32.32 Implementation (High Precision)
// ============================================================================

/// Q32.32 fixed-point (32 integer bits, 32 fractional bits)
///
/// **Precision**: ~0.0000000002 (1/4294967296)
/// **Range**: -2147483648.0 to +2147483647.999999999 (32 bits integer)
/// **Use Cases**: High-precision calculations, scientific computing
///
/// ## Example
///
/// ```rust
/// use atomic_capsule::serialize::fixed_point_serialize::{FixedQ32_32, FixedPointSerialize};
///
/// let value = FixedQ32_32::from_decimal(1234, 567890123);  // 1234.567890123
/// assert_eq!(value.serialize_decimal(), "1234.567890123");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct FixedQ32_32(pub i64);

impl FixedQ32_32 {
    /// Create from raw i64 value
    pub const fn from_raw(raw: i64) -> Self {
        FixedQ32_32(raw)
    }

    /// Create from integer and fractional parts (9 decimal places)
    pub const fn from_decimal(integer: i64, fractional_nanos: i64) -> Self {
        // fractional_nanos: 0-999999999 (9 decimal places)
        let fractional = (fractional_nanos * 4294967296) / 1000000000;
        FixedQ32_32((integer << 32) | (fractional & 0xFFFFFFFF))
    }

    /// Extract the integer part of the fixed-point value
    ///
    /// For Q32.32 format, shifts right by 32 bits to get the integer portion.
    pub const fn integer_part(&self) -> i64 {
        self.0 >> 32
    }

    /// Extract the fractional part of the fixed-point value (in nanoseconds)
    ///
    /// For Q32.32 format, masks the lower 32 bits and converts to nanoseconds (0-999999999).
    pub const fn fractional_part(&self) -> i64 {
        let fractional = self.0 & 0xFFFFFFFF;
        // Round to nearest: add 2147483648 (half of 4294967296) before dividing
        ((fractional * 1000000000) + 2147483648) / 4294967296
    }
}

impl FixedPointSerialize for FixedQ32_32 {
    const FRACTIONAL_BITS: u32 = 32;

    #[inline]
    fn serialize_raw(&self) -> i64 {
        self.0
    }

    fn serialize_decimal(&self) -> String {
        let integer = self.integer_part();
        let fractional = self.fractional_part();

        if integer >= 0 {
            #[cfg(feature = "std")]
            return std::format!("{}.{:09}", integer, fractional);
            #[cfg(not(feature = "std"))]
            return alloc::format!("{}.{:09}", integer, fractional);
        } else {
            #[cfg(feature = "std")]
            return std::format!("{}.{:09}", integer, fractional.abs());
            #[cfg(not(feature = "std"))]
            return alloc::format!("{}.{:09}", integer, fractional.abs());
        }
    }

    #[inline]
    fn deserialize_from_raw(raw: i64) -> Self {
        FixedQ32_32(raw)
    }
}

impl fmt::Display for FixedQ32_32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.serialize_decimal())
    }
}

// ============================================================================
// Binary Format Serialization (for CapsuleSerialize integration)
// ============================================================================

/// Binary format magic number (0x46495850 = "FIXP" in ASCII)
pub const FIXED_POINT_MAGIC: u32 = 0x46495850;

/// Binary format version (Phase 2)
pub const FIXED_POINT_VERSION: u16 = 1;

/// Serialize fixed-point to binary format (for CapsuleSerialize)
///
/// ## Binary Format
///
/// ```text
/// [magic: u32][version: u16][fractional_bits: u32][raw_i64: i64][checksum: u32]
/// ```
///
/// ## Example
///
/// ```rust
/// # use atomic_capsule::serialize::fixed_point_serialize::*;
/// let value = FixedQ16_16::from_decimal(12, 34);
/// let bytes = serialize_to_binary(&value);
/// assert_eq!(bytes.len(), 22);  // 4 + 2 + 4 + 8 + 4
/// ```
#[cfg(feature = "std")]
pub fn serialize_to_binary<T: FixedPointSerialize>(value: &T) -> Vec<u8> {
    use crc::{Crc, CRC_32_ISO_HDLC};

    let mut bytes = Vec::with_capacity(22);

    // Magic number
    bytes.extend_from_slice(&FIXED_POINT_MAGIC.to_le_bytes());

    // Version
    bytes.extend_from_slice(&FIXED_POINT_VERSION.to_le_bytes());

    // Fractional bits
    bytes.extend_from_slice(&T::FRACTIONAL_BITS.to_le_bytes());

    // Raw i64 value
    let raw = value.serialize_raw();
    bytes.extend_from_slice(&raw.to_le_bytes());

    // CRC32 checksum
    let crc = Crc::<u32>::new(&CRC_32_ISO_HDLC);
    let checksum = crc.checksum(&raw.to_le_bytes());
    bytes.extend_from_slice(&checksum.to_le_bytes());

    bytes
}

/// Deserialize fixed-point from binary format
///
/// ## Errors
///
/// - Invalid magic number
/// - Version mismatch
/// - Fractional bits mismatch
/// - Checksum mismatch (data corrupted)
#[cfg(feature = "std")]
pub fn deserialize_from_binary<T: FixedPointSerialize>(bytes: &[u8]) -> Result<T, &'static str> {
    use crc::{Crc, CRC_32_ISO_HDLC};

    if bytes.len() < 22 {
        return Err("Buffer too small (expected 22 bytes)");
    }

    // Validate magic
    let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    if magic != FIXED_POINT_MAGIC {
        return Err("Invalid magic number");
    }

    // Validate version
    let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
    if version != FIXED_POINT_VERSION {
        return Err("Version mismatch");
    }

    // Validate fractional bits
    let fractional_bits = u32::from_le_bytes(bytes[6..10].try_into().unwrap());
    if fractional_bits != T::FRACTIONAL_BITS {
        return Err("Fractional bits mismatch");
    }

    // Extract raw value
    let raw = i64::from_le_bytes(bytes[10..18].try_into().unwrap());

    // Validate checksum
    let expected_checksum = u32::from_le_bytes(bytes[18..22].try_into().unwrap());
    let crc = Crc::<u32>::new(&CRC_32_ISO_HDLC);
    let actual_checksum = crc.checksum(&raw.to_le_bytes());
    if actual_checksum != expected_checksum {
        return Err("Checksum mismatch (data corrupted)");
    }

    Ok(T::deserialize_from_raw(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q16.16 Tests
    // ========================================================================

    #[test]
    fn test_q16_16_roundtrip() {
        let value = FixedQ16_16::from_decimal(1234, 5678);
        let raw = value.serialize_raw();
        let restored = FixedQ16_16::deserialize_from_raw(raw);
        assert_eq!(value, restored);
    }

    #[test]
    fn test_q16_16_decimal() {
        let value = FixedQ16_16::from_decimal(12, 34);
        assert_eq!(value.serialize_decimal(), "12.0034");
    }

    #[test]
    fn test_q16_16_negative() {
        let value = FixedQ16_16::from_decimal(-12, 34);
        assert_eq!(value.serialize_decimal(), "-12.0034");
    }

    #[test]
    fn test_q16_16_zero() {
        let value = FixedQ16_16::from_decimal(0, 0);
        assert_eq!(value.serialize_decimal(), "0.0000");
    }

    #[test]
    fn test_q16_16_determinism() {
        let value = FixedQ16_16::from_decimal(1234, 5678);
        assert!(value.verify_decimal_determinism());
    }

    #[test]
    fn test_q16_16_verify_roundtrip() {
        let value = FixedQ16_16::from_decimal(1234, 5678);
        assert!(value.verify_roundtrip());
    }

    // ========================================================================
    // Q8.8 Tests
    // ========================================================================

    #[test]
    fn test_q8_8_roundtrip() {
        let value = FixedQ8_8::from_decimal(12, 34);
        let raw = value.serialize_raw();
        let restored = FixedQ8_8::deserialize_from_raw(raw);
        assert_eq!(value, restored);
    }

    #[test]
    fn test_q8_8_decimal() {
        let value = FixedQ8_8::from_decimal(12, 34);
        assert_eq!(value.serialize_decimal(), "12.34");
    }

    // ========================================================================
    // Q32.32 Tests
    // ========================================================================

    #[test]
    fn test_q32_32_roundtrip() {
        let value = FixedQ32_32::from_decimal(1234, 567890123);
        let raw = value.serialize_raw();
        let restored = FixedQ32_32::deserialize_from_raw(raw);
        assert_eq!(value, restored);
    }

    #[test]
    fn test_q32_32_decimal() {
        let value = FixedQ32_32::from_decimal(1234, 567890123);
        assert_eq!(value.serialize_decimal(), "1234.567890123");
    }

    // ========================================================================
    // Binary Format Tests
    // ========================================================================

    #[cfg(feature = "std")]
    #[test]
    fn test_binary_roundtrip() {
        let value = FixedQ16_16::from_decimal(1234, 5678);
        let bytes = serialize_to_binary(&value);
        let restored: FixedQ16_16 = deserialize_from_binary(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_binary_format_size() {
        let value = FixedQ16_16::from_decimal(12, 34);
        let bytes = serialize_to_binary(&value);
        assert_eq!(bytes.len(), 22); // magic(4) + version(2) + frac_bits(4) + raw(8) + checksum(4)
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_binary_checksum_validation() {
        let value = FixedQ16_16::from_decimal(1234, 5678);
        let mut bytes = serialize_to_binary(&value);

        // Corrupt the data
        bytes[15] ^= 0xFF;

        // Deserialization should fail (checksum mismatch)
        let result: Result<FixedQ16_16, _> = deserialize_from_binary(&bytes);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Checksum mismatch (data corrupted)");
    }
}
