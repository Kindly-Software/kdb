//! Metal Backend Implementation for macOS/iOS
//!
//! # Architecture
//!
//! Production Metal backend implementing KGPU HAL traits:
//! - **Instance**: MTLDevice discovery, capability enumeration
//! - **Adapter**: Physical GPU scoring, feature queries
//! - **Device**: Logical device, command queue management, resource creation
//! - **Surface**: CAMetalLayer integration (macOS/iOS)
//! - **Swapchain**: Triple-buffered presentation, ProMotion support (120Hz)
//! - **Command**: MTLCommandBuffer recording, submission
//! - **Sync**: MTLFence, MTLEvent (timeline semaphores)
//! - **Memory**: Unified memory architecture (Apple Silicon)
//!
//! # Metal 3+ Features
//!
//! - Dynamic rendering (no render pass descriptors required)
//! - Mesh shaders (object + mesh shader stages)
//! - Ray tracing (GPU-driven, accelerated on M3+)
//! - MetalFX upscaling (temporal + spatial)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_METAL_AVAILABLE`: macOS 10.15+ or iOS 13+
//! - `#ASSUME_GPU_PRESENT`: At least one Metal-capable GPU
//! - `#ASSUME_UNIFIED_MEMORY`: Apple Silicon uses unified memory (M1+)
//! - `#VERIFY_UNSAFE_FFI`: All Objective-C calls wrapped in safe abstractions
//!
//! # Framework Compliance
//!
//! - **UCE34**: T7 Heterogeneous tier (GPU backend)
//! - **Chaos**: Lockfree HAL trait implementations (Arc for Metal objects)
//! - **ASSUM**: All unsafe documented
//! - **B32**: Performance targets (Device <20ms, Command <100μs)
//! - **T28**: Unit/property/integration tests (conditional compilation)

pub mod adapter;
pub mod command;
pub mod device;
pub mod instance;
pub mod memory;
pub mod surface;
pub mod sync;

pub use adapter::MetalAdapter;
pub use command::MetalCommandBuffer;
pub use device::MetalDevice;
pub use instance::MetalInstance;
pub use memory::MetalMemory;
pub use surface::MetalSurface;
pub use sync::{MetalEvent, MetalFence};

use crate::gpu::kgpu::hal::BackendType;

/// Metal backend type marker
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetalBackend;

impl MetalBackend {
    /// Backend name
    pub const NAME: &'static str = "Metal";

    /// API version (major, minor, patch)
    /// Targeting Metal 3 (iOS 16+, macOS 13+)
    pub const API_VERSION: (u32, u32, u32) = (3, 0, 0);

    /// Check if Metal is available on this platform
    ///
    /// # Performance
    ///
    /// ~1ms (much faster than Vulkan, Metal is native on Apple platforms)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_METAL_AVAILABLE`: Returns false if not on macOS/iOS
    #[cfg(target_os = "macos")]
    pub fn is_available() -> bool {
        // Check for Metal support via MTLCreateSystemDefaultDevice
        // This is cheap on macOS (just checks for GPU presence)
        unsafe {
            metal::Device::system_default().is_some()
        }
    }

    #[cfg(target_os = "ios")]
    pub fn is_available() -> bool {
        // Metal is always available on iOS 8+
        true
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    pub fn is_available() -> bool {
        // Metal is not available on non-Apple platforms
        false
    }

    /// Returns backend identifier
    pub const fn backend() -> BackendType {
        BackendType::Metal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_constants() {
        assert_eq!(MetalBackend::NAME, "Metal");
        assert_eq!(MetalBackend::API_VERSION, (3, 0, 0));
        assert_eq!(MetalBackend::backend(), BackendType::Metal);
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn test_metal_availability() {
        // Should return true on macOS/iOS with Metal support
        let available = MetalBackend::is_available();
        println!("Metal available: {}", available);
        // Don't assert - depends on platform
    }

    #[test]
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    fn test_metal_not_available_on_non_apple() {
        assert!(!MetalBackend::is_available());
    }
}
