//! Input Device Subsystem for Capsule OS
//!
//! # Architecture
//! World's first 100% lockfree input subsystem based on Computational Capsule Architecture.
//!
//! ## Tiers
//! - **T5 Streaming**: InputEventCapsule (512B) - Ring buffer event queue
//! - **T1 Atomic**: KeyboardCapsule (256B) - Keyboard state tracking
//! - **T1 Atomic**: MouseCapsule (256B) - Mouse state tracking
//! - **T1 Atomic**: TouchpadCapsule (256B) - Touchpad gesture tracking
//! - **T6 Mixed**: InputSubsystemCapsule (1KB) - Meta-orchestration
//!
//! ## evdev Interface
//! Linux evdev compatible event structures for seamless integration:
//! - `input_event` structure: timestamp, type, code, value
//! - Event types: EV_KEY (keyboard), EV_REL (relative motion), EV_ABS (absolute position)
//! - Device access via /dev/input/eventX character devices
//!
//! ## Performance Targets (B32 Framework)
//! - Event enqueue: <20ns (lockfree ring buffer)
//! - Event dequeue: <15ns (batch support)
//! - Key state lookup: <5ns (bitmap)
//! - Mouse position load: <10ns (atomic)
//! - Full input poll cycle: <100ns
//!
//! ## Design Principles
//! - **100% lockfree**: NO mutex, NO RwLock, NO blocking
//! - **Cache-aligned**: 64/128/256B alignment prevents false sharing
//! - **Generation counters**: ABA prevention for concurrent access
//! - **Atomic state machines**: CAS-based state transitions
//!
//! ## Safety (ASSUM Framework)
//! - 50+ safety assumptions documented with #ASSUME/#VERIFY tags
//! - All capsules have compile-time layout verification
//! - T28 test framework compliance (25+ tests)
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use atomic_capsule::input::{
//!     InputEventCapsule, KeyboardCapsule, MouseCapsule,
//!     EventType, KeyCode, InputEvent,
//! };
//!
//! // Create input subsystem
//! let events = InputEventCapsule::new();
//! let keyboard = KeyboardCapsule::new();
//! let mouse = MouseCapsule::new();
//!
//! // Poll and process events
//! while let Some(event) = events.dequeue() {
//!     match event.event_type {
//!         EventType::Key => keyboard.process_key_event(&event),
//!         EventType::RelativeMotion => mouse.process_rel_event(&event),
//!         EventType::AbsoluteMotion => mouse.process_abs_event(&event),
//!         _ => {}
//!     }
//! }
//!
//! // Check keyboard state
//! if keyboard.is_key_pressed(KeyCode::A) {
//!     // Key A is currently held down
//! }
//!
//! // Get mouse position
//! let (x, y) = mouse.position();
//! let buttons = mouse.buttons();
//! ```
//!
//! # References
//! - [Linux Input Subsystem](https://docs.kernel.org/input/input.html)
//! - [evdev Interface](https://docs.kernel.org/driver-api/input.html)
//! - [libevdev Documentation](https://freedesktop.org/wiki/Software/libevdev/)

// ============================================================================
// MODULE DECLARATIONS
// ============================================================================

mod event;
mod keyboard;
mod mouse;
mod touchpad;
mod subsystem;

// ============================================================================
// PUBLIC RE-EXPORTS
// ============================================================================

// Event types and capsule
pub use event::{
    InputEvent, InputEventCapsule, EventType, EventValue,
    EventTimestamp, EventQueueSnapshot,
    // evdev compatibility constants
    EV_SYN, EV_KEY, EV_REL, EV_ABS, EV_MSC, EV_SW, EV_LED, EV_SND, EV_REP, EV_FF, EV_PWR,
    // Synchronization codes
    SYN_REPORT, SYN_CONFIG, SYN_MT_REPORT, SYN_DROPPED,
    // Relative motion codes
    REL_X, REL_Y, REL_Z, REL_RX, REL_RY, REL_RZ, REL_HWHEEL, REL_DIAL, REL_WHEEL, REL_MISC,
    REL_WHEEL_HI_RES, REL_HWHEEL_HI_RES,
    // Absolute position codes
    ABS_X, ABS_Y, ABS_Z, ABS_RX, ABS_RY, ABS_RZ, ABS_THROTTLE, ABS_RUDDER, ABS_WHEEL,
    ABS_GAS, ABS_BRAKE, ABS_HAT0X, ABS_HAT0Y, ABS_HAT1X, ABS_HAT1Y, ABS_HAT2X, ABS_HAT2Y,
    ABS_HAT3X, ABS_HAT3Y, ABS_PRESSURE, ABS_DISTANCE, ABS_TILT_X, ABS_TILT_Y, ABS_TOOL_WIDTH,
    ABS_MT_SLOT, ABS_MT_TOUCH_MAJOR, ABS_MT_TOUCH_MINOR, ABS_MT_WIDTH_MAJOR, ABS_MT_WIDTH_MINOR,
    ABS_MT_ORIENTATION, ABS_MT_POSITION_X, ABS_MT_POSITION_Y, ABS_MT_TOOL_TYPE,
    ABS_MT_BLOB_ID, ABS_MT_TRACKING_ID, ABS_MT_PRESSURE, ABS_MT_DISTANCE, ABS_MT_TOOL_X, ABS_MT_TOOL_Y,
    // Event queue capacity
    INPUT_EVENT_QUEUE_CAPACITY,
};

// Keyboard capsule and key codes
pub use keyboard::{
    KeyboardCapsule, KeyboardSnapshot, KeyboardState,
    KeyCode, KeyModifiers, KeyEvent, KeyEventKind,
    // Function keys
    KEY_RESERVED, KEY_ESC, KEY_1, KEY_2, KEY_3, KEY_4, KEY_5, KEY_6, KEY_7, KEY_8, KEY_9, KEY_0,
    KEY_MINUS, KEY_EQUAL, KEY_BACKSPACE, KEY_TAB, KEY_Q, KEY_W, KEY_E, KEY_R, KEY_T, KEY_Y,
    KEY_U, KEY_I, KEY_O, KEY_P, KEY_LEFTBRACE, KEY_RIGHTBRACE, KEY_ENTER, KEY_LEFTCTRL,
    KEY_A, KEY_S, KEY_D, KEY_F, KEY_G, KEY_H, KEY_J, KEY_K, KEY_L, KEY_SEMICOLON, KEY_APOSTROPHE,
    KEY_GRAVE, KEY_LEFTSHIFT, KEY_BACKSLASH, KEY_Z, KEY_X, KEY_C, KEY_V, KEY_B, KEY_N, KEY_M,
    KEY_COMMA, KEY_DOT, KEY_SLASH, KEY_RIGHTSHIFT, KEY_KPASTERISK, KEY_LEFTALT, KEY_SPACE,
    KEY_CAPSLOCK, KEY_F1, KEY_F2, KEY_F3, KEY_F4, KEY_F5, KEY_F6, KEY_F7, KEY_F8, KEY_F9, KEY_F10,
    KEY_NUMLOCK, KEY_SCROLLLOCK, KEY_KP7, KEY_KP8, KEY_KP9, KEY_KPMINUS, KEY_KP4, KEY_KP5, KEY_KP6,
    KEY_KPPLUS, KEY_KP1, KEY_KP2, KEY_KP3, KEY_KP0, KEY_KPDOT, KEY_F11, KEY_F12,
    KEY_RIGHTCTRL, KEY_KPSLASH, KEY_SYSRQ, KEY_RIGHTALT, KEY_HOME, KEY_UP, KEY_PAGEUP, KEY_LEFT,
    KEY_RIGHT, KEY_END, KEY_DOWN, KEY_PAGEDOWN, KEY_INSERT, KEY_DELETE, KEY_PAUSE, KEY_LEFTMETA,
    KEY_RIGHTMETA, KEY_COMPOSE,
    // Keyboard state constants
    MAX_KEYCODES, KEY_REPEAT_DELAY_MS, KEY_REPEAT_RATE_MS,
};

// Mouse capsule and button codes
pub use mouse::{
    MouseCapsule, MouseSnapshot, MouseState,
    MouseButton, MouseButtonState, MousePosition, MouseScroll,
    // Button codes
    BTN_MISC, BTN_0, BTN_1, BTN_2, BTN_3, BTN_4, BTN_5, BTN_6, BTN_7, BTN_8, BTN_9,
    BTN_MOUSE, BTN_LEFT, BTN_RIGHT, BTN_MIDDLE, BTN_SIDE, BTN_EXTRA, BTN_FORWARD, BTN_BACK, BTN_TASK,
    // Mouse state constants
    MOUSE_BUTTON_COUNT, MOUSE_HISTORY_SIZE, DEFAULT_MOUSE_SENSITIVITY,
};

// Touchpad capsule
pub use touchpad::{
    TouchpadCapsule, TouchpadSnapshot, TouchpadState,
    TouchGesture, GestureType, TouchPoint, MultiTouchSlot,
    // Gesture constants
    MAX_TOUCH_SLOTS, GESTURE_TAP_TIMEOUT_MS, GESTURE_SWIPE_THRESHOLD,
};

// Input subsystem meta-capsule
pub use subsystem::{
    InputSubsystemCapsule, InputSubsystemSnapshot, InputSubsystemState,
    InputDeviceId, DeviceCapabilities, InputError, InputResult,
    // Device discovery constants
    MAX_INPUT_DEVICES, INPUT_DEVICE_PATH_PREFIX,
};

// ============================================================================
// COMPILE-TIME VERIFICATION
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{size_of, align_of};

    #[test]
    fn test_capsule_sizes() {
        // InputEventCapsule: T5 Streaming, 512B aligned
        assert!(size_of::<InputEventCapsule>() <= 512);
        assert_eq!(align_of::<InputEventCapsule>() % 64, 0);

        // KeyboardCapsule: T1 Atomic, 256B aligned
        assert!(size_of::<KeyboardCapsule>() <= 256);
        assert_eq!(align_of::<KeyboardCapsule>() % 64, 0);

        // MouseCapsule: T1 Atomic, 256B aligned
        assert!(size_of::<MouseCapsule>() <= 256);
        assert_eq!(align_of::<MouseCapsule>() % 64, 0);

        // TouchpadCapsule: T1 Atomic, 256B aligned
        assert!(size_of::<TouchpadCapsule>() <= 256);
        assert_eq!(align_of::<TouchpadCapsule>() % 64, 0);
    }

    #[test]
    fn test_evdev_event_compatibility() {
        // Linux input_event is 24 bytes on 64-bit systems
        assert_eq!(size_of::<InputEvent>(), 24);
    }

    #[test]
    fn test_event_type_values() {
        // Verify evdev compatibility
        assert_eq!(EV_SYN, 0x00);
        assert_eq!(EV_KEY, 0x01);
        assert_eq!(EV_REL, 0x02);
        assert_eq!(EV_ABS, 0x03);
    }
}
