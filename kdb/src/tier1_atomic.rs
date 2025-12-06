//! T1 Atomic Tier Components
//!
//! Lockfree execution state, breakpoints, watchpoints, thread state.
//! Total budget: 64 KB

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};

// ============================================================================
// ExecutionStateCapsule - 4 KB
// ============================================================================

#[repr(C, align(64))]
pub struct ExecutionStateCapsule {
    /// Process ID
    pub pid: AtomicU64,

    /// Current instruction pointer
    pub rip: AtomicU64,

    /// Stack pointer
    pub rsp: AtomicU64,

    /// Base pointer
    pub rbp: AtomicU64,

    /// Execution state: 0=running, 1=paused, 2=crashed, 3=exited
    pub state: AtomicU8,

    /// Signal that caused stop (0 if none)
    pub stop_signal: AtomicU8,

    /// Number of instructions executed
    pub instruction_count: AtomicU64,

    /// Number of breakpoints hit
    pub breakpoint_hits: AtomicU64,

    /// Last error code
    pub last_error: AtomicU32,

    /// Generation counter for TOCTOU prevention
    pub generation: AtomicU64,

    _padding: [u8; 4096 - 10 * 8 - 2 * 1 - 4 - 2],
}

impl ExecutionStateCapsule {
    pub fn new(pid: u64) -> Self {
        Self {
            pid: AtomicU64::new(pid),
            rip: AtomicU64::new(0),
            rsp: AtomicU64::new(0),
            rbp: AtomicU64::new(0),
            state: AtomicU8::new(0),
            stop_signal: AtomicU8::new(0),
            instruction_count: AtomicU64::new(0),
            breakpoint_hits: AtomicU64::new(0),
            last_error: AtomicU32::new(0),
            generation: AtomicU64::new(0),
            _padding: [0; 4096 - 10 * 8 - 2 * 1 - 4 - 2],
        }
    }

    pub fn get_pid(&self) -> u64 {
        self.pid.load(Ordering::Relaxed)
    }

    pub fn get_rip(&self) -> u64 {
        self.rip.load(Ordering::Acquire)
    }

    pub fn set_rip(&self, addr: u64) {
        self.rip.store(addr, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    pub fn is_running(&self) -> bool {
        self.state.load(Ordering::Acquire) == 0
    }

    pub fn pause(&self) {
        self.state.store(1, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    pub fn resume(&self) {
        self.state.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }
}

// ============================================================================
// BreakpointEntry - 64 bytes
// ============================================================================

#[repr(C, align(64))]
pub struct BreakpointEntry {
    /// Breakpoint address
    pub address: AtomicU64,

    /// Hit count
    pub hit_count: AtomicU64,

    /// Enabled flag
    pub enabled: AtomicU8,

    /// Condition type: 0=always, 1=count, 2=expression
    pub condition_type: AtomicU8,

    /// Condition value (count threshold or expression ID)
    pub condition_value: AtomicU64,

    /// Original instruction byte (for software breakpoints)
    pub original_byte: AtomicU8,

    _padding: [u8; 64 - 5 * 8 - 3 * 1 - 5],
}

impl BreakpointEntry {
    pub const fn empty() -> Self {
        Self {
            address: AtomicU64::new(0),
            hit_count: AtomicU64::new(0),
            enabled: AtomicU8::new(0),
            condition_type: AtomicU8::new(0),
            condition_value: AtomicU64::new(0),
            original_byte: AtomicU8::new(0),
            _padding: [0; 64 - 5 * 8 - 3 * 1 - 5],
        }
    }

    pub fn set(&self, address: u64, original_byte: u8) {
        self.address.store(address, Ordering::Release);
        self.original_byte.store(original_byte, Ordering::Release);
        self.enabled.store(1, Ordering::Release);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire) != 0
    }

    pub fn hit(&self) -> u64 {
        self.hit_count.fetch_add(1, Ordering::Relaxed)
    }
}

// ============================================================================
// BreakpointTableCapsule - 16 KB (256 breakpoints)
// ============================================================================

#[repr(C, align(64))]
pub struct BreakpointTableCapsule {
    /// Number of active breakpoints
    pub count: AtomicU64,

    /// Breakpoint entries (256 × 64 bytes = 16,384 bytes)
    pub entries: [BreakpointEntry; 256],
}

impl BreakpointTableCapsule {
    pub fn new() -> Self {
        const EMPTY: BreakpointEntry = BreakpointEntry::empty();
        Self {
            count: AtomicU64::new(0),
            entries: [EMPTY; 256],
        }
    }

    pub fn add_breakpoint(&self, address: u64, original_byte: u8) -> Result<usize, &'static str> {
        let count = self.count.load(Ordering::Acquire);
        if count >= 256 {
            return Err("Breakpoint table full");
        }

        // Find first empty slot
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.address.load(Ordering::Relaxed) == 0 {
                entry.set(address, original_byte);
                self.count.fetch_add(1, Ordering::Release);
                return Ok(i);
            }
        }

        Err("No empty slot found")
    }

    pub fn find_breakpoint(&self, address: u64) -> Option<usize> {
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.address.load(Ordering::Relaxed) == address && entry.is_enabled() {
                return Some(i);
            }
        }
        None
    }
}

// ============================================================================
// WatchpointEntry - 64 bytes
// ============================================================================

#[repr(C, align(64))]
pub struct WatchpointEntry {
    /// Watched address
    pub address: AtomicU64,

    /// Size of watched region (1, 2, 4, 8 bytes)
    pub size: AtomicU8,

    /// Type: 0=read, 1=write, 2=read+write, 3=execute
    pub watch_type: AtomicU8,

    /// Hit count
    pub hit_count: AtomicU64,

    /// Enabled flag
    pub enabled: AtomicU8,

    /// Last value seen
    pub last_value: AtomicU64,

    _padding: [u8; 64 - 4 * 8 - 3 * 1 - 5],
}

impl WatchpointEntry {
    pub const fn empty() -> Self {
        Self {
            address: AtomicU64::new(0),
            size: AtomicU8::new(0),
            watch_type: AtomicU8::new(0),
            hit_count: AtomicU64::new(0),
            enabled: AtomicU8::new(0),
            last_value: AtomicU64::new(0),
            _padding: [0; 64 - 4 * 8 - 3 * 1 - 5],
        }
    }

    pub fn set(&self, address: u64, size: u8, watch_type: u8) {
        self.address.store(address, Ordering::Release);
        self.size.store(size, Ordering::Release);
        self.watch_type.store(watch_type, Ordering::Release);
        self.enabled.store(1, Ordering::Release);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire) != 0
    }

    pub fn hit(&self, value: u64) -> u64 {
        self.last_value.store(value, Ordering::Relaxed);
        self.hit_count.fetch_add(1, Ordering::Relaxed)
    }
}

// ============================================================================
// WatchpointTableCapsule - 4 KB (64 watchpoints)
// ============================================================================

#[repr(C, align(64))]
pub struct WatchpointTableCapsule {
    /// Watchpoint entries (64 × 64 bytes = 4,096 bytes)
    pub entries: [WatchpointEntry; 64],
}

impl WatchpointTableCapsule {
    pub fn new() -> Self {
        const EMPTY: WatchpointEntry = WatchpointEntry::empty();
        Self {
            entries: [EMPTY; 64],
        }
    }

    pub fn add_watchpoint(
        &self,
        address: u64,
        size: u8,
        watch_type: u8,
    ) -> Result<usize, &'static str> {
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.address.load(Ordering::Relaxed) == 0 {
                entry.set(address, size, watch_type);
                return Ok(i);
            }
        }
        Err("Watchpoint table full")
    }
}

// ============================================================================
// ThreadStateCapsule - 2.5 KB per thread
// ============================================================================

#[repr(C, align(64))]
pub struct ThreadStateCapsule {
    /// Thread ID
    pub tid: AtomicU64,

    /// Instruction pointer
    pub rip: AtomicU64,

    /// Stack pointer
    pub rsp: AtomicU64,

    /// Base pointer
    pub rbp: AtomicU64,

    /// Thread state: 0=running, 1=paused, 2=exited
    pub state: AtomicU8,

    /// CPU affinity
    pub cpu: AtomicU8,

    /// Priority
    pub priority: AtomicU8,

    /// General purpose registers (16 × 8 = 128 bytes)
    pub regs: [AtomicU64; 16],

    _padding: [u8; 2560 - (4 + 16) * 8 - 3 * 1 - 5],
}

impl ThreadStateCapsule {
    pub const fn empty() -> Self {
        const ZERO: AtomicU64 = AtomicU64::new(0);
        Self {
            tid: AtomicU64::new(0),
            rip: AtomicU64::new(0),
            rsp: AtomicU64::new(0),
            rbp: AtomicU64::new(0),
            state: AtomicU8::new(0),
            cpu: AtomicU8::new(0),
            priority: AtomicU8::new(0),
            regs: [ZERO; 16],
            _padding: [0; 2560 - (4 + 16) * 8 - 3 * 1 - 5],
        }
    }

    pub fn init(&self, tid: u64) {
        self.tid.store(tid, Ordering::Release);
        self.state.store(0, Ordering::Release);
    }

    pub fn is_active(&self) -> bool {
        self.tid.load(Ordering::Relaxed) != 0
    }
}
