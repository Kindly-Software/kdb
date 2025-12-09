//! Byzantine purple + gold theme system
//! Compatible with iced 0.13
//!
//! ## Migration Notes (0.10 → 0.13)
//!
//! ### Color Module
//! - No changes required - `iced::Color` API is compatible
//!
//! ### Styles Module
//! - `Appearance` types renamed to `Style`
//! - StyleSheet traits removed - use closure-based styling
//! - Style functions now return closures: `impl Fn(&Theme, Status) -> Style`
//!
//! ### Usage Example
//!
//! ```rust,ignore
//! // OLD (iced 0.10):
//! button(text).style(theme::styles::gold_hero_button)
//!
//! // NEW (iced 0.13):
//! button(text).style(theme::styles::gold_hero_button())
//! //                                              ^^^ note the function call
//! ```

pub mod colors;
pub mod styles;

use iced::Theme;

/// Custom Byzantine theme
/// Returns the Dark theme as base (customization via style closures)
pub fn byzantine_theme() -> Theme {
    Theme::Dark
}
