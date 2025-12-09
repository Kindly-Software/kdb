//! Byzantine Royal Purple + Gold Design System
//!
//! Color constants extracted from kindly.services design language.
//! Optimized for dark backgrounds with high contrast accessibility.

// ============================================================================
// Byzantine Purple Spectrum
// ============================================================================

/// Primary Byzantine Purple (main brand color)
pub const PURPLE_BYZANTINE: &str = "#4B0082";

/// Rebecca Purple (secondary purple)
pub const PURPLE_REBECCA: &str = "#663399";

/// Dark purple accent
pub const PURPLE_DARK: &str = "#2D004D";

/// Deep background (near-black)
pub const BG_DEEP: &str = "#0a0014";

// ============================================================================
// Gold Accents
// ============================================================================

/// Primary gold (shimmer effect, accents)
pub const GOLD_PRIMARY: &str = "#FFD700";

/// Secondary gold (orange-gold gradient)
pub const GOLD_SECONDARY: &str = "#FFA500";

// ============================================================================
// Glassmorphism System
// ============================================================================

/// Glass card background
pub const GLASS_BG: &str = "rgba(255, 255, 255, 0.05)";

/// Glass card border
pub const GLASS_BORDER: &str = "rgba(255, 255, 255, 0.1)";

/// Glass blur strength
pub const GLASS_BLUR: &str = "20px";

/// Glass card border radius
pub const GLASS_RADIUS: &str = "24px";

// ============================================================================
// Typography Colors
// ============================================================================

/// Primary text (white)
pub const TEXT_PRIMARY: &str = "#fff";

/// Secondary text (70% opacity)
pub const TEXT_SECONDARY: &str = "rgba(255, 255, 255, 0.7)";

/// Muted text (60% opacity)
pub const TEXT_MUTED: &str = "rgba(255, 255, 255, 0.6)";

/// Code text (slightly blue-tinted)
pub const TEXT_CODE: &str = "#e2e8f0";

// ============================================================================
// Gradients
// ============================================================================

/// Main background gradient (CSS)
pub const BG_GRADIENT: &str = "linear-gradient(135deg, #3b0764 0%, #2D004D 50%, #0a0014 100%)";

/// Gold shimmer gradient (animated)
pub const GOLD_SHIMMER_GRADIENT: &str = "linear-gradient(90deg, #FFD700 0%, #fff 25%, #FFD700 50%, #fff 75%, #FFD700 100%)";

/// Primary button gradient
pub const BUTTON_GRADIENT: &str = "linear-gradient(135deg, #FFD700, #FFA500)";

// ============================================================================
// Spacing (rem units)
// ============================================================================

pub const SPACE_XS: &str = "0.25rem";
pub const SPACE_SM: &str = "0.5rem";
pub const SPACE_MD: &str = "1rem";
pub const SPACE_LG: &str = "1.5rem";
pub const SPACE_XL: &str = "2rem";
pub const SPACE_2XL: &str = "3rem";
pub const SPACE_3XL: &str = "4rem";

// ============================================================================
// Typography (font families)
// ============================================================================

/// Heading font
pub const FONT_HEADING: &str = "'Space Grotesk', sans-serif";

/// Body font
pub const FONT_BODY: &str = "'Inter', -apple-system, BlinkMacSystemFont, sans-serif";

/// Code font
pub const FONT_CODE: &str = "'JetBrains Mono', 'Fira Code', monospace";
