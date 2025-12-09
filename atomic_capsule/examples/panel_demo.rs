//! Panel Capsule demonstration
//!
//! Shows various panel styles and configurations.

use atomic_capsule::terminal::widget::{
    PanelCapsule, BorderStyle, ShadowDirection, Rect, RenderCommandBuffer,
};

fn main() {
    println!("=== PanelCapsule Demonstration ===\n");

    // Create render buffer (80x24 terminal)
    let mut cmd = RenderCommandBuffer::new(80, 24);

    // Demo 1: Basic panel with solid border
    let panel1 = PanelCapsule::new()
        .with_title("Configuration Panel")
        .with_border(BorderStyle::Solid, 0xFFFFFFFF);

    let area1 = Rect::new(2, 2, 30, 8);
    panel1.render(area1, &mut cmd);
    println!("✓ Panel 1: Solid border, title");

    // Demo 2: Rounded panel with shadow
    let panel2 = PanelCapsule::new()
        .with_title("Status Display")
        .with_border(BorderStyle::Rounded, 0x00FF00FF)
        .with_shadow(ShadowDirection::BottomRight, 0x00000088)
        .with_padding(2, 2, 1, 1);

    let area2 = Rect::new(35, 2, 35, 8);
    panel2.render(area2, &mut cmd);
    println!("✓ Panel 2: Rounded border, shadow, padding");

    // Demo 3: Collapsible panel
    let panel3 = PanelCapsule::new()
        .with_title("Collapsible Panel")
        .with_border(BorderStyle::Double, 0xFF0000FF)
        .with_collapsible();

    let area3 = Rect::new(2, 12, 30, 8);
    panel3.render(area3, &mut cmd);
    println!("✓ Panel 3: Double border, collapsible (▼ button)");

    // Demo 4: Toggle collapsed state
    panel3.set_collapsed(true);
    let area4 = Rect::new(35, 12, 35, 8);
    panel3.render(area4, &mut cmd);
    println!("✓ Panel 4: Same panel, collapsed (▶ button)");

    // Demo 5: Performance test
    let iterations = 100_000;
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        panel3.toggle_collapsed();
    }
    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / iterations as u128;
    println!("\n=== Performance ===");
    println!("Toggle operations: {}", iterations);
    println!("Total time: {:?}", elapsed);
    println!("Average per toggle: {}ns", avg_ns);
    println!("Operations/sec: {:.2}M", (iterations as f64 / elapsed.as_secs_f64()) / 1_000_000.0);

    // Demo 6: Click handling
    let panel5 = PanelCapsule::new()
        .with_title("Interactive Panel")
        .with_border(BorderStyle::Thick, 0xFFFF00FF)
        .with_collapsible();

    println!("\n=== Click Handling ===");
    let bounds = Rect::new(0, 0, 30, 10);

    // Click on collapse button (x=28, y=1 is button location)
    let changed = panel5.handle_click(28, 1, bounds);
    println!("Click at (28, 1): state_changed={}, collapsed={}", changed, panel5.is_collapsed());

    // Click outside button
    let changed = panel5.handle_click(5, 5, bounds);
    println!("Click at (5, 5): state_changed={}, collapsed={}", changed, panel5.is_collapsed());

    // Demo 7: Content bounds calculation
    let panel6 = PanelCapsule::new()
        .with_border(BorderStyle::Solid, 0xFFFFFFFF)
        .with_padding(2, 2, 1, 1);

    let outer = Rect::new(0, 0, 40, 20);
    let content = panel6.content_bounds(outer);
    println!("\n=== Layout ===");
    println!("Outer bounds: {:?}", outer);
    println!("Content bounds: {:?}", content);
    println!("Border + padding consumed: {}x{} pixels",
        outer.width - content.width,
        outer.height - content.height
    );

    println!("\n✅ All demonstrations complete!");
}
