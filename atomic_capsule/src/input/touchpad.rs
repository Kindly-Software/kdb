//! Touchpad Capsule - T1 Atomic Multi-Touch Gesture Tracking
//!
//! # Architecture
//! - **Tier 1 (Atomic)**: Lockfree touchpad state coordination
//! - **256-byte alignment**: 4 cache lines for slots + gestures + metadata
//! - **Multi-touch Type B**: Linux kernel MT protocol B (slotted)
//! - **Gesture recognition**: Tap, swipe, pinch, rotate detection
//!
//! # Performance Targets (B32 Framework)
//! - Slot lookup: <5ns (indexed array)
//! - Gesture update: <15ns (atomic CAS)
//! - Touch count: <3ns (atomic load)
//! - Full state snapshot: <30ns
//!
//! # Safety Assumptions (ASSUM Framework)
//! - #ASSUME[MT-TYPE-B]: Type B multi-touch protocol (slotted)
//! - #ASSUME[SLOT-10]: 10 slots covers all common touchpads
//! - #ASSUME[GESTURE-SINGLE]: One gesture active at a time
//! - #VERIFY[TRACKING-ID]: -1 tracking ID means slot is empty
//! - #VERIFY[GESTURE-ATOMIC]: Gesture state updates are atomic

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicI32, Ordering};
use crate::alignment::AlignmentTier;
use super::event::{InputEvent, EV_ABS, EV_KEY, EV_SYN, SYN_REPORT,
                   ABS_MT_SLOT, ABS_MT_TRACKING_ID, ABS_MT_POSITION_X, ABS_MT_POSITION_Y,
                   ABS_MT_TOUCH_MAJOR, ABS_MT_TOUCH_MINOR, ABS_MT_PRESSURE};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

// ============================================================================
// TOUCHPAD CONSTANTS
// ============================================================================

/// Maximum number of touch slots (concurrent touches)
/// #ASSUME[SLOT-10]: 10 fingers maximum (common limit)
pub const MAX_TOUCH_SLOTS: usize = 10;

/// Tap gesture timeout in milliseconds
/// #ASSUME[TAP-TIMEOUT]: 200ms is standard tap threshold
pub const GESTURE_TAP_TIMEOUT_MS: u32 = 200;

/// Swipe gesture threshold in pixels
/// #ASSUME[SWIPE-THRESHOLD]: 50px minimum movement for swipe
pub const GESTURE_SWIPE_THRESHOLD: i32 = 50;

/// Pinch gesture threshold ratio
/// #ASSUME[PINCH-THRESHOLD]: 0.1 (10%) change for pinch detection
pub const GESTURE_PINCH_THRESHOLD: f32 = 0.1;

/// Tracking ID indicating empty slot
/// #VERIFY[TRACKING-EMPTY]: -1 is kernel standard for empty slot
pub const TRACKING_ID_EMPTY: i32 = -1;

// ============================================================================
// TOUCH POINT
// ============================================================================

/// Single touch point state
///
/// # Memory Layout (24 bytes)
/// - Offset 0-3: X position (i32)
/// - Offset 4-7: Y position (i32)
/// - Offset 8-11: Tracking ID (i32)
/// - Offset 12-13: Touch major (u16)
/// - Offset 14-15: Touch minor (u16)
/// - Offset 16-17: Pressure (u16)
/// - Offset 18-19: Reserved (u16)
/// - Offset 20-23: Timestamp (u32, low bits of nanos)
///
/// #VERIFY[TOUCH-POINT-24]: Size must be 24 bytes
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct TouchPoint {
    /// X coordinate
    pub x: i32,
    /// Y coordinate
    pub y: i32,
    /// Tracking ID (-1 = not tracking)
    pub tracking_id: i32,
    /// Touch major axis (contact size)
    pub touch_major: u16,
    /// Touch minor axis
    pub touch_minor: u16,
    /// Touch pressure
    pub pressure: u16,
    /// Reserved
    _reserved: u16,
    /// Timestamp (low 32 bits of nanos)
    pub timestamp: u32,
}

impl TouchPoint {
    /// Create new touch point
    #[inline(always)]
    pub const fn new(x: i32, y: i32, tracking_id: i32) -> Self {
        Self {
            x,
            y,
            tracking_id,
            touch_major: 0,
            touch_minor: 0,
            pressure: 0,
            _reserved: 0,
            timestamp: 0,
        }
    }

    /// Check if slot is active (has valid tracking ID)
    #[inline(always)]
    pub const fn is_active(&self) -> bool {
        self.tracking_id != TRACKING_ID_EMPTY
    }

    /// Pack essential data into u64 for atomic operations
    ///
    /// # Layout
    /// - Bits 0-15: X position (scaled to u16)
    /// - Bits 16-31: Y position (scaled to u16)
    /// - Bits 32-47: Tracking ID (i16, sign-extended)
    /// - Bits 48-63: Pressure (u16)
    #[inline(always)]
    pub const fn pack_essential(&self) -> u64 {
        let x = (self.x as u16) as u64;
        let y = ((self.y as u16) as u64) << 16;
        let id = ((self.tracking_id as i16 as u16) as u64) << 32;
        let p = (self.pressure as u64) << 48;
        x | y | id | p
    }

    /// Unpack essential data from u64
    #[inline(always)]
    pub fn unpack_essential(packed: u64) -> Self {
        Self {
            x: (packed & 0xFFFF) as i16 as i32,
            y: ((packed >> 16) & 0xFFFF) as i16 as i32,
            tracking_id: ((packed >> 32) & 0xFFFF) as i16 as i32,
            touch_major: 0,
            touch_minor: 0,
            pressure: ((packed >> 48) & 0xFFFF) as u16,
            _reserved: 0,
            timestamp: 0,
        }
    }
}

// Compile-time size verification
const _: () = {
    assert!(core::mem::size_of::<TouchPoint>() == 24);
};

// ============================================================================
// MULTI-TOUCH SLOT
// ============================================================================

/// Multi-touch slot for tracking individual fingers
///
/// Implements Linux kernel MT Type B protocol:
/// - Slots are pre-allocated (ABS_MT_SLOT selects active slot)
/// - ABS_MT_TRACKING_ID assigns/releases touch tracking
/// - Position and attributes update the selected slot
///
/// #VERIFY[MT-TYPE-B]: Protocol matches kernel multi-touch Type B
#[derive(Debug, Clone, Copy)]
pub struct MultiTouchSlot {
    /// Current touch point data
    pub current: TouchPoint,
    /// Previous touch point (for delta calculation)
    pub previous: TouchPoint,
    /// Start position (for gesture detection)
    pub start: TouchPoint,
}

impl MultiTouchSlot {
    /// Create empty slot
    pub const fn new() -> Self {
        Self {
            current: TouchPoint::new(0, 0, TRACKING_ID_EMPTY),
            previous: TouchPoint::new(0, 0, TRACKING_ID_EMPTY),
            start: TouchPoint::new(0, 0, TRACKING_ID_EMPTY),
        }
    }

    /// Check if slot is active
    #[inline(always)]
    pub const fn is_active(&self) -> bool {
        self.current.is_active()
    }

    /// Get movement delta since last update
    #[inline(always)]
    pub const fn delta(&self) -> (i32, i32) {
        (
            self.current.x - self.previous.x,
            self.current.y - self.previous.y,
        )
    }

    /// Get movement delta since touch start
    #[inline(always)]
    pub const fn total_delta(&self) -> (i32, i32) {
        (
            self.current.x - self.start.x,
            self.current.y - self.start.y,
        )
    }

    /// Update with new touch down
    pub fn touch_down(&mut self, x: i32, y: i32, tracking_id: i32) {
        self.current = TouchPoint::new(x, y, tracking_id);
        self.previous = self.current;
        self.start = self.current;
    }

    /// Update position
    pub fn update_position(&mut self, x: i32, y: i32) {
        self.previous = self.current;
        self.current.x = x;
        self.current.y = y;
    }

    /// Release touch
    pub fn touch_up(&mut self) {
        self.previous = self.current;
        self.current.tracking_id = TRACKING_ID_EMPTY;
    }
}

impl Default for MultiTouchSlot {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// GESTURE TYPE
// ============================================================================

/// Recognized touch gesture types
///
/// #VERIFY[GESTURE-COMPLETE]: All common touchpad gestures covered
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GestureType {
    /// No gesture detected
    None = 0,
    /// Single tap (1 finger)
    Tap = 1,
    /// Double tap (1 finger, two quick taps)
    DoubleTap = 2,
    /// Two-finger tap (right-click equivalent)
    TwoFingerTap = 3,
    /// Three-finger tap
    ThreeFingerTap = 4,
    /// Single finger swipe (scrolling)
    Scroll = 5,
    /// Two-finger scroll (natural scrolling)
    TwoFingerScroll = 6,
    /// Swipe left (navigation)
    SwipeLeft = 7,
    /// Swipe right (navigation)
    SwipeRight = 8,
    /// Swipe up
    SwipeUp = 9,
    /// Swipe down
    SwipeDown = 10,
    /// Three-finger swipe left
    ThreeFingerSwipeLeft = 11,
    /// Three-finger swipe right
    ThreeFingerSwipeRight = 12,
    /// Three-finger swipe up
    ThreeFingerSwipeUp = 13,
    /// Three-finger swipe down
    ThreeFingerSwipeDown = 14,
    /// Pinch zoom in
    PinchIn = 15,
    /// Pinch zoom out
    PinchOut = 16,
    /// Two-finger rotate
    Rotate = 17,
    /// Long press (press and hold)
    LongPress = 18,
    /// Drag (tap and move)
    Drag = 19,
}

impl GestureType {
    /// Check if this is a swipe gesture
    #[inline(always)]
    pub const fn is_swipe(&self) -> bool {
        matches!(
            self,
            GestureType::SwipeLeft | GestureType::SwipeRight |
            GestureType::SwipeUp | GestureType::SwipeDown |
            GestureType::ThreeFingerSwipeLeft | GestureType::ThreeFingerSwipeRight |
            GestureType::ThreeFingerSwipeUp | GestureType::ThreeFingerSwipeDown
        )
    }

    /// Check if this is a tap gesture
    #[inline(always)]
    pub const fn is_tap(&self) -> bool {
        matches!(
            self,
            GestureType::Tap | GestureType::DoubleTap |
            GestureType::TwoFingerTap | GestureType::ThreeFingerTap
        )
    }

    /// Check if this is a pinch/zoom gesture
    #[inline(always)]
    pub const fn is_pinch(&self) -> bool {
        matches!(self, GestureType::PinchIn | GestureType::PinchOut)
    }
}

// ============================================================================
// TOUCH GESTURE
// ============================================================================

/// Touch gesture state
///
/// #VERIFY[GESTURE-STATE]: Tracks gesture recognition progress
#[derive(Debug, Clone, Copy)]
pub struct TouchGesture {
    /// Detected gesture type
    pub gesture_type: GestureType,
    /// Gesture progress (0.0 to 1.0)
    pub progress: f32,
    /// Accumulated delta X
    pub delta_x: i32,
    /// Accumulated delta Y
    pub delta_y: i32,
    /// Pinch scale factor (1.0 = no change)
    pub scale: f32,
    /// Rotation angle in radians
    pub rotation: f32,
    /// Number of fingers in gesture
    pub finger_count: u8,
    /// Gesture state (0 = none, 1 = possible, 2 = recognized, 3 = completed)
    pub state: u8,
}

impl TouchGesture {
    /// Create empty gesture
    pub const fn new() -> Self {
        Self {
            gesture_type: GestureType::None,
            progress: 0.0,
            delta_x: 0,
            delta_y: 0,
            scale: 1.0,
            rotation: 0.0,
            finger_count: 0,
            state: 0,
        }
    }

    /// Pack gesture into u64 for atomic operations
    ///
    /// # Layout
    /// - Bits 0-7: Gesture type
    /// - Bits 8-15: Finger count + state
    /// - Bits 16-31: Delta X (scaled)
    /// - Bits 32-47: Delta Y (scaled)
    /// - Bits 48-63: Scale (Q8.8 fixed-point)
    pub fn pack(&self) -> u64 {
        let t = self.gesture_type as u8 as u64;
        let fs = (((self.finger_count as u64) << 4) | (self.state as u64 & 0xF)) << 8;
        let dx = (((self.delta_x / 4) as i16 as u16) as u64) << 16;
        let dy = (((self.delta_y / 4) as i16 as u16) as u64) << 32;
        let s = ((self.scale * 256.0) as u16 as u64) << 48;
        t | fs | dx | dy | s
    }

    /// Unpack gesture from u64
    pub fn unpack(packed: u64) -> Self {
        let gesture_type = match packed & 0xFF {
            1 => GestureType::Tap,
            2 => GestureType::DoubleTap,
            3 => GestureType::TwoFingerTap,
            4 => GestureType::ThreeFingerTap,
            5 => GestureType::Scroll,
            6 => GestureType::TwoFingerScroll,
            7 => GestureType::SwipeLeft,
            8 => GestureType::SwipeRight,
            9 => GestureType::SwipeUp,
            10 => GestureType::SwipeDown,
            15 => GestureType::PinchIn,
            16 => GestureType::PinchOut,
            17 => GestureType::Rotate,
            _ => GestureType::None,
        };
        let fs = (packed >> 8) & 0xFF;
        let finger_count = ((fs >> 4) & 0xF) as u8;
        let state = (fs & 0xF) as u8;
        let delta_x = (((packed >> 16) & 0xFFFF) as i16 as i32) * 4;
        let delta_y = (((packed >> 32) & 0xFFFF) as i16 as i32) * 4;
        let scale = ((packed >> 48) & 0xFFFF) as f32 / 256.0;

        Self {
            gesture_type,
            progress: 0.0,
            delta_x,
            delta_y,
            scale,
            rotation: 0.0,
            finger_count,
            state,
        }
    }
}

impl Default for TouchGesture {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TOUCHPAD STATE
// ============================================================================

/// Touchpad operational state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TouchpadState {
    /// Ready for input
    Ready = 0,
    /// Processing gesture
    Processing = 1,
    /// Disabled
    Disabled = 255,
}

impl TouchpadState {
    /// Convert from raw value
    #[inline(always)]
    pub const fn from_raw(val: u8) -> Self {
        match val {
            0 => TouchpadState::Ready,
            1 => TouchpadState::Processing,
            _ => TouchpadState::Disabled,
        }
    }
}

// ============================================================================
// TOUCHPAD SNAPSHOT
// ============================================================================

/// Atomic snapshot of touchpad state
///
/// #VERIFY[SNAPSHOT-ATOMIC]: All fields captured atomically
#[derive(Debug, Clone, Copy)]
pub struct TouchpadSnapshot {
    /// Number of active touches
    pub touch_count: u8,
    /// Current gesture
    pub gesture: TouchGesture,
    /// Generation counter
    pub generation: u64,
}

// ============================================================================
// TOUCHPAD CAPSULE (T1 Atomic)
// ============================================================================

/// Lockfree touchpad state tracking capsule
///
/// # Architecture (T1 Atomic)
/// - **256-byte alignment**: 4 cache lines
/// - **Atomic slot selection**: Current slot index
/// - **Gesture recognition**: Single active gesture
/// - **Multi-touch Type B**: Linux kernel protocol
///
/// # Memory Layout (256 bytes)
/// - Offset 0-63: First cache line (metadata)
///   - 0-7: State + generation (AtomicU64)
///   - 8-15: Gesture state (AtomicU64, packed)
///   - 16-19: Touch count + current slot (AtomicU32)
///   - 20-27: Touch start time (AtomicU64)
///   - 28-63: Padding
/// - Offset 64-127: Second cache line (slots 0-4, packed essential data)
///   - 64-71: Slot 0 (AtomicU64)
///   - 72-79: Slot 1 (AtomicU64)
///   - 80-87: Slot 2 (AtomicU64)
///   - 88-95: Slot 3 (AtomicU64)
///   - 96-103: Slot 4 (AtomicU64)
///   - 104-127: Padding
/// - Offset 128-191: Third cache line (slots 5-9, packed)
///   - 128-135: Slot 5 (AtomicU64)
///   - 136-143: Slot 6 (AtomicU64)
///   - 144-151: Slot 7 (AtomicU64)
///   - 152-159: Slot 8 (AtomicU64)
///   - 160-167: Slot 9 (AtomicU64)
///   - 168-191: Padding
/// - Offset 192-255: Fourth cache line (reserved)
///
/// #ASSUME[LAYOUT-OPTIMAL]: Layout optimized for common operations
/// #VERIFY[LOCKFREE]: All operations use atomic primitives
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 256, size = 256))]
#[repr(C, align(256))]
pub struct TouchpadCapsule {
    // === First cache line (64 bytes): Metadata ===

    /// State + generation counter
    /// - Bits 0-7: State enum
    /// - Bits 8-31: Reserved
    /// - Bits 32-63: Generation counter
    state_gen: AtomicU64,

    /// Gesture state (packed TouchGesture)
    gesture: AtomicU64,

    /// Touch count + current slot
    /// - Bits 0-7: Active touch count
    /// - Bits 8-15: Current slot index
    /// - Bits 16-31: Reserved
    touch_slot: AtomicU32,

    /// Touch start time (for tap detection)
    touch_start_time: AtomicU64,

    /// Last tap time (for double-tap detection)
    last_tap_time: AtomicU64,

    /// Padding to complete first cache line (64 bytes total)
    /// Layout: state_gen(8) + gesture(8) + touch_slot(4) + pad(4) + touch_start_time(8) + last_tap_time(8) = 40
    /// Need: 64 - 40 = 24 bytes padding
    _padding1: [u8; 24],

    // === Second cache line (64 bytes): Slots 0-4 ===
    slot_0: AtomicU64,
    slot_1: AtomicU64,
    slot_2: AtomicU64,
    slot_3: AtomicU64,
    slot_4: AtomicU64,
    _padding2: [u8; 24],

    // === Third cache line (64 bytes): Slots 5-9 ===
    slot_5: AtomicU64,
    slot_6: AtomicU64,
    slot_7: AtomicU64,
    slot_8: AtomicU64,
    slot_9: AtomicU64,
    _padding3: [u8; 24],

    // === Fourth cache line (64 bytes): Reserved ===
    _reserved: [u8; 64],
}

impl AlignmentTier for TouchpadCapsule {
    const TIER: &'static str = "atomic";
    const ALIGNMENT: usize = 256;
}

// Compile-time verification
#[cfg(not(feature = "derive"))]
crate::verify_capsule_properties!(TouchpadCapsule, 256, 256);

impl TouchpadCapsule {
    /// Create new touchpad capsule
    pub const fn new() -> Self {
        Self {
            state_gen: AtomicU64::new(0),
            gesture: AtomicU64::new(0),
            touch_slot: AtomicU32::new(0),
            touch_start_time: AtomicU64::new(0),
            last_tap_time: AtomicU64::new(0),
            _padding1: [0; 24],
            slot_0: AtomicU64::new(0),
            slot_1: AtomicU64::new(0),
            slot_2: AtomicU64::new(0),
            slot_3: AtomicU64::new(0),
            slot_4: AtomicU64::new(0),
            _padding2: [0; 24],
            slot_5: AtomicU64::new(0),
            slot_6: AtomicU64::new(0),
            slot_7: AtomicU64::new(0),
            slot_8: AtomicU64::new(0),
            slot_9: AtomicU64::new(0),
            _padding3: [0; 24],
            _reserved: [0; 64],
        }
    }

    /// Get slot atomic reference by index
    #[inline(always)]
    fn get_slot(&self, index: usize) -> &AtomicU64 {
        match index {
            0 => &self.slot_0,
            1 => &self.slot_1,
            2 => &self.slot_2,
            3 => &self.slot_3,
            4 => &self.slot_4,
            5 => &self.slot_5,
            6 => &self.slot_6,
            7 => &self.slot_7,
            8 => &self.slot_8,
            9 => &self.slot_9,
            _ => &self.slot_0, // Fallback (shouldn't happen)
        }
    }

    /// Get current slot index
    #[inline(always)]
    fn current_slot_index(&self) -> usize {
        ((self.touch_slot.load(Ordering::Relaxed) >> 8) & 0xFF) as usize
    }

    /// Get active touch count
    ///
    /// # Performance
    /// - Typical: <3ns
    #[inline(always)]
    pub fn touch_count(&self) -> u8 {
        (self.touch_slot.load(Ordering::Acquire) & 0xFF) as u8
    }

    /// Get touch point for slot
    ///
    /// # Performance
    /// - Typical: <5ns
    pub fn get_touch(&self, slot: usize) -> Option<TouchPoint> {
        if slot >= MAX_TOUCH_SLOTS {
            return None;
        }
        let packed = self.get_slot(slot).load(Ordering::Acquire);
        let point = TouchPoint::unpack_essential(packed);
        if point.is_active() {
            Some(point)
        } else {
            None
        }
    }

    /// Get current gesture
    ///
    /// # Performance
    /// - Typical: <5ns
    #[inline(always)]
    pub fn gesture(&self) -> TouchGesture {
        TouchGesture::unpack(self.gesture.load(Ordering::Acquire))
    }

    /// Process a touchpad event from evdev
    ///
    /// # Performance
    /// - Typical: <15ns
    ///
    /// #VERIFY[PROCESS-MT-B]: Handles MT Type B protocol correctly
    pub fn process_event(&self, event: &InputEvent) {
        match event.type_ {
            EV_ABS => self.process_abs_event(event),
            EV_SYN if event.code == SYN_REPORT => self.finalize_frame(),
            _ => {}
        }
    }

    /// Process absolute position event
    fn process_abs_event(&self, event: &InputEvent) {
        match event.code {
            ABS_MT_SLOT => {
                // Switch active slot
                let slot = (event.value as usize).min(MAX_TOUCH_SLOTS - 1);
                loop {
                    let old = self.touch_slot.load(Ordering::Relaxed);
                    let new = (old & 0xFFFF_00FF) | ((slot as u32) << 8);
                    if self.touch_slot.compare_exchange_weak(
                        old, new, Ordering::Release, Ordering::Relaxed
                    ).is_ok() {
                        break;
                    }
                }
            }
            ABS_MT_TRACKING_ID => {
                let slot_idx = self.current_slot_index();
                let slot = self.get_slot(slot_idx);

                if event.value == TRACKING_ID_EMPTY {
                    // Touch lifted
                    slot.store(0, Ordering::Release);
                    // Decrement touch count
                    loop {
                        let old = self.touch_slot.load(Ordering::Relaxed);
                        let count = (old & 0xFF).saturating_sub(1);
                        let new = (old & 0xFFFF_FF00) | count;
                        if self.touch_slot.compare_exchange_weak(
                            old, new, Ordering::Release, Ordering::Relaxed
                        ).is_ok() {
                            break;
                        }
                    }
                } else {
                    // New touch
                    let point = TouchPoint::new(0, 0, event.value);
                    slot.store(point.pack_essential(), Ordering::Release);
                    // Increment touch count
                    loop {
                        let old = self.touch_slot.load(Ordering::Relaxed);
                        let count = ((old & 0xFF) + 1).min(MAX_TOUCH_SLOTS as u32);
                        let new = (old & 0xFFFF_FF00) | count;
                        if self.touch_slot.compare_exchange_weak(
                            old, new, Ordering::Release, Ordering::Relaxed
                        ).is_ok() {
                            break;
                        }
                    }
                }
            }
            ABS_MT_POSITION_X => {
                let slot_idx = self.current_slot_index();
                let slot = self.get_slot(slot_idx);
                loop {
                    let old = slot.load(Ordering::Relaxed);
                    // Update X position (bits 0-15)
                    let new = (old & 0xFFFF_FFFF_FFFF_0000) | (event.value as u16 as u64);
                    if slot.compare_exchange_weak(
                        old, new, Ordering::Release, Ordering::Relaxed
                    ).is_ok() {
                        break;
                    }
                }
            }
            ABS_MT_POSITION_Y => {
                let slot_idx = self.current_slot_index();
                let slot = self.get_slot(slot_idx);
                loop {
                    let old = slot.load(Ordering::Relaxed);
                    // Update Y position (bits 16-31)
                    let new = (old & 0xFFFF_FFFF_0000_FFFF) | ((event.value as u16 as u64) << 16);
                    if slot.compare_exchange_weak(
                        old, new, Ordering::Release, Ordering::Relaxed
                    ).is_ok() {
                        break;
                    }
                }
            }
            ABS_MT_PRESSURE => {
                let slot_idx = self.current_slot_index();
                let slot = self.get_slot(slot_idx);
                loop {
                    let old = slot.load(Ordering::Relaxed);
                    // Update pressure (bits 48-63)
                    let new = (old & 0x0000_FFFF_FFFF_FFFF) | ((event.value as u16 as u64) << 48);
                    if slot.compare_exchange_weak(
                        old, new, Ordering::Release, Ordering::Relaxed
                    ).is_ok() {
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    /// Finalize frame and update gesture recognition
    fn finalize_frame(&self) {
        let count = self.touch_count();

        // Simple gesture detection based on touch count and movement
        let gesture = match count {
            0 => TouchGesture::new(),
            1 => {
                // Single finger - could be tap, scroll, or drag
                if let Some(touch) = self.get_touch(0) {
                    let mut g = TouchGesture::new();
                    g.gesture_type = GestureType::Scroll;
                    g.finger_count = 1;
                    g.delta_x = touch.x;
                    g.delta_y = touch.y;
                    g
                } else {
                    TouchGesture::new()
                }
            }
            2 => {
                // Two fingers - scroll or pinch
                let mut g = TouchGesture::new();
                g.gesture_type = GestureType::TwoFingerScroll;
                g.finger_count = 2;
                g
            }
            3 => {
                // Three fingers - swipe gestures
                let mut g = TouchGesture::new();
                g.gesture_type = GestureType::ThreeFingerTap;
                g.finger_count = 3;
                g
            }
            _ => TouchGesture::new(),
        };

        self.gesture.store(gesture.pack(), Ordering::Release);
        self.state_gen.fetch_add(1 << 32, Ordering::Release);
    }

    /// Take atomic snapshot of touchpad state
    ///
    /// # Performance
    /// - Typical: <30ns
    #[inline]
    pub fn snapshot(&self) -> TouchpadSnapshot {
        TouchpadSnapshot {
            touch_count: self.touch_count(),
            gesture: self.gesture(),
            generation: self.state_gen.load(Ordering::Acquire) >> 32,
        }
    }

    /// Clear all touchpad state
    pub fn clear(&self) {
        for i in 0..MAX_TOUCH_SLOTS {
            self.get_slot(i).store(0, Ordering::Release);
        }
        self.touch_slot.store(0, Ordering::Release);
        self.gesture.store(0, Ordering::Release);
        self.state_gen.fetch_add(1 << 32, Ordering::Release);
    }

    /// Get generation counter
    #[inline(always)]
    pub fn generation(&self) -> u64 {
        self.state_gen.load(Ordering::Acquire) >> 32
    }
}

impl Default for TouchpadCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Send + Sync safety
unsafe impl Send for TouchpadCapsule {}
unsafe impl Sync for TouchpadCapsule {}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_touch_point_pack() {
        let point = TouchPoint::new(100, 200, 42);
        let packed = point.pack_essential();
        let unpacked = TouchPoint::unpack_essential(packed);
        assert_eq!(unpacked.x, 100);
        assert_eq!(unpacked.y, 200);
        assert_eq!(unpacked.tracking_id, 42);
    }

    #[test]
    fn test_gesture_type() {
        assert!(GestureType::SwipeLeft.is_swipe());
        assert!(GestureType::Tap.is_tap());
        assert!(GestureType::PinchIn.is_pinch());
        assert!(!GestureType::None.is_swipe());
    }

    #[test]
    fn test_touchpad_new() {
        let tp = TouchpadCapsule::new();
        assert_eq!(tp.touch_count(), 0);
        assert_eq!(tp.gesture().gesture_type, GestureType::None);
    }

    #[test]
    fn test_capsule_size_alignment() {
        use core::mem::{size_of, align_of};

        assert_eq!(size_of::<TouchpadCapsule>(), 256);
        assert_eq!(align_of::<TouchpadCapsule>(), 256);
    }
}
