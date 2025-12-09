//! Metal Texture Capsule - T1 Atomic, 256B cache-aligned
//!
//! Represents a Metal GPU texture (MTLTexture) with type and format management.
//! This is a MOCK implementation for design validation, not real Metal FFI.
//!
//! # Design
//!
//! **Tier**: T1 Atomic (<100ns operations)
//! **Size**: 256B cache-aligned (four 64-byte cache lines)
//! **Performance Targets**:
//! - State query: <10ns (single atomic load)
//! - Dimension query: <20ns (atomic loads + unpacking)
//! - View creation: <100ns (CAS + atomic stores)
//!
//! # Memory Layout
//!
//! ```text
//! MtlTextureCapsule (256B, four cache lines)
//! +-- Cache Line 0 (64B): Handle + primary coordination
//! |   +-- handle: AtomicU64 (8B) - Mock MTLTexture handle
//! |   +-- primary: AtomicU64 (8B) - state(8)|texture_type(8)|pixel_format(16)|generation(32)
//! |   +-- _padding0: [u8; 48]
//! +-- Cache Line 1 (64B): Dimensions
//! |   +-- secondary: AtomicU64 (8B) - width(16)|height(16)|depth(16)|mip_levels(8)|array_length(8)
//! |   +-- _padding1: [u8; 56]
//! +-- Cache Line 2 (64B): Properties
//! |   +-- usage: AtomicU32 (4B)
//! |   +-- storage_mode: AtomicU32 (4B)
//! |   +-- sample_count: AtomicU32 (4B)
//! |   +-- view_count: AtomicU32 (4B)
//! |   +-- _padding2: [u8; 48]
//! +-- Cache Line 3 (64B): Metadata
//!     +-- label_hash: AtomicU64 (8B)
//!     +-- parent_texture: AtomicU64 (8B) - Parent if this is a view
//!     +-- _padding3: [u8; 48]
//! ```
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_MOCK_HANDLE`: handle is a mock value, not a real MTLTexture pointer
//! - `#ASSUME_STATE_MACHINE_VALID`: State transitions validated via CAS
//! - `#ASSUME_GENERATION_MONOTONIC`: Generation counter only increases
//! - `#ASSUME_DIMENSIONS_IMMUTABLE`: Dimensions set at creation, never changed
//!
//! # UCE34 Compliance
//!
//! - **Q10**: T1 Atomic tier (lockfree coordination)
//! - **Q33**: 256B alignment verified at compile time
//! - **Q34**: Generation counter enables audit trail integration

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use super::types::{MTLPixelFormat, MTLStorageMode, MTLTextureType, MTLTextureUsage};

// ============================================================================
// State Constants
// ============================================================================

/// Texture state: Uninitialized
pub const TEXTURE_STATE_UNINITIALIZED: u8 = 0;
/// Texture state: Created
pub const TEXTURE_STATE_CREATED: u8 = 1;
/// Texture state: In render pass
pub const TEXTURE_STATE_IN_RENDER_PASS: u8 = 2;
/// Texture state: In compute pass
pub const TEXTURE_STATE_IN_COMPUTE_PASS: u8 = 3;
/// Texture state: Destroyed
pub const TEXTURE_STATE_DESTROYED: u8 = 4;

// ============================================================================
// Bit Field Layouts
// ============================================================================

// Primary atomic: state(8) | texture_type(8) | pixel_format(16) | generation(32)
const STATE_SHIFT: u32 = 56;
const STATE_MASK: u64 = 0xFF << STATE_SHIFT;
const TEXTURE_TYPE_SHIFT: u32 = 48;
const TEXTURE_TYPE_MASK: u64 = 0xFF << TEXTURE_TYPE_SHIFT;
const PIXEL_FORMAT_SHIFT: u32 = 32;
const PIXEL_FORMAT_MASK: u64 = 0xFFFF << PIXEL_FORMAT_SHIFT;
const GENERATION_MASK: u64 = 0x0000_0000_FFFF_FFFF;

// Secondary atomic: width(16) | height(16) | depth(16) | mip_levels(8) | array_length(8)
const WIDTH_SHIFT: u32 = 48;
const WIDTH_MASK: u64 = 0xFFFF << WIDTH_SHIFT;
const HEIGHT_SHIFT: u32 = 32;
const HEIGHT_MASK: u64 = 0xFFFF << HEIGHT_SHIFT;
const DEPTH_SHIFT: u32 = 16;
const DEPTH_MASK: u64 = 0xFFFF << DEPTH_SHIFT;
const MIP_LEVELS_SHIFT: u32 = 8;
const MIP_LEVELS_MASK: u64 = 0xFF << MIP_LEVELS_SHIFT;
const ARRAY_LENGTH_MASK: u64 = 0xFF;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during Metal texture operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MtlTextureError {
    /// Texture is in invalid state for the requested operation
    InvalidState {
        /// Current state
        current: u8,
        /// Expected state
        expected: u8,
    },
    /// State transition failed
    TransitionFailed {
        /// Expected state
        expected: u8,
        /// Observed state
        observed: u8,
    },
    /// Texture has been destroyed
    TextureDestroyed,
    /// Invalid texture dimensions
    InvalidDimensions {
        /// Invalid dimension description
        reason: &'static str,
    },
    /// Texture is in use
    TextureInUse,
    /// Cannot create view from this texture
    CannotCreateView {
        /// Reason
        reason: &'static str,
    },
}

impl core::fmt::Display for MtlTextureError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidState { current, expected } => {
                write!(f, "Invalid texture state: current={}, expected={}", current, expected)
            }
            Self::TransitionFailed { expected, observed } => {
                write!(f, "Texture transition failed: expected={}, observed={}", expected, observed)
            }
            Self::TextureDestroyed => write!(f, "Texture has been destroyed"),
            Self::InvalidDimensions { reason } => {
                write!(f, "Invalid texture dimensions: {}", reason)
            }
            Self::TextureInUse => write!(f, "Texture is in use"),
            Self::CannotCreateView { reason } => {
                write!(f, "Cannot create texture view: {}", reason)
            }
        }
    }
}

/// Result type for Metal texture operations
pub type MtlTextureResult<T> = Result<T, MtlTextureError>;

// ============================================================================
// Texture Descriptor
// ============================================================================

/// Descriptor for creating a texture
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MtlTextureDescriptor {
    /// Texture type
    pub texture_type: MTLTextureType,
    /// Pixel format
    pub pixel_format: MTLPixelFormat,
    /// Width in pixels
    pub width: u16,
    /// Height in pixels
    pub height: u16,
    /// Depth in pixels (for 3D textures)
    pub depth: u16,
    /// Number of mipmap levels
    pub mip_levels: u8,
    /// Array length (for array textures)
    pub array_length: u8,
    /// Sample count (for MSAA)
    pub sample_count: u32,
    /// Usage flags
    pub usage: MTLTextureUsage,
    /// Storage mode
    pub storage_mode: MTLStorageMode,
}

impl Default for MtlTextureDescriptor {
    fn default() -> Self {
        Self {
            texture_type: MTLTextureType::Type2D,
            pixel_format: MTLPixelFormat::RGBA8Unorm,
            width: 1,
            height: 1,
            depth: 1,
            mip_levels: 1,
            array_length: 1,
            sample_count: 1,
            usage: MTLTextureUsage::SHADER_READ,
            storage_mode: MTLStorageMode::Private,
        }
    }
}

impl MtlTextureDescriptor {
    /// Creates a 2D texture descriptor
    pub fn texture_2d(width: u16, height: u16, format: MTLPixelFormat) -> Self {
        Self {
            texture_type: MTLTextureType::Type2D,
            pixel_format: format,
            width,
            height,
            depth: 1,
            mip_levels: 1,
            array_length: 1,
            sample_count: 1,
            usage: MTLTextureUsage::SHADER_READ,
            storage_mode: MTLStorageMode::Private,
        }
    }

    /// Creates a render target descriptor
    pub fn render_target(width: u16, height: u16, format: MTLPixelFormat) -> Self {
        Self {
            texture_type: MTLTextureType::Type2D,
            pixel_format: format,
            width,
            height,
            depth: 1,
            mip_levels: 1,
            array_length: 1,
            sample_count: 1,
            usage: MTLTextureUsage::RENDER_TARGET.union(MTLTextureUsage::SHADER_READ),
            storage_mode: MTLStorageMode::Private,
        }
    }

    /// Creates a depth texture descriptor
    pub fn depth(width: u16, height: u16) -> Self {
        Self {
            texture_type: MTLTextureType::Type2D,
            pixel_format: MTLPixelFormat::Depth32Float,
            width,
            height,
            depth: 1,
            mip_levels: 1,
            array_length: 1,
            sample_count: 1,
            usage: MTLTextureUsage::RENDER_TARGET,
            storage_mode: MTLStorageMode::Private,
        }
    }
}

// ============================================================================
// Texture Snapshot
// ============================================================================

/// Atomic snapshot of texture state for debugging/monitoring
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MtlTextureSnapshot {
    /// Mock texture handle
    pub handle: u64,
    /// Current state
    pub state: u8,
    /// Texture type
    pub texture_type: MTLTextureType,
    /// Pixel format
    pub pixel_format: MTLPixelFormat,
    /// Generation counter
    pub generation: u32,
    /// Width
    pub width: u16,
    /// Height
    pub height: u16,
    /// Depth
    pub depth: u16,
    /// Mip levels
    pub mip_levels: u8,
    /// Array length
    pub array_length: u8,
    /// Usage flags
    pub usage: MTLTextureUsage,
    /// Storage mode
    pub storage_mode: MTLStorageMode,
    /// Sample count
    pub sample_count: u32,
    /// Number of views
    pub view_count: u32,
}

// ============================================================================
// MtlTextureCapsule
// ============================================================================

/// Metal Texture Capsule - Represents a Metal GPU texture
///
/// Manages texture state, dimensions, and view creation.
/// All operations are lockfree using atomic primitives.
///
/// # Tier: T1 Atomic
/// # Size: 256B (four cache lines, prevents false sharing)
///
/// # State Machine
///
/// - `Uninitialized` (0): Texture not yet created
/// - `Created` (1): Texture created, available for use
/// - `InRenderPass` (2): Texture bound to render pass
/// - `InComputePass` (3): Texture bound to compute pass
/// - `Destroyed` (4): Texture destroyed
///
/// # ASSUM Safety
///
/// - `#ASSUME_MOCK_HANDLE`: handle is mock, not real MTLTexture
/// - `#ASSUME_STATE_MACHINE_VALID`: State transitions validated via CAS
/// - `#ASSUME_DIMENSIONS_IMMUTABLE`: Dimensions set at creation
#[repr(C, align(256))]
pub struct MtlTextureCapsule {
    // ========================================================================
    // Cache Line 0: Handle + primary coordination
    // ========================================================================
    /// Mock MTLTexture handle
    ///
    /// #ASSUME_MOCK_HANDLE: This is a mock value for testing.
    handle: AtomicU64,

    /// Primary coordination channel
    ///
    /// Layout: state(8) | texture_type(8) | pixel_format(16) | generation(32)
    primary: AtomicU64,

    /// Padding to complete first cache line
    _padding0: [u8; 48],

    // ========================================================================
    // Cache Line 1: Dimensions
    // ========================================================================
    /// Secondary channel (dimensions)
    ///
    /// Layout: width(16) | height(16) | depth(16) | mip_levels(8) | array_length(8)
    secondary: AtomicU64,

    /// Padding to complete second cache line
    _padding1: [u8; 56],

    // ========================================================================
    // Cache Line 2: Properties
    // ========================================================================
    /// Usage flags
    usage: AtomicU32,

    /// Storage mode
    storage_mode: AtomicU32,

    /// Sample count (for MSAA)
    sample_count: AtomicU32,

    /// Number of texture views created from this texture
    view_count: AtomicU32,

    /// Padding to complete third cache line
    _padding2: [u8; 48],

    // ========================================================================
    // Cache Line 3: Metadata
    // ========================================================================
    /// Hash of texture label (for debugging)
    label_hash: AtomicU64,

    /// Parent texture handle (if this is a view)
    parent_texture: AtomicU64,

    /// Padding to complete fourth cache line
    _padding3: [u8; 48],
}

// Compile-time size and alignment verification
const _: () = {
    assert!(core::mem::size_of::<MtlTextureCapsule>() == 256);
    assert!(core::mem::align_of::<MtlTextureCapsule>() == 256);
};

impl MtlTextureCapsule {
    /// Creates a new texture in `Uninitialized` state.
    ///
    /// # Performance
    ///
    /// O(1), ~10ns (stack allocation + atomic init)
    #[inline]
    pub const fn new() -> Self {
        Self {
            handle: AtomicU64::new(0),
            primary: AtomicU64::new(0),
            _padding0: [0u8; 48],

            secondary: AtomicU64::new(0),
            _padding1: [0u8; 56],

            usage: AtomicU32::new(0),
            storage_mode: AtomicU32::new(0),
            sample_count: AtomicU32::new(1),
            view_count: AtomicU32::new(0),
            _padding2: [0u8; 48],

            label_hash: AtomicU64::new(0),
            parent_texture: AtomicU64::new(0),
            _padding3: [0u8; 48],
        }
    }

    /// Returns the mock texture handle.
    #[inline]
    pub fn handle(&self) -> u64 {
        self.handle.load(Ordering::Acquire)
    }

    /// Returns the current state.
    #[inline]
    pub fn state(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & STATE_MASK) >> STATE_SHIFT) as u8
    }

    /// Returns the texture type.
    #[inline]
    pub fn texture_type(&self) -> MTLTextureType {
        let primary = self.primary.load(Ordering::Acquire);
        let type_val = ((primary & TEXTURE_TYPE_MASK) >> TEXTURE_TYPE_SHIFT) as u32;
        match type_val {
            0 => MTLTextureType::Type1D,
            1 => MTLTextureType::Type1DArray,
            2 => MTLTextureType::Type2D,
            3 => MTLTextureType::Type2DArray,
            4 => MTLTextureType::Type2DMultisample,
            5 => MTLTextureType::TypeCube,
            6 => MTLTextureType::TypeCubeArray,
            7 => MTLTextureType::Type3D,
            _ => MTLTextureType::Type2D,
        }
    }

    /// Returns the pixel format.
    #[inline]
    pub fn pixel_format(&self) -> MTLPixelFormat {
        let primary = self.primary.load(Ordering::Acquire);
        let format_val = ((primary & PIXEL_FORMAT_MASK) >> PIXEL_FORMAT_SHIFT) as u32;
        // Match common formats
        match format_val {
            70 => MTLPixelFormat::RGBA8Unorm,
            71 => MTLPixelFormat::RGBA8Unorm_sRGB,
            80 => MTLPixelFormat::BGRA8Unorm,
            81 => MTLPixelFormat::BGRA8Unorm_sRGB,
            115 => MTLPixelFormat::RGBA16Float,
            125 => MTLPixelFormat::RGBA32Float,
            252 => MTLPixelFormat::Depth32Float,
            260 => MTLPixelFormat::Depth32Float_Stencil8,
            _ => MTLPixelFormat::Invalid,
        }
    }

    /// Returns the generation counter.
    #[inline]
    pub fn generation(&self) -> u32 {
        let primary = self.primary.load(Ordering::Acquire);
        (primary & GENERATION_MASK) as u32
    }

    /// Returns the texture width.
    #[inline]
    pub fn width(&self) -> u16 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & WIDTH_MASK) >> WIDTH_SHIFT) as u16
    }

    /// Returns the texture height.
    #[inline]
    pub fn height(&self) -> u16 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & HEIGHT_MASK) >> HEIGHT_SHIFT) as u16
    }

    /// Returns the texture depth.
    #[inline]
    pub fn depth(&self) -> u16 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & DEPTH_MASK) >> DEPTH_SHIFT) as u16
    }

    /// Returns the mipmap level count.
    #[inline]
    pub fn mip_levels(&self) -> u8 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & MIP_LEVELS_MASK) >> MIP_LEVELS_SHIFT) as u8
    }

    /// Returns the array length.
    #[inline]
    pub fn array_length(&self) -> u8 {
        let secondary = self.secondary.load(Ordering::Acquire);
        (secondary & ARRAY_LENGTH_MASK) as u8
    }

    /// Returns the usage flags.
    #[inline]
    pub fn usage(&self) -> MTLTextureUsage {
        MTLTextureUsage(self.usage.load(Ordering::Acquire))
    }

    /// Returns the storage mode.
    #[inline]
    pub fn storage_mode(&self) -> MTLStorageMode {
        match self.storage_mode.load(Ordering::Acquire) {
            0 => MTLStorageMode::Shared,
            1 => MTLStorageMode::Managed,
            2 => MTLStorageMode::Private,
            3 => MTLStorageMode::Memoryless,
            _ => MTLStorageMode::Private,
        }
    }

    /// Returns the sample count.
    #[inline]
    pub fn sample_count(&self) -> u32 {
        self.sample_count.load(Ordering::Acquire)
    }

    /// Returns the view count.
    #[inline]
    pub fn view_count(&self) -> u32 {
        self.view_count.load(Ordering::Acquire)
    }

    /// Takes an atomic snapshot of the texture state.
    ///
    /// # Performance
    ///
    /// ~50ns (multiple atomic loads)
    pub fn snapshot(&self) -> MtlTextureSnapshot {
        let primary = self.primary.load(Ordering::Acquire);
        let secondary = self.secondary.load(Ordering::Acquire);

        MtlTextureSnapshot {
            handle: self.handle.load(Ordering::Acquire),
            state: ((primary & STATE_MASK) >> STATE_SHIFT) as u8,
            texture_type: self.texture_type(),
            pixel_format: self.pixel_format(),
            generation: (primary & GENERATION_MASK) as u32,
            width: ((secondary & WIDTH_MASK) >> WIDTH_SHIFT) as u16,
            height: ((secondary & HEIGHT_MASK) >> HEIGHT_SHIFT) as u16,
            depth: ((secondary & DEPTH_MASK) >> DEPTH_SHIFT) as u16,
            mip_levels: ((secondary & MIP_LEVELS_MASK) >> MIP_LEVELS_SHIFT) as u8,
            array_length: (secondary & ARRAY_LENGTH_MASK) as u8,
            usage: MTLTextureUsage(self.usage.load(Ordering::Acquire)),
            storage_mode: self.storage_mode(),
            sample_count: self.sample_count.load(Ordering::Acquire),
            view_count: self.view_count.load(Ordering::Acquire),
        }
    }

    /// Creates the texture with the specified descriptor.
    ///
    /// # Performance
    ///
    /// <100ns (CAS + atomic stores)
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_STATE_MACHINE_VALID`: Validates transition is legal
    /// - `#ASSUME_DIMENSIONS_IMMUTABLE`: Dimensions set here, never changed
    pub fn create(&self, mock_handle: u64, desc: MtlTextureDescriptor) -> MtlTextureResult<()> {
        // Validate dimensions
        if desc.width == 0 {
            return Err(MtlTextureError::InvalidDimensions {
                reason: "width cannot be 0",
            });
        }
        if desc.height == 0 {
            return Err(MtlTextureError::InvalidDimensions {
                reason: "height cannot be 0",
            });
        }

        // #ASSUME_STATE_MACHINE_VALID: Transition from Uninitialized to Created
        let current = self.primary.load(Ordering::Acquire);
        let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

        if current_state != TEXTURE_STATE_UNINITIALIZED {
            return Err(MtlTextureError::InvalidState {
                current: current_state,
                expected: TEXTURE_STATE_UNINITIALIZED,
            });
        }

        // Build new primary value
        let current_gen = (current & GENERATION_MASK) as u32;
        let new_gen = current_gen.wrapping_add(1);
        let new_primary = ((TEXTURE_STATE_CREATED as u64) << STATE_SHIFT)
            | ((desc.texture_type as u64) << TEXTURE_TYPE_SHIFT)
            | ((desc.pixel_format as u64) << PIXEL_FORMAT_SHIFT)
            | (new_gen as u64);

        match self.primary.compare_exchange(
            current,
            new_primary,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {}
            Err(observed) => {
                let observed_state = ((observed & STATE_MASK) >> STATE_SHIFT) as u8;
                return Err(MtlTextureError::TransitionFailed {
                    expected: TEXTURE_STATE_UNINITIALIZED,
                    observed: observed_state,
                });
            }
        }

        // Set handle
        self.handle.store(mock_handle, Ordering::Release);

        // #ASSUME_DIMENSIONS_IMMUTABLE: Set dimensions
        let secondary_value = ((desc.width as u64) << WIDTH_SHIFT)
            | ((desc.height as u64) << HEIGHT_SHIFT)
            | ((desc.depth as u64) << DEPTH_SHIFT)
            | ((desc.mip_levels as u64) << MIP_LEVELS_SHIFT)
            | (desc.array_length as u64);
        self.secondary.store(secondary_value, Ordering::Release);

        // Set properties
        self.usage.store(desc.usage.0, Ordering::Release);
        self.storage_mode.store(desc.storage_mode as u32, Ordering::Release);
        self.sample_count.store(desc.sample_count, Ordering::Release);

        Ok(())
    }

    /// Marks the texture as in render pass.
    ///
    /// # Performance
    ///
    /// <50ns (CAS)
    pub fn begin_render_pass(&self) -> MtlTextureResult<()> {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

            match current_state {
                TEXTURE_STATE_DESTROYED => return Err(MtlTextureError::TextureDestroyed),
                TEXTURE_STATE_IN_RENDER_PASS | TEXTURE_STATE_IN_COMPUTE_PASS => {
                    return Err(MtlTextureError::TextureInUse);
                }
                TEXTURE_STATE_CREATED => {
                    let current_gen = (current & GENERATION_MASK) as u32;
                    let new_gen = current_gen.wrapping_add(1);
                    let new_primary = (current & !STATE_MASK & !GENERATION_MASK)
                        | ((TEXTURE_STATE_IN_RENDER_PASS as u64) << STATE_SHIFT)
                        | (new_gen as u64);

                    if self
                        .primary
                        .compare_exchange(current, new_primary, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        return Ok(());
                    }
                }
                _ => {
                    return Err(MtlTextureError::InvalidState {
                        current: current_state,
                        expected: TEXTURE_STATE_CREATED,
                    });
                }
            }
        }
    }

    /// Marks the texture as in compute pass.
    ///
    /// # Performance
    ///
    /// <50ns (CAS)
    pub fn begin_compute_pass(&self) -> MtlTextureResult<()> {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

            match current_state {
                TEXTURE_STATE_DESTROYED => return Err(MtlTextureError::TextureDestroyed),
                TEXTURE_STATE_IN_RENDER_PASS | TEXTURE_STATE_IN_COMPUTE_PASS => {
                    return Err(MtlTextureError::TextureInUse);
                }
                TEXTURE_STATE_CREATED => {
                    let current_gen = (current & GENERATION_MASK) as u32;
                    let new_gen = current_gen.wrapping_add(1);
                    let new_primary = (current & !STATE_MASK & !GENERATION_MASK)
                        | ((TEXTURE_STATE_IN_COMPUTE_PASS as u64) << STATE_SHIFT)
                        | (new_gen as u64);

                    if self
                        .primary
                        .compare_exchange(current, new_primary, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        return Ok(());
                    }
                }
                _ => {
                    return Err(MtlTextureError::InvalidState {
                        current: current_state,
                        expected: TEXTURE_STATE_CREATED,
                    });
                }
            }
        }
    }

    /// Ends pass usage and returns to Created state.
    ///
    /// # Performance
    ///
    /// <50ns (CAS)
    pub fn end_pass(&self) -> MtlTextureResult<()> {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

            if current_state == TEXTURE_STATE_DESTROYED {
                return Err(MtlTextureError::TextureDestroyed);
            }

            if current_state != TEXTURE_STATE_IN_RENDER_PASS
                && current_state != TEXTURE_STATE_IN_COMPUTE_PASS
            {
                return Ok(()); // Not in a pass
            }

            let current_gen = (current & GENERATION_MASK) as u32;
            let new_gen = current_gen.wrapping_add(1);
            let new_primary = (current & !STATE_MASK & !GENERATION_MASK)
                | ((TEXTURE_STATE_CREATED as u64) << STATE_SHIFT)
                | (new_gen as u64);

            if self
                .primary
                .compare_exchange(current, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(());
            }
        }
    }

    /// Creates a texture view (increments view count).
    ///
    /// # Returns
    ///
    /// Mock view handle on success.
    pub fn create_view(&self) -> MtlTextureResult<u64> {
        let state = self.state();
        if state == TEXTURE_STATE_DESTROYED {
            return Err(MtlTextureError::TextureDestroyed);
        }
        if state == TEXTURE_STATE_UNINITIALIZED {
            return Err(MtlTextureError::InvalidState {
                current: state,
                expected: TEXTURE_STATE_CREATED,
            });
        }

        let view_num = self.view_count.fetch_add(1, Ordering::AcqRel);
        let view_handle = (self.handle() & 0xFFFF_FFFF_0000_0000) | (view_num as u64);
        Ok(view_handle)
    }

    /// Destroys the texture.
    ///
    /// # Performance
    ///
    /// <50ns (CAS)
    pub fn destroy(&self) -> MtlTextureResult<()> {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

            if current_state == TEXTURE_STATE_DESTROYED {
                return Err(MtlTextureError::TextureDestroyed);
            }

            if current_state == TEXTURE_STATE_IN_RENDER_PASS
                || current_state == TEXTURE_STATE_IN_COMPUTE_PASS
            {
                return Err(MtlTextureError::TextureInUse);
            }

            let current_gen = (current & GENERATION_MASK) as u32;
            let new_gen = current_gen.wrapping_add(1);
            let destroyed = ((TEXTURE_STATE_DESTROYED as u64) << STATE_SHIFT) | (new_gen as u64);

            if self
                .primary
                .compare_exchange(current, destroyed, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(());
            }
        }
    }

    /// Checks if the texture is valid.
    #[inline]
    pub fn is_valid(&self) -> bool {
        let state = self.state();
        state != TEXTURE_STATE_DESTROYED && state != TEXTURE_STATE_UNINITIALIZED
    }

    /// Checks if the texture is in use.
    #[inline]
    pub fn is_in_use(&self) -> bool {
        let state = self.state();
        state == TEXTURE_STATE_IN_RENDER_PASS || state == TEXTURE_STATE_IN_COMPUTE_PASS
    }

    /// Calculates the estimated memory size of the texture.
    pub fn estimated_size(&self) -> u64 {
        let bpp = self.pixel_format().bytes_per_pixel() as u64;
        if bpp == 0 {
            // Compressed format
            return (self.width() as u64 * self.height() as u64 * self.depth() as u64) / 2;
        }
        self.width() as u64 * self.height() as u64 * self.depth() as u64 * bpp
    }

    /// Sets the label hash for debugging.
    #[inline]
    pub fn set_label_hash(&self, hash: u64) {
        self.label_hash.store(hash, Ordering::Release);
    }

    /// Gets the label hash.
    #[inline]
    pub fn label_hash(&self) -> u64 {
        self.label_hash.load(Ordering::Acquire)
    }
}

impl Default for MtlTextureCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for MtlTextureCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let snapshot = self.snapshot();
        f.debug_struct("MtlTextureCapsule")
            .field("handle", &format_args!("{:#018x}", snapshot.handle))
            .field("state", &snapshot.state)
            .field("type", &snapshot.texture_type)
            .field("format", &snapshot.pixel_format)
            .field("size", &format_args!("{}x{}x{}", snapshot.width, snapshot.height, snapshot.depth))
            .field("mips", &snapshot.mip_levels)
            .finish()
    }
}

// SAFETY: All operations are atomic; no mutable aliasing possible
unsafe impl Send for MtlTextureCapsule {}
unsafe impl Sync for MtlTextureCapsule {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<MtlTextureCapsule>(), 256);
        assert_eq!(core::mem::align_of::<MtlTextureCapsule>(), 256);
    }

    #[test]
    fn test_initial_state() {
        let texture = MtlTextureCapsule::new();
        assert_eq!(texture.state(), TEXTURE_STATE_UNINITIALIZED);
        assert_eq!(texture.handle(), 0);
        assert_eq!(texture.width(), 0);
        assert_eq!(texture.height(), 0);
        assert_eq!(texture.view_count(), 0);
    }

    #[test]
    fn test_create_2d() {
        let texture = MtlTextureCapsule::new();
        let desc = MtlTextureDescriptor::texture_2d(1024, 768, MTLPixelFormat::RGBA8Unorm);

        texture.create(0x1234, desc).expect("Create failed");

        assert_eq!(texture.state(), TEXTURE_STATE_CREATED);
        assert_eq!(texture.handle(), 0x1234);
        assert_eq!(texture.width(), 1024);
        assert_eq!(texture.height(), 768);
        assert_eq!(texture.depth(), 1);
        assert_eq!(texture.texture_type(), MTLTextureType::Type2D);
        assert!(texture.is_valid());
    }

    #[test]
    fn test_create_render_target() {
        let texture = MtlTextureCapsule::new();
        let desc = MtlTextureDescriptor::render_target(1920, 1080, MTLPixelFormat::BGRA8Unorm);

        texture.create(0x5678, desc).expect("Create failed");

        assert!(texture.usage().contains(MTLTextureUsage::RENDER_TARGET));
        assert!(texture.usage().contains(MTLTextureUsage::SHADER_READ));
    }

    #[test]
    fn test_create_depth() {
        let texture = MtlTextureCapsule::new();
        let desc = MtlTextureDescriptor::depth(1920, 1080);

        texture.create(0x9ABC, desc).expect("Create failed");

        assert_eq!(texture.pixel_format(), MTLPixelFormat::Depth32Float);
    }

    #[test]
    fn test_create_zero_width_fails() {
        let texture = MtlTextureCapsule::new();
        let mut desc = MtlTextureDescriptor::default();
        desc.width = 0;

        let result = texture.create(0x1234, desc);
        assert!(matches!(result, Err(MtlTextureError::InvalidDimensions { .. })));
    }

    #[test]
    fn test_double_create_fails() {
        let texture = MtlTextureCapsule::new();
        let desc = MtlTextureDescriptor::texture_2d(256, 256, MTLPixelFormat::RGBA8Unorm);

        texture.create(0x1234, desc).unwrap();
        let result = texture.create(0x5678, desc);
        assert!(result.is_err());
    }

    #[test]
    fn test_render_pass() {
        let texture = MtlTextureCapsule::new();
        let desc = MtlTextureDescriptor::render_target(1920, 1080, MTLPixelFormat::BGRA8Unorm);
        texture.create(0x1234, desc).unwrap();

        texture.begin_render_pass().expect("Begin render pass failed");
        assert!(texture.is_in_use());
        assert_eq!(texture.state(), TEXTURE_STATE_IN_RENDER_PASS);

        // Cannot begin another pass while in use
        let result = texture.begin_compute_pass();
        assert!(matches!(result, Err(MtlTextureError::TextureInUse)));

        texture.end_pass().expect("End pass failed");
        assert!(!texture.is_in_use());
        assert_eq!(texture.state(), TEXTURE_STATE_CREATED);
    }

    #[test]
    fn test_compute_pass() {
        let texture = MtlTextureCapsule::new();
        let desc = MtlTextureDescriptor::texture_2d(512, 512, MTLPixelFormat::RGBA32Float);
        texture.create(0x1234, desc).unwrap();

        texture.begin_compute_pass().expect("Begin compute pass failed");
        assert_eq!(texture.state(), TEXTURE_STATE_IN_COMPUTE_PASS);

        texture.end_pass().expect("End pass failed");
        assert_eq!(texture.state(), TEXTURE_STATE_CREATED);
    }

    #[test]
    fn test_create_view() {
        let texture = MtlTextureCapsule::new();
        let desc = MtlTextureDescriptor::texture_2d(256, 256, MTLPixelFormat::RGBA8Unorm);
        texture.create(0x1234_0000_0000_0000u64, desc).unwrap();

        let view1 = texture.create_view().expect("View 1 failed");
        assert_eq!(texture.view_count(), 1);
        assert_ne!(view1, 0);

        let view2 = texture.create_view().expect("View 2 failed");
        assert_eq!(texture.view_count(), 2);
        assert_ne!(view2, view1);
    }

    #[test]
    fn test_destroy() {
        let texture = MtlTextureCapsule::new();
        let desc = MtlTextureDescriptor::texture_2d(256, 256, MTLPixelFormat::RGBA8Unorm);
        texture.create(0x1234, desc).unwrap();

        texture.destroy().expect("Destroy failed");
        assert_eq!(texture.state(), TEXTURE_STATE_DESTROYED);
        assert!(!texture.is_valid());
    }

    #[test]
    fn test_destroy_in_use_fails() {
        let texture = MtlTextureCapsule::new();
        let desc = MtlTextureDescriptor::render_target(256, 256, MTLPixelFormat::BGRA8Unorm);
        texture.create(0x1234, desc).unwrap();

        texture.begin_render_pass().unwrap();
        let result = texture.destroy();
        assert!(matches!(result, Err(MtlTextureError::TextureInUse)));

        texture.end_pass().unwrap();
        texture.destroy().expect("Destroy should succeed now");
    }

    #[test]
    fn test_estimated_size() {
        let texture = MtlTextureCapsule::new();
        let desc = MtlTextureDescriptor::texture_2d(1024, 1024, MTLPixelFormat::RGBA8Unorm);
        texture.create(0x1234, desc).unwrap();

        // 1024 * 1024 * 4 = 4MB
        assert_eq!(texture.estimated_size(), 4 * 1024 * 1024);
    }

    #[test]
    fn test_snapshot() {
        let texture = MtlTextureCapsule::new();
        let desc = MtlTextureDescriptor {
            texture_type: MTLTextureType::Type2D,
            pixel_format: MTLPixelFormat::RGBA8Unorm,
            width: 512,
            height: 256,
            depth: 1,
            mip_levels: 4,
            array_length: 1,
            sample_count: 1,
            usage: MTLTextureUsage::SHADER_READ,
            storage_mode: MTLStorageMode::Private,
        };
        texture.create(0x1234, desc).unwrap();

        let snapshot = texture.snapshot();
        assert_eq!(snapshot.handle, 0x1234);
        assert_eq!(snapshot.state, TEXTURE_STATE_CREATED);
        assert_eq!(snapshot.width, 512);
        assert_eq!(snapshot.height, 256);
        assert_eq!(snapshot.mip_levels, 4);
    }

    #[test]
    fn test_generation_increments() {
        let texture = MtlTextureCapsule::new();
        let gen0 = texture.generation();

        let desc = MtlTextureDescriptor::texture_2d(256, 256, MTLPixelFormat::RGBA8Unorm);
        texture.create(0x1234, desc).unwrap();
        let gen1 = texture.generation();
        assert!(gen1 > gen0);

        texture.begin_render_pass().unwrap();
        let gen2 = texture.generation();
        assert!(gen2 > gen1);

        texture.end_pass().unwrap();
        let gen3 = texture.generation();
        assert!(gen3 > gen2);
    }

    #[test]
    fn test_label_hash() {
        let texture = MtlTextureCapsule::new();
        let desc = MtlTextureDescriptor::texture_2d(256, 256, MTLPixelFormat::RGBA8Unorm);
        texture.create(0x1234, desc).unwrap();

        texture.set_label_hash(0xCAFE_BABE);
        assert_eq!(texture.label_hash(), 0xCAFE_BABE);
    }

    #[test]
    fn test_debug_format() {
        let texture = MtlTextureCapsule::new();
        let desc = MtlTextureDescriptor::texture_2d(256, 256, MTLPixelFormat::RGBA8Unorm);
        texture.create(0x1234, desc).unwrap();

        let debug_str = format!("{:?}", texture);
        assert!(debug_str.contains("MtlTextureCapsule"));
        assert!(debug_str.contains("256x256"));
    }

    #[test]
    fn test_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let texture = Arc::new(MtlTextureCapsule::new());
        let desc = MtlTextureDescriptor::texture_2d(256, 256, MTLPixelFormat::RGBA8Unorm);
        texture.create(0x1234, desc).unwrap();

        let mut handles = vec![];

        // Spawn readers
        for _ in 0..4 {
            let tex = Arc::clone(&texture);
            handles.push(thread::spawn(move || {
                for _ in 0..500 {
                    let _ = tex.snapshot();
                    let _ = tex.state();
                    let _ = tex.width();
                    let _ = tex.estimated_size();
                }
            }));
        }

        // Spawn view creators
        for _ in 0..2 {
            let tex = Arc::clone(&texture);
            handles.push(thread::spawn(move || {
                for _ in 0..50 {
                    let _ = tex.create_view();
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        assert!(texture.is_valid());
        assert!(texture.view_count() > 0);
    }
}
