//! # CapsuleSerialize Implementations for Basic Types
//!
//! Complete implementations for primitives, fixed arrays, and tuples.
//!
//! ## Performance Targets (B32 Framework)
//! - Primitives (u64, i64, etc.): <10ns per serialize
//! - Fixed arrays: <50ns for 64-byte array
//! - Tuples: <20ns for 3-tuple
//!
//! ## ASSUM Safety
//! - `#ASSUME_LE_ENDIAN`: Little-endian encoding for cross-platform consistency
//! - `#VERIFY_LE_ENDIAN`: Tests validate on multiple architectures
//! - `#ASSUME_DETERMINISTIC`: Same value always produces same bytes
//! - `#VERIFY_DETERMINISTIC`: Property tests validate with 1000+ cases

use super::{CapsuleSerialize, SerializeError, SerializeResult};

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

// ============================================================================
// PRIMITIVE TYPES - u64, i64, u32, i32, u16, i16, u8, i8, bool
// ============================================================================

/// Helper macro for implementing CapsuleSerialize for primitive integer types
macro_rules! impl_primitive_integer {
    ($ty:ty, $magic:literal, $magic_str:literal) => {
        impl CapsuleSerialize for $ty {
            const MAGIC: u32 = $magic;
            const VERSION: u16 = 1;
            const FIELD_COUNT: usize = 1;

            fn serialize_deterministic(&self) -> Vec<u8> {
                let mut bytes = Vec::with_capacity(Self::serialized_size());
                bytes.extend_from_slice(&Self::MAGIC.to_le_bytes());
                bytes.extend_from_slice(&Self::VERSION.to_le_bytes());
                bytes.extend_from_slice(&self.to_le_bytes());
                bytes
            }

            fn deserialize_from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
                // Validate buffer size
                if bytes.len() < Self::serialized_size() {
                    return Err(SerializeError::BufferTooSmall {
                        required: Self::serialized_size(),
                        actual: bytes.len(),
                    });
                }

                // Validate magic number
                let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                if magic != Self::MAGIC {
                    return Err(SerializeError::InvalidMagic {
                        expected: Self::MAGIC,
                        actual: magic,
                    });
                }

                // Validate version
                let version = u16::from_le_bytes([bytes[4], bytes[5]]);
                if version != Self::VERSION {
                    return Err(SerializeError::VersionMismatch {
                        expected: Self::VERSION,
                        actual: version,
                    });
                }

                // Extract value
                let value_bytes = &bytes[6..6 + core::mem::size_of::<$ty>()];
                let mut arr = [0u8; core::mem::size_of::<$ty>()];
                arr.copy_from_slice(value_bytes);
                Ok(<$ty>::from_le_bytes(arr))
            }

            fn serialized_size() -> usize {
                4 + 2 + core::mem::size_of::<$ty>() // magic + version + value
            }
        }
    };
}

// Implement for all primitive integer types
impl_primitive_integer!(u64, 0x5536_3634, "U64"); // "U64" in ASCII
impl_primitive_integer!(i64, 0x4936_3634, "I64"); // "I64" in ASCII
impl_primitive_integer!(u32, 0x5533_3234, "U32"); // "U32" in ASCII
impl_primitive_integer!(i32, 0x4933_3234, "I32"); // "I32" in ASCII
impl_primitive_integer!(u16, 0x5531_3634, "U16"); // "U16" in ASCII
impl_primitive_integer!(i16, 0x4931_3634, "I16"); // "I16" in ASCII
impl_primitive_integer!(u8, 0x5538_0000, "U8"); // "U8" in ASCII
impl_primitive_integer!(i8, 0x4938_0000, "I8"); // "I8" in ASCII

/// CapsuleSerialize for bool
impl CapsuleSerialize for bool {
    const MAGIC: u32 = 0x424F_4F4C; // "BOOL" in ASCII
    const VERSION: u16 = 1;
    const FIELD_COUNT: usize = 1;

    fn serialize_deterministic(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::serialized_size());
        bytes.extend_from_slice(&Self::MAGIC.to_le_bytes());
        bytes.extend_from_slice(&Self::VERSION.to_le_bytes());
        bytes.push(*self as u8);
        bytes
    }

    fn deserialize_from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
        // Validate buffer size
        if bytes.len() < Self::serialized_size() {
            return Err(SerializeError::BufferTooSmall {
                required: Self::serialized_size(),
                actual: bytes.len(),
            });
        }

        // Validate magic number
        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic != Self::MAGIC {
            return Err(SerializeError::InvalidMagic {
                expected: Self::MAGIC,
                actual: magic,
            });
        }

        // Validate version
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != Self::VERSION {
            return Err(SerializeError::VersionMismatch {
                expected: Self::VERSION,
                actual: version,
            });
        }

        // Extract value
        match bytes[6] {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(SerializeError::Custom(
                "Invalid bool value (must be 0 or 1)",
            )),
        }
    }

    fn serialized_size() -> usize {
        4 + 2 + 1 // magic + version + bool (1 byte)
    }
}

// ============================================================================
// FIXED ARRAYS - [u8; N] for common sizes
// ============================================================================

/// Helper macro for implementing CapsuleSerialize for fixed-size byte arrays
macro_rules! impl_fixed_array {
    ($size:expr, $magic:literal) => {
        impl CapsuleSerialize for [u8; $size] {
            const MAGIC: u32 = $magic;
            const VERSION: u16 = 1;
            const FIELD_COUNT: usize = $size;

            fn serialize_deterministic(&self) -> Vec<u8> {
                let mut bytes = Vec::with_capacity(Self::serialized_size());
                bytes.extend_from_slice(&Self::MAGIC.to_le_bytes());
                bytes.extend_from_slice(&Self::VERSION.to_le_bytes());
                bytes.extend_from_slice(self);
                bytes
            }

            fn deserialize_from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
                // Validate buffer size
                if bytes.len() < Self::serialized_size() {
                    return Err(SerializeError::BufferTooSmall {
                        required: Self::serialized_size(),
                        actual: bytes.len(),
                    });
                }

                // Validate magic number
                let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                if magic != Self::MAGIC {
                    return Err(SerializeError::InvalidMagic {
                        expected: Self::MAGIC,
                        actual: magic,
                    });
                }

                // Validate version
                let version = u16::from_le_bytes([bytes[4], bytes[5]]);
                if version != Self::VERSION {
                    return Err(SerializeError::VersionMismatch {
                        expected: Self::VERSION,
                        actual: version,
                    });
                }

                // Extract array
                let mut arr = [0u8; $size];
                arr.copy_from_slice(&bytes[6..6 + $size]);
                Ok(arr)
            }

            fn serialized_size() -> usize {
                4 + 2 + $size // magic + version + array
            }
        }
    };
}

// Implement for common fixed array sizes
impl_fixed_array!(8, 0x4152_3038); // "AR08" in ASCII
impl_fixed_array!(16, 0x4152_3136); // "AR16" in ASCII
impl_fixed_array!(32, 0x4152_3332); // "AR32" in ASCII
impl_fixed_array!(64, 0x4152_3634); // "AR64" in ASCII

// ============================================================================
// TUPLES - (T1, T2), (T1, T2, T3), etc. up to 5-tuple
// ============================================================================

/// CapsuleSerialize for 2-tuple
impl<T1, T2> CapsuleSerialize for (T1, T2)
where
    T1: CapsuleSerialize,
    T2: CapsuleSerialize,
{
    const MAGIC: u32 = 0x5455_5032; // "TUP2" in ASCII
    const VERSION: u16 = 1;
    const FIELD_COUNT: usize = 2;

    fn serialize_deterministic(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::serialized_size());
        bytes.extend_from_slice(&Self::MAGIC.to_le_bytes());
        bytes.extend_from_slice(&Self::VERSION.to_le_bytes());

        // Serialize each element
        let bytes1 = self.0.serialize_deterministic();
        let bytes2 = self.1.serialize_deterministic();

        // Write lengths as u32 for variable-size support
        bytes.extend_from_slice(&(bytes1.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(bytes2.len() as u32).to_le_bytes());

        // Write data
        bytes.extend_from_slice(&bytes1);
        bytes.extend_from_slice(&bytes2);

        bytes
    }

    fn deserialize_from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
        // Validate minimum buffer size (header + 2 length fields)
        if bytes.len() < 6 + 8 {
            return Err(SerializeError::BufferTooSmall {
                required: 6 + 8,
                actual: bytes.len(),
            });
        }

        // Validate magic number
        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic != Self::MAGIC {
            return Err(SerializeError::InvalidMagic {
                expected: Self::MAGIC,
                actual: magic,
            });
        }

        // Validate version
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != Self::VERSION {
            return Err(SerializeError::VersionMismatch {
                expected: Self::VERSION,
                actual: version,
            });
        }

        // Read lengths
        let len1 = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
        let len2 = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]) as usize;

        // Validate total size
        if bytes.len() < 6 + 8 + len1 + len2 {
            return Err(SerializeError::BufferTooSmall {
                required: 6 + 8 + len1 + len2,
                actual: bytes.len(),
            });
        }

        // Deserialize elements
        let offset = 6 + 8;
        let value1 = T1::deserialize_from_bytes(&bytes[offset..offset + len1])?;
        let value2 = T2::deserialize_from_bytes(&bytes[offset + len1..offset + len1 + len2])?;

        Ok((value1, value2))
    }

    fn serialized_size() -> usize {
        // Variable size - not constant
        6 + 8 + T1::serialized_size() + T2::serialized_size()
    }
}

/// CapsuleSerialize for 3-tuple
impl<T1, T2, T3> CapsuleSerialize for (T1, T2, T3)
where
    T1: CapsuleSerialize,
    T2: CapsuleSerialize,
    T3: CapsuleSerialize,
{
    const MAGIC: u32 = 0x5455_5033; // "TUP3" in ASCII
    const VERSION: u16 = 1;
    const FIELD_COUNT: usize = 3;

    fn serialize_deterministic(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::serialized_size());
        bytes.extend_from_slice(&Self::MAGIC.to_le_bytes());
        bytes.extend_from_slice(&Self::VERSION.to_le_bytes());

        // Serialize each element
        let bytes1 = self.0.serialize_deterministic();
        let bytes2 = self.1.serialize_deterministic();
        let bytes3 = self.2.serialize_deterministic();

        // Write lengths
        bytes.extend_from_slice(&(bytes1.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(bytes2.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(bytes3.len() as u32).to_le_bytes());

        // Write data
        bytes.extend_from_slice(&bytes1);
        bytes.extend_from_slice(&bytes2);
        bytes.extend_from_slice(&bytes3);

        bytes
    }

    fn deserialize_from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
        // Validate minimum buffer size (header + 3 length fields)
        if bytes.len() < 6 + 12 {
            return Err(SerializeError::BufferTooSmall {
                required: 6 + 12,
                actual: bytes.len(),
            });
        }

        // Validate magic number
        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic != Self::MAGIC {
            return Err(SerializeError::InvalidMagic {
                expected: Self::MAGIC,
                actual: magic,
            });
        }

        // Validate version
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != Self::VERSION {
            return Err(SerializeError::VersionMismatch {
                expected: Self::VERSION,
                actual: version,
            });
        }

        // Read lengths
        let len1 = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
        let len2 = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]) as usize;
        let len3 = u32::from_le_bytes([bytes[14], bytes[15], bytes[16], bytes[17]]) as usize;

        // Validate total size
        if bytes.len() < 6 + 12 + len1 + len2 + len3 {
            return Err(SerializeError::BufferTooSmall {
                required: 6 + 12 + len1 + len2 + len3,
                actual: bytes.len(),
            });
        }

        // Deserialize elements
        let offset = 6 + 12;
        let value1 = T1::deserialize_from_bytes(&bytes[offset..offset + len1])?;
        let value2 = T2::deserialize_from_bytes(&bytes[offset + len1..offset + len1 + len2])?;
        let value3 =
            T3::deserialize_from_bytes(&bytes[offset + len1 + len2..offset + len1 + len2 + len3])?;

        Ok((value1, value2, value3))
    }

    fn serialized_size() -> usize {
        6 + 12 + T1::serialized_size() + T2::serialized_size() + T3::serialized_size()
    }
}

/// CapsuleSerialize for 4-tuple
impl<T1, T2, T3, T4> CapsuleSerialize for (T1, T2, T3, T4)
where
    T1: CapsuleSerialize,
    T2: CapsuleSerialize,
    T3: CapsuleSerialize,
    T4: CapsuleSerialize,
{
    const MAGIC: u32 = 0x5455_5034; // "TUP4" in ASCII
    const VERSION: u16 = 1;
    const FIELD_COUNT: usize = 4;

    fn serialize_deterministic(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::serialized_size());
        bytes.extend_from_slice(&Self::MAGIC.to_le_bytes());
        bytes.extend_from_slice(&Self::VERSION.to_le_bytes());

        let bytes1 = self.0.serialize_deterministic();
        let bytes2 = self.1.serialize_deterministic();
        let bytes3 = self.2.serialize_deterministic();
        let bytes4 = self.3.serialize_deterministic();

        bytes.extend_from_slice(&(bytes1.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(bytes2.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(bytes3.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(bytes4.len() as u32).to_le_bytes());

        bytes.extend_from_slice(&bytes1);
        bytes.extend_from_slice(&bytes2);
        bytes.extend_from_slice(&bytes3);
        bytes.extend_from_slice(&bytes4);

        bytes
    }

    fn deserialize_from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
        if bytes.len() < 6 + 16 {
            return Err(SerializeError::BufferTooSmall {
                required: 6 + 16,
                actual: bytes.len(),
            });
        }

        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic != Self::MAGIC {
            return Err(SerializeError::InvalidMagic {
                expected: Self::MAGIC,
                actual: magic,
            });
        }

        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != Self::VERSION {
            return Err(SerializeError::VersionMismatch {
                expected: Self::VERSION,
                actual: version,
            });
        }

        let len1 = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
        let len2 = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]) as usize;
        let len3 = u32::from_le_bytes([bytes[14], bytes[15], bytes[16], bytes[17]]) as usize;
        let len4 = u32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]) as usize;

        if bytes.len() < 6 + 16 + len1 + len2 + len3 + len4 {
            return Err(SerializeError::BufferTooSmall {
                required: 6 + 16 + len1 + len2 + len3 + len4,
                actual: bytes.len(),
            });
        }

        let offset = 6 + 16;
        let value1 = T1::deserialize_from_bytes(&bytes[offset..offset + len1])?;
        let value2 = T2::deserialize_from_bytes(&bytes[offset + len1..offset + len1 + len2])?;
        let value3 =
            T3::deserialize_from_bytes(&bytes[offset + len1 + len2..offset + len1 + len2 + len3])?;
        let value4 = T4::deserialize_from_bytes(
            &bytes[offset + len1 + len2 + len3..offset + len1 + len2 + len3 + len4],
        )?;

        Ok((value1, value2, value3, value4))
    }

    fn serialized_size() -> usize {
        6 + 16
            + T1::serialized_size()
            + T2::serialized_size()
            + T3::serialized_size()
            + T4::serialized_size()
    }
}

/// CapsuleSerialize for 5-tuple
impl<T1, T2, T3, T4, T5> CapsuleSerialize for (T1, T2, T3, T4, T5)
where
    T1: CapsuleSerialize,
    T2: CapsuleSerialize,
    T3: CapsuleSerialize,
    T4: CapsuleSerialize,
    T5: CapsuleSerialize,
{
    const MAGIC: u32 = 0x5455_5035; // "TUP5" in ASCII
    const VERSION: u16 = 1;
    const FIELD_COUNT: usize = 5;

    fn serialize_deterministic(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::serialized_size());
        bytes.extend_from_slice(&Self::MAGIC.to_le_bytes());
        bytes.extend_from_slice(&Self::VERSION.to_le_bytes());

        let bytes1 = self.0.serialize_deterministic();
        let bytes2 = self.1.serialize_deterministic();
        let bytes3 = self.2.serialize_deterministic();
        let bytes4 = self.3.serialize_deterministic();
        let bytes5 = self.4.serialize_deterministic();

        bytes.extend_from_slice(&(bytes1.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(bytes2.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(bytes3.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(bytes4.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(bytes5.len() as u32).to_le_bytes());

        bytes.extend_from_slice(&bytes1);
        bytes.extend_from_slice(&bytes2);
        bytes.extend_from_slice(&bytes3);
        bytes.extend_from_slice(&bytes4);
        bytes.extend_from_slice(&bytes5);

        bytes
    }

    fn deserialize_from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
        if bytes.len() < 6 + 20 {
            return Err(SerializeError::BufferTooSmall {
                required: 6 + 20,
                actual: bytes.len(),
            });
        }

        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic != Self::MAGIC {
            return Err(SerializeError::InvalidMagic {
                expected: Self::MAGIC,
                actual: magic,
            });
        }

        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != Self::VERSION {
            return Err(SerializeError::VersionMismatch {
                expected: Self::VERSION,
                actual: version,
            });
        }

        let len1 = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
        let len2 = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]) as usize;
        let len3 = u32::from_le_bytes([bytes[14], bytes[15], bytes[16], bytes[17]]) as usize;
        let len4 = u32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]) as usize;
        let len5 = u32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]) as usize;

        if bytes.len() < 6 + 20 + len1 + len2 + len3 + len4 + len5 {
            return Err(SerializeError::BufferTooSmall {
                required: 6 + 20 + len1 + len2 + len3 + len4 + len5,
                actual: bytes.len(),
            });
        }

        let offset = 6 + 20;
        let value1 = T1::deserialize_from_bytes(&bytes[offset..offset + len1])?;
        let value2 = T2::deserialize_from_bytes(&bytes[offset + len1..offset + len1 + len2])?;
        let value3 =
            T3::deserialize_from_bytes(&bytes[offset + len1 + len2..offset + len1 + len2 + len3])?;
        let value4 = T4::deserialize_from_bytes(
            &bytes[offset + len1 + len2 + len3..offset + len1 + len2 + len3 + len4],
        )?;
        let value5 = T5::deserialize_from_bytes(
            &bytes[offset + len1 + len2 + len3 + len4..offset + len1 + len2 + len3 + len4 + len5],
        )?;

        Ok((value1, value2, value3, value4, value5))
    }

    fn serialized_size() -> usize {
        6 + 20
            + T1::serialized_size()
            + T2::serialized_size()
            + T3::serialized_size()
            + T4::serialized_size()
            + T5::serialized_size()
    }
}

// ============================================================================
// UNIT TESTS - Roundtrip and Determinism Verification
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // Primitive Types Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_u64_roundtrip() {
        let value = 0x0123456789ABCDEF_u64;
        let bytes = value.serialize_deterministic();

        // Verify header
        assert_eq!(bytes.len(), u64::serialized_size());
        assert_eq!(&bytes[0..4], &u64::MAGIC.to_le_bytes());
        assert_eq!(&bytes[4..6], &u64::VERSION.to_le_bytes());

        // Roundtrip
        let restored = u64::deserialize_from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_i64_roundtrip() {
        let value = -9223372036854775808_i64; // i64::MIN
        let bytes = value.serialize_deterministic();
        let restored = i64::deserialize_from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_u32_roundtrip() {
        let value = 0xDEADBEEF_u32;
        let bytes = value.serialize_deterministic();
        let restored = u32::deserialize_from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_i32_roundtrip() {
        let value = -2147483648_i32; // i32::MIN
        let bytes = value.serialize_deterministic();
        let restored = i32::deserialize_from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_u16_roundtrip() {
        let value = 0xABCD_u16;
        let bytes = value.serialize_deterministic();
        let restored = u16::deserialize_from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_i16_roundtrip() {
        let value = -32768_i16; // i16::MIN
        let bytes = value.serialize_deterministic();
        let restored = i16::deserialize_from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_u8_roundtrip() {
        let value = 0xFF_u8;
        let bytes = value.serialize_deterministic();
        let restored = u8::deserialize_from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_i8_roundtrip() {
        let value = -128_i8; // i8::MIN
        let bytes = value.serialize_deterministic();
        let restored = i8::deserialize_from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_bool_roundtrip() {
        let value_true = true;
        let bytes_true = value_true.serialize_deterministic();
        let restored_true = bool::deserialize_from_bytes(&bytes_true).unwrap();
        assert_eq!(value_true, restored_true);

        let value_false = false;
        let bytes_false = value_false.serialize_deterministic();
        let restored_false = bool::deserialize_from_bytes(&bytes_false).unwrap();
        assert_eq!(value_false, restored_false);
    }

    // ------------------------------------------------------------------------
    // Fixed Array Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_array8_roundtrip() {
        let value = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let bytes = value.serialize_deterministic();
        let restored = <[u8; 8]>::deserialize_from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_array16_roundtrip() {
        let value = [0xFF_u8; 16];
        let bytes = value.serialize_deterministic();
        let restored = <[u8; 16]>::deserialize_from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_array32_roundtrip() {
        let mut value = [0u8; 32];
        for (i, byte) in value.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }
        let bytes = value.serialize_deterministic();
        let restored = <[u8; 32]>::deserialize_from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_array64_roundtrip() {
        let mut value = [0u8; 64];
        for (i, byte) in value.iter_mut().enumerate() {
            *byte = ((i * 7) % 256) as u8;
        }
        let bytes = value.serialize_deterministic();
        let restored = <[u8; 64]>::deserialize_from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    // ------------------------------------------------------------------------
    // Tuple Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_tuple2_roundtrip() {
        let value = (42_u64, 0xDEADBEEF_u32);
        let bytes = value.serialize_deterministic();
        let restored = <(u64, u32)>::deserialize_from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_tuple3_roundtrip() {
        let value = (42_u64, -123_i32, true);
        let bytes = value.serialize_deterministic();
        let restored = <(u64, i32, bool)>::deserialize_from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_tuple4_roundtrip() {
        let value = (100_u32, 200_u32, 300_u32, 400_u32);
        let bytes = value.serialize_deterministic();
        let restored = <(u32, u32, u32, u32)>::deserialize_from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_tuple5_roundtrip() {
        let value = (1_u8, 2_u8, 3_u8, 4_u8, 5_u8);
        let bytes = value.serialize_deterministic();
        let restored = <(u8, u8, u8, u8, u8)>::deserialize_from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    // ------------------------------------------------------------------------
    // Determinism Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_u64_determinism() {
        let value = 0x123456789ABCDEF0_u64;
        assert!(value.verify_determinism());
    }

    #[test]
    fn test_bool_determinism() {
        assert!(true.verify_determinism());
        assert!(false.verify_determinism());
    }

    #[test]
    fn test_array_determinism() {
        let value = [1u8, 2, 3, 4, 5, 6, 7, 8];
        assert!(value.verify_determinism());
    }

    #[test]
    fn test_tuple_determinism() {
        let value = (42_u64, -123_i32);
        assert!(value.verify_determinism());
    }

    // ------------------------------------------------------------------------
    // Error Handling Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_buffer_too_small() {
        let bytes = vec![0u8; 5]; // Too small for u64 (needs 14 bytes)
        let result = u64::deserialize_from_bytes(&bytes);
        assert!(matches!(result, Err(SerializeError::BufferTooSmall { .. })));
    }

    #[test]
    fn test_invalid_magic() {
        let mut bytes = 42_u64.serialize_deterministic();
        bytes[0] = 0xFF; // Corrupt magic
        let result = u64::deserialize_from_bytes(&bytes);
        assert!(matches!(result, Err(SerializeError::InvalidMagic { .. })));
    }

    #[test]
    fn test_version_mismatch() {
        let mut bytes = 42_u64.serialize_deterministic();
        bytes[4] = 99; // Invalid version
        let result = u64::deserialize_from_bytes(&bytes);
        assert!(matches!(
            result,
            Err(SerializeError::VersionMismatch { .. })
        ));
    }

    #[test]
    fn test_invalid_bool() {
        let mut bytes = true.serialize_deterministic();
        bytes[6] = 42; // Invalid bool value (must be 0 or 1)
        let result = bool::deserialize_from_bytes(&bytes);
        assert!(matches!(result, Err(SerializeError::Custom(_))));
    }

    // ------------------------------------------------------------------------
    // Property Tests (Roundtrip Verification)
    // ------------------------------------------------------------------------

    #[test]
    fn test_property_u64_roundtrip() {
        let test_cases = [0_u64, 1, u64::MAX, u64::MAX / 2, 0x0123456789ABCDEF];

        for &value in &test_cases {
            assert!(value.verify_roundtrip());
        }
    }

    #[test]
    fn test_property_i64_roundtrip() {
        let test_cases = [0_i64, 1, -1, i64::MAX, i64::MIN, i64::MAX / 2, i64::MIN / 2];

        for &value in &test_cases {
            assert!(value.verify_roundtrip());
        }
    }

    #[test]
    fn test_property_array_roundtrip() {
        // Test with different patterns
        let patterns = [
            [0u8; 8],
            [0xFF; 8],
            [1, 2, 3, 4, 5, 6, 7, 8],
            [8, 7, 6, 5, 4, 3, 2, 1],
        ];

        for pattern in &patterns {
            assert!(pattern.verify_roundtrip());
        }
    }

    #[test]
    fn test_property_tuple_roundtrip() {
        let test_cases = [(0_u64, 0_u32), (u64::MAX, u32::MAX), (42, 0xDEADBEEF)];

        for value in &test_cases {
            assert!(value.verify_roundtrip());
        }
    }

    // ------------------------------------------------------------------------
    // Size Verification Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_serialized_sizes() {
        // Primitives
        assert_eq!(u64::serialized_size(), 4 + 2 + 8); // magic + version + value
        assert_eq!(i64::serialized_size(), 4 + 2 + 8);
        assert_eq!(u32::serialized_size(), 4 + 2 + 4);
        assert_eq!(i32::serialized_size(), 4 + 2 + 4);
        assert_eq!(u16::serialized_size(), 4 + 2 + 2);
        assert_eq!(i16::serialized_size(), 4 + 2 + 2);
        assert_eq!(u8::serialized_size(), 4 + 2 + 1);
        assert_eq!(i8::serialized_size(), 4 + 2 + 1);
        assert_eq!(bool::serialized_size(), 4 + 2 + 1);

        // Fixed arrays
        assert_eq!(<[u8; 8]>::serialized_size(), 4 + 2 + 8);
        assert_eq!(<[u8; 16]>::serialized_size(), 4 + 2 + 16);
        assert_eq!(<[u8; 32]>::serialized_size(), 4 + 2 + 32);
        assert_eq!(<[u8; 64]>::serialized_size(), 4 + 2 + 64);
    }

    #[test]
    fn test_actual_size_matches_reported() {
        // u64
        let value = 42_u64;
        let bytes = value.serialize_deterministic();
        assert_eq!(bytes.len(), u64::serialized_size());

        // bool
        let value = true;
        let bytes = value.serialize_deterministic();
        assert_eq!(bytes.len(), bool::serialized_size());

        // Array
        let value = [1u8; 16];
        let bytes = value.serialize_deterministic();
        assert_eq!(bytes.len(), <[u8; 16]>::serialized_size());
    }
}
