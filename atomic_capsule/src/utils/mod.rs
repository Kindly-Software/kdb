//! # Utils Module - Capsule OS System Utilities
//!
//! **Framework**: UCE34 (Q1-Q34 systematic discovery)
//! **Tiers Used**: T4 (Batch), T5 (Streaming)
//! **Status**: Production Ready
//! **Chaos Compliance**: 100% lockfree, all atomic primitives
//!
//! ## Purpose
//!
//! System utilities for Capsule OS providing:
//! - Process enumeration from /proc filesystem (ps-like functionality)
//! - System resource monitoring (CPU, memory, I/O statistics)
//! - Real-time resource tracking with streaming updates
//! - Batch process listing with filtering capabilities
//!
//! ## Module Structure
//!
//! ```text
//! utils/
//! ├── mod.rs                    (This file: module exports and documentation)
//! ├── process_list.rs           (T4 Batch: Process enumeration, 2KB)
//! └── resource_monitor.rs       (T5 Streaming: Resource monitoring, 1KB)
//! ```
//!
//! ## Framework Compliance
//!
//! ### UCE34 (Q1-Q34 Systematic Discovery)
//!
//! **Q1-Q9**: Problem Analysis
//! - **Q1 (Problem)**: System visibility - process listing and resource monitoring
//! - **Q2 (Value)**: <1ms process enumeration, <100ns resource snapshot
//! - **Q3 (Scale)**: 65,536 processes max, streaming updates at 100Hz
//! - **Q4 (Context)**: Capsule OS ps/top replacement
//! - **Q5 (Success)**: Lockfree enumeration, atomic snapshots, zero allocation
//! - **Q6 (Data Shape)**: Process array (batch), resource counters (streaming)
//! - **Q7 (Core Operation)**: /proc parsing (batch), atomic counter updates (streaming)
//! - **Q8 (Alternative)**: procps (mutex-heavy), sysinfo crate (allocation-heavy)
//! - **Q9 (Transform)**: Allocation-based -> lockfree batch/streaming
//!
//! **Q10-Q12**: Tier Selection
//! - **Q10 (Tier)**: T4 Batch (process list), T5 Streaming (resource monitor)
//! - **Q11 (Rust Transform)**: AtomicU64 counters, batch array processing
//! - **Q12 (Nightly)**: Optional portable_simd for stat parsing
//!
//! **Q30-Q34**: Validation
//! - **Q30 (Validation)**: Compile-time alignment verification
//! - **Q33 (Atomic Capsule)**: All structures use AtomicU64, DualAtomicU64
//! - **Q34 (Auditability)**: Resource usage audit trail support
//!
//! ### ASSUM Safety (99.99% Target)
//!
//! - **Generation Counters**: Prevent stale reads during enumeration
//! - **Cache Alignment**: 64B/128B prevent false sharing in resource counters
//! - **Memory Ordering**: Acquire/Release for process state, Relaxed for counters
//! - **Bounded Arrays**: Fixed-size arrays prevent allocation
//!
//! ### B32 Benchmarking (Fair Baselines)
//!
//! - **Baseline (procps)**: ~5-10ms for process list on 1000 processes
//! - **Capsule (ProcessListCapsule)**: <1ms target (batch /proc parsing)
//! - **Baseline (sysinfo)**: ~1ms for resource snapshot
//! - **Capsule (ResourceMonitorCapsule)**: <100ns target (atomic loads)
//! - **Expected Speedup**: 5-10x (batch parallelism + zero allocation)
//!
//! ### T28 Testing (4-Tier Pyramid)
//!
//! - **Unit Tests (Q1-Q7)**: 10 tests (process parsing, resource counters)
//! - **Property Tests (Q8-Q14)**: 4 tests (invariants, bounds checking)
//! - **Integration Tests (Q15-Q21)**: 5 tests (full system enumeration)
//! - **Production Tests (Q22-Q28)**: 3 tests (stress tests, concurrent access)
//! - **Total**: 22 tests minimum
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │              ProcessListCapsule (T4 Batch, 2KB)                 │
//! │  ┌─────────────────────────────────────────────────────────┐   │
//! │  │ DualAtomicU64: list_state (process_count | generation)  │   │
//! │  │ [ProcessEntry; 256]: process_slots (batch buffer)       │   │
//! │  │ Operations: enumerate(), filter(), snapshot()           │   │
//! │  └─────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────┘
//!
//! ┌─────────────────────────────────────────────────────────────────┐
//! │            ResourceMonitorCapsule (T5 Streaming, 1KB)          │
//! │  ┌─────────────────────────────────────────────────────────┐   │
//! │  │ DualAtomicU64: cpu_state (usage% | timestamp)           │   │
//! │  │ DualAtomicU64: mem_state (used_kb | total_kb)           │   │
//! │  │ DualAtomicU64: io_state (read_bytes | write_bytes)      │   │
//! │  │ Operations: sample(), snapshot(), delta()               │   │
//! │  └─────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Performance Targets
//!
//! | Operation | Target | Notes |
//! |-----------|--------|-------|
//! | Process enumeration (1000 procs) | <1ms | Batch /proc parsing |
//! | Process filtering | <100us | In-memory filter |
//! | Process snapshot | <50ns | Atomic load |
//! | Resource sample | <500ns | /proc/stat + /proc/meminfo |
//! | Resource snapshot | <10ns | Atomic load |
//! | Resource delta | <20ns | Two atomic loads + subtract |
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use atomic_capsule::utils::{
//!     ProcessListCapsule, ProcessFilter, ProcessEntry,
//!     ResourceMonitorCapsule, ResourceSnapshot,
//! };
//!
//! // Process listing
//! let mut process_list = ProcessListCapsule::new();
//! process_list.enumerate()?;
//!
//! // Filter by user or name
//! let filter = ProcessFilter::by_user(1000);
//! let user_procs = process_list.filter(&filter);
//!
//! // Get specific process info
//! if let Some(entry) = process_list.get_by_pid(1234) {
//!     println!("PID {}: {} ({})", entry.pid, entry.name(), entry.state);
//! }
//!
//! // Resource monitoring
//! let mut monitor = ResourceMonitorCapsule::new();
//!
//! // Take samples
//! monitor.sample()?;
//! std::thread::sleep(Duration::from_secs(1));
//! monitor.sample()?;
//!
//! // Get current snapshot
//! let snapshot = monitor.snapshot();
//! println!("CPU: {}%, Memory: {}/{} KB",
//!     snapshot.cpu_usage_percent,
//!     snapshot.memory_used_kb,
//!     snapshot.memory_total_kb);
//!
//! // Get delta since last sample
//! let delta = monitor.delta();
//! println!("I/O: {} bytes read, {} bytes written",
//!     delta.io_read_bytes,
//!     delta.io_write_bytes);
//! ```

pub mod process_list;
pub mod resource_monitor;

// Re-export public types
pub use process_list::{
    ProcessListCapsule, ProcessEntry, ProcessState, ProcessFilter, ProcessStats,
    ProcessListError, ProcessListResult, MAX_PROCESSES, MAX_PROCESS_NAME_LEN,
};
pub use resource_monitor::{
    ResourceMonitorCapsule, ResourceSnapshot, ResourceDelta, CpuStats, MemoryStats,
    IoStats, ResourceMonitorError, ResourceMonitorResult, MAX_CPUS,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all types are exported
        let _max_procs: usize = MAX_PROCESSES;
        let _max_cpus: usize = MAX_CPUS;
        let _state = ProcessState::Running;
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_PROCESSES, 256);
        assert_eq!(MAX_PROCESS_NAME_LEN, 16);
        assert!(MAX_CPUS >= 8);
    }
}
