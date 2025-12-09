//! Terminal I/O Capsules
//!
//! Zero-dependency terminal handling with computational capsule architecture.
//! Crossterm-compatible API for easy migration.
//!
//! ## Design Principles
//!
//! - **UCE34 Framework**: Systematic terminal I/O discovery
//! - **Chaos Compliant**: 100% lockfree, cache-aligned capsules
//! - **T0 Auditable**: Error types support Q34 audit trails
//! - **Platform Abstraction**: Unified API across Unix/Windows
//! - **Crossterm Compatible**: Drop-in replacement for most use cases
//!
//! ## Module Organization
//!
//! - `error`: Terminal error types (T0 Auditable)
//! - `event`: Event types and queue (T0 Auditable + T5 Streaming)
//! - `parser`: ANSI escape sequence parser (T2 SIMD) - 2-8× speedup
//! - `mode`: Terminal mode management (T1 Atomic) - Raw mode with RAII cleanup
//! - `output`: Buffered terminal output (T4 Batch) - TerminalWriterCapsule
//! - `platform`: Platform-specific backends (Unix/Windows)
//! - `signal`: Unix signal handling (T1 Atomic)
//!
//! ## Feature Flags
//!
//! - `terminal`: Enable terminal module (base)
//! - `terminal-event`: Event types and queue
//! - `terminal-parser`: ANSI parser
//! - `terminal-output`: Output styling
//! - `terminal-simd`: SIMD-accelerated parser (requires nightly)
//! - `terminal-unix`: Unix backend
//! - `terminal-windows`: Windows backend
//! - `terminal-full`: All terminal features
//! - `preset-terminal`: Terminal preset (std + terminal-full)
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use atomic_capsule::terminal::prelude::*;
//!
//! fn main() -> Result<(), TerminalError> {
//!     // Create terminal instance
//!     let mut term = terminal()?;
//!
//!     // Enable raw mode (automatic cleanup on drop)
//!     let _raw = enable_raw_mode()?;
//!
//!     // Poll for events
//!     loop {
//!         if let Some(Event::Key(key)) = term.poll_event(Duration::from_millis(100))? {
//!             if key.code == KeyCode::Char('q') {
//!                 break;
//!             }
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Crossterm Migration
//!
//! Replace crossterm imports:
//!
//! ```rust,ignore
//! // Before (crossterm)
//! use crossterm::{
//!     event::{self, Event, KeyCode},
//!     terminal::{enable_raw_mode, disable_raw_mode},
//! };
//!
//! // After (atomic_capsule)
//! use atomic_capsule::terminal::prelude::*;
//! ```
//!
//! ## References
//!
//! - [Crossterm Design Patterns](https://github.com/crossterm-rs/crossterm)
//! - [Rust Error Handling 2025](https://markaicode.com/rust-error-handling-2025-guide/)
//! - [Terminal Library Best Practices](https://generalistprogrammer.com/tutorials/crossterm-rust-crate-guide)

// ============================================================================
// MODULE DECLARATIONS
// ============================================================================

// Error types (T0 Auditable)
pub mod error;

// Event types and queue (T0 Auditable + T5 Streaming)
#[cfg(feature = "terminal-event")]
pub mod event;

// Compatibility alias: input -> event
// Some code uses `terminal::input::KeyEvent` instead of `terminal::event::KeyEvent`
#[cfg(feature = "terminal-event")]
pub use event as input;

// ANSI parser (T2 SIMD when terminal-simd enabled)
#[cfg(feature = "terminal-parser")]
pub mod parser;

// ANSI parser FSM capsule (T1 Atomic, 128B)
#[cfg(feature = "terminal-parser")]
pub mod ansi_parser_capsule;

// Mode management (T1 Atomic)
#[cfg(feature = "terminal")]
pub mod mode;

// Output styling and colors (T1 Atomic + T3 Fixed-Point)
#[cfg(feature = "terminal-output")]
pub mod output;

// Platform-specific backends
#[cfg(any(feature = "terminal-unix", feature = "terminal-windows"))]
pub mod platform;

// Signal handling (T1 Atomic) - Unix only
#[cfg(all(unix, feature = "terminal-unix"))]
pub mod signal;

// Metacapsule orchestration (T6 Mixed)
#[cfg(feature = "terminal-full")]
pub mod metacapsule;

// GPU rendering (T7 Heterogeneous)
#[cfg(feature = "terminal-gpu")]
pub mod render;

// Widget system (T1 Atomic + T3 Fixed-Point)
#[cfg(feature = "terminal-widgets")]
pub mod widget;

// Style and animation system (T1 Atomic + T3 Fixed-Point)
#[cfg(any(feature = "terminal-gpu", feature = "terminal-style"))]
pub mod style;

// Application orchestration (T6 Mixed)
#[cfg(feature = "terminal-full")]
pub mod app;

// Shell process management (T8 Network - IPC/PTY)
#[cfg(all(unix, feature = "terminal-unix"))]
pub mod shell;

// PTY capsule (T1 Atomic - low-level pseudo-terminal coordination)
#[cfg(all(unix, feature = "terminal-unix"))]
pub mod pty_capsule;

// Scrollback buffer (T5 Streaming - ring buffer history)
#[cfg(feature = "terminal")]
pub mod scrollback_capsule;

// ============================================================================
// PUBLIC RE-EXPORTS
// ============================================================================

// Core types (always available)
pub use error::TerminalError;

// Event types
#[cfg(feature = "terminal-event")]
pub use event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
    MediaKeyCode, ModifierKeyCode,
    MouseButton, MouseEvent, MouseEventKind,
    EventQueueCapsule, EventQueueWithStorage,
};

// Parser types
#[cfg(feature = "terminal-parser")]
pub use parser::{AnsiParserCapsule, ParserState, ParsedSequence};

// ANSI FSM parser capsule (T1 Atomic, 128B)
#[cfg(feature = "terminal-parser")]
pub use ansi_parser_capsule::{
    AnsiParserCapsuleFsm, FsmState, FsmAction, ParseResult,
};

// Mode management
#[cfg(feature = "terminal")]
pub use mode::{
    RawModeCapsule, RawModeError,
    AlternateScreenCapsule, ScreenError,
    CursorCapsule, CursorError,
};

// Output styling
#[cfg(feature = "terminal-output")]
pub use output::{
    StyleCapsule, ColorCapsule, Color, ColorMode,
    TerminalWriterCapsule,
    BOLD, DIM, ITALIC, UNDERLINE, BLINK, REVERSE, HIDDEN, STRIKETHROUGH,
};

// Platform backend trait
#[cfg(any(feature = "terminal-unix", feature = "terminal-windows"))]
pub use platform::TerminalBackend;

// Unix signal handling
#[cfg(all(unix, feature = "terminal-unix"))]
pub use signal::{SignalHandlerCapsule, SignalError};

// Shell process management
#[cfg(all(unix, feature = "terminal-unix"))]
pub use shell::{TerminalShellCapsule, ShellState, ShellError, Signal, Job};

// PTY capsule (T1 Atomic - low-level pseudo-terminal)
#[cfg(all(unix, feature = "terminal-unix"))]
pub use pty_capsule::{PtyCapsule, PtyState, PtyError, pty_flags};

// Scrollback buffer (T5 Streaming)
#[cfg(feature = "terminal")]
pub use scrollback_capsule::{
    ScrollbackCapsule, ScrollbackLine, ScrollbackSnapshot, ScrollDirection,
    DEFAULT_SCROLLBACK_CAPACITY, MAX_LINE_LENGTH,
};

// Metacapsule orchestration
#[cfg(feature = "terminal-full")]
pub use metacapsule::{TerminalMetacapsule, LifecycleState, BackendType, TerminalSnapshot};

// Application orchestration
#[cfg(feature = "terminal-full")]
pub use app::{
    TerminalAppMetacapsule, AppPhase, FrameResult,
    AppMetricsCapsule, RenderStateCapsule, WidgetRootCapsule,
};

// GPU rendering
#[cfg(feature = "terminal-gpu")]
pub use render::{GlyphCacheCapsule, GlyphEntry, GlyphId, GlyphMetrics, RenderError};

// Widget system
#[cfg(feature = "terminal-widgets")]
pub use widget::{
    Widget, Rect, Constraints, RenderCommandBuffer, RenderStyle,
    ButtonCapsule,
};

// Style module
#[cfg(feature = "terminal-gpu")]
pub use style::{
    // Theme colors
    ThemeColorsCapsule, ThemeColor, BuiltinTheme, ThemeSnapshot,
    // Component styles
    ThemeComponentsCapsule,
    ComponentStyle, InputStyle, PanelStyle, ListItemStyle, TabStyle, MenuItemStyle,
    ButtonVariant, InputVariant, PanelVariant,
    // GPU uniforms
    StyleUniformsCapsule, GlobalUniforms, WidgetUniforms,
    WIDGET_FLAG_FOCUSED, WIDGET_FLAG_HOVERED, WIDGET_FLAG_DISABLED, WIDGET_FLAG_SELECTED,
};

// ============================================================================
// CONVENIENCE FUNCTIONS
// ============================================================================

#[cfg(all(feature = "terminal", any(feature = "terminal-unix", feature = "terminal-windows")))]
use core::time::Duration;

/// Create a new terminal instance with platform detection
///
/// # Examples
///
/// ```rust,ignore
/// use atomic_capsule::terminal;
///
/// let mut term = terminal::terminal()?;
/// ```
#[cfg(all(feature = "terminal", feature = "terminal-unix", unix))]
pub fn terminal() -> Result<platform::unix::UnixBackend, TerminalError> {
    platform::unix::UnixBackend::new()
}

#[cfg(all(feature = "terminal", feature = "terminal-windows", windows))]
pub fn terminal() -> Result<platform::windows::WindowsBackend, TerminalError> {
    platform::windows::WindowsBackend::new()
}

/// Enable raw mode (returns RAII guard for automatic cleanup)
///
/// # Examples
///
/// ```rust,ignore
/// use atomic_capsule::terminal;
///
/// let _raw = terminal::enable_raw_mode()?;
/// // Raw mode automatically disabled on drop
/// ```
#[cfg(feature = "terminal")]
pub fn enable_raw_mode() -> Result<mode::RawModeCapsule, TerminalError> {
    let mut raw = mode::RawModeCapsule::new()
        .map_err(|e| TerminalError::from(e))?;
    raw.enable_raw_mode()
        .map_err(|e| TerminalError::from(e))?;
    Ok(raw)
}

/// Disable raw mode (explicit cleanup, not recommended - use RAII guard instead)
///
/// # Examples
///
/// ```rust,ignore
/// use atomic_capsule::terminal;
///
/// terminal::disable_raw_mode()?;
/// ```
#[cfg(feature = "terminal")]
pub fn disable_raw_mode() -> Result<(), TerminalError> {
    let mut raw = mode::RawModeCapsule::new()
        .map_err(|e| TerminalError::from(e))?;
    raw.disable_raw_mode()
        .map_err(|e| TerminalError::from(e))
}

/// Get terminal size (columns, rows)
///
/// # Examples
///
/// ```rust,ignore
/// use atomic_capsule::terminal;
///
/// let (cols, rows) = terminal::size()?;
/// println!("Terminal: {}x{}", cols, rows);
/// ```
#[cfg(all(feature = "terminal", any(feature = "terminal-unix", feature = "terminal-windows")))]
pub fn size() -> Result<(u16, u16), TerminalError> {
    #[cfg(unix)]
    {
        platform::unix::UnixBackend::new()?.size()
    }
    #[cfg(windows)]
    {
        platform::windows::WindowsBackend::new()?.size()
    }
}

// ============================================================================
// PRELUDE MODULE
// ============================================================================

/// Prelude module for convenient imports
///
/// Import everything you need with a single use statement:
///
/// ```rust,ignore
/// use atomic_capsule::terminal::prelude::*;
/// ```
pub mod prelude {
    // Core types
    pub use super::TerminalError;

    // Events
    #[cfg(feature = "terminal-event")]
    pub use super::{
        Event, KeyCode, KeyEvent, KeyModifiers, KeyEventKind,
        MouseEvent, MouseButton, MouseEventKind,
    };

    // Output styling
    #[cfg(feature = "terminal-output")]
    pub use super::{
        Style, Color, Attribute,
        StyleCapsule, ColorCapsule,
    };

    // Convenience functions
    #[cfg(all(feature = "terminal", any(feature = "terminal-unix", feature = "terminal-windows")))]
    pub use super::{terminal, enable_raw_mode, disable_raw_mode, size};

    // Platform backend (for advanced usage)
    #[cfg(any(feature = "terminal-unix", feature = "terminal-windows"))]
    pub use super::platform::TerminalBackend;
}

// Convenience type aliases for prelude
#[cfg(feature = "terminal-output")]
pub type Style = StyleCapsule;

#[cfg(feature = "terminal-output")]
pub type Attribute = u8; // Attribute flags (BOLD, ITALIC, etc.)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_import() {
        let err = TerminalError::NotATty;
        assert_eq!(format!("{}", err), "Not a TTY device");
    }

    #[test]
    #[cfg(feature = "terminal-event")]
    fn test_event_import() {
        let event = Event::Resize(80, 24);
        assert_eq!(event, Event::Resize(80, 24));
    }

    #[test]
    #[cfg(feature = "terminal-event")]
    fn test_key_modifiers_import() {
        let mods = KeyModifiers::SHIFT;
        assert!(mods.contains(KeyModifiers::SHIFT));
    }

    #[test]
    #[cfg(feature = "terminal-event")]
    fn test_key_event_import() {
        let event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert_eq!(event.code, KeyCode::Char('a'));
        assert!(event.modifiers.contains(KeyModifiers::CONTROL));
    }
}

// Style and animation
#[cfg(any(feature = "terminal-widgets", feature = "terminal-style"))]
pub use style::{
    AnimationCapsule, AnimationDirection, AnimationState, AnimatedProperties,
    EasingFunction, FillMode,
};
