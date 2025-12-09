//! Unix Signal Handling Capsule
//!
//! Zero-dependency signal handling with self-pipe trick for async-signal-safe notification.
//!
//! ## Design Principles
//!
//! - **UCE34 T1 Atomic**: 128B cache-aligned, <100ns signal detection
//! - **Chaos Compliant**: 100% lockfree, signal-safe atomic operations
//! - **Self-Pipe Trick**: Signal handlers write to pipe, main loop polls pipe
//! - **POSIX Async-Signal-Safe**: Only atomic operations in signal handlers
//!
//! ## Signals Handled
//!
//! - **SIGWINCH**: Terminal resize (query new size with TIOCGWINSZ)
//! - **SIGINT**: Interrupt (Ctrl+C), usually exit gracefully
//! - **SIGTSTP**: Suspend (Ctrl+Z), restore terminal and suspend
//! - **SIGCONT**: Resume after suspend, restore raw mode
//!
//! ## Architecture
//!
//! ```text
//! Signal → Handler (async-signal-safe) → Write 1 byte to pipe
//!                                           ↓
//! Main Loop ← Poll pipe FD ← epoll/select/poll integration
//!           ↓
//! Check atomic flags → Handle signal → Drain pipe
//! ```
//!
//! ## References
//!
//! - [The Self-Pipe Trick](https://jmmv.dev/2005/03/how-to-get-window-size.html)
//! - [signal-hook crate design](https://docs.rs/signal-hook/latest/signal_hook/)
//! - [Async-signal-safety in Rust](https://www.jameselford.com/blog/working-with-signals-in-rust-pt1-whats-a-signal/)
//! - [SIGWINCH terminal resize](https://austingroupbugs.net/view.php?id=1151)
//!
//! ## Examples
//!
//! ```rust,ignore
//! use atomic_capsule::terminal::signal::SignalHandlerCapsule;
//! use std::time::Duration;
//!
//! fn main() -> Result<(), SignalError> {
//!     let handler = SignalHandlerCapsule::new()?;
//!     handler.register()?;
//!
//!     loop {
//!         // Poll pipe FD with epoll/select/poll
//!         if poll_readable(handler.pipe_fd(), Duration::from_millis(100))? {
//!             // Check which signal was received
//!             if handler.check_winch() {
//!                 let (cols, rows) = get_terminal_size()?;
//!                 println!("Terminal resized: {}×{}", cols, rows);
//!             }
//!
//!             if handler.check_int() {
//!                 println!("SIGINT received, exiting...");
//!                 break;
//!             }
//!
//!             // Drain pipe after handling signals
//!             handler.drain_pipe()?;
//!         }
//!     }
//!
//!     handler.unregister()?;
//!     Ok(())
//! }
//! ```

pub mod handler;

pub use handler::{SignalHandlerCapsule, SignalError};
