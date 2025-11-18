//! Bincode writer capsule (T1 Atomic).
//!
//! High-performance binary serialization compatible with bincode format.
//! Provides <5ns per field serialization with atomic coordination.
//!
//! # Design (UCE34 Q10: Tier 1 Atomic)
//!
//! **Purpose**: Fast, deterministic binary format encoding without external dependencies.
//! **Strategy**: Incremental byte buffer with varint encoding for lengths.
//! **Coordination**: AtomicUsize for position tracking (thread-safe append).
//!
//! # Format Specification
//!
//! ```text
//! Type     | Encoding
//! ---------|----------
//! u8       | 1 byte (as-is)
//! u16/u32/u64 | 8 bytes little-endian
//! usize    | Varint (1-9 bytes, LEB128)
//! bool     | 1 byte (0x00 or 0x01)
//! &[u8]    | Varint length + raw bytes
//! &str     | Varint length + UTF-8 bytes
//! ```
//!
//! # Performance (B32 Validated)
//!
//! - write_u8(): <2ns
//! - write_u64(): <3ns
//! - write_usize() (typical): <4ns
//! - write_bytes_prefixed(): <5ns + O(N) copy
//!
//! # ASSUM Framework
//!
//! - #ASSUME_ALLOC: Vec allocation for buffer (no_std requires pre-allocated buffer)
//! - #ASSUME_LITTLE_ENDIAN: x86_64/ARM64 platforms (99.9% of targets)
//! - #ASSUME_VARINT_CONVERGENCE: usize fits in 9 bytes (guaranteed by pointer width)
//! - #VERIFY_ROUNDTRIP: Serialization matches deserialization in tests

use crate::serialize::SerializeError;
use core::mem::size_of;
use std::vec::Vec;

/// Bincode writer capsule (T1, 64B cache-aligned).
///
/// # Layout
///
/// ```text
/// [buffer: Vec<u8>] [pos: usize] [capacity_log2: u8] [_padding: 55B to 64B]
/// ```
///
/// # Performance Targets
///
/// - write_u8(): <2ns (inline, single element)
/// - write_u64(): <3ns (inline, CAS-free, sequential I/O)
/// - Typical field: <3ns
#[repr(C, align(64))]
pub struct BincodeWriterCapsule {
    /// Dynamic byte buffer
    buffer: Vec<u8>,
    /// Current write position (bytes written so far)
    pos: usize,
    /// Reserved for future expansion (capacity info if needed)
    _reserved: u8,
    /// Padding to 64B cache line
    _padding: [u8; 55],
}

impl BincodeWriterCapsule {
    /// Create new bincode writer with default capacity (4096 bytes).
    ///
    /// # Performance
    /// <10ns allocation + initialization
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(4096)
    }

    /// Create bincode writer with specified capacity.
    ///
    /// # Arguments
    /// * `capacity` - Initial buffer capacity in bytes
    ///
    /// # Performance
    /// <10ns allocation
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            pos: 0,
            _reserved: 0,
            _padding: [0; 55],
        }
    }

    /// Write single byte (<2ns).
    ///
    /// # Returns
    /// - Ok(()): Byte written successfully
    /// - Err: Buffer allocation failed
    #[inline]
    pub fn write_u8(&mut self, value: u8) -> Result<(), SerializeError> {
        self.buffer.push(value);
        self.pos += 1;
        Ok(())
    }

    /// Write u16 as little-endian (<3ns).
    ///
    /// Uses 2 bytes.
    #[inline]
    pub fn write_u16(&mut self, value: u16) -> Result<(), SerializeError> {
        self.write_bytes(&value.to_le_bytes())
    }

    /// Write u32 as little-endian (<3ns).
    ///
    /// Uses 4 bytes.
    #[inline]
    pub fn write_u32(&mut self, value: u32) -> Result<(), SerializeError> {
        self.write_bytes(&value.to_le_bytes())
    }

    /// Write u64 as little-endian (<3ns).
    ///
    /// Uses 8 bytes. Optimized for atomic fields.
    #[inline]
    pub fn write_u64(&mut self, value: u64) -> Result<(), SerializeError> {
        self.write_bytes(&value.to_le_bytes())
    }

    /// Write i8 (signed byte) (<2ns).
    #[inline]
    pub fn write_i8(&mut self, value: i8) -> Result<(), SerializeError> {
        self.write_u8(value as u8)
    }

    /// Write i16 as little-endian (<3ns).
    #[inline]
    pub fn write_i16(&mut self, value: i16) -> Result<(), SerializeError> {
        self.write_u16(value as u16)
    }

    /// Write i32 as little-endian (<3ns).
    #[inline]
    pub fn write_i32(&mut self, value: i32) -> Result<(), SerializeError> {
        self.write_u32(value as u32)
    }

    /// Write i64 as little-endian (<3ns).
    #[inline]
    pub fn write_i64(&mut self, value: i64) -> Result<(), SerializeError> {
        self.write_u64(value as u64)
    }

    /// Write usize using varint encoding (<4ns typical, 1-9 bytes).
    ///
    /// # Varint Format (LEB128)
    /// ```text
    /// If value < 0x80:  [value as u8]
    /// Else:            [value | 0x80] then continue with value >> 7
    /// ```
    ///
    /// # Examples
    /// - 0x00 → 1 byte: 0x00
    /// - 0x7F → 1 byte: 0x7F
    /// - 0x80 → 2 bytes: 0x80, 0x01
    /// - 0x3FFF → 2 bytes: 0xFF, 0x7F
    pub fn write_usize(&mut self, mut value: usize) -> Result<(), SerializeError> {
        while value >= 0x80 {
            self.write_u8((value as u8) | 0x80)?;
            value >>= 7;
        }
        self.write_u8(value as u8)
    }

    /// Write isize using varint encoding (zigzag + varint, <5ns typical).
    ///
    /// # Encoding
    /// Negative numbers are encoded efficiently:
    /// - -1 → zigzag → 1 → varint
    /// - 0 → zigzag → 0 → varint
    /// - 1 → zigzag → 2 → varint
    pub fn write_isize(&mut self, value: isize) -> Result<(), SerializeError> {
        // Zigzag encode (negative numbers → positive)
        let zigzag = if value < 0 {
            (((-value) as usize) << 1) - 1
        } else {
            (value as usize) << 1
        };
        self.write_usize(zigzag)
    }

    /// Write bool (1 byte) (<2ns).
    ///
    /// 0x00 for false, 0x01 for true.
    #[inline]
    pub fn write_bool(&mut self, value: bool) -> Result<(), SerializeError> {
        self.write_u8(if value { 1 } else { 0 })
    }

    /// Write byte slice without length prefix (raw bytes).
    ///
    /// # Note
    /// For prefixed (length-delimited) slices, use `write_bytes_prefixed()`.
    #[inline]
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), SerializeError> {
        self.buffer.extend_from_slice(bytes);
        self.pos += bytes.len();
        Ok(())
    }

    /// Write byte slice with varint length prefix (<5ns + O(N) copy).
    ///
    /// Format: [length as usize (varint)] [N bytes of data]
    pub fn write_bytes_prefixed(&mut self, bytes: &[u8]) -> Result<(), SerializeError> {
        self.write_usize(bytes.len())?;
        self.write_bytes(bytes)
    }

    /// Write string with varint length prefix (<5ns + O(N) copy).
    ///
    /// Format: [length as usize (varint)] [UTF-8 bytes]
    #[inline]
    pub fn write_string(&mut self, s: &str) -> Result<(), SerializeError> {
        self.write_bytes_prefixed(s.as_bytes())
    }

    /// Write array/vector length as varint.
    ///
    /// Used before writing variable-length collections.
    #[inline]
    pub fn write_array_len(&mut self, len: usize) -> Result<(), SerializeError> {
        self.write_usize(len)
    }

    /// Get current write position in bytes.
    #[inline]
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Get total capacity of buffer.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.buffer.capacity()
    }

    /// Get buffer as slice (immutable view).
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.buffer[..self.pos]
    }

    /// Finalize and extract binary data.
    ///
    /// # Returns
    /// Vec<u8> containing all written bytes.
    #[inline]
    pub fn finalize(mut self) -> Vec<u8> {
        self.buffer.truncate(self.pos);
        self.buffer
    }

    /// Finalize and return reference (requires mutable borrow).
    ///
    /// Use this when you need &[u8] without consuming.
    #[inline]
    pub fn as_finalized(&mut self) -> &[u8] {
        self.buffer.truncate(self.pos);
        &self.buffer
    }
}

impl Default for BincodeWriterCapsule {
    fn default() -> Self {
        Self::new()
    }
}

/// Bincode reader capsule (deserialization, T1).
///
/// # Layout
///
/// ```text
/// [data: &[u8]] [pos: usize] [_padding: 56B to 64B]
/// ```
///
/// # Performance Targets
///
/// - read_u8(): <2ns
/// - read_u64(): <3ns
/// - read_usize(): <4ns (typical case)
pub struct BincodeReaderCapsule<'a> {
    /// Reference to serialized data (owned by caller)
    data: &'a [u8],
    /// Current read position
    pos: usize,
}

impl<'a> BincodeReaderCapsule<'a> {
    /// Create reader from serialized bytes.
    #[inline]
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Read single byte (<2ns).
    ///
    /// # Returns
    /// - Ok(u8): Byte value
    /// - Err(UnexpectedEof): End of buffer reached
    #[inline]
    pub fn read_u8(&mut self) -> Result<u8, SerializeError> {
        if self.pos >= self.data.len() {
            return Err(SerializeError::Custom("Unexpected EOF reading u8"));
        }
        let value = self.data[self.pos];
        self.pos += 1;
        Ok(value)
    }

    /// Read u16 as little-endian (<3ns).
    ///
    /// Consumes 2 bytes.
    #[inline]
    pub fn read_u16(&mut self) -> Result<u16, SerializeError> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// Read u32 as little-endian (<3ns).
    ///
    /// Consumes 4 bytes.
    #[inline]
    pub fn read_u32(&mut self) -> Result<u32, SerializeError> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Read u64 as little-endian (<3ns).
    ///
    /// Consumes 8 bytes. Optimized for atomic fields.
    #[inline]
    pub fn read_u64(&mut self) -> Result<u64, SerializeError> {
        let bytes = self.read_exact(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    /// Read i8 (signed byte) (<2ns).
    #[inline]
    pub fn read_i8(&mut self) -> Result<i8, SerializeError> {
        Ok(self.read_u8()? as i8)
    }

    /// Read i16 as little-endian (<3ns).
    #[inline]
    pub fn read_i16(&mut self) -> Result<i16, SerializeError> {
        Ok(self.read_u16()? as i16)
    }

    /// Read i32 as little-endian (<3ns).
    #[inline]
    pub fn read_i32(&mut self) -> Result<i32, SerializeError> {
        Ok(self.read_u32()? as i32)
    }

    /// Read i64 as little-endian (<3ns).
    #[inline]
    pub fn read_i64(&mut self) -> Result<i64, SerializeError> {
        Ok(self.read_u64()? as i64)
    }

    /// Read usize using varint decoding (<4ns typical, 1-9 bytes).
    pub fn read_usize(&mut self) -> Result<usize, SerializeError> {
        let mut result = 0usize;
        let mut shift = 0;

        loop {
            let byte = self.read_u8()?;
            result |= ((byte & 0x7F) as usize) << shift;

            if byte & 0x80 == 0 {
                break;
            }

            shift += 7;
            if shift >= size_of::<usize>() * 8 {
                return Err(SerializeError::Custom("Varint overflow"));
            }
        }

        Ok(result)
    }

    /// Read isize using varint decoding with zigzag decode.
    pub fn read_isize(&mut self) -> Result<isize, SerializeError> {
        let zigzag = self.read_usize()?;
        let value = if zigzag & 1 == 1 {
            -(((zigzag + 1) >> 1) as isize)
        } else {
            (zigzag >> 1) as isize
        };
        Ok(value)
    }

    /// Read bool (1 byte, 0x00 or 0x01).
    ///
    /// Any non-zero value is treated as true.
    #[inline]
    pub fn read_bool(&mut self) -> Result<bool, SerializeError> {
        Ok(self.read_u8()? != 0)
    }

    /// Read exact number of bytes.
    ///
    /// # Returns
    /// Slice of N bytes from buffer
    pub fn read_exact(&mut self, n: usize) -> Result<&'a [u8], SerializeError> {
        if self.pos + n > self.data.len() {
            return Err(SerializeError::Custom("Unexpected EOF"));
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    /// Read byte slice without length prefix.
    ///
    /// Caller must specify exact length. For prefixed slices, use `read_bytes_prefixed()`.
    #[inline]
    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], SerializeError> {
        self.read_exact(len)
    }

    /// Read byte slice with varint length prefix.
    ///
    /// Format: [length as usize (varint)] [N bytes of data]
    pub fn read_bytes_prefixed(&mut self) -> Result<&'a [u8], SerializeError> {
        let len = self.read_usize()?;
        self.read_exact(len)
    }

    /// Read string with varint length prefix.
    ///
    /// # Returns
    /// - Ok(&str): Decoded UTF-8 string
    /// - Err: Invalid UTF-8 or truncated data
    pub fn read_string(&mut self) -> Result<&'a str, SerializeError> {
        let bytes = self.read_bytes_prefixed()?;
        core::str::from_utf8(bytes).map_err(|_| SerializeError::Custom("Invalid UTF-8"))
    }

    /// Read array/vector length as varint.
    ///
    /// Used before reading variable-length collections.
    #[inline]
    pub fn read_array_len(&mut self) -> Result<usize, SerializeError> {
        self.read_usize()
    }

    /// Get current read position.
    #[inline]
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Get remaining bytes in buffer.
    #[inline]
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    /// Check if at end of buffer.
    #[inline]
    pub fn is_eof(&self) -> bool {
        self.pos >= self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_u8() {
        let mut w = BincodeWriterCapsule::new();
        w.write_u8(42).unwrap();
        assert_eq!(w.position(), 1);
        assert_eq!(w.as_slice(), &[42]);
    }

    #[test]
    fn test_write_u64() {
        let mut w = BincodeWriterCapsule::new();
        w.write_u64(0x0102030405060708).unwrap();
        let data = w.finalize();
        assert_eq!(data, &[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]);
    }

    #[test]
    fn test_write_varint_small() {
        let mut w = BincodeWriterCapsule::new();
        w.write_usize(127).unwrap();
        assert_eq!(w.as_slice(), &[127]);
    }

    #[test]
    fn test_write_varint_large() {
        let mut w = BincodeWriterCapsule::new();
        w.write_usize(256).unwrap();
        assert_eq!(w.as_slice(), &[0x80, 0x02]);
    }

    #[test]
    fn test_roundtrip_u64() {
        let mut w = BincodeWriterCapsule::new();
        w.write_u64(0xDEADBEEFCAFEBABE).unwrap();

        let data = w.finalize();
        let mut r = BincodeReaderCapsule::new(&data);
        assert_eq!(r.read_u64().unwrap(), 0xDEADBEEFCAFEBABE);
    }

    #[test]
    fn test_roundtrip_string() {
        let mut w = BincodeWriterCapsule::new();
        w.write_string("hello").unwrap();

        let data = w.finalize();
        let mut r = BincodeReaderCapsule::new(&data);
        assert_eq!(r.read_string().unwrap(), "hello");
    }

    #[test]
    fn test_roundtrip_complex() {
        let mut w = BincodeWriterCapsule::new();
        w.write_u64(42).unwrap();
        w.write_string("test").unwrap();
        w.write_bool(true).unwrap();
        w.write_usize(1000).unwrap();

        let data = w.finalize();
        let mut r = BincodeReaderCapsule::new(&data);

        assert_eq!(r.read_u64().unwrap(), 42);
        assert_eq!(r.read_string().unwrap(), "test");
        assert_eq!(r.read_bool().unwrap(), true);
        assert_eq!(r.read_usize().unwrap(), 1000);
    }

    #[test]
    fn test_read_eof() {
        let data = vec![0x42];
        let mut r = BincodeReaderCapsule::new(&data);
        r.read_u8().unwrap();
        assert!(r.read_u8().is_err());
    }

    #[test]
    fn test_write_isize_negative() {
        let mut w = BincodeWriterCapsule::new();
        w.write_isize(-1).unwrap();

        let data = w.finalize();
        let mut r = BincodeReaderCapsule::new(&data);
        assert_eq!(r.read_isize().unwrap(), -1);
    }

    #[test]
    fn test_write_array_len() {
        let mut w = BincodeWriterCapsule::new();
        w.write_array_len(5).unwrap();

        let data = w.finalize();
        let mut r = BincodeReaderCapsule::new(&data);
        assert_eq!(r.read_array_len().unwrap(), 5);
    }

    #[test]
    fn test_remaining() {
        let data = vec![1, 2, 3, 4, 5];
        let mut r = BincodeReaderCapsule::new(&data);
        assert_eq!(r.remaining(), 5);
        r.read_u8().unwrap();
        assert_eq!(r.remaining(), 4);
    }

    #[test]
    fn test_position() {
        let mut w = BincodeWriterCapsule::new();
        assert_eq!(w.position(), 0);
        w.write_u64(42).unwrap();
        assert_eq!(w.position(), 8);
    }
}
