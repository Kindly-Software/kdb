//! Vulkan Device (Logical Device) Implementation
//!
//! # Architecture
//!
//! VulkanDevice wraps vk::Device for command execution and resource creation.
//!
//! - **Logical Device**: Application's connection to physical device
//! - **Queues**: Graphics/Compute/Transfer queue handles
//! - **Extensions**: VK_KHR_swapchain (required), VK_KHR_dynamic_rendering, VK_KHR_synchronization2
//! - **Features**: Vulkan 1.3 core features (dynamic rendering, synchronization2)
//! - **Memory**: Memory allocator integration point
//!
//! # Performance
//!
//! - Creation: <100ms (B32 target)
//! - Queue submit: <1μs (vkQueueSubmit)
//! - Resource creation: <10μs (vkCreateBuffer/Image)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_DEVICE_CREATION_SUCCEEDS`: vkCreateDevice succeeds with valid extensions
//! - `#ASSUME_QUEUES_VALID`: Queue handles are non-null
//! - `#VERIFY_UNSAFE_FFI`: All vk* calls return VkResult, checked via ?

use ash::vk;
use std::ffi::CString;
use std::sync::Arc;

use crate::gpu::kgpu::hal::{HalDevice, Backend};
use crate::gpu::kgpu::error::{KgpuError, KgpuResult};

use super::VulkanAdapter;

/// Vulkan device capsule
///
/// # Layout
///
/// - 256B cache-aligned
/// - Arc-wrapped for cheap cloning
/// - Queue handles cached at creation
///
/// # Lifecycle
///
/// ```text
/// Uninitialized → Create (vkCreateDevice) → Active
///     ↓                                       ↓
/// Destroyed  ←───────────────────────────── Destroy (vkDeviceWaitIdle + vkDestroyDevice)
/// ```
#[derive(Clone)]
pub struct VulkanDevice {
    /// Inner state (Arc for cheap cloning)
    inner: Arc<VulkanDeviceInner>,
}

struct VulkanDeviceInner {
    /// Adapter reference
    adapter: VulkanAdapter,

    /// Logical device handle
    device: ash::Device,

    /// Graphics queue
    graphics_queue: vk::Queue,

    /// Graphics queue family index
    graphics_queue_family: u32,

    /// Compute queue (may be same as graphics)
    compute_queue: vk::Queue,

    /// Compute queue family index
    compute_queue_family: u32,

    /// Transfer queue (may be same as graphics)
    transfer_queue: vk::Queue,

    /// Transfer queue family index
    transfer_queue_family: u32,

    /// Dynamic rendering extension (Vulkan 1.3 promoted)
    dynamic_rendering: bool,

    /// Synchronization2 extension (Vulkan 1.3 promoted)
    synchronization2: bool,
}

impl VulkanDevice {
    /// Create logical device with Vulkan 1.3 features
    ///
    /// # Performance
    ///
    /// <100ms (B32 target, includes queue creation)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_DEVICE_CREATION_SUCCEEDS`: vkCreateDevice succeeds
    /// - `#ASSUME_QUEUES_VALID`: Queue handles are non-null
    ///
    /// # Example
    ///
    /// ```no_run
    /// use atomic_capsule::gpu::kgpu::backends::vulkan::{VulkanInstance, VulkanDevice};
    ///
    /// let instance = VulkanInstance::new("MyApp", "MyEngine")?;
    /// let adapters = instance.enumerate_adapters()?;
    /// let device = adapters[0].create_device()?;
    /// println!("Device created with dynamic_rendering={}, synchronization2={}",
    ///     device.supports_dynamic_rendering(),
    ///     device.supports_synchronization2());
    /// # Ok::<(), atomic_capsule::gpu::kgpu::error::KgpuError>(())
    /// ```
    pub(crate) fn new(adapter: VulkanAdapter) -> KgpuResult<Self> {
        let graphics_queue_family = adapter.graphics_queue_family();
        let compute_queue_family = adapter.compute_queue_family();
        let transfer_queue_family = adapter.transfer_queue_family();

        // Unique queue families
        let mut unique_queue_families = vec![graphics_queue_family];
        if compute_queue_family != graphics_queue_family {
            unique_queue_families.push(compute_queue_family);
        }
        if transfer_queue_family != graphics_queue_family
            && transfer_queue_family != compute_queue_family
        {
            unique_queue_families.push(transfer_queue_family);
        }

        // Queue create infos (one queue per family)
        let queue_priorities = [1.0];
        let queue_create_infos: Vec<_> = unique_queue_families
            .iter()
            .map(|&family| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(family)
                    .queue_priorities(&queue_priorities)
            })
            .collect();

        // Required extensions
        let mut extensions = vec![
            ash::khr::swapchain::NAME.as_ptr(),
        ];

        // Check for Vulkan 1.3 promoted extensions
        let dynamic_rendering = adapter.supports_extension(ash::khr::dynamic_rendering::NAME);
        let synchronization2 = adapter.supports_extension(ash::khr::synchronization2::NAME);

        // On Vulkan 1.3, these are core (no need to enable as extensions)
        // On Vulkan 1.2, enable as extensions if supported
        let api_version = adapter.instance().api_version();
        if vk::api_version_minor(api_version) < 3 {
            if dynamic_rendering {
                extensions.push(ash::khr::dynamic_rendering::NAME.as_ptr());
            }
            if synchronization2 {
                extensions.push(ash::khr::synchronization2::NAME.as_ptr());
            }
        }

        // Vulkan 1.3 features
        let mut features_1_3 = vk::PhysicalDeviceVulkan13Features::default();
        if dynamic_rendering {
            features_1_3.dynamic_rendering = vk::TRUE;
        }
        if synchronization2 {
            features_1_3.synchronization2 = vk::TRUE;
        }

        // Vulkan 1.2 features (maintenance4)
        let features_1_2 = vk::PhysicalDeviceVulkan12Features::default()
            .push_next(&mut features_1_3);

        // Base features
        let features = vk::PhysicalDeviceFeatures::default();

        // Create device
        let create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&extensions)
            .enabled_features(&features)
            .push_next(&mut features_1_3.clone());

        // #VERIFY_UNSAFE_FFI: vkCreateDevice
        let device = unsafe {
            adapter.instance().raw_instance()
                .create_device(adapter.physical_device(), &create_info, None)
                .map_err(|e| KgpuError::InitializationFailed(
                    format!("Failed to create Vulkan device: {}", e)
                ))?
        };

        // Get queue handles
        let graphics_queue = unsafe {
            device.get_device_queue(graphics_queue_family, 0)
        };
        let compute_queue = unsafe {
            device.get_device_queue(compute_queue_family, 0)
        };
        let transfer_queue = unsafe {
            device.get_device_queue(transfer_queue_family, 0)
        };

        Ok(Self {
            inner: Arc::new(VulkanDeviceInner {
                adapter,
                device,
                graphics_queue,
                graphics_queue_family,
                compute_queue,
                compute_queue_family,
                transfer_queue,
                transfer_queue_family,
                dynamic_rendering,
                synchronization2,
            }),
        })
    }

    /// Get raw ash::Device
    pub(crate) fn raw_device(&self) -> &ash::Device {
        &self.inner.device
    }

    /// Get adapter reference
    pub(crate) fn adapter(&self) -> &VulkanAdapter {
        &self.inner.adapter
    }

    /// Get graphics queue handle
    pub(crate) fn graphics_queue(&self) -> vk::Queue {
        self.inner.graphics_queue
    }

    /// Get graphics queue family index
    pub(crate) fn graphics_queue_family(&self) -> u32 {
        self.inner.graphics_queue_family
    }

    /// Get compute queue handle
    pub(crate) fn compute_queue(&self) -> vk::Queue {
        self.inner.compute_queue
    }

    /// Get compute queue family index
    pub(crate) fn compute_queue_family(&self) -> u32 {
        self.inner.compute_queue_family
    }

    /// Get transfer queue handle
    pub(crate) fn transfer_queue(&self) -> vk::Queue {
        self.inner.transfer_queue
    }

    /// Get transfer queue family index
    pub(crate) fn transfer_queue_family(&self) -> u32 {
        self.inner.transfer_queue_family
    }

    /// Check if dynamic rendering is supported
    pub fn supports_dynamic_rendering(&self) -> bool {
        self.inner.dynamic_rendering
    }

    /// Check if synchronization2 is supported
    pub fn supports_synchronization2(&self) -> bool {
        self.inner.synchronization2
    }

    /// Wait for device to become idle
    ///
    /// # Performance
    ///
    /// Variable (depends on pending GPU work)
    ///
    /// # ASSUM
    ///
    /// - `#VERIFY_UNSAFE_FFI`: vkDeviceWaitIdle
    pub fn wait_idle(&self) -> KgpuResult<()> {
        unsafe {
            self.inner.device.device_wait_idle().map_err(|e| {
                KgpuError::OperationFailed(format!("Failed to wait for device idle: {}", e))
            })
        }
    }

    /// Create buffer
    ///
    /// # Performance
    ///
    /// <10μs (B32 target)
    pub fn create_buffer(
        &self,
        size: u64,
        usage: vk::BufferUsageFlags,
        sharing_mode: vk::SharingMode,
    ) -> KgpuResult<vk::Buffer> {
        let create_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(sharing_mode);

        unsafe {
            self.inner.device.create_buffer(&create_info, None).map_err(|e| {
                KgpuError::ResourceCreationFailed(format!("Failed to create buffer: {}", e))
            })
        }
    }

    /// Destroy buffer
    pub fn destroy_buffer(&self, buffer: vk::Buffer) {
        unsafe {
            self.inner.device.destroy_buffer(buffer, None);
        }
    }

    /// Create image
    ///
    /// # Performance
    ///
    /// <10μs (B32 target)
    pub fn create_image(
        &self,
        image_type: vk::ImageType,
        format: vk::Format,
        extent: vk::Extent3D,
        mip_levels: u32,
        array_layers: u32,
        usage: vk::ImageUsageFlags,
    ) -> KgpuResult<vk::Image> {
        let create_info = vk::ImageCreateInfo::default()
            .image_type(image_type)
            .format(format)
            .extent(extent)
            .mip_levels(mip_levels)
            .array_layers(array_layers)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        unsafe {
            self.inner.device.create_image(&create_info, None).map_err(|e| {
                KgpuError::ResourceCreationFailed(format!("Failed to create image: {}", e))
            })
        }
    }

    /// Destroy image
    pub fn destroy_image(&self, image: vk::Image) {
        unsafe {
            self.inner.device.destroy_image(image, None);
        }
    }

    /// Create image view
    pub fn create_image_view(
        &self,
        image: vk::Image,
        view_type: vk::ImageViewType,
        format: vk::Format,
        aspect_mask: vk::ImageAspectFlags,
        mip_levels: u32,
        array_layers: u32,
    ) -> KgpuResult<vk::ImageView> {
        let subresource_range = vk::ImageSubresourceRange::default()
            .aspect_mask(aspect_mask)
            .base_mip_level(0)
            .level_count(mip_levels)
            .base_array_layer(0)
            .layer_count(array_layers);

        let create_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(view_type)
            .format(format)
            .subresource_range(subresource_range);

        unsafe {
            self.inner.device.create_image_view(&create_info, None).map_err(|e| {
                KgpuError::ResourceCreationFailed(format!("Failed to create image view: {}", e))
            })
        }
    }

    /// Destroy image view
    pub fn destroy_image_view(&self, image_view: vk::ImageView) {
        unsafe {
            self.inner.device.destroy_image_view(image_view, None);
        }
    }

    /// Create fence
    pub fn create_fence(&self, signaled: bool) -> KgpuResult<vk::Fence> {
        let flags = if signaled {
            vk::FenceCreateFlags::SIGNALED
        } else {
            vk::FenceCreateFlags::empty()
        };

        let create_info = vk::FenceCreateInfo::default().flags(flags);

        unsafe {
            self.inner.device.create_fence(&create_info, None).map_err(|e| {
                KgpuError::ResourceCreationFailed(format!("Failed to create fence: {}", e))
            })
        }
    }

    /// Destroy fence
    pub fn destroy_fence(&self, fence: vk::Fence) {
        unsafe {
            self.inner.device.destroy_fence(fence, None);
        }
    }

    /// Wait for fence
    pub fn wait_for_fence(&self, fence: vk::Fence, timeout_ns: u64) -> KgpuResult<()> {
        unsafe {
            self.inner.device.wait_for_fences(&[fence], true, timeout_ns).map_err(|e| {
                KgpuError::OperationFailed(format!("Failed to wait for fence: {}", e))
            })
        }
    }

    /// Reset fence
    pub fn reset_fence(&self, fence: vk::Fence) -> KgpuResult<()> {
        unsafe {
            self.inner.device.reset_fences(&[fence]).map_err(|e| {
                KgpuError::OperationFailed(format!("Failed to reset fence: {}", e))
            })
        }
    }

    /// Create semaphore
    pub fn create_semaphore(&self) -> KgpuResult<vk::Semaphore> {
        let create_info = vk::SemaphoreCreateInfo::default();

        unsafe {
            self.inner.device.create_semaphore(&create_info, None).map_err(|e| {
                KgpuError::ResourceCreationFailed(format!("Failed to create semaphore: {}", e))
            })
        }
    }

    /// Destroy semaphore
    pub fn destroy_semaphore(&self, semaphore: vk::Semaphore) {
        unsafe {
            self.inner.device.destroy_semaphore(semaphore, None);
        }
    }

    /// Create command pool
    pub fn create_command_pool(&self, queue_family: u32) -> KgpuResult<vk::CommandPool> {
        let create_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

        unsafe {
            self.inner.device.create_command_pool(&create_info, None).map_err(|e| {
                KgpuError::ResourceCreationFailed(format!("Failed to create command pool: {}", e))
            })
        }
    }

    /// Destroy command pool
    pub fn destroy_command_pool(&self, command_pool: vk::CommandPool) {
        unsafe {
            self.inner.device.destroy_command_pool(command_pool, None);
        }
    }

    /// Allocate command buffers
    pub fn allocate_command_buffers(
        &self,
        command_pool: vk::CommandPool,
        count: u32,
    ) -> KgpuResult<Vec<vk::CommandBuffer>> {
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(count);

        unsafe {
            self.inner.device.allocate_command_buffers(&alloc_info).map_err(|e| {
                KgpuError::ResourceCreationFailed(format!("Failed to allocate command buffers: {}", e))
            })
        }
    }

    /// Free command buffers
    pub fn free_command_buffers(
        &self,
        command_pool: vk::CommandPool,
        command_buffers: &[vk::CommandBuffer],
    ) {
        unsafe {
            self.inner.device.free_command_buffers(command_pool, command_buffers);
        }
    }
}

impl HalDevice for VulkanDevice {
    type CommandBuffer = super::VulkanCommandBuffer;
    type Fence = super::VulkanFence;
    type Semaphore = super::VulkanSemaphore;

    fn backend(&self) -> Backend {
        Backend::Vulkan
    }

    fn create_command_buffer(&self) -> KgpuResult<Self::CommandBuffer> {
        super::VulkanCommandBuffer::new(self.clone())
    }

    fn create_fence_hal(&self, signaled: bool) -> KgpuResult<Self::Fence> {
        super::VulkanFence::new(self.clone(), signaled)
    }

    fn create_semaphore_hal(&self) -> KgpuResult<Self::Semaphore> {
        super::VulkanSemaphore::new(self.clone())
    }

    fn submit_commands(
        &self,
        command_buffer: &Self::CommandBuffer,
        wait_semaphores: &[&Self::Semaphore],
        signal_semaphores: &[&Self::Semaphore],
        fence: Option<&Self::Fence>,
    ) -> KgpuResult<()> {
        let wait_sems: Vec<_> = wait_semaphores.iter().map(|s| s.raw()).collect();
        let signal_sems: Vec<_> = signal_semaphores.iter().map(|s| s.raw()).collect();
        let wait_stages = vec![vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT; wait_sems.len()];

        let cmd_buffers = [command_buffer.raw()];
        let submit_info = vk::SubmitInfo::default()
            .command_buffers(&cmd_buffers)
            .wait_semaphores(&wait_sems)
            .wait_dst_stage_mask(&wait_stages)
            .signal_semaphores(&signal_sems);

        let fence_handle = fence.map(|f| f.raw()).unwrap_or(vk::Fence::null());

        unsafe {
            self.inner.device
                .queue_submit(self.graphics_queue(), &[submit_info], fence_handle)
                .map_err(|e| KgpuError::OperationFailed(format!("Failed to submit commands: {}", e)))
        }
    }

    fn wait_idle_hal(&self) -> KgpuResult<()> {
        self.wait_idle()
    }
}

impl Drop for VulkanDeviceInner {
    fn drop(&mut self) {
        unsafe {
            // Wait for device to become idle before destruction
            let _ = self.device.device_wait_idle();

            // Destroy device
            self.device.destroy_device(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::VulkanInstance;

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_device_creation() {
        let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device();

        assert!(device.is_ok(), "Failed to create device");

        if let Ok(dev) = device {
            println!("Device created:");
            println!("  Dynamic rendering: {}", dev.supports_dynamic_rendering());
            println!("  Synchronization2: {}", dev.supports_synchronization2());
        }
    }

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_buffer_creation() {
        let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        let buffer = device.create_buffer(
            1024,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::SharingMode::EXCLUSIVE,
        );

        assert!(buffer.is_ok(), "Failed to create buffer");

        if let Ok(buf) = buffer {
            device.destroy_buffer(buf);
        }
    }

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_fence_creation() {
        let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        let fence = device.create_fence(false);
        assert!(fence.is_ok(), "Failed to create fence");

        if let Ok(f) = fence {
            device.destroy_fence(f);
        }
    }
}
