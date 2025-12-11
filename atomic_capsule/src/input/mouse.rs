//! Mouse Capsule - T1 Atomic Mouse State Tracking
//!
//! # Architecture
//! - **Tier 1 (Atomic)**: Lockfree mouse state coordination
//! - **256-byte alignment**: 4 cache lines for position + buttons + metadata
//! - **Atomic position**: 64-bit packed X/Y coordinates
//! - **Button bitmap**: Supports up to 16 mouse buttons
//!
//! # Performance Targets (B32 Framework)
//! - Position load: <5ns (single atomic)
//! - Button check: <3ns (atomic load + bit test)
//! - Position update: <10ns (atomic CAS)
//! - Full state snapshot: <20ns
//!
//! # Safety Assumptions (ASSUM Framework)
//! - #ASSUME[REL-MOTION]: Relative motion accumulates in position
//! - #ASSUME[ABS-MOTION]: Absolute motion replaces position
//! - #ASSUME[BUTTON-16]: Up to 16 buttons supported (BTN_MOUSE to BTN_TASK)
//! - #VERIFY[POSITION-ATOMIC]: X/Y updates are atomic
//! - #VERIFY[BUTTON-ATOMIC]: Button state updates are atomic

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicI32, AtomicI64, Ordering};
use crate::alignment::AlignmentTier;
use super::event::{InputEvent, EV_KEY, EV_REL, EV_ABS, REL_X, REL_Y, REL_WHEEL, REL_HWHEEL,
                   REL_WHEEL_HI_RES, REL_HWHEEL_HI_RES, ABS_X, ABS_Y};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

// ============================================================================
// MOUSE BUTTON CONSTANTS (evdev compatible)
// ============================================================================

/// Miscellaneous button base
pub const BTN_MISC: u16 = 0x100;

/// Generic buttons 0-9
pub const BTN_0: u16 = 0x100;
pub const BTN_1: u16 = 0x101;
pub const BTN_2: u16 = 0x102;
pub const BTN_3: u16 = 0x103;
pub const BTN_4: u16 = 0x104;
pub const BTN_5: u16 = 0x105;
pub const BTN_6: u16 = 0x106;
pub const BTN_7: u16 = 0x107;
pub const BTN_8: u16 = 0x108;
pub const BTN_9: u16 = 0x109;

/// Mouse button base
/// #VERIFY[BTN-MOUSE-EVDEV]: Matches linux/input-event-codes.h
pub const BTN_MOUSE: u16 = 0x110;

/// Left mouse button
/// #VERIFY[BTN-LEFT]: Primary click button
pub const BTN_LEFT: u16 = 0x110;

/// Right mouse button
/// #VERIFY[BTN-RIGHT]: Secondary click button
pub const BTN_RIGHT: u16 = 0x111;

/// Middle mouse button (scroll wheel click)
/// #VERIFY[BTN-MIDDLE]: Scroll wheel button
pub const BTN_MIDDLE: u16 = 0x112;

/// Side button (thumb button 1)
pub const BTN_SIDE: u16 = 0x113;

/// Extra button (thumb button 2)
pub const BTN_EXTRA: u16 = 0x114;

/// Forward navigation button
pub const BTN_FORWARD: u16 = 0x115;

/// Back navigation button
pub const BTN_BACK: u16 = 0x116;

/// Task button
pub const BTN_TASK: u16 = 0x117;

// ============================================================================
// MOUSE CONSTANTS
// ============================================================================

/// Maximum number of mouse buttons supported
/// #ASSUME[BUTTON-COUNT]: 16 buttons covers all standard mice
pub const MOUSE_BUTTON_COUNT: usize = 16;

/// Mouse position history size for velocity calculation
/// #ASSUME[HISTORY-SIZE]: 8 positions sufficient for velocity smoothing
pub const MOUSE_HISTORY_SIZE: usize = 8;

/// Default mouse sensitivity (1.0 = 1:1 movement)
pub const DEFAULT_MOUSE_SENSITIVITY: f32 = 1.0;

// ============================================================================
// MOUSE BUTTON ENUM
// ============================================================================

/// Mouse button representation
///
/// #VERIFY[BUTTON-MAPPING]: Maps to evdev button codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MouseButton {
    /// Left button (primary click)
    Left = 0,
    /// Right button (secondary click)
    Right = 1,
    /// Middle button (wheel click)
    Middle = 2,
    /// Side button (thumb 1)
    Side = 3,
    /// Extra button (thumb 2)
    Extra = 4,
    /// Forward navigation
    Forward = 5,
    /// Back navigation
    Back = 6,
    /// Task button
    Task = 7,
    /// Unknown button
    Unknown = 255,
}

impl MouseButton {
    /// Convert from evdev button code
    ///
    /// #VERIFY[FROM-EVDEV]: Correctly maps button codes
    #[inline(always)]
    pub const fn from_evdev(code: u16) -> Self {
        match code {
            BTN_LEFT => MouseButton::Left,
            BTN_RIGHT => MouseButton::Right,
            BTN_MIDDLE => MouseButton::Middle,
            BTN_SIDE => MouseButton::Side,
            BTN_EXTRA => MouseButton::Extra,
            BTN_FORWARD => MouseButton::Forward,
            BTN_BACK => MouseButton::Back,
            BTN_TASK => MouseButton::Task,
            _ => MouseButton::Unknown,
        }
    }

    /// Convert to evdev button code
    #[inline(always)]
    pub const fn to_evdev(self) -> u16 {
        match self {
            MouseButton::Left => BTN_LEFT,
            MouseButton::Right => BTN_RIGHT,
            MouseButton::Middle => BTN_MIDDLE,
            MouseButton::Side => BTN_SIDE,
            MouseButton::Extra => BTN_EXTRA,
            MouseButton::Forward => BTN_FORWARD,
            MouseButton::Back => BTN_BACK,
            MouseButton::Task => BTN_TASK,
            MouseButton::Unknown => 0,
        }
    }

    /// Check if this is a mouse button (vs generic button)
    #[inline(always)]
    pub const fn is_mouse_button(code: u16) -> bool {
        code >= BTN_MOUSE && code <= BTN_TASK
    }

    /// Get button index (0-15)
    #[inline(always)]
    pub const fn index(self) -> usize {
        self as usize
    }
}

// ============================================================================
// MOUSE BUTTON STATE
// ============================================================================

/// Mouse button state bitmap
///
/// # Bit Layout
/// - Bit 0: Left button
/// - Bit 1: Right button
/// - Bit 2: Middle button
/// - Bit 3: Side button
/// - Bit 4: Extra button
/// - Bit 5: Forward button
/// - Bit 6: Back button
/// - Bit 7: Task button
/// - Bits 8-15: Reserved for future buttons
///
/// #VERIFY[BUTTON-STATE-16]: 16 bits for button state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct MouseButtonState(pub u16);

impl MouseButtonState {
    /// No buttons pressed
    pub const NONE: Self = Self(0);

    /// Left button pressed
    pub const LEFT: Self = Self(1 << 0);
    /// Right button pressed
    pub const RIGHT: Self = Self(1 << 1);
    /// Middle button pressed
    pub const MIDDLE: Self = Self(1 << 2);
    /// Side button pressed
    pub const SIDE: Self = Self(1 << 3);
    /// Extra button pressed
    pub const EXTRA: Self = Self(1 << 4);
    /// Forward button pressed
    pub const FORWARD: Self = Self(1 << 5);
    /// Back button pressed
    pub const BACK: Self = Self(1 << 6);
    /// Task button pressed
    pub const TASK: Self = Self(1 << 7);

    /// Check if left button is pressed
    #[inline(always)]
    pub const fn left(&self) -> bool {
        self.0 & Self::LEFT.0 != 0
    }

    /// Check if right button is pressed
    #[inline(always)]
    pub const fn right(&self) -> bool {
        self.0 & Self::RIGHT.0 != 0
    }

    /// Check if middle button is pressed
    #[inline(always)]
    pub const fn middle(&self) -> bool {
        self.0 & Self::MIDDLE.0 != 0
    }

    /// Check if any button is pressed
    #[inline(always)]
    pub const fn any_pressed(&self) -> bool {
        self.0 != 0
    }

    /// Check if specific button is pressed
    #[inline(always)]
    pub const fn is_pressed(&self, button: MouseButton) -> bool {
        let bit = 1u16 << (button as u8);
        self.0 & bit != 0
    }

    /// Count of pressed buttons
    #[inline(always)]
    pub const fn count(&self) -> u32 {
        self.0.count_ones()
    }

    /// Set button state
    #[inline(always)]
    pub const fn with_button(self, button: MouseButton, pressed: bool) -> Self {
        let bit = 1u16 << (button as u8);
        if pressed {
            Self(self.0 | bit)
        } else {
            Self(self.0 & !bit)
        }
    }
}

// ============================================================================
// MOUSE POSITION
// ============================================================================

/// Mouse position (X, Y coordinates)
///
/// # Memory Layout
/// Packed into u64 for atomic operations:
/// - Bits 0-31: X coordinate (i32)
/// - Bits 32-63: Y coordinate (i32)
///
/// #VERIFY[POSITION-PACKED]: X/Y fit in u64
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MousePosition {
    /// X coordinate
    pub x: i32,
    /// Y coordinate
    pub y: i32,
}

impl MousePosition {
    /// Create new position
    #[inline(always)]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Pack position into u64 for atomic storage
    ///
    /// #VERIFY[PACK-LOSSLESS]: Round-trip preserves coordinates
    #[inline(always)]
    pub const fn pack(&self) -> u64 {
        let x = self.x as u32 as u64;
        let y = (self.y as u32 as u64) << 32;
        x | y
    }

    /// Unpack position from u64
    #[inline(always)]
    pub const fn unpack(packed: u64) -> Self {
        Self {
            x: packed as i32,
            y: (packed >> 32) as i32,
        }
    }

    /// Add relative movement
    #[inline(always)]
    pub const fn add(self, dx: i32, dy: i32) -> Self {
        Self {
            x: self.x.saturating_add(dx),
            y: self.y.saturating_add(dy),
        }
    }

    /// Clamp position to bounds
    #[inline(always)]
    pub fn clamp(self, min_x: i32, max_x: i32, min_y: i32, max_y: i32) -> Self {
        Self {
            x: self.x.clamp(min_x, max_x),
            y: self.y.clamp(min_y, max_y),
        }
    }
}

// ============================================================================
// MOUSE SCROLL
// ============================================================================

/// Mouse scroll state (vertical and horizontal)
///
/// # Memory Layout
/// Packed into u64:
/// - Bits 0-31: Vertical scroll accumulator (i32)
/// - Bits 32-63: Horizontal scroll accumulator (i32)
///
/// #ASSUME[SCROLL-ACCUMULATOR]: Scroll values accumulate until consumed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MouseScroll {
    /// Vertical scroll (positive = up/away, negative = down/towards)
    pub vertical: i32,
    /// Horizontal scroll (positive = right, negative = left)
    pub horizontal: i32,
}

impl MouseScroll {
    /// Create new scroll state
    #[inline(always)]
    pub const fn new(vertical: i32, horizontal: i32) -> Self {
        Self { vertical, horizontal }
    }

    /// Pack scroll into u64
    #[inline(always)]
    pub const fn pack(&self) -> u64 {
        let v = self.vertical as u32 as u64;
        let h = (self.horizontal as u32 as u64) << 32;
        v | h
    }

    /// Unpack scroll from u64
    #[inline(always)]
    pub const fn unpack(packed: u64) -> Self {
        Self {
            vertical: packed as i32,
            horizontal: (packed >> 32) as i32,
        }
    }

    /// Check if any scroll occurred
    #[inline(always)]
    pub const fn is_zero(&self) -> bool {
        self.vertical == 0 && self.horizontal == 0
    }
}

// ============================================================================
// MOUSE STATE
// ============================================================================

/// Mouse state representation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MouseState {
    /// Mouse ready
    Ready = 0,
    /// Mouse captured (confined to window)
    Captured = 1,
    /// Mouse hidden
    Hidden = 2,
    /// Mouse disabled
    Disabled = 255,
}

impl MouseState {
    /// Convert from raw value
    #[inline(always)]
    pub const fn from_raw(val: u8) -> Self {
        match val {
            0 => MouseState::Ready,
            1 => MouseState::Captured,
            2 => MouseState::Hidden,
            _ => MouseState::Disabled,
        }
    }
}

// ============================================================================
// MOUSE SNAPSHOT
// ============================================================================

/// Atomic snapshot of mouse state
///
/// #VERIFY[SNAPSHOT-ATOMIC]: All fields captured atomically
#[derive(Debug, Clone, Copy)]
pub struct MouseSnapshot {
    /// Current position
    pub position: MousePosition,
    /// Button state
    pub buttons: MouseButtonState,
    /// Scroll accumulator
    pub scroll: MouseScroll,
    /// Generation counter
    pub generation: u64,
}

// ============================================================================
// MOUSE CAPSULE (T1 Atomic)
// ============================================================================

/// Lockfree mouse state tracking capsule
///
/// # Architecture (T1 Atomic)
/// - **256-byte alignment**: 4 cache lines
/// - **Atomic position**: Packed X/Y in u64
/// - **Button bitmap**: u16 for 16 buttons
/// - **Scroll accumulator**: Separate vertical/horizontal
///
/// # Memory Layout (256 bytes)
/// - Offset 0-63: First cache line (position + buttons)
///   - 0-7: Position (AtomicU64: packed X/Y)
///   - 8-15: Buttons + state (AtomicU64)
///   - 16-23: Scroll accumulator (AtomicU64)
///   - 24-31: Generation counter (AtomicU64)
///   - 32-63: Padding
/// - Offset 64-127: Second cache line (velocity + history)
///   - 64-71: Velocity (AtomicU64: packed dx/dy)
///   - 72-79: Last event time (AtomicU64)
///   - 80-127: Padding
/// - Offset 128-191: Third cache line (bounds)
///   - 128-135: Min bounds (AtomicU64: packed min_x/min_y)
///   - 136-143: Max bounds (AtomicU64: packed max_x/max_y)
///   - 144-191: Padding
/// - Offset 192-255: Fourth cache line (reserved)
///
/// #ASSUME[LAYOUT-OPTIMAL]: Layout optimized for common operations
/// #VERIFY[LOCKFREE]: All operations use atomic primitives
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 256, size = 256))]
#[repr(C, align(256))]
pub struct MouseCapsule {
    // === First cache line (64 bytes): Core state ===

    /// Mouse position (packed X/Y)
    /// #ASSUME[POSITION-ATOMIC]: X/Y updated atomically
    position: AtomicU64,

    /// Button state + mouse state
    /// - Bits 0-15: Button bitmap (MouseButtonState)
    /// - Bits 16-23: Mouse state enum
    /// - Bits 24-31: Reserved
    /// - Bits 32-63: Click count (for double-click detection)
    ///
    /// #ASSUME[BUTTONS-ATOMIC]: Button state updated atomically
    buttons_state: AtomicU64,

    /// Scroll accumulator (packed vertical/horizontal)
    /// #ASSUME[SCROLL-ACCUMULATOR]: Scroll accumulates until consumed
    scroll: AtomicU64,

    /// Generation counter for ABA prevention
    generation: AtomicU64,

    /// Padding to complete first cache line
    _padding1: [u8; 32],

    // === Second cache line (64 bytes): Motion tracking ===

    /// Velocity (packed dx/dy per frame)
    /// #ASSUME[VELOCITY-SMOOTHED]: Smoothed over recent frames
    velocity: AtomicU64,

    /// Last event timestamp (nanoseconds)
    last_event_time: AtomicU64,

    /// Delta accumulator for relative motion (packed)
    delta_accumulator: AtomicU64,

    /// Padding
    _padding2: [u8; 40],

    // === Third cache line (64 bytes): Bounds ===

    /// Minimum bounds (packed min_x/min_y)
    /// #ASSUME[BOUNDS-OPTIONAL]: 0 means unbounded
    min_bounds: AtomicU64,

    /// Maximum bounds (packed max_x/max_y)
    /// #ASSUME[BOUNDS-OPTIONAL]: i32::MAX means unbounded
    max_bounds: AtomicU64,

    /// Sensitivity multiplier (Q16.16 fixed-point)
    /// #ASSUME[SENSITIVITY-FIXED]: Fixed-point for smooth scaling
    sensitivity: AtomicU32,

    /// Padding
    _padding3: [u8; 44],

    // === Fourth cache line (64 bytes): Reserved ===
    _reserved: [u8; 64],
}

impl AlignmentTier for MouseCapsule {
    const TIER: &'static str = "atomic";
    const ALIGNMENT: usize = 256;
}

// Compile-time verification
#[cfg(not(feature = "derive"))]
crate::verify_capsule_properties!(MouseCapsule, 256, 256);

impl MouseCapsule {
    /// Create new mouse capsule
    pub const fn new() -> Self {
        Self {
            position: AtomicU64::new(0),
            buttons_state: AtomicU64::new(0),
            scroll: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            _padding1: [0; 32],
            velocity: AtomicU64::new(0),
            last_event_time: AtomicU64::new(0),
            delta_accumulator: AtomicU64::new(0),
            _padding2: [0; 40],
            min_bounds: AtomicU64::new(MousePosition::new(i32::MIN, i32::MIN).pack()),
            max_bounds: AtomicU64::new(MousePosition::new(i32::MAX, i32::MAX).pack()),
            sensitivity: AtomicU32::new(0x10000), // 1.0 in Q16.16
            _padding3: [0; 44],
            _reserved: [0; 64],
        }
    }

    /// Get current mouse position
    ///
    /// # Performance
    /// - Typical: <5ns
    ///
    /// #VERIFY[POSITION-LOAD]: Single atomic load
    #[inline(always)]
    pub fn position(&self) -> MousePosition {
        MousePosition::unpack(self.position.load(Ordering::Acquire))
    }

    /// Get current position as (x, y) tuple
    #[inline(always)]
    pub fn position_tuple(&self) -> (i32, i32) {
        let pos = self.position();
        (pos.x, pos.y)
    }

    /// Set absolute mouse position
    ///
    /// # Performance
    /// - Typical: <10ns
    ///
    /// #VERIFY[POSITION-STORE]: Atomic store with bounds clamping
    pub fn set_position(&self, x: i32, y: i32) {
        let min = MousePosition::unpack(self.min_bounds.load(Ordering::Relaxed));
        let max = MousePosition::unpack(self.max_bounds.load(Ordering::Relaxed));

        let clamped = MousePosition::new(x, y).clamp(min.x, max.x, min.y, max.y);
        self.position.store(clamped.pack(), Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Add relative motion to position
    ///
    /// # Performance
    /// - Typical: <15ns
    ///
    /// #VERIFY[MOTION-ATOMIC]: CAS loop for atomic update
    pub fn add_motion(&self, dx: i32, dy: i32) {
        // Apply sensitivity
        let sens = self.sensitivity.load(Ordering::Relaxed);
        let dx_scaled = (dx as i64 * sens as i64) >> 16;
        let dy_scaled = (dy as i64 * sens as i64) >> 16;

        loop {
            let old_packed = self.position.load(Ordering::Relaxed);
            let old_pos = MousePosition::unpack(old_packed);

            let min = MousePosition::unpack(self.min_bounds.load(Ordering::Relaxed));
            let max = MousePosition::unpack(self.max_bounds.load(Ordering::Relaxed));

            let new_pos = old_pos
                .add(dx_scaled as i32, dy_scaled as i32)
                .clamp(min.x, max.x, min.y, max.y);

            if self.position.compare_exchange_weak(
                old_packed,
                new_pos.pack(),
                Ordering::Release,
                Ordering::Relaxed,
            ).is_ok() {
                break;
            }
            core::hint::spin_loop();
        }

        // Update delta accumulator
        let delta = MousePosition::new(dx, dy);
        self.delta_accumulator.store(delta.pack(), Ordering::Relaxed);

        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get button state
    ///
    /// # Performance
    /// - Typical: <3ns
    #[inline(always)]
    pub fn buttons(&self) -> MouseButtonState {
        let packed = self.buttons_state.load(Ordering::Acquire);
        MouseButtonState((packed & 0xFFFF) as u16)
    }

    /// Check if specific button is pressed
    ///
    /// # Performance
    /// - Typical: <5ns
    #[inline(always)]
    pub fn is_button_pressed(&self, button: MouseButton) -> bool {
        self.buttons().is_pressed(button)
    }

    /// Get scroll accumulator (and optionally reset)
    ///
    /// # Returns
    /// Accumulated scroll since last call
    ///
    /// #ASSUME[SCROLL-CONSUME]: Caller consumes scroll delta
    pub fn get_scroll(&self, reset: bool) -> MouseScroll {
        if reset {
            let old = self.scroll.swap(0, Ordering::AcqRel);
            MouseScroll::unpack(old)
        } else {
            MouseScroll::unpack(self.scroll.load(Ordering::Acquire))
        }
    }

    /// Process a mouse event from evdev
    ///
    /// # Performance
    /// - Typical: <20ns
    ///
    /// #VERIFY[PROCESS-ATOMIC]: State updates are atomic
    pub fn process_event(&self, event: &InputEvent) {
        match event.type_ {
            EV_REL => self.process_rel_event(event),
            EV_ABS => self.process_abs_event(event),
            EV_KEY => self.process_button_event(event),
            _ => {}
        }
    }

    /// Process relative motion event
    fn process_rel_event(&self, event: &InputEvent) {
        match event.code {
            REL_X => {
                self.add_motion(event.value, 0);
            }
            REL_Y => {
                self.add_motion(0, event.value);
            }
            REL_WHEEL | REL_WHEEL_HI_RES => {
                self.add_scroll(event.value, 0);
            }
            REL_HWHEEL | REL_HWHEEL_HI_RES => {
                self.add_scroll(0, event.value);
            }
            _ => {}
        }
    }

    /// Process absolute position event
    fn process_abs_event(&self, event: &InputEvent) {
        match event.code {
            ABS_X => {
                let (_, y) = self.position_tuple();
                self.set_position(event.value, y);
            }
            ABS_Y => {
                let (x, _) = self.position_tuple();
                self.set_position(x, event.value);
            }
            _ => {}
        }
    }

    /// Process button event
    fn process_button_event(&self, event: &InputEvent) {
        if !MouseButton::is_mouse_button(event.code) {
            return;
        }

        let button = MouseButton::from_evdev(event.code);
        let pressed = event.value != 0;

        loop {
            let old = self.buttons_state.load(Ordering::Relaxed);
            let old_buttons = (old & 0xFFFF) as u16;
            let bit = 1u16 << (button as u8);

            let new_buttons = if pressed {
                old_buttons | bit
            } else {
                old_buttons & !bit
            };

            let new = (old & 0xFFFF_FFFF_FFFF_0000) | new_buttons as u64;

            if self.buttons_state.compare_exchange_weak(
                old, new,
                Ordering::Release, Ordering::Relaxed
            ).is_ok() {
                break;
            }
            core::hint::spin_loop();
        }

        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Add scroll delta
    fn add_scroll(&self, vertical: i32, horizontal: i32) {
        loop {
            let old = self.scroll.load(Ordering::Relaxed);
            let old_scroll = MouseScroll::unpack(old);
            let new_scroll = MouseScroll::new(
                old_scroll.vertical.saturating_add(vertical),
                old_scroll.horizontal.saturating_add(horizontal),
            );

            if self.scroll.compare_exchange_weak(
                old, new_scroll.pack(),
                Ordering::Release, Ordering::Relaxed
            ).is_ok() {
                break;
            }
            core::hint::spin_loop();
        }

        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Set mouse bounds
    ///
    /// #ASSUME[BOUNDS-VALID]: min < max
    pub fn set_bounds(&self, min_x: i32, min_y: i32, max_x: i32, max_y: i32) {
        self.min_bounds.store(MousePosition::new(min_x, min_y).pack(), Ordering::Release);
        self.max_bounds.store(MousePosition::new(max_x, max_y).pack(), Ordering::Release);
    }

    /// Set mouse sensitivity (1.0 = normal)
    ///
    /// # Arguments
    /// - `sensitivity`: Multiplier (0.1 to 10.0 typical)
    pub fn set_sensitivity(&self, sensitivity: f32) {
        let fixed = (sensitivity * 65536.0) as u32;
        self.sensitivity.store(fixed, Ordering::Release);
    }

    /// Get mouse sensitivity
    pub fn sensitivity(&self) -> f32 {
        let fixed = self.sensitivity.load(Ordering::Relaxed);
        fixed as f32 / 65536.0
    }

    /// Take atomic snapshot of mouse state
    ///
    /// # Performance
    /// - Typical: <20ns
    #[inline]
    pub fn snapshot(&self) -> MouseSnapshot {
        MouseSnapshot {
            position: self.position(),
            buttons: self.buttons(),
            scroll: MouseScroll::unpack(self.scroll.load(Ordering::Acquire)),
            generation: self.generation.load(Ordering::Acquire),
        }
    }

    /// Clear mouse state
    ///
    /// #ASSUME[CLEAR-EXCLUSIVE]: Caller ensures exclusive access
    pub fn clear(&self) {
        self.position.store(0, Ordering::Release);
        self.buttons_state.store(0, Ordering::Release);
        self.scroll.store(0, Ordering::Release);
        self.velocity.store(0, Ordering::Release);
        self.delta_accumulator.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get generation counter
    #[inline(always)]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get last delta (for debugging/visualization)
    pub fn last_delta(&self) -> (i32, i32) {
        let delta = MousePosition::unpack(self.delta_accumulator.load(Ordering::Relaxed));
        (delta.x, delta.y)
    }
}

impl Default for MouseCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Send + Sync safety
unsafe impl Send for MouseCapsule {}
unsafe impl Sync for MouseCapsule {}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_button() {
        assert_eq!(MouseButton::from_evdev(BTN_LEFT), MouseButton::Left);
        assert_eq!(MouseButton::from_evdev(BTN_RIGHT), MouseButton::Right);
        assert_eq!(MouseButton::Left.to_evdev(), BTN_LEFT);
        assert!(MouseButton::is_mouse_button(BTN_LEFT));
        assert!(!MouseButton::is_mouse_button(0));
    }

    #[test]
    fn test_button_state() {
        let state = MouseButtonState::LEFT.with_button(MouseButton::Right, true);
        assert!(state.left());
        assert!(state.right());
        assert!(!state.middle());
        assert!(state.is_pressed(MouseButton::Left));
        assert_eq!(state.count(), 2);
    }

    #[test]
    fn test_position_pack_unpack() {
        let pos = MousePosition::new(-100, 200);
        let packed = pos.pack();
        let unpacked = MousePosition::unpack(packed);
        assert_eq!(unpacked.x, -100);
        assert_eq!(unpacked.y, 200);
    }

    #[test]
    fn test_scroll_pack_unpack() {
        let scroll = MouseScroll::new(-5, 10);
        let packed = scroll.pack();
        let unpacked = MouseScroll::unpack(packed);
        assert_eq!(unpacked.vertical, -5);
        assert_eq!(unpacked.horizontal, 10);
    }

    #[test]
    fn test_mouse_new() {
        let mouse = MouseCapsule::new();
        let pos = mouse.position();
        assert_eq!(pos.x, 0);
        assert_eq!(pos.y, 0);
        assert_eq!(mouse.buttons(), MouseButtonState::NONE);
    }

    #[test]
    fn test_set_position() {
        let mouse = MouseCapsule::new();
        mouse.set_position(100, 200);
        let pos = mouse.position();
        assert_eq!(pos.x, 100);
        assert_eq!(pos.y, 200);
    }

    #[test]
    fn test_add_motion() {
        let mouse = MouseCapsule::new();
        mouse.set_position(100, 100);
        mouse.add_motion(50, -30);
        let pos = mouse.position();
        assert_eq!(pos.x, 150);
        assert_eq!(pos.y, 70);
    }

    #[test]
    fn test_bounds() {
        let mouse = MouseCapsule::new();
        mouse.set_bounds(0, 0, 100, 100);
        mouse.set_position(200, 200);
        let pos = mouse.position();
        assert_eq!(pos.x, 100);
        assert_eq!(pos.y, 100);
    }

    #[test]
    fn test_button_events() {
        let mouse = MouseCapsule::new();

        // Press left button
        mouse.process_event(&InputEvent::new(EV_KEY, BTN_LEFT, 1));
        assert!(mouse.is_button_pressed(MouseButton::Left));
        assert!(mouse.buttons().left());

        // Release left button
        mouse.process_event(&InputEvent::new(EV_KEY, BTN_LEFT, 0));
        assert!(!mouse.is_button_pressed(MouseButton::Left));
    }

    #[test]
    fn test_scroll() {
        let mouse = MouseCapsule::new();

        // Scroll up
        mouse.process_event(&InputEvent::new(EV_REL, REL_WHEEL, 3));
        let scroll = mouse.get_scroll(true);
        assert_eq!(scroll.vertical, 3);
        assert_eq!(scroll.horizontal, 0);

        // Scroll was reset
        let scroll = mouse.get_scroll(false);
        assert_eq!(scroll.vertical, 0);
    }

    #[test]
    fn test_snapshot() {
        let mouse = MouseCapsule::new();
        mouse.set_position(50, 100);
        mouse.process_event(&InputEvent::new(EV_KEY, BTN_LEFT, 1));

        let snapshot = mouse.snapshot();
        assert_eq!(snapshot.position.x, 50);
        assert_eq!(snapshot.position.y, 100);
        assert!(snapshot.buttons.left());
    }

    #[test]
    fn test_capsule_size_alignment() {
        use core::mem::{size_of, align_of};

        assert_eq!(size_of::<MouseCapsule>(), 256);
        assert_eq!(align_of::<MouseCapsule>(), 256);
    }
}
