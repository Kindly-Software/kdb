//! Custom widget styles for Byzantine purple + gold theme
//! Compatible with iced 0.13
//!
//! Iced 0.13 Migration Notes:
//! - `Appearance` types renamed to `Style`
//! - StyleSheet traits removed - use closure-based styling
//! - Functions now return closures that produce styles based on theme/status

use super::colors::*;
use iced::widget::{button, container, progress_bar, slider};
use iced::{Background, Border, Color, Theme};

// ===== BUTTON STYLES =====

/// Gold hero button (MASSIVE, vibrant!)
/// Returns a closure for iced 0.13 closure-based styling
pub fn gold_hero_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let base = button::Style {
            background: Some(Background::Color(GOLD_BRIGHT)),
            border: Border {
                color: GOLD_DARK,
                width: 4.0,          // THICK border for prominence
                radius: 16.0.into(), // Large radius for premium feel
            },
            text_color: Color::BLACK,
            ..Default::default()
        };

        match status {
            button::Status::Active => base,
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(GOLD_LIGHT)),
                border: Border {
                    color: GOLD_BRIGHT,
                    width: 4.0,
                    radius: 16.0.into(),
                },
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(GOLD_DARK)),
                ..base
            },
            button::Status::Disabled => button::Style {
                background: Some(Background::Color(with_alpha(GOLD_BRIGHT, 0.5))),
                text_color: with_alpha(Color::BLACK, 0.5),
                ..base
            },
        }
    }
}

/// Purple secondary button (vibrant!)
/// Returns a closure for iced 0.13 closure-based styling
pub fn purple_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let base = button::Style {
            background: Some(Background::Color(PURPLE_ROYAL)),
            border: Border {
                color: PURPLE_LIGHT, // Bright purple border!
                width: 3.0,          // Thicker border
                radius: 12.0.into(),
            },
            text_color: TEXT_PRIMARY,
            ..Default::default()
        };

        match status {
            button::Status::Active => base,
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(PURPLE_MEDIUM)),
                border: Border {
                    color: GOLD_BRIGHT,
                    width: 3.0,
                    radius: 12.0.into(),
                },
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(PURPLE_DEEP)),
                ..base
            },
            button::Status::Disabled => button::Style {
                background: Some(Background::Color(with_alpha(PURPLE_ROYAL, 0.5))),
                text_color: with_alpha(TEXT_PRIMARY, 0.5),
                ..base
            },
        }
    }
}

// ===== CONTAINER STYLES =====

/// Card background (vibrant purple with thick borders!)
/// Returns a closure for iced 0.13 closure-based styling
pub fn card_background() -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(CARD_BG)),
        border: Border {
            color: PURPLE_ROYAL, // VIBRANT purple border!
            width: 3.0,          // THICK border for prominence
            radius: 16.0.into(), // Large radius for premium feel
        },
        text_color: Some(TEXT_PRIMARY),
        ..Default::default()
    }
}

// ===== PROGRESS BAR STYLE =====

/// Purple → Gold gradient progress bar
/// Returns a closure for iced 0.13 closure-based styling
pub fn gradient_progress_bar() -> impl Fn(&Theme) -> progress_bar::Style {
    move |_theme| progress_bar::Style {
        background: Background::Color(with_alpha(PURPLE_DEEP, 0.3)),
        bar: Background::Color(PURPLE_ROYAL),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 6.0.into(),
        },
    }
}

// ===== SLIDER STYLE =====

/// Purple slider with gold handle
/// Returns a closure for iced 0.13 closure-based styling
pub fn purple_slider() -> impl Fn(&Theme, slider::Status) -> slider::Style {
    move |_theme, _status| slider::Style {
        rail: slider::Rail {
            backgrounds: (Background::Color(PURPLE_DEEP), Background::Color(PURPLE_ROYAL)),
            width: 4.0,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 2.0.into(),
            },
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle { radius: 8.0 },
            background: Background::Color(GOLD_BRIGHT),
            border_width: 2.0,
            border_color: GOLD_DARK,
        },
    }
}
