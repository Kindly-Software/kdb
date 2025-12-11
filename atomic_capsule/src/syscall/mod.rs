//! # Syscall Module - Linux Syscall Emulation for Docker glibc Compatibility
//!
//! **UCE34 Tier 4+5+6: Futex subsystem for userspace mutex/condvar synchronization**
//!
//! This module provides a complete futex implementation compatible with:
//! - Linux futex(2) syscall semantics (FUTEX_WAIT, FUTEX_WAKE, FUTEX_REQUEUE)
//! - Linux futex2 syscall (futex_waitv, variable-size futexes)
//! - glibc pthread implementation (NPTL)
//! - Docker container runtimes expecting Linux syscall ABI
//! - Wine/Proton gaming (WaitForMultipleObjects via futex_waitv)
//!
//! ## Architecture
//!
//! ```text
//! +------------------+     +---------------------+     +------------------+
//! | FutexCapsule     |---->| FutexHashTableCapsule|---->| FutexQueueCapsule|
//! | (T6 Mixed, 256B) |     | (T4 Batch, 4KB)     |     | (T5 Stream, 128B)|
//! +------------------+     +---------------------+     +------------------+
//!         |                        |                          |
//!         v                        v                          v
//!    syscall entry            bucket lookup              waiter queue
//!    (FUTEX_WAIT/WAKE)        (O(1) hash)               (FIFO ordered)
//!         |
//!         +---> handlers/  (WAKE_OP, waitv, variable-size)
//! ```
//!
//! ## Performance Targets (B32 Framework)
//!
//! | Operation      | Target   | Baseline (Linux kernel) | Notes                    |
//! |----------------|----------|-------------------------|--------------------------|
//! | FUTEX_WAIT     | <100ns   | ~200-500ns             | No context switch        |
//! | FUTEX_WAKE(1)  | <50ns    | ~100-200ns             | Single waiter            |
//! | FUTEX_WAKE(N)  | <20ns/w  | ~50-100ns/w            | Per-waiter wake cost     |
//! | FUTEX_WAKE_OP  | <100ns   | ~200ns                 | Atomic + conditional     |
//! | futex_waitv    | <200ns   | ~500ns                 | Multi-futex setup        |
//! | Hash lookup    | <10ns    | ~20-50ns               | FNV-1a + bucket array    |
//! | Queue push     | <30ns    | ~50ns                  | Lockfree MPSC            |
//! | Queue pop      | <30ns    | ~50ns                  | Lockfree SPMC            |
//!
//! ## Futex Operations Supported
//!
//! | Operation           | Linux Op Code | Status    | Notes                      |
//! |---------------------|---------------|-----------|----------------------------|
//! | FUTEX_WAIT          | 0             | Complete  | Atomic compare + block     |
//! | FUTEX_WAKE          | 1             | Complete  | Wake N waiters             |
//! | FUTEX_WAIT_BITSET   | 9             | Complete  | Selective wake via bitmask |
//! | FUTEX_WAKE_BITSET   | 10            | Complete  | Wake matching bitset       |
//! | FUTEX_REQUEUE       | 3             | Complete  | Move waiters between futexes|
//! | FUTEX_CMP_REQUEUE   | 4             | Complete  | Conditional requeue        |
//! | FUTEX_WAKE_OP       | 5             | Complete  | Atomic modify + wake       |
//! | FUTEX_LOCK_PI       | 6             | Planned   | Priority inheritance       |
//! | FUTEX_UNLOCK_PI     | 7             | Planned   | Priority inheritance       |
//!
//! ## futex2 Operations (Linux 5.16+)
//!
//! | Syscall        | Number | Status    | Notes                           |
//! |----------------|--------|-----------|--------------------------------|
//! | futex_waitv    | 449    | Complete  | Wait on multiple futexes        |
//! | futex_wake     | 454    | Complete  | Variable-size wake              |
//! | futex_wait     | 455    | Complete  | Variable-size wait              |
//! | futex_requeue  | 456    | Planned   | Variable-size requeue           |
//!
//! ## Variable-Size Futexes
//!
//! | Size | Alignment | Use Case                          |
//! |------|-----------|-----------------------------------|
//! | u8   | 1-byte    | Spinlocks, flags                  |
//! | u16  | 2-byte    | Semaphores, reader counts         |
//! | u32  | 4-byte    | Standard mutexes (glibc default)  |
//! | u64  | 8-byte    | Timestamps, combined state        |
//!
//! ## Framework Compliance
//!
//! - **UCE34**: T4 (Hash table) + T5 (Waiter queues) + T6 (Coordination)
//! - **Chaos**: 100% lockfree (no mutex/RwLock in hot path)
//! - **ASSUM**: 55 safety annotations (memory ordering, ABA prevention)
//! - **B32**: Fair baselines against Linux kernel futex
//! - **T28**: 28 tests (unit/property/integration/production)
//!
//! ## Docker glibc Compatibility
//!
//! This implementation targets glibc 2.17+ NPTL which uses:
//! - FUTEX_PRIVATE_FLAG (0x80) for process-private futexes
//! - FUTEX_CLOCK_REALTIME (0x100) for absolute timeout
//! - 32-bit futex words (4-byte aligned)
//!
//! ## References
//!
//! - [Linux futex(2) manual](https://man7.org/linux/man-pages/man2/futex.2.html)
//! - [futex2 kernel documentation](https://docs.kernel.org/userspace-api/futex2.html)
//! - [futex_waitv gaming](https://www.collabora.com/news-and-blog/blog/2023/02/17/the-futex-waitv-syscall-gaming-on-linux/)
//! - [FUTEX2 NUMA patches](https://www.phoronix.com/news/FUTEX2-NUMA-Small-Futex)
//! - [LMAX Disruptor pattern](https://lmax-exchange.github.io/disruptor/)
//! - [Basics of Futexes](https://eli.thegreenplace.net/2018/basics-of-futexes/)

pub mod error;
pub mod futex;
pub mod handlers;
pub mod hash_table;
pub mod queue;
pub mod waiter;

// Re-export main types
pub use error::{FutexError, FutexErrorKind};
pub use futex::{FutexCapsule, FutexFlags, FutexOperation, FutexResult};
pub use hash_table::FutexHashTableCapsule;
pub use queue::FutexQueueCapsule;
pub use waiter::{WaiterCapsule, WaiterId, WaiterState};

// Re-export handler types
pub use handlers::futex::FutexHandlerContext;
pub use handlers::variable_size::{FutexWord, VariableSizeFutexCapsule, VariableSizeStats};
pub use handlers::waitv::{FutexSize, FutexWaitvEntry, FutexWaitvResult, WaitvFlags, FUTEX_WAITV_MAX};
pub use handlers::wake_op::{WakeOpCmp, WakeOpParams, WakeOpType};

#[cfg(test)]
mod tests;
