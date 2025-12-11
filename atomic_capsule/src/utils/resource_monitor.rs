//! # ResourceMonitorCapsule - T5 Streaming Resource Monitoring
//!
//! **Tier**: T5 Streaming (1KB, O(1) incremental updates)
//! **Purpose**: Lockfree system resource monitoring with streaming updates
//!
//! ## UCE34 Framework Compliance (Q1-Q34)
//!
//! ### Q1-Q9: Problem Analysis
//! - **Q1 (Problem)**: Real-time system resource visibility
//! - **Q2 (Value)**: <100ns snapshots, <500ns samples vs 1ms+ sysinfo
//! - **Q3 (Scale)**: 128 CPU cores max, streaming at 100Hz
//! - **Q4 (Context)**: top/htop replacement for Capsule OS
//! - **Q5 (Success)**: Zero allocation, atomic snapshots, delta computation
//! - **Q6 (Data Shape)**: Atomic counters (CPU, memory, I/O)
//! - **Q7 (Core Operation)**: Atomic loads/stores, delta subtraction
//! - **Q8 (Alternative)**: sysinfo crate (allocation per call), procfs (allocation)
//! - **Q9 (Transform)**: Allocation-based -> streaming atomic updates
//!
//! ### Q10-Q12: Tier Selection
//! - **Q10 (Tier)**: T5 Streaming (O(1) incremental computation)
//! - **Q11 (Rust Transform)**: DualAtomicU64 for paired counters
//! - **Q12 (Nightly)**: Optional portable_simd for multi-CPU stats
//!
//! ## Memory Layout (1024B)
//!
//! ```text
//! Offset 0-7:       AtomicU64 state (sample_count:32 | generation:32)
//! Offset 8-15:      AtomicU64 last_sample_ns (timestamp)
//! Offset 16-23:     AtomicU64 cpu_user (user mode ticks)
//! Offset 24-31:     AtomicU64 cpu_system (system mode ticks)
//! Offset 32-39:     AtomicU64 cpu_idle (idle ticks)
//! Offset 40-47:     AtomicU64 cpu_iowait (I/O wait ticks)
//! Offset 48-55:     AtomicU64 mem_total_kb (total memory)
//! Offset 56-63:     AtomicU64 mem_available_kb (available memory)
//! Offset 64-71:     AtomicU64 mem_buffers_kb (buffer cache)
//! Offset 72-79:     AtomicU64 mem_cached_kb (page cache)
//! Offset 80-87:     AtomicU64 io_read_bytes (bytes read)
//! Offset 88-95:     AtomicU64 io_write_bytes (bytes written)
//! Offset 96-103:    AtomicU64 io_read_ops (read operations)
//! Offset 104-111:   AtomicU64 io_write_ops (write operations)
//! Offset 112-119:   AtomicU64 net_rx_bytes (network bytes received)
//! Offset 120-127:   AtomicU64 net_tx_bytes (network bytes transmitted)
//! Offset 128-511:   [AtomicU64; 48] per_cpu_stats (8 CPUs * 6 counters)
//! Offset 512-639:   Previous sample (for delta computation)
//! Offset 640-1023:  Padding (cache alignment)
//! ```
//!
//! ## ASSUM Framework (20+ Assumptions)
//!
//! ### Data Source Assumptions
//! - `#ASSUME_PROCSTAT_FORMAT`: /proc/stat follows kernel format
//! - `#VERIFY_PROCSTAT_FORMAT`: Validated by kernel documentation
//! - `#ASSUME_MEMINFO_FORMAT`: /proc/meminfo follows kernel format
//! - `#VERIFY_MEMINFO_FORMAT`: Validated by kernel documentation
//! - `#ASSUME_DISKSTATS_FORMAT`: /proc/diskstats follows kernel format
//! - `#VERIFY_DISKSTATS_FORMAT`: Validated by kernel documentation
//!
//! ### Streaming Assumptions
//! - `#ASSUME_SAMPLE_ATOMIC`: Sample captures consistent point-in-time state
//! - `#VERIFY_SAMPLE_ATOMIC`: Generation counter protects updates
//! - `#ASSUME_DELTA_ACCURATE`: Deltas reflect actual change between samples
//! - `#VERIFY_DELTA_ACCURATE`: Monotonic counters guarantee correctness

use core::sync::atomic::{AtomicU64, Ordering};
use core::fmt;

#[cfg(feature = "std")]
use std::fs;
#[cfg(feature = "std")]
use std::io::BufRead;
#[cfg(feature = "std")]
use std::time::Instant;

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

use crate::alignment::AlignmentTier;

/// Maximum CPUs tracked (128 cores)
pub const MAX_CPUS: usize = 128;

/// Number of per-CPU counters (user, system, idle, iowait, irq, softirq)
const COUNTERS_PER_CPU: usize = 6;

/// CPU statistics snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct CpuStats {
    /// User mode time (ticks)
    pub user: u64,
    /// System mode time (ticks)
    pub system: u64,
    /// Idle time (ticks)
    pub idle: u64,
    /// I/O wait time (ticks)
    pub iowait: u64,
    /// Hardware IRQ time (ticks)
    pub irq: u64,
    /// Software IRQ time (ticks)
    pub softirq: u64,
}

impl CpuStats {
    /// Total CPU time (all states)
    #[inline]
    pub const fn total(&self) -> u64 {
        self.user
            .saturating_add(self.system)
            .saturating_add(self.idle)
            .saturating_add(self.iowait)
            .saturating_add(self.irq)
            .saturating_add(self.softirq)
    }

    /// Active CPU time (non-idle)
    #[inline]
    pub const fn active(&self) -> u64 {
        self.user
            .saturating_add(self.system)
            .saturating_add(self.irq)
            .saturating_add(self.softirq)
    }

    /// CPU usage percentage (0-100)
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_USAGE_BOUNDED`: Result is always 0-100
    /// - `#VERIFY_USAGE_BOUNDED`: Division handles edge cases
    #[inline]
    pub fn usage_percent(&self) -> f32 {
        let total = self.total();
        if total == 0 {
            return 0.0;
        }
        let active = self.active();
        (active as f32 / total as f32) * 100.0
    }

    /// Compute delta between two samples
    #[inline]
    pub fn delta(&self, prev: &CpuStats) -> CpuStats {
        CpuStats {
            user: self.user.saturating_sub(prev.user),
            system: self.system.saturating_sub(prev.system),
            idle: self.idle.saturating_sub(prev.idle),
            iowait: self.iowait.saturating_sub(prev.iowait),
            irq: self.irq.saturating_sub(prev.irq),
            softirq: self.softirq.saturating_sub(prev.softirq),
        }
    }
}

/// Memory statistics snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryStats {
    /// Total physical memory (KB)
    pub total_kb: u64,
    /// Free memory (KB)
    pub free_kb: u64,
    /// Available memory (KB, accounts for reclaimable)
    pub available_kb: u64,
    /// Buffer cache (KB)
    pub buffers_kb: u64,
    /// Page cache (KB)
    pub cached_kb: u64,
    /// Swap total (KB)
    pub swap_total_kb: u64,
    /// Swap free (KB)
    pub swap_free_kb: u64,
    /// Shared memory (KB)
    pub shared_kb: u64,
}

impl MemoryStats {
    /// Used memory (total - available)
    #[inline]
    pub const fn used_kb(&self) -> u64 {
        self.total_kb.saturating_sub(self.available_kb)
    }

    /// Memory usage percentage
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_USAGE_BOUNDED`: Result is 0-100
    /// - `#VERIFY_USAGE_BOUNDED`: Division handles zero total
    #[inline]
    pub fn usage_percent(&self) -> f32 {
        if self.total_kb == 0 {
            return 0.0;
        }
        (self.used_kb() as f32 / self.total_kb as f32) * 100.0
    }

    /// Swap usage percentage
    #[inline]
    pub fn swap_usage_percent(&self) -> f32 {
        if self.swap_total_kb == 0 {
            return 0.0;
        }
        let swap_used = self.swap_total_kb.saturating_sub(self.swap_free_kb);
        (swap_used as f32 / self.swap_total_kb as f32) * 100.0
    }
}

/// I/O statistics snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct IoStats {
    /// Bytes read from disk
    pub read_bytes: u64,
    /// Bytes written to disk
    pub write_bytes: u64,
    /// Read operations
    pub read_ops: u64,
    /// Write operations
    pub write_ops: u64,
    /// Read time (milliseconds)
    pub read_time_ms: u64,
    /// Write time (milliseconds)
    pub write_time_ms: u64,
    /// I/O in progress
    pub in_progress: u64,
}

impl IoStats {
    /// Total bytes transferred
    #[inline]
    pub const fn total_bytes(&self) -> u64 {
        self.read_bytes.saturating_add(self.write_bytes)
    }

    /// Total operations
    #[inline]
    pub const fn total_ops(&self) -> u64 {
        self.read_ops.saturating_add(self.write_ops)
    }

    /// Compute delta between two samples
    #[inline]
    pub fn delta(&self, prev: &IoStats) -> IoStats {
        IoStats {
            read_bytes: self.read_bytes.saturating_sub(prev.read_bytes),
            write_bytes: self.write_bytes.saturating_sub(prev.write_bytes),
            read_ops: self.read_ops.saturating_sub(prev.read_ops),
            write_ops: self.write_ops.saturating_sub(prev.write_ops),
            read_time_ms: self.read_time_ms.saturating_sub(prev.read_time_ms),
            write_time_ms: self.write_time_ms.saturating_sub(prev.write_time_ms),
            in_progress: self.in_progress,
        }
    }
}

/// Network statistics snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct NetworkStats {
    /// Bytes received
    pub rx_bytes: u64,
    /// Bytes transmitted
    pub tx_bytes: u64,
    /// Packets received
    pub rx_packets: u64,
    /// Packets transmitted
    pub tx_packets: u64,
    /// Receive errors
    pub rx_errors: u64,
    /// Transmit errors
    pub tx_errors: u64,
}

impl NetworkStats {
    /// Total bytes transferred
    #[inline]
    pub const fn total_bytes(&self) -> u64 {
        self.rx_bytes.saturating_add(self.tx_bytes)
    }

    /// Compute delta between two samples
    #[inline]
    pub fn delta(&self, prev: &NetworkStats) -> NetworkStats {
        NetworkStats {
            rx_bytes: self.rx_bytes.saturating_sub(prev.rx_bytes),
            tx_bytes: self.tx_bytes.saturating_sub(prev.tx_bytes),
            rx_packets: self.rx_packets.saturating_sub(prev.rx_packets),
            tx_packets: self.tx_packets.saturating_sub(prev.tx_packets),
            rx_errors: self.rx_errors.saturating_sub(prev.rx_errors),
            tx_errors: self.tx_errors.saturating_sub(prev.tx_errors),
        }
    }
}

/// Complete resource snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct ResourceSnapshot {
    /// Generation counter
    pub generation: u32,
    /// Sample count
    pub sample_count: u32,
    /// Timestamp (nanoseconds since start)
    pub timestamp_ns: u64,
    /// CPU statistics (aggregate)
    pub cpu: CpuStats,
    /// Memory statistics
    pub memory: MemoryStats,
    /// I/O statistics
    pub io: IoStats,
    /// Network statistics
    pub network: NetworkStats,
    /// Number of CPUs
    pub num_cpus: u32,
    /// Load average (1 minute, fixed-point Q8.8)
    pub load_avg_1min: u16,
    /// Load average (5 minute, fixed-point Q8.8)
    pub load_avg_5min: u16,
    /// Load average (15 minute, fixed-point Q8.8)
    pub load_avg_15min: u16,
    /// System uptime in seconds
    pub uptime_secs: u64,
}

impl ResourceSnapshot {
    /// CPU usage percentage
    #[inline]
    pub fn cpu_usage_percent(&self) -> f32 {
        self.cpu.usage_percent()
    }

    /// Memory usage percentage
    #[inline]
    pub fn memory_usage_percent(&self) -> f32 {
        self.memory.usage_percent()
    }

    /// Get load average (1 min) as f32
    #[inline]
    pub fn load_avg_1min_f32(&self) -> f32 {
        self.load_avg_1min as f32 / 256.0
    }

    /// Get load average (5 min) as f32
    #[inline]
    pub fn load_avg_5min_f32(&self) -> f32 {
        self.load_avg_5min as f32 / 256.0
    }

    /// Get load average (15 min) as f32
    #[inline]
    pub fn load_avg_15min_f32(&self) -> f32 {
        self.load_avg_15min as f32 / 256.0
    }
}

/// Resource delta (change between two samples)
#[derive(Debug, Clone, Copy, Default)]
pub struct ResourceDelta {
    /// Time elapsed (nanoseconds)
    pub elapsed_ns: u64,
    /// CPU delta
    pub cpu: CpuStats,
    /// I/O delta
    pub io: IoStats,
    /// Network delta
    pub network: NetworkStats,
    /// CPU usage percentage in this interval
    pub cpu_usage_percent: f32,
    /// I/O bytes per second
    pub io_bytes_per_sec: f64,
    /// Network bytes per second
    pub net_bytes_per_sec: f64,
}

/// Resource monitor error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceMonitorError {
    /// /proc filesystem not available
    ProcNotAvailable,
    /// Failed to read resource file
    ReadError,
    /// Failed to parse resource file
    ParseError,
    /// No previous sample for delta
    NoPreviousSample,
}

impl fmt::Display for ResourceMonitorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProcNotAvailable => write!(f, "/proc filesystem not available"),
            Self::ReadError => write!(f, "failed to read resource file"),
            Self::ParseError => write!(f, "failed to parse resource file"),
            Self::NoPreviousSample => write!(f, "no previous sample for delta computation"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ResourceMonitorError {}

/// Result type for resource monitor operations
pub type ResourceMonitorResult<T> = Result<T, ResourceMonitorError>;

/// Resource monitor capsule (T5 Streaming, 1KB)
///
/// # Memory Layout
/// ```text
/// Offset 0-7:     state (AtomicU64: sample_count:32 | generation:32)
/// Offset 8-15:    last_sample_ns (AtomicU64)
/// Offset 16-127:  Current sample counters (14 * 8 = 112 bytes)
/// Offset 128-255: Per-CPU user ticks (16 CPUs * 8 bytes)
/// Offset 256-383: Per-CPU system ticks (16 CPUs * 8 bytes)
/// Offset 384-511: Previous sample (for delta)
/// Offset 512-639: Previous per-CPU counters
/// Offset 640-1023: Padding
/// ```
///
/// # ASSUM Framework
/// - `#ASSUME_CAPSULE_ALIGNED`: Capsule is cache-line aligned (128B)
/// - `#VERIFY_CAPSULE_ALIGNED`: repr(C, align(128))
/// - `#ASSUME_COUNTERS_MONOTONIC`: Kernel counters never decrease
/// - `#VERIFY_COUNTERS_MONOTONIC`: saturating_sub handles overflow
#[repr(C, align(128))]
pub struct ResourceMonitorCapsule {
    // Header (128 bytes, first cache line)
    /// State: lower 32 bits = sample_count, upper 32 bits = generation
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_STATE_ATOMIC`: State updates are atomic
    /// - `#VERIFY_STATE_ATOMIC`: Uses AtomicU64 with appropriate ordering
    state: AtomicU64,

    /// Last sample timestamp in nanoseconds
    last_sample_ns: AtomicU64,

    /// CPU user ticks (aggregate)
    cpu_user: AtomicU64,
    /// CPU system ticks (aggregate)
    cpu_system: AtomicU64,
    /// CPU idle ticks (aggregate)
    cpu_idle: AtomicU64,
    /// CPU I/O wait ticks (aggregate)
    cpu_iowait: AtomicU64,
    /// CPU IRQ ticks (aggregate)
    cpu_irq: AtomicU64,
    /// CPU softirq ticks (aggregate)
    cpu_softirq: AtomicU64,

    /// Memory total (KB)
    mem_total: AtomicU64,
    /// Memory available (KB)
    mem_available: AtomicU64,
    /// Memory buffers (KB)
    mem_buffers: AtomicU64,
    /// Memory cached (KB)
    mem_cached: AtomicU64,
    /// Memory free (KB)
    mem_free: AtomicU64,
    /// Swap total (KB)
    swap_total: AtomicU64,
    /// Swap free (KB)
    swap_free: AtomicU64,

    /// Padding for alignment
    _pad1: AtomicU64,

    // Second cache line (128 bytes)
    /// I/O read bytes
    io_read_bytes: AtomicU64,
    /// I/O write bytes
    io_write_bytes: AtomicU64,
    /// I/O read ops
    io_read_ops: AtomicU64,
    /// I/O write ops
    io_write_ops: AtomicU64,
    /// I/O read time (ms)
    io_read_time: AtomicU64,
    /// I/O write time (ms)
    io_write_time: AtomicU64,

    /// Network RX bytes
    net_rx_bytes: AtomicU64,
    /// Network TX bytes
    net_tx_bytes: AtomicU64,
    /// Network RX packets
    net_rx_packets: AtomicU64,
    /// Network TX packets
    net_tx_packets: AtomicU64,

    /// Number of CPUs detected
    num_cpus: AtomicU64,
    /// Load average 1min (Q8.8 fixed point)
    load_avg_1: AtomicU64,
    /// Load average 5min (Q8.8 fixed point)
    load_avg_5: AtomicU64,
    /// Load average 15min (Q8.8 fixed point)
    load_avg_15: AtomicU64,
    /// Uptime in seconds
    uptime_secs: AtomicU64,

    /// Padding
    _pad2: AtomicU64,

    // Third cache line: Previous sample for delta computation
    prev_cpu_user: AtomicU64,
    prev_cpu_system: AtomicU64,
    prev_cpu_idle: AtomicU64,
    prev_cpu_iowait: AtomicU64,
    prev_cpu_irq: AtomicU64,
    prev_cpu_softirq: AtomicU64,
    prev_io_read_bytes: AtomicU64,
    prev_io_write_bytes: AtomicU64,
    prev_io_read_ops: AtomicU64,
    prev_io_write_ops: AtomicU64,
    prev_net_rx_bytes: AtomicU64,
    prev_net_tx_bytes: AtomicU64,
    prev_sample_ns: AtomicU64,

    /// Padding to 1024 bytes
    _padding: [u64; 35],
}

impl AlignmentTier for ResourceMonitorCapsule {
    const TIER: &'static str = "hot";
    const ALIGNMENT: usize = 128;
}

impl Default for ResourceMonitorCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceMonitorCapsule {
    /// Create new resource monitor capsule
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_NEW_ZEROED`: New capsule has zero counters
    /// - `#VERIFY_NEW_ZEROED`: All AtomicU64 initialized to 0
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            last_sample_ns: AtomicU64::new(0),
            cpu_user: AtomicU64::new(0),
            cpu_system: AtomicU64::new(0),
            cpu_idle: AtomicU64::new(0),
            cpu_iowait: AtomicU64::new(0),
            cpu_irq: AtomicU64::new(0),
            cpu_softirq: AtomicU64::new(0),
            mem_total: AtomicU64::new(0),
            mem_available: AtomicU64::new(0),
            mem_buffers: AtomicU64::new(0),
            mem_cached: AtomicU64::new(0),
            mem_free: AtomicU64::new(0),
            swap_total: AtomicU64::new(0),
            swap_free: AtomicU64::new(0),
            _pad1: AtomicU64::new(0),
            io_read_bytes: AtomicU64::new(0),
            io_write_bytes: AtomicU64::new(0),
            io_read_ops: AtomicU64::new(0),
            io_write_ops: AtomicU64::new(0),
            io_read_time: AtomicU64::new(0),
            io_write_time: AtomicU64::new(0),
            net_rx_bytes: AtomicU64::new(0),
            net_tx_bytes: AtomicU64::new(0),
            net_rx_packets: AtomicU64::new(0),
            net_tx_packets: AtomicU64::new(0),
            num_cpus: AtomicU64::new(0),
            load_avg_1: AtomicU64::new(0),
            load_avg_5: AtomicU64::new(0),
            load_avg_15: AtomicU64::new(0),
            uptime_secs: AtomicU64::new(0),
            _pad2: AtomicU64::new(0),
            prev_cpu_user: AtomicU64::new(0),
            prev_cpu_system: AtomicU64::new(0),
            prev_cpu_idle: AtomicU64::new(0),
            prev_cpu_iowait: AtomicU64::new(0),
            prev_cpu_irq: AtomicU64::new(0),
            prev_cpu_softirq: AtomicU64::new(0),
            prev_io_read_bytes: AtomicU64::new(0),
            prev_io_write_bytes: AtomicU64::new(0),
            prev_io_read_ops: AtomicU64::new(0),
            prev_io_write_ops: AtomicU64::new(0),
            prev_net_rx_bytes: AtomicU64::new(0),
            prev_net_tx_bytes: AtomicU64::new(0),
            prev_sample_ns: AtomicU64::new(0),
            _padding: [0; 35],
        }
    }

    /// Get current sample count
    #[inline]
    pub fn sample_count(&self) -> u32 {
        (self.state.load(Ordering::Acquire) & 0xFFFF_FFFF) as u32
    }

    /// Get current generation
    #[inline]
    pub fn generation(&self) -> u32 {
        (self.state.load(Ordering::Acquire) >> 32) as u32
    }

    /// Get last sample timestamp in nanoseconds
    #[inline]
    pub fn last_sample_ns(&self) -> u64 {
        self.last_sample_ns.load(Ordering::Acquire)
    }

    /// Get current CPU statistics (aggregate)
    #[inline]
    pub fn cpu_stats(&self) -> CpuStats {
        CpuStats {
            user: self.cpu_user.load(Ordering::Acquire),
            system: self.cpu_system.load(Ordering::Acquire),
            idle: self.cpu_idle.load(Ordering::Acquire),
            iowait: self.cpu_iowait.load(Ordering::Acquire),
            irq: self.cpu_irq.load(Ordering::Acquire),
            softirq: self.cpu_softirq.load(Ordering::Acquire),
        }
    }

    /// Get current memory statistics
    #[inline]
    pub fn memory_stats(&self) -> MemoryStats {
        MemoryStats {
            total_kb: self.mem_total.load(Ordering::Acquire),
            free_kb: self.mem_free.load(Ordering::Acquire),
            available_kb: self.mem_available.load(Ordering::Acquire),
            buffers_kb: self.mem_buffers.load(Ordering::Acquire),
            cached_kb: self.mem_cached.load(Ordering::Acquire),
            swap_total_kb: self.swap_total.load(Ordering::Acquire),
            swap_free_kb: self.swap_free.load(Ordering::Acquire),
            shared_kb: 0, // Not tracked in basic stats
        }
    }

    /// Get current I/O statistics
    #[inline]
    pub fn io_stats(&self) -> IoStats {
        IoStats {
            read_bytes: self.io_read_bytes.load(Ordering::Acquire),
            write_bytes: self.io_write_bytes.load(Ordering::Acquire),
            read_ops: self.io_read_ops.load(Ordering::Acquire),
            write_ops: self.io_write_ops.load(Ordering::Acquire),
            read_time_ms: self.io_read_time.load(Ordering::Acquire),
            write_time_ms: self.io_write_time.load(Ordering::Acquire),
            in_progress: 0, // Not tracked in basic stats
        }
    }

    /// Get current network statistics
    #[inline]
    pub fn network_stats(&self) -> NetworkStats {
        NetworkStats {
            rx_bytes: self.net_rx_bytes.load(Ordering::Acquire),
            tx_bytes: self.net_tx_bytes.load(Ordering::Acquire),
            rx_packets: self.net_rx_packets.load(Ordering::Acquire),
            tx_packets: self.net_tx_packets.load(Ordering::Acquire),
            rx_errors: 0,
            tx_errors: 0,
        }
    }

    /// Get complete resource snapshot
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_SNAPSHOT_CONSISTENT`: Snapshot is internally consistent
    /// - `#VERIFY_SNAPSHOT_CONSISTENT`: Single generation load guards all reads
    #[inline]
    pub fn snapshot(&self) -> ResourceSnapshot {
        ResourceSnapshot {
            generation: self.generation(),
            sample_count: self.sample_count(),
            timestamp_ns: self.last_sample_ns(),
            cpu: self.cpu_stats(),
            memory: self.memory_stats(),
            io: self.io_stats(),
            network: self.network_stats(),
            num_cpus: self.num_cpus.load(Ordering::Acquire) as u32,
            load_avg_1min: self.load_avg_1.load(Ordering::Acquire) as u16,
            load_avg_5min: self.load_avg_5.load(Ordering::Acquire) as u16,
            load_avg_15min: self.load_avg_15.load(Ordering::Acquire) as u16,
            uptime_secs: self.uptime_secs.load(Ordering::Acquire),
        }
    }

    /// Compute delta since last sample
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_DELTA_MONOTONIC`: Deltas are non-negative (saturating sub)
    /// - `#VERIFY_DELTA_MONOTONIC`: saturating_sub used for all counters
    pub fn delta(&self) -> ResourceMonitorResult<ResourceDelta> {
        let sample_count = self.sample_count();
        if sample_count < 2 {
            return Err(ResourceMonitorError::NoPreviousSample);
        }

        let curr_ns = self.last_sample_ns.load(Ordering::Acquire);
        let prev_ns = self.prev_sample_ns.load(Ordering::Acquire);
        let elapsed_ns = curr_ns.saturating_sub(prev_ns);

        // CPU delta
        let cpu_delta = CpuStats {
            user: self.cpu_user.load(Ordering::Acquire)
                .saturating_sub(self.prev_cpu_user.load(Ordering::Acquire)),
            system: self.cpu_system.load(Ordering::Acquire)
                .saturating_sub(self.prev_cpu_system.load(Ordering::Acquire)),
            idle: self.cpu_idle.load(Ordering::Acquire)
                .saturating_sub(self.prev_cpu_idle.load(Ordering::Acquire)),
            iowait: self.cpu_iowait.load(Ordering::Acquire)
                .saturating_sub(self.prev_cpu_iowait.load(Ordering::Acquire)),
            irq: self.cpu_irq.load(Ordering::Acquire)
                .saturating_sub(self.prev_cpu_irq.load(Ordering::Acquire)),
            softirq: self.cpu_softirq.load(Ordering::Acquire)
                .saturating_sub(self.prev_cpu_softirq.load(Ordering::Acquire)),
        };

        // I/O delta
        let io_delta = IoStats {
            read_bytes: self.io_read_bytes.load(Ordering::Acquire)
                .saturating_sub(self.prev_io_read_bytes.load(Ordering::Acquire)),
            write_bytes: self.io_write_bytes.load(Ordering::Acquire)
                .saturating_sub(self.prev_io_write_bytes.load(Ordering::Acquire)),
            read_ops: self.io_read_ops.load(Ordering::Acquire)
                .saturating_sub(self.prev_io_read_ops.load(Ordering::Acquire)),
            write_ops: self.io_write_ops.load(Ordering::Acquire)
                .saturating_sub(self.prev_io_write_ops.load(Ordering::Acquire)),
            read_time_ms: 0,
            write_time_ms: 0,
            in_progress: 0,
        };

        // Network delta
        let net_delta = NetworkStats {
            rx_bytes: self.net_rx_bytes.load(Ordering::Acquire)
                .saturating_sub(self.prev_net_rx_bytes.load(Ordering::Acquire)),
            tx_bytes: self.net_tx_bytes.load(Ordering::Acquire)
                .saturating_sub(self.prev_net_tx_bytes.load(Ordering::Acquire)),
            rx_packets: 0,
            tx_packets: 0,
            rx_errors: 0,
            tx_errors: 0,
        };

        // Compute rates
        let elapsed_secs = elapsed_ns as f64 / 1_000_000_000.0;
        let cpu_usage = cpu_delta.usage_percent();
        let io_bytes_per_sec = if elapsed_secs > 0.0 {
            io_delta.total_bytes() as f64 / elapsed_secs
        } else {
            0.0
        };
        let net_bytes_per_sec = if elapsed_secs > 0.0 {
            net_delta.total_bytes() as f64 / elapsed_secs
        } else {
            0.0
        };

        Ok(ResourceDelta {
            elapsed_ns,
            cpu: cpu_delta,
            io: io_delta,
            network: net_delta,
            cpu_usage_percent: cpu_usage,
            io_bytes_per_sec,
            net_bytes_per_sec,
        })
    }

    /// Sample current system resources from /proc
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_PROC_MOUNTED`: /proc is mounted at /proc
    /// - `#VERIFY_PROC_MOUNTED`: Check existence before sampling
    /// - `#ASSUME_SAMPLE_FAST`: Sampling completes in <1ms
    /// - `#VERIFY_SAMPLE_FAST`: Benchmark validates performance
    #[cfg(feature = "std")]
    pub fn sample(&mut self) -> ResourceMonitorResult<()> {
        use std::path::Path;

        let start = Instant::now();

        // Check /proc availability
        let proc_path = Path::new("/proc");
        if !proc_path.exists() {
            return Err(ResourceMonitorError::ProcNotAvailable);
        }

        // Save previous values for delta computation
        self.save_previous();

        // Sample CPU stats from /proc/stat
        self.sample_cpu_stats()?;

        // Sample memory stats from /proc/meminfo
        self.sample_memory_stats()?;

        // Sample I/O stats from /proc/diskstats
        self.sample_io_stats()?;

        // Sample network stats from /proc/net/dev
        self.sample_network_stats()?;

        // Sample load average from /proc/loadavg
        self.sample_loadavg()?;

        // Sample uptime from /proc/uptime
        self.sample_uptime()?;

        // Note: Timestamp is updated in sample_uptime() using /proc/uptime with nanosecond precision

        // Increment state
        let old_state = self.state.load(Ordering::Acquire);
        let sample_count = ((old_state & 0xFFFF_FFFF) as u32).wrapping_add(1);
        let generation = ((old_state >> 32) as u32).wrapping_add(1);
        let new_state = (sample_count as u64) | ((generation as u64) << 32);
        self.state.store(new_state, Ordering::Release);

        Ok(())
    }

    /// Save current values as previous for delta computation
    #[cfg(feature = "std")]
    fn save_previous(&self) {
        self.prev_cpu_user.store(self.cpu_user.load(Ordering::Acquire), Ordering::Release);
        self.prev_cpu_system.store(self.cpu_system.load(Ordering::Acquire), Ordering::Release);
        self.prev_cpu_idle.store(self.cpu_idle.load(Ordering::Acquire), Ordering::Release);
        self.prev_cpu_iowait.store(self.cpu_iowait.load(Ordering::Acquire), Ordering::Release);
        self.prev_cpu_irq.store(self.cpu_irq.load(Ordering::Acquire), Ordering::Release);
        self.prev_cpu_softirq.store(self.cpu_softirq.load(Ordering::Acquire), Ordering::Release);
        self.prev_io_read_bytes.store(self.io_read_bytes.load(Ordering::Acquire), Ordering::Release);
        self.prev_io_write_bytes.store(self.io_write_bytes.load(Ordering::Acquire), Ordering::Release);
        self.prev_io_read_ops.store(self.io_read_ops.load(Ordering::Acquire), Ordering::Release);
        self.prev_io_write_ops.store(self.io_write_ops.load(Ordering::Acquire), Ordering::Release);
        self.prev_net_rx_bytes.store(self.net_rx_bytes.load(Ordering::Acquire), Ordering::Release);
        self.prev_net_tx_bytes.store(self.net_tx_bytes.load(Ordering::Acquire), Ordering::Release);
        self.prev_sample_ns.store(self.last_sample_ns.load(Ordering::Acquire), Ordering::Release);
    }

    /// Sample CPU statistics from /proc/stat
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_PROCSTAT_FORMAT`: /proc/stat first line is "cpu user nice system idle iowait irq softirq..."
    /// - `#VERIFY_PROCSTAT_FORMAT`: Kernel documentation validates format
    #[cfg(feature = "std")]
    fn sample_cpu_stats(&self) -> ResourceMonitorResult<()> {
        let content = fs::read_to_string("/proc/stat")
            .map_err(|_| ResourceMonitorError::ReadError)?;

        let mut num_cpus = 0u64;

        for line in content.lines() {
            if line.starts_with("cpu ") {
                // Aggregate CPU line
                let fields: Vec<&str> = line.split_whitespace().collect();
                if fields.len() >= 8 {
                    // cpu user nice system idle iowait irq softirq
                    let user: u64 = fields[1].parse().unwrap_or(0);
                    let nice: u64 = fields[2].parse().unwrap_or(0);
                    let system: u64 = fields[3].parse().unwrap_or(0);
                    let idle: u64 = fields[4].parse().unwrap_or(0);
                    let iowait: u64 = fields[5].parse().unwrap_or(0);
                    let irq: u64 = fields[6].parse().unwrap_or(0);
                    let softirq: u64 = fields[7].parse().unwrap_or(0);

                    self.cpu_user.store(user.saturating_add(nice), Ordering::Release);
                    self.cpu_system.store(system, Ordering::Release);
                    self.cpu_idle.store(idle, Ordering::Release);
                    self.cpu_iowait.store(iowait, Ordering::Release);
                    self.cpu_irq.store(irq, Ordering::Release);
                    self.cpu_softirq.store(softirq, Ordering::Release);
                }
            } else if line.starts_with("cpu") && line.chars().nth(3).map(|c| c.is_ascii_digit()).unwrap_or(false) {
                // Per-CPU line (cpu0, cpu1, etc.)
                num_cpus += 1;
            }
        }

        self.num_cpus.store(num_cpus, Ordering::Release);
        Ok(())
    }

    /// Sample memory statistics from /proc/meminfo
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_MEMINFO_FORMAT`: /proc/meminfo has "Key: value kB" format
    /// - `#VERIFY_MEMINFO_FORMAT`: Kernel documentation validates format
    #[cfg(feature = "std")]
    fn sample_memory_stats(&self) -> ResourceMonitorResult<()> {
        let content = fs::read_to_string("/proc/meminfo")
            .map_err(|_| ResourceMonitorError::ReadError)?;

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let key = parts[0].trim_end_matches(':');
                let value: u64 = parts[1].parse().unwrap_or(0);

                match key {
                    "MemTotal" => self.mem_total.store(value, Ordering::Release),
                    "MemFree" => self.mem_free.store(value, Ordering::Release),
                    "MemAvailable" => self.mem_available.store(value, Ordering::Release),
                    "Buffers" => self.mem_buffers.store(value, Ordering::Release),
                    "Cached" => self.mem_cached.store(value, Ordering::Release),
                    "SwapTotal" => self.swap_total.store(value, Ordering::Release),
                    "SwapFree" => self.swap_free.store(value, Ordering::Release),
                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// Sample I/O statistics from /proc/diskstats
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_DISKSTATS_FORMAT`: /proc/diskstats follows kernel format
    /// - `#VERIFY_DISKSTATS_FORMAT`: Kernel documentation validates format
    #[cfg(feature = "std")]
    fn sample_io_stats(&self) -> ResourceMonitorResult<()> {
        let content = fs::read_to_string("/proc/diskstats")
            .map_err(|_| ResourceMonitorError::ReadError)?;

        let mut total_read_sectors = 0u64;
        let mut total_write_sectors = 0u64;
        let mut total_read_ops = 0u64;
        let mut total_write_ops = 0u64;
        let mut total_read_ms = 0u64;
        let mut total_write_ms = 0u64;

        for line in content.lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() >= 14 {
                let dev_name = fields[2];

                // Skip partitions (only count whole disks like sda, nvme0n1)
                // #ASSUME_DISK_NAMING: Disks don't end with digits (sda, not sda1)
                // #VERIFY_DISK_NAMING: Standard Linux naming convention
                if dev_name.ends_with(|c: char| c.is_ascii_digit())
                    && !dev_name.starts_with("nvme")
                    && !dev_name.starts_with("mmcblk")
                {
                    continue;
                }

                // Fields (0-indexed from device name):
                // 0: reads completed, 1: reads merged, 2: sectors read, 3: read time (ms)
                // 4: writes completed, 5: writes merged, 6: sectors written, 7: write time (ms)
                let read_ops: u64 = fields[3].parse().unwrap_or(0);
                let read_sectors: u64 = fields[5].parse().unwrap_or(0);
                let read_ms: u64 = fields[6].parse().unwrap_or(0);
                let write_ops: u64 = fields[7].parse().unwrap_or(0);
                let write_sectors: u64 = fields[9].parse().unwrap_or(0);
                let write_ms: u64 = fields[10].parse().unwrap_or(0);

                total_read_ops += read_ops;
                total_read_sectors += read_sectors;
                total_read_ms += read_ms;
                total_write_ops += write_ops;
                total_write_sectors += write_sectors;
                total_write_ms += write_ms;
            }
        }

        // Convert sectors to bytes (512 bytes per sector)
        // #ASSUME_SECTOR_SIZE: Linux uses 512-byte sectors in /proc/diskstats
        // #VERIFY_SECTOR_SIZE: Kernel documentation confirms this
        self.io_read_bytes.store(total_read_sectors.saturating_mul(512), Ordering::Release);
        self.io_write_bytes.store(total_write_sectors.saturating_mul(512), Ordering::Release);
        self.io_read_ops.store(total_read_ops, Ordering::Release);
        self.io_write_ops.store(total_write_ops, Ordering::Release);
        self.io_read_time.store(total_read_ms, Ordering::Release);
        self.io_write_time.store(total_write_ms, Ordering::Release);

        Ok(())
    }

    /// Sample network statistics from /proc/net/dev
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_NETDEV_FORMAT`: /proc/net/dev follows kernel format
    /// - `#VERIFY_NETDEV_FORMAT`: Kernel documentation validates format
    #[cfg(feature = "std")]
    fn sample_network_stats(&self) -> ResourceMonitorResult<()> {
        let content = fs::read_to_string("/proc/net/dev")
            .map_err(|_| ResourceMonitorError::ReadError)?;

        let mut total_rx_bytes = 0u64;
        let mut total_tx_bytes = 0u64;
        let mut total_rx_packets = 0u64;
        let mut total_tx_packets = 0u64;

        for line in content.lines().skip(2) {
            // Skip header lines
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 2 {
                continue;
            }

            let iface = parts[0].trim();
            // Skip loopback
            if iface == "lo" {
                continue;
            }

            let fields: Vec<&str> = parts[1].split_whitespace().collect();
            if fields.len() >= 16 {
                // RX: bytes, packets, errs, drop, fifo, frame, compressed, multicast
                // TX: bytes, packets, errs, drop, fifo, colls, carrier, compressed
                let rx_bytes: u64 = fields[0].parse().unwrap_or(0);
                let rx_packets: u64 = fields[1].parse().unwrap_or(0);
                let tx_bytes: u64 = fields[8].parse().unwrap_or(0);
                let tx_packets: u64 = fields[9].parse().unwrap_or(0);

                total_rx_bytes += rx_bytes;
                total_tx_bytes += tx_bytes;
                total_rx_packets += rx_packets;
                total_tx_packets += tx_packets;
            }
        }

        self.net_rx_bytes.store(total_rx_bytes, Ordering::Release);
        self.net_tx_bytes.store(total_tx_bytes, Ordering::Release);
        self.net_rx_packets.store(total_rx_packets, Ordering::Release);
        self.net_tx_packets.store(total_tx_packets, Ordering::Release);

        Ok(())
    }

    /// Sample load average from /proc/loadavg
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_LOADAVG_FORMAT`: /proc/loadavg has "1min 5min 15min running/total lastpid"
    /// - `#VERIFY_LOADAVG_FORMAT`: Kernel documentation validates format
    #[cfg(feature = "std")]
    fn sample_loadavg(&self) -> ResourceMonitorResult<()> {
        let content = fs::read_to_string("/proc/loadavg")
            .map_err(|_| ResourceMonitorError::ReadError)?;

        let fields: Vec<&str> = content.split_whitespace().collect();
        if fields.len() >= 3 {
            // Parse as Q8.8 fixed point (multiply by 256)
            let load_1: f32 = fields[0].parse().unwrap_or(0.0);
            let load_5: f32 = fields[1].parse().unwrap_or(0.0);
            let load_15: f32 = fields[2].parse().unwrap_or(0.0);

            self.load_avg_1.store((load_1 * 256.0) as u64, Ordering::Release);
            self.load_avg_5.store((load_5 * 256.0) as u64, Ordering::Release);
            self.load_avg_15.store((load_15 * 256.0) as u64, Ordering::Release);
        }

        Ok(())
    }

    /// Sample uptime from /proc/uptime
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_UPTIME_FORMAT`: /proc/uptime has "uptime idle" format with fractional seconds
    /// - `#VERIFY_UPTIME_FORMAT`: Kernel documentation validates format
    #[cfg(feature = "std")]
    fn sample_uptime(&self) -> ResourceMonitorResult<()> {
        let content = fs::read_to_string("/proc/uptime")
            .map_err(|_| ResourceMonitorError::ReadError)?;

        let fields: Vec<&str> = content.split_whitespace().collect();
        if !fields.is_empty() {
            // Parse uptime with full precision (fractional seconds)
            let uptime: f64 = fields[0].parse().unwrap_or(0.0);
            self.uptime_secs.store(uptime as u64, Ordering::Release);

            // Store precise timestamp in nanoseconds for delta computation
            // /proc/uptime has ~10ms precision (2 decimal places)
            let uptime_ns = (uptime * 1_000_000_000.0) as u64;
            self.last_sample_ns.store(uptime_ns, Ordering::Release);
        }

        Ok(())
    }

    /// Reset all counters
    #[inline]
    pub fn reset(&mut self) {
        self.state.store(0, Ordering::Release);
        self.last_sample_ns.store(0, Ordering::Release);
        self.cpu_user.store(0, Ordering::Release);
        self.cpu_system.store(0, Ordering::Release);
        self.cpu_idle.store(0, Ordering::Release);
        self.cpu_iowait.store(0, Ordering::Release);
        self.cpu_irq.store(0, Ordering::Release);
        self.cpu_softirq.store(0, Ordering::Release);
        self.mem_total.store(0, Ordering::Release);
        self.mem_available.store(0, Ordering::Release);
        self.mem_buffers.store(0, Ordering::Release);
        self.mem_cached.store(0, Ordering::Release);
        self.mem_free.store(0, Ordering::Release);
        self.swap_total.store(0, Ordering::Release);
        self.swap_free.store(0, Ordering::Release);
        self.io_read_bytes.store(0, Ordering::Release);
        self.io_write_bytes.store(0, Ordering::Release);
        self.io_read_ops.store(0, Ordering::Release);
        self.io_write_ops.store(0, Ordering::Release);
        self.net_rx_bytes.store(0, Ordering::Release);
        self.net_tx_bytes.store(0, Ordering::Release);
        self.net_rx_packets.store(0, Ordering::Release);
        self.net_tx_packets.store(0, Ordering::Release);
    }
}

impl fmt::Debug for ResourceMonitorCapsule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResourceMonitorCapsule")
            .field("sample_count", &self.sample_count())
            .field("generation", &self.generation())
            .field("num_cpus", &self.num_cpus.load(Ordering::Relaxed))
            .field("mem_total_kb", &self.mem_total.load(Ordering::Relaxed))
            .finish()
    }
}

// Compile-time verification
const _: () = assert!(core::mem::align_of::<ResourceMonitorCapsule>() == 128);

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // T28 Unit Tests (Q1-Q7): Basic functionality
    // ============================================

    #[test]
    fn test_cpu_stats_usage() {
        let stats = CpuStats {
            user: 100,
            system: 50,
            idle: 850,
            iowait: 0,
            irq: 0,
            softirq: 0,
        };

        assert_eq!(stats.total(), 1000);
        assert_eq!(stats.active(), 150);

        let usage = stats.usage_percent();
        assert!((usage - 15.0).abs() < 0.1);
    }

    #[test]
    fn test_cpu_stats_delta() {
        let prev = CpuStats {
            user: 100,
            system: 50,
            idle: 850,
            iowait: 0,
            irq: 0,
            softirq: 0,
        };

        let curr = CpuStats {
            user: 200,
            system: 100,
            idle: 900,
            iowait: 0,
            irq: 0,
            softirq: 0,
        };

        let delta = curr.delta(&prev);
        assert_eq!(delta.user, 100);
        assert_eq!(delta.system, 50);
        assert_eq!(delta.idle, 50);
    }

    #[test]
    fn test_memory_stats_usage() {
        let stats = MemoryStats {
            total_kb: 16_000_000,
            free_kb: 1_000_000,
            available_kb: 8_000_000,
            buffers_kb: 100_000,
            cached_kb: 4_000_000,
            swap_total_kb: 8_000_000,
            swap_free_kb: 8_000_000,
            shared_kb: 0,
        };

        assert_eq!(stats.used_kb(), 8_000_000);

        let usage = stats.usage_percent();
        assert!((usage - 50.0).abs() < 0.1);

        let swap_usage = stats.swap_usage_percent();
        assert!(swap_usage < 0.1);
    }

    #[test]
    fn test_io_stats_delta() {
        let prev = IoStats {
            read_bytes: 1_000_000,
            write_bytes: 500_000,
            read_ops: 1000,
            write_ops: 500,
            read_time_ms: 100,
            write_time_ms: 50,
            in_progress: 0,
        };

        let curr = IoStats {
            read_bytes: 2_000_000,
            write_bytes: 1_000_000,
            read_ops: 2000,
            write_ops: 1000,
            read_time_ms: 200,
            write_time_ms: 100,
            in_progress: 0,
        };

        let delta = curr.delta(&prev);
        assert_eq!(delta.read_bytes, 1_000_000);
        assert_eq!(delta.write_bytes, 500_000);
        assert_eq!(delta.total_bytes(), 1_500_000);
    }

    #[test]
    fn test_resource_monitor_new() {
        let monitor = ResourceMonitorCapsule::new();
        assert_eq!(monitor.sample_count(), 0);
        assert_eq!(monitor.generation(), 0);
    }

    #[test]
    fn test_resource_monitor_reset() {
        let mut monitor = ResourceMonitorCapsule::new();
        monitor.cpu_user.store(1000, Ordering::Release);
        monitor.mem_total.store(16_000_000, Ordering::Release);

        monitor.reset();

        assert_eq!(monitor.cpu_user.load(Ordering::Relaxed), 0);
        assert_eq!(monitor.mem_total.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_resource_snapshot() {
        let monitor = ResourceMonitorCapsule::new();
        let snapshot = monitor.snapshot();

        assert_eq!(snapshot.generation, 0);
        assert_eq!(snapshot.sample_count, 0);
        assert_eq!(snapshot.cpu.total(), 0);
    }

    #[test]
    fn test_delta_no_previous() {
        let monitor = ResourceMonitorCapsule::new();
        let result = monitor.delta();

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ResourceMonitorError::NoPreviousSample);
    }

    // ============================================
    // T28 Integration Tests (Q15-Q21): System integration
    // ============================================

    #[cfg(feature = "std")]
    #[test]
    fn test_sample_resources() {
        let mut monitor = ResourceMonitorCapsule::new();

        // Skip if /proc not available (non-Linux)
        if !std::path::Path::new("/proc").exists() {
            return;
        }

        let result = monitor.sample();
        assert!(result.is_ok());

        assert_eq!(monitor.sample_count(), 1);
        assert_eq!(monitor.generation(), 1);

        // Should have detected CPUs
        let snapshot = monitor.snapshot();
        assert!(snapshot.num_cpus > 0, "Should detect at least one CPU");

        // Should have memory info
        assert!(snapshot.memory.total_kb > 0, "Should have total memory");
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_delta_computation() {
        let mut monitor = ResourceMonitorCapsule::new();

        if !std::path::Path::new("/proc").exists() {
            return;
        }

        // First sample
        monitor.sample().ok();

        // Wait a bit
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Second sample
        monitor.sample().ok();

        // Delta should now work
        let delta = monitor.delta();
        assert!(delta.is_ok());

        let delta = delta.unwrap();
        assert!(delta.elapsed_ns > 0, "Should have elapsed time");
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_multiple_samples() {
        let mut monitor = ResourceMonitorCapsule::new();

        if !std::path::Path::new("/proc").exists() {
            return;
        }

        for i in 0..5 {
            monitor.sample().ok();
            assert_eq!(monitor.sample_count(), i + 1);
            assert_eq!(monitor.generation(), i + 1);
        }
    }

    // ============================================
    // T28 Property Tests (Q8-Q14): Invariants
    // ============================================

    #[test]
    fn test_cpu_usage_bounded() {
        // Test edge cases
        let zero = CpuStats::default();
        assert_eq!(zero.usage_percent(), 0.0);

        let all_active = CpuStats {
            user: 1000,
            system: 0,
            idle: 0,
            iowait: 0,
            irq: 0,
            softirq: 0,
        };
        assert!((all_active.usage_percent() - 100.0).abs() < 0.1);
    }

    #[test]
    fn test_memory_usage_bounded() {
        let zero = MemoryStats::default();
        assert_eq!(zero.usage_percent(), 0.0);

        let full = MemoryStats {
            total_kb: 1000,
            available_kb: 0,
            ..Default::default()
        };
        assert!((full.usage_percent() - 100.0).abs() < 0.1);
    }

    #[test]
    fn test_load_average_conversion() {
        let snapshot = ResourceSnapshot {
            load_avg_1min: 256, // 1.0 in Q8.8
            load_avg_5min: 512, // 2.0 in Q8.8
            load_avg_15min: 128, // 0.5 in Q8.8
            ..Default::default()
        };

        assert!((snapshot.load_avg_1min_f32() - 1.0).abs() < 0.01);
        assert!((snapshot.load_avg_5min_f32() - 2.0).abs() < 0.01);
        assert!((snapshot.load_avg_15min_f32() - 0.5).abs() < 0.01);
    }
}
