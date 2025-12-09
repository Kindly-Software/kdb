//! Vulkan Backend Implementation via ash 0.38
//!
//! # Architecture
//!
//! Production Vulkan backend implementing KGPU HAL traits:
//! - **Instance**: Entry point, adapter enumeration, validation layers
//! - **Adapter**: Physical device selection, feature queries, GPU scoring
//! - **Device**: Logical device, queue management, resource creation
//! - **Surface**: Platform window surface (ash-window integration)
//! - **Swapchain**: Image presentation, VSync, triple-buffering
//! - **Command**: Command buffer recording, submission
//! - **Sync**: Fences, semaphores, Synchronization2 support
//! - **Memory**: Memory allocation, type selection
//!
//! # Vulkan 1.3+ Features
//!
//! - Dynamic rendering (VK_KHR_dynamic_rendering promoted to core)
//! - Synchronization2 (VK_KHR_synchronization2 promoted to core)
//! - Maintenance4 (better spec compliance)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_VULKAN_LOADER_AVAILABLE`: Vulkan SDK installed on system
//! - `#ASSUME_PHYSICAL_DEVICE_VALID`: At least one GPU supports Vulkan 1.0+
//! - `#ASSUME_MEMORY_TYPES_INCLUDE_HOST_VISIBLE`: Required by Vulkan spec
//! - `#VERIFY_UNSAFE_FFI`: All ash FFI calls wrapped in safe abstractions
//!
//! # Framework Compliance
//!
//! - **UCE34**: T7 Heterogeneous tier (GPU backend)
//! - **Chaos**: Lockfree HAL trait implementations
//! - **ASSUM**: All unsafe documented
//! - **B32**: Performance targets (Instance <50ms, Device <100ms)
//! - **T28**: Unit/property/integration tests (may need #[ignore] for CI)

pub mod error;
pub mod adapter;
pub mod command;
pub mod device;
pub mod instance;
pub mod memory;
pub mod surface;
pub mod swapchain;
pub mod sync;

pub use adapter::VulkanAdapter;
pub use command::VulkanCommandBuffer;
pub use device::VulkanDevice;
pub use instance::VulkanInstance;
pub use memory::VulkanMemory;
pub use surface::VulkanSurface;
pub use swapchain::VulkanSwapchain;
pub use sync::{VulkanFence, VulkanSemaphore};

use crate::gpu::kgpu::hal::Backend;

/// Vulkan backend type marker
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VulkanBackend;

impl VulkanBackend {
    /// Backend name
    pub const NAME: &'static str = "Vulkan";

    /// API version (major, minor, patch)
    /// Targeting Vulkan 1.3 (promoted extensions)
    pub const API_VERSION: (u32, u32, u32) = (1, 3, 0);

    /// Check if Vulkan is available on this platform
    ///
    /// # Performance
    ///
    /// ~10ms (loads Vulkan library, checks for vkGetInstanceProcAddr)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_VULKAN_LOADER_AVAILABLE`: Returns false if not installed
    pub fn is_available() -> bool {
        // Try to load Vulkan entry point
        ash::Entry::linked().is_ok() || ash::Entry::load().is_ok()
    }

    /// Returns backend identifier
    pub const fn backend() -> Backend {
        Backend::Vulkan
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_constants() {
        assert_eq!(VulkanBackend::NAME, "Vulkan");
        assert_eq!(VulkanBackend::API_VERSION, (1, 3, 0));
        assert_eq!(VulkanBackend::backend(), Backend::Vulkan);
    }

    #[test]
    fn test_vulkan_availability() {
        // May fail on CI without Vulkan drivers
        let available = VulkanBackend::is_available();
        println!("Vulkan available: {}", available);
        // Don't assert - this is platform-dependent
    }
}
