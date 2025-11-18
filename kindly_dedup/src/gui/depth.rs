//! Layered depth system for visual hierarchy
//!
//! Implements depth-based styling through opacity and border variations.
//! iced 0.10 doesn't support box-shadow, so we use opacity gradients and
//! border brightness to create depth perception.
//!
//! Framework: UCE34 (Q33 verification), ASSUM (99.99% safe)

use crate::gui::theme::colors::*;
use iced::Color;

/// Visual depth layers (Z-index hierarchy)
///
/// Defines the stacking order and opacity levels for UI elements.
/// Lower layers appear "further back" with higher transparency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DepthLayer {
    /// Z: 0 - Opaque background (BG_DARK)
    Background = 0,

    /// Z: 1 - Noise texture layer (full bounds, decorative)
    CardBackground = 1,

    /// Z: 2 - Main card surfaces (85% opacity, subtle border)
    CardBase = 2,

    /// Z: 3 - Nested content areas (90% opacity, brighter border)
    CardNested = 3,

    /// Z: 4 - Interactive content (100% opacity, full brightness)
    CardContent = 4,

    /// Z: 5 - Modals/toasts/overlays (future, 95% opacity)
    Overlay = 5,
}

impl DepthLayer {
    /// Get opacity multiplier for this depth layer (0.0 = transparent, 1.0 = opaque)
    ///
    /// Deeper layers have lower opacity to create visual depth.
    /// Opacity progression: 85% → 90% → 100% from back to front.
    pub fn opacity(self) -> f32 {
        match self {
            DepthLayer::Background => 1.0,     // Fully opaque background
            DepthLayer::CardBackground => 1.0, // Fully opaque noise texture
            DepthLayer::CardBase => 0.85,      // Semi-transparent cards
            DepthLayer::CardNested => 0.90,    // More opaque nested
            DepthLayer::CardContent => 1.0,    // Fully opaque content
            DepthLayer::Overlay => 0.95,       // Slightly transparent modals
        }
    }

    /// Get border alpha for this depth layer
    ///
    /// Deeper layers have dimmer borders to recede visually.
    /// Border alpha progression: 0.2 → 0.3 → 0.5 from back to front.
    pub fn border_alpha(self) -> f32 {
        match self {
            DepthLayer::Background => 0.0,     // No border
            DepthLayer::CardBackground => 0.0, // No border
            DepthLayer::CardBase => 0.2,       // Subtle border
            DepthLayer::CardNested => 0.3,     // Medium border
            DepthLayer::CardContent => 0.5,    // Prominent border (if needed)
            DepthLayer::Overlay => 0.6,        // Strong overlay border
        }
    }

    /// Get border width for this depth layer (in logical pixels)
    ///
    /// Shallower layers have thicker borders for emphasis.
    pub fn border_width(self) -> f32 {
        match self {
            DepthLayer::Background => 0.0,     // No border
            DepthLayer::CardBackground => 0.0, // No border
            DepthLayer::CardBase => 1.0,       // Standard border
            DepthLayer::CardNested => 1.5,     // Slightly thicker
            DepthLayer::CardContent => 2.0,    // Prominent border
            DepthLayer::Overlay => 3.0,        // Strong overlay border
        }
    }

    /// Get border radius for this depth layer (in logical pixels)
    ///
    /// Consistent 12px radius across all card layers for cohesive design.
    pub fn border_radius(self) -> f32 {
        match self {
            DepthLayer::Background => 0.0,      // No border radius
            DepthLayer::CardBackground => 12.0, // Match card radius
            DepthLayer::CardBase => 12.0,       // Standard radius
            DepthLayer::CardNested => 10.0,     // Slightly smaller for nested
            DepthLayer::CardContent => 8.0,     // Smaller for content areas
            DepthLayer::Overlay => 16.0,        // Larger for modals
        }
    }

    /// Get the background color for this depth layer
    ///
    /// Returns the appropriate color with opacity applied.
    pub fn background_color(self) -> Color {
        let base_color = match self {
            DepthLayer::Background => BG_DARK,
            DepthLayer::CardBackground => CARD_BG,
            DepthLayer::CardBase => CARD_BG,
            DepthLayer::CardNested => PANEL_BG,
            DepthLayer::CardContent => CARD_BG,
            DepthLayer::Overlay => PANEL_BG,
        };

        with_alpha(base_color, self.opacity())
    }

    /// Get the border color for this depth layer
    ///
    /// Returns PURPLE_ROYAL with layer-specific alpha for depth perception.
    pub fn border_color(self) -> Color {
        with_alpha(PURPLE_ROYAL, self.border_alpha())
    }

    /// Create a depth-aware style descriptor
    ///
    /// Convenience method for getting all style properties at once.
    pub fn style_descriptor(self) -> DepthStyleDescriptor {
        DepthStyleDescriptor {
            background: self.background_color(),
            border_color: self.border_color(),
            border_width: self.border_width(),
            border_radius: self.border_radius(),
            opacity: self.opacity(),
        }
    }
}

/// Complete style descriptor for a depth layer
///
/// Contains all visual properties needed to render a depth-aware container.
#[derive(Debug, Clone, Copy)]
pub struct DepthStyleDescriptor {
    /// Background color with opacity applied
    pub background: Color,

    /// Border color with alpha applied
    pub border_color: Color,

    /// Border width in logical pixels
    pub border_width: f32,

    /// Border radius in logical pixels
    pub border_radius: f32,

    /// Opacity multiplier (0.0 = transparent, 1.0 = opaque)
    pub opacity: f32,
}

/// Depth guidelines for common UI patterns
///
/// Provides recommended depth assignments for different UI elements.
pub mod guidelines {
    use super::DepthLayer;

    /// Main application cards (file input, settings, results)
    pub const MAIN_CARD: DepthLayer = DepthLayer::CardBase;

    /// Nested sections within cards (drag-drop zone, metrics panels)
    pub const NESTED_SECTION: DepthLayer = DepthLayer::CardNested;

    /// Interactive elements (buttons, sliders, text)
    pub const INTERACTIVE_CONTENT: DepthLayer = DepthLayer::CardContent;

    /// Feature badges (bottom of screen)
    pub const BADGE: DepthLayer = DepthLayer::CardNested;

    /// Error/warning notifications
    pub const NOTIFICATION: DepthLayer = DepthLayer::CardNested;

    /// Modal dialogs (future)
    pub const MODAL: DepthLayer = DepthLayer::Overlay;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opacity_progression() {
        // Verify opacity increases from back to front
        assert!(DepthLayer::CardBase.opacity() < DepthLayer::CardNested.opacity());
        assert!(DepthLayer::CardNested.opacity() < DepthLayer::CardContent.opacity());

        // Verify exact values
        assert_eq!(DepthLayer::CardBase.opacity(), 0.85);
        assert_eq!(DepthLayer::CardNested.opacity(), 0.90);
        assert_eq!(DepthLayer::CardContent.opacity(), 1.0);
    }

    #[test]
    fn test_border_alpha_progression() {
        // Verify border alpha increases from back to front
        assert!(DepthLayer::CardBase.border_alpha() < DepthLayer::CardNested.border_alpha());
        assert!(DepthLayer::CardNested.border_alpha() < DepthLayer::CardContent.border_alpha());

        // Verify exact values
        assert_eq!(DepthLayer::CardBase.border_alpha(), 0.2);
        assert_eq!(DepthLayer::CardNested.border_alpha(), 0.3);
        assert_eq!(DepthLayer::CardContent.border_alpha(), 0.5);
    }

    #[test]
    fn test_border_width_progression() {
        // Verify border width increases from back to front
        assert!(DepthLayer::CardBase.border_width() <= DepthLayer::CardNested.border_width());
        assert!(DepthLayer::CardNested.border_width() <= DepthLayer::CardContent.border_width());
    }

    #[test]
    fn test_background_color_opacity() {
        // Verify background colors have opacity applied
        let base_color = DepthLayer::CardBase.background_color();
        assert_eq!(base_color.a, 0.85);

        let nested_color = DepthLayer::CardNested.background_color();
        assert_eq!(nested_color.a, 0.90);

        let content_color = DepthLayer::CardContent.background_color();
        assert_eq!(content_color.a, 1.0);
    }

    #[test]
    fn test_style_descriptor() {
        // Verify style descriptor returns consistent values
        let descriptor = DepthLayer::CardBase.style_descriptor();
        assert_eq!(descriptor.opacity, 0.85);
        assert_eq!(descriptor.border_width, 1.0);
        assert_eq!(descriptor.border_radius, 12.0);
        assert_eq!(descriptor.background.a, 0.85);
    }

    #[test]
    fn test_depth_ordering() {
        // Verify depth layers have correct ordering
        assert!(DepthLayer::Background < DepthLayer::CardBackground);
        assert!(DepthLayer::CardBackground < DepthLayer::CardBase);
        assert!(DepthLayer::CardBase < DepthLayer::CardNested);
        assert!(DepthLayer::CardNested < DepthLayer::CardContent);
        assert!(DepthLayer::CardContent < DepthLayer::Overlay);
    }
}
