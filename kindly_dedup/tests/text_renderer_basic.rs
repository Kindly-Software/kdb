//! Basic integration test for TextRendererCapsule
//!
//! Tests text rendering module independently

use kindly_dedup::gui_v2::render::{TextRendererCapsule, TextRenderParams};
use kindly_dedup::gui_v2::widgets::Color;

#[test]
fn test_text_renderer_creation() {
    let renderer = TextRendererCapsule::new();
    assert_eq!(renderer.vertex_count(), 0);
    assert!(!renderer.is_atlas_ready());
}

#[test]
fn test_measure_text() {
    let renderer = TextRendererCapsule::new();
    let params = TextRenderParams {
        font_size: 14,
        ..Default::default()
    };

    let (width, height) = renderer.measure_text("Hello", params);
    // 5 chars × 0.6 × 14px = 42px
    assert!((width - 42.0).abs() < 1.0);
    assert!((height - 14.0).abs() < 1.0);
}

#[test]
fn test_generate_vertices() {
    let renderer = TextRendererCapsule::new();
    let params = TextRenderParams {
        font_size: 14,
        color: Color { r: 255, g: 255, b: 255, a: 255 },
        x: 10.0,
        y: 20.0,
        line_height: 1.2,
    };

    let vertices = renderer.generate_text_vertices("ABC", params);
    assert_eq!(vertices.len(), 12); // 3 chars × 4 vertices
}

#[test]
fn test_multiline_text() {
    let renderer = TextRendererCapsule::new();
    let params = TextRenderParams {
        font_size: 14,
        line_height: 1.5,
        ..Default::default()
    };

    let (width, height) = renderer.measure_text("Hello\nWorld", params);
    assert!((width - 42.0).abs() < 1.0); // Max line width: 5 chars
    // 2 lines: (2-1) × 14×1.5 + 14 = 35px
    assert!((height - 35.0).abs() < 1.0);
}

#[test]
fn test_size_and_alignment() {
    use std::mem::{size_of, align_of};

    assert_eq!(size_of::<TextRendererCapsule>(), 512);
    assert_eq!(align_of::<TextRendererCapsule>(), 64);
}
