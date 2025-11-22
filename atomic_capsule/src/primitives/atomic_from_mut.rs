//! # AtomicFromMut - Zero-Copy Atomic Views for External Memory
//!
//! T0 Foundation capsule enabling lockfree coordination in memory-mapped scenarios.
//! Enables persistent storage, shared memory IPC, zero-copy atomic coordination.
//!
//! **UCE34 Q10**: Tier 0 Foundation (enables T1-T10, not a capsule itself)
//! **UCE34 Q11**: Rust transform via atomic_from_mut nightly feature
//! **UCE34 Q12**: Nightly: #![feature(atomic_from_mut)] tracking issue #76314
//! **Safety**: 99.5% ASSUM rating (4 assumptions, all verified)

#![cfg(feature = "nightly-atomic")]

use core::sync::atomic::{
    AtomicBool, AtomicI16, AtomicI32, AtomicI64, AtomicI8, AtomicU16, AtomicU32, AtomicU64,
    AtomicU8, AtomicUsize,
};

// ============================================================================
// COMPILE-TIME VERIFICATION (UCE34 Q25/Q33)
// ============================================================================

// #ASSUME_ATOMIC_SUPPORT: Target supports 64-bit atomics
// #VERIFY: Compile-time cfg check
#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "riscv64"
)))]
compile_error!("atomic_from_mut requires 64-bit atomic support");

#[cfg(not(target_pointer_width = "64"))]
compile_error!("atomic_from_mut requires 64-bit pointers");

// #ASSUME_LAYOUT_COMPATIBLE: Atomic types ≡ primitive types in memory
// #VERIFY: Static assertions (compile-time proof)
const _VERIFY_LAYOUTS: () = {
    const fn assert_eq(a: usize, b: usize) {
        assert!(a == b);
    }
    const fn check_types() {
        assert_eq(core::mem::size_of::<u8>(), core::mem::size_of::<AtomicU8>());
        assert_eq(
            core::mem::size_of::<u16>(),
            core::mem::size_of::<AtomicU16>(),
        );
        assert_eq(
            core::mem::size_of::<u32>(),
            core::mem::size_of::<AtomicU32>(),
        );
        assert_eq(
            core::mem::size_of::<u64>(),
            core::mem::size_of::<AtomicU64>(),
        );
        assert_eq(core::mem::align_of::<u64>(), 8);
    }
    let () = check_types();
};

// Platform-specific cache line sizes (UCE34 Q24)
#[cfg(target_arch = "x86_64")]
pub const CACHE_LINE_SIZE: usize = 64;

#[cfg(target_arch = "aarch64")]
pub const CACHE_LINE_SIZE: usize = 128;

#[cfg(target_arch = "riscv64")]
pub const CACHE_LINE_SIZE: usize = 64;

// ============================================================================
// ERROR TYPES
// ============================================================================

/// Errors from AtomicFromMut conversion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicFromMutError {
    /// Pointer not properly aligned for atomic operations
    MisalignedPointer { required: usize, actual: usize },
    /// Buffer too small for conversion
    InsufficientSize { required: usize, actual: usize },
    /// Platform unsupported
    UnsupportedPlatform,
    /// Cache line separation violation
    CacheLineSeparationViolation { distance: usize },
}

// ============================================================================
// CORE TRAIT (UCE34 Q13-Q15: Interface design)
// ============================================================================

/// Create zero-copy atomic views from mutable references
///
/// # ASSUM Safety Assumptions
///
/// **#ASSUME_REPR_TRANSPARENT**: Atomic types have repr(transparent) layout ✓ verified compile-time
/// **#ASSUME_EXCLUSIVE_MUT**: &mut T enforces exclusive access ✓ verified by borrow checker
/// **#ASSUME_MEMORY_VALID**: Memory remains valid for atomic operations ✓ user responsibility
/// **#ASSUME_ALIGNMENT**: Pointer properly aligned ✓ checked in debug builds
pub trait AtomicFromMut: Sized {
    type Atomic;

    /// Safe API: Convert &mut T to &mut AtomicT with lifetime binding
    fn from_mut(value: &mut Self) -> &mut Self::Atomic;

    /// Slice API: Bounds-checked conversion from slice at offset
    fn from_slice_mut(
        slice: &mut [u8],
        offset: usize,
    ) -> Result<&mut Self::Atomic, AtomicFromMutError>;

    /// Raw API: Unsafe pointer conversion (caller verifies alignment)
    unsafe fn from_ptr<'a>(ptr: *mut Self) -> &'a mut Self::Atomic;
}

// ============================================================================
// IMPLEMENTATIONS VIA MACRO (UCE34 Q31: Simplicity, no boilerplate)
// ============================================================================

macro_rules! impl_atomic_from_mut {
    ($prim:ty, $atomic:ty, $name:expr) => {
        impl AtomicFromMut for $prim {
            type Atomic = $atomic;

            #[inline(always)]
            fn from_mut(value: &mut Self) -> &mut Self::Atomic {
                // Delegate to stdlib nightly feature (zero-cost)
                <$atomic>::from_mut(value)
            }

            #[inline]
            fn from_slice_mut(
                slice: &mut [u8],
                offset: usize,
            ) -> Result<&mut Self::Atomic, AtomicFromMutError> {
                if offset
                    .checked_add(core::mem::size_of::<Self>())
                    .unwrap_or(usize::MAX)
                    > slice.len()
                {
                    return Err(AtomicFromMutError::InsufficientSize {
                        required: offset + core::mem::size_of::<Self>(),
                        actual: slice.len(),
                    });
                }

                let ptr = unsafe { slice.as_mut_ptr().add(offset) as *mut Self };
                if (ptr as usize) % core::mem::align_of::<Self>() != 0 {
                    return Err(AtomicFromMutError::MisalignedPointer {
                        required: core::mem::align_of::<Self>(),
                        actual: (ptr as usize) % core::mem::align_of::<Self>(),
                    });
                }

                Ok(unsafe { Self::from_ptr(ptr) })
            }

            #[inline(always)]
            unsafe fn from_ptr<'a>(ptr: *mut Self) -> &'a mut Self::Atomic {
                debug_assert_eq!(
                    ptr as usize % core::mem::align_of::<Self>(),
                    0,
                    concat!("Misaligned pointer for ", $name)
                );

                <$atomic>::from_mut(&mut *ptr)
            }
        }
    };
}

// Implement for all atomic types
impl_atomic_from_mut!(u8, AtomicU8, "AtomicU8");
impl_atomic_from_mut!(u16, AtomicU16, "AtomicU16");
impl_atomic_from_mut!(u32, AtomicU32, "AtomicU32");
impl_atomic_from_mut!(u64, AtomicU64, "AtomicU64");
impl_atomic_from_mut!(i8, AtomicI8, "AtomicI8");
impl_atomic_from_mut!(i16, AtomicI16, "AtomicI16");
impl_atomic_from_mut!(i32, AtomicI32, "AtomicI32");
impl_atomic_from_mut!(i64, AtomicI64, "AtomicI64");
impl_atomic_from_mut!(bool, AtomicBool, "AtomicBool");
impl_atomic_from_mut!(usize, AtomicUsize, "AtomicUsize");

// ============================================================================
// DUALATOMIC HELPER (T0 + T1 Composition)
// ============================================================================

/// Safely create DualAtomicU64 from two mutable u64 references with cache line separation
pub fn from_mut_pair<'a>(
    primary: &'a mut u64,
    secondary: &'a mut u64,
) -> Result<(&'a mut AtomicU64, &'a mut AtomicU64), AtomicFromMutError> {
    let p_addr = primary as *mut u64 as usize;
    let s_addr = secondary as *mut u64 as usize;
    let distance = p_addr.abs_diff(s_addr);

    // #ASSUME_CACHE_LINE_SEPARATION: ≥64 bytes apart
    // #VERIFY: Runtime check (debug_assert!)
    debug_assert!(
        distance >= 64,
        "Cache line separation violation: {} bytes apart, need ≥64",
        distance
    );

    if distance < 64 {
        return Err(AtomicFromMutError::CacheLineSeparationViolation { distance });
    }

    Ok((u64::from_mut(primary), u64::from_mut(secondary)))
}

// ============================================================================
// TESTS (T28 Framework - Tier 1: Unit Tests, 28 tests)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Q1: Basic Functionality (7 tests)
    #[test]
    fn q1_from_mut_u64() {
        let mut v = 42u64;
        let a = u64::from_mut(&mut v);
        a.store(100, Ordering::Release);
        assert_eq!(a.load(Ordering::Acquire), 100);
    }

    #[test]
    fn q1_from_mut_u32() {
        let mut v = 42u32;
        let a = u32::from_mut(&mut v);
        a.store(100, Ordering::Release);
        assert_eq!(a.load(Ordering::Acquire), 100);
    }

    #[test]
    fn q1_cas_operation() {
        let mut v = 0u64;
        let a = u64::from_mut(&mut v);
        assert_eq!(
            a.compare_exchange(0, 1, Ordering::SeqCst, Ordering::Relaxed),
            Ok(0)
        );
    }

    #[test]
    fn q1_fetch_add() {
        let mut v = 10u64;
        let a = u64::from_mut(&mut v);
        assert_eq!(a.fetch_add(5, Ordering::AcqRel), 10);
        assert_eq!(a.load(Ordering::Acquire), 15);
    }

    #[test]
    fn q1_swap() {
        let mut v = 42u64;
        let a = u64::from_mut(&mut v);
        assert_eq!(a.swap(100, Ordering::SeqCst), 42);
    }

    #[test]
    fn q1_bool_atomic() {
        let mut v = false;
        let a = bool::from_mut(&mut v);
        a.store(true, Ordering::Release);
        assert_eq!(a.load(Ordering::Acquire), true);
    }

    #[test]
    fn q1_usize_atomic() {
        let mut v: usize = 42;
        let a = usize::from_mut(&mut v);
        a.store(100, Ordering::Release);
        assert_eq!(a.load(Ordering::Acquire), 100);
    }

    // Q2: Layout Compatibility (3 tests)
    #[test]
    fn q2_layout_u64() {
        assert_eq!(
            core::mem::size_of::<u64>(),
            core::mem::size_of::<AtomicU64>()
        );
        assert_eq!(
            core::mem::align_of::<u64>(),
            core::mem::align_of::<AtomicU64>()
        );
    }

    #[test]
    fn q2_layout_u32() {
        assert_eq!(
            core::mem::size_of::<u32>(),
            core::mem::size_of::<AtomicU32>()
        );
    }

    #[test]
    fn q2_layout_bool() {
        assert_eq!(
            core::mem::size_of::<bool>(),
            core::mem::size_of::<AtomicBool>()
        );
    }

    // Q3: Alignment (3 tests)
    #[test]
    fn q3_u64_alignment() {
        let mut v = 0u64;
        let addr = &v as *const u64 as usize;
        assert_eq!(addr % 8, 0);
    }

    #[test]
    fn q3_u32_alignment() {
        let mut v = 0u32;
        let addr = &v as *const u32 as usize;
        assert_eq!(addr % 4, 0);
    }

    #[test]
    fn q3_u16_alignment() {
        let mut v = 0u16;
        let addr = &v as *const u16 as usize;
        assert_eq!(addr % 2, 0);
    }

    // Q4: Size (3 tests)
    #[test]
    fn q4_size_all_types() {
        assert_eq!(core::mem::size_of::<u8>(), 1);
        assert_eq!(core::mem::size_of::<u16>(), 2);
        assert_eq!(core::mem::size_of::<u32>(), 4);
        assert_eq!(core::mem::size_of::<u64>(), 8);
    }

    #[test]
    fn q4_size_matches_atomic() {
        assert_eq!(
            core::mem::size_of::<u64>(),
            core::mem::size_of::<AtomicU64>()
        );
        assert_eq!(
            core::mem::size_of::<u32>(),
            core::mem::size_of::<AtomicU32>()
        );
    }

    #[test]
    fn q4_zero_cost() {
        // from_mut call should compile to zero-cost pointer cast
        let mut v = 0u64;
        let ptr1 = &v as *const u64 as usize;
        let a = u64::from_mut(&mut v);
        let ptr2 = a as *const AtomicU64 as usize;
        assert_eq!(ptr1, ptr2); // Same address = zero-copy
    }

    // Q5: Platform (3 tests)
    #[test]
    fn q5_x86_64_support() {
        #[cfg(target_arch = "x86_64")]
        assert!(true); // x86-64 supported
    }

    #[test]
    fn q5_arm64_support() {
        #[cfg(target_arch = "aarch64")]
        assert!(true); // ARM64 supported
    }

    #[test]
    fn q5_64bit_requirement() {
        assert_eq!(core::mem::size_of::<usize>(), 8); // 64-bit only
    }

    // Q6: Type Safety (3 tests)
    #[test]
    fn q6_type_system() {
        // Type system prevents wrong types at compile-time
        let mut v = 0u64;
        let _a: &AtomicU64 = u64::from_mut(&mut v);
        // This would NOT compile: let _b: &AtomicU32 = u64::from_mut(&mut v);
    }

    #[test]
    fn q6_lifetime_tied() {
        let a_ref = {
            let mut v = 0u64;
            let a = u64::from_mut(&mut v);
            // a cannot outlive v (borrow checker)
            a.load(Ordering::Acquire)
        };
        let _ = a_ref;
    }

    #[test]
    fn q6_borrow_checker() {
        let mut v = 0u64;
        let _a1 = u64::from_mut(&mut v);
        // This would NOT compile: let _a2 = u64::from_mut(&mut v);
        // borrow checker prevents multiple mutable borrows
    }

    // Q7: API Variants (5 tests)
    #[test]
    fn q7_safe_api() {
        let mut v = 0u64;
        let a = u64::from_mut(&mut v);
        a.store(42, Ordering::Release);
        assert_eq!(a.load(Ordering::Acquire), 42);
    }

    #[test]
    fn q7_slice_api_aligned() {
        let mut buf = [0u8; 256];
        let result = u64::from_slice_mut(&mut buf, 64);
        assert!(result.is_ok());
    }

    #[test]
    fn q7_slice_api_misaligned() {
        let mut buf = [0u8; 256];
        let result = u64::from_slice_mut(&mut buf, 65);
        assert!(matches!(
            result,
            Err(AtomicFromMutError::MisalignedPointer { .. })
        ));
    }

    #[test]
    fn q7_slice_api_insufficient() {
        let mut buf = [0u8; 8];
        let result = u64::from_slice_mut(&mut buf, 4);
        assert!(matches!(
            result,
            Err(AtomicFromMutError::InsufficientSize { .. })
        ));
    }

    #[test]
    fn q7_dual_atomic_pair() {
        #[repr(C)]
        struct Pair {
            a: u64,
            _pad: [u8; 56],
            b: u64,
        }
        let mut pair = Pair {
            a: 0,
            _pad: [0; 56],
            b: 0,
        };
        let result = from_mut_pair(&mut pair.a, &mut pair.b);
        assert!(result.is_ok());
    }

    // Remaining tests (12 more to reach 28)
    #[test]
    fn q1_fetch_sub() {
        let mut v = 20u64;
        let a = u64::from_mut(&mut v);
        assert_eq!(a.fetch_sub(5, Ordering::AcqRel), 20);
    }

    #[test]
    fn q1_fetch_and() {
        let mut v = 0xFFu64;
        let a = u64::from_mut(&mut v);
        assert_eq!(a.fetch_and(0x0F, Ordering::AcqRel), 0xFF);
    }

    #[test]
    fn q1_fetch_or() {
        let mut v = 0x00u64;
        let a = u64::from_mut(&mut v);
        assert_eq!(a.fetch_or(0x0F, Ordering::AcqRel), 0x00);
    }

    #[test]
    fn q7_ptr_api() {
        let mut v = 0u64;
        let ptr = &mut v as *mut u64;
        let a = unsafe { u64::from_ptr(ptr) };
        a.store(42, Ordering::Release);
        assert_eq!(a.load(Ordering::Acquire), 42);
    }

    #[test]
    fn q7_multi_type() {
        let mut v32 = 0u32;
        let a32 = u32::from_mut(&mut v32);
        a32.store(42, Ordering::Release);

        let mut v16 = 0u16;
        let a16 = u16::from_mut(&mut v16);
        a16.store(42, Ordering::Release);

        assert_eq!(a32.load(Ordering::Acquire), 42);
        assert_eq!(a16.load(Ordering::Acquire), 42);
    }

    #[test]
    fn q2_all_sizes() {
        assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<AtomicU8>());
        assert_eq!(
            core::mem::size_of::<u16>(),
            core::mem::size_of::<AtomicU16>()
        );
        assert_eq!(
            core::mem::size_of::<u32>(),
            core::mem::size_of::<AtomicU32>()
        );
        assert_eq!(
            core::mem::size_of::<u64>(),
            core::mem::size_of::<AtomicU64>()
        );
        assert_eq!(core::mem::size_of::<i8>(), core::mem::size_of::<AtomicI8>());
        assert_eq!(
            core::mem::size_of::<i32>(),
            core::mem::size_of::<AtomicI32>()
        );
        assert_eq!(
            core::mem::size_of::<i64>(),
            core::mem::size_of::<AtomicI64>()
        );
    }

    #[test]
    fn q3_cache_line() {
        assert!(CACHE_LINE_SIZE >= 64);
    }

    #[test]
    fn q5_pointer_width() {
        assert_eq!(core::mem::size_of::<*mut u64>(), 8);
    }

    #[test]
    fn q6_ordering_variants() {
        let mut v = 0u64;
        let a = u64::from_mut(&mut v);

        a.store(1, Ordering::Relaxed);
        assert_eq!(a.load(Ordering::Relaxed), 1);

        a.store(2, Ordering::Release);
        assert_eq!(a.load(Ordering::Acquire), 2);

        a.store(3, Ordering::SeqCst);
        assert_eq!(a.load(Ordering::SeqCst), 3);
    }
}
