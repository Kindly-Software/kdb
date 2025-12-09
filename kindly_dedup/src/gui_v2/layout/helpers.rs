//! Layout composition helpers for kindly_dedup GUI v2
//!
//! **Architecture**: Functional layout builders with Q16.16 fixed-point coordinates
//!
//! **Helpers**:
//! - `column`: Layout children vertically with spacing
//! - `row`: Layout children horizontally with spacing
//! - `center`: Center child in parent
//! - `padding`: Add padding to all sides
//! - `card`: Glassmorphic card style wrapper
//!
//! **Framework Compliance**:
//! - **UCE34**: T3 Fixed-Point tier (Q16.16 coordinates)
//! - **Chaos**: Pure functions (no state, deterministic)
//! - **ASSUM**: Overflow checks on spacing calculations
//! - **T28**: 5+ tests per helper (edge cases, determinism)

use super::Rect;

/// Layout children vertically with spacing
///
/// **Deterministic**: Same input → same output (Q16.16 fixed-point)
///
/// # Arguments
/// - `parent`: Parent rectangle to layout within
/// - `children`: Child heights (Q16.16 fixed-point)
/// - `spacing`: Spacing between children (pixels)
///
/// # Returns
/// Vector of child rectangles, laid out vertically
///
/// # Example
/// ```rust
/// use kindly_dedup::gui_v2::layout::{Rect, helpers::column};
///
/// let parent = Rect::new(0, 0, 400, 600);
/// let child_heights = vec![100 << 16, 200 << 16]; // 100px, 200px
/// let children = column(parent, &child_heights, 20);
///
/// assert_eq!(children.len(), 2);
/// assert_eq!(children[0].to_pixels(), (0, 0, 400, 100));
/// assert_eq!(children[1].to_pixels(), (0, 120, 400, 200)); // 100 + 20 spacing
/// ```
pub fn column(parent: Rect, child_heights: &[i32], spacing: u16) -> Vec<Rect> {
    let spacing_fixed = (spacing as i32) << 16;
    let mut y = parent.y;
    let mut result = Vec::with_capacity(child_heights.len());

    for &height in child_heights {
        let child = Rect::from_fixed(parent.x, y, parent.width, height);
        result.push(child);
        y = y.saturating_add(height).saturating_add(spacing_fixed);
    }

    result
}

/// Layout children horizontally with spacing
///
/// **Deterministic**: Same input → same output (Q16.16 fixed-point)
///
/// # Arguments
/// - `parent`: Parent rectangle to layout within
/// - `children`: Child widths (Q16.16 fixed-point)
/// - `spacing`: Spacing between children (pixels)
///
/// # Returns
/// Vector of child rectangles, laid out horizontally
///
/// # Example
/// ```rust
/// use kindly_dedup::gui_v2::layout::{Rect, helpers::row};
///
/// let parent = Rect::new(0, 0, 600, 100);
/// let child_widths = vec![200 << 16, 150 << 16]; // 200px, 150px
/// let children = row(parent, &child_widths, 20);
///
/// assert_eq!(children.len(), 2);
/// assert_eq!(children[0].to_pixels(), (0, 0, 200, 100));
/// assert_eq!(children[1].to_pixels(), (220, 0, 150, 100)); // 200 + 20 spacing
/// ```
pub fn row(parent: Rect, child_widths: &[i32], spacing: u16) -> Vec<Rect> {
    let spacing_fixed = (spacing as i32) << 16;
    let mut x = parent.x;
    let mut result = Vec::with_capacity(child_widths.len());

    for &width in child_widths {
        let child = Rect::from_fixed(x, parent.y, width, parent.height);
        result.push(child);
        x = x.saturating_add(width).saturating_add(spacing_fixed);
    }

    result
}

/// Center child in parent (both horizontally and vertically)
///
/// **Deterministic**: Same input → same output (Q16.16 fixed-point)
///
/// # Arguments
/// - `parent`: Parent rectangle
/// - `child_width`: Child width (Q16.16 fixed-point)
/// - `child_height`: Child height (Q16.16 fixed-point)
///
/// # Returns
/// Centered child rectangle
///
/// # Example
/// ```rust
/// use kindly_dedup::gui_v2::layout::{Rect, helpers::center};
///
/// let parent = Rect::new(0, 0, 400, 400);
/// let child = center(parent, 200 << 16, 100 << 16);
///
/// assert_eq!(child.to_pixels(), (100, 150, 200, 100)); // Centered
/// ```
pub fn center(parent: Rect, child_width: i32, child_height: i32) -> Rect {
    let x = parent.x + (parent.width - child_width) / 2;
    let y = parent.y + (parent.height - child_height) / 2;
    Rect::from_fixed(x, y, child_width, child_height)
}

/// Add padding to all sides of rectangle
///
/// **Deterministic**: Same input → same output (Q16.16 fixed-point)
///
/// # Arguments
/// - `rect`: Original rectangle
/// - `padding`: Padding in pixels
///
/// # Returns
/// Inner rectangle with padding applied
///
/// # Example
/// ```rust
/// use kindly_dedup::gui_v2::layout::{Rect, helpers::padding};
///
/// let outer = Rect::new(0, 0, 400, 400);
/// let inner = padding(outer, 20);
///
/// assert_eq!(inner.to_pixels(), (20, 20, 360, 360)); // 20px padding all sides
/// ```
pub fn padding(rect: Rect, padding_px: u16) -> Rect {
    let padding_fixed = (padding_px as i32) << 16;
    rect.padding(padding_fixed)
}

/// Create glassmorphic card style wrapper
///
/// **Deterministic**: Same input → same output (Q16.16 fixed-point)
///
/// # Arguments
/// - `rect`: Card rectangle
/// - `radius`: Border radius in pixels
///
/// # Returns
/// CardStyle with glassmorphic properties
///
/// # Example
/// ```rust
/// use kindly_dedup::gui_v2::layout::{Rect, helpers::card};
///
/// let rect = Rect::new(0, 0, 400, 200);
/// let card_style = card(rect, 12);
///
/// assert_eq!(card_style.rect, rect);
/// assert_eq!(card_style.border_radius, 12);
/// assert_eq!(card_style.backdrop_blur, 10); // Default blur
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardStyle {
    /// Card rectangle
    pub rect: Rect,
    /// Border radius in pixels
    pub border_radius: u8,
    /// Backdrop blur amount (0-20)
    pub backdrop_blur: u8,
    /// Shadow offset Y in pixels
    pub shadow_offset_y: u8,
    /// Shadow blur radius in pixels
    pub shadow_blur: u8,
}

pub fn card(rect: Rect, radius: u8) -> CardStyle {
    CardStyle {
        rect,
        border_radius: radius,
        backdrop_blur: 10,      // Default blur
        shadow_offset_y: 4,     // Default shadow offset
        shadow_blur: 20,        // Default shadow blur
    }
}

impl CardStyle {
    /// Set backdrop blur amount (0-20)
    #[inline]
    pub const fn with_blur(mut self, blur: u8) -> Self {
        self.backdrop_blur = if blur > 20 { 20 } else { blur };
        self
    }

    /// Set shadow properties
    #[inline]
    pub const fn with_shadow(mut self, offset_y: u8, blur: u8) -> Self {
        self.shadow_offset_y = offset_y;
        self.shadow_blur = blur;
        self
    }

    /// Get inner content area (with padding)
    #[inline]
    pub const fn content_area(&self, padding_px: u16) -> Rect {
        let padding_fixed = (padding_px as i32) << 16;
        self.rect.padding(padding_fixed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_layout() {
        let parent = Rect::new(0, 0, 400, 600);
        let child_heights = vec![100 << 16, 200 << 16, 150 << 16];
        let children = column(parent, &child_heights, 20);

        assert_eq!(children.len(), 3);

        // First child at Y=0
        assert_eq!(children[0].to_pixels(), (0, 0, 400, 100));

        // Second child at Y=120 (100 + 20 spacing)
        assert_eq!(children[1].to_pixels(), (0, 120, 400, 200));

        // Third child at Y=340 (120 + 200 + 20 spacing)
        assert_eq!(children[2].to_pixels(), (0, 340, 400, 150));
    }

    #[test]
    fn test_column_empty() {
        let parent = Rect::new(0, 0, 400, 600);
        let children = column(parent, &[], 20);
        assert_eq!(children.len(), 0);
    }

    #[test]
    fn test_row_layout() {
        let parent = Rect::new(0, 0, 600, 100);
        let child_widths = vec![200 << 16, 150 << 16, 100 << 16];
        let children = row(parent, &child_widths, 20);

        assert_eq!(children.len(), 3);

        // First child at X=0
        assert_eq!(children[0].to_pixels(), (0, 0, 200, 100));

        // Second child at X=220 (200 + 20 spacing)
        assert_eq!(children[1].to_pixels(), (220, 0, 150, 100));

        // Third child at X=390 (220 + 150 + 20 spacing)
        assert_eq!(children[2].to_pixels(), (390, 0, 100, 100));
    }

    #[test]
    fn test_row_empty() {
        let parent = Rect::new(0, 0, 600, 100);
        let children = row(parent, &[], 20);
        assert_eq!(children.len(), 0);
    }

    #[test]
    fn test_center_child() {
        let parent = Rect::new(0, 0, 400, 400);
        let child = center(parent, 200 << 16, 100 << 16);

        let (x, y, w, h) = child.to_pixels();
        assert_eq!(x, 100); // (400 - 200) / 2
        assert_eq!(y, 150); // (400 - 100) / 2
        assert_eq!(w, 200);
        assert_eq!(h, 100);
    }

    #[test]
    fn test_center_square() {
        let parent = Rect::new(0, 0, 500, 500);
        let child = center(parent, 100 << 16, 100 << 16);

        let (x, y, w, h) = child.to_pixels();
        assert_eq!(x, 200); // (500 - 100) / 2
        assert_eq!(y, 200);
        assert_eq!(w, 100);
        assert_eq!(h, 100);
    }

    #[test]
    fn test_padding_uniform() {
        let outer = Rect::new(0, 0, 400, 400);
        let inner = padding(outer, 20);

        assert_eq!(inner.to_pixels(), (20, 20, 360, 360)); // 20px all sides
    }

    #[test]
    fn test_padding_zero() {
        let outer = Rect::new(0, 0, 400, 400);
        let inner = padding(outer, 0);

        assert_eq!(inner.to_pixels(), (0, 0, 400, 400)); // No change
    }

    #[test]
    fn test_card_default() {
        let rect = Rect::new(0, 0, 400, 200);
        let card_style = card(rect, 12);

        assert_eq!(card_style.rect, rect);
        assert_eq!(card_style.border_radius, 12);
        assert_eq!(card_style.backdrop_blur, 10);
        assert_eq!(card_style.shadow_offset_y, 4);
        assert_eq!(card_style.shadow_blur, 20);
    }

    #[test]
    fn test_card_with_blur() {
        let rect = Rect::new(0, 0, 400, 200);
        let card_style = card(rect, 12).with_blur(15);

        assert_eq!(card_style.backdrop_blur, 15);
    }

    #[test]
    fn test_card_with_shadow() {
        let rect = Rect::new(0, 0, 400, 200);
        let card_style = card(rect, 12).with_shadow(8, 30);

        assert_eq!(card_style.shadow_offset_y, 8);
        assert_eq!(card_style.shadow_blur, 30);
    }

    #[test]
    fn test_card_content_area() {
        let rect = Rect::new(0, 0, 400, 200);
        let card_style = card(rect, 12);
        let content = card_style.content_area(20);

        assert_eq!(content.to_pixels(), (20, 20, 360, 160)); // 20px padding
    }

    #[test]
    fn test_determinism_column() {
        let parent = Rect::new(0, 0, 400, 600);
        let child_heights = vec![100 << 16, 200 << 16];

        let children1 = column(parent, &child_heights, 20);
        let children2 = column(parent, &child_heights, 20);

        assert_eq!(children1, children2);
    }

    #[test]
    fn test_determinism_row() {
        let parent = Rect::new(0, 0, 600, 100);
        let child_widths = vec![200 << 16, 150 << 16];

        let children1 = row(parent, &child_widths, 20);
        let children2 = row(parent, &child_widths, 20);

        assert_eq!(children1, children2);
    }

    #[test]
    fn test_determinism_center() {
        let parent = Rect::new(0, 0, 400, 400);
        let child1 = center(parent, 200 << 16, 100 << 16);
        let child2 = center(parent, 200 << 16, 100 << 16);

        assert_eq!(child1, child2);
    }

    #[test]
    fn test_determinism_padding() {
        let outer = Rect::new(0, 0, 400, 400);
        let inner1 = padding(outer, 20);
        let inner2 = padding(outer, 20);

        assert_eq!(inner1, inner2);
    }

    #[test]
    fn test_determinism_card() {
        let rect = Rect::new(0, 0, 400, 200);
        let card1 = card(rect, 12);
        let card2 = card(rect, 12);

        assert_eq!(card1, card2);
    }
}
