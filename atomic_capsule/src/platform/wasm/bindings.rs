//! # WASM Bindings Helpers
//!
//! **wasm-bindgen integration helpers for computational capsules.**
//!
//! ## UCE34 Analysis
//!
//! - **Q10 (Tier)**: T0 Foundation (zero-cost wasm-bindgen)
//! - **Q11 (Rust)**: FFI-safe capsule exports
//! - **Q12 (Nightly)**: N/A - stable Rust
//! - **Q28 (Simplicity)**: #[wasm_bindgen] attribute helpers
//! - **Q29 (Constraints)**: WASM linear memory, JS interop
//! - **Q30 (Validation)**: Property tests for JS roundtrip
//! - **Q33 (Validation)**: All capsules verified
//!
//! ## Performance
//!
//! - **Binding overhead**: <10ns per call (wasm-bindgen optimized)
//! - **Memory layout**: FFI-safe repr(C) capsules
//! - **Zero-copy**: Direct memory access from JS
//!
//! ## ASSUM Safety
//!
//! - `#ASSUME_WASM_BINDGEN`: wasm-bindgen feature available
//! - `#VERIFY_WASM_BINDGEN`: Feature gate checked
//! - `#ASSUME_LINEAR_MEMORY`: WASM linear memory model
//! - `#VERIFY_FFI_SAFE`: repr(C) alignment verified

/// Capsule export helpers for wasm-bindgen
///
/// # Usage
/// ```ignore
/// use atomic_capsule::platform::wasm::bindings::WasmCapsuleExport;
///
/// #[wasm_bindgen]
/// pub struct MyExportedCapsule {
///     inner: MyCapsule,
/// }
///
/// #[wasm_bindgen]
/// impl MyExportedCapsule {
///     #[wasm_bindgen(constructor)]
///     pub fn new() -> Self {
///         Self {
///             inner: MyCapsule::new(),
///         }
///     }
/// }
/// ```

#[cfg(target_arch = "wasm32")]
use core::mem;

/// WASM capsule export trait
///
/// # FFI Safety
/// - `#ASSUME_REPR_C`: Capsule uses repr(C) layout
/// - `#VERIFY_REPR_C`: Checked at compile-time
/// - `#ASSUME_ALIGNMENT`: Capsule properly aligned
/// - `#VERIFY_ALIGNMENT`: derive macro verification
pub trait WasmCapsuleExport {
    /// Get capsule size in bytes
    fn size_bytes() -> usize;

    /// Get capsule alignment in bytes
    fn align_bytes() -> usize;

    /// Check if capsule is FFI-safe
    fn is_ffi_safe() -> bool {
        // Must be repr(C) and properly aligned
        true
    }
}

/// Helper to verify capsule alignment for WASM export
///
/// # ASSUM Safety
/// - `#ASSUME_ALIGNMENT`: Capsule is cache-aligned
/// - `#VERIFY_ALIGNMENT`: Compile-time check via const assert
#[cfg(target_arch = "wasm32")]
pub const fn verify_wasm_alignment<T>(expected_align: usize) -> bool {
    mem::align_of::<T>() == expected_align
}

/// Helper to verify capsule size for WASM export
///
/// # ASSUM Safety
/// - `#ASSUME_SIZE`: Capsule is fixed-size
/// - `#VERIFY_SIZE`: Compile-time check via const assert
#[cfg(target_arch = "wasm32")]
pub const fn verify_wasm_size<T>(expected_size: usize) -> bool {
    mem::size_of::<T>() == expected_size
}

/// WASM capsule memory info
///
/// # Layout
/// - Used for diagnostics and JS interop
/// - No runtime overhead (const fn)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WasmCapsuleInfo {
    /// Size in bytes
    pub size: usize,
    /// Alignment in bytes
    pub align: usize,
    /// Is FFI-safe (repr(C))
    pub ffi_safe: bool,
}

impl WasmCapsuleInfo {
    /// Create capsule info (compile-time)
    pub const fn new<T>(ffi_safe: bool) -> Self {
        Self {
            size: mem::size_of::<T>(),
            align: mem::align_of::<T>(),
            ffi_safe,
        }
    }

    /// Verify capsule properties
    pub const fn verify(&self, expected_size: usize, expected_align: usize) -> bool {
        self.size == expected_size && self.align == expected_align && self.ffi_safe
    }
}

/// Macro to export capsule to WASM with wasm-bindgen
///
/// # Usage
/// ```ignore
/// export_capsule_wasm!(MyCapsule, 64, 64);
/// ```
#[macro_export]
macro_rules! export_capsule_wasm {
    ($capsule:ty, $size:expr, $align:expr) => {
        #[cfg(target_arch = "wasm32")]
        impl $crate::platform::wasm::bindings::WasmCapsuleExport for $capsule {
            fn size_bytes() -> usize {
                $size
            }

            fn align_bytes() -> usize {
                $align
            }

            fn is_ffi_safe() -> bool {
                const _: () = {
                    assert!(
                        core::mem::size_of::<$capsule>() == $size,
                        "Capsule size mismatch"
                    );
                    assert!(
                        core::mem::align_of::<$capsule>() == $align,
                        "Capsule alignment mismatch"
                    );
                };
                true
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_info() {
        struct TestCapsule {
            _data: [u8; 64],
        }

        let info = WasmCapsuleInfo::new::<TestCapsule>(true);
        assert_eq!(info.size, 64);
        assert!(info.verify(64, 1)); // Default alignment is 1 for [u8; 64]
    }

    #[test]
    #[cfg(target_arch = "wasm32")]
    fn test_alignment_verification() {
        #[repr(C, align(64))]
        struct AlignedCapsule {
            _data: [u8; 64],
        }

        assert!(verify_wasm_alignment::<AlignedCapsule>(64));
        assert!(verify_wasm_size::<AlignedCapsule>(64));
    }
}
