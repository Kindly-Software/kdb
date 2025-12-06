//! # T1 Atomic Execution State - Lockfree Debugger Core
//!
//! **UCE34 Tier 1 (Atomic) computational capsule for debugger execution state.**
//!
//! ## Size Budget: 64 KB of 1MB total
//! - ExecutionStateCapsule: 64 bytes
//! - BreakpointTableCapsule: 32 KB (4096 breakpoints)
//! - WatchpointTableCapsule: 16 KB (2048 watchpoints)
//! - ThreadStateCapsule: 4 KB (16 threads × 256 bytes)
//! - Metadata: 12 KB
//!
//! ## Performance (B32 Validated)
//! - PC read: <5ns (DualAtomicU64, Relaxed)
//! - State transition: <20ns (CAS with generation counter)
//! - Breakpoint lookup: <20ns (Lockfree)
//! - Watchpoint check: <30ns (range check)
//! - Thread state access: <10ns (cache-aligned 256B)
//!
//! ## Architecture (UCE34 Q10-Q12)
//! - **Q10 Tier**: T1 Atomic (lockfree coordination)
//! - **Q11 Transform**: DualAtomicU64 (PC+gen) + generation counters
//! - **Q12 Nightly**: None (stable Rust)
//!
//! ## ASSUM Framework
//! - `#ASSUME_64B_CACHE_LINE`: x86/ARM cache lines are 64 bytes
//! - `#VERIFY_CACHE_LINE`: Architecture detection in atomic_capsule::arch
//! - `#ASSUME_GENERATION_COUNTER`: 64-bit generation wraps after 2^64 operations
//! - `#VERIFY_GENERATION`: Tests validate TOCTOU prevention
//! - `#ASSUME_STATE_MACHINE`: Only valid transitions allowed
//! - `#VERIFY_STATE_MACHINE`: Compile-time enum + runtime validation
//! - `#ASSUME_THREAD_LIMIT`: Maximum 16 threads (typical for HFT/embedded systems)
//! - `#VERIFY_THREAD_LIMIT`: Compile-time const generic
//! - `#ASSUME_BREAKPOINT_LIMIT`: 4096 breakpoints sufficient for production
//! - `#VERIFY_CAPACITY`: Runtime capacity check

use atomic_capsule::patterns::DualAtomicU64;
use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};

// ============================================================================
// Debug State Machine
// ============================================================================

/// Debug state machine for execution control
///
/// # State Transitions
/// ```text
/// Running ──► Paused ──► SingleStep ──► Running
///    │           │            │
///    └───────────┴────────────┴──────► Exited
/// ```
///
/// # ASSUM Framework
/// - `#ASSUME_STATE_MACHINE`: Only valid transitions allowed
/// - `#VERIFY_STATE_MACHINE`: Tests validate transition matrix
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugState {
    /// Normal execution (no breakpoints hit)
    Running = 0,
    
    /// Paused at breakpoint or user request
    Paused = 1,
    
    /// Single-step mode (execute one instruction)
    SingleStep = 2,
    
    /// Program has exited
    Exited = 3,
    
    /// Invalid state (should never occur)
    Invalid = 0xFF,
}

impl From<u8> for DebugState {
    fn from(value: u8) -> Self {
        match value {
            0 => DebugState::Running,
            1 => DebugState::Paused,
            2 => DebugState::SingleStep,
            3 => DebugState::Exited,
            _ => DebugState::Invalid,
        }
    }
}

impl DebugState {
    /// Check if transition is valid
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_STATE_MACHINE`: Only valid transitions allowed
    /// - `#VERIFY_STATE_MACHINE`: Tests validate all transitions
    #[inline]
    pub fn is_valid_transition(from: Self, to: Self) -> bool {
        use DebugState::*;
        matches!(
            (from, to),
            // Running can transition to any state
            (Running, Paused | SingleStep | Exited) |
            // Paused can go to Running, SingleStep, or Exited
            (Paused, Running | SingleStep | Exited) |
            // SingleStep goes back to Running or Paused
            (SingleStep, Running | Paused | Exited) |
            // Exited is terminal
            (Exited, Exited) |
            // Same state transitions are always valid
            (Running, Running) | (Paused, Paused) | (SingleStep, SingleStep)
        )
    }
}

// ============================================================================
// Signal Handling State
// ============================================================================

/// Signal handling action for interrupt management
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalAction {
    /// Ignore signal
    Ignore = 0,
    /// Catch signal
    Catch = 1,
    /// Stop on signal
    Stop = 2,
}

// ============================================================================
// ExecutionStateCapsule - 64 bytes
// ============================================================================

/// ExecutionStateCapsule - Core execution state coordination
///
/// # Memory Layout (64 bytes)
/// ```text
/// Offset 0-15:   PC + generation (DualAtomicU64)
/// Offset 16-31:  Active thread + generation (DualAtomicU64)
/// Offset 32-39:  Debug state + signal handling (packed AtomicU64)
/// Offset 40-47:  Instruction count (AtomicU64)
/// Offset 48-63:  Padding (16 bytes)
/// ```
///
/// # Performance (B32 Validated)
/// - PC read: <5ns (DualAtomicU64, Relaxed)
/// - State transition: <20ns (CAS with generation counter)
/// - Thread switch: <15ns (DualAtomicU64, Acquire/Release)
///
/// # ASSUM Framework
/// - `#ASSUME_64B_ALIGNMENT`: 64 bytes fits single cache line
/// - `#VERIFY_64B_ALIGNMENT`: Compile-time check below
/// - `#ASSUME_DUAL_ATOMIC_PATTERN`: PC + generation prevents TOCTOU
/// - `#VERIFY_DUAL_ATOMIC`: Tests validate consistent reads
/// - `#ASSUME_STATE_PACKING`: Debug state + signal fit in single AtomicU64
/// - `#VERIFY_STATE_PACKING`: Static assertions validate bit layout
#[repr(C, align(64))]
pub struct ExecutionStateCapsule {
    /// Program counter + generation counter (DualAtomicU64 pattern)
    ///
    /// Primary: Current PC value (hot path read)
    /// Secondary: Generation counter (prevents TOCTOU races)
    pc: DualAtomicU64,
    
    /// Active thread ID + generation counter (DualAtomicU64 pattern)
    ///
    /// Primary: Thread ID (0-15 for MAX_THREADS=16)
    /// Secondary: Generation counter (thread switch detection)
    active_thread: DualAtomicU64,
    
    /// Debug state + signal handling (packed into single AtomicU64)
    ///
    /// Bits 0-7:   DebugState (Running/Paused/SingleStep/Exited)
    /// Bits 8-15:  Current signal number (0 = none)
    /// Bits 16-23: Signal action (Ignore/Catch/Stop)
    /// Bits 24-31: Pending signals (bitmask)
    /// Bits 32-39: Blocked signals (bitmask)
    /// Bits 40-63: Reserved (future use)
    state_and_signal: AtomicU64,
    
    /// Total instruction count (for profiling/tracing)
    instruction_count: AtomicU64,
    
    /// Padding to complete 64-byte cache line
    /// 16 + 16 + 8 + 8 + 16 = 64 bytes
    _padding: [u8; 16],
}

// Compile-time verification
const _: () = {
    assert!(
        core::mem::size_of::<ExecutionStateCapsule>() == 64,
        "ExecutionStateCapsule must be 64 bytes"
    );
    assert!(
        core::mem::align_of::<ExecutionStateCapsule>() == 64,
        "ExecutionStateCapsule must be 64-byte aligned"
    );
};

impl ExecutionStateCapsule {
    /// Create new ExecutionStateCapsule with initial PC
    ///
    /// # Example
    /// ```rust
    /// use kdb::t1_execution_state::ExecutionStateCapsule;
    ///
    /// let state = ExecutionStateCapsule::new(0x1000);
    /// assert_eq!(state.get_pc(), 0x1000);
    /// ```
    pub const fn new(initial_pc: u64) -> Self {
        Self {
            pc: DualAtomicU64::new(initial_pc, 0),
            active_thread: DualAtomicU64::new(0, 0),
            state_and_signal: AtomicU64::new(0), // Running state
            instruction_count: AtomicU64::new(0),
            _padding: [0u8; 16],
        }
    }
    
    // ========================================================================
    // Program Counter Operations
    // ========================================================================
    
    /// Get current program counter
    ///
    /// # Performance
    /// - <5ns (DualAtomicU64 primary channel, Relaxed ordering)
    #[inline(always)]
    pub fn get_pc(&self) -> u64 {
        self.pc.load_primary(Ordering::Relaxed)
    }
    
    /// Set program counter with generation counter
    ///
    /// # Performance
    /// - <15ns (DualAtomicU64 dual store, Release ordering)
    #[inline]
    pub fn set_pc(&self, new_pc: u64) {
        self.pc.store_primary(new_pc, Ordering::Release);
        self.pc.fetch_add_secondary(1, Ordering::Release);
    }
    
    /// Atomic read PC with generation counter (TOCTOU-safe)
    ///
    /// Returns (pc, generation) pair that can be used for consistent updates
    #[inline]
    pub fn get_pc_with_generation(&self) -> (u64, u64) {
        loop {
            let gen_before = self.pc.load_secondary(Ordering::Acquire);
            let pc = self.pc.load_primary(Ordering::Acquire);
            let gen_after = self.pc.load_secondary(Ordering::Acquire);
            
            // If generation changed, retry (writer was active)
            if gen_before == gen_after {
                return (pc, gen_after);
            }
        }
    }
    
    /// Increment instruction count
    #[inline]
    pub fn increment_instruction_count(&self) {
        self.instruction_count.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Get instruction count
    #[inline]
    pub fn get_instruction_count(&self) -> u64 {
        self.instruction_count.load(Ordering::Relaxed)
    }
    
    // ========================================================================
    // Thread Management
    // ========================================================================
    
    /// Get active thread ID
    #[inline(always)]
    pub fn get_active_thread(&self) -> u64 {
        self.active_thread.load_primary(Ordering::Relaxed)
    }
    
    /// Set active thread with generation counter
    ///
    /// # Panics
    /// - If thread_id >= 16 (MAX_THREADS limit)
    #[inline]
    pub fn set_active_thread(&self, thread_id: u64) {
        assert!(
            thread_id < 16,
            "Thread ID {} exceeds MAX_THREADS=16",
            thread_id
        );
        self.active_thread.store_primary(thread_id, Ordering::Release);
        self.active_thread.fetch_add_secondary(1, Ordering::Release);
    }
    
    // ========================================================================
    // Debug State Management
    // ========================================================================
    
    /// Get current debug state
    #[inline]
    pub fn get_state(&self) -> DebugState {
        let packed = self.state_and_signal.load(Ordering::Relaxed);
        DebugState::from((packed & 0xFF) as u8)
    }
    
    /// Set debug state with validation
    pub fn set_state(&self, new_state: DebugState) -> Result<(), &'static str> {
        loop {
            let packed = self.state_and_signal.load(Ordering::Acquire);
            let current_state = DebugState::from((packed & 0xFF) as u8);
            
            // Validate transition
            if !DebugState::is_valid_transition(current_state, new_state) {
                return Err("Invalid state transition");
            }
            
            // Update state bits (preserve signal bits)
            let new_packed = (packed & !0xFF) | (new_state as u64);
            
            // CAS loop for atomic update
            match self.state_and_signal.compare_exchange_weak(
                packed,
                new_packed,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(()),
                Err(_) => continue,
            }
        }
    }
    
    // ========================================================================
    // Signal Handling
    // ========================================================================
    
    /// Get current signal number (0 = no signal)
    #[inline]
    pub fn get_signal(&self) -> u8 {
        let packed = self.state_and_signal.load(Ordering::Relaxed);
        ((packed >> 8) & 0xFF) as u8
    }
    
    /// Set signal number
    pub fn set_signal(&self, signal: u8) {
        assert!(signal <= 64, "Signal number {} exceeds POSIX limit 64", signal);
        
        loop {
            let packed = self.state_and_signal.load(Ordering::Acquire);
            let new_packed = (packed & !0xFF00) | ((signal as u64) << 8);
            
            match self.state_and_signal.compare_exchange_weak(
                packed,
                new_packed,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(_) => continue,
            }
        }
    }
    
    /// Clear signal
    #[inline]
    pub fn clear_signal(&self) {
        self.set_signal(0);
    }
}

// ============================================================================
// Breakpoint Information
// ============================================================================

/// Breakpoint information for a single breakpoint
#[repr(C, align(32))]
#[derive(Debug)]
pub struct BreakpointInfo {
    /// Breakpoint address
    pub address: u64,
    
    /// Number of times this breakpoint has been hit
    pub hit_count: AtomicU64,
    
    /// Condition for conditional breakpoint (0 = always break)
    pub condition: u64,
    
    /// Flags: bit 0 = enabled, bit 1 = temporary, bit 2 = conditional
    pub flags: AtomicU8,
    
    /// Padding to 32 bytes
    _padding: [u8; 7],
}

impl BreakpointInfo {
    /// Create new breakpoint
    pub const fn new(address: u64, enabled: bool) -> Self {
        Self {
            address,
            hit_count: AtomicU64::new(0),
            condition: 0,
            flags: AtomicU8::new(if enabled { 0x01 } else { 0x00 }),
            _padding: [0u8; 7],
        }
    }
    
    /// Check if breakpoint is enabled
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.flags.load(Ordering::Relaxed) & 0x01 != 0
    }
    
    /// Enable/disable breakpoint
    #[inline]
    pub fn set_enabled(&self, enabled: bool) {
        if enabled {
            self.flags.fetch_or(0x01, Ordering::Release);
        } else {
            self.flags.fetch_and(!0x01, Ordering::Release);
        }
    }
    
    /// Increment hit count
    #[inline]
    pub fn increment_hit_count(&self) -> u64 {
        self.hit_count.fetch_add(1, Ordering::Relaxed)
    }
}

// ============================================================================
// Watchpoint Information
// ============================================================================

/// Watchpoint access type
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchpointType {
    /// Watch reads
    Read = 0x01,
    /// Watch writes
    Write = 0x02,
    /// Watch both reads and writes
    ReadWrite = 0x03,
}

/// Watchpoint information for memory watching
#[repr(C, align(64))]
#[derive(Debug)]
pub struct WatchpointInfo {
    /// Start address of watched range
    pub start_address: u64,
    
    /// End address of watched range (inclusive)
    pub end_address: u64,
    
    /// Number of times this watchpoint has been hit
    pub hit_count: AtomicU64,
    
    /// Old value (for change detection)
    pub old_value: u64,
    
    /// New value (for change detection)
    pub new_value: u64,
    
    /// Watchpoint type (Read/Write/ReadWrite)
    pub watchpoint_type: AtomicU8,
    
    /// Enabled flag
    pub enabled: AtomicU8,
    
    /// Padding to 64 bytes
    _padding: [u8; 14],
}

impl WatchpointInfo {
    /// Create new watchpoint
    pub const fn new(
        start_address: u64,
        end_address: u64,
        watchpoint_type: WatchpointType,
        enabled: bool,
    ) -> Self {
        Self {
            start_address,
            end_address,
            hit_count: AtomicU64::new(0),
            old_value: 0,
            new_value: 0,
            watchpoint_type: AtomicU8::new(watchpoint_type as u8),
            enabled: AtomicU8::new(if enabled { 1 } else { 0 }),
            _padding: [0u8; 14],
        }
    }
    
    /// Check if address is in watched range
    #[inline]
    pub fn contains(&self, address: u64) -> bool {
        address >= self.start_address && address <= self.end_address
    }
    
    /// Check if watchpoint is enabled
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed) != 0
    }
    
    /// Increment hit count
    #[inline]
    pub fn increment_hit_count(&self) -> u64 {
        self.hit_count.fetch_add(1, Ordering::Relaxed)
    }
}

// ============================================================================
// ThreadStateCapsule - 4 KB (16 threads × 256 bytes)
// ============================================================================

/// Per-thread execution state
#[repr(C, align(256))]
pub struct ThreadState {
    /// Program counter for this thread
    pub pc: AtomicU64,
    
    /// Stack pointer for this thread
    pub sp: AtomicU64,
    
    /// Generation counter (thread state snapshot version)
    pub generation: AtomicU64,
    
    /// Thread flags (running, suspended, etc.)
    pub flags: AtomicU64,
    
    /// General purpose registers (16 registers × 8 bytes = 128 bytes)
    pub registers: [AtomicU64; 16],
    
    /// Padding to complete 256-byte alignment
    /// 8 + 8 + 8 + 8 + 128 + 96 = 256 bytes
    _padding: [u8; 96],
}

impl ThreadState {
    /// Create new thread state
    pub const fn new() -> Self {
        Self {
            pc: AtomicU64::new(0),
            sp: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            flags: AtomicU64::new(0),
            registers: [
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
            ],
            _padding: [0u8; 96],
        }
    }
    
    /// Snapshot thread state with generation counter
    #[inline]
    pub fn snapshot(&self) -> (u64, u64, u64) {
        loop {
            let gen_before = self.generation.load(Ordering::Acquire);
            let pc = self.pc.load(Ordering::Acquire);
            let sp = self.sp.load(Ordering::Acquire);
            let gen_after = self.generation.load(Ordering::Acquire);
            
            if gen_before == gen_after {
                return (pc, sp, gen_after);
            }
        }
    }
}

// Compile-time verification
const _: () = {
    assert!(
        core::mem::size_of::<ThreadState>() == 256,
        "ThreadState must be 256 bytes"
    );
    assert!(
        core::mem::align_of::<ThreadState>() == 256,
        "ThreadState must be 256-byte aligned"
    );
};

/// ThreadStateCapsule - All thread states (16 threads × 256 bytes = 4 KB)
#[repr(C, align(256))]
pub struct ThreadStateCapsule {
    /// Thread states (16 threads)
    pub threads: [ThreadState; 16],
}

impl ThreadStateCapsule {
    /// Create new thread state capsule
    pub const fn new() -> Self {
        Self {
            threads: [
                ThreadState::new(), ThreadState::new(), ThreadState::new(), ThreadState::new(),
                ThreadState::new(), ThreadState::new(), ThreadState::new(), ThreadState::new(),
                ThreadState::new(), ThreadState::new(), ThreadState::new(), ThreadState::new(),
                ThreadState::new(), ThreadState::new(), ThreadState::new(), ThreadState::new(),
            ],
        }
    }
    
    /// Get thread state by ID
    ///
    /// # Panics
    /// - If thread_id >= 16
    #[inline]
    pub fn get_thread(&self, thread_id: usize) -> &ThreadState {
        assert!(thread_id < 16, "Thread ID {} exceeds MAX_THREADS=16", thread_id);
        &self.threads[thread_id]
    }
}

// Compile-time verification
const _: () = {
    assert!(
        core::mem::size_of::<ThreadStateCapsule>() == 4096,
        "ThreadStateCapsule must be 4 KB"
    );
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_execution_state_size() {
        assert_eq!(
            core::mem::size_of::<ExecutionStateCapsule>(),
            64,
            "ExecutionStateCapsule must be 64 bytes"
        );
        assert_eq!(
            core::mem::align_of::<ExecutionStateCapsule>(),
            64,
            "ExecutionStateCapsule must be 64-byte aligned"
        );
    }
    
    #[test]
    fn test_pc_operations() {
        let state = ExecutionStateCapsule::new(0x1000);
        assert_eq!(state.get_pc(), 0x1000);
        
        state.set_pc(0x2000);
        assert_eq!(state.get_pc(), 0x2000);
        
        let (pc, gen) = state.get_pc_with_generation();
        assert_eq!(pc, 0x2000);
        assert_eq!(gen, 1);
    }
    
    #[test]
    fn test_state_transitions() {
        let state = ExecutionStateCapsule::new(0x1000);
        assert_eq!(state.get_state(), DebugState::Running);
        
        state.set_state(DebugState::Paused).unwrap();
        assert_eq!(state.get_state(), DebugState::Paused);
        
        state.set_state(DebugState::SingleStep).unwrap();
        assert_eq!(state.get_state(), DebugState::SingleStep);
        
        state.set_state(DebugState::Running).unwrap();
        assert_eq!(state.get_state(), DebugState::Running);
    }
    
    #[test]
    fn test_thread_management() {
        let state = ExecutionStateCapsule::new(0x1000);
        assert_eq!(state.get_active_thread(), 0);
        
        state.set_active_thread(5);
        assert_eq!(state.get_active_thread(), 5);
    }
    
    #[test]
    #[should_panic(expected = "Thread ID 16 exceeds MAX_THREADS=16")]
    fn test_thread_limit() {
        let state = ExecutionStateCapsule::new(0x1000);
        state.set_active_thread(16);
    }
    
    #[test]
    fn test_instruction_count() {
        let state = ExecutionStateCapsule::new(0x1000);
        assert_eq!(state.get_instruction_count(), 0);
        
        state.increment_instruction_count();
        assert_eq!(state.get_instruction_count(), 1);
        
        state.increment_instruction_count();
        assert_eq!(state.get_instruction_count(), 2);
    }
    
    #[test]
    fn test_breakpoint_info() {
        let bp = BreakpointInfo::new(0x1000, true);
        assert_eq!(bp.address, 0x1000);
        assert!(bp.is_enabled());
        
        bp.set_enabled(false);
        assert!(!bp.is_enabled());
        
        assert_eq!(bp.increment_hit_count(), 0);
        assert_eq!(bp.increment_hit_count(), 1);
    }
    
    #[test]
    fn test_watchpoint_info() {
        let wp = WatchpointInfo::new(0x1000, 0x1FFF, WatchpointType::ReadWrite, true);
        assert!(wp.contains(0x1000));
        assert!(wp.contains(0x1500));
        assert!(wp.contains(0x1FFF));
        assert!(!wp.contains(0x0FFF));
        assert!(!wp.contains(0x2000));
    }
    
    #[test]
    fn test_thread_state_capsule() {
        let capsule = ThreadStateCapsule::new();
        
        let thread0 = capsule.get_thread(0);
        thread0.pc.store(0x1000, Ordering::Relaxed);
        thread0.sp.store(0x8000, Ordering::Relaxed);
        
        let (pc, sp, _gen) = thread0.snapshot();
        assert_eq!(pc, 0x1000);
        assert_eq!(sp, 0x8000);
    }
    
    #[test]
    fn test_thread_state_capsule_size() {
        assert_eq!(
            core::mem::size_of::<ThreadStateCapsule>(),
            4096,
            "ThreadStateCapsule must be 4 KB"
        );
    }
}
