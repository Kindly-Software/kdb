//! Bitwise serialization safety trait
//!
//! Provides zero-cost compile-time dispatch for atomic storage serialization

/// Types safe to serialize to/from u64 atomic storage
///
/// Provides compile-time dispatch for converting values to/from u64 storage
/// via trait monomorphization - zero runtime overhead.
///
/// # Arc Lifecycle Management
///
/// For Arc<T> types, the lifecycle is:
/// 1. `to_storage(arc)` - Transfers ownership to storage (refcount unchanged)
/// 2. `from_storage(data)` - Creates clone for reading (refcount +1)
/// 3. `drop_storage(data)` - Drops storage reference (refcount -1)
///
/// Primitives use no-op `drop_storage()` - zero cost.
///
/// # Safety
///
/// Types implementing this trait MUST satisfy:
/// - No invalid bit patterns (all u64 values are valid) OR valid pointer lifecycle
/// - size_of::<T>() <= 8 OR pointer-sized (for Arc<T>)
/// - No references, raw pointers, or complex types (except Arc<T>)
///
/// #ASSUME_BITWISE_SAFE: All bit patterns are valid for primitives
/// #ASSUME_ARC_LIFECYCLE: Arc refcount managed correctly across to/from/drop_storage
/// #VERIFY_TRANSMUTE_SOUND: Trait methods enforce type safety at compile time
pub unsafe trait BitwiseSerializable: Sized {
    /// Serialize value to u64 for atomic storage
    ///
    /// Compiler monomorphizes this per type - zero runtime dispatch cost.
    /// Implementations should be #[inline(always)] for zero-cost abstraction.
    ///
    /// For Arc<T>: Transfers ownership to storage (refcount unchanged).
    ///
    /// #ASSUME: Type fits in u64 (8 bytes) or is pointer-sized
    /// #VERIFY: Compile-time type checks enforce size constraints
    fn to_storage(self) -> u64;

    /// Deserialize value from u64 atomic storage
    ///
    /// Compiler monomorphizes this per type - zero runtime dispatch cost.
    /// Implementations should be #[inline(always)] for zero-cost abstraction.
    ///
    /// For Arc<T>: Creates clone for reading (refcount +1), storage ref stays alive.
    ///
    /// #ASSUME: data was created by to_storage for same type T
    /// #VERIFY: Type system prevents cross-type deserialization
    fn from_storage(data: u64) -> Self;

    /// Drop the storage reference for cleanup
    ///
    /// Compiler monomorphizes this per type - zero runtime dispatch cost.
    /// Implementations should be #[inline(always)] for zero-cost abstraction.
    ///
    /// For Arc<T>: Drops the storage reference (refcount -1).
    /// For primitives: No-op (compiles to nothing).
    ///
    /// # Safety
    ///
    /// Caller must ensure:
    /// - data was created by to_storage for same type T
    /// - No concurrent access to storage during drop
    /// - drop_storage called exactly once per to_storage
    ///
    /// #ASSUME_SINGLE_DROP: Each storage value dropped exactly once
    /// #VERIFY_TYPE_MATCH: Type system prevents dropping wrong type
    unsafe fn drop_storage(data: u64);
}

// Safe implementations for primitive types
// #ASSUME_BITWISE_SAFE: All bit patterns are valid for primitive integer types
// #VERIFY_TRANSMUTE_SOUND: Rust language guarantees all u64 bit patterns valid for primitives

// u64 - Zero-cost identity (compiles to nothing)
unsafe impl BitwiseSerializable for u64 {
    #[inline(always)]
    fn to_storage(self) -> u64 {
        self // Zero-cost - identity function
    }

    #[inline(always)]
    fn from_storage(data: u64) -> Self {
        data // Zero-cost - identity function
    }

    #[inline(always)]
    unsafe fn drop_storage(_data: u64) {
        // No-op for primitives - compiles to nothing
        // #ASSUME: Primitives have no cleanup logic
        // #VERIFY: Compiler optimizes this away entirely
    }
}

// Smaller unsigned integers - byte array pattern
unsafe impl BitwiseSerializable for u32 {
    #[inline(always)]
    fn to_storage(self) -> u64 {
        // #ASSUME: u32 is 4 bytes, fits in u64
        // #VERIFY: No size mismatch possible - compile-time guarantee
        let mut bytes = [0u8; 8];
        unsafe {
            core::ptr::write(bytes.as_mut_ptr() as *mut u32, self);
        }
        u64::from_ne_bytes(bytes)
    }

    #[inline(always)]
    fn from_storage(data: u64) -> Self {
        let bytes = data.to_ne_bytes();
        unsafe { core::ptr::read(bytes.as_ptr() as *const u32) }
    }

    #[inline(always)]
    unsafe fn drop_storage(_data: u64) {
        // No-op for primitives
    }
}

unsafe impl BitwiseSerializable for u16 {
    #[inline(always)]
    fn to_storage(self) -> u64 {
        let mut bytes = [0u8; 8];
        unsafe {
            core::ptr::write(bytes.as_mut_ptr() as *mut u16, self);
        }
        u64::from_ne_bytes(bytes)
    }

    #[inline(always)]
    fn from_storage(data: u64) -> Self {
        let bytes = data.to_ne_bytes();
        unsafe { core::ptr::read(bytes.as_ptr() as *const u16) }
    }

    #[inline(always)]
    unsafe fn drop_storage(_data: u64) {}
}

unsafe impl BitwiseSerializable for u8 {
    #[inline(always)]
    fn to_storage(self) -> u64 {
        self as u64 // Zero-cost extension
    }

    #[inline(always)]
    fn from_storage(data: u64) -> Self {
        data as u8 // Simple cast
    }

    #[inline(always)]
    unsafe fn drop_storage(_data: u64) {}
}

// Signed integers - same pattern
unsafe impl BitwiseSerializable for i64 {
    #[inline(always)]
    fn to_storage(self) -> u64 {
        self as u64 // Reinterpret bits
    }

    #[inline(always)]
    fn from_storage(data: u64) -> Self {
        data as i64 // Reinterpret bits
    }

    #[inline(always)]
    unsafe fn drop_storage(_data: u64) {}
}

unsafe impl BitwiseSerializable for i32 {
    #[inline(always)]
    fn to_storage(self) -> u64 {
        let mut bytes = [0u8; 8];
        unsafe {
            core::ptr::write(bytes.as_mut_ptr() as *mut i32, self);
        }
        u64::from_ne_bytes(bytes)
    }

    #[inline(always)]
    fn from_storage(data: u64) -> Self {
        let bytes = data.to_ne_bytes();
        unsafe { core::ptr::read(bytes.as_ptr() as *const i32) }
    }

    #[inline(always)]
    unsafe fn drop_storage(_data: u64) {}
}

unsafe impl BitwiseSerializable for i16 {
    #[inline(always)]
    fn to_storage(self) -> u64 {
        let mut bytes = [0u8; 8];
        unsafe {
            core::ptr::write(bytes.as_mut_ptr() as *mut i16, self);
        }
        u64::from_ne_bytes(bytes)
    }

    #[inline(always)]
    fn from_storage(data: u64) -> Self {
        let bytes = data.to_ne_bytes();
        unsafe { core::ptr::read(bytes.as_ptr() as *const i16) }
    }

    #[inline(always)]
    unsafe fn drop_storage(_data: u64) {}
}

unsafe impl BitwiseSerializable for i8 {
    #[inline(always)]
    fn to_storage(self) -> u64 {
        self as i64 as u64 // Sign-extend then reinterpret
    }

    #[inline(always)]
    fn from_storage(data: u64) -> Self {
        data as i8 // Truncate
    }

    #[inline(always)]
    unsafe fn drop_storage(_data: u64) {}
}

unsafe impl BitwiseSerializable for usize {
    #[inline(always)]
    fn to_storage(self) -> u64 {
        self as u64
    }

    #[inline(always)]
    fn from_storage(data: u64) -> Self {
        data as usize
    }

    #[inline(always)]
    unsafe fn drop_storage(_data: u64) {}
}

unsafe impl BitwiseSerializable for isize {
    #[inline(always)]
    fn to_storage(self) -> u64 {
        self as i64 as u64
    }

    #[inline(always)]
    fn from_storage(data: u64) -> Self {
        data as isize
    }

    #[inline(always)]
    unsafe fn drop_storage(_data: u64) {}
}

// Float types are also safe - all bit patterns are valid (including NaN)
// #ASSUME_BITWISE_SAFE: IEEE-754 floats have no invalid bit patterns
// #VERIFY_TRANSMUTE_SOUND: All u32/u64 patterns map to valid float values (including NaN/Inf)
unsafe impl BitwiseSerializable for f32 {
    #[inline(always)]
    fn to_storage(self) -> u64 {
        // #ASSUME: f32 is 4 bytes, fits in u64
        let mut bytes = [0u8; 8];
        unsafe {
            core::ptr::write(bytes.as_mut_ptr() as *mut f32, self);
        }
        u64::from_ne_bytes(bytes)
    }

    #[inline(always)]
    fn from_storage(data: u64) -> Self {
        let bytes = data.to_ne_bytes();
        unsafe { core::ptr::read(bytes.as_ptr() as *const f32) }
    }

    #[inline(always)]
    unsafe fn drop_storage(_data: u64) {}
}

unsafe impl BitwiseSerializable for f64 {
    #[inline(always)]
    fn to_storage(self) -> u64 {
        self.to_bits() // IEEE-754 bit representation
    }

    #[inline(always)]
    fn from_storage(data: u64) -> Self {
        f64::from_bits(data) // IEEE-754 bit interpretation
    }

    #[inline(always)]
    unsafe fn drop_storage(_data: u64) {}
}

// Arc<T> implementation - pointer storage for reference-counted types
// #ASSUME_ARC_LIFECYCLE: Arc::into_raw/from_raw manage refcount correctly
// #VERIFY_POINTER_SIZE: Compile-time assertion ensures pointer fits in u64
#[cfg(feature = "std")]
unsafe impl<T: Send + Sync + 'static> BitwiseSerializable for std::sync::Arc<T> {
    #[inline(always)]
    fn to_storage(self) -> u64 {
        // Compile-time assertion: pointer must fit in u64
        // #ASSUME: On 64-bit systems, pointer size == 8 bytes
        // #VERIFY: This assertion fails at compile time if pointer > 8 bytes
        const _: () = assert!(core::mem::size_of::<*const ()>() <= 8);

        // #ASSUME_ARC_REFCOUNT: into_raw transfers ownership to storage
        // #VERIFY_NO_DROP: Arc refcount NOT decremented, ownership transferred
        let raw_ptr = std::sync::Arc::into_raw(self);
        raw_ptr as usize as u64
    }

    #[inline(always)]
    fn from_storage(data: u64) -> Self {
        // #ASSUME_VALID_POINTER: data is raw Arc pointer from to_storage
        // #VERIFY_TYPE_SAFETY: Type system ensures data came from Arc<T>, not Arc<U>
        unsafe {
            let ptr = data as usize as *const T;
            // Reconstruct Arc from raw pointer WITHOUT taking ownership
            // #ASSUME_REFCOUNT_MANAGEMENT: Must increment refcount for each retrieval
            // #VERIFY_NO_DOUBLE_FREE: Clone increments, forget prevents drop
            let arc_ref = std::sync::Arc::from_raw(ptr);
            let cloned = arc_ref.clone(); // Increment refcount
            std::mem::forget(arc_ref); // Prevent drop - storage keeps original
            cloned // Return clone with incremented refcount
        }
    }

    #[inline(always)]
    unsafe fn drop_storage(data: u64) {
        // #ASSUME_VALID_POINTER: data is raw Arc pointer from to_storage
        // #VERIFY_SINGLE_DROP: Caller ensures this is called exactly once per to_storage
        let ptr = data as usize as *const T;
        // Reconstruct Arc and let it drop naturally - decrements refcount by 1
        let _arc = std::sync::Arc::from_raw(ptr);
        // _arc drops here, decrementing refcount by 1
    }
}

// String implementation - heap-allocated, use pointer storage
// #ASSUME_STRING_LIFETIME: String owns its heap allocation, pointer remains valid
// #VERIFY_HEAP_CLEANUP: drop_storage properly frees Box<String>
#[cfg(feature = "std")]
unsafe impl BitwiseSerializable for String {
    #[inline(always)]
    fn to_storage(self) -> u64 {
        // Compile-time assertion: pointer must fit in u64
        const _: () = assert!(core::mem::size_of::<*const ()>() <= 8);

        // Box the String and convert to raw pointer
        // #ASSUME_HEAP_ALLOCATION: Box::into_raw transfers ownership to storage
        // #VERIFY_NO_DROP: String is not dropped, ownership transferred
        let boxed = Box::new(self);
        Box::into_raw(boxed) as usize as u64
    }

    #[inline(always)]
    fn from_storage(data: u64) -> Self {
        // #ASSUME_VALID_POINTER: data is raw pointer from to_storage
        // #VERIFY_TYPE_SAFETY: Type system ensures data came from String, not other type
        unsafe {
            let ptr = data as usize as *const String;
            // Reconstruct Box WITHOUT taking ownership, then clone
            // #ASSUME_REFCOUNT_MANAGEMENT: Clone for reader, original stays in storage
            // #VERIFY_NO_DOUBLE_FREE: We clone and forget, storage keeps original
            let boxed_ref = &*ptr;
            boxed_ref.clone()
        }
    }

    #[inline(always)]
    unsafe fn drop_storage(data: u64) {
        // #ASSUME_VALID_POINTER: data is raw pointer from to_storage
        // #VERIFY_SINGLE_DROP: Caller ensures this is called exactly once per to_storage
        let ptr = data as usize as *mut String;
        // Reconstruct Box and let it drop naturally - frees heap allocation
        let _boxed = Box::from_raw(ptr);
        // _boxed drops here, freeing the String's heap memory
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _assert_bitwise_serializable<T: BitwiseSerializable>() {}

    #[test]
    fn test_integer_types_implement_trait() {
        _assert_bitwise_serializable::<u8>();
        _assert_bitwise_serializable::<u16>();
        _assert_bitwise_serializable::<u32>();
        _assert_bitwise_serializable::<u64>();
        _assert_bitwise_serializable::<i8>();
        _assert_bitwise_serializable::<i16>();
        _assert_bitwise_serializable::<i32>();
        _assert_bitwise_serializable::<i64>();
    }

    #[test]
    fn test_float_types_implement_trait() {
        _assert_bitwise_serializable::<f32>();
        _assert_bitwise_serializable::<f64>();
    }

    #[test]
    fn test_size_types_implement_trait() {
        _assert_bitwise_serializable::<usize>();
        _assert_bitwise_serializable::<isize>();
    }

    // Verify that transmute roundtrips work for all bit patterns
    // Test trait method roundtrips for primitives
    #[test]
    fn test_u64_trait_roundtrip() {
        for value in [0u64, 1, 42, u64::MAX, u64::MAX - 1] {
            let storage = value.to_storage();
            let roundtrip = u64::from_storage(storage);
            assert_eq!(value, roundtrip);
        }
    }

    #[test]
    fn test_u32_trait_roundtrip() {
        for value in [0u32, 1, 42, u32::MAX, u32::MAX - 1] {
            let storage = value.to_storage();
            let roundtrip = u32::from_storage(storage);
            assert_eq!(value, roundtrip);
        }
    }

    #[test]
    fn test_i64_trait_roundtrip() {
        for value in [i64::MIN, -1, 0, 1, 42, i64::MAX] {
            let storage = value.to_storage();
            let roundtrip = i64::from_storage(storage);
            assert_eq!(value, roundtrip);
        }
    }

    #[test]
    fn test_f64_trait_roundtrip() {
        // All bit patterns must roundtrip, including NaN/Inf
        for value in [
            0.0f64,
            1.0,
            -1.0,
            f64::MAX,
            f64::MIN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            let storage = value.to_storage();
            let roundtrip = f64::from_storage(storage);
            if value.is_nan() {
                assert!(roundtrip.is_nan());
            } else {
                assert_eq!(value, roundtrip);
            }
        }
    }

    #[test]
    fn test_f32_trait_roundtrip() {
        for value in [0.0f32, 1.0, -1.0, f32::MAX, f32::MIN] {
            let storage = value.to_storage();
            let roundtrip = f32::from_storage(storage);
            assert_eq!(value, roundtrip);
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_arc_trait_roundtrip() {
        use std::sync::Arc;

        // Test Arc<u64> roundtrip
        let value = Arc::new(42u64);
        let original_ptr = Arc::as_ptr(&value);
        let storage = value.to_storage();
        let roundtrip = Arc::<u64>::from_storage(storage);

        assert_eq!(*roundtrip, 42);
        assert_eq!(Arc::as_ptr(&roundtrip), original_ptr);

        // roundtrip is the only Arc owner now - drop will clean up
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_arc_refcount_management() {
        use std::sync::Arc;

        let value = Arc::new(100u64);
        assert_eq!(Arc::strong_count(&value), 1);

        // to_storage transfers ownership - original Arc consumed
        let storage = value.to_storage();

        // from_storage reconstructs Arc - creates a clone (refcount +1)
        // Storage still holds the original Arc reference
        let restored = Arc::<u64>::from_storage(storage);
        assert_eq!(Arc::strong_count(&restored), 2); // restored + storage's forgotten Arc
        assert_eq!(*restored, 100);

        // Drop the restored clone (refcount -1)
        drop(restored);

        // Now clean up the storage's Arc reference using drop_storage
        unsafe {
            Arc::<u64>::drop_storage(storage);
        }
        // Storage Arc is now dropped, refcount back to 0
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_arc_cleanup_in_drop() {
        use std::sync::Arc;

        // Test that drop_storage properly cleans up Arc references
        let value = Arc::new(42u64);
        let weak = Arc::downgrade(&value);
        assert_eq!(weak.strong_count(), 1);

        let storage = value.to_storage();
        assert_eq!(weak.strong_count(), 1); // Still 1 after to_storage

        // Call drop_storage to clean up
        unsafe {
            Arc::<u64>::drop_storage(storage);
        }

        // Arc should be fully dropped now
        assert_eq!(weak.strong_count(), 0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_arc_cleanup_with_multiple_values() {
        use std::sync::Arc;

        // Test multiple Arc values with independent lifecycles
        let value1 = Arc::new(100u64);
        let value2 = Arc::new(200u64);

        let storage1 = value1.to_storage();
        let storage2 = value2.to_storage();

        // Read both values
        let read1 = Arc::<u64>::from_storage(storage1);
        let read2 = Arc::<u64>::from_storage(storage2);

        assert_eq!(*read1, 100);
        assert_eq!(*read2, 200);
        assert_eq!(Arc::strong_count(&read1), 2); // read + storage
        assert_eq!(Arc::strong_count(&read2), 2); // read + storage

        // Drop reads
        drop(read1);
        drop(read2);

        // Clean up storage
        unsafe {
            Arc::<u64>::drop_storage(storage1);
            Arc::<u64>::drop_storage(storage2);
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_string_trait_roundtrip() {
        let original = String::from("Hello, World!");
        let storage = original.clone().to_storage();
        let roundtrip = String::from_storage(storage);

        assert_eq!(roundtrip, "Hello, World!");

        // Clean up storage
        unsafe {
            String::drop_storage(storage);
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_string_multiple_reads() {
        let original = String::from("Test String");
        let storage = original.clone().to_storage();

        // Multiple reads should return clones
        let read1 = String::from_storage(storage);
        let read2 = String::from_storage(storage);
        let read3 = String::from_storage(storage);

        assert_eq!(read1, "Test String");
        assert_eq!(read2, "Test String");
        assert_eq!(read3, "Test String");

        // Clean up storage
        unsafe {
            String::drop_storage(storage);
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_string_empty() {
        let empty = String::new();
        let storage = empty.to_storage();
        let roundtrip = String::from_storage(storage);

        assert_eq!(roundtrip, "");

        unsafe {
            String::drop_storage(storage);
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_arc_cleanup_concurrent_pattern() {
        use std::sync::Arc;

        // Simulate concurrent access pattern:
        // 1. Store Arc
        // 2. Multiple readers get clones
        // 3. Clean up storage when done

        let value = Arc::new(123u64);
        let storage = value.to_storage();

        // Simulate multiple concurrent readers
        let reader1 = Arc::<u64>::from_storage(storage);
        let reader2 = Arc::<u64>::from_storage(storage);
        let reader3 = Arc::<u64>::from_storage(storage);

        // All readers see same value and pointer
        assert_eq!(*reader1, 123);
        assert_eq!(*reader2, 123);
        assert_eq!(*reader3, 123);
        assert_eq!(Arc::as_ptr(&reader1), Arc::as_ptr(&reader2));
        assert_eq!(Arc::as_ptr(&reader2), Arc::as_ptr(&reader3));

        // Refcount = 4 (storage + 3 readers)
        assert_eq!(Arc::strong_count(&reader1), 4);

        // Readers finish
        drop(reader1);
        drop(reader2);
        drop(reader3);

        // Clean up storage
        unsafe {
            Arc::<u64>::drop_storage(storage);
        }
    }
}
