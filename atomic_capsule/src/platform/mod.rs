//! # Platform-Specific Capsule Implementations
//!
//! This module provides platform-specific implementations of computational capsules
//! that require OS-level features such as:
//! - File I/O and memory-mapped files (T9 Persistent)
//! - Async I/O and networking (T5 Streaming, T8 Network)
//! - SIMD acceleration (T2 SIMD, requires nightly)
//!
//! ## Architecture
//!
//! The platform module is organized by target:
//! - `native`: Standard OS platforms (Linux, macOS, Windows)
//!   - `persistence`: Memory-mapped file capsules (T9)
//!   - `async_log`: Async logging capsules (T5)
//!   - `network`: Network capsules (T8)
//! - `wasm`: WebAssembly target (minimal feature set)
//!
//! ## Feature Flags
//!
//! Platform-specific features are controlled by presets:
//! - `preset-native`: Enables all native platform features
//! - `preset-wasm`: Enables WASM-compatible features only
//!
//! ## Design Principles (UCE34)
//!
//! - **Q10 (Capsule Tier)**: Platform modules implement T5/T8/T9 tiers
//! - **Q29 (Constraints)**: Respect platform capabilities and limitations
//! - **Q33 (Verification)**: All capsules use `#[derive(ComputationalCapsule)]`
//!
//! ## ASSUM Framework
//!
//! Platform-specific code documents all safety assumptions:
//! - `#ASSUME_MMAP_SAFE`: Memory-mapped file safety invariants
//! - `#ASSUME_ASYNC_SAFE`: Async runtime safety assumptions
//! - `#ASSUME_NETWORK_SAFE`: Network socket safety assumptions

// Native platform (Linux, macOS, Windows)
#[cfg(all(
    not(target_arch = "wasm32"),
    any(
        feature = "preset-native",
        feature = "capsule-mmap",
        feature = "async-log",
        feature = "network"
    )
))]
pub mod native;

// Re-exports for convenience when native features are enabled
#[cfg(all(
    not(target_arch = "wasm32"),
    any(feature = "preset-native", feature = "capsule-mmap")
))]
pub use native::persistence;

#[cfg(all(
    not(target_arch = "wasm32"),
    any(feature = "preset-native", feature = "async-log")
))]
pub use native::async_log;

#[cfg(all(
    not(target_arch = "wasm32"),
    any(feature = "preset-native", feature = "network")
))]
pub use native::network;

// WebAssembly platform (Days 2-3: WASM SIMD backends)
#[cfg(target_arch = "wasm32")]
pub mod wasm;

// Re-exports for WASM when features are enabled
#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
pub use wasm::{detect_simd_backend, has_portable_simd, has_simd128, SimdBackend};

/// Platform detection (compile-time)
///
/// # Examples
/// ```
/// use atomic_capsule::platform::detect_platform;
///
/// let platform = detect_platform();
/// println!("Platform: {}", platform);
/// ```
pub const fn detect_platform() -> &'static str {
    #[cfg(target_arch = "wasm32")]
    return "wasm32";

    #[cfg(all(target_os = "linux", not(target_arch = "wasm32")))]
    return "linux";

    #[cfg(all(target_os = "macos", not(target_arch = "wasm32")))]
    return "macos";

    #[cfg(all(target_os = "windows", not(target_arch = "wasm32")))]
    return "windows";

    #[cfg(not(any(
        target_arch = "wasm32",
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )))]
    return "unknown";
}

/// Check if SIMD is available (compile-time)
pub const fn has_simd() -> bool {
    #[cfg(any(
        all(target_arch = "wasm32", target_feature = "simd128"),
        all(target_arch = "wasm32", feature = "portable_simd"),
        feature = "portable_simd"
    ))]
    return true;

    #[cfg(not(any(
        all(target_arch = "wasm32", target_feature = "simd128"),
        all(target_arch = "wasm32", feature = "portable_simd"),
        feature = "portable_simd"
    )))]
    return false;
}

/// Get SIMD backend name (compile-time)
pub const fn simd_backend_name() -> &'static str {
    #[cfg(all(target_arch = "wasm32", feature = "portable_simd"))]
    return "portable_simd (nightly)";

    #[cfg(all(
        target_arch = "wasm32",
        target_feature = "simd128",
        not(feature = "portable_simd")
    ))]
    return "simd128 (stable)";

    #[cfg(all(feature = "portable_simd", not(target_arch = "wasm32")))]
    return "portable_simd (nightly, native)";

    #[cfg(not(any(
        all(target_arch = "wasm32", feature = "portable_simd"),
        all(target_arch = "wasm32", target_feature = "simd128"),
        feature = "portable_simd"
    )))]
    return "scalar (fallback)";
}
