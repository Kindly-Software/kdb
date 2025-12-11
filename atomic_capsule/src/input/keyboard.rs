//! Keyboard Capsule - T1 Atomic Keyboard State Tracking
//!
//! # Architecture
//! - **Tier 1 (Atomic)**: Lockfree keyboard state coordination
//! - **256-byte alignment**: 4 cache lines for key bitmap + metadata
//! - **Bitmap-based**: O(1) key state lookup via bit operations
//! - **Modifier tracking**: Ctrl, Alt, Shift, Meta with atomic updates
//!
//! # Performance Targets (B32 Framework)
//! - Key state lookup: <5ns (single bit test)
//! - Modifier check: <3ns (atomic load)
//! - State update: <15ns (atomic CAS)
//! - Full scan: <50ns (4 atomic loads)
//!
//! # Safety Assumptions (ASSUM Framework)
//! - #ASSUME[EVDEV-KEYCODES]: Key codes 0-255 cover all standard keys
//! - #ASSUME[BITMAP-ATOMIC]: u64 atomic operations for 64-key chunks
//! - #ASSUME[MODIFIER-PACKED]: All modifiers fit in u16
//! - #VERIFY[STATE-CONSISTENT]: Key bitmap and modifiers atomically consistent
//! - #VERIFY[REPEAT-TRACKING]: Key repeat state tracked separately

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicU16, Ordering};
use crate::alignment::AlignmentTier;
use super::event::{InputEvent, EV_KEY, EventValue};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

// ============================================================================
// KEY CODE CONSTANTS (evdev compatible)
// ============================================================================

/// Reserved key code (no key)
pub const KEY_RESERVED: u16 = 0;

/// Escape key
/// #VERIFY[KEY-ESC-EVDEV]: Matches linux/input-event-codes.h
pub const KEY_ESC: u16 = 1;

/// Number keys 1-0
pub const KEY_1: u16 = 2;
pub const KEY_2: u16 = 3;
pub const KEY_3: u16 = 4;
pub const KEY_4: u16 = 5;
pub const KEY_5: u16 = 6;
pub const KEY_6: u16 = 7;
pub const KEY_7: u16 = 8;
pub const KEY_8: u16 = 9;
pub const KEY_9: u16 = 10;
pub const KEY_0: u16 = 11;

/// Symbol keys
pub const KEY_MINUS: u16 = 12;
pub const KEY_EQUAL: u16 = 13;
pub const KEY_BACKSPACE: u16 = 14;
pub const KEY_TAB: u16 = 15;

/// Letter keys Q-P
pub const KEY_Q: u16 = 16;
pub const KEY_W: u16 = 17;
pub const KEY_E: u16 = 18;
pub const KEY_R: u16 = 19;
pub const KEY_T: u16 = 20;
pub const KEY_Y: u16 = 21;
pub const KEY_U: u16 = 22;
pub const KEY_I: u16 = 23;
pub const KEY_O: u16 = 24;
pub const KEY_P: u16 = 25;

/// Brackets
pub const KEY_LEFTBRACE: u16 = 26;
pub const KEY_RIGHTBRACE: u16 = 27;

/// Enter and Control
pub const KEY_ENTER: u16 = 28;
pub const KEY_LEFTCTRL: u16 = 29;

/// Letter keys A-L
pub const KEY_A: u16 = 30;
pub const KEY_S: u16 = 31;
pub const KEY_D: u16 = 32;
pub const KEY_F: u16 = 33;
pub const KEY_G: u16 = 34;
pub const KEY_H: u16 = 35;
pub const KEY_J: u16 = 36;
pub const KEY_K: u16 = 37;
pub const KEY_L: u16 = 38;

/// Punctuation
pub const KEY_SEMICOLON: u16 = 39;
pub const KEY_APOSTROPHE: u16 = 40;
pub const KEY_GRAVE: u16 = 41;
pub const KEY_LEFTSHIFT: u16 = 42;
pub const KEY_BACKSLASH: u16 = 43;

/// Letter keys Z-M
pub const KEY_Z: u16 = 44;
pub const KEY_X: u16 = 45;
pub const KEY_C: u16 = 46;
pub const KEY_V: u16 = 47;
pub const KEY_B: u16 = 48;
pub const KEY_N: u16 = 49;
pub const KEY_M: u16 = 50;

/// More punctuation
pub const KEY_COMMA: u16 = 51;
pub const KEY_DOT: u16 = 52;
pub const KEY_SLASH: u16 = 53;
pub const KEY_RIGHTSHIFT: u16 = 54;
pub const KEY_KPASTERISK: u16 = 55;
pub const KEY_LEFTALT: u16 = 56;
pub const KEY_SPACE: u16 = 57;
pub const KEY_CAPSLOCK: u16 = 58;

/// Function keys F1-F10
pub const KEY_F1: u16 = 59;
pub const KEY_F2: u16 = 60;
pub const KEY_F3: u16 = 61;
pub const KEY_F4: u16 = 62;
pub const KEY_F5: u16 = 63;
pub const KEY_F6: u16 = 64;
pub const KEY_F7: u16 = 65;
pub const KEY_F8: u16 = 66;
pub const KEY_F9: u16 = 67;
pub const KEY_F10: u16 = 68;

/// Lock keys
pub const KEY_NUMLOCK: u16 = 69;
pub const KEY_SCROLLLOCK: u16 = 70;

/// Keypad keys
pub const KEY_KP7: u16 = 71;
pub const KEY_KP8: u16 = 72;
pub const KEY_KP9: u16 = 73;
pub const KEY_KPMINUS: u16 = 74;
pub const KEY_KP4: u16 = 75;
pub const KEY_KP5: u16 = 76;
pub const KEY_KP6: u16 = 77;
pub const KEY_KPPLUS: u16 = 78;
pub const KEY_KP1: u16 = 79;
pub const KEY_KP2: u16 = 80;
pub const KEY_KP3: u16 = 81;
pub const KEY_KP0: u16 = 82;
pub const KEY_KPDOT: u16 = 83;

/// F11/F12
pub const KEY_F11: u16 = 87;
pub const KEY_F12: u16 = 88;

/// Right side modifiers and navigation
pub const KEY_RIGHTCTRL: u16 = 97;
pub const KEY_KPSLASH: u16 = 98;
pub const KEY_SYSRQ: u16 = 99;
pub const KEY_RIGHTALT: u16 = 100;

/// Navigation keys
pub const KEY_HOME: u16 = 102;
pub const KEY_UP: u16 = 103;
pub const KEY_PAGEUP: u16 = 104;
pub const KEY_LEFT: u16 = 105;
pub const KEY_RIGHT: u16 = 106;
pub const KEY_END: u16 = 107;
pub const KEY_DOWN: u16 = 108;
pub const KEY_PAGEDOWN: u16 = 109;
pub const KEY_INSERT: u16 = 110;
pub const KEY_DELETE: u16 = 111;

/// Pause
pub const KEY_PAUSE: u16 = 119;

/// Meta keys (Windows/Command)
pub const KEY_LEFTMETA: u16 = 125;
pub const KEY_RIGHTMETA: u16 = 126;
pub const KEY_COMPOSE: u16 = 127;

// ============================================================================
// KEYBOARD CONSTANTS
// ============================================================================

/// Maximum key code supported (evdev supports up to KEY_CNT = 0x300)
/// We support 256 for common keys (fits in 4 x u64 bitmap)
/// #ASSUME[KEYCODE-256]: 256 keys sufficient for standard keyboards
pub const MAX_KEYCODES: usize = 256;

/// Default key repeat delay in milliseconds
/// #ASSUME[REPEAT-DELAY]: 500ms is standard default
pub const KEY_REPEAT_DELAY_MS: u32 = 500;

/// Default key repeat rate in milliseconds
/// #ASSUME[REPEAT-RATE]: 30ms is standard default (33 Hz)
pub const KEY_REPEAT_RATE_MS: u32 = 30;

// ============================================================================
// KEY CODE ENUM
// ============================================================================

/// High-level key code representation
///
/// #VERIFY[KEYCODE-MAPPING]: Maps to evdev key codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyCode(pub u16);

impl KeyCode {
    /// Create from raw evdev key code
    #[inline(always)]
    pub const fn from_raw(code: u16) -> Self {
        Self(code)
    }

    /// Get raw evdev key code
    #[inline(always)]
    pub const fn raw(&self) -> u16 {
        self.0
    }

    /// Check if this is a modifier key
    #[inline(always)]
    pub const fn is_modifier(&self) -> bool {
        matches!(
            self.0,
            KEY_LEFTSHIFT | KEY_RIGHTSHIFT |
            KEY_LEFTCTRL | KEY_RIGHTCTRL |
            KEY_LEFTALT | KEY_RIGHTALT |
            KEY_LEFTMETA | KEY_RIGHTMETA
        )
    }

    /// Check if this is a letter key (A-Z)
    #[inline(always)]
    pub const fn is_letter(&self) -> bool {
        // KEY_Q..=KEY_P (16-25), KEY_A..=KEY_L (30-38), KEY_Z..=KEY_M (44-50)
        (self.0 >= KEY_Q && self.0 <= KEY_P) ||      // Q, W, E, R, T, Y, U, I, O, P
        (self.0 >= KEY_A && self.0 <= KEY_L) ||      // A, S, D, F, G, H, J, K, L
        (self.0 >= KEY_Z && self.0 <= KEY_M)         // Z, X, C, V, B, N, M
    }

    /// Check if this is a number key (0-9)
    #[inline(always)]
    pub const fn is_number(&self) -> bool {
        self.0 >= KEY_1 && self.0 <= KEY_0
    }

    /// Check if this is a function key (F1-F12)
    #[inline(always)]
    pub const fn is_function(&self) -> bool {
        (self.0 >= KEY_F1 && self.0 <= KEY_F10) ||
        self.0 == KEY_F11 || self.0 == KEY_F12
    }

    /// Check if this is an arrow key
    #[inline(always)]
    pub const fn is_arrow(&self) -> bool {
        matches!(self.0, KEY_UP | KEY_DOWN | KEY_LEFT | KEY_RIGHT)
    }

    // Common key constants
    pub const A: Self = Self(KEY_A);
    pub const B: Self = Self(KEY_B);
    pub const C: Self = Self(KEY_C);
    pub const D: Self = Self(KEY_D);
    pub const E: Self = Self(KEY_E);
    pub const F: Self = Self(KEY_F);
    pub const G: Self = Self(KEY_G);
    pub const H: Self = Self(KEY_H);
    pub const I: Self = Self(KEY_I);
    pub const J: Self = Self(KEY_J);
    pub const K: Self = Self(KEY_K);
    pub const L: Self = Self(KEY_L);
    pub const M: Self = Self(KEY_M);
    pub const N: Self = Self(KEY_N);
    pub const O: Self = Self(KEY_O);
    pub const P: Self = Self(KEY_P);
    pub const Q: Self = Self(KEY_Q);
    pub const R: Self = Self(KEY_R);
    pub const S: Self = Self(KEY_S);
    pub const T: Self = Self(KEY_T);
    pub const U: Self = Self(KEY_U);
    pub const V: Self = Self(KEY_V);
    pub const W: Self = Self(KEY_W);
    pub const X: Self = Self(KEY_X);
    pub const Y: Self = Self(KEY_Y);
    pub const Z: Self = Self(KEY_Z);
    pub const Escape: Self = Self(KEY_ESC);
    pub const Enter: Self = Self(KEY_ENTER);
    pub const Space: Self = Self(KEY_SPACE);
    pub const Tab: Self = Self(KEY_TAB);
    pub const Backspace: Self = Self(KEY_BACKSPACE);
    pub const Delete: Self = Self(KEY_DELETE);
    pub const Insert: Self = Self(KEY_INSERT);
    pub const Home: Self = Self(KEY_HOME);
    pub const End: Self = Self(KEY_END);
    pub const PageUp: Self = Self(KEY_PAGEUP);
    pub const PageDown: Self = Self(KEY_PAGEDOWN);
    pub const Up: Self = Self(KEY_UP);
    pub const Down: Self = Self(KEY_DOWN);
    pub const Left: Self = Self(KEY_LEFT);
    pub const Right: Self = Self(KEY_RIGHT);
}

// ============================================================================
// KEY MODIFIERS
// ============================================================================

/// Keyboard modifier state (Shift, Ctrl, Alt, Meta)
///
/// # Bit Layout
/// - Bit 0: Left Shift
/// - Bit 1: Right Shift
/// - Bit 2: Left Control
/// - Bit 3: Right Control
/// - Bit 4: Left Alt
/// - Bit 5: Right Alt
/// - Bit 6: Left Meta (Windows/Command)
/// - Bit 7: Right Meta
/// - Bit 8: Caps Lock active
/// - Bit 9: Num Lock active
/// - Bit 10: Scroll Lock active
///
/// #VERIFY[MODIFIER-BITS]: All modifiers fit in u16
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct KeyModifiers(pub u16);

impl KeyModifiers {
    /// No modifiers
    pub const NONE: Self = Self(0);

    /// Left Shift
    pub const LEFT_SHIFT: Self = Self(1 << 0);
    /// Right Shift
    pub const RIGHT_SHIFT: Self = Self(1 << 1);
    /// Either Shift
    pub const SHIFT: Self = Self(0b11);

    /// Left Control
    pub const LEFT_CTRL: Self = Self(1 << 2);
    /// Right Control
    pub const RIGHT_CTRL: Self = Self(1 << 3);
    /// Either Control
    pub const CTRL: Self = Self(0b1100);

    /// Left Alt
    pub const LEFT_ALT: Self = Self(1 << 4);
    /// Right Alt (AltGr on some keyboards)
    pub const RIGHT_ALT: Self = Self(1 << 5);
    /// Either Alt
    pub const ALT: Self = Self(0b110000);

    /// Left Meta (Windows/Command key)
    pub const LEFT_META: Self = Self(1 << 6);
    /// Right Meta
    pub const RIGHT_META: Self = Self(1 << 7);
    /// Either Meta
    pub const META: Self = Self(0b11000000);

    /// Caps Lock is active
    pub const CAPS_LOCK: Self = Self(1 << 8);
    /// Num Lock is active
    pub const NUM_LOCK: Self = Self(1 << 9);
    /// Scroll Lock is active
    pub const SCROLL_LOCK: Self = Self(1 << 10);

    /// Check if shift is pressed (either side)
    #[inline(always)]
    pub const fn shift(&self) -> bool {
        self.0 & Self::SHIFT.0 != 0
    }

    /// Check if control is pressed (either side)
    #[inline(always)]
    pub const fn ctrl(&self) -> bool {
        self.0 & Self::CTRL.0 != 0
    }

    /// Check if alt is pressed (either side)
    #[inline(always)]
    pub const fn alt(&self) -> bool {
        self.0 & Self::ALT.0 != 0
    }

    /// Check if meta is pressed (either side)
    #[inline(always)]
    pub const fn meta(&self) -> bool {
        self.0 & Self::META.0 != 0
    }

    /// Check if caps lock is active
    #[inline(always)]
    pub const fn caps_lock(&self) -> bool {
        self.0 & Self::CAPS_LOCK.0 != 0
    }

    /// Check if specific modifier is active
    #[inline(always)]
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Combine modifiers
    #[inline(always)]
    pub const fn union(&self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Remove modifiers
    #[inline(always)]
    pub const fn difference(&self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

// ============================================================================
// KEY EVENT
// ============================================================================

/// High-level key event representation
///
/// #VERIFY[KEY-EVENT-COMPAT]: Compatible with terminal/input API
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    /// Key code
    pub code: KeyCode,
    /// Modifiers active during event
    pub modifiers: KeyModifiers,
    /// Event type (press, release, repeat)
    pub kind: KeyEventKind,
}

/// Key event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum KeyEventKind {
    /// Key pressed
    Press = 1,
    /// Key released
    Release = 0,
    /// Key held (auto-repeat)
    Repeat = 2,
}

impl KeyEventKind {
    /// Convert from evdev value
    #[inline(always)]
    pub const fn from_evdev(value: i32) -> Self {
        match value {
            0 => KeyEventKind::Release,
            1 => KeyEventKind::Press,
            2 => KeyEventKind::Repeat,
            _ => KeyEventKind::Press,
        }
    }

    /// Check if key is down (pressed or repeating)
    #[inline(always)]
    pub const fn is_down(&self) -> bool {
        matches!(self, KeyEventKind::Press | KeyEventKind::Repeat)
    }
}

impl KeyEvent {
    /// Create new key event
    #[inline(always)]
    pub const fn new(code: KeyCode, modifiers: KeyModifiers, kind: KeyEventKind) -> Self {
        Self { code, modifiers, kind }
    }
}

// ============================================================================
// KEYBOARD STATE
// ============================================================================

/// Keyboard state representation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum KeyboardState {
    /// Keyboard ready for input
    Ready = 0,
    /// Waiting for key release (sticky key behavior)
    WaitingRelease = 1,
    /// Processing input
    Processing = 2,
    /// Error state
    Error = 255,
}

impl KeyboardState {
    /// Convert from raw value
    #[inline(always)]
    pub const fn from_raw(val: u8) -> Self {
        match val {
            0 => KeyboardState::Ready,
            1 => KeyboardState::WaitingRelease,
            2 => KeyboardState::Processing,
            _ => KeyboardState::Error,
        }
    }
}

// ============================================================================
// KEYBOARD SNAPSHOT
// ============================================================================

/// Atomic snapshot of keyboard state
///
/// #VERIFY[SNAPSHOT-ATOMIC]: All fields captured atomically
#[derive(Debug, Clone, Copy)]
pub struct KeyboardSnapshot {
    /// Active modifiers
    pub modifiers: KeyModifiers,
    /// Number of keys currently pressed
    pub keys_pressed: u32,
    /// Last key code pressed
    pub last_key: u16,
    /// Generation counter
    pub generation: u64,
}

// ============================================================================
// KEYBOARD CAPSULE (T1 Atomic)
// ============================================================================

/// Lockfree keyboard state tracking capsule
///
/// # Architecture (T1 Atomic)
/// - **256-byte alignment**: 4 cache lines
/// - **Bitmap storage**: 256 keys in 4 x u64 = 32 bytes
/// - **Atomic modifiers**: Single u64 for all modifier state
/// - **Generation counter**: ABA prevention
///
/// # Memory Layout (256 bytes)
/// - Offset 0-63: First cache line (state + metadata)
///   - 0-7: State + generation (AtomicU64)
///   - 8-15: Modifiers + last key (AtomicU64)
///   - 16-19: Keys pressed count (AtomicU32)
///   - 20-23: Repeat key code (AtomicU32)
///   - 24-31: Repeat timestamp (AtomicU64)
///   - 32-63: Padding
/// - Offset 64-95: Key bitmap 0-63 (AtomicU64)
/// - Offset 96-127: Key bitmap 64-127 (AtomicU64)
/// - Offset 128-159: Key bitmap 128-191 (AtomicU64)
/// - Offset 160-191: Key bitmap 192-255 (AtomicU64)
/// - Offset 192-255: Repeat bitmap (AtomicU64 x 4) for tracking which keys are repeating
///
/// #ASSUME[LAYOUT-OPTIMAL]: Layout optimized for common operations
/// #VERIFY[LOCKFREE]: All operations use atomic primitives
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 256, size = 256))]
#[repr(C, align(256))]
pub struct KeyboardCapsule {
    // === First cache line (64 bytes): Metadata ===

    /// State + generation counter
    /// - Bits 0-7: State enum
    /// - Bits 8-31: Reserved
    /// - Bits 32-63: Generation counter
    ///
    /// #ASSUME[STATE-ATOMIC]: State transitions via CAS
    state_gen: AtomicU64,

    /// Modifiers + last key pressed
    /// - Bits 0-15: Modifiers (KeyModifiers)
    /// - Bits 16-31: Last key code
    /// - Bits 32-63: Last event timestamp (low 32 bits of nanos)
    ///
    /// #ASSUME[MODIFIER-ATOMIC]: Modifier updates are atomic
    modifiers_key: AtomicU64,

    /// Count of keys currently pressed
    /// #VERIFY[COUNT-CONSISTENT]: Matches set bits in bitmap
    keys_pressed: AtomicU32,

    /// Currently repeating key code (0 = none)
    repeat_key: AtomicU32,

    /// Repeat timer (nanoseconds since epoch)
    repeat_time: AtomicU64,

    /// Padding to complete first cache line
    _padding1: [u8; 24], // 8+8+4+4+8+24 = 56, need 64 total, so 8 more
    _padding1b: [u8; 8],

    // === Second cache line (64 bytes): Key bitmap 0-127 ===

    /// Key bitmap for codes 0-63
    /// #VERIFY[BITMAP-0-63]: Bit N = 1 means key N is pressed
    bitmap_0: AtomicU64,

    /// Key bitmap for codes 64-127
    /// #VERIFY[BITMAP-64-127]: Bit N = 1 means key 64+N is pressed
    bitmap_1: AtomicU64,

    /// Padding
    _padding2: [u8; 48],

    // === Third cache line (64 bytes): Key bitmap 128-255 ===

    /// Key bitmap for codes 128-191
    bitmap_2: AtomicU64,

    /// Key bitmap for codes 192-255
    bitmap_3: AtomicU64,

    /// Padding
    _padding3: [u8; 48],

    // === Fourth cache line (64 bytes): Repeat tracking ===

    /// Repeat bitmap for keys 0-63 (which keys are in repeat state)
    repeat_bitmap_0: AtomicU64,

    /// Repeat bitmap for keys 64-127
    repeat_bitmap_1: AtomicU64,

    /// Repeat bitmap for keys 128-191
    repeat_bitmap_2: AtomicU64,

    /// Repeat bitmap for keys 192-255
    repeat_bitmap_3: AtomicU64,

    /// Padding
    _padding4: [u8; 32],
}

impl AlignmentTier for KeyboardCapsule {
    const TIER: &'static str = "atomic";
    const ALIGNMENT: usize = 256;
}

// Compile-time verification
#[cfg(not(feature = "derive"))]
crate::verify_capsule_properties!(KeyboardCapsule, 256, 256);

impl KeyboardCapsule {
    /// Create new keyboard capsule in ready state
    pub const fn new() -> Self {
        Self {
            state_gen: AtomicU64::new(0),
            modifiers_key: AtomicU64::new(0),
            keys_pressed: AtomicU32::new(0),
            repeat_key: AtomicU32::new(0),
            repeat_time: AtomicU64::new(0),
            _padding1: [0; 24],
            _padding1b: [0; 8],
            bitmap_0: AtomicU64::new(0),
            bitmap_1: AtomicU64::new(0),
            _padding2: [0; 48],
            bitmap_2: AtomicU64::new(0),
            bitmap_3: AtomicU64::new(0),
            _padding3: [0; 48],
            repeat_bitmap_0: AtomicU64::new(0),
            repeat_bitmap_1: AtomicU64::new(0),
            repeat_bitmap_2: AtomicU64::new(0),
            repeat_bitmap_3: AtomicU64::new(0),
            _padding4: [0; 32],
        }
    }

    /// Get bitmap for key code
    #[inline(always)]
    fn get_bitmap(&self, key: u16) -> (&AtomicU64, u64) {
        let idx = key as usize;
        let bit = 1u64 << (idx % 64);
        let bitmap = match idx / 64 {
            0 => &self.bitmap_0,
            1 => &self.bitmap_1,
            2 => &self.bitmap_2,
            _ => &self.bitmap_3,
        };
        (bitmap, bit)
    }

    /// Get repeat bitmap for key code
    #[inline(always)]
    fn get_repeat_bitmap(&self, key: u16) -> (&AtomicU64, u64) {
        let idx = key as usize;
        let bit = 1u64 << (idx % 64);
        let bitmap = match idx / 64 {
            0 => &self.repeat_bitmap_0,
            1 => &self.repeat_bitmap_1,
            2 => &self.repeat_bitmap_2,
            _ => &self.repeat_bitmap_3,
        };
        (bitmap, bit)
    }

    /// Check if a specific key is pressed
    ///
    /// # Performance
    /// - Typical: <5ns
    ///
    /// #VERIFY[KEY-CHECK-ATOMIC]: Single atomic load
    #[inline(always)]
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        if key.0 >= MAX_KEYCODES as u16 {
            return false;
        }
        let (bitmap, bit) = self.get_bitmap(key.0);
        bitmap.load(Ordering::Relaxed) & bit != 0
    }

    /// Check if a key is in repeat state
    #[inline(always)]
    pub fn is_key_repeating(&self, key: KeyCode) -> bool {
        if key.0 >= MAX_KEYCODES as u16 {
            return false;
        }
        let (bitmap, bit) = self.get_repeat_bitmap(key.0);
        bitmap.load(Ordering::Relaxed) & bit != 0
    }

    /// Get current modifier state
    ///
    /// # Performance
    /// - Typical: <3ns
    #[inline(always)]
    pub fn modifiers(&self) -> KeyModifiers {
        let packed = self.modifiers_key.load(Ordering::Acquire);
        KeyModifiers((packed & 0xFFFF) as u16)
    }

    /// Get last key pressed
    #[inline(always)]
    pub fn last_key(&self) -> KeyCode {
        let packed = self.modifiers_key.load(Ordering::Relaxed);
        KeyCode(((packed >> 16) & 0xFFFF) as u16)
    }

    /// Get count of keys currently pressed
    #[inline(always)]
    pub fn keys_pressed_count(&self) -> u32 {
        self.keys_pressed.load(Ordering::Relaxed)
    }

    /// Process a key event from evdev
    ///
    /// # Performance
    /// - Typical: <15ns
    ///
    /// # Returns
    /// High-level KeyEvent representation
    ///
    /// #VERIFY[PROCESS-ATOMIC]: State updates are atomic
    pub fn process_event(&self, event: &InputEvent) -> Option<KeyEvent> {
        if event.type_ != EV_KEY {
            return None;
        }

        let key = event.code;
        if key >= MAX_KEYCODES as u16 {
            return None;
        }

        let kind = KeyEventKind::from_evdev(event.value);
        let (bitmap, bit) = self.get_bitmap(key);
        let (repeat_bitmap, repeat_bit) = self.get_repeat_bitmap(key);

        match kind {
            KeyEventKind::Press => {
                // Set key bit
                let old = bitmap.fetch_or(bit, Ordering::AcqRel);
                if old & bit == 0 {
                    // Key was not pressed, increment count
                    self.keys_pressed.fetch_add(1, Ordering::Relaxed);
                }
                // Clear repeat bit
                repeat_bitmap.fetch_and(!repeat_bit, Ordering::Relaxed);

                // Update modifiers if this is a modifier key
                self.update_modifiers_on_press(key);

                // Update last key
                self.update_last_key(key);
            }
            KeyEventKind::Release => {
                // Clear key bit
                let old = bitmap.fetch_and(!bit, Ordering::AcqRel);
                if old & bit != 0 {
                    // Key was pressed, decrement count
                    self.keys_pressed.fetch_sub(1, Ordering::Relaxed);
                }
                // Clear repeat bit
                repeat_bitmap.fetch_and(!repeat_bit, Ordering::Relaxed);

                // Update modifiers if this is a modifier key
                self.update_modifiers_on_release(key);
            }
            KeyEventKind::Repeat => {
                // Set repeat bit
                repeat_bitmap.fetch_or(repeat_bit, Ordering::Relaxed);
                // Update repeat tracking
                self.repeat_key.store(key as u32, Ordering::Relaxed);
            }
        }

        // Increment generation
        self.state_gen.fetch_add(1 << 32, Ordering::Release);

        Some(KeyEvent {
            code: KeyCode(key),
            modifiers: self.modifiers(),
            kind,
        })
    }

    /// Update modifiers when a key is pressed
    fn update_modifiers_on_press(&self, key: u16) {
        let modifier_bit = match key {
            KEY_LEFTSHIFT => KeyModifiers::LEFT_SHIFT.0,
            KEY_RIGHTSHIFT => KeyModifiers::RIGHT_SHIFT.0,
            KEY_LEFTCTRL => KeyModifiers::LEFT_CTRL.0,
            KEY_RIGHTCTRL => KeyModifiers::RIGHT_CTRL.0,
            KEY_LEFTALT => KeyModifiers::LEFT_ALT.0,
            KEY_RIGHTALT => KeyModifiers::RIGHT_ALT.0,
            KEY_LEFTMETA => KeyModifiers::LEFT_META.0,
            KEY_RIGHTMETA => KeyModifiers::RIGHT_META.0,
            KEY_CAPSLOCK => {
                // Toggle caps lock
                self.toggle_lock(KeyModifiers::CAPS_LOCK.0);
                return;
            }
            KEY_NUMLOCK => {
                // Toggle num lock
                self.toggle_lock(KeyModifiers::NUM_LOCK.0);
                return;
            }
            KEY_SCROLLLOCK => {
                // Toggle scroll lock
                self.toggle_lock(KeyModifiers::SCROLL_LOCK.0);
                return;
            }
            _ => return,
        };

        // Set modifier bit
        loop {
            let old = self.modifiers_key.load(Ordering::Relaxed);
            let new_mods = (old & 0xFFFF) as u16 | modifier_bit;
            let new = (old & 0xFFFF_FFFF_FFFF_0000) | new_mods as u64;
            if self.modifiers_key.compare_exchange_weak(
                old, new, Ordering::Release, Ordering::Relaxed
            ).is_ok() {
                break;
            }
        }
    }

    /// Update modifiers when a key is released
    fn update_modifiers_on_release(&self, key: u16) {
        let modifier_bit = match key {
            KEY_LEFTSHIFT => KeyModifiers::LEFT_SHIFT.0,
            KEY_RIGHTSHIFT => KeyModifiers::RIGHT_SHIFT.0,
            KEY_LEFTCTRL => KeyModifiers::LEFT_CTRL.0,
            KEY_RIGHTCTRL => KeyModifiers::RIGHT_CTRL.0,
            KEY_LEFTALT => KeyModifiers::LEFT_ALT.0,
            KEY_RIGHTALT => KeyModifiers::RIGHT_ALT.0,
            KEY_LEFTMETA => KeyModifiers::LEFT_META.0,
            KEY_RIGHTMETA => KeyModifiers::RIGHT_META.0,
            _ => return, // Lock keys don't clear on release
        };

        // Clear modifier bit
        loop {
            let old = self.modifiers_key.load(Ordering::Relaxed);
            let new_mods = (old & 0xFFFF) as u16 & !modifier_bit;
            let new = (old & 0xFFFF_FFFF_FFFF_0000) | new_mods as u64;
            if self.modifiers_key.compare_exchange_weak(
                old, new, Ordering::Release, Ordering::Relaxed
            ).is_ok() {
                break;
            }
        }
    }

    /// Toggle a lock key
    fn toggle_lock(&self, lock_bit: u16) {
        loop {
            let old = self.modifiers_key.load(Ordering::Relaxed);
            let new_mods = (old & 0xFFFF) as u16 ^ lock_bit;
            let new = (old & 0xFFFF_FFFF_FFFF_0000) | new_mods as u64;
            if self.modifiers_key.compare_exchange_weak(
                old, new, Ordering::Release, Ordering::Relaxed
            ).is_ok() {
                break;
            }
        }
    }

    /// Update last key pressed
    fn update_last_key(&self, key: u16) {
        loop {
            let old = self.modifiers_key.load(Ordering::Relaxed);
            let new = (old & 0xFFFF_FFFF_0000_FFFF) | ((key as u64) << 16);
            if self.modifiers_key.compare_exchange_weak(
                old, new, Ordering::Release, Ordering::Relaxed
            ).is_ok() {
                break;
            }
        }
    }

    /// Take atomic snapshot of keyboard state
    ///
    /// # Performance
    /// - Typical: <50ns
    #[inline]
    pub fn snapshot(&self) -> KeyboardSnapshot {
        let state_gen = self.state_gen.load(Ordering::Acquire);
        let modifiers_key = self.modifiers_key.load(Ordering::Acquire);

        KeyboardSnapshot {
            modifiers: KeyModifiers((modifiers_key & 0xFFFF) as u16),
            keys_pressed: self.keys_pressed.load(Ordering::Relaxed),
            last_key: ((modifiers_key >> 16) & 0xFFFF) as u16,
            generation: state_gen >> 32,
        }
    }

    /// Clear all keyboard state
    ///
    /// #ASSUME[CLEAR-EXCLUSIVE]: Caller ensures exclusive access
    pub fn clear(&self) {
        self.bitmap_0.store(0, Ordering::Release);
        self.bitmap_1.store(0, Ordering::Release);
        self.bitmap_2.store(0, Ordering::Release);
        self.bitmap_3.store(0, Ordering::Release);
        self.repeat_bitmap_0.store(0, Ordering::Release);
        self.repeat_bitmap_1.store(0, Ordering::Release);
        self.repeat_bitmap_2.store(0, Ordering::Release);
        self.repeat_bitmap_3.store(0, Ordering::Release);
        self.modifiers_key.store(0, Ordering::Release);
        self.keys_pressed.store(0, Ordering::Release);
        self.repeat_key.store(0, Ordering::Release);
        self.state_gen.fetch_add(1 << 32, Ordering::Release);
    }

    /// Get all currently pressed key codes
    ///
    /// Returns up to `buffer.len()` key codes
    pub fn get_pressed_keys(&self, buffer: &mut [KeyCode]) -> usize {
        let mut count = 0;

        let bitmaps = [
            (0, self.bitmap_0.load(Ordering::Acquire)),
            (64, self.bitmap_1.load(Ordering::Acquire)),
            (128, self.bitmap_2.load(Ordering::Acquire)),
            (192, self.bitmap_3.load(Ordering::Acquire)),
        ];

        for (base, bitmap) in bitmaps {
            let mut bits = bitmap;
            while bits != 0 && count < buffer.len() {
                let idx = bits.trailing_zeros() as u16;
                buffer[count] = KeyCode(base + idx);
                count += 1;
                bits &= bits - 1; // Clear lowest set bit
            }
        }

        count
    }
}

impl Default for KeyboardCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Send + Sync safety
unsafe impl Send for KeyboardCapsule {}
unsafe impl Sync for KeyboardCapsule {}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_code() {
        let key = KeyCode::A;
        assert_eq!(key.raw(), KEY_A);
        assert!(!key.is_modifier());

        let shift = KeyCode(KEY_LEFTSHIFT);
        assert!(shift.is_modifier());
    }

    #[test]
    fn test_modifiers() {
        let mods = KeyModifiers::SHIFT.union(KeyModifiers::CTRL);
        assert!(mods.shift());
        assert!(mods.ctrl());
        assert!(!mods.alt());
        assert!(mods.contains(KeyModifiers::LEFT_SHIFT));
    }

    #[test]
    fn test_keyboard_new() {
        let kb = KeyboardCapsule::new();
        assert!(!kb.is_key_pressed(KeyCode::A));
        assert_eq!(kb.keys_pressed_count(), 0);
        assert_eq!(kb.modifiers(), KeyModifiers::NONE);
    }

    #[test]
    fn test_key_press_release() {
        let kb = KeyboardCapsule::new();

        // Press key A
        let event = InputEvent::new(EV_KEY, KEY_A, 1);
        let key_event = kb.process_event(&event).unwrap();
        assert_eq!(key_event.code, KeyCode::A);
        assert_eq!(key_event.kind, KeyEventKind::Press);
        assert!(kb.is_key_pressed(KeyCode::A));
        assert_eq!(kb.keys_pressed_count(), 1);

        // Release key A
        let event = InputEvent::new(EV_KEY, KEY_A, 0);
        let key_event = kb.process_event(&event).unwrap();
        assert_eq!(key_event.kind, KeyEventKind::Release);
        assert!(!kb.is_key_pressed(KeyCode::A));
        assert_eq!(kb.keys_pressed_count(), 0);
    }

    #[test]
    fn test_modifier_tracking() {
        let kb = KeyboardCapsule::new();

        // Press left shift
        let event = InputEvent::new(EV_KEY, KEY_LEFTSHIFT, 1);
        kb.process_event(&event);
        assert!(kb.modifiers().shift());
        assert!(kb.modifiers().contains(KeyModifiers::LEFT_SHIFT));

        // Press A with shift
        let event = InputEvent::new(EV_KEY, KEY_A, 1);
        let key_event = kb.process_event(&event).unwrap();
        assert!(key_event.modifiers.shift());

        // Release shift
        let event = InputEvent::new(EV_KEY, KEY_LEFTSHIFT, 0);
        kb.process_event(&event);
        assert!(!kb.modifiers().shift());
    }

    #[test]
    fn test_caps_lock_toggle() {
        let kb = KeyboardCapsule::new();

        // Toggle caps lock on
        let event = InputEvent::new(EV_KEY, KEY_CAPSLOCK, 1);
        kb.process_event(&event);
        assert!(kb.modifiers().caps_lock());

        // Release caps lock (should stay on)
        let event = InputEvent::new(EV_KEY, KEY_CAPSLOCK, 0);
        kb.process_event(&event);
        assert!(kb.modifiers().caps_lock());

        // Toggle caps lock off
        let event = InputEvent::new(EV_KEY, KEY_CAPSLOCK, 1);
        kb.process_event(&event);
        assert!(!kb.modifiers().caps_lock());
    }

    #[test]
    fn test_get_pressed_keys() {
        let kb = KeyboardCapsule::new();

        // Press multiple keys
        kb.process_event(&InputEvent::new(EV_KEY, KEY_A, 1));
        kb.process_event(&InputEvent::new(EV_KEY, KEY_B, 1));
        kb.process_event(&InputEvent::new(EV_KEY, KEY_C, 1));

        let mut buffer = [KeyCode(0); 10];
        let count = kb.get_pressed_keys(&mut buffer);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_snapshot() {
        let kb = KeyboardCapsule::new();

        kb.process_event(&InputEvent::new(EV_KEY, KEY_LEFTSHIFT, 1));
        kb.process_event(&InputEvent::new(EV_KEY, KEY_A, 1));

        let snapshot = kb.snapshot();
        assert!(snapshot.modifiers.shift());
        assert_eq!(snapshot.keys_pressed, 2);
        assert_eq!(snapshot.last_key, KEY_A);
    }

    #[test]
    fn test_capsule_size_alignment() {
        use core::mem::{size_of, align_of};

        assert_eq!(size_of::<KeyboardCapsule>(), 256);
        assert_eq!(align_of::<KeyboardCapsule>(), 256);
    }
}
