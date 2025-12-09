//! Vulkan Surface Implementation
//!
//! # Architecture
//!
//! VulkanSurface wraps vk::SurfaceKHR for window presentation via ash-window.
//!
//! - **Platform Surface**: Win32/Xlib/Wayland/Metal surface creation
//! - **Capabilities**: Min/max image count, extent, transforms
//! - **Formats**: SRGB/linear color space, BGRA8/RGBA8 pixel formats
//! - **Present Modes**: Immediate/FIFO/Mailbox/Relaxed FIFO
//!
//! # Performance
//!
//! - Creation: <10ms (B32 target)
//! - Capability query: <1ms (vkGetPhysicalDeviceSurfaceCapabilitiesKHR)
//! - Format query: <1ms (vkGetPhysicalDeviceSurfaceFormatsKHR)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_SURFACE_CREATION_SUCCEEDS`: Window handle is valid
//! - `#ASSUME_FORMATS_AVAILABLE`: At least one surface format supported
//! - `#VERIFY_UNSAFE_FFI`: All vk* surface queries checked

use ash::vk;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::gpu::kgpu::hal::{HalSurface, Backend};
use crate::gpu::kgpu::error::{KgpuError, KgpuResult};

use super::{VulkanInstance, VulkanAdapter};

/// Vulkan surface capsule
///
/// # Layout
///
/// - 128B cache-aligned
/// - Platform-specific surface handle (vk::SurfaceKHR)
/// - Instance reference for cleanup
///
/// # Lifecycle
///
/// ```text
/// Uninitialized → Create (platform vkCreate*SurfaceKHR) → Active
///     ↓                                                     ↓
/// Destroyed  ←────────────────────────────────────────── Destroy (vkDestroySurfaceKHR)
/// ```
pub struct VulkanSurface {
    /// Instance reference
    instance: VulkanInstance,

    /// Surface handle
    surface: vk::SurfaceKHR,

    /// Surface loader (for queries and cleanup)
    surface_loader: ash::khr::surface::Instance,
}

impl VulkanSurface {
    /// Create surface from window handle
    ///
    /// # Performance
    ///
    /// <10ms (B32 target, platform surface creation)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_SURFACE_CREATION_SUCCEEDS`: Window handle is valid
    /// - `#VERIFY_UNSAFE_FFI`: Platform vkCreate*SurfaceKHR
    ///
    /// # Example
    ///
    /// ```no_run
    /// use atomic_capsule::gpu::kgpu::backends::vulkan::{VulkanInstance, VulkanSurface};
    /// use raw_window_handle::HasWindowHandle;
    ///
    /// let instance = VulkanInstance::new("MyApp", "MyEngine")?;
    /// # let window: &dyn HasWindowHandle = unimplemented!();
    /// let surface = VulkanSurface::new(instance, window)?;
    /// # Ok::<(), atomic_capsule::gpu::kgpu::error::KgpuError>(())
    /// ```
    pub(crate) fn new(
        instance: VulkanInstance,
        window: &dyn HasWindowHandle,
    ) -> KgpuResult<Self> {
        let surface_loader = ash::khr::surface::Instance::new(
            instance.entry(),
            instance.raw_instance(),
        );

        // Create platform-specific surface via ash-window
        let surface = unsafe {
            ash_window::create_surface(
                instance.entry(),
                instance.raw_instance(),
                window.window_handle().unwrap().as_raw(),
                None,
            ).map_err(|e| {
                KgpuError::InitializationFailed(format!("Failed to create surface: {}", e))
            })?
        };

        Ok(Self {
            instance,
            surface,
            surface_loader,
        })
    }

    /// Get surface capabilities
    pub fn get_capabilities(&self, adapter: &VulkanAdapter) -> KgpuResult<vk::SurfaceCapabilitiesKHR> {
        unsafe {
            self.surface_loader
                .get_physical_device_surface_capabilities(
                    adapter.physical_device(),
                    self.surface,
                )
                .map_err(|e| {
                    KgpuError::QueryFailed(format!("Failed to query surface capabilities: {}", e))
                })
        }
    }

    /// Get supported surface formats
    pub fn get_formats(&self, adapter: &VulkanAdapter) -> KgpuResult<Vec<vk::SurfaceFormatKHR>> {
        unsafe {
            self.surface_loader
                .get_physical_device_surface_formats(
                    adapter.physical_device(),
                    self.surface,
                )
                .map_err(|e| {
                    KgpuError::QueryFailed(format!("Failed to query surface formats: {}", e))
                })
        }
    }

    /// Get supported present modes
    pub fn get_present_modes(&self, adapter: &VulkanAdapter) -> KgpuResult<Vec<vk::PresentModeKHR>> {
        unsafe {
            self.surface_loader
                .get_physical_device_surface_present_modes(
                    adapter.physical_device(),
                    self.surface,
                )
                .map_err(|e| {
                    KgpuError::QueryFailed(format!("Failed to query present modes: {}", e))
                })
        }
    }

    /// Choose best surface format (prefer SRGB)
    ///
    /// # Priority
    ///
    /// 1. B8G8R8A8_SRGB + SRGB_NONLINEAR (ideal)
    /// 2. R8G8B8A8_SRGB + SRGB_NONLINEAR
    /// 3. First available format (fallback)
    pub fn choose_format(&self, adapter: &VulkanAdapter) -> KgpuResult<vk::SurfaceFormatKHR> {
        let formats = self.get_formats(adapter)?;

        if formats.is_empty() {
            return Err(KgpuError::QueryFailed("No surface formats available".to_string()));
        }

        // Prefer SRGB formats
        for format in &formats {
            if (format.format == vk::Format::B8G8R8A8_SRGB
                || format.format == vk::Format::R8G8B8A8_SRGB)
                && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            {
                return Ok(*format);
            }
        }

        // Fallback to first format
        Ok(formats[0])
    }

    /// Choose best present mode (prefer Mailbox for low latency)
    ///
    /// # Priority
    ///
    /// 1. MAILBOX (triple buffering, low latency)
    /// 2. IMMEDIATE (no VSync, lowest latency, tearing)
    /// 3. FIFO (VSync, always available, guaranteed)
    pub fn choose_present_mode(&self, adapter: &VulkanAdapter) -> KgpuResult<vk::PresentModeKHR> {
        let modes = self.get_present_modes(adapter)?;

        // Prefer mailbox (triple buffering)
        if modes.contains(&vk::PresentModeKHR::MAILBOX) {
            return Ok(vk::PresentModeKHR::MAILBOX);
        }

        // Fallback to immediate (no VSync, tearing)
        if modes.contains(&vk::PresentModeKHR::IMMEDIATE) {
            return Ok(vk::PresentModeKHR::IMMEDIATE);
        }

        // FIFO is always available (Vulkan spec guarantee)
        Ok(vk::PresentModeKHR::FIFO)
    }

    /// Get raw surface handle
    pub(crate) fn raw(&self) -> vk::SurfaceKHR {
        self.surface
    }

    /// Get surface loader
    pub(crate) fn loader(&self) -> &ash::khr::surface::Instance {
        &self.surface_loader
    }
}

impl HalSurface for VulkanSurface {
    type Swapchain = super::VulkanSwapchain;

    fn backend(&self) -> Backend {
        Backend::Vulkan
    }

    fn create_swapchain(
        &self,
        adapter: &VulkanAdapter,
        device: &super::VulkanDevice,
        width: u32,
        height: u32,
    ) -> KgpuResult<Self::Swapchain> {
        super::VulkanSwapchain::new(
            self.instance.clone(),
            adapter.clone(),
            device.clone(),
            self,
            width,
            height,
        )
    }
}

impl Drop for VulkanSurface {
    fn drop(&mut self) {
        unsafe {
            self.surface_loader.destroy_surface(self.surface, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::VulkanInstance;

    // Note: Surface tests require a real window, which is not available in headless CI
    // These tests are marked #[ignore] and should be run manually with a window

    #[test]
    #[ignore] // Requires windowing system
    fn test_surface_capabilities() {
        // This test would require creating a real window
        // Example:
        // let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        // let window = create_test_window();
        // let surface = VulkanSurface::new(instance.clone(), &window).unwrap();
        // let adapters = instance.enumerate_adapters().unwrap();
        // let caps = surface.get_capabilities(&adapters[0]).unwrap();
        // assert!(caps.min_image_count > 0);
    }

    #[test]
    #[ignore] // Requires windowing system
    fn test_surface_formats() {
        // Similar to above - requires real window
    }

    #[test]
    #[ignore] // Requires windowing system
    fn test_present_modes() {
        // Similar to above - requires real window
    }
}
