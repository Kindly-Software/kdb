//! Byzantine Purple + Gold Color Palette
//! Matching kindly.software branding

use iced::Color;

// ===== PRIMARY BACKGROUNDS (More Purple Tint) =====
pub const BG_DARK: Color = Color::from_rgb(0.141, 0.106, 0.220); // #24 1B38 (more purple!)
pub const PANEL_BG: Color = Color::from_rgb(0.200, 0.153, 0.278); // #332747 (vibrant purple)
pub const CARD_BG: Color = Color::from_rgb(0.259, 0.200, 0.337); // #423356 (rich purple)

// ===== BYZANTINE PURPLE SCALE (Increased Saturation) =====
pub const PURPLE_DEEP: Color = Color::from_rgb(0.380, 0.0, 0.620); // #610 09E (deeper, richer)
pub const PURPLE_ROYAL: Color = Color::from_rgb(0.500, 0.200, 0.700); // #8033B3 (MORE vibrant!)
pub const PURPLE_MEDIUM: Color = Color::from_rgb(0.549, 0.275, 0.659); // #8C46A8 (brighter)
pub const PURPLE_LIGHT: Color = Color::from_rgb(0.941, 0.882, 0.988); // #F0E1FC (brighter lavender)

// ===== GOLD ACCENT SCALE =====
pub const GOLD_DARK: Color = Color::from_rgb(0.855, 0.647, 0.125); // #DAA520
pub const GOLD_BRIGHT: Color = Color::from_rgb(1.0, 0.843, 0.0); // #FFD700
pub const GOLD_LIGHT: Color = Color::from_rgb(1.0, 0.929, 0.306); // #FFED4E

// ===== NEUTRAL TEXT =====
pub const TEXT_PRIMARY: Color = Color::from_rgb(0.961, 0.961, 0.969); // #F5F5F7
pub const TEXT_SECONDARY: Color = Color::from_rgb(0.627, 0.627, 0.690); // #A0A0B0
pub const TEXT_TERTIARY: Color = Color::from_rgb(0.439, 0.439, 0.502); // #707080

// ===== FUNCTIONAL =====
pub const SUCCESS: Color = Color::from_rgb(0.0, 0.816, 0.518); // #00D084
pub const ERROR: Color = Color::from_rgb(1.0, 0.231, 0.188); // #FF3B30
pub const WARNING: Color = Color::from_rgb(1.0, 0.584, 0.0); // #FF9500

/// Create color with alpha channel (0.0 = transparent, 1.0 = opaque)
pub fn with_alpha(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}

/// Linear interpolation between two colors
pub fn lerp_color(from: Color, to: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color {
        r: from.r + (to.r - from.r) * t,
        g: from.g + (to.g - from.g) * t,
        b: from.b + (to.b - from.b) * t,
        a: from.a + (to.a - from.a) * t,
    }
}

/// Purple → Gold gradient for progress bars
pub fn progress_gradient(progress: f32) -> Color {
    lerp_color(PURPLE_ROYAL, GOLD_BRIGHT, progress)
}
