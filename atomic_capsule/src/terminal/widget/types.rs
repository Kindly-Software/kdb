//! Widget System Core Types
//!
//! Common types and traits for the widget system.

use core::fmt;

// ============================================================================
// GEOMETRIC PRIMITIVES
// ============================================================================

/// Rectangle area for widget layout
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Rect {
    /// X coordinate (column)
    pub x: u16,
    /// Y coordinate (row)
    pub y: u16,
    /// Width in cells
    pub width: u8,
    /// Height in cells
    pub height: u8,
}

impl Rect {
    /// Create new rectangle
    #[inline]
    pub const fn new(x: u16, y: u16, width: u8, height: u8) -> Self {
        Self { x, y, width, height }
    }

    /// Check if point is inside rectangle
    #[inline]
    pub const fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.x + self.width as u16 &&
        y >= self.y && y < self.y + self.height as u16
    }

    /// Get area (width × height)
    #[inline]
    pub const fn area(&self) -> u16 {
        self.width as u16 * self.height as u16
    }

    /// Check if rectangle is empty
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// Layout constraints for widget measurement
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Constraints {
    pub min_width: u16,
    pub max_width: u16,
    pub min_height: u16,
    pub max_height: u16,
}

impl Constraints {
    /// Create new constraints
    #[inline]
    pub const fn new(min_width: u16, max_width: u16, min_height: u16, max_height: u16) -> Self {
        Self {
            min_width,
            max_width,
            min_height,
            max_height,
        }
    }

    /// Create tight constraints (exact size)
    #[inline]
    pub const fn tight(width: u16, height: u16) -> Self {
        Self {
            min_width: width,
            max_width: width,
            min_height: height,
            max_height: height,
        }
    }

    /// Create loose constraints (flexible size)
    #[inline]
    pub const fn loose(max_width: u16, max_height: u16) -> Self {
        Self {
            min_width: 0,
            max_width,
            min_height: 0,
            max_height,
        }
    }

    /// Clamp width and height to constraints
    #[inline]
    pub const fn clamp(&self, width: u16, height: u16) -> (u16, u16) {
        let w = if width < self.min_width {
            self.min_width
        } else if width > self.max_width {
            self.max_width
        } else {
            width
        };

        let h = if height < self.min_height {
            self.min_height
        } else if height > self.max_height {
            self.max_height
        } else {
            height
        };

        (w, h)
    }
}

// ============================================================================
// COLOR PRIMITIVES
// ============================================================================

/// RGBA8888 color
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Color {
    /// Red component (0-255)
    pub r: u8,
    /// Green component (0-255)
    pub g: u8,
    /// Blue component (0-255)
    pub b: u8,
    /// Alpha component (0-255)
    pub a: u8,
}

impl Color {
    /// Create color from RGBA components
    #[inline]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Create color from RGBA components (alias for new)
    #[inline]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Create color from RGB (alpha = 255)
    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Create color from packed RGBA8888 u32
    #[inline]
    pub const fn from_rgba(rgba: u32) -> Self {
        Self {
            r: ((rgba >> 24) & 0xFF) as u8,
            g: ((rgba >> 16) & 0xFF) as u8,
            b: ((rgba >> 8) & 0xFF) as u8,
            a: (rgba & 0xFF) as u8,
        }
    }

    /// Pack color to RGBA8888 u32
    #[inline]
    pub const fn to_rgba(&self) -> u32 {
        ((self.r as u32) << 24) |
        ((self.g as u32) << 16) |
        ((self.b as u32) << 8) |
        (self.a as u32)
    }

    /// Alias for from_rgba (for compatibility with scroll.rs)
    #[inline]
    pub const fn from_u32(rgba: u32) -> Self {
        Self::from_rgba(rgba)
    }

    /// Alias for from_rgba (for compatibility)
    #[inline]
    pub const fn from_rgba8888(rgba: u32) -> Self {
        Self::from_rgba(rgba)
    }

    // Common colors
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const RED: Self = Self::rgb(255, 0, 0);
    pub const GREEN: Self = Self::rgb(0, 255, 0);
    pub const BLUE: Self = Self::rgb(0, 0, 255);
    pub const YELLOW: Self = Self::rgb(255, 255, 0);
    pub const CYAN: Self = Self::rgb(0, 255, 255);
    pub const MAGENTA: Self = Self::rgb(255, 0, 255);
    pub const GRAY: Self = Self::rgb(128, 128, 128);
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rgba({}, {}, {}, {})", self.r, self.g, self.b, self.a)
    }
}

// ============================================================================
// RENDER COMMAND BUFFER
// ============================================================================

/// Render command type
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RenderCommand {
    /// Draw character at position
    DrawChar {
        x: u16,
        y: u16,
        ch: char,
        color: Color,
    },
    /// Draw text at position
    DrawText {
        x: u16,
        y: u16,
        len: u16,
        color: Color,
    },
    /// Fill rectangle with character
    FillRect {
        rect: Rect,
        ch: char,
        color: Color,
    },
    /// Clear rectangle
    ClearRect {
        rect: Rect,
    },
}

/// Rendering style (colors, attributes)
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct RenderStyle {
    pub fg_color: u32, // RGBA8888
    pub bg_color: u32, // RGBA8888
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl RenderStyle {
    pub const fn new(fg_color: u32, bg_color: u32) -> Self {
        Self {
            fg_color,
            bg_color,
            bold: false,
            italic: false,
            underline: false,
        }
    }

    pub const fn with_bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub const fn with_italic(mut self) -> Self {
        self.italic = true;
        self
    }

    pub const fn with_underline(mut self) -> Self {
        self.underline = true;
        self
    }
}

/// Render command buffer
///
/// Simple command buffer for widget rendering.
/// In a real implementation, this would be a lockfree queue.
#[derive(Debug)]
pub struct RenderCommandBuffer {
    commands: Vec<RenderCommand>,
    text_buffer: Vec<u8>,
}

impl Default for RenderCommandBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderCommandBuffer {
    /// Create new command buffer
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            text_buffer: Vec::new(),
        }
    }

    /// Clear all commands
    pub fn clear(&mut self) {
        self.commands.clear();
        self.text_buffer.clear();
    }

    /// Draw single character
    pub fn draw_char(&mut self, x: u16, y: u16, ch: char, color: Color) {
        self.commands.push(RenderCommand::DrawChar { x, y, ch, color });
    }

    /// Draw text string
    pub fn draw_text(&mut self, x: u16, y: u16, text: &str, color: Color) {
        let start = self.text_buffer.len();
        self.text_buffer.extend_from_slice(text.as_bytes());
        let len = (self.text_buffer.len() - start) as u16;

        self.commands.push(RenderCommand::DrawText { x, y, len, color });
    }

    /// Fill rectangle with character
    pub fn fill_rect(&mut self, rect: Rect, ch: char, color: Color) {
        self.commands.push(RenderCommand::FillRect { rect, ch, color });
    }

    /// Clear rectangle
    pub fn clear_rect(&mut self, rect: Rect) {
        self.commands.push(RenderCommand::ClearRect { rect });
    }

    /// Get commands
    pub fn commands(&self) -> &[RenderCommand] {
        &self.commands
    }

    /// Get text buffer
    pub fn text_buffer(&self) -> &[u8] {
        &self.text_buffer
    }
}

// ============================================================================
// WIDGET TRAIT
// ============================================================================

/// Widget trait for all TUI widgets
///
/// # Type Safety
///
/// Each widget defines its own `State` type for atomic snapshots.
///
/// # Lockfree Coordination
///
/// Widgets use atomic operations for all state updates.
/// The `snapshot()` method provides consistent reads via memory ordering.
pub trait Widget {
    /// Widget state snapshot type
    type State: Copy + Clone;

    /// Take atomic snapshot of widget state
    ///
    /// # Memory Ordering
    ///
    /// Uses `Acquire` ordering to ensure visibility of all prior writes.
    fn snapshot(&self) -> Self::State;

    /// Check if widget is focusable
    ///
    /// Focusable widgets can receive keyboard input.
    fn is_focusable(&self) -> bool {
        false
    }

    /// Get minimum size hint (width, height)
    ///
    /// Layout system uses this for automatic sizing.
    fn min_size(&self) -> (u16, u16) {
        (1, 1)
    }

    /// Get preferred size hint (width, height)
    ///
    /// Optional preference, layout may ignore if space constrained.
    fn preferred_size(&self) -> Option<(u16, u16)> {
        None
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_creation() {
        let rect = Rect::new(10, 20, 30, 40);
        assert_eq!(rect.x, 10);
        assert_eq!(rect.y, 20);
        assert_eq!(rect.width, 30);
        assert_eq!(rect.height, 40);
    }

    #[test]
    fn test_rect_contains() {
        let rect = Rect::new(10, 20, 30, 40);

        assert!(rect.contains(10, 20)); // Top-left
        assert!(rect.contains(39, 59)); // Bottom-right (exclusive)
        assert!(!rect.contains(9, 20)); // Outside left
        assert!(!rect.contains(40, 20)); // Outside right
        assert!(!rect.contains(10, 19)); // Outside top
        assert!(!rect.contains(10, 60)); // Outside bottom
    }

    #[test]
    fn test_rect_area() {
        let rect = Rect::new(0, 0, 10, 20);
        assert_eq!(rect.area(), 200);
    }

    #[test]
    fn test_rect_is_empty() {
        assert!(Rect::new(0, 0, 0, 10).is_empty());
        assert!(Rect::new(0, 0, 10, 0).is_empty());
        assert!(!Rect::new(0, 0, 10, 10).is_empty());
    }

    #[test]
    fn test_color_creation() {
        let color = Color::rgba(255, 128, 64, 32);
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 64);
        assert_eq!(color.a, 32);
    }

    #[test]
    fn test_color_from_rgba() {
        let color = Color::from_rgba(0xFF8040FF);
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 64);
        assert_eq!(color.a, 255);
    }

    #[test]
    fn test_color_to_rgba() {
        let color = Color::rgba(255, 128, 64, 32);
        assert_eq!(color.to_rgba(), 0xFF804020);
    }

    #[test]
    fn test_color_roundtrip() {
        let original = 0xAABBCCDD;
        let color = Color::from_rgba(original);
        let packed = color.to_rgba();
        assert_eq!(packed, original);
    }

    #[test]
    fn test_common_colors() {
        assert_eq!(Color::BLACK.to_rgba(), 0x000000FF);
        assert_eq!(Color::WHITE.to_rgba(), 0xFFFFFFFF);
        assert_eq!(Color::RED.to_rgba(), 0xFF0000FF);
        assert_eq!(Color::GREEN.to_rgba(), 0x00FF00FF);
        assert_eq!(Color::BLUE.to_rgba(), 0x0000FFFF);
    }

    #[test]
    fn test_render_command_buffer() {
        let mut cmd = RenderCommandBuffer::new();

        cmd.draw_char(0, 0, 'A', Color::RED);
        cmd.draw_text(1, 0, "Hello", Color::GREEN);
        cmd.fill_rect(Rect::new(0, 1, 10, 5), '█', Color::BLUE);
        cmd.clear_rect(Rect::new(0, 6, 10, 1));

        assert_eq!(cmd.commands().len(), 4);

        // Verify first command
        match cmd.commands()[0] {
            RenderCommand::DrawChar { x, y, ch, color } => {
                assert_eq!(x, 0);
                assert_eq!(y, 0);
                assert_eq!(ch, 'A');
                assert_eq!(color, Color::RED);
            }
            _ => panic!("Wrong command type"),
        }

        // Verify text buffer
        let text = core::str::from_utf8(cmd.text_buffer()).unwrap();
        assert_eq!(text, "Hello");
    }
}
