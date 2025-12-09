//! KGPU-Driver: Pure Rust GPU Driver Stack (Replaces Vulkan)
//!
//! A Chaos-compliant GPU driver that sits BELOW where Vulkan sits, talking directly
//! to GPU hardware. This is the foundation for Capsule-OS graphics.
//!
//! # Architecture Position
//!
//! ```text
//! Traditional Stack:              KGPU-Driver:
//! ┌─────────────────┐            ┌─────────────────┐
//! │  Application    │            │  Application    │
//! ├─────────────────┤            ├─────────────────┤
//! │  Vulkan API     │ ◄── HERE   │  KGPU API       │
//! ├─────────────────┤            ├─────────────────┤
//! │  Mesa/Driver    │            │  KGPU-Driver    │ ◄── HERE (replaces Mesa)
//! ├─────────────────┤            ├─────────────────┤
//! │  Kernel (DRM)   │            │  Kernel/Direct  │
//! ├─────────────────┤            ├─────────────────┤
//! │  GPU Hardware   │            │  GPU Hardware   │
//! └─────────────────┘            └─────────────────┘
//! ```
//!
//! # Multi-Vendor Strategy
//!
//! | Vendor | Kernel Approach | Userspace Approach | Firmware |
//! |--------|-----------------|-------------------|----------|
//! | Intel | Direct ring buffer (Gen9+) | MI commands | GuC/HuC (loadable) |
//! | AMD | CP ring buffer | PM4 packets | PSP (loadable) |
//! | NVIDIA | **Trojan Kernel** | Pinned memory ring | GSP (bypassed) |
//!
//! # NVIDIA Trojan Kernel
//!
//! Since NVIDIA's GSP firmware is cryptographically locked, we use the "Persistent
//! Kernel" approach:
//!
//! 1. Launch a persistent CUDA kernel at startup (never returns)
//! 2. The kernel polls a ring buffer in pinned shared memory
//! 3. Our Rust code writes commands to the ring buffer
//! 4. The GPU kernel picks up and executes commands instantly (<100ns)
//!
//! This achieves sovereign control over NVIDIA GPUs while remaining 100% legal.
//!
//! # Dual-Target Support
//!
//! The driver supports both Linux and Capsule-OS through the [`GpuPlatform`] trait:
//!
//! - **Linux**: Uses DRM/GEM via existing `linux_drm.rs`, `linux_pci.rs`
//! - **Capsule-OS**: Direct hardware access (no kernel intermediary)
//!
//! # Module Organization
//!
//! - [`platform`]: Core [`GpuPlatform`] trait for dual-target abstraction
//! - [`vendor`]: Vendor detection and generation identification
//! - [`error`]: Error types and results
//! - [`memory`]: GPU memory management capsules
//! - [`memory_pressure`]: PSI-inspired memory pressure management with CLOCK-Pro LRU
//! - [`linux_platform`]: Main Linux platform implementation ([`LinuxGpuPlatformCapsule`])
//! - [`linux_drm`]: DRM device management and capabilities
//! - [`linux_gem`]: GEM buffer object subsystem (Linux DRM)
//! - [`linux_kms`]: KMS display management (connectors, CRTCs, planes)
//! - [`intel_driver`]: Intel i915/xe driver-specific implementation
//! - [`intel_ring`]: Intel MI command ring buffer
//! - [`amd_driver`]: AMD amdgpu driver-specific implementation
//! - [`amd_ring`]: AMD PM4 command ring buffer
//! - [`nvidia_ring`]: NVIDIA Trojan Kernel ring buffer
//! - [`trojan_ptx`]: Embedded CUDA PTX for NVIDIA Trojan Kernel
//! - [`trojan_manager`]: NVIDIA Trojan Kernel lifecycle management
//! - [`cuda_ffi`]: CUDA Driver API bindings
//!
//! # Capsule Tiers Used
//!
//! | Tier | Purpose | Capsules |
//! |------|---------|----------|
//! | T0 | Audit, capabilities | [`GpuDeviceInfo`], Q34 audit |
//! | T1 | Atomic ring buffers | [`LinuxGpuPlatformCapsule`], [`IntelRingCapsule`], [`AmdCpRingCapsule`], [`NvidiaTrojanRingCapsule`], [`GemBufferCapsule`], [`I915ContextCapsule`], [`AmdgpuContextCapsule`] |
//! | T4 | Batch command encoding | PM4 packets, MI commands |
//! | T7 | Backend codegen | Intel Gen EU, AMD GCN, NVIDIA PTX |
//!
//! # Feature Flags
//!
//! - `kgpu-driver`: Enable the driver module
//! - `kgpu-driver-linux`: Enable Linux platform (DRM/GEM)
//! - `kgpu-driver-capsule-os`: Enable Capsule-OS platform (direct)
//! - `kgpu-driver-intel`: Enable Intel GPU support
//! - `kgpu-driver-amd`: Enable AMD GPU support
//! - `kgpu-driver-nvidia`: Enable NVIDIA GPU support (requires CUDA)
//!
//! # Example
//!
//! ```ignore
//! use atomic_capsule::gpu::kgpu_driver::{
//!     GpuPlatform, LinuxGpuPlatformCapsule, MemoryFlags,
//! };
//!
//! // Enumerate available GPUs
//! let devices = LinuxGpuPlatformCapsule::enumerate_devices()?;
//! println!("Found {} GPU(s)", devices.len());
//!
//! for dev in &devices {
//!     println!("  {:?}: {} (vendor: {:04x}, device: {:04x})",
//!              dev.generation, dev.name_str(), dev.vendor_id, dev.device_id);
//! }
//!
//! // Open the first device
//! let handle = LinuxGpuPlatformCapsule::open_device(0)?;
//!
//! // Allocate GPU memory
//! let mem = LinuxGpuPlatformCapsule::alloc_memory(
//!     handle,
//!     4096,
//!     MemoryFlags::GPU_VISIBLE | MemoryFlags::CPU_VISIBLE,
//! )?;
//!
//! // Use platform capsule for extended operations (display management)
//! let platform = LinuxGpuPlatformCapsule::new();
//! platform.initialize()?;
//! let displays = platform.get_displays(handle)?;
//! println!("Found {} display(s)", displays.len());
//!
//! // Clean up
//! LinuxGpuPlatformCapsule::free_memory(handle, mem)?;
//! LinuxGpuPlatformCapsule::close_device(handle)?;
//! ```
//!
//! # Safety
//!
//! KGPU-Driver follows Chaos mandate: 100% lockfree, no mutex/RwLock.
//! All capsules use DualAtomicU64 patterns with generation counters.
//!
//! ASSUM tags document all unsafe assumptions:
//! - `#ASSUME_MMIO_SAFE`: MMIO regions are properly mapped
//! - `#ASSUME_DMA_COHERENT`: DMA buffers are cache-coherent
//! - `#ASSUME_RING_VALID`: Ring buffer pointers are in bounds

#![allow(dead_code)] // Allow during development


#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

// ============================================================================
// Sub-modules
// ============================================================================

pub mod vendor;
pub mod error;
pub mod memory;
pub mod memory_pressure;
pub mod platform;
pub mod intel_ring;
pub mod amd_ring;
pub mod nvidia_ring;
pub mod cross_queue_sync;

// Power management capsules (Phase 5)
pub mod power_state_capsule;
pub mod frequency_manager_capsule;
pub mod thermal_monitor_capsule;

// Trojan PTX module - embedded CUDA PTX bytecode for NVIDIA Trojan Kernel
// Does not require CUDA SDK - just needs kgpu-driver feature
#[cfg(target_os = "linux")]
pub mod trojan_ptx;

#[cfg(all(feature = "kgpu-driver-nvidia", target_os = "linux"))]
pub mod trojan_manager;

// CUDA FFI module - Safe Rust bindings for CUDA Driver API
// Enables the Trojan Kernel approach for NVIDIA GPU control
#[cfg(all(feature = "kgpu-driver-nvidia", target_os = "linux"))]
pub mod cuda_ffi;

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub mod linux_drm;

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub mod linux_gem;

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub mod linux_kms;

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub mod linux_platform;

// Intel i915 driver-specific module
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub mod intel_driver;

// Intel Xe2 backend - Meteor Lake+ GPU support (T1 Atomic capsule)
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub mod intel_xe2_backend;

// Intel Xe2 DRM device management capsule
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub mod xe_drm_capsule;

// Intel Xe2 GEM buffer object capsule
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub mod xe_gem_capsule;

// Intel Xe2 execution capsule
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub mod xe_exec_capsule;

// Intel Xe2 ring buffer capsule
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub mod xe_ring_capsule;

// Intel Xe2 compute capsule (Phase 4)
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub mod xe_compute_capsule;

// Intel Xe2 shader capsule (Phase 4)
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub mod xe_shader_capsule;

// Intel Xe2 display capsule (Phase 5)
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub mod xe_display_capsule;

// Intel Xe2 framebuffer capsule (Phase 5) - PLANNED
// #[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
// pub mod xe_framebuffer_capsule;

// AMD amdgpu driver-specific module
#[cfg(all(feature = "kgpu-driver-amd", target_os = "linux"))]
pub mod amd_driver;

// SPIR-V binary parser - Zero-copy parsing and IR conversion
pub mod spirv_parser;

// Shader IR module - Optimizing intermediate representation
pub mod shader_ir;

// Backend code generation - Multi-vendor GPU code generation
pub mod backend_codegen;

// GPU page table - Universal multi-level page table abstraction
pub mod gpu_page_table;

// GART allocator - Lockfree buddy allocator for GPU aperture memory
pub mod gart_allocator;

// Semaphore pool - Lockfree GPU semaphore allocator (binary/timeline)
pub mod semaphore_pool;

// Timeline fence - Lockfree GPU timeline synchronization (Vulkan/D3D12/dma_fence)
pub mod timeline_fence;

// Command timing - SOTA GPU timestamp query implementation (Vulkan/D3D12)
pub mod command_timing;

// GPU performance counters - Multi-vendor performance monitoring (Intel OA, AMD GRBM, NVIDIA SM, Vulkan)
pub mod gpu_counters;

// Bandwidth profiler - SOTA GPU memory bandwidth analysis (Intel MBM, AMD Infinity Fabric, NVIDIA DCGM, cuThermo heat maps)
pub mod bandwidth_profiler;

// ============================================================================
// Phase 2: Command Submission System (4 capsules, SOTA research-backed)
// ============================================================================

// Ring buffer capsule - Lockfree circular command buffer (Intel/AMD/NVIDIA)
pub mod ring_buffer_capsule;

// Batch builder capsule - Parallel command construction and validation (Mesa ANV-inspired)
pub mod batch_builder_capsule;

// Fence sync capsule - Seqno-based GPU/CPU synchronization (AMD AMDGPU-inspired)
pub mod fence_sync_capsule;

// Command streamer capsule - Hardware CS register programming (Intel CS architecture)
pub mod command_streamer_capsule;

// ============================================================================
// Phase 10: Capsule-OS Direct Platform (Bare Metal GPU Access)
// ============================================================================

// MMIO region - Direct memory-mapped I/O for GPU registers (bypass DRM)
pub mod mmio_region;

// Bare metal allocator - Physical memory management without OS support
pub mod bare_metal_allocator;

// Register access - Type-safe GPU register operations with vendor definitions
pub mod register_access;

// Phase 3: Memory Management (4 capsules, SOTA research-backed)
pub mod gtt_manager_capsule;
pub mod page_table_capsule_phase3;
pub mod gem_object_capsule_phase3;
pub mod eviction_manager_capsule_phase3;

// ============================================================================
// Re-exports
// ============================================================================

pub use vendor::{
    GpuVendor,
    GpuGeneration,
    PciBdf,
    PciDeviceId,
    detect_vendor,
    detect_generation,
};

pub use error::{
    KgpuDriverError,
    KgpuDriverResult,
    ErrorCategory,
    ErrorContext,
};

pub use memory::{
    GpuMemoryCapsule,
    GpuMemorySnapshot,
    MemoryState,
};

pub use memory_pressure::{
    MemoryPressureCapsule,
    PressureLevel,
    PressureAction,
    PressureError,
};

pub use platform::{
    GpuPlatform,
    GpuDeviceInfo,
    MemoryFlags,
    QueueType,
    SubmissionId,
    FenceHandle,
    FirmwareType,
    FirmwareStatus,
    GPU_DEVICE_NAME_LEN,
};

pub use intel_ring::{
    IntelRingCapsule,
    IntelRingSnapshot,
    IntelEngineClass,
    MiOpcode,
    MiCommandHeader,
    RingState,
    RingFlags,
};

pub use amd_ring::{
    AmdCpRingCapsule,
    AmdCpRingSnapshot,
    AmdQueueType,
    CpRingState,
    Pm4Header,
    Pm4Opcode,
    Pm4PacketType,
};

pub use ring_buffer_capsule::{
    RingBufferCapsule,
    RingBufferSnapshot,
    RingError,
};

pub use batch_builder_capsule::{
    BatchBuilderCapsule,
    BatchBuilderSnapshot,
    BatchError,
    RelocationEntry,
};

pub use fence_sync_capsule::{
    FenceSyncCapsule,
    FenceSyncSnapshot,
    FenceError as FenceSyncError,
    FenceMode,
};

pub use command_streamer_capsule::{
    CommandStreamerCapsule,
    CSSnapshot,
    CSError,
    EngineType,
    CSState,
};

pub use nvidia_ring::{
    NvidiaTrojanRingCapsule,
    NvidiaTrojanRingSnapshot,
    TrojanCommand,
    TrojanOpcode,
    TrojanState,
    TrojanKernelParams,
};

pub use cross_queue_sync::{
    CrossQueueSyncCapsule,
    CrossQueueSnapshot,
    QueueStateSnapshot,
    QueueType as CrossQueueType,
};

// Trojan PTX exports - available without CUDA SDK
#[cfg(target_os = "linux")]
pub use trojan_ptx::{
    // PTX selection
    ComputeCapability,
    PtxArchTier,
    select_ptx_tier,
    get_ptx_for_device,
    get_ptx_str_for_device,
    get_inline_ptx,
    // PTX validation
    validate_ptx,
    extract_target_sm,
    // Kernel names
    KERNEL_TROJAN_POLL,
    KERNEL_HEALTH_CHECK,
    KERNEL_RING_RESET,
    KERNEL_TIMESTAMP,
    // Layout verification
    cmd_layout,
    header_layout,
    verify_command_layout,
    // Constants
    PTX_VERSION,
    MIN_CUDA_DRIVER,
    PTX_MAGIC,
    // Inline PTX source
    TROJAN_PTX_INLINE,
    TROJAN_PTX_SM70,
    TROJAN_PTX_SM52_BIN,
    TROJAN_PTX_SM70_BIN,
    TROJAN_PTX_SM80_BIN,
};

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub use linux_drm::{
    DrmNodeType,
    DrmCapability,
    DrmClientCapability,
    DrmVersion,
    DrmDeviceCapsule,
    DrmDeviceInfo,
    PrimeFlags,
    // Driver detection
    vendor_from_driver_name,
    is_open_source_driver,
    // Utility
    fnv1a_hash,
    parse_pci_bdf_from_unique,
    // Device enumeration
    enumerate_drm_devices,
    open_drm_device,
    close_drm_device,
    // Capability queries
    query_drm_capabilities,
    has_prime,
    has_syncobj,
    has_syncobj_timeline,
    has_atomic,
    has_modifiers,
    // High-level
    query_device_info,
    enumerate_and_query_devices,
    // ioctl module
    ioctl,
};

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub use linux_gem::{
    // GEM capsules
    GemBufferCapsule,
    GemBufferSnapshot,
    // Types
    GemState,
    GemFlags,
    // DRM ioctl functions (re-exported from linux_impl)
    create_dumb,
    get_dumb_map_offset,
    mmap_buffer,
    munmap_buffer,
    destroy_dumb,
    gem_close,
    export_prime,
    import_prime,
    gem_flink,
    gem_open,
};

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub use linux_kms::{
    // KMS capsules
    KmsConnectorCapsule,
    KmsConnectorSnapshot,
    KmsCrtcCapsule,
    KmsCrtcSnapshot,
    KmsPlaneCapsule,
    KmsPlaneSnapshot,
    // Resources
    KmsResources,
    KmsPlaneResources,
    // Types
    ConnectorType,
    ConnectionStatus,
    PlaneType,
    DpmsState,
    SubpixelLayout,
    DrmMode,
    VBlankEvent,
    FramebufferHandle,
    DrmDeviceCapsule as KmsDrmDeviceCapsule,
    // Flags
    PageFlipFlags,
    AtomicFlags,
    AtomicRequest,
    AtomicProperty,
    // Constants
    formats,
    rotation,
    mode_flags,
    // ioctl constants
    DRM_IOCTL_MODE_GETRESOURCES,
    DRM_IOCTL_MODE_GETCONNECTOR,
    DRM_IOCTL_MODE_GETENCODER,
    DRM_IOCTL_MODE_GETCRTC,
    DRM_IOCTL_MODE_GETPLANE,
    DRM_IOCTL_MODE_GETPLANERESOURCES,
    DRM_IOCTL_MODE_SETCRTC,
    DRM_IOCTL_MODE_PAGE_FLIP,
    DRM_IOCTL_MODE_ADDFB2,
    DRM_IOCTL_MODE_RMFB,
    DRM_IOCTL_MODE_ATOMIC,
    DRM_IOCTL_WAIT_VBLANK,
};

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub use linux_platform::{
    // Main platform capsule
    LinuxGpuPlatformCapsule,
    // Handle types
    LinuxDeviceHandle,
    LinuxMemoryHandle,
    LinuxFenceHandle,
    // Display info
    DisplayInfo,
    // Snapshot
    PlatformSnapshot,
};

#[cfg(all(feature = "kgpu-driver-nvidia", target_os = "linux"))]
pub use trojan_manager::{
    // Trojan Manager capsule
    TrojanManagerCapsule,
    TrojanManagerSnapshot,
    // Types
    TrojanManagerState,
    TrojanRingHeader,
    TrojanKernelArgs,
};

#[cfg(all(feature = "kgpu-driver-nvidia", target_os = "linux"))]
pub use cuda_ffi::{
    // CUDA type definitions
    CUdevice,
    CUdeviceptr,
    // Result/error types
    CUresult,
    // Device attributes
    CUdevice_attribute,
    // Flag modules
    ctx_flags,
    stream_flags,
    event_flags,
    host_alloc_flags,
    // Library loader
    CudaLibrary,
    // Safe wrapper module
    safe,
};

#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub use intel_driver::{
    // Context capsule
    I915ContextCapsule,
    I915ContextSnapshot,
    // Engine types
    I915EngineClass,
    // Tiling modes
    I915TilingMode,
    // Parameters
    I915Param,
    I915ContextParam,
    // Reset reason
    ResetReason,
    // ioctl structures
    I915Getparam,
    I915GemCreate,
    I915GemMmapOffset,
    I915GemSetTiling,
    I915GemGetTiling,
    I915GemExecObject2,
    I915GemExecbuffer2,
    I915GemContextCreate,
    I915GemContextDestroy,
    I915GemContextParam,
    I915GemBusy,
    I915GemWait,
    I915GemClose,
    I915QueryItem,
    I915Query,
    // Engine info
    EngineInfo,
    EngineInstance,
    // Driver interface
    I915Driver,
    // Flag modules
    context_flags,
    mmap_offset_flags,
    exec_object_flags,
    exec_flags,
    context_create_flags,
    query_id,
};

// Intel Xe2 backend (Meteor Lake+) - T1 Atomic capsule with CPU fallback
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub use intel_xe2_backend::{
    IntelXe2BackendCapsule,
    IntelXe2Error,
};

// Intel Xe2 DRM/GEM capsules
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub use xe_drm_capsule::{
    XeDrmCapsule,
    XeDrmError,
};

#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub use xe_gem_capsule::{
    XeGemCapsule,
    XeGemError,
    GEM_FLAG_DEVICE_LOCAL,
    GEM_FLAG_HOST_VISIBLE,
    GEM_FLAG_HOST_COHERENT,
};

#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub use xe_exec_capsule::{
    XeExecCapsule,
    XeExecError,
    EXEC_PRIORITY_NORMAL,
    EXEC_PRIORITY_HIGH,
    EXEC_PRIORITY_REALTIME,
};

#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub use xe_ring_capsule::{
    XeRingCapsule,
    XeRingError,
    RING_SIZE_4K,
    RING_SIZE_16K,
    RING_SIZE_64K,
    DEFAULT_RING_SIZE,
};

#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub use xe_compute_capsule::{
    XeComputeCapsule,
    XeComputeError,
    ComputeStats,
    XE2_MAX_WORKGROUP_SIZE,
    XE2_MAX_EUS,
    XE2_EU_THREADS,
    XE2_MAX_SHARED_MEMORY,
};

#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub use xe_shader_capsule::{
    XeShaderCapsule,
    XeShaderError,
    SHADER_TYPE_COMPUTE,
    SHADER_TYPE_VERTEX,
    SHADER_TYPE_FRAGMENT,
    SHADER_TYPE_GEOMETRY,
};

// Phase 5 exports
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub use xe_display_capsule::{
    XeDisplayCapsule,
    XeDisplayError,
    ConnectorInfo,
    CONNECTOR_TYPE_HDMI,
    CONNECTOR_TYPE_DP,
    CONNECTOR_TYPE_EDP,
    CONNECTOR_TYPE_VGA,
    XE2_MAX_DISPLAYS,
    XE2_MAX_CRTCS,
    XE2_MAX_REFRESH_HZ,
};

// Phase 5 exports - PLANNED (uncomment when xe_framebuffer_capsule.rs is implemented)
// #[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
// pub use xe_framebuffer_capsule::{
//     XeFramebufferCapsule,
//     XeFramebufferError,
//     FramebufferFormat,
//     FRAMEBUFFER_MAX_WIDTH,
//     FRAMEBUFFER_MAX_HEIGHT,
// };

#[cfg(all(feature = "kgpu-driver-amd", target_os = "linux"))]
pub use amd_driver::{
    // Context capsule
    AmdgpuContextCapsule,
    AmdgpuContextSnapshot,
    // Enums
    AmdgpuHwIp,
    AmdgpuCtxOp,
    AmdgpuInfoId,
    AmdgpuContextState,
    // Bitflags
    AmdgpuDomain,
    AmdgpuBoFlags,
    // ioctl structures (In/Out pattern for bidirectional)
    AmdgpuGemCreateIn,
    AmdgpuGemCreateOut,
    AmdgpuGemMmapIn,
    AmdgpuGemMmapOut,
    AmdgpuCtxIn,
    AmdgpuCtxOut,
    AmdgpuCsIn,
    AmdgpuCsOut,
    AmdgpuCsChunk,
    AmdgpuCsIb,
    AmdgpuDevInfo,
    AmdgpuInfoFfi,
    // Driver interface
    AmdgpuDriver,
    // ioctl constants
    DRM_AMDGPU_GEM_CREATE,
    DRM_AMDGPU_GEM_MMAP,
    DRM_AMDGPU_CTX,
    DRM_AMDGPU_BO_LIST,
    DRM_AMDGPU_CS,
    DRM_AMDGPU_INFO,
    DRM_AMDGPU_GEM_WAIT_IDLE,
    DRM_AMDGPU_GEM_VA,
    DRM_AMDGPU_WAIT_CS,
    DRM_AMDGPU_GEM_METADATA,
    DRM_AMDGPU_GEM_OP,
    DRM_AMDGPU_VM,
    DRM_AMDGPU_FENCE_TO_HANDLE,
    DRM_AMDGPU_SCHED,
};

// SPIR-V parser exports
pub use spirv_parser::{
    // Constants
    SPIRV_MAGIC,
    SPIRV_MAGIC_LE,
    SPIRV_MAGIC_BE,
    SPIRV_HEADER_SIZE_BYTES,
    SPIRV_HEADER_SIZE_WORDS,
    MIN_SPIRV_SIZE_BYTES,
    MAX_SPIRV_VERSION,
    MIN_INSTRUCTION_WORDS,
    // Header parsing
    SpirVHeader,
    // Opcodes
    SpirVOp,
    // Instruction iteration
    SpirVInstruction,
    SpirVInstructionIterator,
    // IR types
    ShaderIr,
    ShaderIrType,
    ShaderIrOpKind,
    ShaderIrInstruction,
    // Conversion
    SpirVToIrConverter,
    // Capsule
    SpirVParserCapsule,
    SpirVParserSnapshot,
    ParserState,
    // Errors
    SpirVParseError,
};

// Shader IR module exports (optimizing IR)
pub use shader_ir::{
    // Types
    IrType,
    IrOpcode,
    SsaValue,
    IrInstruction,
    IrConstant,
    InstructionFlags,
    // Module state
    ModuleState,
    // Capsule
    ShaderIrModuleCapsule,
    ShaderIrModuleSnapshot,
    // Optimization
    OptimizationResult,
    dead_code_elimination,
    constant_folding,
    strength_reduction,
    run_all_passes,
};

// Backend codegen exports
pub use backend_codegen::{
    // Constants
    MAX_CODE_SIZE,
    DEFAULT_INTEL_REGISTERS,
    DEFAULT_AMD_VGPR,
    DEFAULT_AMD_SGPR,
    DEFAULT_NVIDIA_REGISTERS,
    // State
    CodegenState,
    // Output
    GeneratedCode,
    // Trait
    CodegenBackend,
    // Capsule
    CodegenCapsule,
    CodegenSnapshot,
    // Intel Gen backend
    GenOpcode,
    IntelGenBackend,
    // AMD GCN backend
    VopOpcode,
    AmdGcnBackend,
    // NVIDIA PTX backend
    NvidiaPtxBackend,
    // Factory
    create_backend,
    default_version,
};

// GART allocator exports
pub use gart_allocator::{
    // Capsule
    GartAllocatorCapsule,
    // Error types
    GartError,
    GartResult,
    // Vendor types
    GartVendor,
    MemoryDomain,
    IntelGttConfig,
    AmdGartConfig,
    NvidiaApertureConfig,
    VendorConfig,
    // Allocation hint
    AllocHint,
};

// GPU page table exports
pub use gpu_page_table::{
    // Constants
    MAX_PAGE_TABLE_LEVELS,
    PAGE_SIZE_4KB,
    PAGE_SIZE_2MB,
    PAGE_SIZE_1GB,
    VA_BITS_48,
    VA_BITS_32,
    PA_BITS_40,
    // Capsule
    GpuPageTableCapsule,
    // Types
    PageTableConfig,
    PageFlags,
    PhysicalMapping,
    PageTableStats,
    PageTableError,
    PageTableResult,
};

// Semaphore pool exports
pub use semaphore_pool::{
    // Capsule
    SemaphorePoolCapsule,
    SemaphorePoolSnapshot,
    // Types
    SemaphoreType,
    SemaphoreHandle,
    PoolStats,
    PoolError,
    // Constants
    MAX_SEMAPHORES,
};

// Timeline fence exports
pub use timeline_fence::{
    // Capsule
    TimelineFenceCapsule,
    // Types
    TimelineFenceSnapshot,
    FenceState,
    FenceError,
    FenceResult,
    // Constants
    MAX_TIMELINE_VALUE,
    INVALID_SYNC_FD,
};

pub use command_timing::{
    // Capsule
    CommandTimingCapsule,
    // System (with external storage)
    CommandTimingSystem,
    // Types
    CommandType,
    QueryId,
    TimingError,
    TimingStats,
    // Constants
    MAX_QUERIES,
    HISTOGRAM_BUCKETS,
    DEFAULT_GPU_CLOCK_HZ,
};

// GPU counters exports
pub use gpu_counters::{
    // Capsule
    GpuCountersCapsule,
    // Snapshot
    CounterSnapshot,
    // Enums
    CounterCategory,
    CounterId,
    SamplingMode,
    CounterState,
    // Vendor mapping
    VendorCounterMapping,
    get_vendor_mapping,
    // Error types
    CounterError,
    CounterResult,
    // Constants
    MAX_COUNTERS,
    HW_COUNTER_LIMIT,
    SAMPLE_BUFFER_SIZE,
    COUNTER_OVERFLOW_THRESHOLD,
    DEFAULT_SAMPLE_INTERVAL_NS,
};

// Bandwidth profiler exports
pub use bandwidth_profiler::{
    // Capsule
    BandwidthProfilerCapsule,
    // Snapshot
    BandwidthSnapshot,
    // Enums - renamed to avoid conflict with gart_allocator::MemoryDomain
    BandwidthDomain,
};

// ============================================================================
// Phase 10: Capsule-OS Direct Platform Re-exports
// ============================================================================

// MMIO region exports
pub use mmio_region::{
    // Capsule
    MmioRegionCapsule,
    // Types
    MmioRegionType,
    MmioFlags,
    MmioError,
};

// Bare metal allocator exports
pub use bare_metal_allocator::{
    // Capsule
    BareMetalAllocatorCapsule,
    // Types
    MemoryPool,
    PoolType,
    PhysicalAddress,
    AllocationStats,
    AllocError,
};

// Register access exports
pub use register_access::{
    // Capsule
    RegisterAccessCapsule,
    // Types
    GpuVendor as RegisterVendor,
    AccessMode,
    ForcewakeDomain,
    // Vendor register modules
    intel,
    amd,
};

// ============================================================================
// Constants
// ============================================================================

/// KGPU-Driver version
pub const KGPU_DRIVER_VERSION: &str = "2.0.0";

/// Maximum supported GPUs per system
pub const MAX_GPUS: usize = 8;

/// Maximum command size in bytes
pub const MAX_COMMAND_SIZE: usize = 512;

/// NVIDIA Trojan kernel polling interval (nanoseconds)
pub const TROJAN_POLL_INTERVAL_NS: u64 = 100;

// ============================================================================
// Platform Detection
// ============================================================================

/// Detect which platform we're running on at compile time
#[inline]
pub const fn platform_name() -> &'static str {
    #[cfg(feature = "kgpu-driver-linux")]
    { "Linux (DRM)" }

    #[cfg(feature = "kgpu-driver-capsule-os")]
    { "Capsule-OS (Direct)" }

    #[cfg(not(any(feature = "kgpu-driver-linux", feature = "kgpu-driver-capsule-os")))]
    { "Unknown" }
}

/// Check if running on real hardware vs emulator/VM
#[cfg(all(feature = "std", target_arch = "x86_64"))]
pub fn is_real_hardware() -> bool {
    // Check CPUID for hypervisor presence
    // Hypervisor leaf 0x40000000 indicates VM
    //
    // Note: We must save/restore rbx since LLVM reserves it.
    // CPUID destroys eax, ebx, ecx, edx.
    //
    // #ASSUME_CPUID_SAFE: CPUID instruction is always available on x86_64
    // #VERIFY_CPUID_SAFE: x86_64 guarantees CPUID presence
    unsafe {
        let result: u32;
        let _ebx_out: u32;
        core::arch::asm!(
            // Save rbx since LLVM reserves it
            "push rbx",
            "cpuid",
            "mov {ebx_tmp:e}, ebx",
            "pop rbx",
            inout("eax") 0x40000000u32 => result,
            ebx_tmp = out(reg) _ebx_out,
            out("ecx") _,
            out("edx") _,
        );
        // If result < 0x40000000, no hypervisor present
        result < 0x40000000
    }
}

#[cfg(not(all(feature = "std", target_arch = "x86_64")))]
pub fn is_real_hardware() -> bool {
    true // Assume real hardware on non-x86_64
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(KGPU_DRIVER_VERSION, "2.0.0");
    }

    #[test]
    fn test_constants() {
        assert!(MAX_GPUS >= 1);
        assert!(MAX_COMMAND_SIZE >= 64);

        // DEFAULT_RING_SIZE is only available with Intel driver support
        #[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
        {
            assert!(DEFAULT_RING_SIZE >= 4096);
        }
    }

    #[test]
    fn test_platform_name() {
        let name = platform_name();
        assert!(!name.is_empty());
    }
}


// ============================================================================
// Phase 3: Memory Management Exports
// ============================================================================

pub use gtt_manager_capsule::{
    GttManagerCapsule,
    GttManagerError,
    GttManagerResult,
};

pub use page_table_capsule_phase3::{
    PageTableCapsule as Phase3PageTableCapsule,
    PageTableError as Phase3PageTableError,
};

pub use gem_object_capsule_phase3::{
    GemObjectCapsule,
    GemError,
};

pub use eviction_manager_capsule_phase3::{
    EvictionManagerCapsule,
    EvictionError,
};

// ============================================================================
// Phase 5: Power Management Exports (Intel Xe2)
// ============================================================================

pub use power_state_capsule::{
    PowerStateCapsule,
    PowerStateSnapshot,
    PowerState,
};

pub use frequency_manager_capsule::{
    FrequencyManagerCapsule,
    FrequencyManagerSnapshot,
    PState,
    Q16Frequency,
    Q16Voltage,
};

pub use thermal_monitor_capsule::{
    ThermalMonitorCapsule,
    ThermalMonitorSnapshot,
    ThermalState,
    Q16Temperature,
};
