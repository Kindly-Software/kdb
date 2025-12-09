//! Graphics Pipeline Capsules - T7 Heterogeneous Tier
//!
//! State-of-the-art graphics pipeline management with Vulkan 1.3 FFI core,
//! SPIR-V compilation, descriptor set management, and render pass coordination.
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────┐
//! │                Graphics Pipeline Stack                     │
//! ├────────────────────────────────────────────────────────────┤
//! │ VulkanCoreCapsule (NEW)                                    │
//! │   └─ Vulkan 1.3 FFI bindings (ash-based)                  │
//! │   └─ Instance/Device/Queue management                      │
//! │   └─ Lockfree coordination via DualAtomicU64               │
//! ├────────────────────────────────────────────────────────────┤
//! │ SpirVCompilerCapsule                                       │
//! │   └─ GLSL/HLSL → SPIR-V compilation                       │
//! │   └─ Shader reflection & specialization constants         │
//! │   └─ Pipeline cache integration                            │
//! ├────────────────────────────────────────────────────────────┤
//! │ PushDescriptorsCapsule                                     │
//! │   └─ VK_KHR_push_descriptor inline updates                 │
//! ├────────────────────────────────────────────────────────────┤
//! │ RayTracingPipelineCapsule                                  │
//! │   └─ RT pipeline creation & SBT management                 │
//! │   └─ Shader group binding                                  │
//! └────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Modules
//!
//! - [`vulkan_core`]: Vulkan 1.3 FFI core bindings (ash-based, zero-overhead)
//! - [`spirv_compiler`]: SPIR-V shader compilation with caching
//! - [`push_descriptors`]: VK_KHR_push_descriptor inline descriptor updates
//! - [`ray_tracing_pipeline`]: Ray tracing pipeline with SBT management
//! - [`descriptor_indexing`]: Bindless descriptor arrays
//! - [`acceleration_structure`]: RT acceleration structures
//! - [`indirect_commands`]: GPU-driven indirect rendering (NEW)
//!
//! # Future Modules (Phase 2)
//!
//! - `descriptor_manager`: Descriptor set allocation and pooling
//! - `pipeline_cache`: Pipeline state object caching
//! - `command_buffer`: Graphics command buffer encoding

pub mod vulkan_core;
pub mod spirv_compiler;
pub mod push_descriptors;
pub mod ray_tracing_pipeline;
pub mod descriptor_indexing;
pub mod acceleration_structure;
pub mod indirect_commands;
pub mod shader_binding_table;

pub use vulkan_core::{
    VulkanCoreCapsule,
    VulkanVersion,
    QueueCapability,
    PhysicalDeviceType,
    MemoryProperty,
};

pub use spirv_compiler::{
    SpirVCompilerCapsule,
    ShaderStage,
    ShaderModule,
    OptLevel,
    TargetEnv,
    CompilationStats,
};

pub use push_descriptors::{
    PushDescriptorsCapsule,
    DescriptorWrite,
    DescriptorType,
    ImageLayout,
    PushStats,
};

pub use ray_tracing_pipeline::{
    RayTracingPipelineCapsule,
    ShaderGroupType,
    RtShaderStage,
    ShaderGroup,
    SbtRegion,
};

pub use descriptor_indexing::{
    DescriptorIndexingCapsule,
    DescriptorType as DescriptorIndexingType,
    BindingFlag,
    BindingInfo,
    SlotAllocation,
};

pub use acceleration_structure::{
    AccelerationStructureCapsule,
    AccelStructSnapshot,
    AccelStructType,
    GeometryType,
    BuildFlags,
    AccelInstance,
};

pub use indirect_commands::{
    IndirectCommandsCapsule,
    DrawIndirectCommand,
    DrawIndexedIndirectCommand,
    DispatchIndirectCommand,
    IndirectCountBuffer,
    CommandType,
};

pub use shader_binding_table::{
    ShaderBindingTableCapsule,
    SbtRegion as SbtRegionType,
    StridedRegion,
};
