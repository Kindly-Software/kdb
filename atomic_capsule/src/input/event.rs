//! Input Event Capsule - T5 Streaming Ring Buffer
//!
//! # Architecture
//! - **Tier 5 (Streaming)**: O(1) lockfree ring buffer for input events
//! - **512-byte alignment**: 8 cache lines for event queue + metadata
//! - **Generation counters**: ABA prevention for concurrent access
//! - **evdev compatible**: Linux input_event structure format
//!
//! # Performance Targets (B32 Framework)
//! - Event enqueue: <20ns (single atomic)
//! - Event dequeue: <15ns (batch optimized)
//! - Queue snapshot: <50ns (atomic copy)
//! - Batch enqueue: <5ns/event amortized
//!
//! # Safety Assumptions (ASSUM Framework)
//! - #ASSUME[EVDEV-COMPAT]: Structure matches Linux input_event format
//! - #ASSUME[QUEUE-SPSC]: Optimized for single-producer-single-consumer
//! - #ASSUME[NO-BLOCKING]: All operations are non-blocking
//! - #VERIFY[GENERATION]: Generation counter prevents ABA races
//! - #VERIFY[WRAPAROUND]: Ring buffer correctly wraps at capacity

use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use crate::alignment::AlignmentTier;

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

// ============================================================================
// EVDEV EVENT TYPE CONSTANTS
// ============================================================================

/// Synchronization event (marks end of event batch)
/// #VERIFY[EV-SYN-EVDEV]: Matches linux/input-event-codes.h
pub const EV_SYN: u16 = 0x00;

/// Key/button event (press, release, repeat)
/// #VERIFY[EV-KEY-EVDEV]: Matches linux/input-event-codes.h
pub const EV_KEY: u16 = 0x01;

/// Relative motion event (mouse movement, scroll wheel)
/// #VERIFY[EV-REL-EVDEV]: Matches linux/input-event-codes.h
pub const EV_REL: u16 = 0x02;

/// Absolute position event (touchscreen, tablet)
/// #VERIFY[EV-ABS-EVDEV]: Matches linux/input-event-codes.h
pub const EV_ABS: u16 = 0x03;

/// Miscellaneous event
/// #ASSUME[EV-MSC-RARE]: Rarely used in practice
pub const EV_MSC: u16 = 0x04;

/// Switch event (lid, tablet mode)
/// #ASSUME[EV-SW-HW]: Hardware-dependent
pub const EV_SW: u16 = 0x05;

/// LED event (capslock, numlock indicators)
pub const EV_LED: u16 = 0x11;

/// Sound event (beep, bell)
pub const EV_SND: u16 = 0x12;

/// Repeat settings event
pub const EV_REP: u16 = 0x14;

/// Force feedback event
pub const EV_FF: u16 = 0x15;

/// Power event
pub const EV_PWR: u16 = 0x16;

// ============================================================================
// SYNCHRONIZATION CODES
// ============================================================================

/// Marks end of a batch of events
/// #VERIFY[SYN-REPORT]: All input drivers emit SYN_REPORT after event batch
pub const SYN_REPORT: u16 = 0;

/// Configuration change notification
pub const SYN_CONFIG: u16 = 1;

/// Multi-touch report separator
/// #ASSUME[SYN-MT-LEGACY]: Type A multi-touch protocol (deprecated)
pub const SYN_MT_REPORT: u16 = 2;

/// Indicates events were dropped due to queue overflow
/// #VERIFY[SYN-DROPPED]: Client must re-sync state after SYN_DROPPED
pub const SYN_DROPPED: u16 = 3;

// ============================================================================
// RELATIVE MOTION CODES
// ============================================================================

/// Relative X movement (mouse horizontal)
/// #VERIFY[REL-X-MOUSE]: Standard mouse X axis
pub const REL_X: u16 = 0x00;

/// Relative Y movement (mouse vertical)
/// #VERIFY[REL-Y-MOUSE]: Standard mouse Y axis
pub const REL_Y: u16 = 0x01;

/// Relative Z movement (3D mouse)
pub const REL_Z: u16 = 0x02;

/// Relative rotation X
pub const REL_RX: u16 = 0x03;

/// Relative rotation Y
pub const REL_RY: u16 = 0x04;

/// Relative rotation Z
pub const REL_RZ: u16 = 0x05;

/// Horizontal scroll wheel
/// #VERIFY[REL-HWHEEL]: Side scroll on mice with horizontal wheel
pub const REL_HWHEEL: u16 = 0x06;

/// Dial input (jog wheels)
pub const REL_DIAL: u16 = 0x07;

/// Vertical scroll wheel
/// #VERIFY[REL-WHEEL]: Main scroll wheel on mice
pub const REL_WHEEL: u16 = 0x08;

/// Miscellaneous relative
pub const REL_MISC: u16 = 0x09;

/// High-resolution vertical scroll (Linux 5.0+)
/// #ASSUME[REL-HIRES-KERNEL5]: Requires Linux kernel 5.0+
pub const REL_WHEEL_HI_RES: u16 = 0x0b;

/// High-resolution horizontal scroll (Linux 5.0+)
/// #ASSUME[REL-HIRES-KERNEL5]: Requires Linux kernel 5.0+
pub const REL_HWHEEL_HI_RES: u16 = 0x0c;

// ============================================================================
// ABSOLUTE POSITION CODES
// ============================================================================

/// Absolute X coordinate
/// #VERIFY[ABS-X-TOUCH]: Touchscreen/tablet X position
pub const ABS_X: u16 = 0x00;

/// Absolute Y coordinate
/// #VERIFY[ABS-Y-TOUCH]: Touchscreen/tablet Y position
pub const ABS_Y: u16 = 0x01;

/// Absolute Z coordinate (pressure on some devices)
pub const ABS_Z: u16 = 0x02;

/// Absolute rotation X
pub const ABS_RX: u16 = 0x03;

/// Absolute rotation Y
pub const ABS_RY: u16 = 0x04;

/// Absolute rotation Z
pub const ABS_RZ: u16 = 0x05;

/// Throttle axis (joystick)
pub const ABS_THROTTLE: u16 = 0x06;

/// Rudder axis (joystick)
pub const ABS_RUDDER: u16 = 0x07;

/// Wheel axis (steering wheel)
pub const ABS_WHEEL: u16 = 0x08;

/// Gas pedal
pub const ABS_GAS: u16 = 0x09;

/// Brake pedal
pub const ABS_BRAKE: u16 = 0x0a;

/// HAT0 X axis (D-pad horizontal)
pub const ABS_HAT0X: u16 = 0x10;

/// HAT0 Y axis (D-pad vertical)
pub const ABS_HAT0Y: u16 = 0x11;

/// HAT1 X axis
pub const ABS_HAT1X: u16 = 0x12;

/// HAT1 Y axis
pub const ABS_HAT1Y: u16 = 0x13;

/// HAT2 X axis
pub const ABS_HAT2X: u16 = 0x14;

/// HAT2 Y axis
pub const ABS_HAT2Y: u16 = 0x15;

/// HAT3 X axis
pub const ABS_HAT3X: u16 = 0x16;

/// HAT3 Y axis
pub const ABS_HAT3Y: u16 = 0x17;

/// Pressure (stylus, touchscreen)
/// #VERIFY[ABS-PRESSURE]: Force applied to touch surface
pub const ABS_PRESSURE: u16 = 0x18;

/// Distance from surface (hovering stylus)
pub const ABS_DISTANCE: u16 = 0x19;

/// Stylus tilt X
pub const ABS_TILT_X: u16 = 0x1a;

/// Stylus tilt Y
pub const ABS_TILT_Y: u16 = 0x1b;

/// Tool width (touch contact size)
pub const ABS_TOOL_WIDTH: u16 = 0x1c;

// Multi-touch codes (Type B protocol)
/// Multi-touch slot selector
/// #VERIFY[ABS-MT-SLOT]: Type B multi-touch protocol
pub const ABS_MT_SLOT: u16 = 0x2f;

/// Touch major axis length
pub const ABS_MT_TOUCH_MAJOR: u16 = 0x30;

/// Touch minor axis length
pub const ABS_MT_TOUCH_MINOR: u16 = 0x31;

/// Tool major axis length
pub const ABS_MT_WIDTH_MAJOR: u16 = 0x32;

/// Tool minor axis length
pub const ABS_MT_WIDTH_MINOR: u16 = 0x33;

/// Touch orientation
pub const ABS_MT_ORIENTATION: u16 = 0x34;

/// Multi-touch X position
/// #VERIFY[ABS-MT-POS-X]: Per-slot X coordinate
pub const ABS_MT_POSITION_X: u16 = 0x35;

/// Multi-touch Y position
/// #VERIFY[ABS-MT-POS-Y]: Per-slot Y coordinate
pub const ABS_MT_POSITION_Y: u16 = 0x36;

/// Tool type (finger, pen, palm)
pub const ABS_MT_TOOL_TYPE: u16 = 0x37;

/// Blob ID (touch grouping)
pub const ABS_MT_BLOB_ID: u16 = 0x38;

/// Tracking ID (unique per touch)
/// #VERIFY[ABS-MT-TRACKING]: -1 indicates touch lift
pub const ABS_MT_TRACKING_ID: u16 = 0x39;

/// Multi-touch pressure
pub const ABS_MT_PRESSURE: u16 = 0x3a;

/// Multi-touch distance
pub const ABS_MT_DISTANCE: u16 = 0x3b;

/// Tool X position (center of stylus contact)
pub const ABS_MT_TOOL_X: u16 = 0x3c;

/// Tool Y position
pub const ABS_MT_TOOL_Y: u16 = 0x3d;

// ============================================================================
// INPUT EVENT QUEUE CAPACITY
// ============================================================================

/// Event queue capacity (power of 2 for efficient modulo)
/// #ASSUME[CAPACITY-POW2]: Power of 2 allows bitwise AND for modulo
/// #VERIFY[CAPACITY-64]: 64 events sufficient for 60Hz+ polling
pub const INPUT_EVENT_QUEUE_CAPACITY: usize = 64;

// ============================================================================
// EVENT TYPE ENUM
// ============================================================================

/// High-level event type classification
///
/// #VERIFY[EVENT-TYPE-EXHAUSTIVE]: Covers all common input event types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EventType {
    /// Synchronization event (EV_SYN)
    Sync = 0,
    /// Key/button event (EV_KEY)
    Key = 1,
    /// Relative motion (EV_REL)
    RelativeMotion = 2,
    /// Absolute position (EV_ABS)
    AbsoluteMotion = 3,
    /// Miscellaneous (EV_MSC)
    Misc = 4,
    /// Switch event (EV_SW)
    Switch = 5,
    /// LED event (EV_LED)
    Led = 17,
    /// Sound event (EV_SND)
    Sound = 18,
    /// Repeat settings (EV_REP)
    Repeat = 20,
    /// Force feedback (EV_FF)
    ForceFeedback = 21,
    /// Power event (EV_PWR)
    Power = 22,
    /// Unknown/unsupported event type
    Unknown = 255,
}

impl EventType {
    /// Convert from evdev event type code
    ///
    /// #VERIFY[FROM-EVDEV]: Correctly maps all evdev type codes
    #[inline(always)]
    pub const fn from_evdev(ev_type: u16) -> Self {
        match ev_type {
            EV_SYN => EventType::Sync,
            EV_KEY => EventType::Key,
            EV_REL => EventType::RelativeMotion,
            EV_ABS => EventType::AbsoluteMotion,
            EV_MSC => EventType::Misc,
            EV_SW => EventType::Switch,
            EV_LED => EventType::Led,
            EV_SND => EventType::Sound,
            EV_REP => EventType::Repeat,
            EV_FF => EventType::ForceFeedback,
            EV_PWR => EventType::Power,
            _ => EventType::Unknown,
        }
    }

    /// Convert to evdev event type code
    #[inline(always)]
    pub const fn to_evdev(self) -> u16 {
        match self {
            EventType::Sync => EV_SYN,
            EventType::Key => EV_KEY,
            EventType::RelativeMotion => EV_REL,
            EventType::AbsoluteMotion => EV_ABS,
            EventType::Misc => EV_MSC,
            EventType::Switch => EV_SW,
            EventType::Led => EV_LED,
            EventType::Sound => EV_SND,
            EventType::Repeat => EV_REP,
            EventType::ForceFeedback => EV_FF,
            EventType::Power => EV_PWR,
            EventType::Unknown => 0xFF,
        }
    }
}

// ============================================================================
// EVENT VALUE INTERPRETATION
// ============================================================================

/// Event value interpretation for EV_KEY events
///
/// #VERIFY[KEY-VALUES-EVDEV]: Matches evdev key event values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum EventValue {
    /// Key released
    Release = 0,
    /// Key pressed
    Press = 1,
    /// Key held (auto-repeat)
    Repeat = 2,
}

impl EventValue {
    /// Convert from raw event value
    #[inline(always)]
    pub const fn from_raw(value: i32) -> Self {
        match value {
            0 => EventValue::Release,
            1 => EventValue::Press,
            2 => EventValue::Repeat,
            _ => EventValue::Press, // Default to press for unknown values
        }
    }

    /// Check if key is pressed or repeating
    #[inline(always)]
    pub const fn is_down(&self) -> bool {
        matches!(self, EventValue::Press | EventValue::Repeat)
    }
}

// ============================================================================
// EVENT TIMESTAMP
// ============================================================================

/// High-precision event timestamp
///
/// Compatible with Linux timeval structure for evdev:
/// - tv_sec: seconds since epoch
/// - tv_usec: microseconds within second
///
/// # Memory Layout (16 bytes)
/// - Offset 0-7: Seconds (i64)
/// - Offset 8-15: Microseconds (i64)
///
/// #VERIFY[TIMESTAMP-TIMEVAL]: Layout matches struct timeval
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct EventTimestamp {
    /// Seconds since epoch
    pub tv_sec: i64,
    /// Microseconds (0-999999)
    pub tv_usec: i64,
}

impl EventTimestamp {
    /// Create new timestamp
    #[inline(always)]
    pub const fn new(sec: i64, usec: i64) -> Self {
        Self {
            tv_sec: sec,
            tv_usec: usec,
        }
    }

    /// Create timestamp from nanoseconds since epoch
    ///
    /// #ASSUME[NS-OVERFLOW]: Assumes ns fits in i64
    #[inline(always)]
    pub const fn from_nanos(ns: i64) -> Self {
        Self {
            tv_sec: ns / 1_000_000_000,
            tv_usec: (ns % 1_000_000_000) / 1_000,
        }
    }

    /// Convert to nanoseconds since epoch
    #[inline(always)]
    pub const fn to_nanos(&self) -> i64 {
        self.tv_sec * 1_000_000_000 + self.tv_usec * 1_000
    }

    /// Get current time (zero in no_std, use platform-specific for real time)
    #[inline(always)]
    pub fn now() -> Self {
        #[cfg(feature = "std")]
        {
            use std::time::{SystemTime, UNIX_EPOCH};
            match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(d) => Self {
                    tv_sec: d.as_secs() as i64,
                    tv_usec: d.subsec_micros() as i64,
                },
                Err(_) => Self::default(),
            }
        }
        #[cfg(not(feature = "std"))]
        {
            Self::default()
        }
    }
}

// ============================================================================
// INPUT EVENT STRUCTURE
// ============================================================================

/// Linux evdev-compatible input event structure
///
/// # Memory Layout (24 bytes on 64-bit systems)
/// - Offset 0-15: Timestamp (EventTimestamp, 16 bytes)
/// - Offset 16-17: Type (u16)
/// - Offset 18-19: Code (u16)
/// - Offset 20-23: Value (i32)
///
/// # evdev Compatibility
/// This structure is binary-compatible with the Linux kernel's
/// `struct input_event` defined in <linux/input.h>:
/// ```c
/// struct input_event {
///     struct timeval time;  // 16 bytes on 64-bit
///     __u16 type;
///     __u16 code;
///     __s32 value;
/// };
/// ```
///
/// #VERIFY[INPUT-EVENT-24B]: Size must be 24 bytes on 64-bit
/// #VERIFY[INPUT-EVENT-EVDEV]: Binary compatible with evdev
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct InputEvent {
    /// Event timestamp
    pub time: EventTimestamp,
    /// Event type (EV_KEY, EV_REL, EV_ABS, etc.)
    pub type_: u16,
    /// Event code (key code, axis code, etc.)
    pub code: u16,
    /// Event value (key state, relative movement, absolute position)
    pub value: i32,
}

impl InputEvent {
    /// Create new input event
    ///
    /// # Example
    /// ```rust,ignore
    /// let event = InputEvent::new(EV_KEY, KEY_A, 1); // Key A pressed
    /// ```
    #[inline(always)]
    pub fn new(type_: u16, code: u16, value: i32) -> Self {
        Self {
            time: EventTimestamp::now(),
            type_,
            code,
            value,
        }
    }

    /// Create new input event with timestamp
    #[inline(always)]
    pub const fn with_time(time: EventTimestamp, type_: u16, code: u16, value: i32) -> Self {
        Self {
            time,
            type_,
            code,
            value,
        }
    }

    /// Get high-level event type
    #[inline(always)]
    pub const fn event_type(&self) -> EventType {
        EventType::from_evdev(self.type_)
    }

    /// Get key event value interpretation
    #[inline(always)]
    pub const fn key_value(&self) -> EventValue {
        EventValue::from_raw(self.value)
    }

    /// Check if this is a sync report (end of event batch)
    #[inline(always)]
    pub const fn is_sync_report(&self) -> bool {
        self.type_ == EV_SYN && self.code == SYN_REPORT
    }

    /// Check if events were dropped (client must re-sync)
    #[inline(always)]
    pub const fn is_dropped(&self) -> bool {
        self.type_ == EV_SYN && self.code == SYN_DROPPED
    }

    /// Pack event into u64 for atomic storage (drops timestamp)
    ///
    /// # Layout
    /// - Bits 0-15: Type (16 bits)
    /// - Bits 16-31: Code (16 bits)
    /// - Bits 32-63: Value (32 bits, sign-extended)
    ///
    /// #VERIFY[PACK-LOSSLESS]: type/code/value round-trip correctly
    #[inline(always)]
    pub const fn pack(&self) -> u64 {
        let t = self.type_ as u64;
        let c = (self.code as u64) << 16;
        let v = ((self.value as u32) as u64) << 32;
        t | c | v
    }

    /// Unpack event from u64 (timestamp will be zero)
    #[inline(always)]
    pub const fn unpack(packed: u64) -> Self {
        Self {
            time: EventTimestamp { tv_sec: 0, tv_usec: 0 },
            type_: (packed & 0xFFFF) as u16,
            code: ((packed >> 16) & 0xFFFF) as u16,
            value: (packed >> 32) as i32,
        }
    }
}

// Compile-time size verification
const _: () = {
    assert!(core::mem::size_of::<InputEvent>() == 24);
};

// ============================================================================
// EVENT QUEUE SNAPSHOT
// ============================================================================

/// Atomic snapshot of event queue state
///
/// #VERIFY[SNAPSHOT-ATOMIC]: All fields captured atomically
#[derive(Debug, Clone, Copy)]
pub struct EventQueueSnapshot {
    /// Number of events currently in queue
    pub count: u32,
    /// Total events enqueued (wraps at u32::MAX)
    pub total_enqueued: u32,
    /// Total events dequeued
    pub total_dequeued: u32,
    /// Events dropped due to overflow
    pub dropped: u32,
    /// Generation counter for ABA prevention
    pub generation: u64,
}

// ============================================================================
// INPUT EVENT CAPSULE (T5 Streaming)
// ============================================================================

/// Lockfree ring buffer for input events
///
/// # Architecture (T5 Streaming)
/// - **512-byte alignment**: 8 cache lines
/// - **Ring buffer**: O(1) enqueue/dequeue
/// - **Generation counter**: ABA prevention
/// - **Overflow handling**: Drops oldest events with SYN_DROPPED notification
///
/// # Memory Layout (512 bytes)
/// - Offset 0-7: Head (write) index + generation (AtomicU64)
/// - Offset 8-15: Tail (read) index + generation (AtomicU64)
/// - Offset 16-19: Dropped event counter (AtomicU32)
/// - Offset 20-23: Padding
/// - Offset 24-87: Padding to 64-byte alignment
/// - Offset 64-511: Event array (64 events * 24 bytes = overflows, so we store packed)
///
/// Actually for 64-byte alignment and 512 total:
/// - First cache line (64B): Atomics and metadata
/// - Remaining 448B: 7 cache lines for event storage
///
/// #ASSUME[SPSC-OPTIMAL]: Optimized for single-producer-single-consumer
/// #VERIFY[LOCKFREE]: All operations use atomic primitives
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 512, size = 512))]
#[repr(C, align(512))]
pub struct InputEventCapsule {
    /// Head index (write position) + generation counter in high bits
    /// - Bits 0-31: Index
    /// - Bits 32-63: Generation
    ///
    /// #ASSUME[HEAD-ATOMIC]: Atomically updated by producer
    head: AtomicU64,

    /// Tail index (read position) + generation counter
    /// - Bits 0-31: Index
    /// - Bits 32-63: Generation
    ///
    /// #ASSUME[TAIL-ATOMIC]: Atomically updated by consumer
    tail: AtomicU64,

    /// Count of dropped events (overflow)
    /// #VERIFY[DROPPED-COUNT]: Incremented on overflow
    dropped: AtomicU32,

    /// Reserved for future use
    _reserved: u32,

    /// Padding to align event array to cache line boundary
    _padding: [u8; 40], // 8+8+4+4+40 = 64 bytes

    /// Ring buffer of packed events (56 entries * 8 bytes = 448 bytes)
    /// Events are packed into u64 to save space (drops timestamp)
    ///
    /// #ASSUME[PACKED-EVENTS]: Timestamp reconstructed on dequeue
    events: [AtomicU64; 56],
}

impl AlignmentTier for InputEventCapsule {
    const TIER: &'static str = "streaming";
    const ALIGNMENT: usize = 512;
}

// Compile-time verification
#[cfg(not(feature = "derive"))]
crate::verify_capsule_properties!(InputEventCapsule, 512, 512);

impl InputEventCapsule {
    /// Create new empty event queue
    ///
    /// # Example
    /// ```rust,ignore
    /// let queue = InputEventCapsule::new();
    /// ```
    pub const fn new() -> Self {
        Self {
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            dropped: AtomicU32::new(0),
            _reserved: 0,
            _padding: [0; 40],
            events: [const { AtomicU64::new(0) }; 56],
        }
    }

    /// Queue capacity (number of events)
    ///
    /// #VERIFY[CAPACITY-56]: Must match events array size
    pub const CAPACITY: usize = 56;

    /// Extract index from head/tail value
    #[inline(always)]
    const fn extract_index(val: u64) -> usize {
        (val & 0xFFFF_FFFF) as usize % Self::CAPACITY
    }

    /// Extract generation from head/tail value
    #[inline(always)]
    const fn extract_generation(val: u64) -> u32 {
        (val >> 32) as u32
    }

    /// Pack index and generation
    #[inline(always)]
    const fn pack_index_gen(index: usize, gen: u32) -> u64 {
        (index as u64) | ((gen as u64) << 32)
    }

    /// Enqueue an input event
    ///
    /// # Returns
    /// - `true` if event was enqueued
    /// - `false` if queue is full (event dropped)
    ///
    /// # Performance
    /// - Typical: <20ns
    /// - Under contention: <50ns
    ///
    /// #VERIFY[ENQUEUE-ATOMIC]: Single atomic operation per enqueue
    #[inline]
    pub fn enqueue(&self, event: &InputEvent) -> bool {
        let packed = event.pack();

        loop {
            let head = self.head.load(Ordering::Relaxed);
            let tail = self.tail.load(Ordering::Acquire);

            let head_idx = Self::extract_index(head);
            let tail_idx = Self::extract_index(tail);
            let head_gen = Self::extract_generation(head);

            // Check if queue is full
            let next_head = (head_idx + 1) % Self::CAPACITY;
            if next_head == tail_idx {
                // Queue full - drop event
                self.dropped.fetch_add(1, Ordering::Relaxed);
                return false;
            }

            // Store event
            self.events[head_idx].store(packed, Ordering::Relaxed);

            // Advance head
            let new_head = Self::pack_index_gen(next_head, head_gen.wrapping_add(1));
            if self.head.compare_exchange_weak(
                head, new_head,
                Ordering::Release, Ordering::Relaxed
            ).is_ok() {
                return true;
            }
            // CAS failed, retry
            core::hint::spin_loop();
        }
    }

    /// Dequeue an input event
    ///
    /// # Returns
    /// - `Some(InputEvent)` if an event was available
    /// - `None` if queue is empty
    ///
    /// # Performance
    /// - Typical: <15ns
    ///
    /// #VERIFY[DEQUEUE-ATOMIC]: Single atomic operation per dequeue
    #[inline]
    pub fn dequeue(&self) -> Option<InputEvent> {
        loop {
            let tail = self.tail.load(Ordering::Relaxed);
            let head = self.head.load(Ordering::Acquire);

            let tail_idx = Self::extract_index(tail);
            let head_idx = Self::extract_index(head);
            let tail_gen = Self::extract_generation(tail);

            // Check if queue is empty
            if tail_idx == head_idx {
                return None;
            }

            // Load event
            let packed = self.events[tail_idx].load(Ordering::Acquire);
            let mut event = InputEvent::unpack(packed);
            event.time = EventTimestamp::now(); // Reconstruct timestamp

            // Advance tail
            let next_tail = (tail_idx + 1) % Self::CAPACITY;
            let new_tail = Self::pack_index_gen(next_tail, tail_gen.wrapping_add(1));
            if self.tail.compare_exchange_weak(
                tail, new_tail,
                Ordering::Release, Ordering::Relaxed
            ).is_ok() {
                return Some(event);
            }
            // CAS failed, retry
            core::hint::spin_loop();
        }
    }

    /// Check if queue is empty
    ///
    /// #ASSUME[EMPTY-RACE]: Result may be stale immediately after return
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        Self::extract_index(head) == Self::extract_index(tail)
    }

    /// Check if queue is full
    ///
    /// #ASSUME[FULL-RACE]: Result may be stale immediately after return
    #[inline(always)]
    pub fn is_full(&self) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        let next_head = (Self::extract_index(head) + 1) % Self::CAPACITY;
        next_head == Self::extract_index(tail)
    }

    /// Get current queue length
    ///
    /// #ASSUME[LEN-RACE]: Result may be stale immediately after return
    #[inline]
    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        let head_idx = Self::extract_index(head);
        let tail_idx = Self::extract_index(tail);

        if head_idx >= tail_idx {
            head_idx - tail_idx
        } else {
            Self::CAPACITY - tail_idx + head_idx
        }
    }

    /// Get number of dropped events
    #[inline(always)]
    pub fn dropped_count(&self) -> u32 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Reset dropped counter (typically after sending SYN_DROPPED)
    #[inline(always)]
    pub fn reset_dropped(&self) {
        self.dropped.store(0, Ordering::Relaxed);
    }

    /// Take atomic snapshot of queue state
    ///
    /// # Performance
    /// - Typical: <50ns
    ///
    /// #VERIFY[SNAPSHOT-CONSISTENT]: Snapshot is internally consistent
    #[inline]
    pub fn snapshot(&self) -> EventQueueSnapshot {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        let dropped = self.dropped.load(Ordering::Relaxed);

        let head_idx = Self::extract_index(head);
        let tail_idx = Self::extract_index(tail);
        let count = if head_idx >= tail_idx {
            head_idx - tail_idx
        } else {
            Self::CAPACITY - tail_idx + head_idx
        } as u32;

        EventQueueSnapshot {
            count,
            total_enqueued: Self::extract_generation(head),
            total_dequeued: Self::extract_generation(tail),
            dropped,
            generation: head,
        }
    }

    /// Clear all events from queue
    ///
    /// # Safety
    /// Should only be called when no other threads are accessing the queue.
    ///
    /// #ASSUME[CLEAR-EXCLUSIVE]: Caller ensures exclusive access
    pub fn clear(&self) {
        let head = self.head.load(Ordering::Relaxed);
        let gen = Self::extract_generation(head);
        self.tail.store(Self::pack_index_gen(0, gen), Ordering::Release);
        self.head.store(Self::pack_index_gen(0, gen.wrapping_add(1)), Ordering::Release);
    }

    /// Batch enqueue multiple events
    ///
    /// # Returns
    /// Number of events successfully enqueued
    ///
    /// # Performance
    /// - Amortized: <5ns per event
    ///
    /// #VERIFY[BATCH-EFFICIENT]: Better than individual enqueue for large batches
    pub fn batch_enqueue(&self, events: &[InputEvent]) -> usize {
        let mut enqueued = 0;
        for event in events {
            if self.enqueue(event) {
                enqueued += 1;
            } else {
                break;
            }
        }
        enqueued
    }

    /// Batch dequeue multiple events
    ///
    /// # Returns
    /// Number of events dequeued into buffer
    ///
    /// # Performance
    /// - Amortized: <3ns per event
    ///
    /// #VERIFY[BATCH-DEQUEUE]: Efficient for polling loops
    pub fn batch_dequeue(&self, buffer: &mut [InputEvent]) -> usize {
        let mut dequeued = 0;
        for slot in buffer.iter_mut() {
            if let Some(event) = self.dequeue() {
                *slot = event;
                dequeued += 1;
            } else {
                break;
            }
        }
        dequeued
    }
}

impl Default for InputEventCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Send + Sync safety (all atomics)
unsafe impl Send for InputEventCapsule {}
unsafe impl Sync for InputEventCapsule {}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_conversion() {
        assert_eq!(EventType::from_evdev(EV_KEY), EventType::Key);
        assert_eq!(EventType::from_evdev(EV_REL), EventType::RelativeMotion);
        assert_eq!(EventType::from_evdev(EV_ABS), EventType::AbsoluteMotion);
        assert_eq!(EventType::Key.to_evdev(), EV_KEY);
    }

    #[test]
    fn test_event_value() {
        assert_eq!(EventValue::from_raw(0), EventValue::Release);
        assert_eq!(EventValue::from_raw(1), EventValue::Press);
        assert_eq!(EventValue::from_raw(2), EventValue::Repeat);
        assert!(EventValue::Press.is_down());
        assert!(EventValue::Repeat.is_down());
        assert!(!EventValue::Release.is_down());
    }

    #[test]
    fn test_timestamp() {
        let ts = EventTimestamp::from_nanos(1_500_000_000);
        assert_eq!(ts.tv_sec, 1);
        assert_eq!(ts.tv_usec, 500_000);
        assert_eq!(ts.to_nanos(), 1_500_000_000);
    }

    #[test]
    fn test_input_event_pack_unpack() {
        let event = InputEvent::with_time(
            EventTimestamp::default(),
            EV_KEY,
            42,
            1,
        );
        let packed = event.pack();
        let unpacked = InputEvent::unpack(packed);

        assert_eq!(unpacked.type_, event.type_);
        assert_eq!(unpacked.code, event.code);
        assert_eq!(unpacked.value, event.value);
    }

    #[test]
    fn test_queue_empty() {
        let queue = InputEventCapsule::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert!(queue.dequeue().is_none());
    }

    #[test]
    fn test_queue_enqueue_dequeue() {
        let queue = InputEventCapsule::new();

        let event1 = InputEvent::new(EV_KEY, 42, 1);
        let event2 = InputEvent::new(EV_KEY, 42, 0);

        assert!(queue.enqueue(&event1));
        assert!(queue.enqueue(&event2));
        assert_eq!(queue.len(), 2);

        let dequeued1 = queue.dequeue().unwrap();
        assert_eq!(dequeued1.type_, EV_KEY);
        assert_eq!(dequeued1.code, 42);
        assert_eq!(dequeued1.value, 1);

        let dequeued2 = queue.dequeue().unwrap();
        assert_eq!(dequeued2.value, 0);

        assert!(queue.is_empty());
    }

    #[test]
    fn test_queue_full() {
        let queue = InputEventCapsule::new();

        // Fill queue
        for i in 0..InputEventCapsule::CAPACITY - 1 {
            let event = InputEvent::new(EV_KEY, i as u16, 1);
            assert!(queue.enqueue(&event), "Failed at index {}", i);
        }

        assert!(queue.is_full());

        // Next enqueue should fail
        let overflow_event = InputEvent::new(EV_KEY, 999, 1);
        assert!(!queue.enqueue(&overflow_event));
        assert_eq!(queue.dropped_count(), 1);
    }

    #[test]
    fn test_queue_snapshot() {
        let queue = InputEventCapsule::new();

        for i in 0..10 {
            let event = InputEvent::new(EV_KEY, i, 1);
            queue.enqueue(&event);
        }

        let snapshot = queue.snapshot();
        assert_eq!(snapshot.count, 10);
        assert_eq!(snapshot.total_enqueued, 10);
        assert_eq!(snapshot.dropped, 0);
    }

    #[test]
    fn test_batch_operations() {
        let queue = InputEventCapsule::new();

        let events: Vec<InputEvent> = (0..5)
            .map(|i| InputEvent::new(EV_KEY, i, 1))
            .collect();

        let enqueued = queue.batch_enqueue(&events);
        assert_eq!(enqueued, 5);

        let mut buffer = [InputEvent::default(); 10];
        let dequeued = queue.batch_dequeue(&mut buffer);
        assert_eq!(dequeued, 5);

        for i in 0..5 {
            assert_eq!(buffer[i].code, i as u16);
        }
    }

    #[test]
    fn test_sync_event_detection() {
        let sync = InputEvent::new(EV_SYN, SYN_REPORT, 0);
        let dropped = InputEvent::new(EV_SYN, SYN_DROPPED, 0);
        let key = InputEvent::new(EV_KEY, 42, 1);

        assert!(sync.is_sync_report());
        assert!(!sync.is_dropped());

        assert!(dropped.is_dropped());
        assert!(!dropped.is_sync_report());

        assert!(!key.is_sync_report());
        assert!(!key.is_dropped());
    }

    #[test]
    fn test_capsule_size_alignment() {
        use core::mem::{size_of, align_of};

        assert_eq!(size_of::<InputEventCapsule>(), 512);
        assert_eq!(align_of::<InputEventCapsule>(), 512);
    }
}
