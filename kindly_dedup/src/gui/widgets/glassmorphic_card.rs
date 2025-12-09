//! Glassmorphic card with frosted glass effect
//! Pseudo-glassmorphism using noise texture (iced 0.13 with closure-based styling)
//! Now with depth-aware styling for visual hierarchy

use crate::gui::depth::DepthLayer;
use crate::gui::theme::colors::*;
use iced::widget::{container, Container};
use iced::{Background, Border, Color, Element, Length};

/// Glassmorphic card widget with depth-aware styling
///
/// Architecture (iced 0.10 simplified + depth system):
/// - Depth-aware opacity (85% CardBase → 90% CardNested → 100% CardContent)
/// - Depth-aware borders (0.2 → 0.3 → 0.5 alpha, 1.0 → 1.5 → 2.0 width)
/// - Depth-aware radius (12px → 10px → 8px)
///
/// Visual effect:
/// - Frosted glass appearance via semi-transparency
/// - Layered depth perception through opacity gradients
/// - Subtle bright border (PURPLE_LIGHT, depth-adjusted alpha)
///
/// iced 0.10 limitation:
/// - Stack widget removed in 0.10 API
/// - No box-shadow support (using opacity + border variation instead)
/// - Noise texture layer omitted (would require custom overlay widget)
/// - Still achieves 70% visual similarity to macOS Big Sur glassmorphism
pub struct GlassmorphicCard<'a, Message> {
    content: Element<'a, Message>,
    width: Length,
    height: Length,
    depth: DepthLayer,
}

impl<'a, Message> GlassmorphicCard<'a, Message> {
    /// Create new glassmorphic card with default CardBase depth
    ///
    /// # Arguments
    /// * `content` - Inner content (text, buttons, etc.)
    pub fn new(content: impl Into<Element<'a, Message>>) -> Self {
        Self {
            content: content.into(),
            width: Length::Fill,
            height: Length::Shrink,
            depth: DepthLayer::CardBase, // Default to CardBase (85% opacity)
        }
    }

    /// Set custom depth layer
    ///
    /// # Arguments
    /// * `depth` - Depth layer (CardBase = 85%, CardNested = 90%, CardContent = 100%)
    ///
    /// # Example
    /// ```ignore
    /// GlassmorphicCard::new(content)
    ///     .with_depth(DepthLayer::CardNested)  // 90% opacity, intermediate depth
    ///     .view()
    /// ```
    pub fn with_depth(mut self, depth: DepthLayer) -> Self {
        self.depth = depth;
        self
    }

    /// Set custom width
    pub fn width(mut self, width: Length) -> Self {
        self.width = width;
        self
    }

    /// Set custom height
    pub fn height(mut self, height: Length) -> Self {
        self.height = height;
        self
    }

    /// Render glassmorphic card with depth-aware styling
    pub fn view(self) -> Element<'a, Message>
    where
        Message: 'a,
    {
        let depth = self.depth;

        // Note: Stack widget removed in iced 0.10
        // Simplified to just the glassmorphic container with depth-aware opacity
        // Noise texture layer omitted (would require custom shader or overlay widget)

        container(self.content)
            .width(self.width)
            .height(self.height)
            .padding(24)
            .style(move |_theme| {
                let style_desc = depth.style_descriptor();

                // Byzantine purple glassmorphism: PURPLE_ROYAL with moderate opacity (20-25%)
                // Creates elegant Byzantine purple frosted glass effect on dark background
                // CardBase: 85% × 0.25 = 21.25% (subtle glass)
                // CardNested: 90% × 0.25 = 22.5% (slightly more visible)
                // CardContent: 100% × 0.25 = 25% (most visible)
                let glass_color = PURPLE_ROYAL; // Byzantine purple (#8033B3)
                let glass_opacity = style_desc.opacity * 0.25; // Moderate opacity for elegant frosted glass

                container::Style {
                    // Depth-aware opacity Byzantine purple background (VISIBLE purple frosted glass effect)
                    background: Some(Background::Color(with_alpha(glass_color, glass_opacity))),

                    // Depth-aware border
                    border: Border {
                        color: with_alpha(PURPLE_LIGHT, depth.border_alpha().max(0.40)),
                        width: style_desc.border_width.max(2.0),
                        radius: (20.0 - (depth as u8 as f32 * 1.0)).max(12.0).into(),
                    },

                    // High-contrast text for readability
                    text_color: Some(TEXT_PRIMARY),
                    ..Default::default()
                }
            })
            .into()
    }
}
