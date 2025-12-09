// Intel Xe2 Execution Queue Management Capsule
// T1 Atomic Tier: 256B cache-aligned, 100% lockfree
//
// Manages execution queue lifecycle for Intel Xe2 GPU command submission.
// Coordinates queue creation, submission, synchronization, and destruction.
//
// # Overview
// Execution queues are the interface for submitting GPU work on Intel Xe2.
// Each queue has:
// - A queue ID (unique identifier per DRM file descriptor)
// - Engine class (compute, copy, video, etc.)
// - Priority level (NORMAL, HIGH, REALTIME)
// - State machine: IDLE → PENDING → RUNNING → COMPLETED
//
// # Synchronization
// - Uses fences (monotonic counters) for GPU completion tracking
// - Wait operations can timeout or block indefinitely
// - All operations are lockfree atomic

#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
use std::os::unix::io::RawFd;

/// Execution queue states
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
const EXEC_STATE_IDLE: u32 = 0;
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
const EXEC_STATE_PENDING: u32 = 1;
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
const EXEC_STATE_RUNNING: u32 = 2;
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
const EXEC_STATE_COMPLETED: u32 = 3;
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
const EXEC_STATE_ERROR: u32 = 4;

/// Priority levels
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub const EXEC_PRIORITY_NORMAL: u32 = 0;
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub const EXEC_PRIORITY_HIGH: u32 = 1;
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub const EXEC_PRIORITY_REALTIME: u32 = 2;

/// Intel Xe2 Execution Queue Capsule (T1 Atomic, 256B cache-aligned)
///
/// Manages GPU execution queues for command submission and synchronization.
/// Provides lockfree coordination for queue lifecycle and fence-based completion.
///
/// # State Machine
/// ```text
/// IDLE --create_queue()--> IDLE (queue_id set)
///   |
///   +--submit()--> PENDING --GPU_START--> RUNNING --GPU_COMPLETE--> COMPLETED
///                                            |
///                                            +--ERROR--> ERROR
/// ```
///
/// # Memory Safety
/// - #ASSUME: DRM file descriptor remains valid during operations
/// - #VERIFY: All operations check state before proceeding
/// - #ASSUME: GPU fence values are monotonically increasing
/// - #VERIFY: Generation counter prevents ABA race conditions
///
/// # Performance
/// - Queue creation: ~10-50μs (kernel ioctl)
/// - Submit: ~1-5μs (kernel scheduling)
/// - Wait: <100ns check (poll), variable block (timeout)
/// - State transitions: <10ns atomic load/store
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
#[repr(C, align(256))]
pub struct XeExecCapsule {
    // Queue identification
    queue_id: AtomicU32,     // Execution queue ID (0 if not created)
    engine_class: AtomicU32, // Engine class (compute, copy, etc.)
    priority: AtomicU32,     // Priority level (see EXEC_PRIORITY_* constants)

    // State coordination
    state: AtomicU32,      // Current state (see EXEC_STATE_* constants)
    generation: AtomicU64, // Generation counter for ABA prevention

    // Fence tracking (GPU completion synchronization)
    last_fence: AtomicU64,      // Last fence value returned by submit
    completed_fence: AtomicU64, // Last fence value that completed

    // Flags
    queue_created: AtomicBool, // True if queue has been created

    // Statistics (lockfree counters)
    submit_count: AtomicU64,
    complete_count: AtomicU64,
    wait_count: AtomicU64,
    timeout_count: AtomicU64,

    // Padding to exactly 256 bytes
    // Current size without padding:
    //   queue_id: 4 bytes
    //   engine_class: 4 bytes
    //   priority: 4 bytes
    //   state: 4 bytes
    //   generation: 8 bytes (aligned to 8)
    //   last_fence: 8 bytes
    //   completed_fence: 8 bytes
    //   queue_created: 1 byte
    //   submit_count: 8 bytes (aligned to 8)
    //   complete_count: 8 bytes
    //   wait_count: 8 bytes
    //   timeout_count: 8 bytes
    // Total: 4 + 4 + 4 + 4 + 8 + 8 + 8 + 1 + 8 + 8 + 8 + 8 = 73 bytes
    //
    // With repr(C) implicit padding:
    //   - 4 bytes after state (to align generation to 8)
    //   - 7 bytes after queue_created (to align submit_count to 8)
    // Total with implicit padding: 73 + 4 + 7 = 84 bytes
    //
    // Explicit padding needed: 256 - 84 = 172 bytes
    _padding: [u8; 172],
}

/// Execution queue specific errors
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XeExecError {
    /// Queue has not been created yet
    QueueNotCreated,
    /// Queue already exists
    QueueAlreadyCreated,
    /// Submit operation failed
    SubmitFailed { errno: i32 },
    /// Wait operation failed
    WaitFailed { errno: i32 },
    /// Wait operation timed out
    WaitTimeout,
    /// Destroy operation failed
    DestroyFailed { errno: i32 },
}

#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
impl XeExecCapsule {
    /// Create new execution queue capsule in IDLE state
    #[inline]
    pub fn new() -> Self {
        // #ASSUME: Cache-aligned allocation by caller
        // #VERIFY: #[repr(C, align(256))] enforces alignment
        Self {
            queue_id: AtomicU32::new(0),
            engine_class: AtomicU32::new(0),
            priority: AtomicU32::new(EXEC_PRIORITY_NORMAL),
            state: AtomicU32::new(EXEC_STATE_IDLE),
            generation: AtomicU64::new(0),
            last_fence: AtomicU64::new(0),
            completed_fence: AtomicU64::new(0),
            queue_created: AtomicBool::new(false),
            submit_count: AtomicU64::new(0),
            complete_count: AtomicU64::new(0),
            wait_count: AtomicU64::new(0),
            timeout_count: AtomicU64::new(0),
            _padding: [0u8; 172],
        }
    }

    /// Create execution queue
    ///
    /// Creates a new GPU execution queue with the specified engine class and priority.
    /// Queue remains in IDLE state until first submit.
    ///
    /// # Arguments
    /// - `drm_fd`: DRM file descriptor
    /// - `engine_class`: Engine class (0 = compute, 1 = copy, etc.)
    /// - `priority`: Priority level (see EXEC_PRIORITY_* constants)
    ///
    /// # Errors
    /// - `QueueAlreadyCreated`: Queue has already been created
    ///
    /// # State Transition
    /// IDLE (not created) → IDLE (created)
    ///
    /// # Safety
    /// - #ASSUME: drm_fd is a valid open file descriptor
    /// - #VERIFY: Caller must ensure drm_fd remains open
    pub fn create_queue(
        &self,
        drm_fd: RawFd,
        engine_class: u16,
        priority: u32,
    ) -> Result<(), XeExecError> {
        // Check if queue already created
        if self.queue_created.load(Ordering::Acquire) {
            return Err(XeExecError::QueueAlreadyCreated);
        }

        // Phase 1: Simulate queue creation
        // In production, this would call DRM_IOCTL_XE_EXEC_QUEUE_CREATE
        let _ = drm_fd;

        let simulated_queue_id = self.generation.load(Ordering::Relaxed) as u32 + 1;

        // Store queue parameters
        self.queue_id.store(simulated_queue_id, Ordering::Relaxed);
        self.engine_class
            .store(engine_class as u32, Ordering::Relaxed);
        self.priority.store(priority, Ordering::Relaxed);

        // Mark as created
        self.queue_created.store(true, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);

        Ok(())
    }

    /// Submit GPU batch to execution queue
    ///
    /// Submits a command buffer batch to the GPU for execution.
    /// Returns a fence value that can be used to wait for completion.
    ///
    /// # Arguments
    /// - `drm_fd`: DRM file descriptor
    /// - `batch_addr`: GPU virtual address of command buffer
    /// - `batch_size`: Size of command buffer in bytes
    ///
    /// # Returns
    /// Fence value for synchronization
    ///
    /// # Errors
    /// - `QueueNotCreated`: Queue has not been created yet
    /// - `SubmitFailed`: Kernel submission failed
    ///
    /// # State Transition
    /// IDLE/COMPLETED → PENDING
    ///
    /// # Safety
    /// - #ASSUME: batch_addr points to valid GPU memory
    /// - #VERIFY: Caller must ensure batch is well-formed
    pub fn submit(
        &self,
        drm_fd: RawFd,
        batch_addr: u64,
        batch_size: u32,
    ) -> Result<u64, XeExecError> {
        // Check if queue created
        if !self.queue_created.load(Ordering::Acquire) {
            return Err(XeExecError::QueueNotCreated);
        }

        // Phase 1: Simulate batch submission
        // In production, this would call DRM_IOCTL_XE_EXEC
        let _ = (drm_fd, batch_addr, batch_size);

        // Generate new fence value (monotonically increasing)
        let fence = self.last_fence.fetch_add(1, Ordering::Release) + 1;

        // Transition to PENDING state
        self.state.store(EXEC_STATE_PENDING, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
        self.submit_count.fetch_add(1, Ordering::Relaxed);

        // Simulate immediate transition to RUNNING
        // (In reality, GPU scheduler would do this)
        self.state.store(EXEC_STATE_RUNNING, Ordering::Release);

        Ok(fence)
    }

    /// Wait for GPU completion
    ///
    /// Waits for a specific fence value to complete with optional timeout.
    ///
    /// # Arguments
    /// - `drm_fd`: DRM file descriptor
    /// - `fence`: Fence value to wait for
    /// - `timeout_ns`: Timeout in nanoseconds (0 = poll, u64::MAX = infinite)
    ///
    /// # Returns
    /// - `Ok(true)`: Fence completed
    /// - `Ok(false)`: Should never happen (timeout returns Err)
    ///
    /// # Errors
    /// - `QueueNotCreated`: Queue has not been created yet
    /// - `WaitTimeout`: Timeout expired before completion
    /// - `WaitFailed`: Kernel wait operation failed
    ///
    /// # State Transition
    /// RUNNING → COMPLETED (on success)
    ///
    /// # Performance
    /// - Poll (timeout_ns = 0): <100ns
    /// - Block (timeout_ns > 0): Variable, depends on GPU execution time
    pub fn wait(&self, drm_fd: RawFd, fence: u64, timeout_ns: u64) -> Result<bool, XeExecError> {
        // Check if queue created
        if !self.queue_created.load(Ordering::Acquire) {
            return Err(XeExecError::QueueNotCreated);
        }

        self.wait_count.fetch_add(1, Ordering::Relaxed);

        // Phase 1: Simulate wait operation
        // In production, this would call DRM_IOCTL_XE_WAIT_USER_FENCE
        let _ = (drm_fd, timeout_ns);

        // Check if fence already completed
        let completed = self.completed_fence.load(Ordering::Acquire);
        if fence <= completed {
            return Ok(true);
        }

        // Simulate completion for Phase 1
        // In reality, this would block/poll the kernel
        self.completed_fence.store(fence, Ordering::Release);
        self.state.store(EXEC_STATE_COMPLETED, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
        self.complete_count.fetch_add(1, Ordering::Relaxed);

        Ok(true)
    }

    /// Destroy execution queue
    ///
    /// Destroys the GPU execution queue and releases resources.
    /// Queue cannot be used after destruction.
    ///
    /// # Arguments
    /// - `drm_fd`: DRM file descriptor
    ///
    /// # Errors
    /// - `QueueNotCreated`: Queue has not been created yet
    /// - `DestroyFailed`: Kernel destroy operation failed
    ///
    /// # State Transition
    /// ANY → IDLE (not created)
    ///
    /// # Safety
    /// - #ASSUME: No outstanding GPU work on this queue
    /// - #VERIFY: Caller must ensure all submits have completed
    pub fn destroy_queue(&self, drm_fd: RawFd) -> Result<(), XeExecError> {
        // Check if queue created
        if !self.queue_created.load(Ordering::Acquire) {
            return Err(XeExecError::QueueNotCreated);
        }

        // Phase 1: Simulate queue destruction
        // In production, this would call DRM_IOCTL_XE_EXEC_QUEUE_DESTROY
        let _ = drm_fd;

        // Clear all state
        self.queue_id.store(0, Ordering::Relaxed);
        self.engine_class.store(0, Ordering::Relaxed);
        self.priority.store(EXEC_PRIORITY_NORMAL, Ordering::Relaxed);
        self.last_fence.store(0, Ordering::Relaxed);
        self.completed_fence.store(0, Ordering::Relaxed);

        // Mark as not created
        self.queue_created.store(false, Ordering::Release);
        self.state.store(EXEC_STATE_IDLE, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);

        Ok(())
    }

    /// Get current state
    #[inline]
    pub fn get_state(&self) -> u32 {
        self.state.load(Ordering::Acquire)
    }

    /// Get statistics (submit count, complete count)
    #[inline]
    pub fn get_statistics(&self) -> (u64, u64) {
        let submit = self.submit_count.load(Ordering::Relaxed);
        let complete = self.complete_count.load(Ordering::Relaxed);
        (submit, complete)
    }

    /// Check if queue is created
    #[inline]
    pub fn is_created(&self) -> bool {
        self.queue_created.load(Ordering::Acquire)
    }

    /// Get queue ID
    #[inline]
    pub fn queue_id(&self) -> u32 {
        self.queue_id.load(Ordering::Relaxed)
    }

    /// Get engine class
    #[inline]
    pub fn engine_class(&self) -> u32 {
        self.engine_class.load(Ordering::Relaxed)
    }

    /// Get priority
    #[inline]
    pub fn priority(&self) -> u32 {
        self.priority.load(Ordering::Relaxed)
    }

    /// Get last fence value
    #[inline]
    pub fn last_fence(&self) -> u64 {
        self.last_fence.load(Ordering::Relaxed)
    }

    /// Get completed fence value
    #[inline]
    pub fn completed_fence(&self) -> u64 {
        self.completed_fence.load(Ordering::Relaxed)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    /// Get wait count
    #[inline]
    pub fn wait_count(&self) -> u64 {
        self.wait_count.load(Ordering::Relaxed)
    }

    /// Get timeout count
    #[inline]
    pub fn timeout_count(&self) -> u64 {
        self.timeout_count.load(Ordering::Relaxed)
    }
}

#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
impl Default for XeExecCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, feature = "kgpu-driver-intel", target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_alignment() {
        // T28 Q1: Verify 256B cache alignment
        assert_eq!(
            core::mem::size_of::<XeExecCapsule>(),
            256,
            "XeExecCapsule must be exactly 256 bytes"
        );
        assert_eq!(
            core::mem::align_of::<XeExecCapsule>(),
            256,
            "XeExecCapsule must be 256-byte aligned"
        );
    }

    #[test]
    fn test_new_capsule() {
        // T28 Q2: Verify initial state
        let capsule = XeExecCapsule::new();
        assert_eq!(capsule.get_state(), EXEC_STATE_IDLE);
        assert!(!capsule.is_created());
        assert_eq!(capsule.queue_id(), 0);
        assert_eq!(capsule.engine_class(), 0);
        assert_eq!(capsule.priority(), EXEC_PRIORITY_NORMAL);
        assert_eq!(capsule.last_fence(), 0);
        assert_eq!(capsule.completed_fence(), 0);
        let (submit_count, complete_count) = capsule.get_statistics();
        assert_eq!(submit_count, 0);
        assert_eq!(complete_count, 0);
        assert_eq!(capsule.generation(), 0);
    }

    #[test]
    fn test_default() {
        // T28 Q3: Verify Default trait
        let capsule = XeExecCapsule::default();
        assert_eq!(capsule.get_state(), EXEC_STATE_IDLE);
        assert!(!capsule.is_created());
    }

    #[test]
    fn test_create_queue() {
        // T28 Q4: Verify queue creation
        let capsule = XeExecCapsule::new();
        let result = capsule.create_queue(-1, 0, EXEC_PRIORITY_NORMAL);
        assert!(result.is_ok());
        assert!(capsule.is_created());
        assert_ne!(capsule.queue_id(), 0);
        assert_eq!(capsule.engine_class(), 0);
        assert_eq!(capsule.priority(), EXEC_PRIORITY_NORMAL);
        assert_eq!(capsule.generation(), 1);
    }

    #[test]
    fn test_double_create_fails() {
        // T28 Q5: Verify double create fails
        let capsule = XeExecCapsule::new();
        capsule.create_queue(-1, 0, EXEC_PRIORITY_NORMAL).unwrap();
        let result = capsule.create_queue(-1, 0, EXEC_PRIORITY_NORMAL);
        assert!(matches!(result, Err(XeExecError::QueueAlreadyCreated)));
    }

    #[test]
    fn test_submit_without_create_fails() {
        // T28 Q6: Verify submit requires created queue
        let capsule = XeExecCapsule::new();
        let result = capsule.submit(-1, 0x1000, 4096);
        assert!(matches!(result, Err(XeExecError::QueueNotCreated)));
    }

    #[test]
    fn test_submit() {
        // T28 Q7: Verify batch submission
        let capsule = XeExecCapsule::new();
        capsule.create_queue(-1, 0, EXEC_PRIORITY_NORMAL).unwrap();

        let fence = capsule.submit(-1, 0x1000, 4096).unwrap();
        assert_eq!(fence, 1);
        assert_eq!(capsule.last_fence(), 1);
        assert_eq!(capsule.get_state(), EXEC_STATE_RUNNING);
        let (submit_count, _) = capsule.get_statistics();
        assert_eq!(submit_count, 1);
    }

    #[test]
    fn test_multiple_submits() {
        // T28 Q8: Verify fence values increment
        let capsule = XeExecCapsule::new();
        capsule.create_queue(-1, 0, EXEC_PRIORITY_NORMAL).unwrap();

        let fence1 = capsule.submit(-1, 0x1000, 4096).unwrap();
        let fence2 = capsule.submit(-1, 0x2000, 4096).unwrap();
        let fence3 = capsule.submit(-1, 0x3000, 4096).unwrap();

        assert_eq!(fence1, 1);
        assert_eq!(fence2, 2);
        assert_eq!(fence3, 3);
        assert_eq!(capsule.last_fence(), 3);

        let (submit_count, _) = capsule.get_statistics();
        assert_eq!(submit_count, 3);
    }

    #[test]
    fn test_wait_without_create_fails() {
        // T28 Q9: Verify wait requires created queue
        let capsule = XeExecCapsule::new();
        let result = capsule.wait(-1, 1, 0);
        assert!(matches!(result, Err(XeExecError::QueueNotCreated)));
    }

    #[test]
    fn test_wait() {
        // T28 Q10: Verify wait operation
        let capsule = XeExecCapsule::new();
        capsule.create_queue(-1, 0, EXEC_PRIORITY_NORMAL).unwrap();

        let fence = capsule.submit(-1, 0x1000, 4096).unwrap();
        let result = capsule.wait(-1, fence, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
        assert_eq!(capsule.completed_fence(), fence);
        assert_eq!(capsule.get_state(), EXEC_STATE_COMPLETED);

        let (_, complete_count) = capsule.get_statistics();
        assert_eq!(complete_count, 1);
        assert_eq!(capsule.wait_count(), 1);
    }

    #[test]
    fn test_wait_already_completed() {
        // T28 Q11: Verify wait on already completed fence
        let capsule = XeExecCapsule::new();
        capsule.create_queue(-1, 0, EXEC_PRIORITY_NORMAL).unwrap();

        let fence = capsule.submit(-1, 0x1000, 4096).unwrap();
        capsule.wait(-1, fence, 0).unwrap();

        // Wait again on same fence
        let result = capsule.wait(-1, fence, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
        assert_eq!(capsule.wait_count(), 2);
    }

    #[test]
    fn test_destroy_without_create_fails() {
        // T28 Q12: Verify destroy requires created queue
        let capsule = XeExecCapsule::new();
        let result = capsule.destroy_queue(-1);
        assert!(matches!(result, Err(XeExecError::QueueNotCreated)));
    }

    #[test]
    fn test_destroy_queue() {
        // T28 Q13: Verify queue destruction
        let capsule = XeExecCapsule::new();
        capsule.create_queue(-1, 0, EXEC_PRIORITY_NORMAL).unwrap();

        let result = capsule.destroy_queue(-1);
        assert!(result.is_ok());
        assert!(!capsule.is_created());
        assert_eq!(capsule.queue_id(), 0);
        assert_eq!(capsule.get_state(), EXEC_STATE_IDLE);
    }

    #[test]
    fn test_full_lifecycle() {
        // T28 Q14: Verify complete lifecycle
        let capsule = XeExecCapsule::new();

        // Create queue
        capsule.create_queue(-1, 0, EXEC_PRIORITY_HIGH).unwrap();
        assert!(capsule.is_created());
        assert_eq!(capsule.priority(), EXEC_PRIORITY_HIGH);

        // Submit batch
        let fence = capsule.submit(-1, 0x1000, 4096).unwrap();
        assert_eq!(fence, 1);

        // Wait for completion
        capsule.wait(-1, fence, 0).unwrap();
        assert_eq!(capsule.completed_fence(), fence);

        // Destroy queue
        capsule.destroy_queue(-1).unwrap();
        assert!(!capsule.is_created());

        let (submit_count, complete_count) = capsule.get_statistics();
        assert_eq!(submit_count, 1);
        assert_eq!(complete_count, 1);
    }

    #[test]
    fn test_generation_counter() {
        // T28 Q15: Verify generation counter increments
        let capsule = XeExecCapsule::new();
        let gen0 = capsule.generation();

        capsule.create_queue(-1, 0, EXEC_PRIORITY_NORMAL).unwrap();
        let gen1 = capsule.generation();
        assert_eq!(gen1, gen0 + 1);

        let fence = capsule.submit(-1, 0x1000, 4096).unwrap();
        let gen2 = capsule.generation();
        assert_eq!(gen2, gen1 + 1);

        capsule.wait(-1, fence, 0).unwrap();
        let gen3 = capsule.generation();
        assert_eq!(gen3, gen2 + 1);

        capsule.destroy_queue(-1).unwrap();
        let gen4 = capsule.generation();
        assert_eq!(gen4, gen3 + 1);
    }

    #[test]
    fn test_priority_levels() {
        // T28 Q16: Verify all priority levels
        let capsule_normal = XeExecCapsule::new();
        capsule_normal
            .create_queue(-1, 0, EXEC_PRIORITY_NORMAL)
            .unwrap();
        assert_eq!(capsule_normal.priority(), EXEC_PRIORITY_NORMAL);

        let capsule_high = XeExecCapsule::new();
        capsule_high
            .create_queue(-1, 0, EXEC_PRIORITY_HIGH)
            .unwrap();
        assert_eq!(capsule_high.priority(), EXEC_PRIORITY_HIGH);

        let capsule_realtime = XeExecCapsule::new();
        capsule_realtime
            .create_queue(-1, 0, EXEC_PRIORITY_REALTIME)
            .unwrap();
        assert_eq!(capsule_realtime.priority(), EXEC_PRIORITY_REALTIME);
    }

    #[test]
    fn test_accessors() {
        // T28 Q17: Verify all accessor methods
        let capsule = XeExecCapsule::new();

        // All accessors should work without panicking
        let _ = capsule.get_state();
        let _ = capsule.is_created();
        let _ = capsule.queue_id();
        let _ = capsule.engine_class();
        let _ = capsule.priority();
        let _ = capsule.last_fence();
        let _ = capsule.completed_fence();
        let _ = capsule.generation();
        let _ = capsule.get_statistics();
        let _ = capsule.wait_count();
        let _ = capsule.timeout_count();
    }
}
