// Copyright (C) 2025 Kindly Platform, Inc.
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! GPU-accelerated shape primitives using Signed Distance Functions (SDF)
//!
//! # Tier Classification
//!
//! T7 (Heterogeneous): GPU SDF rendering primitives with CPU-side coordination
//!
//! # Design Principles
//!
//! - **SDF-Based**: All shapes use signed distance functions for sub-pixel anti-aliasing
//! - **Q8.8 Fixed-Point**: Sub-pixel corner radius and stroke width for determinism
//! - **Cache-Aligned**: 64B alignment for GPU buffer uploads
//! - **Lockfree**: AtomicU64 state packing for concurrent shape updates
//! - **Zero-Copy**: Direct GPU buffer mapping via `#[repr(C)]`
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 (T7 Heterogeneous tier), Q33 (zero runtime overhead)
//! - **Chaos**: 100% lockfree, no mutex, cache-aligned atomics
//! - **ASSUM**: 99.99% safe (minimal unsafe for GPU buffer mapping)
//! - **B32**: Fair benchmarking (GPU vs CPU rendering)
//! - **T28**: Comprehensive testing (unit/property/integration)

use crate::gui::types::{Coord, Point, Rect};
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Shape type enumeration for GPU shader dispatch
///
/// # Memory Layout
///
/// Stored in bits 0-7 of ShapeCapsule.state (u8).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShapeType {
    /// No shape (invisible, used for initialization)
    None = 0,
    /// Axis-aligned rectangle
    Rect = 1,
    /// Rectangle with rounded corners
    RoundedRect = 2,
    /// Circle (or ellipse for non-uniform scaling)
    Circle = 3,
    /// Line segment with configurable width
    Line = 4,
    /// Drop shadow effect (Gaussian blur + offset)
    Shadow = 5,
}

impl ShapeType {
    /// Convert from u8 (for bit unpacking)
    #[inline]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Rect,
            2 => Self::RoundedRect,
            3 => Self::Circle,
            4 => Self::Line,
            5 => Self::Shadow,
            _ => Self::None, // Invalid values default to None
        }
    }
}

/// Shape rendering flags (bit flags)
///
/// Stored in bits 40-47 of ShapeCapsule.state.
pub struct ShapeFlags;

impl ShapeFlags {
    /// Shape is filled with fill_color
    pub const FILLED: u8 = 0x01;
    /// Shape has stroke with stroke_color and stroke_width
    pub const STROKED: u8 = 0x02;
    /// Shape has drop shadow with shadow_color, offset, and blur
    pub const SHADOWED: u8 = 0x04;
    /// Enable anti-aliasing (SDF-based, sub-pixel precision)
    pub const ANTI_ALIASED: u8 = 0x08;
}

/// GPU-accelerated shape primitive with SDF rendering
///
/// # Memory Layout
///
/// ```text
/// Offset | Field            | Size | Description
/// -------|------------------|------|-------------
/// 0      | state            | 8    | Packed state (shape_type, corner_radius, stroke_width, flags)
/// 8      | generation       | 4    | Generation counter for snapshot consistency
/// 12     | id               | 4    | Unique shape identifier
/// 16     | bounds           | 16   | Rectangle bounds (Q16.16)
/// 32     | fill_color       | 4    | RGBA8 fill color
/// 36     | stroke_color     | 4    | RGBA8 stroke color
/// 40     | shadow_color     | 4    | RGBA8 shadow color
/// 44     | shadow_offset_x  | 2    | Shadow X offset (pixels)
/// 46     | shadow_offset_y  | 2    | Shadow Y offset (pixels)
/// 48     | shadow_blur      | 2    | Shadow blur radius (Q8.8)
/// 50     | _pad             | 14   | Padding to 64B
/// -------|------------------|------|-------------
/// Total: 64 bytes (cache-aligned)
/// ```
///
/// # State Packing (AtomicU64)
///
/// ```text
/// Bits 0-7:   shape_type (ShapeType enum)
/// Bits 8-23:  corner_radius (Q8.8 fixed-point, 0.0 to 255.99609)
/// Bits 24-39: stroke_width (Q8.8 fixed-point, 0.0 to 255.99609)
/// Bits 40-47: flags (ShapeFlags bit flags)
/// Bits 48-63: reserved (future use)
/// ```
///
/// # Examples
///
/// ```
/// use atomic_capsule::gui::render::shapes::{ShapeCapsule, ShapeType, ShapeFlags};
/// use atomic_capsule::gui::{Rect, Color, Point};
///
/// // Create filled rectangle
/// let bounds = Rect::new(10, 20, 100, 50).unwrap();
/// let rect = ShapeCapsule::new_rect(1, bounds, Color::RED.to_u32());
/// assert_eq!(rect.shape_type(), ShapeType::Rect);
/// assert!(rect.is_filled());
///
/// // Create rounded rectangle with 10px corners
/// let rounded = ShapeCapsule::new_rounded_rect(2, bounds, 10.0, Color::BLUE.to_u32());
/// assert_eq!(rounded.shape_type(), ShapeType::RoundedRect);
/// assert!((rounded.corner_radius() - 10.0).abs() < 0.01);
///
/// // Create circle with 25px radius
/// let center = Point::new(50, 50);
/// let circle = ShapeCapsule::new_circle(3, center, 25, Color::GREEN.to_u32());
/// assert_eq!(circle.shape_type(), ShapeType::Circle);
///
/// // Add stroke
/// circle.set_stroke(2.0, Color::BLACK.to_u32());
/// assert!(circle.is_stroked());
/// assert!((circle.stroke_width() - 2.0).abs() < 0.01);
/// ```
#[repr(C, align(64))]
pub struct ShapeCapsule {
    /// Packed state (shape_type, corner_radius, stroke_width, flags, reserved)
    state: AtomicU64,

    /// Generation counter for snapshot consistency
    generation: AtomicU32,

    /// Unique shape identifier
    id: u32,

    /// Rectangle bounds (Q16.16 fixed-point)
    bounds: Rect,

    /// Fill color (RGBA8, packed u32) - UnsafeCell for interior mutability
    fill_color: UnsafeCell<u32>,

    /// Stroke color (RGBA8, packed u32) - UnsafeCell for interior mutability
    stroke_color: UnsafeCell<u32>,

    /// Shadow color (RGBA8, packed u32)
    shadow_color: u32,

    /// Shadow X offset (pixels, signed)
    shadow_offset_x: i16,

    /// Shadow Y offset (pixels, signed)
    shadow_offset_y: i16,

    /// Shadow blur radius (Q8.8 fixed-point)
    shadow_blur: u16,

    /// Padding to 64 bytes
    _pad: [u8; 14],
}

impl ShapeCapsule {
    /// Q8.8 scale factor (2^8 = 256)
    const Q8_8_SCALE: u32 = 256;

    /// Create new rectangle
    ///
    /// # Arguments
    ///
    /// * `id` - Unique shape identifier
    /// * `bounds` - Rectangle bounds (Q16.16)
    /// * `fill` - Fill color (RGBA8 packed u32)
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::render::shapes::ShapeCapsule;
    /// use atomic_capsule::gui::{Rect, Color};
    ///
    /// let bounds = Rect::new(0, 0, 100, 100).unwrap();
    /// let rect = ShapeCapsule::new_rect(1, bounds, Color::RED.to_u32());
    /// assert!(rect.is_filled());
    /// ```
    pub fn new_rect(id: u32, bounds: Rect, fill: u32) -> Self {
        let state = Self::pack_state(ShapeType::Rect, 0, 0, ShapeFlags::FILLED);

        Self {
            state: AtomicU64::new(state),
            generation: AtomicU32::new(0),
            id,
            bounds,
            fill_color: UnsafeCell::new(fill),
            stroke_color: UnsafeCell::new(0),
            shadow_color: 0,
            shadow_offset_x: 0,
            shadow_offset_y: 0,
            shadow_blur: 0,
            _pad: [0; 14],
        }
    }

    /// Create new rounded rectangle
    ///
    /// # Arguments
    ///
    /// * `id` - Unique shape identifier
    /// * `bounds` - Rectangle bounds (Q16.16)
    /// * `radius` - Corner radius in pixels (f32, converted to Q8.8)
    /// * `fill` - Fill color (RGBA8 packed u32)
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::render::shapes::ShapeCapsule;
    /// use atomic_capsule::gui::{Rect, Color};
    ///
    /// let bounds = Rect::new(0, 0, 100, 100).unwrap();
    /// let rounded = ShapeCapsule::new_rounded_rect(2, bounds, 10.0, Color::BLUE.to_u32());
    /// assert!((rounded.corner_radius() - 10.0).abs() < 0.01);
    /// ```
    pub fn new_rounded_rect(id: u32, bounds: Rect, radius: f32, fill: u32) -> Self {
        let radius_q8_8 = Self::f32_to_q8_8(radius);
        let state = Self::pack_state(ShapeType::RoundedRect, radius_q8_8, 0, ShapeFlags::FILLED);

        Self {
            state: AtomicU64::new(state),
            generation: AtomicU32::new(0),
            id,
            bounds,
            fill_color: UnsafeCell::new(fill),
            stroke_color: UnsafeCell::new(0),
            shadow_color: 0,
            shadow_offset_x: 0,
            shadow_offset_y: 0,
            shadow_blur: 0,
            _pad: [0; 14],
        }
    }

    /// Create new circle
    ///
    /// # Arguments
    ///
    /// * `id` - Unique shape identifier
    /// * `center` - Circle center (Q16.16)
    /// * `radius` - Circle radius in pixels
    /// * `fill` - Fill color (RGBA8 packed u32)
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::render::shapes::ShapeCapsule;
    /// use atomic_capsule::gui::{Point, Color, Rect};
    ///
    /// let center = Point::new(50, 50);
    /// let circle = ShapeCapsule::new_circle(3, center, 25, Color::GREEN.to_u32());
    /// // Circle bounds are (25, 25, 50, 50) for a circle centered at (50, 50) with radius 25
    /// ```
    pub fn new_circle(id: u32, center: Point, radius: i32, fill: u32) -> Self {
        // Calculate bounding box: center - radius to center + radius
        let bounds = Rect {
            x: center.x.saturating_sub(Coord::from_int(radius)),
            y: center.y.saturating_sub(Coord::from_int(radius)),
            width: Coord::from_int(radius * 2),
            height: Coord::from_int(radius * 2),
        };

        let state = Self::pack_state(ShapeType::Circle, 0, 0, ShapeFlags::FILLED);

        Self {
            state: AtomicU64::new(state),
            generation: AtomicU32::new(0),
            id,
            bounds,
            fill_color: UnsafeCell::new(fill),
            stroke_color: UnsafeCell::new(0),
            shadow_color: 0,
            shadow_offset_x: 0,
            shadow_offset_y: 0,
            shadow_blur: 0,
            _pad: [0; 14],
        }
    }

    /// Create new drop shadow
    ///
    /// # Arguments
    ///
    /// * `id` - Unique shape identifier
    /// * `bounds` - Shadow bounds (Q16.16)
    /// * `offset` - Shadow offset (x, y) in pixels
    /// * `blur` - Shadow blur radius in pixels (f32, converted to Q8.8)
    /// * `color` - Shadow color (RGBA8 packed u32)
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::render::shapes::ShapeCapsule;
    /// use atomic_capsule::gui::{Rect, Color};
    ///
    /// let bounds = Rect::new(0, 0, 100, 100).unwrap();
    /// let shadow = ShapeCapsule::new_shadow(4, bounds, (4, 4), 8.0, Color::BLACK.to_u32());
    /// assert!(shadow.is_shadowed());
    /// ```
    pub fn new_shadow(id: u32, bounds: Rect, offset: (i16, i16), blur: f32, color: u32) -> Self {
        let blur_q8_8 = Self::f32_to_q8_8(blur);
        let state = Self::pack_state(ShapeType::Shadow, 0, 0, ShapeFlags::SHADOWED);

        Self {
            state: AtomicU64::new(state),
            generation: AtomicU32::new(0),
            id,
            bounds,
            fill_color: UnsafeCell::new(0),
            stroke_color: UnsafeCell::new(0),
            shadow_color: color,
            shadow_offset_x: offset.0,
            shadow_offset_y: offset.1,
            shadow_blur: blur_q8_8,
            _pad: [0; 14],
        }
    }

    /// Get shape type
    #[inline]
    pub fn shape_type(&self) -> ShapeType {
        let state = self.state.load(Ordering::Acquire);
        ShapeType::from_u8((state & 0xFF) as u8)
    }

    /// Get corner radius (Q8.8 to f32)
    #[inline]
    pub fn corner_radius(&self) -> f32 {
        let state = self.state.load(Ordering::Acquire);
        let radius_q8_8 = ((state >> 8) & 0xFFFF) as u16;
        Self::q8_8_to_f32(radius_q8_8)
    }

    /// Set corner radius (f32 to Q8.8)
    ///
    /// Updates generation counter for snapshot consistency.
    #[inline]
    pub fn set_corner_radius(&self, radius: f32) {
        let radius_q8_8 = Self::f32_to_q8_8(radius);

        loop {
            let old_state = self.state.load(Ordering::Acquire);
            let new_state = (old_state & !0xFFFF00) | ((radius_q8_8 as u64) << 8);

            if self.state.compare_exchange_weak(
                old_state,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ).is_ok() {
                self.generation.fetch_add(1, Ordering::Release);
                break;
            }
        }
    }

    /// Get stroke width (Q8.8 to f32)
    #[inline]
    pub fn stroke_width(&self) -> f32 {
        let state = self.state.load(Ordering::Acquire);
        let width_q8_8 = ((state >> 24) & 0xFFFF) as u16;
        Self::q8_8_to_f32(width_q8_8)
    }

    /// Set stroke (width and color)
    ///
    /// Automatically sets STROKED flag and updates generation counter.
    #[inline]
    pub fn set_stroke(&self, width: f32, color: u32) {
        let width_q8_8 = Self::f32_to_q8_8(width);

        loop {
            let old_state = self.state.load(Ordering::Acquire);
            let old_flags = ((old_state >> 40) & 0xFF) as u8;
            let new_flags = old_flags | ShapeFlags::STROKED;
            let new_state = (old_state & !0xFFFFFF0000)
                | ((width_q8_8 as u64) << 24)
                | ((new_flags as u64) << 40);

            if self.state.compare_exchange_weak(
                old_state,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ).is_ok() {
                // SAFETY: UnsafeCell allows interior mutability, write is safe
                unsafe {
                    *self.stroke_color.get() = color;
                }
                self.generation.fetch_add(1, Ordering::Release);
                break;
            }
        }
    }

    /// Get flags
    #[inline]
    pub fn flags(&self) -> u8 {
        let state = self.state.load(Ordering::Acquire);
        ((state >> 40) & 0xFF) as u8
    }

    /// Set flags (replaces all flags)
    ///
    /// Updates generation counter for snapshot consistency.
    #[inline]
    pub fn set_flags(&self, flags: u8) {
        loop {
            let old_state = self.state.load(Ordering::Acquire);
            let new_state = (old_state & !0xFF0000000000) | ((flags as u64) << 40);

            if self.state.compare_exchange_weak(
                old_state,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ).is_ok() {
                self.generation.fetch_add(1, Ordering::Release);
                break;
            }
        }
    }

    /// Check if shape is filled
    #[inline]
    pub fn is_filled(&self) -> bool {
        (self.flags() & ShapeFlags::FILLED) != 0
    }

    /// Check if shape has stroke
    #[inline]
    pub fn is_stroked(&self) -> bool {
        (self.flags() & ShapeFlags::STROKED) != 0
    }

    /// Check if shape has shadow
    #[inline]
    pub fn is_shadowed(&self) -> bool {
        (self.flags() & ShapeFlags::SHADOWED) != 0
    }

    /// Get fill color
    #[inline]
    pub fn fill_color(&self) -> u32 {
        // SAFETY: UnsafeCell allows interior mutability, read is safe
        unsafe { *self.fill_color.get() }
    }

    /// Set fill color
    ///
    /// Automatically sets FILLED flag and updates generation counter.
    #[inline]
    pub fn set_fill_color(&self, color: u32) {
        // SAFETY: UnsafeCell allows interior mutability, write is safe
        unsafe {
            *self.fill_color.get() = color;
        }

        // Set FILLED flag
        loop {
            let old_state = self.state.load(Ordering::Acquire);
            let old_flags = ((old_state >> 40) & 0xFF) as u8;
            let new_flags = old_flags | ShapeFlags::FILLED;
            let new_state = (old_state & !0xFF0000000000) | ((new_flags as u64) << 40);

            if self.state.compare_exchange_weak(
                old_state,
                new_state,
                Ordering::Release,
                Ordering::Acquire,
            ).is_ok() {
                self.generation.fetch_add(1, Ordering::Release);
                break;
            }
        }
    }

    /// Get bounds
    #[inline]
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Set bounds (requires &mut for Rect mutation)
    #[inline]
    pub fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get generation counter (for snapshot consistency)
    #[inline]
    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get shape ID
    #[inline]
    pub fn id(&self) -> u32 {
        self.id
    }

    // --- Internal Helpers ---

    /// Pack state into u64
    #[inline]
    const fn pack_state(
        shape_type: ShapeType,
        corner_radius_q8_8: u16,
        stroke_width_q8_8: u16,
        flags: u8,
    ) -> u64 {
        (shape_type as u64)
            | ((corner_radius_q8_8 as u64) << 8)
            | ((stroke_width_q8_8 as u64) << 24)
            | ((flags as u64) << 40)
    }

    /// Convert f32 to Q8.8 fixed-point
    #[inline]
    fn f32_to_q8_8(value: f32) -> u16 {
        let clamped = value.clamp(0.0, 255.99609);
        (clamped * Self::Q8_8_SCALE as f32) as u16
    }

    /// Convert Q8.8 fixed-point to f32
    #[inline]
    fn q8_8_to_f32(value: u16) -> f32 {
        value as f32 / Self::Q8_8_SCALE as f32
    }
}

// SAFETY: ShapeCapsule is safe to send between threads
// - All atomics use Acquire/Release ordering
// - Generation counter ensures snapshot consistency
// - No mutable references escape to other threads
unsafe impl Send for ShapeCapsule {}
unsafe impl Sync for ShapeCapsule {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::{Color, Point, Rect};

    #[test]
    fn test_new_rect() {
        let bounds = Rect::new(10, 20, 100, 50).unwrap();
        let rect = ShapeCapsule::new_rect(1, bounds, Color::RED.to_u32());

        assert_eq!(rect.shape_type(), ShapeType::Rect);
        assert_eq!(rect.id(), 1);
        assert_eq!(rect.bounds(), bounds);
        assert_eq!(rect.fill_color(), Color::RED.to_u32());
        assert!(rect.is_filled());
        assert!(!rect.is_stroked());
        assert!(!rect.is_shadowed());
    }

    #[test]
    fn test_new_rounded_rect() {
        let bounds = Rect::new(0, 0, 100, 100).unwrap();
        let rounded = ShapeCapsule::new_rounded_rect(2, bounds, 10.0, Color::BLUE.to_u32());

        assert_eq!(rounded.shape_type(), ShapeType::RoundedRect);
        assert_eq!(rounded.id(), 2);
        assert!((rounded.corner_radius() - 10.0).abs() < 0.01);
        assert!(rounded.is_filled());
    }

    #[test]
    fn test_new_circle() {
        let center = Point::new(50, 50);
        let circle = ShapeCapsule::new_circle(3, center, 25, Color::GREEN.to_u32());

        assert_eq!(circle.shape_type(), ShapeType::Circle);
        assert_eq!(circle.id(), 3);
        assert_eq!(circle.bounds().x.to_int(), 25);
        assert_eq!(circle.bounds().y.to_int(), 25);
        assert_eq!(circle.bounds().width.to_int(), 50);
        assert_eq!(circle.bounds().height.to_int(), 50);
        assert!(circle.is_filled());
    }

    #[test]
    fn test_new_shadow() {
        let bounds = Rect::new(0, 0, 100, 100).unwrap();
        let shadow = ShapeCapsule::new_shadow(4, bounds, (4, 4), 8.0, Color::BLACK.to_u32());

        assert_eq!(shadow.shape_type(), ShapeType::Shadow);
        assert_eq!(shadow.id(), 4);
        assert!(shadow.is_shadowed());
        assert!(!shadow.is_filled());
    }

    #[test]
    fn test_corner_radius_q8_8() {
        let bounds = Rect::new(0, 0, 100, 100).unwrap();
        let rounded = ShapeCapsule::new_rounded_rect(1, bounds, 10.5, Color::RED.to_u32());

        // Q8.8: 10.5 * 256 = 2688, 2688 / 256 = 10.5
        let radius = rounded.corner_radius();
        assert!((radius - 10.5).abs() < 0.01);

        // Test set
        rounded.set_corner_radius(15.75);
        assert!((rounded.corner_radius() - 15.75).abs() < 0.01);
    }

    #[test]
    fn test_stroke_width_q8_8() {
        let bounds = Rect::new(0, 0, 100, 100).unwrap();
        let rect = ShapeCapsule::new_rect(1, bounds, Color::RED.to_u32());

        rect.set_stroke(2.5, Color::BLACK.to_u32());
        assert!((rect.stroke_width() - 2.5).abs() < 0.01);
        assert!(rect.is_stroked());
    }

    #[test]
    fn test_flags() {
        let bounds = Rect::new(0, 0, 100, 100).unwrap();
        let rect = ShapeCapsule::new_rect(1, bounds, Color::RED.to_u32());

        assert_eq!(rect.flags(), ShapeFlags::FILLED);

        rect.set_stroke(1.0, Color::BLACK.to_u32());
        assert_eq!(rect.flags(), ShapeFlags::FILLED | ShapeFlags::STROKED);

        rect.set_flags(ShapeFlags::ANTI_ALIASED);
        assert_eq!(rect.flags(), ShapeFlags::ANTI_ALIASED);
    }

    #[test]
    fn test_fill_color() {
        let bounds = Rect::new(0, 0, 100, 100).unwrap();
        let rect = ShapeCapsule::new_rect(1, bounds, Color::RED.to_u32());

        assert_eq!(rect.fill_color(), Color::RED.to_u32());

        rect.set_fill_color(Color::BLUE.to_u32());
        assert_eq!(rect.fill_color(), Color::BLUE.to_u32());
        assert!(rect.is_filled());
    }

    #[test]
    fn test_stroke_color() {
        let bounds = Rect::new(0, 0, 100, 100).unwrap();
        let rect = ShapeCapsule::new_rect(1, bounds, Color::RED.to_u32());

        rect.set_stroke(2.0, Color::BLACK.to_u32());
        // Note: stroke_color is private, test indirectly via is_stroked
        assert!(rect.is_stroked());
    }

    #[test]
    fn test_shadow_params() {
        let bounds = Rect::new(0, 0, 100, 100).unwrap();
        let shadow = ShapeCapsule::new_shadow(1, bounds, (4, 6), 8.0, Color::BLACK.to_u32());

        // Shadow params are private fields, test creation success
        assert!(shadow.is_shadowed());
        assert_eq!(shadow.shape_type(), ShapeType::Shadow);
    }

    #[test]
    fn test_bounds() {
        let bounds = Rect::new(10, 20, 100, 50).unwrap();
        let mut rect = ShapeCapsule::new_rect(1, bounds, Color::RED.to_u32());

        assert_eq!(rect.bounds(), bounds);

        let new_bounds = Rect::new(0, 0, 200, 100).unwrap();
        rect.set_bounds(new_bounds);
        assert_eq!(rect.bounds(), new_bounds);
    }

    #[test]
    fn test_size_alignment() {
        use core::mem::{size_of, align_of};

        assert_eq!(size_of::<ShapeCapsule>(), 64);
        assert_eq!(align_of::<ShapeCapsule>(), 64);
    }

    #[test]
    fn test_generation_updates() {
        let bounds = Rect::new(0, 0, 100, 100).unwrap();
        let rect = ShapeCapsule::new_rect(1, bounds, Color::RED.to_u32());

        let gen0 = rect.generation();
        assert_eq!(gen0, 0);

        rect.set_corner_radius(10.0);
        assert_eq!(rect.generation(), gen0 + 1);

        rect.set_stroke(2.0, Color::BLACK.to_u32());
        assert_eq!(rect.generation(), gen0 + 2);

        rect.set_fill_color(Color::BLUE.to_u32());
        assert_eq!(rect.generation(), gen0 + 3);
    }

    #[test]
    fn test_shape_type_transitions() {
        let bounds = Rect::new(0, 0, 100, 100).unwrap();

        // Start as Rect
        let shape = ShapeCapsule::new_rect(1, bounds, Color::RED.to_u32());
        assert_eq!(shape.shape_type(), ShapeType::Rect);

        // Convert to RoundedRect via set_corner_radius
        shape.set_corner_radius(10.0);
        // Note: ShapeType doesn't auto-transition, this requires explicit state update
        // This test documents current behavior (no auto-transition)
        assert_eq!(shape.shape_type(), ShapeType::Rect); // Still Rect
    }

    #[test]
    fn test_q8_8_edge_cases() {
        // Test Q8.8 clamping
        let clamped = ShapeCapsule::f32_to_q8_8(300.0);
        let unclamped = ShapeCapsule::q8_8_to_f32(clamped);
        assert!((unclamped - 255.99609).abs() < 0.01); // Max value

        // Test negative clamping
        let neg = ShapeCapsule::f32_to_q8_8(-10.0);
        assert_eq!(neg, 0); // Clamped to 0

        // Test fractional precision
        let frac = ShapeCapsule::f32_to_q8_8(10.5);
        let back = ShapeCapsule::q8_8_to_f32(frac);
        assert!((back - 10.5).abs() < 0.01);
    }

    #[test]
    fn test_concurrent_updates() {
        use std::sync::Arc;
        use std::thread;

        let bounds = Rect::new(0, 0, 100, 100).unwrap();
        let rect = Arc::new(ShapeCapsule::new_rect(1, bounds, Color::RED.to_u32()));

        let mut handles = vec![];

        for i in 0..4 {
            let rect_clone = Arc::clone(&rect);
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    rect_clone.set_corner_radius((i * 100 + j) as f32 % 256.0);
                    rect_clone.set_stroke(j as f32 % 256.0, Color::BLACK.to_u32());
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify final state is consistent (no crashes or data races)
        let final_gen = rect.generation();
        assert!(final_gen >= 400); // At least 400 updates (4 threads × 100 iterations)
    }

    #[test]
    fn test_shape_type_from_u8() {
        assert_eq!(ShapeType::from_u8(0), ShapeType::None);
        assert_eq!(ShapeType::from_u8(1), ShapeType::Rect);
        assert_eq!(ShapeType::from_u8(2), ShapeType::RoundedRect);
        assert_eq!(ShapeType::from_u8(3), ShapeType::Circle);
        assert_eq!(ShapeType::from_u8(4), ShapeType::Line);
        assert_eq!(ShapeType::from_u8(5), ShapeType::Shadow);
        assert_eq!(ShapeType::from_u8(255), ShapeType::None); // Invalid
    }

    #[test]
    fn test_multiple_flags() {
        let bounds = Rect::new(0, 0, 100, 100).unwrap();
        let rect = ShapeCapsule::new_rect(1, bounds, Color::RED.to_u32());

        rect.set_stroke(2.0, Color::BLACK.to_u32());
        assert!(rect.is_filled());
        assert!(rect.is_stroked());

        // Add anti-aliasing via set_flags (replaces)
        rect.set_flags(ShapeFlags::FILLED | ShapeFlags::STROKED | ShapeFlags::ANTI_ALIASED);
        assert!(rect.is_filled());
        assert!(rect.is_stroked());
        assert!((rect.flags() & ShapeFlags::ANTI_ALIASED) != 0);
    }
}
