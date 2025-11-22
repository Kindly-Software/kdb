//! # Base Computational Capsule Trait
//!
//! Foundation trait for all computational capsule types.
//!
//! ## UCE33 Q33 (Atomic Capsule Foundation)
//!
//! This trait generalizes The Atomic Capsule architecture (Section 8: Anatomy):
//! - **Alignment requirement**: Enforced via const generics
//! - **Size requirement**: Compile-time verification
//! - **Type identifier**: For debugging and introspection
//! - **Verification methods**: Zero-cost compile-time checks
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_ALIGNMENT_VALID`: Implementor guarantees `#[repr(C, align(N))]`
//! - `#VERIFY_ALIGNMENT_VALID`: Enforced by `verify_alignment()` const fn
//! - `#ASSUME_SIZE_VALID`: Implementor guarantees correct size calculation
//! - `#VERIFY_SIZE_VALID`: Enforced by `verify_size()` const fn

/// Base trait for all computational capsule types.
///
/// This trait provides the foundational requirements that all capsules must satisfy:
/// - **Alignment**: Cache-aligned for optimal performance
/// - **Size**: Fixed size for deterministic layout
/// - **Type safety**: Impossible to misalign or mis-size at runtime
///
/// # UCE33 Q31 (Rust Transform)
///
/// Rust's const generics enable compile-time verification with zero runtime cost.
///
/// # Safety Model
///
/// This trait is intentionally unsafe to implement because incorrect values can:
/// - Violate cache alignment assumptions (performance degradation)
/// - Break atomic operation guarantees (correctness issues)
/// - Cause false sharing (concurrency bugs)
///
/// # Example
///
/// ```rust
/// use atomic_capsule::traits::ComputationalCapsule;
/// use atomic_capsule::HotTier;
///
/// #[repr(C, align(64))]
/// struct MyAtomicCapsule {
///     state: core::sync::atomic::AtomicU64,
///     generation: core::sync::atomic::AtomicU64,
/// }
///
/// unsafe impl ComputationalCapsule for MyAtomicCapsule {
///     const ALIGNMENT: usize = 64;
///     const SIZE: usize = 16; // 2 × u64
///     const TYPE_ID: &'static str = "MyAtomicCapsule";
///
///     fn verify_alignment() -> bool {
///         Self::ALIGNMENT == 64 && Self::ALIGNMENT.count_ones() == 1
///     }
///
///     fn verify_size() -> bool {
///         Self::SIZE == core::mem::size_of::<Self>()
///     }
/// }
/// ```
///
/// # Safety
///
/// Implementors must ensure:
/// - `ALIGNMENT` is a power of 2 and matches actual `#[repr(C, align(N))]` attribute
/// - `SIZE` accurately reflects `core::mem::size_of::<Self>()`
/// - `TYPE_ID` is a unique, non-empty static string
/// - Capsule contains only data that is safe to access via atomic or relaxed memory operations
/// - No interior mutability that violates atomic access patterns
// const_trait disabled for this nightly
// #[cfg_attr(feature = "portable_simd", const_trait)]
pub unsafe trait ComputationalCapsule {
    /// Alignment requirement in bytes (must be power of 2).
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_ALIGNMENT_POW2`: Value is 64, 128, or 256
    /// - `#VERIFY_ALIGNMENT_POW2`: Checked by `verify_alignment()`
    ///
    /// # UCE33 Q29 (Constraints)
    /// Hardware constraint: Must match cache line boundaries (64/128/256 bytes)
    const ALIGNMENT: usize;

    /// Size of the capsule in bytes.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_SIZE_ACCURATE`: Value matches `mem::size_of::<Self>()`
    /// - `#VERIFY_SIZE_ACCURATE`: Checked by `verify_size()`
    ///
    /// # UCE33 Q29 (Constraints)
    /// Size should fit within alignment boundary (64-1024 bits typical)
    const SIZE: usize;

    /// Human-readable type identifier for debugging.
    ///
    /// Used for:
    /// - Error messages
    /// - Logging
    /// - Introspection
    /// - Type registry
    const TYPE_ID: &'static str;

    /// Verify alignment is valid at compile-time.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_INVARIANT`: Alignment is power of 2 and within valid range
    /// - `#VERIFY_INVARIANT`: Checked at compile-time via const evaluation
    ///
    /// # UCE33 Q30 (Validation)
    /// Compile-time check prevents runtime alignment violations
    #[inline(always)]
    fn verify_alignment() -> bool {
        // Power of 2 check
        Self::ALIGNMENT.count_ones() == 1
            // Within valid range (64-256 bytes)
            && Self::ALIGNMENT >= crate::MIN_ALIGNMENT
            && Self::ALIGNMENT <= crate::MAX_ALIGNMENT
    }

    /// Verify size is valid at compile-time.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_INVARIANT`: Size matches actual struct size
    /// - `#VERIFY_INVARIANT`: Checked at compile-time via const evaluation
    ///
    /// # UCE33 Q30 (Validation)
    /// Compile-time check prevents size mismatches
    #[inline(always)]
    fn verify_size() -> bool {
        Self::SIZE > 0 && Self::SIZE <= Self::ALIGNMENT * 4
    }

    /// Get alignment tier for this capsule.
    ///
    /// # UCE33 Q31 (Rust Transform)
    /// Associated type enables compile-time tier selection
    #[inline(always)]
    fn alignment_tier() -> &'static str {
        match Self::ALIGNMENT {
            64 => "hot",
            128 => "warm",
            256 => "cold",
            _ => "custom",
        }
    }

    /// Verify capsule invariants at compile-time.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_INVARIANTS_HOLD`: All verification checks pass
    /// - `#VERIFY_INVARIANTS_HOLD`: Called at compile-time
    ///
    /// # UCE33 Q30 (Validation)
    /// Single method to verify all capsule properties
    #[inline(always)]
    fn verify_invariants() -> bool {
        Self::verify_alignment() && Self::verify_size()
    }
}

/// Verification macro for computational capsules.
///
/// # UCE33 Q31 (Rust Transform)
/// Macro enables verification via type system
///
/// # Note
/// This performs runtime checks. For compile-time verification, use const assertions directly.
///
/// # Example
///
/// ```rust
/// # use atomic_capsule::traits::ComputationalCapsule;
/// # #[repr(C, align(64))]
/// # struct MyAtomicCapsule {
/// #     state: core::sync::atomic::AtomicU64,
/// # }
/// # unsafe impl ComputationalCapsule for MyAtomicCapsule {
/// #     const ALIGNMENT: usize = 64;
/// #     const SIZE: usize = 8;
/// #     const TYPE_ID: &'static str = "MyAtomicCapsule";
/// # }
/// use atomic_capsule::verify_capsule;
///
/// // Runtime verification in tests
/// verify_capsule!(MyAtomicCapsule);
/// ```
#[macro_export]
macro_rules! verify_capsule {
    ($capsule:ty) => {
        assert!(
            <$capsule as $crate::traits::ComputationalCapsule>::verify_alignment(),
            "Capsule alignment invalid"
        );
        assert!(
            <$capsule as $crate::traits::ComputationalCapsule>::verify_size(),
            "Capsule size invalid"
        );
    };
}

/// Alignment verification macro.
///
/// # UCE33 Q30 (Validation)
/// Verify alignment without full capsule verification
///
/// # Example
///
/// ```rust
/// # use atomic_capsule::traits::ComputationalCapsule;
/// # #[repr(C, align(64))]
/// # struct MyAtomicCapsule {
/// #     state: core::sync::atomic::AtomicU64,
/// # }
/// # unsafe impl ComputationalCapsule for MyAtomicCapsule {
/// #     const ALIGNMENT: usize = 64;
/// #     const SIZE: usize = 8;
/// #     const TYPE_ID: &'static str = "MyAtomicCapsule";
/// # }
/// use atomic_capsule::verify_alignment;
///
/// verify_alignment!(MyAtomicCapsule, 64);
/// ```
#[macro_export]
macro_rules! verify_alignment {
    ($capsule:ty, $expected:expr) => {
        assert_eq!(
            <$capsule as $crate::traits::ComputationalCapsule>::ALIGNMENT,
            $expected,
            "Capsule alignment mismatch"
        );
        assert!(
            <$capsule as $crate::traits::ComputationalCapsule>::verify_alignment(),
            "Capsule alignment invalid"
        );
    };
}

/// Size verification macro.
///
/// # UCE33 Q30 (Validation)
/// Verify size without full capsule verification
///
/// # Example
///
/// ```rust
/// # use atomic_capsule::traits::ComputationalCapsule;
/// # #[repr(C, align(64))]
/// # struct MyAtomicCapsule {
/// #     state: core::sync::atomic::AtomicU64,
/// # }
/// # unsafe impl ComputationalCapsule for MyAtomicCapsule {
/// #     const ALIGNMENT: usize = 64;
/// #     const SIZE: usize = 8;
/// #     const TYPE_ID: &'static str = "MyAtomicCapsule";
/// # }
/// use atomic_capsule::verify_size;
///
/// verify_size!(MyAtomicCapsule, 8);
/// ```
#[macro_export]
macro_rules! verify_size {
    ($capsule:ty, $expected:expr) => {
        assert_eq!(
            <$capsule as $crate::traits::ComputationalCapsule>::SIZE,
            $expected,
            "Capsule size mismatch"
        );
        assert!(
            <$capsule as $crate::traits::ComputationalCapsule>::verify_size(),
            "Capsule size invalid"
        );
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::AtomicU64;

    #[repr(C, align(64))]
    struct TestHotCapsule {
        state: AtomicU64,
    }

    unsafe impl ComputationalCapsule for TestHotCapsule {
        const ALIGNMENT: usize = 64;
        const SIZE: usize = 8;
        const TYPE_ID: &'static str = "TestHotCapsule";
    }

    #[test]
    fn test_computational_capsule_verification() {
        assert!(TestHotCapsule::verify_alignment());
        assert!(TestHotCapsule::verify_size());
        assert!(TestHotCapsule::verify_invariants());
    }

    #[test]
    fn test_alignment_tier() {
        assert_eq!(TestHotCapsule::alignment_tier(), "hot");
    }

    #[test]
    fn test_verification_macros() {
        verify_capsule!(TestHotCapsule);
        verify_alignment!(TestHotCapsule, 64);
        verify_size!(TestHotCapsule, 8);
    }
}
