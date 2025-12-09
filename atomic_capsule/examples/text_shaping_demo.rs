//! TextShapingCapsule Demo
//!
//! Demonstrates simple text shaping capabilities for kindly-gui framework.

use atomic_capsule::gui::text::shaping::TextShapingCapsule;

fn main() {
    println!("=== TextShapingCapsule Demo ===\n");

    // Create shaping capsule (font ID 1, 16pt)
    let mut capsule = TextShapingCapsule::new(1, 16.0);
    println!("Created capsule:");
    println!("  Font ID: {}", capsule.font_id());
    println!("  Font size: {:.2}pt", capsule.font_size());
    println!("  Max glyphs: {}\n", TextShapingCapsule::MAX_GLYPHS);

    // Shape simple text
    println!("--- Shaping 'Hello World' ---");
    let count = capsule.shape_text("Hello World");
    println!("Shaped {} glyphs", count);
    println!("Line count: {}", capsule.line_count());
    println!("Generation: {}\n", capsule.generation());

    // Examine glyphs
    println!("Glyphs:");
    for (i, glyph) in capsule.glyphs().iter().enumerate() {
        let ch = char::from_u32(glyph.codepoint).unwrap_or('?');
        println!(
            "  [{}] '{}' (U+{:04X}): x_advance={:.2}px, cluster={}{}",
            i,
            ch,
            glyph.codepoint,
            glyph.x_advance_f32(),
            glyph.cluster,
            if glyph.is_whitespace() {
                " [WHITESPACE]"
            } else {
                ""
            }
        );
    }

    // Get total advance
    let (total_x, total_y) = capsule.total_advance();
    println!("\nTotal advance: ({:.2}, {:.2}) pixels", total_x, total_y);

    // Measure text without full shaping
    println!("\n--- Text Measurement ---");
    let texts = ["Hello", "World", "The quick brown fox"];
    for text in &texts {
        let (w, h) = TextShapingCapsule::measure_text(text, 1, 16.0);
        println!("'{}': {}×{} pixels", text, w, h);
    }

    // Multiline text
    println!("\n--- Multiline Text ---");
    capsule.clear();
    let multiline = "Line 1\nLine 2\nLine 3";
    let count = capsule.shape_text(multiline);
    println!("Shaped '{}': {} glyphs", multiline, count);
    println!("Lines: {}", capsule.line_count());

    let (total_x, total_y) = capsule.total_advance();
    println!("Total advance: ({:.2}, {:.2}) pixels", total_x, total_y);

    // Show line breaks
    println!("\nLine breaks:");
    for (i, glyph) in capsule.glyphs().iter().enumerate() {
        if glyph.is_line_break() {
            println!("  Glyph {} is a line break", i);
        }
    }

    // Max glyphs test
    println!("\n--- Max Glyphs Test ---");
    capsule.clear();
    let long_text = "a".repeat(100);
    let count = capsule.shape_text(&long_text);
    println!(
        "Shaped {} chars, got {} glyphs (max: {})",
        long_text.len(),
        count,
        TextShapingCapsule::MAX_GLYPHS
    );

    // Fixed-point precision demo
    println!("\n--- Fixed-Point Precision ---");
    capsule.clear();
    capsule.shape_text("Test");
    let glyphs = capsule.glyphs();
    if !glyphs.is_empty() {
        let glyph = &glyphs[0];
        println!("First glyph 'T':");
        println!("  x_advance (raw Q8.8): {}", glyph.x_advance);
        println!("  x_advance (f32): {:.4}px", glyph.x_advance_f32());
        println!("  y_advance (raw Q8.8): {}", glyph.y_advance);
        println!("  y_advance (f32): {:.4}px", glyph.y_advance_f32());
    }

    println!("\n=== Demo Complete ===");
}
