//! LabelCapsule Verification Test
//!
//! Standalone test to verify LabelCapsule implementation.

#![cfg(all(feature = "tui-terminal", feature = "terminal-widgets"))]

use atomic_capsule::terminal::widget::foundation::{LabelCapsule, TextAlign, TextOverflow};
use atomic_capsule::terminal::widget::{Rect, RenderCommandBuffer};

#[test]
fn verify_label_size_and_alignment() {
    assert_eq!(core::mem::size_of::<LabelCapsule>(), 128);
    assert_eq!(core::mem::align_of::<LabelCapsule>(), 64);
}

#[test]
fn verify_label_creation() {
    let label = LabelCapsule::new("Hello World");
    assert_eq!(label.text(), "Hello World");
}

#[test]
fn verify_label_text_update() {
    let mut label = LabelCapsule::new("Initial");
    assert_eq!(label.text(), "Initial");

    label.set_text("Updated");
    assert_eq!(label.text(), "Updated");
}

#[test]
fn verify_label_visibility() {
    let label = LabelCapsule::new("Test");

    label.set_visible(false);
    // State is internal, can't check directly but shouldn't panic

    label.set_visible(true);
    // State restored
}

#[test]
fn verify_label_opacity() {
    let label = LabelCapsule::new("Test");

    label.set_opacity(0.5);
    label.set_opacity(0.0);
    label.set_opacity(1.0);

    // Should clamp to [0.0, 1.0]
    label.set_opacity(2.0);
    label.set_opacity(-1.0);
}

#[test]
fn verify_label_alignment() {
    let left = LabelCapsule::new("Left").with_align(TextAlign::Left);
    let center = LabelCapsule::new("Center").with_align(TextAlign::Center);
    let right = LabelCapsule::new("Right").with_align(TextAlign::Right);

    // Just verify construction doesn't panic
    let _ = (left, center, right);
}

#[test]
fn verify_label_overflow() {
    let clip = LabelCapsule::new("Test").with_overflow(TextOverflow::Clip);
    let ellipsis = LabelCapsule::new("Test").with_overflow(TextOverflow::Ellipsis);
    let wrap = LabelCapsule::new("Test").with_overflow(TextOverflow::Wrap);

    // Just verify construction doesn't panic
    let _ = (clip, ellipsis, wrap);
}

#[test]
fn verify_label_styling() {
    let label = LabelCapsule::new("Styled")
        .with_color(0xFF0000FF)
        .with_bold()
        .with_italic();

    assert_eq!(label.text(), "Styled");
}

#[test]
fn verify_label_render() {
    let label = LabelCapsule::new("Render Test");
    let mut cmd = RenderCommandBuffer::new();

    let area = Rect::new(0, 0, 50, 1);
    label.render(area, &mut cmd);

    // Should emit at least one command
    assert!(cmd.commands().len() > 0);
}

#[test]
fn verify_label_truncation() {
    let long_text = "a".repeat(100);
    let label = LabelCapsule::new(&long_text);

    // Should truncate to 63 chars
    assert_eq!(label.text().len(), 63);
}

#[test]
fn verify_label_default() {
    let label = LabelCapsule::default();
    assert_eq!(label.text(), "");
}
