//! Atomic GPU State Capsules
//!
//! Following "The Atomic Capsule" pattern, all GPU state is represented as
//! cache-aligned atomic snapshots enabling one-read decisions.
//!
//! # Capsule Types
//!
//! - **GpuStateCapsule (AGS-128)**: Primary GPU state (frequency, power, temp)
//! - **CommandCapsule (ACC-128)**: Command buffer submission metadata
//! - **MemoryCapsule (AMC-256)**: Memory allocation and GGTT state
//! - **FirmwareCapsule (AFC-128)**: GuC/HuC coordination state

use std::sync::atomic::{AtomicU64, Ordering};

/// GPU State Capsule (AGS-128) - 128-bit atomic GPU state
///
/// Layout:
/// - W0 (head): commit:1 | ver:8 | seq:16 | gpu_id:8 | reserved:31
/// - W1 (body): frequency_mhz:16 | power_mw:16 | temp_celsius:8 | utilization:8 | ver_tail:8
///
/// Decision: Is GPU ready for command submission?
#[repr(C, align(64))]
pub struct GpuStateCapsule {
    head: AtomicU64,
    body: AtomicU64,
}

impl Default for GpuStateCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuStateCapsule {
    /// Create new GPU state capsule
    pub const fn new() -> Self {
        Self {
            head: AtomicU64::new(0),
            body: AtomicU64::new(0),
        }
    }

    /// Publish new GPU state (writer only)
    ///
    /// Two-phase commit:
    /// 1. Update body with odd version
    /// 2. Publish head with even version (commit=1)
    pub fn publish(&self, state: GpuState) {
        let seq = ((self.head.load(Ordering::Relaxed) >> 39) & 0xFFFF).wrapping_add(1);
        let ver = (seq & 0xFF) as u8;

        // Phase 1: Write body with new version
        let body = pack_gpu_state(state, ver);
        self.body.store(body, Ordering::Release);

        // Phase 2: Commit head with same version and commit bit
        let head = pack_head(1, ver, seq as u16, state.gpu_id);
        self.head.store(head, Ordering::Release);
    }

    /// Read GPU state (lockfree, single load)
    pub fn read(&self) -> GpuState {
        let h = self.head.load(Ordering::Acquire);

        // Check if ever published (sequence > 0)
        let seq = (h >> 39) & 0xFFFF;
        if seq == 0 {
            return GpuState::invalid();
        }

        if !is_committed_even(h) {
            return GpuState::invalid();
        }

        let b = self.body.load(Ordering::Acquire);
        if !head_tail_match(h, b) {
            return GpuState::invalid();
        }

        unpack_gpu_state(h, b)
    }
}

/// GPU State snapshot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuState {
    /// GPU ID (0-255)
    pub gpu_id: u8,
    /// Current frequency in MHz
    pub frequency_mhz: u16,
    /// Power consumption in milliwatts
    pub power_mw: u16,
    /// Temperature in Celsius
    pub temp_celsius: u8,
    /// GPU utilization percentage (0-100)
    pub utilization: u8,
    /// Valid state flag
    pub valid: bool,
}

impl GpuState {
    /// Create invalid state
    fn invalid() -> Self {
        Self {
            gpu_id: 0,
            frequency_mhz: 0,
            power_mw: 0,
            temp_celsius: 0,
            utilization: 0,
            valid: false,
        }
    }

    /// Check if state is valid
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Check if GPU is ready for work
    pub fn is_ready(&self) -> bool {
        self.valid && self.temp_celsius < 95 && self.utilization < 95
    }
}

// CommandCapsule and CommandState moved to command.rs module - use those instead

/// Memory Capsule (AMC-256) - Memory allocation state
#[repr(C, align(64))]
pub struct MemoryCapsule {
    head: AtomicU64,
    body0: AtomicU64,
    body1: AtomicU64,
    tail: AtomicU64,
}

impl Default for MemoryCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryCapsule {
    /// Create new memory capsule
    pub const fn new() -> Self {
        Self {
            head: AtomicU64::new(0),
            body0: AtomicU64::new(0),
            body1: AtomicU64::new(0),
            tail: AtomicU64::new(0),
        }
    }

    /// Read memory state
    pub fn read(&self) -> MemoryState {
        let h = self.head.load(Ordering::Relaxed);
        if !is_committed_even(h) {
            return MemoryState::invalid();
        }

        let b0 = self.body0.load(Ordering::Relaxed);
        let b1 = self.body1.load(Ordering::Relaxed);
        let t = self.tail.load(Ordering::Relaxed);

        if !head_tail_match(h, t) {
            return MemoryState::invalid();
        }

        unpack_memory_state(h, b0, b1, t)
    }

    /// Check if allocation is possible (lockfree hot path)
    ///
    /// Returns true if requested MB can be allocated.
    /// This is a lockfree decision point (<5ns target).
    #[inline(always)]
    pub fn can_allocate(&self, size_mb: u16) -> bool {
        let state = self.read();
        if !state.valid {
            return false;
        }

        let required_bytes = (size_mb as u64) * 1024 * 1024;
        state.available_vram >= required_bytes
    }

    /// Publish new memory state (writer only)
    ///
    /// Two-phase commit for memory state updates
    pub fn publish(&self, total: u64, used: u64, available: u64) {
        let seq = ((self.head.load(Ordering::Relaxed) >> 39) & 0xFFFF).wrapping_add(1);
        let ver = (seq & 0xFF) as u8;

        // Phase 1: Write bodies
        self.body0.store(total, Ordering::Release);

        let body1 = ((used & 0xFFFFFFFF) << 32) | (available & 0xFFFFFFFF);
        self.body1.store(body1, Ordering::Release);

        // Phase 2: Write tail with version
        let tail_val = ver as u64;
        self.tail.store(tail_val, Ordering::Release);

        // Phase 3: Commit head
        let head = pack_memory_head(1, ver, seq as u16);
        self.head.store(head, Ordering::Release);
    }
}

/// Memory state snapshot
#[derive(Debug, Clone, Copy)]
pub struct MemoryState {
    /// Total VRAM in bytes
    pub total_vram: u64,
    /// Used VRAM in bytes
    pub used_vram: u64,
    /// Available VRAM in bytes
    pub available_vram: u64,
    /// Valid state flag
    pub valid: bool,
}

impl MemoryState {
    fn invalid() -> Self {
        Self {
            total_vram: 0,
            used_vram: 0,
            available_vram: 0,
            valid: false,
        }
    }

    /// Check if state is valid
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Check if enough memory is available
    pub fn has_available(&self, required: u64) -> bool {
        self.valid && self.available_vram >= required
    }
}

// Helper functions for bit packing

fn is_committed_even(head: u64) -> bool {
    let commit = (head >> 63) & 1;
    commit == 1
}

fn head_tail_match(head: u64, tail: u64) -> bool {
    let head_ver = (head >> 55) & 0xFF;
    let tail_ver = tail & 0xFF;
    head_ver == tail_ver
}

fn pack_head(commit: u8, ver: u8, seq: u16, gpu_id: u8) -> u64 {
    ((commit as u64) << 63) | ((ver as u64) << 55) | ((seq as u64) << 39) | ((gpu_id as u64) << 31)
}

fn pack_memory_head(commit: u8, ver: u8, seq: u16) -> u64 {
    ((commit as u64) << 63) | ((ver as u64) << 55) | ((seq as u64) << 39)
}

fn pack_gpu_state(state: GpuState, ver: u8) -> u64 {
    ((state.frequency_mhz as u64) << 48)
        | ((state.power_mw as u64) << 32)
        | ((state.temp_celsius as u64) << 24)
        | ((state.utilization as u64) << 16)
        | (ver as u64)
}

fn unpack_gpu_state(head: u64, body: u64) -> GpuState {
    GpuState {
        gpu_id: ((head >> 31) & 0xFF) as u8,
        frequency_mhz: ((body >> 48) & 0xFFFF) as u16,
        power_mw: ((body >> 32) & 0xFFFF) as u16,
        temp_celsius: ((body >> 24) & 0xFF) as u8,
        utilization: ((body >> 16) & 0xFF) as u8,
        valid: true,
    }
}

// unpack_command_state removed - CommandState now in command.rs module

fn unpack_memory_state(_head: u64, b0: u64, b1: u64, _tail: u64) -> MemoryState {
    // Memory layout:
    // b0[63:0]  = total_vram (64 bits)
    // b1[63:32] = used_vram_high (32 bits)
    // b1[31:0]  = available_vram_low (32 bits)

    let total_vram = b0;
    let used_vram = (b1 >> 32);
    let available_vram = (b1 & 0xFFFFFFFF);

    MemoryState {
        total_vram,
        used_vram,
        available_vram,
        valid: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_state_capsule_basic() {
        let capsule = GpuStateCapsule::new();
        let state = GpuState {
            gpu_id: 1,
            frequency_mhz: 2100,
            power_mw: 45000,
            temp_celsius: 65,
            utilization: 75,
            valid: true,
        };

        capsule.publish(state);
        let read_state = capsule.read();

        assert!(read_state.is_valid());
        assert_eq!(read_state.gpu_id, state.gpu_id);
        assert_eq!(read_state.frequency_mhz, state.frequency_mhz);
    }

    #[test]
    fn test_gpu_state_ready_check() {
        let state = GpuState {
            gpu_id: 1,
            frequency_mhz: 2100,
            power_mw: 45000,
            temp_celsius: 65,
            utilization: 50,
            valid: true,
        };

        assert!(state.is_ready());

        let hot_state = GpuState {
            temp_celsius: 96,
            ..state
        };
        assert!(!hot_state.is_ready());
    }

    #[test]
    fn test_memory_capsule_availability() {
        let capsule = MemoryCapsule::new();
        let state = capsule.read();

        // Initial read should be invalid
        assert!(!state.is_valid());
    }
}
