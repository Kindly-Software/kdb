//! Font Atlas Demo
//!
//! Demonstrates the FontAtlasCapsule creation and usage.

use kindly_dedup::gui_v2::render::FontAtlasCapsule;

fn main() {
    println!("=== Font Atlas Demo ===\n");

    println!("Creating font atlas...");
    let atlas = FontAtlasCapsule::new();

    println!("Atlas dimensions: {}x{}", atlas.width(), atlas.height());
    println!("Glyph size: {}x{}", atlas.glyph_size(), atlas.glyph_size());
    println!("Generation: {}", atlas.generation());

    let data = atlas.texture_data();
    println!("Texture data size: {} bytes ({} MB)", data.len(), data.len() / 1024 / 1024);

    // Test UV coordinates for some characters
    println!("\nUV coordinates:");
    for ch in ['A', 'a', '0', '!', ' ', '~'].iter() {
        let (u0, v0, u1, v1) = FontAtlasCapsule::glyph_uv(*ch);
        println!("  '{}': ({:.4}, {:.4}) to ({:.4}, {:.4})", ch, u0, v0, u1, v1);
    }

    // Check that 'A' has visible pixels
    let base_x = 128u32; // 'A' is at index 33, column 1
    let base_y = 256u32; // Row 2
    let margin = 8u32;

    let mut white_pixel_count = 0;
    for y in base_y + margin..base_y + 120 {
        for x in base_x + margin..base_x + 120 {
            let offset = ((y * 2048 + x) * 4) as usize;
            if offset + 3 < data.len() {
                if data[offset] == 255 && data[offset + 1] == 255 && data[offset + 2] == 255 {
                    white_pixel_count += 1;
                }
            }
        }
    }

    println!("\n'A' glyph statistics:");
    println!("  White pixels: {}", white_pixel_count);
    println!("  Expected: 112×112 = {} pixels (full glyph minus 8px margin)", 112*112);

    // Check that space is empty
    let space_x = 0u32;
    let space_y = 0u32;

    let mut space_white_pixels = 0;
    for y in space_y..space_y + 128 {
        for x in space_x..space_x + 128 {
            let offset = ((y * 2048 + x) * 4) as usize;
            if offset + 3 < data.len() {
                if data[offset] != 0 || data[offset + 1] != 0 || data[offset + 2] != 0 || data[offset + 3] != 0 {
                    space_white_pixels += 1;
                }
            }
        }
    }

    println!("\nSpace glyph statistics:");
    println!("  Non-zero pixels: {} (should be 0)", space_white_pixels);

    // Test all printable ASCII characters
    println!("\nVerifying all 95 printable ASCII characters...");
    let mut all_valid = true;
    for ascii in 32..=126 {
        let ch = ascii as u8 as char;
        let (u0, v0, u1, v1) = FontAtlasCapsule::glyph_uv(ch);

        if u0 < 0.0 || u0 >= 1.0 || v0 < 0.0 || v0 >= 1.0 || u1 <= u0 || v1 <= v0 {
            println!("  ERROR: Invalid UV for '{}'", ch);
            all_valid = false;
        }
    }

    if all_valid {
        println!("  All characters have valid UV coordinates ✓");
    }

    println!("\n=== Font atlas created successfully! ===");
}
