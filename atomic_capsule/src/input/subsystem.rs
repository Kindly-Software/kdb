//! Input Subsystem Meta-Capsule - T6 Mixed Orchestration
//!
//! # Architecture
//! - **Tier 6 (Mixed)**: Meta-capsule orchestrating T1 + T5 capsules
//! - **1KB alignment**: Multiple cache lines for device registry + coordination
//! - **Device discovery**: Auto-detection of /dev/input/eventX devices
//! - **Event routing**: Routes events to appropriate device capsules
//!
//! # Performance Targets (B32 Framework)
//! - Device lookup: <10ns (atomic bitmap)
//! - Event routing: <20ns (type dispatch)
//! - Full poll cycle: <100ns (all devices)
//! - Device registration: <50ns
//!
//! # Safety Assumptions (ASSUM Framework)
//! - #ASSUME[EVDEV-PATH]: Devices at /dev/input/eventX
//! - #ASSUME[MAX-DEVICES-16]: 16 devices sufficient for typical systems
//! - #ASSUME[SINGLE-WRITER]: Device registration is single-threaded
//! - #VERIFY[EVENT-ROUTING]: Events routed to correct device type
//! - #VERIFY[DEVICE-BITMAP]: Active device tracking is consistent

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicU16, Ordering};
use crate::alignment::AlignmentTier;
use super::event::{InputEvent, InputEventCapsule, EventType, EV_KEY, EV_REL, EV_ABS};
use super::keyboard::KeyboardCapsule;
use super::mouse::MouseCapsule;
use super::touchpad::TouchpadCapsule;

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

// ============================================================================
// INPUT SUBSYSTEM CONSTANTS
// ============================================================================

/// Maximum number of input devices supported
/// #ASSUME[MAX-16]: 16 devices covers keyboards, mice, touchpads, gamepads
pub const MAX_INPUT_DEVICES: usize = 16;

/// Path prefix for evdev devices
/// #VERIFY[EVDEV-PREFIX]: Standard Linux input device path
pub const INPUT_DEVICE_PATH_PREFIX: &str = "/dev/input/event";

/// Event poll timeout in microseconds
pub const INPUT_POLL_TIMEOUT_US: u32 = 1000;

// ============================================================================
// INPUT DEVICE ID
// ============================================================================

/// Input device identifier
///
/// Uniquely identifies a device by bus type, vendor, product, and version.
/// Compatible with Linux struct input_id.
///
/// #VERIFY[INPUT-ID-COMPAT]: Matches linux/input.h struct input_id
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct InputDeviceId {
    /// Bus type (USB, Bluetooth, etc.)
    pub bus_type: u16,
    /// Vendor ID
    pub vendor: u16,
    /// Product ID
    pub product: u16,
    /// Version
    pub version: u16,
}

impl InputDeviceId {
    /// Create new device ID
    #[inline(always)]
    pub const fn new(bus_type: u16, vendor: u16, product: u16, version: u16) -> Self {
        Self { bus_type, vendor, product, version }
    }

    /// Pack device ID into u64
    #[inline(always)]
    pub const fn pack(&self) -> u64 {
        let b = self.bus_type as u64;
        let v = (self.vendor as u64) << 16;
        let p = (self.product as u64) << 32;
        let ver = (self.version as u64) << 48;
        b | v | p | ver
    }

    /// Unpack device ID from u64
    #[inline(always)]
    pub const fn unpack(packed: u64) -> Self {
        Self {
            bus_type: (packed & 0xFFFF) as u16,
            vendor: ((packed >> 16) & 0xFFFF) as u16,
            product: ((packed >> 32) & 0xFFFF) as u16,
            version: ((packed >> 48) & 0xFFFF) as u16,
        }
    }

    /// Check if this is a null/empty device ID
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.bus_type == 0 && self.vendor == 0 && self.product == 0 && self.version == 0
    }
}

// ============================================================================
// DEVICE CAPABILITIES
// ============================================================================

/// Device capability flags
///
/// Bitmap indicating what event types the device supports.
///
/// #VERIFY[CAPABILITIES-EVDEV]: Matches evdev capability detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct DeviceCapabilities(pub u32);

impl DeviceCapabilities {
    /// No capabilities
    pub const NONE: Self = Self(0);

    /// Supports synchronization events (EV_SYN)
    pub const SYNC: Self = Self(1 << 0);
    /// Supports key/button events (EV_KEY)
    pub const KEY: Self = Self(1 << 1);
    /// Supports relative motion (EV_REL)
    pub const RELATIVE: Self = Self(1 << 2);
    /// Supports absolute position (EV_ABS)
    pub const ABSOLUTE: Self = Self(1 << 3);
    /// Supports miscellaneous events (EV_MSC)
    pub const MISC: Self = Self(1 << 4);
    /// Supports switch events (EV_SW)
    pub const SWITCH: Self = Self(1 << 5);
    /// Supports LED events (EV_LED)
    pub const LED: Self = Self(1 << 11);
    /// Supports sound events (EV_SND)
    pub const SOUND: Self = Self(1 << 12);
    /// Supports repeat settings (EV_REP)
    pub const REPEAT: Self = Self(1 << 14);
    /// Supports force feedback (EV_FF)
    pub const FORCE_FEEDBACK: Self = Self(1 << 15);

    /// Check if device is a keyboard
    #[inline(always)]
    pub const fn is_keyboard(&self) -> bool {
        self.0 & Self::KEY.0 != 0 && self.0 & Self::REPEAT.0 != 0
    }

    /// Check if device is a mouse
    #[inline(always)]
    pub const fn is_mouse(&self) -> bool {
        self.0 & Self::KEY.0 != 0 && self.0 & Self::RELATIVE.0 != 0
    }

    /// Check if device is a touchpad/touchscreen
    #[inline(always)]
    pub const fn is_touchpad(&self) -> bool {
        self.0 & Self::ABSOLUTE.0 != 0
    }

    /// Check if device is a gamepad/joystick
    #[inline(always)]
    pub const fn is_gamepad(&self) -> bool {
        self.0 & Self::KEY.0 != 0 && self.0 & Self::ABSOLUTE.0 != 0 &&
        self.0 & Self::RELATIVE.0 == 0
    }

    /// Check if capability is present
    #[inline(always)]
    pub const fn has(&self, cap: Self) -> bool {
        self.0 & cap.0 != 0
    }

    /// Add capability
    #[inline(always)]
    pub const fn with(self, cap: Self) -> Self {
        Self(self.0 | cap.0)
    }
}

// ============================================================================
// INPUT ERROR
// ============================================================================

/// Input subsystem error types
///
/// #VERIFY[ERROR-COMPLETE]: All error cases covered
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputError {
    /// Device not found
    DeviceNotFound,
    /// Maximum devices reached
    MaxDevicesReached,
    /// Device already registered
    DeviceAlreadyRegistered,
    /// Invalid device index
    InvalidDeviceIndex,
    /// Permission denied
    PermissionDenied,
    /// Device busy
    DeviceBusy,
    /// I/O error
    IoError,
    /// Invalid event
    InvalidEvent,
    /// Not initialized
    NotInitialized,
}

/// Input operation result
pub type InputResult<T> = Result<T, InputError>;

// ============================================================================
// INPUT SUBSYSTEM STATE
// ============================================================================

/// Input subsystem operational state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InputSubsystemState {
    /// Not initialized
    Uninitialized = 0,
    /// Initializing
    Initializing = 1,
    /// Ready for input
    Ready = 2,
    /// Processing events
    Processing = 3,
    /// Suspended
    Suspended = 4,
    /// Error state
    Error = 255,
}

impl InputSubsystemState {
    /// Convert from raw value
    #[inline(always)]
    pub const fn from_raw(val: u8) -> Self {
        match val {
            0 => InputSubsystemState::Uninitialized,
            1 => InputSubsystemState::Initializing,
            2 => InputSubsystemState::Ready,
            3 => InputSubsystemState::Processing,
            4 => InputSubsystemState::Suspended,
            _ => InputSubsystemState::Error,
        }
    }
}

// ============================================================================
// INPUT SUBSYSTEM SNAPSHOT
// ============================================================================

/// Atomic snapshot of input subsystem state
///
/// #VERIFY[SNAPSHOT-ATOMIC]: All fields captured atomically
#[derive(Debug, Clone, Copy)]
pub struct InputSubsystemSnapshot {
    /// Current state
    pub state: InputSubsystemState,
    /// Number of registered devices
    pub device_count: u8,
    /// Active device bitmap
    pub active_devices: u16,
    /// Total events processed
    pub events_processed: u64,
    /// Generation counter
    pub generation: u64,
}

// ============================================================================
// INPUT SUBSYSTEM CAPSULE (T6 Mixed)
// ============================================================================

/// Meta-capsule orchestrating the input subsystem
///
/// # Architecture (T6 Mixed)
/// - **1024-byte alignment**: 16 cache lines
/// - **Device registry**: Up to 16 devices with capabilities
/// - **Event routing**: Type-based dispatch to sub-capsules
/// - **Statistics**: Event counts and timing
///
/// # Memory Layout (1024 bytes)
/// - Offset 0-63: First cache line (state + counters)
///   - 0-7: State + generation (AtomicU64)
///   - 8-15: Events processed counter (AtomicU64)
///   - 16-19: Device count + active bitmap (AtomicU32)
///   - 20-27: Last event time (AtomicU64)
///   - 28-63: Padding
/// - Offset 64-191: Second-third cache lines (device IDs, 16 x u64)
/// - Offset 192-255: Fourth cache line (device capabilities, 16 x u32)
/// - Offset 256-511: Fifth-eighth cache lines (reserved for expansion)
/// - Offset 512-1023: Ninth-sixteenth cache lines (statistics)
///
/// #ASSUME[LAYOUT-OPTIMAL]: Layout optimized for device lookup
/// #VERIFY[LOCKFREE]: All operations use atomic primitives
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 1024, size = 1024))]
#[repr(C, align(1024))]
pub struct InputSubsystemCapsule {
    // === First cache line (64 bytes): Core state ===

    /// State + generation counter
    /// - Bits 0-7: State enum
    /// - Bits 8-15: Device count
    /// - Bits 16-31: Active device bitmap
    /// - Bits 32-63: Generation counter
    state_gen: AtomicU64,

    /// Total events processed
    events_processed: AtomicU64,

    /// Keyboard event count
    keyboard_events: AtomicU64,

    /// Mouse event count
    mouse_events: AtomicU64,

    /// Touch event count
    touch_events: AtomicU64,

    /// Padding
    _padding1: [u8; 24],

    // === Second-third cache lines (128 bytes): Device IDs ===
    /// Device IDs (packed InputDeviceId)
    device_ids: [AtomicU64; 16],

    // === Fourth cache line (64 bytes): Capabilities ===
    /// Device capabilities
    device_caps: [AtomicU32; 16],

    // === Fifth-eighth cache lines (256 bytes): Reserved ===
    _reserved1: [u8; 256],

    // === Ninth-sixteenth cache lines (512 bytes): Statistics ===
    /// Per-device event counts
    device_event_counts: [AtomicU64; 16],

    /// Last event timestamps per device
    device_last_event: [AtomicU64; 16],

    /// Error counts
    error_counts: [AtomicU32; 16],

    /// Padding to reach 1024 bytes
    /// Layout: 64 + 128 + 64 + 256 + 128 + 128 + 64 = 832
    /// Need: 1024 - 832 = 192 bytes padding
    _padding_final: [u8; 192],
}

impl AlignmentTier for InputSubsystemCapsule {
    const TIER: &'static str = "mixed";
    const ALIGNMENT: usize = 1024;
}

// Compile-time verification
#[cfg(not(feature = "derive"))]
crate::verify_capsule_properties!(InputSubsystemCapsule, 1024, 1024);

impl InputSubsystemCapsule {
    /// Create new input subsystem capsule
    pub const fn new() -> Self {
        Self {
            state_gen: AtomicU64::new(0),
            events_processed: AtomicU64::new(0),
            keyboard_events: AtomicU64::new(0),
            mouse_events: AtomicU64::new(0),
            touch_events: AtomicU64::new(0),
            _padding1: [0; 24],
            device_ids: [const { AtomicU64::new(0) }; 16],
            device_caps: [const { AtomicU32::new(0) }; 16],
            _reserved1: [0; 256],
            device_event_counts: [const { AtomicU64::new(0) }; 16],
            device_last_event: [const { AtomicU64::new(0) }; 16],
            error_counts: [const { AtomicU32::new(0) }; 16],
            _padding_final: [0; 192],
        }
    }

    /// Get current state
    #[inline(always)]
    pub fn state(&self) -> InputSubsystemState {
        InputSubsystemState::from_raw((self.state_gen.load(Ordering::Acquire) & 0xFF) as u8)
    }

    /// Get device count
    #[inline(always)]
    pub fn device_count(&self) -> u8 {
        ((self.state_gen.load(Ordering::Acquire) >> 8) & 0xFF) as u8
    }

    /// Get active device bitmap
    #[inline(always)]
    pub fn active_devices(&self) -> u16 {
        ((self.state_gen.load(Ordering::Acquire) >> 16) & 0xFFFF) as u16
    }

    /// Get total events processed
    #[inline(always)]
    pub fn events_processed(&self) -> u64 {
        self.events_processed.load(Ordering::Relaxed)
    }

    /// Initialize the input subsystem
    ///
    /// # Performance
    /// - Typical: <100ns
    ///
    /// #VERIFY[INIT-ATOMIC]: State transition is atomic
    pub fn initialize(&self) -> InputResult<()> {
        loop {
            let old = self.state_gen.load(Ordering::Relaxed);
            let state = (old & 0xFF) as u8;

            if state != InputSubsystemState::Uninitialized as u8 {
                if state == InputSubsystemState::Ready as u8 {
                    return Ok(()); // Already initialized
                }
                return Err(InputError::DeviceBusy);
            }

            // Set to Initializing
            let new = (old & 0xFFFF_FFFF_FFFF_FF00) | (InputSubsystemState::Initializing as u64);
            if self.state_gen.compare_exchange_weak(
                old, new, Ordering::Release, Ordering::Relaxed
            ).is_ok() {
                break;
            }
        }

        // Transition to Ready
        loop {
            let old = self.state_gen.load(Ordering::Relaxed);
            let new = (old & 0xFFFF_FFFF_FFFF_FF00) | (InputSubsystemState::Ready as u64);
            if self.state_gen.compare_exchange_weak(
                old, new, Ordering::Release, Ordering::Relaxed
            ).is_ok() {
                break;
            }
        }

        // Increment generation
        self.state_gen.fetch_add(1 << 32, Ordering::Release);

        Ok(())
    }

    /// Register a new input device
    ///
    /// # Returns
    /// Device index on success
    ///
    /// # Performance
    /// - Typical: <50ns
    ///
    /// #VERIFY[REGISTER-ATOMIC]: Device registration is atomic
    pub fn register_device(&self, id: InputDeviceId, caps: DeviceCapabilities) -> InputResult<usize> {
        let count = self.device_count() as usize;
        if count >= MAX_INPUT_DEVICES {
            return Err(InputError::MaxDevicesReached);
        }

        // Find empty slot
        for i in 0..MAX_INPUT_DEVICES {
            let slot_id = self.device_ids[i].load(Ordering::Relaxed);
            if slot_id == 0 {
                // Try to claim slot
                if self.device_ids[i].compare_exchange(
                    0, id.pack(),
                    Ordering::Release, Ordering::Relaxed
                ).is_ok() {
                    // Set capabilities
                    self.device_caps[i].store(caps.0, Ordering::Release);

                    // Update count and active bitmap
                    loop {
                        let old = self.state_gen.load(Ordering::Relaxed);
                        let old_count = ((old >> 8) & 0xFF) as u8;
                        let old_bitmap = ((old >> 16) & 0xFFFF) as u16;
                        let new_count = (old_count + 1) as u64;
                        let new_bitmap = (old_bitmap | (1 << i)) as u64;
                        let new = (old & 0xFFFF_FFFF_0000_00FF) |
                                  (new_count << 8) |
                                  (new_bitmap << 16);
                        if self.state_gen.compare_exchange_weak(
                            old, new, Ordering::Release, Ordering::Relaxed
                        ).is_ok() {
                            break;
                        }
                    }

                    // Increment generation
                    self.state_gen.fetch_add(1 << 32, Ordering::Release);

                    return Ok(i);
                }
            }
        }

        Err(InputError::MaxDevicesReached)
    }

    /// Unregister an input device
    ///
    /// # Performance
    /// - Typical: <30ns
    pub fn unregister_device(&self, index: usize) -> InputResult<()> {
        if index >= MAX_INPUT_DEVICES {
            return Err(InputError::InvalidDeviceIndex);
        }

        // Check if device exists
        let id = self.device_ids[index].load(Ordering::Relaxed);
        if id == 0 {
            return Err(InputError::DeviceNotFound);
        }

        // Clear device
        self.device_ids[index].store(0, Ordering::Release);
        self.device_caps[index].store(0, Ordering::Release);
        self.device_event_counts[index].store(0, Ordering::Release);

        // Update count and bitmap
        loop {
            let old = self.state_gen.load(Ordering::Relaxed);
            let old_count = ((old >> 8) & 0xFF) as u8;
            let old_bitmap = ((old >> 16) & 0xFFFF) as u16;
            let new_count = old_count.saturating_sub(1) as u64;
            let new_bitmap = (old_bitmap & !(1 << index)) as u64;
            let new = (old & 0xFFFF_FFFF_0000_00FF) |
                      (new_count << 8) |
                      (new_bitmap << 16);
            if self.state_gen.compare_exchange_weak(
                old, new, Ordering::Release, Ordering::Relaxed
            ).is_ok() {
                break;
            }
        }

        // Increment generation
        self.state_gen.fetch_add(1 << 32, Ordering::Release);

        Ok(())
    }

    /// Get device capabilities
    pub fn get_device_caps(&self, index: usize) -> Option<DeviceCapabilities> {
        if index >= MAX_INPUT_DEVICES {
            return None;
        }
        let caps = self.device_caps[index].load(Ordering::Acquire);
        if caps != 0 {
            Some(DeviceCapabilities(caps))
        } else {
            None
        }
    }

    /// Get device ID
    pub fn get_device_id(&self, index: usize) -> Option<InputDeviceId> {
        if index >= MAX_INPUT_DEVICES {
            return None;
        }
        let id = self.device_ids[index].load(Ordering::Acquire);
        if id != 0 {
            Some(InputDeviceId::unpack(id))
        } else {
            None
        }
    }

    /// Route event to appropriate handler
    ///
    /// Dispatches events to keyboard, mouse, or touchpad based on type.
    ///
    /// # Performance
    /// - Typical: <20ns
    ///
    /// #VERIFY[ROUTE-CORRECT]: Events routed to correct sub-capsule
    pub fn route_event(
        &self,
        event: &InputEvent,
        device_index: usize,
        keyboard: &KeyboardCapsule,
        mouse: &MouseCapsule,
        touchpad: &TouchpadCapsule,
    ) {
        // Update statistics
        self.events_processed.fetch_add(1, Ordering::Relaxed);
        if device_index < MAX_INPUT_DEVICES {
            self.device_event_counts[device_index].fetch_add(1, Ordering::Relaxed);
        }

        // Route based on event type
        match event.event_type() {
            EventType::Key => {
                // Could be keyboard key or mouse button
                let caps = if device_index < MAX_INPUT_DEVICES {
                    DeviceCapabilities(self.device_caps[device_index].load(Ordering::Relaxed))
                } else {
                    DeviceCapabilities::NONE
                };

                if caps.is_mouse() || super::mouse::MouseButton::is_mouse_button(event.code) {
                    mouse.process_event(event);
                    self.mouse_events.fetch_add(1, Ordering::Relaxed);
                } else {
                    keyboard.process_event(event);
                    self.keyboard_events.fetch_add(1, Ordering::Relaxed);
                }
            }
            EventType::RelativeMotion => {
                mouse.process_event(event);
                self.mouse_events.fetch_add(1, Ordering::Relaxed);
            }
            EventType::AbsoluteMotion => {
                touchpad.process_event(event);
                self.touch_events.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    /// Take atomic snapshot of subsystem state
    ///
    /// # Performance
    /// - Typical: <50ns
    #[inline]
    pub fn snapshot(&self) -> InputSubsystemSnapshot {
        let state_gen = self.state_gen.load(Ordering::Acquire);

        InputSubsystemSnapshot {
            state: InputSubsystemState::from_raw((state_gen & 0xFF) as u8),
            device_count: ((state_gen >> 8) & 0xFF) as u8,
            active_devices: ((state_gen >> 16) & 0xFFFF) as u16,
            events_processed: self.events_processed.load(Ordering::Relaxed),
            generation: state_gen >> 32,
        }
    }

    /// Get keyboard event count
    #[inline(always)]
    pub fn keyboard_event_count(&self) -> u64 {
        self.keyboard_events.load(Ordering::Relaxed)
    }

    /// Get mouse event count
    #[inline(always)]
    pub fn mouse_event_count(&self) -> u64 {
        self.mouse_events.load(Ordering::Relaxed)
    }

    /// Get touch event count
    #[inline(always)]
    pub fn touch_event_count(&self) -> u64 {
        self.touch_events.load(Ordering::Relaxed)
    }

    /// Get per-device event count
    pub fn device_event_count(&self, index: usize) -> u64 {
        if index < MAX_INPUT_DEVICES {
            self.device_event_counts[index].load(Ordering::Relaxed)
        } else {
            0
        }
    }

    /// Clear all state
    pub fn clear(&self) {
        self.events_processed.store(0, Ordering::Release);
        self.keyboard_events.store(0, Ordering::Release);
        self.mouse_events.store(0, Ordering::Release);
        self.touch_events.store(0, Ordering::Release);

        for i in 0..MAX_INPUT_DEVICES {
            self.device_ids[i].store(0, Ordering::Release);
            self.device_caps[i].store(0, Ordering::Release);
            self.device_event_counts[i].store(0, Ordering::Release);
            self.device_last_event[i].store(0, Ordering::Release);
            self.error_counts[i].store(0, Ordering::Release);
        }

        // Reset to uninitialized
        loop {
            let old = self.state_gen.load(Ordering::Relaxed);
            let gen = old >> 32;
            let new = ((gen + 1) << 32) | (InputSubsystemState::Uninitialized as u64);
            if self.state_gen.compare_exchange_weak(
                old, new, Ordering::Release, Ordering::Relaxed
            ).is_ok() {
                break;
            }
        }
    }

    /// Get generation counter
    #[inline(always)]
    pub fn generation(&self) -> u64 {
        self.state_gen.load(Ordering::Acquire) >> 32
    }
}

impl Default for InputSubsystemCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Send + Sync safety
unsafe impl Send for InputSubsystemCapsule {}
unsafe impl Sync for InputSubsystemCapsule {}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_id_pack() {
        let id = InputDeviceId::new(0x03, 0x1234, 0x5678, 0x0100);
        let packed = id.pack();
        let unpacked = InputDeviceId::unpack(packed);
        assert_eq!(unpacked.bus_type, 0x03);
        assert_eq!(unpacked.vendor, 0x1234);
        assert_eq!(unpacked.product, 0x5678);
        assert_eq!(unpacked.version, 0x0100);
    }

    #[test]
    fn test_capabilities() {
        let caps = DeviceCapabilities::KEY
            .with(DeviceCapabilities::RELATIVE)
            .with(DeviceCapabilities::SYNC);
        assert!(caps.is_mouse());
        assert!(!caps.is_keyboard());
        assert!(!caps.is_touchpad());
    }

    #[test]
    fn test_subsystem_new() {
        let sub = InputSubsystemCapsule::new();
        assert_eq!(sub.state(), InputSubsystemState::Uninitialized);
        assert_eq!(sub.device_count(), 0);
        assert_eq!(sub.events_processed(), 0);
    }

    #[test]
    fn test_initialize() {
        let sub = InputSubsystemCapsule::new();
        assert!(sub.initialize().is_ok());
        assert_eq!(sub.state(), InputSubsystemState::Ready);
    }

    #[test]
    fn test_register_device() {
        let sub = InputSubsystemCapsule::new();
        sub.initialize().unwrap();

        let id = InputDeviceId::new(0x03, 0x1234, 0x5678, 0x0100);
        let caps = DeviceCapabilities::KEY.with(DeviceCapabilities::REPEAT);

        let index = sub.register_device(id, caps).unwrap();
        assert_eq!(index, 0);
        assert_eq!(sub.device_count(), 1);
        assert!(sub.active_devices() & 1 != 0);

        let retrieved_id = sub.get_device_id(0).unwrap();
        assert_eq!(retrieved_id.vendor, 0x1234);

        let retrieved_caps = sub.get_device_caps(0).unwrap();
        assert!(retrieved_caps.is_keyboard());
    }

    #[test]
    fn test_unregister_device() {
        let sub = InputSubsystemCapsule::new();
        sub.initialize().unwrap();

        let id = InputDeviceId::new(0x03, 0x1234, 0x5678, 0x0100);
        let caps = DeviceCapabilities::KEY;

        let index = sub.register_device(id, caps).unwrap();
        assert_eq!(sub.device_count(), 1);

        sub.unregister_device(index).unwrap();
        assert_eq!(sub.device_count(), 0);
        assert!(sub.get_device_id(0).is_none());
    }

    #[test]
    fn test_snapshot() {
        let sub = InputSubsystemCapsule::new();
        sub.initialize().unwrap();

        let id = InputDeviceId::new(0x03, 0x1234, 0x5678, 0x0100);
        let caps = DeviceCapabilities::KEY;
        sub.register_device(id, caps).unwrap();

        let snapshot = sub.snapshot();
        assert_eq!(snapshot.state, InputSubsystemState::Ready);
        assert_eq!(snapshot.device_count, 1);
        assert_eq!(snapshot.active_devices & 1, 1);
    }

    #[test]
    fn test_capsule_size_alignment() {
        use core::mem::{size_of, align_of};

        assert_eq!(size_of::<InputSubsystemCapsule>(), 1024);
        assert_eq!(align_of::<InputSubsystemCapsule>(), 1024);
    }
}
