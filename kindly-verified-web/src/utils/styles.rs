/// Byzantine Purple & Gold Design System
/// Ported from kindly-web with enhancements for image detection UI
///
/// Core Philosophy:
/// - Byzantine Royal Purple (#663399) as primary
/// - Metallic Gold (#FFD700) as accent
/// - Glassmorphism for depth and premium feel
/// - 8px grid system for consistent spacing

// ============================================================================
// COLOR PALETTE
// ============================================================================

/// Primary Byzantine purple
pub const COLOR_PURPLE: &str = "#663399";
/// Secondary darker purple
pub const COLOR_PURPLE_DARK: &str = "#4B0082";
/// Light purple for highlights
pub const COLOR_PURPLE_LIGHT: &str = "#8B5CF6";
/// Metallic gold accent
pub const COLOR_GOLD: &str = "#FFD700";
/// Dark gold for depth
pub const COLOR_GOLD_DARK: &str = "#DAA520";
/// Background deep purple
pub const COLOR_BG_DARK: &str = "#1a0033";
/// Background mid purple
pub const COLOR_BG_MID: &str = "#2d1b4e";
/// Success green
pub const COLOR_SUCCESS: &str = "#10B981";
/// Warning orange
pub const COLOR_WARNING: &str = "#F59E0B";
/// Error red
pub const COLOR_ERROR: &str = "#EF4444";
/// Neutral gray
pub const COLOR_NEUTRAL: &str = "#9CA3AF";

// ============================================================================
// GRADIENTS
// ============================================================================

/// Hero gradient (purple to dark purple)
pub fn gradient_hero() -> String {
    format!(
        "linear-gradient(135deg, {} 0%, {} 100%)",
        COLOR_BG_DARK, COLOR_BG_MID
    )
}

/// Gold gradient for premium elements
pub fn gradient_gold() -> String {
    format!(
        "linear-gradient(135deg, {} 0%, {} 100%)",
        COLOR_GOLD, COLOR_GOLD_DARK
    )
}

/// Purple shimmer for hover effects
pub fn gradient_purple_shimmer() -> String {
    format!(
        "linear-gradient(135deg, {} 0%, {} 50%, {} 100%)",
        COLOR_PURPLE_LIGHT, COLOR_PURPLE, COLOR_PURPLE_DARK
    )
}

/// Detection result gradient (green to purple)
pub fn gradient_detection(confidence: f32) -> String {
    let start_color = if confidence > 0.8 {
        COLOR_SUCCESS
    } else if confidence > 0.5 {
        COLOR_WARNING
    } else {
        COLOR_ERROR
    };
    format!(
        "linear-gradient(90deg, {} 0%, {} 100%)",
        start_color, COLOR_PURPLE
    )
}

// ============================================================================
// GLASSMORPHISM
// ============================================================================

#[derive(Clone, Copy, Debug)]
pub enum GlassBlur {
    Light = 8,
    Medium = 16,
    Heavy = 24,
    Ultra = 32,
}

/// Generate glassmorphism CSS
pub fn glassmorphism(blur: GlassBlur, opacity: f32) -> String {
    format!(
        "background: rgba(102, 51, 153, {});
         backdrop-filter: blur({}px);
         -webkit-backdrop-filter: blur({}px);
         border: 1px solid rgba(255, 215, 0, 0.2);
         box-shadow: 0 8px 32px 0 rgba(31, 38, 135, 0.37);",
        opacity,
        blur as u8,
        blur as u8
    )
}

// ============================================================================
// SPACING (8px grid)
// ============================================================================

pub const SPACING_XS: &str = "0.25rem";  // 4px
pub const SPACING_SM: &str = "0.5rem";   // 8px
pub const SPACING_MD: &str = "1rem";     // 16px
pub const SPACING_LG: &str = "1.5rem";   // 24px
pub const SPACING_XL: &str = "2rem";     // 32px
pub const SPACING_2XL: &str = "3rem";    // 48px
pub const SPACING_3XL: &str = "4rem";    // 64px
pub const SPACING_4XL: &str = "6rem";    // 96px
pub const SPACING_5XL: &str = "8rem";    // 128px

// ============================================================================
// BREAKPOINTS
// ============================================================================

#[derive(Clone, Copy, Debug)]
pub enum Breakpoint {
    Xs = 0,     // 0px and up
    Sm = 640,   // 640px and up
    Md = 768,   // 768px and up
    Lg = 1024,  // 1024px and up
    Xl = 1280,  // 1280px and up
}

/// Generate media query string
pub fn media_query(bp: Breakpoint) -> String {
    format!("@media (min-width: {}px)", bp as u32)
}

// ============================================================================
// TYPOGRAPHY
// ============================================================================

pub const FONT_SANS: &str = r#"-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif"#;
pub const FONT_MONO: &str = r#"'SF Mono', Monaco, 'Cascadia Code', 'Courier New', monospace"#;

pub fn text_heading_xl() -> String {
    format!(
        "font-size: 3.5rem;
         font-weight: 800;
         line-height: 1.1;
         letter-spacing: -0.02em;
         background: {};
         -webkit-background-clip: text;
         -webkit-text-fill-color: transparent;
         background-clip: text;",
        gradient_gold()
    )
}

pub fn text_heading_lg() -> String {
    format!(
        "font-size: 2.5rem;
         font-weight: 700;
         line-height: 1.2;
         color: {};",
        COLOR_GOLD
    )
}

pub fn text_heading_md() -> String {
    format!(
        "font-size: 1.75rem;
         font-weight: 600;
         line-height: 1.3;
         color: {};",
        COLOR_GOLD
    )
}

pub fn text_heading_sm() -> String {
    format!(
        "font-size: 1.25rem;
         font-weight: 600;
         line-height: 1.4;
         color: {};",
        COLOR_GOLD
    )
}

pub fn text_body() -> String {
    format!(
        "font-size: 1rem;
         line-height: 1.6;
         color: rgba(255, 255, 255, 0.9);"
    )
}

pub fn text_caption() -> String {
    format!(
        "font-size: 0.875rem;
         line-height: 1.4;
         color: rgba(255, 255, 255, 0.7);"
    )
}

// ============================================================================
// EFFECTS
// ============================================================================

/// Glow effect for gold elements
pub fn glow_gold() -> String {
    format!(
        "box-shadow: 0 0 20px rgba(255, 215, 0, 0.5),
                     0 0 40px rgba(255, 215, 0, 0.3),
                     0 0 60px rgba(255, 215, 0, 0.1);"
    )
}

/// Glow effect for purple elements
pub fn glow_purple() -> String {
    format!(
        "box-shadow: 0 0 20px rgba(102, 51, 153, 0.5),
                     0 0 40px rgba(102, 51, 153, 0.3),
                     0 0 60px rgba(102, 51, 153, 0.1);"
    )
}

/// Hover lift effect
pub fn hover_lift() -> String {
    "transform: translateY(-4px);
     transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);".to_string()
}

// ============================================================================
// BUTTON STYLES
// ============================================================================

pub fn button_primary() -> String {
    format!(
        "{}
         padding: {} {};
         border-radius: 12px;
         font-weight: 600;
         font-size: 1rem;
         cursor: pointer;
         border: 2px solid {};
         transition: all 0.3s ease;
         color: #1a0033;",
        glassmorphism(GlassBlur::Medium, 0.2),
        SPACING_MD,
        SPACING_XL,
        COLOR_GOLD
    )
}

pub fn button_primary_hover() -> String {
    format!("{} {}", hover_lift(), glow_gold())
}

// ============================================================================
// CARD STYLES
// ============================================================================

pub fn card_glass() -> String {
    format!(
        "{}
         border-radius: 24px;
         padding: {};
         transition: all 0.3s ease;",
        glassmorphism(GlassBlur::Medium, 0.15),
        SPACING_XL
    )
}

pub fn card_glass_hover() -> String {
    format!("{} {}", hover_lift(), glow_purple())
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Convert percentage to confidence color
pub fn confidence_color(confidence: f32) -> &'static str {
    if confidence >= 0.9 {
        COLOR_SUCCESS
    } else if confidence >= 0.7 {
        COLOR_GOLD
    } else if confidence >= 0.5 {
        COLOR_WARNING
    } else {
        COLOR_ERROR
    }
}

/// Generate confidence badge style
pub fn confidence_badge(confidence: f32) -> String {
    let color = confidence_color(confidence);
    format!(
        "background: {};
         color: white;
         padding: {} {};
         border-radius: 8px;
         font-weight: 600;
         font-size: 0.875rem;
         display: inline-block;",
        color, SPACING_XS, SPACING_SM
    )
}
