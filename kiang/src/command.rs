//! Command Capsule (CMD-128) - Command buffer metadata tracking
//!
//! Implements lockfree command buffer state coordination following "The Atomic Capsule"
//! pattern for zero-overhead GPU command submission tracking.
//!
//! # Architecture
//!
//! - **CommandCapsule (CMD-128)**: Tracks command buffer metadata atomically
//! - **Two-phase commit**: Odd→even version protocol for consistent state
//! - **State machine**: PENDING → SUBMITTED → EXECUTING → COMPLETED
//! - **Hot path optimization**: <5ns readiness check via single atomic load
//!
//! # Decision Answered
//!
//! **"Is this command buffer ready for GPU submission?"**
//!
//! # Layout (128 bits = 2×64-bit words)
//!
//! ```text
//! W0 (head): commit:1 | ver:8 | buffer_id:24 | size_kb:16 | priority:4 | state:4 | reserved:7
//! W1 (body): timestamp_us:32 | duration_us:24 | ver_tail:8
//! ```
//!
//! # Performance Targets
//!
//! - Readiness check: <5ns (single atomic load + branch)
//! - State transition: <15ns (CAS operation)
//! - Version validation: Zero-cost (compile-time bit mask)

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ============================================================================
// Legacy Command Types (for queue_coordinator/batch_coordinator compatibility)
// ============================================================================

/// GPU command types (legacy, for routing)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandType {
    /// Render command
    Render,
    /// Compute command
    Compute,
    /// Copy/DMA command
    Copy,
    /// Video encode/decode
    Video,
}

/// GPU command (legacy, for queue compatibility)
#[derive(Debug, Clone, Copy)]
pub struct Command {
    /// Command type
    pub cmd_type: CommandType,
    /// Buffer ID
    pub buffer_id: u32,
    /// Buffer size in bytes
    pub size: u32,
    /// Priority (0-255)
    pub priority: u8,
}

/// Command submission errors (legacy)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    /// Queue is full
    QueueFull,
    /// Invalid command
    InvalidCommand,
}

/// Command queue (legacy - basic implementation for compatibility)
pub struct CommandQueue {
    queue: Mutex<VecDeque<Command>>,
    capacity: usize,
}

impl CommandQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    pub fn submit(&self, cmd: Command) -> Result<(), CommandError> {
        let mut q = self.queue.lock().unwrap();
        if q.len() >= self.capacity {
            return Err(CommandError::QueueFull);
        }
        q.push_back(cmd);
        Ok(())
    }

    pub fn dequeue(&self) -> Option<Command> {
        self.queue.lock().unwrap().pop_front()
    }

    pub fn len(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.lock().unwrap().is_empty()
    }
}

// ============================================================================
// CommandCapsule (CMD-128) - Modern Implementation
// ============================================================================

/// Command Capsule (CMD-128) - 128-bit atomic command buffer tracker
///
/// Encodes command buffer metadata in a lockfree atomic structure.
/// Readers make submission decisions via single atomic load.
///
/// # Safety
///
/// #ASSUME_SINGLE_WRITER: One command submission thread updates capsule state
/// #VERIFY_CONCURRENT_READS: Property tests validate readers see consistent state
#[repr(C, align(64))]
pub struct CommandCapsule {
    /// Head word: commit bit, version, buffer metadata
    head: AtomicU64,
    /// Body word: timing information, tail version
    body: AtomicU64,
}

/// Command buffer state machine
///
/// State transitions:
/// PENDING → SUBMITTED → EXECUTING → COMPLETED
///
/// Invalid transitions are prevented by state machine logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CommandState {
    /// Initial state - command being prepared
    Pending = 0,
    /// Submitted to GPU driver
    Submitted = 1,
    /// Currently executing on GPU
    Executing = 2,
    /// Execution completed
    Completed = 3,
}

impl CommandState {
    /// Check if state transition is valid
    #[inline]
    pub fn can_transition_to(&self, next: CommandState) -> bool {
        use CommandState::*;
        matches!(
            (self, next),
            (Pending, Submitted)
                | (Submitted, Executing)
                | (Executing, Completed)
                | (Completed, Pending) // Reset to pending for reuse
        )
    }

    /// Parse state from 4-bit value
    #[inline]
    fn from_bits(bits: u8) -> Self {
        match bits & 0xF {
            1 => CommandState::Submitted,
            2 => CommandState::Executing,
            3 => CommandState::Completed,
            _ => CommandState::Pending,
        }
    }
}

/// Command buffer priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum CommandPriority {
    /// Low priority - background tasks
    Low = 0,
    /// Normal priority - standard rendering
    Normal = 1,
    /// High priority - interactive workloads
    High = 2,
    /// Real-time priority - latency-sensitive
    RealTime = 3,
}

impl CommandPriority {
    /// Parse priority from 4-bit value
    #[inline]
    fn from_bits(bits: u8) -> Self {
        match bits & 0xF {
            3 => CommandPriority::RealTime,
            2 => CommandPriority::High,
            1 => CommandPriority::Normal,
            _ => CommandPriority::Low,
        }
    }
}

/// Command buffer metadata update
///
/// Used to publish new command state via two-phase commit.
#[derive(Debug, Clone, Copy)]
pub struct CommandUpdate {
    /// Buffer identifier (24-bit)
    pub buffer_id: u32,
    /// Buffer size in KB (16-bit, max 64MB)
    pub size_kb: u16,
    /// Submission priority
    pub priority: CommandPriority,
    /// Current state
    pub state: CommandState,
    /// Timestamp in microseconds
    pub timestamp_us: u32,
    /// Execution duration in microseconds (0 if not completed)
    pub duration_us: u32,
}

/// Command buffer state snapshot
///
/// Represents a consistent point-in-time view of command buffer state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSnapshot {
    /// Buffer identifier
    pub buffer_id: u32,
    /// Buffer size in KB
    pub size_kb: u16,
    /// Submission priority
    pub priority: CommandPriority,
    /// Current state
    pub state: CommandState,
    /// Timestamp in microseconds
    pub timestamp_us: u32,
    /// Execution duration in microseconds (0 if not completed)
    pub duration_us: u32,
}

impl CommandSnapshot {
    /// Check if command is ready for submission
    ///
    /// Ready conditions:
    /// - State is PENDING
    /// - Buffer size is non-zero
    /// - Buffer ID is valid
    #[inline(always)]
    pub fn is_ready_for_submission(&self) -> bool {
        self.state == CommandState::Pending && self.size_kb > 0 && self.buffer_id > 0
    }

    /// Check if command is executing
    #[inline(always)]
    pub fn is_executing(&self) -> bool {
        self.state == CommandState::Executing
    }

    /// Check if command is completed
    #[inline(always)]
    pub fn is_completed(&self) -> bool {
        self.state == CommandState::Completed
    }

    /// Get execution duration if completed
    #[inline]
    pub fn execution_duration_us(&self) -> Option<u32> {
        if self.is_completed() && self.duration_us > 0 {
            Some(self.duration_us)
        } else {
            None
        }
    }
}

impl CommandCapsule {
    /// Create new command capsule
    ///
    /// Initializes in PENDING state with empty metadata.
    pub const fn new(_buffer_id: u32) -> Self {
        Self {
            head: AtomicU64::new(0),
            body: AtomicU64::new(0),
        }
    }

    /// Create capsule with initial state
    pub fn with_state(buffer_id: u32, size_kb: u16, priority: CommandPriority) -> Self {
        let capsule = Self::new(buffer_id);

        let update = CommandUpdate {
            buffer_id,
            size_kb,
            priority,
            state: CommandState::Pending,
            timestamp_us: current_timestamp_us(),
            duration_us: 0,
        };

        capsule.publish(update);
        capsule
    }

    /// Publish command state update (two-phase commit)
    ///
    /// Protocol:
    /// 1. Write body with ODD version (uncommitted)
    /// 2. Write head with EVEN version + commit bit (committed)
    ///
    /// # Safety
    ///
    /// #ASSUME_SINGLE_WRITER: Only command submission thread calls this
    /// #VERIFY_TOCTOU_PREVENTED: Two-phase commit prevents torn reads
    pub fn publish(&self, cmd: CommandUpdate) {
        // Get current version for increment
        let current = self.head.load(Ordering::Relaxed);
        let old_ver = ((current >> 55) & 0xFF) as u8;

        // Force next odd version, then derive even version
        let ver_odd = (old_ver.wrapping_add(1)) | 1; // Force odd
        let ver_even = (ver_odd.wrapping_add(1)) & !1; // Force even

        // Phase 1: Write body with ODD tail version (uncommitted state)
        let body = pack_command_body(
            cmd.timestamp_us,
            cmd.duration_us,
            ver_odd, // ODD version marks uncommitted
        );
        // #ASSUME_MEMORY_ORDERING: Relaxed sufficient as commit bit gates visibility
        // #VERIFY_ORDERING_SUFFICIENT: Two-phase commit enforces happens-before
        self.body.store(body, Ordering::Relaxed);

        // Phase 2: Commit head with EVEN version and commit bit
        let head = pack_command_head(
            1,        // commit=1
            ver_even, // EVEN version marks committed
            cmd.buffer_id,
            cmd.size_kb,
            cmd.priority as u8,
            cmd.state as u8,
        );
        // #ASSUME_MEMORY_ORDERING: Release ensures body write is visible before commit
        // #VERIFY_ORDERING_SUFFICIENT: Readers use Acquire to see complete state
        self.head.store(head, Ordering::Release);
    }

    /// Is command ready for submission? (hot path <5ns)
    ///
    /// This is the critical decision point for GPU command submission.
    /// Must be extremely fast as it's checked on every submission attempt.
    ///
    /// # Performance
    ///
    /// Target: <5ns on modern x86_64
    /// - 1 atomic load (head)
    /// - 1 branch (commit check)
    /// - 1 atomic load (body, if committed)
    /// - 1 branch (version check)
    /// - Bit extraction (compile-time masks)
    ///
    /// #ASSUME_MEMORY_ORDERING: Acquire on head ensures we see complete state
    /// #VERIFY_PERFORMANCE: Benchmark validates <5ns latency
    #[inline(always)]
    pub fn is_ready(&self) -> bool {
        // Fast path: Single atomic load for commit check
        let h = self.head.load(Ordering::Acquire);

        // Fast rejection: Check commit bit (most common case is committed)
        if !is_committed_even(h) {
            return false;
        }

        // Load body for full state check
        let b = self.body.load(Ordering::Relaxed);

        // Version consistency check (TOCTOU prevention)
        if !head_tail_match(h, b) {
            return false;
        }

        // Extract state and validate readiness
        let state = CommandState::from_bits(((h >> 3) & 0xF) as u8); // State at bit 3
        let size_kb = ((h >> 11) & 0xFFFF) as u16;
        let buffer_id = ((h >> 27) & 0xFFFFFF) as u32;

        // Ready conditions: PENDING state, non-zero size, valid buffer ID
        state == CommandState::Pending && size_kb > 0 && buffer_id > 0
    }

    /// Read command state snapshot
    ///
    /// Returns None if state is uncommitted or inconsistent.
    /// Use this for detailed state inspection beyond readiness check.
    pub fn read(&self) -> Option<CommandSnapshot> {
        // Load head with Acquire ordering
        let h = self.head.load(Ordering::Acquire);

        // Check commit bit and even version
        if !is_committed_even(h) {
            return None;
        }

        // Load body
        let b = self.body.load(Ordering::Relaxed);

        // Verify version consistency
        if !head_tail_match(h, b) {
            return None;
        }

        // Unpack and return snapshot
        Some(unpack_command_snapshot(h, b))
    }

    /// Transition to next state
    ///
    /// Validates state machine transitions before applying.
    /// Returns true if transition succeeded, false if invalid.
    pub fn transition_to(&self, new_state: CommandState) -> bool {
        let Some(current) = self.read() else {
            return false;
        };

        // Validate state transition
        if !current.state.can_transition_to(new_state) {
            return false;
        }

        // Update with new state
        let mut update = CommandUpdate {
            buffer_id: current.buffer_id,
            size_kb: current.size_kb,
            priority: current.priority,
            state: new_state,
            timestamp_us: current.timestamp_us,
            duration_us: current.duration_us,
        };

        // If transitioning to COMPLETED, calculate execution duration
        if new_state == CommandState::Completed {
            let now = current_timestamp_us();
            update.duration_us = now.saturating_sub(current.timestamp_us);
        }

        self.publish(update);
        true
    }

    /// Mark command as submitted
    #[inline]
    pub fn mark_submitted(&self) -> bool {
        self.transition_to(CommandState::Submitted)
    }

    /// Mark command as executing
    #[inline]
    pub fn mark_executing(&self) -> bool {
        self.transition_to(CommandState::Executing)
    }

    /// Mark command as completed
    #[inline]
    pub fn mark_completed(&self) -> bool {
        self.transition_to(CommandState::Completed)
    }

    /// Reset command to pending state for reuse
    pub fn reset(&self, buffer_id: u32, size_kb: u16, priority: CommandPriority) {
        let update = CommandUpdate {
            buffer_id,
            size_kb,
            priority,
            state: CommandState::Pending,
            timestamp_us: current_timestamp_us(),
            duration_us: 0,
        };
        self.publish(update);
    }
}

// ============================================================================
// Helper Functions - Bit Packing/Unpacking
// ============================================================================

/// Check if head word has commit bit set and version is even
///
/// Two-phase commit protocol requires:
/// - commit bit = 1 (committed state)
/// - version is even (head version)
#[inline(always)]
fn is_committed_even(head: u64) -> bool {
    let commit = (head >> 63) & 1;
    let ver = (head >> 55) & 0xFF;
    commit == 1 && (ver & 1) == 0
}

/// Check if head and body versions match (TOCTOU prevention)
///
/// Two-phase commit protocol: head (even) = tail (odd) + 1
/// This prevents reading torn state during concurrent updates.
#[inline(always)]
fn head_tail_match(head: u64, body: u64) -> bool {
    let head_ver = (head >> 55) & 0xFF;
    let tail_ver = body & 0xFF;

    // Head version must be even, tail must be odd, and head = tail + 1
    (head_ver & 1) == 0 && (tail_ver & 1) == 1 && head_ver == tail_ver.wrapping_add(1)
}

/// Pack command head word
///
/// Layout: commit:1 | ver:8 | buffer_id:24 | size_kb:16 | priority:4 | state:4 | reserved:7
fn pack_command_head(
    commit: u8,
    ver: u8,
    buffer_id: u32,
    size_kb: u16,
    priority: u8,
    state: u8,
) -> u64 {
    ((commit as u64) << 63)
        | ((ver as u64) << 55)
        | ((buffer_id as u64 & 0xFFFFFF) << 27)  // 24 bits
        | ((size_kb as u64) << 11)                // 16 bits
        | ((priority as u64 & 0xF) << 7)          // 4 bits
        | ((state as u64 & 0xF) << 3) // 4 bits
    // 7 bits reserved
}

/// Pack command body word
///
/// Layout: timestamp_us:32 | duration_us:24 | ver_tail:8
fn pack_command_body(timestamp_us: u32, duration_us: u32, ver: u8) -> u64 {
    ((timestamp_us as u64) << 32)            // 32 bits at top
        | ((duration_us as u64 & 0xFFFFFF) << 8)  // 24 bits in middle
        | (ver as u64) // 8 bits at LSB
}

/// Unpack command snapshot from head and body words
fn unpack_command_snapshot(head: u64, body: u64) -> CommandSnapshot {
    CommandSnapshot {
        buffer_id: ((head >> 27) & 0xFFFFFF) as u32,
        size_kb: ((head >> 11) & 0xFFFF) as u16,
        priority: CommandPriority::from_bits(((head >> 7) & 0xF) as u8),
        state: CommandState::from_bits(((head >> 3) & 0xF) as u8),
        timestamp_us: ((body >> 32) & 0xFFFFFFFF) as u32,
        duration_us: ((body >> 8) & 0xFFFFFF) as u32, // 24 bits
    }
}

/// Get current timestamp in microseconds
#[inline]
fn current_timestamp_us() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_micros() as u32
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_command_capsule_basic() {
        let cmd = CommandCapsule::with_state(1001, 256, CommandPriority::Normal);

        let snapshot = cmd.read().expect("Should read valid state");
        assert_eq!(snapshot.buffer_id, 1001);
        assert_eq!(snapshot.size_kb, 256);
        assert_eq!(snapshot.priority, CommandPriority::Normal);
        assert_eq!(snapshot.state, CommandState::Pending);
    }

    #[test]
    fn test_readiness_check() {
        let cmd = CommandCapsule::with_state(1002, 512, CommandPriority::High);

        // Should be ready in PENDING state
        assert!(cmd.is_ready());

        // Should NOT be ready after submission
        assert!(cmd.mark_submitted());
        assert!(!cmd.is_ready());

        // Should NOT be ready while executing
        assert!(cmd.mark_executing());
        assert!(!cmd.is_ready());

        // Should NOT be ready when completed
        assert!(cmd.mark_completed());
        assert!(!cmd.is_ready());
    }

    #[test]
    fn test_state_machine_transitions() {
        let cmd = CommandCapsule::with_state(1003, 1024, CommandPriority::RealTime);

        // Valid transitions
        assert!(cmd.transition_to(CommandState::Submitted));
        assert_eq!(cmd.read().unwrap().state, CommandState::Submitted);

        assert!(cmd.transition_to(CommandState::Executing));
        assert_eq!(cmd.read().unwrap().state, CommandState::Executing);

        assert!(cmd.transition_to(CommandState::Completed));
        assert_eq!(cmd.read().unwrap().state, CommandState::Completed);

        // Invalid transition (Completed -> Executing not allowed)
        assert!(!cmd.transition_to(CommandState::Executing));

        // Valid reset transition (Completed -> Pending)
        assert!(cmd.transition_to(CommandState::Pending));
        assert_eq!(cmd.read().unwrap().state, CommandState::Pending);
    }

    #[test]
    fn test_invalid_state_transitions() {
        let cmd = CommandCapsule::with_state(1004, 2048, CommandPriority::Low);

        // Cannot skip states
        assert!(!cmd.transition_to(CommandState::Executing));
        assert_eq!(cmd.read().unwrap().state, CommandState::Pending);

        assert!(!cmd.transition_to(CommandState::Completed));
        assert_eq!(cmd.read().unwrap().state, CommandState::Pending);
    }

    #[test]
    fn test_execution_duration() {
        let cmd = CommandCapsule::with_state(1005, 128, CommandPriority::Normal);

        // No duration in PENDING state
        assert_eq!(cmd.read().unwrap().execution_duration_us(), None);

        cmd.mark_submitted();
        cmd.mark_executing();

        // Simulate execution time
        std::thread::sleep(Duration::from_micros(100));

        cmd.mark_completed();

        // Should have execution duration
        let snapshot = cmd.read().unwrap();
        let duration = snapshot.execution_duration_us();
        assert!(
            duration.is_some(),
            "Duration should be set after completion"
        );
        assert!(
            duration.unwrap() >= 100,
            "Duration should be at least 100us, got {}",
            duration.unwrap()
        ); // At least 100us
    }

    #[test]
    fn test_version_consistency() {
        let cmd = CommandCapsule::new(1006);

        // Publish multiple updates
        for i in 0..10 {
            let update = CommandUpdate {
                buffer_id: 1006,
                size_kb: (i * 100) as u16,
                priority: CommandPriority::Normal,
                state: CommandState::Pending,
                timestamp_us: current_timestamp_us(),
                duration_us: 0,
            };
            cmd.publish(update);
        }

        // Should always read consistent state
        let snapshot = cmd.read().expect("Should have valid state");
        assert_eq!(snapshot.size_kb, 900); // Last published value
    }

    #[test]
    fn test_concurrent_reads() {
        let cmd = Arc::new(CommandCapsule::with_state(
            1007,
            4096,
            CommandPriority::High,
        ));
        let mut handles = vec![];

        // Spawn 10 reader threads
        for _ in 0..10 {
            let cmd_clone = Arc::clone(&cmd);
            let handle = thread::spawn(move || {
                for _ in 0..1000 {
                    // All reads should succeed or return None (never panic)
                    let _ = cmd_clone.read();
                    let _ = cmd_clone.is_ready();
                }
            });
            handles.push(handle);
        }

        // Wait for all readers
        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_concurrent_updates_and_reads() {
        let cmd = Arc::new(CommandCapsule::with_state(
            1008,
            512,
            CommandPriority::Normal,
        ));

        // Writer thread
        let cmd_writer = Arc::clone(&cmd);
        let writer = thread::spawn(move || {
            for i in 0..100 {
                let update = CommandUpdate {
                    buffer_id: 1008,
                    size_kb: (i * 10) as u16,
                    priority: CommandPriority::Normal,
                    state: CommandState::Pending,
                    timestamp_us: current_timestamp_us(),
                    duration_us: 0,
                };
                cmd_writer.publish(update);
                thread::sleep(Duration::from_micros(10));
            }
        });

        // Reader threads
        let mut readers = vec![];
        for _ in 0..5 {
            let cmd_reader = Arc::clone(&cmd);
            let handle = thread::spawn(move || {
                let mut valid_reads = 0;
                for _ in 0..500 {
                    if cmd_reader.read().is_some() {
                        valid_reads += 1;
                    }
                }
                valid_reads
            });
            readers.push(handle);
        }

        writer.join().unwrap();

        // All readers should get mostly valid reads
        for reader in readers {
            let valid_reads = reader.join().unwrap();
            assert!(valid_reads > 400); // At least 80% valid reads
        }
    }

    #[test]
    fn test_reset_command() {
        let cmd = CommandCapsule::with_state(1009, 1024, CommandPriority::Normal);

        // Complete full lifecycle
        assert!(cmd.mark_submitted());
        assert!(cmd.mark_executing());
        assert!(cmd.mark_completed());

        // Reset for reuse
        cmd.reset(1009, 2048, CommandPriority::High);

        let snapshot = cmd.read().unwrap();
        assert_eq!(snapshot.state, CommandState::Pending);
        assert_eq!(snapshot.size_kb, 2048);
        assert_eq!(snapshot.priority, CommandPriority::High);
        assert_eq!(snapshot.duration_us, 0);

        // Should be ready again
        assert!(cmd.is_ready());
    }

    #[test]
    fn test_priority_levels() {
        let priorities = [
            CommandPriority::Low,
            CommandPriority::Normal,
            CommandPriority::High,
            CommandPriority::RealTime,
        ];

        for (i, priority) in priorities.iter().enumerate() {
            let cmd = CommandCapsule::with_state(2000 + i as u32, 256, *priority);
            assert_eq!(cmd.read().unwrap().priority, *priority);
        }
    }

    #[test]
    fn test_buffer_id_limits() {
        // Test 24-bit buffer ID limit
        let max_buffer_id = 0xFFFFFF;
        let cmd = CommandCapsule::with_state(max_buffer_id, 256, CommandPriority::Normal);
        assert_eq!(cmd.read().unwrap().buffer_id, max_buffer_id);
    }

    #[test]
    fn test_size_kb_limits() {
        // Test 16-bit size limit (max 64MB when interpreted as KB)
        let max_size_kb = 0xFFFF;
        let cmd = CommandCapsule::with_state(3001, max_size_kb, CommandPriority::Normal);
        assert_eq!(cmd.read().unwrap().size_kb, max_size_kb);
    }
}
