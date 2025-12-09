//! ProgressCapsule Demo
//!
//! Demonstrates the ProgressCapsule with different styles and animation.

#![cfg(all(feature = "tui-terminal", feature = "terminal-widgets"))]

extern crate alloc;

use atomic_capsule::terminal::{
    ProgressCapsule, ProgressStyle, ProgressState,
    Widget, Rect, Constraints, RenderCommandBuffer,
};

fn main() {
    println!("ProgressCapsule Demo\n");

    // Create progress bars with different styles
    let styles = [
        ("Bar", ProgressStyle::Bar),
        ("Striped", ProgressStyle::Striped),
        ("Blocks", ProgressStyle::Blocks),
        ("Dots", ProgressStyle::Dots),
    ];

    for (name, style) in &styles {
        let progress = ProgressCapsule::new()
            .with_style(*style)
            .with_width(40)
            .with_label(name);

        // Set to 75%
        progress.set_value(0.75);

        println!("{}: {:.0}%", name, progress.value() * 100.0);

        // Render to command buffer
        let state = ProgressState::default();
        let area = Rect::new(0, 0, 60, 2);
        let mut cmd = RenderCommandBuffer::new();

        progress.render(area, &state, &mut cmd);
        println!("  Commands: {}", cmd.commands().len());
    }

    println!("\n=== Animation Test ===");
    let progress = ProgressCapsule::new()
        .with_style(ProgressStyle::Blocks)
        .with_width(40)
        .with_label("Downloading");

    progress.set_value(0.0);
    progress.set_value_animated(1.0);

    println!("Initial: {:.2}", progress.value());

    // Simulate 60fps animation for 500ms
    for frame in 0..30 {
        progress.update_animation(16);
        println!("Frame {}: {:.2}", frame, progress.value());
    }

    println!("Final: {:.2}", progress.value());

    println!("\n=== Indeterminate Mode ===");
    let progress = ProgressCapsule::new()
        .with_style(ProgressStyle::Striped)
        .with_width(40);

    progress.set_indeterminate(true);
    println!("Indeterminate: {}", progress.is_indeterminate());

    for _ in 0..5 {
        progress.update_animation(50);
    }

    println!("\n=== Widget Trait ===");
    let progress = ProgressCapsule::new()
        .with_width(40)
        .with_label("Progress");

    let state = ProgressState::default();
    let constraints = Constraints::loose(80, 10);

    let (width, height) = progress.measure(constraints, &state);
    println!("Measured size: {}x{}", width, height);
    println!("Focusable: {}", progress.focusable());
    println!("Tab index: {}", progress.tab_index());

    println!("\n=== Size Verification ===");
    println!("ProgressCapsule size: {} bytes", core::mem::size_of::<ProgressCapsule>());
    println!("ProgressCapsule align: {} bytes", core::mem::align_of::<ProgressCapsule>());
    println!("ProgressState size: {} bytes", core::mem::size_of::<ProgressState>());

    println!("\nDemo complete!");
}
