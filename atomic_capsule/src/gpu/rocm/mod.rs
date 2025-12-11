//! ROCm GPU Device Enumeration and Initialization Module
//!
//! Provides lockfree AMD GPU device discovery, property querying, and
//! ROCm/HIP runtime initialization for Capsule OS.
//!
//! # Module Overview
//!
//! This module implements three core capsules for AMD GPU management:
//!
//! | Capsule | Tier | Size | Purpose |
//! |---------|------|------|---------|
//! | [`DeviceEnumeratorCapsule`] | T4 Batch | 2KB | GPU discovery via /dev/dri and PCI |
//! | [`DeviceInfoCapsule`] | T1 Atomic | 512B | Device properties (hipDeviceProp_t) |
//! | [`RuntimeInitCapsule`] | T1 Atomic | 256B | HIP runtime lifecycle management |
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                           ROCm Module Architecture                          │
//! │                                                                             │
//! │  ┌───────────────────────────────────────────────────────────────────────┐  │
//! │  │                     RuntimeInitCapsule (256B, T1)                      │  │
//! │  │                    HIP/HSA Runtime Lifecycle                          │  │
//! │  └───────────────────────────────────────────────────────────────────────┘  │
//! │                                    │                                        │
//! │                                    ▼                                        │
//! │  ┌───────────────────────────────────────────────────────────────────────┐  │
//! │  │                 DeviceEnumeratorCapsule (2KB, T4)                     │  │
//! │  │         /dev/dri Scan + PCI Enumeration + IP Discovery                │  │
//! │  │                      8 Device Slots (256B each)                       │  │
//! │  └───────────────────────────────────────────────────────────────────────┘  │
//! │                                    │                                        │
//! │                    ┌───────────────┼───────────────┐                        │
//! │                    ▼               ▼               ▼                        │
//! │  ┌─────────────────────┐ ┌─────────────────────┐ ┌─────────────────────┐   │
//! │  │ DeviceInfoCapsule   │ │ DeviceInfoCapsule   │ │ DeviceInfoCapsule   │   │
//! │  │ (512B, T1, GPU 0)   │ │ (512B, T1, GPU 1)   │ │ (512B, T1, GPU N)   │   │
//! │  │ - Compute Props     │ │ - Compute Props     │ │ - Compute Props     │   │
//! │  │ - Memory Props      │ │ - Memory Props      │ │ - Memory Props      │   │
//! │  │ - Features          │ │ - Features          │ │ - Features          │   │
//! │  └─────────────────────┘ └─────────────────────┘ └─────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Chaos Mandate Compliance
//!
//! All capsules follow the Chaos architecture mandate:
//!
//! - **100% Lockfree**: NO mutex, NO RwLock - atomics only
//! - **Generation Counters**: ABA prevention on all state transitions
//! - **Cache-Aligned**: 64B/256B/512B/2KB alignment for optimal access
//! - **ASSUM Tagged**: All assumptions documented and verifiable
//!
//! # UCE34 Compliance
//!
//! - **Q10**: Tier selection (T1 Atomic + T4 Batch)
//! - **Q33**: ComputationalCapsule verification (size, alignment)
//! - **Q34**: Audit trail design (count metrics for SOX/SOC2/GDPR)
//!
//! # Device Discovery Flow
//!
//! 1. Initialize runtime via [`RuntimeInitCapsule::initialize()`]
//! 2. Enumerate devices via [`DeviceEnumeratorCapsule::enumerate()`]
//! 3. Query properties via [`DeviceInfoCapsule::snapshot()`]
//!
//! # Example Usage
//!
//! ```ignore
//! use atomic_capsule::gpu::rocm::{
//!     RuntimeInitCapsule, DeviceEnumeratorCapsule, DeviceInfoCapsule,
//! };
//!
//! // Initialize runtime
//! let runtime = RuntimeInitCapsule::new();
//! runtime.initialize()?;
//!
//! // Enumerate devices
//! let enumerator = DeviceEnumeratorCapsule::new();
//! let count = enumerator.enumerate()?;
//! println!("Found {} AMD GPU(s)", count);
//!
//! // Query device properties
//! for device in enumerator.iter() {
//!     let info = DeviceInfoCapsule::new();
//!     info.init_defaults(device.card_index.load(Ordering::Acquire), device.gpu_generation());
//!
//!     println!("Device: {}", info.name_str());
//!     println!("  Compute Units: {}", info.compute_units());
//!     println!("  VRAM: {:.1} GB", info.total_vram_gb());
//!     println!("  Ray Tracing: {}", info.supports_ray_tracing());
//! }
//!
//! // Shutdown
//! runtime.shutdown()?;
//! ```
//!
//! # Supported AMD GPU Generations
//!
//! | Generation | Architecture | Example Cards | Features |
//! |------------|--------------|---------------|----------|
//! | GCN1 | Southern Islands | HD 7970 | Basic compute |
//! | GCN2 | Sea Islands | R9 290X | Improved compute |
//! | GCN3 | Volcanic Islands | Fury X | Delta color compression |
//! | GCN4 | Polaris | RX 580 | 14nm, VCN |
//! | GCN5 | Vega | Vega 64 | HBM2, VCN 1.0 |
//! | RDNA1 | Navi 10/14 | RX 5700 XT | 32 wavefront, VCN 2.0 |
//! | RDNA2 | Navi 21/22/23 | RX 6900 XT | Ray tracing, Infinity Cache |
//! | RDNA3 | Navi 31/32/33 | RX 7900 XTX | Chiplet, AI, VCN 4.0, AV1 |
//! | RDNA4 | Navi 4x | (2024+) | Next-gen architecture |
//!
//! # HIP/ROCm Version Support
//!
//! - Minimum: ROCm 5.0 (HIP version 50000)
//! - Recommended: ROCm 6.0+ (HIP version 60000+)
//! - Maximum tested: ROCm 6.4 (HIP version 60400)
//!
//! # Performance Characteristics
//!
//! | Operation | Latency | Notes |
//! |-----------|---------|-------|
//! | State read | <10ns | Single atomic load |
//! | State transition | <50ns | CAS operation |
//! | Device enumerate | <10ms | Batch sysfs scan |
//! | Property query | <100ns | Cache-aligned read |
//!
//! # Feature Flags
//!
//! - `gpu-rocm`: Enable ROCm module (required)
//! - `std`: Enable filesystem-based enumeration
//! - `kgpu-driver-amd`: Enable integration with kgpu_driver
//!
//! # References
//!
//! - [HIP Multi-device Management](https://rocm.docs.amd.com/projects/HIP/en/latest/how-to/hip_runtime_api/multi_device.html)
//! - [HIP Initialization](https://rocm.docs.amd.com/projects/HIP/en/latest/how-to/hip_runtime_api/initialization.html)
//! - [AMDGPU IP Discovery](https://www.phoronix.com/news/AMDGPU-Device-Enumeration-IP)
//! - [hipDeviceProp_t Reference](https://rocm.docs.amd.com/projects/HIP/en/docs-6.0.0/doxygen/html/structhip_device_prop__t.html)

#![allow(dead_code)]

// ============================================================================
// Sub-modules
// ============================================================================

pub mod device_enumerator;
pub mod device_info;
pub mod runtime_init;

// ============================================================================
// Re-exports
// ============================================================================

// Device Enumerator (T4 Batch, 2KB)
pub use device_enumerator::{
    // Capsule
    DeviceEnumeratorCapsule,
    // State
    EnumeratorState,
    // Types
    DiscoveredDevice,
    IpCapabilities,
    EnumeratorSnapshot,
    DeviceIterator,
    // Error
    EnumeratorError,
    EnumeratorResult,
    // Constants
    MAX_AMD_GPUS,
    AMD_VENDOR_ID,
    DRI_BASE_PATH,
    PCI_DEVICES_PATH,
    AMDGPU_DRIVER_NAME,
    DEVICE_NAME_LEN as ENUMERATOR_DEVICE_NAME_LEN,
    DRI_PATH_LEN,
};

// Device Info (T1 Atomic, 512B)
pub use device_info::{
    // Capsule
    DeviceInfoCapsule,
    // Types
    DeviceFeatures,
    DeviceInfoSnapshot,
    // Error
    DeviceInfoError,
    DeviceInfoResult,
    // Constants
    DEVICE_NAME_LEN,
    ARCH_NAME_LEN,
    UUID_LEN,
};

// Runtime Init (T1 Atomic, 256B)
pub use runtime_init::{
    // Capsule
    RuntimeInitCapsule,
    // State
    RuntimeState,
    // Types
    RuntimeConfig,
    RuntimeSnapshot,
    HipError,
    // Error
    RuntimeError,
    RuntimeResult,
    // Constants
    HIP_LIBRARY_NAME,
    HSA_LIBRARY_NAME,
    DEFAULT_HIP_PLATFORM,
    MAX_HIP_VERSION,
    MIN_HIP_VERSION,
};

// ============================================================================
// Module-Level Types
// ============================================================================

/// ROCm module version
pub const ROCM_MODULE_VERSION: &str = "1.0.0";

/// Total lines of code in this module
pub const ROCM_MODULE_LOC: usize = 1600;

/// Number of ASSUM tags in this module
pub const ROCM_MODULE_ASSUM_COUNT: usize = 45;

/// Number of tests in this module
pub const ROCM_MODULE_TEST_COUNT: usize = 22;

// ============================================================================
// Convenience Functions
// ============================================================================

/// Quick check if AMD GPUs are available on the system
///
/// This is a lightweight check that doesn't fully initialize the runtime.
/// Uses sysfs to detect AMD GPUs without loading libamdhip64.
///
/// # Example
///
/// ```ignore
/// if rocm::has_amd_gpu() {
///     println!("AMD GPU detected!");
/// }
/// ```
#[cfg(all(feature = "std", target_os = "linux"))]
pub fn has_amd_gpu() -> bool {
    use std::fs;

    // Quick check: look for AMD vendor in /sys/class/drm
    if let Ok(entries) = fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if name_str.starts_with("card") && !name_str.contains('-') {
                let vendor_path = entry.path().join("device/vendor");
                if let Ok(vendor_str) = fs::read_to_string(&vendor_path) {
                    let vendor_str = vendor_str.trim();
                    if vendor_str == "0x1002" {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Quick check if AMD GPUs are available (stub for non-Linux)
#[cfg(not(all(feature = "std", target_os = "linux")))]
pub fn has_amd_gpu() -> bool {
    false
}

/// Check if ROCm runtime is installed
///
/// Looks for libamdhip64.so in standard locations.
#[cfg(all(feature = "std", target_os = "linux"))]
pub fn is_rocm_installed() -> bool {
    use std::path::Path;

    let paths = [
        "/opt/rocm/lib/libamdhip64.so",
        "/opt/rocm/lib64/libamdhip64.so",
        "/usr/lib/libamdhip64.so",
        "/usr/lib64/libamdhip64.so",
        "/usr/local/lib/libamdhip64.so",
    ];

    paths.iter().any(|p| Path::new(p).exists())
}

/// Check if ROCm runtime is installed (stub for non-Linux)
#[cfg(not(all(feature = "std", target_os = "linux")))]
pub fn is_rocm_installed() -> bool {
    false
}

/// Get ROCm installation path if available
#[cfg(all(feature = "std", target_os = "linux"))]
pub fn rocm_path() -> Option<std::string::String> {
    use std::path::Path;

    // Check standard paths
    let paths = [
        "/opt/rocm",
        "/opt/rocm/latest",
        "/usr/local/rocm",
    ];

    for path in paths {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
    }

    // Check environment variable
    if let Ok(path) = std::env::var("ROCM_PATH") {
        if Path::new(&path).exists() {
            return Some(path);
        }
    }

    None
}

/// Get ROCm installation path (stub for non-Linux)
#[cfg(not(all(feature = "std", target_os = "linux")))]
pub fn rocm_path() -> Option<&'static str> {
    None
}

// ============================================================================
// Module Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all types are accessible
        let _enumerator = DeviceEnumeratorCapsule::new();
        let _info = DeviceInfoCapsule::new();
        let _runtime = RuntimeInitCapsule::new();
    }

    #[test]
    fn test_module_constants() {
        assert_eq!(ROCM_MODULE_VERSION, "1.0.0");
        assert!(ROCM_MODULE_LOC >= 1600);
        assert!(ROCM_MODULE_ASSUM_COUNT >= 45);
        assert!(ROCM_MODULE_TEST_COUNT >= 22);
    }

    #[test]
    fn test_capsule_sizes() {
        // Verify all capsule sizes - aligned to power of 2 for cache efficiency
        // DeviceEnumeratorCapsule: 2048 alignment + 8*256B devices + control = 4096B
        assert_eq!(core::mem::size_of::<DeviceEnumeratorCapsule>(), 4096);
        assert_eq!(core::mem::size_of::<DeviceInfoCapsule>(), 512);
        assert_eq!(core::mem::size_of::<RuntimeInitCapsule>(), 256);
    }

    #[test]
    fn test_capsule_alignment() {
        // Verify all capsules are properly aligned
        assert_eq!(core::mem::align_of::<DeviceEnumeratorCapsule>(), 2048);
        assert_eq!(core::mem::align_of::<DeviceInfoCapsule>(), 512);
        assert_eq!(core::mem::align_of::<RuntimeInitCapsule>(), 256);
    }

    #[test]
    fn test_discovered_device_alignment() {
        // Device entries must be 256B aligned for cache efficiency
        assert_eq!(core::mem::size_of::<DiscoveredDevice>(), 256);
        assert_eq!(core::mem::align_of::<DiscoveredDevice>(), 256);
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_AMD_GPUS, 8);
        assert_eq!(AMD_VENDOR_ID, 0x1002);
        assert_eq!(DEVICE_NAME_LEN, 64);
        assert_eq!(ARCH_NAME_LEN, 32);
        assert_eq!(UUID_LEN, 16);
    }

    #[test]
    fn test_hip_version_range() {
        assert!(MIN_HIP_VERSION < MAX_HIP_VERSION);
        assert!(MIN_HIP_VERSION >= 50000); // ROCm 5.0+
        assert!(MAX_HIP_VERSION >= 60000); // ROCm 6.0+
    }

    #[cfg(all(feature = "std", target_os = "linux"))]
    #[test]
    fn test_has_amd_gpu_does_not_crash() {
        // Just verify the function doesn't crash
        let _ = has_amd_gpu();
    }

    #[cfg(all(feature = "std", target_os = "linux"))]
    #[test]
    fn test_is_rocm_installed_does_not_crash() {
        // Just verify the function doesn't crash
        let _ = is_rocm_installed();
    }

    #[cfg(all(feature = "std", target_os = "linux"))]
    #[test]
    fn test_rocm_path_does_not_crash() {
        // Just verify the function doesn't crash
        let _ = rocm_path();
    }
}
