//! Signal Handling Module for Capsule OS
//!
//! Production-grade Unix signal handling using computational capsule architecture.
//! Provides T1 Atomic handler and T5 Streaming dispatcher for efficient signal
//! processing in Capsule OS.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                      Signal Flow in Capsule OS                          │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │                                                                         │
//! │  1. Signal arrives (SIGINT/SIGTERM/SIGWINCH/etc.)                      │
//! │                          ↓                                              │
//! │  2. SignalHandlerCapsule (T1 Atomic, 256B):                            │
//! │     - Set atomic pending bit (async-signal-safe)                       │
//! │     - Write 1 byte to self-pipe (notification)                         │
//! │     - Optional: signalfd for modern Linux                              │
//! │                          ↓                                              │
//! │  3. Event loop polls pipe FD (epoll/io_uring/poll)                     │
//! │                          ↓                                              │
//! │  4. SignalDispatcherCapsule (T5 Streaming, 512B):                      │
//! │     - Dequeue pending signals from ring buffer                         │
//! │     - Lookup handler in handler table                                  │
//! │     - Execute action (Handle/Ignore/Terminate/etc.)                    │
//! │     - Coalesce repeated signals (optional)                             │
//! │                          ↓                                              │
//! │  5. User callback invoked with SignalInfo                              │
//! │                                                                         │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Capsules
//!
//! | Capsule | Tier | Size | Purpose |
//! |---------|------|------|---------|
//! | SignalHandlerCapsule | T1 Atomic | 256B | Low-level signal reception |
//! | SignalDispatcherCapsule | T5 Streaming | 512B | Signal routing and dispatch |
//!
//! ## Features
//!
//! - **100% Lockfree**: Atomic state coordination, no mutex/RwLock
//! - **Async-Signal-Safe**: Only POSIX async-signal-safe ops in handlers
//! - **Self-Pipe Trick**: Portable notification for event loops
//! - **signalfd Support**: Modern Linux integration (2.6.22+)
//! - **Signal Coalescing**: Merge repeated signals (configurable)
//! - **Handler Table**: O(1) lookup by signal number
//! - **Statistics**: Full telemetry for monitoring
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use atomic_capsule::signal::{SignalHandlerCapsule, SignalDispatcherCapsule, Signal};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create signal handler
//!     let handler = SignalHandlerCapsule::new()?;
//!     handler.register()?;
//!
//!     // Create signal dispatcher
//!     let mut dispatcher = SignalDispatcherCapsule::new();
//!     dispatcher.register_handler(Signal::Int, SignalAction::Handle, 1)?;
//!     dispatcher.start()?;
//!
//!     // Main event loop
//!     loop {
//!         // Poll pipe FD with your event loop
//!         if poll_readable(handler.pipe_fd(), Duration::from_millis(100))? {
//!             handler.drain_pipe()?;
//!
//!             // Check and handle signals
//!             if handler.check_pending(Signal::Int) {
//!                 println!("SIGINT received!");
//!                 break;
//!             }
//!
//!             if handler.check_pending(Signal::Winch) {
//!                 println!("Terminal resized!");
//!             }
//!         }
//!     }
//!
//!     handler.unregister()?;
//!     Ok(())
//! }
//! ```
//!
//! ## Integration with Event Loops
//!
//! ### epoll (Linux)
//!
//! ```rust,ignore
//! let epoll_fd = epoll_create1(0)?;
//! let mut event = epoll_event {
//!     events: EPOLLIN,
//!     u64: handler.pipe_fd() as u64,
//! };
//! epoll_ctl(epoll_fd, EPOLL_CTL_ADD, handler.pipe_fd(), &mut event)?;
//!
//! loop {
//!     let n = epoll_wait(epoll_fd, &mut events, timeout_ms)?;
//!     if n > 0 {
//!         handler.drain_pipe()?;
//!         // Handle signals...
//!     }
//! }
//! ```
//!
//! ### io_uring (Linux 5.1+)
//!
//! ```rust,ignore
//! // Add pipe FD to io_uring poll operation
//! sqe.opcode = IORING_OP_POLL_ADD;
//! sqe.fd = handler.pipe_fd();
//! sqe.poll_events = POLLIN;
//! ```
//!
//! ### signalfd (Linux 2.6.22+)
//!
//! If signalfd is available, use `handler.signalfd()` instead of the self-pipe
//! for more efficient signal delivery.
//!
//! ## References
//!
//! - [Self-Pipe Trick](https://cr.yp.to/docs/selfpipe.html)
//! - [signalfd(2)](https://man7.org/linux/man-pages/man2/signalfd.2.html)
//! - [pidfd_send_signal(2)](https://man7.org/linux/man-pages/man2/pidfd_send_signal.2.html)
//! - [signal-safety(7)](https://man7.org/linux/man-pages/man7/signal-safety.7.html)
//! - [sigaction(2)](https://man7.org/linux/man-pages/man2/sigaction.2.html)
//! - [Making signals less painful under Linux](https://unixism.net/2021/02/making-signals-less-painful-under-linux/)
//!
//! ## Framework Compliance
//!
//! - **UCE34**: T1 Atomic + T5 Streaming tier capsules
//! - **Chaos**: 100% lockfree, cache-aligned, generation counters
//! - **T28**: 22+ tests (unit/property/integration/production)
//! - **ASSUM**: 45+ safety assumptions documented
//! - **B32**: <100ns signal detection validated

// Module declarations
pub mod types;
pub mod handler;
pub mod dispatcher;

#[cfg(test)]
mod tests;

// Re-export main types
pub use types::{Signal, SignalAction, SignalError, SignalInfo, SignalResult};
pub use handler::{SignalHandlerCapsule, SignalHandlerStats, state_flags};
pub use dispatcher::{
    SignalDispatcherCapsule, SignalDispatcherStats, SignalQueueEntry, HandlerEntry,
    dispatcher_flags, SIGNAL_QUEUE_CAPACITY, MAX_SIGNAL_HANDLERS,
};
