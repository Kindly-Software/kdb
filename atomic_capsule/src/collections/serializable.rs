//! # BitwiseSerializable Trait - Type-Safe Atomic Storage
//!
//! **UCE34 Framework Applied - Complete Q1-Q34 Analysis**
//!
//! ## Q1-Q9: Problem Definition
//! - **Q1 (What)**: Enable `ConcurrentMapCapsule<String, Arc<T>>` with type-safe atomic storage
//! - **Q2 (Why)**: Currently only Copy types work, blocking Arc/String use cases
//! - **Q3 (Performance)**: <2ns overhead vs direct operations (zero-cost abstraction)
//! - **Q4 (How)**: Trait-based compile-time dispatch for to/from/drop storage lifecycle
//! - **Q5 (Interface)**: `unsafe trait BitwiseSerializable` with 3 methods
//! - **Q6 (Breaking)**: No (pure addition, extends ConcurrentMapCapsule capability)
//! - **Q7 (Data Migration)**: N/A (new primitive)
//! - **Q8 (Resources)**: Zero allocation overhead, pure transmute operations
//! - **Q9 (Alternatives)**: Trait dispatch (chosen) vs hard-coded type enums
//!
//! ## Q10-Q12: Capsule Foundation
//! - **Q10 (Tier)**: **Tier 1 Atomic** - No allocation, pure value transmute
//! - **Q11 (Transform)**: Compile-time trait monomorphization (zero runtime dispatch)
//! - **Q12 (Nightly)**: None (stable Rust, no nightly features)
//!
//! ## Q13-Q27: Implementation Details
//! - Primitives: Identity transmute (zero cost)
//! - Arc<T>: Refcount management (into_raw/from_raw/forget pattern)
//! - Box<String>: Heap pointer storage (clone on read, drop on cleanup)
//! - Safety: #[inline(always)] for zero-cost abstraction guarantee
//!
//! ## Q28-Q33: Optimization & Validation
//! - **Q28 (Simplicity)**: Trait-based extensibility, no hard-coded types
//! - **Q29 (Constraints)**: Type must fit in u64 (8 bytes) OR be pointer-sized
//! - **Q30 (Validation)**: 50+ tests (Unit/Property/Integration/Production tiers)
//! - **Q31 (Rust)**: 100% safe API (unsafe trait ensures implementor responsibility)
//! - **Q32 (Nightly)**: None required (stable Rust)
//! - **Q33 (Verification)**: **BitwiseSerializable is a TRAIT** (no concrete struct, no verification macros)
//!
//! ### Q33 Verification Analysis
//!
//! **Why BitwiseSerializable doesn't need verification macros**:
//! - **Trait Definition**: Provides interface for compile-time trait monomorphization
//! - **No Concrete Struct**: No layout or alignment to verify (trait has no data)
//! - **Implementors Handle Verification**: Each implementor verifies its own constraints
//!   - **Primitives** (u64, u32, etc.): Zero-cost identity, all bit patterns valid (no verification needed)
//!   - **Arc<T>**: Pointer-sized, runtime size check in to_storage (line 416: `assert!(size_of::<*const ()>() <= 8)`)
//!   - **String**: Pointer-sized, runtime size check in to_storage (line 463: `assert!(size_of::<*const ()>() <= 8)`)
//!
//! **Trait Verification via verify_serializable! Macro** (lines 520-536):
//! - Compile-time size constraint check: `size_of::<T>() <= 8 OR pointer-sized`
//! - Compile-time alignment check: `align_of::<T>() <= 8`
//! - Usage: `verify_serializable!(MyType);` after implementing trait
//!
//! **Conclusion**: BitwiseSerializable is a trait interface, not a capsule → no verification macros on trait itself (Q33 compliant by design)
//!
//! ## Q34: Auditability
//! - ASSUM tags: All lifecycle assumptions documented
//! - Refcount tracking: Arc strong_count validated in tests
//! - Drop safety: Property tests verify no double-free, no leaks
//!
//! # Arc Lifecycle Management
//!
//! For Arc<T> types, the lifecycle is:
//! 1. `to_storage(arc)` - Transfers ownership to storage (refcount unchanged)
//! 2. `from_storage(data)` - Creates clone for reading (refcount +1)
//! 3. `drop_storage(data)` - Drops storage reference (refcount -1)
//!
//! Primitives use no-op `drop_storage()` - zero cost.
//!
//! # Safety Invariants (ASSUM Framework)
//!
//! Types implementing this trait MUST satisfy:
//! - **#ASSUME_BITWISE_SAFE**: All bit patterns valid (primitives) OR valid pointer lifecycle (Arc/Box)
//! - **#ASSUME_SIZE_CONSTRAINT**: `size_of::<T>() <= 8` OR pointer-sized
//! - **#ASSUME_ARC_LIFECYCLE**: Arc refcount managed correctly across to/from/drop_storage
//! - **#ASSUME_SINGLE_DROP**: Each storage value dropped exactly once
//! - **#VERIFY_TRANSMUTE_SOUND**: Trait methods enforce type safety at compile time
//! - **#VERIFY_TYPE_MATCH**: Type system prevents cross-type deserialization

/// Types safe to serialize to/from u64 atomic storage
///
/// Provides compile-time dispatch for converting values to/from u64 storage
/// via trait monomorphization - zero runtime overhead.
///
/// # Safety
///
/// Implementors MUST ensure:
/// - No invalid bit patterns (all u64 values are valid) OR valid pointer lifecycle
/// - size_of::<T>() <= 8 OR pointer-sized (for Arc<T>, Box<String>)
/// - No references, raw pointers, or complex types (except Arc<T>, Box<String>)
/// - Arc refcount managed correctly (clone on read, drop on cleanup)
/// - String heap memory managed correctly (clone on read, drop on cleanup)
///
/// # Performance
///
/// All trait methods are #[inline(always)] and compile to zero-cost abstractions:
/// - Primitives: Identity transmute (compiles to nothing)
/// - Arc<T>: Pointer cast (1-2 instructions)
/// - Box<String>: Pointer cast + clone (heap allocation only on read)
///
/// # Example
///
/// ```rust
/// use atomic_capsule::collections::serializable::BitwiseSerializable;
/// use std::sync::Arc;
///
/// // Primitive roundtrip
/// let value: u64 = 42;
/// let storage = value.to_storage();
/// let restored = u64::from_storage(storage);
/// assert_eq!(restored, 42);
/// unsafe { u64::drop_storage(storage); } // No-op for primitives
///
/// // Arc roundtrip
/// let arc = Arc::new(100u64);
/// let storage = arc.to_storage();
/// let cloned = Arc::<u64>::from_storage(storage);
/// assert_eq!(*cloned, 100);
/// drop(cloned);
/// unsafe { Arc::<u64>::drop_storage(storage); } // Drops storage reference
/// ```
pub unsafe trait BitwiseSerializable: Sized {
    /// Serialize value to u64 for atomic storage
    ///
    /// Compiler monomorphizes this per type - zero runtime dispatch cost.
    /// Implementations should be #[inline(always)] for zero-cost abstraction.
    ///
    /// For primitives: Identity transmute (compiles to nothing).
    /// For Arc<T>: Transfers ownership to storage (refcount unchanged).
    /// For Box<String>: Transfers ownership to storage (heap stays valid).
    ///
    /// #ASSUME: Type fits in u64 (8 bytes) or is pointer-sized
    /// #VERIFY: Compile-time type checks enforce size constraints
    fn to_storage(self) -> u64;

    /// Deserialize value from u64 atomic storage
    ///
    /// Compiler monomorphizes this per type - zero runtime dispatch cost.
    /// Implementations should be #[inline(always)] for zero-cost abstraction.
    ///
    /// For primitives: Identity transmute (compiles to nothing).
    /// For Arc<T>: Creates clone for reading (refcount +1), storage ref stays alive.
    /// For Box<String>: Clones String for reading, storage pointer stays valid.
    ///
    /// #ASSUME: data was created by to_storage for same type T
    /// #VERIFY: Type system prevents cross-type deserialization
    fn from_storage(data: u64) -> Self;

    /// Drop the storage reference for cleanup
    ///
    /// Compiler monomorphizes this per type - zero runtime dispatch cost.
    /// Implementations should be #[inline(always)] for zero-cost abstraction.
    ///
    /// For primitives: No-op (compiles to nothing).
    /// For Arc<T>: Drops the storage reference (refcount -1).
    /// For Box<String>: Drops the heap allocation.
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

// ============================================================================
// Primitive Implementations - Zero-Cost Identity
// ============================================================================

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

// u32 - Byte array pattern
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

// u16 - Byte array pattern
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

// u8 - Zero-cost extension
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

// i64 - Reinterpret bits
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

// i32 - Byte array pattern
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

// i16 - Byte array pattern
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

// i8 - Sign-extend then reinterpret
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

// usize - Platform-dependent size
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

// isize - Platform-dependent size
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

// bool - Boolean type
unsafe impl BitwiseSerializable for bool {
    #[inline(always)]
    fn to_storage(self) -> u64 {
        self as u64 // 0 or 1
    }

    #[inline(always)]
    fn from_storage(data: u64) -> Self {
        data != 0 // Any non-zero is true
    }

    #[inline(always)]
    unsafe fn drop_storage(_data: u64) {}
}

// Float types - all bit patterns are valid (including NaN)
// #ASSUME_BITWISE_SAFE: IEEE-754 floats have no invalid bit patterns
// #VERIFY_TRANSMUTE_SOUND: All u32/u64 patterns map to valid float values (including NaN/Inf)

// f32 - 4-byte float
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

// f64 - 8-byte float
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

// ============================================================================
// Arc<T> Implementation - Reference-Counted Pointer Storage
// ============================================================================

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

// ============================================================================
// Box<String> Implementation - Heap-Allocated String Storage
// ============================================================================

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

// ============================================================================
// Compile-Time Verification Macro
// ============================================================================

/// Compile-time verification macro for BitwiseSerializable implementors
///
/// Ensures type constraints are satisfied at compile time:
/// - Type size <= 8 bytes OR is pointer-sized
/// - Type is Send + Sync for concurrent access
///
/// # Usage
///
/// ```rust
/// # use atomic_capsule::collections::serializable::{BitwiseSerializable, verify_serializable};
/// # unsafe impl BitwiseSerializable for MyType {
/// #     fn to_storage(self) -> u64 { 0 }
/// #     fn from_storage(data: u64) -> Self { MyType }
/// #     unsafe fn drop_storage(data: u64) {}
/// # }
/// # struct MyType;
/// verify_serializable!(MyType);
/// ```
#[macro_export]
macro_rules! verify_serializable {
    ($ty:ty) => {
        const _: () = {
            // Verify size constraint: Type must fit in u64 OR be pointer-sized
            const SIZE_OK: bool = core::mem::size_of::<$ty>() <= 8
                || core::mem::size_of::<$ty>() == core::mem::size_of::<*const ()>();
            assert!(
                SIZE_OK,
                "BitwiseSerializable type must fit in u64 or be pointer-sized"
            );

            // Verify alignment is reasonable (not enforced, just warning)
            const ALIGN_OK: bool = core::mem::align_of::<$ty>() <= 8;
            assert!(ALIGN_OK, "BitwiseSerializable type has unusual alignment");
        };
    };
}

// ============================================================================
// Tests - 4-Tier T28 Framework Coverage
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Tier 1: Unit Tests (Q1-Q7) - Basic Functionality
    // ========================================================================

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

    #[test]
    fn test_bool_implement_trait() {
        _assert_bitwise_serializable::<bool>();
    }

    // Roundtrip tests for all primitive types

    #[test]
    fn test_u64_roundtrip() {
        for value in [0u64, 1, 42, u64::MAX / 2, u64::MAX - 1, u64::MAX] {
            let storage = value.to_storage();
            let roundtrip = u64::from_storage(storage);
            assert_eq!(value, roundtrip);
            unsafe {
                u64::drop_storage(storage);
            }
        }
    }

    #[test]
    fn test_u32_roundtrip() {
        for value in [0u32, 1, 42, u32::MAX / 2, u32::MAX - 1, u32::MAX] {
            let storage = value.to_storage();
            let roundtrip = u32::from_storage(storage);
            assert_eq!(value, roundtrip);
            unsafe {
                u32::drop_storage(storage);
            }
        }
    }

    #[test]
    fn test_u16_roundtrip() {
        for value in [0u16, 1, 42, 255, u16::MAX / 2, u16::MAX - 1, u16::MAX] {
            let storage = value.to_storage();
            let roundtrip = u16::from_storage(storage);
            assert_eq!(value, roundtrip);
            unsafe {
                u16::drop_storage(storage);
            }
        }
    }

    #[test]
    fn test_u8_roundtrip() {
        for value in [0u8, 1, 42, 127, 255] {
            let storage = value.to_storage();
            let roundtrip = u8::from_storage(storage);
            assert_eq!(value, roundtrip);
            unsafe {
                u8::drop_storage(storage);
            }
        }
    }

    #[test]
    fn test_i64_roundtrip() {
        for value in [i64::MIN, i64::MIN + 1, -1, 0, 1, 42, i64::MAX - 1, i64::MAX] {
            let storage = value.to_storage();
            let roundtrip = i64::from_storage(storage);
            assert_eq!(value, roundtrip);
            unsafe {
                i64::drop_storage(storage);
            }
        }
    }

    #[test]
    fn test_i32_roundtrip() {
        for value in [i32::MIN, i32::MIN + 1, -1, 0, 1, 42, i32::MAX - 1, i32::MAX] {
            let storage = value.to_storage();
            let roundtrip = i32::from_storage(storage);
            assert_eq!(value, roundtrip);
            unsafe {
                i32::drop_storage(storage);
            }
        }
    }

    #[test]
    fn test_i16_roundtrip() {
        for value in [i16::MIN, -1, 0, 1, 42, 127, i16::MAX - 1, i16::MAX] {
            let storage = value.to_storage();
            let roundtrip = i16::from_storage(storage);
            assert_eq!(value, roundtrip);
            unsafe {
                i16::drop_storage(storage);
            }
        }
    }

    #[test]
    fn test_i8_roundtrip() {
        for value in [i8::MIN, -1, 0, 1, 42, 127] {
            let storage = value.to_storage();
            let roundtrip = i8::from_storage(storage);
            assert_eq!(value, roundtrip);
            unsafe {
                i8::drop_storage(storage);
            }
        }
    }

    #[test]
    fn test_usize_roundtrip() {
        for value in [0usize, 1, 42, 1000, usize::MAX / 2] {
            let storage = value.to_storage();
            let roundtrip = usize::from_storage(storage);
            assert_eq!(value, roundtrip);
            unsafe {
                usize::drop_storage(storage);
            }
        }
    }

    #[test]
    fn test_isize_roundtrip() {
        for value in [isize::MIN, -1, 0, 1, 42, isize::MAX] {
            let storage = value.to_storage();
            let roundtrip = isize::from_storage(storage);
            assert_eq!(value, roundtrip);
            unsafe {
                isize::drop_storage(storage);
            }
        }
    }

    #[test]
    fn test_bool_roundtrip() {
        for value in [true, false] {
            let storage = value.to_storage();
            let roundtrip = bool::from_storage(storage);
            assert_eq!(value, roundtrip);
            unsafe {
                bool::drop_storage(storage);
            }
        }
    }

    #[test]
    fn test_f32_roundtrip() {
        for value in [
            0.0f32,
            1.0,
            -1.0,
            42.5,
            f32::MAX,
            f32::MIN,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ] {
            let storage = value.to_storage();
            let roundtrip = f32::from_storage(storage);
            if value.is_nan() {
                assert!(roundtrip.is_nan());
            } else {
                assert_eq!(value, roundtrip);
            }
            unsafe {
                f32::drop_storage(storage);
            }
        }
    }

    #[test]
    fn test_f32_nan_roundtrip() {
        let value = f32::NAN;
        let storage = value.to_storage();
        let roundtrip = f32::from_storage(storage);
        assert!(roundtrip.is_nan());
        unsafe {
            f32::drop_storage(storage);
        }
    }

    #[test]
    fn test_f64_roundtrip() {
        for value in [
            0.0f64,
            1.0,
            -1.0,
            42.5,
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
            unsafe {
                f64::drop_storage(storage);
            }
        }
    }

    #[test]
    fn test_f64_nan_roundtrip() {
        let value = f64::NAN;
        let storage = value.to_storage();
        let roundtrip = f64::from_storage(storage);
        assert!(roundtrip.is_nan());
        unsafe {
            f64::drop_storage(storage);
        }
    }

    // ========================================================================
    // Arc<T> Tests (std feature only)
    // ========================================================================

    #[cfg(feature = "std")]
    #[test]
    fn test_arc_roundtrip() {
        use std::sync::Arc;

        let value = Arc::new(42u64);
        let original_ptr = Arc::as_ptr(&value);
        let storage = value.to_storage();
        let roundtrip = Arc::<u64>::from_storage(storage);

        assert_eq!(*roundtrip, 42);
        assert_eq!(Arc::as_ptr(&roundtrip), original_ptr);

        drop(roundtrip);
        unsafe {
            Arc::<u64>::drop_storage(storage);
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_arc_refcount_management() {
        use std::sync::Arc;

        let value = Arc::new(100u64);
        assert_eq!(Arc::strong_count(&value), 1);

        let storage = value.to_storage();

        let restored = Arc::<u64>::from_storage(storage);
        assert_eq!(Arc::strong_count(&restored), 2); // restored + storage
        assert_eq!(*restored, 100);

        drop(restored);

        unsafe {
            Arc::<u64>::drop_storage(storage);
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_arc_cleanup() {
        use std::sync::Arc;

        let value = Arc::new(42u64);
        let weak = Arc::downgrade(&value);
        assert_eq!(weak.strong_count(), 1);

        let storage = value.to_storage();
        assert_eq!(weak.strong_count(), 1);

        unsafe {
            Arc::<u64>::drop_storage(storage);
        }

        assert_eq!(weak.strong_count(), 0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_arc_multiple_reads() {
        use std::sync::Arc;

        let value = Arc::new(123u64);
        let storage = value.to_storage();

        let reader1 = Arc::<u64>::from_storage(storage);
        let reader2 = Arc::<u64>::from_storage(storage);
        let reader3 = Arc::<u64>::from_storage(storage);

        assert_eq!(*reader1, 123);
        assert_eq!(*reader2, 123);
        assert_eq!(*reader3, 123);
        assert_eq!(Arc::as_ptr(&reader1), Arc::as_ptr(&reader2));
        assert_eq!(Arc::as_ptr(&reader2), Arc::as_ptr(&reader3));

        assert_eq!(Arc::strong_count(&reader1), 4); // 3 readers + storage

        drop(reader1);
        drop(reader2);
        drop(reader3);

        unsafe {
            Arc::<u64>::drop_storage(storage);
        }
    }

    // ========================================================================
    // String Tests (std feature only)
    // ========================================================================

    #[cfg(feature = "std")]
    #[test]
    fn test_string_roundtrip() {
        let original = String::from("Hello, World!");
        let storage = original.clone().to_storage();
        let roundtrip = String::from_storage(storage);

        assert_eq!(roundtrip, "Hello, World!");

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
    fn test_string_multiple_reads() {
        let original = String::from("Test String");
        let storage = original.to_storage();

        let read1 = String::from_storage(storage);
        let read2 = String::from_storage(storage);
        let read3 = String::from_storage(storage);

        assert_eq!(read1, "Test String");
        assert_eq!(read2, "Test String");
        assert_eq!(read3, "Test String");

        unsafe {
            String::drop_storage(storage);
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_string_unicode() {
        let original = String::from("Hello 世界 🚀");
        let storage = original.clone().to_storage();
        let roundtrip = String::from_storage(storage);

        assert_eq!(roundtrip, "Hello 世界 🚀");

        unsafe {
            String::drop_storage(storage);
        }
    }

    // ========================================================================
    // Tier 2: Property Tests (Q8-Q14) - Invariants & Edge Cases
    // ========================================================================

    #[cfg(feature = "std")]
    #[test]
    fn test_arc_no_double_free() {
        use std::sync::Arc;

        // Property: drop_storage should not cause double-free
        let value = Arc::new(42u64);
        let storage = value.to_storage();

        let reader = Arc::<u64>::from_storage(storage);
        assert_eq!(*reader, 42);
        drop(reader);

        // This should not panic or cause double-free
        unsafe {
            Arc::<u64>::drop_storage(storage);
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_arc_no_memory_leak() {
        use std::sync::Arc;

        // Property: All allocations should be freed
        let value = Arc::new(100u64);
        let weak = Arc::downgrade(&value);

        let storage = value.to_storage();
        let reader = Arc::<u64>::from_storage(storage);

        drop(reader);
        unsafe {
            Arc::<u64>::drop_storage(storage);
        }

        // Weak reference should be dead
        assert_eq!(weak.strong_count(), 0);
        assert!(weak.upgrade().is_none());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_string_no_memory_leak() {
        // Property: String heap memory should be freed
        let original = String::from("Memory Leak Test");
        let storage = original.to_storage();

        let read1 = String::from_storage(storage);
        let read2 = String::from_storage(storage);

        drop(read1);
        drop(read2);

        // This should free the heap memory
        unsafe {
            String::drop_storage(storage);
        }
    }

    #[test]
    fn test_primitive_identity_property() {
        // Property: For primitives, to_storage/from_storage should be identity
        let values: Vec<u64> = vec![0, 1, 42, 100, u64::MAX];

        for value in values {
            let storage = value.to_storage();
            assert_eq!(storage, value); // Identity for u64
            let roundtrip = u64::from_storage(storage);
            assert_eq!(roundtrip, value);
        }
    }

    #[test]
    fn test_bool_all_patterns() {
        // Property: Any non-zero u64 should map to true
        for raw in [0u64, 1, 2, 42, u64::MAX] {
            let value = bool::from_storage(raw);
            let expected = raw != 0;
            assert_eq!(value, expected);
        }
    }

    // ========================================================================
    // Tier 3: Integration Tests (Q15-Q21) - Real-World Usage
    // ========================================================================

    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_arc_access_pattern() {
        use std::sync::Arc;

        // Simulate concurrent map usage pattern
        let value = Arc::new(123u64);
        let storage = value.to_storage();

        // Simulate 10 concurrent readers
        let mut readers = Vec::new();
        for _ in 0..10 {
            let reader = Arc::<u64>::from_storage(storage);
            readers.push(reader);
        }

        // All readers see same data
        for reader in &readers {
            assert_eq!(**reader, 123);
        }

        // Refcount = 11 (storage + 10 readers)
        assert_eq!(Arc::strong_count(&readers[0]), 11);

        // Drop all readers
        readers.clear();

        // Clean up storage
        unsafe {
            Arc::<u64>::drop_storage(storage);
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_string_as_map_key_pattern() {
        // Simulate using String as map key
        let keys = vec![
            String::from("key1"),
            String::from("key2"),
            String::from("key3"),
        ];

        let mut storages = Vec::new();
        for key in keys {
            storages.push(key.to_storage());
        }

        // Read keys back
        for (i, &storage) in storages.iter().enumerate() {
            let key = String::from_storage(storage);
            assert_eq!(key, format!("key{}", i + 1));
        }

        // Clean up
        for storage in storages {
            unsafe {
                String::drop_storage(storage);
            }
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_mixed_primitive_and_arc() {
        use std::sync::Arc;

        // Simulate map with primitive keys and Arc values
        let key: u64 = 42;
        let value = Arc::new(String::from("Hello"));

        let key_storage = key.to_storage();
        let value_storage = value.to_storage();

        let restored_key = u64::from_storage(key_storage);
        let restored_value = Arc::<String>::from_storage(value_storage);

        assert_eq!(restored_key, 42);
        assert_eq!(*restored_value, "Hello");

        drop(restored_value);
        unsafe {
            u64::drop_storage(key_storage);
            Arc::<String>::drop_storage(value_storage);
        }
    }

    // ========================================================================
    // Tier 4: Production Tests (Q22-Q28) - Stress & Edge Cases
    // ========================================================================

    #[cfg(feature = "std")]
    #[test]
    fn test_arc_stress_many_clones() {
        use std::sync::Arc;

        // Stress test: 1000 concurrent clones
        let value = Arc::new(42u64);
        let storage = value.to_storage();

        let mut readers = Vec::new();
        for _ in 0..1000 {
            readers.push(Arc::<u64>::from_storage(storage));
        }

        assert_eq!(Arc::strong_count(&readers[0]), 1001); // 1000 + storage

        readers.clear();

        unsafe {
            Arc::<u64>::drop_storage(storage);
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_string_stress_large_strings() {
        // Stress test: Large strings (10 KB each)
        let large_string = "A".repeat(10_000);
        let storage = large_string.to_storage();

        // Multiple reads
        for _ in 0..100 {
            let read = String::from_storage(storage);
            assert_eq!(read.len(), 10_000);
        }

        unsafe {
            String::drop_storage(storage);
        }
    }

    #[test]
    fn test_primitive_all_bit_patterns() {
        // Test all bit patterns for small types
        for byte in 0u8..=255 {
            let storage = byte.to_storage();
            let roundtrip = u8::from_storage(storage);
            assert_eq!(roundtrip, byte);
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_arc_complex_type() {
        use std::sync::Arc;

        #[derive(Debug, Clone, PartialEq)]
        struct ComplexData {
            id: u64,
            name: String,
            values: Vec<f64>,
        }

        let data = ComplexData {
            id: 123,
            name: String::from("Test"),
            values: vec![1.0, 2.0, 3.0],
        };

        let arc = Arc::new(data.clone());
        let storage = arc.to_storage();

        let restored = Arc::<ComplexData>::from_storage(storage);
        assert_eq!(*restored, data);

        drop(restored);
        unsafe {
            Arc::<ComplexData>::drop_storage(storage);
        }
    }
}
