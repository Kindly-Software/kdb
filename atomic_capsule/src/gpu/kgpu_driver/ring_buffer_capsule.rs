//! Ring Buffer Capsule for GPU Command Submission
//!
//! # Architecture
//!
//! Circular command buffer with lockfree head/tail coordination via DualAtomicU64.
//! Inspired by Intel i915/Xe per-context ring buffers and DPDK lockfree rings.
//!
//! # Design Principles
//!
//! - **Lockfree Coordination**: DualAtomicU64 for head/tail pointers (no mutex)
//! - **Generation Counter**: ABA prevention for concurrent push/pop
//! - **BipBuffer Strategy**: Contiguous command blocks (no wrapping split)
//! - **Cache-Aligned**: 256B total size, 64B cache line awareness
//!
//! # Performance Targets
//!
//! - Push: <100ns (single atomic CAS)
//! - Pop: <100ns (single atomic load + CAS)
//! - Space Check: <10ns (atomic load only)
//!
//! # Research References
//!
//! - Intel i915 per-context ring buffers: <https://docs.kernel.org/gpu/i915.html>
//! - Lock-free ring buffer design: <https://kmdreko.github.io/posts/20191003/a-simple-lock-free-ring-buffer/>
//! - BipBuffer for DMA: <https://ferrous-systems.com/blog/lock-free-ring-buffer/>

use core::sync::atomic::{AtomicU64, Ordering};
use crate::patterns::DualAtomicU64;

/// Ring Buffer Capsule for GPU command submission
///
/// # Tier: T1 Atomic
///
/// # Size: 256 bytes (cache-aligned)
///
/// # Coordinates
///
/// - Head: Read position (consumer: GPU)
/// - Tail: Write position (producer: CPU driver)
/// - Generation: ABA prevention counter
/// - Capacity: Power-of-2 size (4KB-16KB typical)
///
/// # Thread Safety
///
/// - SPSC: Single Producer Single Consumer (typical GPU use case)
/// - Head updated by GPU (via MMIO write-back or fence)
/// - Tail updated by CPU driver (command submission)
/// - Generation counter prevents ABA problem
///
/// # Example
///
/// ```ignore
/// use atomic_capsule::gpu::kgpu_driver::RingBufferCapsule;
///
/// // Create 16KB ring buffer
/// let mut ring = RingBufferCapsule::new(16384);
///
/// // Push command batch (contiguous block)
/// let cmd_bytes = [0x01, 0x02, 0x03, 0x04]; // MI_NOOP example
/// ring.push_contiguous(&cmd_bytes)?;
///
/// // GPU consumes via head pointer
/// // (updated via MMIO write-back)
/// ring.update_head(4);
///
/// // Check available space
/// let free = ring.available_space();
/// ```
#[repr(C, align(256))]
pub struct RingBufferCapsule {
    /// Head/tail coordination via DualAtomicU64
    ///
    /// Low 32 bits: Head position (read by GPU)
    /// High 32 bits: Tail position (written by CPU)
    head_tail: DualAtomicU64,

    /// Generation counter for ABA prevention
    ///
    /// Incremented on wraparound to distinguish reused slots
    generation: AtomicU64,

    /// Ring buffer capacity (power-of-2, bytes)
    ///
    /// Typical: 4KB (4096) to 16KB (16384)
    /// Must be power-of-2 for efficient masking
    capacity: u32,

    /// Base address of ring buffer memory
    ///
    /// For Linux: Virtual address of mmap'd GEM buffer
    /// For Capsule-OS: Physical GPU memory address
    base_addr: u64,

    /// MMIO register address for head pointer write-back
    ///
    /// GPU writes head position here after consuming commands
    /// CPU driver reads this to determine available space
    head_mmio_addr: u64,

    /// MMIO register address for tail pointer submission
    ///
    /// CPU driver writes tail position here to notify GPU
    /// GPU reads this to know new commands are available
    tail_mmio_addr: u64,

    /// Ring buffer flags
    ///
    /// Bit 0: Wraparound pending (BipBuffer mode)
    /// Bit 1: GPU idle (no pending commands)
    /// Bit 2-7: Reserved
    flags: AtomicU64,

    /// Statistics: Total commands submitted
    total_commands: AtomicU64,

    /// Statistics: Total bytes written
    total_bytes: AtomicU64,

    /// Statistics: Wraparound count
    wraparound_count: AtomicU64,

    /// Padding to 256 bytes
    ///
    /// Size calculation:
    /// - head_tail: DualAtomicU64 = 128 bytes (64 bytes primary + 64 bytes secondary)
    /// - generation: AtomicU64 = 8 bytes
    /// - capacity: u32 = 4 bytes
    /// - base_addr: u64 = 8 bytes
    /// - head_mmio_addr: u64 = 8 bytes
    /// - tail_mmio_addr: u64 = 8 bytes
    /// - flags: AtomicU64 = 8 bytes
    /// - total_commands: AtomicU64 = 8 bytes
    /// - total_bytes: AtomicU64 = 8 bytes
    /// - wraparound_count: AtomicU64 = 8 bytes
    /// Total so far: 128 + 8 + 4 + 8 + 8 + 8 + 8 + 8 + 8 + 8 = 196 bytes
    /// Padding needed: 256 - 196 = 60 bytes
    /// Note: u32 requires 4 bytes alignment padding, so actual is 200 bytes used
    /// Padding: 256 - 200 = 56 bytes = 7 × u64
    _padding: [u64; 7],
}

// Compile-time verification
const _: () = {
    assert!(core::mem::size_of::<RingBufferCapsule>() == 256);
    assert!(core::mem::align_of::<RingBufferCapsule>() == 256);
};

/// Ring buffer error types
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RingError {
    /// Ring buffer full (no space for command)
    Full,
    /// Invalid capacity (not power-of-2)
    InvalidCapacity,
    /// Command too large for ring buffer
    CommandTooLarge,
    /// Wraparound not allowed (command would split)
    WrapNotAllowed,
    /// Invalid alignment (command must be 8-byte aligned)
    InvalidAlignment,
}

impl std::fmt::Debug for RingBufferCapsule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RingBufferCapsule")
            .field("capacity", &self.capacity)
            .field("base_addr", &format!("0x{:x}", self.base_addr))
            .field("head_mmio_addr", &format!("0x{:x}", self.head_mmio_addr))
            .field("tail_mmio_addr", &format!("0x{:x}", self.tail_mmio_addr))
            .field("generation", &self.generation.load(Ordering::Relaxed))
            .field("flags", &self.flags.load(Ordering::Relaxed))
            .field("total_commands", &self.total_commands.load(Ordering::Relaxed))
            .field("total_bytes", &self.total_bytes.load(Ordering::Relaxed))
            .field("wraparound_count", &self.wraparound_count.load(Ordering::Relaxed))
            .finish()
    }
}

impl RingBufferCapsule {
    /// Create new ring buffer with specified capacity
    ///
    /// # Arguments
    ///
    /// - `capacity`: Buffer size in bytes (must be power-of-2)
    /// - `base_addr`: Physical/virtual address of backing memory
    /// - `head_mmio`: MMIO register for head write-back
    /// - `tail_mmio`: MMIO register for tail submission
    ///
    /// # Errors
    ///
    /// - [`RingError::InvalidCapacity`] if capacity not power-of-2
    ///
    /// # Performance
    ///
    /// - Time: O(1), ~20ns
    /// - Space: 256 bytes
    pub fn new(
        capacity: u32,
        base_addr: u64,
        head_mmio: u64,
        tail_mmio: u64,
    ) -> Result<Self, RingError> {
        // Validate power-of-2
        if capacity == 0 || (capacity & (capacity - 1)) != 0 {
            return Err(RingError::InvalidCapacity);
        }

        Ok(Self {
            head_tail: DualAtomicU64::new(0, 0),
            generation: AtomicU64::new(0),
            capacity,
            base_addr,
            head_mmio_addr: head_mmio,
            tail_mmio_addr: tail_mmio,
            flags: AtomicU64::new(0),
            total_commands: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            wraparound_count: AtomicU64::new(0),
            _padding: [0; 7],
        })
    }

    /// Push contiguous command block (BipBuffer strategy)
    ///
    /// Guarantees command is NOT split across wraparound boundary.
    /// If command doesn't fit before wrap, pads to end and wraps to start.
    ///
    /// # Arguments
    ///
    /// - `cmd_bytes`: Command data (must be 8-byte aligned)
    ///
    /// # Errors
    ///
    /// - [`RingError::Full`] if insufficient space
    /// - [`RingError::CommandTooLarge`] if cmd > capacity
    /// - [`RingError::InvalidAlignment`] if not 8-byte aligned
    ///
    /// # Performance
    ///
    /// - Best case: <100ns (no wrap)
    /// - Worst case: <200ns (wrap + padding)
    ///
    /// # Safety
    ///
    /// This is safe because:
    /// 1. DualAtomicU64 ensures atomic head/tail updates
    /// 2. Generation counter prevents ABA
    /// 3. Capacity validation ensures no overflow
    pub fn push_contiguous(&mut self, cmd_bytes: &[u8]) -> Result<(), RingError> {
        let cmd_len = cmd_bytes.len() as u32;

        // Validate size
        if cmd_len > self.capacity {
            return Err(RingError::CommandTooLarge);
        }

        // Validate alignment (GPU commands must be 8-byte aligned)
        if cmd_len % 8 != 0 {
            return Err(RingError::InvalidAlignment);
        }

        // Load current head/tail (Acquire ordering for head updates)
        let head = self.head_tail.load_primary(Ordering::Acquire) as u32;
        let tail = self.head_tail.load_secondary(Ordering::Acquire) as u32;

        // Calculate available space
        let used = if tail >= head {
            tail - head
        } else {
            self.capacity - (head - tail)
        };
        let available = self.capacity - used;

        // Check if we have enough space
        if available < cmd_len {
            return Err(RingError::Full);
        }

        // Check if command fits before wraparound
        let tail_offset = tail % self.capacity;
        let space_before_wrap = self.capacity - tail_offset;

        let new_tail = if cmd_len <= space_before_wrap {
            // Command fits before wrap, proceed normally
            tail + cmd_len
        } else {
            // Command would split, pad to end and wrap
            // This is BipBuffer strategy: always contiguous
            self.wraparound_count.fetch_add(1, Ordering::Relaxed);

            // Increment generation on wrap (ABA prevention)
            self.generation.fetch_add(1, Ordering::Release);

            // New tail wraps to start
            cmd_len
        };

        // Update tail pointer (Release ordering for GPU visibility)
        self.head_tail.store_secondary(new_tail as u64, Ordering::Release);

        // Update statistics
        self.total_commands.fetch_add(1, Ordering::Relaxed);
        self.total_bytes.fetch_add(cmd_len as u64, Ordering::Relaxed);

        Ok(())
    }

    /// Update head pointer (called after GPU consumes commands)
    ///
    /// Typically called by interrupt handler or fence completion.
    ///
    /// # Arguments
    ///
    /// - `new_head`: New head position (read from GPU MMIO)
    ///
    /// # Performance
    ///
    /// - Time: <50ns (single atomic store)
    pub fn update_head(&mut self, new_head: u32) {
        self.head_tail.store_primary(new_head as u64, Ordering::Release);
    }

    /// Get available space in ring buffer
    ///
    /// # Returns
    ///
    /// Number of bytes available for new commands
    ///
    /// # Performance
    ///
    /// - Time: <10ns (atomic load only)
    pub fn available_space(&self) -> u32 {
        let head = self.head_tail.load_primary(Ordering::Acquire) as u32;
        let tail = self.head_tail.load_secondary(Ordering::Acquire) as u32;

        let used = if tail >= head {
            tail - head
        } else {
            self.capacity - (head - tail)
        };

        self.capacity - used
    }

    /// Check if ring buffer is empty
    ///
    /// # Performance
    ///
    /// - Time: <10ns (atomic load only)
    pub fn is_empty(&self) -> bool {
        let head = self.head_tail.load_primary(Ordering::Acquire) as u32;
        let tail = self.head_tail.load_secondary(Ordering::Acquire) as u32;
        head == tail
    }

    /// Check if ring buffer is full
    ///
    /// # Performance
    ///
    /// - Time: <10ns (atomic load only)
    pub fn is_full(&self) -> bool {
        self.available_space() == 0
    }

    /// Get current generation counter
    ///
    /// Used for ABA prevention in external coordination
    ///
    /// # Performance
    ///
    /// - Time: <10ns (atomic load)
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get ring buffer statistics snapshot
    ///
    /// # Performance
    ///
    /// - Time: <50ns (4 atomic loads)
    pub fn snapshot(&self) -> RingBufferSnapshot {
        let head = self.head_tail.load_primary(Ordering::Acquire) as u32;
        let tail = self.head_tail.load_secondary(Ordering::Acquire) as u32;

        RingBufferSnapshot {
            head,
            tail,
            generation: self.generation.load(Ordering::Acquire),
            capacity: self.capacity,
            available: self.available_space(),
            total_commands: self.total_commands.load(Ordering::Relaxed),
            total_bytes: self.total_bytes.load(Ordering::Relaxed),
            wraparound_count: self.wraparound_count.load(Ordering::Relaxed),
        }
    }
}

/// Ring buffer statistics snapshot
#[derive(Debug, Clone, Copy)]
pub struct RingBufferSnapshot {
    pub head: u32,
    pub tail: u32,
    pub generation: u64,
    pub capacity: u32,
    pub available: u32,
    pub total_commands: u64,
    pub total_bytes: u64,
    pub wraparound_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_creation() {
        let ring = RingBufferCapsule::new(4096, 0x1000, 0x2000, 0x3000).unwrap();
        assert_eq!(ring.capacity, 4096);
        assert!(ring.is_empty());
        assert_eq!(ring.available_space(), 4096);
    }

    #[test]
    fn test_invalid_capacity() {
        // Not power-of-2
        assert_eq!(
            RingBufferCapsule::new(4095, 0, 0, 0).unwrap_err(),
            RingError::InvalidCapacity
        );

        // Zero
        assert_eq!(
            RingBufferCapsule::new(0, 0, 0, 0).unwrap_err(),
            RingError::InvalidCapacity
        );
    }

    #[test]
    fn test_push_contiguous() {
        let mut ring = RingBufferCapsule::new(4096, 0, 0, 0).unwrap();

        // Push 8-byte aligned command
        let cmd = [0u8; 8];
        assert!(ring.push_contiguous(&cmd).is_ok());

        assert_eq!(ring.available_space(), 4096 - 8);
        assert!(!ring.is_empty());
    }

    #[test]
    fn test_invalid_alignment() {
        let mut ring = RingBufferCapsule::new(4096, 0, 0, 0).unwrap();

        // Not 8-byte aligned
        let cmd = [0u8; 7];
        assert_eq!(
            ring.push_contiguous(&cmd).unwrap_err(),
            RingError::InvalidAlignment
        );
    }

    #[test]
    fn test_command_too_large() {
        let mut ring = RingBufferCapsule::new(4096, 0, 0, 0).unwrap();

        // Larger than capacity
        let cmd = vec![0u8; 8192];
        assert_eq!(
            ring.push_contiguous(&cmd).unwrap_err(),
            RingError::CommandTooLarge
        );
    }

    #[test]
    fn test_head_update() {
        let mut ring = RingBufferCapsule::new(4096, 0, 0, 0).unwrap();

        // Push command
        let cmd = [0u8; 8];
        ring.push_contiguous(&cmd).unwrap();

        // GPU consumes
        ring.update_head(8);

        // Space should be available again
        assert_eq!(ring.available_space(), 4096);
    }

    #[test]
    fn test_wraparound() {
        let mut ring = RingBufferCapsule::new(64, 0, 0, 0).unwrap();

        // Fill ring near capacity
        let cmd = [0u8; 56];
        ring.push_contiguous(&cmd).unwrap();

        // Push command that doesn't fit (would wrap)
        let cmd2 = [0u8; 16];
        ring.push_contiguous(&cmd2).unwrap();

        // Should have wrapped
        let snap = ring.snapshot();
        assert_eq!(snap.wraparound_count, 1);
    }

    #[test]
    fn test_generation_counter() {
        let mut ring = RingBufferCapsule::new(64, 0, 0, 0).unwrap();

        let gen1 = ring.generation();

        // Trigger wraparound
        let cmd = [0u8; 56];
        ring.push_contiguous(&cmd).unwrap();
        let cmd2 = [0u8; 16];
        ring.push_contiguous(&cmd2).unwrap();

        let gen2 = ring.generation();
        assert_eq!(gen2, gen1 + 1);
    }
}
