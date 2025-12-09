// Vulkan Render Pass Capsule - T7 Heterogeneous Tier
// UCE34: Q10 T7 (GPU acceleration), Q33 verification, Q34 audit
// Chaos: 100% lockfree, cache-aligned, DualAtomicU64 coordination
//
// Research Summary (2024-2025):
// - VK_KHR_dynamic_rendering (Vulkan 1.3 core) preferred for desktop, simpler API
// - VK_KHR_dynamic_rendering_local_read (Roadmap 2024) adds subpass-like functionality
// - Traditional render passes still critical for mobile (55% bandwidth savings on tile GPUs)
// - Subpass dependencies enable on-chip tile memory optimization (Mali G76: 262k vs 614k tiles)
// - Input attachments restrict reads to same pixel (gbuffer optimization)
//
// References:
// - https://docs.vulkan.org/features/latest/features/proposals/VK_KHR_dynamic_rendering.html
// - https://www.khronos.org/blog/streamlining-subpasses
// - https://docs.vulkan.org/samples/latest/samples/performance/subpasses/README.html
// - https://gpuopen.com/learn/vulkan-renderpasses/
// - https://developer.samsung.com/galaxy-gamedev/resources/articles/renderpasses.html

use core::sync::atomic::{AtomicU64, Ordering};
use crate::patterns::dual_atomic::DualAtomicU64;

/// Attachment load operation (VkAttachmentLoadOp)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LoadOp {
    Load = 0,      // VK_ATTACHMENT_LOAD_OP_LOAD
    Clear = 1,     // VK_ATTACHMENT_LOAD_OP_CLEAR
    DontCare = 2,  // VK_ATTACHMENT_LOAD_OP_DONT_CARE
}

/// Attachment store operation (VkAttachmentStoreOp)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum StoreOp {
    Store = 0,     // VK_ATTACHMENT_STORE_OP_STORE
    DontCare = 1,  // VK_ATTACHMENT_STORE_OP_DONT_CARE
}

/// Image layout transitions (VkImageLayout)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ImageLayout {
    Undefined = 0,                  // VK_IMAGE_LAYOUT_UNDEFINED
    General = 1,                    // VK_IMAGE_LAYOUT_GENERAL
    ColorAttachment = 2,            // VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL
    DepthStencilAttachment = 3,     // VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL
    DepthStencilReadOnly = 4,       // VK_IMAGE_LAYOUT_DEPTH_STENCIL_READ_ONLY_OPTIMAL
    ShaderReadOnly = 5,             // VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL
    TransferSrc = 6,                // VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL
    TransferDst = 7,                // VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL
    Preinitialized = 8,             // VK_IMAGE_LAYOUT_PREINITIALIZED
    DepthReadOnlyStencilAttachment = 1000117000,  // VK_IMAGE_LAYOUT_DEPTH_READ_ONLY_STENCIL_ATTACHMENT_OPTIMAL
    DepthAttachmentStencilReadOnly = 1000117001,  // VK_IMAGE_LAYOUT_DEPTH_ATTACHMENT_STENCIL_READ_ONLY_OPTIMAL
    PresentSrc = 1000001002,        // VK_IMAGE_LAYOUT_PRESENT_SRC_KHR (Swapchain)
    AttachmentOptimal = 1000314000, // VK_IMAGE_LAYOUT_ATTACHMENT_OPTIMAL (Vulkan 1.2+)
    ReadOnlyOptimal = 1000314001,   // VK_IMAGE_LAYOUT_READ_ONLY_OPTIMAL (Vulkan 1.2+)
}

/// Pipeline stage flags (VkPipelineStageFlagBits)
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum PipelineStage {
    TopOfPipe = 0x00000001,
    DrawIndirect = 0x00000002,
    VertexInput = 0x00000004,
    VertexShader = 0x00000008,
    FragmentShader = 0x00000080,
    EarlyFragmentTests = 0x00000100,
    LateFragmentTests = 0x00000200,
    ColorAttachmentOutput = 0x00000400,
    BottomOfPipe = 0x00002000,
    AllGraphics = 0x0000FFFF,
}

/// Access flags (VkAccessFlagBits)
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum AccessFlags {
    None = 0,
    IndirectCommandRead = 0x00000001,
    IndexRead = 0x00000002,
    VertexAttributeRead = 0x00000004,
    UniformRead = 0x00000008,
    InputAttachmentRead = 0x00000010,
    ShaderRead = 0x00000020,
    ShaderWrite = 0x00000040,
    ColorAttachmentRead = 0x00000080,
    ColorAttachmentWrite = 0x00000100,
    DepthStencilAttachmentRead = 0x00000200,
    DepthStencilAttachmentWrite = 0x00000400,
    TransferRead = 0x00000800,
    TransferWrite = 0x00001000,
    MemoryRead = 0x00008000,
    MemoryWrite = 0x00010000,
}

/// Render pass attachment descriptor
/// Describes one image attachment (color, depth, or input)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct AttachmentDesc {
    pub format: u32,                // VkFormat
    pub samples: u32,               // VkSampleCountFlagBits
    pub load_op: LoadOp,
    pub store_op: StoreOp,
    pub stencil_load_op: LoadOp,
    pub stencil_store_op: StoreOp,
    pub initial_layout: ImageLayout,
    pub final_layout: ImageLayout,
}

impl Default for AttachmentDesc {
    fn default() -> Self {
        Self {
            format: 0,
            samples: 1,
            load_op: LoadOp::DontCare,
            store_op: StoreOp::DontCare,
            stencil_load_op: LoadOp::DontCare,
            stencil_store_op: StoreOp::DontCare,
            initial_layout: ImageLayout::Undefined,
            final_layout: ImageLayout::General,
        }
    }
}

/// Attachment reference for subpass
#[repr(C)]
#[derive(Clone, Copy)]
pub struct AttachmentRef {
    pub attachment: u32,  // Index into attachments array, or VK_ATTACHMENT_UNUSED (0xFFFFFFFF)
    pub layout: ImageLayout,
}

impl Default for AttachmentRef {
    fn default() -> Self {
        Self {
            attachment: 0xFFFFFFFF, // VK_ATTACHMENT_UNUSED
            layout: ImageLayout::Undefined,
        }
    }
}

/// Subpass descriptor
/// Max 8 color attachments (matches Vulkan minColorAttachments guarantee)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SubpassDesc {
    pub color_attachments: [AttachmentRef; 8],
    pub color_count: u32,
    pub depth_attachment: AttachmentRef,
    pub input_attachments: [AttachmentRef; 4],  // Input attachments for deferred rendering
    pub input_count: u32,
    pub preserve_attachments: [u32; 4],  // Attachments to preserve unchanged
    pub preserve_count: u32,
    pub resolve_attachments: [AttachmentRef; 8],  // MSAA resolve targets
    pub resolve_count: u32,
}

impl Default for SubpassDesc {
    fn default() -> Self {
        Self {
            color_attachments: [AttachmentRef::default(); 8],
            color_count: 0,
            depth_attachment: AttachmentRef::default(),
            input_attachments: [AttachmentRef::default(); 4],
            input_count: 0,
            preserve_attachments: [0xFFFFFFFF; 4],
            preserve_count: 0,
            resolve_attachments: [AttachmentRef::default(); 8],
            resolve_count: 0,
        }
    }
}

/// Subpass dependency descriptor
/// Defines execution and memory dependencies between subpasses
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SubpassDependency {
    pub src_subpass: u32,      // VK_SUBPASS_EXTERNAL = 0xFFFFFFFF
    pub dst_subpass: u32,
    pub src_stage_mask: u32,   // VkPipelineStageFlags
    pub dst_stage_mask: u32,
    pub src_access_mask: u32,  // VkAccessFlags
    pub dst_access_mask: u32,
    pub dependency_flags: u32, // VkDependencyFlags (BY_REGION = 0x1)
}

impl Default for SubpassDependency {
    fn default() -> Self {
        Self {
            src_subpass: 0xFFFFFFFF,  // VK_SUBPASS_EXTERNAL
            dst_subpass: 0,
            src_stage_mask: PipelineStage::ColorAttachmentOutput as u32,
            dst_stage_mask: PipelineStage::ColorAttachmentOutput as u32,
            src_access_mask: 0,
            dst_access_mask: AccessFlags::ColorAttachmentWrite as u32,
            dependency_flags: 0,
        }
    }
}

/// Clear value union (color or depth/stencil)
#[repr(C)]
#[derive(Clone, Copy)]
pub union ClearValue {
    pub color: [f32; 4],
    pub depth_stencil: DepthStencilClearValue,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DepthStencilClearValue {
    pub depth: f32,
    pub stencil: u32,
}

/// Vulkan Render Pass Capsule
/// 1024-byte aligned for multi-pass support with up to 8 attachments and 4 subpasses
///
/// Key Optimizations (2024-2025 Research):
/// 1. Subpass merging for tile-based GPUs (55% bandwidth reduction on Mali G76)
/// 2. Input attachments for deferred rendering (on-chip gbuffer access)
/// 3. Transient attachments with lazy allocation (VK_MEMORY_PROPERTY_LAZILY_ALLOCATED_BIT)
/// 4. VK_DEPENDENCY_BY_REGION_BIT for framebuffer-space dependencies
/// 5. External dependencies for proper synchronization with pre/post-renderpass work
///
/// #ASSUME_DEVICE_VALID: VkDevice handle valid and supports Vulkan 1.0+
/// #ASSUME_ATTACHMENT_COMPATIBLE: Attachments match framebuffer format/dimensions
/// #ASSUME_SUBPASS_ORDER: Subpasses executed in sequential order 0..N
/// #ASSUME_LAYOUT_TRANSITIONS: Automatic layout transitions correct per Vulkan spec
/// #ASSUME_DEPENDENCY_VALID: Subpass dependencies form valid acyclic graph
#[repr(C, align(1024))]
pub struct VulkanRenderPassCapsule {
    // T1 Atomic coordination (64 bytes)
    stats: DualAtomicU64,           // [0-31] begins count, [32-63] ends count
    total_subpass_advances: AtomicU64,
    current_subpass: AtomicU64,     // Active subpass index
    flags: AtomicU64,               // Bit 0: dynamic rendering enabled

    // Render pass state (64 bytes)
    handle: AtomicU64,              // VkRenderPass handle (0 if using dynamic rendering)
    framebuffer: AtomicU64,         // Current VkFramebuffer
    render_area_x: AtomicU64,       // Packed: [0-31] offset.x, [32-63] extent.width
    render_area_y: AtomicU64,       // Packed: [0-31] offset.y, [32-63] extent.height

    // Attachments (max 8, 512 bytes)
    attachments: [AttachmentDesc; 8],
    attachment_count: u32,
    _pad1: u32,

    // Subpasses (max 4, 256 bytes each = 1024 bytes, but we need padding calculation)
    // Actually SubpassDesc is 128 bytes, so 4 × 128 = 512 bytes
    subpasses: [SubpassDesc; 4],
    subpass_count: u32,
    _pad2: u32,

    // Dependencies (max 8, 32 bytes each = 256 bytes)
    dependencies: [SubpassDependency; 8],
    dependency_count: u32,
    _pad3: u32,

    // Clear values (128 bytes for 8 attachments × 16 bytes each)
    clear_values: [ClearValue; 8],

    // Padding calculation:
    // 64 (stats) + 64 (state) + 512 (attachments) + 512 (subpasses) + 256 (deps) + 128 (clear) = 1536 bytes
    // Wait, let me recalculate:
    // DualAtomicU64: 16 bytes
    // 3 × AtomicU64: 24 bytes
    // Total T1: 40 bytes, not 64. Let me fix:
    // DualAtomicU64: 16, AtomicU64 × 3: 24, AtomicU64 × 4: 32 = 72 bytes for atomics
    // AttachmentDesc: 32 bytes × 8 = 256 bytes
    // SubpassDesc: let's calculate properly
    _padding: [u8; 0], // Calculate below
}

// Let's calculate sizes properly:
// DualAtomicU64: 16 bytes
// AtomicU64 × 7: 56 bytes
// Total atomics: 72 bytes
//
// AttachmentDesc: 4+4+1+1+1+1+4+4 = 20 bytes, but with alignment padding to 24 bytes
// AttachmentDesc × 8: 192 bytes (let's assume 32 bytes with padding) = 256 bytes
//
// AttachmentRef: 4+4 = 8 bytes
// SubpassDesc: 8×8 + 4 + 8 + 8×4 + 4 + 4×4 + 4 + 8×8 + 4 = 64+4+8+32+4+16+4+64+4 = 200 bytes
// With padding to 256 bytes × 4 = 1024 bytes
//
// SubpassDependency: 7×4 = 28 bytes, padded to 32 bytes × 8 = 256 bytes
//
// ClearValue: 16 bytes × 8 = 128 bytes
//
// Total: 72 + 256 + 1024 + 256 + 128 = 1736 bytes
// Need padding: 1024 - (1736 % 1024) = 1024 - 712 = 312 bytes
// Wait, 1736 > 1024, so we need 2048 alignment
//
// Let me recalculate more carefully with repr(C) packing:

// Size calculation helper
const fn calculate_render_pass_size() -> usize {
    use core::mem::size_of;

    let atomics = size_of::<DualAtomicU64>() + size_of::<AtomicU64>() * 7;
    let attachments = size_of::<AttachmentDesc>() * 8 + 8; // +8 for count+padding
    let subpasses = size_of::<SubpassDesc>() * 4 + 8;
    let dependencies = size_of::<SubpassDependency>() * 8 + 8;
    let clear_values = size_of::<ClearValue>() * 8;

    atomics + attachments + subpasses + dependencies + clear_values
}

const fn calculate_padding() -> usize {
    const TARGET: usize = 1024;
    const BASE_SIZE: usize = calculate_render_pass_size();

    if BASE_SIZE <= TARGET {
        TARGET - BASE_SIZE
    } else {
        // Round up to next multiple of 1024
        let next_multiple = ((BASE_SIZE + TARGET - 1) / TARGET) * TARGET;
        next_multiple - BASE_SIZE
    }
}

// Redefine with correct padding
#[repr(C, align(1024))]
pub struct VulkanRenderPassCapsuleActual {
    // T1 Atomic coordination
    stats: DualAtomicU64,
    total_subpass_advances: AtomicU64,
    current_subpass: AtomicU64,
    flags: AtomicU64,

    // Render pass state
    handle: AtomicU64,
    framebuffer: AtomicU64,
    render_area_x: AtomicU64,
    render_area_y: AtomicU64,

    // Attachments (max 8)
    attachments: [AttachmentDesc; 8],
    attachment_count: u32,
    _pad1: u32,

    // Subpasses (max 4)
    subpasses: [SubpassDesc; 4],
    subpass_count: u32,
    _pad2: u32,

    // Dependencies (max 8)
    dependencies: [SubpassDependency; 8],
    dependency_count: u32,
    _pad3: u32,

    // Clear values
    clear_values: [ClearValue; 8],

    // Dynamic padding
    _padding: [u8; calculate_padding()],
}

// Type alias for easier use
pub type VulkanRenderPassCapsule = VulkanRenderPassCapsuleActual;

impl VulkanRenderPassCapsule {
    /// Create new render pass capsule
    pub const fn new() -> Self {
        Self {
            stats: DualAtomicU64::new(0, 0),
            total_subpass_advances: AtomicU64::new(0),
            current_subpass: AtomicU64::new(0),
            flags: AtomicU64::new(0),
            handle: AtomicU64::new(0),
            framebuffer: AtomicU64::new(0),
            render_area_x: AtomicU64::new(0),
            render_area_y: AtomicU64::new(0),
            attachments: [AttachmentDesc {
                format: 0,
                samples: 1,
                load_op: LoadOp::DontCare,
                store_op: StoreOp::DontCare,
                stencil_load_op: LoadOp::DontCare,
                stencil_store_op: StoreOp::DontCare,
                initial_layout: ImageLayout::Undefined,
                final_layout: ImageLayout::General,
            }; 8],
            attachment_count: 0,
            _pad1: 0,
            subpasses: [SubpassDesc {
                color_attachments: [AttachmentRef { attachment: 0xFFFFFFFF, layout: ImageLayout::Undefined }; 8],
                color_count: 0,
                depth_attachment: AttachmentRef { attachment: 0xFFFFFFFF, layout: ImageLayout::Undefined },
                input_attachments: [AttachmentRef { attachment: 0xFFFFFFFF, layout: ImageLayout::Undefined }; 4],
                input_count: 0,
                preserve_attachments: [0xFFFFFFFF; 4],
                preserve_count: 0,
                resolve_attachments: [AttachmentRef { attachment: 0xFFFFFFFF, layout: ImageLayout::Undefined }; 8],
                resolve_count: 0,
            }; 4],
            subpass_count: 0,
            _pad2: 0,
            dependencies: [SubpassDependency {
                src_subpass: 0xFFFFFFFF,
                dst_subpass: 0,
                src_stage_mask: PipelineStage::ColorAttachmentOutput as u32,
                dst_stage_mask: PipelineStage::ColorAttachmentOutput as u32,
                src_access_mask: 0,
                dst_access_mask: AccessFlags::ColorAttachmentWrite as u32,
                dependency_flags: 0,
            }; 8],
            dependency_count: 0,
            _pad3: 0,
            clear_values: [ClearValue { color: [0.0, 0.0, 0.0, 0.0] }; 8],
            _padding: [0u8; calculate_padding()],
        }
    }

    /// Set render pass handle (VkRenderPass)
    #[inline]
    pub fn set_handle(&self, handle: u64) {
        self.handle.store(handle, Ordering::Release);
    }

    /// Get render pass handle
    #[inline]
    pub fn get_handle(&self) -> u64 {
        self.handle.load(Ordering::Acquire)
    }

    /// Set framebuffer handle (VkFramebuffer)
    #[inline]
    pub fn set_framebuffer(&self, framebuffer: u64) {
        self.framebuffer.store(framebuffer, Ordering::Release);
    }

    /// Get framebuffer handle
    #[inline]
    pub fn get_framebuffer(&self) -> u64 {
        self.framebuffer.load(Ordering::Acquire)
    }

    /// Set render area (offset and extent)
    #[inline]
    pub fn set_render_area(&self, offset_x: u32, offset_y: u32, width: u32, height: u32) {
        let x_packed = ((offset_x as u64) << 32) | (width as u64);
        let y_packed = ((offset_y as u64) << 32) | (height as u64);
        self.render_area_x.store(x_packed, Ordering::Release);
        self.render_area_y.store(y_packed, Ordering::Release);
    }

    /// Get render area
    #[inline]
    pub fn get_render_area(&self) -> (u32, u32, u32, u32) {
        let x_packed = self.render_area_x.load(Ordering::Acquire);
        let y_packed = self.render_area_y.load(Ordering::Acquire);

        let offset_x = (x_packed >> 32) as u32;
        let width = x_packed as u32;
        let offset_y = (y_packed >> 32) as u32;
        let height = y_packed as u32;

        (offset_x, offset_y, width, height)
    }

    /// Begin render pass
    /// Increments begin counter in stats
    #[inline]
    pub fn begin(&self) {
        // Increment begins counter (lower 32 bits)
        let (begins, ends) = self.stats.load(Ordering::Acquire);
        self.stats.store(begins + 1, ends, Ordering::Release);

        // Reset current subpass
        self.current_subpass.store(0, Ordering::Release);
    }

    /// End render pass
    /// Increments end counter in stats
    #[inline]
    pub fn end(&self) {
        // Increment ends counter (upper 32 bits)
        let (begins, ends) = self.stats.load(Ordering::Acquire);
        self.stats.store(begins, ends + 1, Ordering::Release);
    }

    /// Advance to next subpass
    /// Returns new subpass index, or None if no more subpasses
    #[inline]
    pub fn next_subpass(&self) -> Option<u32> {
        let current = self.current_subpass.fetch_add(1, Ordering::AcqRel);
        self.total_subpass_advances.fetch_add(1, Ordering::Relaxed);

        let next = current + 1;
        if next < self.subpass_count {
            Some(next as u32)
        } else {
            None
        }
    }

    /// Get current subpass index
    #[inline]
    pub fn current_subpass_index(&self) -> u32 {
        self.current_subpass.load(Ordering::Acquire) as u32
    }

    /// Get statistics (begins, ends, subpass advances)
    #[inline]
    pub fn get_stats(&self) -> (u64, u64, u64) {
        let (begins, ends) = self.stats.load(Ordering::Acquire);
        let advances = self.total_subpass_advances.load(Ordering::Acquire);
        (begins, ends, advances)
    }

    /// Enable dynamic rendering mode (VK_KHR_dynamic_rendering)
    #[inline]
    pub fn enable_dynamic_rendering(&self) {
        self.flags.fetch_or(1, Ordering::Release);
    }

    /// Check if dynamic rendering enabled
    #[inline]
    pub fn is_dynamic_rendering(&self) -> bool {
        self.flags.load(Ordering::Acquire) & 1 != 0
    }

    /// Set clear color for attachment index
    #[inline]
    pub fn set_clear_color(&mut self, index: usize, r: f32, g: f32, b: f32, a: f32) {
        if index < 8 {
            self.clear_values[index].color = [r, g, b, a];
        }
    }

    /// Set clear depth/stencil for attachment index
    #[inline]
    pub fn set_clear_depth_stencil(&mut self, index: usize, depth: f32, stencil: u32) {
        if index < 8 {
            self.clear_values[index].depth_stencil = DepthStencilClearValue { depth, stencil };
        }
    }

    /// Add attachment descriptor
    /// Returns attachment index, or None if full
    pub fn add_attachment(&mut self, desc: AttachmentDesc) -> Option<usize> {
        let count = self.attachment_count as usize;
        if count < 8 {
            self.attachments[count] = desc;
            self.attachment_count += 1;
            Some(count)
        } else {
            None
        }
    }

    /// Add subpass descriptor
    /// Returns subpass index, or None if full
    pub fn add_subpass(&mut self, desc: SubpassDesc) -> Option<usize> {
        let count = self.subpass_count as usize;
        if count < 4 {
            self.subpasses[count] = desc;
            self.subpass_count += 1;
            Some(count)
        } else {
            None
        }
    }

    /// Add subpass dependency
    /// Returns dependency index, or None if full
    pub fn add_dependency(&mut self, dep: SubpassDependency) -> Option<usize> {
        let count = self.dependency_count as usize;
        if count < 8 {
            self.dependencies[count] = dep;
            self.dependency_count += 1;
            Some(count)
        } else {
            None
        }
    }

    /// Get attachment descriptor
    #[inline]
    pub fn get_attachment(&self, index: usize) -> Option<AttachmentDesc> {
        if index < self.attachment_count as usize {
            Some(self.attachments[index])
        } else {
            None
        }
    }

    /// Get subpass descriptor
    #[inline]
    pub fn get_subpass(&self, index: usize) -> Option<SubpassDesc> {
        if index < self.subpass_count as usize {
            Some(self.subpasses[index])
        } else {
            None
        }
    }

    /// Get subpass dependency
    #[inline]
    pub fn get_dependency(&self, index: usize) -> Option<SubpassDependency> {
        if index < self.dependency_count as usize {
            Some(self.dependencies[index])
        } else {
            None
        }
    }

    /// Reset capsule state (for reuse)
    pub fn reset(&mut self) {
        self.stats.store(0, 0, Ordering::Release);
        self.total_subpass_advances.store(0, Ordering::Release);
        self.current_subpass.store(0, Ordering::Release);
        self.flags.store(0, Ordering::Release);
        self.handle.store(0, Ordering::Release);
        self.framebuffer.store(0, Ordering::Release);
        self.render_area_x.store(0, Ordering::Release);
        self.render_area_y.store(0, Ordering::Release);
        self.attachment_count = 0;
        self.subpass_count = 0;
        self.dependency_count = 0;
    }
}

// Verify capsule properties
const _: () = {
    use core::mem::{size_of, align_of};

    const SIZE: usize = size_of::<VulkanRenderPassCapsule>();
    const ALIGN: usize = align_of::<VulkanRenderPassCapsule>();

    // Must be 1024-byte aligned
    assert!(ALIGN == 1024, "VulkanRenderPassCapsule must be 1024-byte aligned");

    // Size must be multiple of alignment
    assert!(SIZE % ALIGN == 0, "VulkanRenderPassCapsule size must be multiple of alignment");

    // Must fit in at least 1024 bytes (may be larger due to complex structure)
    assert!(SIZE >= 1024, "VulkanRenderPassCapsule must be at least 1024 bytes");
};

// Builder pattern for easier construction
pub struct RenderPassBuilder {
    capsule: VulkanRenderPassCapsule,
}

impl RenderPassBuilder {
    pub fn new() -> Self {
        Self {
            capsule: VulkanRenderPassCapsule::new(),
        }
    }

    /// Add color attachment with common settings
    pub fn add_color_attachment(
        mut self,
        format: u32,
        load_op: LoadOp,
        store_op: StoreOp,
        initial_layout: ImageLayout,
        final_layout: ImageLayout,
    ) -> Self {
        let desc = AttachmentDesc {
            format,
            samples: 1,
            load_op,
            store_op,
            stencil_load_op: LoadOp::DontCare,
            stencil_store_op: StoreOp::DontCare,
            initial_layout,
            final_layout,
        };
        self.capsule.add_attachment(desc);
        self
    }

    /// Add depth/stencil attachment
    pub fn add_depth_stencil_attachment(
        mut self,
        format: u32,
        depth_load_op: LoadOp,
        depth_store_op: StoreOp,
        stencil_load_op: LoadOp,
        stencil_store_op: StoreOp,
        initial_layout: ImageLayout,
        final_layout: ImageLayout,
    ) -> Self {
        let desc = AttachmentDesc {
            format,
            samples: 1,
            load_op: depth_load_op,
            store_op: depth_store_op,
            stencil_load_op,
            stencil_store_op,
            initial_layout,
            final_layout,
        };
        self.capsule.add_attachment(desc);
        self
    }

    /// Add simple subpass with one color attachment
    pub fn add_simple_subpass(mut self, color_attachment_index: u32) -> Self {
        let mut desc = SubpassDesc::default();
        desc.color_attachments[0] = AttachmentRef {
            attachment: color_attachment_index,
            layout: ImageLayout::ColorAttachment,
        };
        desc.color_count = 1;
        self.capsule.add_subpass(desc);
        self
    }

    /// Add external dependency (for synchronization with pre-renderpass work)
    pub fn add_external_dependency(mut self) -> Self {
        let dep = SubpassDependency {
            src_subpass: 0xFFFFFFFF,  // VK_SUBPASS_EXTERNAL
            dst_subpass: 0,
            src_stage_mask: PipelineStage::ColorAttachmentOutput as u32,
            dst_stage_mask: PipelineStage::ColorAttachmentOutput as u32,
            src_access_mask: 0,
            dst_access_mask: AccessFlags::ColorAttachmentWrite as u32,
            dependency_flags: 0,
        };
        self.capsule.add_dependency(dep);
        self
    }

    /// Build the capsule
    pub fn build(self) -> VulkanRenderPassCapsule {
        self.capsule
    }
}

impl Default for RenderPassBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_alignment() {
        use core::mem::{size_of, align_of};

        assert_eq!(align_of::<VulkanRenderPassCapsule>(), 1024);
        assert!(size_of::<VulkanRenderPassCapsule>() >= 1024);
        assert_eq!(size_of::<VulkanRenderPassCapsule>() % 1024, 0);
    }

    #[test]
    fn test_new_capsule() {
        let capsule = VulkanRenderPassCapsule::new();
        assert_eq!(capsule.get_handle(), 0);
        assert_eq!(capsule.get_framebuffer(), 0);
        assert_eq!(capsule.attachment_count, 0);
        assert_eq!(capsule.subpass_count, 0);
        assert_eq!(capsule.dependency_count, 0);
    }

    #[test]
    fn test_handle_operations() {
        let capsule = VulkanRenderPassCapsule::new();

        capsule.set_handle(0x12345678);
        assert_eq!(capsule.get_handle(), 0x12345678);

        capsule.set_framebuffer(0x87654321);
        assert_eq!(capsule.get_framebuffer(), 0x87654321);
    }

    #[test]
    fn test_render_area() {
        let capsule = VulkanRenderPassCapsule::new();

        capsule.set_render_area(10, 20, 1920, 1080);
        let (x, y, w, h) = capsule.get_render_area();
        assert_eq!(x, 10);
        assert_eq!(y, 20);
        assert_eq!(w, 1920);
        assert_eq!(h, 1080);
    }

    #[test]
    fn test_begin_end() {
        let capsule = VulkanRenderPassCapsule::new();

        capsule.begin();
        let (begins, ends, _) = capsule.get_stats();
        assert_eq!(begins, 1);
        assert_eq!(ends, 0);

        capsule.end();
        let (begins, ends, _) = capsule.get_stats();
        assert_eq!(begins, 1);
        assert_eq!(ends, 1);
    }

    #[test]
    fn test_subpass_tracking() {
        let mut capsule = VulkanRenderPassCapsule::new();

        // Add 3 subpasses
        for _ in 0..3 {
            capsule.add_subpass(SubpassDesc::default());
        }

        assert_eq!(capsule.current_subpass_index(), 0);

        capsule.begin();
        assert_eq!(capsule.current_subpass_index(), 0);

        assert_eq!(capsule.next_subpass(), Some(1));
        assert_eq!(capsule.current_subpass_index(), 1);

        assert_eq!(capsule.next_subpass(), Some(2));
        assert_eq!(capsule.current_subpass_index(), 2);

        assert_eq!(capsule.next_subpass(), None);
    }

    #[test]
    fn test_attachment_operations() {
        let mut capsule = VulkanRenderPassCapsule::new();

        let desc = AttachmentDesc {
            format: 37,  // VK_FORMAT_R8G8B8A8_UNORM
            samples: 1,
            load_op: LoadOp::Clear,
            store_op: StoreOp::Store,
            stencil_load_op: LoadOp::DontCare,
            stencil_store_op: StoreOp::DontCare,
            initial_layout: ImageLayout::Undefined,
            final_layout: ImageLayout::ColorAttachment,
        };

        let idx = capsule.add_attachment(desc);
        assert_eq!(idx, Some(0));
        assert_eq!(capsule.attachment_count, 1);

        let retrieved = capsule.get_attachment(0).unwrap();
        assert_eq!(retrieved.format, 37);
        assert_eq!(retrieved.load_op, LoadOp::Clear);
    }

    #[test]
    fn test_max_attachments() {
        let mut capsule = VulkanRenderPassCapsule::new();

        // Add 8 attachments (max)
        for _ in 0..8 {
            assert!(capsule.add_attachment(AttachmentDesc::default()).is_some());
        }

        // 9th should fail
        assert!(capsule.add_attachment(AttachmentDesc::default()).is_none());
    }

    #[test]
    fn test_clear_values() {
        let mut capsule = VulkanRenderPassCapsule::new();

        capsule.set_clear_color(0, 1.0, 0.5, 0.25, 1.0);
        capsule.set_clear_depth_stencil(1, 1.0, 0);

        unsafe {
            assert_eq!(capsule.clear_values[0].color, [1.0, 0.5, 0.25, 1.0]);
            assert_eq!(capsule.clear_values[1].depth_stencil.depth, 1.0);
            assert_eq!(capsule.clear_values[1].depth_stencil.stencil, 0);
        }
    }

    #[test]
    fn test_dynamic_rendering_flag() {
        let capsule = VulkanRenderPassCapsule::new();

        assert!(!capsule.is_dynamic_rendering());

        capsule.enable_dynamic_rendering();
        assert!(capsule.is_dynamic_rendering());
    }

    #[test]
    fn test_builder_simple() {
        let capsule = RenderPassBuilder::new()
            .add_color_attachment(
                37,  // VK_FORMAT_R8G8B8A8_UNORM
                LoadOp::Clear,
                StoreOp::Store,
                ImageLayout::Undefined,
                ImageLayout::PresentSrc,
            )
            .add_simple_subpass(0)
            .add_external_dependency()
            .build();

        assert_eq!(capsule.attachment_count, 1);
        assert_eq!(capsule.subpass_count, 1);
        assert_eq!(capsule.dependency_count, 1);
    }

    #[test]
    fn test_reset() {
        let mut capsule = VulkanRenderPassCapsule::new();

        capsule.set_handle(123);
        capsule.begin();
        capsule.add_attachment(AttachmentDesc::default());

        capsule.reset();

        assert_eq!(capsule.get_handle(), 0);
        assert_eq!(capsule.attachment_count, 0);
        let (begins, ends, advances) = capsule.get_stats();
        assert_eq!(begins, 0);
        assert_eq!(ends, 0);
        assert_eq!(advances, 0);
    }
}
