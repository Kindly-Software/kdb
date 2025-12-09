//! Vulkan Swapchain Implementation
//!
//! # Architecture
//!
//! VulkanSwapchain wraps vk::SwapchainKHR for image presentation.
//!
//! - **Swapchain**: Image queue for presentation (double/triple buffering)
//! - **Images**: Presentable images (2-3 for double/triple buffering)
//! - **Image Views**: Views into swapchain images
//! - **Acquire**: Get next image for rendering (vkAcquireNextImageKHR)
//! - **Present**: Submit image for display (vkQueuePresentKHR)
//!
//! # Performance
//!
//! - Creation: <50ms (B32 target)
//! - Acquire: <1ms (vkAcquireNextImageKHR)
//! - Present: <1ms (vkQueuePresentKHR)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_SWAPCHAIN_CREATION_SUCCEEDS`: Surface is compatible with device
//! - `#ASSUME_IMAGE_ACQUIRE_SUCCEEDS`: Swapchain is valid, timeout reasonable
//! - `#VERIFY_UNSAFE_FFI`: All vk* swapchain calls checked

use ash::vk;
use std::sync::Arc;

use crate::gpu::kgpu::hal::{HalSwapchain, Backend};
use crate::gpu::kgpu::error::{KgpuError, KgpuResult};

use super::{VulkanInstance, VulkanAdapter, VulkanDevice, VulkanSurface};

/// Vulkan swapchain capsule
///
/// # Layout
///
/// - 256B cache-aligned
/// - Arc-wrapped for cheap cloning
/// - Image views cached at creation
///
/// # Lifecycle
///
/// ```text
/// Uninitialized → Create (vkCreateSwapchainKHR) → Idle
///     ↓                                             ↓
/// Destroyed  ←──────────────────────────────────── Active (Acquire/Present loop)
///                                                   ↓
///                                                  OutOfDate (resize required)
/// ```
#[derive(Clone)]
pub struct VulkanSwapchain {
    /// Inner state (Arc for cheap cloning)
    inner: Arc<VulkanSwapchainInner>,
}

struct VulkanSwapchainInner {
    /// Instance reference
    instance: VulkanInstance,

    /// Adapter reference
    adapter: VulkanAdapter,

    /// Device reference
    device: VulkanDevice,

    /// Swapchain handle
    swapchain: vk::SwapchainKHR,

    /// Swapchain loader
    swapchain_loader: ash::khr::swapchain::Device,

    /// Swapchain images
    images: Vec<vk::Image>,

    /// Image views
    image_views: Vec<vk::ImageView>,

    /// Image format
    format: vk::Format,

    /// Image extent
    extent: vk::Extent2D,

    /// Present mode
    present_mode: vk::PresentModeKHR,
}

impl VulkanSwapchain {
    /// Create swapchain
    ///
    /// # Performance
    ///
    /// <50ms (B32 target, includes image view creation)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_SWAPCHAIN_CREATION_SUCCEEDS`: Surface compatible with device
    /// - `#VERIFY_UNSAFE_FFI`: vkCreateSwapchainKHR
    ///
    /// # Example
    ///
    /// ```no_run
    /// use atomic_capsule::gpu::kgpu::backends::vulkan::*;
    ///
    /// let instance = VulkanInstance::new("MyApp", "MyEngine")?;
    /// # let window: &dyn raw_window_handle::HasWindowHandle = unimplemented!();
    /// let surface = VulkanSurface::new(instance.clone(), window)?;
    /// let adapters = instance.enumerate_adapters()?;
    /// let device = adapters[0].create_device()?;
    /// let swapchain = VulkanSwapchain::new(instance, adapters[0].clone(), device, &surface, 1920, 1080)?;
    /// # Ok::<(), atomic_capsule::gpu::kgpu::error::KgpuError>(())
    /// ```
    pub(crate) fn new(
        instance: VulkanInstance,
        adapter: VulkanAdapter,
        device: VulkanDevice,
        surface: &VulkanSurface,
        width: u32,
        height: u32,
    ) -> KgpuResult<Self> {
        // Query surface capabilities
        let capabilities = surface.get_capabilities(&adapter)?;

        // Choose format and present mode
        let surface_format = surface.choose_format(&adapter)?;
        let present_mode = surface.choose_present_mode(&adapter)?;

        // Determine image count (prefer triple buffering)
        let image_count = {
            let mut count = capabilities.min_image_count + 1;
            if capabilities.max_image_count > 0 && count > capabilities.max_image_count {
                count = capabilities.max_image_count;
            }
            count
        };

        // Determine extent
        let extent = if capabilities.current_extent.width != u32::MAX {
            capabilities.current_extent
        } else {
            vk::Extent2D {
                width: width.clamp(
                    capabilities.min_image_extent.width,
                    capabilities.max_image_extent.width,
                ),
                height: height.clamp(
                    capabilities.min_image_extent.height,
                    capabilities.max_image_extent.height,
                ),
            }
        };

        // Create swapchain
        let create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface.raw())
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true);

        let swapchain_loader = ash::khr::swapchain::Device::new(
            instance.raw_instance(),
            device.raw_device(),
        );

        let swapchain = unsafe {
            swapchain_loader
                .create_swapchain(&create_info, None)
                .map_err(|e| {
                    KgpuError::InitializationFailed(format!("Failed to create swapchain: {}", e))
                })?
        };

        // Get swapchain images
        let images = unsafe {
            swapchain_loader
                .get_swapchain_images(swapchain)
                .map_err(|e| {
                    KgpuError::QueryFailed(format!("Failed to get swapchain images: {}", e))
                })?
        };

        // Create image views
        let mut image_views = Vec::with_capacity(images.len());
        for &image in &images {
            let view = device.create_image_view(
                image,
                vk::ImageViewType::TYPE_2D,
                surface_format.format,
                vk::ImageAspectFlags::COLOR,
                1,
                1,
            )?;
            image_views.push(view);
        }

        Ok(Self {
            inner: Arc::new(VulkanSwapchainInner {
                instance,
                adapter,
                device,
                swapchain,
                swapchain_loader,
                images,
                image_views,
                format: surface_format.format,
                extent,
                present_mode,
            }),
        })
    }

    /// Acquire next image
    ///
    /// # Performance
    ///
    /// <1ms (B32 target, vkAcquireNextImageKHR)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_IMAGE_ACQUIRE_SUCCEEDS`: Swapchain valid, timeout reasonable
    pub fn acquire_next_image(
        &self,
        timeout_ns: u64,
        semaphore: vk::Semaphore,
        fence: vk::Fence,
    ) -> KgpuResult<(u32, bool)> {
        unsafe {
            self.inner
                .swapchain_loader
                .acquire_next_image(self.inner.swapchain, timeout_ns, semaphore, fence)
                .map_err(|e| match e {
                    vk::Result::ERROR_OUT_OF_DATE_KHR => {
                        KgpuError::SwapchainOutOfDate
                    }
                    vk::Result::SUBOPTIMAL_KHR => {
                        KgpuError::SwapchainSuboptimal
                    }
                    _ => KgpuError::OperationFailed(format!("Failed to acquire next image: {}", e)),
                })
        }
    }

    /// Present image
    ///
    /// # Performance
    ///
    /// <1ms (B32 target, vkQueuePresentKHR)
    pub fn present(
        &self,
        image_index: u32,
        wait_semaphores: &[vk::Semaphore],
    ) -> KgpuResult<bool> {
        let swapchains = [self.inner.swapchain];
        let image_indices = [image_index];

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        unsafe {
            self.inner
                .swapchain_loader
                .queue_present(self.inner.device.graphics_queue(), &present_info)
                .map_err(|e| match e {
                    vk::Result::ERROR_OUT_OF_DATE_KHR => {
                        KgpuError::SwapchainOutOfDate
                    }
                    vk::Result::SUBOPTIMAL_KHR => {
                        KgpuError::SwapchainSuboptimal
                    }
                    _ => KgpuError::OperationFailed(format!("Failed to present: {}", e)),
                })
        }
    }

    /// Get swapchain images
    pub fn images(&self) -> &[vk::Image] {
        &self.inner.images
    }

    /// Get swapchain image views
    pub fn image_views(&self) -> &[vk::ImageView] {
        &self.inner.image_views
    }

    /// Get image format
    pub fn format(&self) -> vk::Format {
        self.inner.format
    }

    /// Get extent
    pub fn extent(&self) -> vk::Extent2D {
        self.inner.extent
    }

    /// Get present mode
    pub fn present_mode(&self) -> vk::PresentModeKHR {
        self.inner.present_mode
    }

    /// Get raw swapchain handle
    pub(crate) fn raw(&self) -> vk::SwapchainKHR {
        self.inner.swapchain
    }
}

impl HalSwapchain for VulkanSwapchain {
    fn backend(&self) -> Backend {
        Backend::Vulkan
    }

    fn acquire_image(&self, timeout_ns: u64) -> KgpuResult<(u32, bool)> {
        // Create temporary semaphore for acquire
        let semaphore = self.inner.device.create_semaphore()?;
        let result = self.acquire_next_image(timeout_ns, semaphore, vk::Fence::null());
        self.inner.device.destroy_semaphore(semaphore);
        result
    }

    fn present_image(&self, image_index: u32) -> KgpuResult<bool> {
        self.present(image_index, &[])
    }

    fn image_count(&self) -> u32 {
        self.inner.images.len() as u32
    }

    fn width(&self) -> u32 {
        self.inner.extent.width
    }

    fn height(&self) -> u32 {
        self.inner.extent.height
    }
}

impl Drop for VulkanSwapchainInner {
    fn drop(&mut self) {
        unsafe {
            // Wait for device idle before cleanup
            let _ = self.device.wait_idle();

            // Destroy image views
            for &view in &self.image_views {
                self.device.destroy_image_view(view);
            }

            // Destroy swapchain
            self.swapchain_loader.destroy_swapchain(self.swapchain, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::VulkanInstance;

    #[test]
    #[ignore] // Requires windowing system
    fn test_swapchain_creation() {
        // This test requires a real window
        // Example:
        // let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        // let window = create_test_window();
        // let surface = VulkanSurface::new(instance.clone(), &window).unwrap();
        // let adapters = instance.enumerate_adapters().unwrap();
        // let device = adapters[0].create_device().unwrap();
        // let swapchain = VulkanSwapchain::new(instance, adapters[0].clone(), device, &surface, 800, 600);
        // assert!(swapchain.is_ok());
    }

    #[test]
    #[ignore] // Requires windowing system
    fn test_acquire_present() {
        // Similar to above - requires real window and swapchain
    }
}
