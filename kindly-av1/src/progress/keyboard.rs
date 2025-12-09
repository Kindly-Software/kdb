//! 💜 Kindly-AV1 Keyboard Input Handler
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Keyboard input handling for interactive CLI dashboard with optional crossterm dependency.
//!
//! ## Design Philosophy
//!
//! This module is designed for easy replacement of crossterm:
//! - `KeyboardInput` trait defines clean interface for future implementations
//! - `KeyAction` enum is standalone (no deps)
//! - Crossterm usage isolated behind `cli-interactive` feature flag
//! - Stub implementation for display-only mode
//!
//! ## Architecture
//!
//! ```text
//! KeyboardInput Trait (replaceable interface)
//! ├── CrosstermKeyboardHandler (behind cli-interactive feature)
//! │   ├── Raw mode enable/restore
//! │   ├── Non-blocking key polling
//! │   └── Key event mapping
//! └── StubKeyboardHandler (display-only fallback)
//!     └── Always returns None
//! ```
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Standalone design, no tier requirements (input layer)
//! - **Chaos**: No state capsules (stateless input handler)
//! - **IMPL-2**: Designed for future replacement (trait-based)

use std::io;

/// Keyboard actions for dashboard control
///
/// Standalone enum with no dependencies for easy future replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// Toggle pause/resume encoding (Space)
    TogglePause,

    /// Cancel encoding and exit (Q/q or Esc)
    Cancel,

    /// Increase quality target (+/=)
    QualityUp,

    /// Decrease quality target (-/_)
    QualityDown,

    /// Toggle GPU acceleration on/off (G/g)
    ToggleGpu,

    /// Save checkpoint (S/s, when paused)
    SaveCheckpoint,

    /// Open output file in default player (O/o, when complete)
    OpenOutput,

    /// Re-encode with current settings (R/r, when complete/error)
    ReEncode,

    /// View error logs (L/l, when error)
    ViewLogs,

    /// Exit dashboard (Enter, when complete)
    Exit,

    // ============================================================================
    // Navigation (for wizard/menu)
    // ============================================================================

    /// Move selection up in menu/list (Up arrow)
    Up,

    /// Move selection down in menu/list (Down arrow)
    Down,

    /// Move to next field in wizard (Tab)
    Tab,

    // ============================================================================
    // Menu/Wizard triggers
    // ============================================================================

    /// Open command menu overlay (/)
    OpenMenu,

    /// Select current item in menu context (Enter when in menu)
    Select,

    /// Go back one step in wizard (Backspace/Esc in wizard)
    Back,

    // ============================================================================
    // Text input for wizard file path
    // ============================================================================

    /// Any printable character for file path input
    Char(char),

    /// No action or unknown key
    None,
}

impl KeyAction {
    /// Returns true if this action requires paused state
    #[inline]
    pub const fn requires_paused(self) -> bool {
        matches!(self, KeyAction::SaveCheckpoint)
    }

    /// Returns true if this action is only valid when encoding is complete
    #[inline]
    pub const fn requires_complete(self) -> bool {
        matches!(self, KeyAction::OpenOutput | KeyAction::Exit)
    }

    /// Returns true if this action is only valid when encoding has errored
    #[inline]
    pub const fn requires_error(self) -> bool {
        matches!(self, KeyAction::ViewLogs)
    }

    /// Returns true if this is a navigation action (Up/Down/Tab)
    #[inline]
    pub const fn is_navigation(self) -> bool {
        matches!(self, KeyAction::Up | KeyAction::Down | KeyAction::Tab)
    }

    /// Returns true if this action triggers menu (/)
    #[inline]
    pub const fn is_menu_trigger(self) -> bool {
        matches!(self, KeyAction::OpenMenu)
    }

    /// Returns true if this is a text input character
    #[inline]
    pub const fn is_char(self) -> bool {
        matches!(self, KeyAction::Char(_))
    }

    /// Get the character if this is a Char variant
    #[inline]
    pub const fn as_char(self) -> Option<char> {
        if let KeyAction::Char(c) = self {
            Some(c)
        } else {
            None
        }
    }

    /// Returns human-readable description of the action
    pub const fn description(self) -> &'static str {
        match self {
            KeyAction::TogglePause => "Toggle pause/resume",
            KeyAction::Cancel => "Cancel encoding",
            KeyAction::QualityUp => "Increase quality",
            KeyAction::QualityDown => "Decrease quality",
            KeyAction::ToggleGpu => "Toggle GPU acceleration",
            KeyAction::SaveCheckpoint => "Save checkpoint (paused only)",
            KeyAction::OpenOutput => "Open output file (complete only)",
            KeyAction::ReEncode => "Re-encode (complete/error only)",
            KeyAction::ViewLogs => "View error logs (error only)",
            KeyAction::Exit => "Exit dashboard (complete only)",
            KeyAction::Up => "Move selection up",
            KeyAction::Down => "Move selection down",
            KeyAction::Tab => "Next field",
            KeyAction::OpenMenu => "Open command menu",
            KeyAction::Select => "Select item",
            KeyAction::Back => "Go back",
            KeyAction::Char(_) => "Text input",
            KeyAction::None => "No action",
        }
    }
}

/// Trait for keyboard input providers
///
/// This allows swapping out crossterm for a custom implementation later.
/// The trait is designed to be minimal and stateless where possible.
pub trait KeyboardInput {
    /// Poll for a key press with timeout
    ///
    /// Returns `Some(KeyAction)` if a key was pressed within the timeout,
    /// or `None` if no key was pressed or timeout expired.
    ///
    /// # Arguments
    ///
    /// * `timeout_ms` - Maximum time to wait for a key press in milliseconds
    fn poll_key(&mut self, timeout_ms: u64) -> io::Result<Option<KeyAction>>;

    /// Enable raw mode (disable line buffering, echo)
    ///
    /// This must be called before `poll_key` to enable non-blocking input.
    /// Terminal should be restored via `restore_terminal` on drop or error.
    fn enable_raw_mode(&mut self) -> io::Result<()>;

    /// Restore normal terminal mode
    ///
    /// This should be called on drop or when exiting interactive mode.
    /// Safe to call multiple times (idempotent).
    fn restore_terminal(&mut self) -> io::Result<()>;
}

// ============================================================================
// Crossterm Implementation (behind cli-interactive feature)
// ============================================================================

#[cfg(feature = "cli-crossterm")]
mod crossterm_impl {
    use super::*;
    use crossterm::{
        event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
        terminal,
    };
    use std::time::Duration;

    /// Crossterm-based keyboard input handler
    ///
    /// This implementation uses crossterm for raw mode and non-blocking input.
    /// It can be replaced with a custom implementation by implementing the
    /// `KeyboardInput` trait.
    pub struct CrosstermKeyboardHandler {
        raw_mode_enabled: bool,
    }

    impl CrosstermKeyboardHandler {
        /// Create a new crossterm keyboard handler
        ///
        /// Raw mode is not enabled by default. Call `enable_raw_mode` first.
        pub fn new() -> Self {
            Self {
                raw_mode_enabled: false,
            }
        }

        /// Map crossterm key event to KeyAction
        fn map_key_event(key: KeyCode, _modifiers: KeyModifiers) -> KeyAction {
            match key {
                // Original encoding control keys
                KeyCode::Char(' ') => KeyAction::TogglePause,
                KeyCode::Char('q') | KeyCode::Char('Q') => KeyAction::Cancel,
                KeyCode::Char('+') | KeyCode::Char('=') => KeyAction::QualityUp,
                KeyCode::Char('-') | KeyCode::Char('_') => KeyAction::QualityDown,
                KeyCode::Char('g') | KeyCode::Char('G') => KeyAction::ToggleGpu,
                KeyCode::Char('s') | KeyCode::Char('S') => KeyAction::SaveCheckpoint,
                KeyCode::Char('o') | KeyCode::Char('O') => KeyAction::OpenOutput,
                KeyCode::Char('r') | KeyCode::Char('R') => KeyAction::ReEncode,
                KeyCode::Char('l') | KeyCode::Char('L') => KeyAction::ViewLogs,
                KeyCode::Enter => KeyAction::Exit,
                KeyCode::Esc => KeyAction::Cancel,

                // NEW: Navigation keys
                KeyCode::Up => KeyAction::Up,
                KeyCode::Down => KeyAction::Down,
                KeyCode::Tab => KeyAction::Tab,

                // NEW: Menu/Wizard triggers
                KeyCode::Char('/') => KeyAction::OpenMenu,
                KeyCode::Backspace => KeyAction::Back,

                // NEW: Text input for file paths
                KeyCode::Char(c) if c.is_ascii_graphic() || c == ' ' => KeyAction::Char(c),

                _ => KeyAction::None,
            }
        }
    }

    impl Default for CrosstermKeyboardHandler {
        fn default() -> Self {
            Self::new()
        }
    }

    impl KeyboardInput for CrosstermKeyboardHandler {
        fn poll_key(&mut self, timeout_ms: u64) -> io::Result<Option<KeyAction>> {
            // Poll for key event with timeout
            if event::poll(Duration::from_millis(timeout_ms))? {
                if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
                    let action = Self::map_key_event(code, modifiers);
                    if action != KeyAction::None {
                        return Ok(Some(action));
                    }
                }
            }
            Ok(None)
        }

        fn enable_raw_mode(&mut self) -> io::Result<()> {
            if !self.raw_mode_enabled {
                terminal::enable_raw_mode()?;
                self.raw_mode_enabled = true;
            }
            Ok(())
        }

        fn restore_terminal(&mut self) -> io::Result<()> {
            if self.raw_mode_enabled {
                terminal::disable_raw_mode()?;
                self.raw_mode_enabled = false;
            }
            Ok(())
        }
    }

    impl Drop for CrosstermKeyboardHandler {
        fn drop(&mut self) {
            // Best-effort terminal restoration (ignore errors in drop)
            let _ = self.restore_terminal();
        }
    }
}

#[cfg(feature = "cli-crossterm")]
pub use crossterm_impl::CrosstermKeyboardHandler;

// ============================================================================
// Kindly-Term Implementation (REMOVED - atomic_capsule::terminal doesn't exist)
// ============================================================================
//
// NOTE: The atomic_capsule crate does NOT have a `terminal` module at that path.
// It has a `tui` module with KeyboardInputHistoryCapsule, but that's for history
// tracking, not for raw terminal input.
//
// For now, we only support crossterm backend. A future Chaos-compliant terminal
// backend would need to be implemented in atomic_capsule first.
//
// If you need Chaos-compliant terminal handling, consider:
// 1. Adding a proper terminal input module to atomic_capsule
// 2. Using crossterm (already available via cli-crossterm feature)
// 3. Implementing a libc-based raw mode handler here

// ============================================================================
// Stub Implementation (when no interactive feature is enabled)
// ============================================================================

#[cfg(not(feature = "cli-crossterm"))]
mod stub_impl {
    use super::*;

    /// Stub keyboard input handler for display-only mode
    ///
    /// This implementation always returns `None` when polled, effectively
    /// making the dashboard display-only (no interactive controls).
    pub struct StubKeyboardHandler;

    impl StubKeyboardHandler {
        /// Create a new stub keyboard handler
        pub const fn new() -> Self {
            Self
        }
    }

    impl Default for StubKeyboardHandler {
        fn default() -> Self {
            Self::new()
        }
    }

    impl KeyboardInput for StubKeyboardHandler {
        #[inline]
        fn poll_key(&mut self, _timeout_ms: u64) -> io::Result<Option<KeyAction>> {
            // Display-only mode: no keyboard input
            Ok(None)
        }

        #[inline]
        fn enable_raw_mode(&mut self) -> io::Result<()> {
            // No-op: display-only mode
            Ok(())
        }

        #[inline]
        fn restore_terminal(&mut self) -> io::Result<()> {
            // No-op: display-only mode
            Ok(())
        }
    }
}

#[cfg(not(feature = "cli-crossterm"))]
pub use stub_impl::StubKeyboardHandler;

// ============================================================================
// Convenience Type Alias
// ============================================================================

/// Default keyboard handler based on feature flags
///
/// Priority: cli-crossterm > stub
///
/// This type alias allows code to use `DefaultKeyboardHandler` without
/// worrying about which implementation is active.
///
/// NOTE: cli-kindly-term feature removed because atomic_capsule::terminal doesn't exist.
#[cfg(feature = "cli-crossterm")]
pub type DefaultKeyboardHandler = CrosstermKeyboardHandler;

/// Default keyboard handler based on feature flags (stub fallback)
#[cfg(not(feature = "cli-crossterm"))]
pub type DefaultKeyboardHandler = StubKeyboardHandler;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_action_enum_variants() {
        // Verify all variants exist and are distinct
        assert_ne!(KeyAction::TogglePause, KeyAction::Cancel);
        assert_ne!(KeyAction::QualityUp, KeyAction::QualityDown);
        assert_ne!(KeyAction::ToggleGpu, KeyAction::SaveCheckpoint);
        assert_ne!(KeyAction::OpenOutput, KeyAction::ReEncode);
        assert_ne!(KeyAction::ViewLogs, KeyAction::Exit);
        assert_eq!(KeyAction::None, KeyAction::None);
    }

    #[test]
    fn test_key_action_state_requirements() {
        // Test requires_paused
        assert!(KeyAction::SaveCheckpoint.requires_paused());
        assert!(!KeyAction::TogglePause.requires_paused());
        assert!(!KeyAction::Cancel.requires_paused());

        // Test requires_complete
        assert!(KeyAction::OpenOutput.requires_complete());
        assert!(KeyAction::Exit.requires_complete());
        assert!(!KeyAction::SaveCheckpoint.requires_complete());
        assert!(!KeyAction::TogglePause.requires_complete());

        // Test requires_error
        assert!(KeyAction::ViewLogs.requires_error());
        assert!(!KeyAction::Cancel.requires_error());
        assert!(!KeyAction::OpenOutput.requires_error());
    }

    #[test]
    fn test_key_action_descriptions() {
        // Verify all actions have non-empty descriptions
        assert!(!KeyAction::TogglePause.description().is_empty());
        assert!(!KeyAction::Cancel.description().is_empty());
        assert!(!KeyAction::QualityUp.description().is_empty());
        assert!(!KeyAction::QualityDown.description().is_empty());
        assert!(!KeyAction::ToggleGpu.description().is_empty());
        assert!(!KeyAction::SaveCheckpoint.description().is_empty());
        assert!(!KeyAction::OpenOutput.description().is_empty());
        assert!(!KeyAction::ReEncode.description().is_empty());
        assert!(!KeyAction::ViewLogs.description().is_empty());
        assert!(!KeyAction::Exit.description().is_empty());
        assert!(!KeyAction::None.description().is_empty());
    }

    #[cfg(not(feature = "cli-crossterm"))]
    #[test]
    fn test_stub_handler_returns_none() {
        let mut handler = StubKeyboardHandler::new();

        // Verify stub always returns None
        let result = handler.poll_key(100);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        // Verify enable/restore are no-ops
        assert!(handler.enable_raw_mode().is_ok());
        assert!(handler.restore_terminal().is_ok());
    }

    #[cfg(not(feature = "cli-crossterm"))]
    #[test]
    fn test_stub_handler_default() {
        let handler = StubKeyboardHandler::default();
        assert_eq!(std::mem::size_of_val(&handler), 0); // ZST
    }

    #[cfg(feature = "cli-crossterm")]
    #[test]
    fn test_crossterm_handler_creation() {
        let _handler = CrosstermKeyboardHandler::new();
        // Can't test raw_mode_enabled (private field), but verify no panic
    }

    #[cfg(feature = "cli-crossterm")]
    #[test]
    fn test_crossterm_handler_default() {
        let _handler = CrosstermKeyboardHandler::default();
        // Can't test raw_mode_enabled (private field), but verify no panic
    }

    #[cfg(feature = "cli-crossterm")]
    #[test]
    #[ignore = "Requires TTY - run manually with: cargo test --lib test_crossterm_raw_mode_toggle -- --ignored"]
    fn test_crossterm_raw_mode_toggle() {
        let mut handler = CrosstermKeyboardHandler::new();

        // Enable raw mode
        assert!(handler.enable_raw_mode().is_ok());

        // Enable again (idempotent)
        assert!(handler.enable_raw_mode().is_ok());

        // Restore terminal
        assert!(handler.restore_terminal().is_ok());

        // Restore again (idempotent)
        assert!(handler.restore_terminal().is_ok());
    }

    // ============================================================================
    // Kindly-Term Handler Tests (REMOVED - feature doesn't exist)
    // ============================================================================
    //
    // NOTE: Tests removed because cli-kindly-term feature and KindlyTermKeyboardHandler
    // implementation were removed (atomic_capsule::terminal doesn't exist).
    //
    // If you implement a Chaos-compliant terminal handler in the future, add tests here.

    #[test]
    fn test_default_keyboard_handler_type_alias() {
        // Verify DefaultKeyboardHandler exists and can be constructed
        let _handler = DefaultKeyboardHandler::default();

        // Verify it implements KeyboardInput trait
        fn accepts_keyboard_input<T: KeyboardInput>(_handler: &T) {}
        let handler = DefaultKeyboardHandler::default();
        accepts_keyboard_input(&handler);
    }

    // ============================================================================
    // Tests for NEW navigation/menu variants
    // ============================================================================

    #[test]
    fn test_navigation_actions() {
        // Test navigation variants exist and are distinct
        assert_ne!(KeyAction::Up, KeyAction::Down);
        assert_ne!(KeyAction::Down, KeyAction::Tab);
        assert_ne!(KeyAction::Up, KeyAction::Tab);
    }

    #[test]
    fn test_menu_trigger_actions() {
        // Test menu/wizard trigger variants
        assert_ne!(KeyAction::OpenMenu, KeyAction::Select);
        assert_ne!(KeyAction::Select, KeyAction::Back);
        assert_ne!(KeyAction::OpenMenu, KeyAction::Back);
    }

    #[test]
    fn test_char_action_variant() {
        // Test Char variant with different characters
        let char_a = KeyAction::Char('a');
        let char_b = KeyAction::Char('b');
        let char_slash = KeyAction::Char('/');

        assert_ne!(char_a, char_b);
        assert_ne!(char_a, char_slash);
        assert_eq!(char_a, KeyAction::Char('a'));
    }

    #[test]
    fn test_is_navigation() {
        // Positive cases
        assert!(KeyAction::Up.is_navigation());
        assert!(KeyAction::Down.is_navigation());
        assert!(KeyAction::Tab.is_navigation());

        // Negative cases
        assert!(!KeyAction::OpenMenu.is_navigation());
        assert!(!KeyAction::Back.is_navigation());
        assert!(!KeyAction::Char('a').is_navigation());
        assert!(!KeyAction::TogglePause.is_navigation());
        assert!(!KeyAction::None.is_navigation());
    }

    #[test]
    fn test_is_menu_trigger() {
        // Positive case
        assert!(KeyAction::OpenMenu.is_menu_trigger());

        // Negative cases
        assert!(!KeyAction::Up.is_menu_trigger());
        assert!(!KeyAction::Select.is_menu_trigger());
        assert!(!KeyAction::Back.is_menu_trigger());
        assert!(!KeyAction::Char('/').is_menu_trigger());
        assert!(!KeyAction::None.is_menu_trigger());
    }

    #[test]
    fn test_is_char() {
        // Positive cases
        assert!(KeyAction::Char('a').is_char());
        assert!(KeyAction::Char('Z').is_char());
        assert!(KeyAction::Char('/').is_char());
        assert!(KeyAction::Char(' ').is_char());

        // Negative cases
        assert!(!KeyAction::Up.is_char());
        assert!(!KeyAction::OpenMenu.is_char());
        assert!(!KeyAction::None.is_char());
        assert!(!KeyAction::TogglePause.is_char());
    }

    #[test]
    fn test_as_char() {
        // Positive cases
        assert_eq!(KeyAction::Char('a').as_char(), Some('a'));
        assert_eq!(KeyAction::Char('Z').as_char(), Some('Z'));
        assert_eq!(KeyAction::Char('/').as_char(), Some('/'));
        assert_eq!(KeyAction::Char(' ').as_char(), Some(' '));

        // Negative cases
        assert_eq!(KeyAction::Up.as_char(), None);
        assert_eq!(KeyAction::OpenMenu.as_char(), None);
        assert_eq!(KeyAction::None.as_char(), None);
        assert_eq!(KeyAction::TogglePause.as_char(), None);
    }

    #[test]
    fn test_new_action_descriptions() {
        // Verify new actions have non-empty descriptions
        assert!(!KeyAction::Up.description().is_empty());
        assert!(!KeyAction::Down.description().is_empty());
        assert!(!KeyAction::Tab.description().is_empty());
        assert!(!KeyAction::OpenMenu.description().is_empty());
        assert!(!KeyAction::Select.description().is_empty());
        assert!(!KeyAction::Back.description().is_empty());
        assert!(!KeyAction::Char('a').description().is_empty());

        // Verify specific descriptions
        assert_eq!(KeyAction::Up.description(), "Move selection up");
        assert_eq!(KeyAction::Down.description(), "Move selection down");
        assert_eq!(KeyAction::Tab.description(), "Next field");
        assert_eq!(KeyAction::OpenMenu.description(), "Open command menu");
        assert_eq!(KeyAction::Select.description(), "Select item");
        assert_eq!(KeyAction::Back.description(), "Go back");
        assert_eq!(KeyAction::Char('x').description(), "Text input");
    }

    #[test]
    fn test_new_actions_not_require_special_state() {
        // Navigation actions should not require special state
        assert!(!KeyAction::Up.requires_paused());
        assert!(!KeyAction::Down.requires_paused());
        assert!(!KeyAction::Tab.requires_paused());

        assert!(!KeyAction::Up.requires_complete());
        assert!(!KeyAction::Down.requires_complete());
        assert!(!KeyAction::Tab.requires_complete());

        assert!(!KeyAction::Up.requires_error());
        assert!(!KeyAction::Down.requires_error());
        assert!(!KeyAction::Tab.requires_error());

        // Menu/wizard triggers should not require special state
        assert!(!KeyAction::OpenMenu.requires_paused());
        assert!(!KeyAction::Select.requires_complete());
        assert!(!KeyAction::Back.requires_error());

        // Char variant should not require special state
        assert!(!KeyAction::Char('a').requires_paused());
        assert!(!KeyAction::Char('a').requires_complete());
        assert!(!KeyAction::Char('a').requires_error());
    }

    #[test]
    fn test_char_variant_printable_chars() {
        // Test various printable character categories

        // Lowercase letters
        for c in 'a'..='z' {
            let action = KeyAction::Char(c);
            assert!(action.is_char());
            assert_eq!(action.as_char(), Some(c));
        }

        // Uppercase letters
        for c in 'A'..='Z' {
            let action = KeyAction::Char(c);
            assert!(action.is_char());
            assert_eq!(action.as_char(), Some(c));
        }

        // Digits
        for c in '0'..='9' {
            let action = KeyAction::Char(c);
            assert!(action.is_char());
            assert_eq!(action.as_char(), Some(c));
        }

        // Common punctuation for file paths
        for c in ['.', '/', '-', '_', ' '].iter() {
            let action = KeyAction::Char(*c);
            assert!(action.is_char());
            assert_eq!(action.as_char(), Some(*c));
        }
    }

    #[test]
    fn test_navigation_and_menu_helpers_comprehensive() {
        // Build list of all actions that should be navigation
        let nav_actions = [KeyAction::Up, KeyAction::Down, KeyAction::Tab];

        // Build list of all actions that should NOT be navigation
        let non_nav_actions = [
            KeyAction::TogglePause,
            KeyAction::Cancel,
            KeyAction::QualityUp,
            KeyAction::QualityDown,
            KeyAction::ToggleGpu,
            KeyAction::SaveCheckpoint,
            KeyAction::OpenOutput,
            KeyAction::ReEncode,
            KeyAction::ViewLogs,
            KeyAction::Exit,
            KeyAction::OpenMenu,
            KeyAction::Select,
            KeyAction::Back,
            KeyAction::Char('a'),
            KeyAction::None,
        ];

        // Test all navigation actions
        for action in &nav_actions {
            assert!(action.is_navigation(), "Expected {:?} to be navigation", action);
        }

        // Test all non-navigation actions
        for action in &non_nav_actions {
            assert!(!action.is_navigation(), "Expected {:?} to NOT be navigation", action);
        }

        // Test menu trigger
        assert!(KeyAction::OpenMenu.is_menu_trigger());

        // Test all non-menu-trigger actions
        let all_non_menu: Vec<KeyAction> = nav_actions
            .iter()
            .chain(non_nav_actions.iter().filter(|a| **a != KeyAction::OpenMenu))
            .copied()
            .collect();

        for action in &all_non_menu {
            assert!(!action.is_menu_trigger(), "Expected {:?} to NOT be menu trigger", action);
        }
    }
}
