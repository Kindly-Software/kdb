//! Rendering Primitives for GPU Rendering
//!
//! # Overview
//!
//! Lightweight shape and text command types for widget rendering.
//! Widgets emit these primitives, which are then batched and sent to GPU.
//!
//! # Architecture
//!
//! ```text
//! Widget::render()
//!   ↓ Emits shapes + text
//! Vec<Shape> + Vec<TextCommand>
//!   ↓ Batching
//! GPU Vertex Buffers
//!   ↓ Draw calls
//! Screen Framebuffer
//! ```
//!
//! # Performance Targets (B32)
//!
//! - Shape creation: <10ns (stack allocation)
//! - Text command: <20ns (string copy avoided via &'static str)
//! - Batch collection: <1µs (5-20 widgets)
//! - GPU upload: <100µs (memcpy to mapped buffer)
//!
//! # Framework Compliance
//!
//! - **UCE34**: T0 Auditable (simple POD types)
//! - **Chaos**: 100% safe Rust (no atomic state needed)
//! - **T28**: Unit tests for shape creation

use crate::gui_v2::{layout::Rect, widgets::Color};

/// 2D Shape for rendering
///
/// # Variants
///
/// - **Rectangle**: Axis-aligned box with optional border
/// - **Circle**: For buttons, avatars
/// - **Line**: For separators, borders
/// - **Gradient**: Linear gradient fill
#[derive(Debug, Clone)]
pub enum Shape {
    /// Filled rectangle
    Rectangle {
        bounds: Rect,
        fill_color: Color,
        border_color: Option<Color>,
        border_width: u32,
        corner_radius: u32,
    },

    /// Circle (for buttons, icons)
    Circle {
        center_x: i32,
        center_y: i32,
        radius: u32,
        fill_color: Color,
        border_color: Option<Color>,
        border_width: u32,
    },

    /// Line segment
    Line {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: Color,
        width: u32,
    },

    /// Linear gradient rectangle
    Gradient {
        bounds: Rect,
        start_color: Color,
        end_color: Color,
        /// Angle in degrees (0 = left-to-right, 90 = top-to-bottom)
        angle: f32,
    },
}

impl Shape {
    /// Create simple filled rectangle (no border, no corners)
    pub fn rect(bounds: Rect, fill_color: Color) -> Self {
        Shape::Rectangle {
            bounds,
            fill_color,
            border_color: None,
            border_width: 0,
            corner_radius: 0,
        }
    }

    /// Create rectangle with border
    pub fn rect_with_border(
        bounds: Rect,
        fill_color: Color,
        border_color: Color,
        border_width: u32,
    ) -> Self {
        Shape::Rectangle {
            bounds,
            fill_color,
            border_color: Some(border_color),
            border_width,
            corner_radius: 0,
        }
    }

    /// Create rounded rectangle
    pub fn rounded_rect(bounds: Rect, fill_color: Color, corner_radius: u32) -> Self {
        Shape::Rectangle {
            bounds,
            fill_color,
            border_color: None,
            border_width: 0,
            corner_radius,
        }
    }

    /// Create simple circle
    pub fn circle(center_x: i32, center_y: i32, radius: u32, fill_color: Color) -> Self {
        Shape::Circle {
            center_x,
            center_y,
            radius,
            fill_color,
            border_color: None,
            border_width: 0,
        }
    }

    /// Create line
    pub fn line(x1: i32, y1: i32, x2: i32, y2: i32, color: Color, width: u32) -> Self {
        Shape::Line {
            x1,
            y1,
            x2,
            y2,
            color,
            width,
        }
    }

    /// Create horizontal gradient (left-to-right)
    pub fn gradient_h(bounds: Rect, start_color: Color, end_color: Color) -> Self {
        Shape::Gradient {
            bounds,
            start_color,
            end_color,
            angle: 0.0,
        }
    }

    /// Create vertical gradient (top-to-bottom)
    pub fn gradient_v(bounds: Rect, start_color: Color, end_color: Color) -> Self {
        Shape::Gradient {
            bounds,
            start_color,
            end_color,
            angle: 90.0,
        }
    }
}

/// Text rendering command
///
/// # Layout
///
/// - **Position**: (x, y) is top-left corner of text bounding box
/// - **Size**: Font size in pixels (16px, 24px, 48px, 64px supported)
/// - **Color**: RGBA color
/// - **Text**: UTF-8 string (&'static str for zero-copy, String for dynamic)
#[derive(Debug, Clone)]
pub struct TextCommand {
    /// Text content
    pub text: TextContent,
    /// Top-left position
    pub x: i32,
    pub y: i32,
    /// Font size in pixels
    pub font_size: u32,
    /// Text color
    pub color: Color,
    /// Horizontal alignment (Left, Center, Right)
    pub align: TextAlign,
}

/// Text content (zero-copy for static strings)
#[derive(Debug, Clone)]
pub enum TextContent {
    /// Static string (zero-copy, used for UI labels)
    Static(&'static str),
    /// Dynamic string (heap-allocated, used for user data)
    Dynamic(String),
}

impl From<&'static str> for TextContent {
    fn from(s: &'static str) -> Self {
        TextContent::Static(s)
    }
}

impl From<String> for TextContent {
    fn from(s: String) -> Self {
        TextContent::Dynamic(s)
    }
}

impl TextContent {
    /// Get string slice
    pub fn as_str(&self) -> &str {
        match self {
            TextContent::Static(s) => s,
            TextContent::Dynamic(s) => s,
        }
    }
}

/// Text alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

impl TextCommand {
    /// Create text command (left-aligned)
    pub fn new<T: Into<TextContent>>(
        text: T,
        x: i32,
        y: i32,
        font_size: u32,
        color: Color,
    ) -> Self {
        Self {
            text: text.into(),
            x,
            y,
            font_size,
            color,
            align: TextAlign::Left,
        }
    }

    /// Create centered text command
    pub fn centered<T: Into<TextContent>>(
        text: T,
        x: i32,
        y: i32,
        font_size: u32,
        color: Color,
    ) -> Self {
        Self {
            text: text.into(),
            x,
            y,
            font_size,
            color,
            align: TextAlign::Center,
        }
    }

    /// Estimate text width (rough approximation, actual width from font metrics)
    pub fn estimate_width(&self) -> i32 {
        // Rough estimate: font_size * 0.6 per character (monospace assumption)
        let char_count = self.text.as_str().chars().count();
        (char_count as f32 * self.font_size as f32 * 0.6) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shape_rect() {
        let bounds = Rect {
            x: 10,
            y: 20,
            width: 100,
            height: 50,
        };
        let color = Color::rgb(255, 0, 0);

        let shape = Shape::rect(bounds.clone(), color);

        match shape {
            Shape::Rectangle {
                bounds: b,
                fill_color,
                border_color,
                border_width,
                corner_radius,
            } => {
                assert_eq!(b, bounds);
                assert_eq!(fill_color, color);
                assert!(border_color.is_none());
                assert_eq!(border_width, 0);
                assert_eq!(corner_radius, 0);
            }
            _ => panic!("Expected Rectangle"),
        }
    }

    #[test]
    fn test_shape_rect_with_border() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 100,
        };
        let fill = Color::rgb(255, 255, 255);
        let border = Color::rgb(0, 0, 0);

        let shape = Shape::rect_with_border(bounds.clone(), fill, border, 2);

        match shape {
            Shape::Rectangle {
                border_color,
                border_width,
                ..
            } => {
                assert_eq!(border_color, Some(border));
                assert_eq!(border_width, 2);
            }
            _ => panic!("Expected Rectangle"),
        }
    }

    #[test]
    fn test_shape_rounded_rect() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let shape = Shape::rounded_rect(bounds, Color::rgb(128, 128, 128), 10);

        match shape {
            Shape::Rectangle { corner_radius, .. } => {
                assert_eq!(corner_radius, 10);
            }
            _ => panic!("Expected Rectangle"),
        }
    }

    #[test]
    fn test_shape_circle() {
        let shape = Shape::circle(50, 50, 25, Color::rgb(0, 255, 0));

        match shape {
            Shape::Circle {
                center_x,
                center_y,
                radius,
                fill_color,
                ..
            } => {
                assert_eq!(center_x, 50);
                assert_eq!(center_y, 50);
                assert_eq!(radius, 25);
                assert_eq!(fill_color.g, 255);
            }
            _ => panic!("Expected Circle"),
        }
    }

    #[test]
    fn test_shape_line() {
        let shape = Shape::line(0, 0, 100, 100, Color::rgb(255, 0, 0), 2);

        match shape {
            Shape::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                ..
            } => {
                assert_eq!(x1, 0);
                assert_eq!(y1, 0);
                assert_eq!(x2, 100);
                assert_eq!(y2, 100);
                assert_eq!(width, 2);
            }
            _ => panic!("Expected Line"),
        }
    }

    #[test]
    fn test_shape_gradient_h() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 50,
        };
        let start = Color::rgb(255, 0, 0);
        let end = Color::rgb(0, 0, 255);

        let shape = Shape::gradient_h(bounds.clone(), start, end);

        match shape {
            Shape::Gradient {
                bounds: b,
                start_color,
                end_color,
                angle,
            } => {
                assert_eq!(b, bounds);
                assert_eq!(start_color, start);
                assert_eq!(end_color, end);
                assert_eq!(angle, 0.0);
            }
            _ => panic!("Expected Gradient"),
        }
    }

    #[test]
    fn test_shape_gradient_v() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 50,
        };
        let shape = Shape::gradient_v(bounds, Color::rgb(255, 0, 0), Color::rgb(0, 0, 255));

        match shape {
            Shape::Gradient { angle, .. } => {
                assert_eq!(angle, 90.0);
            }
            _ => panic!("Expected Gradient"),
        }
    }

    #[test]
    fn test_text_command_static() {
        let cmd = TextCommand::new("Hello", 10, 20, 24, Color::rgb(0, 0, 0));

        assert_eq!(cmd.text.as_str(), "Hello");
        assert_eq!(cmd.x, 10);
        assert_eq!(cmd.y, 20);
        assert_eq!(cmd.font_size, 24);
        assert_eq!(cmd.align, TextAlign::Left);
    }

    #[test]
    fn test_text_command_dynamic() {
        let text = String::from("Dynamic text");
        let cmd = TextCommand::new(text.clone(), 50, 100, 16, Color::rgb(255, 255, 255));

        assert_eq!(cmd.text.as_str(), "Dynamic text");
    }

    #[test]
    fn test_text_command_centered() {
        let cmd = TextCommand::centered("Centered", 100, 200, 32, Color::rgb(128, 128, 128));

        assert_eq!(cmd.align, TextAlign::Center);
    }

    #[test]
    fn test_text_estimate_width() {
        let cmd = TextCommand::new("Test", 0, 0, 20, Color::rgb(0, 0, 0));

        // "Test" = 4 chars, 20px font → ~48px (4 * 20 * 0.6)
        let width = cmd.estimate_width();
        assert!(width >= 40 && width <= 60, "Width {} out of range", width);
    }

    #[test]
    fn test_text_content_static() {
        let content = TextContent::from("static");
        assert_eq!(content.as_str(), "static");

        match content {
            TextContent::Static(_) => {}
            _ => panic!("Expected Static"),
        }
    }

    #[test]
    fn test_text_content_dynamic() {
        let content = TextContent::from(String::from("dynamic"));
        assert_eq!(content.as_str(), "dynamic");

        match content {
            TextContent::Dynamic(_) => {}
            _ => panic!("Expected Dynamic"),
        }
    }
}
