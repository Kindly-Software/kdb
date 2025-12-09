//! # Static Assertions Capsule - T28 Comprehensive Tests
//!
//! **Tier**: T0 Auditable (0ns runtime, compile-time verification)
//!
//! ## Test Structure (T28 Framework)
//!
//! - **Q1-Q7 (Unit)**: Basic macro functionality
//! - **Q8-Q14 (Property)**: Type invariants and composition
//! - **Q15-Q21 (Integration)**: Real capsule usage
//! - **Q22-Q28 (Production)**: Platform-specific and edge cases
//!
//! ## ASSUM Safety Framework
//!
//! ```text
//! #ASSUME_CONST_EVALUATION: All assertions evaluate at compile-time
//! #VERIFY_CONST_EVALUATION: Rust language guarantee (const fn)
//! ```

// Import static assertion macros from crate root (exported via #[macro_export])
use atomic_capsule::{
    const_assert, assert_eq_size, assert_size, assert_eq_align, assert_align,
    assert_pow2_size, assert_pow2_align, assert_no_padding, assert_align_ge_size
};
use core::sync::atomic::{AtomicU32, AtomicU64};

// ============================================================================
// Q1-Q7: Unit Tests (Basic Macro Functionality)
// ============================================================================

#[test]
fn q1_const_assert_true() {
    // Verify basic true assertion compiles
    const_assert!(true);
    const_assert!(1 + 1 == 2);
}

#[test]
fn q2_const_assert_with_message() {
    // Verify assertion with custom message
    const_assert!(2 + 2 == 4, "Math is broken");
}

#[test]
fn q3_assert_eq_size_primitives() {
    // Verify same-size primitive types
    assert_eq_size!(u32, i32);
    assert_eq_size!(u64, i64);
    assert_eq_size!(u16, i16);
}

#[test]
fn q4_assert_size_basic() {
    // Verify basic size assertions
    assert_size!(u8, 1);
    assert_size!(u16, 2);
    assert_size!(u32, 4);
    assert_size!(u64, 8);
}

#[test]
fn q5_assert_eq_align_primitives() {
    // Verify same-alignment primitive types
    assert_eq_align!(u32, i32);
    assert_eq_align!(u64, i64);
}

#[test]
fn q6_assert_align_basic() {
    // Verify basic alignment assertions
    assert_align!(u8, 1);
    assert_align!(u16, 2);
    assert_align!(u32, 4);
    assert_align!(u64, 8);
}

#[test]
fn q7_assert_pow2_size_primitives() {
    // Verify power-of-2 sizes for primitive types
    assert_pow2_size!(u8);   // 1 = 2^0
    assert_pow2_size!(u16);  // 2 = 2^1
    assert_pow2_size!(u32);  // 4 = 2^2
    assert_pow2_size!(u64);  // 8 = 2^3
}

// ============================================================================
// Q8-Q14: Property Tests (Type Invariants and Composition)
// ============================================================================

#[test]
fn q8_atomic_types_size_alignment() {
    // AtomicU32: 4 bytes, 4-byte aligned
    assert_eq_size!(AtomicU32, u32);
    assert_eq_align!(AtomicU32, u32);
    assert_size!(AtomicU32, 4);
    assert_align!(AtomicU32, 4);

    // AtomicU64: 8 bytes, 8-byte aligned
    assert_eq_size!(AtomicU64, u64);
    assert_eq_align!(AtomicU64, u64);
    assert_size!(AtomicU64, 8);
    assert_align!(AtomicU64, 8);
}

#[test]
fn q9_array_size_alignment() {
    // Arrays should have size = element_size × count
    assert_size!([u8; 16], 16);
    assert_size!([u32; 4], 16);
    assert_size!([u64; 2], 16);

    // Arrays should have alignment = element alignment
    assert_align!([u8; 16], 1);
    assert_align!([u32; 4], 4);
    assert_align!([u64; 2], 8);
}

#[test]
fn q10_repr_c_no_padding() {
    #[repr(C)]
    struct Packed {
        a: u32,
        b: u32,
    }

    assert_size!(Packed, 8);
    assert_align!(Packed, 4);
    assert_no_padding!(Packed, 8);
}

#[test]
fn q11_repr_c_align_override() {
    #[repr(C, align(64))]
    struct CacheLineAligned {
        value: u64,
    }

    // With align(64), the struct is padded to 64 bytes (not 8)
    assert_size!(CacheLineAligned, 64);
    assert_align!(CacheLineAligned, 64);
    assert_pow2_align!(CacheLineAligned);
}

#[test]
fn q12_transparent_wrapper() {
    #[repr(transparent)]
    struct Wrapper(u64);

    assert_eq_size!(Wrapper, u64);
    assert_eq_align!(Wrapper, u64);
}

#[test]
fn q13_nested_repr_c() {
    #[repr(C)]
    struct Inner {
        a: u16,
        b: u16,
    }

    #[repr(C)]
    struct Outer {
        inner: Inner,
        c: u32,
    }

    assert_size!(Inner, 4);
    assert_size!(Outer, 8);
    assert_no_padding!(Inner, 4);
    assert_no_padding!(Outer, 8);
}

#[test]
fn q14_atomic_alignment_ge_size() {
    // Atomics must have alignment >= size for lock-free operations
    assert_align_ge_size!(AtomicU32);
    assert_align_ge_size!(AtomicU64);
}

// ============================================================================
// Q15-Q21: Integration Tests (Real Capsule Usage)
// ============================================================================

#[test]
fn q15_fixed_point_q8_8() {
    #[repr(transparent)]
    struct Q8_8(i16);

    assert_size!(Q8_8, 2);
    assert_align!(Q8_8, 2);
    assert_eq_size!(Q8_8, i16);
    assert_eq_align!(Q8_8, i16);
}

#[test]
fn q16_fixed_point_q16_16() {
    #[repr(transparent)]
    struct Q16_16(i32);

    assert_size!(Q16_16, 4);
    assert_align!(Q16_16, 4);
    assert_eq_size!(Q16_16, i32);
    assert_eq_align!(Q16_16, i32);
}

#[test]
fn q17_fixed_point_q32_32() {
    #[repr(transparent)]
    struct Q32_32(i64);

    assert_size!(Q32_32, 8);
    assert_align!(Q32_32, 8);
    assert_eq_size!(Q32_32, i64);
    assert_eq_align!(Q32_32, i64);
}

#[test]
fn q18_dual_atomic_u64() {
    #[repr(C, align(16))]
    struct DualAtomicU64 {
        primary: AtomicU64,
        secondary: AtomicU64,
    }

    assert_size!(DualAtomicU64, 16);
    assert_align!(DualAtomicU64, 16);
    assert_pow2_size!(DualAtomicU64);
    assert_pow2_align!(DualAtomicU64);
    assert_no_padding!(DualAtomicU64, 16);
}

#[test]
fn q19_simd_f32x8_layout() {
    #[repr(C, align(32))]
    struct SimdF32x8([f32; 8]);

    assert_size!(SimdF32x8, 32);
    assert_align!(SimdF32x8, 32);
    assert_pow2_size!(SimdF32x8);
    assert_pow2_align!(SimdF32x8);
}

#[test]
fn q20_cache_line_capsule_64b() {
    #[repr(C, align(64))]
    struct CapsuleCacheLineAligned {
        metadata: AtomicU64,
        data: [u8; 56],
    }

    assert_size!(CapsuleCacheLineAligned, 64);
    assert_align!(CapsuleCacheLineAligned, 64);
    assert_pow2_size!(CapsuleCacheLineAligned);
    assert_pow2_align!(CapsuleCacheLineAligned);
    assert_no_padding!(CapsuleCacheLineAligned, 64);
}

#[test]
fn q21_cache_line_capsule_128b() {
    #[repr(C, align(128))]
    struct LargeCapsule {
        primary: AtomicU64,
        secondary: AtomicU64,
        padding: [u8; 112],
    }

    assert_size!(LargeCapsule, 128);
    assert_align!(LargeCapsule, 128);
    assert_no_padding!(LargeCapsule, 128);
}

// ============================================================================
// Q22-Q28: Production Tests (Platform-Specific and Edge Cases)
// ============================================================================

#[test]
fn q22_platform_pointer_width_64bit() {
    #[cfg(target_pointer_width = "64")]
    {
        assert_size!(usize, 8);
        assert_size!(*const (), 8);
        assert_align!(usize, 8);
    }
}

#[test]
fn q23_platform_pointer_width_32bit() {
    #[cfg(target_pointer_width = "32")]
    {
        assert_size!(usize, 4);
        assert_size!(*const (), 4);
        assert_align!(usize, 4);
    }
}

#[test]
fn q24_zero_sized_types() {
    struct ZST;

    assert_size!(ZST, 0);
    assert_align!(ZST, 1);
}

#[test]
fn q25_phantom_data() {
    use core::marker::PhantomData;

    struct WithPhantom<T> {
        _phantom: PhantomData<T>,
    }

    assert_size!(WithPhantom<u64>, 0);
    assert_align!(WithPhantom<u64>, 1);
}

#[test]
fn q26_option_non_null_optimization() {
    // Option<&T> should be same size as *const T (non-null optimization)
    assert_eq_size!(Option<&u32>, *const u32);
}

#[test]
fn q27_result_optimization() {
    // Result<(), ()> should be 1 byte (niche optimization)
    assert_size!(Result<(), ()>, 1);
}

#[test]
fn q28_complex_nested_structure() {
    #[repr(C)]
    struct Level1 {
        a: u32,
        b: u32,
    }

    #[repr(C)]
    struct Level2 {
        inner: Level1,
        c: u64,
    }

    #[repr(C, align(64))]
    struct Level3 {
        level2: Level2,
        padding: [u8; 48],
    }

    assert_size!(Level1, 8);
    assert_size!(Level2, 16);
    assert_size!(Level3, 64);
    assert_align!(Level3, 64);
}

// ============================================================================
// Additional Coverage Tests (Beyond T28)
// ============================================================================

#[test]
fn test_multiple_assertions_same_type() {
    // Verify multiple assertions on the same type don't conflict
    assert_size!(u64, 8);
    assert_align!(u64, 8);
    assert_pow2_size!(u64);
    assert_pow2_align!(u64);
    assert_align_ge_size!(u64);
}

#[test]
fn test_arrays_of_complex_types() {
    #[repr(C)]
    struct Complex {
        a: u32,
        b: u64,
    }

    assert_size!(Complex, 16);  // 4 + 4 padding + 8
    assert_size!([Complex; 4], 64);
}

#[test]
fn test_alignment_greater_than_size() {
    #[repr(C, align(64))]
    struct SmallButAligned {
        value: u8,
    }

    // With align(64), the struct is padded to 64 bytes (not 1)
    assert_size!(SmallButAligned, 64);
    assert_align!(SmallButAligned, 64);
}
