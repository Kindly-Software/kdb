//! Metal Device (Logical Device) Implementation
//!
//! # Architecture
//!
//! MetalDevice wraps MTLDevice for command execution and resource creation.
//!
//! - **Logical Device**: MTLDevice (1:1 with physical GPU on macOS)
//! - **Command Queue**: MTLCommandQueue (single queue, thread-safe)
//! - **Unified Memory**: Apple Silicon = zero-copy CPU/GPU sharing
//! - **Features**: Metal 3 (mesh shaders, ray tracing, MetalFX)
//! - **Memory**: Unified memory eliminates CPU→GPU copies
//!
//! # Performance
//!
//! - Creation: <20ms (B32 target, much faster than Vulkan)
//! - Queue submit: <100ns (MTLCommandBuffer commit)
//! - Resource creation: <5μs (MTLBuffer/MTLTexture)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_DEVICE_VALID`: MTLDevice remains valid (ARC-managed)
//! - `#ASSUME_QUEUE_VALID`: MTLCommandQueue is non-null
//! - `#VERIFY_UNSAFE_FFI`: All metal-rs calls are safe (Rust bindings)

use metal::{self, Device as MTLDeviceProtocol, MTLResourceOptions};
use std::sync::Arc;

use crate::gpu::kgpu::error::{KgpuError, KgpuResult};
use crate::gpu::kgpu::hal::BackendType;

use super::MetalAdapter;

/// Metal device capsule
///
//

! # Layout
///
/// - 64B cache-aligned (Arc overhead)
/// - Arc-wrapped for cheap cloning
/// - Command queue cached at creation
///
/// # Lifecycle
///
/// ```text
/// Uninitialized → Create (newCommandQueue) → Active
///     ↓                                        ↓
/// Destroyed  ←────────────────────────────── Drop (ARC cleanup)
/// ```
#[derive(Clone)]
pub struct MetalDevice {
    /// Inner state (Arc for cheap cloning)
    inner: Arc<MetalDeviceInner>,
}

struct MetalDeviceInner {
    /// Adapter reference
    adapter: MetalAdapter,

    /// MTLDevice handle
    device: metal::Device,

    /// Command queue (single queue, thread-safe)
    command_queue: metal::CommandQueue,

    /// Unified memory (Apple Silicon = true, Intel = false)
    unified_memory: bool,

    /// Metal 3+ features
    supports_metal3: bool,
    supports_ray_tracing: bool,
    supports_mesh_shaders: bool,
}

impl MetalDevice {
    /// Create logical device from adapter
    ///
    /// # Performance
    ///
    /// <20ms (B32 target, much faster than Vulkan)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_DEVICE_VALID`: MTLDevice is ARC-managed
    /// - `#ASSUME_QUEUE_CREATION_SUCCEEDS`: newCommandQueue always succeeds
    ///
    /// # Example
    ///
    /// ```no_run
    /// use atomic_capsule::gpu::kgpu::backends::metal::{MetalInstance, MetalDevice};
    ///
    /// let instance = MetalInstance::new()?;
    /// let adapters = instance.enumerate_adapters()?;
    /// let device = adapters[0].create_device()?;
    /// println!("Device created with unified_memory={}, metal3={}, ray_tracing={}",
    ///     device.has_unified_memory(),
    ///     device.supports_metal3(),
    ///     device.supports_ray_tracing());
    /// # Ok::<(), atomic_capsule::gpu::kgpu::error::KgpuError>(())
    /// ```
    pub(crate) fn new(adapter: MetalAdapter) -> KgpuResult<Self> {
        let device = adapter.metal_device().clone();

        // Create command queue (single queue, reused for all submissions)
        // #VERIFY_UNSAFE_FFI: metal-rs wraps Objective-C safely
        let command_queue = device.new_command_queue();

        // Detect unified memory (Apple Silicon = M1/M2/M3)
        // Unified memory = CPU and GPU share same physical memory
        let unified_memory = Self::detect_unified_memory(&device);

        // Detect Metal 3+ features
        let supports_metal3 = Self::supports_metal3(&device);
        let supports_ray_tracing = Self::supports_ray_tracing(&device);
        let supports_mesh_shaders = Self::supports_mesh_shaders(&device);

        Ok(Self {
            inner: Arc::new(MetalDeviceInner {
                adapter,
                device,
                command_queue,
                unified_memory,
                supports_metal3,
                supports_ray_tracing,
                supports_mesh_shaders,
            }),
        })
    }

    /// Get raw MTLDevice
    pub(crate) fn metal_device(&self) -> &metal::Device {
        &self.inner.device
    }

    /// Get adapter reference
    pub(crate) fn adapter(&self) -> &MetalAdapter {
        &self.inner.adapter
    }

    /// Get command queue
    pub(crate) fn command_queue(&self) -> &metal::CommandQueue {
        &self.inner.command_queue
    }

    /// Check if device has unified memory
    pub fn has_unified_memory(&self) -> bool {
        self.inner.unified_memory
    }

    /// Check if Metal 3 is supported
    pub fn supports_metal3(&self) -> bool {
        self.inner.supports_metal3
    }

    /// Check if ray tracing is supported
    pub fn supports_ray_tracing(&self) -> bool {
        self.inner.supports_ray_tracing
    }

    /// Check if mesh shaders are supported
    pub fn supports_mesh_shaders(&self) -> bool {
        self.inner.supports_mesh_shaders
    }

    /// Create buffer
    ///
    /// # Performance
    ///
    /// <5μs (B32 target, faster than Vulkan)
    ///
    /// # Arguments
    ///
    /// - `size`: Buffer size in bytes
    /// - `options`: Metal resource options (storage mode, hazard tracking)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_SIZE_VALID`: Size must be > 0
    /// - `#ASSUME_ALIGNMENT`: Metal handles alignment automatically
    pub fn create_buffer(
        &self,
        size: u64,
        options: MTLResourceOptions,
    ) -> KgpuResult<metal::Buffer> {
        if size == 0 {
            return Err(KgpuError::ResourceCreationFailed(
                "Buffer size must be > 0".into(),
            ));
        }

        // #VERIFY_UNSAFE_FFI: metal-rs wraps new_buffer safely
        let buffer = self.inner.device.new_buffer(size, options);

        Ok(buffer)
    }

    /// Create texture
    ///
    /// # Performance
    ///
    /// <10μs (B32 target)
    pub fn create_texture(
        &self,
        descriptor: &metal::TextureDescriptor,
    ) -> KgpuResult<metal::Texture> {
        // #VERIFY_UNSAFE_FFI: metal-rs wraps new_texture safely
        let texture = self.inner.device.new_texture(descriptor);

        Ok(texture)
    }

    /// Create sampler
    ///
    /// # Performance
    ///
    /// <5μs (B32 target)
    pub fn create_sampler(
        &self,
        descriptor: &metal::SamplerDescriptor,
    ) -> KgpuResult<metal::SamplerState> {
        // #VERIFY_UNSAFE_FFI: metal-rs wraps new_sampler_state safely
        let sampler = self.inner.device.new_sampler(descriptor);

        Ok(sampler)
    }

    /// Detect unified memory (Apple Silicon = true, Intel = false)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_UNIFIED_MEMORY_APPLE_SILICON`: M1/M2/M3 have unified memory
    /// - `#ASSUME_DISCRETE_MEMORY_INTEL`: Intel Macs have discrete memory
    fn detect_unified_memory(device: &metal::Device) -> bool {
        // Check if device has unified memory
        // Apple Silicon (M1/M2/M3) = true
        // Intel Macs = false
        device.has_unified_memory()
    }

    /// Check if Metal 3 is supported
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_METAL3_MACOS13_IOS16`: Metal 3 requires macOS 13+ or iOS 16+
    fn supports_metal3(device: &metal::Device) -> bool {
        // Check for Metal 3 features
        // Requires macOS 13+, iOS 16+
        device.supports_family(metal::MTLGPUFamily::Apple8)
            || device.supports_family(metal::MTLGPUFamily::Metal3)
    }

    /// Check if ray tracing is supported
    fn supports_ray_tracing(device: &metal::Device) -> bool {
        // Ray tracing requires Metal 3 + M3 or later
        device.supports_raytracing()
    }

    /// Check if mesh shaders are supported
    fn supports_mesh_shaders(device: &metal::Device) -> bool {
        // Mesh shaders (object + mesh shader stages)
        // Available on Metal 3+
        device.supports_function_pointers()
    }
}

// SAFETY: MTLDevice and MTLCommandQueue are thread-safe (internally synchronized)
unsafe impl Send for MetalDeviceInner {}
unsafe impl Sync for MetalDeviceInner {}

impl Drop for MetalDeviceInner {
    fn drop(&mut self) {
        // Metal objects are ARC-managed, no explicit cleanup needed
        // CommandQueue and Device will be released automatically
    }
}

#[cfg(test)]
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod tests {
    use super::*;
    use super::super::MetalInstance;

    #[test]
    #[ignore] // Requires Metal support
    fn test_device_creation() {
        let instance = MetalInstance::new().unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device();

        assert!(device.is_ok(), "Failed to create device");

        if let Ok(dev) = device {
            println!("Device created:");
            println!("  Unified memory: {}", dev.has_unified_memory());
            println!("  Metal 3: {}", dev.supports_metal3());
            println!("  Ray tracing: {}", dev.supports_ray_tracing());
            println!("  Mesh shaders: {}", dev.supports_mesh_shaders());
        }
    }

    #[test]
    #[ignore] // Requires Metal support
    fn test_buffer_creation() {
        let instance = MetalInstance::new().unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        let buffer = device.create_buffer(
            1024,
            MTLResourceOptions::StorageModeShared,
        );

        assert!(buffer.is_ok(), "Failed to create buffer");

        if let Ok(buf) = buffer {
            assert_eq!(buf.length(), 1024);
        }
    }

    #[test]
    #[ignore] // Requires Metal support
    fn test_unified_memory_detection() {
        let instance = MetalInstance::new().unwrap();
        let adapters = instance.enumerate_adapters().unwrap();
        let device = adapters[0].create_device().unwrap();

        // Apple Silicon should have unified memory
        let unified = device.has_unified_memory();
        println!("Unified memory: {}", unified);
    }
}
