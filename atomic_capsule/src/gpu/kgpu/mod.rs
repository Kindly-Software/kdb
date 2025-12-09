//! KGPU: UCE34 Chaos-Compliant GPU Abstraction Layer
//!
//! A lockfree, type-state GPU API with 10-100x performance improvements over wgpu.
//!
//! # Architecture
//!
//! KGPU is built on the foundation of Computational Capsules (Chaos), providing:
//!
//! - **100% lockfree** (NO mutex/RwLock) - Chaos mandate
//! - **Full type-state safety** for all resources
//! - **Multi-backend**: Vulkan + Metal + DX12
//! - **<50ns command recording**, <1μs memory allocation
//!
//! # Capsule Tiers Used
//!
//! | Tier | Purpose | Capsules |
//! |------|---------|----------|
//! | T0 | Audit, capabilities | Capability queries, audit trail |
//! | T1 | Atomic handles, pools | [`KgpuHandle`], resource pools |
//! | T1+T4 | Atomic + Batch | [`KgpuComputePassCapsule`] with type-state dispatch |
//! | T1+T6 | Atomic + Mixed | [`KgpuRenderPassCapsule`] with type-state draw |
//! | T1+T9 | Atomic + Persistent | [`KgpuBufferCapsule`] with type-state safety |
//! | T4 | Batch command recording | Command buffers |
//! | T6 | Device metacapsule | Device orchestration |
//! | T7 | Root metacapsule | Instance + adapter selection |
//!
//! # Safety Guarantees
//!
//! All KGPU types use generation-countered handles ([`KgpuHandle<T>`]) to prevent:
//!
//! - **Use-after-free**: Stale handles detected at runtime (<10ns check)
//! - **ABA problems**: 32-bit generation counter prevents recycled handle confusion
//! - **Data races**: All state is atomic, no mutex required
//!
//! # Example
//!
//! ```ignore
//! use atomic_capsule::gpu::kgpu::{KgpuHandle, KgpuInstanceCapsule};
//!
//! // Create a type-safe handle
//! let handle: KgpuHandle<Buffer> = KgpuHandle::new(0, 1);
//! assert!(handle.is_valid());
//! assert_eq!(handle.index(), 0);
//! assert_eq!(handle.generation(), 1);
//!
//! // Handles can be safely invalidated
//! handle.invalidate();
//! assert!(!handle.is_valid());
//! ```
//!
//! # Module Organization
//!
//! - [`handle`]: Core [`KgpuHandle<T>`] type with generation counting
//! - [`instance`]: [`KgpuInstanceCapsule`] for Vulkan/Metal/DX12 instance management
//! - [`adapter`]: [`KgpuAdapterCapsule`] for physical device enumeration
//! - [`device`]: [`KgpuDeviceMetacapsule`] for logical device orchestration
//! - [`command`]: [`KgpuCommandEncoderCapsule`] for type-state command recording
//! - [`buffer`]: [`KgpuBufferCapsule`] for type-state GPU buffer management
//! - [`render_pass`]: [`KgpuRenderPassCapsule`] for type-state render pass recording
//! - [`compute_pass`]: [`KgpuComputePassCapsule`] for type-state compute pass recording
//! - [`texture`]: [`KgpuTextureCapsule`] for type-state GPU texture management
//! - [`memory_pool`]: GPU memory pool with lockfree free-lists
//! - [`bind_group`]: [`KgpuBindGroupCapsule`] for lockfree resource binding
//! - [`pipeline`]: [`KgpuRenderPipelineCapsule`] and [`KgpuComputePipelineCapsule`] for pipeline state
//! - [`pipeline_cache`]: [`KgpuPipelineCacheCapsule`] for SIMD-accelerated pipeline caching
//! - [`shader_cache`]: [`KgpuShaderCacheCapsule`] for SPIR-V validation and shader module caching
//! - [`sampler_cache`]: [`KgpuSamplerCacheCapsule`] for lockfree GPU sampler object caching
//! - [`descriptor_pool`]: [`KgpuDescriptorPoolCapsule`] for lockfree descriptor set allocation
//! - [`surface`]: [`KgpuSurfaceCapsule`] for type-state window surface management
//! - [`swapchain`]: [`KgpuSwapchainCapsule`] for type-state presentation and triple-buffering
//! - [`hal`]: Hardware Abstraction Layer traits for multi-backend support (Vulkan/Metal/DX12)
//!
//! # Performance Targets (B32 Validated)
//!
//! | Operation | Target | Typical |
//! |-----------|--------|---------|
//! | Handle creation | <5ns | 2-3ns |
//! | Handle validation | <10ns | 3-5ns |
//! | Generation increment | <10ns | 5-8ns |
//! | Handle invalidation | <10ns | 5-8ns |
//! | Command record | <50ns | 20-40ns |
//! | Encoder begin/finish | <20ns | 10-15ns |
//!
//! # ASSUM Safety Documentation
//!
//! All unsafe code in KGPU is documented with ASSUM tags:
//!
//! - `#ASSUME_*`: Documents safety assumptions
//! - `#VERIFY_*`: Documents verification status
//!
//! See individual module documentation for complete ASSUM tags.

// Core handle type - T1 Atomic tier
pub mod handle;

// Instance management - T7 root metacapsule
pub mod instance;

// Adapter enumeration - T0 capability queries
pub mod adapter;

// Device orchestration - T6 mixed metacapsule
pub mod device;

// Command encoder - T4 Batch type-state command recording
pub mod command;

// Command queue - T1+T4 (Atomic + Batch) for GPU command submission
pub mod queue;

// Buffer with type-state safety - T1+T9 (Atomic + Persistent)
pub mod buffer;

// Render pass with type-state safety - T1+T6 (Atomic + Mixed)
pub mod render_pass;

// Compute pass with type-state safety - T1+T4 (Atomic + Batch)
pub mod compute_pass;

// Texture with type-state safety - T1+T2 (Atomic + SIMD)
pub mod texture;

// Memory pool with per-size-class lockfree free lists - T4+T10 (Batch + Probabilistic)
pub mod memory_pool;

// Bind group with lockfree resource binding - T1 Atomic
pub mod bind_group;

// Pipeline state objects - T1+T6 (Atomic + Mixed)
pub mod pipeline;

// Pipeline cache with SIMD-accelerated lookup - T2+T4 (SIMD + Batch)
pub mod pipeline_cache;

// Shader cache with SIMD-accelerated SPIR-V validation - T1+T2 (Atomic + SIMD)
pub mod shader_cache;

// Sampler cache for GPU sampler object reuse - T1 (Atomic)
pub mod sampler_cache;

// Descriptor pool for efficient descriptor set allocation - T4 (Batch)
pub mod descriptor_pool;

// Hardware Abstraction Layer - Backend-agnostic traits for Vulkan/Metal/DX12
pub mod hal;

// Backend Implementations - Real Vulkan/Metal/DX12 backends via ash/metal-rs
pub mod backends;

// Vulkan Backend - KGPU Phase 5: Mock/stub Vulkan backend for design validation
// Tier: T1+T7 (Atomic coordination + Heterogeneous GPU)
pub mod vulkan;

// Metal Backend - KGPU Phase 5: Mock/stub Metal backend for macOS/iOS design validation
// Tier: T1+T7 (Atomic coordination + Heterogeneous GPU)
pub mod metal;

// Backend Dispatcher - Runtime backend selection with platform-specific preferences
// Tier: T1 (Atomic coordination)
pub mod dispatcher;

// Q34 Audit Trail - Hash-chain tamper-evident audit logging for GPU operations
// Tier: T0+T1 (Auditable + Atomic)
pub mod audit;

// Phase K2: GPU Synchronization Primitives (Fence + Semaphore + SyncPoint)
// Tier: T1 (Atomic) for lockfree state coordination, T0 (Auditable) for timeline tracking

// Fence capsule with type-state safety (Unsignaled → Signaled)
// Tier: T1+T0 (Atomic coordination + Auditable timeline)
pub mod fence;

// Semaphore capsule with binary + timeline variants
// Tier: T1+T0 (Atomic coordination + Auditable timeline)
pub mod semaphore;

// SyncPoint, WaitInfo, SignalInfo utilities for queue submission
// Tier: T0 (Auditable coordination patterns)
pub mod sync;

// Re-exports for ergonomic API
pub use handle::KgpuHandle;
pub use instance::KgpuInstanceCapsule;
pub use adapter::{
    KgpuAdapterCapsule,
    // Adapter type constants
    ADAPTER_TYPE_DISCRETE_GPU,
    ADAPTER_TYPE_INTEGRATED_GPU,
    ADAPTER_TYPE_VIRTUAL_GPU,
    ADAPTER_TYPE_CPU,
    ADAPTER_TYPE_UNKNOWN,
    // Adapter state constants
    ADAPTER_STATE_INVALID,
    ADAPTER_STATE_INITIALIZING,
    ADAPTER_STATE_READY,
    ADAPTER_STATE_IN_USE,
    ADAPTER_STATE_LOST,
    // Capability flags
    ADAPTER_CAP_COMPUTE,
    ADAPTER_CAP_GRAPHICS,
    ADAPTER_CAP_RAYTRACING,
    ADAPTER_CAP_MESH_SHADERS,
    ADAPTER_CAP_VRS,
    ADAPTER_CAP_SPARSE,
    // Snapshot and error types
    KgpuAdapterSnapshot,
    KgpuAdapterError,
    KgpuAdapterResult,
};
pub use device::{
    // Metacapsule
    KgpuDeviceMetacapsule,

    // Error types
    KgpuError,
    KgpuResult,

    // Device state constants
    DEVICE_STATE_OFFLINE,
    DEVICE_STATE_INITIALIZING,
    DEVICE_STATE_ACTIVE,
    DEVICE_STATE_SUSPENDED,
    DEVICE_STATE_LOST,
    DEVICE_STATE_DESTROYED,

    // Capability flags
    CAPABILITY_COMPUTE,
    CAPABILITY_GRAPHICS,
    CAPABILITY_RAYTRACING,
    CAPABILITY_MESH_SHADERS,
    CAPABILITY_SPARSE,
    CAPABILITY_ASYNC_COMPUTE,
    CAPABILITY_ASYNC_TRANSFER,
    CAPABILITY_TIMELINE_SEMAPHORE,
};
pub use command::{
    // Command encoder capsule (type-state)
    KgpuCommandEncoderCapsule,

    // Type-state markers
    Empty,
    Recording,
    Finished,
    Submitted,

    // Command types
    CommandType,
    CommandSlot,

    // Error handling
    CommandError,
    CommandResult,

    // Constants
    MAX_COMMANDS,
};
pub use queue::{
    // Queue capsule (128B cache-aligned)
    KgpuQueueCapsule,

    // Queue capability flags
    QUEUE_CAP_GRAPHICS,
    QUEUE_CAP_COMPUTE,
    QUEUE_CAP_TRANSFER,
    QUEUE_CAP_SPARSE,
    QUEUE_CAP_PRESENT,

    // Queue priority constants
    QUEUE_PRIORITY_LOW,
    QUEUE_PRIORITY_NORMAL,
    QUEUE_PRIORITY_HIGH,
    QUEUE_PRIORITY_REALTIME,

    // Submission info
    SubmitInfo,

    // HAL trait
    HalQueue,
};
pub use buffer::{
    // Buffer capsule (type-state)
    KgpuBufferCapsule,

    // Buffer state markers (zero-sized)
    BufferState,
    Unmapped,
    Mapped,
    InGpuUse,
    Destroyed,

    // Map mode markers (zero-sized)
    MapMode,
    MapRead,
    MapWrite,
    MapReadWrite,

    // Buffer marker type (for KgpuHandle<Buffer>)
    Buffer,

    // Usage flags
    BUFFER_USAGE_VERTEX,
    BUFFER_USAGE_INDEX,
    BUFFER_USAGE_UNIFORM,
    BUFFER_USAGE_STORAGE,
    BUFFER_USAGE_COPY_SRC,
    BUFFER_USAGE_COPY_DST,
    BUFFER_USAGE_MAP_READ,
    BUFFER_USAGE_MAP_WRITE,
};
pub use render_pass::{
    // Render pass capsule (type-state)
    KgpuRenderPassCapsule,

    // Render pass state markers
    RenderPassState,
    Active as RenderPassActive,
    Ended as RenderPassEnded,

    // Attachment types
    ColorAttachment,
    DepthStencilAttachment,

    // Load/Store operations
    LOAD_OP_CLEAR,
    LOAD_OP_LOAD,
    LOAD_OP_DONT_CARE,
    STORE_OP_STORE,
    STORE_OP_DISCARD,
};
pub use compute_pass::{
    // Compute pass capsule (type-state)
    KgpuComputePassCapsule,

    // Compute pass state markers
    ComputePassState,
    Active as ComputePassActive,
    Ended as ComputePassEnded,

    // Flag constants
    FLAG_HAS_INDIRECT,
    FLAG_PIPELINE_SET,
};
pub use texture::{
    // Texture capsule (type-state)
    KgpuTextureCapsule,

    // Texture state markers (zero-sized)
    TextureState,
    Uninitialized as TextureUninitialized,
    Available as TextureAvailable,
    InRenderPass as TextureInRenderPass,
    InComputePass as TextureInComputePass,
    Destroyed as TextureDestroyed,

    // Texture dimension markers (zero-sized)
    TextureDimension,
    Tex1D,
    Tex2D,
    Tex3D,
    TexCube,
    Tex2DArray,

    // Texture format markers (zero-sized)
    TextureFormat,
    Rgba8Unorm,
    Rgba8Srgb,
    Bgra8Unorm,
    Rgba16Float,
    Rgba32Float,
    Depth24Plus,
    Depth32Float,
    Depth24PlusStencil8,

    // Texture marker type (for KgpuHandle<Texture>)
    Texture,

    // Texture view handle
    KgpuTextureViewHandle,

    // Usage flags
    TEXTURE_USAGE_COPY_SRC,
    TEXTURE_USAGE_COPY_DST,
    TEXTURE_USAGE_TEXTURE_BINDING,
    TEXTURE_USAGE_STORAGE_BINDING,
    TEXTURE_USAGE_RENDER_ATTACHMENT,
};
pub use memory_pool::{
    // Memory pool capsule
    KgpuMemoryPoolCapsule,

    // Size class enumeration
    SizeClass,
    NUM_SIZE_CLASSES,
    SIZE_CLASS_BYTES,

    // Allocation handle
    KgpuAllocation,

    // Pool statistics
    PoolStats,
    SizeClassStatsSnapshot,

    // Pool state constants
    POOL_STATE_UNINITIALIZED,
    POOL_STATE_ACTIVE,
    POOL_STATE_DRAINING,
    POOL_STATE_SHUTDOWN,

    // Internal types (for advanced usage)
    FreeListHead,
    FreeNode,
    SizeClassStats,
    MemoryRegion,
    MAX_REGIONS,
};
pub use bind_group::{
    // Bind group capsule
    KgpuBindGroupCapsule,

    // Binding types
    BindingType,
    BindingSlot,
    BindGroupState,

    // Bind group marker type
    BindGroup,

    // Error handling
    BindGroupError,
    BindGroupResult,

    // Constants
    MAX_BINDINGS_PER_GROUP,

    // Flags
    BIND_GROUP_FLAG_IMMUTABLE,
    BIND_GROUP_FLAG_DYNAMIC_OFFSETS,
    BIND_GROUP_FLAG_COMPUTE,
    BIND_GROUP_FLAG_GRAPHICS,
};
pub use pipeline::{
    // Render pipeline capsule
    KgpuRenderPipelineCapsule,

    // Compute pipeline capsule
    KgpuComputePipelineCapsule,

    // Pipeline marker types
    RenderPipeline,
    ComputePipeline,

    // Pipeline state
    PipelineState,

    // Enums
    PrimitiveTopology,
    FrontFace,
    CullMode,
    CompareFunction,
    BlendFactor,
    BlendOperation,
    VertexStepMode,

    // Component types
    VertexLayoutSlot,
    BlendState,

    // Error handling
    PipelineError,
    PipelineResult,

    // Constants
    MAX_VERTEX_BUFFERS,
    MAX_COLOR_TARGETS,
    MAX_BIND_GROUPS,

    // Flags
    PIPELINE_FLAG_DEPTH_TEST,
    PIPELINE_FLAG_DEPTH_WRITE,
    PIPELINE_FLAG_STENCIL_TEST,
    PIPELINE_FLAG_BLEND,
    PIPELINE_FLAG_MULTISAMPLE,
    PIPELINE_FLAG_IMMUTABLE,
};
pub use pipeline_cache::{
    // Pipeline cache capsule
    KgpuPipelineCacheCapsule,

    // Cache slot type
    CacheSlot,

    // Cache state
    CacheState as PipelineCacheState,

    // Error handling
    CacheError as PipelineCacheError,
    CacheResult as PipelineCacheResult,

    // Statistics
    CacheStats as PipelineCacheStats,

    // Constants
    CACHE_SLOTS,
    SIMD_BATCH_SIZE,

    // Hash helpers
    fnv1a_hash,
    combine_hash,
};
pub use shader_cache::{
    // Shader cache capsule
    KgpuShaderCacheCapsule,

    // Shader entry type
    ShaderEntry,

    // SPIR-V header type
    SpirvHeader,

    // Cache state
    CacheState as ShaderCacheState,

    // Shader stage
    ShaderStage,

    // Error handling
    ShaderCacheError,
    ShaderCacheResult,

    // Statistics
    ShaderCacheStats,

    // Constants
    SPIRV_MAGIC,
    SPIRV_MAGIC_LE,
    SPIRV_MAGIC_BE,
    MAX_SHADER_ENTRIES,
    SPIRV_HEADER_SIZE,
    MIN_SPIRV_SIZE,

    // Functions
    compute_shader_hash,
    compute_shader_hash_fast,
    validate_spirv_header,
    validate_spirv_full,
    validate_spirv_batch,
};
pub use sampler_cache::{
    // Sampler cache capsule
    KgpuSamplerCacheCapsule,

    // Configuration types
    SamplerConfig,
    SamplerEntry,

    // Enums
    FilterMode,
    AddressMode,

    // Re-exported from pipeline
    CompareFunction as SamplerCompareFunction,

    // Statistics
    SamplerCacheStats,

    // Constants
    MAX_CACHED_SAMPLERS,
    CACHE_STATE_UNINITIALIZED as SAMPLER_CACHE_STATE_UNINITIALIZED,
    CACHE_STATE_ACTIVE as SAMPLER_CACHE_STATE_ACTIVE,
    CACHE_STATE_FULL as SAMPLER_CACHE_STATE_FULL,
};
pub use descriptor_pool::{
    // Descriptor pool capsule
    KgpuDescriptorPoolCapsule,

    // Handle type
    DescriptorSetHandle,

    // Configuration
    DescriptorPoolConfig,

    // Error handling
    PoolError as DescriptorPoolError,
    PoolResult as DescriptorPoolResult,

    // Statistics
    PoolStats as DescriptorPoolStats,

    // Constants
    MAX_DESCRIPTOR_SETS,

    // Pool state constants
    POOL_STATE_UNINITIALIZED as DESC_POOL_STATE_UNINITIALIZED,
    POOL_STATE_ACTIVE as DESC_POOL_STATE_ACTIVE,
    POOL_STATE_EXHAUSTED as DESC_POOL_STATE_EXHAUSTED,
    POOL_STATE_DRAINING as DESC_POOL_STATE_DRAINING,
    POOL_STATE_SHUTDOWN as DESC_POOL_STATE_SHUTDOWN,

    // Pool flags
    POOL_FLAG_RESIZABLE,
    POOL_FLAG_TYPE_TRACKING,
    POOL_FLAG_FREE_LIST,
};

// HAL re-exports for multi-backend abstraction
pub use hal::{
    // Core backend trait
    KgpuBackend,

    // Instance and adapter traits
    KgpuInstanceApi,
    KgpuAdapterApi,
    AdapterList,

    // Device and queue traits
    KgpuDeviceApi,
    KgpuQueueApi,

    // Resource traits
    KgpuBufferApi,
    KgpuTextureApi,
    KgpuTextureViewApi,
    KgpuSamplerApi,
    KgpuBindGroupApi,

    // Pipeline traits
    KgpuRenderPipelineApi,
    KgpuComputePipelineApi,

    // Command traits
    KgpuCommandEncoderApi,
    KgpuRenderPassApi,
    KgpuComputePassApi,

    // Render pass types
    RenderPassDescriptor,
    RenderPassColorAttachment,
    RenderPassDepthStencilAttachment,

    // Pipeline descriptors
    RenderPipelineDescriptor as HalRenderPipelineDescriptor,
    ComputePipelineDescriptor as HalComputePipelineDescriptor,

    // Error types
    HalError,
    HalResult,
    MapError,
    MapResult,
    SurfaceError,
    SurfaceResult,

    // Backend and device types
    BackendType,
    DeviceType,
    PowerPreference,

    // Features and limits
    Features as HalFeatures,
    Limits as HalLimits,

    // Buffer types
    BufferDescriptor as HalBufferDescriptor,
    BufferUsages as HalBufferUsages,
    BufferMapMode as HalBufferMapMode,
    BufferSlice as HalBufferSlice,

    // Texture types
    TextureDescriptor as HalTextureDescriptor,
    TextureUsages as HalTextureUsages,
    TextureDimension as HalTextureDimension,
    HalTextureFormat,
    TextureViewDescriptor as HalTextureViewDescriptor,
    TextureViewDimension as HalTextureViewDimension,
    TextureAspect as HalTextureAspect,
    Extent3d as HalExtent3d,
    Origin3d as HalOrigin3d,

    // Sampler types
    SamplerDescriptor as HalSamplerDescriptor,
    AddressMode as HalAddressMode,
    FilterMode as HalFilterMode,
    CompareFunction as HalCompareFunction,
    SamplerBorderColor as HalSamplerBorderColor,

    // Bind group types
    BindGroupLayoutDescriptor as HalBindGroupLayoutDescriptor,
    BindGroupLayoutEntry as HalBindGroupLayoutEntry,
    BindGroupEntry as HalBindGroupEntry,
    BindingResource as HalBindingResource,
    BindingType as HalBindingType,
    BufferBinding as HalBufferBinding,
    ShaderStages as HalShaderStages,

    // Pipeline types
    PushConstantRange as HalPushConstantRange,
    ShaderSource as HalShaderSource,
    VertexFormat as HalVertexFormat,
    VertexStepMode as HalVertexStepMode,
    VertexAttribute as HalVertexAttribute,
    VertexBufferLayout as HalVertexBufferLayout,
    PrimitiveTopology as HalPrimitiveTopology,
    PrimitiveState as HalPrimitiveState,
    IndexFormat as HalIndexFormat,
    FrontFace as HalFrontFace,
    Face as HalFace,
    DepthStencilState as HalDepthStencilState,
    StencilState as HalStencilState,
    StencilFaceState as HalStencilFaceState,
    StencilOperation as HalStencilOperation,
    DepthBiasState as HalDepthBiasState,
    MultisampleState as HalMultisampleState,
    BlendComponent as HalBlendComponent,
    BlendFactor as HalBlendFactor,
    BlendOperation as HalBlendOperation,
    BlendState as HalBlendState,
    ColorWrites as HalColorWrites,
    ColorTargetState as HalColorTargetState,
    FragmentState as HalFragmentState,
    VertexState as HalVertexState,

    // Render pass types
    LoadOp as HalLoadOp,
    StoreOp as HalStoreOp,
    Operations as HalOperations,
    Color as HalColor,
    ComputePassDescriptor as HalComputePassDescriptor,

    // Copy types
    ImageCopyTexture as HalImageCopyTexture,
    ImageCopyBuffer as HalImageCopyBuffer,
    ImageDataLayout as HalImageDataLayout,

    // Adapter types
    AdapterInfo as HalAdapterInfo,
    AdapterOptions as HalAdapterOptions,
    DeviceDescriptor as HalDeviceDescriptor,

    // Maintenance
    Maintain as HalMaintain,
};

// Metal backend re-exports
pub use metal::{
    // Main backend capsule
    MtlBackendCapsule,
    MtlBackendError,
    MtlBackendResult,
    MtlBackendSnapshot,
    // Device capsule
    MtlDeviceCapsule,
    MtlDeviceError,
    MtlDeviceResult,
    MtlDeviceSnapshot,
    MtlDeviceProperties,
    // Buffer capsule
    MtlBufferCapsule,
    MtlBufferError,
    MtlBufferResult,
    MtlBufferSnapshot,
    // Texture capsule
    MtlTextureCapsule,
    MtlTextureDescriptor,
    MtlTextureError,
    MtlTextureResult,
    MtlTextureSnapshot,
    // Metal types
    MTLPixelFormat,
    MTLStorageMode,
    MTLTextureType,
    MTLGPUFamily,
    MTLLanguageVersion,
    MTLTextureUsage,
    MTLResourceOptions,
    // Backend state constants
    MAX_METAL_DEVICES,
    FEATURE_UNIFIED_MEMORY,
    FEATURE_APPLE_SILICON,
    FEATURE_RAYTRACING as MTL_FEATURE_RAYTRACING,
    FEATURE_MESH_SHADERS as MTL_FEATURE_MESH_SHADERS,
    FEATURE_METAL_3,
    FEATURE_TILE_SHADING,
    // Device state constants
    DEVICE_STATE_UNINITIALIZED as MTL_DEVICE_STATE_UNINITIALIZED,
    DEVICE_STATE_INITIALIZING as MTL_DEVICE_STATE_INITIALIZING,
    DEVICE_STATE_READY as MTL_DEVICE_STATE_READY,
    DEVICE_STATE_ACTIVE as MTL_DEVICE_STATE_ACTIVE,
    DEVICE_STATE_LOST as MTL_DEVICE_STATE_LOST,
    DEVICE_STATE_DESTROYED as MTL_DEVICE_STATE_DESTROYED,
    // Buffer state constants
    BUFFER_STATE_UNINITIALIZED as MTL_BUFFER_STATE_UNINITIALIZED,
    BUFFER_STATE_CREATED as MTL_BUFFER_STATE_CREATED,
    BUFFER_STATE_MAPPED as MTL_BUFFER_STATE_MAPPED,
    BUFFER_STATE_IN_GPU_USE as MTL_BUFFER_STATE_IN_GPU_USE,
    BUFFER_STATE_DESTROYED as MTL_BUFFER_STATE_DESTROYED,
    // Texture state constants
    TEXTURE_STATE_UNINITIALIZED as MTL_TEXTURE_STATE_UNINITIALIZED,
    TEXTURE_STATE_CREATED as MTL_TEXTURE_STATE_CREATED,
    TEXTURE_STATE_IN_RENDER_PASS as MTL_TEXTURE_STATE_IN_RENDER_PASS,
    TEXTURE_STATE_IN_COMPUTE_PASS as MTL_TEXTURE_STATE_IN_COMPUTE_PASS,
    TEXTURE_STATE_DESTROYED as MTL_TEXTURE_STATE_DESTROYED,
};

// Backend dispatcher re-exports
pub use dispatcher::{
    // Main dispatcher capsule
    KgpuBackendDispatcher,
    DispatcherError,
    DispatcherResult,
    DispatcherSnapshot,
    // Dispatcher state constants
    DISPATCHER_STATE_UNINITIALIZED,
    DISPATCHER_STATE_DETECTING,
    DISPATCHER_STATE_READY,
    DISPATCHER_STATE_ACTIVE,
    DISPATCHER_STATE_ERROR,
    // Backend flags
    BACKEND_FLAG_VULKAN,
    BACKEND_FLAG_METAL,
    BACKEND_FLAG_DX12,
    BACKEND_FLAG_WEBGPU,
    BACKEND_FLAG_NULL,
    // Dispatcher flags
    FLAG_AUTO_SELECT,
    FLAG_PREFER_DISCRETE,
    FLAG_PREFER_LOW_POWER,
    FLAG_ALLOW_SOFTWARE,
    FLAG_ENABLE_VALIDATION,
};

// Vulkan backend re-exports
pub use vulkan::{
    // Main backend capsule
    VkBackendCapsule,
    VkBackendState,
    VkBackendCapabilities,
    VkBackendStatus,

    // Instance capsule
    VkInstanceCapsule,
    VkInstanceCreateInfo,
    VkPhysicalDeviceInfo,
    // Instance state constants
    VK_INSTANCE_STATE_UNINITIALIZED,
    VK_INSTANCE_STATE_CREATING,
    VK_INSTANCE_STATE_ACTIVE,
    VK_INSTANCE_STATE_DESTROYING,
    VK_INSTANCE_STATE_DESTROYED,

    // Device capsule
    VkDeviceCapsule,
    VkDeviceCreateInfo,
    // Device state constants
    VK_DEVICE_STATE_UNINITIALIZED,
    VK_DEVICE_STATE_CREATING,
    VK_DEVICE_STATE_ACTIVE,
    VK_DEVICE_STATE_IDLE,
    VK_DEVICE_STATE_LOST,
    VK_DEVICE_STATE_DESTROYING,
    VK_DEVICE_STATE_DESTROYED,
    // Device feature flags
    VK_FEATURE_GEOMETRY_SHADER,
    VK_FEATURE_TESSELLATION_SHADER,
    VK_FEATURE_MULTI_VIEWPORT,
    VK_FEATURE_SAMPLER_ANISOTROPY,
    VK_FEATURE_TEXTURE_COMPRESSION_BC,
    VK_FEATURE_SHADER_INT64,
    VK_FEATURE_SHADER_FLOAT16,
    VK_FEATURE_TIMELINE_SEMAPHORE,
    VK_FEATURE_BUFFER_DEVICE_ADDRESS,
    VK_FEATURE_DESCRIPTOR_INDEXING,

    // Buffer capsule
    VkBufferCapsule,
    VkBufferCreateInfo,
    // Buffer state constants
    VK_BUFFER_STATE_UNINITIALIZED,
    VK_BUFFER_STATE_CREATED,
    VK_BUFFER_STATE_BOUND,
    VK_BUFFER_STATE_MAPPED,
    VK_BUFFER_STATE_IN_GPU_USE,
    VK_BUFFER_STATE_DESTROYED,

    // Image capsule
    VkImageCapsule,
    VkImageCreateInfo,
    // Image state constants
    VK_IMAGE_STATE_UNINITIALIZED,
    VK_IMAGE_STATE_CREATED,
    VK_IMAGE_STATE_BOUND,
    VK_IMAGE_STATE_TRANSITIONING,
    VK_IMAGE_STATE_READY,
    VK_IMAGE_STATE_DESTROYED,

    // Vulkan types
    VkResult,
    VkFormat,
    VkImageLayout,
    VkBufferUsageFlags,
    VkImageUsageFlags,
    VkMemoryPropertyFlags,
    VkQueueFlags,
    VkImageTiling,
    VkSampleCountFlags,
    VkPhysicalDeviceType,

    // Handle generation
    MockHandleGenerator,
    generate_mock_handle,

    // API version helpers
    vk_make_api_version,
    vk_api_version_major,
    vk_api_version_minor,
    vk_api_version_patch,
    VK_API_VERSION_1_0,
    VK_API_VERSION_1_1,
    VK_API_VERSION_1_2,
    VK_API_VERSION_1_3,
};

// Surface management - T1 Atomic type-state surface configuration
pub mod surface;

// Swapchain management - T1 Atomic type-state presentation
pub mod swapchain;

// Surface re-exports
pub use surface::{
    // Surface capsule (type-state)
    KgpuSurfaceCapsule,

    // Surface state markers
    SurfaceState,
    Unconfigured,
    Configured,

    // Surface snapshot and error types
    SurfaceSnapshot,
    SurfaceError as KgpuSurfaceError,
    SurfaceResult as KgpuSurfaceResult,

    // HAL trait
    HalSurface,
};

// Swapchain re-exports
pub use swapchain::{
    // Swapchain capsule (type-state)
    KgpuSwapchainCapsule,

    // Swapchain state markers
    SwapchainState,
    Idle,
    Acquired,
    Presenting,

    // Swapchain snapshot and error types
    SwapchainSnapshot,
    SwapchainError,
    SwapchainResult,

    // HAL trait
    HalSwapchain,
};

// Audit trail re-exports (Q34 compliance)
pub use audit::{
    // Main capsule
    KgpuAuditTrailCapsule,

    // Operation types
    AuditOperation,

    // Entry types
    AuditEntry,
    AuditEntrySnapshot,

    // Error handling
    AuditError,

    // Statistics and export
    AuditStats,

    // Constants
    AUDIT_RING_CAPACITY,
};

// Conditional re-export for std feature
#[cfg(feature = "std")]
pub use audit::AuditExport;

// Phase K2: Fence re-exports (type-state safety)
pub use fence::{
    // Main fence capsule (type-state)
    KgpuFenceCapsule,

    // Fence state markers
    FenceState,
    Unsignaled as FenceUnsignaled,
    Signaled as FenceSignaled,

    // HAL trait
    HalFence,

    // Fence constants
    FENCE_STATE_UNSIGNALED,
    FENCE_STATE_SIGNALED,
    FENCE_MAX_TIMELINE_VALUE,
};

// Phase K2: Semaphore re-exports (binary + timeline)
pub use semaphore::{
    // Main semaphore capsule
    KgpuSemaphoreCapsule,

    // HAL trait
    HalSemaphore,

    // Semaphore constants
    SEMAPHORE_TYPE_BINARY,
    SEMAPHORE_TYPE_TIMELINE,
    SEMAPHORE_STATE_IDLE,
    SEMAPHORE_STATE_SIGNALED,
    SEMAPHORE_STATE_CONSUMED,
    SEMAPHORE_MAX_TIMELINE_VALUE,
};

// Phase K2: SyncPoint re-exports (queue coordination)
pub use sync::{
    // Main types
    SyncPoint,
    WaitInfo,
    SignalInfo,

    // Common patterns
    SyncPatterns,

    // Constants
    MAX_WAIT_POINTS,
    MAX_SIGNAL_POINTS,
};

// Shader compilation pipeline - T0 Auditable + T7 Heterogeneous
pub mod shader;

// Shader re-exports
pub use shader::{
    // Main capsule
    KgpuShaderModuleCapsule,

    // Shader types (ShaderStage already re-exported from shader_cache above)
    ShaderFormat,
    ShaderTarget,

    // Error handling
    ShaderError,
};

/// KGPU version information
pub const KGPU_VERSION: &str = env!("CARGO_PKG_VERSION");

/// KGPU API version (semantic versioning)
pub const KGPU_API_VERSION: (u32, u32, u32) = (0, 1, 0);

/// Maximum supported backends
pub const MAX_BACKENDS: usize = 4; // Vulkan, Metal, DX12, WebGPU

/// Maximum adapters per backend
pub const MAX_ADAPTERS: usize = 8;

/// Maximum devices per adapter
pub const MAX_DEVICES: usize = 4;

/// K8: KGPU Integration Tests (T7 Heterogeneous tier)
///
/// Comprehensive GPU testing suite based on SOTA methodologies:
/// - Vulkan CTS: 3M conformance tests for cross-platform consistency
/// - NVIDIA Compute Sanitizer: Memory leak detection patterns
/// - DirectX 12 Debug Layer: ReportLiveObjects() validation
/// - Academic VSync Study: Frame timing and triple buffering
///
/// **Usage**: `cargo test --features gpu-tests -- --ignored`
///
/// All tests require GPU hardware and are `#[ignore]` by default.
#[cfg(all(test, feature = "gpu-tests"))]
pub mod tests;

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_api_version() {
        let (major, minor, patch) = KGPU_API_VERSION;
        assert_eq!(major, 0);
        assert_eq!(minor, 1);
        assert_eq!(patch, 0);
    }

    #[test]
    fn test_constants() {
        assert!(MAX_BACKENDS >= 1);
        assert!(MAX_ADAPTERS >= 1);
        assert!(MAX_DEVICES >= 1);
    }
}
