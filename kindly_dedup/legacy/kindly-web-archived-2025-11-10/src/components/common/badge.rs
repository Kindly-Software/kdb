use leptos::prelude::*;
use crate::utils::{theme::*, build_style};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BadgeVariant {
    Success,
    Warning,
    Error,
    Info,
    Neutral,
}

#[component]
pub fn Badge(
    #[prop(optional)] variant: BadgeVariant,
    children: Children,
) -> impl IntoView {
    let variant = variant;

    let (bg_color, text_color) = match variant {
        BadgeVariant::Success => (COLOR_SUCCESS, COLOR_WHITE),
        BadgeVariant::Warning => (COLOR_WARNING, COLOR_BLACK),
        BadgeVariant::Error => (COLOR_ERROR, COLOR_WHITE),
        BadgeVariant::Info => (COLOR_INFO, COLOR_WHITE),
        BadgeVariant::Neutral => (COLOR_GRAY, COLOR_WHITE),
    };

    let padding_value = format!("{} {}", SPACING_1, SPACING_2);
    let font_weight = "600"; // FONT_WEIGHT_SEMIBOLD as string

    let styles = vec![
        ("font-family", FONT_FAMILY_PRIMARY),
        ("font-size", FONT_SIZE_XS),
        ("font-weight", font_weight),
        ("color", text_color),
        ("background-color", bg_color),
        ("padding", padding_value.as_str()),
        ("border-radius", RADIUS_FULL),
        ("display", "inline-flex"),
        ("align-items", "center"),
        ("justify-content", "center"),
        ("text-transform", "uppercase"),
        ("letter-spacing", "0.05em"),
    ];

    let style_string = build_style(&styles);

    view! {
        <span style=style_string>
            {children()}
        </span>
    }
}

impl Default for BadgeVariant {
    fn default() -> Self {
        BadgeVariant::Neutral
    }
}
