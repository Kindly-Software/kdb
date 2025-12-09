//! T28 Tests for ProgressCapsule
//!
//! Test coverage:
//! - Q1-Q7: Unit tests (8 tests)
//! - Q8-Q14: Property tests (4 tests)
//! - Q15-Q21: Integration tests (2 tests)

#![cfg(all(feature = "tui-terminal", feature = "terminal-widgets", feature = "std"))]

use atomic_capsule::terminal::{
    ProgressCapsule, ProgressStyle, ProgressState,
    Widget, Rect, Constraints, RenderCommandBuffer, RenderStyle,
};

// ============================================================================
// T28 Q1-Q7: UNIT TESTS
// ============================================================================

#[test]
fn test_new_default_state() {
    let progress = ProgressCapsule::new();
    assert_eq!(progress.value(), 0.0);
    assert_eq!(progress.target(), 0.0);
    assert!(!progress.is_indeterminate());
    assert_eq!(progress.style(), ProgressStyle::Bar);
}

#[test]
fn test_set_value_immediate() {
    let progress = ProgressCapsule::new();
    progress.set_value(0.5);
    assert!((progress.value() - 0.5).abs() < 0.001);
    assert!((progress.target() - 0.5).abs() < 0.001);
}

#[test]
fn test_set_value_clamping() {
    let progress = ProgressCapsule::new();

    // Test lower bound
    progress.set_value(-0.5);
    assert_eq!(progress.value(), 0.0);

    // Test upper bound
    progress.set_value(1.5);
    assert_eq!(progress.value(), 1.0);
}

#[test]
fn test_set_value_animated() {
    let progress = ProgressCapsule::new();
    progress.set_value(0.0);
    progress.set_value_animated(1.0);

    assert_eq!(progress.value(), 0.0); // Current unchanged
    assert_eq!(progress.target(), 1.0); // Target updated
}

#[test]
fn test_indeterminate_mode() {
    let progress = ProgressCapsule::new();

    assert!(!progress.is_indeterminate());

    progress.set_indeterminate(true);
    assert!(progress.is_indeterminate());

    progress.set_indeterminate(false);
    assert!(!progress.is_indeterminate());
}

#[test]
fn test_update_animation_value() {
    let progress = ProgressCapsule::new();
    progress.set_value(0.0);
    progress.set_value_animated(1.0);

    // Animate for 100ms (should move toward target)
    progress.update_animation(100);
    let value1 = progress.value();
    assert!(value1 > 0.0 && value1 < 1.0);

    // Animate more (should continue moving)
    progress.update_animation(100);
    let value2 = progress.value();
    assert!(value2 > value1);
}

#[test]
fn test_builder_pattern() {
    let progress = ProgressCapsule::new()
        .with_style(ProgressStyle::Blocks)
        .with_width(50)
        .with_height(2)
        .with_label("Test");

    assert_eq!(progress.style(), ProgressStyle::Blocks);
    assert_eq!(progress.width(), 50);
    assert_eq!(progress.height(), 2);
    assert_eq!(progress.label(), "Test");
}

#[test]
fn test_widget_trait() {
    let progress = ProgressCapsule::new();
    progress.set_value(0.75);

    let state = ProgressState::default();
    let constraints = Constraints::loose(100, 10);
    let (width, height) = progress.measure(constraints, &state);

    assert!(width >= 12); // At least 10 + 2 brackets
    assert!(height >= 1);

    assert!(!progress.focusable());
    assert_eq!(progress.tab_index(), u16::MAX);
}

// ============================================================================
// T28 Q8-Q14: PROPERTY TESTS
// ============================================================================

#[cfg(feature = "proptest")]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_value_always_in_bounds(value in -10.0f32..10.0f32) {
            let progress = ProgressCapsule::new();
            progress.set_value(value);
            let actual = progress.value();
            prop_assert!(actual >= 0.0 && actual <= 1.0);
        }

        #[test]
        fn prop_animation_converges(target in 0.0f32..1.0f32) {
            let progress = ProgressCapsule::new();
            progress.set_value(0.0);
            progress.set_value_animated(target);

            // Animate for sufficient time
            for _ in 0..20 {
                progress.update_animation(16); // 60fps
            }

            let final_value = progress.value();
            prop_assert!((final_value - target).abs() < 0.01);
        }

        #[test]
        fn prop_indeterminate_phase_wraps(iterations in 1usize..100) {
            let progress = ProgressCapsule::new();
            progress.set_indeterminate(true);

            for _ in 0..iterations {
                progress.update_animation(50);
            }

            // Should always remain in indeterminate mode
            prop_assert!(progress.is_indeterminate());
        }

        #[test]
        fn prop_label_truncates(label in ".*") {
            let progress = ProgressCapsule::new().with_label(&label);
            let stored = progress.label();
            prop_assert!(stored.len() <= 24);
        }
    }
}

// ============================================================================
// T28 Q15-Q21: INTEGRATION TESTS
// ============================================================================

#[test]
fn test_progress_lifecycle() {
    let progress = ProgressCapsule::new()
        .with_style(ProgressStyle::Striped)
        .with_width(40)
        .with_label("Loading");

    // Start at 0%
    assert_eq!(progress.value(), 0.0);

    // Animate to 50%
    progress.set_value_animated(0.5);
    for _ in 0..10 {
        progress.update_animation(16);
    }
    assert!(progress.value() > 0.4 && progress.value() < 0.6);

    // Jump to 100%
    progress.set_value(1.0);
    assert_eq!(progress.value(), 1.0);

    // Switch to indeterminate
    progress.set_indeterminate(true);
    progress.update_animation(50);
    assert!(progress.is_indeterminate());
}

#[test]
fn test_concurrent_updates() {
    use std::sync::Arc;
    use std::thread;

    let progress: Arc<ProgressCapsule> = Arc::new(ProgressCapsule::new());
    let mut handles = vec![];

    // Spawn threads updating value
    for i in 0..4 {
        let p: Arc<ProgressCapsule> = Arc::clone(&progress);
        handles.push(thread::spawn(move || {
            let target = (i as f32) / 4.0;
            p.set_value_animated(target);
            for _ in 0..10 {
                p.update_animation(10);
                thread::yield_now();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Should have converged to some valid value
    let final_value = progress.value();
    assert!(final_value >= 0.0 && final_value <= 1.0);
}

// ============================================================================
// RENDERING TESTS
// ============================================================================

#[test]
fn test_render_basic() {
    let progress = ProgressCapsule::new()
        .with_width(20)
        .with_style(ProgressStyle::Bar)
        .with_show_percent(true);

    progress.set_value(0.5);

    let state = ProgressState::default();
    let area = Rect::new(0, 0, 30, 1);
    let mut cmd = RenderCommandBuffer::new();

    progress.render(area, &state, &mut cmd);

    // Should have at least one render command
    assert!(!cmd.commands().is_empty());
}

#[test]
fn test_render_all_styles() {
    let styles = [
        ProgressStyle::Bar,
        ProgressStyle::Striped,
        ProgressStyle::Blocks,
        ProgressStyle::Dots,
    ];

    for style in &styles {
        let progress = ProgressCapsule::new()
            .with_style(*style)
            .with_width(20);

        progress.set_value(0.5);

        let state = ProgressState::default();
        let area = Rect::new(0, 0, 30, 1);
        let mut cmd = RenderCommandBuffer::new();

        progress.render(area, &state, &mut cmd);

        // Should render without panic
        assert!(!cmd.commands().is_empty());
    }
}

// ============================================================================
// SIZE AND ALIGNMENT TESTS
// ============================================================================

#[test]
fn test_capsule_size() {
    use core::mem::{size_of, align_of};

    assert_eq!(size_of::<ProgressCapsule>(), 128);
    assert_eq!(align_of::<ProgressCapsule>(), 64);
}

#[test]
fn test_state_size() {
    use core::mem::size_of;

    // State should be small for efficient copying
    assert!(size_of::<ProgressState>() <= 16);
}

// ============================================================================
// COLOR TESTS
// ============================================================================

#[test]
fn test_custom_colors() {
    let progress = ProgressCapsule::new()
        .with_fill_color(0xFF0000FF) // Red
        .with_track_color(0x00FF00FF) // Green
        .with_text_color(0x0000FFFF); // Blue

    progress.set_value(0.5);

    let state = ProgressState::default();
    let area = Rect::new(0, 0, 30, 1);
    let mut cmd = RenderCommandBuffer::new();

    progress.render(area, &state, &mut cmd);

    // Rendering should succeed with custom colors
    assert!(!cmd.commands().is_empty());
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[test]
fn test_zero_width() {
    let progress = ProgressCapsule::new()
        .with_width(0); // Auto-fill

    progress.set_value(0.5);

    let state = ProgressState::default();
    let area = Rect::new(0, 0, 50, 1);
    let mut cmd = RenderCommandBuffer::new();

    progress.render(area, &state, &mut cmd);

    // Should render without panic
    assert!(!cmd.commands().is_empty());
}

#[test]
fn test_long_label() {
    let long_label = "This is a very long label that exceeds 24 characters";
    let progress = ProgressCapsule::new().with_label(long_label);

    // Label should be truncated to 24 characters
    assert_eq!(progress.label().len(), 24);
}

#[test]
fn test_rapid_animation_updates() {
    let progress = ProgressCapsule::new();
    progress.set_value(0.0);
    progress.set_value_animated(1.0);

    // Rapid updates (1000 frames at 60fps)
    for _ in 0..1000 {
        progress.update_animation(1);
    }

    // Should converge to target without overflow
    let final_value = progress.value();
    assert!((final_value - 1.0).abs() < 0.01);
}
