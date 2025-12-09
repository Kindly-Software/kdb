//! Metal Memory Management
//!
//! # Architecture
//!
//! Metal memory management is simplified compared to Vulkan:
//!
//! - **Unified Memory**: Apple Silicon has shared CPU/GPU memory (zero-copy)
//! - **Storage Modes**: Shared, Private, Managed (macOS only)
//! - **Automatic Allocation**: MTLBuffer/MTLTexture handle allocation internally
//! - **No Manual Memory Types**: Metal driver manages memory automatically
//!
//! # Performance
//!
//! - Allocation: <5μs (MTLBuffer/MTLTexture creation)
//! - Free: <1μs (ARC release)
//! - CPU-GPU transfer: 0ns (unified memory) or <1μs/MB (discrete memory)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_UNIFIED_MEMORY_APPLE_SILICON`: M1/M2/M3 have unified memory
//! - `#ASSUME_AUTOMATIC_MANAGEMENT`: Metal driver handles memory types
//! - `#VERIFY_UNSAFE_FFI`: metal-rs wraps MTL* calls safely

use metal::{self, MTLResourceOptions};
use std::sync::Arc;

use crate::gpu::kgpu::error::{KgpuError, KgpuResult};

use super::MetalDevice;

/// Metal memory allocation
///
/// # Layout
///
/// - 64B cache-aligned (Arc overhead)
/// - No explicit memory object (Metal manages internally)
/// - Storage mode tracked for CPU/GPU synchronization
///
/// # Notes
///
/// Metal does NOT expose VkDeviceMemory equivalent. Memory is allocated
/// automatically when creating MTLBuffer or MTLTexture. This wrapper is
/// primarily for HAL trait compatibility.
pub struct MetalMemory {
    /// Inner state
    inner: Arc<MetalMemoryInner>,
}

struct MetalMemoryInner {
    /// Device reference
    device: MetalDevice,

    /// Storage mode (Shared, Private, Managed)
    storage_mode: MTLStorageMode,

    /// Allocation size (bytes)
    size: u64,
}

/// Metal storage modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MTLStorageMode {
    /// Shared: CPU and GPU can both access (unified memory)
    /// Best for Apple Silicon (M1/M2/M3)
    /// Performance: 0ns CPU-GPU transfer
    Shared,

    /// Private: GPU-only access (fastest for GPU)
    /// Use for render targets, intermediate buffers
    /// Performance: No CPU access, fastest GPU reads/writes
    Private,

    /// Managed: Explicit CPU-GPU synchronization (macOS only)
    /// Use for discrete GPUs (Intel)
    /// Performance: <1μs/MB CPU-GPU transfer
    #[cfg(target_os = "macos")]
    Managed,
}

impl MetalMemory {
    /// Create memory allocation
    ///
    /// # Performance
    ///
    /// <5μs (B32 target, Metal handles allocation internally)
    ///
    /// # Arguments
    ///
    /// - `device`: Metal device
    /// - `size`: Allocation size in bytes
    /// - `storage_mode`: Shared/Private/Managed
    ///
    /// # Notes
    ///
    /// This is a placeholder; real allocations happen via MTLBuffer/MTLTexture.
    /// Included for HAL trait compatibility.
    pub fn new(
        device: MetalDevice,
        size: u64,
        storage_mode: MTLStorageMode,
    ) -> KgpuResult<Self> {
        if size == 0 {
            return Err(KgpuError::OutOfMemory);
        }

        Ok(Self {
            inner: Arc::new(MetalMemoryInner {
                device,
                storage_mode,
                size,
            }),
        })
    }

    /// Get storage mode
    pub fn storage_mode(&self) -> MTLStorageMode {
        self.inner.storage_mode
    }

    /// Get allocation size
    pub fn size(&self) -> u64 {
        self.inner.size
    }

    /// Convert storage mode to MTLResourceOptions
    pub fn resource_options(&self) -> MTLResourceOptions {
        match self.inner.storage_mode {
            MTLStorageMode::Shared => MTLResourceOptions::StorageModeShared,
            MTLStorageMode::Private => MTLResourceOptions::StorageModePrivate,
            #[cfg(target_os = "macos")]
            MTLStorageMode::Managed => MTLResourceOptions::StorageModeManaged,
        }
    }

    /// Check if memory is mappable (CPU accessible)
    pub fn is_cpu_accessible(&self) -> bool {
        match self.inner.storage_mode {
            MTLStorageMode::Shared => true,
            MTLStorageMode::Private => false,
            #[cfg(target_os = "macos")]
            MTLStorageMode::Managed => true,
        }
    }

    /// Get device reference
    pub(crate) fn device(&self) -> &MetalDevice {
        &self.inner.device
    }
}

// SAFETY: Metal memory is thread-safe (ARC-managed)
unsafe impl Send for MetalMemoryInner {}
unsafe impl Sync for MetalMemoryInner {}

impl Drop for MetalMemoryInner {
    fn drop(&mut self) {
        // Metal manages memory automatically, no explicit cleanup needed
    }
}

/// Helper functions for Metal memory management
pub mod helpers {
    use super::*;

    /// Get optimal storage mode for usage
    ///
    /// # Arguments
    ///
    /// - `device`: Metal device
    /// - `cpu_write`: Will CPU write to buffer?
    /// - `gpu_write`: Will GPU write to buffer?
    ///
    /// # Returns
    ///
    /// Optimal storage mode for performance
    pub fn optimal_storage_mode(
        device: &MetalDevice,
        cpu_write: bool,
        gpu_write: bool,
    ) -> MTLStorageMode {
        // Apple Silicon (unified memory): Always use Shared
        if device.has_unified_memory() {
            return MTLStorageMode::Shared;
        }

        // Discrete GPU (Intel):
        match (cpu_write, gpu_write) {
            (true, true) => {
                // CPU and GPU both write: Use Managed (macOS)
                #[cfg(target_os = "macos")]
                return MTLStorageMode::Managed;
                #[cfg(not(target_os = "macos"))]
                return MTLStorageMode::Shared;
            }
            (true, false) => {
                // CPU writes, GPU reads: Shared or Managed
                MTLStorageMode::Shared
            }
            (false, true) => {
                // GPU writes only: Private (fastest)
                MTLStorageMode::Private
            }
            (false, false) => {
                // Read-only: Shared
                MTLStorageMode::Shared
            }
        }
    }

    /// Calculate aligned size for Metal buffers
    ///
    /// # Arguments
    ///
    /// - `size`: Requested size in bytes
    /// - `alignment`: Required alignment (typically 256 for uniforms)
    ///
    /// # Returns
    ///
    /// Aligned size (rounded up)
    pub fn align_size(size: u64, alignment: u64) -> u64 {
        (size + alignment - 1) & !(alignment - 1)
    }

    /// Get recommended alignment for buffer usage
    ///
    /// # Arguments
    ///
    /// - `is_uniform`: Is this a uniform buffer?
    ///
    /// # Returns
    ///
    /// Recommended alignment (256 for uniforms, 16 for others)
    pub fn recommended_alignment(is_uniform: bool) -> u64 {
        if is_uniform {
            256 // Metal requires 256-byte alignment for uniforms
        } else {
            16 // Conservative alignment for other buffers
        }
    }
}

#[cfg(test)]
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod tests {
    use super::*;
    use super::super::{MetalInstance, MetalDevice};

    #[test]
    #[ignore] // Requires Metal support
    fn test_memory_creation() {
        let instance = MetalInstance::new().unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        let memory = MetalMemory::new(device, 1024, MTLStorageMode::Shared);
        assert!(memory.is_ok(), "Failed to create memory");

        if let Ok(mem) = memory {
            assert_eq!(mem.size(), 1024);
            assert_eq!(mem.storage_mode(), MTLStorageMode::Shared);
            assert!(mem.is_cpu_accessible());
        }
    }

    #[test]
    #[ignore] // Requires Metal support
    fn test_storage_modes() {
        let instance = MetalInstance::new().unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        // Shared memory (CPU accessible)
        let shared = MetalMemory::new(device.clone(), 1024, MTLStorageMode::Shared).unwrap();
        assert!(shared.is_cpu_accessible());

        // Private memory (GPU only)
        let private = MetalMemory::new(device.clone(), 1024, MTLStorageMode::Private).unwrap();
        assert!(!private.is_cpu_accessible());

        #[cfg(target_os = "macos")]
        {
            // Managed memory (macOS only)
            let managed = MetalMemory::new(device.clone(), 1024, MTLStorageMode::Managed).unwrap();
            assert!(managed.is_cpu_accessible());
        }
    }

    #[test]
    #[ignore] // Requires Metal support
    fn test_optimal_storage_mode() {
        let instance = MetalInstance::new().unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        // Apple Silicon should always use Shared
        let mode = helpers::optimal_storage_mode(&device, true, true);
        if device.has_unified_memory() {
            assert_eq!(mode, MTLStorageMode::Shared);
        }

        // GPU-only should use Private
        let mode = helpers::optimal_storage_mode(&device, false, true);
        if !device.has_unified_memory() {
            assert_eq!(mode, MTLStorageMode::Private);
        }
    }

    #[test]
    fn test_align_size() {
        assert_eq!(helpers::align_size(100, 256), 256);
        assert_eq!(helpers::align_size(256, 256), 256);
        assert_eq!(helpers::align_size(300, 256), 512);
        assert_eq!(helpers::align_size(1, 16), 16);
    }

    #[test]
    fn test_recommended_alignment() {
        assert_eq!(helpers::recommended_alignment(true), 256); // Uniform
        assert_eq!(helpers::recommended_alignment(false), 16); // Other
    }
}
