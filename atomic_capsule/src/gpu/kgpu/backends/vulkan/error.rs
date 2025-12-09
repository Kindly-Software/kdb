//! Vulkan Backend Error Types
//!
//! Re-exports HAL error types for convenience.

pub use crate::gpu::kgpu::hal::error::{HalError as KgpuError, HalResult as KgpuResult};

// Additional Vulkan-specific errors

/// Swapchain out of date (needs recreation)
pub struct SwapchainOutOfDate;

impl From<SwapchainOutOfDate> for KgpuError {
    fn from(_: SwapchainOutOfDate) -> Self {
        KgpuError::InvalidState("swapchain out of date")
    }
}

/// Swapchain suboptimal (should recreate)
pub struct SwapchainSuboptimal;

impl From<SwapchainSuboptimal> for KgpuError {
    fn from(_: SwapchainSuboptimal) -> Self {
        KgpuError::InvalidState("swapchain suboptimal")
    }
}
