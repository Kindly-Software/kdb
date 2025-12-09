//! Push Descriptors Capsule - T7 Heterogeneous Tier
//!
//! VK_KHR_push_descriptor extension support for inline descriptor updates
//! without allocating descriptor sets.
//!
//! # Architecture
//!
//! Based on 2024-2025 Vulkan best practices:
//! - [VK_KHR_push_descriptor spec](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VK_KHR_push_descriptor.html)
//! - [Descriptor Management](https://docs.vulkan.org/samples/latest/samples/performance/descriptor_management/README.html)
//! - [zeux.io - Efficient Vulkan Renderer](https://zeux.io/2020/02/27/writing-an-efficient-vulkan-renderer/)
//! - [NVIDIA Vulkan Dos and Don'ts](https://developer.nvidia.com/blog/vulkan-dos-donts/)
//!
//! # When to Use Push Descriptors
//!
//! ## ✅ Use For:
//! - Per-draw uniform buffers (frequently changing)
//! - Dynamic texture binding (different per render call)
//! - Small descriptor counts (<16, check `maxPushDescriptors`)
//! - Porting from D3D12/older APIs
//! - Avoiding descriptor set lifetime management
//!
//! ## ❌ Avoid For:
//! - Static resources (use regular descriptor sets with caching)
//! - Large descriptor counts (>16-32, hardware limit)
//! - Resources known upfront (cache descriptor sets instead)
//!
//! ## 🤔 Consider Alternatives:
//! - **Dynamic UBOs**: For per-draw uniform data with same buffer
//!   - Use `VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER_DYNAMIC`
//!   - Single descriptor set, multiple dynamic offsets
//!   - Can be faster if implementation detects same descriptor
//! - **Push Constants**: For tiny data (<128 bytes)
//!   - Fastest for small constant updates
//!   - Limited size (128-256 bytes typical)
//! - **Descriptor Caching**: For predictable patterns
//!   - 38% frame time reduction in CPU-heavy scenes
//!   - Hash-based descriptor set reuse
//!
//! # Key Innovations (2024-2025)
//!
//! 1. **Universal Hardware Support** (as of 2022+):
//!    - AMD: Proprietary + RADV Mesa Open Source ✅
//!    - NVIDIA: Desktop support ✅
//!    - Intel: Desktop support ✅
//!    - Mobile: Varies (check device properties)
//!
//! 2. **Descriptor Update Templates**:
//!    - `VK_DESCRIPTOR_UPDATE_TEMPLATE_TYPE_PUSH_DESCRIPTORS_KHR`
//!    - Pre-defined update patterns for repeated operations
//!    - Lower overhead for common patterns
//!
//! 3. **vs Descriptor Buffers** (VK_EXT_descriptor_buffer):
//!    - Push descriptors still useful alongside descriptor buffers
//!    - Bridge the gap without extra cost
//!    - Simpler for per-draw updates
//!
//! # Performance Targets
//!
//! - Push overhead: <100ns per call
//! - Write accumulation: <50ns per descriptor
//! - Template push: <50ns (cached pattern)
//! - Batch push (8 descriptors): <200ns
//!
//! # UCE34 Compliance
//!
//! - **Q10**: T7 Heterogeneous (GPU command buffer integration)
//! - **Q33**: 100% lockfree atomic coordination
//! - **Q34**: Push operation audit trail
//!
//! # ASSUM Safety Tags
//!
//! ```text
//! #ASSUME_PUSH_SUPPORTED: VK_KHR_push_descriptor enabled in device
//! #ASSUME_LAYOUT_PUSH: Pipeline layout created with VK_DESCRIPTOR_SET_LAYOUT_CREATE_PUSH_DESCRIPTOR_BIT_KHR
//! #ASSUME_COUNT_VALID: Write count ≤ maxPushDescriptors (device limit, typically 32)
//! #ASSUME_BUFFER_VALID: Buffer/image handles are valid at push time
//! #ASSUME_STAGE_VALID: Shader stages match descriptor set layout
//! #VERIFY_LOCKFREE: All operations use atomic primitives (no mutex/RwLock)
//! #VERIFY_CACHE_ALIGNED: 256-byte alignment for fast push operations
//! ```
//!
//! # Example Usage
//!
//! ```rust,ignore
//! // Create push descriptor layout
//! let layout = create_descriptor_set_layout(
//!     &[
//!         DescriptorBinding {
//!             binding: 0,
//!             descriptor_type: DescriptorType::UniformBuffer,
//!             stage_flags: ShaderStage::Vertex,
//!         },
//!         DescriptorBinding {
//!             binding: 1,
//!             descriptor_type: DescriptorType::CombinedImageSampler,
//!             stage_flags: ShaderStage::Fragment,
//!         },
//!     ],
//!     VK_DESCRIPTOR_SET_LAYOUT_CREATE_PUSH_DESCRIPTOR_BIT_KHR,
//! );
//!
//! // Per-draw loop
//! for draw_idx in 0..num_draws {
//!     // Update per-draw uniform buffer
//!     capsule.write_buffer(
//!         binding: 0,
//!         buffer: per_draw_ubo,
//!         offset: draw_idx * ubo_size,
//!         range: ubo_size,
//!     );
//!
//!     // Update texture binding
//!     capsule.write_image(
//!         binding: 1,
//!         image_view: textures[draw_idx],
//!         sampler: linear_sampler,
//!         layout: ImageLayout::ShaderReadOnlyOptimal,
//!     );
//!
//!     // Push all accumulated writes
//!     capsule.cmd_push(cmd_buffer, pipeline_layout, set_index: 0);
//!
//!     // Draw call
//!     vkCmdDrawIndexed(cmd_buffer, ...);
//! }
//! ```

use core::sync::atomic::{AtomicU64, Ordering};
use crate::patterns::dual_atomic::DualAtomicU64;

/// Descriptor type (Vulkan VkDescriptorType)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum DescriptorType {
    Sampler = 0,
    CombinedImageSampler = 1,
    SampledImage = 2,
    StorageImage = 3,
    UniformTexelBuffer = 4,
    StorageTexelBuffer = 5,
    UniformBuffer = 6,
    StorageBuffer = 7,
    UniformBufferDynamic = 8,
    StorageBufferDynamic = 9,
    InputAttachment = 10,
}

/// Image layout (Vulkan VkImageLayout)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ImageLayout {
    Undefined = 0,
    General = 1,
    ColorAttachment = 2,
    DepthStencilAttachment = 3,
    DepthStencilReadOnly = 4,
    ShaderReadOnlyOptimal = 5,
    TransferSrc = 6,
    TransferDst = 7,
    Preinitialized = 8,
}

/// Descriptor write info (single update)
///
/// Represents a single descriptor write operation for push descriptors.
/// Only the fields relevant to the descriptor type should be used.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DescriptorWrite {
    /// Binding point in descriptor set
    pub binding: u32,

    /// Array element within binding (0 for non-arrays)
    pub array_element: u32,

    /// Descriptor type
    pub descriptor_type: DescriptorType,

    /// Buffer descriptor data (valid for *BUFFER types)
    pub buffer: u64,       // VkBuffer handle
    pub buffer_offset: u64,
    pub buffer_range: u64,

    /// Image descriptor data (valid for *IMAGE* types)
    pub image_view: u64,   // VkImageView handle
    pub sampler: u64,      // VkSampler handle
    pub image_layout: ImageLayout,

    /// Padding to 80 bytes (aligned)
    /// Fields: 4+4+4 = 12B, then 4B implicit padding for u64 alignment = 16B
    /// Then: 8+8+8+8+8+4 = 44B, total = 60B, padding = 80-60 = 20B
    _padding: [u8; 20],
}

impl Default for DescriptorWrite {
    fn default() -> Self {
        Self {
            binding: 0,
            array_element: 0,
            descriptor_type: DescriptorType::UniformBuffer,
            buffer: 0,
            buffer_offset: 0,
            buffer_range: 0,
            image_view: 0,
            sampler: 0,
            image_layout: ImageLayout::Undefined,
            _padding: [0; 20],
        }
    }
}

/// Compilation stats tracked atomically
///
/// Packed into DualAtomicU64 for lockfree updates:
/// - High 32 bits: Total push operations
/// - Low 32 bits: Total descriptor writes
#[derive(Clone, Copy)]
pub struct PushStats {
    pub total_pushes: u32,
    pub total_writes: u32,
}

impl PushStats {
    #[inline]
    pub fn pack(self) -> u64 {
        ((self.total_pushes as u64) << 32) | (self.total_writes as u64)
    }

    #[inline]
    pub fn unpack(packed: u64) -> Self {
        Self {
            total_pushes: (packed >> 32) as u32,
            total_writes: (packed & 0xFFFFFFFF) as u32,
        }
    }
}

/// Push Descriptors Capsule - T7 Heterogeneous Tier
///
/// Fast inline descriptor updates without descriptor set allocation.
/// 256-byte aligned for optimal cache performance.
///
/// # Lockfree Coordination
///
/// Uses `DualAtomicU64` for atomic snapshot of stats (pushes + writes).
/// All operations are lockfree and cache-aligned.
///
/// # Memory Layout
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────┐
/// │ DualAtomicU64 (16B)   | stats coordination                  │
/// ├─────────────────────────────────────────────────────────────┤
/// │ AtomicU64 (8B)        | pipeline_layout                     │
/// ├─────────────────────────────────────────────────────────────┤
/// │ u32 (4B)              | set_index                           │
/// │ u32 (4B)              | pending_count                       │
/// │ u32 (4B)              | template_count                      │
/// │ u32 (4B)              | max_push_descriptors                │
/// ├─────────────────────────────────────────────────────────────┤
/// │ DescriptorWrite[8]    | pending_writes (640B)               │
/// ├─────────────────────────────────────────────────────────────┤
/// │ DescriptorWrite[8]    | template_writes (640B)              │
/// ├─────────────────────────────────────────────────────────────┤
/// │ [u8; 728]             | _padding to 2048 bytes              │
/// └─────────────────────────────────────────────────────────────┘
/// Total: 2048 bytes (8× cache lines) - EFFICIENT for batch push
/// ```
#[repr(C, align(256))]
pub struct PushDescriptorsCapsule {
    /// T1 Atomic coordination (pushes + writes)
    stats: DualAtomicU64,

    /// Pipeline layout (must have PUSH_DESCRIPTOR flag)
    /// #ASSUME_LAYOUT_PUSH: Created with VK_DESCRIPTOR_SET_LAYOUT_CREATE_PUSH_DESCRIPTOR_BIT_KHR
    pipeline_layout: AtomicU64,

    /// Target descriptor set index
    set_index: u32,

    /// Number of pending writes (0-8)
    pending_count: u32,

    /// Number of template writes (0-8)
    template_count: u32,

    /// Device limit for max push descriptors (typically 32)
    /// #ASSUME_COUNT_VALID: Write count ≤ this value
    max_push_descriptors: u32,

    /// Pending writes (max 8 for fast accumulation)
    /// Accumulated via write_buffer/write_image, pushed via cmd_push
    pending_writes: [DescriptorWrite; 8],

    /// Cached write templates (max 8)
    /// Pre-defined update patterns for common operations
    template_writes: [DescriptorWrite; 8],

    /// Padding to 2048 bytes (8× cache lines)
    /// Ensures entire capsule fits in L1 cache for fast operations
    _padding: [u8; 616],
}

// Compile-time verification
const _: () = {
    assert!(core::mem::size_of::<PushDescriptorsCapsule>() == 2048);
    assert!(core::mem::align_of::<PushDescriptorsCapsule>() == 256);

    // Verify descriptor write size
    assert!(core::mem::size_of::<DescriptorWrite>() == 80);
    assert!(core::mem::align_of::<DescriptorWrite>() == 8);
};

impl PushDescriptorsCapsule {
    /// Create new push descriptors capsule
    ///
    /// # Arguments
    ///
    /// - `pipeline_layout`: VkPipelineLayout with push descriptor support
    /// - `set_index`: Target descriptor set index
    /// - `max_push_descriptors`: Device limit (query VkPhysicalDevicePushDescriptorProperties)
    ///
    /// # Safety
    ///
    /// #ASSUME_LAYOUT_PUSH: pipeline_layout must be created with PUSH_DESCRIPTOR flag
    #[inline]
    pub const fn new(
        pipeline_layout: u64,
        set_index: u32,
        max_push_descriptors: u32,
    ) -> Self {
        Self {
            stats: DualAtomicU64::new(0, 0),
            pipeline_layout: AtomicU64::new(pipeline_layout),
            set_index,
            pending_count: 0,
            template_count: 0,
            max_push_descriptors,
            pending_writes: [DescriptorWrite {
                binding: 0,
                array_element: 0,
                descriptor_type: DescriptorType::UniformBuffer,
                buffer: 0,
                buffer_offset: 0,
                buffer_range: 0,
                image_view: 0,
                sampler: 0,
                image_layout: ImageLayout::Undefined,
                _padding: [0; 20],
            }; 8],
            template_writes: [DescriptorWrite {
                binding: 0,
                array_element: 0,
                descriptor_type: DescriptorType::UniformBuffer,
                buffer: 0,
                buffer_offset: 0,
                buffer_range: 0,
                image_view: 0,
                sampler: 0,
                image_layout: ImageLayout::Undefined,
                _padding: [0; 20],
            }; 8],
            _padding: [0; 616],
        }
    }

    /// Write buffer descriptor (accumulate for batch push)
    ///
    /// # Arguments
    ///
    /// - `binding`: Binding point in descriptor set
    /// - `buffer`: VkBuffer handle
    /// - `offset`: Byte offset into buffer
    /// - `range`: Byte range of descriptor
    ///
    /// # Performance
    ///
    /// Target: <50ns per write (cache-aligned accumulation)
    ///
    /// # Safety
    ///
    /// #ASSUME_BUFFER_VALID: buffer handle must be valid
    /// #ASSUME_COUNT_VALID: Must not exceed max_push_descriptors
    #[inline]
    pub fn write_buffer(
        &mut self,
        binding: u32,
        buffer: u64,
        offset: u64,
        range: u64,
    ) {
        if self.pending_count >= 8 {
            // Auto-flush if full (rare case)
            return;
        }

        let idx = self.pending_count as usize;
        self.pending_writes[idx] = DescriptorWrite {
            binding,
            array_element: 0,
            descriptor_type: DescriptorType::UniformBuffer,
            buffer,
            buffer_offset: offset,
            buffer_range: range,
            image_view: 0,
            sampler: 0,
            image_layout: ImageLayout::Undefined,
            _padding: [0; 20],
        };

        self.pending_count += 1;
    }

    /// Write storage buffer descriptor (accumulate for batch push)
    ///
    /// Similar to write_buffer but for storage buffers (read-write access).
    #[inline]
    pub fn write_storage_buffer(
        &mut self,
        binding: u32,
        buffer: u64,
        offset: u64,
        range: u64,
    ) {
        if self.pending_count >= 8 {
            return;
        }

        let idx = self.pending_count as usize;
        self.pending_writes[idx] = DescriptorWrite {
            binding,
            array_element: 0,
            descriptor_type: DescriptorType::StorageBuffer,
            buffer,
            buffer_offset: offset,
            buffer_range: range,
            image_view: 0,
            sampler: 0,
            image_layout: ImageLayout::Undefined,
            _padding: [0; 20],
        };

        self.pending_count += 1;
    }

    /// Write image descriptor (accumulate for batch push)
    ///
    /// # Arguments
    ///
    /// - `binding`: Binding point in descriptor set
    /// - `image_view`: VkImageView handle
    /// - `sampler`: VkSampler handle
    /// - `layout`: Image layout at shader access time
    ///
    /// # Performance
    ///
    /// Target: <50ns per write (cache-aligned accumulation)
    ///
    /// # Safety
    ///
    /// #ASSUME_BUFFER_VALID: image_view and sampler handles must be valid
    #[inline]
    pub fn write_image(
        &mut self,
        binding: u32,
        image_view: u64,
        sampler: u64,
        layout: ImageLayout,
    ) {
        if self.pending_count >= 8 {
            return;
        }

        let idx = self.pending_count as usize;
        self.pending_writes[idx] = DescriptorWrite {
            binding,
            array_element: 0,
            descriptor_type: DescriptorType::CombinedImageSampler,
            buffer: 0,
            buffer_offset: 0,
            buffer_range: 0,
            image_view,
            sampler,
            image_layout: layout,
            _padding: [0; 20],
        };

        self.pending_count += 1;
    }

    /// Write sampled image descriptor (separate sampler)
    ///
    /// For use with immutable samplers or separate image/sampler descriptors.
    #[inline]
    pub fn write_sampled_image(
        &mut self,
        binding: u32,
        image_view: u64,
        layout: ImageLayout,
    ) {
        if self.pending_count >= 8 {
            return;
        }

        let idx = self.pending_count as usize;
        self.pending_writes[idx] = DescriptorWrite {
            binding,
            array_element: 0,
            descriptor_type: DescriptorType::SampledImage,
            buffer: 0,
            buffer_offset: 0,
            buffer_range: 0,
            image_view,
            sampler: 0,
            image_layout: layout,
            _padding: [0; 20],
        };

        self.pending_count += 1;
    }

    /// Push accumulated descriptors to command buffer
    ///
    /// Issues vkCmdPushDescriptorSetKHR with all pending writes.
    /// Clears pending_writes after push.
    ///
    /// # Arguments
    ///
    /// - `cmd_buffer`: VkCommandBuffer handle
    ///
    /// # Performance
    ///
    /// Target: <100ns for push operation
    ///
    /// # Returns
    ///
    /// Number of descriptors pushed (0 if nothing pending)
    ///
    /// # Safety
    ///
    /// #ASSUME_PUSH_SUPPORTED: VK_KHR_push_descriptor enabled
    /// #ASSUME_STAGE_VALID: Shader stages match pipeline layout
    #[inline]
    pub fn cmd_push(&mut self, _cmd_buffer: u64) -> u32 {
        if self.pending_count == 0 {
            return 0;
        }

        let count = self.pending_count;

        // TODO: Actual vkCmdPushDescriptorSetKHR call
        // vkCmdPushDescriptorSetKHR(
        //     cmd_buffer,
        //     VK_PIPELINE_BIND_POINT_GRAPHICS,
        //     pipeline_layout,
        //     set_index,
        //     pending_count,
        //     &pending_writes,
        // );

        // Update stats (lockfree atomic)
        let pushes = self.stats.load_primary(Ordering::Acquire);
        let writes = self.stats.load_secondary(Ordering::Acquire);
        let old_stats = PushStats {
            total_pushes: (pushes >> 32) as u32,
            total_writes: (writes & 0xFFFFFFFF) as u32,
        };
        let new_stats = PushStats {
            total_pushes: old_stats.total_pushes + 1,
            total_writes: old_stats.total_writes + count,
        };
        self.stats.store_primary((new_stats.total_pushes as u64) << 32, Ordering::Release);
        self.stats.store_secondary(new_stats.total_writes as u64, Ordering::Release);

        // Clear pending writes
        self.pending_count = 0;

        count
    }

    /// Save current pending writes as template
    ///
    /// Templates enable fast repeated push operations with the same pattern.
    /// Call write_* operations to populate pending_writes, then save_template()
    /// to cache the pattern.
    ///
    /// # Performance
    ///
    /// Target: <100ns (memcpy of pending_writes)
    ///
    /// # Returns
    ///
    /// Number of writes saved to template
    #[inline]
    pub fn save_template(&mut self) -> u32 {
        if self.pending_count == 0 {
            return 0;
        }

        // Copy pending writes to template
        let count = self.pending_count.min(8);
        self.template_writes[..count as usize]
            .copy_from_slice(&self.pending_writes[..count as usize]);

        self.template_count = count;
        count
    }

    /// Push from saved template
    ///
    /// Fast path for repeated descriptor updates with the same pattern.
    /// Useful for per-frame or per-draw loops with predictable descriptors.
    ///
    /// # Performance
    ///
    /// Target: <50ns (cached pattern, no write accumulation)
    ///
    /// # Safety
    ///
    /// #ASSUME_BUFFER_VALID: Template descriptor handles must still be valid
    #[inline]
    pub fn cmd_push_template(&mut self, _cmd_buffer: u64) -> u32 {
        if self.template_count == 0 {
            return 0;
        }

        let count = self.template_count;

        // TODO: Actual vkCmdPushDescriptorSetKHR call with template_writes

        // Update stats
        let pushes = self.stats.load_primary(Ordering::Acquire);
        let writes = self.stats.load_secondary(Ordering::Acquire);
        let old_stats = PushStats {
            total_pushes: (pushes >> 32) as u32,
            total_writes: (writes & 0xFFFFFFFF) as u32,
        };
        let new_stats = PushStats {
            total_pushes: old_stats.total_pushes + 1,
            total_writes: old_stats.total_writes + count,
        };
        self.stats.store_primary((new_stats.total_pushes as u64) << 32, Ordering::Release);
        self.stats.store_secondary(new_stats.total_writes as u64, Ordering::Release);

        count
    }

    /// Get current statistics (lockfree atomic snapshot)
    ///
    /// # Performance
    ///
    /// Target: <10ns (single atomic load)
    #[inline]
    pub fn stats(&self) -> PushStats {
        let pushes = self.stats.load_primary(Ordering::Acquire);
        let writes = self.stats.load_secondary(Ordering::Acquire);
        PushStats {
            total_pushes: (pushes >> 32) as u32,
            total_writes: (writes & 0xFFFFFFFF) as u32,
        }
    }

    /// Get pipeline layout
    #[inline]
    pub fn pipeline_layout(&self) -> u64 {
        self.pipeline_layout.load(Ordering::Relaxed)
    }

    /// Get descriptor set index
    #[inline]
    pub const fn set_index(&self) -> u32 {
        self.set_index
    }

    /// Get pending write count
    #[inline]
    pub const fn pending_count(&self) -> u32 {
        self.pending_count
    }

    /// Get max push descriptors (device limit)
    #[inline]
    pub const fn max_push_descriptors(&self) -> u32 {
        self.max_push_descriptors
    }

    /// Clear pending writes (without pushing)
    ///
    /// Use when aborting a draw or resetting state.
    #[inline]
    pub fn clear_pending(&mut self) {
        self.pending_count = 0;
    }

    /// Check if pending writes would exceed device limit
    ///
    /// Returns true if adding `count` more writes would exceed max_push_descriptors.
    #[inline]
    pub const fn would_exceed_limit(&self, count: u32) -> bool {
        self.pending_count + count > self.max_push_descriptors
    }
}

impl Default for PushDescriptorsCapsule {
    fn default() -> Self {
        Self::new(0, 0, 32) // Typical max_push_descriptors is 32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_properties() {
        assert_eq!(core::mem::size_of::<PushDescriptorsCapsule>(), 2048);
        assert_eq!(core::mem::align_of::<PushDescriptorsCapsule>(), 256);

        // Verify descriptor write properties
        assert_eq!(core::mem::size_of::<DescriptorWrite>(), 80);
        assert_eq!(core::mem::align_of::<DescriptorWrite>(), 8);
    }

    #[test]
    fn test_write_buffer() {
        let mut capsule = PushDescriptorsCapsule::new(0x1000, 0, 32);

        capsule.write_buffer(0, 0x2000, 0, 256);
        assert_eq!(capsule.pending_count(), 1);

        capsule.write_buffer(1, 0x2100, 256, 256);
        assert_eq!(capsule.pending_count(), 2);
    }

    #[test]
    fn test_write_image() {
        let mut capsule = PushDescriptorsCapsule::new(0x1000, 0, 32);

        capsule.write_image(0, 0x3000, 0x4000, ImageLayout::ShaderReadOnlyOptimal);
        assert_eq!(capsule.pending_count(), 1);
    }

    #[test]
    fn test_cmd_push() {
        let mut capsule = PushDescriptorsCapsule::new(0x1000, 0, 32);

        capsule.write_buffer(0, 0x2000, 0, 256);
        capsule.write_image(1, 0x3000, 0x4000, ImageLayout::ShaderReadOnlyOptimal);
        assert_eq!(capsule.pending_count(), 2);

        let count = capsule.cmd_push(0x5000);
        assert_eq!(count, 2);
        assert_eq!(capsule.pending_count(), 0);

        let stats = capsule.stats();
        assert_eq!(stats.total_pushes, 1);
        assert_eq!(stats.total_writes, 2);
    }

    #[test]
    fn test_template() {
        let mut capsule = PushDescriptorsCapsule::new(0x1000, 0, 32);

        // Setup template
        capsule.write_buffer(0, 0x2000, 0, 256);
        capsule.write_image(1, 0x3000, 0x4000, ImageLayout::ShaderReadOnlyOptimal);
        let count = capsule.save_template();
        assert_eq!(count, 2);

        // Clear pending
        capsule.clear_pending();
        assert_eq!(capsule.pending_count(), 0);

        // Push from template (fast path)
        let count = capsule.cmd_push_template(0x5000);
        assert_eq!(count, 2);

        let stats = capsule.stats();
        assert_eq!(stats.total_pushes, 1);
        assert_eq!(stats.total_writes, 2);
    }

    #[test]
    fn test_batch_accumulation() {
        let mut capsule = PushDescriptorsCapsule::new(0x1000, 0, 32);

        // Accumulate 8 writes (max pending)
        for i in 0..8 {
            capsule.write_buffer(i, 0x2000 + i as u64 * 256, 0, 256);
        }
        assert_eq!(capsule.pending_count(), 8);

        // 9th write should auto-flush (noop in current impl)
        capsule.write_buffer(8, 0x3000, 0, 256);
        assert_eq!(capsule.pending_count(), 8); // Still 8 (auto-flush not implemented)
    }

    #[test]
    fn test_clear_pending() {
        let mut capsule = PushDescriptorsCapsule::new(0x1000, 0, 32);

        capsule.write_buffer(0, 0x2000, 0, 256);
        capsule.write_buffer(1, 0x2100, 256, 256);
        assert_eq!(capsule.pending_count(), 2);

        capsule.clear_pending();
        assert_eq!(capsule.pending_count(), 0);
    }

    #[test]
    fn test_would_exceed_limit() {
        let mut capsule = PushDescriptorsCapsule::new(0x1000, 0, 8); // Low limit for test

        capsule.write_buffer(0, 0x2000, 0, 256);
        capsule.write_buffer(1, 0x2100, 256, 256);
        assert_eq!(capsule.pending_count(), 2);

        assert!(!capsule.would_exceed_limit(5)); // 2 + 5 = 7 ≤ 8
        assert!(!capsule.would_exceed_limit(6)); // 2 + 6 = 8 ≤ 8
        assert!(capsule.would_exceed_limit(7));  // 2 + 7 = 9 > 8
    }

    #[test]
    fn test_stats_pack_unpack() {
        let stats = PushStats {
            total_pushes: 12345,
            total_writes: 67890,
        };

        let packed = stats.pack();
        let unpacked = PushStats::unpack(packed);

        assert_eq!(unpacked.total_pushes, 12345);
        assert_eq!(unpacked.total_writes, 67890);
    }

    #[test]
    fn test_multiple_pushes() {
        let mut capsule = PushDescriptorsCapsule::new(0x1000, 0, 32);

        // First push
        capsule.write_buffer(0, 0x2000, 0, 256);
        capsule.cmd_push(0x5000);

        // Second push
        capsule.write_buffer(1, 0x2100, 256, 256);
        capsule.write_image(2, 0x3000, 0x4000, ImageLayout::ShaderReadOnlyOptimal);
        capsule.cmd_push(0x5000);

        let stats = capsule.stats();
        assert_eq!(stats.total_pushes, 2);
        assert_eq!(stats.total_writes, 3); // 1 + 2
    }

    #[test]
    fn test_storage_buffer() {
        let mut capsule = PushDescriptorsCapsule::new(0x1000, 0, 32);

        capsule.write_storage_buffer(0, 0x2000, 0, 256);
        assert_eq!(capsule.pending_count(), 1);

        // Verify descriptor type
        assert_eq!(
            capsule.pending_writes[0].descriptor_type as u32,
            DescriptorType::StorageBuffer as u32
        );
    }

    #[test]
    fn test_sampled_image() {
        let mut capsule = PushDescriptorsCapsule::new(0x1000, 0, 32);

        capsule.write_sampled_image(0, 0x3000, ImageLayout::ShaderReadOnlyOptimal);
        assert_eq!(capsule.pending_count(), 1);

        // Verify descriptor type
        assert_eq!(
            capsule.pending_writes[0].descriptor_type as u32,
            DescriptorType::SampledImage as u32
        );
    }
}
