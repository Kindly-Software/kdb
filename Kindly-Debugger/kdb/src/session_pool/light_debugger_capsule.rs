//! LightDebuggerCapsule - Minimal 64KB debugger for quick attach/inspect operations
//!
//! Part of the Session Pool architecture for handling ~1,500 concurrent "light" sessions.
//!
//! # Memory Budget (64KB = 65,536 bytes)
//!
//! - ExecutionState: 256 bytes (pid, state, basic registers)
//! - MiniReplayEngine: 4,096 bytes (64 mini-snapshots × 64 bytes each)
//! - BreakpointTable: 8,192 bytes (128 breakpoints × 64 bytes each)
//! - BasicThreadState: 4,096 bytes (16 threads × 256 bytes each)
//! - TraceBuffer: 32,768 bytes (4,096 trace entries × 8 bytes each)
//! - Metadata: 256 bytes (generation, timestamps, tier info)
//! - Padding: ~15,872 bytes to reach 64KB
//!
//! # Architecture
//!
//! T1 Atomic tier with T5 Streaming mini-replay for time-travel debugging.
//! 100% lockfree (COCA compliant), DualAtomicU64 for coordinated state.
//!
//! # Upgrade Triggers
//!
//! Upgrade to MEDIUM tier when:
//! - snapshot_count >= 48 (75% of 64 mini capacity)
//! - breakpoint_count >= 96 (75% of 128 capacity)
//! - trace_buffer 90% full
//!
//! #ASSUME_LOCKFREE_ONLY: All coordination via atomics, no mutex/RwLock
//! #ASSUME_CACHE_ALIGNED: 128B alignment prevents false sharing
//! #ASSUME_GENERATION_COUNTERS: TOCTOU prevention via DualAtomicU64 pattern

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

// ============================================================================
// Constants
// ============================================================================

/// Maximum mini-snapshots (64 × 64B = 4,096 bytes)
pub const MAX_MINI_SNAPSHOTS: usize = 64;

/// Maximum breakpoints (128 × 64B = 8,192 bytes)
pub const MAX_LIGHT_BREAKPOINTS: usize = 128;

/// Maximum threads (16 × 256B = 4,096 bytes)
pub const MAX_LIGHT_THREADS: usize = 16;

/// Maximum trace entries (4,096 × 8B = 32,768 bytes)
pub const MAX_TRACE_ENTRIES: usize = 4096;

/// Upgrade threshold: snapshot count (75% of 64)
pub const UPGRADE_SNAPSHOT_THRESHOLD: u64 = 48;

/// Upgrade threshold: breakpoint count (75% of 128)
pub const UPGRADE_BREAKPOINT_THRESHOLD: u64 = 96;

/// Upgrade threshold: trace buffer fill ratio (90%)
pub const UPGRADE_TRACE_THRESHOLD: f64 = 0.90;

// ============================================================================
// DualAtomicU64 - Coordinated state pattern
// ============================================================================

/// DualAtomicU64 for coordinated state updates.
///
/// Encodes multiple fields in a single atomic for consistency:
/// - High 32 bits: generation counter (TOCTOU prevention)
/// - Low 32 bits: state/flags
///
/// #ASSUME_ATOMICITY: 64-bit atomics are lock-free on x86_64
/// #VERIFY_UNIT_TEST: test_dual_atomic_coordination
#[repr(C, align(8))]
pub struct DualAtomicU64 {
    value: AtomicU64,
}

impl DualAtomicU64 {
    pub const fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
        }
    }

    /// Load with generation check.
    #[inline]
    pub fn load(&self) -> (u32, u32) {
        let val = self.value.load(Ordering::Acquire);
        let generation = (val >> 32) as u32;
        let state = val as u32;
        (generation, state)
    }

    /// Store with generation increment.
    #[inline]
    pub fn store(&self, state: u32) {
        let val = self.value.load(Ordering::Relaxed);
        let old_gen = (val >> 32) as u32;
        let new_gen = old_gen.wrapping_add(1);
        let new_val = ((new_gen as u64) << 32) | (state as u64);
        self.value.store(new_val, Ordering::Release);
    }

    /// Atomic compare-and-swap with generation check.
    #[inline]
    pub fn compare_exchange(
        &self,
        expected_gen: u32,
        expected_state: u32,
        new_state: u32,
    ) -> Result<(u32, u32), (u32, u32)> {
        let expected = ((expected_gen as u64) << 32) | (expected_state as u64);
        let new_gen = expected_gen.wrapping_add(1);
        let desired = ((new_gen as u64) << 32) | (new_state as u64);

        match self.value.compare_exchange(
            expected,
            desired,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok((new_gen, new_state)),
            Err(actual) => {
                let gen = (actual >> 32) as u32;
                let state = actual as u32;
                Err((gen, state))
            }
        }
    }

    /// Get raw value for serialization.
    #[inline]
    pub fn raw(&self) -> u64 {
        self.value.load(Ordering::Acquire)
    }
}

// ============================================================================
// LightExecutionState - 256 bytes
// ============================================================================

/// Minimal execution state for light debugging sessions.
///
/// Contains only essential registers and state for quick attach/inspect.
///
/// Memory: 256 bytes (cache-line aligned)
#[repr(C, align(64))]
pub struct LightExecutionState {
    /// Process ID + generation (DualAtomicU64 pattern)
    /// High 32: generation, Low 32: pid truncated (full pid in pid_full)
    pub pid_state: DualAtomicU64,

    /// Full 64-bit PID
    pub pid_full: AtomicU64,

    /// Instruction pointer
    pub rip: AtomicU64,

    /// Stack pointer
    pub rsp: AtomicU64,

    /// Base pointer
    pub rbp: AtomicU64,

    /// Execution state: 0=detached, 1=attached, 2=paused, 3=running, 4=crashed
    pub state: AtomicU8,

    /// Stop signal (SIGSTOP, SIGTRAP, etc.)
    pub stop_signal: AtomicU8,

    /// Last error code
    pub last_error: AtomicU8,

    /// Reserved for future use
    _reserved: [u8; 5],

    /// General-purpose registers (16 × 8 = 128 bytes)
    /// rax, rbx, rcx, rdx, rsi, rdi, r8-r15
    pub regs: [AtomicU64; 16],

    /// Padding to 256 bytes
    _padding: [u8; 256 - 8 - 8 - 8 - 8 - 8 - 1 - 1 - 1 - 5 - 128],
}

impl LightExecutionState {
    pub const fn new() -> Self {
        const ZERO: AtomicU64 = AtomicU64::new(0);
        Self {
            pid_state: DualAtomicU64::new(),
            pid_full: AtomicU64::new(0),
            rip: AtomicU64::new(0),
            rsp: AtomicU64::new(0),
            rbp: AtomicU64::new(0),
            state: AtomicU8::new(0),
            stop_signal: AtomicU8::new(0),
            last_error: AtomicU8::new(0),
            _reserved: [0; 5],
            regs: [ZERO; 16],
            _padding: [0; 256 - 8 - 8 - 8 - 8 - 8 - 1 - 1 - 1 - 5 - 128],
        }
    }

    /// Initialize for a new process.
    #[inline]
    pub fn attach(&self, pid: u64) {
        self.pid_full.store(pid, Ordering::Release);
        self.pid_state.store(pid as u32); // Truncated for quick comparison
        self.state.store(1, Ordering::Release); // Attached
    }

    /// Get current PID.
    #[inline]
    pub fn get_pid(&self) -> u64 {
        self.pid_full.load(Ordering::Acquire)
    }

    /// Get instruction pointer.
    #[inline]
    pub fn get_rip(&self) -> u64 {
        self.rip.load(Ordering::Acquire)
    }

    /// Set instruction pointer with state update.
    #[inline]
    pub fn set_rip(&self, addr: u64) {
        self.rip.store(addr, Ordering::Release);
    }

    /// Check if attached to a process.
    #[inline]
    pub fn is_attached(&self) -> bool {
        self.state.load(Ordering::Acquire) >= 1
    }
}

// ============================================================================
// MiniSnapshot - 64 bytes
// ============================================================================

/// Minimal snapshot for quick time-travel (registers only).
///
/// #ASSUME_COPY_SNAPSHOT: All data is Copy for safe lockfree reads
#[repr(C, align(64))]
pub struct MiniSnapshot {
    /// Snapshot ID (monotonic, never reused)
    pub snapshot_id: AtomicU64,

    /// Instruction pointer at snapshot time
    pub rip: AtomicU64,

    /// Stack pointer at snapshot time
    pub rsp: AtomicU64,

    /// Base pointer at snapshot time
    pub rbp: AtomicU64,

    /// Timestamp (nanoseconds since epoch, truncated to 48 bits)
    pub timestamp: AtomicU64,

    /// Flags: bit 0=valid, bit 1=has_regs, bits 4-7=state
    pub flags: AtomicU8,

    /// Padding to 64 bytes
    _padding: [u8; 64 - 5 * 8 - 1],
}

impl MiniSnapshot {
    pub const fn empty() -> Self {
        Self {
            snapshot_id: AtomicU64::new(0),
            rip: AtomicU64::new(0),
            rsp: AtomicU64::new(0),
            rbp: AtomicU64::new(0),
            timestamp: AtomicU64::new(0),
            flags: AtomicU8::new(0),
            _padding: [0; 64 - 5 * 8 - 1],
        }
    }

    /// Save snapshot (<10ns target).
    #[inline]
    pub fn save(&self, id: u64, rip: u64, rsp: u64, rbp: u64) {
        self.snapshot_id.store(id, Ordering::Release);
        self.rip.store(rip, Ordering::Release);
        self.rsp.store(rsp, Ordering::Release);
        self.rbp.store(rbp, Ordering::Release);
        // Timestamp: use coarse clock for speed
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        self.timestamp.store(ts, Ordering::Release);
        self.flags.store(1, Ordering::Release); // Valid
    }

    /// Check if snapshot is valid.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.flags.load(Ordering::Acquire) & 1 != 0
    }

    /// Get snapshot state.
    #[inline]
    pub fn get_state(&self) -> (u64, u64, u64, u64) {
        (
            self.snapshot_id.load(Ordering::Acquire),
            self.rip.load(Ordering::Acquire),
            self.rsp.load(Ordering::Acquire),
            self.rbp.load(Ordering::Acquire),
        )
    }
}

// ============================================================================
// MiniReplayEngine - 4,096 bytes (64 snapshots)
// ============================================================================

/// Minimal replay engine for light time-travel.
///
/// Memory: 4,096 bytes total
/// - Header: 64 bytes (current + total + padding)
/// - Snapshots: 63 × 64 = 4,032 bytes
#[repr(C, align(64))]
pub struct MiniReplayEngine {
    /// Current snapshot position
    pub current: AtomicU64,

    /// Total snapshots taken
    pub total: AtomicU64,

    /// Padding to fill header to 64 bytes
    _header_padding: [u8; 48],

    /// Mini-snapshots (ring buffer) - 63 snapshots to fit 4096 total
    pub snapshots: [MiniSnapshot; 63],
}

impl MiniReplayEngine {
    pub const fn new() -> Self {
        const EMPTY: MiniSnapshot = MiniSnapshot::empty();
        Self {
            current: AtomicU64::new(0),
            total: AtomicU64::new(0),
            _header_padding: [0; 48],
            snapshots: [EMPTY; 63],
        }
    }

    /// Take mini-snapshot (<10ns target).
    ///
    /// #ASSUME_LOCKFREE_ONLY: Uses only atomic operations
    /// #VERIFY_UNIT_TEST: test_mini_snapshot_timing
    /// Maximum snapshots in mini replay engine
    const MAX_SNAPSHOTS: usize = 63;

    #[inline]
    pub fn take_snapshot(&self, rip: u64, rsp: u64, rbp: u64) -> Result<u64, LightDebugError> {
        let id = self.total.fetch_add(1, Ordering::Relaxed);
        let idx = (id as usize) % Self::MAX_SNAPSHOTS;

        self.snapshots[idx].save(id, rip, rsp, rbp);
        self.current.store(id, Ordering::Release);

        Ok(id)
    }

    /// Step backward in time.
    pub fn step_backward(&self) -> Result<(u64, u64, u64, u64), LightDebugError> {
        let current = self.current.load(Ordering::Acquire);
        if current == 0 {
            return Err(LightDebugError::AtFirstSnapshot);
        }

        let prev_id = current - 1;
        let idx = (prev_id as usize) % Self::MAX_SNAPSHOTS;

        if !self.snapshots[idx].is_valid() {
            return Err(LightDebugError::SnapshotInvalid);
        }

        self.current.store(prev_id, Ordering::Release);
        Ok(self.snapshots[idx].get_state())
    }

    /// Get current snapshot count.
    #[inline]
    pub fn snapshot_count(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }
}

// ============================================================================
// LightBreakpointEntry - 64 bytes
// ============================================================================

/// Minimal breakpoint entry.
#[repr(C, align(64))]
pub struct LightBreakpointEntry {
    /// Breakpoint address
    pub address: AtomicU64,

    /// Hit count
    pub hit_count: AtomicU64,

    /// Original instruction byte (for software breakpoints)
    pub original_byte: AtomicU8,

    /// Flags: bit 0=enabled, bit 1=temporary
    pub flags: AtomicU8,

    /// Padding to 64 bytes
    _padding: [u8; 64 - 2 * 8 - 2],
}

impl LightBreakpointEntry {
    pub const fn empty() -> Self {
        Self {
            address: AtomicU64::new(0),
            hit_count: AtomicU64::new(0),
            original_byte: AtomicU8::new(0),
            flags: AtomicU8::new(0),
            _padding: [0; 64 - 2 * 8 - 2],
        }
    }

    /// Set breakpoint at address.
    #[inline]
    pub fn set(&self, addr: u64, original_byte: u8) {
        self.address.store(addr, Ordering::Release);
        self.original_byte.store(original_byte, Ordering::Release);
        self.flags.store(1, Ordering::Release); // Enabled
        self.hit_count.store(0, Ordering::Release);
    }

    /// Check if enabled.
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.flags.load(Ordering::Acquire) & 1 != 0
    }

    /// Record a hit.
    #[inline]
    pub fn hit(&self) -> u64 {
        self.hit_count.fetch_add(1, Ordering::Relaxed)
    }

    /// Clear breakpoint.
    #[inline]
    pub fn clear(&self) {
        self.flags.store(0, Ordering::Release);
        self.address.store(0, Ordering::Release);
    }
}

// ============================================================================
// LightBreakpointTable - 8,192 bytes (128 breakpoints)
// ============================================================================

/// Minimal breakpoint table for light sessions.
///
/// Memory: 8,192 bytes total
/// - Header: 64 bytes (count + padding)
/// - Entries: 127 × 64 = 8,128 bytes
#[repr(C, align(64))]
pub struct LightBreakpointTable {
    /// Number of active breakpoints
    pub count: AtomicU64,

    /// Padding to fill header to 64 bytes
    _header_padding: [u8; 56],

    /// Breakpoint entries (127 entries to fit 8192 total)
    pub entries: [LightBreakpointEntry; 127],
}

impl LightBreakpointTable {
    /// Maximum breakpoints in table
    const MAX_ENTRIES: usize = 127;

    pub const fn new() -> Self {
        const EMPTY: LightBreakpointEntry = LightBreakpointEntry::empty();
        Self {
            count: AtomicU64::new(0),
            _header_padding: [0; 56],
            entries: [EMPTY; 127],
        }
    }

    /// Add breakpoint. Returns slot index.
    pub fn add(&self, addr: u64, original_byte: u8) -> Result<u8, LightDebugError> {
        let count = self.count.load(Ordering::Acquire);
        if count >= Self::MAX_ENTRIES as u64 {
            return Err(LightDebugError::BreakpointTableFull);
        }

        // Find empty slot
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.address.load(Ordering::Relaxed) == 0 {
                entry.set(addr, original_byte);
                self.count.fetch_add(1, Ordering::Release);
                return Ok(i as u8);
            }
        }

        Err(LightDebugError::NoEmptySlot)
    }

    /// Find breakpoint by address.
    pub fn find(&self, addr: u64) -> Option<u8> {
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.address.load(Ordering::Relaxed) == addr && entry.is_enabled() {
                return Some(i as u8);
            }
        }
        None
    }

    /// Get breakpoint count.
    #[inline]
    pub fn breakpoint_count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
}

// ============================================================================
// LightThreadState - 256 bytes per thread
// ============================================================================

/// Minimal thread state for light sessions.
#[repr(C, align(64))]
pub struct LightThreadState {
    /// Thread ID
    pub tid: AtomicU64,

    /// Instruction pointer
    pub rip: AtomicU64,

    /// Stack pointer
    pub rsp: AtomicU64,

    /// Base pointer
    pub rbp: AtomicU64,

    /// Thread state: 0=inactive, 1=running, 2=paused, 3=exited
    pub state: AtomicU8,

    /// Padding to 256 bytes
    _padding: [u8; 256 - 4 * 8 - 1],
}

impl LightThreadState {
    pub const fn empty() -> Self {
        Self {
            tid: AtomicU64::new(0),
            rip: AtomicU64::new(0),
            rsp: AtomicU64::new(0),
            rbp: AtomicU64::new(0),
            state: AtomicU8::new(0),
            _padding: [0; 256 - 4 * 8 - 1],
        }
    }

    /// Initialize thread.
    #[inline]
    pub fn init(&self, tid: u64) {
        self.tid.store(tid, Ordering::Release);
        self.state.store(1, Ordering::Release);
    }

    /// Check if active.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.tid.load(Ordering::Relaxed) != 0
    }
}

// ============================================================================
// TraceEntry - 8 bytes (compact)
// ============================================================================

/// Compact trace entry for high-throughput tracing.
///
/// Layout:
/// - Bits 0-47: Address (48 bits = 256TB address space)
/// - Bits 48-55: Event type (8 bits)
/// - Bits 56-63: Flags (8 bits)
#[repr(transparent)]
pub struct TraceEntry {
    value: AtomicU64,
}

impl TraceEntry {
    pub const fn empty() -> Self {
        Self {
            value: AtomicU64::new(0),
        }
    }

    /// Pack trace entry.
    #[inline]
    pub fn pack(addr: u64, event_type: u8, flags: u8) -> u64 {
        (addr & 0x0000_FFFF_FFFF_FFFF)
            | ((event_type as u64) << 48)
            | ((flags as u64) << 56)
    }

    /// Store trace entry.
    #[inline]
    pub fn store(&self, addr: u64, event_type: u8, flags: u8) {
        self.value.store(Self::pack(addr, event_type, flags), Ordering::Release);
    }

    /// Load and unpack trace entry.
    #[inline]
    pub fn load(&self) -> (u64, u8, u8) {
        let val = self.value.load(Ordering::Acquire);
        let addr = val & 0x0000_FFFF_FFFF_FFFF;
        let event_type = ((val >> 48) & 0xFF) as u8;
        let flags = ((val >> 56) & 0xFF) as u8;
        (addr, event_type, flags)
    }
}

// ============================================================================
// LightTraceBuffer - 32,768 bytes (4,096 entries)
// ============================================================================

/// High-throughput trace buffer for light sessions.
///
/// Memory: 32,768 bytes total
/// - Header: 64 bytes (4 × u64 + padding)
/// - Entries: 4,088 × 8 = 32,704 bytes
#[repr(C, align(64))]
pub struct LightTraceBuffer {
    /// Write position (ring buffer)
    pub write_pos: AtomicU64,

    /// Read position (for consumers)
    pub read_pos: AtomicU64,

    /// Total entries written
    pub total_written: AtomicU64,

    /// Dropped count (overflow)
    pub dropped: AtomicU64,

    /// Padding to fill header to 64 bytes
    _header_padding: [u8; 32],

    /// Trace entries (4,088 entries to fit 32,768 total)
    pub entries: [TraceEntry; 4088],
}

impl LightTraceBuffer {
    /// Maximum trace entries
    const MAX_ENTRIES: usize = 4088;

    pub const fn new() -> Self {
        const EMPTY: TraceEntry = TraceEntry::empty();
        Self {
            write_pos: AtomicU64::new(0),
            read_pos: AtomicU64::new(0),
            total_written: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            _header_padding: [0; 32],
            entries: [EMPTY; 4088],
        }
    }

    /// Record trace event (<10ns).
    #[inline]
    pub fn record(&self, addr: u64, event_type: u8, flags: u8) {
        let pos = self.write_pos.fetch_add(1, Ordering::Relaxed);
        let idx = (pos as usize) % Self::MAX_ENTRIES;
        self.entries[idx].store(addr, event_type, flags);
        self.total_written.fetch_add(1, Ordering::Relaxed);
    }

    /// Get fill ratio (for upgrade detection).
    #[inline]
    pub fn fill_ratio(&self) -> f64 {
        let written = self.total_written.load(Ordering::Relaxed);
        let read = self.read_pos.load(Ordering::Relaxed);
        let pending = written.saturating_sub(read);
        (pending as f64) / (Self::MAX_ENTRIES as f64)
    }

    /// Get trace count.
    #[inline]
    pub fn trace_count(&self) -> u64 {
        self.total_written.load(Ordering::Relaxed)
    }
}

// ============================================================================
// LightMetadata - 256 bytes
// ============================================================================

/// Session metadata for light debugger.
#[repr(C, align(64))]
pub struct LightMetadata {
    /// Session generation (for pool management)
    pub generation: AtomicU64,

    /// Session creation timestamp (nanoseconds)
    pub created_at: AtomicU64,

    /// Last activity timestamp (nanoseconds)
    pub last_activity: AtomicU64,

    /// Session tier: 0=LIGHT, 1=MEDIUM, 2=HEAVY
    pub tier: AtomicU8,

    /// Session flags
    pub flags: AtomicU8,

    /// Upgrade reason (if upgraded)
    pub upgrade_reason: AtomicU8,

    /// Padding to 256 bytes
    _padding: [u8; 256 - 3 * 8 - 3],
}

impl LightMetadata {
    pub const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            created_at: AtomicU64::new(0),
            last_activity: AtomicU64::new(0),
            tier: AtomicU8::new(0), // LIGHT
            flags: AtomicU8::new(0),
            upgrade_reason: AtomicU8::new(0),
            _padding: [0; 256 - 3 * 8 - 3],
        }
    }

    /// Initialize for new session.
    pub fn init(&self, generation: u64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        self.generation.store(generation, Ordering::Release);
        self.created_at.store(now, Ordering::Release);
        self.last_activity.store(now, Ordering::Release);
        self.tier.store(0, Ordering::Release);
        self.flags.store(1, Ordering::Release); // Active
    }

    /// Update last activity.
    #[inline]
    pub fn touch(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        self.last_activity.store(now, Ordering::Release);
    }
}

// ============================================================================
// Error types
// ============================================================================

/// Light debugger errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightDebugError {
    /// Already at first snapshot
    AtFirstSnapshot,
    /// Snapshot is invalid (overwritten)
    SnapshotInvalid,
    /// Breakpoint table is full
    BreakpointTableFull,
    /// No empty slot available
    NoEmptySlot,
    /// Not attached to a process
    NotAttached,
    /// Already attached
    AlreadyAttached,
    /// Ptrace operation failed
    PtraceError,
}

impl std::fmt::Display for LightDebugError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AtFirstSnapshot => write!(f, "Already at first snapshot"),
            Self::SnapshotInvalid => write!(f, "Snapshot is invalid"),
            Self::BreakpointTableFull => write!(f, "Breakpoint table full"),
            Self::NoEmptySlot => write!(f, "No empty slot"),
            Self::NotAttached => write!(f, "Not attached to process"),
            Self::AlreadyAttached => write!(f, "Already attached"),
            Self::PtraceError => write!(f, "Ptrace operation failed"),
        }
    }
}

impl std::error::Error for LightDebugError {}

// ============================================================================
// Upgrade reason enum
// ============================================================================

/// Reason for upgrading from LIGHT to MEDIUM tier.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeReason {
    /// No upgrade needed
    None = 0,
    /// Snapshot count exceeded threshold
    SnapshotThreshold = 1,
    /// Breakpoint count exceeded threshold
    BreakpointThreshold = 2,
    /// Trace buffer near full
    TraceBufferFull = 3,
    /// User requested upgrade
    UserRequested = 4,
}

// ============================================================================
// LightDebuggerCapsule - 64 KB
// ============================================================================

/// LightDebuggerCapsule - Minimal 64KB debugger for quick attach/inspect.
///
/// # Memory Layout (65,536 bytes = 64 KB)
///
/// | Component          | Size (bytes) | Purpose                    |
/// |--------------------|--------------|----------------------------|
/// | LightExecutionState| 256          | Process state + registers  |
/// | MiniReplayEngine   | 4,096        | 64 mini-snapshots          |
/// | LightBreakpointTable | 8,192      | 128 breakpoints            |
/// | BasicThreadState   | 4,096        | 16 threads × 256 bytes     |
/// | LightTraceBuffer   | 32,768       | 4,096 trace entries        |
/// | LightMetadata      | 256          | Session metadata           |
/// | Padding            | 15,872       | Alignment to 64KB          |
///
/// # Performance Targets
///
/// - Snapshot capture: <10ns
/// - Breakpoint lookup: <100ns
/// - Trace record: <10ns
/// - Attach: ~5μs (ptrace overhead)
///
/// #ASSUME_LOCKFREE_ONLY: All coordination via atomics
/// #ASSUME_CACHE_ALIGNED: 128B alignment prevents false sharing
/// #VERIFY_COMPILE_TIME: Static assertion on size
#[repr(C, align(128))]
pub struct LightDebuggerCapsule {
    // ====================================================================
    // Component breakdown (total: 49,664 bytes before padding)
    // ====================================================================

    /// Execution state (256 bytes)
    pub execution: LightExecutionState,

    /// Mini replay engine (4,096 bytes)
    pub replay: MiniReplayEngine,

    /// Breakpoint table (8,192 bytes)
    pub breakpoints: LightBreakpointTable,

    /// Thread states (16 × 256 = 4,096 bytes)
    pub threads: [LightThreadState; MAX_LIGHT_THREADS],

    /// Trace buffer (32,768 bytes)
    pub trace: LightTraceBuffer,

    /// Session metadata (256 bytes)
    pub metadata: LightMetadata,

    /// Padding to reach exactly 64KB (15,872 bytes)
    _padding: [u8; 15872],
}

// Compile-time size verification
const _: () = {
    assert!(
        std::mem::size_of::<LightDebuggerCapsule>() == 65536,
        "LightDebuggerCapsule must be exactly 65,536 bytes (64 KB)"
    );
    assert!(
        std::mem::align_of::<LightDebuggerCapsule>() == 128,
        "LightDebuggerCapsule must be 128-byte aligned"
    );
};

impl LightDebuggerCapsule {
    /// Create new light debugger capsule.
    ///
    /// # Arguments
    /// * `pid` - Process ID to attach to (0 for detached state)
    pub fn new(pid: u64) -> Self {
        const EMPTY_THREAD: LightThreadState = LightThreadState::empty();

        let capsule = Self {
            execution: LightExecutionState::new(),
            replay: MiniReplayEngine::new(),
            breakpoints: LightBreakpointTable::new(),
            threads: [EMPTY_THREAD; MAX_LIGHT_THREADS],
            trace: LightTraceBuffer::new(),
            metadata: LightMetadata::new(),
            _padding: [0; 15872],
        };

        if pid != 0 {
            capsule.execution.attach(pid);
        }

        capsule
    }

    // ========================================================================
    // Core API
    // ========================================================================

    /// Attach to process.
    ///
    /// # Arguments
    /// * `pid` - Process ID to attach to
    ///
    /// # Performance
    /// ~5μs (ptrace overhead)
    pub fn attach(&self, pid: u64) -> Result<(), LightDebugError> {
        if self.execution.is_attached() {
            return Err(LightDebugError::AlreadyAttached);
        }

        self.execution.attach(pid);
        self.metadata.touch();
        self.trace.record(pid, 1, 0); // Event type 1 = attach

        Ok(())
    }

    /// Take mini-snapshot (registers only).
    ///
    /// # Performance
    /// <10ns target
    ///
    /// #ASSUME_LOCKFREE_ONLY: Uses only atomic operations
    /// #VERIFY_UNIT_TEST: test_mini_snapshot_timing
    #[inline]
    pub fn take_mini_snapshot(&self) -> Result<u64, LightDebugError> {
        let rip = self.execution.get_rip();
        let rsp = self.execution.rsp.load(Ordering::Relaxed);
        let rbp = self.execution.rbp.load(Ordering::Relaxed);

        let id = self.replay.take_snapshot(rip, rsp, rbp)?;
        self.metadata.touch();

        Ok(id)
    }

    /// Set breakpoint at address.
    ///
    /// # Performance
    /// <100ns
    pub fn set_breakpoint(&self, addr: u64) -> Result<u8, LightDebugError> {
        // In real implementation: read original byte via ptrace
        let original_byte = 0x90; // NOP placeholder

        let idx = self.breakpoints.add(addr, original_byte)?;
        self.trace.record(addr, 2, 0); // Event type 2 = breakpoint set
        self.metadata.touch();

        Ok(idx)
    }

    /// Get current execution state (lockfree read).
    ///
    /// # Performance
    /// <10ns
    #[inline]
    pub fn get_execution_state(&self) -> LightExecutionStateSnapshot {
        LightExecutionStateSnapshot {
            pid: self.execution.get_pid(),
            rip: self.execution.get_rip(),
            rsp: self.execution.rsp.load(Ordering::Acquire),
            rbp: self.execution.rbp.load(Ordering::Acquire),
            state: self.execution.state.load(Ordering::Acquire),
        }
    }

    /// Check if upgrade to MEDIUM tier is needed.
    ///
    /// Upgrade triggers:
    /// - snapshot_count >= 48 (75% of 64)
    /// - breakpoint_count >= 96 (75% of 128)
    /// - trace_buffer 90% full
    #[inline]
    pub fn upgrade_needed(&self) -> Option<UpgradeReason> {
        // Check snapshot threshold
        if self.replay.snapshot_count() >= UPGRADE_SNAPSHOT_THRESHOLD {
            return Some(UpgradeReason::SnapshotThreshold);
        }

        // Check breakpoint threshold
        if self.breakpoints.breakpoint_count() >= UPGRADE_BREAKPOINT_THRESHOLD {
            return Some(UpgradeReason::BreakpointThreshold);
        }

        // Check trace buffer fill ratio
        if self.trace.fill_ratio() >= UPGRADE_TRACE_THRESHOLD {
            return Some(UpgradeReason::TraceBufferFull);
        }

        None
    }

    /// Get session statistics.
    pub fn get_stats(&self) -> LightDebuggerStats {
        LightDebuggerStats {
            snapshot_count: self.replay.snapshot_count(),
            breakpoint_count: self.breakpoints.breakpoint_count(),
            trace_count: self.trace.trace_count(),
            trace_fill_ratio: self.trace.fill_ratio(),
            is_attached: self.execution.is_attached(),
            upgrade_reason: self.upgrade_needed(),
        }
    }
}

// ============================================================================
// Snapshot types
// ============================================================================

/// Execution state snapshot (read-only copy).
#[derive(Debug, Clone, Copy)]
pub struct LightExecutionStateSnapshot {
    pub pid: u64,
    pub rip: u64,
    pub rsp: u64,
    pub rbp: u64,
    pub state: u8,
}

/// Light debugger statistics.
#[derive(Debug, Clone)]
pub struct LightDebuggerStats {
    pub snapshot_count: u64,
    pub breakpoint_count: u64,
    pub trace_count: u64,
    pub trace_fill_ratio: f64,
    pub is_attached: bool,
    pub upgrade_reason: Option<UpgradeReason>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    // ===== Size/Alignment Tests =====

    #[test]
    fn test_light_debugger_capsule_size() {
        assert_eq!(
            size_of::<LightDebuggerCapsule>(),
            65536,
            "LightDebuggerCapsule must be exactly 64KB"
        );
    }

    #[test]
    fn test_light_debugger_capsule_alignment() {
        assert_eq!(
            align_of::<LightDebuggerCapsule>(),
            128,
            "LightDebuggerCapsule must be 128-byte aligned"
        );
    }

    #[test]
    fn test_component_sizes() {
        assert_eq!(size_of::<LightExecutionState>(), 256);
        assert_eq!(size_of::<MiniReplayEngine>(), 4096);
        assert_eq!(size_of::<LightBreakpointTable>(), 8192);
        assert_eq!(size_of::<LightThreadState>(), 256);
        assert_eq!(size_of::<LightTraceBuffer>(), 32768);
        assert_eq!(size_of::<LightMetadata>(), 256);
        assert_eq!(size_of::<MiniSnapshot>(), 64);
        assert_eq!(size_of::<LightBreakpointEntry>(), 64);
    }

    // ===== Functional Tests =====

    #[test]
    fn test_new_capsule() {
        let capsule = Box::new(LightDebuggerCapsule::new(0));
        assert!(!capsule.execution.is_attached());
        assert_eq!(capsule.replay.snapshot_count(), 0);
        assert_eq!(capsule.breakpoints.breakpoint_count(), 0);
    }

    #[test]
    fn test_attach() {
        let capsule = Box::new(LightDebuggerCapsule::new(0));
        capsule.attach(12345).unwrap();
        assert!(capsule.execution.is_attached());
        assert_eq!(capsule.execution.get_pid(), 12345);
    }

    #[test]
    fn test_mini_snapshot() {
        let capsule = Box::new(LightDebuggerCapsule::new(12345));
        capsule.execution.rip.store(0x1000, Ordering::Release);
        capsule.execution.rsp.store(0x7fff_0000, Ordering::Release);
        capsule.execution.rbp.store(0x7fff_0008, Ordering::Release);

        let id = capsule.take_mini_snapshot().unwrap();
        assert_eq!(id, 0);
        assert_eq!(capsule.replay.snapshot_count(), 1);
    }

    #[test]
    fn test_breakpoint_management() {
        let capsule = Box::new(LightDebuggerCapsule::new(12345));

        let idx0 = capsule.set_breakpoint(0x1000).unwrap();
        assert_eq!(idx0, 0);

        let idx1 = capsule.set_breakpoint(0x2000).unwrap();
        assert_eq!(idx1, 1);

        assert_eq!(capsule.breakpoints.breakpoint_count(), 2);
        assert!(capsule.breakpoints.find(0x1000).is_some());
        assert!(capsule.breakpoints.find(0x2000).is_some());
        assert!(capsule.breakpoints.find(0x3000).is_none());
    }

    #[test]
    fn test_upgrade_trigger_snapshots() {
        let capsule = Box::new(LightDebuggerCapsule::new(12345));

        // Take snapshots up to threshold
        for i in 0..UPGRADE_SNAPSHOT_THRESHOLD {
            capsule.execution.rip.store(0x1000 + i * 4, Ordering::Release);
            capsule.take_mini_snapshot().unwrap();
        }

        let reason = capsule.upgrade_needed();
        assert_eq!(reason, Some(UpgradeReason::SnapshotThreshold));
    }

    #[test]
    fn test_upgrade_trigger_breakpoints() {
        let capsule = Box::new(LightDebuggerCapsule::new(12345));

        // Add breakpoints up to threshold
        for i in 0..UPGRADE_BREAKPOINT_THRESHOLD {
            capsule.set_breakpoint(0x1000 + i * 4).unwrap();
        }

        let reason = capsule.upgrade_needed();
        assert_eq!(reason, Some(UpgradeReason::BreakpointThreshold));
    }

    #[test]
    fn test_execution_state_snapshot() {
        let capsule = Box::new(LightDebuggerCapsule::new(12345));
        capsule.execution.rip.store(0xDEAD_BEEF, Ordering::Release);
        capsule.execution.rsp.store(0x7fff_0000, Ordering::Release);
        capsule.execution.rbp.store(0x7fff_0008, Ordering::Release);
        capsule.execution.state.store(2, Ordering::Release); // Paused

        let state = capsule.get_execution_state();
        assert_eq!(state.pid, 12345);
        assert_eq!(state.rip, 0xDEAD_BEEF);
        assert_eq!(state.rsp, 0x7fff_0000);
        assert_eq!(state.rbp, 0x7fff_0008);
        assert_eq!(state.state, 2);
    }

    #[test]
    fn test_dual_atomic_coordination() {
        let dual = DualAtomicU64::new();

        // Initial state
        let (gen, state) = dual.load();
        assert_eq!(gen, 0);
        assert_eq!(state, 0);

        // Store increments generation
        dual.store(42);
        let (gen, state) = dual.load();
        assert_eq!(gen, 1);
        assert_eq!(state, 42);

        // Compare-exchange with correct generation
        let result = dual.compare_exchange(1, 42, 100);
        assert!(result.is_ok());
        let (gen, state) = dual.load();
        assert_eq!(gen, 2);
        assert_eq!(state, 100);

        // Compare-exchange with wrong generation fails
        let result = dual.compare_exchange(1, 100, 200);
        assert!(result.is_err());
    }

    #[test]
    fn test_generation_counter_wraparound() {
        let dual = DualAtomicU64::new();

        // Set generation to near max
        dual.value.store((u32::MAX as u64 - 1) << 32, Ordering::Release);

        // Store should wrap around
        dual.store(1);
        dual.store(2);
        dual.store(3);

        let (gen, _) = dual.load();
        assert_eq!(gen, 1); // Wrapped from MAX-1 -> MAX -> 0 -> 1
    }

    // ===== Timing Tests (informational) =====

    #[test]
    fn test_snapshot_timing() {
        let capsule = Box::new(LightDebuggerCapsule::new(12345));
        capsule.execution.rip.store(0x1000, Ordering::Release);
        capsule.execution.rsp.store(0x7fff_0000, Ordering::Release);

        // Warmup
        for _ in 0..100 {
            capsule.take_mini_snapshot().ok();
        }

        // Measure
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            capsule.take_mini_snapshot().ok();
        }
        let elapsed = start.elapsed();
        let per_op = elapsed.as_nanos() / 1000;

        // Target: <10ns, but allow higher for CI environments
        assert!(
            per_op < 1000, // 1μs max (generous for CI)
            "Snapshot should be fast, got {}ns",
            per_op
        );

        println!("Snapshot timing: {}ns per operation", per_op);
    }
}
