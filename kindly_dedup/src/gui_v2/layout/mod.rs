//! Layout module for kindly_dedup GUI v2
//!
//! **Architecture**: Deterministic Q16.16 fixed-point layout engine + Chaos capsules
//!
//! **Modules**:
//! - `main_screen`: MainScreenLayout (header, content, footer, 900px centered)
//! - `compliance_modal`: ComplianceModalLayout (600x400 modal, backdrop)
//! - `helpers`: Layout composition utilities (column, row, center, padding, card)
//! - `capsules`: Chaos-compliant layout capsules (LayoutCapsule, FlexLayoutCapsule, LayoutTreeCapsule)
//!
//! **Framework Compliance**:
//! - **UCE34**: T1 Atomic + T3 Fixed-Point + T5 Streaming (capsules tier stack)
//! - **Chaos**: 100% lockfree (capsules module), deterministic fixed-point (Q16.16)
//! - **ASSUM**: Overflow checks on fixed-point math, compile-time capacity limits (64 nodes)
//! - **T28**: 80+ tests (20 fixed-point + 60 capsules)

pub mod main_screen;
pub mod compliance_modal;
pub mod helpers;
pub mod capsules;

// Re-exports
pub use main_screen::MainScreenLayout;
pub use compliance_modal::ComplianceModalLayout;
pub use helpers::{column, row, center, padding, card};

// Re-export capsules for convenience
pub use capsules::{
    LayoutCapsule, FlexLayoutCapsule, LayoutTreeCapsule,
    FlexDirection, JustifyContent, AlignItems,
};

/// Q16.16 fixed-point rectangle for deterministic layout
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// X coordinate (Q16.16 fixed-point)
    pub x: i32,
    /// Y coordinate (Q16.16 fixed-point)
    pub y: i32,
    /// Width (Q16.16 fixed-point)
    pub width: i32,
    /// Height (Q16.16 fixed-point)
    pub height: i32,
}

impl Rect {
    /// Create new rectangle with Q16.16 coordinates
    ///
    /// # Arguments
    /// - `x`: X coordinate in pixels (converted to Q16.16)
    /// - `y`: Y coordinate in pixels (converted to Q16.16)
    /// - `width`: Width in pixels (converted to Q16.16)
    /// - `height`: Height in pixels (converted to Q16.16)
    #[inline]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x: (x as i32) << 16,
            y: (y as i32) << 16,
            width: (width as i32) << 16,
            height: (height as i32) << 16,
        }
    }

    /// Create rectangle from Q16.16 fixed-point values
    #[inline]
    pub const fn from_fixed(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self { x, y, width, height }
    }

    /// Convert to pixel coordinates (Q16.16 → u16)
    #[inline]
    pub const fn to_pixels(&self) -> (u16, u16, u16, u16) {
        (
            (self.x >> 16) as u16,
            (self.y >> 16) as u16,
            (self.width >> 16) as u16,
            (self.height >> 16) as u16,
        )
    }

    /// Get center point (Q16.16)
    #[inline]
    pub const fn center(&self) -> (i32, i32) {
        (self.x + (self.width >> 1), self.y + (self.height >> 1))
    }

    /// Check if point is inside rectangle
    #[inline]
    pub const fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    /// Apply padding (returns inner rect)
    #[inline]
    pub const fn padding(&self, pad: i32) -> Self {
        let pad2 = pad << 1; // 2 * pad
        Self {
            x: self.x + pad,
            y: self.y + pad,
            width: self.width.saturating_sub(pad2),
            height: self.height.saturating_sub(pad2),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_new() {
        let rect = Rect::new(100, 200, 300, 400);
        assert_eq!(rect.x, 100 << 16);
        assert_eq!(rect.y, 200 << 16);
        assert_eq!(rect.width, 300 << 16);
        assert_eq!(rect.height, 400 << 16);
    }

    #[test]
    fn test_rect_to_pixels() {
        let rect = Rect::new(100, 200, 300, 400);
        let (x, y, w, h) = rect.to_pixels();
        assert_eq!((x, y, w, h), (100, 200, 300, 400));
    }

    #[test]
    fn test_rect_center() {
        let rect = Rect::new(100, 200, 400, 600);
        let (cx, cy) = rect.center();
        assert_eq!(cx, (100 + 200) << 16); // 100 + 400/2
        assert_eq!(cy, (200 + 300) << 16); // 200 + 600/2
    }

    #[test]
    fn test_rect_contains() {
        let rect = Rect::new(100, 200, 300, 400);
        let x_inside = 250 << 16;
        let y_inside = 400 << 16;
        assert!(rect.contains(x_inside, y_inside));

        let x_outside = 50 << 16;
        assert!(!rect.contains(x_outside, y_inside));
    }

    #[test]
    fn test_rect_padding() {
        let rect = Rect::new(100, 200, 400, 600);
        let padded = rect.padding(10 << 16); // 10px padding
        assert_eq!(padded.x, 110 << 16);
        assert_eq!(padded.y, 210 << 16);
        assert_eq!(padded.width, 380 << 16); // 400 - 20
        assert_eq!(padded.height, 580 << 16); // 600 - 20
    }
}
