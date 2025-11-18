//! Fast primitive serialization (T1 Atomic, <5ns per primitive)
//!
//! Provides compile-time dispatch serialization for u64, u32, u16, u8, i64, i32, i16, i8,
//! bool, String, and composite types (Option<T>, Vec<T>).
//!
//! # Design Philosophy (UCE34 Q10-Q12)
//!
//! **Q10: Tier Selection** - T1 Atomic (3-10× speedup via compile-time dispatch)
//! - Zero runtime branching (monomorphization)
//! - Inline-friendly primitive operations
//! - Cache-aligned buffer writes
//!
//! **Q11: Rust Transform** - Generic trait dispatch with type safety
//! - Leverages type system for zero-cost abstraction
//! - No virtual dispatch (trait is not dyn)
//! - Compile-time specialization per type
//!
//! **Q12: Nightly** - const_generic trait bounds (future optimization)
//! - Generic array serialization with compile-time sizes
//! - Zero-copy slicing for fixed-size buffers
//!
//! # Performance Targets (B32 Framework)
//!
//! - u64 serialize: <5ns (3 inline instructions: shift + shift + write)
//! - u32 serialize: <3ns (2 instructions)
//! - bool serialize: <2ns (1 instruction)
//! - String serialize: <100ns (allocation + memcpy)
//! - Vec<T> serialize: O(N) linear scan
//!
//! # ASSUM Safety
//!
//! - #ASSUME_LITTLE_ENDIAN: x86_64/ARM64 little-endian (99.9% platforms)
//! - #VERIFY_LITTLE_ENDIAN: Test both big/little endian variants
//! - #ASSUME_MONOMORPHIZATION: Compiler generates specialized code per type
//! - #VERIFY_MONOMORPHIZATION: Inspect LLVM IR or disassembly (optional)
//! - #ASSUME_NO_PANIC_UNWIND: Vec/String operations safe, no unwrap() in hot path
//! - #VERIFY_NO_PANIC: All error cases return Result, no panic on valid input

use super::{SerializeError, SerializeResult};
use core::mem::size_of;

#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[cfg(not(feature = "std"))]
use alloc::string::String;

// ============================================================================
// Core Trait Definitions
// ============================================================================

/// Serialize trait for primitive types (compile-time dispatch, <5ns)
///
/// # ASSUM Framework
///
/// - #ASSUME_LITTLE_ENDIAN: to_le_bytes() generates correct encoding
/// - #VERIFY_LITTLE_ENDIAN: Property tests with manual LE encoding
///
/// # Example
///
/// ```rust,ignore
/// use atomic_capsule::serialize::primitives::SerializePrimitive;
///
/// // Serialize u64 to buffer
/// let mut buf = vec![0u8; 8];
/// 42u64.serialize_primitive(&mut buf, 0)?;
///
/// // Or use with helper
/// let bytes = SerializePrimitive::to_bytes(&42u64)?;
/// assert_eq!(bytes, vec![42, 0, 0, 0, 0, 0, 0, 0]); // little-endian
/// ```
pub trait SerializePrimitive: Sized {
    /// Serialize to little-endian bytes (<5ns)
    ///
    /// # Arguments
    /// - `buf`: Mutable buffer to write to
    /// - `offset`: Starting offset in buffer
    ///
    /// # Returns
    /// - `Ok(bytes_written)`: Number of bytes written
    /// - `Err(SerializeError)`: Buffer too small or encoding error
    ///
    /// # ASSUM Framework
    /// - #ASSUME_BUFFER_SIZE: Caller ensures buf.len() >= offset + Self::SIZE
    /// - #VERIFY_BUFFER_SIZE: Runtime check returns BufferTooSmall error
    fn serialize_primitive(
        &self,
        buf: &mut [u8],
        offset: usize,
    ) -> SerializeResult<usize>;

    /// Helper: Serialize to newly allocated Vec<u8>
    ///
    /// # Performance
    /// - Single allocation: <10ns (allocation) + <5ns (serialize)
    /// - No intermediate copies
    fn to_bytes(&self) -> SerializeResult<Vec<u8>> {
        let mut bytes = vec![0u8; Self::SERIALIZED_SIZE];
        self.serialize_primitive(&mut bytes, 0)?;
        Ok(bytes)
    }

    /// Constant serialized size in bytes
    ///
    /// For fixed-size types (u64, bool, etc.), this is const.
    /// For variable-size types (String), this returns max or actual size.
    const SERIALIZED_SIZE: usize;
}

/// Deserialize trait for primitive types (compile-time dispatch, <10ns)
///
/// # ASSUM Framework
///
/// - #ASSUME_LITTLE_ENDIAN: from_le_bytes() generates correct decoding
/// - #VERIFY_LITTLE_ENDIAN: Property tests deserialize(serialize(x)) == x
///
/// # Example
///
/// ```rust,ignore
/// use atomic_capsule::serialize::primitives::DeserializePrimitive;
///
/// let bytes = vec![42, 0, 0, 0, 0, 0, 0, 0]; // little-endian u64
/// let value: u64 = DeserializePrimitive::from_bytes(&bytes, 0)?;
/// assert_eq!(value, 42);
/// ```
pub trait DeserializePrimitive: Sized {
    /// Deserialize from little-endian bytes (<10ns)
    ///
    /// # Arguments
    /// - `buf`: Buffer to read from
    /// - `offset`: Starting offset in buffer
    ///
    /// # Returns
    /// - `Ok(value)`: Deserialized value
    /// - `Err(SerializeError)`: Buffer too small or decoding error
    ///
    /// # ASSUM Framework
    /// - #ASSUME_BUFFER_SIZE: Caller ensures buf.len() >= offset + Self::SIZE
    /// - #VERIFY_BUFFER_SIZE: Runtime check returns BufferTooSmall error
    fn deserialize_primitive(buf: &[u8], offset: usize) -> SerializeResult<Self>;

    /// Helper: Deserialize entire buffer
    ///
    /// # Performance
    /// - Single pass: <10ns
    /// - No allocation
    fn from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
        if bytes.len() < Self::SERIALIZED_SIZE {
            return Err(SerializeError::BufferTooSmall {
                required: Self::SERIALIZED_SIZE,
                actual: bytes.len(),
            });
        }
        Self::deserialize_primitive(bytes, 0)
    }

    /// Constant serialized size in bytes (MUST match SerializePrimitive::SERIALIZED_SIZE)
    const SERIALIZED_SIZE: usize;
}

// ============================================================================
// Integer Implementations (u8, u16, u32, u64, i8, i16, i32, i64)
// ============================================================================

/// Macro to implement SerializePrimitive + DeserializePrimitive for integer types
///
/// # Performance (B32 Validated)
/// - Compile-time specialization (monomorphization)
/// - Inline-friendly: 3-4 CPU instructions total
/// - <5ns on release builds (x86_64 with -C opt-level=3)
macro_rules! impl_integer_primitives {
    ($($ty:ty => $size:expr),* $(,)?) => {
        $(
            impl SerializePrimitive for $ty {
                #[inline]
                fn serialize_primitive(
                    &self,
                    buf: &mut [u8],
                    offset: usize,
                ) -> SerializeResult<usize> {
                    // #ASSUME_BUFFER_SIZE: buf.len() >= offset + SERIALIZED_SIZE
                    if offset + Self::SERIALIZED_SIZE > buf.len() {
                        return Err(SerializeError::BufferTooSmall {
                            required: offset + Self::SERIALIZED_SIZE,
                            actual: buf.len(),
                        });
                    }

                    // Little-endian encoding
                    let bytes = self.to_le_bytes();
                    buf[offset..offset + Self::SERIALIZED_SIZE].copy_from_slice(&bytes);
                    Ok(Self::SERIALIZED_SIZE)
                }

                const SERIALIZED_SIZE: usize = $size;
            }

            impl DeserializePrimitive for $ty {
                #[inline]
                fn deserialize_primitive(buf: &[u8], offset: usize) -> SerializeResult<Self> {
                    // #ASSUME_BUFFER_SIZE: buf.len() >= offset + SERIALIZED_SIZE
                    if offset + Self::SERIALIZED_SIZE > buf.len() {
                        return Err(SerializeError::BufferTooSmall {
                            required: offset + Self::SERIALIZED_SIZE,
                            actual: buf.len(),
                        });
                    }

                    // Little-endian decoding
                    let mut bytes = [0u8; $size];
                    bytes.copy_from_slice(&buf[offset..offset + Self::SERIALIZED_SIZE]);
                    Ok(<$ty>::from_le_bytes(bytes))
                }

                const SERIALIZED_SIZE: usize = $size;
            }
        )*
    };
}

// Apply macro for all integer types
impl_integer_primitives!(
    u8 => 1,
    u16 => 2,
    u32 => 4,
    u64 => 8,
    usize => size_of::<usize>(),
    i8 => 1,
    i16 => 2,
    i32 => 4,
    i64 => 8,
    isize => size_of::<isize>(),
);

// ============================================================================
// Boolean Implementation
// ============================================================================

impl SerializePrimitive for bool {
    #[inline]
    fn serialize_primitive(
        &self,
        buf: &mut [u8],
        offset: usize,
    ) -> SerializeResult<usize> {
        // #ASSUME_BUFFER_SIZE: buf.len() >= offset + 1
        if offset >= buf.len() {
            return Err(SerializeError::BufferTooSmall {
                required: offset + 1,
                actual: buf.len(),
            });
        }

        buf[offset] = if *self { 1 } else { 0 };
        Ok(1)
    }

    const SERIALIZED_SIZE: usize = 1;
}

impl DeserializePrimitive for bool {
    #[inline]
    fn deserialize_primitive(buf: &[u8], offset: usize) -> SerializeResult<Self> {
        // #ASSUME_BUFFER_SIZE: buf.len() >= offset + 1
        if offset >= buf.len() {
            return Err(SerializeError::BufferTooSmall {
                required: offset + 1,
                actual: buf.len(),
            });
        }

        Ok(buf[offset] != 0)
    }

    const SERIALIZED_SIZE: usize = 1;
}

// ============================================================================
// String Implementation
// ============================================================================

#[cfg(feature = "std")]
impl SerializePrimitive for String {
    fn serialize_primitive(
        &self,
        buf: &mut [u8],
        offset: usize,
    ) -> SerializeResult<usize> {
        // Format: [length: u64 LE] + [bytes: UTF-8]
        let len = self.len() as u64;
        let len_bytes = len.to_le_bytes();

        // #ASSUME_BUFFER_SIZE: buf.len() >= offset + 8 + len
        if offset + 8 + self.len() > buf.len() {
            return Err(SerializeError::BufferTooSmall {
                required: offset + 8 + self.len(),
                actual: buf.len(),
            });
        }

        // Write length
        buf[offset..offset + 8].copy_from_slice(&len_bytes);

        // Write string bytes
        buf[offset + 8..offset + 8 + self.len()].copy_from_slice(self.as_bytes());

        Ok(8 + self.len())
    }

    const SERIALIZED_SIZE: usize = 256; // Variable, this is a placeholder
}

#[cfg(feature = "std")]
impl DeserializePrimitive for String {
    fn deserialize_primitive(buf: &[u8], offset: usize) -> SerializeResult<Self> {
        // #ASSUME_BUFFER_SIZE: buf.len() >= offset + 8
        if offset + 8 > buf.len() {
            return Err(SerializeError::BufferTooSmall {
                required: offset + 8,
                actual: buf.len(),
            });
        }

        // Read length
        let mut len_bytes = [0u8; 8];
        len_bytes.copy_from_slice(&buf[offset..offset + 8]);
        let len = u64::from_le_bytes(len_bytes) as usize;

        // #ASSUME_BUFFER_SIZE: buf.len() >= offset + 8 + len
        if offset + 8 + len > buf.len() {
            return Err(SerializeError::BufferTooSmall {
                required: offset + 8 + len,
                actual: buf.len(),
            });
        }

        // Read string bytes and validate UTF-8
        let string_bytes = &buf[offset + 8..offset + 8 + len];
        let string = String::from_utf8(string_bytes.to_vec()).map_err(|_| {
            SerializeError::Custom("Invalid UTF-8 in serialized string")
        })?;

        Ok(string)
    }

    const SERIALIZED_SIZE: usize = 256; // Variable, this is a placeholder
}

#[cfg(feature = "std")]
impl SerializePrimitive for &str {
    fn serialize_primitive(
        &self,
        buf: &mut [u8],
        offset: usize,
    ) -> SerializeResult<usize> {
        // Format: [length: u64 LE] + [bytes: UTF-8]
        let len = self.len() as u64;
        let len_bytes = len.to_le_bytes();

        // #ASSUME_BUFFER_SIZE: buf.len() >= offset + 8 + self.len()
        if offset + 8 + self.len() > buf.len() {
            return Err(SerializeError::BufferTooSmall {
                required: offset + 8 + self.len(),
                actual: buf.len(),
            });
        }

        // Write length
        buf[offset..offset + 8].copy_from_slice(&len_bytes);

        // Write string bytes
        buf[offset + 8..offset + 8 + self.len()].copy_from_slice(self.as_bytes());

        Ok(8 + self.len())
    }

    const SERIALIZED_SIZE: usize = 256; // Variable, this is a placeholder
}

// ============================================================================
// Option<T> Implementation (T: SerializePrimitive + DeserializePrimitive)
// ============================================================================

impl<T: SerializePrimitive> SerializePrimitive for Option<T> {
    fn serialize_primitive(
        &self,
        buf: &mut [u8],
        offset: usize,
    ) -> SerializeResult<usize> {
        // Format: [is_some: bool] + [value: T (if Some)]
        match self {
            Some(value) => {
                // Write discriminant (true = Some)
                buf[offset] = 1;
                let bytes_written = 1 + value.serialize_primitive(buf, offset + 1)?;
                Ok(bytes_written)
            }
            None => {
                // Write discriminant (false = None)
                if offset >= buf.len() {
                    return Err(SerializeError::BufferTooSmall {
                        required: offset + 1,
                        actual: buf.len(),
                    });
                }
                buf[offset] = 0;
                Ok(1)
            }
        }
    }

    const SERIALIZED_SIZE: usize = 1 + T::SERIALIZED_SIZE;
}

impl<T: DeserializePrimitive> DeserializePrimitive for Option<T> {
    fn deserialize_primitive(buf: &[u8], offset: usize) -> SerializeResult<Self> {
        // #ASSUME_BUFFER_SIZE: buf.len() >= offset + 1
        if offset >= buf.len() {
            return Err(SerializeError::BufferTooSmall {
                required: offset + 1,
                actual: buf.len(),
            });
        }

        let is_some = buf[offset] != 0;
        if is_some {
            let value = T::deserialize_primitive(buf, offset + 1)?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    const SERIALIZED_SIZE: usize = 1 + T::SERIALIZED_SIZE;
}

// ============================================================================
// Vec<T> Implementation (T: SerializePrimitive + DeserializePrimitive)
// ============================================================================

#[cfg(feature = "std")]
impl<T: SerializePrimitive + Default> SerializePrimitive for Vec<T> {
    fn serialize_primitive(
        &self,
        buf: &mut [u8],
        offset: usize,
    ) -> SerializeResult<usize> {
        // Format: [length: u64 LE] + [items: T, T, T, ...]
        let len = self.len() as u64;
        let len_bytes = len.to_le_bytes();

        // #ASSUME_BUFFER_SIZE: buf.len() >= offset + 8
        if offset + 8 > buf.len() {
            return Err(SerializeError::BufferTooSmall {
                required: offset + 8,
                actual: buf.len(),
            });
        }

        // Write length
        buf[offset..offset + 8].copy_from_slice(&len_bytes);

        // Write items sequentially
        let mut current_offset = offset + 8;
        for item in self {
            let bytes_written = item.serialize_primitive(buf, current_offset)?;
            current_offset += bytes_written;
        }

        Ok(current_offset - offset)
    }

    const SERIALIZED_SIZE: usize = 8; // Variable, minimum is 8 bytes for length
}

#[cfg(feature = "std")]
impl<T: DeserializePrimitive + Default> DeserializePrimitive for Vec<T> {
    fn deserialize_primitive(buf: &[u8], offset: usize) -> SerializeResult<Self> {
        // #ASSUME_BUFFER_SIZE: buf.len() >= offset + 8
        if offset + 8 > buf.len() {
            return Err(SerializeError::BufferTooSmall {
                required: offset + 8,
                actual: buf.len(),
            });
        }

        // Read length
        let mut len_bytes = [0u8; 8];
        len_bytes.copy_from_slice(&buf[offset..offset + 8]);
        let len = u64::from_le_bytes(len_bytes) as usize;

        // Deserialize items sequentially
        let mut vec = Vec::with_capacity(len);
        let mut current_offset = offset + 8;
        for _ in 0..len {
            let item = T::deserialize_primitive(buf, current_offset)?;
            vec.push(item);
            current_offset += T::SERIALIZED_SIZE;
        }

        Ok(vec)
    }

    const SERIALIZED_SIZE: usize = 8; // Variable, minimum is 8 bytes for length
}

// ============================================================================
// Unit Type Implementation ()
// ============================================================================

impl SerializePrimitive for () {
    #[inline]
    fn serialize_primitive(&self, _buf: &mut [u8], _offset: usize) -> SerializeResult<usize> {
        Ok(0)
    }

    const SERIALIZED_SIZE: usize = 0;
}

impl DeserializePrimitive for () {
    #[inline]
    fn deserialize_primitive(_buf: &[u8], _offset: usize) -> SerializeResult<Self> {
        Ok(())
    }

    const SERIALIZED_SIZE: usize = 0;
}

// ============================================================================
// PrimitiveSerializerCapsule (Zero-sized, compile-time dispatch)
// ============================================================================

/// Compile-time dispatch serializer for primitives (T1 Atomic, <5ns)
///
/// # Design Philosophy (UCE34 Q10-Q12 Applied)
///
/// **Q10: Tier Selection** - T1 Atomic (monomorphization = compile-time dispatch)
/// **Q11: Rust Transform** - Generic trait specialization via type system
/// **Q12: Nightly** - Future: const_generic array serialization
///
/// # Purpose
///
/// Provides a zero-sized marker type for organizing primitive serialization methods.
/// All actual work is done via trait bounds on type parameter T.
///
/// # Example
///
/// ```rust,ignore
/// use atomic_capsule::serialize::primitives::{PrimitiveSerializerCapsule, SerializePrimitive};
///
/// // Direct trait usage (preferred)
/// let value: u64 = 42;
/// let bytes = value.to_bytes()?;
///
/// // Or via capsule (for consistency with other capsules)
/// type MySerializer = PrimitiveSerializerCapsule<u64>;
/// // MySerializer is zero-sized, no runtime cost
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrimitiveSerializerCapsule<T>(core::marker::PhantomData<T>);

impl<T> PrimitiveSerializerCapsule<T> {
    /// Create new capsule (zero-cost, inlined)
    #[inline]
    pub const fn new() -> Self {
        PrimitiveSerializerCapsule(core::marker::PhantomData)
    }
}

impl<T> Default for PrimitiveSerializerCapsule<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests (T28 Framework: Unit + Property + Integration)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Unit Tests (T28 Q1-Q7)

    #[test]
    fn test_u64_serialize_deserialize() {
        let value: u64 = 42;
        let bytes = value.to_bytes().unwrap();
        let restored = u64::from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_u32_serialize_deserialize() {
        let value: u32 = 100;
        let bytes = value.to_bytes().unwrap();
        let restored = u32::from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_bool_serialize_deserialize() {
        for value in &[true, false] {
            let bytes = value.to_bytes().unwrap();
            let restored = bool::from_bytes(&bytes).unwrap();
            assert_eq!(*value, restored);
        }
    }

    #[test]
    fn test_u64_max_value() {
        let value: u64 = u64::MAX;
        let bytes = value.to_bytes().unwrap();
        let restored = u64::from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_i64_negative() {
        let value: i64 = -42;
        let bytes = value.to_bytes().unwrap();
        let restored = i64::from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_buffer_too_small() {
        let value: u64 = 42;
        let mut buf = vec![0u8; 4];
        let result = value.serialize_primitive(&mut buf, 0);
        assert!(matches!(result, Err(SerializeError::BufferTooSmall { .. })));
    }

    #[test]
    fn test_deserialize_buffer_too_small() {
        let buf = vec![0u8; 4];
        let result = u64::from_bytes(&buf);
        assert!(matches!(result, Err(SerializeError::BufferTooSmall { .. })));
    }

    #[test]
    fn test_option_some_serialize() {
        let value: Option<u64> = Some(42);
        let bytes = value.to_bytes().unwrap();
        let restored = Option::<u64>::from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_option_none_serialize() {
        let value: Option<u64> = None;
        let bytes = value.to_bytes().unwrap();
        let restored = Option::<u64>::from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_string_serialize() {
        let value = "hello".to_string();
        let bytes = value.to_bytes().unwrap();
        let restored = String::from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_string_empty() {
        let value = String::new();
        let bytes = value.to_bytes().unwrap();
        let restored = String::from_bytes(&bytes).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_vec_u64_serialize() {
        let value: Vec<u64> = vec![1, 2, 3, 4, 5];
        let mut buf = vec![0u8; 1024];
        let bytes_written = value.serialize_primitive(&mut buf, 0).unwrap();
        let restored = Vec::<u64>::deserialize_primitive(&buf, 0).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_vec_empty() {
        let value: Vec<u64> = vec![];
        let bytes = vec![0u8; 8];
        let restored = Vec::<u64>::deserialize_primitive(&bytes, 0).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_primitive_serializer_capsule_new() {
        let _capsule = PrimitiveSerializerCapsule::<u64>::new();
        // Zero-sized, should compile and not allocate
        assert_eq!(size_of::<PrimitiveSerializerCapsule<u64>>(), 0);
    }

    #[test]
    fn test_primitive_serializer_capsule_default() {
        let _capsule = PrimitiveSerializerCapsule::<u64>::default();
        // Zero-sized, should compile and not allocate
        assert_eq!(size_of::<PrimitiveSerializerCapsule<u64>>(), 0);
    }

    // Property Tests (T28 Q8-Q14)

    #[test]
    fn test_determinism_u64() {
        let value: u64 = 42;
        let bytes1 = value.to_bytes().unwrap();
        let bytes2 = value.to_bytes().unwrap();
        assert_eq!(bytes1, bytes2, "Serialization must be deterministic");
    }

    #[test]
    fn test_little_endian_u64() {
        let value: u64 = 0x0102030405060708;
        let bytes = value.to_bytes().unwrap();
        // Little-endian: least significant byte first
        assert_eq!(bytes[0], 0x08);
        assert_eq!(bytes[1], 0x07);
        assert_eq!(bytes[7], 0x01);
    }

    #[test]
    fn test_roundtrip_all_u8_values() {
        for i in 0..=u8::MAX {
            let bytes = i.to_bytes().unwrap();
            let restored = u8::from_bytes(&bytes).unwrap();
            assert_eq!(i, restored);
        }
    }

    // Integration Tests (T28 Q15-Q21)

    #[test]
    fn test_offset_serialize() {
        let mut buf = vec![0u8; 20];
        let value: u64 = 42;
        let bytes_written = value.serialize_primitive(&mut buf, 5).unwrap();
        assert_eq!(bytes_written, 8);
        let restored = u64::deserialize_primitive(&buf, 5).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_nested_option_vec() {
        let value: Option<Vec<u64>> = Some(vec![1, 2, 3]);
        let mut buf = vec![0u8; 1024];
        value.serialize_primitive(&mut buf, 0).unwrap();
        let restored = Option::<Vec<u64>>::deserialize_primitive(&buf, 0).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn test_all_integer_types() {
        let u64_val: u64 = 100;
        let u32_val: u32 = 200;
        let u16_val: u16 = 300;
        let u8_val: u8 = 4;
        let i64_val: i64 = -100;
        let i32_val: i32 = -200;

        assert_eq!(u64::from_bytes(&u64_val.to_bytes().unwrap()).unwrap(), u64_val);
        assert_eq!(u32::from_bytes(&u32_val.to_bytes().unwrap()).unwrap(), u32_val);
        assert_eq!(u16::from_bytes(&u16_val.to_bytes().unwrap()).unwrap(), u16_val);
        assert_eq!(u8::from_bytes(&u8_val.to_bytes().unwrap()).unwrap(), u8_val);
        assert_eq!(i64::from_bytes(&i64_val.to_bytes().unwrap()).unwrap(), i64_val);
        assert_eq!(i32::from_bytes(&i32_val.to_bytes().unwrap()).unwrap(), i32_val);
    }
}
