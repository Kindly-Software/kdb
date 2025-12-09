//! Rendering subsystem for gui_v2
//!
//! # Architecture
//!
//! - **ShapeRendererCapsule**: T2 SIMD batching for rectangles/borders (GPU accelerated)
//! - **TextRendererCapsule**: T1+T2 lockfree text rendering
//! - **Glyph atlas**: Pre-rasterized ASCII glyphs (32-126)
//! - **Text shaping**: Simple left-to-right layout
//!
//! # Modules
//!
//! - `shape_renderer`: ShapeRendererCapsule (batch GPU rendering, SDF-based)
//! - `text_renderer`: TextRendererCapsule (glyph atlas + shaping)
//! - `widget_renderer`: WidgetRendererCapsule (T7 Heterogeneous widget→GPU)
//! - `shapes.wgsl`: WGSL shader for shape rendering (embedded)
//! - `text.wgsl`: WGSL shader for text rendering (embedded)
//!
//! # Framework Compliance
//!
//! - **UCE34**: T2 SIMD tier (vectorized batch operations), T7 Heterogeneous (widget_renderer)
//! - **Chaos**: 100% lockfree (AtomicU64 state, no mutex)
//! - **ASSUM**: Fixed capacity validated, overflow checks
//! - **B32**: <1ms render @ 1000 shapes/glyphs target
//! - **T28**: 42+ tests (shape + text + widget rendering)

pub mod shape_renderer;
pub mod text_renderer;
pub mod widget_renderer;
pub mod font_atlas;

// Re-exports
pub use shape_renderer::{Shape, ShapeInstance, ShapeRendererCapsule};
pub use text_renderer::{TextRendererCapsule, TextVertex, TextRenderParams};
pub use widget_renderer::{WidgetRendererCapsule, WidgetVertex, WidgetRenderBatch, DrawCall};
pub use font_atlas::FontAtlasCapsule;

// Embed WGSL shaders at compile-time
pub const SHAPES_WGSL: &str = include_str!("shapes.wgsl");
pub const TEXT_WGSL: &str = include_str!("text.wgsl");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wgsl_shaders_embedded() {
        // Verify shape shader
        assert!(SHAPES_WGSL.contains("vs_main"));
        assert!(SHAPES_WGSL.contains("fs_main"));
        assert!(SHAPES_WGSL.contains("sdf_rounded_rect"));

        // Verify text shader
        assert!(TEXT_WGSL.contains("vs_main"));
        assert!(TEXT_WGSL.contains("fs_main"));
    }

    #[test]
    fn test_module_exports() {
        use crate::gui_v2::layout::Rect;
        use crate::gui_v2::widgets::Color;

        // Verify shape rendering exports
        let mut renderer = ShapeRendererCapsule::new();
        let rect = Rect::new(0, 0, 100, 100);
        let shape = Shape::FilledRect {
            rect,
            color: Color::rgb(255, 0, 0),
        };

        let _instance = ShapeInstance::from_shape(&shape);
        let _ = renderer.push_filled_rect(rect, Color::rgb(0, 255, 0));

        // Verify text rendering exports
        let _text_renderer = TextRendererCapsule::new();

        // Verify widget rendering exports
        let mut widget_renderer = WidgetRendererCapsule::new();
        widget_renderer.begin_frame((800, 600));
        let _batch = widget_renderer.end_frame();
    }
}
