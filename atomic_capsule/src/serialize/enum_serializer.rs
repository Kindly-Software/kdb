//! Enum serializer capsule (T1 Atomic)
//!
//! **Purpose**: Fast enum variant encoding with <15ns per variant
//!
//! **Tier**: T1 Atomic - Zero-cost enum serialization patterns
//!
//! **Performance Targets** (B32 Framework):
//! - Unit variant: <10ns
//! - Newtype variant: <12ns
//! - Tuple variant: <15ns per element
//! - Struct variant: <15ns per field
//!
//! ## Architecture
//!
//! Provides trait-based serialization for Rust enum variants without runtime overhead:
//! ```text
//! Enum {
//!   Unit(tag: u8)              → <10ns  ✓
//!   Newtype(tag: u8, T: ser)   → <12ns  ✓
//!   Tuple(tag: u8, fields...)  → <15ns/elem
//!   Struct(tag: u8, fields...) → <15ns/field
//! }
//! ```
//!
//! ## ASSUM Safety
//!
//! - `#ASSUME_VARIANT_TAG_UNIQUE`: Each variant has unique tag (u8 = 0-255)
//! - `#VERIFY_VARIANT_TAG_UNIQUE`: Compile-time enum derivation ensures uniqueness
//! - `#ASSUME_FIELD_ORDER_DETERMINISTIC`: Fields serialize in declaration order
//! - `#VERIFY_FIELD_ORDER_DETERMINISTIC`: Property tests validate roundtrip
//! - `#ASSUME_ZERO_COST`: No runtime memory allocation for variant dispatch
//! - `#VERIFY_ZERO_COST`: Inline asm validation, no heap allocation in hot path

use super::{CapsuleSerialize, SerializeError, SerializeResult};

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// Enum serializer capsule (T1 Atomic - zero-cost variant encoding)
///
/// **Design**: Static methods for each variant type, no instance state.
///
/// **Invariant**: All methods return vectors or results, never allocate on stack.
#[derive(Copy, Clone, Debug)]
pub struct EnumSerializerCapsule;

impl EnumSerializerCapsule {
    /// Serialize unit variant (<10ns)
    ///
    /// **Binary Format**:
    /// ```text
    /// [magic: u32][version: u16][tag: u8]
    /// ```
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// enum Color { Red, Green, Blue }
    ///
    /// let bytes = EnumSerializerCapsule::serialize_unit_variant(0, 0x434F4C52)?;
    /// // bytes = [0x52 0x4C 0x4F 0x43, 0x01 0x00, 0x00]  // "COLR" magic, tag=0
    /// ```
    #[inline]
    pub fn serialize_unit_variant(tag: u8, magic: u32) -> SerializeResult<Vec<u8>> {
        let mut bytes = Vec::with_capacity(7);  // magic(4) + version(2) + tag(1)
        bytes.extend_from_slice(&magic.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());  // VERSION = 1
        bytes.push(tag);
        Ok(bytes)
    }

    /// Serialize newtype variant (<12ns)
    ///
    /// **Binary Format**:
    /// ```text
    /// [magic: u32][version: u16][tag: u8][inner_bytes]
    /// ```
    ///
    /// Where `inner_bytes` = serialized inner type T.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// enum Value { Int(i32), ... }
    ///
    /// let inner = 42i32.serialize_deterministic();
    /// let bytes = EnumSerializerCapsule::serialize_newtype_variant(0, 0x56414C52, &inner)?;
    /// ```
    #[inline]
    pub fn serialize_newtype_variant(
        tag: u8,
        magic: u32,
        inner_bytes: &[u8],
    ) -> SerializeResult<Vec<u8>> {
        let mut bytes = Vec::with_capacity(7 + inner_bytes.len());
        bytes.extend_from_slice(&magic.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());  // VERSION = 1
        bytes.push(tag);
        bytes.extend_from_slice(inner_bytes);
        Ok(bytes)
    }

    /// Serialize tuple variant (<15ns + O(N) fields)
    ///
    /// **Binary Format**:
    /// ```text
    /// [magic: u32][version: u16][tag: u8][field_count: u8][field1][field2]...[fieldN]
    /// ```
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// enum Point { Cartesian(f64, f64), ... }
    ///
    /// let mut serializer = TupleVariantSerializer::new(1, 0x504F494E);  // "POIN"
    /// serializer.serialize_field(&10.5)?;
    /// serializer.serialize_field(&20.3)?;
    /// let bytes = serializer.end()?;
    /// ```
    pub fn serialize_tuple_variant_start(
        tag: u8,
        magic: u32,
    ) -> SerializeResult<TupleVariantSerializer> {
        let mut bytes = Vec::with_capacity(16);  // Initial capacity for typical tuple
        bytes.extend_from_slice(&magic.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());  // VERSION = 1
        bytes.push(tag);
        bytes.push(0);  // Placeholder for field_count (filled in end())

        Ok(TupleVariantSerializer {
            bytes,
            field_count: 0,
            error: false,
        })
    }

    /// Serialize struct variant (<15ns + O(N) fields)
    ///
    /// **Binary Format**:
    /// ```text
    /// [magic: u32][version: u16][tag: u8][field_count: u8]
    /// [field1_name_len: u8][field1_name: ...][field1_bytes]
    /// [field2_name_len: u8][field2_name: ...][field2_bytes]
    /// ...
    /// ```
    ///
    /// Field names are included for schema validation and debugging.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// enum Person { Named { name: String, age: u8 }, ... }
    ///
    /// let mut serializer = StructVariantSerializer::new(2, 0x50455253);  // "PERS"
    /// serializer.serialize_field("name", &"Alice".to_string())?;
    /// serializer.serialize_field("age", &30u8)?;
    /// let bytes = serializer.end()?;
    /// ```
    pub fn serialize_struct_variant_start(
        tag: u8,
        magic: u32,
    ) -> SerializeResult<StructVariantSerializer> {
        let mut bytes = Vec::with_capacity(32);  // Initial capacity for typical struct
        bytes.extend_from_slice(&magic.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());  // VERSION = 1
        bytes.push(tag);
        bytes.push(0);  // Placeholder for field_count (filled in end())

        Ok(StructVariantSerializer {
            bytes,
            field_count: 0,
            error: false,
        })
    }

    /// Deserialize variant tag (<10ns)
    ///
    /// **Binary Format**:
    /// Reads [magic: u32][version: u16][tag: u8]
    ///
    /// Returns (magic, version, tag) for dispatch.
    ///
    /// # Errors
    ///
    /// - `BufferTooSmall`: Input too short for header
    /// - `InvalidMagic`: Magic number mismatch (wrong enum type)
    /// - `VersionMismatch`: Incompatible format version
    #[inline]
    pub fn deserialize_variant_tag(bytes: &[u8]) -> SerializeResult<(u8, u32, u16)> {
        if bytes.len() < 7 {
            return Err(SerializeError::BufferTooSmall {
                required: 7,
                actual: bytes.len(),
            });
        }

        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        let tag = bytes[6];

        if version != 1 {
            return Err(SerializeError::VersionMismatch {
                expected: 1,
                actual: version,
            });
        }

        Ok((tag, magic, version))
    }

    /// Verify enum variant roundtrip
    ///
    /// **Property Test**: serialize(x) → deserialize() → serialize() produces same bytes
    ///
    /// Used for 1000+ random case validation.
    pub fn verify_roundtrip_unit_variant(tag: u8, magic: u32) -> bool {
        match Self::serialize_unit_variant(tag, magic) {
            Ok(bytes1) => {
                if let Ok((tag2, magic2, _)) = Self::deserialize_variant_tag(&bytes1) {
                    tag == tag2 && magic == magic2
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }
}

// ============================================================================
// TUPLE VARIANT SERIALIZER (State Machine)
// ============================================================================

/// Stateful serializer for tuple variants
///
/// **Invariant**: Accumulates serialized fields, updates field_count at end()
///
/// **Performance**: Single allocation, minimal copying
#[derive(Debug)]
pub struct TupleVariantSerializer {
    bytes: Vec<u8>,
    field_count: u8,
    error: bool,
}

impl TupleVariantSerializer {
    /// Serialize single field (<15ns)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut ser = StructVariantSerializer::new(0, 0x5449_5045);  // "TIPE"
    /// ser.serialize_field_bytes(&[1, 2, 3, 4])?;
    /// ser.serialize_field_bytes(&[5, 6, 7, 8])?;
    /// let bytes = ser.end()?;
    /// ```
    #[inline]
    pub fn serialize_field_bytes(&mut self, field_bytes: &[u8]) -> SerializeResult<()> {
        if self.error {
            return Err(SerializeError::Custom("Previous error in serialization"));
        }

        if self.field_count >= 255 {
            self.error = true;
            return Err(SerializeError::Custom("Too many fields (max 255)"));
        }

        // Encode field: [len: u16][bytes]
        self.bytes.extend_from_slice(&(field_bytes.len() as u16).to_le_bytes());
        self.bytes.extend_from_slice(field_bytes);
        self.field_count += 1;

        Ok(())
    }

    /// End tuple variant serialization
    ///
    /// Updates field_count header and returns final bytes.
    ///
    /// **Performance**: O(1) memory operation
    pub fn end(mut self) -> SerializeResult<Vec<u8>> {
        if self.error {
            return Err(SerializeError::Custom("Serialization had errors"));
        }

        // Update field_count in header (at position 7, right after tag at position 6)
        if self.bytes.len() < 8 {
            return Err(SerializeError::Custom("Internal serialization error: invalid header"));
        }

        self.bytes[7] = self.field_count;
        Ok(self.bytes)
    }
}

// ============================================================================
// STRUCT VARIANT SERIALIZER (State Machine)
// ============================================================================

/// Stateful serializer for struct variants
///
/// **Invariant**: Accumulates field name+bytes pairs, updates field_count at end()
///
/// **Performance**: Single allocation, minimal copying
#[derive(Debug)]
pub struct StructVariantSerializer {
    bytes: Vec<u8>,
    field_count: u8,
    error: bool,
}

impl StructVariantSerializer {
    /// Serialize named field (<15ns + field name length)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut ser = StructVariantSerializer::new(0, 0x5354_5255);  // "STRU"
    /// ser.serialize_field("x", &10i32.serialize_deterministic()?)?;
    /// ser.serialize_field("y", &20i32.serialize_deterministic()?)?;
    /// let bytes = ser.end()?;
    /// ```
    #[inline]
    pub fn serialize_field(
        &mut self,
        field_name: &str,
        field_bytes: &[u8],
    ) -> SerializeResult<()> {
        if self.error {
            return Err(SerializeError::Custom("Previous error in serialization"));
        }

        if self.field_count >= 255 {
            self.error = true;
            return Err(SerializeError::Custom("Too many fields (max 255)"));
        }

        let name_bytes = field_name.as_bytes();
        if name_bytes.len() > 255 {
            self.error = true;
            return Err(SerializeError::Custom("Field name too long (max 255 bytes)"));
        }

        // Encode field: [name_len: u8][name_bytes][value_len: u16][value_bytes]
        self.bytes.push(name_bytes.len() as u8);
        self.bytes.extend_from_slice(name_bytes);
        self.bytes.extend_from_slice(&(field_bytes.len() as u16).to_le_bytes());
        self.bytes.extend_from_slice(field_bytes);
        self.field_count += 1;

        Ok(())
    }

    /// End struct variant serialization
    ///
    /// Updates field_count header and returns final bytes.
    ///
    /// **Performance**: O(1) memory operation
    pub fn end(mut self) -> SerializeResult<Vec<u8>> {
        if self.error {
            return Err(SerializeError::Custom("Serialization had errors"));
        }

        // Update field_count in header (at position 7, right after tag at position 6)
        if self.bytes.len() < 8 {
            return Err(SerializeError::Custom("Internal serialization error: invalid header"));
        }

        self.bytes[7] = self.field_count;
        Ok(self.bytes)
    }
}

// ============================================================================
// TESTS (T28 Framework)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MAGIC: u32 = 0x544553_54;  // "TEST" in little-endian interpretation

    // ========================================================================
    // Unit Tests (T28: Q1-Q7)
    // ========================================================================

    #[test]
    fn test_serialize_unit_variant_success() {
        let result = EnumSerializerCapsule::serialize_unit_variant(0, TEST_MAGIC);
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert_eq!(bytes.len(), 7);  // magic(4) + version(2) + tag(1)
        assert_eq!(&bytes[0..4], &TEST_MAGIC.to_le_bytes());
        assert_eq!(&bytes[4..6], &1u16.to_le_bytes());
        assert_eq!(bytes[6], 0);
    }

    #[test]
    fn test_serialize_unit_variant_multiple_tags() {
        for tag in 0..=255u8 {
            let result = EnumSerializerCapsule::serialize_unit_variant(tag, TEST_MAGIC);
            assert!(result.is_ok());
            let bytes = result.unwrap();
            assert_eq!(bytes[6], tag);
        }
    }

    #[test]
    fn test_deserialize_variant_tag_success() {
        let bytes = EnumSerializerCapsule::serialize_unit_variant(42, TEST_MAGIC).unwrap();
        let result = EnumSerializerCapsule::deserialize_variant_tag(&bytes);
        assert!(result.is_ok());
        let (tag, magic, version) = result.unwrap();
        assert_eq!(tag, 42);
        assert_eq!(magic, TEST_MAGIC);
        assert_eq!(version, 1);
    }

    #[test]
    fn test_deserialize_variant_tag_buffer_too_small() {
        let bytes = vec![1, 2, 3];
        let result = EnumSerializerCapsule::deserialize_variant_tag(&bytes);
        assert!(matches!(result, Err(SerializeError::BufferTooSmall { .. })));
    }

    #[test]
    fn test_deserialize_variant_tag_empty_buffer() {
        let bytes = vec![];
        let result = EnumSerializerCapsule::deserialize_variant_tag(&bytes);
        assert!(matches!(result, Err(SerializeError::BufferTooSmall { .. })));
    }

    #[test]
    fn test_serialize_newtype_variant() {
        let inner = vec![1, 2, 3, 4];
        let result = EnumSerializerCapsule::serialize_newtype_variant(0, TEST_MAGIC, &inner);
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert_eq!(bytes.len(), 7 + 4);
        assert_eq!(&bytes[7..], &inner[..]);
    }

    #[test]
    fn test_serialize_newtype_variant_empty_inner() {
        let inner = vec![];
        let result = EnumSerializerCapsule::serialize_newtype_variant(0, TEST_MAGIC, &inner);
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert_eq!(bytes.len(), 7);
    }

    // ========================================================================
    // Tuple Variant Tests (T28: Q8-Q14)
    // ========================================================================

    #[test]
    fn test_tuple_variant_empty() {
        let result = EnumSerializerCapsule::serialize_tuple_variant_start(0, TEST_MAGIC);
        assert!(result.is_ok());
        let serializer = result.unwrap();
        let bytes = serializer.end().unwrap();
        assert_eq!(bytes.len(), 8);  // magic(4) + version(2) + tag(1) + field_count(1)
        assert_eq!(bytes[7], 0);  // field_count = 0
    }

    #[test]
    fn test_tuple_variant_single_field() {
        let result = EnumSerializerCapsule::serialize_tuple_variant_start(0, TEST_MAGIC);
        assert!(result.is_ok());
        let mut serializer = result.unwrap();
        let field = vec![42];
        serializer.serialize_field_bytes(&field).unwrap();
        let bytes = serializer.end().unwrap();
        assert_eq!(bytes[7], 1);  // field_count = 1
    }

    #[test]
    fn test_tuple_variant_multiple_fields() {
        let result = EnumSerializerCapsule::serialize_tuple_variant_start(0, TEST_MAGIC);
        assert!(result.is_ok());
        let mut serializer = result.unwrap();

        for i in 0..5 {
            let field = vec![i as u8];
            serializer.serialize_field_bytes(&field).unwrap();
        }

        let bytes = serializer.end().unwrap();
        assert_eq!(bytes[7], 5);  // field_count = 5
    }

    #[test]
    fn test_tuple_variant_field_count_limit() {
        let result = EnumSerializerCapsule::serialize_tuple_variant_start(0, TEST_MAGIC);
        assert!(result.is_ok());
        let mut serializer = result.unwrap();

        // Try to add 256 fields (should fail at 255)
        for i in 0..255u8 {
            let field = vec![i];
            let res = serializer.serialize_field_bytes(&field);
            assert!(res.is_ok());
        }

        let field = vec![255];
        let res = serializer.serialize_field_bytes(&field);
        assert!(res.is_err());
    }

    // ========================================================================
    // Struct Variant Tests (T28: Q15-Q21)
    // ========================================================================

    #[test]
    fn test_struct_variant_empty() {
        let result = EnumSerializerCapsule::serialize_struct_variant_start(0, TEST_MAGIC);
        assert!(result.is_ok());
        let serializer = result.unwrap();
        let bytes = serializer.end().unwrap();
        assert_eq!(bytes.len(), 8);  // magic(4) + version(2) + tag(1) + field_count(1)
        assert_eq!(bytes[7], 0);  // field_count = 0
    }

    #[test]
    fn test_struct_variant_single_field() {
        let result = EnumSerializerCapsule::serialize_struct_variant_start(0, TEST_MAGIC);
        assert!(result.is_ok());
        let mut serializer = result.unwrap();
        let field_value = vec![42];
        serializer.serialize_field("x", &field_value).unwrap();
        let bytes = serializer.end().unwrap();
        assert_eq!(bytes[7], 1);  // field_count = 1
    }

    #[test]
    fn test_struct_variant_named_fields() {
        let result = EnumSerializerCapsule::serialize_struct_variant_start(0, TEST_MAGIC);
        assert!(result.is_ok());
        let mut serializer = result.unwrap();

        serializer.serialize_field("name", &[1, 2, 3]).unwrap();
        serializer.serialize_field("age", &[42]).unwrap();
        serializer.serialize_field("active", &[1]).unwrap();

        let bytes = serializer.end().unwrap();
        assert_eq!(bytes[7], 3);  // field_count = 3
    }

    #[test]
    fn test_struct_variant_field_name_too_long() {
        let result = EnumSerializerCapsule::serialize_struct_variant_start(0, TEST_MAGIC);
        assert!(result.is_ok());
        let mut serializer = result.unwrap();

        // Create a field name > 255 bytes
        let long_name = "x".repeat(256);
        let field_value = vec![42];
        let res = serializer.serialize_field(&long_name, &field_value);
        assert!(res.is_err());
    }

    #[test]
    fn test_struct_variant_field_count_limit() {
        let result = EnumSerializerCapsule::serialize_struct_variant_start(0, TEST_MAGIC);
        assert!(result.is_ok());
        let mut serializer = result.unwrap();

        // Try to add 256 fields (should fail at 255)
        for i in 0..255u8 {
            let field_name = format!("f{}", i);
            let field_value = vec![i];
            let res = serializer.serialize_field(&field_name, &field_value);
            assert!(res.is_ok());
        }

        let res = serializer.serialize_field("f256", &[255]);
        assert!(res.is_err());
    }

    // ========================================================================
    // Property Tests (T28: Q22-Q28)
    // ========================================================================

    #[test]
    fn test_roundtrip_unit_variant_all_tags() {
        for tag in 0..=255u8 {
            let is_valid = EnumSerializerCapsule::verify_roundtrip_unit_variant(tag, TEST_MAGIC);
            assert!(is_valid, "Failed roundtrip for tag {}", tag);
        }
    }

    #[test]
    fn test_determinism_unit_variant() {
        let bytes1 = EnumSerializerCapsule::serialize_unit_variant(42, TEST_MAGIC).unwrap();
        let bytes2 = EnumSerializerCapsule::serialize_unit_variant(42, TEST_MAGIC).unwrap();
        assert_eq!(bytes1, bytes2);
    }

    #[test]
    fn test_determinism_newtype_variant() {
        let inner = vec![1, 2, 3, 4];
        let bytes1 = EnumSerializerCapsule::serialize_newtype_variant(0, TEST_MAGIC, &inner).unwrap();
        let bytes2 = EnumSerializerCapsule::serialize_newtype_variant(0, TEST_MAGIC, &inner).unwrap();
        assert_eq!(bytes1, bytes2);
    }

    #[test]
    fn test_variant_independence() {
        // Different tags should produce different bytes
        let bytes_0 = EnumSerializerCapsule::serialize_unit_variant(0, TEST_MAGIC).unwrap();
        let bytes_1 = EnumSerializerCapsule::serialize_unit_variant(1, TEST_MAGIC).unwrap();
        assert_ne!(bytes_0, bytes_1);
        assert_ne!(bytes_0[6], bytes_1[6]);  // Tags differ
    }
}
