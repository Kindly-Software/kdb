//! Custom widget styles for Byzantine purple + gold theme
//! Compatible with iced 0.10

use super::colors::*;
use iced::widget::{button, container, progress_bar, slider};
use iced::{Background, Color, Theme};

// ===== BUTTON STYLES =====

/// Gold hero button (MASSIVE, vibrant!)
pub fn gold_hero_button(_theme: &Theme) -> button::Appearance {
    button::Appearance {
        background: Some(Background::Color(GOLD_BRIGHT)),
        border_radius: 16.0.into(), // Large radius for premium feel
        border_width: 4.0,          // THICK border for prominence
        border_color: GOLD_DARK,
        text_color: Color::BLACK,
        ..Default::default()
    }
}

/// Purple secondary button (vibrant!)
pub fn purple_button(_theme: &Theme) -> button::Appearance {
    button::Appearance {
        background: Some(Background::Color(PURPLE_ROYAL)),
        border_radius: 12.0.into(),
        border_width: 3.0,          // Thicker border
        border_color: PURPLE_LIGHT, // Bright purple border!
        text_color: TEXT_PRIMARY,
        ..Default::default()
    }
}

// ===== CONTAINER STYLES =====

/// Card background (vibrant purple with thick borders!)
pub fn card_background(_theme: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(Background::Color(CARD_BG)),
        border_radius: 16.0.into(), // Large radius for premium feel
        border_width: 3.0,          // THICK border for prominence
        border_color: PURPLE_ROYAL, // VIBRANT purple border!
        text_color: Some(TEXT_PRIMARY),
        ..Default::default()
    }
}

// ===== PROGRESS BAR STYLE =====

/// Purple → Gold gradient progress bar
pub fn gradient_progress_bar(_theme: &Theme) -> progress_bar::Appearance {
    progress_bar::Appearance {
        background: Background::Color(with_alpha(PURPLE_DEEP, 0.3)),
        bar: Background::Color(PURPLE_ROYAL),
        border_radius: 6.0.into(),
    }
}

// ===== SLIDER STYLE =====

/// Purple slider with gold handle
pub fn purple_slider(_theme: &Theme) -> slider::Appearance {
    slider::Appearance {
        rail: slider::Rail {
            colors: (PURPLE_DEEP, PURPLE_ROYAL),
            width: 4.0,
            border_radius: 2.0.into(),
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle { radius: 8.0 },
            color: GOLD_BRIGHT,
            border_color: GOLD_DARK,
            border_width: 2.0,
        },
    }
}
