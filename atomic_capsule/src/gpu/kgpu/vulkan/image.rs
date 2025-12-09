//! VkImageCapsule - Vulkan Image (Mock)
//!
//! **Tier**: T1 Atomic
//! **Size**: 256B cache-aligned
//! **Purpose**: Mock Vulkan image for design validation
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_MOCK_IMAGE`: This is a mock, not real Vulkan FFI
//! - `#ASSUME_HANDLE_VALID`: Mock handles are always "valid" (non-zero)
//! - `#ASSUME_STATE_ATOMIC`: All state changes use atomic operations
//! - `#ASSUME_LAYOUT_TRACKING`: Image layout tracked atomically

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use super::types::{
    generate_mock_handle, VkFormat, VkImageLayout, VkImageTiling, VkImageUsageFlags,
    VkMemoryPropertyFlags, VkResult, VkSampleCountFlags,
};

// ============================================================================
// State Constants
// ============================================================================

/// Image is not initialized
pub const VK_IMAGE_STATE_UNINITIALIZED: u8 = 0;
/// Image is created but not bound
pub const VK_IMAGE_STATE_CREATED: u8 = 1;
/// Image is bound to memory
pub const VK_IMAGE_STATE_BOUND: u8 = 2;
/// Image is in transition
pub const VK_IMAGE_STATE_TRANSITIONING: u8 = 3;
/// Image is ready for use
pub const VK_IMAGE_STATE_READY: u8 = 4;
/// Image has been destroyed
pub const VK_IMAGE_STATE_DESTROYED: u8 = 5;

/// Image state enum for type-safe state management
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VkImageState {
    /// Image is not initialized
    Uninitialized = VK_IMAGE_STATE_UNINITIALIZED,
    /// Image is created but not bound
    Created = VK_IMAGE_STATE_CREATED,
    /// Image is bound to memory
    Bound = VK_IMAGE_STATE_BOUND,
    /// Image is in transition
    Transitioning = VK_IMAGE_STATE_TRANSITIONING,
    /// Image is ready for use
    Ready = VK_IMAGE_STATE_READY,
    /// Image has been destroyed
    Destroyed = VK_IMAGE_STATE_DESTROYED,
}

impl From<u8> for VkImageState {
    fn from(value: u8) -> Self {
        match value {
            VK_IMAGE_STATE_UNINITIALIZED => VkImageState::Uninitialized,
            VK_IMAGE_STATE_CREATED => VkImageState::Created,
            VK_IMAGE_STATE_BOUND => VkImageState::Bound,
            VK_IMAGE_STATE_TRANSITIONING => VkImageState::Transitioning,
            VK_IMAGE_STATE_READY => VkImageState::Ready,
            VK_IMAGE_STATE_DESTROYED => VkImageState::Destroyed,
            _ => VkImageState::Uninitialized,
        }
    }
}

impl VkImageState {
    /// Get the raw u8 value
    #[inline]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

// ============================================================================
// Bit Field Layouts
// ============================================================================

// Primary: state(8) | format(8) | layout(8) | generation(40)
const STATE_SHIFT: u64 = 56;
const STATE_MASK: u64 = 0xFF << STATE_SHIFT;
const FORMAT_SHIFT: u64 = 48;
const FORMAT_MASK: u64 = 0xFF << FORMAT_SHIFT;
const LAYOUT_SHIFT: u64 = 40;
const LAYOUT_MASK: u64 = 0xFF << LAYOUT_SHIFT;
const GENERATION_MASK: u64 = 0x00_00_00_FF_FFFF_FFFF;

// Secondary: width(16) | height(16) | depth(16) | mip_levels(8) | array_layers(8)
const WIDTH_SHIFT: u64 = 48;
const WIDTH_MASK: u64 = 0xFFFF << WIDTH_SHIFT;
const HEIGHT_SHIFT: u64 = 32;
const HEIGHT_MASK: u64 = 0xFFFF << HEIGHT_SHIFT;
const DEPTH_SHIFT: u64 = 16;
const DEPTH_MASK: u64 = 0xFFFF << DEPTH_SHIFT;
const MIP_SHIFT: u64 = 8;
const MIP_MASK: u64 = 0xFF << MIP_SHIFT;
const ARRAY_MASK: u64 = 0xFF;

// ============================================================================
// Create Info
// ============================================================================

/// Image creation parameters
#[derive(Debug, Clone)]
pub struct VkImageCreateInfo {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Depth (1 for 2D images)
    pub depth: u32,
    /// Number of mip levels
    pub mip_levels: u32,
    /// Number of array layers
    pub array_layers: u32,
    /// Image format
    pub format: VkFormat,
    /// Tiling mode
    pub tiling: VkImageTiling,
    /// Initial layout
    pub initial_layout: VkImageLayout,
    /// Usage flags
    pub usage: VkImageUsageFlags,
    /// Sample count
    pub samples: VkSampleCountFlags,
    /// Memory properties
    pub memory_properties: VkMemoryPropertyFlags,
}

impl Default for VkImageCreateInfo {
    fn default() -> Self {
        Self {
            width: 1,
            height: 1,
            depth: 1,
            mip_levels: 1,
            array_layers: 1,
            format: VkFormat::R8G8B8A8Unorm,
            tiling: VkImageTiling::Optimal,
            initial_layout: VkImageLayout::Undefined,
            usage: VkImageUsageFlags::SAMPLED | VkImageUsageFlags::TRANSFER_DST,
            samples: VkSampleCountFlags::Count1,
            memory_properties: VkMemoryPropertyFlags::DEVICE_LOCAL,
        }
    }
}

impl VkImageCreateInfo {
    /// Create a 2D texture info
    pub fn texture_2d(width: u32, height: u32, format: VkFormat, mip_levels: u32) -> Self {
        Self {
            width,
            height,
            depth: 1,
            mip_levels,
            array_layers: 1,
            format,
            tiling: VkImageTiling::Optimal,
            initial_layout: VkImageLayout::Undefined,
            usage: VkImageUsageFlags::SAMPLED | VkImageUsageFlags::TRANSFER_DST,
            samples: VkSampleCountFlags::Count1,
            memory_properties: VkMemoryPropertyFlags::DEVICE_LOCAL,
        }
    }

    /// Create a render target info
    pub fn render_target(width: u32, height: u32, format: VkFormat) -> Self {
        Self {
            width,
            height,
            depth: 1,
            mip_levels: 1,
            array_layers: 1,
            format,
            tiling: VkImageTiling::Optimal,
            initial_layout: VkImageLayout::Undefined,
            usage: VkImageUsageFlags::COLOR_ATTACHMENT | VkImageUsageFlags::SAMPLED,
            samples: VkSampleCountFlags::Count1,
            memory_properties: VkMemoryPropertyFlags::DEVICE_LOCAL,
        }
    }

    /// Create a depth buffer info
    pub fn depth_buffer(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            depth: 1,
            mip_levels: 1,
            array_layers: 1,
            format: VkFormat::D32Sfloat,
            tiling: VkImageTiling::Optimal,
            initial_layout: VkImageLayout::Undefined,
            usage: VkImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            samples: VkSampleCountFlags::Count1,
            memory_properties: VkMemoryPropertyFlags::DEVICE_LOCAL,
        }
    }

    /// Create a depth-stencil buffer info
    pub fn depth_stencil_buffer(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            depth: 1,
            mip_levels: 1,
            array_layers: 1,
            format: VkFormat::D24UnormS8Uint,
            tiling: VkImageTiling::Optimal,
            initial_layout: VkImageLayout::Undefined,
            usage: VkImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            samples: VkSampleCountFlags::Count1,
            memory_properties: VkMemoryPropertyFlags::DEVICE_LOCAL,
        }
    }

    /// Create a storage image info
    pub fn storage_image(width: u32, height: u32, format: VkFormat) -> Self {
        Self {
            width,
            height,
            depth: 1,
            mip_levels: 1,
            array_layers: 1,
            format,
            tiling: VkImageTiling::Optimal,
            initial_layout: VkImageLayout::Undefined,
            usage: VkImageUsageFlags::STORAGE | VkImageUsageFlags::TRANSFER_DST,
            samples: VkSampleCountFlags::Count1,
            memory_properties: VkMemoryPropertyFlags::DEVICE_LOCAL,
        }
    }

    /// Create a cubemap info
    pub fn cubemap(size: u32, format: VkFormat, mip_levels: u32) -> Self {
        Self {
            width: size,
            height: size,
            depth: 1,
            mip_levels,
            array_layers: 6,
            format,
            tiling: VkImageTiling::Optimal,
            initial_layout: VkImageLayout::Undefined,
            usage: VkImageUsageFlags::SAMPLED | VkImageUsageFlags::TRANSFER_DST,
            samples: VkSampleCountFlags::Count1,
            memory_properties: VkMemoryPropertyFlags::DEVICE_LOCAL,
        }
    }
}

// ============================================================================
// VkImageCapsule
// ============================================================================

/// Mock Vulkan Image Capsule
///
/// # Tier: T1 Atomic
/// # Size: 256B cache-aligned
///
/// # ASSUM Safety
///
/// - `#ASSUME_MOCK_IMAGE`: No real Vulkan operations
/// - `#ASSUME_STATE_ATOMIC`: Lockfree state management
/// - `#ASSUME_LAYOUT_VALID`: Layout transitions validated
#[repr(C, align(256))]
pub struct VkImageCapsule {
    /// Mock VkImage handle
    handle: AtomicU64,

    /// Mock VkDeviceMemory handle
    memory: AtomicU64,

    /// Primary coordination: state(8) | format(8) | layout(8) | generation(40)
    primary: AtomicU64,

    /// Secondary: width(16) | height(16) | depth(16) | mip_levels(8) | array_layers(8)
    secondary: AtomicU64,

    /// Usage flags
    usage: AtomicU32,

    /// Sample count
    samples: AtomicU32,

    /// Tiling mode
    tiling: AtomicU32,

    /// Memory properties
    memory_properties: AtomicU32,

    /// Memory size (bytes)
    memory_size: AtomicU64,

    /// Memory offset
    memory_offset: AtomicU64,

    /// Reserved for alignment
    _reserved: [u8; 184],
}

// Compile-time verification
const _: () = {
    assert!(core::mem::size_of::<VkImageCapsule>() == 256);
    assert!(core::mem::align_of::<VkImageCapsule>() == 256);
};

impl VkImageCapsule {
    /// Create a new image capsule in uninitialized state
    pub const fn new() -> Self {
        Self {
            handle: AtomicU64::new(0),
            memory: AtomicU64::new(0),
            primary: AtomicU64::new(0),
            secondary: AtomicU64::new(0),
            usage: AtomicU32::new(0),
            samples: AtomicU32::new(VkSampleCountFlags::Count1 as u32),
            tiling: AtomicU32::new(VkImageTiling::Optimal as u32),
            memory_properties: AtomicU32::new(0),
            memory_size: AtomicU64::new(0),
            memory_offset: AtomicU64::new(0),
            _reserved: [0; 184],
        }
    }

    /// Create and initialize an image
    ///
    /// # Arguments
    ///
    /// * `info` - Image creation parameters
    ///
    /// # Returns
    ///
    /// `VkResult::Success` on success
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_DIMENSIONS_VALID`: Width/height must be > 0
    /// - `#ASSUME_STATE_TRANSITION`: Uninitialized -> Created
    pub fn create(&self, info: &VkImageCreateInfo) -> VkResult {
        if info.width == 0 || info.height == 0 {
            return VkResult::ErrorInitializationFailed;
        }

        let current = self.primary.load(Ordering::Acquire);
        let state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

        if state != VK_IMAGE_STATE_UNINITIALIZED {
            return VkResult::ErrorInitializationFailed;
        }

        // Generate handle
        let handle = generate_mock_handle();
        self.handle.store(handle, Ordering::Release);

        // Store dimensions
        let width = (info.width.min(0xFFFF) as u64) << WIDTH_SHIFT;
        let height = (info.height.min(0xFFFF) as u64) << HEIGHT_SHIFT;
        let depth = (info.depth.min(0xFFFF) as u64) << DEPTH_SHIFT;
        let mips = (info.mip_levels.min(0xFF) as u64) << MIP_SHIFT;
        let layers = info.array_layers.min(0xFF) as u64;
        let secondary = width | height | depth | mips | layers;
        self.secondary.store(secondary, Ordering::Release);

        // Store other parameters
        self.usage.store(info.usage.0, Ordering::Release);
        self.samples.store(info.samples as u32, Ordering::Release);
        self.tiling.store(info.tiling as u32, Ordering::Release);
        self.memory_properties
            .store(info.memory_properties.0, Ordering::Release);

        // Calculate memory size (mock)
        let bytes_per_pixel = info.format.bytes_per_pixel() as u64;
        let base_size = (info.width as u64) * (info.height as u64) * (info.depth as u64);
        let size = base_size * bytes_per_pixel * (info.array_layers as u64);
        self.memory_size.store(size, Ordering::Release);

        // Update primary with state, format, and layout
        let gen = (current & GENERATION_MASK).wrapping_add(1) & GENERATION_MASK;
        let format_bits = (info.format as u64) << FORMAT_SHIFT;
        let layout_bits = (info.initial_layout as u64) << LAYOUT_SHIFT;
        let new_primary =
            ((VK_IMAGE_STATE_CREATED as u64) << STATE_SHIFT) | format_bits | layout_bits | gen;

        self.primary.store(new_primary, Ordering::Release);

        VkResult::Success
    }

    /// Bind memory to the image
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_MEMORY_VALID`: Memory handle assumed valid (mock)
    /// - `#ASSUME_STATE_TRANSITION`: Created -> Bound
    pub fn bind_memory(&self, memory: u64, offset: u64) -> VkResult {
        if self.state() != VK_IMAGE_STATE_CREATED {
            return VkResult::ErrorInitializationFailed;
        }

        self.memory.store(memory, Ordering::Release);
        self.memory_offset.store(offset, Ordering::Release);

        // Update state to Bound
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let gen = (current & GENERATION_MASK).wrapping_add(1) & GENERATION_MASK;
            let format = current & FORMAT_MASK;
            let layout = current & LAYOUT_MASK;
            let new_primary =
                ((VK_IMAGE_STATE_BOUND as u64) << STATE_SHIFT) | format | layout | gen;

            if self
                .primary
                .compare_exchange(current, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }

        VkResult::Success
    }

    /// Transition image layout
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_LAYOUT_TRANSITION`: Layout change tracked atomically
    /// - `#ASSUME_STATE_TRANSITION`: Bound -> Transitioning -> Ready
    pub fn transition_layout(&self, new_layout: VkImageLayout) -> VkResult {
        let state = self.state();
        if state != VK_IMAGE_STATE_BOUND && state != VK_IMAGE_STATE_READY {
            return VkResult::ErrorInitializationFailed;
        }

        // Update layout atomically
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let gen = (current & GENERATION_MASK).wrapping_add(1) & GENERATION_MASK;
            let format = current & FORMAT_MASK;
            let new_layout_bits = (new_layout as u64) << LAYOUT_SHIFT;
            let new_primary =
                ((VK_IMAGE_STATE_READY as u64) << STATE_SHIFT) | format | new_layout_bits | gen;

            if self
                .primary
                .compare_exchange(current, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }

        VkResult::Success
    }

    /// Destroy the image
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_STATE_TRANSITION`: Any -> Destroyed
    pub fn destroy(&self) -> VkResult {
        if self.state() == VK_IMAGE_STATE_DESTROYED {
            return VkResult::Success;
        }

        // Clear all fields
        self.handle.store(0, Ordering::Release);
        self.memory.store(0, Ordering::Release);
        self.secondary.store(0, Ordering::Release);
        self.memory_size.store(0, Ordering::Release);
        self.memory_offset.store(0, Ordering::Release);

        // Update state
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let gen = (current & GENERATION_MASK).wrapping_add(1) & GENERATION_MASK;
            let new_primary = ((VK_IMAGE_STATE_DESTROYED as u64) << STATE_SHIFT) | gen;

            if self
                .primary
                .compare_exchange(current, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }

        VkResult::Success
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Get current state
    #[inline]
    pub fn state(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & STATE_MASK) >> STATE_SHIFT) as u8
    }

    /// Get mock handle
    #[inline]
    pub fn handle(&self) -> u64 {
        self.handle.load(Ordering::Acquire)
    }

    /// Get memory handle
    #[inline]
    pub fn memory(&self) -> u64 {
        self.memory.load(Ordering::Acquire)
    }

    /// Get current layout
    #[inline]
    pub fn layout(&self) -> VkImageLayout {
        let primary = self.primary.load(Ordering::Acquire);
        let layout_raw = ((primary & LAYOUT_MASK) >> LAYOUT_SHIFT) as u32;

        // Safe conversion (we only store valid enum values)
        match layout_raw {
            0 => VkImageLayout::Undefined,
            1 => VkImageLayout::General,
            2 => VkImageLayout::ColorAttachmentOptimal,
            3 => VkImageLayout::DepthStencilAttachmentOptimal,
            5 => VkImageLayout::ShaderReadOnlyOptimal,
            6 => VkImageLayout::TransferSrcOptimal,
            7 => VkImageLayout::TransferDstOptimal,
            _ => VkImageLayout::Undefined,
        }
    }

    /// Get format
    #[inline]
    pub fn format(&self) -> VkFormat {
        let primary = self.primary.load(Ordering::Acquire);
        let format_raw = ((primary & FORMAT_MASK) >> FORMAT_SHIFT) as u32;

        // Safe conversion based on stored value
        match format_raw {
            0 => VkFormat::Undefined,
            37 => VkFormat::R8G8B8A8Unorm,
            43 => VkFormat::R8G8B8A8Srgb,
            44 => VkFormat::B8G8R8A8Unorm,
            97 => VkFormat::R16G16B16A16Sfloat,
            109 => VkFormat::R32G32B32A32Sfloat,
            126 => VkFormat::D32Sfloat,
            129 => VkFormat::D24UnormS8Uint,
            _ => VkFormat::Undefined,
        }
    }

    /// Get width
    #[inline]
    pub fn width(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & WIDTH_MASK) >> WIDTH_SHIFT) as u32
    }

    /// Get height
    #[inline]
    pub fn height(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & HEIGHT_MASK) >> HEIGHT_SHIFT) as u32
    }

    /// Get depth
    #[inline]
    pub fn depth(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & DEPTH_MASK) >> DEPTH_SHIFT) as u32
    }

    /// Get mip levels
    #[inline]
    pub fn mip_levels(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & MIP_MASK) >> MIP_SHIFT) as u32
    }

    /// Get array layers
    #[inline]
    pub fn array_layers(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        (secondary & ARRAY_MASK) as u32
    }

    /// Get usage flags
    #[inline]
    pub fn usage(&self) -> VkImageUsageFlags {
        VkImageUsageFlags(self.usage.load(Ordering::Acquire))
    }

    /// Get sample count
    #[inline]
    pub fn samples(&self) -> VkSampleCountFlags {
        let raw = self.samples.load(Ordering::Acquire);
        match raw {
            1 => VkSampleCountFlags::Count1,
            2 => VkSampleCountFlags::Count2,
            4 => VkSampleCountFlags::Count4,
            8 => VkSampleCountFlags::Count8,
            16 => VkSampleCountFlags::Count16,
            32 => VkSampleCountFlags::Count32,
            64 => VkSampleCountFlags::Count64,
            _ => VkSampleCountFlags::Count1,
        }
    }

    /// Get memory size
    #[inline]
    pub fn memory_size(&self) -> u64 {
        self.memory_size.load(Ordering::Acquire)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        let primary = self.primary.load(Ordering::Acquire);
        primary & GENERATION_MASK
    }

    /// Check if image is bound
    #[inline]
    pub fn is_bound(&self) -> bool {
        let state = self.state();
        state >= VK_IMAGE_STATE_BOUND && state < VK_IMAGE_STATE_DESTROYED
    }

    /// Check if depth format
    #[inline]
    pub fn is_depth(&self) -> bool {
        self.format().is_depth()
    }
}

impl Default for VkImageCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: All operations are atomic
unsafe impl Send for VkImageCapsule {}
unsafe impl Sync for VkImageCapsule {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<VkImageCapsule>(), 256);
        assert_eq!(core::mem::align_of::<VkImageCapsule>(), 256);
    }

    #[test]
    fn test_initial_state() {
        let image = VkImageCapsule::new();
        assert_eq!(image.state(), VK_IMAGE_STATE_UNINITIALIZED);
        assert_eq!(image.handle(), 0);
    }

    #[test]
    fn test_create_texture_2d() {
        let image = VkImageCapsule::new();
        let info = VkImageCreateInfo::texture_2d(512, 512, VkFormat::R8G8B8A8Unorm, 1);

        let result = image.create(&info);
        assert!(result.is_success());
        assert_eq!(image.state(), VK_IMAGE_STATE_CREATED);
        assert!(image.handle() > 0);
        assert_eq!(image.width(), 512);
        assert_eq!(image.height(), 512);
        assert_eq!(image.depth(), 1);
        assert_eq!(image.mip_levels(), 1);
    }

    #[test]
    fn test_create_render_target() {
        let image = VkImageCapsule::new();
        let info = VkImageCreateInfo::render_target(1920, 1080, VkFormat::R8G8B8A8Unorm);

        image.create(&info);
        assert!(image.usage().contains(VkImageUsageFlags::COLOR_ATTACHMENT));
        assert_eq!(image.width(), 1920);
        assert_eq!(image.height(), 1080);
    }

    #[test]
    fn test_create_depth_buffer() {
        let image = VkImageCapsule::new();
        let info = VkImageCreateInfo::depth_buffer(1920, 1080);

        image.create(&info);
        assert!(image.is_depth());
        assert!(image
            .usage()
            .contains(VkImageUsageFlags::DEPTH_STENCIL_ATTACHMENT));
    }

    #[test]
    fn test_create_storage_image() {
        let image = VkImageCapsule::new();
        let info = VkImageCreateInfo::storage_image(256, 256, VkFormat::R32G32B32A32Sfloat);

        image.create(&info);
        assert!(image.usage().contains(VkImageUsageFlags::STORAGE));
    }

    #[test]
    fn test_create_cubemap() {
        let image = VkImageCapsule::new();
        let info = VkImageCreateInfo::cubemap(256, VkFormat::R8G8B8A8Unorm, 8);

        image.create(&info);
        assert_eq!(image.array_layers(), 6);
        assert_eq!(image.mip_levels(), 8);
    }

    #[test]
    fn test_create_zero_dimensions_fails() {
        let image = VkImageCapsule::new();
        let info = VkImageCreateInfo {
            width: 0,
            height: 512,
            ..Default::default()
        };

        let result = image.create(&info);
        assert!(result.is_error());
    }

    #[test]
    fn test_bind_memory() {
        let image = VkImageCapsule::new();
        image.create(&VkImageCreateInfo::texture_2d(
            256,
            256,
            VkFormat::R8G8B8A8Unorm,
            1,
        ));

        let memory = generate_mock_handle();
        let result = image.bind_memory(memory, 0);

        assert!(result.is_success());
        assert_eq!(image.state(), VK_IMAGE_STATE_BOUND);
        assert_eq!(image.memory(), memory);
    }

    #[test]
    fn test_transition_layout() {
        let image = VkImageCapsule::new();
        image.create(&VkImageCreateInfo::texture_2d(
            256,
            256,
            VkFormat::R8G8B8A8Unorm,
            1,
        ));
        image.bind_memory(generate_mock_handle(), 0);

        assert_eq!(image.layout(), VkImageLayout::Undefined);

        let result = image.transition_layout(VkImageLayout::ShaderReadOnlyOptimal);
        assert!(result.is_success());
        assert_eq!(image.state(), VK_IMAGE_STATE_READY);
        assert_eq!(image.layout(), VkImageLayout::ShaderReadOnlyOptimal);
    }

    #[test]
    fn test_destroy() {
        let image = VkImageCapsule::new();
        image.create(&VkImageCreateInfo::texture_2d(
            256,
            256,
            VkFormat::R8G8B8A8Unorm,
            1,
        ));
        image.bind_memory(generate_mock_handle(), 0);

        let result = image.destroy();
        assert!(result.is_success());
        assert_eq!(image.state(), VK_IMAGE_STATE_DESTROYED);
        assert_eq!(image.handle(), 0);
    }

    #[test]
    fn test_memory_size_calculation() {
        let image = VkImageCapsule::new();
        image.create(&VkImageCreateInfo::texture_2d(
            256,
            256,
            VkFormat::R8G8B8A8Unorm,
            1,
        ));

        // 256 * 256 * 4 bytes = 262144
        assert_eq!(image.memory_size(), 256 * 256 * 4);
    }

    #[test]
    fn test_generation_increments() {
        let image = VkImageCapsule::new();
        let gen0 = image.generation();

        image.create(&VkImageCreateInfo::default());
        let gen1 = image.generation();
        assert!(gen1 > gen0);

        image.bind_memory(generate_mock_handle(), 0);
        let gen2 = image.generation();
        assert!(gen2 > gen1);

        image.transition_layout(VkImageLayout::General);
        let gen3 = image.generation();
        assert!(gen3 > gen2);
    }

    #[test]
    fn test_is_bound() {
        let image = VkImageCapsule::new();
        assert!(!image.is_bound());

        image.create(&VkImageCreateInfo::default());
        assert!(!image.is_bound());

        image.bind_memory(generate_mock_handle(), 0);
        assert!(image.is_bound());

        image.transition_layout(VkImageLayout::General);
        assert!(image.is_bound());

        image.destroy();
        assert!(!image.is_bound());
    }

    #[test]
    fn test_double_destroy() {
        let image = VkImageCapsule::new();
        image.create(&VkImageCreateInfo::default());
        image.destroy();

        let result = image.destroy();
        assert!(result.is_success()); // Idempotent
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_reads() {
        use std::sync::Arc;
        use std::thread;

        let image = Arc::new(VkImageCapsule::new());
        image.create(&VkImageCreateInfo::texture_2d(
            512,
            512,
            VkFormat::R8G8B8A8Unorm,
            4,
        ));
        image.bind_memory(generate_mock_handle(), 0);

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let img = Arc::clone(&image);
                thread::spawn(move || {
                    for _ in 0..1000 {
                        let _ = img.state();
                        let _ = img.width();
                        let _ = img.height();
                        let _ = img.layout();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert!(image.is_bound());
    }
}
