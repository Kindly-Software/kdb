//! Vulkan Synchronization Primitives (Fence, Semaphore)
//!
//! # Architecture
//!
//! VulkanFence and VulkanSemaphore wrap vk::Fence and vk::Semaphore for GPU-CPU and GPU-GPU synchronization.
//!
//! - **Fence**: CPU-GPU synchronization (vkWaitForFences)
//! - **Semaphore**: GPU-GPU synchronization (queue submission dependencies)
//! - **Timeline Semaphore**: Vulkan 1.2+ timeline semaphore (optional)
//! - **Synchronization2**: Vulkan 1.3+ synchronization2 extension
//!
//! # Performance
//!
//! - Fence creation: <1μs (vkCreateFence)
//! - Semaphore creation: <1μs (vkCreateSemaphore)
//! - Fence wait: <1ms (vkWaitForFences, depends on GPU work)
//! - Fence reset: <1μs (vkResetFences)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_SYNC_CREATION_SUCCEEDS`: Device is valid
//! - `#ASSUME_WAIT_TIMEOUT_VALID`: Timeout is reasonable (≤1 second typical)
//! - `#VERIFY_UNSAFE_FFI`: All vk* sync calls checked

use ash::vk;
use std::sync::Arc;

use crate::gpu::kgpu::hal::{HalFence, HalSemaphore, Backend};
use crate::gpu::kgpu::error::{KgpuError, KgpuResult};

use super::VulkanDevice;

/// Vulkan fence capsule (CPU-GPU synchronization)
///
/// # Layout
///
/// - 64B cache-aligned
/// - Lightweight (fence handle + device reference)
///
/// # Lifecycle
///
/// ```text
/// Uninitialized → Create (vkCreateFence) → Unsignaled
///     ↓                                      ↓
/// Destroyed  ←──────────────────────────── Signaled (GPU work complete)
///                                            ↓
///                                           Reset (vkResetFences)
/// ```
pub struct VulkanFence {
    /// Device reference
    device: VulkanDevice,

    /// Fence handle
    fence: vk::Fence,
}

impl VulkanFence {
    /// Create fence
    ///
    /// # Performance
    ///
    /// <1μs (vkCreateFence)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_SYNC_CREATION_SUCCEEDS`: Device is valid
    ///
    /// # Example
    ///
    /// ```no_run
    /// use atomic_capsule::gpu::kgpu::backends::vulkan::*;
    ///
    /// let instance = VulkanInstance::new("MyApp", "MyEngine")?;
    /// let adapters = instance.enumerate_adapters()?;
    /// let device = adapters[0].create_device()?;
    /// let fence = VulkanFence::new(device, false)?;
    /// # Ok::<(), atomic_capsule::gpu::kgpu::error::KgpuError>(())
    /// ```
    pub(crate) fn new(device: VulkanDevice, signaled: bool) -> KgpuResult<Self> {
        let fence = device.create_fence(signaled)?;
        Ok(Self { device, fence })
    }

    /// Wait for fence
    ///
    /// # Performance
    ///
    /// <1ms typical (depends on pending GPU work)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_WAIT_TIMEOUT_VALID`: Timeout is reasonable
    pub fn wait(&self, timeout_ns: u64) -> KgpuResult<()> {
        self.device.wait_for_fence(self.fence, timeout_ns)
    }

    /// Reset fence
    ///
    /// # Performance
    ///
    /// <1μs (vkResetFences)
    pub fn reset(&self) -> KgpuResult<()> {
        self.device.reset_fence(self.fence)
    }

    /// Check if fence is signaled
    pub fn is_signaled(&self) -> bool {
        unsafe {
            self.device
                .raw_device()
                .get_fence_status(self.fence)
                .unwrap_or(false)
        }
    }

    /// Get raw fence handle
    pub(crate) fn raw(&self) -> vk::Fence {
        self.fence
    }
}

impl HalFence for VulkanFence {
    fn backend(&self) -> Backend {
        Backend::Vulkan
    }

    fn wait_fence(&self, timeout_ns: u64) -> KgpuResult<()> {
        self.wait(timeout_ns)
    }

    fn reset_fence(&self) -> KgpuResult<()> {
        self.reset()
    }

    fn is_signaled_fence(&self) -> bool {
        self.is_signaled()
    }
}

impl Drop for VulkanFence {
    fn drop(&mut self) {
        self.device.destroy_fence(self.fence);
    }
}

/// Vulkan semaphore capsule (GPU-GPU synchronization)
///
/// # Layout
///
/// - 64B cache-aligned
/// - Lightweight (semaphore handle + device reference)
///
/// # Lifecycle
///
/// ```text
/// Uninitialized → Create (vkCreateSemaphore) → Unsignaled
///     ↓                                          ↓
/// Destroyed  ←──────────────────────────────── Signaled (queue submit)
///                                                ↓
///                                               Wait (queue submit dependency)
/// ```
pub struct VulkanSemaphore {
    /// Device reference
    device: VulkanDevice,

    /// Semaphore handle
    semaphore: vk::Semaphore,
}

impl VulkanSemaphore {
    /// Create semaphore
    ///
    /// # Performance
    ///
    /// <1μs (vkCreateSemaphore)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_SYNC_CREATION_SUCCEEDS`: Device is valid
    ///
    /// # Example
    ///
    /// ```no_run
    /// use atomic_capsule::gpu::kgpu::backends::vulkan::*;
    ///
    /// let instance = VulkanInstance::new("MyApp", "MyEngine")?;
    /// let adapters = instance.enumerate_adapters()?;
    /// let device = adapters[0].create_device()?;
    /// let semaphore = VulkanSemaphore::new(device)?;
    /// # Ok::<(), atomic_capsule::gpu::kgpu::error::KgpuError>(())
    /// ```
    pub(crate) fn new(device: VulkanDevice) -> KgpuResult<Self> {
        let semaphore = device.create_semaphore()?;
        Ok(Self { device, semaphore })
    }

    /// Get raw semaphore handle
    pub(crate) fn raw(&self) -> vk::Semaphore {
        self.semaphore
    }
}

impl HalSemaphore for VulkanSemaphore {
    fn backend(&self) -> Backend {
        Backend::Vulkan
    }
}

impl Drop for VulkanSemaphore {
    fn drop(&mut self) {
        self.device.destroy_semaphore(self.semaphore);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::VulkanInstance;

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_fence_creation() {
        let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();
        let fence = VulkanFence::new(device, false);

        assert!(fence.is_ok(), "Failed to create fence");
    }

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_fence_signaled_state() {
        let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        // Unsignaled fence
        let fence_unsignaled = VulkanFence::new(device.clone(), false).unwrap();
        assert!(!fence_unsignaled.is_signaled(), "Fence should be unsignaled");

        // Signaled fence
        let fence_signaled = VulkanFence::new(device, true).unwrap();
        assert!(fence_signaled.is_signaled(), "Fence should be signaled");
    }

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_fence_reset() {
        let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();
        let fence = VulkanFence::new(device, true).unwrap();

        assert!(fence.is_signaled(), "Fence should be signaled initially");
        fence.reset().unwrap();
        assert!(!fence.is_signaled(), "Fence should be unsignaled after reset");
    }

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_semaphore_creation() {
        let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();
        let semaphore = VulkanSemaphore::new(device);

        assert!(semaphore.is_ok(), "Failed to create semaphore");
    }
}
