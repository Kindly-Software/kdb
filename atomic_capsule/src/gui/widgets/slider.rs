// Copyright (C) 2025 Kindly Platform, Inc.
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! SliderCapsule - Lockfree slider/range input widget
//!
//! # Tier Classification
//!
//! T1 (Atomic) + T3 (Fixed-Point): Lockfree state coordination with Q8.8 fixed-point values
//!
//! # Design Principles
//!
//! - **Lockfree**: 100% atomic state coordination, no mutex
//! - **Deterministic**: Q8.8 fixed-point for exact value reproduction
//! - **Cache-Aligned**: 64B alignment prevents false sharing
//! - **Sub-10ns Access**: All read operations <10ns
//! - **Sub-15ns Updates**: All write operations <15ns
//!
//! # State Packing (AtomicU64)
//!
//! ```text
//! | current_value (Q8.8) | min_value (Q8.8) | max_value (Q8.8) | drag_state | reserved |
//! |       0-15           |      16-31       |      32-47       |   48-55    |  56-63   |
//! ```
//!
//! # Performance Targets (B32)
//!
//! - `value()`: <5ns (single atomic load + shift)
//! - `set_value()`: <15ns (CAS loop with clamping)
//! - `normalized()`: <10ns (value load + arithmetic)
//! - `on_drag()`: <15ns (coordinate → value conversion + CAS)
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 (T1+T3 tier), Q33 (zero runtime overhead)
//! - **Chaos**: 100% lockfree, no mutex, cache-aligned
//! - **ASSUM**: 100% safe (no unsafe code)
//! - **T28**: Unit tests, property tests, integration tests

use super::super::types::{Coord, Rect};
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Slider drag state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DragState {
    /// Slider is idle (not being interacted with)
    Idle = 0,
    /// Slider thumb is being dragged
    Dragging = 1,
    /// Slider is animating to target value
    Animating = 2,
}

impl DragState {
    /// Convert u8 to DragState (saturating to Idle if invalid)
    #[inline]
    const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Idle,
            1 => Self::Dragging,
            2 => Self::Animating,
            _ => Self::Idle, // Invalid values default to Idle
        }
    }
}

/// Slider orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Orientation {
    /// Horizontal slider (left to right)
    Horizontal = 0,
    /// Vertical slider (bottom to top)
    Vertical = 1,
}

impl Orientation {
    /// Convert u8 to Orientation
    #[inline]
    const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Horizontal,
            1 => Self::Vertical,
            _ => Self::Horizontal, // Default to horizontal
        }
    }
}

/// Lockfree slider widget with Q8.8 fixed-point values
///
/// # Memory Layout (64 bytes, cache-aligned)
///
/// ```text
/// Offset | Field           | Size | Description
/// -------|----------------|------|-------------
/// 0      | state          | 8    | Packed AtomicU64 (value, min, max, drag_state)
/// 8      | generation     | 4    | Generation counter (atomic updates)
/// 12     | id             | 4    | Widget ID
/// 16     | bounds         | 16   | Track rectangle (x, y, width, height as Coord/i32)
/// 32     | thumb_width    | 2    | Thumb handle width in pixels
/// 34     | orientation    | 1    | 0=horizontal, 1=vertical
/// 35     | _pad           | 29   | Padding to 64 bytes
/// ```
///
/// # State Packing (64 bits)
///
/// ```text
/// Bits   | Field          | Type  | Range
/// -------|----------------|-------|-------
/// 0-15   | current_value  | Q8.8  | 0.0-255.99
/// 16-31  | min_value      | Q8.8  | 0.0-255.99
/// 32-47  | max_value      | Q8.8  | 0.0-255.99
/// 48-55  | drag_state     | u8    | 0=Idle, 1=Dragging, 2=Animating
/// 56-63  | reserved       | u8    | Future use
/// ```
///
/// # Q8.8 Fixed-Point Format
///
/// - 8 bits integer, 8 bits fractional
/// - Range: 0.0 to 255.99609375
/// - Precision: 1/256 ≈ 0.00390625
/// - Example: 42.5 → 0x2A80 (42 << 8 | 128)
///
/// # Examples
///
/// ```
/// use atomic_capsule::gui::widgets::SliderCapsule;
/// use atomic_capsule::gui::Rect;
///
/// let bounds = Rect::new(10, 10, 200, 20).unwrap();
/// let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);
///
/// assert_eq!(slider.value(), 50.0);
/// assert_eq!(slider.normalized(), 0.5);
///
/// slider.set_value(75.0);
/// assert_eq!(slider.value(), 75.0);
/// assert_eq!(slider.normalized(), 0.75);
/// ```
#[repr(C, align(64))]
pub struct SliderCapsule {
    /// Packed state: [current_value:16 | min_value:16 | max_value:16 | drag_state:8 | reserved:8]
    state: AtomicU64,
    /// Generation counter for update tracking
    generation: AtomicU32,
    /// Widget ID (for event routing)
    id: u32,
    /// Track bounds (slider rail)
    bounds: Rect,
    /// Thumb handle width in pixels
    thumb_width: u16,
    /// Slider orientation (0=horizontal, 1=vertical)
    orientation: u8,
    /// Padding to 64 bytes
    _pad: [u8; 29],
}

impl SliderCapsule {
    /// Q8.8 fractional bits (8)
    const FRAC_BITS: u32 = 8;

    /// Q8.8 scale factor (2^8 = 256)
    const SCALE: u32 = 1 << Self::FRAC_BITS;

    /// Maximum Q8.8 value (255.99609375)
    const MAX_Q8_8: u16 = 0xFFFF;

    /// Bit masks for state packing
    const CURRENT_MASK: u64 = 0xFFFF;
    const MIN_MASK: u64 = 0xFFFF << 16;
    const MAX_MASK: u64 = 0xFFFF << 32;
    const DRAG_STATE_MASK: u64 = 0xFF << 48;

    /// Bit shifts for state packing
    const MIN_SHIFT: u32 = 16;
    const MAX_SHIFT: u32 = 32;
    const DRAG_STATE_SHIFT: u32 = 48;

    /// Create new slider with Q8.8 fixed-point values
    ///
    /// # Arguments
    ///
    /// - `id`: Widget ID for event routing
    /// - `min`: Minimum value (clamped to 0.0-255.99)
    /// - `max`: Maximum value (clamped to 0.0-255.99)
    /// - `initial`: Initial value (clamped to [min, max])
    /// - `bounds`: Track rectangle for rendering
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::widgets::SliderCapsule;
    /// use atomic_capsule::gui::Rect;
    ///
    /// let bounds = Rect::new(0, 0, 200, 20).unwrap();
    /// let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);
    /// assert_eq!(slider.value(), 50.0);
    /// ```
    #[inline]
    pub fn new(id: u32, min: f32, max: f32, initial: f32, bounds: Rect) -> Self {
        // Convert f32 to Q8.8 (clamped to 0.0-255.99)
        let min_q8 = Self::f32_to_q8_8(min);
        let max_q8 = Self::f32_to_q8_8(max);
        let initial_q8 = Self::f32_to_q8_8(initial);

        // Clamp initial to [min, max]
        let clamped = initial_q8.clamp(min_q8, max_q8);

        // Pack state: current | min | max | drag_state=Idle | reserved=0
        let packed = (clamped as u64)
            | ((min_q8 as u64) << Self::MIN_SHIFT)
            | ((max_q8 as u64) << Self::MAX_SHIFT)
            | ((DragState::Idle as u64) << Self::DRAG_STATE_SHIFT);

        Self {
            state: AtomicU64::new(packed),
            generation: AtomicU32::new(0),
            id,
            bounds,
            thumb_width: 12, // Default 12px thumb width
            orientation: Orientation::Horizontal as u8,
            _pad: [0; 29],
        }
    }

    /// Create new slider with custom thumb width and orientation
    #[inline]
    pub fn with_config(
        id: u32,
        min: f32,
        max: f32,
        initial: f32,
        bounds: Rect,
        thumb_width: u16,
        orientation: Orientation,
    ) -> Self {
        let mut slider = Self::new(id, min, max, initial, bounds);
        slider.thumb_width = thumb_width;
        slider.orientation = orientation as u8;
        slider
    }

    /// Convert f32 to Q8.8 fixed-point (saturating)
    #[inline]
    const fn f32_to_q8_8(value: f32) -> u16 {
        // Clamp to [0.0, 255.99609375]
        let clamped = if value < 0.0 {
            0.0
        } else if value > 255.99609375 {
            255.99609375
        } else {
            value
        };

        // Convert to Q8.8
        (clamped * Self::SCALE as f32) as u16
    }

    /// Convert Q8.8 fixed-point to f32
    #[inline]
    const fn q8_8_to_f32(value: u16) -> f32 {
        value as f32 / Self::SCALE as f32
    }

    /// Get current value (Q8.8 → f32)
    ///
    /// # Performance
    ///
    /// - Target: <5ns (single atomic load + shift + conversion)
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::widgets::SliderCapsule;
    /// use atomic_capsule::gui::Rect;
    ///
    /// let bounds = Rect::new(0, 0, 200, 20).unwrap();
    /// let slider = SliderCapsule::new(1, 0.0, 100.0, 42.5, bounds);
    /// assert!((slider.value() - 42.5).abs() < 0.01);
    /// ```
    #[inline]
    pub fn value(&self) -> f32 {
        let state = self.state.load(Ordering::Acquire);
        let current_q8 = (state & Self::CURRENT_MASK) as u16;
        Self::q8_8_to_f32(current_q8)
    }

    /// Get minimum value
    #[inline]
    pub fn min(&self) -> f32 {
        let state = self.state.load(Ordering::Acquire);
        let min_q8 = ((state & Self::MIN_MASK) >> Self::MIN_SHIFT) as u16;
        Self::q8_8_to_f32(min_q8)
    }

    /// Get maximum value
    #[inline]
    pub fn max(&self) -> f32 {
        let state = self.state.load(Ordering::Acquire);
        let max_q8 = ((state & Self::MAX_MASK) >> Self::MAX_SHIFT) as u16;
        Self::q8_8_to_f32(max_q8)
    }

    /// Get current drag state
    #[inline]
    pub fn drag_state(&self) -> DragState {
        let state = self.state.load(Ordering::Acquire);
        let drag_u8 = ((state & Self::DRAG_STATE_MASK) >> Self::DRAG_STATE_SHIFT) as u8;
        DragState::from_u8(drag_u8)
    }

    /// Set value (clamped to [min, max])
    ///
    /// # Performance
    ///
    /// - Target: <15ns (CAS loop with clamping)
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::widgets::SliderCapsule;
    /// use atomic_capsule::gui::Rect;
    ///
    /// let bounds = Rect::new(0, 0, 200, 20).unwrap();
    /// let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);
    ///
    /// slider.set_value(75.0);
    /// assert_eq!(slider.value(), 75.0);
    ///
    /// // Values clamped to [min, max]
    /// slider.set_value(150.0);
    /// assert_eq!(slider.value(), 100.0);
    ///
    /// slider.set_value(-10.0);
    /// assert_eq!(slider.value(), 0.0);
    /// ```
    #[inline]
    pub fn set_value(&self, value: f32) {
        let new_q8 = Self::f32_to_q8_8(value);

        // CAS loop to update current value (clamped to [min, max])
        let mut state = self.state.load(Ordering::Acquire);
        loop {
            let min_q8 = ((state & Self::MIN_MASK) >> Self::MIN_SHIFT) as u16;
            let max_q8 = ((state & Self::MAX_MASK) >> Self::MAX_SHIFT) as u16;

            // Clamp new value to [min, max]
            let clamped = new_q8.clamp(min_q8, max_q8);

            // Replace current value, preserve min/max/drag_state
            let new_state = (state & !Self::CURRENT_MASK) | (clamped as u64);

            match self.state.compare_exchange_weak(
                state,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Increment generation on successful update
                    self.generation.fetch_add(1, Ordering::Release);
                    return;
                }
                Err(current) => state = current,
            }
        }
    }

    /// Get normalized value (0.0-1.0)
    ///
    /// # Performance
    ///
    /// - Target: <10ns (value load + arithmetic)
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::widgets::SliderCapsule;
    /// use atomic_capsule::gui::Rect;
    ///
    /// let bounds = Rect::new(0, 0, 200, 20).unwrap();
    /// let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);
    /// assert!((slider.normalized() - 0.5).abs() < 0.01);
    /// ```
    #[inline]
    pub fn normalized(&self) -> f32 {
        let state = self.state.load(Ordering::Acquire);
        let current_q8 = (state & Self::CURRENT_MASK) as u16;
        let min_q8 = ((state & Self::MIN_MASK) >> Self::MIN_SHIFT) as u16;
        let max_q8 = ((state & Self::MAX_MASK) >> Self::MAX_SHIFT) as u16;

        // Avoid division by zero
        let range = max_q8.saturating_sub(min_q8);
        if range == 0 {
            return 0.0;
        }

        let offset = current_q8.saturating_sub(min_q8);
        offset as f32 / range as f32
    }

    /// Start drag operation
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::widgets::{SliderCapsule, DragState};
    /// use atomic_capsule::gui::{Rect, Coord};
    ///
    /// let bounds = Rect::new(0, 0, 200, 20).unwrap();
    /// let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);
    ///
    /// slider.on_drag_start(Coord::from_int(100), Coord::from_int(10));
    /// assert_eq!(slider.drag_state(), DragState::Dragging);
    /// ```
    #[inline]
    pub fn on_drag_start(&self, _x: Coord, _y: Coord) {
        // CAS loop to set drag state to Dragging
        let mut state = self.state.load(Ordering::Acquire);
        loop {
            let new_state = (state & !Self::DRAG_STATE_MASK)
                | ((DragState::Dragging as u64) << Self::DRAG_STATE_SHIFT);

            match self.state.compare_exchange_weak(
                state,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.generation.fetch_add(1, Ordering::Release);
                    return;
                }
                Err(current) => state = current,
            }
        }
    }

    /// Update value from drag position
    ///
    /// # Performance
    ///
    /// - Target: <15ns (coordinate → value conversion + CAS)
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::widgets::SliderCapsule;
    /// use atomic_capsule::gui::{Rect, Coord};
    ///
    /// let bounds = Rect::new(0, 0, 200, 20).unwrap();
    /// let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);
    ///
    /// // Drag to middle (x=100 in 200px track)
    /// slider.on_drag(Coord::from_int(100), Coord::from_int(10));
    /// assert!((slider.value() - 50.0).abs() < 1.0);
    /// ```
    #[inline]
    pub fn on_drag(&self, x: Coord, y: Coord) {
        let orientation = Orientation::from_u8(self.orientation);

        // Calculate normalized position (0.0-1.0) based on orientation
        let normalized = match orientation {
            Orientation::Horizontal => {
                let track_start = self.bounds.x.raw();
                let track_width = self.bounds.width.raw();
                if track_width == 0 {
                    return;
                }

                let offset = x.raw().saturating_sub(track_start);
                (offset as f32 / track_width as f32).clamp(0.0, 1.0)
            }
            Orientation::Vertical => {
                // Vertical: bottom=0.0, top=1.0
                let track_start = self.bounds.y.raw();
                let track_height = self.bounds.height.raw();
                if track_height == 0 {
                    return;
                }

                let offset = y.raw().saturating_sub(track_start);
                // Invert for bottom-to-top
                1.0 - (offset as f32 / track_height as f32).clamp(0.0, 1.0)
            }
        };

        // Convert normalized to value in [min, max]
        let state = self.state.load(Ordering::Acquire);
        let min_q8 = ((state & Self::MIN_MASK) >> Self::MIN_SHIFT) as u16;
        let max_q8 = ((state & Self::MAX_MASK) >> Self::MAX_SHIFT) as u16;

        let range = max_q8.saturating_sub(min_q8);
        let value_q8 = min_q8 + (range as f32 * normalized) as u16;

        // Update current value via CAS loop
        let mut state = self.state.load(Ordering::Acquire);
        loop {
            let new_state = (state & !Self::CURRENT_MASK) | (value_q8 as u64);

            match self.state.compare_exchange_weak(
                state,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.generation.fetch_add(1, Ordering::Release);
                    return;
                }
                Err(current) => state = current,
            }
        }
    }

    /// End drag operation
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::widgets::{SliderCapsule, DragState};
    /// use atomic_capsule::gui::{Rect, Coord};
    ///
    /// let bounds = Rect::new(0, 0, 200, 20).unwrap();
    /// let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);
    ///
    /// slider.on_drag_start(Coord::from_int(100), Coord::from_int(10));
    /// assert_eq!(slider.drag_state(), DragState::Dragging);
    ///
    /// slider.on_drag_end();
    /// assert_eq!(slider.drag_state(), DragState::Idle);
    /// ```
    #[inline]
    pub fn on_drag_end(&self) {
        // CAS loop to set drag state to Idle
        let mut state = self.state.load(Ordering::Acquire);
        loop {
            let new_state = (state & !Self::DRAG_STATE_MASK)
                | ((DragState::Idle as u64) << Self::DRAG_STATE_SHIFT);

            match self.state.compare_exchange_weak(
                state,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.generation.fetch_add(1, Ordering::Release);
                    return;
                }
                Err(current) => state = current,
            }
        }
    }

    /// Get thumb rectangle for rendering
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::widgets::SliderCapsule;
    /// use atomic_capsule::gui::Rect;
    ///
    /// let bounds = Rect::new(0, 0, 200, 20).unwrap();
    /// let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);
    ///
    /// let thumb = slider.thumb_rect();
    /// // At 50% (value=50.0, range=0-100), thumb center at x=100
    /// assert!((thumb.x.to_int() - 94).abs() <= 1); // 100 - thumb_width/2
    /// ```
    #[inline]
    pub fn thumb_rect(&self) -> Rect {
        let normalized = self.normalized();
        let orientation = Orientation::from_u8(self.orientation);

        match orientation {
            Orientation::Horizontal => {
                // Thumb center at (track_x + normalized * track_width)
                let track_width = self.bounds.width.raw();
                let thumb_center_offset = (track_width as f32 * normalized) as i32;
                let thumb_x = self
                    .bounds
                    .x
                    .saturating_add(Coord::from_raw(thumb_center_offset));

                // Center thumb on position
                let thumb_width_coord = Coord::from_int(self.thumb_width as i32);
                let half_width = Coord::from_raw(thumb_width_coord.raw() / 2);

                Rect {
                    x: thumb_x.saturating_sub(half_width),
                    y: self.bounds.y,
                    width: thumb_width_coord,
                    height: self.bounds.height,
                }
            }
            Orientation::Vertical => {
                // Thumb center at (track_y + (1.0 - normalized) * track_height)
                // (1.0 - normalized) because y increases downward
                let track_height = self.bounds.height.raw();
                let thumb_center_offset = (track_height as f32 * (1.0 - normalized)) as i32;
                let thumb_y = self
                    .bounds
                    .y
                    .saturating_add(Coord::from_raw(thumb_center_offset));

                // Center thumb on position
                let thumb_height_coord = Coord::from_int(self.thumb_width as i32);
                let half_height = Coord::from_raw(thumb_height_coord.raw() / 2);

                Rect {
                    x: self.bounds.x,
                    y: thumb_y.saturating_sub(half_height),
                    width: self.bounds.width,
                    height: thumb_height_coord,
                }
            }
        }
    }

    /// Check if slider is being dragged
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::widgets::SliderCapsule;
    /// use atomic_capsule::gui::{Rect, Coord};
    ///
    /// let bounds = Rect::new(0, 0, 200, 20).unwrap();
    /// let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);
    ///
    /// assert!(!slider.is_dragging());
    ///
    /// slider.on_drag_start(Coord::from_int(100), Coord::from_int(10));
    /// assert!(slider.is_dragging());
    ///
    /// slider.on_drag_end();
    /// assert!(!slider.is_dragging());
    /// ```
    #[inline]
    pub fn is_dragging(&self) -> bool {
        matches!(self.drag_state(), DragState::Dragging)
    }

    /// Get widget ID
    #[inline]
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Get track bounds
    #[inline]
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Get current generation counter
    #[inline]
    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get orientation
    #[inline]
    pub fn orientation(&self) -> Orientation {
        Orientation::from_u8(self.orientation)
    }
}

// Verify size is exactly 64 bytes
const _: [(); 64] = [(); core::mem::size_of::<SliderCapsule>()];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_alignment() {
        assert_eq!(core::mem::size_of::<SliderCapsule>(), 64);
        assert_eq!(core::mem::align_of::<SliderCapsule>(), 64);
    }

    #[test]
    fn test_creation() {
        let bounds = Rect::new(0, 0, 200, 20).unwrap();
        let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);

        assert_eq!(slider.id(), 1);
        assert!((slider.value() - 50.0).abs() < 0.1);
        assert!((slider.min() - 0.0).abs() < 0.1);
        assert!((slider.max() - 100.0).abs() < 0.1);
        assert_eq!(slider.drag_state(), DragState::Idle);
        assert!(!slider.is_dragging());
    }

    #[test]
    fn test_set_value() {
        let bounds = Rect::new(0, 0, 200, 20).unwrap();
        let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);

        slider.set_value(75.0);
        assert!((slider.value() - 75.0).abs() < 0.1);

        // Clamp to max
        slider.set_value(150.0);
        assert!((slider.value() - 100.0).abs() < 0.1);

        // Clamp to min
        slider.set_value(-10.0);
        assert!((slider.value() - 0.0).abs() < 0.1);
    }

    #[test]
    fn test_normalized() {
        let bounds = Rect::new(0, 0, 200, 20).unwrap();
        let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);

        assert!((slider.normalized() - 0.5).abs() < 0.01);

        slider.set_value(0.0);
        assert!((slider.normalized() - 0.0).abs() < 0.01);

        slider.set_value(100.0);
        assert!((slider.normalized() - 1.0).abs() < 0.01);

        slider.set_value(25.0);
        assert!((slider.normalized() - 0.25).abs() < 0.01);
    }

    #[test]
    fn test_drag_lifecycle() {
        let bounds = Rect::new(0, 0, 200, 20).unwrap();
        let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);

        // Initial state
        assert!(!slider.is_dragging());
        assert_eq!(slider.drag_state(), DragState::Idle);

        // Start drag
        slider.on_drag_start(Coord::from_int(100), Coord::from_int(10));
        assert!(slider.is_dragging());
        assert_eq!(slider.drag_state(), DragState::Dragging);

        // End drag
        slider.on_drag_end();
        assert!(!slider.is_dragging());
        assert_eq!(slider.drag_state(), DragState::Idle);
    }

    #[test]
    fn test_horizontal_drag() {
        let bounds = Rect::new(0, 0, 200, 20).unwrap();
        let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);

        // Drag to start (x=0)
        slider.on_drag(Coord::from_int(0), Coord::from_int(10));
        assert!((slider.value() - 0.0).abs() < 1.0);

        // Drag to middle (x=100)
        slider.on_drag(Coord::from_int(100), Coord::from_int(10));
        assert!((slider.value() - 50.0).abs() < 1.0);

        // Drag to end (x=200)
        slider.on_drag(Coord::from_int(200), Coord::from_int(10));
        assert!((slider.value() - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_vertical_drag() {
        let bounds = Rect::new(0, 0, 20, 200).unwrap();
        let slider = SliderCapsule::with_config(
            1,
            0.0,
            100.0,
            50.0,
            bounds,
            12,
            Orientation::Vertical,
        );

        assert_eq!(slider.orientation(), Orientation::Vertical);

        // Drag to bottom (y=200, value should be 0)
        slider.on_drag(Coord::from_int(10), Coord::from_int(200));
        assert!((slider.value() - 0.0).abs() < 1.0);

        // Drag to middle (y=100, value should be 50)
        slider.on_drag(Coord::from_int(10), Coord::from_int(100));
        assert!((slider.value() - 50.0).abs() < 1.0);

        // Drag to top (y=0, value should be 100)
        slider.on_drag(Coord::from_int(10), Coord::from_int(0));
        assert!((slider.value() - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_thumb_rect_horizontal() {
        let bounds = Rect::new(0, 0, 200, 20).unwrap();
        let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);

        let thumb = slider.thumb_rect();

        // At 50% (value=50, range=0-100), thumb center at x=100
        // Thumb width=12, so x = 100 - 6 = 94
        assert!((thumb.x.to_int() - 94).abs() <= 1);
        assert_eq!(thumb.y.to_int(), 0);
        assert_eq!(thumb.width.to_int(), 12);
        assert_eq!(thumb.height.to_int(), 20);
    }

    #[test]
    fn test_thumb_rect_vertical() {
        let bounds = Rect::new(0, 0, 20, 200).unwrap();
        let slider = SliderCapsule::with_config(
            1,
            0.0,
            100.0,
            50.0,
            bounds,
            12,
            Orientation::Vertical,
        );

        let thumb = slider.thumb_rect();

        // At 50% (value=50, range=0-100), thumb center at y=100 (from bottom)
        // Thumb height=12, so y = 100 - 6 = 94
        assert_eq!(thumb.x.to_int(), 0);
        assert!((thumb.y.to_int() - 94).abs() <= 1);
        assert_eq!(thumb.width.to_int(), 20);
        assert_eq!(thumb.height.to_int(), 12);
    }

    #[test]
    fn test_generation_updates() {
        let bounds = Rect::new(0, 0, 200, 20).unwrap();
        let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);

        let gen0 = slider.generation();

        slider.set_value(75.0);
        let gen1 = slider.generation();
        assert_eq!(gen1, gen0 + 1);

        slider.on_drag_start(Coord::from_int(100), Coord::from_int(10));
        let gen2 = slider.generation();
        assert_eq!(gen2, gen1 + 1);

        slider.on_drag(Coord::from_int(150), Coord::from_int(10));
        let gen3 = slider.generation();
        assert_eq!(gen3, gen2 + 1);

        slider.on_drag_end();
        let gen4 = slider.generation();
        assert_eq!(gen4, gen3 + 1);
    }

    #[test]
    fn test_q8_8_precision() {
        let bounds = Rect::new(0, 0, 200, 20).unwrap();
        let slider = SliderCapsule::new(1, 0.0, 100.0, 0.0, bounds);

        // Test Q8.8 precision (1/256 ≈ 0.00390625)
        slider.set_value(42.5);
        assert!((slider.value() - 42.5).abs() < 0.01);

        slider.set_value(100.25);
        assert!((slider.value() - 100.0).abs() < 0.01); // Clamped to max

        slider.set_value(0.00390625); // Smallest increment
        assert!(slider.value() > 0.0);
    }

    #[test]
    fn test_edge_cases() {
        // Zero-width track (normalized still valid - uses min/max not bounds)
        let bounds = Rect::new(0, 0, 0, 20).unwrap();
        let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);
        assert_eq!(slider.normalized(), 0.5); // Value position is still valid

        // Min == Max (division by zero guard)
        let bounds = Rect::new(0, 0, 200, 20).unwrap();
        let slider = SliderCapsule::new(1, 50.0, 50.0, 50.0, bounds);
        assert_eq!(slider.normalized(), 0.0); // Avoid division by zero
        assert!((slider.value() - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_concurrent_updates() {
        use std::sync::Arc;
        use std::thread;

        let bounds = Rect::new(0, 0, 200, 20).unwrap();
        let slider = Arc::new(SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds));

        let mut handles = vec![];

        // 4 threads updating value concurrently
        for i in 0..4 {
            let slider = Arc::clone(&slider);
            handles.push(thread::spawn(move || {
                for j in 0..100 {
                    let value = ((i * 100 + j) % 101) as f32;
                    slider.set_value(value);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Final value should be valid (in [0, 100])
        let final_value = slider.value();
        assert!(final_value >= 0.0 && final_value <= 100.0);

        // Generation counter should reflect all updates
        assert!(slider.generation() >= 400);
    }

    #[test]
    fn test_drag_state_transitions() {
        let bounds = Rect::new(0, 0, 200, 20).unwrap();
        let slider = SliderCapsule::new(1, 0.0, 100.0, 50.0, bounds);

        // Idle → Dragging
        slider.on_drag_start(Coord::from_int(100), Coord::from_int(10));
        assert_eq!(slider.drag_state(), DragState::Dragging);

        // Dragging → Idle
        slider.on_drag_end();
        assert_eq!(slider.drag_state(), DragState::Idle);

        // Rapid start/end
        for _ in 0..100 {
            slider.on_drag_start(Coord::from_int(100), Coord::from_int(10));
            assert!(slider.is_dragging());
            slider.on_drag_end();
            assert!(!slider.is_dragging());
        }
    }
}
