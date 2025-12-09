//! KGPU HAL Types: Descriptors, Enums, and Supporting Types
//!
//! This module provides all the data types used by HAL traits:
//!
//! - **Enums**: GPU state enumerations (DeviceType, BackendType, etc.)
//! - **Bitflags**: Feature and usage flags (Features, BufferUsages, etc.)
//! - **Descriptors**: Configuration structures for resource creation
//! - **Info Types**: Read-only information about GPU resources
//!
//! # Design Philosophy
//!
//! - All types are `Copy` where possible for zero-overhead passing
//! - No heap allocations in core types
//! - Cache-aligned where performance-critical
//! - ASSUM-documented safety assumptions
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_TYPES_COPY`: Most types are Copy for zero-overhead
//! - `#ASSUME_TYPES_SEND_SYNC`: All types are Send + Sync

use core::ops::Range;

// ============================================================================
// Backend and Device Types
// ============================================================================

/// Supported GPU backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BackendType {
    /// Vulkan backend (cross-platform, Linux/Windows/Android).
    Vulkan = 0,

    /// Metal backend (Apple platforms: macOS/iOS).
    Metal = 1,

    /// DirectX 12 backend (Windows).
    Dx12 = 2,

    /// WebGPU backend (browsers via wasm32).
    WebGpu = 3,

    /// Null/mock backend for testing.
    Null = 255,
}

impl BackendType {
    /// Returns the backend name as a static string.
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            BackendType::Vulkan => "Vulkan",
            BackendType::Metal => "Metal",
            BackendType::Dx12 => "DX12",
            BackendType::WebGpu => "WebGPU",
            BackendType::Null => "Null",
        }
    }

    /// Returns true if this backend is available on the current platform.
    #[inline]
    pub const fn is_platform_supported(self) -> bool {
        match self {
            BackendType::Vulkan => cfg!(any(target_os = "linux", target_os = "windows", target_os = "android")),
            BackendType::Metal => cfg!(any(target_os = "macos", target_os = "ios")),
            BackendType::Dx12 => cfg!(target_os = "windows"),
            BackendType::WebGpu => cfg!(target_arch = "wasm32"),
            BackendType::Null => true,
        }
    }
}

impl Default for BackendType {
    fn default() -> Self {
        // Platform-specific defaults
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        return BackendType::Metal;

        #[cfg(target_os = "windows")]
        return BackendType::Dx12;

        #[cfg(target_arch = "wasm32")]
        return BackendType::WebGpu;

        #[cfg(all(
            not(any(target_os = "macos", target_os = "ios")),
            not(target_os = "windows"),
            not(target_arch = "wasm32")
        ))]
        return BackendType::Vulkan;
    }
}

/// GPU device type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DeviceType {
    /// Discrete GPU (dedicated graphics card).
    DiscreteGpu = 0,

    /// Integrated GPU (CPU-integrated graphics).
    IntegratedGpu = 1,

    /// Virtual GPU (cloud/VM).
    VirtualGpu = 2,

    /// CPU software renderer.
    Cpu = 3,

    /// Unknown device type.
    Unknown = 255,
}

impl DeviceType {
    /// Returns true if this is a hardware GPU.
    #[inline]
    pub const fn is_hardware(self) -> bool {
        matches!(self, DeviceType::DiscreteGpu | DeviceType::IntegratedGpu)
    }

    /// Returns relative performance tier (higher = better).
    #[inline]
    pub const fn performance_tier(self) -> u8 {
        match self {
            DeviceType::DiscreteGpu => 4,
            DeviceType::IntegratedGpu => 3,
            DeviceType::VirtualGpu => 2,
            DeviceType::Cpu => 1,
            DeviceType::Unknown => 0,
        }
    }
}

impl Default for DeviceType {
    fn default() -> Self {
        DeviceType::Unknown
    }
}

/// Power preference for adapter selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum PowerPreference {
    /// No preference (system default).
    #[default]
    None = 0,

    /// Prefer low power consumption (integrated GPU).
    LowPower = 1,

    /// Prefer high performance (discrete GPU).
    HighPerformance = 2,
}

// ============================================================================
// Adapter Info
// ============================================================================

/// Information about a GPU adapter (physical device).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterInfo {
    /// Human-readable device name.
    pub name: [u8; 256],
    /// Length of the name string.
    pub name_len: usize,

    /// Vendor identifier (PCI vendor ID).
    pub vendor: u32,

    /// Device identifier (PCI device ID).
    pub device: u32,

    /// Device type classification.
    pub device_type: DeviceType,

    /// Backend providing this adapter.
    pub backend: BackendType,

    /// Driver version (backend-specific encoding).
    pub driver_version: u32,
}

impl AdapterInfo {
    /// Creates a new adapter info with the given name.
    pub fn new(name: &str, device_type: DeviceType, backend: BackendType) -> Self {
        let mut name_buf = [0u8; 256];
        let name_bytes = name.as_bytes();
        let len = name_bytes.len().min(255);
        name_buf[..len].copy_from_slice(&name_bytes[..len]);

        Self {
            name: name_buf,
            name_len: len,
            vendor: 0,
            device: 0,
            device_type,
            backend,
            driver_version: 0,
        }
    }

    /// Returns the device name as a string slice.
    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }
}

impl Default for AdapterInfo {
    fn default() -> Self {
        Self::new("Unknown", DeviceType::Unknown, BackendType::Null)
    }
}

/// Options for adapter selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AdapterOptions {
    /// Power preference for adapter selection.
    pub power_preference: PowerPreference,

    /// Force software rendering (CPU backend).
    pub force_fallback_adapter: bool,

    /// Preferred backend (None = any).
    pub preferred_backend: Option<BackendType>,
}

// ============================================================================
// Device Descriptor
// ============================================================================

/// Descriptor for device creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceDescriptor {
    /// Label for debugging.
    pub label: Option<&'static str>,

    /// Required features.
    pub required_features: Features,

    /// Required limits.
    pub required_limits: Limits,
}

impl Default for DeviceDescriptor {
    fn default() -> Self {
        Self {
            label: None,
            required_features: Features::empty(),
            required_limits: Limits::default(),
        }
    }
}

// ============================================================================
// Features
// ============================================================================

/// GPU feature flags.
///
/// # ASSUM Safety
///
/// - `#ASSUME_FEATURES_STABLE`: Feature bit assignments are stable across versions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Features(u64);

impl Features {
    // Basic features
    pub const DEPTH_CLIP_CONTROL: Self = Self(1 << 0);
    pub const DEPTH_CLAMPING: Self = Self(1 << 1);
    pub const TIMESTAMP_QUERY: Self = Self(1 << 2);
    pub const PIPELINE_STATISTICS_QUERY: Self = Self(1 << 3);

    // Texture compression
    pub const TEXTURE_COMPRESSION_BC: Self = Self(1 << 4);
    pub const TEXTURE_COMPRESSION_ETC2: Self = Self(1 << 5);
    pub const TEXTURE_COMPRESSION_ASTC: Self = Self(1 << 6);
    pub const TEXTURE_COMPRESSION_ASTC_HDR: Self = Self(1 << 7);

    // Advanced rendering
    pub const MULTI_DRAW_INDIRECT: Self = Self(1 << 8);
    pub const MULTI_DRAW_INDIRECT_COUNT: Self = Self(1 << 9);
    pub const PUSH_CONSTANTS: Self = Self(1 << 10);
    pub const ADDRESS_MODE_CLAMP_TO_BORDER: Self = Self(1 << 11);
    pub const ADDRESS_MODE_CLAMP_TO_ZERO: Self = Self(1 << 12);

    // Shader features
    pub const SHADER_F16: Self = Self(1 << 16);
    pub const SHADER_I64: Self = Self(1 << 17);
    pub const SHADER_FLOAT64: Self = Self(1 << 18);
    pub const SHADER_PRIMITIVE_INDEX: Self = Self(1 << 19);
    pub const SHADER_EARLY_DEPTH_TEST: Self = Self(1 << 20);

    // Storage features
    pub const STORAGE_BUFFER_ARRAY_DYNAMIC_INDEXING: Self = Self(1 << 24);
    pub const SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING: Self = Self(1 << 25);
    pub const UNIFORM_BUFFER_AND_STORAGE_TEXTURE_ARRAY_NON_UNIFORM_INDEXING: Self = Self(1 << 26);

    // Advanced
    pub const MAPPABLE_PRIMARY_BUFFERS: Self = Self(1 << 32);
    pub const PARTIALLY_BOUND_BINDING_ARRAY: Self = Self(1 << 33);
    pub const TEXTURE_BINDING_ARRAY: Self = Self(1 << 34);
    pub const STORAGE_RESOURCE_BINDING_ARRAY: Self = Self(1 << 35);

    // Ray tracing
    pub const RAY_TRACING: Self = Self(1 << 40);
    pub const RAY_QUERY: Self = Self(1 << 41);

    // Mesh shaders
    pub const MESH_SHADER: Self = Self(1 << 44);
    pub const TASK_SHADER: Self = Self(1 << 45);

    // VRS
    pub const VARIABLE_RATE_SHADING: Self = Self(1 << 48);

    /// Creates an empty feature set.
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Creates a feature set with all features enabled.
    #[inline]
    pub const fn all() -> Self {
        Self(u64::MAX)
    }

    /// Returns true if this feature set is empty.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns true if this feature set contains all the given features.
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns the union of two feature sets.
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns the intersection of two feature sets.
    #[inline]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Returns the raw bits.
    #[inline]
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Creates from raw bits.
    #[inline]
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }
}

impl core::ops::BitOr for Features {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl core::ops::BitAnd for Features {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        self.intersection(rhs)
    }
}

// ============================================================================
// Limits
// ============================================================================

/// GPU device limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    // Texture limits
    pub max_texture_dimension_1d: u32,
    pub max_texture_dimension_2d: u32,
    pub max_texture_dimension_3d: u32,
    pub max_texture_array_layers: u32,

    // Buffer limits
    pub max_buffer_size: u64,
    pub max_uniform_buffer_binding_size: u32,
    pub max_storage_buffer_binding_size: u32,
    pub max_vertex_buffers: u32,
    pub max_vertex_attributes: u32,
    pub max_vertex_buffer_array_stride: u32,

    // Binding limits
    pub max_bind_groups: u32,
    pub max_bindings_per_bind_group: u32,
    pub max_dynamic_uniform_buffers_per_pipeline_layout: u32,
    pub max_dynamic_storage_buffers_per_pipeline_layout: u32,
    pub max_sampled_textures_per_shader_stage: u32,
    pub max_samplers_per_shader_stage: u32,
    pub max_storage_buffers_per_shader_stage: u32,
    pub max_storage_textures_per_shader_stage: u32,
    pub max_uniform_buffers_per_shader_stage: u32,

    // Compute limits
    pub max_compute_workgroup_storage_size: u32,
    pub max_compute_invocations_per_workgroup: u32,
    pub max_compute_workgroup_size_x: u32,
    pub max_compute_workgroup_size_y: u32,
    pub max_compute_workgroup_size_z: u32,
    pub max_compute_workgroups_per_dimension: u32,

    // Push constants
    pub max_push_constant_size: u32,

    // Subgroup
    pub min_subgroup_size: u32,
    pub max_subgroup_size: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_texture_dimension_1d: 8192,
            max_texture_dimension_2d: 8192,
            max_texture_dimension_3d: 2048,
            max_texture_array_layers: 256,

            max_buffer_size: 256 * 1024 * 1024, // 256 MB
            max_uniform_buffer_binding_size: 64 * 1024,
            max_storage_buffer_binding_size: 128 * 1024 * 1024,
            max_vertex_buffers: 8,
            max_vertex_attributes: 16,
            max_vertex_buffer_array_stride: 2048,

            max_bind_groups: 4,
            max_bindings_per_bind_group: 1000,
            max_dynamic_uniform_buffers_per_pipeline_layout: 8,
            max_dynamic_storage_buffers_per_pipeline_layout: 4,
            max_sampled_textures_per_shader_stage: 16,
            max_samplers_per_shader_stage: 16,
            max_storage_buffers_per_shader_stage: 8,
            max_storage_textures_per_shader_stage: 4,
            max_uniform_buffers_per_shader_stage: 12,

            max_compute_workgroup_storage_size: 16384,
            max_compute_invocations_per_workgroup: 256,
            max_compute_workgroup_size_x: 256,
            max_compute_workgroup_size_y: 256,
            max_compute_workgroup_size_z: 64,
            max_compute_workgroups_per_dimension: 65535,

            max_push_constant_size: 128,

            min_subgroup_size: 4,
            max_subgroup_size: 128,
        }
    }
}

impl Limits {
    /// Returns the minimum limits (most compatible).
    pub const fn downlevel_defaults() -> Self {
        Self {
            max_texture_dimension_1d: 2048,
            max_texture_dimension_2d: 2048,
            max_texture_dimension_3d: 256,
            max_texture_array_layers: 256,

            max_buffer_size: 128 * 1024 * 1024,
            max_uniform_buffer_binding_size: 16 * 1024,
            max_storage_buffer_binding_size: 128 * 1024 * 1024,
            max_vertex_buffers: 8,
            max_vertex_attributes: 16,
            max_vertex_buffer_array_stride: 2048,

            max_bind_groups: 4,
            max_bindings_per_bind_group: 640,
            max_dynamic_uniform_buffers_per_pipeline_layout: 8,
            max_dynamic_storage_buffers_per_pipeline_layout: 4,
            max_sampled_textures_per_shader_stage: 16,
            max_samplers_per_shader_stage: 16,
            max_storage_buffers_per_shader_stage: 4,
            max_storage_textures_per_shader_stage: 4,
            max_uniform_buffers_per_shader_stage: 12,

            max_compute_workgroup_storage_size: 16352,
            max_compute_invocations_per_workgroup: 256,
            max_compute_workgroup_size_x: 256,
            max_compute_workgroup_size_y: 256,
            max_compute_workgroup_size_z: 64,
            max_compute_workgroups_per_dimension: 65535,

            max_push_constant_size: 0,

            min_subgroup_size: 4,
            max_subgroup_size: 128,
        }
    }
}

// ============================================================================
// Buffer Types
// ============================================================================

/// Buffer usage flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BufferUsages(u32);

impl BufferUsages {
    pub const MAP_READ: Self = Self(1 << 0);
    pub const MAP_WRITE: Self = Self(1 << 1);
    pub const COPY_SRC: Self = Self(1 << 2);
    pub const COPY_DST: Self = Self(1 << 3);
    pub const INDEX: Self = Self(1 << 4);
    pub const VERTEX: Self = Self(1 << 5);
    pub const UNIFORM: Self = Self(1 << 6);
    pub const STORAGE: Self = Self(1 << 7);
    pub const INDIRECT: Self = Self(1 << 8);
    pub const QUERY_RESOLVE: Self = Self(1 << 9);

    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl core::ops::BitOr for BufferUsages {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

/// Descriptor for buffer creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferDescriptor {
    /// Debug label.
    pub label: Option<&'static str>,

    /// Buffer size in bytes.
    pub size: u64,

    /// Buffer usage flags.
    pub usage: BufferUsages,

    /// Map at creation.
    pub mapped_at_creation: bool,
}

impl Default for BufferDescriptor {
    fn default() -> Self {
        Self {
            label: None,
            size: 0,
            usage: BufferUsages::empty(),
            mapped_at_creation: false,
        }
    }
}

/// Buffer mapping mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferMapMode {
    /// Map for reading.
    Read,
    /// Map for writing.
    Write,
}

/// Slice of a buffer for mapping.
#[derive(Debug)]
pub struct BufferSlice<'a> {
    /// Pointer to the buffer (for trait object storage).
    /// This is intentionally opaque - backends interpret as needed.
    _marker: core::marker::PhantomData<&'a ()>,
}

impl<'a> BufferSlice<'a> {
    /// Creates a new buffer slice placeholder.
    pub const fn new() -> Self {
        Self {
            _marker: core::marker::PhantomData,
        }
    }
}

impl<'a> Default for BufferSlice<'a> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Texture Types
// ============================================================================

/// Texture usage flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextureUsages(u32);

impl TextureUsages {
    pub const COPY_SRC: Self = Self(1 << 0);
    pub const COPY_DST: Self = Self(1 << 1);
    pub const TEXTURE_BINDING: Self = Self(1 << 2);
    pub const STORAGE_BINDING: Self = Self(1 << 3);
    pub const RENDER_ATTACHMENT: Self = Self(1 << 4);

    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl core::ops::BitOr for TextureUsages {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

/// Texture dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum TextureDimension {
    /// 1D texture.
    D1 = 1,

    /// 2D texture (most common).
    #[default]
    D2 = 2,

    /// 3D texture.
    D3 = 3,
}

/// Texture format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum HalTextureFormat {
    // 8-bit formats
    R8Unorm = 0,
    R8Snorm = 1,
    R8Uint = 2,
    R8Sint = 3,

    // 16-bit formats
    R16Uint = 10,
    R16Sint = 11,
    R16Float = 12,
    Rg8Unorm = 13,
    Rg8Snorm = 14,
    Rg8Uint = 15,
    Rg8Sint = 16,

    // 32-bit formats
    R32Uint = 20,
    R32Sint = 21,
    R32Float = 22,
    Rg16Uint = 23,
    Rg16Sint = 24,
    Rg16Float = 25,
    Rgba8Unorm = 26,
    Rgba8UnormSrgb = 27,
    Rgba8Snorm = 28,
    Rgba8Uint = 29,
    Rgba8Sint = 30,
    Bgra8Unorm = 31,
    Bgra8UnormSrgb = 32,

    // 64-bit formats
    Rg32Uint = 40,
    Rg32Sint = 41,
    Rg32Float = 42,
    Rgba16Uint = 43,
    Rgba16Sint = 44,
    Rgba16Float = 45,

    // 128-bit formats
    Rgba32Uint = 50,
    Rgba32Sint = 51,
    Rgba32Float = 52,

    // Depth/stencil formats
    Depth16Unorm = 100,
    Depth24Plus = 101,
    Depth24PlusStencil8 = 102,
    Depth32Float = 103,
    Depth32FloatStencil8 = 104,
    Stencil8 = 105,

    // Compressed formats (BC)
    Bc1RgbaUnorm = 200,
    Bc1RgbaUnormSrgb = 201,
    Bc2RgbaUnorm = 202,
    Bc2RgbaUnormSrgb = 203,
    Bc3RgbaUnorm = 204,
    Bc3RgbaUnormSrgb = 205,
    Bc4RUnorm = 206,
    Bc4RSnorm = 207,
    Bc5RgUnorm = 208,
    Bc5RgSnorm = 209,
    Bc6hRgbUfloat = 210,
    Bc6hRgbFloat = 211,
    Bc7RgbaUnorm = 212,
    Bc7RgbaUnormSrgb = 213,
}

impl Default for HalTextureFormat {
    fn default() -> Self {
        HalTextureFormat::Rgba8Unorm
    }
}

impl HalTextureFormat {
    /// Returns the bytes per pixel/block for this format.
    pub const fn bytes_per_block(self) -> u32 {
        match self {
            HalTextureFormat::R8Unorm
            | HalTextureFormat::R8Snorm
            | HalTextureFormat::R8Uint
            | HalTextureFormat::R8Sint
            | HalTextureFormat::Stencil8 => 1,

            HalTextureFormat::R16Uint
            | HalTextureFormat::R16Sint
            | HalTextureFormat::R16Float
            | HalTextureFormat::Rg8Unorm
            | HalTextureFormat::Rg8Snorm
            | HalTextureFormat::Rg8Uint
            | HalTextureFormat::Rg8Sint
            | HalTextureFormat::Depth16Unorm => 2,

            HalTextureFormat::R32Uint
            | HalTextureFormat::R32Sint
            | HalTextureFormat::R32Float
            | HalTextureFormat::Rg16Uint
            | HalTextureFormat::Rg16Sint
            | HalTextureFormat::Rg16Float
            | HalTextureFormat::Rgba8Unorm
            | HalTextureFormat::Rgba8UnormSrgb
            | HalTextureFormat::Rgba8Snorm
            | HalTextureFormat::Rgba8Uint
            | HalTextureFormat::Rgba8Sint
            | HalTextureFormat::Bgra8Unorm
            | HalTextureFormat::Bgra8UnormSrgb
            | HalTextureFormat::Depth24Plus
            | HalTextureFormat::Depth24PlusStencil8
            | HalTextureFormat::Depth32Float => 4,

            HalTextureFormat::Rg32Uint
            | HalTextureFormat::Rg32Sint
            | HalTextureFormat::Rg32Float
            | HalTextureFormat::Rgba16Uint
            | HalTextureFormat::Rgba16Sint
            | HalTextureFormat::Rgba16Float
            | HalTextureFormat::Depth32FloatStencil8
            | HalTextureFormat::Bc1RgbaUnorm
            | HalTextureFormat::Bc1RgbaUnormSrgb
            | HalTextureFormat::Bc4RUnorm
            | HalTextureFormat::Bc4RSnorm => 8,

            HalTextureFormat::Rgba32Uint
            | HalTextureFormat::Rgba32Sint
            | HalTextureFormat::Rgba32Float
            | HalTextureFormat::Bc2RgbaUnorm
            | HalTextureFormat::Bc2RgbaUnormSrgb
            | HalTextureFormat::Bc3RgbaUnorm
            | HalTextureFormat::Bc3RgbaUnormSrgb
            | HalTextureFormat::Bc5RgUnorm
            | HalTextureFormat::Bc5RgSnorm
            | HalTextureFormat::Bc6hRgbUfloat
            | HalTextureFormat::Bc6hRgbFloat
            | HalTextureFormat::Bc7RgbaUnorm
            | HalTextureFormat::Bc7RgbaUnormSrgb => 16,
        }
    }

    /// Returns true if this is a depth format.
    pub const fn is_depth(self) -> bool {
        matches!(
            self,
            HalTextureFormat::Depth16Unorm
                | HalTextureFormat::Depth24Plus
                | HalTextureFormat::Depth24PlusStencil8
                | HalTextureFormat::Depth32Float
                | HalTextureFormat::Depth32FloatStencil8
        )
    }

    /// Returns true if this is a stencil format.
    pub const fn is_stencil(self) -> bool {
        matches!(
            self,
            HalTextureFormat::Depth24PlusStencil8
                | HalTextureFormat::Depth32FloatStencil8
                | HalTextureFormat::Stencil8
        )
    }

    /// Returns true if this is a compressed format.
    pub const fn is_compressed(self) -> bool {
        matches!(
            self,
            HalTextureFormat::Bc1RgbaUnorm
                | HalTextureFormat::Bc1RgbaUnormSrgb
                | HalTextureFormat::Bc2RgbaUnorm
                | HalTextureFormat::Bc2RgbaUnormSrgb
                | HalTextureFormat::Bc3RgbaUnorm
                | HalTextureFormat::Bc3RgbaUnormSrgb
                | HalTextureFormat::Bc4RUnorm
                | HalTextureFormat::Bc4RSnorm
                | HalTextureFormat::Bc5RgUnorm
                | HalTextureFormat::Bc5RgSnorm
                | HalTextureFormat::Bc6hRgbUfloat
                | HalTextureFormat::Bc6hRgbFloat
                | HalTextureFormat::Bc7RgbaUnorm
                | HalTextureFormat::Bc7RgbaUnormSrgb
        )
    }
}

/// Extent in 3 dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Extent3d {
    pub width: u32,
    pub height: u32,
    pub depth_or_array_layers: u32,
}

impl Extent3d {
    /// Creates a new extent.
    pub const fn new(width: u32, height: u32, depth_or_array_layers: u32) -> Self {
        Self {
            width,
            height,
            depth_or_array_layers,
        }
    }

    /// Returns the total number of texels.
    pub const fn texel_count(&self) -> u64 {
        self.width as u64 * self.height as u64 * self.depth_or_array_layers as u64
    }
}

/// Descriptor for texture creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureDescriptor {
    /// Debug label.
    pub label: Option<&'static str>,

    /// Texture dimensions.
    pub size: Extent3d,

    /// Mip level count.
    pub mip_level_count: u32,

    /// Sample count (1 for non-MSAA).
    pub sample_count: u32,

    /// Texture dimension.
    pub dimension: TextureDimension,

    /// Texture format.
    pub format: HalTextureFormat,

    /// Usage flags.
    pub usage: TextureUsages,

    /// View formats for this texture (empty = same as base format).
    pub view_formats_count: u32,
}

impl Default for TextureDescriptor {
    fn default() -> Self {
        Self {
            label: None,
            size: Extent3d::new(1, 1, 1),
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: HalTextureFormat::Rgba8Unorm,
            usage: TextureUsages::empty(),
            view_formats_count: 0,
        }
    }
}

/// Texture view dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum TextureViewDimension {
    D1 = 1,
    #[default]
    D2 = 2,
    D2Array = 3,
    Cube = 4,
    CubeArray = 5,
    D3 = 6,
}

/// Texture aspect for views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum TextureAspect {
    #[default]
    All = 0,
    StencilOnly = 1,
    DepthOnly = 2,
}

/// Descriptor for texture view creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureViewDescriptor {
    /// Debug label.
    pub label: Option<&'static str>,

    /// Format (None = inherit from texture).
    pub format: Option<HalTextureFormat>,

    /// View dimension.
    pub dimension: Option<TextureViewDimension>,

    /// Texture aspect.
    pub aspect: TextureAspect,

    /// Base mip level.
    pub base_mip_level: u32,

    /// Mip level count (None = all remaining).
    pub mip_level_count: Option<u32>,

    /// Base array layer.
    pub base_array_layer: u32,

    /// Array layer count (None = all remaining).
    pub array_layer_count: Option<u32>,
}

impl Default for TextureViewDescriptor {
    fn default() -> Self {
        Self {
            label: None,
            format: None,
            dimension: None,
            aspect: TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        }
    }
}

// ============================================================================
// Sampler Types
// ============================================================================

/// Texture address mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum AddressMode {
    #[default]
    ClampToEdge = 0,
    Repeat = 1,
    MirrorRepeat = 2,
    ClampToBorder = 3,
}

/// Texture filtering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum FilterMode {
    #[default]
    Nearest = 0,
    Linear = 1,
}

/// Comparison function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum CompareFunction {
    #[default]
    Never = 0,
    Less = 1,
    Equal = 2,
    LessEqual = 3,
    Greater = 4,
    NotEqual = 5,
    GreaterEqual = 6,
    Always = 7,
}

/// Descriptor for sampler creation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplerDescriptor {
    /// Debug label.
    pub label: Option<&'static str>,

    /// Address mode for U coordinate.
    pub address_mode_u: AddressMode,
    /// Address mode for V coordinate.
    pub address_mode_v: AddressMode,
    /// Address mode for W coordinate.
    pub address_mode_w: AddressMode,

    /// Magnification filter.
    pub mag_filter: FilterMode,
    /// Minification filter.
    pub min_filter: FilterMode,
    /// Mipmap filter.
    pub mipmap_filter: FilterMode,

    /// LOD clamp range.
    pub lod_min_clamp: f32,
    pub lod_max_clamp: f32,

    /// Depth comparison function (None = no comparison).
    pub compare: Option<CompareFunction>,

    /// Anisotropic filtering (1 = disabled).
    pub anisotropy_clamp: u16,

    /// Border color for ClampToBorder mode.
    pub border_color: Option<SamplerBorderColor>,
}

impl Default for SamplerDescriptor {
    fn default() -> Self {
        Self {
            label: None,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 32.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        }
    }
}

/// Border color for sampler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SamplerBorderColor {
    TransparentBlack = 0,
    OpaqueBlack = 1,
    OpaqueWhite = 2,
    Zero = 3,
}

// ============================================================================
// Bind Group Types
// ============================================================================

/// Binding type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BindingType {
    UniformBuffer = 0,
    StorageBuffer = 1,
    ReadOnlyStorageBuffer = 2,
    Sampler = 3,
    ComparisonSampler = 4,
    SampledTexture = 5,
    StorageTexture = 6,
}

/// Bind group layout entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindGroupLayoutEntry {
    /// Binding index.
    pub binding: u32,

    /// Shader stage visibility.
    pub visibility: ShaderStages,

    /// Binding type.
    pub ty: BindingType,

    /// Count (1 for non-array, >1 for array).
    pub count: Option<u32>,
}

/// Descriptor for bind group layout creation.
#[derive(Debug, Clone)]
pub struct BindGroupLayoutDescriptor<'a> {
    /// Debug label.
    pub label: Option<&'static str>,

    /// Layout entries.
    pub entries: &'a [BindGroupLayoutEntry],
}

/// Bind group entry.
#[derive(Debug, Clone, Copy)]
pub struct BindGroupEntry<'a> {
    /// Binding index.
    pub binding: u32,

    /// Resource reference.
    pub resource: BindingResource<'a>,
}

/// Resource for binding.
#[derive(Debug, Clone, Copy)]
pub enum BindingResource<'a> {
    /// Buffer binding.
    Buffer(BufferBinding<'a>),

    /// Sampler binding.
    Sampler,

    /// Texture view binding.
    TextureView,

    /// Placeholder for runtime type.
    _Phantom(core::marker::PhantomData<&'a ()>),
}

/// Buffer binding info.
#[derive(Debug, Clone, Copy)]
pub struct BufferBinding<'a> {
    /// Offset into the buffer.
    pub offset: u64,

    /// Size of the binding (None = to end of buffer).
    pub size: Option<u64>,

    /// Lifetime marker.
    _marker: core::marker::PhantomData<&'a ()>,
}

impl<'a> BufferBinding<'a> {
    pub const fn new(offset: u64, size: Option<u64>) -> Self {
        Self {
            offset,
            size,
            _marker: core::marker::PhantomData,
        }
    }
}

/// Shader stage visibility flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ShaderStages(u32);

impl ShaderStages {
    pub const NONE: Self = Self(0);
    pub const VERTEX: Self = Self(1 << 0);
    pub const FRAGMENT: Self = Self(1 << 1);
    pub const COMPUTE: Self = Self(1 << 2);
    pub const VERTEX_FRAGMENT: Self = Self(Self::VERTEX.0 | Self::FRAGMENT.0);
    pub const ALL: Self = Self(Self::VERTEX.0 | Self::FRAGMENT.0 | Self::COMPUTE.0);

    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl core::ops::BitOr for ShaderStages {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

// ============================================================================
// Pipeline Types
// ============================================================================

/// Pipeline layout descriptor.
#[derive(Debug, Clone)]
pub struct PipelineLayoutDescriptor<'a, D> {
    /// Debug label.
    pub label: Option<&'static str>,

    /// Bind group layouts.
    pub bind_group_layouts: &'a [*const D],

    /// Push constant ranges.
    pub push_constant_ranges: &'a [PushConstantRange],
}

/// Push constant range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushConstantRange {
    /// Shader stages.
    pub stages: ShaderStages,

    /// Byte offset.
    pub offset: u32,

    /// Byte size.
    pub size: u32,
}

/// Shader source.
#[derive(Debug, Clone, Copy)]
pub enum ShaderSource<'a> {
    /// SPIR-V binary.
    SpirV(&'a [u32]),

    /// WGSL source.
    Wgsl(&'a str),
}

/// Vertex format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VertexFormat {
    Uint8x2 = 0,
    Uint8x4 = 1,
    Sint8x2 = 2,
    Sint8x4 = 3,
    Unorm8x2 = 4,
    Unorm8x4 = 5,
    Snorm8x2 = 6,
    Snorm8x4 = 7,
    Uint16x2 = 8,
    Uint16x4 = 9,
    Sint16x2 = 10,
    Sint16x4 = 11,
    Unorm16x2 = 12,
    Unorm16x4 = 13,
    Snorm16x2 = 14,
    Snorm16x4 = 15,
    Float16x2 = 16,
    Float16x4 = 17,
    Float32 = 18,
    Float32x2 = 19,
    Float32x3 = 20,
    Float32x4 = 21,
    Uint32 = 22,
    Uint32x2 = 23,
    Uint32x3 = 24,
    Uint32x4 = 25,
    Sint32 = 26,
    Sint32x2 = 27,
    Sint32x3 = 28,
    Sint32x4 = 29,
}

impl VertexFormat {
    /// Returns the size in bytes.
    pub const fn size(self) -> u64 {
        match self {
            VertexFormat::Uint8x2 | VertexFormat::Sint8x2 | VertexFormat::Unorm8x2 | VertexFormat::Snorm8x2 => 2,
            VertexFormat::Uint8x4 | VertexFormat::Sint8x4 | VertexFormat::Unorm8x4 | VertexFormat::Snorm8x4 => 4,
            VertexFormat::Uint16x2 | VertexFormat::Sint16x2 | VertexFormat::Unorm16x2 | VertexFormat::Snorm16x2 | VertexFormat::Float16x2 => 4,
            VertexFormat::Uint16x4 | VertexFormat::Sint16x4 | VertexFormat::Unorm16x4 | VertexFormat::Snorm16x4 | VertexFormat::Float16x4 => 8,
            VertexFormat::Float32 | VertexFormat::Uint32 | VertexFormat::Sint32 => 4,
            VertexFormat::Float32x2 | VertexFormat::Uint32x2 | VertexFormat::Sint32x2 => 8,
            VertexFormat::Float32x3 | VertexFormat::Uint32x3 | VertexFormat::Sint32x3 => 12,
            VertexFormat::Float32x4 | VertexFormat::Uint32x4 | VertexFormat::Sint32x4 => 16,
        }
    }
}

/// Vertex step mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum VertexStepMode {
    #[default]
    Vertex = 0,
    Instance = 1,
}

/// Vertex attribute descriptor.
#[derive(Debug, Clone, Copy)]
pub struct VertexAttribute {
    /// Format of the attribute.
    pub format: VertexFormat,

    /// Byte offset within the vertex.
    pub offset: u64,

    /// Shader location.
    pub shader_location: u32,
}

/// Vertex buffer layout.
#[derive(Debug, Clone)]
pub struct VertexBufferLayout<'a> {
    /// Stride between vertices.
    pub array_stride: u64,

    /// Step mode.
    pub step_mode: VertexStepMode,

    /// Attributes in this buffer.
    pub attributes: &'a [VertexAttribute],
}

/// Primitive topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum PrimitiveTopology {
    PointList = 0,
    LineList = 1,
    LineStrip = 2,
    #[default]
    TriangleList = 3,
    TriangleStrip = 4,
}

/// Index format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum IndexFormat {
    #[default]
    Uint16 = 0,
    Uint32 = 1,
}

impl IndexFormat {
    /// Returns the size in bytes.
    pub const fn size(self) -> usize {
        match self {
            IndexFormat::Uint16 => 2,
            IndexFormat::Uint32 => 4,
        }
    }
}

/// Front face winding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum FrontFace {
    #[default]
    Ccw = 0,
    Cw = 1,
}

/// Face culling mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Face {
    #[default]
    Front = 0,
    Back = 1,
}

/// Primitive state.
#[derive(Debug, Clone, Copy, Default)]
pub struct PrimitiveState {
    pub topology: PrimitiveTopology,
    pub strip_index_format: Option<IndexFormat>,
    pub front_face: FrontFace,
    pub cull_mode: Option<Face>,
    pub unclipped_depth: bool,
    pub conservative: bool,
}

/// Depth stencil state.
#[derive(Debug, Clone, Copy)]
pub struct DepthStencilState {
    pub format: HalTextureFormat,
    pub depth_write_enabled: bool,
    pub depth_compare: CompareFunction,
    pub stencil: StencilState,
    pub bias: DepthBiasState,
}

/// Stencil state.
#[derive(Debug, Clone, Copy, Default)]
pub struct StencilState {
    pub front: StencilFaceState,
    pub back: StencilFaceState,
    pub read_mask: u32,
    pub write_mask: u32,
}

/// Stencil operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum StencilOperation {
    #[default]
    Keep = 0,
    Zero = 1,
    Replace = 2,
    IncrementClamp = 3,
    DecrementClamp = 4,
    Invert = 5,
    IncrementWrap = 6,
    DecrementWrap = 7,
}

/// Stencil face state.
#[derive(Debug, Clone, Copy)]
pub struct StencilFaceState {
    pub compare: CompareFunction,
    pub fail_op: StencilOperation,
    pub depth_fail_op: StencilOperation,
    pub pass_op: StencilOperation,
}

impl Default for StencilFaceState {
    fn default() -> Self {
        Self {
            compare: CompareFunction::Always,
            fail_op: StencilOperation::Keep,
            depth_fail_op: StencilOperation::Keep,
            pass_op: StencilOperation::Keep,
        }
    }
}

/// Depth bias state.
#[derive(Debug, Clone, Copy, Default)]
pub struct DepthBiasState {
    pub constant: i32,
    pub slope_scale: f32,
    pub clamp: f32,
}

/// Multisample state.
#[derive(Debug, Clone, Copy)]
pub struct MultisampleState {
    pub count: u32,
    pub mask: u64,
    pub alpha_to_coverage_enabled: bool,
}

impl Default for MultisampleState {
    fn default() -> Self {
        Self {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        }
    }
}

/// Blend component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlendComponent {
    pub src_factor: BlendFactor,
    pub dst_factor: BlendFactor,
    pub operation: BlendOperation,
}

impl Default for BlendComponent {
    fn default() -> Self {
        Self {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Add,
        }
    }
}

/// Blend factor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum BlendFactor {
    #[default]
    Zero = 0,
    One = 1,
    Src = 2,
    OneMinusSrc = 3,
    SrcAlpha = 4,
    OneMinusSrcAlpha = 5,
    Dst = 6,
    OneMinusDst = 7,
    DstAlpha = 8,
    OneMinusDstAlpha = 9,
    SrcAlphaSaturated = 10,
    Constant = 11,
    OneMinusConstant = 12,
}

/// Blend operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum BlendOperation {
    #[default]
    Add = 0,
    Subtract = 1,
    ReverseSubtract = 2,
    Min = 3,
    Max = 4,
}

/// Blend state.
#[derive(Debug, Clone, Copy)]
pub struct BlendState {
    pub color: BlendComponent,
    pub alpha: BlendComponent,
}

impl Default for BlendState {
    fn default() -> Self {
        Self {
            color: BlendComponent::default(),
            alpha: BlendComponent::default(),
        }
    }
}

/// Color write mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColorWrites(u32);

impl ColorWrites {
    pub const RED: Self = Self(1 << 0);
    pub const GREEN: Self = Self(1 << 1);
    pub const BLUE: Self = Self(1 << 2);
    pub const ALPHA: Self = Self(1 << 3);
    pub const ALL: Self = Self(Self::RED.0 | Self::GREEN.0 | Self::BLUE.0 | Self::ALPHA.0);

    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl Default for ColorWrites {
    fn default() -> Self {
        Self::ALL
    }
}

impl core::ops::BitOr for ColorWrites {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

/// Color target state.
#[derive(Debug, Clone, Copy)]
pub struct ColorTargetState {
    pub format: HalTextureFormat,
    pub blend: Option<BlendState>,
    pub write_mask: ColorWrites,
}

/// Fragment state.
#[derive(Debug, Clone)]
pub struct FragmentState<'a, D> {
    pub module: *const D,
    pub entry_point: &'static str,
    pub targets: &'a [Option<ColorTargetState>],
}

/// Vertex state.
#[derive(Debug, Clone)]
pub struct VertexState<'a, D> {
    pub module: *const D,
    pub entry_point: &'static str,
    pub buffers: &'a [VertexBufferLayout<'a>],
}

// ============================================================================
// Render Pass Types
// ============================================================================

/// Load operation for attachments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum LoadOp<V> {
    /// Clear with specified value.
    Clear(V),

    /// Load existing contents.
    #[default]
    Load,
}

/// Store operation for attachments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum StoreOp {
    /// Store results.
    #[default]
    Store = 0,

    /// Discard results.
    Discard = 1,
}

/// Color value.
#[derive(Debug, Clone, Copy, Default)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Color {
    pub const BLACK: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const WHITE: Self = Self { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const RED: Self = Self { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const GREEN: Self = Self { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };
    pub const BLUE: Self = Self { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
    pub const TRANSPARENT: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
}

/// Operations for render pass attachments.
#[derive(Debug, Clone, Copy)]
pub struct Operations<V> {
    pub load: LoadOp<V>,
    pub store: StoreOp,
}

impl<V: Default> Default for Operations<V> {
    fn default() -> Self {
        Self {
            load: LoadOp::Load,
            store: StoreOp::Store,
        }
    }
}

/// Compute pass descriptor.
#[derive(Debug, Clone, Copy, Default)]
pub struct ComputePassDescriptor<'a> {
    pub label: Option<&'a str>,
}

// ============================================================================
// Copy Operations
// ============================================================================

/// Image copy texture.
#[derive(Debug, Clone, Copy)]
pub struct ImageCopyTexture<'a> {
    /// Texture reference (opaque pointer).
    pub texture: *const (),

    /// Mip level.
    pub mip_level: u32,

    /// Origin.
    pub origin: Origin3d,

    /// Texture aspect.
    pub aspect: TextureAspect,

    /// Lifetime marker.
    _marker: core::marker::PhantomData<&'a ()>,
}

impl<'a> ImageCopyTexture<'a> {
    /// Creates a new image copy texture.
    pub const fn new(texture: *const (), mip_level: u32, origin: Origin3d, aspect: TextureAspect) -> Self {
        Self {
            texture,
            mip_level,
            origin,
            aspect,
            _marker: core::marker::PhantomData,
        }
    }
}

/// Image copy buffer.
#[derive(Debug, Clone, Copy)]
pub struct ImageCopyBuffer<'a> {
    /// Buffer reference (opaque pointer).
    pub buffer: *const (),

    /// Data layout.
    pub layout: ImageDataLayout,

    /// Lifetime marker.
    _marker: core::marker::PhantomData<&'a ()>,
}

impl<'a> ImageCopyBuffer<'a> {
    /// Creates a new image copy buffer.
    pub const fn new(buffer: *const (), layout: ImageDataLayout) -> Self {
        Self {
            buffer,
            layout,
            _marker: core::marker::PhantomData,
        }
    }
}

/// 3D origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Origin3d {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

impl Origin3d {
    pub const ZERO: Self = Self { x: 0, y: 0, z: 0 };
}

/// Image data layout.
#[derive(Debug, Clone, Copy, Default)]
pub struct ImageDataLayout {
    /// Offset into the buffer.
    pub offset: u64,

    /// Bytes per row (None = tightly packed).
    pub bytes_per_row: Option<u32>,

    /// Rows per image (None = tightly packed).
    pub rows_per_image: Option<u32>,
}

// ============================================================================
// Maintenance
// ============================================================================

/// Maintenance mode for device polling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Maintain {
    /// Wait for work to complete.
    #[default]
    Wait,

    /// Poll without waiting.
    Poll,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_type_default() {
        let _backend = BackendType::default();
        // Default is platform-specific, just ensure it compiles
    }

    #[test]
    fn test_backend_type_name() {
        assert_eq!(BackendType::Vulkan.name(), "Vulkan");
        assert_eq!(BackendType::Metal.name(), "Metal");
        assert_eq!(BackendType::Dx12.name(), "DX12");
    }

    #[test]
    fn test_device_type_is_hardware() {
        assert!(DeviceType::DiscreteGpu.is_hardware());
        assert!(DeviceType::IntegratedGpu.is_hardware());
        assert!(!DeviceType::Cpu.is_hardware());
        assert!(!DeviceType::VirtualGpu.is_hardware());
    }

    #[test]
    fn test_device_type_performance_tier() {
        assert!(DeviceType::DiscreteGpu.performance_tier() > DeviceType::IntegratedGpu.performance_tier());
        assert!(DeviceType::IntegratedGpu.performance_tier() > DeviceType::Cpu.performance_tier());
    }

    #[test]
    fn test_features_operations() {
        let f1 = Features::DEPTH_CLIP_CONTROL;
        let f2 = Features::TIMESTAMP_QUERY;

        let combined = f1 | f2;
        assert!(combined.contains(f1));
        assert!(combined.contains(f2));
        assert!(!f1.contains(f2));
    }

    #[test]
    fn test_features_empty() {
        let empty = Features::empty();
        assert!(empty.is_empty());
        assert!(!Features::DEPTH_CLIP_CONTROL.is_empty());
    }

    #[test]
    fn test_limits_default() {
        let limits = Limits::default();
        assert!(limits.max_texture_dimension_2d >= 2048);
        assert!(limits.max_buffer_size >= 128 * 1024 * 1024);
    }

    #[test]
    fn test_buffer_usages() {
        let usage = BufferUsages::VERTEX | BufferUsages::INDEX;
        assert!(usage.contains(BufferUsages::VERTEX));
        assert!(usage.contains(BufferUsages::INDEX));
        assert!(!usage.contains(BufferUsages::UNIFORM));
    }

    #[test]
    fn test_texture_format_bytes_per_block() {
        assert_eq!(HalTextureFormat::R8Unorm.bytes_per_block(), 1);
        assert_eq!(HalTextureFormat::Rgba8Unorm.bytes_per_block(), 4);
        assert_eq!(HalTextureFormat::Rgba32Float.bytes_per_block(), 16);
    }

    #[test]
    fn test_texture_format_is_depth() {
        assert!(HalTextureFormat::Depth32Float.is_depth());
        assert!(HalTextureFormat::Depth24PlusStencil8.is_depth());
        assert!(!HalTextureFormat::Rgba8Unorm.is_depth());
    }

    #[test]
    fn test_extent3d() {
        let extent = Extent3d::new(1920, 1080, 1);
        assert_eq!(extent.texel_count(), 1920 * 1080);
    }

    #[test]
    fn test_adapter_info() {
        let info = AdapterInfo::new("Test GPU", DeviceType::DiscreteGpu, BackendType::Vulkan);
        assert_eq!(info.name_str(), "Test GPU");
        assert_eq!(info.device_type, DeviceType::DiscreteGpu);
    }

    #[test]
    fn test_shader_stages() {
        let stages = ShaderStages::VERTEX | ShaderStages::FRAGMENT;
        assert!(stages.contains(ShaderStages::VERTEX));
        assert!(stages.contains(ShaderStages::FRAGMENT));
        assert!(!stages.contains(ShaderStages::COMPUTE));
    }

    #[test]
    fn test_color_constants() {
        assert_eq!(Color::BLACK.r, 0.0);
        assert_eq!(Color::WHITE.r, 1.0);
        assert_eq!(Color::TRANSPARENT.a, 0.0);
    }

    #[test]
    fn test_vertex_format_size() {
        assert_eq!(VertexFormat::Float32.size(), 4);
        assert_eq!(VertexFormat::Float32x4.size(), 16);
        assert_eq!(VertexFormat::Uint8x4.size(), 4);
    }

    #[test]
    fn test_index_format_size() {
        assert_eq!(IndexFormat::Uint16.size(), 2);
        assert_eq!(IndexFormat::Uint32.size(), 4);
    }

    #[test]
    fn test_color_writes() {
        let mask = ColorWrites::RED | ColorWrites::GREEN;
        assert_eq!(mask.bits(), ColorWrites::RED.bits() | ColorWrites::GREEN.bits());
    }

    #[test]
    fn test_send_sync_bounds() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<BackendType>();
        assert_send_sync::<DeviceType>();
        assert_send_sync::<Features>();
        assert_send_sync::<Limits>();
        assert_send_sync::<BufferUsages>();
        assert_send_sync::<TextureUsages>();
        assert_send_sync::<AdapterInfo>();
    }
}
