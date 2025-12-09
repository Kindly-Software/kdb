//! Unit tests for BatchConstructorCapsule (T4 Batch Tier)
//! Direct implementation testing without GPU module integration

// Since GPU module has gating, we'll test the capsule directly
// by copying the types here for validation purposes

use std::sync::atomic::{AtomicU64, Ordering};

/// Command batch state FSM
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BatchState {
    Idle = 0,
    Recording = 1,
    Submitting = 2,
    Submitted = 3,
}

/// Thread state for tracking completion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ThreadCompletionState {
    Empty = 0,
    Recording = 1,
    Completed = 2,
}

/// Error types for batch construction operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchError {
    NotRecording,
    InvalidThreadId,
    ThreadSlotFull,
    GenerationMismatch,
    InvalidStateTransition,
    SubmissionTimeout,
}

/// Batch constructor capsule - 512-byte cache-aligned structure
#[repr(C, align(512))]
pub struct BatchConstructorCapsule {
    primary: AtomicU64,
    secondary: AtomicU64,
    thread_completion: [AtomicU64; 8],
    _padding: [u8; 432],
}

// Runtime verification (simpler than const array indexing)
#[test]
fn verify_size_alignment() {
    assert_eq!(std::mem::size_of::<BatchConstructorCapsule>(), 512);
    assert!(std::mem::align_of::<BatchConstructorCapsule>() >= 512);
}

impl BatchConstructorCapsule {
    pub const fn new() -> Self {
        BatchConstructorCapsule {
            primary: AtomicU64::new(0),
            secondary: AtomicU64::new(0),
            thread_completion: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            _padding: [0; 432],
        }
    }

    pub fn start_batch(&self) -> Result<(), BatchError> {
        let current_primary = self.primary.load(Ordering::Acquire);
        let state = (current_primary & 0xF) as u8;

        if state != BatchState::Idle as u8 {
            return Err(BatchError::InvalidStateTransition);
        }

        let current_gen = ((current_primary >> 8) & 0xFF) as u8;
        let next_gen = current_gen.wrapping_add(1);

        let new_primary = (BatchState::Recording as u64)
            | (0u64 << 4)
            | ((next_gen as u64) << 8);

        self.primary.store(new_primary, Ordering::Release);

        let new_secondary = ((next_gen as u64) << 32);
        self.secondary.store(new_secondary, Ordering::Release);

        for thread_slot in &self.thread_completion {
            thread_slot.store(0, Ordering::Release);
        }

        Ok(())
    }

    pub fn submit_thread(&self, thread_id: u8) -> Result<u32, BatchError> {
        if thread_id >= 8 {
            return Err(BatchError::InvalidThreadId);
        }

        let current_primary = self.primary.load(Ordering::Acquire);
        let state = (current_primary & 0xF) as u8;
        let active_threads = ((current_primary >> 4) & 0xF) as u8;

        if state != BatchState::Recording as u8 {
            return Err(BatchError::NotRecording);
        }

        if active_threads >= 8 {
            return Err(BatchError::ThreadSlotFull);
        }

        let current_gen = ((current_primary >> 8) & 0xFF) as u8;
        let new_primary = (BatchState::Recording as u64)
            | ((active_threads + 1) as u64) << 4
            | ((current_gen as u64) << 8);

        let thread_slot_value = (thread_id as u64)
            | ((ThreadCompletionState::Recording as u64) << 8)
            | ((current_gen as u64) << 32);

        let _ = self.primary.compare_exchange(
            current_primary,
            new_primary,
            Ordering::Release,
            Ordering::Acquire,
        );

        self.thread_completion[thread_id as usize].store(thread_slot_value, Ordering::Release);

        Ok(0)
    }

    pub fn record_command(&self, thread_id: u8) -> Result<(), BatchError> {
        if thread_id >= 8 {
            return Err(BatchError::InvalidThreadId);
        }

        let current_primary = self.primary.load(Ordering::Acquire);
        let state = (current_primary & 0xF) as u8;

        if state != BatchState::Recording as u8 {
            return Err(BatchError::NotRecording);
        }

        let thread_slot = self.thread_completion[thread_id as usize].load(Ordering::Acquire);
        let current_count = ((thread_slot >> 16) & 0xFFFF) as u32;
        let new_count = current_count.saturating_add(1);

        let new_thread_slot = (thread_slot & !0xFFFF0000) | ((new_count as u64) << 16);
        self.thread_completion[thread_id as usize].store(new_thread_slot, Ordering::Release);

        let secondary = self.secondary.load(Ordering::Acquire);
        let total_commands = (secondary & 0xFFFFFFFF) as u32;
        let new_total = total_commands.saturating_add(1);
        let current_gen = ((secondary >> 32) & 0xFFFFFFFF) as u32;

        let new_secondary = (new_total as u64) | ((current_gen as u64) << 32);
        self.secondary.store(new_secondary, Ordering::Release);

        Ok(())
    }

    pub fn finish_batch(&self) -> Result<u32, BatchError> {
        let current_primary = self.primary.load(Ordering::Acquire);
        let state = (current_primary & 0xF) as u8;

        if state == BatchState::Submitted as u8 {
            let secondary = self.secondary.load(Ordering::Acquire);
            return Ok((secondary & 0xFFFFFFFF) as u32);
        }

        if state != BatchState::Recording as u8 && state != BatchState::Submitting as u8 {
            return Err(BatchError::InvalidStateTransition);
        }

        let current_gen = ((current_primary >> 8) & 0xFF) as u8;
        let active_threads = ((current_primary >> 4) & 0xF) as u8;

        let submitting_primary = (BatchState::Submitting as u64)
            | ((active_threads as u64) << 4)
            | ((current_gen as u64) << 8);

        let _ = self.primary.compare_exchange(
            current_primary,
            submitting_primary,
            Ordering::Release,
            Ordering::Acquire,
        );

        for i in 0..active_threads {
            let thread_slot = self.thread_completion[i as usize].load(Ordering::Acquire);
            if thread_slot != 0 {
                let new_slot = if (thread_slot & 0xFF) != (ThreadCompletionState::Completed as u64) {
                    (thread_slot & !0xFF00) | ((ThreadCompletionState::Completed as u64) << 8)
                } else {
                    thread_slot
                };
                self.thread_completion[i as usize].store(new_slot, Ordering::Release);
            }
        }

        let submitted_primary = (BatchState::Submitted as u64)
            | ((active_threads as u64) << 4)
            | ((current_gen as u64) << 8);

        self.primary.store(submitted_primary, Ordering::Release);

        let secondary = self.secondary.load(Ordering::Acquire);
        Ok((secondary & 0xFFFFFFFF) as u32)
    }

    pub fn snapshot(&self) -> (BatchState, u8, u32) {
        let primary = self.primary.load(Ordering::Acquire);
        let secondary = self.secondary.load(Ordering::Acquire);

        let state = match (primary & 0xF) as u8 {
            0 => BatchState::Idle,
            1 => BatchState::Recording,
            2 => BatchState::Submitting,
            3 => BatchState::Submitted,
            _ => BatchState::Idle,
        };
        let active_threads = ((primary >> 4) & 0xF) as u8;
        let total_commands = (secondary & 0xFFFFFFFF) as u32;

        (state, active_threads, total_commands)
    }

    pub fn reset(&self) {
        self.primary.store(0, Ordering::Release);
        self.secondary.store(0, Ordering::Release);
        for slot in &self.thread_completion {
            slot.store(0, Ordering::Release);
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[test]
fn test_new_idle() {
    let batch = BatchConstructorCapsule::new();
    let (state, active, total) = batch.snapshot();
    assert_eq!(state, BatchState::Idle);
    assert_eq!(active, 0);
    assert_eq!(total, 0);
}

#[test]
fn test_start_batch() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    let (state, active, total) = batch.snapshot();
    assert_eq!(state, BatchState::Recording);
    assert_eq!(active, 0);
    assert_eq!(total, 0);
}

#[test]
fn test_submit_thread_valid() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    for i in 0..8 {
        assert!(batch.submit_thread(i).is_ok());
    }
}

#[test]
fn test_record_command() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());
    assert!(batch.submit_thread(0).is_ok());

    for _ in 0..10 {
        assert!(batch.record_command(0).is_ok());
    }

    let (_, _, total) = batch.snapshot();
    assert_eq!(total, 10);
}

#[test]
fn test_finish_batch() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());
    assert!(batch.submit_thread(0).is_ok());
    assert!(batch.submit_thread(1).is_ok());

    assert!(batch.record_command(0).is_ok());
    assert!(batch.record_command(0).is_ok());
    assert!(batch.record_command(1).is_ok());

    let total = batch.finish_batch().expect("finish_batch failed");
    assert_eq!(total, 3);

    let (state, _, _) = batch.snapshot();
    assert_eq!(state, BatchState::Submitted);
}

#[test]
fn test_all_8_threads() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    for i in 0..8 {
        assert!(batch.submit_thread(i).is_ok());
        for _ in 0..10 {
            assert!(batch.record_command(i).is_ok());
        }
    }

    let total = batch.finish_batch().expect("finish_batch failed");
    assert_eq!(total, 80);
}

#[test]
fn test_reset() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());
    assert!(batch.submit_thread(0).is_ok());
    assert!(batch.record_command(0).is_ok());

    batch.reset();

    let (state, active, total) = batch.snapshot();
    assert_eq!(state, BatchState::Idle);
    assert_eq!(active, 0);
    assert_eq!(total, 0);
}

#[test]
fn test_size_verification() {
    assert_eq!(std::mem::size_of::<BatchConstructorCapsule>(), 512);
    assert!(std::mem::align_of::<BatchConstructorCapsule>() >= 512);
}

#[test]
fn test_stress_high_command_volume() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    for i in 0..8 {
        assert!(batch.submit_thread(i).is_ok());
    }

    for thread_id in 0..8 {
        for _ in 0..1000 {
            assert!(batch.record_command(thread_id).is_ok());
        }
    }

    let total = batch.finish_batch().expect("finish_batch failed");
    assert_eq!(total, 8000);
}

#[test]
fn test_parallel_simulation() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    // Simulate 4 threads recording 25 commands each
    for thread_id in 0..4 {
        assert!(batch.submit_thread(thread_id).is_ok());
    }

    for thread_id in 0..4 {
        for _ in 0..25 {
            assert!(batch.record_command(thread_id).is_ok());
        }
    }

    let total = batch.finish_batch().expect("finish_batch failed");
    assert_eq!(total, 100);

    let (state, active, _) = batch.snapshot();
    assert_eq!(state, BatchState::Submitted);
    assert_eq!(active, 4);
}
