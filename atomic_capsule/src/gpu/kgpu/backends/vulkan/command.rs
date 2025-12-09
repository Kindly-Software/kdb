//! Vulkan Command Buffer Implementation
//!
//! # Architecture
//!
//! VulkanCommandBuffer wraps vk::CommandBuffer for GPU command recording and submission.
//!
//! - **Command Pool**: Per-thread command buffer allocator
//! - **Command Buffer**: Primary command buffer (vk::CommandBuffer)
//! - **Recording**: Begin/end recording (vkBeginCommandBuffer/vkEndCommandBuffer)
//! - **Submission**: Submit to queue (vkQueueSubmit)
//! - **Reset**: Reset for reuse (vkResetCommandBuffer)
//!
//! # Performance
//!
//! - Allocation: <10μs (from pool)
//! - Begin recording: <1μs (vkBeginCommandBuffer)
//! - End recording: <1μs (vkEndCommandBuffer)
//! - Submit: <1μs (vkQueueSubmit)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_COMMAND_BUFFER_VALID`: Command pool and buffer are valid
//! - `#ASSUME_RECORDING_STATE_VALID`: Begin before recording, end before submit
//! - `#VERIFY_UNSAFE_FFI`: All vk* command calls checked

use ash::vk;
use std::sync::Arc;

use crate::gpu::kgpu::hal::{HalCommandBuffer, Backend};
use crate::gpu::kgpu::error::{KgpuError, KgpuResult};

use super::VulkanDevice;

/// Vulkan command buffer capsule
///
/// # Layout
///
/// - 128B cache-aligned
/// - Command pool per device (not per thread for simplicity)
/// - Primary command buffer
///
/// # Lifecycle
///
/// ```text
/// Uninitialized → Allocate → Initial
///     ↓                        ↓
/// Destroyed  ←──────────────── Recording ⟷ Executable
///                              ↑           ↓
///                              └─── Submit ──→ Pending → Complete
/// ```
pub struct VulkanCommandBuffer {
    /// Device reference
    device: VulkanDevice,

    /// Command pool
    command_pool: vk::CommandPool,

    /// Command buffer
    command_buffer: vk::CommandBuffer,

    /// Recording state (true if in recording mode)
    recording: bool,
}

impl VulkanCommandBuffer {
    /// Create command buffer
    ///
    /// # Performance
    ///
    /// <10μs (B32 target, pool creation + buffer allocation)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_COMMAND_BUFFER_VALID`: Pool and buffer creation succeed
    ///
    /// # Example
    ///
    /// ```no_run
    /// use atomic_capsule::gpu::kgpu::backends::vulkan::*;
    ///
    /// let instance = VulkanInstance::new("MyApp", "MyEngine")?;
    /// let adapters = instance.enumerate_adapters()?;
    /// let device = adapters[0].create_device()?;
    /// let cmd = VulkanCommandBuffer::new(device)?;
    /// # Ok::<(), atomic_capsule::gpu::kgpu::error::KgpuError>(())
    /// ```
    pub(crate) fn new(device: VulkanDevice) -> KgpuResult<Self> {
        // Create command pool
        let command_pool = device.create_command_pool(device.graphics_queue_family())?;

        // Allocate command buffer
        let command_buffers = device.allocate_command_buffers(command_pool, 1)?;
        let command_buffer = command_buffers[0];

        Ok(Self {
            device,
            command_pool,
            command_buffer,
            recording: false,
        })
    }

    /// Begin recording
    ///
    /// # Performance
    ///
    /// <1μs (vkBeginCommandBuffer)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_RECORDING_STATE_VALID`: Not already recording
    pub fn begin(&mut self) -> KgpuResult<()> {
        if self.recording {
            return Err(KgpuError::InvalidState("Command buffer already recording".to_string()));
        }

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe {
            self.device
                .raw_device()
                .begin_command_buffer(self.command_buffer, &begin_info)
                .map_err(|e| {
                    KgpuError::OperationFailed(format!("Failed to begin command buffer: {}", e))
                })?;
        }

        self.recording = true;
        Ok(())
    }

    /// End recording
    ///
    /// # Performance
    ///
    /// <1μs (vkEndCommandBuffer)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_RECORDING_STATE_VALID`: Currently recording
    pub fn end(&mut self) -> KgpuResult<()> {
        if !self.recording {
            return Err(KgpuError::InvalidState("Command buffer not recording".to_string()));
        }

        unsafe {
            self.device
                .raw_device()
                .end_command_buffer(self.command_buffer)
                .map_err(|e| {
                    KgpuError::OperationFailed(format!("Failed to end command buffer: {}", e))
                })?;
        }

        self.recording = false;
        Ok(())
    }

    /// Reset command buffer
    ///
    /// # Performance
    ///
    /// <1μs (vkResetCommandBuffer)
    pub fn reset(&mut self) -> KgpuResult<()> {
        unsafe {
            self.device
                .raw_device()
                .reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty())
                .map_err(|e| {
                    KgpuError::OperationFailed(format!("Failed to reset command buffer: {}", e))
                })?;
        }

        self.recording = false;
        Ok(())
    }

    /// Copy buffer to buffer
    pub fn copy_buffer(
        &mut self,
        src: vk::Buffer,
        dst: vk::Buffer,
        size: u64,
    ) -> KgpuResult<()> {
        if !self.recording {
            return Err(KgpuError::InvalidState("Command buffer not recording".to_string()));
        }

        let region = vk::BufferCopy::default()
            .src_offset(0)
            .dst_offset(0)
            .size(size);

        unsafe {
            self.device.raw_device().cmd_copy_buffer(
                self.command_buffer,
                src,
                dst,
                &[region],
            );
        }

        Ok(())
    }

    /// Copy buffer to image
    pub fn copy_buffer_to_image(
        &mut self,
        buffer: vk::Buffer,
        image: vk::Image,
        width: u32,
        height: u32,
    ) -> KgpuResult<()> {
        if !self.recording {
            return Err(KgpuError::InvalidState("Command buffer not recording".to_string()));
        }

        let region = vk::BufferImageCopy::default()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(
                vk::ImageSubresourceLayers::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .mip_level(0)
                    .base_array_layer(0)
                    .layer_count(1),
            )
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
            .image_extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            });

        unsafe {
            self.device.raw_device().cmd_copy_buffer_to_image(
                self.command_buffer,
                buffer,
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
            );
        }

        Ok(())
    }

    /// Pipeline barrier (image layout transition)
    pub fn pipeline_barrier_image(
        &mut self,
        image: vk::Image,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        src_stage: vk::PipelineStageFlags,
        dst_stage: vk::PipelineStageFlags,
        src_access: vk::AccessFlags,
        dst_access: vk::AccessFlags,
    ) -> KgpuResult<()> {
        if !self.recording {
            return Err(KgpuError::InvalidState("Command buffer not recording".to_string()));
        }

        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1),
            )
            .src_access_mask(src_access)
            .dst_access_mask(dst_access);

        unsafe {
            self.device.raw_device().cmd_pipeline_barrier(
                self.command_buffer,
                src_stage,
                dst_stage,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }

        Ok(())
    }

    /// Clear color image
    pub fn clear_color_image(
        &mut self,
        image: vk::Image,
        layout: vk::ImageLayout,
        color: vk::ClearColorValue,
    ) -> KgpuResult<()> {
        if !self.recording {
            return Err(KgpuError::InvalidState("Command buffer not recording".to_string()));
        }

        let range = vk::ImageSubresourceRange::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .base_mip_level(0)
            .level_count(1)
            .base_array_layer(0)
            .layer_count(1);

        unsafe {
            self.device.raw_device().cmd_clear_color_image(
                self.command_buffer,
                image,
                layout,
                &color,
                &[range],
            );
        }

        Ok(())
    }

    /// Get raw command buffer handle
    pub(crate) fn raw(&self) -> vk::CommandBuffer {
        self.command_buffer
    }
}

impl HalCommandBuffer for VulkanCommandBuffer {
    fn backend(&self) -> Backend {
        Backend::Vulkan
    }

    fn begin_recording(&mut self) -> KgpuResult<()> {
        self.begin()
    }

    fn end_recording(&mut self) -> KgpuResult<()> {
        self.end()
    }

    fn reset_buffer(&mut self) -> KgpuResult<()> {
        self.reset()
    }
}

impl Drop for VulkanCommandBuffer {
    fn drop(&mut self) {
        // Free command buffer
        self.device.free_command_buffers(self.command_pool, &[self.command_buffer]);

        // Destroy command pool
        self.device.destroy_command_pool(self.command_pool);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::VulkanInstance;

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_command_buffer_creation() {
        let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();
        let cmd = VulkanCommandBuffer::new(device);

        assert!(cmd.is_ok(), "Failed to create command buffer");
    }

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_command_buffer_recording() {
        let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();
        let mut cmd = VulkanCommandBuffer::new(device).unwrap();

        assert!(cmd.begin().is_ok(), "Failed to begin recording");
        assert!(cmd.end().is_ok(), "Failed to end recording");
    }

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_command_buffer_reset() {
        let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();
        let mut cmd = VulkanCommandBuffer::new(device).unwrap();

        cmd.begin().unwrap();
        cmd.end().unwrap();
        assert!(cmd.reset().is_ok(), "Failed to reset command buffer");
    }
}
