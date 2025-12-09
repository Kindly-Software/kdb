//! Font Atlas Smoke Test
//!
//! Verifies the FontAtlasCapsule can be instantiated and used.

use kindly_dedup::gui_v2::render::FontAtlasCapsule;

#[test]
fn test_font_atlas_creation() {
    let atlas = FontAtlasCapsule::new();
    assert_eq!(atlas.width(), 2048);
    assert_eq!(atlas.height(), 2048);
    assert_eq!(atlas.glyph_size(), 128);
}

#[test]
fn test_font_atlas_texture_data() {
    let atlas = FontAtlasCapsule::new();
    let data = atlas.texture_data();
    assert_eq!(data.len(), 2048 * 2048 * 4); // 16MB RGBA
}

#[test]
fn test_font_atlas_glyph_uv() {
    // Test space character (first glyph)
    let (u0, v0, u1, v1) = FontAtlasCapsule::glyph_uv(' ');
    assert_eq!(u0, 0.0);
    assert_eq!(v0, 0.0);
    assert!((u1 - 0.0625).abs() < 0.001); // 128/2048

    // Test 'A' character
    let (u0, v0, u1, v1) = FontAtlasCapsule::glyph_uv('A');
    assert!((u0 - 0.0625).abs() < 0.001); // 128/2048
    assert!((v0 - 0.125).abs() < 0.001);  // 256/2048
}

#[test]
fn test_font_atlas_has_visible_pixels() {
    let atlas = FontAtlasCapsule::new();
    let data = atlas.texture_data();

    // Check that 'A' glyph has white pixels
    let base_x = 128u32;
    let base_y = 256u32;
    let margin = 8u32;

    let mut found_white = false;
    for y in base_y + margin..base_y + 120 {
        for x in base_x + margin..base_x + 120 {
            let offset = ((y * 2048 + x) * 4) as usize;
            if offset + 3 < data.len() {
                if data[offset] == 255 && data[offset + 1] == 255 && data[offset + 2] == 255 {
                    found_white = true;
                    break;
                }
            }
        }
        if found_white {
            break;
        }
    }

    assert!(found_white, "'A' glyph should have white pixels");
}

#[test]
fn test_all_printable_ascii_glyphs() {
    // Verify all 95 printable ASCII characters have valid UV coordinates
    for ascii in 32..=126 {
        let ch = ascii as u8 as char;
        let (u0, v0, u1, v1) = FontAtlasCapsule::glyph_uv(ch);

        assert!(u0 >= 0.0 && u0 <= 1.0, "Invalid u0 for '{}'", ch);
        assert!(v0 >= 0.0 && v0 <= 1.0, "Invalid v0 for '{}'", ch);
        assert!(u1 > u0, "u1 <= u0 for '{}'", ch);
        assert!(v1 > v0, "v1 <= v0 for '{}'", ch);
    }
}
