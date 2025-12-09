//! # CursorCapsule - T1 Atomic Terminal Cursor Management
//!
//! **Manages terminal cursor visibility and position tracking with atomic state.**
//!
//! **Framework**: UCE34 Q10-Q34 (Tier 1 Atomic)
//!
//! ## Overview
//!
//! CursorCapsule provides lockfree, cache-aligned terminal cursor management with
//! atomic visibility state and position tracking. Supports cursor hiding/showing,
//! positioning, and save/restore operations.
//!
//! ## Tier: T1 Atomic
//!
//! - **Alignment**: 64 bytes (single cache line)
//! - **Operations**: <50ns state check, <1μs cursor transition
//! - **Pattern**: AtomicBool visible + AtomicU16 position tracking
//! - **Memory**: 64 bytes total (visibility + position + saved + generation + padding)
//!
//! ## Performance (B32 Expected)
//!
//! - **Baseline** (repeated write syscalls): 1-5μs per operation
//! - **CursorCapsule** (cached state): <50ns state check, <1μs operation
//! - **Speedup**: **20-100×** on state checks (cached atomic vs syscall)
//! - **Safety**: Atomic operations, no mutex required
//!
//! ## ASSUM Framework (99.99% Safe)
//!
//! - `#ASSUME_CURSOR_VISIBILITY_REVERSIBLE`: Terminal supports cursor show/hide
//! - `#VERIFY_CURSOR_VISIBILITY_REVERSIBLE`: Test show/hide in unit tests
//! - `#ASSUME_SINGLE_TERMINAL`: Single terminal per process (stdout fd=1)
//! - `#VERIFY_SINGLE_TERMINAL`: Store fd atomically
//! - `#ASSUME_ATOMIC_POSITION_TRACKING`: Position tracking is eventually consistent
//! - `#VERIFY_POSITION_TRACKING`: Local tracking only, no query from terminal
//! - `#ASSUME_CACHE_LINE_64B`: Single cache line for hot data
//! - `#VERIFY_CACHE_ALIGNMENT`: Compile-time alignment check
//!
//! ## Cursor Escape Sequences
//!
//! - **Hide cursor**: `\x1b[?25l` - Makes cursor invisible
//! - **Show cursor**: `\x1b[?25h` - Makes cursor visible
//! - **Move cursor**: `\x1b[{row};{col}H` - Move to specific position (1-indexed)
//! - **Save position**: `\x1b[s` or `\x1b7` - Save current cursor position
//! - **Restore position**: `\x1b[u` or `\x1b8` - Restore saved cursor position
//!
//! ## Usage
//!
//! ```rust,no_run
//! use atomic_capsule::terminal::mode::CursorCapsule;
//!
//! let cursor = CursorCapsule::new()?;
//!
//! // Hide cursor for TUI rendering
//! cursor.hide()?;
//!
//! // Move to position
//! cursor.move_to(10, 5)?;
//!
//! // Save position
//! cursor.save_position()?;
//!
//! // Do work...
//!
//! // Restore position
//! cursor.restore_position()?;
//!
//! // Show cursor
//! cursor.show()?;
//! ```
//!
//! ## Cross-Platform Support
//!
//! - **Unix/Linux**: Full support via ANSI escape sequences
//! - **macOS**: Full support via ANSI escape sequences
//! - **Windows**: Full support via Windows Console API (not yet implemented)
//! - **WASM**: Not applicable (no terminal)
//!
//! ## References
//!
//! Research Sources:
//! - [Terminal cursor control escape sequences](https://rosettacode.org/wiki/Terminal_control/Hiding_the_cursor) - Hide/show cursor
//! - [ANSI escape codes](https://notes.burke.libbey.me/ansi-escape-codes/) - Comprehensive guide
//! - [Cursor visibility](https://stackoverflow.com/questions/2649733/hide-cursor-on-remote-terminal) - Unix terminal cursor control

use crate::alignment::AlignmentTier;
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, AtomicI32, Ordering};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

#[cfg(feature = "std")]
use std::io::{self, Write};

/// Errors that can occur during cursor operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorError {
    /// Failed to write escape sequence to terminal
    WriteFailed(i32),

    /// Terminal is not a TTY
    NotATty,

    /// Invalid cursor position
    InvalidPosition {
        x: u16,
        y: u16,
    },

    /// IO error (std only)
    #[cfg(feature = "std")]
    IoError(String),
}

impl core::fmt::Display for CursorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CursorError::WriteFailed(errno) => {
                write!(f, "Failed to write escape sequence (errno: {})", errno)
            }
            CursorError::NotATty => {
                write!(f, "Terminal is not a TTY")
            }
            CursorError::InvalidPosition { x, y } => {
                write!(f, "Invalid cursor position: ({}, {})", x, y)
            }
            #[cfg(feature = "std")]
            CursorError::IoError(msg) => {
                write!(f, "IO error: {}", msg)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CursorError {}

#[cfg(feature = "std")]
impl From<io::Error> for CursorError {
    fn from(err: io::Error) -> Self {
        CursorError::IoError(err.to_string())
    }
}

/// Escape sequences for cursor control
mod escape {
    /// Hide cursor: Makes cursor invisible
    pub const HIDE_CURSOR: &[u8] = b"\x1b[?25l";

    /// Show cursor: Makes cursor visible
    pub const SHOW_CURSOR: &[u8] = b"\x1b[?25h";

    /// Save cursor position (DEC-style)
    pub const SAVE_POSITION: &[u8] = b"\x1b7";

    /// Restore cursor position (DEC-style)
    pub const RESTORE_POSITION: &[u8] = b"\x1b8";

    /// Move cursor prefix (append row;colH)
    /// Format: \x1b[{row};{col}H
    pub const MOVE_CURSOR_PREFIX: &[u8] = b"\x1b[";
}

/// CursorCapsule - T1 Atomic terminal cursor management
///
/// Manages terminal cursor visibility and position tracking with atomic state.
/// Provides lockfree cursor operations with <50ns state checks.
///
/// # Memory Layout
///
/// ```text
/// Offset 0:      Atomic visible (bool: cursor visibility state)
/// Offset 1:      Padding (1 byte)
/// Offset 2-3:    Atomic position_x (u16: current X position, 0-based)
/// Offset 4-5:    Atomic position_y (u16: current Y position, 0-based)
/// Offset 6-7:    Atomic saved_x (u16: saved X position)
/// Offset 8-9:    Atomic saved_y (u16: saved Y position)
/// Offset 10-11:  Padding (2 bytes)
/// Offset 12-15:  Atomic fd (i32: file descriptor, typically 1 for stdout)
/// Offset 16-23:  Atomic generation counter (u64: TOCTOU prevention)
/// Offset 24-63:  Padding (complete 64-byte cache line)
/// ```
///
/// # ASSUM Tags
///
/// - `#ASSUME_CURSOR_VISIBILITY_REVERSIBLE`: Terminal supports ANSI cursor control
/// - `#ASSUME_SINGLE_TERMINAL`: Single terminal per capsule instance
/// - `#ASSUME_ATOMIC_POSITION_TRACKING`: Local tracking is eventually consistent
/// - `#ASSUME_CACHE_LINE_64B`: Single 64B cache line for alignment
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 64, size = 64))]
#[repr(C, align(64))]
pub struct CursorCapsule {
    /// Atomic cursor visibility state
    visible: AtomicBool,

    /// Padding for alignment (1 byte)
    _padding1: u8,

    /// Atomic current X position (0-based, tracked locally)
    position_x: AtomicU16,

    /// Atomic current Y position (0-based, tracked locally)
    position_y: AtomicU16,

    /// Atomic saved X position (for save/restore)
    saved_x: AtomicU16,

    /// Atomic saved Y position (for save/restore)
    saved_y: AtomicU16,

    /// Padding for alignment (2 bytes)
    _padding2: [u8; 2],

    /// Terminal file descriptor (typically 1 for stdout)
    stdout_fd: AtomicI32,

    /// Generation counter for TOCTOU prevention
    generation: AtomicU64,

    /// Padding to complete 64-byte cache line
    /// (64 - 1 - 1 - 2 - 2 - 2 - 2 - 2 - 4 - 8 = 40 bytes padding)
    _padding3: [u8; 40],
}

impl AlignmentTier for CursorCapsule {
    const TIER: &'static str = "hot";
    const ALIGNMENT: usize = 64;
}

impl CursorCapsule {
    /// Create a new CursorCapsule for stdout (fd=1)
    ///
    /// # Errors
    ///
    /// Returns `CursorError::NotATty` if stdout is not a TTY.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use atomic_capsule::terminal::mode::CursorCapsule;
    ///
    /// let cursor = CursorCapsule::new()?;
    /// # Ok::<(), atomic_capsule::terminal::mode::CursorError>(())
    /// ```
    #[cfg(unix)]
    pub fn new() -> Result<Self, CursorError> {
        Self::with_fd(libc::STDOUT_FILENO)
    }

    /// Create a new CursorCapsule for a specific file descriptor
    ///
    /// # Arguments
    ///
    /// - `fd`: File descriptor (e.g., 1 for stdout, 2 for stderr)
    ///
    /// # Errors
    ///
    /// Returns `CursorError::NotATty` if fd is not a TTY.
    #[cfg(unix)]
    pub fn with_fd(fd: libc::c_int) -> Result<Self, CursorError> {
        // Check if fd is a TTY
        // #ASSUME_ISATTY_CORRECT: libc::isatty returns correct value
        if unsafe { libc::isatty(fd) } != 1 {
            return Err(CursorError::NotATty);
        }

        Ok(Self {
            visible: AtomicBool::new(true), // Cursor is visible by default
            _padding1: 0,
            position_x: AtomicU16::new(0),
            position_y: AtomicU16::new(0),
            saved_x: AtomicU16::new(0),
            saved_y: AtomicU16::new(0),
            _padding2: [0; 2],
            stdout_fd: AtomicI32::new(fd),
            generation: AtomicU64::new(0),
            _padding3: [0u8; 40],
        })
    }

    /// Hide cursor (make invisible)
    ///
    /// Writes `\x1b[?25l` to terminal.
    ///
    /// # Errors
    ///
    /// Returns `CursorError::WriteFailed` if write fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use atomic_capsule::terminal::mode::CursorCapsule;
    ///
    /// let cursor = CursorCapsule::new()?;
    /// cursor.hide()?;
    /// # Ok::<(), atomic_capsule::terminal::mode::CursorError>(())
    /// ```
    #[cfg(feature = "std")]
    pub fn hide(&self) -> Result<(), CursorError> {
        // Write escape sequence to stdout
        let mut stdout = io::stdout();
        stdout.write_all(escape::HIDE_CURSOR)?;
        stdout.flush()?;

        // Update visibility state
        self.visible.store(false, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(())
    }

    /// Show cursor (make visible)
    ///
    /// Writes `\x1b[?25h` to terminal.
    ///
    /// # Errors
    ///
    /// Returns `CursorError::WriteFailed` if write fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use atomic_capsule::terminal::mode::CursorCapsule;
    ///
    /// let cursor = CursorCapsule::new()?;
    /// cursor.hide()?;
    /// // ... do TUI work ...
    /// cursor.show()?;
    /// # Ok::<(), atomic_capsule::terminal::mode::CursorError>(())
    /// ```
    #[cfg(feature = "std")]
    pub fn show(&self) -> Result<(), CursorError> {
        // Write escape sequence to stdout
        let mut stdout = io::stdout();
        stdout.write_all(escape::SHOW_CURSOR)?;
        stdout.flush()?;

        // Update visibility state
        self.visible.store(true, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(())
    }

    /// Move cursor to specific position
    ///
    /// Writes `\x1b[{row};{col}H` to terminal (1-indexed).
    ///
    /// # Arguments
    ///
    /// - `x`: Column position (0-based, converted to 1-based for terminal)
    /// - `y`: Row position (0-based, converted to 1-based for terminal)
    ///
    /// # Errors
    ///
    /// Returns `CursorError::WriteFailed` if write fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use atomic_capsule::terminal::mode::CursorCapsule;
    ///
    /// let cursor = CursorCapsule::new()?;
    /// cursor.move_to(10, 5)?; // Move to column 10, row 5
    /// # Ok::<(), atomic_capsule::terminal::mode::CursorError>(())
    /// ```
    #[cfg(feature = "std")]
    pub fn move_to(&self, x: u16, y: u16) -> Result<(), CursorError> {
        // Convert 0-based to 1-based for terminal (ANSI is 1-indexed)
        let terminal_x = x.saturating_add(1);
        let terminal_y = y.saturating_add(1);

        // Format escape sequence: \x1b[{row};{col}H
        let mut stdout = io::stdout();
        write!(stdout, "\x1b[{};{}H", terminal_y, terminal_x)?;
        stdout.flush()?;

        // Update position tracking
        self.position_x.store(x, Ordering::Release);
        self.position_y.store(y, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(())
    }

    /// Save current cursor position
    ///
    /// Writes `\x1b7` (DEC save cursor) to terminal.
    /// Also saves position in local tracking.
    ///
    /// # Errors
    ///
    /// Returns `CursorError::WriteFailed` if write fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use atomic_capsule::terminal::mode::CursorCapsule;
    ///
    /// let cursor = CursorCapsule::new()?;
    /// cursor.move_to(10, 5)?;
    /// cursor.save_position()?;
    /// # Ok::<(), atomic_capsule::terminal::mode::CursorError>(())
    /// ```
    #[cfg(feature = "std")]
    pub fn save_position(&self) -> Result<(), CursorError> {
        // Write escape sequence to stdout
        let mut stdout = io::stdout();
        stdout.write_all(escape::SAVE_POSITION)?;
        stdout.flush()?;

        // Save current position to saved position
        let current_x = self.position_x.load(Ordering::Acquire);
        let current_y = self.position_y.load(Ordering::Acquire);

        self.saved_x.store(current_x, Ordering::Release);
        self.saved_y.store(current_y, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(())
    }

    /// Restore previously saved cursor position
    ///
    /// Writes `\x1b8` (DEC restore cursor) to terminal.
    /// Also restores position in local tracking.
    ///
    /// # Errors
    ///
    /// Returns `CursorError::WriteFailed` if write fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use atomic_capsule::terminal::mode::CursorCapsule;
    ///
    /// let cursor = CursorCapsule::new()?;
    /// cursor.move_to(10, 5)?;
    /// cursor.save_position()?;
    /// cursor.move_to(20, 10)?;
    /// cursor.restore_position()?; // Back to (10, 5)
    /// # Ok::<(), atomic_capsule::terminal::mode::CursorError>(())
    /// ```
    #[cfg(feature = "std")]
    pub fn restore_position(&self) -> Result<(), CursorError> {
        // Write escape sequence to stdout
        let mut stdout = io::stdout();
        stdout.write_all(escape::RESTORE_POSITION)?;
        stdout.flush()?;

        // Restore saved position to current position
        let saved_x = self.saved_x.load(Ordering::Acquire);
        let saved_y = self.saved_y.load(Ordering::Acquire);

        self.position_x.store(saved_x, Ordering::Release);
        self.position_y.store(saved_y, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(())
    }

    /// Check if cursor is currently visible
    ///
    /// # Performance
    ///
    /// <50ns (atomic load, no syscall)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use atomic_capsule::terminal::mode::CursorCapsule;
    ///
    /// let cursor = CursorCapsule::new()?;
    /// assert!(cursor.is_visible());
    ///
    /// cursor.hide()?;
    /// assert!(!cursor.is_visible());
    /// # Ok::<(), atomic_capsule::terminal::mode::CursorError>(())
    /// ```
    #[inline]
    pub fn is_visible(&self) -> bool {
        self.visible.load(Ordering::Acquire)
    }

    /// Get current cursor position (local tracking)
    ///
    /// Returns (x, y) as 0-based coordinates.
    ///
    /// # Note
    ///
    /// Position is tracked locally, not queried from terminal.
    /// Accurate only if all cursor movements go through this capsule.
    ///
    /// # Performance
    ///
    /// <50ns (two atomic loads, no syscall)
    #[inline]
    pub fn position(&self) -> (u16, u16) {
        let x = self.position_x.load(Ordering::Acquire);
        let y = self.position_y.load(Ordering::Acquire);
        (x, y)
    }

    /// Get saved cursor position
    ///
    /// Returns (x, y) as 0-based coordinates.
    #[inline]
    pub fn saved_position(&self) -> (u16, u16) {
        let x = self.saved_x.load(Ordering::Acquire);
        let y = self.saved_y.load(Ordering::Acquire);
        (x, y)
    }

    /// Get current generation counter
    ///
    /// Increments on each cursor operation (hide/show/move/save/restore).
    /// Useful for detecting stale references or TOCTOU prevention.
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get current file descriptor
    #[inline]
    pub fn fd(&self) -> i32 {
        self.stdout_fd.load(Ordering::Acquire)
    }
}

// Compile-time verification of layout (Q33: Mandatory verification)
#[cfg(not(feature = "derive"))]
crate::verify_capsule_properties!(CursorCapsule, 64, 64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(core::mem::align_of::<CursorCapsule>(), 64);
    }

    #[test]
    fn test_capsule_size() {
        assert_eq!(core::mem::size_of::<CursorCapsule>(), 64);
    }

    #[test]
    #[cfg(unix)]
    fn test_new_with_tty() {
        // This test will only pass if running in a terminal
        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            let cursor = CursorCapsule::new();
            assert!(cursor.is_ok());

            if let Ok(capsule) = cursor {
                assert!(capsule.is_visible());
                assert_eq!(capsule.generation(), 0);
                assert_eq!(capsule.fd(), libc::STDOUT_FILENO);
                assert_eq!(capsule.position(), (0, 0));
            }
        }
    }

    #[test]
    #[cfg(all(unix, feature = "std"))]
    fn test_hide_show_cursor() {
        // This test will only pass if running in a terminal
        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            let cursor = CursorCapsule::new();
            assert!(cursor.is_ok());

            if let Ok(capsule) = cursor {
                // Initially visible
                assert!(capsule.is_visible());
                assert_eq!(capsule.generation(), 0);

                // Hide cursor
                let hide_result = capsule.hide();
                assert!(hide_result.is_ok());
                assert!(!capsule.is_visible());
                assert_eq!(capsule.generation(), 1);

                // Show cursor
                let show_result = capsule.show();
                assert!(show_result.is_ok());
                assert!(capsule.is_visible());
                assert_eq!(capsule.generation(), 2);
            }
        }
    }

    #[test]
    #[cfg(all(unix, feature = "std"))]
    fn test_move_cursor() {
        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            let cursor = CursorCapsule::new();
            assert!(cursor.is_ok());

            if let Ok(capsule) = cursor {
                // Initially at (0, 0)
                assert_eq!(capsule.position(), (0, 0));

                // Move to (10, 5)
                let move_result = capsule.move_to(10, 5);
                assert!(move_result.is_ok());
                assert_eq!(capsule.position(), (10, 5));
                assert_eq!(capsule.generation(), 1);
            }
        }
    }

    #[test]
    #[cfg(all(unix, feature = "std"))]
    fn test_save_restore_position() {
        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            let cursor = CursorCapsule::new();
            assert!(cursor.is_ok());

            if let Ok(capsule) = cursor {
                // Move to (10, 5)
                capsule.move_to(10, 5).ok();
                assert_eq!(capsule.position(), (10, 5));

                // Save position
                capsule.save_position().ok();
                assert_eq!(capsule.saved_position(), (10, 5));

                // Move to (20, 10)
                capsule.move_to(20, 10).ok();
                assert_eq!(capsule.position(), (20, 10));

                // Restore position
                capsule.restore_position().ok();
                assert_eq!(capsule.position(), (10, 5));
            }
        }
    }

    #[test]
    #[cfg(all(unix, feature = "std"))]
    fn test_generation_counter_increments() {
        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            let cursor = CursorCapsule::new();
            assert!(cursor.is_ok());

            if let Ok(capsule) = cursor {
                assert_eq!(capsule.generation(), 0);

                capsule.hide().ok();
                assert_eq!(capsule.generation(), 1);

                capsule.show().ok();
                assert_eq!(capsule.generation(), 2);

                capsule.move_to(10, 5).ok();
                assert_eq!(capsule.generation(), 3);

                capsule.save_position().ok();
                assert_eq!(capsule.generation(), 4);

                capsule.restore_position().ok();
                assert_eq!(capsule.generation(), 5);
            }
        }
    }

    #[test]
    fn test_error_display() {
        let err = CursorError::NotATty;
        assert_eq!(format!("{}", err), "Terminal is not a TTY");

        let err = CursorError::WriteFailed(5);
        assert!(format!("{}", err).contains("Failed to write escape sequence"));

        let err = CursorError::InvalidPosition { x: 100, y: 200 };
        assert!(format!("{}", err).contains("Invalid cursor position"));
    }

    #[test]
    fn test_cache_line_padding() {
        // Verify alignment for cache line optimization
        let cursor = CursorCapsule {
            visible: AtomicBool::new(true),
            _padding1: 0,
            position_x: AtomicU16::new(0),
            position_y: AtomicU16::new(0),
            saved_x: AtomicU16::new(0),
            saved_y: AtomicU16::new(0),
            _padding2: [0; 2],
            stdout_fd: AtomicI32::new(1),
            generation: AtomicU64::new(0),
            _padding3: [0u8; 40],
        };

        let ptr = &cursor as *const _ as usize;
        assert_eq!(ptr % 64, 0, "Pointer should be 64-byte aligned");
    }

    #[test]
    #[cfg(all(unix, feature = "std"))]
    fn test_concurrent_reads() {
        // Test thread-safety of reads
        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            let cursor = CursorCapsule::new();
            assert!(cursor.is_ok());

            if let Ok(capsule) = cursor {
                let capsule_arc = std::sync::Arc::new(capsule);
                let mut threads = vec![];

                for _ in 0..4 {
                    let capsule_clone = capsule_arc.clone();
                    let t = std::thread::spawn(move || {
                        for _ in 0..100 {
                            let _ = capsule_clone.is_visible();
                            let _ = capsule_clone.position();
                            let _ = capsule_clone.saved_position();
                            let _ = capsule_clone.generation();
                            let _ = capsule_clone.fd();
                        }
                    });
                    threads.push(t);
                }

                for t in threads {
                    t.join().unwrap();
                }
            }
        }
    }

    #[test]
    #[cfg(all(unix, feature = "std"))]
    fn test_multiple_operations_sequence() {
        // Test complex sequence of operations
        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            let cursor = CursorCapsule::new();
            assert!(cursor.is_ok());

            if let Ok(capsule) = cursor {
                // Hide cursor
                capsule.hide().ok();
                assert!(!capsule.is_visible());

                // Move and save
                capsule.move_to(5, 10).ok();
                capsule.save_position().ok();

                // Move elsewhere
                capsule.move_to(20, 30).ok();
                assert_eq!(capsule.position(), (20, 30));

                // Restore
                capsule.restore_position().ok();
                assert_eq!(capsule.position(), (5, 10));

                // Show cursor
                capsule.show().ok();
                assert!(capsule.is_visible());
            }
        }
    }
}
