//! # Alignment Validation for T9 Persistent Capsules
//!
//! Compile-time and runtime alignment verification.
//!
//! **UCE34 Q25**: Verification - alignment is CRITICAL for atomic safety
//! **UCE34 Q33**: Validation - compile-time + runtime checks
//!
//! # Safety
//!
//! Atomic operations require natural alignment:
//! - u8: 1-byte aligned
//! - u16: 2-byte aligned
//! - u32: 4-byte aligned
//! - u64: 8-byte aligned
//!
//! Misaligned atomics cause:
//! - **x86-64**: Performance degradation (slow path)
//! - **ARM**: SIGBUS crash (undefined behavior)
//! - **RISC-V**: SIGBUS crash (undefined behavior)
//!
//! # Architecture
//!
//! Three layers of protection:
//! 1. **Compile-time**: const assertions on capsule layouts
//! 2. **Runtime**: alignment checks on mmap offsets
//! 3. **Debug**: Debug assertions on atomic_from_mut calls

use crate::persistence::mmap_capsule::PersistentError;

// ============================================================================
// COMPILE-TIME VERIFICATION
// ============================================================================

/// Verify capsule alignment at compile-time
///
/// # Example
///
/// ```rust,ignore
/// #[repr(C, align(64))]
/// struct MyCapsule {
///     value: u64,
///     _padding: [u8; 56],
/// }
///
/// verify_capsule_alignment!(MyCapsule, 64, 64);
/// ```
#[macro_export]
macro_rules! verify_capsule_alignment {
    ($capsule:ty, $expected_size:expr, $expected_align:expr) => {
        const _: () = {
            const SIZE: usize = core::mem::size_of::<$capsule>();
            const ALIGN: usize = core::mem::align_of::<$capsule>();

            assert!(SIZE == $expected_size, "Size mismatch");
            assert!(ALIGN == $expected_align, "Alignment mismatch");
        };
    };
}

// ============================================================================
// RUNTIME VALIDATION
// ============================================================================

/// Validate offset alignment
///
/// # Arguments
///
/// - `offset`: Byte offset to check
/// - `required`: Required alignment (power of 2)
///
/// # Errors
///
/// Returns `InvalidAlignment` if offset not aligned to required boundary.
///
/// # Performance
///
/// - <1ns (single modulo operation)
///
/// # Example
///
/// ```rust,ignore
/// validate_alignment(128, 8)?; // OK: 128 % 8 == 0
/// validate_alignment(129, 8)?; // ERR: 129 % 8 == 1
/// ```
#[inline]
pub fn validate_alignment(offset: usize, required: usize) -> Result<(), PersistentError> {
    if offset % required != 0 {
        return Err(PersistentError::InvalidAlignment { offset, required });
    }
    Ok(())
}

/// Compute next aligned offset
///
/// # Arguments
///
/// - `offset`: Current offset
/// - `align`: Required alignment (power of 2)
///
/// # Returns
///
/// Next offset that satisfies alignment (offset rounded up).
///
/// # Performance
///
/// - <1ns (bit masking)
///
/// # Example
///
/// ```rust,ignore
/// assert_eq!(compute_aligned_offset(129, 8), 136);
/// assert_eq!(compute_aligned_offset(128, 8), 128); // Already aligned
/// ```
#[inline]
pub fn compute_aligned_offset(offset: usize, align: usize) -> usize {
    // Round up to next multiple of align
    // Formula: (offset + align - 1) & !(align - 1)
    debug_assert!(align.is_power_of_two(), "Alignment must be power of 2");

    (offset + align - 1) & !(align - 1)
}

/// Check if offset is aligned
///
/// # Arguments
///
/// - `offset`: Offset to check
/// - `align`: Required alignment (power of 2)
///
/// # Returns
///
/// `true` if offset is aligned, `false` otherwise.
///
/// # Performance
///
/// - <1ns (single modulo operation)
#[inline]
pub const fn is_aligned(offset: usize, align: usize) -> bool {
    offset % align == 0
}

/// Validate atomic type alignment for mmap offset
///
/// # Arguments
///
/// - `offset`: Mmap offset
/// - `atomic_type`: Type name (for error messages)
///
/// # Type Parameters
///
/// - `T`: Atomic primitive type (u8, u16, u32, u64)
///
/// # Errors
///
/// Returns `InvalidAlignment` if offset doesn't satisfy atomic alignment.
///
/// # Example
///
/// ```rust,ignore
/// validate_atomic_alignment::<u64>(128, "AtomicU64")?; // OK
/// validate_atomic_alignment::<u64>(129, "AtomicU64")?; // ERR
/// ```
#[inline]
pub fn validate_atomic_alignment<T>(
    offset: usize,
    _atomic_type: &str,
) -> Result<(), PersistentError> {
    let required = core::mem::align_of::<T>();
    validate_alignment(offset, required)
}

// ============================================================================
// ALIGNMENT CONSTANTS
// ============================================================================

/// Common alignment requirements
pub mod align {
    /// 8-byte alignment (u64, AtomicU64)
    pub const U64: usize = 8;

    /// 4-byte alignment (u32, AtomicU32)
    pub const U32: usize = 4;

    /// 2-byte alignment (u16, AtomicU16)
    pub const U16: usize = 2;

    /// 1-byte alignment (u8, AtomicU8)
    pub const U8: usize = 1;

    /// Cache line alignment (x86-64)
    pub const CACHE_LINE: usize = 64;

    /// ARM cache line alignment
    pub const CACHE_LINE_ARM: usize = 128;

    /// Page alignment (mmap)
    pub const PAGE: usize = 4096;

    /// Huge page alignment (2MB)
    pub const HUGE_PAGE: usize = 2 * 1024 * 1024;
}

// ============================================================================
// PLATFORM-SPECIFIC HELPERS
// ============================================================================

/// Get platform cache line size
#[inline]
pub const fn cache_line_size() -> usize {
    #[cfg(target_arch = "x86_64")]
    {
        align::CACHE_LINE
    }

    #[cfg(target_arch = "aarch64")]
    {
        align::CACHE_LINE_ARM
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        align::CACHE_LINE // Default to 64B
    }
}

/// Validate cache line separation for DualAtomic pattern
///
/// # Arguments
///
/// - `offset1`: First atomic offset
/// - `offset2`: Second atomic offset
///
/// # Errors
///
/// Returns error if atomics are within same cache line (false sharing risk).
///
/// # Example
///
/// ```rust,ignore
/// validate_cache_line_separation(0, 64)?; // OK: Different cache lines
/// validate_cache_line_separation(0, 8)?;  // ERR: Same cache line
/// ```
#[inline]
pub fn validate_cache_line_separation(
    offset1: usize,
    offset2: usize,
) -> Result<(), PersistentError> {
    let cache_line = cache_line_size();
    let distance = offset1.abs_diff(offset2);

    if distance < cache_line {
        return Err(PersistentError::InvalidAlignment {
            offset: distance,
            required: cache_line,
        });
    }

    Ok(())
}

// ============================================================================
// TESTS (T28 Framework)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_alignment_ok() {
        assert!(validate_alignment(0, 8).is_ok());
        assert!(validate_alignment(8, 8).is_ok());
        assert!(validate_alignment(16, 8).is_ok());
        assert!(validate_alignment(128, 64).is_ok());
    }

    #[test]
    fn test_validate_alignment_err() {
        assert!(validate_alignment(1, 8).is_err());
        assert!(validate_alignment(7, 8).is_err());
        assert!(validate_alignment(129, 64).is_err());
    }

    #[test]
    fn test_compute_aligned_offset() {
        // Already aligned
        assert_eq!(compute_aligned_offset(0, 8), 0);
        assert_eq!(compute_aligned_offset(8, 8), 8);
        assert_eq!(compute_aligned_offset(128, 64), 128);

        // Round up
        assert_eq!(compute_aligned_offset(1, 8), 8);
        assert_eq!(compute_aligned_offset(7, 8), 8);
        assert_eq!(compute_aligned_offset(9, 8), 16);
        assert_eq!(compute_aligned_offset(129, 64), 192);
    }

    #[test]
    fn test_is_aligned() {
        assert!(is_aligned(0, 8));
        assert!(is_aligned(8, 8));
        assert!(is_aligned(16, 8));

        assert!(!is_aligned(1, 8));
        assert!(!is_aligned(7, 8));
        assert!(!is_aligned(9, 8));
    }

    #[test]
    fn test_atomic_alignment() {
        // u64 requires 8-byte alignment
        assert!(validate_atomic_alignment::<u64>(0, "u64").is_ok());
        assert!(validate_atomic_alignment::<u64>(8, "u64").is_ok());
        assert!(validate_atomic_alignment::<u64>(1, "u64").is_err());

        // u32 requires 4-byte alignment
        assert!(validate_atomic_alignment::<u32>(0, "u32").is_ok());
        assert!(validate_atomic_alignment::<u32>(4, "u32").is_ok());
        assert!(validate_atomic_alignment::<u32>(1, "u32").is_err());

        // u16 requires 2-byte alignment
        assert!(validate_atomic_alignment::<u16>(0, "u16").is_ok());
        assert!(validate_atomic_alignment::<u16>(2, "u16").is_ok());
        assert!(validate_atomic_alignment::<u16>(1, "u16").is_err());
    }

    #[test]
    fn test_cache_line_separation() {
        let cache_line = cache_line_size();

        // Valid separation
        assert!(validate_cache_line_separation(0, cache_line).is_ok());
        assert!(validate_cache_line_separation(0, cache_line * 2).is_ok());

        // Invalid separation (same cache line)
        assert!(validate_cache_line_separation(0, 8).is_err());
        assert!(validate_cache_line_separation(0, cache_line - 1).is_err());
    }

    #[test]
    fn test_alignment_constants() {
        assert_eq!(align::U64, 8);
        assert_eq!(align::U32, 4);
        assert_eq!(align::U16, 2);
        assert_eq!(align::U8, 1);
        assert_eq!(align::CACHE_LINE, 64);
        assert_eq!(align::PAGE, 4096);
    }

    #[test]
    fn test_cache_line_size_platform() {
        let size = cache_line_size();

        #[cfg(target_arch = "x86_64")]
        assert_eq!(size, 64);

        #[cfg(target_arch = "aarch64")]
        assert_eq!(size, 128);

        // At minimum, must be power of 2 and >= 64
        assert!(size.is_power_of_two());
        assert!(size >= 64);
    }

    #[test]
    fn test_compile_time_verification() {
        // Example capsule
        #[repr(C, align(64))]
        struct TestCapsule {
            value: u64,
            _padding: [u8; 56],
        }

        // Compile-time verification
        verify_capsule_alignment!(TestCapsule, 64, 64);

        // Runtime verification
        assert_eq!(core::mem::size_of::<TestCapsule>(), 64);
        assert_eq!(core::mem::align_of::<TestCapsule>(), 64);
    }
}
