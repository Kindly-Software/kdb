//! Metal Backend Types - Enums and Type Definitions
//!
//! Provides Metal-specific type definitions for the KGPU Metal backend.
//! These are MOCK types for design validation - not real Metal FFI bindings.
//!
//! # Design
//!
//! These types mirror Apple Metal API concepts but are implemented as
//! pure Rust enums and structs for cross-platform compilation and testing.
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_MOCK_HANDLES`: All handles are mock values, not real Metal objects
//! - `#ASSUME_ENUM_REPR`: Enum values match Metal SDK for future FFI compatibility
//!
//! # UCE34 Compliance
//!
//! - **Q10**: Types support T1 Atomic tier capsule integration
//! - **Q33**: All types are Copy + Clone for atomic-friendly usage

// ============================================================================
// Pixel Format (MTLPixelFormat equivalent)
// ============================================================================

/// Metal pixel format enumeration
///
/// Values match Apple's MTLPixelFormat for FFI compatibility.
/// See: https://developer.apple.com/documentation/metal/mtlpixelformat
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
#[allow(non_camel_case_types)]
pub enum MTLPixelFormat {
    /// Invalid/unspecified format
    Invalid = 0,

    // ========================================================================
    // 8-bit formats
    // ========================================================================
    /// 8-bit unsigned normalized (single channel)
    R8Unorm = 10,
    /// 8-bit signed normalized (single channel)
    R8Snorm = 12,
    /// 8-bit unsigned integer (single channel)
    R8Uint = 13,
    /// 8-bit signed integer (single channel)
    R8Sint = 14,

    // ========================================================================
    // 16-bit formats
    // ========================================================================
    /// 16-bit unsigned normalized (single channel)
    R16Unorm = 20,
    /// 16-bit float (single channel)
    R16Float = 25,
    /// 16-bit unsigned normalized (RG)
    RG8Unorm = 30,

    // ========================================================================
    // 32-bit formats
    // ========================================================================
    /// 32-bit float (single channel)
    R32Float = 55,
    /// 32-bit unsigned normalized (RG)
    RG16Unorm = 60,
    /// 32-bit float (RG)
    RG16Float = 65,

    // ========================================================================
    // RGBA 8-bit formats
    // ========================================================================
    /// RGBA 8-bit unsigned normalized
    RGBA8Unorm = 70,
    /// RGBA 8-bit unsigned normalized (sRGB)
    RGBA8Unorm_sRGB = 71,
    /// RGBA 8-bit signed normalized
    RGBA8Snorm = 72,
    /// RGBA 8-bit unsigned integer
    RGBA8Uint = 73,
    /// RGBA 8-bit signed integer
    RGBA8Sint = 74,

    // ========================================================================
    // BGRA 8-bit formats (common for display)
    // ========================================================================
    /// BGRA 8-bit unsigned normalized
    BGRA8Unorm = 80,
    /// BGRA 8-bit unsigned normalized (sRGB)
    BGRA8Unorm_sRGB = 81,

    // ========================================================================
    // 16-bit per channel formats
    // ========================================================================
    /// RGBA 16-bit unsigned normalized
    RGBA16Unorm = 110,
    /// RGBA 16-bit signed normalized
    RGBA16Snorm = 112,
    /// RGBA 16-bit float
    RGBA16Float = 115,

    // ========================================================================
    // 32-bit per channel formats
    // ========================================================================
    /// RGBA 32-bit float
    RGBA32Float = 125,
    /// RGBA 32-bit unsigned integer
    RGBA32Uint = 123,
    /// RGBA 32-bit signed integer
    RGBA32Sint = 124,

    // ========================================================================
    // Depth/Stencil formats
    // ========================================================================
    /// 16-bit depth
    Depth16Unorm = 250,
    /// 32-bit float depth
    Depth32Float = 252,
    /// 8-bit stencil
    Stencil8 = 253,
    /// 24-bit depth + 8-bit stencil (packed)
    Depth24Unorm_Stencil8 = 255,
    /// 32-bit float depth + 8-bit stencil
    Depth32Float_Stencil8 = 260,

    // ========================================================================
    // Compressed formats (BC/ASTC)
    // ========================================================================
    /// BC1 (DXT1) RGB
    BC1_RGBA = 130,
    /// BC2 (DXT3) RGBA
    BC2_RGBA = 131,
    /// BC3 (DXT5) RGBA
    BC3_RGBA = 132,
    /// BC7 RGBA
    BC7_RGBAUnorm = 140,
}

impl MTLPixelFormat {
    /// Returns the bytes per pixel for this format (0 for compressed/invalid)
    #[inline]
    pub const fn bytes_per_pixel(&self) -> u32 {
        match self {
            Self::Invalid => 0,
            Self::R8Unorm | Self::R8Snorm | Self::R8Uint | Self::R8Sint | Self::Stencil8 => 1,
            Self::R16Unorm | Self::R16Float | Self::RG8Unorm | Self::Depth16Unorm => 2,
            Self::R32Float
            | Self::RG16Unorm
            | Self::RG16Float
            | Self::RGBA8Unorm
            | Self::RGBA8Unorm_sRGB
            | Self::RGBA8Snorm
            | Self::RGBA8Uint
            | Self::RGBA8Sint
            | Self::BGRA8Unorm
            | Self::BGRA8Unorm_sRGB
            | Self::Depth32Float
            | Self::Depth24Unorm_Stencil8 => 4,
            Self::Depth32Float_Stencil8 => 5,
            Self::RGBA16Unorm | Self::RGBA16Snorm | Self::RGBA16Float => 8,
            Self::RGBA32Float | Self::RGBA32Uint | Self::RGBA32Sint => 16,
            // Compressed formats - return 0 (block-based)
            Self::BC1_RGBA | Self::BC2_RGBA | Self::BC3_RGBA | Self::BC7_RGBAUnorm => 0,
        }
    }

    /// Returns true if this is a depth format
    #[inline]
    pub const fn is_depth(&self) -> bool {
        matches!(
            self,
            Self::Depth16Unorm
                | Self::Depth32Float
                | Self::Depth24Unorm_Stencil8
                | Self::Depth32Float_Stencil8
        )
    }

    /// Returns true if this is a stencil format
    #[inline]
    pub const fn is_stencil(&self) -> bool {
        matches!(
            self,
            Self::Stencil8 | Self::Depth24Unorm_Stencil8 | Self::Depth32Float_Stencil8
        )
    }

    /// Returns true if this is an sRGB format
    #[inline]
    pub const fn is_srgb(&self) -> bool {
        matches!(self, Self::RGBA8Unorm_sRGB | Self::BGRA8Unorm_sRGB)
    }

    /// Returns true if this is a compressed format
    #[inline]
    pub const fn is_compressed(&self) -> bool {
        matches!(
            self,
            Self::BC1_RGBA | Self::BC2_RGBA | Self::BC3_RGBA | Self::BC7_RGBAUnorm
        )
    }
}

impl Default for MTLPixelFormat {
    fn default() -> Self {
        Self::Invalid
    }
}

// ============================================================================
// Storage Mode (MTLStorageMode equivalent)
// ============================================================================

/// Metal resource storage mode
///
/// Determines CPU/GPU accessibility and synchronization requirements.
/// Values match Apple's MTLStorageMode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum MTLStorageMode {
    /// CPU and GPU accessible (automatic sync on iOS, manual on macOS)
    #[default]
    Shared = 0,
    /// macOS only: Explicit synchronization required between CPU/GPU
    Managed = 1,
    /// GPU-only access (fastest for GPU operations)
    Private = 2,
    /// Tile memory only (iOS/Apple Silicon - memoryless attachments)
    Memoryless = 3,
}

impl MTLStorageMode {
    /// Returns true if CPU can access this storage mode directly
    #[inline]
    pub const fn is_cpu_accessible(&self) -> bool {
        matches!(self, Self::Shared | Self::Managed)
    }

    /// Returns true if this mode is supported on macOS
    #[inline]
    pub const fn is_macos_supported(&self) -> bool {
        // All modes supported on macOS (though Memoryless only on Apple Silicon)
        true
    }

    /// Returns true if this mode is supported on iOS
    #[inline]
    pub const fn is_ios_supported(&self) -> bool {
        // Managed is not supported on iOS
        !matches!(self, Self::Managed)
    }
}

// ============================================================================
// Texture Type (MTLTextureType equivalent)
// ============================================================================

/// Metal texture type enumeration
///
/// Values match Apple's MTLTextureType.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum MTLTextureType {
    /// 1D texture
    Type1D = 0,
    /// 1D texture array
    Type1DArray = 1,
    /// 2D texture
    #[default]
    Type2D = 2,
    /// 2D texture array
    Type2DArray = 3,
    /// 2D multisample texture
    Type2DMultisample = 4,
    /// Cube texture (6 faces)
    TypeCube = 5,
    /// Cube texture array
    TypeCubeArray = 6,
    /// 3D texture
    Type3D = 7,
    /// 2D multisample texture array
    Type2DMultisampleArray = 8,
    /// Texture buffer
    TypeTextureBuffer = 9,
}

impl MTLTextureType {
    /// Returns the number of dimensions for this texture type
    #[inline]
    pub const fn dimensions(&self) -> u8 {
        match self {
            Self::Type1D | Self::Type1DArray | Self::TypeTextureBuffer => 1,
            Self::Type2D
            | Self::Type2DArray
            | Self::Type2DMultisample
            | Self::Type2DMultisampleArray
            | Self::TypeCube
            | Self::TypeCubeArray => 2,
            Self::Type3D => 3,
        }
    }

    /// Returns true if this is an array type
    #[inline]
    pub const fn is_array(&self) -> bool {
        matches!(
            self,
            Self::Type1DArray
                | Self::Type2DArray
                | Self::TypeCubeArray
                | Self::Type2DMultisampleArray
        )
    }

    /// Returns true if this is a multisample type
    #[inline]
    pub const fn is_multisample(&self) -> bool {
        matches!(self, Self::Type2DMultisample | Self::Type2DMultisampleArray)
    }

    /// Returns true if this is a cube type
    #[inline]
    pub const fn is_cube(&self) -> bool {
        matches!(self, Self::TypeCube | Self::TypeCubeArray)
    }
}

// ============================================================================
// GPU Family (MTLGPUFamily equivalent)
// ============================================================================

/// Metal GPU family enumeration
///
/// Identifies the GPU generation and feature set.
/// Values match Apple's MTLGPUFamily.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u32)]
pub enum MTLGPUFamily {
    // ========================================================================
    // Apple GPU families (iOS/iPadOS/tvOS/Apple Silicon Macs)
    // ========================================================================
    /// Apple A7 (iPhone 5s, iPad Air, iPad mini 2)
    Apple1 = 1001,
    /// Apple A8 (iPhone 6, iPad mini 4)
    Apple2 = 1002,
    /// Apple A9 (iPhone 6s, iPad 5th gen)
    Apple3 = 1003,
    /// Apple A10 (iPhone 7)
    Apple4 = 1004,
    /// Apple A11 (iPhone 8/X)
    Apple5 = 1005,
    /// Apple A12 (iPhone XS/XR, iPad Air 3)
    Apple6 = 1006,
    /// Apple A13 (iPhone 11)
    Apple7 = 1007,
    /// Apple A14/M1 (iPhone 12, M1 Macs)
    Apple8 = 1008,
    /// Apple A15/M2 (iPhone 13/14, M2 Macs)
    Apple9 = 1009,

    // ========================================================================
    // Mac GPU families (Intel Macs)
    // ========================================================================
    /// macOS GPU Family 1 (basic Metal support)
    Mac1 = 2001,
    /// macOS GPU Family 2 (advanced features)
    Mac2 = 2002,

    // ========================================================================
    // Common families
    // ========================================================================
    /// Common family 1 (baseline Metal support)
    Common1 = 3001,
    /// Common family 2 (enhanced baseline)
    Common2 = 3002,
    /// Common family 3 (modern baseline)
    Common3 = 3003,

    /// Unknown GPU family
    Unknown = 0,
}

impl MTLGPUFamily {
    /// Returns true if this is an Apple Silicon GPU family
    #[inline]
    pub const fn is_apple_silicon(&self) -> bool {
        matches!(
            self,
            Self::Apple1
                | Self::Apple2
                | Self::Apple3
                | Self::Apple4
                | Self::Apple5
                | Self::Apple6
                | Self::Apple7
                | Self::Apple8
                | Self::Apple9
        )
    }

    /// Returns true if this family supports ray tracing
    #[inline]
    pub const fn supports_raytracing(&self) -> bool {
        matches!(self, Self::Apple6 | Self::Apple7 | Self::Apple8 | Self::Apple9)
    }

    /// Returns true if this family supports mesh shaders
    #[inline]
    pub const fn supports_mesh_shaders(&self) -> bool {
        matches!(self, Self::Apple7 | Self::Apple8 | Self::Apple9)
    }

    /// Returns true if this family has unified memory architecture
    #[inline]
    pub const fn has_unified_memory(&self) -> bool {
        self.is_apple_silicon()
    }

    /// Returns the Metal feature set level (1-3)
    #[inline]
    pub const fn feature_level(&self) -> u8 {
        match self {
            Self::Apple1 | Self::Apple2 | Self::Mac1 | Self::Common1 => 1,
            Self::Apple3 | Self::Apple4 | Self::Apple5 | Self::Mac2 | Self::Common2 => 2,
            Self::Apple6
            | Self::Apple7
            | Self::Apple8
            | Self::Apple9
            | Self::Common3 => 3,
            Self::Unknown => 0,
        }
    }
}

impl Default for MTLGPUFamily {
    fn default() -> Self {
        Self::Unknown
    }
}

// ============================================================================
// Metal Language Version
// ============================================================================

/// Metal Shading Language version
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u32)]
#[allow(non_camel_case_types)]
pub enum MTLLanguageVersion {
    /// Metal 1.0 (iOS 8, macOS 10.11)
    Version1_0 = 0x10000,
    /// Metal 1.1 (iOS 9, macOS 10.11)
    Version1_1 = 0x10001,
    /// Metal 1.2 (iOS 10, macOS 10.12)
    Version1_2 = 0x10002,
    /// Metal 2.0 (iOS 11, macOS 10.13)
    Version2_0 = 0x20000,
    /// Metal 2.1 (iOS 12, macOS 10.14)
    Version2_1 = 0x20001,
    /// Metal 2.2 (iOS 13, macOS 10.15)
    Version2_2 = 0x20002,
    /// Metal 2.3 (iOS 14, macOS 11)
    Version2_3 = 0x20003,
    /// Metal 2.4 (iOS 15, macOS 12)
    Version2_4 = 0x20004,
    /// Metal 3.0 (iOS 16, macOS 13)
    Version3_0 = 0x30000,
    /// Metal 3.1 (iOS 17, macOS 14)
    Version3_1 = 0x30001,
}

impl MTLLanguageVersion {
    /// Returns the major version number
    #[inline]
    pub const fn major(&self) -> u32 {
        (*self as u32) >> 16
    }

    /// Returns the minor version number
    #[inline]
    pub const fn minor(&self) -> u32 {
        (*self as u32) & 0xFFFF
    }
}

impl Default for MTLLanguageVersion {
    fn default() -> Self {
        Self::Version2_4 // Reasonable default for modern systems
    }
}

// ============================================================================
// Texture Usage Flags
// ============================================================================

/// Metal texture usage flags (bitflags)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MTLTextureUsage(pub u32);

impl MTLTextureUsage {
    /// Unknown usage (default)
    pub const UNKNOWN: Self = Self(0);
    /// Texture can be read in shaders
    pub const SHADER_READ: Self = Self(1 << 0);
    /// Texture can be written in shaders
    pub const SHADER_WRITE: Self = Self(1 << 1);
    /// Texture can be used as render target
    pub const RENDER_TARGET: Self = Self(1 << 2);
    /// Texture pixels can be read by CPU
    pub const PIXEL_FORMAT_VIEW: Self = Self(1 << 4);

    /// Check if a usage flag is set
    #[inline]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Combine usage flags
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

// ============================================================================
// Resource Options
// ============================================================================

/// Metal resource options (combines storage mode, CPU cache mode, hazard tracking)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MTLResourceOptions(pub u32);

impl MTLResourceOptions {
    // Storage mode (bits 0-3)
    const STORAGE_MODE_SHIFT: u32 = 0;
    const STORAGE_MODE_MASK: u32 = 0xF;

    // CPU cache mode (bits 4-7)
    const CPU_CACHE_MODE_SHIFT: u32 = 4;
    const CPU_CACHE_MODE_MASK: u32 = 0xF0;

    // Hazard tracking mode (bits 8-9)
    const HAZARD_TRACKING_SHIFT: u32 = 8;
    #[allow(dead_code)]
    const HAZARD_TRACKING_MASK: u32 = 0x300;

    /// Default options (Shared storage, default cache)
    pub const DEFAULT: Self = Self(0);
    /// Shared storage mode
    pub const STORAGE_MODE_SHARED: Self = Self(MTLStorageMode::Shared as u32);
    /// Managed storage mode (macOS only)
    pub const STORAGE_MODE_MANAGED: Self = Self(MTLStorageMode::Managed as u32);
    /// Private storage mode (GPU-only)
    pub const STORAGE_MODE_PRIVATE: Self = Self(MTLStorageMode::Private as u32);
    /// Memoryless storage mode (tile memory only)
    pub const STORAGE_MODE_MEMORYLESS: Self = Self(MTLStorageMode::Memoryless as u32);

    /// Default CPU cache mode
    pub const CPU_CACHE_MODE_DEFAULT: Self = Self(0 << Self::CPU_CACHE_MODE_SHIFT);
    /// Write-combined CPU cache mode (faster writes)
    pub const CPU_CACHE_MODE_WRITE_COMBINED: Self = Self(1 << Self::CPU_CACHE_MODE_SHIFT);

    /// Default hazard tracking
    pub const HAZARD_TRACKING_MODE_DEFAULT: Self = Self(0 << Self::HAZARD_TRACKING_SHIFT);
    /// Untracked hazard mode
    pub const HAZARD_TRACKING_MODE_UNTRACKED: Self = Self(1 << Self::HAZARD_TRACKING_SHIFT);
    /// Tracked hazard mode
    pub const HAZARD_TRACKING_MODE_TRACKED: Self = Self(2 << Self::HAZARD_TRACKING_SHIFT);

    /// Get the storage mode from options
    #[inline]
    pub const fn storage_mode(&self) -> MTLStorageMode {
        match (self.0 & Self::STORAGE_MODE_MASK) >> Self::STORAGE_MODE_SHIFT {
            0 => MTLStorageMode::Shared,
            1 => MTLStorageMode::Managed,
            2 => MTLStorageMode::Private,
            3 => MTLStorageMode::Memoryless,
            _ => MTLStorageMode::Shared,
        }
    }

    /// Get the CPU cache mode from options
    #[inline]
    pub const fn cpu_cache_mode(&self) -> u32 {
        (self.0 & Self::CPU_CACHE_MODE_MASK) >> Self::CPU_CACHE_MODE_SHIFT
    }

    /// Combine options
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

// ============================================================================
// Backend State
// ============================================================================

/// Metal backend state constants
pub mod state {
    /// Backend not initialized
    pub const BACKEND_STATE_UNINITIALIZED: u8 = 0;
    /// Backend initializing
    pub const BACKEND_STATE_INITIALIZING: u8 = 1;
    /// Backend ready for use
    pub const BACKEND_STATE_READY: u8 = 2;
    /// Backend in use (devices active)
    pub const BACKEND_STATE_ACTIVE: u8 = 3;
    /// Backend shutting down
    pub const BACKEND_STATE_SHUTTING_DOWN: u8 = 4;
    /// Backend destroyed
    pub const BACKEND_STATE_DESTROYED: u8 = 5;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_format_bytes_per_pixel() {
        assert_eq!(MTLPixelFormat::Invalid.bytes_per_pixel(), 0);
        assert_eq!(MTLPixelFormat::R8Unorm.bytes_per_pixel(), 1);
        assert_eq!(MTLPixelFormat::R16Float.bytes_per_pixel(), 2);
        assert_eq!(MTLPixelFormat::RGBA8Unorm.bytes_per_pixel(), 4);
        assert_eq!(MTLPixelFormat::BGRA8Unorm.bytes_per_pixel(), 4);
        assert_eq!(MTLPixelFormat::RGBA16Float.bytes_per_pixel(), 8);
        assert_eq!(MTLPixelFormat::RGBA32Float.bytes_per_pixel(), 16);
        assert_eq!(MTLPixelFormat::Depth32Float.bytes_per_pixel(), 4);
    }

    #[test]
    fn test_pixel_format_is_depth() {
        assert!(MTLPixelFormat::Depth16Unorm.is_depth());
        assert!(MTLPixelFormat::Depth32Float.is_depth());
        assert!(MTLPixelFormat::Depth32Float_Stencil8.is_depth());
        assert!(!MTLPixelFormat::RGBA8Unorm.is_depth());
    }

    #[test]
    fn test_pixel_format_is_stencil() {
        assert!(MTLPixelFormat::Stencil8.is_stencil());
        assert!(MTLPixelFormat::Depth24Unorm_Stencil8.is_stencil());
        assert!(MTLPixelFormat::Depth32Float_Stencil8.is_stencil());
        assert!(!MTLPixelFormat::Depth32Float.is_stencil());
    }

    #[test]
    fn test_pixel_format_is_srgb() {
        assert!(MTLPixelFormat::RGBA8Unorm_sRGB.is_srgb());
        assert!(MTLPixelFormat::BGRA8Unorm_sRGB.is_srgb());
        assert!(!MTLPixelFormat::RGBA8Unorm.is_srgb());
    }

    #[test]
    fn test_storage_mode_cpu_accessible() {
        assert!(MTLStorageMode::Shared.is_cpu_accessible());
        assert!(MTLStorageMode::Managed.is_cpu_accessible());
        assert!(!MTLStorageMode::Private.is_cpu_accessible());
        assert!(!MTLStorageMode::Memoryless.is_cpu_accessible());
    }

    #[test]
    fn test_texture_type_dimensions() {
        assert_eq!(MTLTextureType::Type1D.dimensions(), 1);
        assert_eq!(MTLTextureType::Type2D.dimensions(), 2);
        assert_eq!(MTLTextureType::Type3D.dimensions(), 3);
        assert_eq!(MTLTextureType::TypeCube.dimensions(), 2);
    }

    #[test]
    fn test_texture_type_is_array() {
        assert!(MTLTextureType::Type1DArray.is_array());
        assert!(MTLTextureType::Type2DArray.is_array());
        assert!(!MTLTextureType::Type2D.is_array());
    }

    #[test]
    fn test_gpu_family_is_apple_silicon() {
        assert!(MTLGPUFamily::Apple8.is_apple_silicon());
        assert!(MTLGPUFamily::Apple9.is_apple_silicon());
        assert!(!MTLGPUFamily::Mac1.is_apple_silicon());
        assert!(!MTLGPUFamily::Unknown.is_apple_silicon());
    }

    #[test]
    fn test_gpu_family_supports_raytracing() {
        assert!(MTLGPUFamily::Apple6.supports_raytracing());
        assert!(MTLGPUFamily::Apple8.supports_raytracing());
        assert!(!MTLGPUFamily::Apple5.supports_raytracing());
        assert!(!MTLGPUFamily::Mac1.supports_raytracing());
    }

    #[test]
    fn test_gpu_family_feature_level() {
        assert_eq!(MTLGPUFamily::Apple1.feature_level(), 1);
        assert_eq!(MTLGPUFamily::Apple3.feature_level(), 2);
        assert_eq!(MTLGPUFamily::Apple8.feature_level(), 3);
    }

    #[test]
    fn test_language_version() {
        assert_eq!(MTLLanguageVersion::Version2_4.major(), 2);
        assert_eq!(MTLLanguageVersion::Version2_4.minor(), 4);
        assert_eq!(MTLLanguageVersion::Version3_0.major(), 3);
        assert_eq!(MTLLanguageVersion::Version3_0.minor(), 0);
    }

    #[test]
    fn test_texture_usage_flags() {
        let usage = MTLTextureUsage::SHADER_READ.union(MTLTextureUsage::SHADER_WRITE);
        assert!(usage.contains(MTLTextureUsage::SHADER_READ));
        assert!(usage.contains(MTLTextureUsage::SHADER_WRITE));
        assert!(!usage.contains(MTLTextureUsage::RENDER_TARGET));
    }

    #[test]
    fn test_resource_options() {
        let opts = MTLResourceOptions::STORAGE_MODE_PRIVATE
            .union(MTLResourceOptions::CPU_CACHE_MODE_WRITE_COMBINED);
        assert_eq!(opts.storage_mode(), MTLStorageMode::Private);
    }

    #[test]
    fn test_defaults() {
        assert_eq!(MTLPixelFormat::default(), MTLPixelFormat::Invalid);
        assert_eq!(MTLStorageMode::default(), MTLStorageMode::Shared);
        assert_eq!(MTLTextureType::default(), MTLTextureType::Type2D);
        assert_eq!(MTLGPUFamily::default(), MTLGPUFamily::Unknown);
    }
}
