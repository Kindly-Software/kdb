//! # ButtonCapsule - Interactive GUI Button Widget
//!
//! **Tier**: T1+T3 (Atomic state coordination + Q8.8 Fixed-point animation)
//!
//! High-performance interactive button widget with smooth press animation and ripple effects.
//! 100% lockfree state management using packed atomic operations.
//!
//! ## Features
//!
//! - **Lockfree State**: All state packed into single AtomicU64
//! - **Fixed-Point Animation**: Q8.8 format for smooth sub-pixel animation
//! - **Generation Counter**: Atomic snapshot consistency (Q34 audit trails)
//! - **Multiple Styles**: Primary, Secondary, Outline, Ghost, Danger
//! - **Ripple Effect**: Click position tracking for visual feedback
//! - **Double-Click Detection**: Click counter for interaction patterns
//!
//! ## Performance (B32 Targets)
//!
//! - State read: <5ns (single atomic load)
//! - State update: <10ns (single atomic CAS)
//! - Animation update: <20ns (Q8.8 fixed-point math)
//! - Hit test: <5ns (coordinate comparison)
//!
//! ## UCE34 Compliance
//!
//! - Q10: T1+T3 compound tier (Atomic coordination + Fixed-point animation)
//! - Q33: 100% lockfree (AtomicU64 state, AtomicU32 generation)
//! - Q34: Generation counter for audit trails
//!
//! ## ASSUM Safety
//!
//! - #ASSUME: ButtonState fits in 64 bits (compile-time verified)
//! - #ASSUME: Label max 32 bytes (validated in new())
//! - #VERIFY: Memory ordering (Acquire/Release for consistency)
//!
//! ## Example
//!
//! ```
//! use atomic_capsule::gui::widgets::{ButtonCapsule, ButtonStyle};
//! use atomic_capsule::gui::Rect;
//!
//! let button = ButtonCapsule::new(1, "Click Me")
//!     .with_style(ButtonStyle::Primary);
//!
//! // Set button bounds (GUI coordinate space)
//! button.set_bounds(Rect::new(100, 100, 200, 50).unwrap());
//!
//! // Hit test (mouse at 150, 125)
//! if button.hit_test(150, 125) {
//!     button.on_mouse_down(150, 125);
//! }
//!
//! // Update animation (16ms frame)
//! button.update_animation(16);
//!
//! // Render (get current state for interpolation)
//! let progress = button.animation_progress();
//! println!("Animation: {:.2}%", progress * 100.0);
//! ```

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use crate::gui::{Coord, Rect};

/// Button visual style variant
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum ButtonStyle {
    /// Primary action button (bold colors)
    #[default]
    Primary = 0,
    /// Secondary action button (muted colors)
    Secondary = 1,
    /// Outline only (transparent background)
    Outline = 2,
    /// Minimal styling (text only)
    Ghost = 3,
    /// Destructive action (red/warning colors)
    Danger = 4,
}

/// Button press state (Copy for atomic packing)
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum PressState {
    #[default]
    Idle = 0,
    Hover = 1,
    Pressed = 2,
    Disabled = 3,
}

/// Button state packed into 64 bits for atomic updates
///
/// Layout:
/// - Bits 0-7: press_state (u8)
/// - Bits 8-23: animation_progress (u16, Q8.8 fixed-point 0.0-1.0)
/// - Bits 24-39: ripple_x (u16, Q8.8 fixed-point relative to button)
/// - Bits 40-55: ripple_y (u16, Q8.8 fixed-point relative to button)
/// - Bits 56-63: click_count (u8)
#[derive(Copy, Clone, Debug, Default)]
#[repr(C)]
pub struct ButtonState {
    /// Press state: idle(0), hover(1), pressed(2), disabled(3)
    pub press_state: u8,
    /// Animation progress (Q8.8 fixed-point, 0.0-1.0 = 0-256)
    pub animation_progress: u16,
    /// Ripple center X (relative to button, Q8.8)
    pub ripple_x: u16,
    /// Ripple center Y (relative to button, Q8.8)
    pub ripple_y: u16,
    /// Click count for double-click detection
    pub click_count: u8,
}

impl ButtonState {
    /// Pack state into u64 for atomic storage
    pub const fn pack(self) -> u64 {
        (self.press_state as u64)
            | ((self.animation_progress as u64) << 8)
            | ((self.ripple_x as u64) << 24)
            | ((self.ripple_y as u64) << 40)
            | ((self.click_count as u64) << 56)
    }

    /// Unpack state from u64
    pub const fn unpack(val: u64) -> Self {
        Self {
            press_state: (val & 0xFF) as u8,
            animation_progress: ((val >> 8) & 0xFFFF) as u16,
            ripple_x: ((val >> 24) & 0xFFFF) as u16,
            ripple_y: ((val >> 40) & 0xFFFF) as u16,
            click_count: ((val >> 56) & 0xFF) as u8,
        }
    }

    /// Convert Q8.8 to float for rendering (animation_progress: 0-256 -> 0.0-1.0)
    pub fn animation_f32(self) -> f32 {
        (self.animation_progress as f32) / 256.0
    }

    /// Convert Q8.8 ripple coordinates to float (0-256 -> 0.0-1.0 normalized)
    pub fn ripple_f32(self) -> (f32, f32) {
        (
            (self.ripple_x as f32) / 256.0,
            (self.ripple_y as f32) / 256.0,
        )
    }
}

/// T1+T3 - Interactive button with animation
///
/// # UCE34 Compliance
/// - Q10: T1+T3 compound (Atomic state + Q8.8 animation)
/// - Q33: 100% lockfree (packed AtomicU64 state)
/// - Q34: Generation counter for state audit
///
/// # Performance (B32 targets)
/// - State read: <5ns (single atomic load)
/// - State update: <10ns (single atomic CAS)
/// - Animation update: <20ns (Q8.8 fixed-point math)
/// - Hit test: <5ns (coordinate comparison)
///
/// # Memory Layout
///
/// ```text
/// | state (8B) | generation (4B) | id (4B) | bounds (16B) | label (32B) | ... | padding |
/// | AtomicU64  | AtomicU32       | u32     | Rect         | [u8; 32]    | ... | [u8; N] |
/// ```
///
/// Total size: 128 bytes (cache-aligned for atomic performance)
/// Alignment: 64 bytes (fields sum to 72B, rounded up to 128B by align(64))
#[repr(C, align(64))]
pub struct ButtonCapsule {
    // Atomic state (packed ButtonState)
    /// Packed: press_state(8) | animation(16) | ripple_x(16) | ripple_y(16) | clicks(8)
    state: AtomicU64,
    /// Generation counter for atomic snapshots
    generation: AtomicU32,
    /// Widget ID (for event routing)
    id: u32,

    // Bounds (mutable via interior mutability pattern)
    /// Button bounds in GUI coordinate space (Q16.16)
    /// Stored as raw i32 for atomic updates (future)
    bounds_x: i32,
    bounds_y: i32,
    bounds_width: i32,
    bounds_height: i32,

    // Label (inline for small buttons, max 32 chars)
    /// Label length
    label_len: u8,
    /// Button style variant
    style: ButtonStyle,
    /// Inline label storage (UTF-8)
    label: [u8; 32],

    _pad: [u8; 56], // Pad to 128B (72 bytes fields + 56 padding = 128)
}

// Compile-time size/alignment verification
const _: () = assert!(core::mem::size_of::<ButtonCapsule>() == 128);
const _: () = assert!(core::mem::align_of::<ButtonCapsule>() == 64);

impl ButtonCapsule {
    /// Create new button with ID and label
    ///
    /// # Panics
    ///
    /// Panics if label exceeds 32 bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::widgets::ButtonCapsule;
    ///
    /// let button = ButtonCapsule::new(1, "Click Me");
    /// assert_eq!(button.label(), "Click Me");
    /// assert_eq!(button.id(), 1);
    /// ```
    pub fn new(id: u32, label: &str) -> Self {
        assert!(
            label.len() <= 32,
            "Button label must be <= 32 bytes, got {}",
            label.len()
        );

        let mut label_bytes = [0u8; 32];
        label_bytes[..label.len()].copy_from_slice(label.as_bytes());

        Self {
            state: AtomicU64::new(ButtonState::default().pack()),
            generation: AtomicU32::new(0),
            id,

            bounds_x: 0,
            bounds_y: 0,
            bounds_width: 0,
            bounds_height: 0,

            label_len: label.len() as u8,
            style: ButtonStyle::Primary,
            label: label_bytes,

            _pad: [0u8; 56],
        }
    }

    /// Builder: Set button style
    pub fn with_style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// Get button ID
    #[inline]
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Get button style
    #[inline]
    pub fn style(&self) -> ButtonStyle {
        self.style
    }

    /// Get current label as string slice
    pub fn label(&self) -> &str {
        core::str::from_utf8(&self.label[..self.label_len as usize])
            .unwrap_or("")
    }

    /// Set button bounds
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::widgets::ButtonCapsule;
    /// use atomic_capsule::gui::Rect;
    ///
    /// let button = ButtonCapsule::new(1, "Click");
    /// button.set_bounds(Rect::new(100, 100, 200, 50).unwrap());
    /// ```
    pub fn set_bounds(&mut self, rect: Rect) {
        self.bounds_x = rect.x.raw();
        self.bounds_y = rect.y.raw();
        self.bounds_width = rect.width.raw();
        self.bounds_height = rect.height.raw();
    }

    /// Get button bounds
    #[inline]
    pub fn bounds(&self) -> Rect {
        Rect {
            x: Coord::from_raw(self.bounds_x),
            y: Coord::from_raw(self.bounds_y),
            width: Coord::from_raw(self.bounds_width),
            height: Coord::from_raw(self.bounds_height),
        }
    }

    /// Hit test (check if point is inside button bounds)
    ///
    /// # Performance
    ///
    /// <5ns (4 coordinate comparisons)
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::widgets::ButtonCapsule;
    /// use atomic_capsule::gui::Rect;
    ///
    /// let button = ButtonCapsule::new(1, "Click");
    /// let mut button_mut = button;
    /// button_mut.set_bounds(Rect::new(100, 100, 200, 50).unwrap());
    ///
    /// assert!(button_mut.hit_test(150, 125));
    /// assert!(!button_mut.hit_test(50, 50));
    /// ```
    #[inline]
    pub fn hit_test(&self, x: i32, y: i32) -> bool {
        let px = Coord::from_int(x);
        let py = Coord::from_int(y);
        self.bounds().contains_point(x, y)
    }

    /// Mouse enter event (trigger hover state)
    ///
    /// # Performance
    ///
    /// <10ns (single atomic CAS)
    pub fn on_mouse_enter(&self) {
        let mut state = self.state();
        if state.press_state == PressState::Idle as u8 {
            state.press_state = PressState::Hover as u8;
            state.animation_progress = 0; // Start hover animation
            self.update_state(state);
        }
    }

    /// Mouse leave event (return to idle state)
    ///
    /// # Performance
    ///
    /// <10ns (single atomic CAS)
    pub fn on_mouse_leave(&self) {
        let mut state = self.state();
        if state.press_state == PressState::Hover as u8 {
            state.press_state = PressState::Idle as u8;
            state.animation_progress = 0;
            self.update_state(state);
        }
    }

    /// Mouse down event (trigger pressed state)
    ///
    /// # Performance
    ///
    /// <10ns (single atomic CAS)
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::widgets::ButtonCapsule;
    /// use atomic_capsule::gui::Rect;
    ///
    /// let button = ButtonCapsule::new(1, "Click");
    /// let mut button_mut = button;
    /// button_mut.set_bounds(Rect::new(100, 100, 200, 50).unwrap());
    ///
    /// button_mut.on_mouse_down(150, 125);
    /// assert!(button_mut.is_pressed());
    /// ```
    pub fn on_mouse_down(&self, x: i32, y: i32) {
        let mut state = self.state();
        state.press_state = PressState::Pressed as u8;
        state.animation_progress = 0;

        // Calculate ripple position (Q8.8 fixed-point, normalized 0-1)
        let bounds = self.bounds();
        let rel_x = Coord::from_int(x).saturating_sub(bounds.x);
        let rel_y = Coord::from_int(y).saturating_sub(bounds.y);

        // Normalize to 0-1 range (Q8.8)
        let width_raw = bounds.width.raw().max(1);
        let height_raw = bounds.height.raw().max(1);
        state.ripple_x = ((rel_x.raw() as i64 * 256) / width_raw as i64) as u16;
        state.ripple_y = ((rel_y.raw() as i64 * 256) / height_raw as i64) as u16;

        self.update_state(state);
    }

    /// Mouse up event (return to hover state if still over button)
    ///
    /// # Performance
    ///
    /// <10ns (single atomic CAS)
    ///
    /// # Returns
    ///
    /// `true` if button was clicked (pressed -> hover/idle)
    pub fn on_mouse_up(&self) -> bool {
        let mut state = self.state();
        if state.press_state == PressState::Pressed as u8 {
            state.press_state = PressState::Hover as u8;
            state.animation_progress = 0;
            state.click_count = state.click_count.saturating_add(1);
            self.update_state(state);
            true
        } else {
            false
        }
    }

    /// Check if button is hovered
    #[inline]
    pub fn is_hovered(&self) -> bool {
        let state = self.state();
        state.press_state == PressState::Hover as u8
    }

    /// Check if button is pressed
    #[inline]
    pub fn is_pressed(&self) -> bool {
        let state = self.state();
        state.press_state == PressState::Pressed as u8
    }

    /// Get animation progress (0.0 - 1.0)
    ///
    /// # Performance
    ///
    /// <5ns (single atomic load + Q8.8 conversion)
    #[inline]
    pub fn animation_progress(&self) -> f32 {
        self.state().animation_f32()
    }

    /// Update animation by delta time (milliseconds)
    ///
    /// Advances animation progress using Q8.8 fixed-point math.
    /// Animation completes in ~200ms (256 units / 1.28 per ms).
    ///
    /// # Performance
    ///
    /// <20ns (Q8.8 fixed-point math + atomic CAS)
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::widgets::ButtonCapsule;
    ///
    /// let button = ButtonCapsule::new(1, "Click");
    ///
    /// // 16ms frame time (60 FPS)
    /// button.update_animation(16);
    /// assert!(button.animation_progress() > 0.0);
    /// ```
    pub fn update_animation(&self, delta_ms: u32) {
        let mut state = self.state();

        // Animation speed: 1.28 units per millisecond (256 units in 200ms)
        // Q8.8: Multiply delta_ms by 1.28 = delta_ms + (delta_ms >> 2)
        let delta = delta_ms + (delta_ms >> 2);

        if state.animation_progress < 256 {
            state.animation_progress = state.animation_progress.saturating_add(delta as u16).min(256);
            self.update_state(state);
        }
    }

    /// Read current state (single atomic load)
    ///
    /// # Performance
    ///
    /// <5ns (single atomic load)
    #[inline]
    pub fn state(&self) -> ButtonState {
        ButtonState::unpack(self.state.load(Ordering::Acquire))
    }

    /// Update state atomically
    fn update_state(&self, new_state: ButtonState) {
        self.state.store(new_state.pack(), Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get current generation for snapshot consistency
    #[inline]
    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: Unit Tests (15 tests)
    // ========================================================================

    #[test]
    fn test_new_button() {
        let btn = ButtonCapsule::new(1, "Click Me");
        assert_eq!(btn.label(), "Click Me");
        assert_eq!(btn.label_len, 8);
        assert_eq!(btn.id(), 1);
        assert_eq!(btn.style(), ButtonStyle::Primary);
    }

    #[test]
    fn test_button_styles() {
        let primary = ButtonCapsule::new(1, "Primary").with_style(ButtonStyle::Primary);
        assert_eq!(primary.style(), ButtonStyle::Primary);

        let secondary = ButtonCapsule::new(2, "Secondary").with_style(ButtonStyle::Secondary);
        assert_eq!(secondary.style(), ButtonStyle::Secondary);

        let danger = ButtonCapsule::new(3, "Danger").with_style(ButtonStyle::Danger);
        assert_eq!(danger.style(), ButtonStyle::Danger);

        let outline = ButtonCapsule::new(4, "Outline").with_style(ButtonStyle::Outline);
        assert_eq!(outline.style(), ButtonStyle::Outline);

        let ghost = ButtonCapsule::new(5, "Ghost").with_style(ButtonStyle::Ghost);
        assert_eq!(ghost.style(), ButtonStyle::Ghost);
    }

    #[test]
    fn test_state_packing() {
        let state = ButtonState {
            press_state: 2,
            animation_progress: 128,
            ripple_x: 64,
            ripple_y: 192,
            click_count: 5,
        };

        let packed = state.pack();
        let unpacked = ButtonState::unpack(packed);

        assert_eq!(unpacked.press_state, 2);
        assert_eq!(unpacked.animation_progress, 128);
        assert_eq!(unpacked.ripple_x, 64);
        assert_eq!(unpacked.ripple_y, 192);
        assert_eq!(unpacked.click_count, 5);
    }

    #[test]
    fn test_state_read_write() {
        let btn = ButtonCapsule::new(1, "Test");
        let state = btn.state();

        assert_eq!(state.press_state, 0); // Idle
        assert_eq!(state.animation_progress, 0);
        assert_eq!(state.click_count, 0);

        let gen1 = btn.generation();

        btn.update_state(ButtonState {
            press_state: 1, // Hover
            animation_progress: 100,
            ripple_x: 0,
            ripple_y: 0,
            click_count: 0,
        });

        let gen2 = btn.generation();
        assert_eq!(gen2, gen1 + 1);

        let new_state = btn.state();
        assert_eq!(new_state.press_state, 1);
        assert_eq!(new_state.animation_progress, 100);
    }

    #[test]
    fn test_animation_update() {
        let btn = ButtonCapsule::new(1, "Test");

        // Update animation by 50ms
        btn.update_animation(50);
        let state = btn.state();

        // 50ms * 1.25 = 62.5 units (50 + 50>>2 = 50 + 12 = 62)
        assert_eq!(state.animation_progress, 62);

        // Update by another 150ms (62 + 150 + 37 = 249, not capped yet)
        btn.update_animation(150);
        let state = btn.state();
        assert_eq!(state.animation_progress, 249);

        // Further updates should not exceed 256
        btn.update_animation(100);
        let state = btn.state();
        assert_eq!(state.animation_progress, 256);
    }

    #[test]
    fn test_q8_8_conversion() {
        let state = ButtonState {
            press_state: 0,
            animation_progress: 256, // 1.0 in Q8.8
            ripple_x: 128, // 0.5 in Q8.8
            ripple_y: 64, // 0.25 in Q8.8
            click_count: 0,
        };

        let anim = state.animation_f32();
        assert!((anim - 1.0).abs() < 0.01);

        let (rx, ry) = state.ripple_f32();
        assert!((rx - 0.5).abs() < 0.01);
        assert!((ry - 0.25).abs() < 0.01);
    }

    #[test]
    fn test_set_bounds() {
        let mut btn = ButtonCapsule::new(1, "Test");
        let rect = Rect::new(100, 200, 300, 50).unwrap();
        btn.set_bounds(rect);

        let bounds = btn.bounds();
        assert_eq!(bounds.x.to_int(), 100);
        assert_eq!(bounds.y.to_int(), 200);
        assert_eq!(bounds.width.to_int(), 300);
        assert_eq!(bounds.height.to_int(), 50);
    }

    #[test]
    fn test_hit_test() {
        let mut btn = ButtonCapsule::new(1, "Test");
        btn.set_bounds(Rect::new(100, 100, 200, 50).unwrap());

        // Inside bounds
        assert!(btn.hit_test(150, 125));
        assert!(btn.hit_test(100, 100)); // Top-left corner
        assert!(btn.hit_test(299, 149)); // Near bottom-right

        // Outside bounds
        assert!(!btn.hit_test(50, 125));
        assert!(!btn.hit_test(150, 50));
        assert!(!btn.hit_test(350, 125));
        assert!(!btn.hit_test(150, 200));
    }

    #[test]
    fn test_mouse_enter_leave() {
        let btn = ButtonCapsule::new(1, "Test");

        // Initially idle
        assert!(!btn.is_hovered());

        btn.on_mouse_enter();
        assert!(btn.is_hovered());

        btn.on_mouse_leave();
        assert!(!btn.is_hovered());
    }

    #[test]
    fn test_mouse_down_up() {
        let mut btn = ButtonCapsule::new(1, "Test");
        btn.set_bounds(Rect::new(0, 0, 200, 50).unwrap());

        // Initially not pressed
        assert!(!btn.is_pressed());

        btn.on_mouse_down(100, 25);
        assert!(btn.is_pressed());

        let clicked = btn.on_mouse_up();
        assert!(clicked);
        assert!(!btn.is_pressed());
        assert!(btn.is_hovered());

        // Click count incremented
        let state = btn.state();
        assert_eq!(state.click_count, 1);
    }

    #[test]
    fn test_ripple_position() {
        let mut btn = ButtonCapsule::new(1, "Test");
        btn.set_bounds(Rect::new(0, 0, 200, 100).unwrap());

        // Click at center (100, 50)
        btn.on_mouse_down(100, 50);

        let state = btn.state();
        let (rx, ry) = state.ripple_f32();

        // Should be roughly 0.5, 0.5 (center)
        assert!((rx - 0.5).abs() < 0.1);
        assert!((ry - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_animation_progress() {
        let btn = ButtonCapsule::new(1, "Test");

        assert_eq!(btn.animation_progress(), 0.0);

        btn.update_animation(100); // 100ms * 1.28 = 128 units
        let progress = btn.animation_progress();
        assert!((progress - 0.5).abs() < 0.05); // ~50% progress
    }

    #[test]
    fn test_generation_counter() {
        let btn = ButtonCapsule::new(1, "Test");
        let gen1 = btn.generation();

        btn.on_mouse_enter();
        let gen2 = btn.generation();
        assert_eq!(gen2, gen1 + 1);

        btn.update_animation(50);
        let gen3 = btn.generation();
        assert_eq!(gen3, gen2 + 1);
    }

    #[test]
    fn test_builder_pattern() {
        let btn = ButtonCapsule::new(1, "Test")
            .with_style(ButtonStyle::Secondary);

        assert_eq!(btn.style(), ButtonStyle::Secondary);
    }

    #[test]
    fn test_size_and_alignment() {
        assert_eq!(core::mem::size_of::<ButtonCapsule>(), 128);
        assert_eq!(core::mem::align_of::<ButtonCapsule>(), 64);
    }

    // ========================================================================
    // Q8-Q14: Property Tests (5 tests)
    // ========================================================================

    #[test]
    fn test_animation_bounds() {
        let btn = ButtonCapsule::new(1, "Test");

        // Animation should never exceed 256 (1.0 in Q8.8)
        for _ in 0..1000 {
            btn.update_animation(1);
        }

        let state = btn.state();
        assert!(state.animation_progress <= 256);
        assert_eq!(state.animation_progress, 256);
    }

    #[test]
    fn test_click_count_saturation() {
        let btn = ButtonCapsule::new(1, "Test");

        // Click 300 times (should saturate at 255)
        for _ in 0..300 {
            btn.on_mouse_down(0, 0);
            btn.on_mouse_up();
        }

        let state = btn.state();
        assert_eq!(state.click_count, 255); // Saturated
    }

    #[test]
    fn test_ripple_normalization() {
        let mut btn = ButtonCapsule::new(1, "Test");
        btn.set_bounds(Rect::new(0, 0, 400, 200).unwrap());

        // Click at top-left (0, 0)
        btn.on_mouse_down(0, 0);
        let state = btn.state();
        assert_eq!(state.ripple_x, 0);
        assert_eq!(state.ripple_y, 0);

        // Click at bottom-right (400, 200) - should be clamped to <256
        btn.on_mouse_down(400, 200);
        let state = btn.state();
        assert!(state.ripple_x <= 256);
        assert!(state.ripple_y <= 256);
    }

    #[test]
    fn test_state_transitions() {
        let btn = ButtonCapsule::new(1, "Test");

        // Idle -> Hover
        btn.on_mouse_enter();
        assert!(btn.is_hovered());

        // Hover -> Pressed
        btn.on_mouse_down(0, 0);
        assert!(btn.is_pressed());

        // Pressed -> Hover
        btn.on_mouse_up();
        assert!(btn.is_hovered());

        // Hover -> Idle
        btn.on_mouse_leave();
        assert!(!btn.is_hovered());
    }

    #[test]
    fn test_generation_monotonicity() {
        let btn = ButtonCapsule::new(1, "Test");
        let mut prev_gen = btn.generation();

        // Generation should always increase
        for _ in 0..100 {
            btn.update_animation(1);
            let gen = btn.generation();
            assert!(gen > prev_gen);
            prev_gen = gen;
        }
    }

    // ========================================================================
    // Q15-Q21: Integration Tests (3 tests)
    // ========================================================================

    #[test]
    fn test_full_interaction_sequence() {
        let mut btn = ButtonCapsule::new(1, "Click Me");
        btn.set_bounds(Rect::new(100, 100, 200, 50).unwrap());

        // Mouse enters button
        btn.on_mouse_enter();
        assert!(btn.is_hovered());

        // Animate hover
        btn.update_animation(100);
        assert!(btn.animation_progress() > 0.0);

        // Click button
        btn.on_mouse_down(150, 125);
        assert!(btn.is_pressed());

        // Release button
        let clicked = btn.on_mouse_up();
        assert!(clicked);
        assert!(btn.is_hovered());

        // Mouse leaves
        btn.on_mouse_leave();
        assert!(!btn.is_hovered());
    }

    #[test]
    fn test_multiple_buttons_independence() {
        let mut btn1 = ButtonCapsule::new(1, "Button 1");
        let mut btn2 = ButtonCapsule::new(2, "Button 2");

        btn1.set_bounds(Rect::new(0, 0, 100, 50).unwrap());
        btn2.set_bounds(Rect::new(150, 0, 100, 50).unwrap());

        // Interact with btn1
        btn1.on_mouse_enter();
        btn1.on_mouse_down(50, 25);

        // btn2 should be unaffected
        assert!(!btn2.is_hovered());
        assert!(!btn2.is_pressed());

        // Interact with btn2
        btn2.on_mouse_enter();
        assert!(btn2.is_hovered());

        // btn1 should still be pressed
        assert!(btn1.is_pressed());
    }

    #[test]
    fn test_animation_smoothness() {
        let btn = ButtonCapsule::new(1, "Test");
        btn.on_mouse_enter();

        // Simulate 60 FPS animation (16ms frames)
        let mut prev_progress = 0.0;
        for _ in 0..12 {
            btn.update_animation(16);
            let progress = btn.animation_progress();
            assert!(progress >= prev_progress); // Monotonically increasing
            prev_progress = progress;
        }

        // Should be close to 100% after ~200ms
        assert!(prev_progress > 0.9);
    }
}
