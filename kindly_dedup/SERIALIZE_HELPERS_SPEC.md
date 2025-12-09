# serialize_helpers.rs Specification

**Status**: Specification for Agent 1 Implementation

**Location**: `/home/samuel/Primitives/kindly_dedup/src/serialize_helpers.rs`

**Purpose**: Bridge layer from serde patterns to atomic_capsule::serialize::CapsuleSerialize

---

## Module Overview

```rust
//! # Serialization Helpers for CapsuleSerialize Migration
//!
//! Provides helper functions and macros to simplify migration from serde
//! to atomic_capsule::serialize::CapsuleSerialize pattern.
//!
//! ## Feature Flags
//!
//! - `json` - JSON serialization helpers (requires simd-json or atomic_capsule JSON)
//! - `bincode` - Bincode format helpers (for binary config storage)
//!
//! ## Examples
//!
//! ### Manual Implementation
//!
//! ```rust,ignore
//! use crate::serialize_helpers::*;
//! use atomic_capsule::serialize::CapsuleSerialize;
//!
//! #[derive(Debug, Clone)]
//! pub struct MyConfig {
//!     pub name: String,
//!     pub version: u32,
//! }
//!
//! impl CapsuleSerialize for MyConfig {
//!     const MAGIC: u32 = 0x4D59434F; // "MYCO"
//!     const VERSION: u16 = 1;
//!     const FIELD_COUNT: usize = 2;
//!
//!     fn serialize_deterministic(&self) -> Vec<u8> {
//!         let mut bytes = vec![];
//!         bytes.extend_from_slice(&Self::MAGIC.to_le_bytes());
//!         bytes.extend_from_slice(&Self::VERSION.to_le_bytes());
//!         serialize_string(&self.name, &mut bytes);
//!         bytes.extend_from_slice(&self.version.to_le_bytes());
//!         bytes
//!     }
//!
//!     fn deserialize_from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
//!         let (magic, version, rest) = deserialize_header(bytes)?;
//!         validate_magic(magic, Self::MAGIC)?;
//!         validate_version(version, Self::VERSION)?;
//!
//!         let (name, rest) = deserialize_string(rest)?;
//!         let version = u32::from_le_bytes(rest[0..4].try_into()?);
//!
//!         Ok(Self { name, version })
//!     }
//!
//!     fn serialized_size() -> usize {
//!         4 + 2 + 4 + 4 // MAGIC + VERSION + string size field + version
//!     }
//! }
//! ```

use atomic_capsule::serialize::{CapsuleSerialize, SerializeResult, SerializeError};
use std::io::{self, ErrorKind};

// ============================================================================
// HELPER TRAITS
// ============================================================================

/// Trait for types that can be serialized with magic/version header
pub trait HeaderSerialize: CapsuleSerialize {
    /// Validate magic number
    fn validate_magic(&self, actual: u32) -> SerializeResult<()> {
        if actual != Self::MAGIC {
            Err(SerializeError::InvalidMagic)
        } else {
            Ok(())
        }
    }

    /// Validate version compatibility
    fn validate_version(&self, actual: u16) -> SerializeResult<()> {
        if actual != Self::VERSION {
            Err(SerializeError::VersionMismatch)
        } else {
            Ok(())
        }
    }
}

// ============================================================================
// PRIMITIVE SERIALIZATION HELPERS
// ============================================================================

/// Serialize u8 to bytes (no encoding needed)
#[inline]
pub fn serialize_u8(value: u8, bytes: &mut Vec<u8>) {
    bytes.push(value);
}

/// Deserialize u8 from bytes
#[inline]
pub fn deserialize_u8(bytes: &[u8]) -> SerializeResult<(u8, &[u8])> {
    if bytes.is_empty() {
        return Err(SerializeError::BufferTooSmall);
    }
    Ok((bytes[0], &bytes[1..]))
}

/// Serialize u16 (little-endian)
#[inline]
pub fn serialize_u16(value: u16, bytes: &mut Vec<u8>) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

/// Deserialize u16 (little-endian)
#[inline]
pub fn deserialize_u16(bytes: &[u8]) -> SerializeResult<(u16, &[u8])> {
    if bytes.len() < 2 {
        return Err(SerializeError::BufferTooSmall);
    }
    let value = u16::from_le_bytes(bytes[0..2].try_into().map_err(|_| SerializeError::InvalidData)?);
    Ok((value, &bytes[2..]))
}

/// Serialize u32 (little-endian)
#[inline]
pub fn serialize_u32(value: u32, bytes: &mut Vec<u8>) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

/// Deserialize u32 (little-endian)
#[inline]
pub fn deserialize_u32(bytes: &[u8]) -> SerializeResult<(u32, &[u8])> {
    if bytes.len() < 4 {
        return Err(SerializeError::BufferTooSmall);
    }
    let value = u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| SerializeError::InvalidData)?);
    Ok((value, &bytes[4..]))
}

/// Serialize u64 (little-endian)
#[inline]
pub fn serialize_u64(value: u64, bytes: &mut Vec<u8>) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

/// Deserialize u64 (little-endian)
#[inline]
pub fn deserialize_u64(bytes: &[u8]) -> SerializeResult<(u64, &[u8])> {
    if bytes.len() < 8 {
        return Err(SerializeError::BufferTooSmall);
    }
    let value = u64::from_le_bytes(bytes[0..8].try_into().map_err(|_| SerializeError::InvalidData)?);
    Ok((value, &bytes[8..]))
}

/// Serialize bool (0x00 = false, 0x01 = true)
#[inline]
pub fn serialize_bool(value: bool, bytes: &mut Vec<u8>) {
    bytes.push(if value { 0x01 } else { 0x00 });
}

/// Deserialize bool
#[inline]
pub fn deserialize_bool(bytes: &[u8]) -> SerializeResult<(bool, &[u8])> {
    if bytes.is_empty() {
        return Err(SerializeError::BufferTooSmall);
    }
    match bytes[0] {
        0x00 => Ok((false, &bytes[1..])),
        0x01 => Ok((true, &bytes[1..])),
        _ => Err(SerializeError::InvalidData),
    }
}

// ============================================================================
// STRING SERIALIZATION
// ============================================================================

/// Serialize String with length prefix (u32 little-endian)
///
/// Format: [length: u32 LE][utf8 bytes...]
#[inline]
pub fn serialize_string(value: &str, bytes: &mut Vec<u8>) {
    let len = value.len() as u32;
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

/// Deserialize String with length prefix
///
/// Format: [length: u32 LE][utf8 bytes...]
pub fn deserialize_string(bytes: &[u8]) -> SerializeResult<(String, &[u8])> {
    if bytes.len() < 4 {
        return Err(SerializeError::BufferTooSmall);
    }

    let len = u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| SerializeError::InvalidData)?) as usize;
    let start = 4;
    let end = start + len;

    if bytes.len() < end {
        return Err(SerializeError::BufferTooSmall);
    }

    let string = String::from_utf8(bytes[start..end].to_vec())
        .map_err(|_| SerializeError::InvalidData)?;

    Ok((string, &bytes[end..]))
}

// ============================================================================
// HEADER SERIALIZATION
// ============================================================================

/// Serialize magic + version header
///
/// Format: [magic: u32 LE][version: u16 LE]
#[inline]
pub fn serialize_header(magic: u32, version: u16, bytes: &mut Vec<u8>) {
    bytes.extend_from_slice(&magic.to_le_bytes());
    bytes.extend_from_slice(&version.to_le_bytes());
}

/// Deserialize magic + version header
///
/// Format: [magic: u32 LE][version: u16 LE]
///
/// Returns: (magic, version, remaining bytes)
pub fn deserialize_header(bytes: &[u8]) -> SerializeResult<(u32, u16, &[u8])> {
    if bytes.len() < 6 {
        return Err(SerializeError::BufferTooSmall);
    }

    let magic = u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| SerializeError::InvalidData)?);
    let version = u16::from_le_bytes(bytes[4..6].try_into().map_err(|_| SerializeError::InvalidData)?);

    Ok((magic, version, &bytes[6..]))
}

// ============================================================================
// VALIDATION HELPERS
// ============================================================================

/// Validate magic number matches expected value
pub fn validate_magic(actual: u32, expected: u32) -> SerializeResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(SerializeError::InvalidMagic)
    }
}

/// Validate version matches expected value
pub fn validate_version(actual: u16, expected: u16) -> SerializeResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(SerializeError::VersionMismatch)
    }
}

/// Validate buffer has minimum required size
pub fn validate_size(bytes: &[u8], required: usize) -> SerializeResult<()> {
    if bytes.len() >= required {
        Ok(())
    } else {
        Err(SerializeError::BufferTooSmall)
    }
}

// ============================================================================
// JSON SERIALIZATION (Feature-Gated)
// ============================================================================

#[cfg(feature = "json")]
pub mod json {
    use super::*;

    /// Serialize CapsuleSerialize to JSON string (if applicable)
    ///
    /// Note: Most CapsuleSerialize types serialize to binary, not JSON.
    /// This helper is for types that support both formats.
    pub fn to_json<T: CapsuleSerialize>(value: &T) -> SerializeResult<String> {
        // Use atomic_capsule JSON if available, or implement custom JSON serialization
        Err(SerializeError::UnsupportedFormat)
    }

    /// Deserialize CapsuleSerialize from JSON string
    pub fn from_json<T: CapsuleSerialize>(json: &str) -> SerializeResult<T> {
        Err(SerializeError::UnsupportedFormat)
    }
}

// ============================================================================
// COLLECTION SERIALIZATION
// ============================================================================

/// Serialize Vec<T> where T can be converted to bytes
///
/// Format: [count: u32 LE][item1][item2]...[itemN]
pub fn serialize_vec<T: AsRef<[u8]>>(items: &[T], bytes: &mut Vec<u8>) -> SerializeResult<()> {
    let count = items.len() as u32;
    bytes.extend_from_slice(&count.to_le_bytes());
    for item in items {
        bytes.extend_from_slice(item.as_ref());
    }
    Ok(())
}

/// Serialize Vec<u32> with fixed-size items
///
/// Format: [count: u32 LE][item1: u32 LE]...[itemN: u32 LE]
pub fn serialize_vec_u32(items: &[u32], bytes: &mut Vec<u8>) {
    let count = items.len() as u32;
    bytes.extend_from_slice(&count.to_le_bytes());
    for &item in items {
        bytes.extend_from_slice(&item.to_le_bytes());
    }
}

/// Deserialize Vec<u32>
pub fn deserialize_vec_u32(bytes: &[u8]) -> SerializeResult<(Vec<u32>, &[u8])> {
    if bytes.len() < 4 {
        return Err(SerializeError::BufferTooSmall);
    }

    let count = u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| SerializeError::InvalidData)?) as usize;
    let mut items = Vec::with_capacity(count);
    let mut pos = 4;

    for _ in 0..count {
        if pos + 4 > bytes.len() {
            return Err(SerializeError::BufferTooSmall);
        }
        let value = u32::from_le_bytes(bytes[pos..pos+4].try_into().map_err(|_| SerializeError::InvalidData)?);
        items.push(value);
        pos += 4;
    }

    Ok((items, &bytes[pos..]))
}

// ============================================================================
// MACROS FOR CODE GENERATION
// ============================================================================

/// Macro to implement CapsuleSerialize for simple structs
///
/// Usage:
/// ```rust,ignore
/// #[derive(Debug, Clone)]
/// pub struct MyStruct {
///     pub field1: u32,
///     pub field2: String,
/// }
///
/// impl_capsule_serialize!(MyStruct {
///     magic: 0x4D535452; // "MSTR"
///     version: 1;
///     fields: [
///         field1: u32,
///         field2: String,
///     ];
/// });
/// ```
#[macro_export]
macro_rules! impl_capsule_serialize {
    (
        $struct_name:ident {
            magic: $magic:expr;
            version: $version:expr;
            fields: [
                $($field_name:ident: $field_type:ty),* $(,)?
            ];
        }
    ) => {
        impl CapsuleSerialize for $struct_name {
            const MAGIC: u32 = $magic;
            const VERSION: u16 = $version;
            const FIELD_COUNT: usize = <[()]>::len(&[$(stringify!($field_name)),*]);

            fn serialize_deterministic(&self) -> Vec<u8> {
                let mut bytes = Vec::new();
                bytes.extend_from_slice(&Self::MAGIC.to_le_bytes());
                bytes.extend_from_slice(&Self::VERSION.to_le_bytes());

                $(
                    // Field-specific serialization would go here
                    // This is a template - actual implementation depends on field types
                )*

                bytes
            }

            fn deserialize_from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
                let (magic, version, rest) = deserialize_header(bytes)?;
                validate_magic(magic, Self::MAGIC)?;
                validate_version(version, Self::VERSION)?;

                // Deserialization logic would go here
                Err(SerializeError::Unimplemented)
            }

            fn serialized_size() -> usize {
                6 // magic (4) + version (2)
            }
        }
    };
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_u32() {
        let mut bytes = Vec::new();
        serialize_u32(0x12345678, &mut bytes);
        assert_eq!(bytes, vec![0x78, 0x56, 0x34, 0x12]);
    }

    #[test]
    fn test_deserialize_u32() {
        let bytes = vec![0x78, 0x56, 0x34, 0x12];
        let (value, rest) = deserialize_u32(&bytes).unwrap();
        assert_eq!(value, 0x12345678);
        assert!(rest.is_empty());
    }

    #[test]
    fn test_serialize_string() {
        let mut bytes = Vec::new();
        serialize_string("hello", &mut bytes);
        assert_eq!(bytes.len(), 4 + 5); // length prefix + "hello"
    }

    #[test]
    fn test_deserialize_string() {
        let mut bytes = Vec::new();
        serialize_string("hello", &mut bytes);
        let (string, rest) = deserialize_string(&bytes).unwrap();
        assert_eq!(string, "hello");
        assert!(rest.is_empty());
    }

    #[test]
    fn test_serialize_bool() {
        let mut bytes = Vec::new();
        serialize_bool(true, &mut bytes);
        assert_eq!(bytes, vec![0x01]);

        bytes.clear();
        serialize_bool(false, &mut bytes);
        assert_eq!(bytes, vec![0x00]);
    }
}
```

## Expected API Summary

| Function | Signature | Purpose |
|----------|-----------|---------|
| `serialize_u8` | `(u8, &mut Vec<u8>) -> ()` | Serialize single byte |
| `serialize_u32` | `(u32, &mut Vec<u8>) -> ()` | Serialize 32-bit int LE |
| `serialize_u64` | `(u64, &mut Vec<u8>) -> ()` | Serialize 64-bit int LE |
| `serialize_string` | `(&str, &mut Vec<u8>) -> ()` | Serialize String with length |
| `serialize_header` | `(u32, u16, &mut Vec<u8>) -> ()` | Serialize magic+version |
| `deserialize_u32` | `(&[u8]) -> Result<(u32, &[u8])>` | Deserialize 32-bit int |
| `deserialize_string` | `(&[u8]) -> Result<(String, &[u8])>` | Deserialize String |
| `deserialize_header` | `(&[u8]) -> Result<(u32, u16, &[u8])>` | Deserialize magic+version |
| `validate_magic` | `(u32, u32) -> Result<()>` | Check magic number |
| `validate_version` | `(u16, u16) -> Result<()>` | Check version |

---

## Implementation Notes for Agent 1

1. **Use atomic_capsule types**: Import from `atomic_capsule::serialize`
2. **Little-endian only**: All multi-byte integers use LE for cross-platform
3. **Length prefixes**: Strings/Vecs use u32 LE length prefix
4. **Magic numbers**: Use 4-byte ASCII codes (e.g., 0x4D59434F = "MYCO")
5. **Error handling**: Propagate SerializeError properly
6. **Tests**: Add roundtrip tests for all helpers
7. **Documentation**: Add examples showing usage

---

## Next Steps for Agent 2

Once this module is implemented:

1. Add to `src/lib.rs` module exports:
   ```rust
   pub mod serialize_helpers;
   pub use serialize_helpers::*;
   ```

2. Start migration following SERIALIZE_MIGRATION_CHECKLIST.md

3. Use patterns from this spec in manual CapsuleSerialize implementations
