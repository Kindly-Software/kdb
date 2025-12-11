//! # Futex Syscall Handlers Module
//!
//! **UCE34 T6 Mixed: Individual syscall handlers for futex operations**
//!
//! This module provides dedicated handler functions for each futex operation,
//! organized for maintainability and clear separation of concerns.
//!
//! ## Architecture
//!
//! ```text
//! futex_syscall() ─┬─> futex_wait_handler
//!                  ├─> futex_wake_handler
//!                  ├─> futex_requeue_handler
//!                  ├─> futex_wake_op_handler
//!                  ├─> futex_waitv_handler (futex2)
//!                  └─> futex_pi_handlers (priority inheritance)
//! ```
//!
//! ## futex2 Support (Linux 5.16+)
//!
//! This module implements the futex2 syscall interface:
//! - `futex_waitv()`: Wait on multiple futexes (Win32 WaitForMultipleObjects)
//! - Variable-sized futexes: 8-bit, 16-bit, 32-bit, 64-bit
//! - NUMA-aware futex operations
//!
//! ## References
//!
//! - [futex2 kernel docs](https://docs.kernel.org/userspace-api/futex2.html)
//! - [futex_waitv gaming](https://www.collabora.com/news-and-blog/blog/2023/02/17/the-futex-waitv-syscall-gaming-on-linux/)
//! - [FUTEX2 NUMA patches](https://www.phoronix.com/news/FUTEX2-NUMA-Small-Futex)

pub mod futex;
pub mod wake_op;
pub mod waitv;
pub mod variable_size;

pub use futex::*;
pub use wake_op::*;
pub use waitv::*;
pub use variable_size::*;
