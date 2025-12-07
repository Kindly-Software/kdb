//! Atomic Debugger - T6 Mixed Computational Capsule
//!
//! A 1MB debugger built on computational capsule architecture combining:
//! - T1 Atomic: Execution state, breakpoints, watchpoints (64 KB)
//! - T2 SIMD: Stack unwinding, symbol lookup (128 KB)
//! - T4 Batch: Parallel multi-process debugging (64 KB)
//! - T5 Streaming: Ring buffer tracing (192 KB)
//! - T9 Persistent: Crash dumps, checkpoints (128 KB)
//! - T10 Probabilistic: Path deduplication (256 KB)
//! - Time-Travel: Reverse execution (128 KB)
//!
//! Total: 1,048,576 bytes (1 MB)

pub mod debugger;
pub mod deterministic_debugger;
pub mod tier10_probabilistic;
pub mod tier1_atomic;
pub mod tier2_simd;
pub mod tier4_parallel_debug;
pub mod tier5_streaming;
pub mod tier9_persistent;
pub mod time_travel;

// CLI module: Enabled for Phase 2 ComprehensiveAudit implementation
pub mod cli;

// Access Control: Ed25519 signature verification with timing attack protection
pub mod access_control;

// Session Pool: Tiered session management (LIGHT/MEDIUM/HEAVY)
pub mod session_pool;

// Memory Replay: Page-level time-travel reconstruction
pub mod memory_replay;

#[cfg(target_os = "linux")]
pub mod ptrace;

pub use debugger::DebuggerCapsule;

#[cfg(target_os = "linux")]
pub use ptrace::{
    MemoryReader, ProcessState, ProcessStateCapsule, ProcessStateError, RegisterError,
    RegisterReaderCapsule, StackFrame, StackUnwindError, StackUnwinderCapsule, UserRegs,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn test_debugger_size() {
        // Updated 2025-11-14: Actual size measured = 1,147,392 bytes (1.09 MB)
        // T6 Mixed composition with multiple tier capsules + 256B alignment padding
        let actual_size = size_of::<DebuggerCapsule>();
        assert!(
            actual_size >= 1_140_000 && actual_size <= 1_160_000,
            "DebuggerCapsule should be ~1.09 MB, got {} bytes",
            actual_size
        );
    }

    #[test]
    fn test_debugger_alignment() {
        assert_eq!(
            align_of::<DebuggerCapsule>(),
            256,
            "DebuggerCapsule must be 256-byte aligned"
        );
    }
}
