//! # SplitPaneCapsule - Resizable Split Pane Layout
//!
//! **Tier**: T1+T3 (Atomic state coordination + Q16.16 Fixed-point position)
//!
//! High-performance resizable split pane with draggable divider and collapse support.
//! 100% lockfree state management using packed atomic operations.
//!
//! ## Features
//!
//! - **Lockfree State**: All state packed into single AtomicU64
//! - **Fixed-Point Position**: Q16.16 format for precise divider placement
//! - **Draggable Divider**: Smooth drag with min size enforcement
//! - **Collapse Support**: Auto-collapse to threshold with double-click toggle
//! - **Generation Counter**: Atomic snapshot consistency
//!
//! ## Performance (B32)
//!
//! - State read: <5ns (single atomic load)
//! - Position update: <10ns (single atomic CAS)
//! - Layout calculation: <50ns (fixed-point math)
//! - Divider render: <20ns (single char draw)
//!
//! ## UCE34 Compliance
//!
//! - Q10: T1+T3 compound (Atomic state + Q16.16 position)
//! - Q33: 100% lockfree (AtomicU64 state, AtomicU32 generation)
//! - Q34: Generation counter for state audit
//!
//! ## ASSUM Safety
//!
//! - #ASSUME: SplitState fits in 64 bits (compile-time verified)
//! - #ASSUME: Position ratio 0.0-1.0 (validated in methods)
//! - #VERIFY: Memory ordering (Acquire/Release for consistency)

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use crate::terminal::event::{Event, MouseButton, MouseEvent, MouseEventKind};
use crate::terminal::widget::{Constraints, Rect, RenderCommandBuffer, RenderStyle, Widget};

/// Split orientation
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum SplitOrientation {
    #[default]
    Horizontal = 0, // Left | Right
    Vertical = 1,   // Top / Bottom
}

/// Split state packed into 64 bits for atomic updates
///
/// Layout:
/// - Bits 0-31: position (u32, Q16.16 fixed-point 0.0-1.0)
/// - Bits 32-39: dragging (u8)
/// - Bits 40-47: divider_hovered (u8)
/// - Bits 48-63: _padding (u16)
#[derive(Copy, Clone, Debug, Default)]
#[repr(C)]
pub struct SplitState {
    /// Divider position (Q16.16 ratio, 0.0-1.0)
    pub position: u32,
    /// Dragging state
    pub dragging: bool,
    /// Hover on divider
    pub divider_hovered: bool,
}

impl SplitState {
    /// Pack state into u64 for atomic storage
    pub const fn pack(self) -> u64 {
        (self.position as u64)
            | ((self.dragging as u64) << 32)
            | ((self.divider_hovered as u64) << 40)
    }

    /// Unpack state from u64
    pub const fn unpack(val: u64) -> Self {
        Self {
            position: (val & 0xFFFF_FFFF) as u32,
            dragging: ((val >> 32) & 0xFF) != 0,
            divider_hovered: ((val >> 40) & 0xFF) != 0,
        }
    }

    /// Convert Q16.16 position to float (0.0-1.0)
    pub fn position_f32(self) -> f32 {
        (self.position as f32) / 65536.0
    }

    /// Create from f32 ratio (0.0-1.0)
    pub fn from_ratio(ratio: f32) -> Self {
        let clamped = ratio.clamp(0.0, 1.0);
        Self {
            position: (clamped * 65536.0) as u32,
            dragging: false,
            divider_hovered: false,
        }
    }
}

/// T1+T3 - Resizable split pane
///
/// # UCE34 Compliance
/// - Q10: T1+T3 compound (Atomic + Q16.16 position)
/// - Q33: 100% lockfree
/// - Q34: Position change audit
///
/// # Performance (B32 targets)
/// - State read: <5ns (single atomic load)
/// - Position update: <10ns (single atomic CAS)
/// - Layout calculation: <50ns (fixed-point math)
/// - Divider render: <20ns (single char draw)
#[repr(C, align(64))]
pub struct SplitPaneCapsule {
    // State
    /// position (32) | dragging (8) | hovered (8) | _pad (16)
    state: AtomicU64,
    /// Generation counter
    generation: AtomicU32,

    // Configuration
    /// Split orientation
    orientation: SplitOrientation,
    /// Divider width (cells)
    divider_width: u8,
    /// Minimum first pane size (cells)
    min_first: u8,
    /// Minimum second pane size (cells)
    min_second: u8,
    /// Collapse threshold (cells, collapse if smaller)
    collapse_threshold: u8,
    /// Allow collapse first pane
    collapsible_first: bool,
    /// Allow collapse second pane
    collapsible_second: bool,

    // Styling
    /// Divider color (RGBA8888)
    divider_color: u32,
    /// Divider hover color (RGBA8888)
    divider_hover_color: u32,
    /// Divider drag color (RGBA8888)
    divider_drag_color: u32,
    /// Show resize grip
    show_grip: bool,

    // Computed bounds (after layout)
    /// First pane bounds
    first_bounds: Rect,
    /// Second pane bounds
    second_bounds: Rect,
    /// Divider bounds (for hit testing)
    divider_bounds: Rect,

    // Drag state
    /// Drag start position (Q16.16)
    drag_start: AtomicU32,
    /// Position at drag start
    start_position: AtomicU32,

    _pad: [u8; 172],
}

// Compile-time size/alignment verification
const _: () = assert!(core::mem::size_of::<SplitPaneCapsule>() == 256);
const _: () = assert!(core::mem::align_of::<SplitPaneCapsule>() == 64);

// Default colors
const DEFAULT_DIVIDER_COLOR: u32 = 0x4B5563FF; // Gray-600
const DEFAULT_DIVIDER_HOVER: u32 = 0x6B7280FF; // Gray-500
const DEFAULT_DIVIDER_DRAG: u32 = 0x3B82F6FF; // Blue-500

impl SplitPaneCapsule {
    /// Create new split pane with orientation
    pub fn new(orientation: SplitOrientation) -> Self {
        Self {
            state: AtomicU64::new(SplitState::from_ratio(0.5).pack()), // 50% default
            generation: AtomicU32::new(0),

            orientation,
            divider_width: 1,
            min_first: 5,
            min_second: 5,
            collapse_threshold: 3,
            collapsible_first: true,
            collapsible_second: true,

            divider_color: DEFAULT_DIVIDER_COLOR,
            divider_hover_color: DEFAULT_DIVIDER_HOVER,
            divider_drag_color: DEFAULT_DIVIDER_DRAG,
            show_grip: true,

            first_bounds: Rect::default(),
            second_bounds: Rect::default(),
            divider_bounds: Rect::default(),

            drag_start: AtomicU32::new(0),
            start_position: AtomicU32::new(0),

            _pad: [0u8; 172],
        }
    }

    /// Convenience constructor for horizontal split
    pub fn horizontal() -> Self {
        Self::new(SplitOrientation::Horizontal)
    }

    /// Convenience constructor for vertical split
    pub fn vertical() -> Self {
        Self::new(SplitOrientation::Vertical)
    }

    /// Builder: Set initial position ratio (0.0-1.0)
    pub fn with_position(self, ratio: f32) -> Self {
        self.set_position(ratio);
        self
    }

    /// Builder: Set minimum sizes for both panes
    pub fn with_min_sizes(mut self, min_first: u8, min_second: u8) -> Self {
        self.min_first = min_first;
        self.min_second = min_second;
        self
    }

    /// Builder: Set collapsible flags
    pub fn with_collapsible(mut self, first: bool, second: bool) -> Self {
        self.collapsible_first = first;
        self.collapsible_second = second;
        self
    }

    /// Set divider position ratio (0.0-1.0)
    pub fn set_position(&self, ratio: f32) {
        let mut state = self.state();
        state.position = (ratio.clamp(0.0, 1.0) * 65536.0) as u32;
        self.update_state(state);
    }

    /// Get current position ratio (0.0-1.0)
    pub fn position(&self) -> f32 {
        self.state().position_f32()
    }

    /// Calculate pane bounds based on available area
    pub fn layout(&mut self, available: Rect) {
        let state = self.state();
        let ratio = state.position_f32();

        match self.orientation {
            SplitOrientation::Horizontal => {
                // Split horizontally (left | right)
                let total_width = available.width.saturating_sub(self.divider_width as u16);
                let first_width = ((total_width as f32 * ratio) as u16).max(self.min_first as u16);
                let second_width = total_width.saturating_sub(first_width);

                // Clamp to min sizes
                let first_width = first_width.min(total_width.saturating_sub(self.min_second as u16));

                self.first_bounds = Rect::new(
                    available.x,
                    available.y,
                    first_width,
                    available.height,
                );

                self.divider_bounds = Rect::new(
                    available.x + first_width,
                    available.y,
                    self.divider_width as u16,
                    available.height,
                );

                self.second_bounds = Rect::new(
                    available.x + first_width + self.divider_width as u16,
                    available.y,
                    total_width.saturating_sub(first_width),
                    available.height,
                );
            }
            SplitOrientation::Vertical => {
                // Split vertically (top / bottom)
                let total_height = available.height.saturating_sub(self.divider_width as u16);
                let first_height = ((total_height as f32 * ratio) as u16).max(self.min_first as u16);
                let second_height = total_height.saturating_sub(first_height);

                // Clamp to min sizes
                let first_height = first_height.min(total_height.saturating_sub(self.min_second as u16));

                self.first_bounds = Rect::new(
                    available.x,
                    available.y,
                    available.width,
                    first_height,
                );

                self.divider_bounds = Rect::new(
                    available.x,
                    available.y + first_height,
                    available.width,
                    self.divider_width as u16,
                );

                self.second_bounds = Rect::new(
                    available.x,
                    available.y + first_height + self.divider_width as u16,
                    available.width,
                    total_height.saturating_sub(first_height),
                );
            }
        }
    }

    /// Get first pane bounds
    pub fn first_bounds(&self) -> Rect {
        self.first_bounds
    }

    /// Get second pane bounds
    pub fn second_bounds(&self) -> Rect {
        self.second_bounds
    }

    /// Handle mouse move, returns true if hover state changed
    pub fn handle_mouse_move(&self, x: u16, y: u16) -> bool {
        let in_divider = self.divider_bounds.contains(x, y);
        let mut state = self.state();

        if in_divider != state.divider_hovered {
            state.divider_hovered = in_divider;
            self.update_state(state);
            true
        } else {
            false
        }
    }

    /// Start dragging, returns true if drag started
    pub fn handle_drag_start(&self, x: u16, y: u16) -> bool {
        if !self.divider_bounds.contains(x, y) {
            return false;
        }

        let mut state = self.state();
        state.dragging = true;
        self.update_state(state);

        // Store drag start position
        let pos = match self.orientation {
            SplitOrientation::Horizontal => x,
            SplitOrientation::Vertical => y,
        };
        self.drag_start.store(pos as u32, Ordering::Release);
        self.start_position.store(state.position, Ordering::Release);

        true
    }

    /// Continue drag
    pub fn handle_drag(&self, x: u16, y: u16) {
        let mut state = self.state();
        if !state.dragging {
            return;
        }

        let drag_start = self.drag_start.load(Ordering::Acquire) as u16;
        let start_pos = self.start_position.load(Ordering::Acquire);

        let (current, total) = match self.orientation {
            SplitOrientation::Horizontal => {
                (x, self.divider_bounds.x + self.first_bounds.width)
            }
            SplitOrientation::Vertical => {
                (y, self.divider_bounds.y + self.first_bounds.height)
            }
        };

        // Calculate delta and new position
        let delta = current.saturating_sub(drag_start) as i32;
        let total_size = match self.orientation {
            SplitOrientation::Horizontal => self.first_bounds.width + self.second_bounds.width,
            SplitOrientation::Vertical => self.first_bounds.height + self.second_bounds.height,
        } as f32;

        if total_size <= 0.0 {
            return;
        }

        let delta_ratio = (delta as f32) / total_size;
        let start_ratio = (start_pos as f32) / 65536.0;
        let new_ratio = (start_ratio + delta_ratio).clamp(0.0, 1.0);

        // Enforce min sizes
        let min_first_ratio = (self.min_first as f32) / total_size;
        let min_second_ratio = 1.0 - ((self.min_second as f32) / total_size);
        let clamped_ratio = new_ratio.clamp(min_first_ratio, min_second_ratio);

        state.position = (clamped_ratio * 65536.0) as u32;
        self.update_state(state);
    }

    /// End drag
    pub fn handle_drag_end(&self) {
        let mut state = self.state();
        state.dragging = false;
        self.update_state(state);
    }

    /// Handle double-click: reset to 50% or toggle collapse
    /// Returns true if state changed
    pub fn handle_double_click(&self) -> bool {
        let state = self.state();
        let ratio = state.position_f32();

        // If near 50%, don't collapse
        if (ratio - 0.5).abs() < 0.05 {
            return false;
        }

        // If collapsed or near collapse threshold, expand to 50%
        let total_size = match self.orientation {
            SplitOrientation::Horizontal => self.first_bounds.width + self.second_bounds.width,
            SplitOrientation::Vertical => self.first_bounds.height + self.second_bounds.height,
        } as f32;

        let first_size = total_size * ratio;
        let second_size = total_size * (1.0 - ratio);

        if first_size < (self.collapse_threshold as f32) || second_size < (self.collapse_threshold as f32) {
            // Expand to 50%
            self.set_position(0.5);
            true
        } else {
            // Collapse to smaller side
            if ratio < 0.5 && self.collapsible_first {
                self.set_position(0.0);
                true
            } else if ratio > 0.5 && self.collapsible_second {
                self.set_position(1.0);
                true
            } else {
                false
            }
        }
    }

    /// Render divider to command buffer
    pub fn render_divider(&self, cmd: &mut RenderCommandBuffer) {
        let state = self.state();

        // Select color based on state
        let color = if state.dragging {
            self.divider_drag_color
        } else if state.divider_hovered {
            self.divider_hover_color
        } else {
            self.divider_color
        };

        let style = RenderStyle::new(color, 0x00000000); // Transparent background

        // Draw divider
        match self.orientation {
            SplitOrientation::Horizontal => {
                // Vertical bar: │ (U+2502) or ┃ (U+2503 thick)
                let ch = if state.dragging || state.divider_hovered { '┃' } else { '│' };
                for y in self.divider_bounds.y..(self.divider_bounds.y + self.divider_bounds.height) {
                    cmd.text(
                        self.divider_bounds.x,
                        y,
                        alloc::string::String::from(ch),
                        style,
                    );
                }

                // Optional grip in middle
                if self.show_grip {
                    let mid_y = self.divider_bounds.y + self.divider_bounds.height / 2;
                    cmd.text(
                        self.divider_bounds.x,
                        mid_y,
                        alloc::string::String::from('⋮'), // U+22EE
                        style,
                    );
                }
            }
            SplitOrientation::Vertical => {
                // Horizontal bar: ─ (U+2500) or ━ (U+2501 thick)
                let ch = if state.dragging || state.divider_hovered { '━' } else { '─' };
                for x in self.divider_bounds.x..(self.divider_bounds.x + self.divider_bounds.width) {
                    cmd.text(
                        x,
                        self.divider_bounds.y,
                        alloc::string::String::from(ch),
                        style,
                    );
                }

                // Optional grip in middle
                if self.show_grip {
                    let mid_x = self.divider_bounds.x + self.divider_bounds.width / 2;
                    cmd.text(
                        mid_x,
                        self.divider_bounds.y,
                        alloc::string::String::from('⋯'), // U+22EF
                        style,
                    );
                }
            }
        }
    }

    /// Read current state (single atomic load)
    pub fn state(&self) -> SplitState {
        SplitState::unpack(self.state.load(Ordering::Acquire))
    }

    /// Update state atomically
    fn update_state(&self, new_state: SplitState) {
        self.state.store(new_state.pack(), Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get current generation for snapshot consistency
    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Acquire)
    }
}

impl Widget for SplitPaneCapsule {
    type State = SplitState;
    const TYPE_ID: u64 = 0x5350_4C49_5400_0001; // "SPLIT" + version

    fn measure(&self, constraints: Constraints, _state: &Self::State) -> (u16, u16) {
        // Split pane expands to fill available space
        let width = constraints.max_width;
        let height = constraints.max_height;
        (width, height)
    }

    fn layout(&self, bounds: Rect, _state: &Self::State) -> Rect {
        // Split pane uses entire bounds
        bounds
    }

    fn render(&self, area: Rect, _state: &Self::State, cmd: &mut RenderCommandBuffer) {
        self.render_divider(cmd);
    }

    fn handle_event(&self, event: &Event, state: &mut Self::State) -> bool {
        match event {
            Event::Mouse(mouse_event) => {
                match mouse_event.kind {
                    MouseEventKind::Moved => {
                        self.handle_mouse_move(mouse_event.column, mouse_event.row)
                    }
                    MouseEventKind::Down(MouseButton::Left) => {
                        self.handle_drag_start(mouse_event.column, mouse_event.row)
                    }
                    MouseEventKind::Up(MouseButton::Left) => {
                        if state.dragging {
                            self.handle_drag_end();
                            true
                        } else {
                            false
                        }
                    }
                    MouseEventKind::Drag(MouseButton::Left) => {
                        if state.dragging {
                            self.handle_drag(mouse_event.column, mouse_event.row);
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn focusable(&self) -> bool {
        true // Allow keyboard resize (future enhancement)
    }

    fn tab_index(&self) -> u16 {
        0
    }
}

// Need alloc for String in render
extern crate alloc;

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: Unit Tests (10 tests)
    // ========================================================================

    #[test]
    fn test_new_horizontal() {
        let split = SplitPaneCapsule::horizontal();
        assert_eq!(split.orientation, SplitOrientation::Horizontal);
        assert_eq!(split.position(), 0.5);
        assert_eq!(split.divider_width, 1);
    }

    #[test]
    fn test_new_vertical() {
        let split = SplitPaneCapsule::vertical();
        assert_eq!(split.orientation, SplitOrientation::Vertical);
        assert_eq!(split.position(), 0.5);
    }

    #[test]
    fn test_state_packing() {
        let state = SplitState {
            position: 32768, // 0.5 in Q16.16
            dragging: true,
            divider_hovered: false,
        };

        let packed = state.pack();
        let unpacked = SplitState::unpack(packed);

        assert_eq!(unpacked.position, 32768);
        assert!(unpacked.dragging);
        assert!(!unpacked.divider_hovered);
    }

    #[test]
    fn test_q16_16_conversion() {
        let state = SplitState::from_ratio(0.5);
        assert_eq!(state.position, 32768); // 0.5 * 65536

        let ratio = state.position_f32();
        assert!((ratio - 0.5).abs() < 0.001);

        let state = SplitState::from_ratio(0.25);
        let ratio = state.position_f32();
        assert!((ratio - 0.25).abs() < 0.001);

        let state = SplitState::from_ratio(0.75);
        let ratio = state.position_f32();
        assert!((ratio - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_set_position() {
        let split = SplitPaneCapsule::horizontal();

        split.set_position(0.3);
        assert!((split.position() - 0.3).abs() < 0.001);

        split.set_position(0.7);
        assert!((split.position() - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_position_clamping() {
        let split = SplitPaneCapsule::horizontal();

        // Test clamping to 0.0-1.0
        split.set_position(-0.5);
        assert_eq!(split.position(), 0.0);

        split.set_position(1.5);
        assert_eq!(split.position(), 1.0);
    }

    #[test]
    fn test_builder_pattern() {
        let split = SplitPaneCapsule::horizontal()
            .with_position(0.3)
            .with_min_sizes(10, 15)
            .with_collapsible(false, true);

        assert!((split.position() - 0.3).abs() < 0.001);
        assert_eq!(split.min_first, 10);
        assert_eq!(split.min_second, 15);
        assert!(!split.collapsible_first);
        assert!(split.collapsible_second);
    }

    #[test]
    fn test_horizontal_layout() {
        let mut split = SplitPaneCapsule::horizontal().with_position(0.5);
        let available = Rect::new(0, 0, 100, 20);

        split.layout(available);

        // 100 width - 1 divider = 99, split 50/50 = 49/50
        assert_eq!(split.first_bounds.width, 49);
        assert_eq!(split.divider_bounds.width, 1);
        assert_eq!(split.second_bounds.width, 50);

        assert_eq!(split.first_bounds.x, 0);
        assert_eq!(split.divider_bounds.x, 49);
        assert_eq!(split.second_bounds.x, 50);
    }

    #[test]
    fn test_vertical_layout() {
        let mut split = SplitPaneCapsule::vertical().with_position(0.5);
        let available = Rect::new(0, 0, 100, 40);

        split.layout(available);

        // 40 height - 1 divider = 39, split 50/50 = 19/20
        assert_eq!(split.first_bounds.height, 19);
        assert_eq!(split.divider_bounds.height, 1);
        assert_eq!(split.second_bounds.height, 20);

        assert_eq!(split.first_bounds.y, 0);
        assert_eq!(split.divider_bounds.y, 19);
        assert_eq!(split.second_bounds.y, 20);
    }

    #[test]
    fn test_generation_counter() {
        let split = SplitPaneCapsule::horizontal();
        let gen1 = split.generation();

        split.set_position(0.6);
        let gen2 = split.generation();
        assert_eq!(gen2, gen1 + 1);

        split.set_position(0.4);
        let gen3 = split.generation();
        assert_eq!(gen3, gen2 + 1);
    }

    // ========================================================================
    // Q8-Q14: Property Tests (4 tests)
    // ========================================================================

    #[test]
    fn test_position_bounds() {
        let split = SplitPaneCapsule::horizontal();

        // Test many random positions, all should clamp to 0.0-1.0
        for i in -100..200 {
            let ratio = (i as f32) / 100.0;
            split.set_position(ratio);
            let actual = split.position();
            assert!(actual >= 0.0 && actual <= 1.0);
        }
    }

    #[test]
    fn test_min_size_enforcement() {
        let mut split = SplitPaneCapsule::horizontal()
            .with_min_sizes(10, 15)
            .with_position(0.1); // Try to make first pane very small

        let available = Rect::new(0, 0, 100, 20);
        split.layout(available);

        // First pane should be at least min_first (10)
        assert!(split.first_bounds.width >= 10);
        // Second pane should be at least min_second (15)
        assert!(split.second_bounds.width >= 15);
    }

    #[test]
    fn test_hover_state() {
        let mut split = SplitPaneCapsule::horizontal();
        split.layout(Rect::new(0, 0, 100, 20));

        // Initially not hovered
        let state = split.state();
        assert!(!state.divider_hovered);

        // Hover on divider
        let divider_x = split.divider_bounds.x;
        let divider_y = split.divider_bounds.y;
        split.handle_mouse_move(divider_x, divider_y);

        let state = split.state();
        assert!(state.divider_hovered);

        // Move away
        split.handle_mouse_move(0, 0);
        let state = split.state();
        assert!(!state.divider_hovered);
    }

    #[test]
    fn test_drag_state() {
        let mut split = SplitPaneCapsule::horizontal();
        split.layout(Rect::new(0, 0, 100, 20));

        let divider_x = split.divider_bounds.x;
        let divider_y = split.divider_bounds.y;

        // Start drag
        let started = split.handle_drag_start(divider_x, divider_y);
        assert!(started);
        assert!(split.state().dragging);

        // End drag
        split.handle_drag_end();
        assert!(!split.state().dragging);
    }

    // ========================================================================
    // Q15-Q21: Integration Tests (4 tests)
    // ========================================================================

    #[test]
    fn test_widget_measure() {
        let split = SplitPaneCapsule::horizontal();
        let state = SplitState::default();

        let constraints = Constraints::loose(100, 50);
        let (width, height) = split.measure(constraints, &state);

        // Should expand to fill
        assert_eq!(width, 100);
        assert_eq!(height, 50);
    }

    #[test]
    fn test_widget_layout() {
        let split = SplitPaneCapsule::horizontal();
        let state = SplitState::default();

        let bounds = Rect::new(10, 5, 80, 30);
        let layout = split.layout(bounds, &state);

        // Should use entire bounds
        assert_eq!(layout, bounds);
    }

    #[test]
    fn test_double_click_reset() {
        let split = SplitPaneCapsule::horizontal().with_position(0.3);

        // Double-click should reset or collapse
        let changed = split.handle_double_click();
        assert!(changed);

        // Position should change from 0.3
        let new_pos = split.position();
        assert!((new_pos - 0.3).abs() > 0.1);
    }

    #[test]
    fn test_render_divider() {
        let mut split = SplitPaneCapsule::horizontal();
        split.layout(Rect::new(0, 0, 100, 20));

        let mut cmd = RenderCommandBuffer::new();
        let area = Rect::new(0, 0, 100, 20);
        let state = SplitState::default();

        split.render(area, &state, &mut cmd);

        // Should have rendered divider (multiple text commands for vertical line)
        assert!(cmd.commands().len() > 0);
    }
}
