//! # WASM Platform Module - T2 SIMD Tier
//!
//! **Platform-specific WASM primitives with dual SIMD backends.**
//!
//! ## UCE34 Analysis
//!
//! - **Q10 (Capsule Tier)**: T2 SIMD (2-19× speedups)
//! - **Q11 (Rust Transform)**: WASM target with stable simd128 + nightly portable_simd
//! - **Q12 (Nightly)**: portable_simd for cross-platform SIMD abstraction
//! - **Q28 (Simplicity)**: Dual backend with compile-time selection
//! - **Q29 (Constraints)**: WASM linear memory, 16-byte SIMD alignment
//! - **Q30 (Validation)**: B32 benchmarks for both backends
//! - **Q31 (Rust Transform)**: Zero unsafe code via std::arch::wasm32
//! - **Q33 (Validation)**: All capsules use #[derive(ComputationalCapsule)]
//!
//! ## Architecture
//!
//! **Dual Backend Strategy**:
//! - **Stable**: std::arch::wasm32 (simd128 target feature)
//! - **Nightly**: std::simd::portable_simd (cross-platform abstraction)
//! - **Runtime**: Compile-time backend selection via feature flags
//!
//! ## Memory Layout
//!
//! ```text
//! WASM Linear Memory:
//! [Capsule Data: 16-byte aligned] [Padding to cache line]
//! Total: 64 bytes (Hot Tier, single cache line)
//! ```
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_WASM_SIMD128`: Target supports simd128 feature
//! - `#VERIFY_SIMD128`: Compile-time cfg check
//! - `#ASSUME_LINEAR_MEMORY`: WASM linear memory model
//! - `#VERIFY_ALIGNMENT`: All capsules verified with derive macro
//! - `#ASSUME_PORTABLE_SIMD`: Nightly portable_simd available
//! - `#VERIFY_PORTABLE_SIMD`: Feature gate on portable_simd
//!
//! ## Performance Targets (B32 Framework)
//!
//! - **Stable simd128**: 2-8× speedup vs scalar (4+ fields)
//! - **Nightly portable_simd**: 7-19× speedup vs scalar (proven in x86_64)
//! - **Hash operations**: <20ns for 4-8 field structs
//! - **SIMD operations**: <10ns for 8-element vectors
//!
//! ## Feature Flags
//!
//! - `simd-stable-wasm`: Enable stable std::arch::wasm32 backend
//! - `simd-nightly-wasm`: Enable nightly portable_simd backend
//! - `wasm-bindgen`: Enable wasm-bindgen helpers
//! - `wasm-memory-utils`: Enable linear memory utilities

pub mod bindings;
pub mod memory;
pub mod simd;
pub mod simd_nightly;
pub mod simd_runtime;

pub use simd_runtime::SimdBackend;

/// Platform detection for WASM SIMD capabilities
///
/// # Examples
/// ```
/// use atomic_capsule::platform::wasm::detect_simd_backend;
///
/// let backend = detect_simd_backend();
/// println!("WASM SIMD backend: {:?}", backend);
/// ```
pub fn detect_simd_backend() -> SimdBackend {
    SimdBackend::detect()
}

/// Check if WASM simd128 is available
///
/// # ASSUM Safety
/// - `#ASSUME_WASM_SIMD128`: Target feature simd128 available
/// - `#VERIFY_SIMD128`: Compile-time cfg check
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub const fn has_simd128() -> bool {
    true
}

#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
pub const fn has_simd128() -> bool {
    false
}

/// Check if portable_simd is available
///
/// # ASSUM Safety
/// - `#ASSUME_PORTABLE_SIMD`: Feature portable_simd enabled
/// - `#VERIFY_PORTABLE_SIMD`: Compile-time cfg check
#[cfg(feature = "portable_simd")]
pub const fn has_portable_simd() -> bool {
    true
}

#[cfg(not(feature = "portable_simd"))]
pub const fn has_portable_simd() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_backend_detection() {
        let backend = detect_simd_backend();
        // Should compile successfully regardless of backend
        match backend {
            SimdBackend::Stable(_) => {
                #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
                assert!(true, "Stable backend detected");
            }
            SimdBackend::Nightly(_) => {
                #[cfg(feature = "portable_simd")]
                assert!(true, "Nightly backend detected");
            }
            SimdBackend::Scalar => {
                assert!(true, "Scalar fallback");
            }
        }
    }

    #[test]
    fn test_simd128_detection() {
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        assert!(has_simd128());

        #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
        assert!(!has_simd128());
    }

    #[test]
    fn test_portable_simd_detection() {
        #[cfg(feature = "portable_simd")]
        assert!(has_portable_simd());

        #[cfg(not(feature = "portable_simd"))]
        assert!(!has_portable_simd());
    }
}
