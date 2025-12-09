//! GPU Style Types - Local definitions to avoid broken widget dependency
//!
//! T7 Heterogeneous tier types for GPU uniform buffers.
//! Decoupled from widget module to allow independent compilation.

/// RGBA color (std140 compatible)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    #[inline]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[inline]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const TRANSPARENT: Self = Self { r: 0, g: 0, b: 0, a: 0 };
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };
    pub const RED: Self = Self { r: 255, g: 0, b: 0, a: 255 };
    pub const GREEN: Self = Self { r: 0, g: 255, b: 0, a: 255 };
    pub const BLUE: Self = Self { r: 0, g: 0, b: 255, a: 255 };
}

/// Rectangle for widget bounds (std140 compatible)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u8,
    pub height: u8,
}

impl Rect {
    #[inline]
    pub const fn new(x: u16, y: u16, width: u8, height: u8) -> Self {
        Self { x, y, width, height }
    }

    #[inline]
    pub const fn contains(&self, px: u16, py: u16) -> bool {
        px >= self.x && px < self.x + self.width as u16 &&
        py >= self.y && py < self.y + self.height as u16
    }

    #[inline]
    pub const fn area(&self) -> u16 {
        self.width as u16 * self.height as u16
    }

    pub const ZERO: Self = Self { x: 0, y: 0, width: 0, height: 0 };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_new() {
        let c = Color::new(255, 128, 64, 200);
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 128);
        assert_eq!(c.b, 64);
        assert_eq!(c.a, 200);
    }

    #[test]
    fn test_rect_contains() {
        let r = Rect::new(10, 20, 30, 40);
        assert!(r.contains(10, 20));
        assert!(r.contains(39, 59));
        assert!(!r.contains(40, 20));
        assert!(!r.contains(10, 60));
    }

    #[test]
    fn test_rect_area() {
        let r = Rect::new(0, 0, 10, 20);
        assert_eq!(r.area(), 200);
    }
}
