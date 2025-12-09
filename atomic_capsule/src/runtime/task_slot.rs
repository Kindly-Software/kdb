//! Task Slot - Individual Task Container (64B Cache-Aligned)
//!
//! T1 Atomic tier primitive for async runtime task storage.
//!
//! # Architecture
//!
//! Each TaskSlot is a 64-byte cache-aligned container for a single async task:
//! - state: AtomicU32 (Free=0, Allocated=1, Running=2, Suspended=3, Completed=4)
//! - generation: AtomicU32 (ABA prevention, monotonically increasing)
//! - future_ptr: AtomicU64 (type-erased future pointer)
//! - waker_ptr: AtomicU64 (registered waker for notifications)
//! - user_data: AtomicU64 (application-specific metadata)
//!
//! # Safety (ASSUM Framework)
//!
//! #ASSUME_GENERATION_MONOTONIC: Generation counters never decrease
//! #ASSUME_SLOT_REUSE_SAFE: Generation mismatch detected before slot access
//! #ASSUME_ACQUIRE_RELEASE: All cross-thread reads use Acquire, writes use Release
//!
//! # Performance
//!
//! - State transition: <10ns (atomic store)
//! - Generation check: <5ns (atomic load + compare)
//! - Full slot access: <30ns (load all fields)

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Task state machine states
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskSlotState {
    /// Slot is free and available for allocation
    Free = 0,
    /// Slot has been allocated but future not yet stored
    Allocated = 1,
    /// Task is actively being polled
    Running = 2,
    /// Task is suspended waiting for wakeup
    Suspended = 3,
    /// Task has completed (success or error)
    Completed = 4,
    /// Task was cancelled
    Cancelled = 5,
}

impl TaskSlotState {
    /// Convert from raw u32 value
    #[inline]
    pub const fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(TaskSlotState::Free),
            1 => Some(TaskSlotState::Allocated),
            2 => Some(TaskSlotState::Running),
            3 => Some(TaskSlotState::Suspended),
            4 => Some(TaskSlotState::Completed),
            5 => Some(TaskSlotState::Cancelled),
            _ => None,
        }
    }

    /// Check if state allows polling
    #[inline]
    pub const fn can_poll(&self) -> bool {
        matches!(self, TaskSlotState::Allocated | TaskSlotState::Suspended)
    }

    /// Check if state is terminal
    #[inline]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, TaskSlotState::Completed | TaskSlotState::Cancelled | TaskSlotState::Free)
    }
}

/// Individual task container (64B cache-aligned)
///
/// # Memory Layout (64 bytes total)
///
/// ```text
/// Offset  Size  Field         Description
/// 0       4     state         TaskSlotState (atomic)
/// 4       4     generation    ABA prevention counter (atomic)
/// 8       8     future_ptr    Type-erased future pointer
/// 16      8     waker_ptr     Registered waker pointer
/// 24      8     user_data     Application metadata
/// 32      4     task_id       Unique task identifier
/// 36      4     priority      Task priority (0=high, 255=low)
/// 40      24    _padding      Cache line padding
/// ```
#[repr(C, align(64))]
pub struct TaskSlot {
    /// Current state of the task slot
    state: AtomicU32,

    /// Generation counter for ABA prevention
    /// Incremented on each slot release
    generation: AtomicU32,

    /// Pointer to type-erased future (Pin<Box<dyn Future<Output = ()>>>)
    /// Valid only when state is Allocated/Running/Suspended
    future_ptr: AtomicU64,

    /// Pointer to registered waker for async notification
    /// Valid only when state is Suspended
    waker_ptr: AtomicU64,

    /// Application-specific metadata
    user_data: AtomicU64,

    /// Unique task identifier within the pool
    task_id: AtomicU32,

    /// Task priority (0 = highest, 255 = lowest)
    priority: AtomicU32,

    /// Padding to ensure 64-byte alignment
    _padding: [u8; 24],
}

// SAFETY: TaskSlot only contains atomic types, which are Send + Sync
unsafe impl Send for TaskSlot {}
unsafe impl Sync for TaskSlot {}

impl TaskSlot {
    /// Create a new free task slot
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU32::new(TaskSlotState::Free as u32),
            generation: AtomicU32::new(0),
            future_ptr: AtomicU64::new(0),
            waker_ptr: AtomicU64::new(0),
            user_data: AtomicU64::new(0),
            task_id: AtomicU32::new(0),
            priority: AtomicU32::new(128), // Default to normal priority
            _padding: [0u8; 24],
        }
    }

    /// Create a new slot with specific task ID
    #[inline]
    pub const fn with_id(task_id: u32) -> Self {
        Self {
            state: AtomicU32::new(TaskSlotState::Free as u32),
            generation: AtomicU32::new(0),
            future_ptr: AtomicU64::new(0),
            waker_ptr: AtomicU64::new(0),
            user_data: AtomicU64::new(0),
            task_id: AtomicU32::new(task_id),
            priority: AtomicU32::new(128),
            _padding: [0u8; 24],
        }
    }

    /// Get current state (Acquire ordering for visibility)
    #[inline]
    pub fn state(&self) -> TaskSlotState {
        // #ASSUME_ACQUIRE_RELEASE: Use Acquire for cross-thread visibility
        let raw = self.state.load(Ordering::Acquire);
        TaskSlotState::from_u32(raw).unwrap_or(TaskSlotState::Free)
    }

    /// Get current generation counter
    #[inline]
    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get task ID
    #[inline]
    pub fn task_id(&self) -> u32 {
        self.task_id.load(Ordering::Relaxed)
    }

    /// Get priority
    #[inline]
    pub fn priority(&self) -> u32 {
        self.priority.load(Ordering::Relaxed)
    }

    /// Set priority
    #[inline]
    pub fn set_priority(&self, priority: u32) {
        self.priority.store(priority.min(255), Ordering::Relaxed);
    }

    /// Get user data
    #[inline]
    pub fn user_data(&self) -> u64 {
        self.user_data.load(Ordering::Acquire)
    }

    /// Set user data
    #[inline]
    pub fn set_user_data(&self, data: u64) {
        self.user_data.store(data, Ordering::Release);
    }

    /// Try to allocate this slot (Free -> Allocated)
    ///
    /// Returns the new generation if successful, None if slot was not free.
    ///
    /// # Safety
    ///
    /// #ASSUME_CAS_CONVERGENCE: CAS will eventually succeed or fail definitively
    #[inline]
    pub fn try_allocate(&self, expected_gen: u32) -> Option<u32> {
        // First verify generation matches (ABA prevention)
        // #ASSUME_GENERATION_MONOTONIC: Generation only increases
        let current_gen = self.generation.load(Ordering::Acquire);
        if current_gen != expected_gen {
            return None;
        }

        // Try to transition Free -> Allocated
        match self.state.compare_exchange(
            TaskSlotState::Free as u32,
            TaskSlotState::Allocated as u32,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Some(current_gen),
            Err(_) => None,
        }
    }

    /// Store future pointer (must be in Allocated state)
    ///
    /// # Safety
    ///
    /// Caller must ensure:
    /// - Slot is in Allocated state
    /// - future_ptr points to valid Pin<Box<dyn Future>>
    /// - The future will not be dropped until slot is released
    #[inline]
    pub unsafe fn store_future(&self, future_ptr: *mut (), generation: u32) -> bool {
        // #ASSUME_SLOT_REUSE_SAFE: Verify generation before access
        if self.generation.load(Ordering::Acquire) != generation {
            return false;
        }

        if self.state.load(Ordering::Acquire) != TaskSlotState::Allocated as u32 {
            return false;
        }

        self.future_ptr.store(future_ptr as u64, Ordering::Release);
        true
    }

    /// Load future pointer
    ///
    /// # Safety
    ///
    /// Caller must ensure the returned pointer is used while slot is valid.
    #[inline]
    pub unsafe fn load_future(&self, generation: u32) -> Option<*mut ()> {
        // #ASSUME_SLOT_REUSE_SAFE: Verify generation before access
        if self.generation.load(Ordering::Acquire) != generation {
            return None;
        }

        let state = self.state.load(Ordering::Acquire);
        if state != TaskSlotState::Running as u32 && state != TaskSlotState::Suspended as u32 {
            return None;
        }

        let ptr = self.future_ptr.load(Ordering::Acquire);
        if ptr == 0 {
            None
        } else {
            Some(ptr as *mut ())
        }
    }

    /// Store waker pointer (for async notification)
    ///
    /// # Safety
    ///
    /// Caller must ensure waker_ptr points to valid RawWaker data.
    #[inline]
    pub unsafe fn store_waker(&self, waker_ptr: *const (), generation: u32) -> bool {
        if self.generation.load(Ordering::Acquire) != generation {
            return false;
        }

        self.waker_ptr.store(waker_ptr as u64, Ordering::Release);
        true
    }

    /// Load waker pointer
    #[inline]
    pub fn load_waker(&self, generation: u32) -> Option<*const ()> {
        if self.generation.load(Ordering::Acquire) != generation {
            return None;
        }

        let ptr = self.waker_ptr.load(Ordering::Acquire);
        if ptr == 0 {
            None
        } else {
            Some(ptr as *const ())
        }
    }

    /// Transition to Running state (Allocated/Suspended -> Running)
    #[inline]
    pub fn start_poll(&self, generation: u32) -> bool {
        if self.generation.load(Ordering::Acquire) != generation {
            return false;
        }

        let current = self.state.load(Ordering::Acquire);
        if current != TaskSlotState::Allocated as u32 && current != TaskSlotState::Suspended as u32 {
            return false;
        }

        self.state.store(TaskSlotState::Running as u32, Ordering::Release);
        true
    }

    /// Transition to Suspended state (Running -> Suspended)
    #[inline]
    pub fn suspend(&self, generation: u32) -> bool {
        if self.generation.load(Ordering::Acquire) != generation {
            return false;
        }

        match self.state.compare_exchange(
            TaskSlotState::Running as u32,
            TaskSlotState::Suspended as u32,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// Mark task as completed (Running -> Completed)
    #[inline]
    pub fn complete(&self, generation: u32) -> bool {
        if self.generation.load(Ordering::Acquire) != generation {
            return false;
        }

        match self.state.compare_exchange(
            TaskSlotState::Running as u32,
            TaskSlotState::Completed as u32,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// Cancel the task (any non-terminal state -> Cancelled)
    #[inline]
    pub fn cancel(&self, generation: u32) -> bool {
        if self.generation.load(Ordering::Acquire) != generation {
            return false;
        }

        let current = self.state.load(Ordering::Acquire);
        let current_state = TaskSlotState::from_u32(current).unwrap_or(TaskSlotState::Free);

        if current_state.is_terminal() {
            return false;
        }

        self.state.store(TaskSlotState::Cancelled as u32, Ordering::Release);
        true
    }

    /// Release the slot back to the pool
    ///
    /// Clears all data and increments generation counter.
    ///
    /// # Safety
    ///
    /// #ASSUME_GENERATION_MONOTONIC: Generation is incremented, never decremented
    #[inline]
    pub fn release(&self) -> u32 {
        // Clear all pointers first
        self.future_ptr.store(0, Ordering::Release);
        self.waker_ptr.store(0, Ordering::Release);
        self.user_data.store(0, Ordering::Release);

        // Increment generation (wrapping is OK, just need it to change)
        // #ASSUME_GENERATION_MONOTONIC: wrapping_add ensures forward progress
        let new_gen = self.generation.fetch_add(1, Ordering::AcqRel).wrapping_add(1);

        // Finally set state to Free (must be last for visibility)
        self.state.store(TaskSlotState::Free as u32, Ordering::Release);

        new_gen
    }

    /// Reset slot to initial state (for testing/initialization)
    #[inline]
    pub fn reset(&self) {
        self.state.store(TaskSlotState::Free as u32, Ordering::Release);
        self.generation.store(0, Ordering::Release);
        self.future_ptr.store(0, Ordering::Release);
        self.waker_ptr.store(0, Ordering::Release);
        self.user_data.store(0, Ordering::Release);
        self.priority.store(128, Ordering::Release);
    }

    /// Check if slot is available for allocation
    #[inline]
    pub fn is_free(&self) -> bool {
        self.state.load(Ordering::Acquire) == TaskSlotState::Free as u32
    }

    /// Get packed state for atomic snapshot (state + generation in single u64)
    #[inline]
    pub fn packed_state(&self) -> u64 {
        let state = self.state.load(Ordering::Acquire);
        let gen = self.generation.load(Ordering::Acquire);
        ((gen as u64) << 32) | (state as u64)
    }
}

impl Default for TaskSlot {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for TaskSlot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TaskSlot")
            .field("state", &self.state())
            .field("generation", &self.generation())
            .field("task_id", &self.task_id())
            .field("priority", &self.priority())
            .finish()
    }
}

// Compile-time size verification
const _: () = {
    assert!(core::mem::size_of::<TaskSlot>() == 64, "TaskSlot must be 64 bytes");
    assert!(core::mem::align_of::<TaskSlot>() == 64, "TaskSlot must be 64-byte aligned");
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_slot_size_and_alignment() {
        assert_eq!(core::mem::size_of::<TaskSlot>(), 64);
        assert_eq!(core::mem::align_of::<TaskSlot>(), 64);
    }

    #[test]
    fn test_task_slot_new() {
        let slot = TaskSlot::new();
        assert_eq!(slot.state(), TaskSlotState::Free);
        assert_eq!(slot.generation(), 0);
        assert!(slot.is_free());
    }

    #[test]
    fn test_task_slot_with_id() {
        let slot = TaskSlot::with_id(42);
        assert_eq!(slot.task_id(), 42);
        assert_eq!(slot.state(), TaskSlotState::Free);
    }

    #[test]
    fn test_task_slot_allocate() {
        let slot = TaskSlot::new();

        // Should succeed with correct generation
        let new_gen = slot.try_allocate(0);
        assert!(new_gen.is_some());
        assert_eq!(new_gen.unwrap(), 0);
        assert_eq!(slot.state(), TaskSlotState::Allocated);

        // Should fail if already allocated
        let fail = slot.try_allocate(0);
        assert!(fail.is_none());
    }

    #[test]
    fn test_task_slot_generation_check() {
        let slot = TaskSlot::new();

        // Should fail with wrong generation
        let fail = slot.try_allocate(1);
        assert!(fail.is_none());
        assert!(slot.is_free()); // Still free
    }

    #[test]
    fn test_task_slot_state_transitions() {
        let slot = TaskSlot::new();

        // Allocate
        let gen = slot.try_allocate(0).unwrap();
        assert_eq!(slot.state(), TaskSlotState::Allocated);

        // Start poll
        assert!(slot.start_poll(gen));
        assert_eq!(slot.state(), TaskSlotState::Running);

        // Suspend
        assert!(slot.suspend(gen));
        assert_eq!(slot.state(), TaskSlotState::Suspended);

        // Resume poll
        assert!(slot.start_poll(gen));
        assert_eq!(slot.state(), TaskSlotState::Running);

        // Complete
        assert!(slot.complete(gen));
        assert_eq!(slot.state(), TaskSlotState::Completed);
    }

    #[test]
    fn test_task_slot_release() {
        let slot = TaskSlot::new();

        let gen = slot.try_allocate(0).unwrap();
        slot.start_poll(gen);
        slot.complete(gen);

        // Release should increment generation
        let new_gen = slot.release();
        assert_eq!(new_gen, 1);
        assert!(slot.is_free());
        assert_eq!(slot.generation(), 1);

        // Should be able to allocate again with new generation
        let next_gen = slot.try_allocate(1);
        assert!(next_gen.is_some());
    }

    #[test]
    fn test_task_slot_cancel() {
        let slot = TaskSlot::new();

        let gen = slot.try_allocate(0).unwrap();
        slot.start_poll(gen);

        // Cancel while running
        assert!(slot.cancel(gen));
        assert_eq!(slot.state(), TaskSlotState::Cancelled);

        // Cannot cancel again (terminal state)
        assert!(!slot.cancel(gen));
    }

    #[test]
    fn test_task_slot_priority() {
        let slot = TaskSlot::new();
        assert_eq!(slot.priority(), 128); // Default

        slot.set_priority(0); // High priority
        assert_eq!(slot.priority(), 0);

        slot.set_priority(255); // Low priority
        assert_eq!(slot.priority(), 255);

        slot.set_priority(1000); // Clamped to 255
        assert_eq!(slot.priority(), 255);
    }

    #[test]
    fn test_task_slot_user_data() {
        let slot = TaskSlot::new();
        assert_eq!(slot.user_data(), 0);

        slot.set_user_data(0xDEADBEEF);
        assert_eq!(slot.user_data(), 0xDEADBEEF);
    }

    #[test]
    fn test_task_slot_packed_state() {
        let slot = TaskSlot::new();

        // Free state with generation 0
        let packed = slot.packed_state();
        assert_eq!(packed & 0xFFFFFFFF, TaskSlotState::Free as u64);
        assert_eq!(packed >> 32, 0);

        // After allocation
        slot.try_allocate(0);
        let packed = slot.packed_state();
        assert_eq!(packed & 0xFFFFFFFF, TaskSlotState::Allocated as u64);
    }

    #[test]
    fn test_task_slot_state_enum() {
        assert_eq!(TaskSlotState::from_u32(0), Some(TaskSlotState::Free));
        assert_eq!(TaskSlotState::from_u32(1), Some(TaskSlotState::Allocated));
        assert_eq!(TaskSlotState::from_u32(2), Some(TaskSlotState::Running));
        assert_eq!(TaskSlotState::from_u32(3), Some(TaskSlotState::Suspended));
        assert_eq!(TaskSlotState::from_u32(4), Some(TaskSlotState::Completed));
        assert_eq!(TaskSlotState::from_u32(5), Some(TaskSlotState::Cancelled));
        assert_eq!(TaskSlotState::from_u32(6), None);

        assert!(TaskSlotState::Allocated.can_poll());
        assert!(TaskSlotState::Suspended.can_poll());
        assert!(!TaskSlotState::Running.can_poll());

        assert!(TaskSlotState::Completed.is_terminal());
        assert!(TaskSlotState::Cancelled.is_terminal());
        assert!(TaskSlotState::Free.is_terminal());
        assert!(!TaskSlotState::Running.is_terminal());
    }

    #[test]
    fn test_task_slot_reset() {
        let slot = TaskSlot::new();

        slot.try_allocate(0);
        slot.set_user_data(123);
        slot.set_priority(10);

        slot.reset();

        assert!(slot.is_free());
        assert_eq!(slot.generation(), 0);
        assert_eq!(slot.user_data(), 0);
        assert_eq!(slot.priority(), 128);
    }
}
