//! KGPU HAL: Hardware Abstraction Layer Traits
//!
//! This module defines backend-agnostic GPU operation traits that Vulkan, Metal,
//! and DX12 backends implement. The HAL provides:
//!
//! - **Type-safe abstractions**: Associated types ensure type consistency
//! - **Backend independence**: Same code works across all GPU backends
//! - **Zero-cost abstractions**: Trait methods are inlined by the compiler
//! - **Object-safe core**: Most traits can be used as trait objects
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                          KgpuBackend (Root)                             │
//! │  Defines all associated types for a complete backend implementation     │
//! └─────────────────────────────────────────────────────────────────────────┘
//!                                     │
//!              ┌──────────────────────┼──────────────────────┐
//!              ▼                      ▼                      ▼
//!    ┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐
//!    │ KgpuInstanceApi │   │  KgpuAdapterApi │   │   KgpuDeviceApi │
//!    │  (Entry point)  │   │ (Physical GPU)  │   │ (Logical device)│
//!    └─────────────────┘   └─────────────────┘   └─────────────────┘
//!              │                                         │
//!              ▼                                         ▼
//!    ┌─────────────────────────────────────────────────────────────────────┐
//!    │                         Resource APIs                                │
//!    │  KgpuBufferApi │ KgpuTextureApi │ KgpuSamplerApi │ KgpuBindGroupApi  │
//!    └─────────────────────────────────────────────────────────────────────┘
//!                                     │
//!                                     ▼
//!    ┌─────────────────────────────────────────────────────────────────────┐
//!    │                        Command Recording                             │
//!    │    KgpuCommandEncoderApi │ KgpuRenderPassApi │ KgpuComputePassApi   │
//!    └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Chaos Compliance
//!
//! All HAL traits are designed for lockfree implementations:
//!
//! - No mutex or RwLock in trait requirements
//! - All types are Send + Sync for multi-threaded use
//! - Associated types allow backend-specific atomic implementations
//!
//! # ASSUM Safety Tags
//!
//! - `#ASSUME_BACKEND_SEND_SYNC`: All backend implementations are thread-safe
//! - `#ASSUME_TRAIT_OBJECT_SAFE`: Core traits support dynamic dispatch
//! - `#ASSUME_LIFETIME_CORRECT`: Borrowed data outlives usage

pub mod error;
pub mod types;

pub use error::*;
pub use types::*;

use core::future::Future;
use core::ops::Range;

// ============================================================================
// KgpuBackend - Root Backend Trait
// ============================================================================

/// Root trait defining a complete GPU backend.
///
/// `KgpuBackend` is the entry point for any GPU backend implementation.
/// It defines all the associated types needed for a complete backend.
///
/// # Associated Types
///
/// Each associated type represents a backend-specific implementation of
/// a HAL concept. For example, a Vulkan backend would define:
///
/// - `Instance` → VulkanInstance
/// - `Adapter` → VulkanPhysicalDevice
/// - `Device` → VulkanDevice
/// - etc.
///
/// # ASSUM Safety
///
/// - `#ASSUME_BACKEND_STATIC`: Backend types are `'static` for safe storage
/// - `#ASSUME_BACKEND_SEND_SYNC`: All associated types are thread-safe
///
/// # Example
///
/// ```ignore
/// struct VulkanBackend;
///
/// impl KgpuBackend for VulkanBackend {
///     type Instance = VulkanInstance;
///     type Adapter = VulkanPhysicalDevice;
///     // ... other types
///
///     fn name() -> &'static str { "Vulkan" }
///     fn api_version() -> (u32, u32, u32) { (1, 3, 0) }
///     fn is_available() -> bool { /* check Vulkan support */ }
/// }
/// ```
pub trait KgpuBackend: Send + Sync + 'static {
    /// Instance type (entry point to the backend).
    type Instance: KgpuInstanceApi;

    /// Adapter type (physical GPU).
    type Adapter: KgpuAdapterApi;

    /// Device type (logical device).
    type Device: KgpuDeviceApi;

    /// Queue type (command submission).
    type Queue: KgpuQueueApi;

    /// Buffer type (GPU memory).
    type Buffer: KgpuBufferApi;

    /// Texture type (image data).
    type Texture: KgpuTextureApi;

    /// Texture view type.
    type TextureView: KgpuTextureViewApi;

    /// Sampler type (texture sampling).
    type Sampler: KgpuSamplerApi;

    /// Bind group type (resource binding).
    type BindGroup: KgpuBindGroupApi;

    /// Bind group layout type.
    type BindGroupLayout: Send + Sync;

    /// Pipeline layout type.
    type PipelineLayout: Send + Sync;

    /// Render pipeline type.
    type RenderPipeline: KgpuRenderPipelineApi;

    /// Compute pipeline type.
    type ComputePipeline: KgpuComputePipelineApi;

    /// Shader module type.
    type ShaderModule: Send + Sync;

    /// Command encoder type.
    type CommandEncoder: KgpuCommandEncoderApi;

    /// Command buffer type (finished commands).
    type CommandBuffer: Send + Sync;

    /// Returns the backend name.
    fn name() -> &'static str;

    /// Returns the API version as (major, minor, patch).
    fn api_version() -> (u32, u32, u32);

    /// Returns true if this backend is available on the current platform.
    fn is_available() -> bool;
}

// ============================================================================
// KgpuInstanceApi - Instance Creation
// ============================================================================

/// Instance API for backend initialization and adapter enumeration.
///
/// The instance is the entry point to the GPU backend. It handles:
///
/// - Adapter (physical GPU) enumeration
/// - Surface format queries
/// - Backend-level configuration
///
/// # ASSUM Safety
///
/// - `#ASSUME_INSTANCE_THREAD_SAFE`: Instance can be used from any thread
/// - `#ASSUME_ADAPTER_LIFETIME`: Adapters are valid for instance lifetime
pub trait KgpuInstanceApi: Send + Sync + Sized {
    /// Associated adapter type.
    type Adapter: KgpuAdapterApi;

    /// Creates a new instance.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the backend is not available or initialization fails.
    fn new() -> HalResult<Self>;

    /// Enumerates all available adapters.
    ///
    /// Returns information about each physical GPU without creating handles.
    fn enumerate_adapters(&self) -> HalResult<AdapterList>;

    /// Requests a specific adapter based on options.
    ///
    /// # Arguments
    ///
    /// * `options` - Selection criteria (power preference, etc.)
    ///
    /// # Errors
    ///
    /// Returns `Err` if no suitable adapter is found.
    fn request_adapter(&self, options: &AdapterOptions) -> HalResult<Self::Adapter>;

    /// Returns supported surface formats.
    fn surface_formats(&self) -> &[HalTextureFormat];
}

/// List of adapter information.
#[derive(Debug, Clone)]
pub struct AdapterList {
    /// Adapter information entries.
    pub adapters: [Option<AdapterInfo>; 8],
    /// Number of valid entries.
    pub count: usize,
}

impl Default for AdapterList {
    fn default() -> Self {
        Self {
            adapters: [None, None, None, None, None, None, None, None],
            count: 0,
        }
    }
}

impl AdapterList {
    /// Returns an iterator over valid adapter infos.
    pub fn iter(&self) -> impl Iterator<Item = &AdapterInfo> {
        self.adapters[..self.count].iter().filter_map(|a| a.as_ref())
    }

    /// Adds an adapter to the list.
    pub fn push(&mut self, info: AdapterInfo) -> bool {
        if self.count < 8 {
            self.adapters[self.count] = Some(info);
            self.count += 1;
            true
        } else {
            false
        }
    }
}

// ============================================================================
// KgpuAdapterApi - Physical GPU
// ============================================================================

/// Adapter API for physical GPU information and device creation.
///
/// An adapter represents a physical GPU. It provides:
///
/// - Device capabilities and limits
/// - Feature support queries
/// - Logical device creation
///
/// # ASSUM Safety
///
/// - `#ASSUME_ADAPTER_VALID`: Adapter remains valid while instance exists
/// - `#ASSUME_FEATURES_IMMUTABLE`: Features don't change after enumeration
pub trait KgpuAdapterApi: Send + Sync + Sized {
    /// Associated device type.
    type Device: KgpuDeviceApi;

    /// Returns adapter information.
    fn info(&self) -> &AdapterInfo;

    /// Returns supported features.
    fn features(&self) -> Features;

    /// Returns device limits.
    fn limits(&self) -> &Limits;

    /// Creates a logical device from this adapter.
    ///
    /// # Arguments
    ///
    /// * `descriptor` - Device configuration including required features/limits.
    ///
    /// # Errors
    ///
    /// Returns `Err` if requested features are not supported.
    fn request_device(&self, descriptor: &DeviceDescriptor) -> HalResult<Self::Device>;

    /// Returns true if the adapter supports the given features.
    #[inline]
    fn supports(&self, features: Features) -> bool {
        self.features().contains(features)
    }
}

// ============================================================================
// KgpuDeviceApi - Logical Device
// ============================================================================

/// Device API for resource creation and management.
///
/// The device is the main interface for GPU operations:
///
/// - Resource creation (buffers, textures, samplers)
/// - Pipeline creation (render, compute)
/// - Bind group management
/// - Command encoder creation
///
/// # ASSUM Safety
///
/// - `#ASSUME_DEVICE_THREAD_SAFE`: Device can be used from any thread
/// - `#ASSUME_RESOURCE_TRACKING`: Device tracks resource lifetime
pub trait KgpuDeviceApi: Send + Sync + Sized {
    // Associated types
    type Queue: KgpuQueueApi;
    type Buffer: KgpuBufferApi;
    type Texture: KgpuTextureApi;
    type TextureView: KgpuTextureViewApi;
    type Sampler: KgpuSamplerApi;
    type BindGroupLayout: Send + Sync;
    type BindGroup: KgpuBindGroupApi;
    type PipelineLayout: Send + Sync;
    type ShaderModule: Send + Sync;
    type RenderPipeline: KgpuRenderPipelineApi;
    type ComputePipeline: KgpuComputePipelineApi;
    type CommandEncoder: KgpuCommandEncoderApi;

    /// Returns the command queue.
    fn queue(&self) -> &Self::Queue;

    /// Returns the device features.
    fn features(&self) -> Features;

    /// Returns the device limits.
    fn limits(&self) -> &Limits;

    // ========================================================================
    // Buffer Operations
    // ========================================================================

    /// Creates a buffer.
    fn create_buffer(&self, descriptor: &BufferDescriptor) -> HalResult<Self::Buffer>;

    // ========================================================================
    // Texture Operations
    // ========================================================================

    /// Creates a texture.
    fn create_texture(&self, descriptor: &TextureDescriptor) -> HalResult<Self::Texture>;

    // ========================================================================
    // Sampler Operations
    // ========================================================================

    /// Creates a sampler.
    fn create_sampler(&self, descriptor: &SamplerDescriptor) -> HalResult<Self::Sampler>;

    // ========================================================================
    // Bind Group Operations
    // ========================================================================

    /// Creates a bind group layout.
    fn create_bind_group_layout(
        &self,
        descriptor: &BindGroupLayoutDescriptor<'_>,
    ) -> HalResult<Self::BindGroupLayout>;

    /// Creates a bind group.
    fn create_bind_group(
        &self,
        layout: &Self::BindGroupLayout,
        entries: &[BindGroupEntry<'_>],
        label: Option<&'static str>,
    ) -> HalResult<Self::BindGroup>;

    // ========================================================================
    // Pipeline Operations
    // ========================================================================

    /// Creates a pipeline layout.
    fn create_pipeline_layout(
        &self,
        bind_group_layouts: &[&Self::BindGroupLayout],
        push_constant_ranges: &[PushConstantRange],
        label: Option<&'static str>,
    ) -> HalResult<Self::PipelineLayout>;

    /// Creates a shader module.
    fn create_shader_module(&self, source: ShaderSource<'_>) -> HalResult<Self::ShaderModule>;

    /// Creates a render pipeline.
    fn create_render_pipeline(
        &self,
        descriptor: &RenderPipelineDescriptor<'_, Self>,
    ) -> HalResult<Self::RenderPipeline>;

    /// Creates a compute pipeline.
    fn create_compute_pipeline(
        &self,
        descriptor: &ComputePipelineDescriptor<'_, Self>,
    ) -> HalResult<Self::ComputePipeline>;

    // ========================================================================
    // Command Operations
    // ========================================================================

    /// Creates a command encoder.
    fn create_command_encoder(
        &self,
        label: Option<&'static str>,
    ) -> HalResult<Self::CommandEncoder>;

    // ========================================================================
    // Device Maintenance
    // ========================================================================

    /// Polls the device for completed work.
    ///
    /// Returns `true` if there is more work pending.
    fn poll(&self, maintain: Maintain) -> bool;

    /// Returns device-specific error if device was lost.
    fn device_lost_reason(&self) -> Option<&'static str>;
}

/// Render pipeline descriptor.
#[derive(Debug)]
pub struct RenderPipelineDescriptor<'a, D: KgpuDeviceApi + ?Sized> {
    /// Debug label.
    pub label: Option<&'static str>,

    /// Pipeline layout.
    pub layout: Option<&'a D::PipelineLayout>,

    /// Vertex shader.
    pub vertex: VertexState<'a, D::ShaderModule>,

    /// Primitive state.
    pub primitive: PrimitiveState,

    /// Depth stencil state.
    pub depth_stencil: Option<DepthStencilState>,

    /// Multisample state.
    pub multisample: MultisampleState,

    /// Fragment shader and targets.
    pub fragment: Option<FragmentState<'a, D::ShaderModule>>,

    /// Multiview configuration.
    pub multiview: Option<core::num::NonZeroU32>,
}

/// Compute pipeline descriptor.
#[derive(Debug)]
pub struct ComputePipelineDescriptor<'a, D: KgpuDeviceApi + ?Sized> {
    /// Debug label.
    pub label: Option<&'static str>,

    /// Pipeline layout.
    pub layout: Option<&'a D::PipelineLayout>,

    /// Compute shader module.
    pub module: &'a D::ShaderModule,

    /// Entry point name.
    pub entry_point: &'static str,
}

// ============================================================================
// KgpuQueueApi - Command Submission
// ============================================================================

/// Queue API for command submission and data transfer.
///
/// The queue handles:
///
/// - Command buffer submission
/// - Direct buffer/texture writes
/// - Synchronization
///
/// # ASSUM Safety
///
/// - `#ASSUME_QUEUE_THREAD_SAFE`: Queue operations are thread-safe
/// - `#ASSUME_SUBMIT_ORDERED`: Submissions are processed in order
pub trait KgpuQueueApi: Send + Sync {
    /// Command buffer type.
    type CommandBuffer: Send + Sync;

    /// Submits command buffers for execution.
    fn submit<I>(&self, command_buffers: I)
    where
        I: IntoIterator<Item = Self::CommandBuffer>;

    /// Writes data directly to a buffer.
    fn write_buffer(&self, buffer: &impl KgpuBufferApi, offset: u64, data: &[u8]) -> HalResult<()>;

    /// Writes data directly to a texture.
    fn write_texture(
        &self,
        destination: ImageCopyTexture<'_>,
        data: &[u8],
        data_layout: ImageDataLayout,
        size: Extent3d,
    ) -> HalResult<()>;

    /// Blocks until all submitted work completes.
    fn on_submitted_work_done(&self) -> impl Future<Output = ()> + Send;
}

// ============================================================================
// KgpuBufferApi - GPU Buffer
// ============================================================================

/// Buffer API for GPU memory management.
///
/// Buffers store vertex, index, uniform, and storage data on the GPU.
///
/// # ASSUM Safety
///
/// - `#ASSUME_BUFFER_VALID`: Buffer remains valid until destroyed
/// - `#ASSUME_MAP_EXCLUSIVE`: Only one mapping active at a time
pub trait KgpuBufferApi: Send + Sync {
    /// Returns the buffer size in bytes.
    fn size(&self) -> u64;

    /// Returns the buffer usage flags.
    fn usage(&self) -> BufferUsages;

    /// Maps the buffer for CPU access.
    ///
    /// # Arguments
    ///
    /// * `mode` - Read or write access.
    /// * `range` - Byte range to map.
    ///
    /// # Returns
    ///
    /// A future that resolves when mapping completes.
    fn map_async(
        &self,
        mode: BufferMapMode,
        range: Range<u64>,
    ) -> impl Future<Output = MapResult<()>> + Send;

    /// Unmaps the buffer.
    fn unmap(&self);

    /// Returns a slice of the mapped buffer.
    fn slice(&self, range: Range<u64>) -> BufferSlice<'_>;

    /// Returns a pointer to the mapped memory.
    ///
    /// # Safety
    ///
    /// Buffer must be mapped. Pointer is valid only while mapped.
    fn mapped_ptr(&self) -> Option<*mut u8>;

    /// Destroys the buffer.
    fn destroy(&self);

    /// Returns true if the buffer is currently mapped.
    fn is_mapped(&self) -> bool;
}

// ============================================================================
// KgpuTextureApi - GPU Texture
// ============================================================================

/// Texture API for GPU image data.
///
/// Textures store image data for sampling or render targets.
///
/// # ASSUM Safety
///
/// - `#ASSUME_TEXTURE_VALID`: Texture valid until destroyed
/// - `#ASSUME_VIEW_LIFETIME`: Views valid while texture exists
pub trait KgpuTextureApi: Send + Sync {
    /// Associated view type.
    type View: KgpuTextureViewApi;

    /// Creates a view of this texture.
    fn create_view(&self, descriptor: &TextureViewDescriptor) -> Self::View;

    /// Returns the texture dimensions.
    fn size(&self) -> Extent3d;

    /// Returns the mip level count.
    fn mip_level_count(&self) -> u32;

    /// Returns the sample count.
    fn sample_count(&self) -> u32;

    /// Returns the texture dimension.
    fn dimension(&self) -> TextureDimension;

    /// Returns the texture format.
    fn format(&self) -> HalTextureFormat;

    /// Returns the usage flags.
    fn usage(&self) -> TextureUsages;

    /// Destroys the texture.
    fn destroy(&self);
}

// ============================================================================
// KgpuTextureViewApi - Texture View
// ============================================================================

/// Texture view API.
///
/// Views provide a specific interpretation of a texture for shaders.
pub trait KgpuTextureViewApi: Send + Sync {}

// ============================================================================
// KgpuSamplerApi - Texture Sampler
// ============================================================================

/// Sampler API for texture filtering.
///
/// Samplers define how textures are read in shaders.
pub trait KgpuSamplerApi: Send + Sync {}

// ============================================================================
// KgpuBindGroupApi - Resource Binding
// ============================================================================

/// Bind group API for shader resource binding.
///
/// Bind groups combine resources (buffers, textures, samplers) for shaders.
pub trait KgpuBindGroupApi: Send + Sync {}

// ============================================================================
// KgpuRenderPipelineApi - Render Pipeline
// ============================================================================

/// Render pipeline API.
///
/// Render pipelines define the complete graphics pipeline state.
pub trait KgpuRenderPipelineApi: Send + Sync {
    /// Returns the bind group layout for the given index.
    fn get_bind_group_layout(&self, index: u32) -> Option<()>;
}

// ============================================================================
// KgpuComputePipelineApi - Compute Pipeline
// ============================================================================

/// Compute pipeline API.
///
/// Compute pipelines define the compute shader state.
pub trait KgpuComputePipelineApi: Send + Sync {
    /// Returns the bind group layout for the given index.
    fn get_bind_group_layout(&self, index: u32) -> Option<()>;
}

// ============================================================================
// KgpuCommandEncoderApi - Command Recording
// ============================================================================

/// Command encoder API for recording GPU commands.
///
/// Command encoders record a sequence of commands for later submission.
///
/// # ASSUM Safety
///
/// - `#ASSUME_ENCODER_SINGLE_USE`: Each encoder used once
/// - `#ASSUME_PASS_EXCLUSIVE`: Only one pass active at a time
pub trait KgpuCommandEncoderApi: Send {
    /// Render pass type.
    type RenderPass<'a>: KgpuRenderPassApi
    where
        Self: 'a;

    /// Compute pass type.
    type ComputePass<'a>: KgpuComputePassApi
    where
        Self: 'a;

    /// Command buffer type (finished encoder).
    type CommandBuffer: Send + Sync;

    /// Begins a render pass.
    fn begin_render_pass<'a>(
        &'a mut self,
        descriptor: &RenderPassDescriptor<'a>,
    ) -> Self::RenderPass<'a>;

    /// Begins a compute pass.
    fn begin_compute_pass<'a>(
        &'a mut self,
        descriptor: &ComputePassDescriptor<'a>,
    ) -> Self::ComputePass<'a>;

    /// Copies data from one buffer to another.
    fn copy_buffer_to_buffer(
        &mut self,
        source: &impl KgpuBufferApi,
        source_offset: u64,
        destination: &impl KgpuBufferApi,
        destination_offset: u64,
        size: u64,
    );

    /// Copies data from a texture to another texture.
    fn copy_texture_to_texture(
        &mut self,
        source: ImageCopyTexture<'_>,
        destination: ImageCopyTexture<'_>,
        copy_size: Extent3d,
    );

    /// Copies data from a buffer to a texture.
    fn copy_buffer_to_texture(
        &mut self,
        source: ImageCopyBuffer<'_>,
        destination: ImageCopyTexture<'_>,
        copy_size: Extent3d,
    );

    /// Copies data from a texture to a buffer.
    fn copy_texture_to_buffer(
        &mut self,
        source: ImageCopyTexture<'_>,
        destination: ImageCopyBuffer<'_>,
        copy_size: Extent3d,
    );

    /// Finishes recording and returns the command buffer.
    fn finish(self) -> Self::CommandBuffer;
}

/// Render pass descriptor.
#[derive(Debug)]
pub struct RenderPassDescriptor<'a> {
    /// Debug label.
    pub label: Option<&'a str>,

    /// Color attachments.
    pub color_attachments: &'a [Option<RenderPassColorAttachment<'a>>],

    /// Depth/stencil attachment.
    pub depth_stencil_attachment: Option<RenderPassDepthStencilAttachment<'a>>,

    /// Occlusion query set.
    pub occlusion_query_set: Option<()>,

    /// Timestamp writes.
    pub timestamp_writes: Option<()>,
}

/// Color attachment for render pass.
#[derive(Debug)]
pub struct RenderPassColorAttachment<'a> {
    /// The texture view to render to.
    pub view: *const (),

    /// Resolve target for MSAA.
    pub resolve_target: Option<*const ()>,

    /// Load/store operations.
    pub ops: Operations<Color>,

    /// Lifetime marker.
    _marker: core::marker::PhantomData<&'a ()>,
}

impl<'a> RenderPassColorAttachment<'a> {
    /// Creates a new color attachment.
    pub const fn new(view: *const (), ops: Operations<Color>) -> Self {
        Self {
            view,
            resolve_target: None,
            ops,
            _marker: core::marker::PhantomData,
        }
    }
}

/// Depth/stencil attachment for render pass.
#[derive(Debug)]
pub struct RenderPassDepthStencilAttachment<'a> {
    /// The texture view.
    pub view: *const (),

    /// Depth operations.
    pub depth_ops: Option<Operations<f32>>,

    /// Stencil operations.
    pub stencil_ops: Option<Operations<u32>>,

    /// Lifetime marker.
    _marker: core::marker::PhantomData<&'a ()>,
}

impl<'a> RenderPassDepthStencilAttachment<'a> {
    /// Creates a new depth/stencil attachment.
    pub const fn new(view: *const (), depth_ops: Option<Operations<f32>>) -> Self {
        Self {
            view,
            depth_ops,
            stencil_ops: None,
            _marker: core::marker::PhantomData,
        }
    }
}

// ============================================================================
// KgpuRenderPassApi - Render Pass Commands
// ============================================================================

/// Render pass API for draw commands.
///
/// Records draw commands within a render pass.
///
/// # ASSUM Safety
///
/// - `#ASSUME_PASS_ACTIVE`: Commands only valid while pass is active
/// - `#ASSUME_PIPELINE_SET`: Pipeline must be set before drawing
pub trait KgpuRenderPassApi {
    /// Sets the render pipeline.
    fn set_pipeline(&mut self, pipeline: &impl KgpuRenderPipelineApi);

    /// Sets a bind group.
    fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: &impl KgpuBindGroupApi,
        offsets: &[u32],
    );

    /// Sets a vertex buffer.
    fn set_vertex_buffer(&mut self, slot: u32, buffer: &impl KgpuBufferApi, offset: u64);

    /// Sets the index buffer.
    fn set_index_buffer(
        &mut self,
        buffer: &impl KgpuBufferApi,
        format: IndexFormat,
        offset: u64,
    );

    /// Sets the viewport.
    fn set_viewport(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        min_depth: f32,
        max_depth: f32,
    );

    /// Sets the scissor rectangle.
    fn set_scissor_rect(&mut self, x: u32, y: u32, width: u32, height: u32);

    /// Sets the blend constant.
    fn set_blend_constant(&mut self, color: Color);

    /// Sets the stencil reference.
    fn set_stencil_reference(&mut self, reference: u32);

    /// Draws primitives.
    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>);

    /// Draws indexed primitives.
    fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>);

    /// Draws with indirect parameters.
    fn draw_indirect(&mut self, indirect_buffer: &impl KgpuBufferApi, indirect_offset: u64);

    /// Draws indexed with indirect parameters.
    fn draw_indexed_indirect(
        &mut self,
        indirect_buffer: &impl KgpuBufferApi,
        indirect_offset: u64,
    );

    /// Sets push constants.
    fn set_push_constants(&mut self, stages: ShaderStages, offset: u32, data: &[u8]);
}

// ============================================================================
// KgpuComputePassApi - Compute Pass Commands
// ============================================================================

/// Compute pass API for dispatch commands.
///
/// Records compute dispatch commands.
///
/// # ASSUM Safety
///
/// - `#ASSUME_PASS_ACTIVE`: Commands only valid while pass is active
/// - `#ASSUME_PIPELINE_SET`: Pipeline must be set before dispatch
pub trait KgpuComputePassApi {
    /// Sets the compute pipeline.
    fn set_pipeline(&mut self, pipeline: &impl KgpuComputePipelineApi);

    /// Sets a bind group.
    fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: &impl KgpuBindGroupApi,
        offsets: &[u32],
    );

    /// Dispatches workgroups.
    fn dispatch_workgroups(&mut self, x: u32, y: u32, z: u32);

    /// Dispatches workgroups with indirect parameters.
    fn dispatch_workgroups_indirect(
        &mut self,
        indirect_buffer: &impl KgpuBufferApi,
        indirect_offset: u64,
    );

    /// Sets push constants.
    fn set_push_constants(&mut self, offset: u32, data: &[u8]);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Type Bounds Tests
    // ========================================================================

    #[test]
    fn test_error_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<HalError>();
        assert_send_sync::<MapError>();
        assert_send_sync::<SurfaceError>();
    }

    #[test]
    fn test_types_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<BackendType>();
        assert_send_sync::<DeviceType>();
        assert_send_sync::<Features>();
        assert_send_sync::<Limits>();
        assert_send_sync::<BufferUsages>();
        assert_send_sync::<TextureUsages>();
        assert_send_sync::<AdapterInfo>();
        assert_send_sync::<AdapterOptions>();
        assert_send_sync::<DeviceDescriptor>();
    }

    // ========================================================================
    // AdapterList Tests
    // ========================================================================

    #[test]
    fn test_adapter_list_default() {
        let list = AdapterList::default();
        assert_eq!(list.count, 0);
        assert_eq!(list.iter().count(), 0);
    }

    #[test]
    fn test_adapter_list_push() {
        let mut list = AdapterList::default();
        let info = AdapterInfo::new("Test GPU", DeviceType::DiscreteGpu, BackendType::Vulkan);

        assert!(list.push(info.clone()));
        assert_eq!(list.count, 1);
        assert_eq!(list.iter().count(), 1);
        assert_eq!(list.iter().next().unwrap().name_str(), "Test GPU");
    }

    #[test]
    fn test_adapter_list_full() {
        let mut list = AdapterList::default();

        for i in 0..8 {
            let info = AdapterInfo::new("GPU", DeviceType::DiscreteGpu, BackendType::Vulkan);
            assert!(list.push(info), "Should push adapter {}", i);
        }

        let info = AdapterInfo::new("Overflow", DeviceType::DiscreteGpu, BackendType::Vulkan);
        assert!(!list.push(info), "Should not push beyond 8");
    }

    // ========================================================================
    // Feature Tests
    // ========================================================================

    #[test]
    fn test_features_empty() {
        let f = Features::empty();
        assert!(f.is_empty());
        assert!(!Features::RAY_TRACING.is_empty());
    }

    #[test]
    fn test_features_contains() {
        let f = Features::RAY_TRACING | Features::MESH_SHADER;
        assert!(f.contains(Features::RAY_TRACING));
        assert!(f.contains(Features::MESH_SHADER));
        assert!(!f.contains(Features::DEPTH_CLIP_CONTROL));
    }

    #[test]
    fn test_features_union() {
        let f1 = Features::RAY_TRACING;
        let f2 = Features::MESH_SHADER;
        let combined = f1 | f2;
        assert!(combined.contains(f1));
        assert!(combined.contains(f2));
    }

    #[test]
    fn test_features_intersection() {
        let f1 = Features::RAY_TRACING | Features::MESH_SHADER;
        let f2 = Features::MESH_SHADER | Features::TASK_SHADER;
        let common = f1 & f2;
        assert!(common.contains(Features::MESH_SHADER));
        assert!(!common.contains(Features::RAY_TRACING));
        assert!(!common.contains(Features::TASK_SHADER));
    }

    // ========================================================================
    // Buffer Usage Tests
    // ========================================================================

    #[test]
    fn test_buffer_usages_empty() {
        let u = BufferUsages::empty();
        assert!(u.is_empty());
    }

    #[test]
    fn test_buffer_usages_combined() {
        let u = BufferUsages::VERTEX | BufferUsages::INDEX;
        assert!(u.contains(BufferUsages::VERTEX));
        assert!(u.contains(BufferUsages::INDEX));
        assert!(!u.contains(BufferUsages::UNIFORM));
    }

    // ========================================================================
    // Texture Format Tests
    // ========================================================================

    #[test]
    fn test_texture_format_bytes_per_block() {
        assert_eq!(HalTextureFormat::R8Unorm.bytes_per_block(), 1);
        assert_eq!(HalTextureFormat::Rgba8Unorm.bytes_per_block(), 4);
        assert_eq!(HalTextureFormat::Rgba16Float.bytes_per_block(), 8);
        assert_eq!(HalTextureFormat::Rgba32Float.bytes_per_block(), 16);
    }

    #[test]
    fn test_texture_format_is_depth() {
        assert!(HalTextureFormat::Depth32Float.is_depth());
        assert!(HalTextureFormat::Depth24Plus.is_depth());
        assert!(!HalTextureFormat::Rgba8Unorm.is_depth());
    }

    #[test]
    fn test_texture_format_is_stencil() {
        assert!(HalTextureFormat::Stencil8.is_stencil());
        assert!(HalTextureFormat::Depth24PlusStencil8.is_stencil());
        assert!(!HalTextureFormat::Depth32Float.is_stencil());
    }

    #[test]
    fn test_texture_format_is_compressed() {
        assert!(HalTextureFormat::Bc1RgbaUnorm.is_compressed());
        assert!(HalTextureFormat::Bc7RgbaUnorm.is_compressed());
        assert!(!HalTextureFormat::Rgba8Unorm.is_compressed());
    }

    // ========================================================================
    // Extent Tests
    // ========================================================================

    #[test]
    fn test_extent3d_texel_count() {
        let e = Extent3d::new(1920, 1080, 1);
        assert_eq!(e.texel_count(), 1920 * 1080);

        let e2 = Extent3d::new(256, 256, 6);
        assert_eq!(e2.texel_count(), 256 * 256 * 6);
    }

    // ========================================================================
    // Limits Tests
    // ========================================================================

    #[test]
    fn test_limits_default() {
        let limits = Limits::default();
        assert!(limits.max_texture_dimension_2d >= 2048);
        assert!(limits.max_buffer_size >= 128 * 1024 * 1024);
        assert!(limits.max_bind_groups >= 4);
    }

    #[test]
    fn test_limits_downlevel() {
        let limits = Limits::downlevel_defaults();
        assert!(limits.max_texture_dimension_2d >= 2048);
        assert!(limits.max_push_constant_size == 0);
    }

    // ========================================================================
    // Backend Type Tests
    // ========================================================================

    #[test]
    fn test_backend_type_name() {
        assert_eq!(BackendType::Vulkan.name(), "Vulkan");
        assert_eq!(BackendType::Metal.name(), "Metal");
        assert_eq!(BackendType::Dx12.name(), "DX12");
        assert_eq!(BackendType::WebGpu.name(), "WebGPU");
        assert_eq!(BackendType::Null.name(), "Null");
    }

    #[test]
    fn test_backend_null_always_supported() {
        assert!(BackendType::Null.is_platform_supported());
    }

    // ========================================================================
    // Device Type Tests
    // ========================================================================

    #[test]
    fn test_device_type_is_hardware() {
        assert!(DeviceType::DiscreteGpu.is_hardware());
        assert!(DeviceType::IntegratedGpu.is_hardware());
        assert!(!DeviceType::VirtualGpu.is_hardware());
        assert!(!DeviceType::Cpu.is_hardware());
    }

    #[test]
    fn test_device_type_performance_tier() {
        assert!(DeviceType::DiscreteGpu.performance_tier() > DeviceType::IntegratedGpu.performance_tier());
        assert!(DeviceType::IntegratedGpu.performance_tier() > DeviceType::VirtualGpu.performance_tier());
        assert!(DeviceType::VirtualGpu.performance_tier() > DeviceType::Cpu.performance_tier());
    }

    // ========================================================================
    // Color Tests
    // ========================================================================

    #[test]
    fn test_color_constants() {
        assert_eq!(Color::BLACK.r, 0.0);
        assert_eq!(Color::BLACK.a, 1.0);
        assert_eq!(Color::WHITE.r, 1.0);
        assert_eq!(Color::TRANSPARENT.a, 0.0);
    }

    // ========================================================================
    // Shader Stages Tests
    // ========================================================================

    #[test]
    fn test_shader_stages_combined() {
        let stages = ShaderStages::VERTEX | ShaderStages::FRAGMENT;
        assert!(stages.contains(ShaderStages::VERTEX));
        assert!(stages.contains(ShaderStages::FRAGMENT));
        assert!(!stages.contains(ShaderStages::COMPUTE));
    }

    #[test]
    fn test_shader_stages_all() {
        assert!(ShaderStages::ALL.contains(ShaderStages::VERTEX));
        assert!(ShaderStages::ALL.contains(ShaderStages::FRAGMENT));
        assert!(ShaderStages::ALL.contains(ShaderStages::COMPUTE));
    }

    // ========================================================================
    // Vertex Format Tests
    // ========================================================================

    #[test]
    fn test_vertex_format_size() {
        assert_eq!(VertexFormat::Float32.size(), 4);
        assert_eq!(VertexFormat::Float32x2.size(), 8);
        assert_eq!(VertexFormat::Float32x3.size(), 12);
        assert_eq!(VertexFormat::Float32x4.size(), 16);
    }

    // ========================================================================
    // Index Format Tests
    // ========================================================================

    #[test]
    fn test_index_format_size() {
        assert_eq!(IndexFormat::Uint16.size(), 2);
        assert_eq!(IndexFormat::Uint32.size(), 4);
    }

    // ========================================================================
    // Error Tests
    // ========================================================================

    #[test]
    fn test_hal_error_is_recoverable() {
        assert!(HalError::OutOfDeviceMemory.is_recoverable());
        assert!(HalError::Timeout.is_recoverable());
        assert!(!HalError::DeviceLost.is_recoverable());
    }

    #[test]
    fn test_hal_error_is_device_lost() {
        assert!(HalError::DeviceLost.is_device_lost());
        assert!(!HalError::OutOfDeviceMemory.is_device_lost());
    }

    #[test]
    fn test_hal_error_codes() {
        // Memory errors: 1xxx
        assert!(HalError::OutOfDeviceMemory.error_code() >= 1000);
        assert!(HalError::OutOfDeviceMemory.error_code() < 2000);

        // Device errors: 2xxx
        assert!(HalError::DeviceLost.error_code() >= 2000);
        assert!(HalError::DeviceLost.error_code() < 3000);
    }

    // ========================================================================
    // RenderPassColorAttachment Tests
    // ========================================================================

    #[test]
    fn test_render_pass_color_attachment() {
        let view = core::ptr::null();
        let ops = Operations {
            load: LoadOp::Clear(Color::BLACK),
            store: StoreOp::Store,
        };
        let attachment = RenderPassColorAttachment::new(view, ops);
        assert!(attachment.resolve_target.is_none());
    }

    // ========================================================================
    // BufferBinding Tests
    // ========================================================================

    #[test]
    fn test_buffer_binding() {
        let binding = BufferBinding::new(0, Some(1024));
        assert_eq!(binding.offset, 0);
        assert_eq!(binding.size, Some(1024));
    }

    // ========================================================================
    // Operations Tests
    // ========================================================================

    #[test]
    fn test_operations_default() {
        let ops: Operations<f32> = Operations::default();
        assert!(matches!(ops.load, LoadOp::Load));
        assert!(matches!(ops.store, StoreOp::Store));
    }
}
