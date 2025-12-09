//! Spacer and Divider Capsules - T0 Auditable Layout Primitives
//!
//! # Overview
//!
//! **SpacerCapsule**: Flex spacer for dynamic layout. Takes available space in flex
//! containers with configurable grow/shrink factors. Zero runtime cost.
//!
//! **DividerCapsule**: Visual separator with multiple styles (solid, dashed, dotted,
//! double, thick). Supports horizontal/vertical orientation and custom styling.
//!
//! # Tier Classification
//!
//! - **Tier**: T0 Auditable (compile-time/const, minimal runtime state)
//! - **Size**: 64B each (cache-line aligned)
//! - **Speedup**: 0ns (pure layout primitives, no computation)
//! - **Framework**: UCE34 Q10 (T0), Chaos compliant (64B alignment)
//!
//! # Examples
//!
//! ```rust
//! use atomic_capsule::terminal::widget::foundation::{SpacerCapsule, DividerCapsule, DividerStyle};
//!
//! // Flex spacer (takes available space)
//! let spacer = SpacerCapsule::flex(1);
//!
//! // Fixed size spacer (5 cells)
//! let fixed = SpacerCapsule::fixed(5);
//!
//! // Horizontal divider with custom style
//! let divider = DividerCapsule::horizontal()
//!     .with_style(DividerStyle::Double)
//!     .with_color(0x808080FF)
//!     .with_margin(1, 1);
//! ```

use crate::terminal::widget::rect::Rect;
use crate::terminal::widget::render_command::{RenderCommand, RenderCommandBuffer};
use crate::terminal::widget::Widget;

/// Divider style variants
///
/// Unicode line drawing characters for different visual styles.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum DividerStyle {
    /// Solid line: ─ (U+2500) or │ (U+2502)
    #[default]
    Solid = 0,
    /// Dashed line: ┄ (U+2504) or ┆ (U+2506)
    Dashed = 1,
    /// Dotted line: ┈ (U+2508) or ┊ (U+250A)
    Dotted = 2,
    /// Double line: ═ (U+2550) or ║ (U+2551)
    Double = 3,
    /// Thick line: ━ (U+2501) or ┃ (U+2503)
    Thick = 4,
}

impl DividerStyle {
    /// Get horizontal character for style
    #[inline]
    pub const fn horizontal_char(self) -> char {
        match self {
            DividerStyle::Solid => '─',  // U+2500
            DividerStyle::Dashed => '┄', // U+2504
            DividerStyle::Dotted => '┈', // U+2508
            DividerStyle::Double => '═', // U+2550
            DividerStyle::Thick => '━',  // U+2501
        }
    }

    /// Get vertical character for style
    #[inline]
    pub const fn vertical_char(self) -> char {
        match self {
            DividerStyle::Solid => '│',  // U+2502
            DividerStyle::Dashed => '┆', // U+2506
            DividerStyle::Dotted => '┊', // U+250A
            DividerStyle::Double => '║', // U+2551
            DividerStyle::Thick => '┃',  // U+2503
        }
    }
}

/// T0 Auditable - Flex spacer for layout
///
/// Takes up available space in flex containers with configurable grow/shrink factors.
/// Zero runtime cost, pure layout primitive.
///
/// # Layout Behavior
///
/// - **flex_grow > 0**: Grows proportionally to fill available space
/// - **flex_grow = 0**: Fixed size (uses min_size)
/// - **min_size/max_size**: Constrains final size
///
/// # Memory Layout
///
/// ```text
/// Offset | Field           | Size | Align
/// -------|-----------------|------|------
/// 0      | flex_grow       | 2    | 2
/// 2      | flex_shrink     | 2    | 2
/// 4      | min_size        | 2    | 2
/// 6      | max_size        | 2    | 2
/// 8      | axis            | 1    | 1
/// 9      | _pad            | 55   | 1
/// -------|-----------------|------|------
/// Total: 64 bytes, align 64
/// ```
#[repr(C, align(64))]
#[derive(Copy, Clone, Debug)]
pub struct SpacerCapsule {
    /// Flex grow factor (0 = fixed, >0 = grow proportionally)
    pub flex_grow: u16,
    /// Flex shrink factor (0 = no shrink, >0 = shrink proportionally)
    pub flex_shrink: u16,
    /// Minimum size (cells)
    pub min_size: u16,
    /// Maximum size (0 = unlimited)
    pub max_size: u16,
    /// Axis: Horizontal(0), Vertical(1)
    pub axis: u8,

    _pad: [u8; 55],
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<SpacerCapsule>() == 64);
const _: () = assert!(core::mem::align_of::<SpacerCapsule>() == 64);

impl Default for SpacerCapsule {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl SpacerCapsule {
    /// Create default flex spacer (flex=1, horizontal)
    #[inline]
    pub const fn new() -> Self {
        Self {
            flex_grow: 1,
            flex_shrink: 1,
            min_size: 0,
            max_size: 0, // unlimited
            axis: 0,     // horizontal
            _pad: [0u8; 55],
        }
    }

    /// Create fixed size spacer (no flex)
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::terminal::widget::foundation::SpacerCapsule;
    ///
    /// let spacer = SpacerCapsule::fixed(10); // Always 10 cells
    /// assert_eq!(spacer.measure(100), 10);
    /// ```
    #[inline]
    pub const fn fixed(size: u16) -> Self {
        Self {
            flex_grow: 0,
            flex_shrink: 0,
            min_size: size,
            max_size: size,
            axis: 0,
            _pad: [0u8; 55],
        }
    }

    /// Create flex spacer with grow factor
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::terminal::widget::foundation::SpacerCapsule;
    ///
    /// let spacer = SpacerCapsule::flex(2); // Grows 2× relative to flex=1
    /// ```
    #[inline]
    pub const fn flex(grow: u16) -> Self {
        Self {
            flex_grow: grow,
            flex_shrink: 1,
            min_size: 0,
            max_size: 0,
            axis: 0,
            _pad: [0u8; 55],
        }
    }

    /// Create horizontal spacer
    #[inline]
    pub const fn horizontal() -> Self {
        let mut spacer = Self::new();
        spacer.axis = 0;
        spacer
    }

    /// Create vertical spacer
    #[inline]
    pub const fn vertical() -> Self {
        let mut spacer = Self::new();
        spacer.axis = 1;
        spacer
    }

    /// Set minimum size constraint
    #[inline]
    pub const fn with_min_size(mut self, min: u16) -> Self {
        self.min_size = min;
        self
    }

    /// Set maximum size constraint
    #[inline]
    pub const fn with_max_size(mut self, max: u16) -> Self {
        self.max_size = max;
        self
    }

    /// Set flex shrink factor
    #[inline]
    pub const fn with_shrink(mut self, shrink: u16) -> Self {
        self.flex_shrink = shrink;
        self
    }

    /// Calculate size given available space
    ///
    /// # Algorithm
    ///
    /// 1. If flex_grow = 0, return min_size (fixed)
    /// 2. If flex_grow > 0, return available space
    /// 3. Apply min/max constraints
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::terminal::widget::foundation::SpacerCapsule;
    ///
    /// let fixed = SpacerCapsule::fixed(5);
    /// assert_eq!(fixed.measure(100), 5);
    ///
    /// let flex = SpacerCapsule::flex(1);
    /// assert_eq!(flex.measure(100), 100);
    ///
    /// let constrained = SpacerCapsule::flex(1).with_max_size(50);
    /// assert_eq!(constrained.measure(100), 50);
    /// ```
    #[inline]
    pub const fn measure(&self, available: u16) -> u16 {
        // Fixed size (flex_grow = 0)
        if self.flex_grow == 0 {
            return self.min_size;
        }

        // Flex size (use available space)
        let mut size = available;

        // Apply min constraint
        if size < self.min_size {
            size = self.min_size;
        }

        // Apply max constraint (0 = unlimited)
        if self.max_size > 0 && size > self.max_size {
            size = self.max_size;
        }

        size
    }

    /// Get flex grow factor
    #[inline]
    pub const fn flex_grow(&self) -> u16 {
        self.flex_grow
    }

    /// Check if spacer is horizontal
    #[inline]
    pub const fn is_horizontal(&self) -> bool {
        self.axis == 0
    }

    /// Check if spacer is vertical
    #[inline]
    pub const fn is_vertical(&self) -> bool {
        self.axis == 1
    }
}

impl Widget for SpacerCapsule {
    #[inline]
    fn render(&self, _area: Rect, _cmd: &mut RenderCommandBuffer) {
        // Spacer is invisible, no rendering needed
    }

    #[inline]
    fn focusable(&self) -> bool {
        false // Spacers are not interactive
    }
}

/// T0 Auditable - Visual separator
///
/// Renders a horizontal or vertical divider line with configurable style,
/// color, and margins.
///
/// # Styles
///
/// - **Solid**: ─ or │ (default)
/// - **Dashed**: ┄ or ┆
/// - **Dotted**: ┈ or ┊
/// - **Double**: ═ or ║
/// - **Thick**: ━ or ┃
///
/// # Memory Layout
///
/// ```text
/// Offset | Field           | Size | Align
/// -------|-----------------|------|------
/// 0      | style           | 1    | 1
/// 1      | orientation     | 1    | 1
/// 2      | thickness       | 1    | 1
/// 3      | margin_before   | 1    | 1
/// 4      | margin_after    | 1    | 1
/// 5      | _pad1           | 3    | 1
/// 8      | color           | 4    | 4
/// 12     | _pad2           | 52   | 1
/// -------|-----------------|------|------
/// Total: 64 bytes, align 64
/// ```
#[repr(C, align(64))]
#[derive(Copy, Clone, Debug)]
pub struct DividerCapsule {
    /// Divider style
    pub style: DividerStyle,
    /// Orientation: Horizontal(0), Vertical(1)
    pub orientation: u8,
    /// Thickness (cells, typically 1)
    pub thickness: u8,
    /// Margin before (cells)
    pub margin_before: u8,
    /// Margin after (cells)
    pub margin_after: u8,

    _pad1: [u8; 3],

    /// Color (RGBA8888)
    pub color: u32,

    _pad2: [u8; 52],
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<DividerCapsule>() == 64);
const _: () = assert!(core::mem::align_of::<DividerCapsule>() == 64);

impl Default for DividerCapsule {
    #[inline]
    fn default() -> Self {
        Self::horizontal()
    }
}

impl DividerCapsule {
    /// Create horizontal divider (default solid, white)
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::terminal::widget::foundation::DividerCapsule;
    ///
    /// let divider = DividerCapsule::horizontal();
    /// assert!(divider.is_horizontal());
    /// ```
    #[inline]
    pub const fn horizontal() -> Self {
        Self {
            style: DividerStyle::Solid,
            orientation: 0, // horizontal
            thickness: 1,
            margin_before: 0,
            margin_after: 0,
            _pad1: [0u8; 3],
            color: 0xFFFFFFFF, // white
            _pad2: [0u8; 52],
        }
    }

    /// Create vertical divider (default solid, white)
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::terminal::widget::foundation::DividerCapsule;
    ///
    /// let divider = DividerCapsule::vertical();
    /// assert!(divider.is_vertical());
    /// ```
    #[inline]
    pub const fn vertical() -> Self {
        Self {
            style: DividerStyle::Solid,
            orientation: 1, // vertical
            thickness: 1,
            margin_before: 0,
            margin_after: 0,
            _pad1: [0u8; 3],
            color: 0xFFFFFFFF,
            _pad2: [0u8; 52],
        }
    }

    /// Set divider style
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::terminal::widget::foundation::{DividerCapsule, DividerStyle};
    ///
    /// let divider = DividerCapsule::horizontal()
    ///     .with_style(DividerStyle::Double);
    /// ```
    #[inline]
    pub const fn with_style(mut self, style: DividerStyle) -> Self {
        self.style = style;
        self
    }

    /// Set divider color (RGBA8888)
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::terminal::widget::foundation::DividerCapsule;
    ///
    /// let divider = DividerCapsule::horizontal()
    ///     .with_color(0x808080FF); // gray
    /// ```
    #[inline]
    pub const fn with_color(mut self, rgba: u32) -> Self {
        self.color = rgba;
        self
    }

    /// Set margins before and after divider
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::terminal::widget::foundation::DividerCapsule;
    ///
    /// let divider = DividerCapsule::horizontal()
    ///     .with_margin(1, 1); // 1 cell margin on each side
    /// ```
    #[inline]
    pub const fn with_margin(mut self, before: u8, after: u8) -> Self {
        self.margin_before = before;
        self.margin_after = after;
        self
    }

    /// Set thickness (cells)
    #[inline]
    pub const fn with_thickness(mut self, thickness: u8) -> Self {
        self.thickness = thickness;
        self
    }

    /// Check if divider is horizontal
    #[inline]
    pub const fn is_horizontal(&self) -> bool {
        self.orientation == 0
    }

    /// Check if divider is vertical
    #[inline]
    pub const fn is_vertical(&self) -> bool {
        self.orientation == 1
    }

    /// Get divider character based on orientation and style
    #[inline]
    pub const fn char(&self) -> char {
        if self.orientation == 0 {
            self.style.horizontal_char()
        } else {
            self.style.vertical_char()
        }
    }

    /// Render divider to command buffer
    ///
    /// # Algorithm
    ///
    /// 1. Apply margins to area
    /// 2. Get divider character based on orientation/style
    /// 3. Fill area with character using color
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::terminal::widget::foundation::DividerCapsule;
    /// use atomic_capsule::terminal::widget::rect::Rect;
    /// use atomic_capsule::terminal::widget::render_command::RenderCommandBuffer;
    ///
    /// let divider = DividerCapsule::horizontal();
    /// let area = Rect::new(0, 0, 80, 1);
    /// let mut cmd = RenderCommandBuffer::new();
    ///
    /// divider.render(area, &mut cmd);
    /// ```
    pub fn render(&self, area: Rect, cmd: &mut RenderCommandBuffer) {
        // Apply margins
        let content_area = if self.orientation == 0 {
            // Horizontal: margins affect vertical space
            if area.height < self.margin_before as u16 + self.margin_after as u16 {
                return; // Not enough space
            }
            Rect::new(
                area.x,
                area.y + self.margin_before as u16,
                area.width,
                area.height - self.margin_before as u16 - self.margin_after as u16,
            )
        } else {
            // Vertical: margins affect horizontal space
            if area.width < self.margin_before as u16 + self.margin_after as u16 {
                return; // Not enough space
            }
            Rect::new(
                area.x + self.margin_before as u16,
                area.y,
                area.width - self.margin_before as u16 - self.margin_after as u16,
                area.height,
            )
        };

        // Get divider character
        let ch = self.char();

        // Fill area with divider character
        if self.orientation == 0 {
            // Horizontal divider
            for row in 0..self.thickness.min(content_area.height as u8) {
                for col in 0..content_area.width {
                    cmd.push(RenderCommand::DrawChar {
                        x: content_area.x + col,
                        y: content_area.y + row as u16,
                        ch,
                        fg: self.color,
                        bg: 0x00000000, // transparent background
                    });
                }
            }
        } else {
            // Vertical divider
            for col in 0..self.thickness.min(content_area.width as u8) {
                for row in 0..content_area.height {
                    cmd.push(RenderCommand::DrawChar {
                        x: content_area.x + col as u16,
                        y: content_area.y + row,
                        ch,
                        fg: self.color,
                        bg: 0x00000000,
                    });
                }
            }
        }
    }
}

impl Widget for DividerCapsule {
    #[inline]
    fn render(&self, area: Rect, cmd: &mut RenderCommandBuffer) {
        DividerCapsule::render(self, area, cmd);
    }

    #[inline]
    fn focusable(&self) -> bool {
        false // Dividers are not interactive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // T28 Q1-Q7: Unit Tests - SpacerCapsule
    // ============================================================================

    #[test]
    fn test_spacer_size_and_alignment() {
        assert_eq!(core::mem::size_of::<SpacerCapsule>(), 64);
        assert_eq!(core::mem::align_of::<SpacerCapsule>(), 64);
    }

    #[test]
    fn test_spacer_default() {
        let spacer = SpacerCapsule::new();
        assert_eq!(spacer.flex_grow, 1);
        assert_eq!(spacer.flex_shrink, 1);
        assert_eq!(spacer.min_size, 0);
        assert_eq!(spacer.max_size, 0);
        assert_eq!(spacer.axis, 0);
    }

    #[test]
    fn test_spacer_fixed() {
        let spacer = SpacerCapsule::fixed(10);
        assert_eq!(spacer.flex_grow, 0);
        assert_eq!(spacer.min_size, 10);
        assert_eq!(spacer.max_size, 10);
        assert_eq!(spacer.measure(100), 10);
        assert_eq!(spacer.measure(5), 10); // Always returns fixed size
    }

    #[test]
    fn test_spacer_flex() {
        let spacer = SpacerCapsule::flex(2);
        assert_eq!(spacer.flex_grow, 2);
        assert_eq!(spacer.measure(100), 100);
        assert_eq!(spacer.measure(50), 50);
    }

    #[test]
    fn test_spacer_horizontal_vertical() {
        let h = SpacerCapsule::horizontal();
        assert!(h.is_horizontal());
        assert!(!h.is_vertical());

        let v = SpacerCapsule::vertical();
        assert!(!v.is_horizontal());
        assert!(v.is_vertical());
    }

    #[test]
    fn test_spacer_constraints() {
        let spacer = SpacerCapsule::flex(1)
            .with_min_size(10)
            .with_max_size(50);

        assert_eq!(spacer.measure(5), 10);   // min constraint
        assert_eq!(spacer.measure(30), 30);  // within range
        assert_eq!(spacer.measure(100), 50); // max constraint
    }

    // ============================================================================
    // T28 Q1-Q7: Unit Tests - DividerCapsule
    // ============================================================================

    #[test]
    fn test_divider_size_and_alignment() {
        assert_eq!(core::mem::size_of::<DividerCapsule>(), 64);
        assert_eq!(core::mem::align_of::<DividerCapsule>(), 64);
    }

    #[test]
    fn test_divider_horizontal_vertical() {
        let h = DividerCapsule::horizontal();
        assert!(h.is_horizontal());
        assert!(!h.is_vertical());
        assert_eq!(h.orientation, 0);

        let v = DividerCapsule::vertical();
        assert!(!v.is_horizontal());
        assert!(v.is_vertical());
        assert_eq!(v.orientation, 1);
    }

    #[test]
    fn test_divider_styles() {
        let divider = DividerCapsule::horizontal();

        // Test all styles
        let solid = divider.with_style(DividerStyle::Solid);
        assert_eq!(solid.char(), '─');

        let dashed = divider.with_style(DividerStyle::Dashed);
        assert_eq!(dashed.char(), '┄');

        let dotted = divider.with_style(DividerStyle::Dotted);
        assert_eq!(dotted.char(), '┈');

        let double = divider.with_style(DividerStyle::Double);
        assert_eq!(double.char(), '═');

        let thick = divider.with_style(DividerStyle::Thick);
        assert_eq!(thick.char(), '━');
    }

    #[test]
    fn test_divider_vertical_styles() {
        let divider = DividerCapsule::vertical();

        assert_eq!(divider.with_style(DividerStyle::Solid).char(), '│');
        assert_eq!(divider.with_style(DividerStyle::Dashed).char(), '┆');
        assert_eq!(divider.with_style(DividerStyle::Dotted).char(), '┊');
        assert_eq!(divider.with_style(DividerStyle::Double).char(), '║');
        assert_eq!(divider.with_style(DividerStyle::Thick).char(), '┃');
    }

    #[test]
    fn test_divider_color_and_margins() {
        let divider = DividerCapsule::horizontal()
            .with_color(0x808080FF)
            .with_margin(2, 3);

        assert_eq!(divider.color, 0x808080FF);
        assert_eq!(divider.margin_before, 2);
        assert_eq!(divider.margin_after, 3);
    }

    #[test]
    fn test_divider_render_empty_area() {
        let divider = DividerCapsule::horizontal();
        let area = Rect::new(0, 0, 0, 0);
        let mut cmd = RenderCommandBuffer::new();

        divider.render(area, &mut cmd);
        assert_eq!(cmd.len(), 0); // No commands for empty area
    }

    // ============================================================================
    // T28 Q8-Q14: Property Tests
    // ============================================================================

    #[cfg(feature = "std")]
    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn test_spacer_measure_bounds(
                available in 0u16..1000,
                min in 0u16..100,
                max in 100u16..1000,
            ) {
                let spacer = SpacerCapsule::flex(1)
                    .with_min_size(min)
                    .with_max_size(max);

                let size = spacer.measure(available);

                // Size must be within bounds
                prop_assert!(size >= min);
                if max > 0 {
                    prop_assert!(size <= max);
                }
            }

            #[test]
            fn test_divider_render_safe(
                x in 0u16..100,
                y in 0u16..100,
                width in 0u16..100,
                height in 0u16..100,
                margin_before in 0u8..10,
                margin_after in 0u8..10,
            ) {
                let divider = DividerCapsule::horizontal()
                    .with_margin(margin_before, margin_after);

                let area = Rect::new(x, y, width, height);
                let mut cmd = RenderCommandBuffer::new();

                // Should not panic regardless of input
                divider.render(area, &mut cmd);

                // All commands should be within area bounds
                for command in cmd.iter() {
                    match command {
                        RenderCommand::DrawChar { x: cx, y: cy, .. } => {
                            prop_assert!(*cx >= x);
                            prop_assert!(*cy >= y);
                            prop_assert!(*cx < x + width);
                            prop_assert!(*cy < y + height);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
