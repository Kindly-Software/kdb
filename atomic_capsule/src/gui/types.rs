// Copyright (C) 2025 Kindly Platform, Inc.
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Core geometric and color types for Chaos-compliant GUI framework
//!
//! # Tier Classification
//!
//! T0 (Auditable) + T3 (Fixed-Point): Deterministic Q16.16 coordinates
//!
//! # Design Principles
//!
//! - **Deterministic**: Q16.16 fixed-point for exact reproducibility
//! - **FFI-Safe**: All types are `#[repr(C)]` for cross-language usage
//! - **Cache-Aligned**: Types with atomics use 64B/128B alignment
//! - **Zero-Copy**: Designed for direct GPU buffer uploads
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 (T0/T3 tier), Q33 (no runtime overhead)
//! - **Chaos**: No mutex, no Arc, deterministic construction
//! - **ASSUM**: 100% safe (no unsafe code in public API)

use super::error::{GuiError, GuiResult};
use core::fmt;

/// Q16.16 fixed-point coordinate type
///
/// Provides deterministic sub-pixel precision with 16 bits of fractional precision.
/// Range: -32768.0 to 32767.99998 (exact)
///
/// # Memory Layout
///
/// ```text
/// | Sign (1) | Integer (15) | Fraction (16) |
/// |    31    |    30..16    |     15..0     |
/// ```
///
/// # Examples
///
/// ```
/// use atomic_capsule::gui::Coord;
///
/// let c = Coord::from_int(100);
/// assert_eq!(c.to_int(), 100);
///
/// let c2 = Coord::from_float(100.5);
/// assert_eq!(c2.to_float(), 100.5);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Coord(i32);

impl Coord {
    /// Fractional bits (16)
    pub const FRAC_BITS: u32 = 16;

    /// Scale factor (2^16 = 65536)
    pub const SCALE: i32 = 1 << Self::FRAC_BITS;

    /// Zero coordinate
    pub const ZERO: Self = Self(0);

    /// One pixel
    pub const ONE: Self = Self(Self::SCALE);

    /// Minimum value (-32768.0)
    pub const MIN: Self = Self(i32::MIN);

    /// Maximum value (32767.99998)
    pub const MAX: Self = Self(i32::MAX);

    /// Create from integer pixels
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::Coord;
    ///
    /// let c = Coord::from_int(42);
    /// assert_eq!(c.to_int(), 42);
    /// ```
    #[inline]
    pub const fn from_int(pixels: i32) -> Self {
        Self(pixels.saturating_mul(Self::SCALE))
    }

    /// Create from floating-point pixels
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::Coord;
    ///
    /// let c = Coord::from_float(42.5);
    /// assert!((c.to_float() - 42.5).abs() < 0.0001);
    /// ```
    #[inline]
    pub fn from_float(pixels: f32) -> Self {
        Self((pixels * Self::SCALE as f32) as i32)
    }

    /// Create from raw Q16.16 value
    ///
    /// # Safety
    ///
    /// No validation is performed. Use with caution.
    #[inline]
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    /// Get raw Q16.16 value
    #[inline]
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// Convert to integer pixels (truncate fraction)
    #[inline]
    pub const fn to_int(self) -> i32 {
        self.0 >> Self::FRAC_BITS
    }

    /// Convert to floating-point pixels
    #[inline]
    pub fn to_float(self) -> f32 {
        self.0 as f32 / Self::SCALE as f32
    }

    /// Add two coordinates (saturating)
    #[inline]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    /// Subtract two coordinates (saturating)
    #[inline]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    /// Multiply by scalar (saturating)
    #[inline]
    pub const fn saturating_mul(self, scalar: i32) -> Self {
        Self(self.0.saturating_mul(scalar))
    }

    /// Get absolute value
    #[inline]
    pub const fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Check if coordinate is zero
    #[inline]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Check if coordinate is positive
    #[inline]
    pub const fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// Check if coordinate is negative
    #[inline]
    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }
}

impl fmt::Display for Coord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.to_float())
    }
}

/// 2D point with Q16.16 coordinates
///
/// # FFI Safety
///
/// `#[repr(C)]` guarantees layout compatibility with C structs.
///
/// # Examples
///
/// ```
/// use atomic_capsule::gui::Point;
///
/// let p = Point::new(100, 200);
/// assert_eq!(p.x.to_int(), 100);
/// assert_eq!(p.y.to_int(), 200);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Point {
    /// X coordinate
    pub x: Coord,
    /// Y coordinate
    pub y: Coord,
}

impl Point {
    /// Origin point (0, 0)
    pub const ORIGIN: Self = Self {
        x: Coord::ZERO,
        y: Coord::ZERO,
    };

    /// Create point from integer coordinates
    #[inline]
    pub const fn new(x: i32, y: i32) -> Self {
        Self {
            x: Coord::from_int(x),
            y: Coord::from_int(y),
        }
    }

    /// Create point from floating-point coordinates
    #[inline]
    pub fn from_float(x: f32, y: f32) -> Self {
        Self {
            x: Coord::from_float(x),
            y: Coord::from_float(y),
        }
    }

    /// Translate point by offset (saturating)
    #[inline]
    pub const fn translate(self, dx: Coord, dy: Coord) -> Self {
        Self {
            x: self.x.saturating_add(dx),
            y: self.y.saturating_add(dy),
        }
    }

    /// Calculate squared distance to another point (avoids sqrt)
    #[inline]
    pub fn distance_squared(self, other: Self) -> i64 {
        let dx = (self.x.0 - other.x.0) as i64;
        let dy = (self.y.0 - other.y.0) as i64;
        dx * dx + dy * dy
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

/// 2D size with Q16.16 dimensions
///
/// # Invariants
///
/// - Width and height are always non-negative
/// - Zero-area sizes are allowed (useful for empty regions)
///
/// # Examples
///
/// ```
/// use atomic_capsule::gui::Size;
///
/// let s = Size::new(800, 600);
/// assert_eq!(s.width.to_int(), 800);
/// assert_eq!(s.height.to_int(), 600);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Size {
    /// Width (non-negative)
    pub width: Coord,
    /// Height (non-negative)
    pub height: Coord,
}

impl Size {
    /// Zero size (empty)
    pub const ZERO: Self = Self {
        width: Coord::ZERO,
        height: Coord::ZERO,
    };

    /// Create size from integer dimensions
    ///
    /// # Errors
    ///
    /// Returns `InvalidDimensions` if width or height is negative.
    #[inline]
    pub const fn new(width: i32, height: i32) -> GuiResult<Self> {
        if width < 0 || height < 0 {
            return Err(GuiError::InvalidDimensions {
                width: width as u32,
                height: height as u32,
            });
        }
        Ok(Self {
            width: Coord::from_int(width),
            height: Coord::from_int(height),
        })
    }

    /// Create size from floating-point dimensions
    ///
    /// # Errors
    ///
    /// Returns `InvalidDimensions` if width or height is negative.
    #[inline]
    pub fn from_float(width: f32, height: f32) -> GuiResult<Self> {
        if width < 0.0 || height < 0.0 {
            return Err(GuiError::InvalidDimensions {
                width: width as u32,
                height: height as u32,
            });
        }
        Ok(Self {
            width: Coord::from_float(width),
            height: Coord::from_float(height),
        })
    }

    /// Create size unchecked (caller guarantees non-negative)
    ///
    /// # Safety
    ///
    /// Caller must ensure width and height are non-negative.
    #[inline]
    pub const fn new_unchecked(width: Coord, height: Coord) -> Self {
        Self { width, height }
    }

    /// Check if size is zero (empty)
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.width.is_zero() || self.height.is_zero()
    }

    /// Calculate area (saturating multiplication)
    ///
    /// Returns raw Q16.16 value (not squared pixels).
    #[inline]
    pub const fn area(self) -> i64 {
        (self.width.0 as i64).saturating_mul(self.height.0 as i64)
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// 2D rectangle with Q16.16 coordinates
///
/// # Invariants
///
/// - Width and height are always non-negative
/// - Coordinates can be negative (off-screen rectangles)
///
/// # Examples
///
/// ```
/// use atomic_capsule::gui::Rect;
///
/// let r = Rect::new(10, 20, 100, 50).unwrap();
/// assert!(r.contains_point(50, 40));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Rect {
    /// X coordinate of top-left corner
    pub x: Coord,
    /// Y coordinate of top-left corner
    pub y: Coord,
    /// Width (non-negative)
    pub width: Coord,
    /// Height (non-negative)
    pub height: Coord,
}

impl Rect {
    /// Zero rectangle at origin
    pub const ZERO: Self = Self {
        x: Coord::ZERO,
        y: Coord::ZERO,
        width: Coord::ZERO,
        height: Coord::ZERO,
    };

    /// Create rectangle from integer coordinates
    ///
    /// # Errors
    ///
    /// Returns `InvalidRect` if width or height is negative.
    #[inline]
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> GuiResult<Self> {
        if width < 0 || height < 0 {
            return Err(GuiError::InvalidRect {
                x: x as u32,
                y: y as u32,
                width: width as u32,
                height: height as u32,
            });
        }
        Ok(Self {
            x: Coord::from_int(x),
            y: Coord::from_int(y),
            width: Coord::from_int(width),
            height: Coord::from_int(height),
        })
    }

    /// Create rectangle from point and size
    #[inline]
    pub const fn from_point_size(point: Point, size: Size) -> Self {
        Self {
            x: point.x,
            y: point.y,
            width: size.width,
            height: size.height,
        }
    }

    /// Get top-left corner
    #[inline]
    pub const fn origin(self) -> Point {
        Point {
            x: self.x,
            y: self.y,
        }
    }

    /// Get size
    #[inline]
    pub const fn size(self) -> Size {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    /// Get bottom-right corner (exclusive)
    #[inline]
    pub const fn bottom_right(self) -> Point {
        Point {
            x: self.x.saturating_add(self.width),
            y: self.y.saturating_add(self.height),
        }
    }

    /// Check if rectangle is empty
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.width.is_zero() || self.height.is_zero()
    }

    /// Check if point is inside rectangle
    #[inline]
    pub const fn contains_point(self, px: i32, py: i32) -> bool {
        let p = Point::new(px, py);
        p.x.0 >= self.x.0
            && p.x.0 < self.x.saturating_add(self.width).0
            && p.y.0 >= self.y.0
            && p.y.0 < self.y.saturating_add(self.height).0
    }

    /// Check if this rectangle completely contains another
    #[inline]
    pub const fn contains_rect(self, other: Self) -> bool {
        let other_br = other.bottom_right();
        other.x.0 >= self.x.0
            && other.y.0 >= self.y.0
            && other_br.x.0 <= self.x.saturating_add(self.width).0
            && other_br.y.0 <= self.y.saturating_add(self.height).0
    }

    /// Check if this rectangle intersects another
    #[inline]
    pub const fn intersects(self, other: Self) -> bool {
        let self_br = self.bottom_right();
        let other_br = other.bottom_right();
        self.x.0 < other_br.x.0
            && self_br.x.0 > other.x.0
            && self.y.0 < other_br.y.0
            && self_br.y.0 > other.y.0
    }

    /// Calculate intersection rectangle
    ///
    /// Returns `None` if rectangles don't intersect.
    #[inline]
    pub fn intersection(self, other: Self) -> Option<Self> {
        if !self.intersects(other) {
            return None;
        }

        let x1 = if self.x.0 > other.x.0 {
            self.x
        } else {
            other.x
        };
        let y1 = if self.y.0 > other.y.0 {
            self.y
        } else {
            other.y
        };

        let self_br = self.bottom_right();
        let other_br = other.bottom_right();

        let x2 = if self_br.x.0 < other_br.x.0 {
            self_br.x
        } else {
            other_br.x
        };
        let y2 = if self_br.y.0 < other_br.y.0 {
            self_br.y
        } else {
            other_br.y
        };

        Some(Self {
            x: x1,
            y: y1,
            width: x2.saturating_sub(x1),
            height: y2.saturating_sub(y1),
        })
    }

    /// Calculate bounding rectangle (union)
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        let x1 = if self.x.0 < other.x.0 {
            self.x
        } else {
            other.x
        };
        let y1 = if self.y.0 < other.y.0 {
            self.y
        } else {
            other.y
        };

        let self_br = self.bottom_right();
        let other_br = other.bottom_right();

        let x2 = if self_br.x.0 > other_br.x.0 {
            self_br.x
        } else {
            other_br.x
        };
        let y2 = if self_br.y.0 > other_br.y.0 {
            self_br.y
        } else {
            other_br.y
        };

        Self {
            x: x1,
            y: y1,
            width: x2.saturating_sub(x1),
            height: y2.saturating_sub(y1),
        }
    }

    /// Translate rectangle by offset
    #[inline]
    pub const fn translate(self, dx: Coord, dy: Coord) -> Self {
        Self {
            x: self.x.saturating_add(dx),
            y: self.y.saturating_add(dy),
            width: self.width,
            height: self.height,
        }
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}, {}, {}, {}]",
            self.x, self.y, self.width, self.height
        )
    }
}

/// RGBA color with 8-bit components (packed u32)
///
/// # Memory Layout
///
/// ```text
/// | R (8) | G (8) | B (8) | A (8) |
/// | 31-24 | 23-16 | 15-8  | 7-0   |
/// ```
///
/// Little-endian byte order: [A, B, G, R]
///
/// # Examples
///
/// ```
/// use atomic_capsule::gui::Color;
///
/// let red = Color::rgb(255, 0, 0);
/// assert_eq!(red.r(), 255);
/// assert_eq!(red.a(), 255);
///
/// let transparent = Color::rgba(255, 0, 0, 128);
/// assert_eq!(transparent.a(), 128);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Color(u32);

impl Color {
    /// Black color (0, 0, 0, 255)
    pub const BLACK: Self = Self::rgb(0, 0, 0);

    /// White color (255, 255, 255, 255)
    pub const WHITE: Self = Self::rgb(255, 255, 255);

    /// Red color (255, 0, 0, 255)
    pub const RED: Self = Self::rgb(255, 0, 0);

    /// Green color (0, 255, 0, 255)
    pub const GREEN: Self = Self::rgb(0, 255, 0);

    /// Blue color (0, 0, 255, 255)
    pub const BLUE: Self = Self::rgb(0, 0, 255);

    /// Transparent color (0, 0, 0, 0)
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);

    /// Create color from RGB components (alpha = 255)
    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }

    /// Create color from RGBA components
    #[inline]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self(((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32))
    }

    /// Create color from packed u32 (RGBA order)
    #[inline]
    pub const fn from_u32(rgba: u32) -> Self {
        Self(rgba)
    }

    /// Get packed u32 value
    #[inline]
    pub const fn to_u32(self) -> u32 {
        self.0
    }

    /// Get red component
    #[inline]
    pub const fn r(self) -> u8 {
        (self.0 >> 24) as u8
    }

    /// Get green component
    #[inline]
    pub const fn g(self) -> u8 {
        (self.0 >> 16) as u8
    }

    /// Get blue component
    #[inline]
    pub const fn b(self) -> u8 {
        (self.0 >> 8) as u8
    }

    /// Get alpha component
    #[inline]
    pub const fn a(self) -> u8 {
        self.0 as u8
    }

    /// Set red component
    #[inline]
    pub const fn with_r(self, r: u8) -> Self {
        Self((self.0 & 0x00FFFFFF) | ((r as u32) << 24))
    }

    /// Set green component
    #[inline]
    pub const fn with_g(self, g: u8) -> Self {
        Self((self.0 & 0xFF00FFFF) | ((g as u32) << 16))
    }

    /// Set blue component
    #[inline]
    pub const fn with_b(self, b: u8) -> Self {
        Self((self.0 & 0xFFFF00FF) | ((b as u32) << 8))
    }

    /// Set alpha component
    #[inline]
    pub const fn with_a(self, a: u8) -> Self {
        Self((self.0 & 0xFFFFFF00) | (a as u32))
    }

    /// Premultiply alpha
    ///
    /// Converts straight alpha to premultiplied alpha (required for GPU blending).
    #[inline]
    pub fn premultiply(self) -> Self {
        let a = self.a() as u32;
        let r = ((self.r() as u32 * a) / 255) as u8;
        let g = ((self.g() as u32 * a) / 255) as u8;
        let b = ((self.b() as u32 * a) / 255) as u8;
        Self::rgba(r, g, b, a as u8)
    }

    /// Linear interpolation between two colors
    ///
    /// `t` is clamped to [0.0, 1.0].
    #[inline]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let r = (self.r() as f32 * (1.0 - t) + other.r() as f32 * t) as u8;
        let g = (self.g() as f32 * (1.0 - t) + other.g() as f32 * t) as u8;
        let b = (self.b() as f32 * (1.0 - t) + other.b() as f32 * t) as u8;
        let a = (self.a() as f32 * (1.0 - t) + other.a() as f32 * t) as u8;
        Self::rgba(r, g, b, a)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rgba({}, {}, {}, {})",
            self.r(),
            self.g(),
            self.b(),
            self.a()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coord_conversions() {
        let c = Coord::from_int(42);
        assert_eq!(c.to_int(), 42);
        assert!((c.to_float() - 42.0).abs() < 0.0001);

        let c2 = Coord::from_float(42.5);
        assert_eq!(c2.to_int(), 42);
        assert!((c2.to_float() - 42.5).abs() < 0.0001);
    }

    #[test]
    fn test_coord_arithmetic() {
        let c1 = Coord::from_int(10);
        let c2 = Coord::from_int(5);

        assert_eq!(c1.saturating_add(c2).to_int(), 15);
        assert_eq!(c1.saturating_sub(c2).to_int(), 5);
        assert_eq!(c1.saturating_mul(2).to_int(), 20);
    }

    #[test]
    fn test_point_creation() {
        let p = Point::new(100, 200);
        assert_eq!(p.x.to_int(), 100);
        assert_eq!(p.y.to_int(), 200);

        let p2 = Point::from_float(100.5, 200.25);
        assert!((p2.x.to_float() - 100.5).abs() < 0.0001);
        assert!((p2.y.to_float() - 200.25).abs() < 0.0001);
    }

    #[test]
    fn test_size_creation() {
        let s = Size::new(800, 600).unwrap();
        assert_eq!(s.width.to_int(), 800);
        assert_eq!(s.height.to_int(), 600);
        assert!(!s.is_empty());

        let empty = Size::ZERO;
        assert!(empty.is_empty());
    }

    #[test]
    fn test_size_validation() {
        assert!(Size::new(-10, 100).is_err());
        assert!(Size::new(100, -10).is_err());
        assert!(Size::new(0, 0).is_ok());
    }

    #[test]
    fn test_rect_contains_point() {
        let r = Rect::new(10, 20, 100, 50).unwrap();
        assert!(r.contains_point(50, 40));
        assert!(r.contains_point(10, 20));
        assert!(!r.contains_point(110, 70));
        assert!(!r.contains_point(5, 30));
    }

    #[test]
    fn test_rect_intersection() {
        let r1 = Rect::new(0, 0, 100, 100).unwrap();
        let r2 = Rect::new(50, 50, 100, 100).unwrap();

        let inter = r1.intersection(r2).unwrap();
        assert_eq!(inter.x.to_int(), 50);
        assert_eq!(inter.y.to_int(), 50);
        assert_eq!(inter.width.to_int(), 50);
        assert_eq!(inter.height.to_int(), 50);

        let r3 = Rect::new(200, 200, 50, 50).unwrap();
        assert!(r1.intersection(r3).is_none());
    }

    #[test]
    fn test_rect_union() {
        let r1 = Rect::new(0, 0, 50, 50).unwrap();
        let r2 = Rect::new(25, 25, 50, 50).unwrap();

        let u = r1.union(r2);
        assert_eq!(u.x.to_int(), 0);
        assert_eq!(u.y.to_int(), 0);
        assert_eq!(u.width.to_int(), 75);
        assert_eq!(u.height.to_int(), 75);
    }

    #[test]
    fn test_color_components() {
        let c = Color::rgba(255, 128, 64, 32);
        assert_eq!(c.r(), 255);
        assert_eq!(c.g(), 128);
        assert_eq!(c.b(), 64);
        assert_eq!(c.a(), 32);
    }

    #[test]
    fn test_color_constants() {
        assert_eq!(Color::BLACK.r(), 0);
        assert_eq!(Color::WHITE.r(), 255);
        assert_eq!(Color::RED.r(), 255);
        assert_eq!(Color::RED.g(), 0);
        assert_eq!(Color::TRANSPARENT.a(), 0);
    }

    #[test]
    fn test_color_premultiply() {
        let c = Color::rgba(255, 128, 64, 128);
        let pre = c.premultiply();
        // Alpha 128/255 ≈ 0.5, so components should be halved
        assert!(pre.r() >= 126 && pre.r() <= 128);
        assert!(pre.g() >= 63 && pre.g() <= 65);
        assert!(pre.b() >= 31 && pre.b() <= 33);
        assert_eq!(pre.a(), 128);
    }

    #[test]
    fn test_color_lerp() {
        let c1 = Color::BLACK;
        let c2 = Color::WHITE;
        let mid = c1.lerp(c2, 0.5);
        assert!(mid.r() >= 127 && mid.r() <= 128);
        assert!(mid.g() >= 127 && mid.g() <= 128);
        assert!(mid.b() >= 127 && mid.b() <= 128);
    }
}
