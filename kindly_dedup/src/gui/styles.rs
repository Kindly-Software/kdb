//! Iced 0.13 style functions for kindly_dedup GUI

use iced::widget::{button, container, slider};
use iced::{Border, Color, Shadow, Theme, Vector};

use super::theme::colors::*;

/// Helper to create transparent colors
pub fn with_alpha(color: Color, alpha: f32) -> Color {
    Color {
        a: alpha,
        ..color
    }
}

/// Purple button style
pub fn purple_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Active => button::Style {
            background: Some(iced::Background::Color(PURPLE_ROYAL)),
            border: Border {
                color: PURPLE_MEDIUM,
                width: 2.0,
                radius: 8.0.into(),
            },
            text_color: TEXT_PRIMARY,
            ..Default::default()
        },
        button::Status::Hovered => button::Style {
            background: Some(iced::Background::Color(with_alpha(PURPLE_ROYAL, 0.5))),
            border: Border {
                color: GOLD_BRIGHT,
                width: 3.0,
                radius: 12.0.into(),
            },
            text_color: Color::WHITE,
            shadow: Shadow {
                offset: Vector::new(0.0, 4.0),
                ..Default::default()
            },
            ..Default::default()
        },
        button::Status::Pressed => button::Style {
            background: Some(iced::Background::Color(PURPLE_DEEP)),
            border: Border {
                color: PURPLE_DEEP,
                width: 2.0,
                radius: 8.0.into(),
            },
            text_color: TEXT_PRIMARY,
            ..Default::default()
        },
        _ => button::Style::default(),
    }
}

/// Gold button style (with enabled state)
pub fn gold_button_style(enabled: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        if enabled {
            match status {
                button::Status::Active => button::Style {
                    background: Some(iced::Background::Color(with_alpha(GOLD_BRIGHT, 0.4))),
                    border: Border {
                        color: with_alpha(Color::WHITE, 0.3),
                        width: 2.0,
                        radius: 12.0.into(),
                    },
                    text_color: Color::BLACK,
                    shadow: Shadow {
                        offset: Vector::new(0.0, 6.0),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                button::Status::Hovered => button::Style {
                    background: Some(iced::Background::Color(with_alpha(GOLD_BRIGHT, 0.6))),
                    border: Border {
                        color: with_alpha(Color::WHITE, 0.5),
                        width: 3.0,
                        radius: 12.0.into(),
                    },
                    text_color: Color::BLACK,
                    shadow: Shadow {
                        offset: Vector::new(0.0, 8.0),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                button::Status::Pressed => button::Style {
                    background: Some(iced::Background::Color(GOLD_DARK)),
                    border: Border {
                        color: GOLD_DARK,
                        width: 2.0,
                        radius: 12.0.into(),
                    },
                    text_color: Color::BLACK,
                    ..Default::default()
                },
                _ => button::Style::default(),
            }
        } else {
            button::Style {
                background: Some(iced::Background::Color(with_alpha(GOLD_BRIGHT, 0.2))),
                border: Border {
                    color: with_alpha(GOLD_DARK, 0.3),
                    width: 2.0,
                    radius: 12.0.into(),
                },
                text_color: TEXT_TERTIARY,
                ..Default::default()
            }
        }
    }
}

/// Badge button style
pub fn badge_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Active => button::Style {
            background: Some(iced::Background::Color(with_alpha(PURPLE_DEEP, 0.4))),
            border: Border {
                color: with_alpha(PURPLE_ROYAL, 0.6),
                width: 2.0,
                radius: 12.0.into(),
            },
            text_color: TEXT_PRIMARY,
            ..Default::default()
        },
        button::Status::Hovered => button::Style {
            background: Some(iced::Background::Color(with_alpha(GOLD_BRIGHT, 0.4))),
            border: Border {
                color: with_alpha(Color::WHITE, 0.3),
                width: 2.0,
                radius: 12.0.into(),
            },
            text_color: Color::BLACK,
            shadow: Shadow {
                offset: Vector::new(0.0, 6.0),
                ..Default::default()
            },
            ..Default::default()
        },
        _ => button::Style::default(),
    }
}

/// Error button style
pub fn error_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Active => button::Style {
            background: Some(iced::Background::Color(with_alpha(PURPLE_DEEP, 0.3))),
            border: Border {
                color: PURPLE_ROYAL,
                width: 4.0,
                radius: 12.0.into(),
            },
            text_color: PURPLE_LIGHT,
            ..Default::default()
        },
        button::Status::Hovered => button::Style {
            background: Some(iced::Background::Color(with_alpha(PURPLE_ROYAL, 0.5))),
            border: Border {
                color: GOLD_BRIGHT,
                width: 3.0,
                radius: 12.0.into(),
            },
            text_color: GOLD_BRIGHT,
            shadow: Shadow {
                offset: Vector::new(0.0, 4.0),
                ..Default::default()
            },
            ..Default::default()
        },
        _ => button::Style::default(),
    }
}

/// Link button style
pub fn link_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Active => button::Style {
            background: None,
            border: Border::default(),
            text_color: GOLD_BRIGHT,
            ..Default::default()
        },
        button::Status::Hovered => button::Style {
            background: None,
            border: Border::default(),
            text_color: with_alpha(GOLD_BRIGHT, 0.7),
            ..Default::default()
        },
        _ => button::Style::default(),
    }
}

/// Purple slider style
pub fn purple_slider_style(_theme: &Theme, status: slider::Status) -> slider::Style {
    let base_rail = slider::Rail {
        backgrounds: (iced::Background::Color(PURPLE_DEEP), iced::Background::Color(PURPLE_ROYAL)),
        width: 4.0,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 2.0.into(),
        },
    };
    match status {
        slider::Status::Active => slider::Style {
            rail: base_rail,
            handle: slider::Handle {
                shape: slider::HandleShape::Circle { radius: 8.0 },
                background: iced::Background::Color(with_alpha(GOLD_BRIGHT, 0.5)),
                border_width: 2.0,
                border_color: with_alpha(Color::WHITE, 0.4),
            },
        },
        slider::Status::Hovered => slider::Style {
            rail: base_rail,
            handle: slider::Handle {
                shape: slider::HandleShape::Circle { radius: 10.0 },
                background: iced::Background::Color(with_alpha(GOLD_BRIGHT, 0.6)),
                border_width: 3.0,
                border_color: with_alpha(Color::WHITE, 0.6),
            },
        },
        slider::Status::Dragged => slider::Style {
            rail: base_rail,
            handle: slider::Handle {
                shape: slider::HandleShape::Circle { radius: 9.0 },
                background: iced::Background::Color(with_alpha(GOLD_BRIGHT, 0.7)),
                border_width: 3.0,
                border_color: with_alpha(GOLD_LIGHT, 0.8),
            },
        },
    }
}

/// Modal backdrop container style
pub fn modal_backdrop_style(backdrop_color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(iced::Background::Color(backdrop_color)),
        ..Default::default()
    }
}
