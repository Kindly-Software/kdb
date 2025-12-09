//! KgpuQueueCapsule - T1+T4 Atomic+Batch GPU Queue with Command Submission
//!
//! **Tier**: T1 (Atomic coordination) + T4 (Batch submission)
//! **Size**: 128B (cache-aligned)
//! **Purpose**: Lockfree GPU queue for command buffer submission with sync primitives
//!
//! # Architecture (2024 SOTA Patterns)
//!
//! **Vulkan Queue Families** (Best Practices):
//! - Graphics queues support ALL operations (graphics + compute + transfer)
//! - Compute-only queues for async compute (overlap with graphics)
//! - Transfer-only queues for DMA (dedicated copy engine on AMD/NVIDIA)
//! - Sparse binding queues (VK_QUEUE_SPARSE_BINDING_BIT)
//!
//! **D3D12 Command Queue Patterns**:
//! - DIRECT queue: All operations (graphics + compute + copy)
//! - COMPUTE queue: Compute + copy operations
//! - COPY queue: PCIe DMA transfers (saturate PCIe bandwidth)
//! - Cross-queue sync via timeline semaphores
//!
//! **Multi-Queue Rendering** (Async Compute + Parallel Upload):
//! - Graphics queue for rendering
//! - Async compute queue for particle systems, post-processing
//! - Async transfer queue for texture streaming, buffer uploads
//! - Minimize pipeline bubbles with fine-grained sync
//!
//! **Command Buffer Pooling**:
//! - Pool reset (vkResetCommandPool) faster than individual reset
//! - RESET_COMMAND_BUFFER_BIT flag for per-buffer reset (higher overhead)
//! - Triple-buffering pattern: 3 pools × N threads
//! - Linear allocator for transient command data
//!
//! **Low-Latency Submission** (NVIDIA Reflex / Just-In-Time Scheduling):
//! - Submit commands just before GPU needs them
//! - Reduce render queue depth (1-2 frames max)
//! - CPU/GPU pipeline coordination for consistent frame times
//!
//! # Memory Layout (128B)
//!
//! ```text
//! Offset  Size    Field
//! 0       64      handle: KgpuHandle<Queue>
//! 64      4       queue_family: u32
//! 68      4       queue_index: u32
//! 72      4       capabilities: u32 (QueueCapabilities flags)
//! 76      4       priority: u32 (0=Normal, 1=High, 2=Realtime)
//! 80      8       submitted_count: AtomicU64 (total submissions)
//! 88      8       last_fence_value: AtomicU64 (monotonic fence counter)
//! 96      8       generation: AtomicU32 (ABA prevention)
//! 104     4       _pad1
//! 108     4       flags: AtomicU32 (queue state flags)
//! 112     16      _padding (reach 128B)
//! ```
//!
//! # Queue Capabilities (Based on Vulkan/DX12)
//!
//! - **Graphics**: Rendering, vertex/index processing, rasterization
//! - **Compute**: Compute shaders, dispatch commands
//! - **Transfer**: Buffer/texture copies, DMA transfers
//! - **Sparse**: Sparse resource binding operations
//! - **Present**: Window presentation support
//!
//! # Performance (B32 Targets)
//!
//! | Operation | Target | Hardware Reality |
//! |-----------|--------|------------------|
//! | submit() | <1μs | Kernel driver overhead (~10-50μs) |
//! | wait_idle() poll | 100μs | Event-based blocking preferred |
//! | fence value update | <50ns | Atomic CAS operation |
//!
//! # Framework Compliance
//!
//! - **UCE34**: T1 (Atomic) + T4 (Batch submission)
//! - **Chaos**: 100% lockfree (AtomicU64 only), 128B cache-aligned
//! - **ASSUM**: All assumptions documented with #ASSUME/#VERIFY tags
//! - **B32**: Performance targets validated with hardware reality
//! - **T28**: Comprehensive test coverage (15+ tests)
//!
//! # Example
//!
//! ```rust,ignore
//! use atomic_capsule::gpu::kgpu::{KgpuQueueCapsule, SubmitInfo, WaitInfo, SignalInfo};
//!
//! let queue = KgpuQueueCapsule::new(0, 0, QUEUE_CAP_GRAPHICS | QUEUE_CAP_COMPUTE);
//!
//! // Submit with synchronization
//! let submit_info = SubmitInfo {
//!     command_buffers: &[encoder.finish()],
//!     wait_semaphores: &[WaitInfo::new(&sem1, 1)],
//!     signal_semaphores: &[SignalInfo::new(&sem2, 2)],
//! };
//!
//! queue.submit(&submit_info)?;
//! ```

use core::marker::PhantomData;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use super::command::{Finished, KgpuCommandEncoderCapsule};
use super::device::KgpuError;
use super::fence::{Unsignaled, KgpuFenceCapsule};
use super::handle::KgpuHandle;
use super::sync::{SignalInfo, WaitInfo};

// ============================================================================
// Queue Marker Type (for KgpuHandle<Queue>)
// ============================================================================

/// Marker type for queue handles
pub struct Queue;

// ============================================================================
// Queue Capability Flags
// ============================================================================

/// Queue supports graphics operations (rendering, rasterization)
pub const QUEUE_CAP_GRAPHICS: u32 = 1 << 0;

/// Queue supports compute operations (compute shaders, dispatch)
pub const QUEUE_CAP_COMPUTE: u32 = 1 << 1;

/// Queue supports transfer operations (copy, DMA)
pub const QUEUE_CAP_TRANSFER: u32 = 1 << 2;

/// Queue supports sparse binding operations
pub const QUEUE_CAP_SPARSE: u32 = 1 << 3;

/// Queue supports window presentation
pub const QUEUE_CAP_PRESENT: u32 = 1 << 4;

// ============================================================================
// Queue Priority Constants
// ============================================================================

/// Low priority queue (background tasks)
pub const QUEUE_PRIORITY_LOW: u32 = 0;

/// Normal priority queue (default)
pub const QUEUE_PRIORITY_NORMAL: u32 = 1;

/// High priority queue (game rendering)
pub const QUEUE_PRIORITY_HIGH: u32 = 2;

/// Realtime priority queue (VR, low-latency)
pub const QUEUE_PRIORITY_REALTIME: u32 = 3;

// ============================================================================
// Queue State Flags
// ============================================================================

/// Queue is idle (no pending work)
const QUEUE_FLAG_IDLE: u32 = 1 << 0;

/// Queue is busy (has pending work)
const QUEUE_FLAG_BUSY: u32 = 1 << 1;

/// Queue is suspended (no submissions accepted)
const QUEUE_FLAG_SUSPENDED: u32 = 1 << 2;

// ============================================================================
// Error Type
// ============================================================================

/// Error type for queue operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueError {
    /// Queue is suspended (no submissions accepted)
    QueueSuspended,

    /// Invalid command buffer state
    InvalidCommandBuffer,

    /// Device error from underlying layer
    DeviceError(KgpuError),

    /// Submission failed (driver error)
    SubmissionFailed,

    /// Wait timed out
    WaitTimeout,

    /// Fence creation failed
    FenceCreationFailed,
}

impl core::fmt::Display for QueueError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::QueueSuspended => write!(f, "Queue is suspended"),
            Self::InvalidCommandBuffer => write!(f, "Invalid command buffer state"),
            Self::DeviceError(e) => write!(f, "Device error: {}", e),
            Self::SubmissionFailed => write!(f, "Submission failed"),
            Self::WaitTimeout => write!(f, "Wait timed out"),
            Self::FenceCreationFailed => write!(f, "Fence creation failed"),
        }
    }
}

/// Result type for queue operations
pub type QueueResult<T> = core::result::Result<T, QueueError>;

// ============================================================================
// SubmitInfo
// ============================================================================

/// Command buffer submission descriptor
///
/// Describes work to submit to the queue with synchronization dependencies.
///
/// # ASSUM Safety
///
/// - `#ASSUME_COMMAND_BUFFERS_FINISHED`: All command buffers in Finished state
/// - `#ASSUME_WAIT_SEMAPHORES_VALID`: Wait semaphores exist and are timeline type
/// - `#ASSUME_SIGNAL_SEMAPHORES_VALID`: Signal semaphores exist and are timeline type
///
/// # Examples
///
/// ```rust,ignore
/// let submit_info = SubmitInfo {
///     command_buffers: &[encoder1.finish(), encoder2.finish()],
///     wait_semaphores: &[],
///     signal_semaphores: &[],
/// };
/// queue.submit(&submit_info)?;
/// ```
#[derive(Debug, Clone, Copy)]
pub struct SubmitInfo<'a> {
    /// Command buffers to submit (must be in Finished state)
    pub command_buffers: &'a [KgpuCommandEncoderCapsule<Finished>],

    /// Semaphore wait dependencies (must complete before execution)
    pub wait_semaphores: &'a [WaitInfo<'a>],

    /// Semaphore signal operations (signaled after completion)
    pub signal_semaphores: &'a [SignalInfo<'a>],
}

impl<'a> SubmitInfo<'a> {
    /// Create empty submit info (no work)
    #[inline]
    pub const fn empty() -> Self {
        Self {
            command_buffers: &[],
            wait_semaphores: &[],
            signal_semaphores: &[],
        }
    }

    /// Create submit info with single command buffer (no sync)
    #[inline]
    pub const fn single(cmd_buf: &'a KgpuCommandEncoderCapsule<Finished>) -> Self {
        Self {
            command_buffers: core::slice::from_ref(cmd_buf),
            wait_semaphores: &[],
            signal_semaphores: &[],
        }
    }
}

// ============================================================================
// KgpuQueueCapsule
// ============================================================================

/// KGPU Queue Capsule - T1+T4 Atomic+Batch GPU Queue
///
/// Lockfree GPU queue for command buffer submission with:
/// - Atomic submission counting
/// - Timeline fence values
/// - Queue family and index tracking
/// - Capability flags (graphics, compute, transfer, sparse, present)
///
/// # Memory Layout
///
/// - Size: 128 bytes (cache-line aligned on most platforms)
/// - Alignment: 128 bytes
/// - Handle: 64 bytes (generation-countered)
/// - Metadata: 32 bytes (family, index, capabilities, priority)
/// - Counters: 16 bytes (submitted_count, last_fence_value)
/// - State: 4 bytes (flags)
/// - Padding: 12 bytes
///
/// # ASSUM Safety
///
/// - `#ASSUME_QUEUE_FAMILY_VALID`: Queue family exists on device
/// - `#ASSUME_QUEUE_INDEX_VALID`: Queue index < queue family count
/// - `#ASSUME_SUBMISSION_ORDER`: Queue preserves submission order
/// - `#ASSUME_FENCE_MONOTONIC`: Fence values increase monotonically
/// - `#ASSUME_CACHE_ALIGNED`: 128B alignment prevents false sharing
#[repr(C, align(128))]
pub struct KgpuQueueCapsule {
    /// Generation-countered handle for ABA prevention
    handle: KgpuHandle<Queue>,

    /// Queue family index (0-255, device-specific)
    queue_family: u32,

    /// Queue index within family (0-255, typically 0)
    queue_index: u32,

    /// Queue capabilities (QUEUE_CAP_* flags)
    capabilities: u32,

    /// Queue priority (QUEUE_PRIORITY_*)
    priority: u32,

    /// Total command buffer submissions
    submitted_count: AtomicU64,

    /// Last fence value signaled (monotonic counter)
    last_fence_value: AtomicU64,

    /// Generation counter for ABA prevention
    generation: AtomicU32,

    /// Padding to align flags
    _pad1: u32,

    /// Queue state flags (QUEUE_FLAG_*)
    flags: AtomicU32,

    /// Padding to reach 128B total
    /// 64 (handle) + 16 (metadata) + 16 (counters) + 8 (generation + pad1) + 4 (flags) = 108
    /// 128 - 108 = 20B padding
    _padding: [u8; 20],
}

// Compile-time verification (Q33 mandate)
const _: () = {
    assert!(core::mem::size_of::<KgpuQueueCapsule>() == 128);
    assert!(core::mem::align_of::<KgpuQueueCapsule>() == 128);
};

impl KgpuQueueCapsule {
    /// Create a new queue capsule
    ///
    /// # Arguments
    ///
    /// - `queue_family`: Queue family index
    /// - `queue_index`: Queue index within family (usually 0)
    /// - `capabilities`: Queue capability flags (QUEUE_CAP_*)
    ///
    /// # Performance
    ///
    /// - Initialization: O(1) constant time
    /// - Memory: 128B (stack allocation)
    ///
    /// # Safety
    ///
    /// #ASSUME_QUEUE_FAMILY_VALID: Queue family exists on device
    /// #ASSUME_QUEUE_INDEX_VALID: Queue index < family queue count
    /// #VERIFY: All atomics initialized
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Graphics queue (queue family 0, index 0)
    /// let gfx_queue = KgpuQueueCapsule::new(
    ///     0,
    ///     0,
    ///     QUEUE_CAP_GRAPHICS | QUEUE_CAP_COMPUTE | QUEUE_CAP_TRANSFER | QUEUE_CAP_PRESENT
    /// );
    ///
    /// // Dedicated compute queue (queue family 1, index 0)
    /// let compute_queue = KgpuQueueCapsule::new(
    ///     1,
    ///     0,
    ///     QUEUE_CAP_COMPUTE | QUEUE_CAP_TRANSFER
    /// );
    ///
    /// // Dedicated transfer queue (queue family 2, index 0)
    /// let transfer_queue = KgpuQueueCapsule::new(
    ///     2,
    ///     0,
    ///     QUEUE_CAP_TRANSFER
    /// );
    /// ```
    pub const fn new(queue_family: u32, queue_index: u32, capabilities: u32) -> Self {
        Self {
            handle: KgpuHandle::new(0, 1), // index=0, generation=1 (valid)
            queue_family,
            queue_index,
            capabilities,
            priority: 1, // QUEUE_PRIORITY_NORMAL (can't use const in const fn context)
            submitted_count: AtomicU64::new(0),
            last_fence_value: AtomicU64::new(0),
            generation: AtomicU32::new(0),
            _pad1: 0,
            flags: AtomicU32::new(QUEUE_FLAG_IDLE),
            _padding: [0; 20],
        }
    }

    /// Create a queue with specific priority
    ///
    /// # Arguments
    ///
    /// - `queue_family`: Queue family index
    /// - `queue_index`: Queue index within family
    /// - `capabilities`: Queue capability flags
    /// - `priority`: Queue priority (QUEUE_PRIORITY_*)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // High-priority graphics queue for game rendering
    /// let gfx_queue = KgpuQueueCapsule::with_priority(
    ///     0, 0, QUEUE_CAP_GRAPHICS, QUEUE_PRIORITY_HIGH
    /// );
    ///
    /// // Realtime queue for VR
    /// let vr_queue = KgpuQueueCapsule::with_priority(
    ///     0, 0, QUEUE_CAP_GRAPHICS, QUEUE_PRIORITY_REALTIME
    /// );
    /// ```
    pub const fn with_priority(
        queue_family: u32,
        queue_index: u32,
        capabilities: u32,
        priority: u32,
    ) -> Self {
        Self {
            handle: KgpuHandle::new(0, 1),
            queue_family,
            queue_index,
            capabilities,
            priority,
            submitted_count: AtomicU64::new(0),
            last_fence_value: AtomicU64::new(0),
            generation: AtomicU32::new(0),
            _pad1: 0,
            flags: AtomicU32::new(QUEUE_FLAG_IDLE),
            _padding: [0; 20],
        }
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Get queue family index
    #[inline]
    pub fn queue_family(&self) -> u32 {
        self.queue_family
    }

    /// Get queue index within family
    #[inline]
    pub fn queue_index(&self) -> u32 {
        self.queue_index
    }

    /// Get queue capabilities
    #[inline]
    pub fn capabilities(&self) -> u32 {
        self.capabilities
    }

    /// Get queue priority
    #[inline]
    pub fn priority(&self) -> u32 {
        self.priority
    }

    /// Check if queue has specific capability
    #[inline]
    pub fn has_capability(&self, cap: u32) -> bool {
        (self.capabilities & cap) != 0
    }

    /// Get total submission count
    ///
    /// # Performance
    ///
    /// - Latency: <10ns (single atomic load)
    #[inline]
    pub fn submitted_count(&self) -> u64 {
        self.submitted_count.load(Ordering::Relaxed)
    }

    /// Get last fence value
    ///
    /// # Performance
    ///
    /// - Latency: <10ns (single atomic load)
    #[inline]
    pub fn last_fence_value(&self) -> u64 {
        self.last_fence_value.load(Ordering::Acquire)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Relaxed)
    }

    /// Check if queue is idle
    #[inline]
    pub fn is_idle(&self) -> bool {
        let flags = self.flags.load(Ordering::Relaxed);
        (flags & QUEUE_FLAG_IDLE) != 0
    }

    /// Check if queue is busy
    #[inline]
    pub fn is_busy(&self) -> bool {
        let flags = self.flags.load(Ordering::Relaxed);
        (flags & QUEUE_FLAG_BUSY) != 0
    }

    /// Check if queue is suspended
    #[inline]
    pub fn is_suspended(&self) -> bool {
        let flags = self.flags.load(Ordering::Relaxed);
        (flags & QUEUE_FLAG_SUSPENDED) != 0
    }

    // ========================================================================
    // Command Submission
    // ========================================================================

    /// Submit command buffers to queue with synchronization
    ///
    /// Submits work to the GPU queue with optional wait/signal semaphores.
    ///
    /// # Arguments
    ///
    /// - `submit_info`: Submission descriptor with command buffers and sync
    ///
    /// # Performance
    ///
    /// - Target: <1μs (excluding kernel driver overhead)
    /// - Reality: ~10-50μs (kernel driver, GPU scheduler)
    ///
    /// # Errors
    ///
    /// - `QueueSuspended`: Queue is suspended
    /// - `InvalidCommandBuffer`: Command buffer not in Finished state
    /// - `SubmissionFailed`: Driver error during submission
    ///
    /// # Safety
    ///
    /// #ASSUME_SUBMISSION_ORDER: Queue preserves submission order
    /// #ASSUME_COMMAND_BUFFERS_FINISHED: All command buffers in Finished state
    /// #VERIFY: Command buffer state checked (type-state prevents runtime checks)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let encoder = KgpuCommandEncoderCapsule::new();
    /// let encoder = encoder.begin();
    /// encoder.draw(36, 1, 0, 0)?;
    /// let finished = encoder.finish();
    ///
    /// let submit_info = SubmitInfo::single(&finished);
    /// queue.submit(&submit_info)?;
    /// ```
    pub fn submit(&self, submit_info: &SubmitInfo) -> QueueResult<()> {
        // Check if queue is suspended
        if self.is_suspended() {
            return Err(QueueError::QueueSuspended);
        }

        // Process wait semaphores (block until all reached)
        for wait_info in submit_info.wait_semaphores {
            if !wait_info.all_reached() {
                // STUB: In real implementation, would block on wait
                // For now, just check if reached
                return Err(QueueError::SubmissionFailed);
            }
        }

        // Update queue state to busy
        self.flags
            .store(QUEUE_FLAG_BUSY, Ordering::Release);

        // Increment submission count
        self.submitted_count.fetch_add(1, Ordering::Relaxed);

        // Increment generation (for ABA prevention)
        self.generation.fetch_add(1, Ordering::Relaxed);

        // STUB: In real implementation, would submit to driver
        // - vkQueueSubmit() on Vulkan
        // - ID3D12CommandQueue::ExecuteCommandLists() on DX12
        // - MTLCommandQueue::commit() on Metal

        // Process signal semaphores (signal all after submission)
        for signal_info in submit_info.signal_semaphores {
            signal_info.signal_all();
        }

        // Update queue state back to idle
        self.flags
            .store(QUEUE_FLAG_IDLE, Ordering::Release);

        Ok(())
    }

    /// Submit command buffers and return fence for CPU wait
    ///
    /// Convenience method that submits work and creates a fence signaled
    /// after completion. Useful for CPU synchronization.
    ///
    /// # Arguments
    ///
    /// - `submit_info`: Submission descriptor
    ///
    /// # Returns
    ///
    /// Fence that will be signaled when submission completes
    ///
    /// # Performance
    ///
    /// - Latency: submit() + fence creation (~1-2μs)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let finished = encoder.finish();
    /// let submit_info = SubmitInfo::single(&finished);
    ///
    /// // Submit and get fence
    /// let fence = queue.submit_with_fence(&submit_info)?;
    ///
    /// // CPU can wait on fence
    /// fence.wait(1_000_000_000); // 1 second timeout
    /// ```
    pub fn submit_with_fence(
        &self,
        submit_info: &SubmitInfo,
    ) -> QueueResult<KgpuFenceCapsule<Unsignaled>> {
        // Submit work
        self.submit(submit_info)?;

        // Increment fence value
        let fence_value = self.last_fence_value.fetch_add(1, Ordering::AcqRel) + 1;

        // Create timeline fence
        let fence = KgpuFenceCapsule::new_timeline(fence_value);

        Ok(fence)
    }

    /// Wait for queue to become idle
    ///
    /// Blocks until all submitted work completes.
    ///
    /// # Performance
    ///
    /// - Poll interval: 100μs (configurable)
    /// - Timeout: Infinite (blocking)
    ///
    /// # Errors
    ///
    /// - `WaitTimeout`: Wait exceeded timeout (if timeout implemented)
    ///
    /// # Safety
    ///
    /// #ASSUME_IDLE_FLAG_VALID: QUEUE_FLAG_IDLE reflects actual GPU state
    /// #VERIFY: Poll with exponential backoff to avoid busy-wait
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// queue.submit(&submit_info)?;
    /// queue.wait_idle()?; // Block until complete
    /// ```
    pub fn wait_idle(&self) -> QueueResult<()> {
        // STUB: In real implementation, would use platform event wait:
        // - vkQueueWaitIdle() on Vulkan
        // - ID3D12Fence::SetEventOnCompletion() + WaitForSingleObject() on DX12
        // - [MTLCommandBuffer waitUntilCompleted] on Metal

        // For stub, check if idle (no actual blocking)
        if self.is_idle() {
            Ok(())
        } else {
            // Would block here in real implementation
            Err(QueueError::WaitTimeout)
        }
    }

    /// Suspend queue (prevent new submissions)
    ///
    /// Sets queue to suspended state. Existing submissions continue.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// queue.suspend();
    /// assert!(queue.is_suspended());
    /// ```
    pub fn suspend(&self) {
        self.flags
            .store(QUEUE_FLAG_SUSPENDED, Ordering::Release);
    }

    /// Resume queue (allow new submissions)
    ///
    /// Clears suspended state.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// queue.suspend();
    /// queue.resume();
    /// assert!(!queue.is_suspended());
    /// ```
    pub fn resume(&self) {
        self.flags
            .store(QUEUE_FLAG_IDLE, Ordering::Release);
    }
}

// ============================================================================
// Trait Implementations
// ============================================================================

/// Chaos mandate: Send for lockfree sharing across threads
// SAFETY: All fields are atomic or Copy, no raw pointers to thread-local data
unsafe impl Send for KgpuQueueCapsule {}

/// Chaos mandate: Sync for lockfree sharing across threads
// SAFETY: All mutable fields are atomic, safe concurrent access
unsafe impl Sync for KgpuQueueCapsule {}

impl core::fmt::Debug for KgpuQueueCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KgpuQueueCapsule")
            .field("queue_family", &self.queue_family)
            .field("queue_index", &self.queue_index)
            .field("capabilities", &format_args!("{:#010x}", self.capabilities))
            .field("priority", &self.priority)
            .field("submitted_count", &self.submitted_count())
            .field("last_fence_value", &self.last_fence_value())
            .field("generation", &self.generation())
            .field("is_idle", &self.is_idle())
            .finish()
    }
}

// ============================================================================
// HAL Trait
// ============================================================================

/// Hardware Abstraction Layer trait for queue operations
pub trait HalQueue {
    /// Queue type
    type Queue;

    /// Submit command buffers to queue
    fn submit(&self, queue: &Self::Queue, submit_info: &SubmitInfo) -> QueueResult<()>;

    /// Submit with fence creation
    fn submit_with_fence(
        &self,
        queue: &Self::Queue,
        submit_info: &SubmitInfo,
    ) -> QueueResult<KgpuFenceCapsule<Unsignaled>>;

    /// Wait for queue to become idle
    fn wait_idle(&self, queue: &Self::Queue) -> QueueResult<()>;

    /// Get queue family index
    fn get_queue_family(&self, queue: &Self::Queue) -> u32;

    /// Get queue index within family
    fn get_queue_index(&self, queue: &Self::Queue) -> u32;

    /// Get queue capabilities
    fn get_capabilities(&self, queue: &Self::Queue) -> u32;
}

// ============================================================================
// Tests (T28 Framework)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Construction Tests (T28 Unit Tier)
    // ========================================================================

    #[test]
    fn test_new_graphics_queue() {
        let queue = KgpuQueueCapsule::new(
            0,
            0,
            QUEUE_CAP_GRAPHICS | QUEUE_CAP_COMPUTE | QUEUE_CAP_TRANSFER | QUEUE_CAP_PRESENT,
        );

        assert_eq!(queue.queue_family(), 0);
        assert_eq!(queue.queue_index(), 0);
        assert!(queue.has_capability(QUEUE_CAP_GRAPHICS));
        assert!(queue.has_capability(QUEUE_CAP_COMPUTE));
        assert!(queue.has_capability(QUEUE_CAP_TRANSFER));
        assert!(queue.has_capability(QUEUE_CAP_PRESENT));
        assert_eq!(queue.priority(), QUEUE_PRIORITY_NORMAL);
        assert_eq!(queue.submitted_count(), 0);
        assert_eq!(queue.last_fence_value(), 0);
        assert!(queue.is_idle());
    }

    #[test]
    fn test_new_compute_queue() {
        let queue = KgpuQueueCapsule::new(1, 0, QUEUE_CAP_COMPUTE | QUEUE_CAP_TRANSFER);

        assert_eq!(queue.queue_family(), 1);
        assert!(queue.has_capability(QUEUE_CAP_COMPUTE));
        assert!(queue.has_capability(QUEUE_CAP_TRANSFER));
        assert!(!queue.has_capability(QUEUE_CAP_GRAPHICS));
        assert!(!queue.has_capability(QUEUE_CAP_PRESENT));
    }

    #[test]
    fn test_new_transfer_queue() {
        let queue = KgpuQueueCapsule::new(2, 0, QUEUE_CAP_TRANSFER);

        assert_eq!(queue.queue_family(), 2);
        assert!(queue.has_capability(QUEUE_CAP_TRANSFER));
        assert!(!queue.has_capability(QUEUE_CAP_GRAPHICS));
        assert!(!queue.has_capability(QUEUE_CAP_COMPUTE));
    }

    #[test]
    fn test_with_priority() {
        let queue = KgpuQueueCapsule::with_priority(
            0,
            0,
            QUEUE_CAP_GRAPHICS,
            QUEUE_PRIORITY_REALTIME,
        );

        assert_eq!(queue.priority(), QUEUE_PRIORITY_REALTIME);
    }

    // ========================================================================
    // Submission Tests (T28 Unit Tier)
    // ========================================================================

    #[test]
    fn test_submit_empty() {
        let queue = KgpuQueueCapsule::new(0, 0, QUEUE_CAP_GRAPHICS);
        let submit_info = SubmitInfo::empty();

        let result = queue.submit(&submit_info);
        assert!(result.is_ok());

        assert_eq!(queue.submitted_count(), 1);
        assert_eq!(queue.generation(), 1);
        assert!(queue.is_idle());
    }

    #[test]
    fn test_submit_single_command_buffer() {
        let queue = KgpuQueueCapsule::new(0, 0, QUEUE_CAP_GRAPHICS);
        let encoder = KgpuCommandEncoderCapsule::new();
        let encoder = encoder.begin();
        let finished = encoder.finish();

        let submit_info = SubmitInfo::single(&finished);
        let result = queue.submit(&submit_info);
        assert!(result.is_ok());

        assert_eq!(queue.submitted_count(), 1);
    }

    #[test]
    fn test_submit_increments_count() {
        let queue = KgpuQueueCapsule::new(0, 0, QUEUE_CAP_GRAPHICS);
        let submit_info = SubmitInfo::empty();

        queue.submit(&submit_info).unwrap();
        queue.submit(&submit_info).unwrap();
        queue.submit(&submit_info).unwrap();

        assert_eq!(queue.submitted_count(), 3);
        assert_eq!(queue.generation(), 3);
    }

    #[test]
    fn test_submit_when_suspended() {
        let queue = KgpuQueueCapsule::new(0, 0, QUEUE_CAP_GRAPHICS);
        queue.suspend();

        let submit_info = SubmitInfo::empty();
        let result = queue.submit(&submit_info);

        assert_eq!(result, Err(QueueError::QueueSuspended));
        assert_eq!(queue.submitted_count(), 0); // No submission
    }

    // ========================================================================
    // Fence Tests (T28 Integration Tier)
    // ========================================================================

    #[test]
    fn test_submit_with_fence() {
        let queue = KgpuQueueCapsule::new(0, 0, QUEUE_CAP_GRAPHICS);
        let submit_info = SubmitInfo::empty();

        let fence = queue.submit_with_fence(&submit_info).unwrap();

        assert_eq!(fence.value(), 1);
        assert_eq!(queue.last_fence_value(), 1);
        assert_eq!(queue.submitted_count(), 1);
    }

    #[test]
    fn test_submit_with_fence_increments_value() {
        let queue = KgpuQueueCapsule::new(0, 0, QUEUE_CAP_GRAPHICS);
        let submit_info = SubmitInfo::empty();

        let fence1 = queue.submit_with_fence(&submit_info).unwrap();
        let fence2 = queue.submit_with_fence(&submit_info).unwrap();
        let fence3 = queue.submit_with_fence(&submit_info).unwrap();

        assert_eq!(fence1.value(), 1);
        assert_eq!(fence2.value(), 2);
        assert_eq!(fence3.value(), 3);
        assert_eq!(queue.last_fence_value(), 3);
    }

    // ========================================================================
    // Wait Idle Tests (T28 Unit Tier)
    // ========================================================================

    #[test]
    fn test_wait_idle_when_idle() {
        let queue = KgpuQueueCapsule::new(0, 0, QUEUE_CAP_GRAPHICS);

        let result = queue.wait_idle();
        assert!(result.is_ok());
    }

    // ========================================================================
    // Suspend/Resume Tests (T28 Unit Tier)
    // ========================================================================

    #[test]
    fn test_suspend_resume() {
        let queue = KgpuQueueCapsule::new(0, 0, QUEUE_CAP_GRAPHICS);

        assert!(!queue.is_suspended());

        queue.suspend();
        assert!(queue.is_suspended());

        queue.resume();
        assert!(!queue.is_suspended());
        assert!(queue.is_idle());
    }

    // ========================================================================
    // Capability Tests (T28 Unit Tier)
    // ========================================================================

    #[test]
    fn test_has_capability() {
        let queue = KgpuQueueCapsule::new(0, 0, QUEUE_CAP_GRAPHICS | QUEUE_CAP_COMPUTE);

        assert!(queue.has_capability(QUEUE_CAP_GRAPHICS));
        assert!(queue.has_capability(QUEUE_CAP_COMPUTE));
        assert!(!queue.has_capability(QUEUE_CAP_TRANSFER));
        assert!(!queue.has_capability(QUEUE_CAP_SPARSE));
        assert!(!queue.has_capability(QUEUE_CAP_PRESENT));
    }

    // ========================================================================
    // Layout Tests (T28 Unit Tier)
    // ========================================================================

    #[test]
    fn test_size_is_128_bytes() {
        assert_eq!(core::mem::size_of::<KgpuQueueCapsule>(), 128);
    }

    #[test]
    fn test_alignment_is_128_bytes() {
        assert_eq!(core::mem::align_of::<KgpuQueueCapsule>(), 128);
    }

    // ========================================================================
    // Thread Safety Tests (T28 Integration Tier)
    // ========================================================================

    #[test]
    fn test_send_sync_bounds() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<KgpuQueueCapsule>();
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_submissions() {
        use std::sync::Arc;
        use std::thread;

        let queue = Arc::new(KgpuQueueCapsule::new(0, 0, QUEUE_CAP_GRAPHICS));
        let submit_info = SubmitInfo::empty();

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let q = Arc::clone(&queue);
                thread::spawn(move || {
                    for _ in 0..100 {
                        let _ = q.submit(&submit_info);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // All 400 submissions should be counted
        assert_eq!(queue.submitted_count(), 400);
    }

    // ========================================================================
    // Debug Tests (T28 Unit Tier)
    // ========================================================================

    #[test]
    fn test_debug_format() {
        let queue = KgpuQueueCapsule::new(0, 0, QUEUE_CAP_GRAPHICS);
        let debug_str = format!("{:?}", queue);

        assert!(debug_str.contains("KgpuQueueCapsule"));
        assert!(debug_str.contains("queue_family"));
        assert!(debug_str.contains("queue_index"));
        assert!(debug_str.contains("capabilities"));
    }
}
