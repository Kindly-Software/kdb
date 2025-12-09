//! Platform-Specific Terminal Backends
//!
//! Cross-platform terminal abstraction layer following best practices from crossterm research.
//!
//! ## Design Principles
//!
//! - **Trait-Based**: `TerminalBackend` trait for platform abstraction
//! - **Send + Sync**: Thread-safe backend for concurrent access
//! - **Zero-Copy**: Minimize allocations in hot paths
//! - **Platform Detection**: Compile-time backend selection
//!
//! ## Platform Support
//!
//! - **Unix**: Linux, macOS, BSD (termios + ioctl)
//! - **Windows**: Windows 10+ (Console API + VT100)
//!
//! ## References
//!
//! - [Crossterm Platform Abstraction](https://docs.rs/crossterm/latest/crossterm/)
//! - [Terminal Library Design Patterns](https://generalistprogrammer.com/tutorials/crossterm-rust-crate-guide)

use crate::terminal::error::TerminalError;
use crate::terminal::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use core::time::Duration;

#[cfg(unix)]
pub mod unix;

#[cfg(windows)]
pub mod windows;

/// Terminal backend trait
///
/// # Design
///
/// - **Send + Sync**: Thread-safe for concurrent access
/// - **Fallible**: All operations return Result<T, TerminalError>
/// - **Composable**: Backends can be wrapped with middleware
///
/// # Platform Implementations
///
/// - Unix: `unix::UnixBackend` (termios + ioctl)
/// - Windows: `windows::WindowsBackend` (Console API + VT100)
///
/// # Examples
///
/// ```rust,ignore
/// use atomic_capsule::terminal::platform::{TerminalBackend, Event};
///
/// fn process_events<B: TerminalBackend>(backend: &mut B) -> Result<(), TerminalError> {
///     backend.enable_raw_mode()?;
///
///     loop {
///         if let Some(event) = backend.poll_event(Duration::from_millis(100))? {
///             match event {
///                 Event::Key(key) => println!("Key: {:?}", key),
///                 Event::Resize(w, h) => println!("Resize: {}x{}", w, h),
///                 _ => {}
///             }
///         }
///     }
/// }
/// ```
pub trait TerminalBackend: Send + Sync {
    /// Enable raw mode (disable line buffering, echo, signals)
    ///
    /// # Errors
    ///
    /// - `AlreadyRawMode`: Raw mode already enabled
    /// - `SetAttrFailed`: Failed to set terminal attributes
    fn enable_raw_mode(&mut self) -> Result<(), TerminalError>;

    /// Disable raw mode (restore original terminal settings)
    ///
    /// # Errors
    ///
    /// - `NotRawMode`: Not in raw mode
    /// - `SetAttrFailed`: Failed to restore terminal attributes
    fn disable_raw_mode(&mut self) -> Result<(), TerminalError>;

    /// Poll for terminal event with timeout
    ///
    /// # Returns
    ///
    /// - `Ok(Some(event))`: Event received
    /// - `Ok(None)`: Timeout (no event)
    /// - `Err(e)`: I/O error
    ///
    /// # Errors
    ///
    /// - `IoError`: I/O error during poll
    /// - `ParseError`: Invalid escape sequence
    fn poll_event(&mut self, timeout: Duration) -> Result<Option<Event>, TerminalError>;

    /// Read terminal event (blocking)
    ///
    /// # Errors
    ///
    /// - `IoError`: I/O error during read
    /// - `ParseError`: Invalid escape sequence
    fn read_event(&mut self) -> Result<Event, TerminalError>;

    /// Write bytes to terminal
    ///
    /// # Errors
    ///
    /// - `IoError`: I/O error during write
    fn write(&mut self, buf: &[u8]) -> Result<usize, TerminalError>;

    /// Flush write buffer
    ///
    /// # Errors
    ///
    /// - `IoError`: I/O error during flush
    fn flush(&mut self) -> Result<(), TerminalError>;

    /// Get terminal size (columns, rows)
    ///
    /// # Errors
    ///
    /// - `IoError`: Failed to get terminal size
    /// - `NotATty`: Not a TTY device
    fn size(&self) -> Result<(u16, u16), TerminalError>;

    /// Enter alternate screen buffer
    ///
    /// # Errors
    ///
    /// - `IoError`: I/O error during operation
    /// - `Unsupported`: Platform doesn't support alternate screen
    fn enter_alternate_screen(&mut self) -> Result<(), TerminalError>;

    /// Leave alternate screen buffer
    ///
    /// # Errors
    ///
    /// - `IoError`: I/O error during operation
    fn leave_alternate_screen(&mut self) -> Result<(), TerminalError>;

    /// Enable mouse capture
    ///
    /// # Errors
    ///
    /// - `IoError`: I/O error during operation
    /// - `Unsupported`: Platform doesn't support mouse capture
    fn enable_mouse_capture(&mut self) -> Result<(), TerminalError>;

    /// Disable mouse capture
    ///
    /// # Errors
    ///
    /// - `IoError`: I/O error during operation
    fn disable_mouse_capture(&mut self) -> Result<(), TerminalError>;

    /// Show cursor
    ///
    /// # Errors
    ///
    /// - `IoError`: I/O error during operation
    fn show_cursor(&mut self) -> Result<(), TerminalError>;

    /// Hide cursor
    ///
    /// # Errors
    ///
    /// - `IoError`: I/O error during operation
    fn hide_cursor(&mut self) -> Result<(), TerminalError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_modifiers() {
        let mods = KeyModifiers::SHIFT;
        assert!(mods.contains(KeyModifiers::SHIFT));
        assert!(!mods.contains(KeyModifiers::CONTROL));
        assert!(!mods.contains(KeyModifiers::ALT));

        let mods = KeyModifiers::SHIFT | KeyModifiers::CONTROL;
        assert!(mods.contains(KeyModifiers::SHIFT));
        assert!(mods.contains(KeyModifiers::CONTROL));
        assert!(!mods.contains(KeyModifiers::ALT));
    }

    #[test]
    fn test_event_equality() {
        let event1 = Event::Resize(80, 24);
        let event2 = Event::Resize(80, 24);
        assert_eq!(event1, event2);

        let event3 = Event::Resize(100, 30);
        assert_ne!(event1, event3);
    }

    #[test]
    fn test_key_event_equality() {
        let key1 = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let key2 = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_mouse_event_equality() {
        use crate::terminal::event::MouseButton;
        let mouse1 = MouseEvent::new(
            MouseEventKind::Down(MouseButton::Left),
            10,
            5,
            KeyModifiers::NONE,
        );
        let mouse2 = MouseEvent::new(
            MouseEventKind::Down(MouseButton::Left),
            10,
            5,
            KeyModifiers::NONE,
        );
        assert_eq!(mouse1, mouse2);
    }
}
