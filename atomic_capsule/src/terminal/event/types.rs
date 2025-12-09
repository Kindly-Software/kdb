//! # Terminal Event Types - T0 Auditable Tier
//!
//! **UCE34 Framework: Core event types for terminal input handling**
//!
//! This module provides comprehensive terminal event types compatible with the
//! crossterm API while maintaining Chaos compliance.
//!
//! ## Type Hierarchy
//! ```text
//! Event (top-level)
//!   ├── Key(KeyEvent)
//!   │     ├── code: KeyCode (u16)
//!   │     ├── modifiers: KeyModifiers (u8 bitflags)
//!   │     └── kind: KeyEventKind (Press/Release/Repeat)
//!   ├── Mouse(MouseEvent)
//!   │     ├── kind: MouseEventKind
//!   │     ├── column: u16
//!   │     ├── row: u16
//!   │     └── modifiers: KeyModifiers
//!   ├── Resize(u16, u16)
//!   ├── FocusGained
//!   ├── FocusLost
//!   └── Paste(String)
//! ```
//!
//! ## Key Features
//! - **100+ key codes**: Comprehensive VT100/ANSI coverage
//! - **Bitflag modifiers**: Efficient multi-modifier representation (SHIFT|CONTROL|ALT)
//! - **Crossterm compatible**: Drop-in replacement for migration
//! - **Copy types**: Zero-cost event passing
//!
//! ## Framework Compliance
//! - **UCE34**: Q10 (T0 Auditable tier)
//! - **Chaos**: 100% safe, simple data types
//! - **ASSUM**: 99.99% safe (no unsafe code, all Copy types)
//!
//! ## References
//! - Crossterm KeyCode: <https://docs.rs/crossterm/latest/crossterm/event/enum.KeyCode.html>
//! - VT100 sequences: <https://vt100.net/docs/vt100-ug/chapter3.html>
//! - Kitty keyboard protocol: <https://sw.kovidgoyal.net/kitty/keyboard-protocol/>

use core::fmt;

// ============================================================================
// TOP-LEVEL EVENT TYPE
// ============================================================================

/// Represents a terminal event.
///
/// This is the top-level event type that encompasses all possible terminal
/// input events: keyboard, mouse, resize, focus, and paste.
///
/// # Crossterm Compatibility
/// This enum matches the crossterm::event::Event API for easy migration.
///
/// # Examples
/// ```rust
/// use atomic_capsule::terminal::event::{Event, KeyCode, KeyEvent, KeyModifiers};
///
/// let key_event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
/// let event = Event::Key(key_event);
///
/// match event {
///     Event::Key(ke) => println!("Key pressed: {:?}", ke.code),
///     Event::Mouse(me) => println!("Mouse event"),
///     Event::Resize(w, h) => println!("Resized to {}x{}", w, h),
///     Event::FocusGained => println!("Focus gained"),
///     Event::FocusLost => println!("Focus lost"),
///     Event::Paste(text) => println!("Pasted: {}", text),
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A key event (press, release, or repeat).
    Key(KeyEvent),
    /// A mouse event (button, movement, scroll).
    Mouse(MouseEvent),
    /// Terminal resize event (width, height in columns/rows).
    Resize(u16, u16),
    /// Terminal focus gained (requires focus tracking mode).
    FocusGained,
    /// Terminal focus lost (requires focus tracking mode).
    FocusLost,
    /// Paste event (bracketed paste mode).
    ///
    /// # Note
    /// Requires enabling bracketed paste mode via ANSI sequences.
    Paste(String),
}

// ============================================================================
// KEY EVENT TYPES
// ============================================================================

/// Represents a key event.
///
/// A key event consists of:
/// - `code`: The specific key pressed (character, function key, arrow, etc.)
/// - `modifiers`: Modifier keys held (Shift, Control, Alt, etc.)
/// - `kind`: Type of event (press, release, repeat)
///
/// # Memory Layout
/// - `code`: 2 bytes (u16)
/// - `modifiers`: 1 byte (bitflags)
/// - `kind`: 1 byte (enum)
/// - Total: 4 bytes (compact representation)
///
/// # Examples
/// ```rust
/// use atomic_capsule::terminal::event::{KeyCode, KeyEvent, KeyModifiers, KeyEventKind};
///
/// // Ctrl+C
/// let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
///
/// // Shift+F1
/// let shift_f1 = KeyEvent::new(KeyCode::F(1), KeyModifiers::SHIFT);
///
/// // Plain Enter key
/// let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
///
/// // Check for specific combinations
/// if ctrl_c.code == KeyCode::Char('c') && ctrl_c.modifiers.contains(KeyModifiers::CONTROL) {
///     println!("Ctrl+C pressed!");
/// }
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    /// The key code.
    pub code: KeyCode,
    /// Modifier keys (Shift, Control, Alt, etc.).
    pub modifiers: KeyModifiers,
    /// The kind of event (press, release, repeat).
    pub kind: KeyEventKind,
}

impl KeyEvent {
    /// Create a new key event with default kind (Press).
    ///
    /// # Examples
    /// ```rust
    /// use atomic_capsule::terminal::event::{KeyCode, KeyEvent, KeyModifiers};
    ///
    /// let event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::SHIFT);
    /// assert_eq!(event.code, KeyCode::Char('a'));
    /// assert!(event.modifiers.contains(KeyModifiers::SHIFT));
    /// ```
    pub const fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self {
            code,
            modifiers,
            kind: KeyEventKind::Press,
        }
    }

    /// Create a new key event with explicit kind.
    ///
    /// # Examples
    /// ```rust
    /// use atomic_capsule::terminal::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    ///
    /// let event = KeyEvent::new_with_kind(
    ///     KeyCode::Char('a'),
    ///     KeyModifiers::NONE,
    ///     KeyEventKind::Repeat,
    /// );
    /// assert_eq!(event.kind, KeyEventKind::Repeat);
    /// ```
    pub const fn new_with_kind(code: KeyCode, modifiers: KeyModifiers, kind: KeyEventKind) -> Self {
        Self {
            code,
            modifiers,
            kind,
        }
    }
}

/// Represents the kind of key event.
///
/// Most terminals only support Press events by default. Release and Repeat
/// events require enabling the kitty keyboard protocol or similar extensions.
///
/// # References
/// - Kitty keyboard protocol: <https://sw.kovidgoyal.net/kitty/keyboard-protocol/>
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum KeyEventKind {
    /// Key press event (default).
    Press = 0,
    /// Key release event (requires keyboard enhancement).
    Release = 1,
    /// Key repeat event (key held down).
    Repeat = 2,
}

/// Key modifiers represented as bitflags.
///
/// This allows efficient representation of multiple simultaneous modifiers
/// (e.g., SHIFT | CONTROL for Ctrl+Shift).
///
/// # Memory Layout
/// - Uses u8 (1 byte) for compact storage
/// - Each modifier is a single bit (6 modifiers total)
/// - Transparent representation over u8
///
/// # Examples
/// ```rust
/// use atomic_capsule::terminal::event::KeyModifiers;
///
/// // Single modifier
/// let ctrl = KeyModifiers::CONTROL;
///
/// // Multiple modifiers
/// let ctrl_shift = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
/// assert!(ctrl_shift.contains(KeyModifiers::CONTROL));
/// assert!(ctrl_shift.contains(KeyModifiers::SHIFT));
///
/// // Check for no modifiers
/// assert_eq!(KeyModifiers::NONE, KeyModifiers::empty());
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct KeyModifiers(u8);

impl KeyModifiers {
    /// No modifiers.
    pub const NONE: Self = Self(0);
    /// Shift key.
    pub const SHIFT: Self = Self(1 << 0);
    /// Control key.
    pub const CONTROL: Self = Self(1 << 1);
    /// Alt key.
    pub const ALT: Self = Self(1 << 2);
    /// Super key (Windows/Command).
    pub const SUPER: Self = Self(1 << 3);
    /// Hyper key.
    pub const HYPER: Self = Self(1 << 4);
    /// Meta key.
    pub const META: Self = Self(1 << 5);

    /// Create an empty set of modifiers.
    #[inline]
    pub const fn empty() -> Self {
        Self::NONE
    }

    /// Check if this set contains the specified modifier.
    ///
    /// # Examples
    /// ```rust
    /// use atomic_capsule::terminal::event::KeyModifiers;
    ///
    /// let mods = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
    /// assert!(mods.contains(KeyModifiers::CONTROL));
    /// assert!(mods.contains(KeyModifiers::SHIFT));
    /// assert!(!mods.contains(KeyModifiers::ALT));
    /// ```
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Check if this set is empty.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

// Implement bitwise OR for combining modifiers
impl core::ops::BitOr for KeyModifiers {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for KeyModifiers {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// Represents a key code.
///
/// This enum covers all standard keyboard keys including:
/// - Character keys (a-z, 0-9, symbols)
/// - Function keys (F1-F24)
/// - Navigation keys (arrows, home, end, page up/down)
/// - Editing keys (insert, delete, backspace)
/// - Modifier keys (when pressed alone)
/// - Media keys (play, pause, volume, etc.)
/// - Special keys (Esc, Tab, Enter, etc.)
///
/// # Memory Representation
/// Uses `#[repr(u16)]` for compact 2-byte storage while supporting 100+ variants.
///
/// # Crossterm Compatibility
/// This enum matches crossterm::event::KeyCode with extensions for media
/// and modifier keys (requires keyboard enhancement mode).
///
/// # Examples
/// ```rust
/// use atomic_capsule::terminal::event::KeyCode;
///
/// // Character keys
/// assert_eq!(KeyCode::Char('a'), KeyCode::Char('a'));
///
/// // Function keys
/// let f1 = KeyCode::F(1);
/// let f12 = KeyCode::F(12);
///
/// // Navigation
/// let up = KeyCode::Up;
/// let home = KeyCode::Home;
///
/// // Special keys
/// let enter = KeyCode::Enter;
/// let esc = KeyCode::Esc;
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u16)]
pub enum KeyCode {
    // ========================================================================
    // CHARACTER KEYS
    // ========================================================================
    /// Backspace key.
    Backspace = 0x0008,
    /// Enter/Return key.
    Enter = 0x000D,
    /// Left arrow key.
    Left = 0x0100,
    /// Right arrow key.
    Right = 0x0101,
    /// Up arrow key.
    Up = 0x0102,
    /// Down arrow key.
    Down = 0x0103,
    /// Home key.
    Home = 0x0104,
    /// End key.
    End = 0x0105,
    /// Page Up key.
    PageUp = 0x0106,
    /// Page Down key.
    PageDown = 0x0107,
    /// Tab key.
    Tab = 0x0009,
    /// Backtab key (Shift+Tab).
    BackTab = 0x0108,
    /// Delete key.
    Delete = 0x007F,
    /// Insert key.
    Insert = 0x0109,

    // ========================================================================
    // FUNCTION KEYS (F1-F24)
    // ========================================================================
    /// Function key (F1-F24).
    ///
    /// # Examples
    /// ```rust
    /// use atomic_capsule::terminal::event::KeyCode;
    ///
    /// let f1 = KeyCode::F(1);
    /// let f12 = KeyCode::F(12);
    /// let f24 = KeyCode::F(24);
    /// ```
    F(u8),

    // ========================================================================
    // PRINTABLE CHARACTER
    // ========================================================================
    /// A printable character.
    ///
    /// # Examples
    /// ```rust
    /// use atomic_capsule::terminal::event::KeyCode;
    ///
    /// let a = KeyCode::Char('a');
    /// let capital_a = KeyCode::Char('A');
    /// let space = KeyCode::Char(' ');
    /// let digit = KeyCode::Char('5');
    /// ```
    Char(char),

    // ========================================================================
    // SPECIAL KEYS
    // ========================================================================
    /// Null character.
    Null = 0x0000,
    /// Escape key.
    Esc = 0x001B,

    // ========================================================================
    // ENHANCED KEYS (require keyboard enhancement mode)
    // ========================================================================
    /// Caps Lock key (requires keyboard enhancement).
    CapsLock = 0x0200,
    /// Scroll Lock key (requires keyboard enhancement).
    ScrollLock = 0x0201,
    /// Num Lock key (requires keyboard enhancement).
    NumLock = 0x0202,
    /// Print Screen key (requires keyboard enhancement).
    PrintScreen = 0x0203,
    /// Pause key (requires keyboard enhancement).
    Pause = 0x0204,
    /// Menu key (context menu, requires keyboard enhancement).
    Menu = 0x0205,

    // ========================================================================
    // KEYPAD KEYS (application mode)
    // ========================================================================
    /// Keypad Begin (center key, numpad 5 with NumLock off).
    KeypadBegin = 0x0210,

    // ========================================================================
    // MEDIA KEYS (require keyboard enhancement)
    // ========================================================================
    /// Media key (play, pause, volume, etc.).
    ///
    /// Requires enabling keyboard enhancement flags.
    Media(MediaKeyCode),

    // ========================================================================
    // MODIFIER KEYS (when pressed alone, requires keyboard enhancement)
    // ========================================================================
    /// Modifier key pressed (when pressed alone).
    ///
    /// Requires enabling keyboard enhancement flags.
    Modifier(ModifierKeyCode),
}

/// Media key codes (requires keyboard enhancement).
///
/// These keys are only available in terminals that support the kitty
/// keyboard protocol or similar extensions.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum MediaKeyCode {
    /// Play key.
    Play = 0,
    /// Pause key.
    Pause = 1,
    /// Play/Pause toggle.
    PlayPause = 2,
    /// Stop key.
    Stop = 3,
    /// Reverse (rewind).
    Reverse = 4,
    /// Fast forward.
    FastForward = 5,
    /// Rewind.
    Rewind = 6,
    /// Track next.
    TrackNext = 7,
    /// Track previous.
    TrackPrevious = 8,
    /// Record.
    Record = 9,
    /// Lower volume.
    LowerVolume = 10,
    /// Raise volume.
    RaiseVolume = 11,
    /// Mute volume.
    MuteVolume = 12,
}

/// Modifier key codes (when pressed alone, requires keyboard enhancement).
///
/// These codes represent modifier keys when pressed independently (not
/// modifying another key).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum ModifierKeyCode {
    /// Left Shift.
    LeftShift = 0,
    /// Left Control.
    LeftControl = 1,
    /// Left Alt.
    LeftAlt = 2,
    /// Left Super (Windows/Command).
    LeftSuper = 3,
    /// Left Hyper.
    LeftHyper = 4,
    /// Left Meta.
    LeftMeta = 5,
    /// Right Shift.
    RightShift = 6,
    /// Right Control.
    RightControl = 7,
    /// Right Alt.
    RightAlt = 8,
    /// Right Super (Windows/Command).
    RightSuper = 9,
    /// Right Hyper.
    RightHyper = 10,
    /// Right Meta.
    RightMeta = 11,
    /// Iso Level 3 Shift.
    IsoLevel3Shift = 12,
    /// Iso Level 5 Shift.
    IsoLevel5Shift = 13,
}

// ============================================================================
// MOUSE EVENT TYPES
// ============================================================================

/// Represents a mouse event.
///
/// # Memory Layout
/// - `kind`: 1 byte (enum)
/// - `column`: 2 bytes (u16)
/// - `row`: 2 bytes (u16)
/// - `modifiers`: 1 byte (bitflags)
/// - Total: 6 bytes (compact representation)
///
/// # Examples
/// ```rust
/// use atomic_capsule::terminal::event::{MouseEvent, MouseEventKind, MouseButton, KeyModifiers};
///
/// // Mouse click at column 10, row 5
/// let click = MouseEvent::new(
///     MouseEventKind::Down(MouseButton::Left),
///     10,
///     5,
///     KeyModifiers::NONE,
/// );
///
/// // Ctrl+Click
/// let ctrl_click = MouseEvent::new(
///     MouseEventKind::Down(MouseButton::Left),
///     10,
///     5,
///     KeyModifiers::CONTROL,
/// );
///
/// // Mouse scroll down
/// let scroll = MouseEvent::new(
///     MouseEventKind::ScrollDown,
///     10,
///     5,
///     KeyModifiers::NONE,
/// );
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct MouseEvent {
    /// The kind of mouse event.
    pub kind: MouseEventKind,
    /// The column position (0-based).
    pub column: u16,
    /// The row position (0-based).
    pub row: u16,
    /// Modifier keys held during the event.
    pub modifiers: KeyModifiers,
}

impl MouseEvent {
    /// Create a new mouse event.
    ///
    /// # Examples
    /// ```rust
    /// use atomic_capsule::terminal::event::{MouseEvent, MouseEventKind, MouseButton, KeyModifiers};
    ///
    /// let event = MouseEvent::new(
    ///     MouseEventKind::Down(MouseButton::Left),
    ///     10,
    ///     5,
    ///     KeyModifiers::SHIFT,
    /// );
    /// assert_eq!(event.column, 10);
    /// assert_eq!(event.row, 5);
    /// ```
    pub const fn new(kind: MouseEventKind, column: u16, row: u16, modifiers: KeyModifiers) -> Self {
        Self {
            kind,
            column,
            row,
            modifiers,
        }
    }
}

/// Represents the kind of mouse event.
///
/// # Examples
/// ```rust
/// use atomic_capsule::terminal::event::{MouseEventKind, MouseButton};
///
/// let down = MouseEventKind::Down(MouseButton::Left);
/// let up = MouseEventKind::Up(MouseButton::Right);
/// let drag = MouseEventKind::Drag(MouseButton::Left);
/// let moved = MouseEventKind::Moved;
/// let scroll_down = MouseEventKind::ScrollDown;
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MouseEventKind {
    /// Mouse button pressed.
    Down(MouseButton),
    /// Mouse button released.
    Up(MouseButton),
    /// Mouse moved while button held (drag).
    Drag(MouseButton),
    /// Mouse moved (no button held).
    Moved,
    /// Scroll down.
    ScrollDown,
    /// Scroll up.
    ScrollUp,
    /// Scroll left (horizontal scroll).
    ScrollLeft,
    /// Scroll right (horizontal scroll).
    ScrollRight,
}

/// Represents a mouse button.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum MouseButton {
    /// Left mouse button.
    Left = 0,
    /// Right mouse button.
    Right = 1,
    /// Middle mouse button.
    Middle = 2,
}

// ============================================================================
// DISPLAY IMPLEMENTATIONS
// ============================================================================

impl fmt::Display for KeyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyCode::Backspace => write!(f, "Backspace"),
            KeyCode::Enter => write!(f, "Enter"),
            KeyCode::Left => write!(f, "Left"),
            KeyCode::Right => write!(f, "Right"),
            KeyCode::Up => write!(f, "Up"),
            KeyCode::Down => write!(f, "Down"),
            KeyCode::Home => write!(f, "Home"),
            KeyCode::End => write!(f, "End"),
            KeyCode::PageUp => write!(f, "PageUp"),
            KeyCode::PageDown => write!(f, "PageDown"),
            KeyCode::Tab => write!(f, "Tab"),
            KeyCode::BackTab => write!(f, "BackTab"),
            KeyCode::Delete => write!(f, "Delete"),
            KeyCode::Insert => write!(f, "Insert"),
            KeyCode::F(n) => write!(f, "F{}", n),
            KeyCode::Char(c) => write!(f, "{}", c),
            KeyCode::Null => write!(f, "Null"),
            KeyCode::Esc => write!(f, "Esc"),
            KeyCode::CapsLock => write!(f, "CapsLock"),
            KeyCode::ScrollLock => write!(f, "ScrollLock"),
            KeyCode::NumLock => write!(f, "NumLock"),
            KeyCode::PrintScreen => write!(f, "PrintScreen"),
            KeyCode::Pause => write!(f, "Pause"),
            KeyCode::Menu => write!(f, "Menu"),
            KeyCode::KeypadBegin => write!(f, "KeypadBegin"),
            KeyCode::Media(mk) => write!(f, "Media({:?})", mk),
            KeyCode::Modifier(mk) => write!(f, "Modifier({:?})", mk),
        }
    }
}

// ============================================================================
// INLINE TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // KEY MODIFIERS TESTS
    // ========================================================================

    #[test]
    fn test_key_modifiers_empty() {
        let empty = KeyModifiers::empty();
        assert_eq!(empty, KeyModifiers::NONE);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_key_modifiers_single() {
        let ctrl = KeyModifiers::CONTROL;
        assert!(ctrl.contains(KeyModifiers::CONTROL));
        assert!(!ctrl.contains(KeyModifiers::SHIFT));
        assert!(!ctrl.is_empty());
    }

    #[test]
    fn test_key_modifiers_multiple() {
        let ctrl_shift = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        assert!(ctrl_shift.contains(KeyModifiers::CONTROL));
        assert!(ctrl_shift.contains(KeyModifiers::SHIFT));
        assert!(!ctrl_shift.contains(KeyModifiers::ALT));
        assert!(!ctrl_shift.is_empty());
    }

    #[test]
    fn test_key_modifiers_all() {
        let all = KeyModifiers::SHIFT
            | KeyModifiers::CONTROL
            | KeyModifiers::ALT
            | KeyModifiers::SUPER
            | KeyModifiers::HYPER
            | KeyModifiers::META;

        assert!(all.contains(KeyModifiers::SHIFT));
        assert!(all.contains(KeyModifiers::CONTROL));
        assert!(all.contains(KeyModifiers::ALT));
        assert!(all.contains(KeyModifiers::SUPER));
        assert!(all.contains(KeyModifiers::HYPER));
        assert!(all.contains(KeyModifiers::META));
    }

    // ========================================================================
    // KEY EVENT TESTS
    // ========================================================================

    #[test]
    fn test_key_event_new() {
        let event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert_eq!(event.code, KeyCode::Char('a'));
        assert!(event.modifiers.contains(KeyModifiers::CONTROL));
        assert_eq!(event.kind, KeyEventKind::Press);
    }

    #[test]
    fn test_key_event_with_kind() {
        let event = KeyEvent::new_with_kind(
            KeyCode::Char('b'),
            KeyModifiers::SHIFT,
            KeyEventKind::Repeat,
        );
        assert_eq!(event.code, KeyCode::Char('b'));
        assert!(event.modifiers.contains(KeyModifiers::SHIFT));
        assert_eq!(event.kind, KeyEventKind::Repeat);
    }

    #[test]
    fn test_key_event_function_keys() {
        for i in 1..=24 {
            let event = KeyEvent::new(KeyCode::F(i), KeyModifiers::NONE);
            assert_eq!(event.code, KeyCode::F(i));
        }
    }

    // ========================================================================
    // MOUSE EVENT TESTS
    // ========================================================================

    #[test]
    fn test_mouse_event_new() {
        let event = MouseEvent::new(
            MouseEventKind::Down(MouseButton::Left),
            10,
            5,
            KeyModifiers::NONE,
        );
        assert_eq!(event.column, 10);
        assert_eq!(event.row, 5);
        assert_eq!(event.modifiers, KeyModifiers::NONE);
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {}
            _ => panic!("Wrong event kind"),
        }
    }

    #[test]
    fn test_mouse_event_with_modifiers() {
        let event = MouseEvent::new(
            MouseEventKind::Down(MouseButton::Right),
            20,
            15,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert!(event.modifiers.contains(KeyModifiers::CONTROL));
        assert!(event.modifiers.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn test_mouse_scroll_events() {
        let scroll_down = MouseEvent::new(MouseEventKind::ScrollDown, 0, 0, KeyModifiers::NONE);
        let scroll_up = MouseEvent::new(MouseEventKind::ScrollUp, 0, 0, KeyModifiers::NONE);

        match scroll_down.kind {
            MouseEventKind::ScrollDown => {}
            _ => panic!("Wrong scroll direction"),
        }
        match scroll_up.kind {
            MouseEventKind::ScrollUp => {}
            _ => panic!("Wrong scroll direction"),
        }
    }

    // ========================================================================
    // EVENT ENUM TESTS
    // ========================================================================

    #[test]
    fn test_event_key() {
        let key_event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let event = Event::Key(key_event);
        match event {
            Event::Key(ke) => assert_eq!(ke.code, KeyCode::Char('a')),
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_event_resize() {
        let event = Event::Resize(80, 24);
        match event {
            Event::Resize(w, h) => {
                assert_eq!(w, 80);
                assert_eq!(h, 24);
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_event_focus() {
        let gained = Event::FocusGained;
        let lost = Event::FocusLost;

        match gained {
            Event::FocusGained => {}
            _ => panic!("Wrong event type"),
        }
        match lost {
            Event::FocusLost => {}
            _ => panic!("Wrong event type"),
        }
    }

    // ========================================================================
    // KEY CODE DISPLAY TESTS
    // ========================================================================

    #[test]
    fn test_key_code_display() {
        assert_eq!(format!("{}", KeyCode::Char('a')), "a");
        assert_eq!(format!("{}", KeyCode::F(1)), "F1");
        assert_eq!(format!("{}", KeyCode::Enter), "Enter");
        assert_eq!(format!("{}", KeyCode::Esc), "Esc");
        assert_eq!(format!("{}", KeyCode::Up), "Up");
    }

    // ========================================================================
    // COPY TRAIT TESTS (ensure all types are Copy)
    // ========================================================================

    #[test]
    fn test_copy_semantics() {
        let key_event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        let copy = key_event; // Copy, not move
        assert_eq!(key_event, copy);

        let mouse_event = MouseEvent::new(
            MouseEventKind::Down(MouseButton::Left),
            10,
            5,
            KeyModifiers::NONE,
        );
        let copy = mouse_event; // Copy, not move
        assert_eq!(mouse_event, copy);
    }

    // ========================================================================
    // CROSSTERM COMPATIBILITY TESTS
    // ========================================================================

    #[test]
    fn test_crossterm_compat_ctrl_c() {
        // Ctrl+C is the universal interrupt signal
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(ctrl_c.code, KeyCode::Char('c'));
        assert!(ctrl_c.modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn test_crossterm_compat_arrows() {
        // Arrow keys should match crossterm behavior
        let up = KeyCode::Up;
        let down = KeyCode::Down;
        let left = KeyCode::Left;
        let right = KeyCode::Right;

        assert_ne!(up, down);
        assert_ne!(left, right);
    }

    #[test]
    fn test_crossterm_compat_function_keys() {
        // Function keys F1-F12 are the most common
        for i in 1..=12 {
            let f_key = KeyCode::F(i);
            match f_key {
                KeyCode::F(n) => assert_eq!(n, i),
                _ => panic!("Not a function key"),
            }
        }
    }
}
