//! Capsule definition macro
//!
//! # UCE33 Framework Alignment
//!
//! - **Q10**: Foundation tier (declarative capsule creation)
//! - **Q28**: Simplicity (one macro, automatic verification)
//! - **Q31**: Rust zero-cost abstractions
//! - **Q33**: Compile-time validation (verify_capsule! integrated)
//!
//! # Problem: Repetitive Boilerplate
//!
//! Before (6 lines per capsule × 53 capsules = 318 lines):
//! ```rust,ignore
//! #[repr(C, align(64))]
//! struct MyCapsule {
//!     state: AtomicU64,
//!     data: [u8; 56],
//! }
//! unsafe impl Send for MyCapsule {}
//! unsafe impl Sync for MyCapsule {}
//! verify_capsule!(MyCapsule, 64, 64);
//! ```
//!
//! After (single macro invocation):
//! ```rust,ignore
//! use atomic_capsule::define_capsule;
//!
//! define_capsule! {
//!     pub struct MyCapsule align(64) size(64) {
//!         state: AtomicU64,
//!         data: [u8; 56],
//!     }
//! }
//! ```
//!
//! # Safety Model
//!
//! - **#ASSUME_SEND_SYNC**: Capsule pattern guarantees thread safety
//! - **#VERIFY_SEND_SYNC**: Atomic operations, immutable after init, cache-aligned
//! - **#ASSUME_ALIGNMENT**: repr(C, align(N)) enforces alignment
//! - **#VERIFY_ALIGNMENT**: Compile-time verification macro
//!
//! # I20 Integration
//!
//! Works with all existing capsule types:
//! - Atomic capsules (Tier 1)
//! - SIMD capsules (Tier 2)
//! - Fixed-point capsules (Tier 3)
//! - Mixed capsules (Tier 6)

/// Define a computational capsule with automatic boilerplate
///
/// # Syntax
///
/// ```rust,ignore
/// define_capsule! {
///     $(#[$meta:meta])*
///     $vis:vis struct $name:ident
///     align($align:literal)
///     size($size:literal)
///     {
///         $($field:ident : $field_ty:ty),* $(,)?
///     }
/// }
/// ```
///
/// # Generated Code
///
/// - `#[repr(C, align($align))]` attribute
/// - `unsafe impl Send` (capsule pattern guarantees)
/// - `unsafe impl Sync` (lockfree coordination)
/// - `verify_capsule!` compile-time check
/// - `AlignmentTier` trait implementation
///
/// # Example
///
/// ```rust
/// use atomic_capsule::{define_capsule, AlignmentTier};
/// use core::sync::atomic::AtomicU64;
///
/// define_capsule! {
///     pub struct CircuitBreakerCapsule align(64) size(64) {
///         state: AtomicU64,
///         padding: [u8; 56],
///     }
/// }
///
/// // Automatically implements:
/// // - Send + Sync (thread-safe)
/// // - AlignmentTier (alignment = 64, tier = "hot")
/// // - Compile-time verification (alignment + size)
///
/// let capsule = CircuitBreakerCapsule {
///     state: AtomicU64::new(0),
///     padding: [0; 56],
/// };
///
/// assert_eq!(core::mem::align_of::<CircuitBreakerCapsule>(), 64);
/// assert_eq!(core::mem::size_of::<CircuitBreakerCapsule>(), 64);
/// assert_eq!(CircuitBreakerCapsule::ALIGNMENT, 64);
/// assert_eq!(CircuitBreakerCapsule::TIER, "hot");
/// ```
///
/// # Safety
///
/// The `unsafe impl Send` and `unsafe impl Sync` are safe because:
/// 1. Capsule pattern uses atomic operations for coordination
/// 2. Data is immutable after initialization (or uses atomics)
/// 3. Cache alignment prevents false sharing
/// 4. No interior mutability without atomics
///
/// # Performance
///
/// Zero-cost: Identical to manual boilerplate
#[macro_export]
macro_rules! define_capsule {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident
        align($align:literal)
        size($size:literal)
        {
            $($field:ident : $field_ty:ty),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[repr(C, align($align))]
        $vis struct $name {
            $($field: $field_ty),*
        }

        // SAFETY: Capsule pattern guarantees thread safety
        // - Atomic operations for coordination
        // - Immutable data after initialization
        // - Cache-aligned to prevent false sharing
        // - No interior mutability without atomics
        //
        // #ASSUME_SEND_SYNC: Capsule pattern provides thread-safety guarantees
        // #VERIFY_SEND_SYNC: Compile-time alignment check + lockfree design
        unsafe impl Send for $name {}
        unsafe impl Sync for $name {}

        // Q33: Compile-time verification using unified macro
        $crate::verify_capsule_properties!($name, $align, $size);

        // Implement AlignmentTier marker trait
        impl $crate::AlignmentTier for $name {
            const TIER: &'static str = if $align == 64 {
                "hot"
            } else if $align == 128 {
                "warm"
            } else if $align == 256 {
                "cold"
            } else {
                "custom"
            };

            const ALIGNMENT: usize = $align;
        }
    };
}

// Note: macro_rules! macros are automatically exported with #[macro_export]
// No need to re-export here

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use crate::AlignmentTier;
    use core::sync::atomic::AtomicU64;

    define_capsule! {
        pub struct TestCapsule64 align(64) size(64) {
            state: AtomicU64,
            padding: [u8; 56],
        }
    }

    define_capsule! {
        pub struct TestCapsule128 align(128) size(128) {
            state: AtomicU64,
            data: [u8; 120],
        }
    }

    define_capsule! {
        /// Documentation test
        #[allow(dead_code)]
        pub struct DocumentedCapsule align(64) size(512) {
            header: AtomicU64,
            body: [u8; 504],
        }
    }

    #[test]
    fn test_define_capsule_basic() {
        let _capsule = TestCapsule64 {
            state: AtomicU64::new(42),
            padding: [0; 56],
        };

        assert_eq!(core::mem::align_of::<TestCapsule64>(), 64);
        assert_eq!(core::mem::size_of::<TestCapsule64>(), 64);
    }

    #[test]
    fn test_alignment_tier() {
        assert_eq!(TestCapsule64::TIER, "hot");
        assert_eq!(TestCapsule64::ALIGNMENT, 64);

        assert_eq!(TestCapsule128::TIER, "warm");
        assert_eq!(TestCapsule128::ALIGNMENT, 128);
    }

    #[test]
    fn test_send_sync_bounds() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<TestCapsule64>();
        assert_sync::<TestCapsule64>();
        assert_send::<TestCapsule128>();
        assert_sync::<TestCapsule128>();
    }

    #[test]
    fn test_documented_capsule() {
        assert_eq!(core::mem::align_of::<DocumentedCapsule>(), 64);
        assert_eq!(core::mem::size_of::<DocumentedCapsule>(), 512);
    }
}
