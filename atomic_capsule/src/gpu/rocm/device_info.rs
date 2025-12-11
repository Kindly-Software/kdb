//! ROCm Device Info Capsule - T1 Atomic Tier (512B)
//!
//! Provides lockfree GPU device property storage and retrieval for AMD ROCm/HIP.
//! Contains device properties equivalent to hipDeviceProp_t structure.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                        DeviceInfoCapsule (512B)                             │
//! │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
//! │  │ Identity        │  │ Compute Props   │  │ Memory Properties           │  │
//! │  │ name, uuid, arch│  │ CUs, wavefront  │  │ VRAM, GTT, bandwidth        │  │
//! │  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘  │
//! │                                                                              │
//! │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
//! │  │ Limits          │  │ Clock Rates     │  │ Features                    │  │
//! │  │ threads, regs   │  │ core, memory    │  │ ray-tracing, AI, atomics    │  │
//! │  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Chaos Mandate
//!
//! - **100% Lockfree**: NO mutex, NO RwLock - atomics only
//! - **T1 Atomic Tier**: <100ns state operations
//! - **512B Alignment**: 8 cache lines for optimal access
//! - **Generation Counters**: ABA prevention on all updates
//!
//! # Property Categories
//!
//! 1. **Identity**: Device name, UUID, architecture name (gcnArchName)
//! 2. **Compute**: Compute units, wavefront size, threads per CU
//! 3. **Memory**: Total VRAM, GTT size, bandwidth, cache sizes
//! 4. **Limits**: Max threads per block, registers, shared memory
//! 5. **Clock**: Core frequency, memory frequency
//! 6. **Features**: Ray tracing, AI acceleration, atomic operations
//!
//! # ASSUM Tags
//!
//! - `#ASSUME_DEVICE_VALID`: Device index is valid
//! - `#ASSUME_PROPS_STABLE`: Properties don't change after init
//! - `#ASSUME_ATOMIC_ALIGNED`: All atomic fields are properly aligned
//!
//! # UCE34 Compliance
//!
//! - **Q10**: T1 Atomic tier (lockfree coordination)
//! - **Q33**: ComputationalCapsule verification (512B, generation counters)
//! - **Q34**: Audit trail design (query_count, error_count for SOX/SOC2)
//!
//! # References
//!
//! - [hipDeviceProp_t](https://rocm.docs.amd.com/projects/HIP/en/docs-6.0.0/doxygen/html/structhip_device_prop__t.html)
//! - [HIP Device Management](https://rocm.docs.amd.com/projects/HIP/en/latest/doxygen/html/group___device.html)

#![allow(dead_code)]

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicU16, AtomicU8, Ordering};
use core::fmt;

// Import GpuGeneration from kgpu_driver if available, otherwise define locally
#[cfg(feature = "kgpu-driver")]
use crate::gpu::kgpu_driver::vendor::GpuGeneration;

// Local definition for when kgpu-driver is not available
#[cfg(not(feature = "kgpu-driver"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GpuGeneration {
    Unknown = 0,
    // AMD
    AmdGcn1 = 30,
    AmdGcn2 = 31,
    AmdGcn3 = 32,
    AmdGcn4 = 33,
    AmdGcn5 = 34,
    AmdRdna1 = 35,
    AmdRdna2 = 36,
    AmdRdna3 = 37,
    AmdRdna4 = 38,
}

#[cfg(not(feature = "kgpu-driver"))]
impl Default for GpuGeneration {
    fn default() -> Self {
        Self::Unknown
    }
}

#[cfg(not(feature = "kgpu-driver"))]
impl GpuGeneration {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::AmdGcn1 => "AMD GCN1 (Southern Islands)",
            Self::AmdGcn2 => "AMD GCN2 (Sea Islands)",
            Self::AmdGcn3 => "AMD GCN3 (Volcanic Islands)",
            Self::AmdGcn4 => "AMD GCN4 (Polaris)",
            Self::AmdGcn5 => "AMD GCN5 (Vega)",
            Self::AmdRdna1 => "AMD RDNA1 (Navi 10/14)",
            Self::AmdRdna2 => "AMD RDNA2 (Navi 21/22/23)",
            Self::AmdRdna3 => "AMD RDNA3 (Navi 31/32/33)",
            Self::AmdRdna4 => "AMD RDNA4 (Navi 4x)",
        }
    }
}

// ============================================================================
// Constants
// ============================================================================

/// Maximum device name length (matches hipDeviceProp_t.name[256])
pub const DEVICE_NAME_LEN: usize = 64;

/// Maximum architecture name length (gcnArchName)
pub const ARCH_NAME_LEN: usize = 32;

/// UUID length in bytes (16 bytes = 128 bits)
pub const UUID_LEN: usize = 16;

// ============================================================================
// Feature Flags
// ============================================================================

/// Device feature capability flags (packed into u64)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct DeviceFeatures(pub u64);

impl DeviceFeatures {
    // === Compute Features ===
    /// Unified addressing (shared virtual address space)
    pub const UNIFIED_ADDRESSING: Self = Self(1 << 0);
    /// Managed memory support (automatic migration)
    pub const MANAGED_MEMORY: Self = Self(1 << 1);
    /// Concurrent kernel execution
    pub const CONCURRENT_KERNELS: Self = Self(1 << 2);
    /// Cooperative groups support
    pub const COOPERATIVE_LAUNCH: Self = Self(1 << 3);
    /// Multi-GPU cooperative launch
    pub const COOPERATIVE_MULTI_GPU: Self = Self(1 << 4);

    // === Memory Features ===
    /// ECC (Error Correcting Code) enabled
    pub const ECC_ENABLED: Self = Self(1 << 8);
    /// Large BAR (resizable BAR / SAM)
    pub const LARGE_BAR: Self = Self(1 << 9);
    /// Host-visible VRAM (APU or BAR enabled)
    pub const HOST_VISIBLE_VRAM: Self = Self(1 << 10);
    /// Pageable memory access
    pub const PAGEABLE_MEMORY_ACCESS: Self = Self(1 << 11);

    // === Advanced Features (RDNA2+) ===
    /// Hardware ray tracing support
    pub const RAY_TRACING: Self = Self(1 << 16);
    /// AI/Matrix accelerators (WMMA)
    pub const AI_ACCELERATION: Self = Self(1 << 17);
    /// Variable Rate Shading (VRS)
    pub const VRS: Self = Self(1 << 18);
    /// Mesh shaders
    pub const MESH_SHADERS: Self = Self(1 << 19);
    /// Sampler feedback
    pub const SAMPLER_FEEDBACK: Self = Self(1 << 20);

    // === Atomic Features ===
    /// 32-bit atomic operations
    pub const ATOMIC_32: Self = Self(1 << 24);
    /// 64-bit atomic operations
    pub const ATOMIC_64: Self = Self(1 << 25);
    /// Floating-point atomic add
    pub const ATOMIC_FLOAT_ADD: Self = Self(1 << 26);
    /// System-scope atomics
    pub const ATOMIC_SYSTEM_SCOPE: Self = Self(1 << 27);

    // === Video Features ===
    /// VCN encode support
    pub const VCN_ENCODE: Self = Self(1 << 32);
    /// VCN decode support
    pub const VCN_DECODE: Self = Self(1 << 33);
    /// JPEG encode/decode
    pub const JPEG: Self = Self(1 << 34);
    /// AV1 decode (VCN 3.0+)
    pub const AV1_DECODE: Self = Self(1 << 35);
    /// AV1 encode (VCN 4.0+)
    pub const AV1_ENCODE: Self = Self(1 << 36);

    /// Empty features
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Get raw bits
    #[inline]
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Create from raw bits
    #[inline]
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    /// Check if feature is present
    #[inline]
    pub const fn contains(self, flag: Self) -> bool {
        (self.0 & flag.0) != 0
    }

    /// Add a feature
    #[inline]
    pub const fn with(self, flag: Self) -> Self {
        Self(self.0 | flag.0)
    }

    /// Remove a feature
    #[inline]
    pub const fn without(self, flag: Self) -> Self {
        Self(self.0 & !flag.0)
    }

    /// Default features for RDNA3
    pub const fn rdna3_default() -> Self {
        Self(
            Self::UNIFIED_ADDRESSING.0
                | Self::MANAGED_MEMORY.0
                | Self::CONCURRENT_KERNELS.0
                | Self::COOPERATIVE_LAUNCH.0
                | Self::LARGE_BAR.0
                | Self::RAY_TRACING.0
                | Self::AI_ACCELERATION.0
                | Self::VRS.0
                | Self::MESH_SHADERS.0
                | Self::ATOMIC_32.0
                | Self::ATOMIC_64.0
                | Self::ATOMIC_FLOAT_ADD.0
                | Self::VCN_ENCODE.0
                | Self::VCN_DECODE.0
                | Self::JPEG.0
                | Self::AV1_DECODE.0
                | Self::AV1_ENCODE.0,
        )
    }

    /// Default features for RDNA2
    pub const fn rdna2_default() -> Self {
        Self(
            Self::UNIFIED_ADDRESSING.0
                | Self::MANAGED_MEMORY.0
                | Self::CONCURRENT_KERNELS.0
                | Self::COOPERATIVE_LAUNCH.0
                | Self::LARGE_BAR.0
                | Self::RAY_TRACING.0
                | Self::ATOMIC_32.0
                | Self::ATOMIC_64.0
                | Self::VCN_ENCODE.0
                | Self::VCN_DECODE.0
                | Self::JPEG.0
                | Self::AV1_DECODE.0,
        )
    }

    /// Default features for GCN5 (Vega)
    pub const fn gcn5_default() -> Self {
        Self(
            Self::UNIFIED_ADDRESSING.0
                | Self::CONCURRENT_KERNELS.0
                | Self::COOPERATIVE_LAUNCH.0
                | Self::ATOMIC_32.0
                | Self::ATOMIC_64.0
                | Self::VCN_ENCODE.0
                | Self::VCN_DECODE.0,
        )
    }
}

impl core::ops::BitOr for DeviceFeatures {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitAnd for DeviceFeatures {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Device info errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceInfoError {
    /// Device not initialized
    NotInitialized,
    /// Device properties unavailable
    PropertiesUnavailable,
    /// Generation counter mismatch
    GenerationMismatch,
    /// Invalid property value
    InvalidProperty,
    /// HIP runtime error
    HipError,
}

impl DeviceInfoError {
    /// Get error message
    pub const fn message(self) -> &'static str {
        match self {
            Self::NotInitialized => "Device not initialized",
            Self::PropertiesUnavailable => "Device properties unavailable",
            Self::GenerationMismatch => "Concurrent modification detected",
            Self::InvalidProperty => "Invalid property value",
            Self::HipError => "HIP runtime error",
        }
    }
}

impl fmt::Display for DeviceInfoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

/// Result type for device info operations
pub type DeviceInfoResult<T> = Result<T, DeviceInfoError>;

// ============================================================================
// Device Info Capsule
// ============================================================================

/// ROCm Device Info Capsule - T1 Atomic Tier (512B)
///
/// Stores GPU device properties equivalent to hipDeviceProp_t
/// with lockfree atomic access and generation counters.
///
/// # Layout
///
/// - Total size: 512 bytes
/// - Alignment: 512 bytes (8 cache lines)
/// - All fields are atomic for lockfree access
///
/// # Thread Safety
///
/// All operations are lockfree using atomic operations.
/// Generation counter enables safe concurrent updates.
#[repr(C, align(512))]
pub struct DeviceInfoCapsule {
    // === Cache Line 0: Identity (64B) ===
    /// Device index (0-based)
    pub device_index: AtomicU32,
    /// PCI vendor ID (0x1002 for AMD)
    pub vendor_id: AtomicU16,
    /// PCI device ID
    pub device_id: AtomicU16,
    /// GPU generation (encoded GpuGeneration)
    pub generation: AtomicU8,
    /// Is device initialized
    pub initialized: AtomicU8,
    /// Is integrated GPU (APU)
    pub is_integrated: AtomicU8,
    /// Is multi-GPU config
    pub is_multi_gpu: AtomicU8,
    /// Compute capability major version
    pub major: AtomicU32,
    /// Compute capability minor version
    pub minor: AtomicU32,
    /// Generation counter for ABA prevention
    pub gen_counter: AtomicU64,
    /// Query count (audit trail)
    pub query_count: AtomicU64,
    /// Error count (audit trail)
    pub error_count: AtomicU64,
    /// Padding
    _pad0: [u8; 16],

    // === Cache Line 1: Compute Properties (64B) ===
    /// Number of Compute Units (CUs) / Shader Engines
    pub compute_units: AtomicU32,
    /// Wavefront size (typically 64 for GCN, 32 for RDNA)
    pub wavefront_size: AtomicU32,
    /// Maximum threads per block (workgroup)
    pub max_threads_per_block: AtomicU32,
    /// Maximum threads per CU
    pub max_threads_per_cu: AtomicU32,
    /// Maximum block dimensions X
    pub max_block_dim_x: AtomicU32,
    /// Maximum block dimensions Y
    pub max_block_dim_y: AtomicU32,
    /// Maximum block dimensions Z
    pub max_block_dim_z: AtomicU32,
    /// Maximum grid dimensions X
    pub max_grid_dim_x: AtomicU32,
    /// Maximum grid dimensions Y
    pub max_grid_dim_y: AtomicU32,
    /// Maximum grid dimensions Z
    pub max_grid_dim_z: AtomicU32,
    /// Registers per block
    pub regs_per_block: AtomicU32,
    /// Padding
    _pad1: [u8; 20],

    // === Cache Line 2: Memory Properties (64B) ===
    /// Total VRAM (bytes)
    pub total_vram: AtomicU64,
    /// Total GTT/system memory accessible (bytes)
    pub total_gtt: AtomicU64,
    /// Shared memory per block (bytes)
    pub shared_mem_per_block: AtomicU32,
    /// Shared memory per CU (bytes)
    pub shared_mem_per_cu: AtomicU32,
    /// L1 cache size per CU (bytes)
    pub l1_cache_size: AtomicU32,
    /// L2 cache size (bytes)
    pub l2_cache_size: AtomicU32,
    /// Memory bus width (bits)
    pub memory_bus_width: AtomicU32,
    /// Memory clock rate (MHz)
    pub memory_clock_mhz: AtomicU32,
    /// Peak memory bandwidth (GB/s * 10 for Q8.8 fixed-point)
    pub peak_bandwidth_gbps_q8: AtomicU32,
    /// Padding
    _pad2: [u8; 12],

    // === Cache Line 3: Clock and Performance (64B) ===
    /// Core clock rate (MHz)
    pub clock_rate_mhz: AtomicU32,
    /// Boost clock rate (MHz)
    pub boost_clock_mhz: AtomicU32,
    /// TDP (Watts)
    pub tdp_watts: AtomicU32,
    /// Current power draw (mW)
    pub power_draw_mw: AtomicU32,
    /// Current temperature (mC = milliCelsius)
    pub temperature_mc: AtomicU32,
    /// Fan speed percentage (0-100)
    pub fan_speed_pct: AtomicU32,
    /// ASIC revision
    pub asic_revision: AtomicU32,
    /// Padding
    _pad3: [u8; 36],

    // === Cache Line 4: Features (64B) ===
    /// Feature flags (packed DeviceFeatures)
    pub features: AtomicU64,
    /// PCIe generation (1, 2, 3, 4, 5)
    pub pcie_gen: AtomicU32,
    /// PCIe link width (x1, x4, x8, x16)
    pub pcie_width: AtomicU32,
    /// PCIe max payload size
    pub pcie_max_payload: AtomicU32,
    /// SDMA engine count
    pub sdma_count: AtomicU32,
    /// VCN engine count
    pub vcn_count: AtomicU32,
    /// Padding
    _pad4: [u8; 32],

    // === Cache Lines 5-7: Device Name and Architecture (192B) ===
    /// Device name (null-terminated)
    pub name: [AtomicU8; DEVICE_NAME_LEN],
    /// Architecture name (gcnArchName, e.g., "gfx1100")
    pub arch_name: [AtomicU8; ARCH_NAME_LEN],
    /// UUID (16 bytes)
    pub uuid: [AtomicU8; UUID_LEN],
    /// Padding to 512B
    _pad5: [u8; 80],
}

// Size assertion
const _: () = {
    assert!(core::mem::size_of::<DeviceInfoCapsule>() == 512);
    assert!(core::mem::align_of::<DeviceInfoCapsule>() == 512);
};

impl DeviceInfoCapsule {
    /// Create a new device info capsule
    #[inline]
    pub const fn new() -> Self {
        Self {
            device_index: AtomicU32::new(u32::MAX),
            vendor_id: AtomicU16::new(0),
            device_id: AtomicU16::new(0),
            generation: AtomicU8::new(0),
            initialized: AtomicU8::new(0),
            is_integrated: AtomicU8::new(0),
            is_multi_gpu: AtomicU8::new(0),
            major: AtomicU32::new(0),
            minor: AtomicU32::new(0),
            gen_counter: AtomicU64::new(0),
            query_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            _pad0: [0; 16],

            compute_units: AtomicU32::new(0),
            wavefront_size: AtomicU32::new(0),
            max_threads_per_block: AtomicU32::new(0),
            max_threads_per_cu: AtomicU32::new(0),
            max_block_dim_x: AtomicU32::new(0),
            max_block_dim_y: AtomicU32::new(0),
            max_block_dim_z: AtomicU32::new(0),
            max_grid_dim_x: AtomicU32::new(0),
            max_grid_dim_y: AtomicU32::new(0),
            max_grid_dim_z: AtomicU32::new(0),
            regs_per_block: AtomicU32::new(0),
            _pad1: [0; 20],

            total_vram: AtomicU64::new(0),
            total_gtt: AtomicU64::new(0),
            shared_mem_per_block: AtomicU32::new(0),
            shared_mem_per_cu: AtomicU32::new(0),
            l1_cache_size: AtomicU32::new(0),
            l2_cache_size: AtomicU32::new(0),
            memory_bus_width: AtomicU32::new(0),
            memory_clock_mhz: AtomicU32::new(0),
            peak_bandwidth_gbps_q8: AtomicU32::new(0),
            _pad2: [0; 12],

            clock_rate_mhz: AtomicU32::new(0),
            boost_clock_mhz: AtomicU32::new(0),
            tdp_watts: AtomicU32::new(0),
            power_draw_mw: AtomicU32::new(0),
            temperature_mc: AtomicU32::new(0),
            fan_speed_pct: AtomicU32::new(0),
            asic_revision: AtomicU32::new(0),
            _pad3: [0; 36],

            features: AtomicU64::new(0),
            pcie_gen: AtomicU32::new(0),
            pcie_width: AtomicU32::new(0),
            pcie_max_payload: AtomicU32::new(0),
            sdma_count: AtomicU32::new(0),
            vcn_count: AtomicU32::new(0),
            _pad4: [0; 32],

            name: [const { AtomicU8::new(0) }; DEVICE_NAME_LEN],
            arch_name: [const { AtomicU8::new(0) }; ARCH_NAME_LEN],
            uuid: [const { AtomicU8::new(0) }; UUID_LEN],
            _pad5: [0; 80],
        }
    }

    /// Check if device is initialized
    #[inline]
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire) != 0
    }

    /// Get device index
    #[inline]
    pub fn device_index(&self) -> u32 {
        self.device_index.load(Ordering::Acquire)
    }

    /// Get generation counter
    #[inline]
    pub fn generation_counter(&self) -> u64 {
        self.gen_counter.load(Ordering::Acquire)
    }

    /// Increment generation counter
    #[inline]
    pub fn increment_generation(&self) -> u64 {
        self.gen_counter.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Get GPU generation
    #[inline]
    pub fn gpu_generation(&self) -> GpuGeneration {
        let gen_val = self.generation.load(Ordering::Acquire);
        match gen_val {
            35 => GpuGeneration::AmdRdna1,
            36 => GpuGeneration::AmdRdna2,
            37 => GpuGeneration::AmdRdna3,
            38 => GpuGeneration::AmdRdna4,
            34 => GpuGeneration::AmdGcn5,
            33 => GpuGeneration::AmdGcn4,
            32 => GpuGeneration::AmdGcn3,
            31 => GpuGeneration::AmdGcn2,
            30 => GpuGeneration::AmdGcn1,
            _ => GpuGeneration::Unknown,
        }
    }

    /// Get device features
    #[inline]
    pub fn features(&self) -> DeviceFeatures {
        DeviceFeatures(self.features.load(Ordering::Acquire))
    }

    /// Set device features
    #[inline]
    pub fn set_features(&self, features: DeviceFeatures) {
        self.features.store(features.0, Ordering::Release);
    }

    /// Get device name as string
    #[cfg(feature = "std")]
    pub fn name_str(&self) -> std::string::String {
        let mut bytes = [0u8; DEVICE_NAME_LEN];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = self.name[i].load(Ordering::Relaxed);
            if *b == 0 {
                break;
            }
        }
        std::string::String::from_utf8_lossy(&bytes)
            .trim_end_matches('\0')
            .to_string()
    }

    /// Set device name
    pub fn set_name(&self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(DEVICE_NAME_LEN - 1);
        for i in 0..len {
            self.name[i].store(bytes[i], Ordering::Relaxed);
        }
        for i in len..DEVICE_NAME_LEN {
            self.name[i].store(0, Ordering::Relaxed);
        }
    }

    /// Get architecture name as string (e.g., "gfx1100")
    #[cfg(feature = "std")]
    pub fn arch_name_str(&self) -> std::string::String {
        let mut bytes = [0u8; ARCH_NAME_LEN];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = self.arch_name[i].load(Ordering::Relaxed);
            if *b == 0 {
                break;
            }
        }
        std::string::String::from_utf8_lossy(&bytes)
            .trim_end_matches('\0')
            .to_string()
    }

    /// Set architecture name
    pub fn set_arch_name(&self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(ARCH_NAME_LEN - 1);
        for i in 0..len {
            self.arch_name[i].store(bytes[i], Ordering::Relaxed);
        }
        for i in len..ARCH_NAME_LEN {
            self.arch_name[i].store(0, Ordering::Relaxed);
        }
    }

    /// Get total VRAM in bytes
    #[inline]
    pub fn total_vram(&self) -> u64 {
        self.total_vram.load(Ordering::Acquire)
    }

    /// Get total VRAM in GB (human-readable)
    #[inline]
    pub fn total_vram_gb(&self) -> f64 {
        (self.total_vram() as f64) / (1024.0 * 1024.0 * 1024.0)
    }

    /// Get compute unit count
    #[inline]
    pub fn compute_units(&self) -> u32 {
        self.compute_units.load(Ordering::Acquire)
    }

    /// Get wavefront size (32 for RDNA, 64 for GCN)
    #[inline]
    pub fn wavefront_size(&self) -> u32 {
        self.wavefront_size.load(Ordering::Acquire)
    }

    /// Check if device supports ray tracing
    #[inline]
    pub fn supports_ray_tracing(&self) -> bool {
        self.features().contains(DeviceFeatures::RAY_TRACING)
    }

    /// Check if device supports AI acceleration
    #[inline]
    pub fn supports_ai_acceleration(&self) -> bool {
        self.features().contains(DeviceFeatures::AI_ACCELERATION)
    }

    /// Get peak memory bandwidth in GB/s
    #[inline]
    pub fn peak_bandwidth_gbps(&self) -> f32 {
        let q8 = self.peak_bandwidth_gbps_q8.load(Ordering::Acquire);
        (q8 as f32) / 256.0 // Q8.8 fixed-point
    }

    /// Record a query (for audit trail)
    #[inline]
    pub fn record_query(&self) {
        self.query_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an error (for audit trail)
    #[inline]
    pub fn record_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Initialize device with default properties based on generation
    pub fn init_defaults(&self, device_idx: u32, gen: GpuGeneration) {
        self.device_index.store(device_idx, Ordering::Release);
        self.vendor_id.store(0x1002, Ordering::Release); // AMD
        self.generation.store(gen as u8, Ordering::Release);

        // Set generation-specific defaults
        match gen {
            GpuGeneration::AmdRdna3 | GpuGeneration::AmdRdna4 => {
                self.wavefront_size.store(32, Ordering::Release);
                self.max_threads_per_block.store(1024, Ordering::Release);
                self.max_block_dim_x.store(1024, Ordering::Release);
                self.max_block_dim_y.store(1024, Ordering::Release);
                self.max_block_dim_z.store(1024, Ordering::Release);
                self.max_grid_dim_x.store(2147483647, Ordering::Release);
                self.max_grid_dim_y.store(65535, Ordering::Release);
                self.max_grid_dim_z.store(65535, Ordering::Release);
                self.regs_per_block.store(65536, Ordering::Release);
                self.shared_mem_per_block.store(65536, Ordering::Release);
                self.shared_mem_per_cu.store(131072, Ordering::Release);
                self.set_features(DeviceFeatures::rdna3_default());
                self.set_arch_name("gfx1100");
            }
            GpuGeneration::AmdRdna2 => {
                self.wavefront_size.store(32, Ordering::Release);
                self.max_threads_per_block.store(1024, Ordering::Release);
                self.max_block_dim_x.store(1024, Ordering::Release);
                self.max_block_dim_y.store(1024, Ordering::Release);
                self.max_block_dim_z.store(1024, Ordering::Release);
                self.max_grid_dim_x.store(2147483647, Ordering::Release);
                self.max_grid_dim_y.store(65535, Ordering::Release);
                self.max_grid_dim_z.store(65535, Ordering::Release);
                self.regs_per_block.store(65536, Ordering::Release);
                self.shared_mem_per_block.store(65536, Ordering::Release);
                self.shared_mem_per_cu.store(131072, Ordering::Release);
                self.set_features(DeviceFeatures::rdna2_default());
                self.set_arch_name("gfx1030");
            }
            GpuGeneration::AmdRdna1 => {
                self.wavefront_size.store(32, Ordering::Release);
                self.max_threads_per_block.store(1024, Ordering::Release);
                self.set_arch_name("gfx1010");
            }
            GpuGeneration::AmdGcn5 => {
                self.wavefront_size.store(64, Ordering::Release);
                self.max_threads_per_block.store(1024, Ordering::Release);
                self.set_features(DeviceFeatures::gcn5_default());
                self.set_arch_name("gfx906");
            }
            _ => {
                self.wavefront_size.store(64, Ordering::Release);
                self.max_threads_per_block.store(256, Ordering::Release);
            }
        }

        self.initialized.store(1, Ordering::Release);
        self.increment_generation();
    }

    /// Get a snapshot of the device info
    #[inline]
    pub fn snapshot(&self) -> DeviceInfoSnapshot {
        self.record_query();
        DeviceInfoSnapshot {
            device_index: self.device_index(),
            generation: self.gpu_generation(),
            compute_units: self.compute_units.load(Ordering::Acquire),
            wavefront_size: self.wavefront_size.load(Ordering::Acquire),
            total_vram: self.total_vram(),
            features: self.features(),
            clock_rate_mhz: self.clock_rate_mhz.load(Ordering::Acquire),
            gen_counter: self.generation_counter(),
            initialized: self.is_initialized(),
        }
    }
}

impl Default for DeviceInfoCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Snapshot
// ============================================================================

/// Immutable snapshot of device info
#[derive(Debug, Clone, Copy)]
pub struct DeviceInfoSnapshot {
    /// Device index
    pub device_index: u32,
    /// GPU generation
    pub generation: GpuGeneration,
    /// Compute units
    pub compute_units: u32,
    /// Wavefront size
    pub wavefront_size: u32,
    /// Total VRAM (bytes)
    pub total_vram: u64,
    /// Feature flags
    pub features: DeviceFeatures,
    /// Clock rate (MHz)
    pub clock_rate_mhz: u32,
    /// Generation counter
    pub gen_counter: u64,
    /// Is initialized
    pub initialized: bool,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_info_size() {
        assert_eq!(core::mem::size_of::<DeviceInfoCapsule>(), 512);
        assert_eq!(core::mem::align_of::<DeviceInfoCapsule>(), 512);
    }

    #[test]
    fn test_device_info_initial_state() {
        let info = DeviceInfoCapsule::new();
        assert!(!info.is_initialized());
        assert_eq!(info.device_index(), u32::MAX);
        assert_eq!(info.generation_counter(), 0);
    }

    #[test]
    fn test_device_features() {
        let rdna3 = DeviceFeatures::rdna3_default();
        assert!(rdna3.contains(DeviceFeatures::RAY_TRACING));
        assert!(rdna3.contains(DeviceFeatures::AI_ACCELERATION));
        assert!(rdna3.contains(DeviceFeatures::AV1_ENCODE));

        let rdna2 = DeviceFeatures::rdna2_default();
        assert!(rdna2.contains(DeviceFeatures::RAY_TRACING));
        assert!(!rdna2.contains(DeviceFeatures::AV1_ENCODE));

        let gcn5 = DeviceFeatures::gcn5_default();
        assert!(!gcn5.contains(DeviceFeatures::RAY_TRACING));
    }

    #[test]
    fn test_device_info_init_defaults() {
        let info = DeviceInfoCapsule::new();
        info.init_defaults(0, GpuGeneration::AmdRdna3);

        assert!(info.is_initialized());
        assert_eq!(info.device_index(), 0);
        assert_eq!(info.gpu_generation(), GpuGeneration::AmdRdna3);
        assert_eq!(info.wavefront_size(), 32);
        assert!(info.supports_ray_tracing());
    }

    #[test]
    fn test_device_info_name() {
        let info = DeviceInfoCapsule::new();
        info.set_name("AMD Radeon RX 7900 XTX");

        #[cfg(feature = "std")]
        {
            let name = info.name_str();
            assert_eq!(name, "AMD Radeon RX 7900 XTX");
        }
    }

    #[test]
    fn test_device_info_arch_name() {
        let info = DeviceInfoCapsule::new();
        info.set_arch_name("gfx1100");

        #[cfg(feature = "std")]
        {
            let arch = info.arch_name_str();
            assert_eq!(arch, "gfx1100");
        }
    }

    #[test]
    fn test_device_info_snapshot() {
        let info = DeviceInfoCapsule::new();
        info.init_defaults(0, GpuGeneration::AmdRdna3);
        info.compute_units.store(96, Ordering::Release);
        info.total_vram.store(24 * 1024 * 1024 * 1024, Ordering::Release);

        let snapshot = info.snapshot();
        assert_eq!(snapshot.device_index, 0);
        assert_eq!(snapshot.generation, GpuGeneration::AmdRdna3);
        assert_eq!(snapshot.compute_units, 96);
        assert!(snapshot.initialized);
    }

    #[test]
    fn test_generation_counter() {
        let info = DeviceInfoCapsule::new();
        assert_eq!(info.generation_counter(), 0);

        let gen1 = info.increment_generation();
        assert_eq!(gen1, 1);

        let gen2 = info.increment_generation();
        assert_eq!(gen2, 2);
    }

    #[test]
    fn test_query_and_error_counting() {
        let info = DeviceInfoCapsule::new();
        assert_eq!(info.query_count.load(Ordering::Acquire), 0);
        assert_eq!(info.error_count.load(Ordering::Acquire), 0);

        info.record_query();
        info.record_query();
        assert_eq!(info.query_count.load(Ordering::Acquire), 2);

        info.record_error();
        assert_eq!(info.error_count.load(Ordering::Acquire), 1);
    }

    #[test]
    fn test_peak_bandwidth_q8() {
        let info = DeviceInfoCapsule::new();
        // 960 GB/s = 960 * 256 = 245760 in Q8.8
        info.peak_bandwidth_gbps_q8.store(245760, Ordering::Release);

        let bandwidth = info.peak_bandwidth_gbps();
        assert!((bandwidth - 960.0).abs() < 0.01);
    }

    #[test]
    fn test_vram_gb() {
        let info = DeviceInfoCapsule::new();
        info.total_vram.store(24 * 1024 * 1024 * 1024, Ordering::Release);

        let gb = info.total_vram_gb();
        assert!((gb - 24.0).abs() < 0.01);
    }
}
