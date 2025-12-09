//! Integration tests for StyleUniformsCapsule
//!
//! T28 Q15-Q21: Integration tier tests

#![cfg(all(feature = "tui-terminal", feature = "terminal-gpu"))]

use atomic_capsule::terminal::style::{
    StyleUniformsCapsule, GlobalUniforms, WidgetUniforms,
    WIDGET_FLAG_FOCUSED, WIDGET_FLAG_HOVERED,
};
use atomic_capsule::terminal::widget::{Rect, Color};

#[test]
fn test_style_uniforms_basic_usage() {
    let uniforms = StyleUniformsCapsule::new();

    // Verify default initialization
    assert_eq!(uniforms.generation(), 0);
    assert!(!uniforms.needs_upload());

    // Update global theme
    uniforms.update_global_simple(
        [0.8, 0.4, 0.9, 1.0],
        1920.0,
        1080.0,
    );

    assert_eq!(uniforms.generation(), 1);
    assert!(uniforms.needs_upload());
}

#[test]
fn test_widget_upload_workflow() {
    let uniforms = StyleUniformsCapsule::new();

    // Update widget 0
    uniforms.update_widget_simple(
        0,
        Color::new(255, 0, 0, 255),
        Color::new(0, 0, 0, 255),
        Rect::new(0, 0, 100, 50),
    );

    assert!(uniforms.needs_widget_upload(0));
    assert!(!uniforms.needs_widget_upload(1));

    // Simulate GPU upload
    let gen = uniforms.begin_upload();
    let bytes = uniforms.prepare_widget_upload(0).unwrap();
    assert_eq!(bytes.len(), 32); // WidgetUniforms size

    assert!(uniforms.end_upload(gen));
    uniforms.clear_widget_dirty(0);

    assert!(!uniforms.needs_widget_upload(0));
}

#[test]
fn test_concurrent_modification_detection() {
    let uniforms = StyleUniformsCapsule::new();

    let gen1 = uniforms.begin_upload();

    // Concurrent update
    uniforms.update_time(1.5);

    // Should detect modification
    assert!(!uniforms.end_upload(gen1));

    // Retry with new generation
    let gen2 = uniforms.begin_upload();
    assert!(uniforms.end_upload(gen2));
}

#[test]
fn test_60fps_simulation() {
    let uniforms = StyleUniformsCapsule::new();

    for frame in 0..60 {
        let time = frame as f32 / 60.0;
        uniforms.update_time(time);

        let gen = uniforms.begin_upload();
        let _bytes = uniforms.prepare_global_upload();
        uniforms.end_upload(gen);
        uniforms.clear_dirty();
    }

    assert_eq!(uniforms.frame_number(), 60);
    assert_eq!(uniforms.upload_count(), 60);
}

#[test]
fn test_batched_widget_updates() {
    let uniforms = StyleUniformsCapsule::new();

    // Update all 4 widgets
    for slot in 0..4 {
        uniforms.update_widget_simple(
            slot,
            Color::new(255, (slot * 64) as u8, 0, 255),
            Color::new(0, 0, 0, 255),
            Rect::new(slot as u16 * 100, 0, 100, 100),
        );
    }

    // Verify all widgets dirty
    for slot in 0..4 {
        assert!(uniforms.needs_widget_upload(slot));
    }

    // Upload all at once
    let gen = uniforms.begin_upload();
    let all_bytes = uniforms.prepare_all_widgets_upload();
    assert_eq!(all_bytes.len(), 128); // 4 × 32B
    uniforms.end_upload(gen);
    uniforms.clear_dirty();

    assert!(!uniforms.needs_upload());
}

#[test]
fn test_layout_sizes() {
    use core::mem::{size_of, align_of};

    // Capsule layout
    assert_eq!(size_of::<StyleUniformsCapsule>(), 256);
    assert_eq!(align_of::<StyleUniformsCapsule>(), 64);

    // GlobalUniforms layout (should fit in 96B for std140)
    assert!(size_of::<GlobalUniforms>() <= 96);
    assert_eq!(align_of::<GlobalUniforms>(), 16);

    // WidgetUniforms layout
    assert_eq!(size_of::<WidgetUniforms>(), 32);
    assert_eq!(align_of::<WidgetUniforms>(), 16);
}
