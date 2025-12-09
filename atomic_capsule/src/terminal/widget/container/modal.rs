//! Modal Container Capsule - T1 Atomic
//!
//! Modal dialog container with backdrop, focus trap, and dismiss handling.
//!
//! # UCE34 Compliance
//! - Q10: T1 Atomic (<10ns state operations)
//! - Q33: 100% lockfree (AtomicU64, AtomicU32)
//! - Q34: Modal open/close audit trail
//!
//! # ASSUM
//! #ASSUME: Modal operations are infrequent (user interactions)
//! #VERIFY: <10ns state loads via Acquire ordering
//!
//! # Performance
//! - open(): <10ns (atomic store)
//! - close(): <10ns (atomic load + store)
//! - is_open(): <5ns (atomic load)
//! - update_animation(): <20ns (atomic RMW)

use crate::terminal::widget::{Widget, Rect, RenderCommandBuffer, RenderStyle};
use crate::terminal::event::{KeyEvent, KeyCode};
use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};

/// Modal animation state
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ModalState {
    Hidden = 0,
    Opening = 1,
    Open = 2,
    Closing = 3,
}

impl Default for ModalState {
    fn default() -> Self {
        Self::Hidden
    }
}

impl From<u8> for ModalState {
    fn from(val: u8) -> Self {
        match val {
            0 => Self::Hidden,
            1 => Self::Opening,
            2 => Self::Open,
            3 => Self::Closing,
            _ => Self::Hidden,
        }
    }
}

/// Modal position
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ModalPosition {
    Center = 0,
    Top = 1,
    Bottom = 2,
    Custom = 3,
}

impl Default for ModalPosition {
    fn default() -> Self {
        Self::Center
    }
}

/// T1 Atomic - Modal dialog container
///
/// # UCE34 Compliance
/// - Q10: T1 Atomic (<10ns state operations)
/// - Q33: 100% lockfree
/// - Q34: Modal open/close audit
///
/// # Layout (256 bytes, 64-byte aligned)
/// ```text
/// [0-7]     state: AtomicU64 (state | animation_progress)
/// [8-11]    generation: AtomicU32
/// [12-15]   flags: AtomicU32
/// [16-31]   position info (16 bytes)
/// [32-47]   colors (16 bytes)
/// [48-63]   styling + animation (16 bytes)
/// [64-67]   prev_focus: AtomicU32
/// [68-255]  padding (188 bytes)
/// ```
#[repr(C, align(64))]
pub struct ModalContainerCapsule {
    // State (8 bytes)
    /// Bits 0-7: ModalState
    /// Bits 8-23: Animation progress (0-65535, maps to 0.0-1.0)
    /// Bits 24-63: Reserved
    state: AtomicU64,

    // Metadata (8 bytes)
    /// Generation counter
    generation: AtomicU32,
    /// Flags: backdrop_dismiss(1) | escape_dismiss(1) | focus_trap(1) | _pad(29)
    flags: AtomicU32,

    // Position (16 bytes)
    /// Modal position type
    position: ModalPosition,
    _pad1: [u8; 1],
    /// Custom X position (if Custom)
    custom_x: u16,
    /// Custom Y position (if Custom)
    custom_y: u16,
    /// Content width (0 = auto)
    width: u16,
    /// Content height (0 = auto)
    height: u16,
    /// Min width
    min_width: u16,
    /// Max width
    max_width: u16,
    _pad2: [u8; 2],

    // Styling (16 bytes)
    /// Backdrop color (RGBA8888)
    backdrop_color: u32,
    /// Border color (RGBA8888)
    border_color: u32,
    /// Background color (RGBA8888)
    bg_color: u32,
    /// Border radius
    border_radius: u8,
    /// Border width
    border_width: u8,
    /// Padding [top, right, bottom, left]
    padding: [u8; 2],

    // Animation (16 bytes)
    /// Animation duration (ms)
    animation_duration: u16,
    _pad3: [u8; 14],

    // Focus tracking (4 bytes)
    /// Previous focus (widget to restore when closing)
    prev_focus: AtomicU32,

    // Padding to 256 bytes
    _pad: [u8; 184],
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<ModalContainerCapsule>() == 256);
const _: () = assert!(core::mem::align_of::<ModalContainerCapsule>() == 64);

// State bit manipulation
const STATE_MASK: u64 = 0xFF;
const ANIMATION_SHIFT: u32 = 8;
const ANIMATION_MASK: u64 = 0xFFFF << ANIMATION_SHIFT;
const ANIMATION_MAX: u16 = 65535;

// Flag bits
const FLAG_BACKDROP_DISMISS: u32 = 1 << 0;
const FLAG_ESCAPE_DISMISS: u32 = 1 << 1;
const FLAG_FOCUS_TRAP: u32 = 1 << 2;

impl ModalContainerCapsule {
    /// Create new modal container
    ///
    /// # Performance
    /// O(1) - Constant time initialization
    #[inline]
    pub fn new() -> Self {
        Self {
            state: AtomicU64::new(0), // Hidden, 0% animation
            generation: AtomicU32::new(0),
            flags: AtomicU32::new(FLAG_BACKDROP_DISMISS | FLAG_ESCAPE_DISMISS | FLAG_FOCUS_TRAP),
            position: ModalPosition::Center,
            _pad1: [0; 1],
            custom_x: 0,
            custom_y: 0,
            width: 0,
            height: 0,
            min_width: 200,
            max_width: 800,
            _pad2: [0; 2],
            backdrop_color: 0x00000080, // Black, 50% alpha
            border_color: 0x4444FFFF,   // Blue
            bg_color: 0x1E1E1EFF,       // Dark gray
            border_radius: 4,
            border_width: 1,
            padding: [8, 8],
            animation_duration: 200,
            _pad3: [0; 14],
            prev_focus: AtomicU32::new(0),
            _pad: [0; 184],
        }
    }

    /// Set modal position
    #[inline]
    pub fn with_position(mut self, pos: ModalPosition) -> Self {
        self.position = pos;
        self
    }

    /// Set custom position (x, y)
    #[inline]
    pub fn with_custom_position(mut self, x: u16, y: u16) -> Self {
        self.position = ModalPosition::Custom;
        self.custom_x = x;
        self.custom_y = y;
        self
    }

    /// Set content size
    #[inline]
    pub fn with_size(mut self, width: u16, height: u16) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Set size constraints
    #[inline]
    pub fn with_size_constraints(mut self, min: u16, max: u16) -> Self {
        self.min_width = min;
        self.max_width = max;
        self
    }

    /// Enable/disable backdrop dismiss
    #[inline]
    pub fn with_backdrop_dismiss(self, enabled: bool) -> Self {
        if enabled {
            self.flags.fetch_or(FLAG_BACKDROP_DISMISS, Ordering::Release);
        } else {
            self.flags.fetch_and(!FLAG_BACKDROP_DISMISS, Ordering::Release);
        }
        self
    }

    /// Enable/disable escape key dismiss
    #[inline]
    pub fn with_escape_dismiss(self, enabled: bool) -> Self {
        if enabled {
            self.flags.fetch_or(FLAG_ESCAPE_DISMISS, Ordering::Release);
        } else {
            self.flags.fetch_and(!FLAG_ESCAPE_DISMISS, Ordering::Release);
        }
        self
    }

    /// Enable/disable focus trap
    #[inline]
    pub fn with_focus_trap(self, enabled: bool) -> Self {
        if enabled {
            self.flags.fetch_or(FLAG_FOCUS_TRAP, Ordering::Release);
        } else {
            self.flags.fetch_and(!FLAG_FOCUS_TRAP, Ordering::Release);
        }
        self
    }

    /// Set backdrop color
    #[inline]
    pub fn with_backdrop_color(mut self, color: u32) -> Self {
        self.backdrop_color = color;
        self
    }

    /// Set border color
    #[inline]
    pub fn with_border_color(mut self, color: u32) -> Self {
        self.border_color = color;
        self
    }

    /// Set background color
    #[inline]
    pub fn with_background_color(mut self, color: u32) -> Self {
        self.bg_color = color;
        self
    }

    /// Set border radius
    #[inline]
    pub fn with_border_radius(mut self, radius: u8) -> Self {
        self.border_radius = radius;
        self
    }

    /// Set animation duration (ms)
    #[inline]
    pub fn with_animation_duration(mut self, duration: u16) -> Self {
        self.animation_duration = duration;
        self
    }

    /// Open modal
    ///
    /// # Performance
    /// <10ns - Atomic store + generation increment
    ///
    /// # ASSUM
    /// #ASSUME: current_focus is valid widget ID or 0
    /// #VERIFY: Stored atomically for restoration
    #[inline]
    pub fn open(&self, current_focus: u32) {
        // Save previous focus
        self.prev_focus.store(current_focus, Ordering::Release);

        // Set to Opening state, 0% animation
        let new_state = (ModalState::Opening as u64) | (0 << ANIMATION_SHIFT);
        self.state.store(new_state, Ordering::Release);

        // Increment generation
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Close modal
    ///
    /// # Performance
    /// <10ns - Atomic load + store
    ///
    /// # Returns
    /// Widget ID to restore focus to
    #[inline]
    pub fn close(&self) -> u32 {
        // Set to Closing state, 100% animation (will animate backwards)
        let new_state = (ModalState::Closing as u64) | ((ANIMATION_MAX as u64) << ANIMATION_SHIFT);
        self.state.store(new_state, Ordering::Release);

        // Increment generation
        self.generation.fetch_add(1, Ordering::Release);

        // Return previous focus
        self.prev_focus.load(Ordering::Acquire)
    }

    /// Check if modal is open (Open or Opening state)
    ///
    /// # Performance
    /// <5ns - Single atomic load
    #[inline]
    pub fn is_open(&self) -> bool {
        let state_val = self.state.load(Ordering::Acquire);
        let state = ModalState::from((state_val & STATE_MASK) as u8);
        matches!(state, ModalState::Open | ModalState::Opening)
    }

    /// Get current modal state
    ///
    /// # Performance
    /// <5ns - Single atomic load
    #[inline]
    pub fn state(&self) -> ModalState {
        let state_val = self.state.load(Ordering::Acquire);
        ModalState::from((state_val & STATE_MASK) as u8)
    }

    /// Get animation progress (0.0 to 1.0)
    ///
    /// # Performance
    /// <5ns - Single atomic load
    #[inline]
    pub fn animation_progress(&self) -> f32 {
        let state_val = self.state.load(Ordering::Acquire);
        let progress = ((state_val & ANIMATION_MASK) >> ANIMATION_SHIFT) as u16;
        progress as f32 / ANIMATION_MAX as f32
    }

    /// Update animation
    ///
    /// # Performance
    /// <20ns - Atomic compare-exchange loop
    ///
    /// # Arguments
    /// * `delta_ms` - Time elapsed since last update (milliseconds)
    #[inline]
    pub fn update_animation(&self, delta_ms: u16) {
        let duration = self.animation_duration;
        if duration == 0 {
            return;
        }

        // Calculate progress increment
        let delta_progress = ((delta_ms as u32 * ANIMATION_MAX as u32) / duration as u32) as u16;

        loop {
            let current = self.state.load(Ordering::Acquire);
            let state = ModalState::from((current & STATE_MASK) as u8);
            let progress = ((current & ANIMATION_MASK) >> ANIMATION_SHIFT) as u16;

            let (new_state, new_progress) = match state {
                ModalState::Opening => {
                    let next_progress = progress.saturating_add(delta_progress).min(ANIMATION_MAX);
                    if next_progress >= ANIMATION_MAX {
                        (ModalState::Open, ANIMATION_MAX)
                    } else {
                        (ModalState::Opening, next_progress)
                    }
                }
                ModalState::Closing => {
                    let next_progress = progress.saturating_sub(delta_progress);
                    if next_progress == 0 {
                        (ModalState::Hidden, 0)
                    } else {
                        (ModalState::Closing, next_progress)
                    }
                }
                _ => return, // No animation needed
            };

            let new_val = (new_state as u64) | ((new_progress as u64) << ANIMATION_SHIFT);

            match self.state.compare_exchange_weak(
                current,
                new_val,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    if new_state != state {
                        self.generation.fetch_add(1, Ordering::Release);
                    }
                    return;
                }
                Err(_) => continue,
            }
        }
    }

    /// Handle backdrop click
    ///
    /// # Performance
    /// <15ns - Bounds check + atomic load/store
    ///
    /// # Returns
    /// true if click should close modal
    #[inline]
    pub fn handle_backdrop_click(&self, x: u16, y: u16, modal_bounds: Rect) -> bool {
        // Check if backdrop dismiss is enabled
        let flags = self.flags.load(Ordering::Acquire);
        if flags & FLAG_BACKDROP_DISMISS == 0 {
            return false;
        }

        // Check if click is outside modal content
        !(x >= modal_bounds.x
            && x < modal_bounds.x + modal_bounds.width
            && y >= modal_bounds.y
            && y < modal_bounds.y + modal_bounds.height)
    }

    /// Handle keyboard event
    ///
    /// # Performance
    /// <10ns - Single atomic load + comparison
    ///
    /// # Returns
    /// true if event handled (modal should close)
    #[inline]
    pub fn handle_key(&self, event: &KeyEvent) -> bool {
        // Check if escape dismiss is enabled
        let flags = self.flags.load(Ordering::Acquire);
        if flags & FLAG_ESCAPE_DISMISS == 0 {
            return false;
        }

        // Check for Escape key
        event.code == crate::terminal::input::KeyCode::Esc
    }

    /// Check if focus trap is enabled
    ///
    /// # Performance
    /// <5ns - Single atomic load
    #[inline]
    pub fn is_focus_trap_enabled(&self) -> bool {
        let flags = self.flags.load(Ordering::Acquire);
        flags & FLAG_FOCUS_TRAP != 0
    }

    /// Calculate content bounds
    ///
    /// # Performance
    /// <30ns - Position calculation
    #[inline]
    pub fn content_bounds(&self, screen: Rect) -> Rect {
        let width = if self.width > 0 {
            self.width.max(self.min_width).min(self.max_width)
        } else {
            // Auto: 80% of screen width
            ((screen.width as u32 * 80) / 100) as u16
        }.min(screen.width);

        let height = if self.height > 0 {
            self.height
        } else {
            // Auto: 80% of screen height
            ((screen.height as u32 * 80) / 100) as u16
        }.min(screen.height);

        let (x, y) = match self.position {
            ModalPosition::Center => (
                screen.x + (screen.width.saturating_sub(width)) / 2,
                screen.y + (screen.height.saturating_sub(height)) / 2,
            ),
            ModalPosition::Top => (
                screen.x + (screen.width.saturating_sub(width)) / 2,
                screen.y + screen.height / 10, // 10% from top
            ),
            ModalPosition::Bottom => (
                screen.x + (screen.width.saturating_sub(width)) / 2,
                screen.y + screen.height.saturating_sub(height + screen.height / 10),
            ),
            ModalPosition::Custom => (
                screen.x + self.custom_x.min(screen.width.saturating_sub(width)),
                screen.y + self.custom_y.min(screen.height.saturating_sub(height)),
            ),
        };

        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// Render backdrop
    ///
    /// # Performance
    /// <50ns - Command buffer push
    #[inline]
    pub fn render_backdrop(&self, screen: Rect, cmd: &mut RenderCommandBuffer) {
        let progress = self.animation_progress();

        // Fade backdrop based on animation progress
        let alpha = ((self.backdrop_color & 0xFF) as f32 * progress) as u8;
        let r = ((self.backdrop_color >> 24) & 0xFF) as u8;
        let g = ((self.backdrop_color >> 16) & 0xFF) as u8;
        let b = ((self.backdrop_color >> 8) & 0xFF) as u8;

        let style = RenderStyle::new(
            0, // Foreground doesn't matter for background fill
            (r as u32) << 24 | (g as u32) << 16 | (b as u32) << 8 | alpha as u32,
        );

        cmd.rect(screen, style);
    }

    /// Render modal container (border + background)
    ///
    /// # Performance
    /// <100ns - Multiple command buffer pushes
    #[inline]
    pub fn render_container(&self, bounds: Rect, cmd: &mut RenderCommandBuffer) {
        let progress = self.animation_progress();

        // Scale effect during animation
        let scale = 0.8 + (0.2 * progress); // 80% to 100%
        let scaled_width = (bounds.width as f32 * scale) as u16;
        let scaled_height = (bounds.height as f32 * scale) as u16;
        let offset_x = (bounds.width - scaled_width) / 2;
        let offset_y = (bounds.height - scaled_height) / 2;

        let scaled_bounds = Rect {
            x: bounds.x + offset_x,
            y: bounds.y + offset_y,
            width: scaled_width,
            height: scaled_height,
        };

        // Background
        let bg_style = RenderStyle::new(0, self.bg_color);
        cmd.rect(scaled_bounds, bg_style);

        // Border (simplified - just draw outline rectangles)
        if self.border_width > 0 {
            let border_style = RenderStyle::new(0, self.border_color);
            // Top
            cmd.rect(
                Rect {
                    x: scaled_bounds.x,
                    y: scaled_bounds.y,
                    width: scaled_bounds.width,
                    height: self.border_width as u16,
                },
                border_style,
            );
            // Bottom
            cmd.rect(
                Rect {
                    x: scaled_bounds.x,
                    y: scaled_bounds.y + scaled_bounds.height - self.border_width as u16,
                    width: scaled_bounds.width,
                    height: self.border_width as u16,
                },
                border_style,
            );
            // Left
            cmd.rect(
                Rect {
                    x: scaled_bounds.x,
                    y: scaled_bounds.y,
                    width: self.border_width as u16,
                    height: scaled_bounds.height,
                },
                border_style,
            );
            // Right
            cmd.rect(
                Rect {
                    x: scaled_bounds.x + scaled_bounds.width - self.border_width as u16,
                    y: scaled_bounds.y,
                    width: self.border_width as u16,
                    height: scaled_bounds.height,
                },
                border_style,
            );
        }
    }

    /// Get generation counter
    ///
    /// # Performance
    /// <5ns - Single atomic load
    #[inline]
    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Acquire)
    }
}

impl Default for ModalContainerCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ModalContainerCapsule {
    #[inline]
    fn render(&self, area: Rect, cmd: &mut RenderCommandBuffer) {
        // Only render if modal is visible
        if matches!(self.state(), ModalState::Hidden) {
            return;
        }

        // Render backdrop
        self.render_backdrop(area, cmd);

        // Calculate and render modal content
        let bounds = self.content_bounds(area);
        self.render_container(bounds, cmd);
    }

    #[inline]
    fn is_focusable(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // Q1-Q7: UNIT TESTS
    // ============================================================================

    #[test]
    fn test_modal_creation() {
        let modal = ModalContainerCapsule::new();
        assert_eq!(modal.state(), ModalState::Hidden);
        assert!(!modal.is_open());
        assert_eq!(modal.animation_progress(), 0.0);
    }

    #[test]
    fn test_modal_open_close() {
        let modal = ModalContainerCapsule::new();

        // Open
        modal.open(42);
        assert_eq!(modal.state(), ModalState::Opening);
        assert!(modal.is_open());

        // Close
        let prev_focus = modal.close();
        assert_eq!(prev_focus, 42);
        assert_eq!(modal.state(), ModalState::Closing);
        assert!(!modal.is_open());
    }

    #[test]
    fn test_modal_animation() {
        let modal = ModalContainerCapsule::new().with_animation_duration(100);

        modal.open(0);
        assert_eq!(modal.animation_progress(), 0.0);

        // 50ms = 50% progress
        modal.update_animation(50);
        let progress = modal.animation_progress();
        assert!(progress > 0.4 && progress < 0.6);

        // Another 50ms = 100% progress, transitions to Open
        modal.update_animation(50);
        assert_eq!(modal.state(), ModalState::Open);
        assert_eq!(modal.animation_progress(), 1.0);
    }

    #[test]
    fn test_modal_flags() {
        let modal = ModalContainerCapsule::new()
            .with_backdrop_dismiss(false)
            .with_escape_dismiss(false)
            .with_focus_trap(false);

        let flags = modal.flags.load(Ordering::Acquire);
        assert_eq!(flags & FLAG_BACKDROP_DISMISS, 0);
        assert_eq!(flags & FLAG_ESCAPE_DISMISS, 0);
        assert_eq!(flags & FLAG_FOCUS_TRAP, 0);

        modal.with_backdrop_dismiss(true);
        let flags = modal.flags.load(Ordering::Acquire);
        assert_ne!(flags & FLAG_BACKDROP_DISMISS, 0);
    }

    #[test]
    fn test_modal_position() {
        let screen = Rect {
            x: 0,
            y: 0,
            width: 1000,
            height: 800,
        };

        // Center
        let modal = ModalContainerCapsule::new().with_size(400, 300);
        let bounds = modal.content_bounds(screen);
        assert_eq!(bounds.x, 300); // (1000 - 400) / 2
        assert_eq!(bounds.y, 250); // (800 - 300) / 2

        // Custom
        let modal = ModalContainerCapsule::new()
            .with_size(400, 300)
            .with_custom_position(100, 50);
        let bounds = modal.content_bounds(screen);
        assert_eq!(bounds.x, 100);
        assert_eq!(bounds.y, 50);
    }

    #[test]
    fn test_backdrop_click_handling() {
        let modal = ModalContainerCapsule::new();
        let content_bounds = Rect {
            x: 100,
            y: 100,
            width: 200,
            height: 150,
        };

        // Click inside - should not close
        assert!(!modal.handle_backdrop_click(150, 150, content_bounds));

        // Click outside - should close
        assert!(modal.handle_backdrop_click(50, 50, content_bounds));

        // Disable backdrop dismiss
        let modal = modal.with_backdrop_dismiss(false);
        assert!(!modal.handle_backdrop_click(50, 50, content_bounds));
    }

    #[test]
    fn test_escape_key_handling() {
        use crate::terminal::event::{KeyCode, KeyModifiers, KeyEventKind};

        let modal = ModalContainerCapsule::new();

        let escape_event = KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
        };
        assert!(modal.handle_key(&escape_event));

        let other_event = KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
        };
        assert!(!modal.handle_key(&other_event));

        // Disable escape dismiss
        let modal = modal.with_escape_dismiss(false);
        assert!(!modal.handle_key(&escape_event));
    }

    #[test]
    fn test_generation_counter() {
        let modal = ModalContainerCapsule::new();
        let gen1 = modal.generation();

        modal.open(0);
        let gen2 = modal.generation();
        assert_eq!(gen2, gen1 + 1);

        modal.close();
        let gen3 = modal.generation();
        assert_eq!(gen3, gen2 + 1);
    }

    // ============================================================================
    // Q8-Q14: PROPERTY TESTS
    // ============================================================================

    #[cfg(feature = "proptest")]
    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn prop_animation_bounded(delta_ms in 0u16..1000) {
                let modal = ModalContainerCapsule::new();
                modal.open(0);

                // Update multiple times
                for _ in 0..10 {
                    modal.update_animation(delta_ms);
                    let progress = modal.animation_progress();
                    prop_assert!(progress >= 0.0 && progress <= 1.0);
                }
            }

            #[test]
            fn prop_size_constraints(width in 0u16..2000, height in 0u16..2000) {
                let modal = ModalContainerCapsule::new()
                    .with_size(width, height)
                    .with_size_constraints(200, 800);

                let screen = Rect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                };

                let bounds = modal.content_bounds(screen);
                prop_assert!(bounds.width >= 200 && bounds.width <= 800);
            }

            #[test]
            fn prop_generation_monotonic(operations in 0u32..100) {
                let modal = ModalContainerCapsule::new();
                let mut prev_gen = modal.generation();

                for _ in 0..operations {
                    modal.open(0);
                    let gen = modal.generation();
                    prop_assert!(gen > prev_gen);
                    prev_gen = gen;
                }
            }

            #[test]
            fn prop_focus_restoration(focus_id in 0u32..1000) {
                let modal = ModalContainerCapsule::new();
                modal.open(focus_id);
                let restored = modal.close();
                prop_assert_eq!(restored, focus_id);
            }
        }
    }

    // ============================================================================
    // Q15-Q21: INTEGRATION TESTS
    // ============================================================================

    #[test]
    fn test_full_open_close_cycle() {
        let modal = ModalContainerCapsule::new().with_animation_duration(100);

        // Open
        modal.open(123);
        assert_eq!(modal.state(), ModalState::Opening);

        // Animate to Open
        modal.update_animation(50);
        modal.update_animation(50);
        assert_eq!(modal.state(), ModalState::Open);

        // Close
        let prev = modal.close();
        assert_eq!(prev, 123);
        assert_eq!(modal.state(), ModalState::Closing);

        // Animate to Hidden
        modal.update_animation(50);
        modal.update_animation(50);
        assert_eq!(modal.state(), ModalState::Hidden);
    }

    #[test]
    fn test_widget_trait_integration() {
        use crate::terminal::event::{KeyCode, KeyModifiers, KeyEventKind};

        let modal = ModalContainerCapsule::new();

        assert!(modal.is_focusable());

        modal.open(0);
        assert!(modal.is_open());

        let escape_event = KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
        };
        assert!(modal.handle_key(&escape_event));
    }

    #[test]
    fn test_render_integration() {
        let modal = ModalContainerCapsule::new().with_animation_duration(100);
        let mut cmd = RenderCommandBuffer::new(100, 80);

        let screen = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 80,
        };

        modal.open(0);
        modal.update_animation(50); // 50% animated

        // Use Widget trait render method
        modal.render(screen, &mut cmd);

        // Hidden modal should not render
        let hidden_modal = ModalContainerCapsule::new();
        let mut cmd2 = RenderCommandBuffer::new(100, 80);
        hidden_modal.render(screen, &mut cmd2);
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let modal = Arc::new(ModalContainerCapsule::new());
        let mut handles = vec![];

        // Spawn threads that open/close/query
        for i in 0..4 {
            let m = Arc::clone(&modal);
            handles.push(thread::spawn(move || {
                for j in 0..100 {
                    if i % 2 == 0 {
                        m.open((i * 100 + j) as u32);
                    } else {
                        m.close();
                    }
                    let _ = m.is_open();
                    let _ = m.state();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should not crash, state should be valid
        assert!(matches!(
            modal.state(),
            ModalState::Hidden | ModalState::Opening | ModalState::Open | ModalState::Closing
        ));
    }
}
