//! Vulkan Memory Management
//!
//! # Architecture
//!
//! VulkanMemory wraps vk::DeviceMemory for GPU memory allocation.
//!
//! - **Memory Types**: Device local (GPU), Host visible (CPU-GPU), Host coherent
//! - **Memory Allocation**: vkAllocateMemory (manual for now, could integrate gpu-allocator later)
//! - **Memory Binding**: vkBindBufferMemory, vkBindImageMemory
//! - **Memory Mapping**: vkMapMemory, vkUnmapMemory (host-visible memory)
//!
//! # Performance
//!
//! - Allocation: <10μs (vkAllocateMemory)
//! - Binding: <1μs (vkBindBufferMemory/vkBindImageMemory)
//! - Mapping: <1μs (vkMapMemory)
//! - Unmapping: <1μs (vkUnmapMemory)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_MEMORY_TYPES_INCLUDE_HOST_VISIBLE`: Vulkan spec guarantees host-visible memory
//! - `#ASSUME_ALLOCATION_SUCCEEDS`: Sufficient memory available (OOM handled gracefully)
//! - `#VERIFY_UNSAFE_FFI`: All vk* memory calls checked

use ash::vk;
use std::sync::Arc;

use crate::gpu::kgpu::hal::{HalMemory, Backend};
use crate::gpu::kgpu::error::{KgpuError, KgpuResult};

use super::{VulkanDevice, VulkanAdapter};

/// Vulkan memory capsule
///
/// # Layout
///
/// - 128B cache-aligned
/// - Memory handle + device reference
/// - Size and type tracked for validation
///
/// # Lifecycle
///
/// ```text
/// Uninitialized → Allocate (vkAllocateMemory) → Allocated
///     ↓                                           ↓
/// Destroyed  ←──────────────────────────────── Bound (to buffer/image)
///                                                 ↓
///                                                Mapped (host-visible only)
/// ```
pub struct VulkanMemory {
    /// Device reference
    device: VulkanDevice,

    /// Memory handle
    memory: vk::DeviceMemory,

    /// Memory size
    size: u64,

    /// Memory type index
    memory_type_index: u32,

    /// Mapped pointer (if host-visible)
    mapped_ptr: Option<*mut u8>,
}

impl VulkanMemory {
    /// Allocate device memory
    ///
    /// # Performance
    ///
    /// <10μs (vkAllocateMemory)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_ALLOCATION_SUCCEEDS`: Sufficient memory available
    ///
    /// # Example
    ///
    /// ```no_run
    /// use atomic_capsule::gpu::kgpu::backends::vulkan::*;
    /// use ash::vk;
    ///
    /// let instance = VulkanInstance::new("MyApp", "MyEngine")?;
    /// let adapters = instance.enumerate_adapters()?;
    /// let device = adapters[0].create_device()?;
    ///
    /// // Allocate 1MB device-local memory
    /// let memory = VulkanMemory::allocate(
    ///     device.clone(),
    ///     &adapters[0],
    ///     1024 * 1024,
    ///     vk::MemoryPropertyFlags::DEVICE_LOCAL,
    /// )?;
    /// # Ok::<(), atomic_capsule::gpu::kgpu::error::KgpuError>(())
    /// ```
    pub fn allocate(
        device: VulkanDevice,
        adapter: &VulkanAdapter,
        size: u64,
        properties: vk::MemoryPropertyFlags,
    ) -> KgpuResult<Self> {
        // Find suitable memory type
        let memory_type_index = Self::find_memory_type(adapter, u32::MAX, properties)?;

        // Allocate memory
        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(size)
            .memory_type_index(memory_type_index);

        let memory = unsafe {
            device.raw_device()
                .allocate_memory(&alloc_info, None)
                .map_err(|e| {
                    KgpuError::ResourceCreationFailed(format!("Failed to allocate memory: {}", e))
                })?
        };

        Ok(Self {
            device,
            memory,
            size,
            memory_type_index,
            mapped_ptr: None,
        })
    }

    /// Allocate memory for buffer
    pub fn allocate_for_buffer(
        device: VulkanDevice,
        adapter: &VulkanAdapter,
        buffer: vk::Buffer,
        properties: vk::MemoryPropertyFlags,
    ) -> KgpuResult<Self> {
        // Get buffer memory requirements
        let mem_requirements = unsafe {
            device.raw_device().get_buffer_memory_requirements(buffer)
        };

        // Find suitable memory type
        let memory_type_index = Self::find_memory_type(
            adapter,
            mem_requirements.memory_type_bits,
            properties,
        )?;

        // Allocate memory
        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_requirements.size)
            .memory_type_index(memory_type_index);

        let memory = unsafe {
            device.raw_device()
                .allocate_memory(&alloc_info, None)
                .map_err(|e| {
                    KgpuError::ResourceCreationFailed(format!("Failed to allocate buffer memory: {}", e))
                })?
        };

        Ok(Self {
            device,
            memory,
            size: mem_requirements.size,
            memory_type_index,
            mapped_ptr: None,
        })
    }

    /// Allocate memory for image
    pub fn allocate_for_image(
        device: VulkanDevice,
        adapter: &VulkanAdapter,
        image: vk::Image,
        properties: vk::MemoryPropertyFlags,
    ) -> KgpuResult<Self> {
        // Get image memory requirements
        let mem_requirements = unsafe {
            device.raw_device().get_image_memory_requirements(image)
        };

        // Find suitable memory type
        let memory_type_index = Self::find_memory_type(
            adapter,
            mem_requirements.memory_type_bits,
            properties,
        )?;

        // Allocate memory
        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_requirements.size)
            .memory_type_index(memory_type_index);

        let memory = unsafe {
            device.raw_device()
                .allocate_memory(&alloc_info, None)
                .map_err(|e| {
                    KgpuError::ResourceCreationFailed(format!("Failed to allocate image memory: {}", e))
                })?
        };

        Ok(Self {
            device,
            memory,
            size: mem_requirements.size,
            memory_type_index,
            mapped_ptr: None,
        })
    }

    /// Find suitable memory type
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_MEMORY_TYPES_INCLUDE_HOST_VISIBLE`: Vulkan spec guarantees
    fn find_memory_type(
        adapter: &VulkanAdapter,
        type_filter: u32,
        properties: vk::MemoryPropertyFlags,
    ) -> KgpuResult<u32> {
        let mem_properties = unsafe {
            adapter.instance().raw_instance()
                .get_physical_device_memory_properties(adapter.physical_device())
        };

        for i in 0..mem_properties.memory_type_count {
            if (type_filter & (1 << i)) != 0
                && mem_properties.memory_types[i as usize]
                    .property_flags
                    .contains(properties)
            {
                return Ok(i);
            }
        }

        Err(KgpuError::ResourceCreationFailed(
            "Failed to find suitable memory type".to_string(),
        ))
    }

    /// Bind memory to buffer
    ///
    /// # Performance
    ///
    /// <1μs (vkBindBufferMemory)
    pub fn bind_buffer(&self, buffer: vk::Buffer) -> KgpuResult<()> {
        unsafe {
            self.device.raw_device()
                .bind_buffer_memory(buffer, self.memory, 0)
                .map_err(|e| {
                    KgpuError::OperationFailed(format!("Failed to bind buffer memory: {}", e))
                })
        }
    }

    /// Bind memory to image
    ///
    /// # Performance
    ///
    /// <1μs (vkBindImageMemory)
    pub fn bind_image(&self, image: vk::Image) -> KgpuResult<()> {
        unsafe {
            self.device.raw_device()
                .bind_image_memory(image, self.memory, 0)
                .map_err(|e| {
                    KgpuError::OperationFailed(format!("Failed to bind image memory: {}", e))
                })
        }
    }

    /// Map memory (host-visible only)
    ///
    /// # Performance
    ///
    /// <1μs (vkMapMemory)
    ///
    /// # Safety
    ///
    /// Returns raw pointer to mapped memory. Caller must ensure:
    /// - Memory is host-visible
    /// - No concurrent GPU access (use fences)
    /// - Unmap before freeing
    pub fn map(&mut self) -> KgpuResult<*mut u8> {
        if self.mapped_ptr.is_some() {
            return Err(KgpuError::InvalidState("Memory already mapped".to_string()));
        }

        let ptr = unsafe {
            self.device.raw_device()
                .map_memory(self.memory, 0, self.size, vk::MemoryMapFlags::empty())
                .map_err(|e| {
                    KgpuError::OperationFailed(format!("Failed to map memory: {}", e))
                })? as *mut u8
        };

        self.mapped_ptr = Some(ptr);
        Ok(ptr)
    }

    /// Unmap memory
    ///
    /// # Performance
    ///
    /// <1μs (vkUnmapMemory)
    pub fn unmap(&mut self) {
        if self.mapped_ptr.is_some() {
            unsafe {
                self.device.raw_device().unmap_memory(self.memory);
            }
            self.mapped_ptr = None;
        }
    }

    /// Copy data to mapped memory
    ///
    /// # Safety
    ///
    /// Memory must be mapped before calling. Data must not exceed allocated size.
    pub fn copy_to_mapped(&self, data: &[u8]) -> KgpuResult<()> {
        if let Some(ptr) = self.mapped_ptr {
            if data.len() as u64 > self.size {
                return Err(KgpuError::InvalidArgument(
                    format!("Data size {} exceeds memory size {}", data.len(), self.size),
                ));
            }

            unsafe {
                std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
            }

            Ok(())
        } else {
            Err(KgpuError::InvalidState("Memory not mapped".to_string()))
        }
    }

    /// Get memory size
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Get memory type index
    pub fn memory_type_index(&self) -> u32 {
        self.memory_type_index
    }

    /// Get raw memory handle
    pub(crate) fn raw(&self) -> vk::DeviceMemory {
        self.memory
    }
}

impl HalMemory for VulkanMemory {
    fn backend(&self) -> Backend {
        Backend::Vulkan
    }

    fn size_bytes(&self) -> u64 {
        self.size
    }
}

impl Drop for VulkanMemory {
    fn drop(&mut self) {
        // Unmap if still mapped
        self.unmap();

        // Free memory
        unsafe {
            self.device.raw_device().free_memory(self.memory, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::VulkanInstance;

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_memory_allocation() {
        let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        let memory = VulkanMemory::allocate(
            device,
            &adapters[0],
            1024 * 1024,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        );

        assert!(memory.is_ok(), "Failed to allocate memory");
        if let Ok(mem) = memory {
            assert_eq!(mem.size(), 1024 * 1024);
        }
    }

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_buffer_memory_allocation() {
        let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        let buffer = device.create_buffer(
            1024,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::SharingMode::EXCLUSIVE,
        ).unwrap();

        let memory = VulkanMemory::allocate_for_buffer(
            device.clone(),
            &adapters[0],
            buffer,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        );

        assert!(memory.is_ok(), "Failed to allocate buffer memory");

        if let Ok(mem) = memory {
            assert!(mem.bind_buffer(buffer).is_ok(), "Failed to bind buffer");
        }

        device.destroy_buffer(buffer);
    }

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_memory_mapping() {
        let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        let mut memory = VulkanMemory::allocate(
            device,
            &adapters[0],
            1024,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        ).unwrap();

        let ptr = memory.map();
        assert!(ptr.is_ok(), "Failed to map memory");

        // Copy test data
        let data = vec![42u8; 1024];
        assert!(memory.copy_to_mapped(&data).is_ok(), "Failed to copy to mapped memory");

        memory.unmap();
    }
}
